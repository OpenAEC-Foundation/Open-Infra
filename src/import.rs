//! Import operations shared by the workbench window and the command line:
//! fetch with the engine, convert with `emit`, write through `bridge`.

use ocs_plugin_api::host::codec::Color;

use crate::bridge::{self, Target};
use crate::emit;
use crate::gis::caps::{WfsCaps, WfsType, WmsCaps, WmsLayer};
use crate::gis::raster::{self, Options as RasterOptions};
use crate::gis::servers::{self, MapServer, ServerKind};
use crate::gis::site::Site;
use crate::gis::vector::{self, Feature, Options as VectorOptions};
use crate::gis::{caps, osm, wfs};

fn target() -> Result<Target, String> {
    bridge::target().ok_or_else(|| "geen tekening actief — klik in Open CAD Studio op een knop van het GIS-tabblad".into())
}

pub fn wms(site: &Site, server: &MapServer, caps: &WmsCaps, layer: &WmsLayer, opt: &RasterOptions, fade: u8, progress: &dyn Fn(String)) -> Result<String, String> {
    let t = target()?;
    let placed = raster::import_wms(site, caps, layer, opt, &bridge::image_dir(), progress)?;
    let layer_name = emit::layer_name("GIS-WMS", &layer.title);
    bridge::ensure_layer(&layer_name, Color::from_index(8));
    let handles = bridge::add_entities_handles(&format!("GIS WMS {}", layer.title), emit::images(site, &placed, &layer_name, t.unit_factor, fade))?;
    let n = handles.len();
    bridge::send_to_back(&handles);
    bridge::zoom_extents();
    Ok(format!("{n} kaartdeel/delen van {} op laag {layer_name} ({}).", server.name, server.attribution))
}

pub fn xyz(site: &Site, server: &MapServer, opt: &RasterOptions, fade: u8, progress: &dyn Fn(String)) -> Result<String, String> {
    let t = target()?;
    let placed = raster::import_xyz(site, server, opt, &bridge::image_dir(), progress)?;
    let layer_name = emit::layer_name("GIS", &server.name);
    bridge::ensure_layer(&layer_name, Color::from_index(8));
    let handles = bridge::add_entities_handles(&format!("GIS {}", server.name), emit::images(site, &placed, &layer_name, t.unit_factor, fade))?;
    bridge::send_to_back(&handles);
    bridge::zoom_extents();
    Ok(format!("{} op laag {layer_name} ({}).", server.name, server.attribution))
}

pub struct VectorStyle {
    pub aci: i16,
    pub label_field: Option<String>,
    /// Text height in metres.
    pub text_m: f64,
    pub clip: bool,
}

pub fn place_features(site: &Site, features: &[Feature], data_crs: &str, layer: &str, label: &str, style: &VectorStyle) -> Result<usize, String> {
    let t = target()?;
    let to_site = vector::data_to_site(site, data_crs, features)?;
    let geom = vector::to_drawing(
        site,
        features,
        &*to_site,
        &VectorOptions { unit_factor: t.unit_factor, clip: style.clip, label_field: style.label_field.clone() },
    );
    if geom.is_empty() {
        return Ok(0);
    }
    bridge::ensure_layer(layer, Color::from_index(style.aci));
    bridge::add_entities(label, emit::geometry(&geom, layer, style.text_m * t.unit_factor))
}

pub fn wfs_fetch(site: &Site, caps: &WfsCaps, ft: &WfsType, limit: usize, progress: &dyn Fn(String)) -> Result<wfs::WfsResult, String> {
    wfs::fetch(site, caps, ft, limit, progress)
}

pub fn wfs_place(site: &Site, ft: &WfsType, result: &wfs::WfsResult, style: &VectorStyle) -> Result<String, String> {
    let layer = emit::layer_name("GIS", ft.name.rsplit(':').next().unwrap_or(&ft.name));
    let n = place_features(site, &result.features, &result.data_crs, &layer, &format!("GIS WFS {}", ft.title), style)?;
    bridge::zoom_extents();
    Ok(format!("{n} objecten van {} op laag {layer}.", ft.title))
}

pub fn osm_fetch(site: &Site, theme_ids: &[&str], progress: &dyn Fn(String)) -> Result<Vec<Vec<Feature>>, String> {
    let themes: Vec<&osm::Theme> = osm::THEMES.iter().filter(|t| theme_ids.contains(&t.id)).collect();
    if themes.is_empty() {
        return Err("kies minstens één thema".into());
    }
    osm::fetch(site, &themes, progress)
}

/// Place OpenStreetMap features fetched for `theme_ids` (same order).
pub fn osm_place(site: &Site, theme_ids: &[&str], per_theme: &[Vec<Feature>], labels: bool, text_m: f64, clip: bool) -> Result<String, String> {
    target()?;
    let mut summary = Vec::new();
    for (id, features) in theme_ids.iter().zip(per_theme) {
        let Some(theme) = osm::THEMES.iter().find(|t| t.id == *id) else { continue };
        let style = VectorStyle { aci: theme.aci, label_field: labels.then(|| theme.label_tag.map(str::to_string)).flatten(), text_m, clip };
        let n = place_features(site, features, "EPSG:4326", theme.layer, &format!("GIS OSM {}", theme.label), &style)?;
        summary.push(format!("{}: {n}", theme.label));
    }
    bridge::zoom_extents();
    Ok(format!("OpenStreetMap — {} (© OpenStreetMap-bijdragers, ODbL).", summary.join(", ")))
}

pub fn osm(site: &Site, theme_ids: &[&str], labels: bool, text_m: f64, clip: bool, progress: &dyn Fn(String)) -> Result<String, String> {
    target()?;
    let per_theme = osm_fetch(site, theme_ids, progress)?;
    osm_place(site, theme_ids, &per_theme, labels, text_m, clip)
}

/// A base point marker in the drawing (the GeoOrigin of the FreeCAD workbench):
/// cross, circle and the coordinates, on layer GIS-BASISPUNT.
pub fn geo_origin(site: &Site) -> Result<String, String> {
    use ocs_plugin_api::host::codec::{Circle, EntityType, Line, Text, Vector3};
    let t = target()?;
    let f = t.unit_factor;
    let p = site.to_drawing([site.origin_e, site.origin_n], f);
    let r = (site.width.max(site.height) / 100.0).clamp(0.5, 25.0) * f;
    let layer = "GIS-BASISPUNT";
    bridge::ensure_layer(layer, Color::from_index(1));
    let mut ents = vec![
        EntityType::Line(Line::from_points(Vector3::new(p[0] - 2.0 * r, p[1], 0.0), Vector3::new(p[0] + 2.0 * r, p[1], 0.0))),
        EntityType::Line(Line::from_points(Vector3::new(p[0], p[1] - 2.0 * r, 0.0), Vector3::new(p[0], p[1] + 2.0 * r, 0.0))),
        EntityType::Circle(Circle::from_center_radius(Vector3::new(p[0], p[1], 0.0), r)),
        EntityType::Text(
            Text::with_value(format!("GIS-basispunt {}  X {:.3}  Y {:.3}", site.crs, site.origin_e, site.origin_n), Vector3::new(p[0] + 2.2 * r, p[1] + 0.4 * r, 0.0))
                .with_height(0.6 * r),
        ),
    ];
    for e in &mut ents {
        e.common_mut().layer = layer.into();
        e.common_mut().color = Color::ByLayer;
    }
    bridge::add_entities("GIS basispunt", ents)?;
    Ok(format!("Basispunt geplaatst op laag {layer}."))
}

/// A dataset that can be imported in one go (ribbon "Snel", PDOK (NL) page).
pub enum Dataset {
    Raster { server: &'static str },
    Wfs { server: &'static str, typename: &'static str, label: Option<&'static str>, aci: i16 },
}

pub struct Catalogued {
    pub id: &'static str,
    pub label: &'static str,
    pub group: &'static str,
    pub default_on: bool,
    pub data: Dataset,
}

pub const PDOK: &[Catalogued] = &[
    Catalogued { id: "luchtfoto", label: "Luchtfoto 8 cm (Beeldmateriaal)", group: "Kaartbeelden", default_on: true, data: Dataset::Raster { server: "pdok-luchtfoto" } },
    Catalogued { id: "kadasterkaart", label: "Kadastrale kaart", group: "Kaartbeelden", default_on: false, data: Dataset::Raster { server: "pdok-kadastralekaart-wms" } },
    Catalogued { id: "ahn", label: "AHN hoogtekaart (DTM 0,5 m)", group: "Kaartbeelden", default_on: false, data: Dataset::Raster { server: "pdok-ahn-wms" } },
    Catalogued { id: "bag", label: "BAG panden", group: "Vectordata", default_on: true, data: Dataset::Wfs { server: "pdok-bag-wfs", typename: "bag:pand", label: None, aci: 30 } },
    Catalogued { id: "percelen", label: "Kadastrale percelen (met perceelnummer)", group: "Vectordata", default_on: true, data: Dataset::Wfs { server: "pdok-kadastralekaart-wfs", typename: "kadastralekaart:Perceel", label: Some("perceelnummer"), aci: 3 } },
    Catalogued { id: "nwb", label: "NWB wegvakken (met straatnaam)", group: "Infra", default_on: true, data: Dataset::Wfs { server: "pdok-nwbwegen-wfs", typename: "nwbwegen:wegvakken", label: Some("stt_naam"), aci: 2 } },
    Catalogued { id: "hecto", label: "NWB hectometerpunten", group: "Infra", default_on: false, data: Dataset::Wfs { server: "pdok-nwbwegen-wfs", typename: "nwbwegen:hectopunten", label: Some("hectomtrng"), aci: 2 } },
    Catalogued { id: "spoor", label: "Spoorassen (ProRail)", group: "Infra", default_on: false, data: Dataset::Wfs { server: "pdok-spoorwegen-wfs", typename: "spoorwegen:spooras", label: None, aci: 6 } },
    Catalogued { id: "spoorkm", label: "Spoorkilometrering (ProRail)", group: "Infra", default_on: false, data: Dataset::Wfs { server: "pdok-spoorwegen-wfs", typename: "spoorwegen:kilometrering", label: Some("kmGeocodeT"), aci: 6 } },
    Catalogued { id: "vaarwegen", label: "NWB vaarwegen", group: "Infra", default_on: false, data: Dataset::Wfs { server: "pdok-nwbvaarwegen-wfs", typename: "", label: None, aci: 5 } },
];

/// Import one catalogued dataset with sensible defaults.
pub fn run_dataset(site: &Site, data: &Dataset, progress: &dyn Fn(String)) -> Result<String, String> {
    match data {
        Dataset::Raster { server } => {
            let srv = servers::find(server).ok_or_else(|| format!("server {server} onbekend"))?;
            if srv.kind == ServerKind::Xyz {
                return xyz(site, &srv, &RasterOptions { resolution: 0.5, format: "image/png".into() }, 0, progress);
            }
            let caps = caps::fetch_wms(&srv.url)?;
            let layer = caps.layers.iter().find(|l| l.name == srv.default_layer).or(caps.layers.first()).cloned().ok_or("geen lagen")?;
            let opt = RasterOptions { resolution: 0.25, format: raster::default_format(&caps, &layer.name) };
            wms(site, &srv, &caps, &layer, &opt, 0, progress)
        }
        Dataset::Wfs { server, typename, label, aci } => {
            let srv = servers::find(server).ok_or_else(|| format!("server {server} onbekend"))?;
            let caps = caps::fetch_wfs(&srv.url)?;
            let ft = if typename.is_empty() { caps.types.first().cloned() } else { caps.types.iter().find(|t| t.name.eq_ignore_ascii_case(typename)).cloned() }
                .ok_or_else(|| format!("objecttype {typename} niet gevonden"))?;
            let res = wfs_fetch(site, &caps, &ft, 20_000, progress)?;
            let style = VectorStyle { aci: *aci, label_field: label.map(str::to_string), text_m: 1.0, clip: true };
            wfs_place(site, &ft, &res, &style)
        }
    }
}

/// Aerial photo for the site: PDOK 8 cm in the Netherlands, Sentinel-2
/// (10 m) elsewhere — the "Import Aerialphoto" option of the location dialog.
pub fn aerial(site: &Site, progress: &dyn Fn(String)) -> Result<String, String> {
    let in_nl = site.lon > 3.2 && site.lon < 7.3 && site.lat > 50.7 && site.lat < 53.7;
    if in_nl {
        return run_dataset(site, &Dataset::Raster { server: "pdok-luchtfoto" }, progress);
    }
    let srv = servers::find("eox-s2-wms").ok_or("server eox-s2-wms onbekend")?;
    let caps = caps::fetch_wms(&srv.url)?;
    let layer = caps.layers.iter().find(|l| l.name == srv.default_layer).or(caps.layers.first()).cloned().ok_or("geen lagen")?;
    let res = (site.width.max(site.height) / 2048.0).max(5.0);
    wms(site, &srv, &caps, &layer, &RasterOptions { resolution: res, format: "image/jpeg".into() }, 0, progress)
}
