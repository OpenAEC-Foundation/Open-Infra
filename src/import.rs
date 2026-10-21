//! Import operations shared by the workbench window and the command line:
//! fetch with the engine, convert with `emit`, write through `bridge`.

use ocs_plugin_api::host::codec::Color;

use crate::bridge::{self, Target};
use crate::emit;
use crate::gis::caps::{WfsCaps, WfsType, WmsCaps, WmsLayer};
use crate::gis::raster::{self, Options as RasterOptions};
use crate::gis::servers::MapServer;
use crate::gis::site::Site;
use crate::gis::vector::{self, Feature, Options as VectorOptions};
use crate::gis::{osm, wfs};

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

pub fn osm(site: &Site, theme_ids: &[&str], labels: bool, text_m: f64, clip: bool, progress: &dyn Fn(String)) -> Result<String, String> {
    target()?;
    let themes: Vec<&osm::Theme> = osm::THEMES.iter().filter(|t| theme_ids.contains(&t.id)).collect();
    if themes.is_empty() {
        return Err("kies minstens één thema".into());
    }
    let per_theme = osm::fetch(site, &themes, progress)?;
    let mut summary = Vec::new();
    for (theme, features) in themes.iter().zip(per_theme) {
        let style = VectorStyle { aci: theme.aci, label_field: labels.then(|| theme.label_tag.map(str::to_string)).flatten(), text_m, clip };
        let n = place_features(site, &features, "EPSG:4326", theme.layer, &format!("GIS OSM {}", theme.label), &style)?;
        summary.push(format!("{}: {n}", theme.label));
    }
    bridge::zoom_extents();
    Ok(format!("OpenStreetMap — {} (© OpenStreetMap-bijdragers, ODbL).", summary.join(", ")))
}
