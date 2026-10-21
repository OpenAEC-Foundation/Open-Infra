//! WFS GetFeature as GeoJSON — in the project CRS when the server offers it,
//! otherwise WGS 84 — with startIndex paging for servers with a page limit.

use super::caps::{pick_geojson_format, WfsCaps, WfsType};
use super::http;
use super::site::Site;
use super::vector::{parse_geojson, Feature};

pub struct WfsResult {
    pub features: Vec<Feature>,
    pub data_crs: String,
    pub truncated: bool,
}

pub fn fetch(site: &Site, caps: &WfsCaps, ft: &WfsType, limit: usize, progress: &dyn Fn(String)) -> Result<WfsResult, String> {
    let v2 = caps.version.starts_with('2');
    let use_site = ft.crs.is_empty() || ft.crs.iter().any(|c| c == &site.crs);
    let data_crs = if use_site { site.crs.clone() } else { "EPSG:4326".to_string() };
    let bbox = if use_site {
        let b = site.bbox();
        format!("{},{},{},{},{}", b[0], b[1], b[2], b[3], site.crs)
    } else {
        let [w, s, e, n] = site.bbox_lonlat()?;
        // urn form = lat/lon axis order (WFS 1.1 / 2.0).
        format!("{s},{w},{n},{e},urn:ogc:def:crs:EPSG::4326")
    };
    let format = pick_geojson_format(&caps.output_formats);
    let page = limit.min(1000);
    let mut features = Vec::new();
    let mut truncated = false;
    let mut start = 0;
    while start < limit {
        let want = page.min(limit - start);
        let mut params: Vec<(&str, String)> = vec![
            ("SERVICE", "WFS".into()),
            ("VERSION", if v2 { "2.0.0" } else { "1.1.0" }.into()),
            ("REQUEST", "GetFeature".into()),
            (if v2 { "TYPENAMES" } else { "TYPENAME" }, ft.name.clone()),
            ("SRSNAME", data_crs.clone()),
            ("BBOX", bbox.clone()),
            ("OUTPUTFORMAT", format.clone()),
            (if v2 { "COUNT" } else { "MAXFEATURES" }, want.to_string()),
        ];
        if start > 0 {
            params.push(("STARTINDEX", start.to_string()));
        }
        progress(format!("Objecten ophalen… ({})", features.len()));
        let body = http::get_text(&http::with_params(&caps.get_feature_url, &params))?;
        let json: serde_json::Value = serde_json::from_str(&body)
            .map_err(|_| http::service_error(&body).unwrap_or_else(|| "server gaf geen GeoJSON terug".into()))?;
        let got = parse_geojson(&json);
        let n = got.len();
        features.extend(got);
        if n < want {
            break;
        }
        if !v2 {
            // WFS 1.x has no reliable paging.
            truncated = n >= page;
            break;
        }
        start += page;
        if start >= limit {
            truncated = true;
        }
    }
    Ok(WfsResult { features, data_crs, truncated })
}
