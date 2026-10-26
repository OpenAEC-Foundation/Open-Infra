//! Address search: PDOK Locatieserver (Dutch addresses, parcels, roads) and
//! OpenStreetMap Nominatim (the rest of the world).

use super::http::{self, encode};

#[derive(Clone, Debug)]
pub struct Hit {
    pub label: String,
    pub lon: f64,
    pub lat: f64,
    pub source: &'static str,
    pub country: String,
}

fn pdok(q: &str) -> Result<Vec<Hit>, String> {
    pdok_query(&format!("q={}", encode(q)))
}

/// PDOK search within one place (woonplaats): "Grote Kerk Dordrecht" stays in
/// Dordrecht instead of finding the street "Grote Kerk" in Hoorn.
fn pdok_in_place(q: &str, place: &str) -> Result<Vec<Hit>, String> {
    pdok_query(&format!("q={}&fq=woonplaatsnaam:{}", encode(q), encode(place)))
}

fn pdok_query(params: &str) -> Result<Vec<Hit>, String> {
    let url = format!("https://api.pdok.nl/bzk/locatieserver/search/v3_1/free?rows=6&fl=weergavenaam,centroide_ll,type&{params}");
    let json = http::get_json(&url)?;
    let docs = json["response"]["docs"].as_array().cloned().unwrap_or_default();
    Ok(docs
        .iter()
        .filter_map(|d| {
            let ll = d["centroide_ll"].as_str()?;
            let inner = ll.strip_prefix("POINT(")?.strip_suffix(')')?;
            let mut it = inner.split_whitespace().filter_map(|v| v.parse::<f64>().ok());
            let (lon, lat) = (it.next()?, it.next()?);
            Some(Hit {
                label: format!("{} ({})", d["weergavenaam"].as_str().unwrap_or("?"), d["type"].as_str().unwrap_or("")),
                lon,
                lat,
                source: "PDOK",
                country: "nl".into(),
            })
        })
        .collect())
}

fn nominatim(q: &str) -> Result<Vec<Hit>, String> {
    let url = format!(
        "https://nominatim.openstreetmap.org/search?format=jsonv2&addressdetails=1&limit=6&q={}",
        encode(q)
    );
    let json = http::get_json(&url)?;
    Ok(json
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|r| {
            Some(Hit {
                label: r["display_name"].as_str()?.to_string(),
                lon: r["lon"].as_str()?.parse().ok()?,
                lat: r["lat"].as_str()?.parse().ok()?,
                source: "OSM",
                country: r["address"]["country_code"].as_str().unwrap_or("").to_string(),
            })
        })
        .collect())
}

/// "52.09, 5.11" (lat, lon) typed directly.
fn parse_lat_lon(q: &str) -> Option<Hit> {
    let parts: Vec<f64> = q.split([',', ';', ' ']).filter(|s| !s.is_empty()).map(|s| s.parse().ok()).collect::<Option<_>>()?;
    let [lat, lon] = parts.as_slice() else { return None };
    (lat.abs() <= 90.0 && lon.abs() <= 180.0).then(|| Hit {
        label: format!("{lat}, {lon}"),
        lon: *lon,
        lat: *lat,
        source: "invoer",
        country: String::new(),
    })
}

pub fn search(q: &str) -> Result<Vec<Hit>, String> {
    if let Some(h) = parse_lat_lon(q) {
        return Ok(vec![h]);
    }
    let (nl, world) = std::thread::scope(|s| {
        let a = s.spawn(|| pdok(q));
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
    // A hit that names every word of the query beats a looser one ("Grote
    // Kerk Dordrecht" → the church in Dordrecht, not the street "Grote Kerk"
    // in Hoorn); the order above decides between equals.
    let wanted = words(q);
    let score = |h: &Hit| {
        let label = words(&h.label).join(" ");
        wanted.iter().filter(|w| label.contains(w.as_str())).count()
    };
    hits.sort_by_key(|h| std::cmp::Reverse(score(h)));
    // Nothing names every word, and the query ends in a place: look in that
    // place first.
    let complete = hits.first().is_some_and(|h| score(h) == wanted.len());
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
    Ok(hits)
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
    #[test]
    fn words_fold_case_and_accents() {
        assert_eq!(super::words("Zürich Hauptbahnhof"), vec!["zurich", "hauptbahnhof"]);
        assert_eq!(super::words("Grote Kerk, Hoorn (weg)"), vec!["grote", "kerk", "hoorn", "weg"]);
    }
}
