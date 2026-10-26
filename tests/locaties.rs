//! Testset: Open Infra op verschillende locaties.
//!
//! **Offline** (`cargo test --test locaties`, ook in CI):
//! - het coördinatenstelsel per locatie (op land en op positie), ook rond
//!   de Nederlandse landsgrenzen;
//! - transformaties heen en terug;
//! - de projectlocatie in de tekening: absoluut en relatief, meters en mm;
//! - de NLCS-koppeling van echte BGT-kenmerken (vastgelegde testdata);
//! - de KLIC-voorbeeldlevering van GIS2BIM (als die naast deze map staat);
//! - hoogtelijnen en profielstations op een kunstmatig terrein;
//! - het formaat van het tekenblad.
//!
//! **Online** (`cargo test --test locaties -- --ignored --nocapture`): per
//! locatie zoeken, kaartbeeld, BGT, BRK, OSM, AHN-hoogtes, -profiel en
//! -puntenwolk ophalen en verwerken, met een rapport in
//! `target/locatietest/rapport.md` (en `.json`). Overbelaste openbare
//! diensten (Overpass, Nominatim) tellen als waarschuwing, niet als fout.

use std::path::PathBuf;
use std::time::Instant;

use serde_json::Value;

use open_infra::gis::crs::{self, Transformer};
use open_infra::gis::site::Site;
use open_infra::gis::{ahn, bgt, geocode, klic, osm, pointcloud, raster, servers, vector};
use open_infra::{basisdata, nlcs, tekenblad};

// ── Testlocaties ────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
struct Locatie {
    id: String,
    naam: String,
    zoek: String,
    lon: f64,
    lat: f64,
    land: String,
    crs: String,
    breedte: f64,
    hoogte: f64,
    ahn: Option<(f64, f64)>,
    bgt: bool,
    brk: bool,
    osm: bool,
    puntenwolk: bool,
}

fn data_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data")
}

fn json(name: &str) -> Value {
    let p = data_dir().join(name);
    serde_json::from_str(&std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))).unwrap()
}

fn locaties() -> Vec<Locatie> {
    let d = json("locaties.json");
    d["locaties"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| {
            let v = &l["verwacht"];
            Locatie {
                id: l["id"].as_str().unwrap().into(),
                naam: l["naam"].as_str().unwrap().into(),
                zoek: l["zoek"].as_str().unwrap().into(),
                lon: l["lon"].as_f64().unwrap(),
                lat: l["lat"].as_f64().unwrap(),
                land: l["land"].as_str().unwrap().into(),
                crs: l["crs"].as_str().unwrap().into(),
                breedte: l["breedte"].as_f64().unwrap(),
                hoogte: l["hoogte"].as_f64().unwrap(),
                ahn: v["ahn"].as_array().map(|a| (a[0].as_f64().unwrap(), a[1].as_f64().unwrap())),
                bgt: v["bgt"].as_bool().unwrap_or(false),
                brk: v["brk"].as_bool().unwrap_or(false),
                osm: v["osm"].as_bool().unwrap_or(false),
                puntenwolk: v["puntenwolk"].as_bool().unwrap_or(false),
            }
        })
        .collect()
}

/// De projectlocatie zoals de werkbank die maakt.
fn site(l: &Locatie) -> Site {
    let mut s = Site { label: l.naam.clone(), lon: l.lon, lat: l.lat, crs: l.crs.clone(), width: l.breedte, height: l.hoogte, ..Site::default() };
    s.reproject_origin();
    s
}

// ── Offline ─────────────────────────────────────────────────────────────────

#[test]
fn stelsel_per_locatie() {
    let mut fout = Vec::new();
    for l in locaties() {
        let op_land = crs::suggest_for(&l.land, l.lon, l.lat);
        let op_positie = crs::suggest(l.lon, l.lat);
        if op_land != l.crs {
            fout.push(format!("{}: op land '{}' → {op_land}, verwacht {}", l.id, l.land, l.crs));
        }
        if op_positie != l.crs {
            fout.push(format!("{}: op positie → {op_positie}, verwacht {}", l.id, l.crs));
        }
        if crs::known(&l.crs).is_none() {
            fout.push(format!("{}: {} is niet ingebouwd (zou zonder internet moeten werken)", l.id, l.crs));
        }
    }
    assert!(fout.is_empty(), "\n{}", fout.join("\n"));
}

#[test]
fn stelselkeuze_rond_de_grens() {
    let d = json("locaties.json");
    let g = &d["grenspunten"];
    let mut fout = Vec::new();
    let mut check = |naam: &str, lon: f64, lat: f64, verwacht: &str| {
        let s = crs::suggest(lon, lat);
        if s != verwacht {
            fout.push(format!("{naam} ({lon}, {lat}) → {s}, verwacht {verwacht}"));
        }
    };
    for p in g["nl"].as_array().unwrap() {
        check(p[0].as_str().unwrap(), p[1].as_f64().unwrap(), p[2].as_f64().unwrap(), "EPSG:28992");
    }
    for p in g["be"].as_array().unwrap() {
        check(p[0].as_str().unwrap(), p[1].as_f64().unwrap(), p[2].as_f64().unwrap(), "EPSG:31370");
    }
    for p in g["anders"].as_array().unwrap() {
        check(p[0].as_str().unwrap(), p[1].as_f64().unwrap(), p[2].as_f64().unwrap(), p[3].as_str().unwrap());
    }
    assert!(fout.is_empty(), "\n{}", fout.join("\n"));
}

#[test]
fn transformatie_heen_en_terug() {
    for l in locaties() {
        let t = Transformer::new("EPSG:4326", &l.crs).unwrap_or_else(|e| panic!("{}: {e}", l.id));
        let back = Transformer::new(&l.crs, "EPSG:4326").unwrap();
        let p = t.apply([l.lon, l.lat]).unwrap();
        let q = back.apply(p).unwrap();
        // Heen en terug binnen 1e-7 graad (≈ 1 cm); stelsels met een
        // datumverschuiving (RD, BNG, Lambert 72) halen ongeveer 1 mm.
        assert!((q[0] - l.lon).abs() < 1e-7 && (q[1] - l.lat).abs() < 1e-7, "{}: {:?} → {:?} → {:?}", l.id, [l.lon, l.lat], p, q);
        // Een vierkante kilometer blijft ongeveer een vierkante kilometer.
        let e = t.apply([l.lon + 0.01, l.lat]).unwrap();
        let n = t.apply([l.lon, l.lat + 0.01]).unwrap();
        let dx = (e[0] - p[0]).hypot(e[1] - p[1]);
        let dy = (n[0] - p[0]).hypot(n[1] - p[1]);
        let expect_dx = 0.01_f64.to_radians() * 6_378_137.0 * l.lat.to_radians().cos();
        let expect_dy = 0.01_f64.to_radians() * 6_370_000.0;
        assert!((dx / expect_dx - 1.0).abs() < 0.01, "{}: oost-west-schaal {dx:.1} m, verwacht ~{expect_dx:.1}", l.id);
        assert!((dy / expect_dy - 1.0).abs() < 0.01, "{}: noord-zuid-schaal {dy:.1} m, verwacht ~{expect_dy:.1}", l.id);
    }
}

#[test]
fn projectlocatie_in_de_tekening() {
    for l in locaties() {
        for (absoluut, f) in [(true, 1.0), (true, 1000.0), (false, 1.0), (false, 1000.0)] {
            let mut s = site(&l);
            s.absolute = absoluut;
            let r = s.drawing_rect(f);
            assert!(((r[2] - r[0]) - l.breedte * f).abs() < 1e-6, "{}: breedte in de tekening", l.id);
            assert!(((r[3] - r[1]) - l.hoogte * f).abs() < 1e-6, "{}: hoogte in de tekening", l.id);
            let o = s.to_drawing([s.origin_e, s.origin_n], f);
            if absoluut {
                assert!((o[0] - s.origin_e * f).abs() < 1e-6, "{}: absoluut basispunt", l.id);
            } else {
                assert!(o[0].abs() < 1e-9 && o[1].abs() < 1e-9, "{}: relatief basispunt niet op 0,0", l.id);
            }
            let p = [s.origin_e + 12.345, s.origin_n - 67.89];
            let back = s.from_drawing(s.to_drawing(p, f), f);
            assert!((back[0] - p[0]).abs() < 1e-6 && (back[1] - p[1]).abs() < 1e-6, "{}: heen en terug", l.id);
        }
        // Het importgebied in lon/lat bevat de locatie.
        let b = site(&l).bbox_lonlat().unwrap();
        assert!(b[0] < l.lon && l.lon < b[2] && b[1] < l.lat && l.lat < b[3], "{}: {b:?}", l.id);
    }
}

#[test]
fn bgt_kenmerken_naar_nlcs() {
    let d = json("bgt-kenmerken.json");
    let objecten = d["objecten"].as_array().unwrap();
    assert!(objecten.len() > 50);
    let mut fout = Vec::new();
    let mut lagen = std::collections::BTreeSet::new();
    for o in objecten {
        let c = o["collectie"].as_str().unwrap();
        let props = o["kenmerken"].as_object().unwrap();
        if bgt::is_label(c) {
            let obj = nlcs::bgt_label_object(c).unwrap_or_else(|| panic!("{c}: geen NLCS-object voor teksten"));
            lagen.insert(nlcs::layer(obj, nlcs::Status::B, "XX", nlcs::Element::T).name);
            continue;
        }
        let id = nlcs::bgt_lookup(c, props).map(|x| x.0).or_else(|| nlcs::bgt_default(c));
        match id.and_then(nlcs::object) {
            Some(obj) => {
                let l = nlcs::layer(obj, nlcs::Status::B, "XX", nlcs::Element::G);
                let delen: Vec<&str> = l.name.splitn(5, '-').collect();
                if delen.len() != 5 || delen[0] != "B" || delen[1] != "XX" || !l.name.ends_with("-G") || l.lineweight.is_none() {
                    fout.push(format!("{c}: ongeldige laag {}", l.name));
                }
                lagen.insert(l.name);
            }
            None => fout.push(format!("{c} {props:?}: geen NLCS-object")),
        }
    }
    println!("{} BGT-kenmerkcombinaties → {} NLCS-lagen", objecten.len(), lagen.len());
    assert!(fout.is_empty(), "\n{}", fout.join("\n"));
    // Asfalt, klinkers en tegels moeten op verschillende lagen landen.
    assert!(lagen.len() >= 15, "te weinig onderscheid: {lagen:?}");
}

/// De KLIC-voorbeeldlevering van GIS2BIM, als die naast deze repository
/// staat (of `OPEN_INFRA_KLIC` naar een levering wijst).
fn klic_voorbeeld() -> Option<PathBuf> {
    let p = std::env::var("OPEN_INFRA_KLIC").map(PathBuf::from).unwrap_or_else(|_| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../GIS2BIM/supportfiles/Sandbox/KLIC/Levering_21O072584_1.zip")
    });
    p.exists().then_some(p)
}

#[test]
fn klic_levering_naar_nlcs_en_imkl() {
    let Some(p) = klic_voorbeeld() else {
        println!("KLIC-voorbeeld niet gevonden; zet OPEN_INFRA_KLIC of clone GIS2BIM naast deze map");
        return;
    };
    let lev = klic::read(&p).unwrap();
    assert_eq!(lev.meldnummer, "21O072584");
    assert_eq!(lev.features.len(), 538);
    let mut fout = Vec::new();
    for f in &lev.features {
        let (t, k) = (klic::thema(f), klic::kind(f));
        match nlcs::klic_object(t, k) {
            Some(obj) if obj.group == "KL" || obj.group == "RI" => {}
            other => fout.push(format!("{t}/{k}: {:?}", other.map(|o| format!("{} {}", o.group, o.name)))),
        }
        // Elk thema in de levering heeft zijn eigen IMKL-kleur.
        if nlcs::imkl_rgb(t) == nlcs::imkl_rgb("overig") {
            fout.push(format!("{t}: geen IMKL-themakleur"));
        }
        // Alles ligt rond de Grote Kerk.
        for p in f.lines.iter().flat_map(|(l, _)| l.iter()).chain(f.points.iter()) {
            if !(104_700.0..105_100.0).contains(&p[0]) || !(425_200.0..425_450.0).contains(&p[1]) {
                fout.push(format!("{t}/{k}: punt {p:?} buiten het leveringsgebied"));
                break;
            }
        }
    }
    fout.dedup();
    assert!(fout.is_empty(), "\n{}", fout.join("\n"));
    // Labels: thema, beheerder, diameter en materiaal waar bekend.
    assert!(lev.features.iter().any(|f| klic::label(f).starts_with("W Evides")));
}

#[test]
fn hoogtelijnen_en_profiel_op_een_dijk() {
    // Een dijk van 5 m hoog, oost-west, op een terrein van -2 m NAP.
    let (w, h, cel) = (120usize, 80usize, 0.5);
    let mut g = ahn::Grid { x0: 100_000.0, y1: 400_040.0, cell: cel, w, h, z: vec![-2.0; w * h] };
    for r in 0..h {
        let y = g.y1 - (r as f64 + 0.5) * cel;
        let afstand = (y - 400_020.0).abs();
        let z = if afstand < 3.0 { 3.0 } else if afstand < 13.0 { 3.0 - (afstand - 3.0) / 2.0 } else { -2.0 };
        for c in 0..w {
            g.z[r * w + c] = z as f32;
        }
    }
    // Een gat (water) dat de lijnen moet onderbreken, niet laten ontsporen.
    for r in 30..34 {
        for c in 50..54 {
            g.z[r * w + c] = f32::NAN;
        }
    }
    let (lo, hi) = g.range().unwrap();
    assert_eq!((lo, hi), (-2.0, 3.0));
    let niveaus = ahn::levels((lo, hi), 1.0);
    assert_eq!(niveaus, vec![-2.0, -1.0, 0.0, 1.0, 2.0, 3.0]);
    let lijnen = ahn::contours(&g, &niveaus, 5.0);
    // Elke tussenhoogte ligt aan beide kanten van de dijk.
    for (niveau, verwacht_y) in [(-1.0, [400_009.0, 400_031.0]), (0.0, [400_011.0, 400_029.0]), (1.0, [400_013.0, 400_027.0])] {
        let (_, l) = lijnen.iter().find(|(n, _)| *n == niveau).unwrap_or_else(|| panic!("geen hoogtelijn {niveau}"));
        for (pts, _) in l {
            for p in pts {
                let dichtst = verwacht_y.iter().map(|y| (p[1] - y).abs()).fold(f64::MAX, f64::min);
                assert!(dichtst < 0.6, "hoogtelijn {niveau} op y={} (verwacht {verwacht_y:?})", p[1]);
            }
        }
    }
    // Het profiel dwars over de dijk: stations elke meter, dijkkruin +3.
    let route = [[100_030.0, 400_000.0], [100_030.0, 400_040.0]];
    let st = ahn::stations(&route, 1.0);
    assert_eq!(st.len(), 41);
    let kruin = st.iter().map(|(_, p)| g.sample(p[0], p[1]).unwrap_or(f64::MIN)).fold(f64::MIN, f64::max);
    assert!((kruin - 3.0).abs() < 1e-6, "kruin {kruin}");
}

#[test]
fn tekenblad_kiest_het_formaat() {
    let l = &locaties()[0];
    for (breedte, hoogte, schaal, formaat) in [(80.0, 60.0, 500.0, "A3"), (150.0, 120.0, 500.0, "A2"), (300.0, 250.0, 500.0, "A1"), (450.0, 350.0, 500.0, "A0"), (300.0, 250.0, 1000.0, "A2")] {
        let mut s = site(l);
        s.width = breedte;
        s.height = hoogte;
        assert_eq!(tekenblad::paper_for(&s, schaal).0, formaat, "{breedte} × {hoogte} m op 1:{schaal}");
    }
}

#[test]
fn nap_notatie() {
    use open_infra::hoogte::nap;
    assert_eq!(nap(2.434, 2), "+2,43");
    assert_eq!(nap(-6.1, 2), "-6,10");
    assert_eq!(nap(322.5, 1), "+322,5");
}

// ── Online ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
enum Status {
    Ok,
    /// Geen data, zoals verwacht buiten de dekking.
    Leeg,
    /// Openbare dienst overbelast of traag: waarschuwing.
    Netwerk,
    Fout,
}

struct Regel {
    locatie: String,
    bron: &'static str,
    status: Status,
    aantal: usize,
    detail: String,
    seconden: f64,
}

fn netwerkprobleem(e: &str) -> bool {
    let e = e.to_ascii_lowercase();
    ["timeout", "timed out", "429", "502", "503", "504", "overbelast", "too many", "connection", "dns", "niet bereikbaar"].iter().any(|k| e.contains(k))
}

struct Rapport {
    regels: Vec<Regel>,
}

impl Rapport {
    /// Een bron proberen; `verwacht` zegt of er data moet zijn.
    fn bron(&mut self, l: &Locatie, bron: &'static str, verwacht: bool, f: impl FnOnce() -> Result<(usize, String), String>) {
        let t = Instant::now();
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).unwrap_or_else(|p| {
            Err(format!("PANIEK: {}", p.downcast_ref::<String>().cloned().or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default()))
        });
        let seconden = t.elapsed().as_secs_f64();
        let (status, aantal, detail) = match r {
            Ok((n, d)) if n > 0 && verwacht => (Status::Ok, n, d),
            Ok((n, d)) if n > 0 => (Status::Fout, n, format!("onverwacht data: {d}")),
            Ok((_, d)) if verwacht => (Status::Fout, 0, format!("geen data: {d}")),
            Ok((_, d)) => (Status::Leeg, 0, d),
            Err(e) if e.starts_with("PANIEK") => (Status::Fout, 0, e),
            Err(e) if netwerkprobleem(&e) => (Status::Netwerk, 0, e),
            Err(e) if !verwacht => (Status::Leeg, 0, format!("netjes geweigerd: {e}")),
            Err(e) => (Status::Fout, 0, e),
        };
        let teken = match status {
            Status::Ok => "✔",
            Status::Leeg => "·",
            Status::Netwerk => "⚠",
            Status::Fout => "✘",
        };
        println!("  {teken} {:<12} {:<11} {:>7} {:>6.1}s  {}", l.id, bron, aantal, seconden, detail.chars().take(110).collect::<String>());
        self.regels.push(Regel { locatie: l.id.clone(), bron, status, aantal, detail, seconden });
    }

    fn schrijf(&self) -> PathBuf {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/locatietest");
        std::fs::create_dir_all(&dir).unwrap();
        let mut md = String::from("# Locatietest Open Infra\n\n");
        md.push_str(&format!("Versie {}. ✔ data zoals verwacht · leeg zoals verwacht ⚠ dienst overbelast ✘ fout\n\n", env!("CARGO_PKG_VERSION")));
        let bronnen: Vec<&str> = {
            let mut b: Vec<&str> = Vec::new();
            for r in &self.regels {
                if !b.contains(&r.bron) {
                    b.push(r.bron);
                }
            }
            b
        };
        md.push_str(&format!("| locatie | {} |\n|---|{}\n", bronnen.join(" | "), "---|".repeat(bronnen.len())));
        let mut locs: Vec<&str> = Vec::new();
        for r in &self.regels {
            if !locs.contains(&r.locatie.as_str()) {
                locs.push(&r.locatie);
            }
        }
        for l in &locs {
            let cellen: Vec<String> = bronnen
                .iter()
                .map(|b| match self.regels.iter().find(|r| r.locatie == *l && r.bron == *b) {
                    Some(r) => {
                        let t = match r.status {
                            Status::Ok => "✔",
                            Status::Leeg => "·",
                            Status::Netwerk => "⚠",
                            Status::Fout => "✘",
                        };
                        if r.aantal > 0 { format!("{t} {}", r.aantal) } else { t.to_string() }
                    }
                    None => String::new(),
                })
                .collect();
            md.push_str(&format!("| {l} | {} |\n", cellen.join(" | ")));
        }
        md.push_str("\n## Details\n\n| locatie | bron | status | aantal | tijd (s) | detail |\n|---|---|---|---|---|---|\n");
        for r in &self.regels {
            md.push_str(&format!("| {} | {} | {:?} | {} | {:.1} | {} |\n", r.locatie, r.bron, r.status, r.aantal, r.seconden, r.detail.replace('|', "/")));
        }
        let p = dir.join("rapport.md");
        std::fs::write(&p, md).unwrap();
        let js: Vec<Value> = self
            .regels
            .iter()
            .map(|r| serde_json::json!({"locatie": r.locatie, "bron": r.bron, "status": format!("{:?}", r.status), "aantal": r.aantal, "seconden": r.seconden, "detail": r.detail}))
            .collect();
        std::fs::write(dir.join("rapport.json"), serde_json::to_string_pretty(&js).unwrap()).unwrap();
        p
    }
}

/// Elk object raakt het importgebied: zijn omhullende overlapt het (een lange
/// weg kan het gebied kruisen zonder er een hoekpunt in te hebben).
fn in_bbox(features: &[vector::Feature], b: [f64; 4], marge: f64) -> bool {
    features.iter().all(|f| {
        let mut e = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
        for p in f.lines.iter().flat_map(|(l, _)| l.iter()).chain(f.points.iter()) {
            e = [e[0].min(p[0]), e[1].min(p[1]), e[2].max(p[0]), e[3].max(p[1])];
        }
        e[0] <= b[2] + marge && e[2] >= b[0] - marge && e[1] <= b[3] + marge && e[3] >= b[1] - marge
    })
}

#[test]
#[ignore]
fn locaties_online() {
    let alleen: Option<Vec<String>> = std::env::var("LOCATIES").ok().map(|s| s.split(',').map(str::to_string).collect());
    let mut rapport = Rapport { regels: Vec::new() };
    for l in locaties() {
        if alleen.as_ref().is_some_and(|a| !a.contains(&l.id)) {
            continue;
        }
        println!("{} — {}", l.id, l.naam);
        let s = site(&l);
        s.register_crs();
        let in_nl = l.ahn.is_some();

        rapport.bron(&l, "zoeken", true, || {
            let hits = geocode::search(&l.zoek)?;
            let h = hits.first().ok_or("geen resultaat")?;
            let afstand = ((h.lon - l.lon) * l.lat.to_radians().cos()).hypot(h.lat - l.lat) * 111_000.0;
            if afstand > 3_000.0 {
                return Err(format!("eerste treffer '{}' ligt {:.1} km verderop", h.label, afstand / 1000.0));
            }
            if !l.land.is_empty() && !h.country.is_empty() && h.country != l.land {
                return Err(format!("land {} in plaats van {}", h.country, l.land));
            }
            Ok((hits.len(), format!("'{}' op {:.0} m ({})", h.label, afstand, h.source)))
        });

        rapport.bron(&l, "kaartbeeld", true, || {
            let osm = servers::find("osm-tiles").ok_or("server osm-tiles ontbreekt")?;
            let p = raster::preview_xyz(&s, &osm)?;
            let px = p.size[0] * p.size[1];
            Ok((px, format!("{} × {} px OSM", p.size[0], p.size[1])))
        });

        if in_nl {
            rapport.bron(&l, "luchtfoto", true, || {
                let srv = servers::find("pdok-luchtfoto").ok_or("server pdok-luchtfoto ontbreekt")?;
                let caps = open_infra::gis::caps::fetch_wms(&srv.url)?;
                let layer = caps.layers.iter().find(|x| x.name == srv.default_layer).ok_or("laag ontbreekt")?;
                let p = raster::preview_wms(&s, &caps, layer, "image/jpeg", 512)?;
                // Een luchtfoto is niet effen.
                let first = &p.rgba[0..4];
                let variatie = p.rgba.chunks(4).filter(|c| (c[0] as i32 - first[0] as i32).abs() > 20).count();
                Ok((variatie, format!("{} × {} px, {variatie} afwijkende pixels", p.size[0], p.size[1])))
            });
        }

        rapport.bron(&l, "bgt", l.bgt, || {
            let data = bgt::fetch(&s, &["wegdeel", "pand", "waterdeel", "begroeidterreindeel"], &|_| {})?;
            let n: usize = data.iter().map(|(_, f)| f.len()).sum();
            let b = basisdata::rd_bbox(&s)?;
            let mut ongekoppeld = 0;
            for (c, fs) in &data {
                if !in_bbox(fs, b, 50.0) {
                    return Err(format!("{c}: objecten buiten het importgebied"));
                }
                if fs.iter().any(|f| !bgt::is_current(f)) {
                    return Err(format!("{c}: historische versies meegekomen"));
                }
                ongekoppeld += fs.iter().filter(|f| nlcs::bgt_lookup(c, &f.props).is_none() && nlcs::bgt_default(c).is_none()).count();
            }
            if ongekoppeld > 0 {
                return Err(format!("{ongekoppeld} objecten zonder NLCS-object"));
            }
            let per: Vec<String> = data.iter().map(|(c, f)| format!("{c} {}", f.len())).collect();
            Ok((n, per.join(", ")))
        });

        rapport.bron(&l, "brk", l.brk, || {
            let k = basisdata::cadastre_fetch(&s, true, &|_| {})?;
            Ok((k.boundaries.len(), format!("{} grenzen, {} percelen", k.boundaries.len(), k.parcels.len())))
        });

        rapport.bron(&l, "osm", l.osm, || {
            let themes: Vec<&osm::Theme> = osm::THEMES.iter().filter(|t| t.id == "gebouwen" || t.id == "wegen").collect();
            let per = osm::fetch(&s, &themes, &|_| {})?;
            let n: usize = per.iter().map(Vec::len).sum();
            Ok((n, format!("gebouwen {}, wegen {}", per[0].len(), per.get(1).map_or(0, Vec::len))))
        });

        let rd = basisdata::rd_bbox(&s);
        rapport.bron(&l, "ahn", in_nl, || {
            let b = rd.clone()?;
            let g = ahn::fetch(b, ahn::Model::Dtm, 1.0, &|_| {})?;
            if g.coverage() < 0.05 {
                return Ok((0, format!("{:.0} % dekking", g.coverage() * 100.0)));
            }
            let (lo, hi) = g.range().ok_or("geen hoogtes")?;
            if let Some((min, max)) = l.ahn {
                if lo < min - 0.5 || hi > max + 0.5 {
                    return Err(format!("hoogtes {lo:.2} – {hi:.2} m NAP buiten het verwachte bereik {min} – {max}"));
                }
            }
            let lijnen = ahn::contours(&g.smoothed(), &ahn::levels((lo, hi), 0.5), 10.0);
            let n: usize = lijnen.iter().map(|(_, v)| v.len()).sum();
            Ok((g.w * g.h, format!("{lo:.2} – {hi:.2} m NAP, {:.0} % dekking, {n} hoogtelijnen (0,5 m)", g.coverage() * 100.0)))
        });

        rapport.bron(&l, "profiel", in_nl, || {
            let b = rd.clone()?;
            let y = (b[1] + b[3]) / 2.0;
            let st = ahn::profile(&[[b[0] + 10.0, y], [b[2] - 10.0, y]], 2.0, true, 0.5, &|_| {})?;
            let met = st.iter().filter(|s| s.ground.is_some()).count();
            Ok((met, format!("{} stations, {met} met maaiveld, {} met oppervlak", st.len(), st.iter().filter(|s| s.surface.is_some()).count())))
        });

        rapport.bron(&l, "puntenwolk", l.puntenwolk, || {
            let b = rd.clone()?;
            let (cx, cy) = ((b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0);
            let opt = pointcloud::Options { spacing: 1.0, max_points: 200_000, ..pointcloud::Options::default() };
            let c = pointcloud::fetch([cx - 20.0, cy - 20.0, cx + 20.0, cy + 20.0], &opt, &|_| {})?;
            let mut klassen: Vec<u8> = c.class.clone();
            klassen.sort();
            klassen.dedup();
            let path = std::env::temp_dir().join(format!("open-infra-{}.rcs", l.id));
            pointcloud::write_rcs(&path, &c.xyz, &pointcloud::colours(&c, pointcloud::Colouring::Classification), &c.intensity)?;
            let grootte = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            let _ = std::fs::remove_file(&path);
            let terugval = if c.fallback_tiles > 0 { format!(", {} tegel(s) uit AHN4", c.fallback_tiles) } else { String::new() };
            Ok((c.xyz.len(), format!("klassen {klassen:?}, .rcs {} kB{terugval}", grootte / 1024)))
        });

        rapport.bron(&l, "tekenblad", true, || {
            let (formaat, _, _) = tekenblad::paper_for(&s, 500.0);
            Ok((1, format!("{formaat} bij 1:500")))
        });
    }
    let pad = rapport.schrijf();
    println!("\nRapport: {}", pad.display());
    let fouten: Vec<String> = rapport.regels.iter().filter(|r| r.status == Status::Fout).map(|r| format!("{} / {}: {}", r.locatie, r.bron, r.detail)).collect();
    let waarschuwingen = rapport.regels.iter().filter(|r| r.status == Status::Netwerk).count();
    println!("{} controles, {} fouten, {waarschuwingen} waarschuwingen (overbelaste diensten)", rapport.regels.len(), fouten.len());
    assert!(fouten.is_empty(), "\n{}", fouten.join("\n"));
}
