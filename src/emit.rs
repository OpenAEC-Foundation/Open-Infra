//! GIS engine output → Open CAD Studio (DWG) entities.

use ocs_plugin_api::host::codec::{
    entities::{RasterImage, TextHorizontalAlignment, TextVerticalAlignment},
    Circle, Color, EntityType, LwPolyline, Text, Vector2, Vector3,
};

use crate::gis::raster::PlacedImage;
use crate::gis::site::Site;
use crate::gis::vector::Geometry;

fn on_layer(mut e: EntityType, layer: &str) -> EntityType {
    let c = e.common_mut();
    c.layer = layer.to_string();
    c.color = Color::ByLayer;
    e
}

/// Polylines, point markers and centred labels. `text_h` is in drawing units.
pub fn geometry(g: &Geometry, layer: &str, text_h: f64) -> Vec<EntityType> {
    let mut out = Vec::with_capacity(g.polylines.len() + g.points.len() + g.labels.len());
    for (pts, closed) in &g.polylines {
        let mut lw = LwPolyline::new();
        for p in pts {
            lw.add_point_with_bulge(Vector2::new(p[0], p[1]), 0.0);
        }
        lw.is_closed = *closed;
        out.push(on_layer(EntityType::LwPolyline(lw), layer));
    }
    for p in &g.points {
        out.push(on_layer(EntityType::Circle(Circle::from_center_radius(Vector3::new(p[0], p[1], 0.0), text_h * 0.25)), layer));
    }
    for (p, s) in &g.labels {
        let at = Vector3::new(p[0], p[1], 0.0);
        let mut t = Text::with_value(s.clone(), at).with_height(text_h);
        t.horizontal_alignment = TextHorizontalAlignment::Center;
        t.vertical_alignment = TextVerticalAlignment::Middle;
        t.alignment_point = Some(at);
        out.push(on_layer(EntityType::Text(t), layer));
    }
    out
}

/// IMAGE entities for downloaded map tiles (lower-left insertion, Y north).
pub fn images(site: &Site, placed: &[PlacedImage], layer: &str, unit_factor: f64, fade: u8) -> Vec<EntityType> {
    placed
        .iter()
        .map(|p| {
            let ll = site.to_drawing([p.bbox[0], p.bbox[1]], unit_factor);
            let ur = site.to_drawing([p.bbox[2], p.bbox[3]], unit_factor);
            let mut img = RasterImage::with_size(
                &p.path.to_string_lossy(),
                Vector3::new(ll[0], ll[1], 0.0),
                p.px[0] as f64,
                p.px[1] as f64,
                ur[0] - ll[0],
                ur[1] - ll[1],
            );
            img.fade = fade.min(90);
            on_layer(EntityType::RasterImage(img), layer)
        })
        .collect()
}

/// DWG layer names: no `<>/\":;?*|,=` characters, upper case like the rest.
pub fn layer_name(prefix: &str, name: &str) -> String {
    let clean: String = name
        .chars()
        .map(|c| if "<>/\\\":;?*|,=`".contains(c) { '-' } else { c })
        .collect();
    let mut s = format!("{prefix}-{clean}").to_uppercase();
    s.truncate(60);
    s
}
