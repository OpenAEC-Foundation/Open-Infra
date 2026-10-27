//! GWSW — Gegevenswoordenboek Stedelijk Water: the sewer networks Dutch
//! municipalities and water boards publish, served by PDOK as OGC API
//! Features (`beheer-stedelijk-watersystemen-gwsw`, CC0), and their mapping
//! to the NLCS main group RI (riolering).

use super::ogcapi;
use super::site::Site;
use super::vector::Feature;

pub const API: &str = "https://api.pdok.nl/rioned/beheer-stedelijk-watersystemen-gwsw/ogc/v1";

/// The collections, in drawing order (pipes under the manholes).
pub const COLLECTIONS: &[(&str, &str)] = &[
    ("beheerleiding", "Leidingen"),
    ("aansluitingleiding", "Aansluitleidingen"),
    ("beheerput", "Putten"),
    ("beheerbouwwerk", "Bouwwerken (gemalen, uitlaten)"),
    ("beheerpomp", "Pompen"),
    ("aansluitingpunt", "Aansluitpunten"),
];

/// The last part of a GWSW URI (`http://data.gwsw.nl/1.5/totaal/Beton` → `Beton`).
pub fn term(v: Option<&serde_json::Value>) -> String {
    v.and_then(|v| v.as_str()).map(|s| s.rsplit(['/', '#']).next().unwrap_or(s).to_string()).unwrap_or_default()
}

/// NLCS system prefix in group RI for a GWSW pipe type or system type.
pub fn system(kind: &str, stelseltype: &str) -> &'static str {
    let k = format!("{} {}", kind, stelseltype).to_ascii_lowercase();
    if k.contains("drain") {
        "DRAINAGE"
    } else if k.contains("druk") || k.contains("pers") {
        "PERSLEIDING"
    } else if k.contains("vacuum") {
        "VACUUMLEIDING"
    } else if k.contains("infiltr") {
        "ITR"
    } else if k.contains("hemelwater") || k.contains("regenwater") || k.contains("hwa") {
        "HWA"
    } else if k.contains("vuilwater") || k.contains("afvalwater") || k.contains("dwa") {
        "DWA"
    } else {
        // Gemengd riool, overstort- and bergingsleidingen, unknown.
        "GWA"
    }
}

fn material(m: &str) -> Option<&'static str> {
    let m = m.to_ascii_lowercase();
    Some(match () {
        _ if m.contains("beton") => "BETON",
        _ if m.contains("gres") => "GRES",
        _ if m.contains("pvc") => "PVC",
        _ if m == "pe" || m.contains("polyethyleen") || m.starts_with("hdpe") => "PE",
        _ if m == "pp" || m.contains("polypropyleen") => "PP",
        _ if m.contains("gietijzer") => "GIETIJZER",
        _ if m.contains("staal") => "STAAL",
        _ if m.contains("metsel") => "METSELWERK",
        _ if m.contains("asbest") => "ASBESTCEMENT",
        _ => return None,
    })
}

/// NLCS object names to try, most specific first, for a feature of
/// `collection`. The caller takes the first one the NLCS table has.
pub fn nlcs_candidates(collection: &str, f: &Feature) -> Vec<String> {
    let kind = term(f.props.get("type"));
    let stelsel = term(f.props.get("stelseltype"));
    let sys = system(&kind, &stelsel);
    let mat = material(&term(f.props.get("materiaal_leiding")));
    let width = f.props.get("breedte_leiding").and_then(|v| v.as_f64()).filter(|w| *w > 0.0).map(|w| w.round() as i64);
    let mut out = Vec::new();
    match collection {
        "beheerleiding" => {
            let base = match sys {
                "DRAINAGE" => "DRAINAGE_DRAIN".to_string(),
                "PERSLEIDING" | "VACUUMLEIDING" => sys.to_string(),
                s => format!("{s}_RIOOLLEIDING"),
            };
            if let Some(m) = mat {
                if let Some(w) = width {
                    out.push(format!("{base}_{m}_{w}"));
                }
                out.push(format!("{base}_{m}"));
            }
            out.push(base);
        }
        "aansluitingleiding" => {
            if kind.to_ascii_lowercase().contains("kolk") {
                out.push(format!("{sys}_KOLKAANSLUITLEIDING"));
            }
            out.push(format!("{sys}_PERCEELAANSLUITLEIDING"));
            out.push(format!("{sys}_KOLKAANSLUITLEIDING"));
        }
        "beheerput" => {
            let k = kind.to_ascii_lowercase();
            let sub = if k.contains("blinde") { "BLINDEPUT" } else if k.contains("pomp") { "POMPPUT" } else if k.contains("overstort") { "OVERSTORTPUT" } else { "INSPECTIEPUT" };
            out.push(format!("{sys}_RIOOLPUT_{sub}"));
            out.push(format!("{sys}_RIOOLPUT"));
        }
        "beheerbouwwerk" | "beheerpomp" => {
            let k = kind.to_ascii_lowercase();
            if k.contains("uitlaat") || k.contains("overstort") {
                out.push(format!("{sys}_UITLAATCONSTRUCTIE"));
            } else {
                out.push(format!("{sys}_RIOOLPUT_POMPPUT"));
                out.push("PERSLEIDING_GEMAAL".into());
            }
        }
        _ => {
            out.push(format!("{sys}_PERCEELAANSLUITPUNT"));
            out.push(format!("{sys}_AANSLUITPUT"));
        }
    }
    out.push(format!("{sys}_RIOOLLEIDING"));
    out.push("GWA_RIOOLLEIDING".into());
    out
}

/// Fetch the GWSW collections in the site extent (in RD).
pub fn fetch(site: &Site, collections: &[&str], progress: &dyn Fn(String)) -> Result<Vec<(String, Vec<Feature>)>, String> {
    let b = if site.crs == "EPSG:28992" {
        site.bbox()
    } else {
        super::site::envelope(site.bbox(), &super::crs::Transformer::new(&site.crs, "EPSG:28992")?)?
    };
    collections
        .iter()
        .map(|c| ogcapi::fetch_items(API, c, b, 28992, None, 50_000, progress).map(|(f, _)| (c.to_string(), f)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feature(pairs: &[(&str, serde_json::Value)]) -> Feature {
        let mut f = Feature::default();
        for (k, v) in pairs {
            f.props.insert((*k).into(), v.clone());
        }
        f
    }

    #[test]
    fn pipes_map_to_the_most_specific_nlcs_object() {
        let f = feature(&[("type", "http://data.gwsw.nl/1.5/totaal/GemengdRiool".into()), ("materiaal_leiding", "http://data.gwsw.nl/1.5/totaal/Beton".into()), ("breedte_leiding", 800.into())]);
        let c = nlcs_candidates("beheerleiding", &f);
        assert_eq!(c[0], "GWA_RIOOLLEIDING_BETON_800");
        assert!(c.contains(&"GWA_RIOOLLEIDING".to_string()));
        let h = feature(&[("type", "Hemelwaterriool".into())]);
        assert_eq!(nlcs_candidates("beheerleiding", &h)[0], "HWA_RIOOLLEIDING");
        let p = feature(&[("type", "Drukleiding".into())]);
        assert_eq!(nlcs_candidates("beheerleiding", &p)[0], "PERSLEIDING");
        let put = feature(&[("type", "Inspectieput".into()), ("stelseltype", "Regenwaterstelsel".into())]);
        assert_eq!(nlcs_candidates("beheerput", &put)[0], "HWA_RIOOLPUT_INSPECTIEPUT");
    }

    #[test]
    fn the_first_candidates_exist_in_nlcs() {
        for (coll, kind) in [("beheerleiding", "GemengdRiool"), ("beheerleiding", "Vuilwaterriool"), ("beheerleiding", "Infiltratieriool"), ("beheerput", "Inspectieput"), ("aansluitingleiding", "Kolkaansluitleiding"), ("beheerbouwwerk", "Uitlaatconstructie"), ("beheerbouwwerk", "Rioolgemaal")] {
            let f = feature(&[("type", kind.into())]);
            let found = nlcs_candidates(coll, &f).iter().any(|n| crate::nlcs::object_by_name("RI", n).is_some());
            assert!(found, "{coll} {kind}: no NLCS object");
        }
    }
}
