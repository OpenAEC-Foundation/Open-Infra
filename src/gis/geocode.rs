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
    let url = format!(
        "https://api.pdok.nl/bzk/locatieserver/search/v3_1/free?rows=6&fl=weergavenaam,centroide_ll,type&q={}",
        encode(q)
    );
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
    Ok(if dutch { [nl, world].concat() } else { [world, nl].concat() })
}
