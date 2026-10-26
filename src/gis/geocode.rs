//! Address search: PDOK Locatieserver (Dutch addresses, parcels, roads) and
//! OpenStreetMap Nominatim (the rest of the world).
//!
//! [`suggest`] is the type-ahead search (PDOK only: Nominatim's usage policy
//! forbids autocomplete); [`search`] the full search on Enter.

use super::http::{self, encode};

#[derive(Clone, Debug)]
pub struct Hit {
    /// One line: name and place, for the site description.
    pub label: String,
    /// Main line: "Lange Geldersekade 2", "Dordrecht", "Trafalgar Square".
    pub name: String,
    /// Second line: "3311 CK Dordrecht", "Zuid-Holland", "Londen · Verenigd Koninkrijk".
    pub place: String,
    /// What it is, in Dutch: adres, straat, plaats, gemeente, perceel, …
    pub kind: String,
    pub lon: f64,
    pub lat: f64,
    pub source: &'static str,
    pub country: String,
}

impl Hit {
    fn new(name: String, place: String, kind: &str, lon: f64, lat: f64, source: &'static str, country: &str) -> Self {
        let label = if place.is_empty() { name.clone() } else { format!("{name}, {place}") };
        Self { label, name, place, kind: kind.into(), lon, lat, source, country: country.into() }
    }
}

// ── PDOK ────────────────────────────────────────────────────────────────────

fn pdok(q: &str, rows: usize) -> Result<Vec<Hit>, String> {
    pdok_query(&format!("q={}", encode(q)), rows)
}

/// PDOK search within one place (woonplaats): "Grote Kerk Dordrecht" stays in
/// Dordrecht instead of finding the street "Grote Kerk" in Hoorn.
fn pdok_in_place(q: &str, place: &str) -> Result<Vec<Hit>, String> {
    pdok_query(&format!("q={}&fq=woonplaatsnaam:{}", encode(q), encode(place)), 6)
}

fn pdok_query(params: &str, rows: usize) -> Result<Vec<Hit>, String> {
    let url = format!("https://api.pdok.nl/bzk/locatieserver/search/v3_1/free?rows={rows}&fl=weergavenaam,centroide_ll,type&{params}");
    let json = http::get_json(&url)?;
    let docs = json["response"]["docs"].as_array().cloned().unwrap_or_default();
    Ok(docs
        .iter()
        .filter_map(|d| {
            let ll = d["centroide_ll"].as_str()?;
            let inner = ll.strip_prefix("POINT(")?.strip_suffix(')')?;
            let mut it = inner.split_whitespace().filter_map(|v| v.parse::<f64>().ok());
            let (lon, lat) = (it.next()?, it.next()?);
            let typ = d["type"].as_str().unwrap_or("");
            let (name, place) = pdok_parts(d["weergavenaam"].as_str()?, typ);
            Some(Hit::new(name, place, pdok_kind(typ), lon, lat, "PDOK", "nl"))
        })
        .collect())
}

fn pdok_kind(typ: &str) -> &str {
    match typ {
        "adres" => "adres",
        "weg" => "straat",
        "postcode" => "postcode",
        "woonplaats" => "plaats",
        "gemeente" => "gemeente",
        "provincie" => "provincie",
        "perceel" => "perceel",
        "appartementsrecht" => "appartement",
        "hectometerpaal" => "hectometer",
        "waterschap" => "waterschap",
        other => other,
    }
}

/// "Lange Geldersekade 2, 3311CK Dordrecht" → ("Lange Geldersekade 2", "3311 CK Dordrecht").
fn pdok_parts(weergave: &str, typ: &str) -> (String, String) {
    let parts: Vec<&str> = weergave.split(", ").collect();
    let name = parts[0].trim();
    match typ {
        "gemeente" | "provincie" | "waterschap" => {
            let bare = name.strip_prefix("Gemeente ").or_else(|| name.strip_prefix("Provincie ")).unwrap_or(name);
            (bare.to_string(), typ.to_string())
        }
        "perceel" | "appartementsrecht" => (name.to_string(), "kadastraal perceel".into()),
        _ => {
            // Drop repeats ("Dordrecht, Dordrecht, Zuid-Holland").
            let mut rest: Vec<String> = Vec::new();
            for p in &parts[1..] {
                let p = postcode_space(p.trim());
                if p != name && !rest.contains(&p) {
                    rest.push(p);
                }
            }
            (name.to_string(), rest.join(", "))
        }
    }
}

/// "3311CK Dordrecht" → "3311 CK Dordrecht".
fn postcode_space(s: &str) -> String {
    let b = s.as_bytes();
    if b.len() >= 6 && b[..4].iter().all(u8::is_ascii_digit) && b[4..6].iter().all(u8::is_ascii_uppercase) && (b.len() == 6 || b[6] == b' ') {
        format!("{} {}", &s[..4], &s[4..])
    } else {
        s.to_string()
    }
}

// ── Nominatim ───────────────────────────────────────────────────────────────

fn nominatim(q: &str) -> Result<Vec<Hit>, String> {
    let url = format!("https://nominatim.openstreetmap.org/search?format=jsonv2&addressdetails=1&limit=6&accept-language=nl&q={}", encode(q));
    let json = http::get_json(&url)?;
    Ok(json.as_array().cloned().unwrap_or_default().iter().filter_map(nominatim_hit).collect())
}

fn nominatim_hit(r: &serde_json::Value) -> Option<Hit> {
    let a = &r["address"];
    let s = |k: &str| a[k].as_str().unwrap_or("").to_string();
    let display = r["display_name"].as_str()?;
    let addresstype = r["addresstype"].as_str().unwrap_or("");
    let typ = r["type"].as_str().unwrap_or("");
    let mut name = r["name"].as_str().filter(|n| !n.is_empty()).map(str::to_string).unwrap_or_else(|| display.split(", ").next().unwrap_or(display).to_string());
    if matches!(addresstype, "house" | "building") && !s("road").is_empty() && !s("house_number").is_empty() {
        name = format!("{} {}", s("road"), s("house_number"));
    }
    let locality = ["city", "town", "village", "hamlet", "municipality", "suburb"].iter().map(|k| s(k)).find(|v| !v.is_empty() && *v != name).unwrap_or_default();
    let cc = s("country_code");
    let wider = if cc == "nl" { s("state") } else { s("country") };
    let place = match (locality.is_empty(), wider.is_empty() || wider == name) {
        (false, false) => format!("{locality} · {wider}"),
        (false, true) => locality,
        (true, false) => wider,
        (true, true) => String::new(),
    };
    let kind = match (addresstype, typ) {
        ("road", _) => "straat",
        ("house" | "building", _) => "adres",
        ("city" | "town" | "village" | "hamlet" | "suburb" | "neighbourhood" | "quarter" | "city_district", _) => "plaats",
        ("municipality", _) => "gemeente",
        ("state" | "province", _) => "provincie",
        ("country", _) => "land",
        (_, "place_of_worship") => "kerk",
        (_, "station" | "halt") => "station",
        (_, "bridge") => "brug",
        (_, "square") => "plein",
        (_, "park") => "park",
        (_, "attraction" | "monument" | "museum") => "bezienswaardigheid",
        (_, "river" | "canal" | "water") => "water",
        _ => "plek",
    };
    Some(Hit::new(name, place, kind, r["lon"].as_str()?.parse().ok()?, r["lat"].as_str()?.parse().ok()?, "OSM", &cc))
}

// ── Coordinates typed in ────────────────────────────────────────────────────

/// "51.814, 4.660" (lat, lon) or RD "104885 425320" (x y).
fn parse_coordinates(q: &str) -> Option<Hit> {
    let parts: Vec<f64> = q.split([',', ';', ' ']).filter(|s| !s.is_empty()).map(|s| s.parse().ok()).collect::<Option<_>>()?;
    let [a, b] = parts.as_slice() else { return None };
    if a.abs() <= 90.0 && b.abs() <= 180.0 {
        return Some(Hit::new(format!("{a}, {b}"), "breedtegraad, lengtegraad (WGS 84)".into(), "coördinaat", *b, *a, "invoer", ""));
    }
    if (0.0..300_000.0).contains(a) && (289_000.0..630_000.0).contains(b) {
        let ll = super::crs::transform("EPSG:28992", "EPSG:4326", [*a, *b]).ok()?;
        return Some(Hit::new(format!("X {a:.2}  Y {b:.2}"), "RD-coördinaat (EPSG:28992)".into(), "coördinaat", ll[0], ll[1], "invoer", "nl"));
    }
    None
}

// ── Searching ───────────────────────────────────────────────────────────────

/// Type-ahead suggestions: Dutch places from PDOK, or typed coordinates.
pub fn suggest(q: &str) -> Result<Vec<Hit>, String> {
    if let Some(h) = parse_coordinates(q) {
        return Ok(vec![h]);
    }
    let mut hits = pdok(q, 10)?;
    rank(&mut hits, q);
    Ok(tidy(relevant(hits, q), 8, q))
}

/// The full search (Enter): PDOK and Nominatim together.
pub fn search(q: &str) -> Result<Vec<Hit>, String> {
    if let Some(h) = parse_coordinates(q) {
        return Ok(vec![h]);
    }
    let (nl, world) = std::thread::scope(|s| {
        let a = s.spawn(|| pdok(q, 8));
        let b = s.spawn(|| nominatim(q));
        (a.join().unwrap_or_else(|_| Err("PDOK".into())), b.join().unwrap_or_else(|_| Err("Nominatim".into())))
    });
    if let (Err(a), Err(b)) = (&nl, &world) {
        return Err(format!("zoekdiensten niet bereikbaar ({a}; {b})"));
    }
    let nl = nl.unwrap_or_default();
    let world = world.unwrap_or_default();
    // PDOK always answers with (fuzzy) Dutch matches; put them first only when
    // the worldwide search also points to the Netherlands.
    let dutch = world.first().map_or(true, |h| h.country == "nl");
    let mut hits = if dutch { [nl, world].concat() } else { [world, nl].concat() };
    let complete = rank(&mut hits, q);
    // Nothing names every word, and the query ends in a place: look in that
    // place first.
    let tokens: Vec<&str> = q.split_whitespace().collect();
    if !complete && tokens.len() >= 2 {
        let last = tokens[tokens.len() - 1].trim_matches(|c: char| !c.is_alphanumeric() && c != '-');
        let mut place = last.to_string();
        if let Some(first) = place.get(0..1) {
            place = first.to_uppercase() + &place[1..];
        }
        let rest = tokens[..tokens.len() - 1].join(" ");
        if let Ok(local) = pdok_in_place(&rest, &place) {
            if !local.is_empty() {
                hits = [local, hits].concat();
            }
        }
    }
    Ok(tidy(relevant(hits, q), 12, q))
}

/// Without hits that share no word with the query (PDOK's fuzzy matching
/// offers "Bergen (L)" for "Lange Geldersekade"), unless nothing matches.
fn relevant(hits: Vec<Hit>, q: &str) -> Vec<Hit> {
    let wanted = words(q);
    let matches = |h: &Hit| {
        let label = words(&h.label).join(" ");
        wanted.iter().any(|w| label.contains(w.as_str()))
    };
    if hits.iter().any(matches) {
        hits.into_iter().filter(matches).collect()
    } else {
        hits
    }
}

/// A hit that names every word of the query beats a looser one ("Grote Kerk
/// Dordrecht" → in Dordrecht, not the street "Grote Kerk" in Hoorn); the
/// order so far decides between equals. True when the best names them all.
fn rank(hits: &mut [Hit], q: &str) -> bool {
    let wanted = words(q);
    let typed = q.trim().to_lowercase();
    let score = |h: &Hit| {
        let label = words(&h.label).join(" ");
        wanted.iter().filter(|w| label.contains(w.as_str())).count()
    };
    // Exactly what was typed ("Lange Geldersekade 2") goes first.
    let exact = |h: &Hit| h.name.trim().to_lowercase() == typed || h.label.to_lowercase().starts_with(&typed);
    hits.sort_by_key(|h| (std::cmp::Reverse(exact(h)), std::cmp::Reverse(score(h))));
    let best = hits.first().map(score).unwrap_or(0);
    best == wanted.len()
}

/// "Lange Geldersekade 2a" → "lange geldersekade".
fn street_of(name: &str) -> String {
    let lower = name.trim().to_lowercase();
    match lower.rsplit_once(' ') {
        Some((street, number)) if number.chars().next().is_some_and(|c| c.is_ascii_digit()) => street.to_string(),
        _ => lower,
    }
}

/// Without near-duplicates (the same street as "weg", "postcode" and from
/// OpenStreetMap), at most `max`. Without a house number in the query, one
/// line per street rather than every address in it.
fn tidy(hits: Vec<Hit>, max: usize, q: &str) -> Vec<Hit> {
    let numbered = q.chars().any(|c| c.is_ascii_digit());
    let mut out: Vec<Hit> = Vec::new();
    for h in hits {
        let same = out.iter().any(|o| {
            let metres = ((o.lon - h.lon) * h.lat.to_radians().cos()).hypot(o.lat - h.lat) * 111_000.0;
            let name = o.name.trim().to_lowercase() == h.name.trim().to_lowercase();
            let street = !numbered && h.kind == "adres" && street_of(&o.name) == street_of(&h.name) && metres < 2_000.0;
            (name && metres < 150.0) || (name && h.kind == "postcode") || street || metres < 2.0
        });
        if !same {
            out.push(h);
        }
        if out.len() == max {
            break;
        }
    }
    out
}

/// Lower-case words of at least two letters, accents folded.
fn words(s: &str) -> Vec<String> {
    let folded: String = s
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'à' | 'á' | 'â' | 'ä' => 'a',
            'è' | 'é' | 'ê' | 'ë' => 'e',
            'ì' | 'í' | 'î' | 'ï' => 'i',
            'ò' | 'ó' | 'ô' | 'ö' => 'o',
            'ù' | 'ú' | 'û' | 'ü' => 'u',
            c => c,
        })
        .collect();
    folded.split(|c: char| !c.is_alphanumeric()).filter(|w| w.chars().count() >= 2).map(str::to_string).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_fold_case_and_accents() {
        assert_eq!(words("Zürich Hauptbahnhof"), vec!["zurich", "hauptbahnhof"]);
        assert_eq!(words("Grote Kerk, Hoorn (weg)"), vec!["grote", "kerk", "hoorn", "weg"]);
    }

    #[test]
    fn pdok_names_split_into_two_lines() {
        assert_eq!(pdok_parts("Lange Geldersekade 2, 3311CK Dordrecht", "adres"), ("Lange Geldersekade 2".into(), "3311 CK Dordrecht".into()));
        assert_eq!(pdok_parts("Lange Geldersekade, Dordrecht", "weg"), ("Lange Geldersekade".into(), "Dordrecht".into()));
        assert_eq!(pdok_parts("Dordrecht, Dordrecht, Zuid-Holland", "woonplaats"), ("Dordrecht".into(), "Zuid-Holland".into()));
        assert_eq!(pdok_parts("Gemeente Dordrecht", "gemeente"), ("Dordrecht".into(), "gemeente".into()));
        assert_eq!(pdok_parts("Dordrecht E 233", "perceel").1, "kadastraal perceel");
    }

    #[test]
    fn coordinates_lat_lon_and_rd() {
        let h = parse_coordinates("51.814, 4.660").unwrap();
        assert_eq!((h.lat, h.lon), (51.814, 4.660));
        let rd = parse_coordinates("104885 425320").unwrap();
        assert!((rd.lat - 51.8143).abs() < 0.001 && (rd.lon - 4.6604).abs() < 0.001, "{rd:?}");
        assert_eq!(rd.kind, "coördinaat");
        assert!(parse_coordinates("Dordrecht").is_none());
    }

    #[test]
    fn unrelated_hits_are_dropped() {
        let h = |name: &str| Hit::new(name.into(), "x".into(), "straat", 4.66, 51.81, "PDOK", "nl");
        let r = relevant(vec![h("Lange Geldersekade"), h("Bergen (L)")], "Lange Geldersekade");
        assert_eq!(r.len(), 1);
        // Nothing matches: keep what there is.
        assert_eq!(relevant(vec![h("Bergen (L)")], "Geldersekade").len(), 1);
    }

    #[test]
    fn duplicates_are_dropped() {
        let h = |name: &str, kind: &str, lon: f64, src| Hit::new(name.into(), "Dordrecht".into(), kind, lon, 51.81, src, "nl");
        let hits = vec![
            h("Lange Geldersekade", "straat", 4.6610, "PDOK"),
            h("Lange Geldersekade", "postcode", 4.6650, "PDOK"),
            h("Lange Geldersekade", "straat", 4.6611, "OSM"),
            h("Lange Geldersekade 2", "adres", 4.6612, "PDOK"),
        ];
        let t = tidy(hits.clone(), 8, "Lange Geldersekade 2");
        assert_eq!(t.iter().map(|x| x.kind.as_str()).collect::<Vec<_>>(), vec!["straat", "adres"]);
        // Without a number: the street, not each address in it.
        let t = tidy(hits, 8, "Lange Geldersekade");
        assert_eq!(t.iter().map(|x| x.kind.as_str()).collect::<Vec<_>>(), vec!["straat"]);
        assert_eq!(street_of("Lange Geldersekade 2a"), "lange geldersekade");
    }
}
