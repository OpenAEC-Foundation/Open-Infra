//! Map server catalogue: presets for the Netherlands (PDOK), Flanders and
//! worldwide sources, plus servers the user adds (stored in the config dir).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ServerKind {
    Wms,
    Wfs,
    Xyz,
}

impl ServerKind {
    pub fn label(self) -> &'static str {
        match self {
            ServerKind::Wms => "WMS",
            ServerKind::Wfs => "WFS",
            ServerKind::Xyz => "XYZ-tegels",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MapServer {
    pub id: String,
    pub kind: ServerKind,
    pub name: String,
    pub region: String,
    /// Service endpoint (WMS/WFS) or tile template with {z}/{x}/{y} (XYZ).
    pub url: String,
    #[serde(default)]
    pub attribution: String,
    /// Layer / feature type preselected in the workbench.
    #[serde(default)]
    pub default_layer: String,
    /// XYZ only: highest zoom level the server provides.
    #[serde(default)]
    pub max_zoom: Option<u32>,
    #[serde(skip)]
    pub builtin: bool,
}

fn preset(id: &str, kind: ServerKind, region: &str, name: &str, url: &str, layer: &str, attribution: &str) -> MapServer {
    MapServer {
        id: id.into(),
        kind,
        name: name.into(),
        region: region.into(),
        url: url.into(),
        attribution: attribution.into(),
        default_layer: layer.into(),
        max_zoom: if kind == ServerKind::Xyz { Some(19) } else { None },
        builtin: true,
    }
}

pub fn presets() -> Vec<MapServer> {
    use ServerKind::*;
    const NL: &str = "Nederland";
    vec![
        // Nederland — PDOK WMS
        preset("pdok-luchtfoto", Wms, NL, "PDOK Luchtfoto RGB (actueel, 8 cm)", "https://service.pdok.nl/hwh/luchtfotorgb/wms/v1_0", "Actueel_orthoHR", "Beeldmateriaal.nl / PDOK"),
        preset("pdok-kadastralekaart-wms", Wms, NL, "PDOK Kadastrale kaart", "https://service.pdok.nl/kadaster/kadastralekaart/wms/v5_0", "Kadastralekaart", "Kadaster / PDOK"),
        preset("pdok-bag-wms", Wms, NL, "PDOK BAG (panden)", "https://service.pdok.nl/lv/bag/wms/v2_0", "pand", "Kadaster / PDOK"),
        preset("pdok-top10nl-wms", Wms, NL, "PDOK TOP10NL topografie", "https://service.pdok.nl/brt/top10nl/wms/v1_0", "", "Kadaster / PDOK"),
        preset("pdok-ahn-wms", Wms, NL, "PDOK AHN hoogtebestand (DTM 0,5 m)", "https://service.pdok.nl/rws/ahn/wms/v1_0", "dtm_05m", "AHN / Rijkswaterstaat / PDOK"),
        // Nederland — PDOK WFS
        preset("pdok-bag-wfs", Wfs, NL, "PDOK BAG — panden en verblijfsobjecten", "https://service.pdok.nl/lv/bag/wfs/v2_0", "bag:pand", "Kadaster / PDOK"),
        preset("pdok-kadastralekaart-wfs", Wfs, NL, "PDOK Kadastrale kaart — percelen", "https://service.pdok.nl/kadaster/kadastralekaart/wfs/v5_0", "kadastralekaart:Perceel", "Kadaster / PDOK"),
        preset("pdok-nwbwegen-wfs", Wfs, NL, "PDOK NWB Wegen (wegvakken, hectopunten)", "https://service.pdok.nl/rws/nwbwegen/wfs/v1_0", "nwbwegen:wegvakken", "Rijkswaterstaat / PDOK"),
        preset("pdok-nwbvaarwegen-wfs", Wfs, NL, "PDOK NWB Vaarwegen", "https://service.pdok.nl/rws/nwbvaarwegen/wfs/v1_0", "", "Rijkswaterstaat / PDOK"),
        preset("pdok-spoorwegen-wfs", Wfs, NL, "PDOK Spoorwegen (ProRail)", "https://service.pdok.nl/prorail/spoorwegen/wfs/v1_0", "spoorwegen:spooras", "ProRail / PDOK"),
        preset("pdok-bestuurlijk-wfs", Wfs, NL, "PDOK Bestuurlijke gebieden", "https://service.pdok.nl/kadaster/bestuurlijkegebieden/wfs/v1_0", "", "Kadaster / PDOK"),
        preset("pdok-natura2000-wfs", Wfs, NL, "PDOK Natura 2000", "https://service.pdok.nl/rvo/natura2000/wfs/v1_0", "", "RVO / PDOK"),
        // België — Vlaanderen
        preset("vl-ortho-wms", Wms, "België (Vlaanderen)", "Digitaal Vlaanderen — Orthofoto", "https://geo.api.vlaanderen.be/OMWRGBMRVL/wms", "", "Digitaal Vlaanderen"),
        preset("vl-grb-wfs", Wfs, "België (Vlaanderen)", "Digitaal Vlaanderen — GRB", "https://geo.api.vlaanderen.be/GRB/wfs", "", "Digitaal Vlaanderen"),
        // Wereldwijd
        preset("osm-tiles", Xyz, "Wereld", "OpenStreetMap (tegels)", "https://tile.openstreetmap.org/{z}/{x}/{y}.png", "", "© OpenStreetMap-bijdragers (ODbL)"),
        preset("terrestris-osm-wms", Wms, "Wereld", "terrestris OSM WMS", "https://ows.terrestris.de/osm/service", "OSM-WMS", "© terrestris, © OpenStreetMap-bijdragers"),
        preset("eox-s2-wms", Wms, "Wereld", "EOX Sentinel-2 cloudless (satelliet)", "https://tiles.maps.eox.at/wms", "s2cloudless-2023", "Sentinel-2 cloudless by EOX (CC BY-NC-SA 4.0)"),
        preset("nasa-gibs-wms", Wms, "Wereld", "NASA GIBS (aardobservatie)", "https://gibs.earthdata.nasa.gov/wms/epsg4326/best/wms.cgi", "BlueMarble_ShadedRelief_Bathymetry", "NASA EOSDIS GIBS"),
        preset("gebco-wms", Wms, "Wereld", "GEBCO bathymetrie", "https://wms.gebco.net/mapserv", "GEBCO_LATEST_SUB_ICE_TOPO", "GEBCO Compilation Group"),
    ]
}

/// `<config>/OpenCADStudio/open-infra/` — beside the host's own settings.
pub fn config_dir() -> PathBuf {
    let base = if let Some(a) = std::env::var_os("APPDATA") {
        PathBuf::from(a)
    } else if cfg!(target_os = "macos") {
        PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join("Library/Application Support")
    } else if let Some(x) = std::env::var_os("XDG_CONFIG_HOME") {
        PathBuf::from(x)
    } else {
        PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config")
    };
    base.join("OpenCADStudio").join("open-infra")
}

fn user_file() -> PathBuf {
    config_dir().join("servers.json")
}

pub fn load_user() -> Vec<MapServer> {
    std::fs::read_to_string(user_file())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_user(servers: &[MapServer]) -> Result<(), String> {
    let dir = config_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let json = serde_json::to_string_pretty(servers).map_err(|e| e.to_string())?;
    std::fs::write(user_file(), json).map_err(|e| e.to_string())
}

pub fn all() -> Vec<MapServer> {
    let mut v = presets();
    v.extend(load_user());
    v
}

pub fn find(id: &str) -> Option<MapServer> {
    all().into_iter().find(|s| s.id.eq_ignore_ascii_case(id))
}
