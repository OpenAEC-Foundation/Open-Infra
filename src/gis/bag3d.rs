//! 3D BAG (3D geoinformation group, TU Delft; CC BY 4.0): every building of
//! the BAG as LoD 1.2, 1.3 or 2.2 solids with roof, wall and ground
//! surfaces, from api.3dbag.nl (CityJSON 2.0 features, RD + NAP).

use serde_json::Value;

pub const API: &str = "https://api.3dbag.nl/collections/pand/items";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lod {
    L12,
    L13,
    L22,
}

impl Lod {
    pub fn code(self) -> &'static str {
        match self {
            Lod::L12 => "1.2",
            Lod::L13 => "1.3",
            Lod::L22 => "2.2",
        }
    }

    pub fn parse(s: &str) -> Option<Lod> {
        match s.trim().trim_start_matches(['L', 'l']).trim_start_matches(['o', 'O', 'd', 'D']) {
            "1.2" | "12" => Some(Lod::L12),
            "1.3" | "13" => Some(Lod::L13),
            "2.2" | "22" => Some(Lod::L22),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Roof,
    Wall,
    Ground,
    Other,
}

/// A planar surface: outer ring first, then holes; RD metres and NAP.
pub type Surface = Vec<Vec<[f64; 3]>>;

#[derive(Clone, Debug, Default)]
pub struct Building {
    pub id: String,
    pub year: Option<i64>,
    pub status: String,
    /// Ground level and roof (70th percentile) in m NAP.
    pub ground: Option<f64>,
    pub roof: Option<f64>,
    pub surfaces: Vec<(Kind, Surface)>,
}

fn kind_of(t: &str) -> Kind {
    match t {
        "RoofSurface" => Kind::Roof,
        "WallSurface" | "OuterCeilingSurface" | "ClosureSurface" => Kind::Wall,
        "GroundSurface" | "OuterFloorSurface" => Kind::Ground,
        _ => Kind::Other,
    }
}

/// One page: the buildings, the next page and the total.
pub fn parse_page(doc: &Value, lod: Lod) -> Result<(Vec<Building>, Option<String>, usize), String> {
    let t = &doc["metadata"]["transform"];
    let scale: Vec<f64> = t["scale"].as_array().map(|a| a.iter().filter_map(Value::as_f64).collect()).unwrap_or_else(|| vec![1.0; 3]);
    let shift: Vec<f64> = t["translate"].as_array().map(|a| a.iter().filter_map(Value::as_f64).collect()).unwrap_or_else(|| vec![0.0; 3]);
    if scale.len() != 3 || shift.len() != 3 {
        return Err("3D BAG: onbekende transformatie".into());
    }
    let mut out = Vec::new();
    for f in doc["features"].as_array().into_iter().flatten() {
        let verts: Vec<[f64; 3]> = f["vertices"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|v| {
                let a = v.as_array()?;
                Some([a.first()?.as_f64()? * scale[0] + shift[0], a.get(1)?.as_f64()? * scale[1] + shift[1], a.get(2)?.as_f64()? * scale[2] + shift[2]])
            })
            .collect();
        let Some(objects) = f["CityObjects"].as_object() else { continue };
        let mut b = Building::default();
        for (id, o) in objects {
            if o["type"] == "Building" {
                let a = &o["attributes"];
                b.id = a["identificatie"].as_str().unwrap_or(id).to_string();
                b.year = a["oorspronkelijkbouwjaar"].as_i64();
                b.status = a["status"].as_str().unwrap_or("").to_string();
                b.ground = a["b3_h_maaiveld"].as_f64();
                b.roof = a["b3_h_dak_70p"].as_f64();
            }
            for g in o["geometry"].as_array().into_iter().flatten() {
                if g["lod"].as_str() != Some(lod.code()) {
                    continue;
                }
                let sem: Vec<Kind> = g["semantics"]["surfaces"].as_array().into_iter().flatten().map(|s| kind_of(s["type"].as_str().unwrap_or(""))).collect();
                let ring = |r: &Value| -> Vec<[f64; 3]> { r.as_array().into_iter().flatten().filter_map(|i| verts.get(i.as_u64()? as usize).copied()).collect() };
                let surf = |s: &Value| -> Surface { s.as_array().into_iter().flatten().map(ring).filter(|r| r.len() >= 3).collect() };
                match g["type"].as_str() {
                    // [shell][surface][ring][vertex]; semantics values [shell][surface].
                    Some("Solid") => {
                        for (si, shell) in g["boundaries"].as_array().into_iter().flatten().enumerate() {
                            for (fi, s) in shell.as_array().into_iter().flatten().enumerate() {
                                let k = g["semantics"]["values"][si][fi].as_u64().and_then(|i| sem.get(i as usize).copied()).unwrap_or(Kind::Other);
                                let sf = surf(s);
                                if !sf.is_empty() {
                                    b.surfaces.push((k, sf));
                                }
                            }
                        }
                    }
                    // [surface][ring][vertex]; values [surface].
                    Some("MultiSurface") | Some("CompositeSurface") => {
                        for (fi, s) in g["boundaries"].as_array().into_iter().flatten().enumerate() {
                            let k = g["semantics"]["values"][fi].as_u64().and_then(|i| sem.get(i as usize).copied()).unwrap_or(Kind::Other);
                            let sf = surf(s);
                            if !sf.is_empty() {
                                b.surfaces.push((k, sf));
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        if !b.surfaces.is_empty() {
            out.push(b);
        }
    }
    let next = doc["links"].as_array().into_iter().flatten().find(|l| l["rel"] == "next").and_then(|l| l["href"].as_str()).map(str::to_string);
    let matched = doc["numberMatched"].as_u64().unwrap_or(0) as usize;
    Ok((out, next, matched))
}

/// The buildings in an RD box, page by page, up to `max`. The API counts
/// city objects (a building and its part: two per building) in its limit,
/// offsets and total; the second value says whether `max` cut the list.
pub fn fetch(bbox: [f64; 4], lod: Lod, max: usize, progress: &dyn Fn(String)) -> Result<(Vec<Building>, bool), String> {
    let mut url = format!("{API}?bbox={:.2},{:.2},{:.2},{:.2}&limit=100", bbox[0], bbox[1], bbox[2], bbox[3]);
    let mut all = Vec::new();
    let mut cut = false;
    for _ in 0..5000 {
        let doc = super::http::get_json(&url).map_err(|e| format!("3D BAG: {e}"))?;
        let (b, next, objects) = parse_page(&doc, lod)?;
        all.extend(b);
        progress(format!("3D BAG: {} van ongeveer {} panden…", all.len(), objects / 2));
        match next {
            Some(n) if all.len() < max => url = n,
            Some(_) => {
                cut = true;
                break;
            }
            None => break,
        }
    }
    if all.len() > max {
        all.truncate(max);
        cut = true;
    }
    Ok((all, cut))
}

/// Triangles of a planar surface with holes: projected on its main plane
/// and cut into ears.
pub fn triangulate(surface: &Surface) -> Vec<[[f64; 3]; 3]> {
    let outer = &surface[0];
    // Newell's normal picks the plane to project on.
    let mut n = [0.0f64; 3];
    for i in 0..outer.len() {
        let (a, b) = (outer[i], outer[(i + 1) % outer.len()]);
        n[0] += (a[1] - b[1]) * (a[2] + b[2]);
        n[1] += (a[2] - b[2]) * (a[0] + b[0]);
        n[2] += (a[0] - b[0]) * (a[1] + b[1]);
    }
    let drop = if n[2].abs() >= n[0].abs() && n[2].abs() >= n[1].abs() {
        2
    } else if n[0].abs() >= n[1].abs() {
        0
    } else {
        1
    };
    let (u, v) = match drop {
        0 => (1, 2),
        1 => (2, 0),
        _ => (0, 1),
    };
    let mut pts: Vec<[f64; 3]> = Vec::new();
    let mut flat: Vec<f64> = Vec::new();
    let mut holes = Vec::new();
    for (i, ring) in surface.iter().enumerate() {
        // CityJSON rings do not repeat their first point; others may.
        let mut r: &[[f64; 3]] = ring;
        if r.len() > 3 && r.first() == r.last() {
            r = &r[..r.len() - 1];
        }
        if i > 0 {
            holes.push(pts.len());
        }
        for p in r {
            pts.push(*p);
            flat.push(p[u]);
            flat.push(p[v]);
        }
    }
    // Small numbers keep the ear test exact enough.
    let (ou, ov) = (flat[0], flat[1]);
    for (i, c) in flat.iter_mut().enumerate() {
        *c -= if i % 2 == 0 { ou } else { ov };
    }
    match earcutr::earcut(&flat, &holes, 2) {
        Ok(idx) => idx.chunks_exact(3).map(|t| [pts[t[0]], pts[t[1]], pts[t[2]]]).collect(),
        Err(_) => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_page_of_cityjson() {
        let doc: Value = serde_json::from_str(
            r#"{"type":"FeatureCollection","numberMatched":7,"metadata":{"transform":{"scale":[0.001,0.001,0.001],"translate":[100000,400000,0]}},
            "links":[{"rel":"next","href":"https://api.3dbag.nl/next"}],
            "features":[{"type":"CityJSONFeature","id":"p1","vertices":[[0,0,0],[10000,0,0],[10000,10000,0],[0,10000,0],[0,0,6000],[10000,0,6000],[10000,10000,6000],[0,10000,6000]],
            "CityObjects":{"p1":{"type":"Building","attributes":{"identificatie":"NL.IMBAG.Pand.1","oorspronkelijkbouwjaar":1930,"status":"Pand in gebruik","b3_h_maaiveld":0.5,"b3_h_dak_70p":6.0},"children":["p1-0"]},
            "p1-0":{"type":"BuildingPart","geometry":[{"type":"Solid","lod":"1.2","boundaries":[[[[0,3,2,1]],[[4,5,6,7]],[[0,1,5,4]]]],"semantics":{"surfaces":[{"type":"GroundSurface"},{"type":"RoofSurface"},{"type":"WallSurface"}],"values":[[0,1,2]]}},
            {"type":"Solid","lod":"2.2","boundaries":[[[[0,1,2]]]]}]}}}]}"#,
        )
        .unwrap();
        let (b, next, matched) = parse_page(&doc, Lod::L12).unwrap();
        assert_eq!((b.len(), matched, next.as_deref()), (1, 7, Some("https://api.3dbag.nl/next")));
        assert_eq!(b[0].id, "NL.IMBAG.Pand.1");
        assert_eq!(b[0].year, Some(1930));
        let kinds: Vec<Kind> = b[0].surfaces.iter().map(|s| s.0).collect();
        assert_eq!(kinds, [Kind::Ground, Kind::Roof, Kind::Wall]);
        assert_eq!(b[0].surfaces[1].1[0][0], [100000.0, 400000.0, 6.0]);
        // A wall triangulates on its own plane.
        let tris = triangulate(&b[0].surfaces[2].1);
        assert_eq!(tris.len(), 2);
        // A roof with a hole.
        let roof: Surface = vec![
            vec![[0.0, 0.0, 6.0], [10.0, 0.0, 6.0], [10.0, 10.0, 6.0], [0.0, 10.0, 6.0]],
            vec![[4.0, 4.0, 6.0], [4.0, 6.0, 6.0], [6.0, 6.0, 6.0], [6.0, 4.0, 6.0]],
        ];
        let tris = triangulate(&roof);
        let area: f64 = tris.iter().map(|t| ((t[1][0] - t[0][0]) * (t[2][1] - t[0][1]) - (t[2][0] - t[0][0]) * (t[1][1] - t[0][1])).abs() / 2.0).sum();
        assert!((area - 96.0).abs() < 1e-9, "{area}");
        assert_eq!(Lod::parse("lod2.2"), Some(Lod::L22));
        assert_eq!(Lod::parse("13"), Some(Lod::L13));
    }
}
