//! OGC API – Features client (GeoJSON, bbox in a chosen CRS, `next`-link
//! paging). Used for the BGT and usable for any OGC API Features service.

use super::http;
use super::vector::{parse_geojson, Feature};

pub fn crs_uri(epsg: u32) -> String {
    format!("http://www.opengis.net/def/crs/EPSG/0/{epsg}")
}

/// All items of `collection` within `bbox` (in `epsg`), up to `limit`,
/// optionally narrowed by a CQL2 text `filter` (OGC API – Features part 3).
/// Returns the features and whether the limit cut the result short.
pub fn fetch_items(base: &str, collection: &str, bbox: [f64; 4], epsg: u32, filter: Option<&str>, limit: usize, progress: &dyn Fn(String)) -> Result<(Vec<Feature>, bool), String> {
    let crs = crs_uri(epsg);
    let mut first = format!(
        "{}/collections/{}/items?f=json&limit={}&bbox={},{},{},{}&bbox-crs={}&crs={}",
        base.trim_end_matches('/'),
        collection,
        limit.min(1000),
        bbox[0], bbox[1], bbox[2], bbox[3],
        http::encode(&crs),
        http::encode(&crs),
    );
    if let Some(f) = filter {
        first.push_str(&format!("&filter={}&filter-lang=cql2-text", http::encode(f)));
    }
    let mut url = Some(first);
    let mut out = Vec::new();
    while let Some(u) = url.take() {
        progress(format!("{collection}: {} objecten…", out.len()));
        let json = http::get_json(&u)?;
        out.extend(parse_geojson(&json));
        if out.len() >= limit {
            out.truncate(limit);
            return Ok((out, true));
        }
        url = json["links"].as_array().and_then(|links| {
            links.iter().find(|l| l["rel"] == "next").and_then(|l| l["href"].as_str()).map(str::to_string)
        });
    }
    Ok((out, false))
}
