//! Coordinate reference systems: built-in proj definitions for NL/BE/DE/UK/…,
//! automatic UTM anywhere in the world, and any other EPSG code fetched once
//! from epsg.io.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use proj4rs::proj::Proj;

use super::http;

pub type XY = [f64; 2];

#[derive(Clone, Debug)]
pub struct CrsInfo {
    pub code: String,
    pub name: String,
    pub region: String,
    pub proj4: String,
}

const BUILTIN: &[(&str, &str, &str, &str)] = &[
    // Position-vector 7-parameter set (proj4 convention); ~0.2 m from
    // RDNAPTRANS, which needs the correction grid for better.
    ("EPSG:28992", "Amersfoort / RD New", "Nederland",
     "+proj=sterea +lat_0=52.15616055555555 +lon_0=5.38763888888889 +k=0.9999079 +x_0=155000 +y_0=463000 +ellps=bessel +towgs84=565.417,50.3319,465.552,-0.398957,0.343988,-1.8774,4.0725 +units=m +no_defs"),
    ("EPSG:4326", "WGS 84 (lat/lon)", "Wereld", "+proj=longlat +datum=WGS84 +no_defs"),
    ("EPSG:4258", "ETRS89 (lat/lon)", "Europa", "+proj=longlat +ellps=GRS80 +towgs84=0,0,0,0,0,0,0 +no_defs"),
    ("EPSG:3857", "WGS 84 / Pseudo-Mercator", "Wereld (webkaarten)",
     "+proj=merc +a=6378137 +b=6378137 +lat_ts=0 +lon_0=0 +x_0=0 +y_0=0 +k=1 +units=m +nadgrids=@null +no_defs"),
    ("EPSG:25831", "ETRS89 / UTM zone 31N", "Europa (0°–6°O)", "+proj=utm +zone=31 +ellps=GRS80 +towgs84=0,0,0,0,0,0,0 +units=m +no_defs"),
    ("EPSG:25832", "ETRS89 / UTM zone 32N", "Europa (6°–12°O)", "+proj=utm +zone=32 +ellps=GRS80 +towgs84=0,0,0,0,0,0,0 +units=m +no_defs"),
    ("EPSG:25833", "ETRS89 / UTM zone 33N", "Europa (12°–18°O)", "+proj=utm +zone=33 +ellps=GRS80 +towgs84=0,0,0,0,0,0,0 +units=m +no_defs"),
    ("EPSG:31370", "Belge 1972 / Belgian Lambert 72", "België",
     "+proj=lcc +lat_0=90 +lon_0=4.36748666666667 +lat_1=51.1666672333333 +lat_2=49.8333339 +x_0=150000.013 +y_0=5400088.438 +ellps=intl +towgs84=-106.8686,52.2978,-103.7239,0.3366,-0.457,1.8422,-1.2747 +units=m +no_defs"),
    ("EPSG:3812", "ETRS89 / Belgian Lambert 2008", "België",
     "+proj=lcc +lat_0=50.797815 +lon_0=4.35921583333333 +lat_1=49.8333333333333 +lat_2=51.1666666666667 +x_0=649328 +y_0=665262 +ellps=GRS80 +towgs84=0,0,0,0,0,0,0 +units=m +no_defs"),
    ("EPSG:27700", "OSGB36 / British National Grid", "Verenigd Koninkrijk",
     "+proj=tmerc +lat_0=49 +lon_0=-2 +k=0.9996012717 +x_0=400000 +y_0=-100000 +ellps=airy +towgs84=446.448,-125.157,542.06,0.15,0.247,0.842,-20.489 +units=m +no_defs"),
    ("EPSG:2056", "CH1903+ / LV95", "Zwitserland",
     "+proj=somerc +lat_0=46.9524055555556 +lon_0=7.43958333333333 +k_0=1 +x_0=2600000 +y_0=1200000 +ellps=bessel +towgs84=674.374,15.056,405.346,0,0,0,0 +units=m +no_defs"),
    ("EPSG:2154", "RGF93 / Lambert-93", "Frankrijk",
     "+proj=lcc +lat_0=46.5 +lon_0=3 +lat_1=49 +lat_2=44 +x_0=700000 +y_0=6600000 +ellps=GRS80 +towgs84=0,0,0,0,0,0,0 +units=m +no_defs"),
];

fn registry() -> &'static Mutex<HashMap<String, CrsInfo>> {
    static REG: OnceLock<Mutex<HashMap<String, CrsInfo>>> = OnceLock::new();
    REG.get_or_init(|| {
        let mut m = HashMap::new();
        for (code, name, region, proj4) in BUILTIN {
            m.insert(code.to_string(), CrsInfo { code: code.to_string(), name: name.to_string(), region: region.to_string(), proj4: proj4.to_string() });
        }
        Mutex::new(m)
    })
}

pub fn builtin_list() -> Vec<CrsInfo> {
    let reg = registry().lock().unwrap();
    BUILTIN.iter().filter_map(|(c, ..)| reg.get(*c).cloned()).collect()
}

/// `28992`, `epsg:28992`, `urn:ogc:def:crs:EPSG::28992` → `EPSG:28992`.
pub fn normalize(code: &str) -> String {
    let t = code.trim();
    if t.eq_ignore_ascii_case("CRS:84") {
        return "EPSG:4326".into();
    }
    let digits: String = t.chars().rev().take_while(|c| c.is_ascii_digit()).collect::<Vec<_>>().into_iter().rev().collect();
    if digits.len() >= 4 { format!("EPSG:{digits}") } else { t.to_ascii_uppercase() }
}

/// UTM zone (WGS 84) for a lon/lat, including the Norway/Svalbard exceptions.
pub fn utm_code_for(lon: f64, lat: f64) -> String {
    let mut zone = ((lon + 180.0) / 6.0).floor() as i32 + 1;
    if (56.0..64.0).contains(&lat) && (3.0..12.0).contains(&lon) {
        zone = 32;
    }
    if (72.0..84.0).contains(&lat) {
        zone = match lon {
            l if (0.0..9.0).contains(&l) => 31,
            l if (9.0..21.0).contains(&l) => 33,
            l if (21.0..33.0).contains(&l) => 35,
            l if (33.0..42.0).contains(&l) => 37,
            _ => zone,
        };
    }
    let zone = zone.clamp(1, 60);
    format!("EPSG:{}{:02}", if lat >= 0.0 { 326 } else { 327 }, zone)
}

fn define_utm(code: &str) -> Option<CrsInfo> {
    let n = code.strip_prefix("EPSG:")?;
    if n.len() != 5 || !(n.starts_with("326") || n.starts_with("327")) {
        return None;
    }
    let zone: u32 = n[3..].parse().ok().filter(|z| (1..=60).contains(z))?;
    let south = n.starts_with("327");
    Some(CrsInfo {
        code: code.to_string(),
        name: format!("WGS 84 / UTM zone {zone}{}", if south { "S" } else { "N" }),
        region: "Wereld (UTM)".into(),
        proj4: format!("+proj=utm +zone={zone}{} +datum=WGS84 +units=m +no_defs", if south { " +south" } else { "" }),
    })
}

/// National grid for a country (ISO code from the geocoder), else by position.
pub fn suggest_for(country: &str, lon: f64, lat: f64) -> String {
    match country {
        "nl" => "EPSG:28992".into(),
        "be" => "EPSG:31370".into(),
        "de" | "dk" => if lon < 12.0 { "EPSG:25832".into() } else { "EPSG:25833".into() },
        "gb" => "EPSG:27700".into(),
        "ch" | "li" => "EPSG:2056".into(),
        "fr" => "EPSG:2154".into(),
        "" => suggest(lon, lat),
        _ => utm_code_for(lon, lat),
    }
}

/// National grid where we know one (by bounding box), otherwise UTM.
pub fn suggest(lon: f64, lat: f64) -> String {
    let inside = |w: f64, s: f64, e: f64, n: f64| lon > w && lon < e && lat > s && lat < n;
    if inside(3.2, 50.7, 7.3, 53.7) { return "EPSG:28992".into(); }
    if inside(2.5, 49.4, 6.5, 51.55) { return "EPSG:31370".into(); }
    if inside(-8.7, 49.8, 1.8, 60.9) { return "EPSG:27700".into(); }
    if inside(5.9, 45.8, 10.5, 47.9) { return "EPSG:2056".into(); }
    // Germany and Denmark: ETRS89 / UTM (official grids).
    if inside(5.8, 47.2, 15.1, 57.8) && !inside(-5.2, 41.3, 7.9, 49.6) {
        return if lon < 12.0 { "EPSG:25832".into() } else { "EPSG:25833".into() };
    }
    if inside(-5.2, 41.3, 9.6, 51.1) { return "EPSG:2154".into(); }
    utm_code_for(lon, lat)
}

/// Info for a known CRS without network access (built-in, UTM, or cached).
pub fn known(code: &str) -> Option<CrsInfo> {
    let code = normalize(code);
    if let Some(i) = registry().lock().unwrap().get(&code) {
        return Some(i.clone());
    }
    let utm = define_utm(&code)?;
    registry().lock().unwrap().insert(code, utm.clone());
    Some(utm)
}

/// Make sure the CRS is usable; unknown EPSG codes are fetched from epsg.io.
pub fn ensure(code: &str) -> Result<CrsInfo, String> {
    let code = normalize(code);
    if let Some(i) = known(&code) {
        return Ok(i);
    }
    let num = code.strip_prefix("EPSG:").ok_or_else(|| format!("onbekend stelsel {code}"))?;
    let def = http::get_text(&format!("https://epsg.io/{num}.proj4"))?.trim().to_string();
    if !def.starts_with('+') {
        return Err(format!("onbekend coördinatenstelsel {code}"));
    }
    Proj::from_proj_string(&def).map_err(|e| format!("{code}: {e}"))?;
    let name = http::get_json(&format!("https://epsg.io/{num}.json"))
        .ok()
        .and_then(|j| j.get("name").and_then(|n| n.as_str()).map(str::to_string))
        .unwrap_or_else(|| code.clone());
    let info = CrsInfo { code: code.clone(), name, region: "epsg.io".into(), proj4: def };
    registry().lock().unwrap().insert(code, info.clone());
    Ok(info)
}

/// Register a definition that was stored with a drawing (no network needed).
pub fn register(info: CrsInfo) {
    registry().lock().unwrap().entry(info.code.clone()).or_insert(info);
}

pub fn is_geographic(code: &str) -> bool {
    known(code).map(|i| i.proj4.contains("+proj=longlat")).unwrap_or(false)
}

fn proj(code: &str) -> Result<Proj, String> {
    let info = known(code).ok_or_else(|| format!("coördinatenstelsel {code} niet geladen"))?;
    Proj::from_proj_string(&info.proj4).map_err(|e| format!("{code}: {e}"))
}

/// A reusable transformation between two systems (degrees for geographic CRS).
pub struct Transformer {
    src: Proj,
    dst: Proj,
    src_geo: bool,
    dst_geo: bool,
    identity: bool,
}

impl Transformer {
    pub fn new(from: &str, to: &str) -> Result<Self, String> {
        let (from, to) = (normalize(from), normalize(to));
        Ok(Self {
            src: proj(&from)?,
            dst: proj(&to)?,
            src_geo: is_geographic(&from),
            dst_geo: is_geographic(&to),
            identity: from == to,
        })
    }

    pub fn apply(&self, p: XY) -> Result<XY, String> {
        if self.identity {
            return Ok(p);
        }
        let mut pt = if self.src_geo { (p[0].to_radians(), p[1].to_radians(), 0.0) } else { (p[0], p[1], 0.0) };
        proj4rs::transform::transform(&self.src, &self.dst, &mut pt).map_err(|e| e.to_string())?;
        Ok(if self.dst_geo { [pt.0.to_degrees(), pt.1.to_degrees()] } else { [pt.0, pt.1] })
    }
}

pub fn transform(from: &str, to: &str, p: XY) -> Result<XY, String> {
    Transformer::new(from, to)?.apply(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rd_roundtrip_amersfoort() {
        // Onze Lieve Vrouwetoren Amersfoort ≈ RD (155000, 463000).
        let ll = transform("EPSG:28992", "EPSG:4326", [155000.0, 463000.0]).unwrap();
        // RDNAPTRANS reference 52.15517440 N, 5.38720621 E; allow ~1 m.
        assert!((ll[0] - 5.38720621).abs() < 1.5e-5, "lon {}", ll[0]);
        assert!((ll[1] - 52.15517440).abs() < 1e-5, "lat {}", ll[1]);
        let rd = transform("EPSG:4326", "EPSG:28992", ll).unwrap();
        assert!((rd[0] - 155000.0).abs() < 0.01 && (rd[1] - 463000.0).abs() < 0.01);
    }

    #[test]
    fn utm_zones() {
        assert_eq!(utm_code_for(36.82, -1.28), "EPSG:32737");
        assert_eq!(utm_code_for(5.0, 60.0), "EPSG:32632");
        assert_eq!(suggest(5.12, 52.09), "EPSG:28992");
        assert_eq!(suggest(9.98, 53.54), "EPSG:25832");
        assert_eq!(suggest(13.40, 52.52), "EPSG:25833");
        assert_eq!(suggest_for("de", 6.08, 50.78), "EPSG:25832");
        assert_eq!(suggest_for("ke", 36.82, -1.28), "EPSG:32737");
        let nairobi = transform("EPSG:4326", "EPSG:32737", [36.82017, -1.2851625]).unwrap();
        assert!((nairobi[0] - 257441.2).abs() < 1.0 && (nairobi[1] - 9857847.2).abs() < 1.0, "{nairobi:?}");
    }

    #[test]
    fn normalizes_codes() {
        assert_eq!(normalize("urn:ogc:def:crs:EPSG::28992"), "EPSG:28992");
        assert_eq!(normalize("CRS:84"), "EPSG:4326");
        assert_eq!(normalize("3857"), "EPSG:3857");
    }
}
