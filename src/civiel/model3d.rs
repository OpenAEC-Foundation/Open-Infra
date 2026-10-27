//! 3D city and landscape model: 3D BAG buildings, the AHN terrain coloured
//! by the aerial photo, and the BGT in 3D (areas draped on the ground,
//! buildings and walls extruded to the AHN surface, water at its level,
//! bridges at deck height, lines draped, posts and trees as symbols).
//! Meshes are polyface meshes: one entity per object, colours per layer
//! (and for the photo terrain one mesh per palette colour).

use std::collections::HashMap;

use ocs_plugin_api::host::codec::entities::{Mesh, MeshFace, Polyline3D};
use ocs_plugin_api::host::codec::{Color, EntityType, Vector3};

use super::{record, v3, Ctx, Out};
use crate::gis::bag3d::{self, Kind, Lod};
use crate::gis::crs::XY;
use crate::gis::vector::Feature;
use crate::hoogte::nap;
use crate::nlcs::{self, Element, LayerSpec};

pub type P3 = [f64; 3];

/// Faces gathered into meshes (MESH entities: plain vertex and face lists,
/// far lighter than polyface meshes, whose every vertex is a record):
/// shared corners, at most 100 000 vertices a mesh.
#[derive(Default)]
pub struct MeshBuilder {
    verts: Vec<P3>,
    index: HashMap<(i64, i64, i64), usize>,
    faces: Vec<[usize; 4]>,
    done: Vec<Mesh>,
}

const NO: usize = usize::MAX;
const MAX_VERTS: usize = 100_000;

impl MeshBuilder {
    fn vertex(&mut self, p: P3) -> usize {
        let key = ((p[0] * 1000.0).round() as i64, (p[1] * 1000.0).round() as i64, (p[2] * 1000.0).round() as i64);
        if let Some(i) = self.index.get(&key) {
            return *i;
        }
        self.verts.push(p);
        self.index.insert(key, self.verts.len() - 1);
        self.verts.len() - 1
    }

    fn room(&mut self, n: usize) {
        if self.verts.len() + n > MAX_VERTS {
            self.flush();
        }
    }

    fn flush(&mut self) {
        if !self.faces.is_empty() {
            let mut m = Mesh::new();
            m.vertices = self.verts.iter().map(|p| Vector3::new(p[0], p[1], p[2])).collect();
            m.faces = self.faces.iter().map(|f| MeshFace::new(if f[3] == NO { vec![f[0], f[1], f[2]] } else { f.to_vec() })).collect();
            self.done.push(m);
        }
        self.verts.clear();
        self.index.clear();
        self.faces.clear();
    }

    pub fn tri(&mut self, a: P3, b: P3, c: P3) {
        self.room(3);
        let (i, j, k) = (self.vertex(a), self.vertex(b), self.vertex(c));
        if i != j && j != k && i != k {
            self.faces.push([i, j, k, NO]);
        }
    }

    pub fn quad(&mut self, a: P3, b: P3, c: P3, d: P3) {
        self.room(4);
        let (i, j, k, l) = (self.vertex(a), self.vertex(b), self.vertex(c), self.vertex(d));
        if i != j && j != k && k != l && l != i && i != k && j != l {
            self.faces.push([i, j, k, l]);
        } else {
            self.faces.push([i, j, k, NO]);
        }
    }

    pub fn is_empty(&self) -> bool {
        self.faces.is_empty() && self.done.is_empty()
    }

    pub fn faces(&self) -> usize {
        self.faces.len() + self.done.iter().map(|m| m.faces.len()).sum::<usize>()
    }

    /// The meshes as entities on `layer`, ByLayer unless a colour is given.
    pub fn finish(mut self, layer: &str, color: Option<Color>, data: Option<&ocs_plugin_api::host::ExtendedDataRecord>) -> Vec<EntityType> {
        self.flush();
        self.done
            .into_iter()
            .map(|m| {
                let mut e = EntityType::Mesh(m);
                let c = e.common_mut();
                c.layer = layer.to_string();
                c.color = color.unwrap_or(Color::ByLayer);
                if let Some(d) = data {
                    c.extended_data.add_record(d.clone());
                }
                e
            })
            .collect()
    }
}

/// RD → drawing for this project.
fn rd_to_drawing(ctx: &Ctx) -> Result<Box<dyn Fn(XY) -> Option<XY>>, String> {
    let site = ctx.site.clone().ok_or("stel eerst de projectlocatie in (GIS → Locatie, of GIS_SITE)")?;
    let to = crate::gis::vector::data_to_site(&site, "EPSG:28992", &[])?;
    let f = ctx.f;
    Ok(Box::new(move |p: XY| to(p).map(|q| site.to_drawing(q, f))))
}

fn project_rd_bbox(ctx: &Ctx) -> Result<[f64; 4], String> {
    let site = ctx.site.as_ref().ok_or("stel eerst de projectlocatie in (GIS → Locatie, of GIS_SITE)")?;
    crate::basisdata::rd_bbox(site)
}

fn spec(ctx: &Ctx, group: &str, name: &str) -> Result<LayerSpec, String> {
    ctx.layer(group, name, Element::G)
}

// ── 3D BAG ──────────────────────────────────────────────────────────────────

/// `CIV_BAG3D`: the buildings of the project area from the 3D BAG, one roof
/// mesh (OG-BEBOUWING) and one wall mesh (OG-BEBOUWING_HOOFDGEBOUW) each.
pub fn bag3d(opt: &super::CivielOptions, lod: Lod, max: usize, realistic: bool, progress: &dyn Fn(String)) -> Result<String, String> {
    let ctx = Ctx::new(opt)?;
    let bbox = project_rd_bbox(&ctx)?;
    let (buildings, cut) = bag3d::fetch(bbox, lod, max, progress)?;
    if buildings.is_empty() {
        return Err("3D BAG: geen panden in het projectgebied (alleen Nederland)".into());
    }
    progress(format!("3D BAG: {} panden omzetten…", buildings.len()));
    let to = rd_to_drawing(&ctx)?;
    let f = ctx.f;
    let roofs = spec(&ctx, "OG", "BEBOUWING")?;
    let walls = spec(&ctx, "OG", "BEBOUWING_HOOFDGEBOUW")?;
    let mut out = Out::default();
    let mut faces = 0;
    let p3 = |p: &[f64; 3]| to([p[0], p[1]]).map(|q| [q[0], q[1], p[2] * f]);
    for b in &buildings {
        let d = record(&[
            ("bron", format!("3D BAG LoD {}", lod.code())),
            ("identificatie", b.id.clone()),
            ("bouwjaar", b.year.map(|y| y.to_string()).unwrap_or_default()),
            ("status", b.status.clone()),
            ("maaiveld_m", b.ground.map(|z| format!("{z:.2}")).unwrap_or_default()),
            ("dak_70p_m", b.roof.map(|z| format!("{z:.2}")).unwrap_or_default()),
            ("hoogte_m", match (b.ground, b.roof) {
                (Some(g), Some(r)) => format!("{:.2}", r - g),
                _ => String::new(),
            }),
        ]);
        let (mut rm, mut wm) = (MeshBuilder::default(), MeshBuilder::default());
        for (k, surf) in &b.surfaces {
            if *k == Kind::Ground {
                continue;
            }
            let target = if *k == Kind::Roof { &mut rm } else { &mut wm };
            for t in bag3d::triangulate(surf) {
                if let (Some(a), Some(bb), Some(c)) = (p3(&t[0]), p3(&t[1]), p3(&t[2])) {
                    target.tri(a, bb, c);
                }
            }
        }
        faces += rm.faces() + wm.faces();
        // Flat roofs grey, pitched ones tiled; walls light brick.
        let (roof_c, wall_c) = if realistic {
            let pitched = b.surfaces.iter().filter(|(k, _)| *k == Kind::Roof).any(|(_, s)| {
                let r = &s[0];
                let zs = r.iter().map(|p| p[2]);
                zs.clone().fold(f64::MIN, f64::max) - zs.fold(f64::MAX, f64::min) > 1.0
            });
            (Some(if pitched { Color::Rgb { r: 142, g: 78, b: 62 } } else { Color::Rgb { r: 104, g: 104, b: 108 } }), Some(Color::Rgb { r: 214, g: 200, b: 184 }))
        } else {
            (None, None)
        };
        if !rm.is_empty() {
            out.put(&roofs, rm.finish(&roofs.name, roof_c, Some(&d)));
        }
        if !wm.is_empty() {
            out.put(&walls, wm.finish(&walls.name, wall_c, Some(&d)));
        }
    }
    progress("3D BAG tekenen…".into());
    let n = out.write(&format!("3D BAG LoD {}", lod.code()))?;
    crate::bridge::note_source("3D BAG (TU Delft 3D geoinformation, CC BY 4.0)");
    super::api::note("panden", buildings.len());
    super::api::note("afgekapt", cut);
    super::api::note("vlakken", faces);
    super::api::note("lod", lod.code());
    Ok(format!(
        "3D BAG LoD {}: {} panden{} met {faces} driehoeken — {n} elementen op {} en {}.",
        lod.code(),
        buildings.len(),
        if cut { format!(" (maximum {max} bereikt; verhoog MAX=)") } else { String::new() },
        roofs.name,
        walls.name
    ))
}

// ── Terrain with the aerial photo ───────────────────────────────────────────

/// Fill cells without a height (water, under buildings): each connected gap
/// becomes flat, half a metre under its lowest bank, so a water surface
/// drawn on top lies clearly above it (GPU depth is coarse at RD
/// coordinates) and a river bed shows no stripes.
pub fn fill_gaps(z: &mut [f32], w: usize, h: usize) {
    let mut seen = vec![false; w * h];
    let mut region = Vec::new();
    let mut stack = Vec::new();
    for start in 0..w * h {
        if seen[start] || z[start].is_finite() {
            continue;
        }
        region.clear();
        stack.push(start);
        seen[start] = true;
        let mut low = f32::INFINITY;
        while let Some(i) = stack.pop() {
            region.push(i);
            let (c, r) = ((i % w) as i64, (i / w) as i64);
            for (dc, dr) in [(-1i64, 0i64), (1, 0), (0, -1), (0, 1), (-1, -1), (1, 1), (-1, 1), (1, -1)] {
                let (cc, rr) = (c + dc, r + dr);
                if cc < 0 || rr < 0 || cc as usize >= w || rr as usize >= h {
                    continue;
                }
                let j = rr as usize * w + cc as usize;
                if z[j].is_finite() {
                    low = low.min(z[j]);
                } else if !seen[j] && dc.abs() + dr.abs() == 1 {
                    seen[j] = true;
                    stack.push(j);
                }
            }
        }
        if low.is_finite() {
            for &i in &region {
                z[i] = low - 0.5;
            }
        }
    }
}

/// A palette colour: 8 levels of red and green, 6 of blue.
fn quantize(rgb: [u8; 3]) -> (u8, u8, u8) {
    let q = |v: u8, n: u32| -> u8 {
        let step = 255.0 / (n - 1) as f64;
        ((v as f64 / step).round() * step).round().min(255.0) as u8
    };
    (q(rgb[0], 8), q(rgb[1], 8), q(rgb[2], 6))
}

/// The PDOK aerial photo (8 cm, actual) of an RD box, w×h pixels.
fn aerial(bbox: [f64; 4], w: u32, h: u32) -> Result<image::RgbaImage, String> {
    let url = crate::gis::http::with_params(
        "https://service.pdok.nl/hwh/luchtfotorgb/wms/v1_0",
        &[
            ("SERVICE", "WMS".into()),
            ("VERSION", "1.3.0".into()),
            ("REQUEST", "GetMap".into()),
            ("LAYERS", "Actueel_orthoHR".into()),
            ("STYLES", String::new()),
            ("CRS", "EPSG:28992".into()),
            ("BBOX", format!("{},{},{},{}", bbox[0], bbox[1], bbox[2], bbox[3])),
            ("WIDTH", w.to_string()),
            ("HEIGHT", h.to_string()),
            ("FORMAT", "image/jpeg".into()),
        ],
    );
    let bytes = crate::gis::http::get_image(&url)?;
    image::load_from_memory(&bytes).map(|i| i.to_rgba8()).map_err(|e| format!("luchtfoto onleesbaar: {e}"))
}

/// `CIV_FOTOTERREIN`: the AHN ground of the project area as a 3D surface in
/// `cell` m cells, each coloured by the aerial photo (OG-DTM).
pub fn photo_terrain(opt: &super::CivielOptions, cell: f64, progress: &dyn Fn(String)) -> Result<String, String> {
    let ctx = Ctx::new(opt)?;
    let b = project_rd_bbox(&ctx)?;
    let cell = cell.max(0.5);
    let cells = ((b[2] - b[0]) / cell) * ((b[3] - b[1]) / cell);
    if cells > 400_000.0 {
        return Err(format!("{cells:.0} cellen is te veel: kies een grotere cel dan {cell} m"));
    }
    let bron = super::hoogtebron(&ctx)?;
    progress(format!("{} ophalen…", bron.label()));
    let mut grid = bron.grid(b, cell, progress)?;
    fill_gaps(&mut grid.z, grid.w, grid.h);
    // The photo at about four pixels a cell, in tiles the service accepts.
    let px = (cell / 4.0).max(0.25);
    let (pw, ph) = ((grid.w as f64 * grid.cell / px).ceil() as u32, (grid.h as f64 * grid.cell / px).ceil() as u32);
    let gb = [grid.x0, grid.y1 - grid.h as f64 * grid.cell, grid.x0 + grid.w as f64 * grid.cell, grid.y1];
    let tile = 2000u32;
    let mut photo = image::RgbaImage::new(pw.max(1), ph.max(1));
    let (tx, ty) = (pw.div_ceil(tile), ph.div_ceil(tile));
    for j in 0..ty {
        for i in 0..tx {
            progress(format!("Luchtfoto {}/{}…", j * tx + i + 1, tx * ty));
            let (x0p, y0p) = (i * tile, j * tile);
            let (wp, hp) = ((pw - x0p).min(tile), (ph - y0p).min(tile));
            let bb = [gb[0] + x0p as f64 * px, gb[3] - (y0p + hp) as f64 * px, gb[0] + (x0p + wp) as f64 * px, gb[3] - y0p as f64 * px];
            let img = aerial(bb, wp, hp)?;
            image::imageops::replace(&mut photo, &img, x0p as i64, y0p as i64);
        }
    }
    progress("Terrein kleuren…".into());
    let to = rd_to_drawing(&ctx)?;
    let f = ctx.f;
    let at = |c: usize, r: usize| -> Option<P3> {
        let p = grid.centre(c, r);
        let z = grid.z[r * grid.w + c];
        z.is_finite().then_some(())?;
        to(p).map(|q| [q[0], q[1], z as f64 * f])
    };
    let mut buckets: HashMap<(u8, u8, u8), MeshBuilder> = HashMap::new();
    for r in 0..grid.h.saturating_sub(1) {
        for c in 0..grid.w.saturating_sub(1) {
            let (Some(a), Some(bq), Some(cq), Some(d)) = (at(c, r + 1), at(c + 1, r + 1), at(c + 1, r), at(c, r)) else { continue };
            // Mean photo colour over the cell between the four centres.
            let (cx, cy) = ((c as f64 + 1.0) * grid.cell, (r as f64 + 1.0) * grid.cell);
            let (u0, v0) = (((cx - grid.cell / 2.0) / px) as u32, ((cy - grid.cell / 2.0) / px) as u32);
            let n = (grid.cell / px).round().max(1.0) as u32;
            let (mut s, mut k) = ([0u32; 3], 0u32);
            for v in v0..(v0 + n).min(photo.height()) {
                for u in u0..(u0 + n).min(photo.width()) {
                    let p = photo.get_pixel(u, v);
                    s[0] += p[0] as u32;
                    s[1] += p[1] as u32;
                    s[2] += p[2] as u32;
                    k += 1;
                }
            }
            let rgb = if k > 0 { [(s[0] / k) as u8, (s[1] / k) as u8, (s[2] / k) as u8] } else { [128, 128, 128] };
            buckets.entry(quantize(rgb)).or_default().quad(a, bq, cq, d);
        }
    }
    let layer = spec(&ctx, "OG", "DTM")?;
    let d = record(&[("bron", format!("{} + luchtfoto PDOK", bron.label())), ("cel_m", format!("{cell}"))]);
    let mut out = Out::default();
    let mut quads = 0;
    let colours = buckets.len();
    for ((r, g, bl), m) in buckets {
        quads += m.faces();
        out.put(&layer, m.finish(&layer.name, Some(Color::Rgb { r, g, b: bl }), Some(&d)));
    }
    let n = out.write("Terrein met luchtfoto")?;
    crate::bridge::note_source(bron.attribution());
    crate::bridge::note_source("Luchtfoto (Beeldmateriaal.nl / PDOK, CC BY 4.0)");
    let (lo, hi) = grid.range().unwrap_or((0.0, 0.0));
    super::api::note("cellen", quads);
    super::api::note("kleuren", colours);
    super::api::note("hoogte_min", lo);
    super::api::note("hoogte_max", hi);
    Ok(format!("Terrein met luchtfoto: {quads} cellen van {cell} m in {colours} kleuren, {} tot {} m — {n} elementen op {}.", nap(lo, 2), nap(hi, 2), layer.name))
}

// ── BGT in 3D ───────────────────────────────────────────────────────────────

fn ring_area(r: &[XY]) -> f64 {
    let mut a = 0.0;
    for i in 0..r.len() {
        let (p, q) = (r[i], r[(i + 1) % r.len()]);
        a += p[0] * q[1] - q[0] * p[1];
    }
    a / 2.0
}

fn in_ring(p: XY, r: &[XY]) -> bool {
    let mut inside = false;
    let mut j = r.len() - 1;
    for i in 0..r.len() {
        let (a, b) = (r[i], r[j]);
        if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0] {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// A ring clipped to a rectangle [x0, y0, x1, y1] (Sutherland–Hodgman);
/// empty when nothing is left.
pub fn clip_ring(r: &[XY], b: [f64; 4]) -> Vec<XY> {
    let mut pts: Vec<XY> = r.to_vec();
    if pts.len() > 1 && pts.first() == pts.last() {
        pts.pop();
    }
    // Each edge: inside test and the crossing with the boundary line.
    for side in 0..4 {
        if pts.is_empty() {
            break;
        }
        let inside = |p: XY| match side {
            0 => p[0] >= b[0],
            1 => p[0] <= b[2],
            2 => p[1] >= b[1],
            _ => p[1] <= b[3],
        };
        let cross = |a: XY, c: XY| -> XY {
            let (k, v) = match side {
                0 => (0, b[0]),
                1 => (0, b[2]),
                2 => (1, b[1]),
                _ => (1, b[3]),
            };
            let t = (v - a[k]) / (c[k] - a[k]);
            [a[0] + (c[0] - a[0]) * t, a[1] + (c[1] - a[1]) * t]
        };
        let mut out = Vec::with_capacity(pts.len() + 4);
        for i in 0..pts.len() {
            let (cur, prev) = (pts[i], pts[(i + pts.len() - 1) % pts.len()]);
            match (inside(prev), inside(cur)) {
                (true, true) => out.push(cur),
                (true, false) => out.push(cross(prev, cur)),
                (false, true) => {
                    out.push(cross(prev, cur));
                    out.push(cur);
                }
                (false, false) => {}
            }
        }
        pts = out;
    }
    if pts.len() >= 3 && ring_area(&pts).abs() > 1e-9 { pts } else { Vec::new() }
}

/// Closed rings → polygons: each ring inside another is its hole.
pub fn polygons(rings: &[Vec<XY>]) -> Vec<(Vec<XY>, Vec<Vec<XY>>)> {
    let mut rs: Vec<Vec<XY>> = rings
        .iter()
        .map(|r| {
            let mut r = r.clone();
            if r.len() > 1 && r.first() == r.last() {
                r.pop();
            }
            r
        })
        .filter(|r| r.len() >= 3)
        .collect();
    rs.sort_by(|a, b| ring_area(b).abs().total_cmp(&ring_area(a).abs()));
    let mut out: Vec<(Vec<XY>, Vec<Vec<XY>>)> = Vec::new();
    for r in rs {
        match out.iter_mut().find(|(o, _)| in_ring(r[0], o)) {
            Some((_, holes)) => holes.push(r),
            None => out.push((r, Vec::new())),
        }
    }
    out
}

fn inside_polygon(p: XY, outer: &[XY], holes: &[Vec<XY>]) -> bool {
    in_ring(p, outer) && !holes.iter().any(|h| in_ring(p, h))
}

/// A ring with points at most `step` apart.
fn densify(r: &[XY], step: f64, closed: bool) -> Vec<XY> {
    let mut out = Vec::new();
    let n = if closed { r.len() } else { r.len() - 1 };
    for i in 0..n {
        let (a, b) = (r[i], r[(i + 1) % r.len()]);
        let len = (b[0] - a[0]).hypot(b[1] - a[1]);
        let k = (len / step).ceil().max(1.0) as usize;
        for j in 0..k {
            let t = j as f64 / k as f64;
            out.push([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]);
        }
    }
    if !closed {
        out.push(r[r.len() - 1]);
    }
    out
}

/// Flat triangles of a polygon with holes (plan).
fn flat_triangles(outer: &[XY], holes: &[Vec<XY>]) -> Vec<[XY; 3]> {
    let s: bag3d::Surface = std::iter::once(outer).chain(holes.iter().map(Vec::as_slice)).map(|r| r.iter().map(|p| [p[0], p[1], 0.0]).collect()).collect();
    bag3d::triangulate(&s).into_iter().map(|t| [[t[0][0], t[0][1]], [t[1][0], t[1][1]], [t[2][0], t[2][1]]]).collect()
}

/// Triangles of a polygon that follow the ground: its edges every `step`
/// m and a grid of `step` m inside, triangulated, kept where inside.
fn draped_triangles(outer: &[XY], holes: &[Vec<XY>], step: f64) -> Vec<[XY; 3]> {
    let mut pts: Vec<XY> = densify(outer, step, true);
    for h in holes {
        pts.extend(densify(h, step, true));
    }
    let (mut lo, mut hi) = ([f64::MAX; 2], [f64::MIN; 2]);
    for p in outer {
        lo = [lo[0].min(p[0]), lo[1].min(p[1])];
        hi = [hi[0].max(p[0]), hi[1].max(p[1])];
    }
    let mut y = (lo[1] / step).ceil() * step;
    while y < hi[1] {
        let mut x = (lo[0] / step).ceil() * step;
        while x < hi[0] {
            if inside_polygon([x, y], outer, holes) {
                pts.push([x, y]);
            }
            x += step;
        }
        y += step;
    }
    if pts.len() < 3 {
        return flat_triangles(outer, holes);
    }
    let c = [lo[0], lo[1]];
    let d: Vec<delaunator::Point> = pts.iter().map(|p| delaunator::Point { x: p[0] - c[0], y: p[1] - c[1] }).collect();
    let t = delaunator::triangulate(&d);
    let mut out = Vec::new();
    for tri in t.triangles.chunks_exact(3) {
        let (a, b, cc) = (pts[tri[0]], pts[tri[1]], pts[tri[2]]);
        let m = [(a[0] + b[0] + cc[0]) / 3.0, (a[1] + b[1] + cc[1]) / 3.0];
        if inside_polygon(m, outer, holes) {
            out.push([a, b, cc]);
        }
    }
    if out.is_empty() {
        flat_triangles(outer, holes)
    } else {
        out
    }
}

/// Natural colours for a realistic model (instead of the NLCS layer
/// colours): by BGT class and its physical appearance.
pub fn natural_colour(collection: &str, props: &serde_json::Map<String, serde_json::Value>) -> (u8, u8, u8) {
    let get = |k: &str| props.get(k).and_then(|v| v.as_str()).unwrap_or("").to_ascii_lowercase();
    let look = format!("{} {} {}", get("fysiek_voorkomen"), get("plus_fysiek_voorkomen"), get("functie"));
    let has = |w: &str| look.contains(w);
    match collection {
        "wegdeel" | "onbegroeidterreindeel" | "ondersteunendwegdeel" if has("groen") || has("gras") => (104, 142, 74),
        "wegdeel" | "onbegroeidterreindeel" | "ondersteunendwegdeel" => {
            if has("gesloten") || has("asfalt") || has("beton") {
                (78, 78, 82)
            } else if has("klinker") || has("tegel") || has("open verharding") {
                (150, 122, 104)
            } else if has("zand") {
                (214, 196, 150)
            } else if has("half") || has("grind") || has("erf") || has("onverhard") {
                (170, 158, 136)
            } else {
                (120, 118, 116)
            }
        }
        "begroeidterreindeel" => {
            if has("bos") {
                (62, 98, 52)
            } else if has("struik") || has("heide") || has("riet") {
                (96, 118, 66)
            } else {
                (106, 148, 76)
            }
        }
        "waterdeel" | "ondersteunendwaterdeel" => (72, 108, 132),
        "overbruggingsdeel" => (150, 148, 144),
        "pand" | "overigbouwwerk" | "gebouwinstallatie" => (206, 196, 182),
        "scheiding_vlak" | "scheiding_lijn" | "overigescheiding" | "kunstwerkdeel_vlak" | "kunstwerkdeel_lijn" => (140, 120, 104),
        "spoor" => (96, 86, 76),
        "vegetatieobject_punt" | "vegetatieobject_vlak" | "vegetatieobject_lijn" => (66, 112, 54),
        _ => (128, 128, 128),
    }
}

/// How a BGT object becomes 3D.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape {
    /// On the ground.
    Draped,
    /// Flat at the lowest ground along its edge (water).
    Level,
    /// Flat at the AHN surface (bridge decks).
    Deck,
    /// From the ground up to the AHN surface (buildings, walls).
    Extruded,
}

pub fn shape_of(collection: &str) -> Shape {
    match collection {
        "waterdeel" | "ondersteunendwaterdeel" => Shape::Level,
        "overbruggingsdeel" => Shape::Deck,
        "pand" | "overigbouwwerk" | "gebouwinstallatie" | "scheiding_vlak" | "kunstwerkdeel_vlak" | "overigescheiding" | "tunneldeel" => Shape::Extruded,
        _ => Shape::Draped,
    }
}

/// Height of posts and trees above the ground (m).
fn symbol_height(collection: &str) -> f64 {
    match collection {
        "vegetatieobject_punt" => 8.0,
        "mast" => 10.0,
        "paal" | "bord" | "weginrichtingselement_punt" => 3.0,
        _ => 1.5,
    }
}

/// The 3D BGT groups: everything but labels and slope lines.
pub fn default_groups() -> Vec<&'static str> {
    crate::gis::bgt::GROUPS.iter().filter(|g| g.id != "labels" && g.id != "kruinlijnen").map(|g| g.id).collect()
}

/// `CIV_BGT3D`: BGT objects of the project area in 3D, on their NLCS layers.
pub fn bgt3d(opt: &super::CivielOptions, groups: &[String], with_buildings: bool, realistic: bool, progress: &dyn Fn(String)) -> Result<String, String> {
    let ctx = Ctx::new(opt)?;
    let site = ctx.site.clone().ok_or("stel eerst de projectlocatie in")?;
    let collections: Vec<&str> = crate::gis::bgt::GROUPS
        .iter()
        .filter(|g| groups.iter().any(|x| x.eq_ignore_ascii_case(g.id)))
        .flat_map(|g| g.collections.iter().copied())
        .filter(|c| !crate::gis::bgt::is_label(c))
        .filter(|c| with_buildings || !matches!(*c, "pand" | "overigbouwwerk" | "gebouwinstallatie"))
        .collect();
    if collections.is_empty() {
        return Err(format!("geen BGT-groepen gekozen; kies uit {}", default_groups().join(", ")));
    }
    let data = crate::gis::bgt::fetch(&site, &collections, progress)?;
    let total: usize = data.iter().map(|(_, v)| v.len()).sum();
    progress(format!("BGT in 3D: {total} objecten…"));
    let mut bron = super::hoogtebron(&ctx)?;
    let to = rd_to_drawing(&ctx)?;
    let f = ctx.f;
    let k = ctx.k;
    let area = site.drawing_rect(f);
    let mut out = Out::default();
    let (mut meshes, mut lines, mut symbols, mut faces, mut skipped) = (0usize, 0usize, 0usize, 0usize, 0usize);
    for (ci, (collection, features)) in data.iter().enumerate() {
        progress(format!("BGT in 3D: {collection} ({}/{})…", ci + 1, data.len()));
        let shape = shape_of(collection);
        for feat in features {
            let Some(obj) = nlcs::bgt_lookup(collection, &feat.props).map(|(id, _)| id).or_else(|| nlcs::bgt_default(collection)).and_then(nlcs::object) else {
                skipped += 1;
                continue;
            };
            let layer = nlcs::layer(obj, opt.nlcs.status, &opt.nlcs.discipline, Element::G);
            let id = feat.props.get("lokaal_id").or_else(|| feat.props.get("identificatie_lokaal_id")).and_then(|v| v.as_str()).unwrap_or("").to_string();
            let d = record(&[("bron", "BGT 3D".into()), ("objecttype", collection.clone()), ("lokaal_id", id), ("nlcs", obj.id.to_string())]);
            let colour = realistic.then(|| natural_colour(collection, &feat.props)).map(|(r, g, b)| Color::Rgb { r, g, b });
            let made = object_3d(&mut bron, &*to, f, k, feat, collection, shape, &layer, &d, area, colour, &mut out, &mut faces);
            match made {
                Made::Mesh => meshes += 1,
                Made::Lines(n) => lines += n,
                Made::Symbols(n) => symbols += n,
                Made::Nothing => skipped += 1,
            }
        }
    }
    progress("BGT in 3D tekenen…".into());
    let n = out.write("BGT in 3D")?;
    crate::bridge::note_source("BGT (PDOK, CC0)");
    crate::bridge::note_source(bron.attribution());
    super::api::note("objecten", total);
    super::api::note("meshes", meshes);
    super::api::note("lijnen", lines);
    super::api::note("symbolen", symbols);
    super::api::note("driehoeken", faces);
    Ok(format!(
        "BGT in 3D: {total} objecten → {meshes} vlakken ({faces} driehoeken), {lines} lijnen, {symbols} symbolen{} — {n} elementen.",
        if skipped > 0 { format!(", {skipped} overgeslagen") } else { String::new() }
    ))
}

enum Made {
    Mesh,
    Lines(usize),
    Symbols(usize),
    Nothing,
}

#[allow(clippy::too_many_arguments)]
fn object_3d(
    bron: &mut super::Hoogtebron,
    to: &dyn Fn(XY) -> Option<XY>,
    f: f64,
    k: f64,
    feat: &Feature,
    collection: &str,
    shape: Shape,
    layer: &LayerSpec,
    d: &ocs_plugin_api::host::ExtendedDataRecord,
    area: [f64; 4],
    colour: Option<Color>,
    out: &mut Out,
    faces: &mut usize,
) -> Made {
    // In the project area only, like the 2D import.
    let rings: Vec<Vec<XY>> = feat
        .lines
        .iter()
        .filter(|(_, closed)| *closed)
        .map(|(r, _)| clip_ring(&r.iter().filter_map(|p| to(*p)).collect::<Vec<_>>(), area))
        .filter(|r| !r.is_empty())
        .collect();
    let open: Vec<Vec<XY>> = feat
        .lines
        .iter()
        .filter(|(_, closed)| !*closed)
        .flat_map(|(r, _)| crate::gis::vector::clip_line(&r.iter().filter_map(|p| to(*p)).collect::<Vec<_>>(), area))
        .collect();
    let step = 3.0 * f;
    if !rings.is_empty() {
        let mut m = MeshBuilder::default();
        for (outer, holes) in polygons(&rings) {
            let edge: Vec<f64> = outer.iter().filter_map(|p| bron.height(*p)).collect();
            let ground_min = edge.iter().cloned().fold(f64::MAX, f64::min);
            let ground_mid = if edge.is_empty() { 0.0 } else { edge.iter().sum::<f64>() / edge.len() as f64 };
            match shape {
                Shape::Draped => {
                    let area = ring_area(&outer).abs() / (f * f);
                    let s = if area > 50_000.0 { 6.0 * f } else if area > 5_000.0 { 4.0 * f } else { step };
                    for t in draped_triangles(&outer, &holes, s) {
                        let mut z = |p: XY| bron.height(p).unwrap_or(ground_mid) * f;
                        let (za, zb, zc) = (z(t[0]), z(t[1]), z(t[2]));
                        m.tri([t[0][0], t[0][1], za], [t[1][0], t[1][1], zb], [t[2][0], t[2][1], zc]);
                    }
                }
                Shape::Level | Shape::Deck => {
                    let z = if shape == Shape::Level {
                        // Just above the (gap-filled) ground, so water shows.
                        if edge.is_empty() { 0.0 } else { ground_min + 0.05 }
                    } else {
                        // The deck: the middle AHN surface inside.
                        let mut s: Vec<f64> = draped_triangles(&outer, &holes, step * 2.0).iter().filter_map(|t| bron.surface([(t[0][0] + t[1][0] + t[2][0]) / 3.0, (t[0][1] + t[1][1] + t[2][1]) / 3.0])).collect();
                        s.sort_by(f64::total_cmp);
                        s.get(s.len() / 2).copied().unwrap_or(ground_mid)
                    } * f;
                    for t in flat_triangles(&outer, &holes) {
                        m.tri([t[0][0], t[0][1], z], [t[1][0], t[1][1], z], [t[2][0], t[2][1], z]);
                    }
                }
                Shape::Extruded => {
                    let base = if edge.is_empty() { 0.0 } else { ground_min };
                    // The top: the 70th percentile of the AHN surface inside.
                    let mut s: Vec<f64> = draped_triangles(&outer, &holes, step.min(2.0 * f)).iter().filter_map(|t| bron.surface([(t[0][0] + t[1][0] + t[2][0]) / 3.0, (t[0][1] + t[1][1] + t[2][1]) / 3.0])).collect();
                    s.sort_by(f64::total_cmp);
                    let default_h = if collection == "pand" { 6.0 } else { 1.0 };
                    let top = s.get(s.len() * 7 / 10).copied().filter(|t| *t > base + 0.2).unwrap_or(base + default_h);
                    let (zb, zt) = (base * f, top * f);
                    for t in flat_triangles(&outer, &holes) {
                        m.tri([t[0][0], t[0][1], zt], [t[1][0], t[1][1], zt], [t[2][0], t[2][1], zt]);
                    }
                    for r in std::iter::once(&outer).chain(holes.iter()) {
                        for i in 0..r.len() {
                            let (a, b) = (r[i], r[(i + 1) % r.len()]);
                            m.quad([a[0], a[1], zb], [b[0], b[1], zb], [b[0], b[1], zt], [a[0], a[1], zt]);
                        }
                    }
                }
            }
        }
        if m.is_empty() {
            return Made::Nothing;
        }
        *faces += m.faces();
        out.put(layer, m.finish(&layer.name, colour, Some(d)));
        return Made::Mesh;
    }
    if !open.is_empty() {
        let mut n = 0;
        for l in open.iter().filter(|l| l.len() >= 2) {
            let pts: Vec<Vector3> = densify(l, step, false).into_iter().map(|p| v3(p, bron.height(p).unwrap_or(0.0) * f)).collect();
            let mut e = EntityType::Polyline3D(Polyline3D::from_points(pts));
            let c = e.common_mut();
            c.layer = layer.name.clone();
            c.color = colour.unwrap_or(Color::ByLayer);
            c.linetype_scale = k;
            c.extended_data.add_record(d.clone());
            out.put(layer, [e]);
            n += 1;
        }
        return if n > 0 { Made::Lines(n) } else { Made::Nothing };
    }
    let pts: Vec<XY> = feat.points.iter().filter_map(|p| to(*p)).filter(|p| crate::gis::vector::inside(*p, area)).collect();
    if pts.is_empty() {
        return Made::Nothing;
    }
    let h = symbol_height(collection) * f;
    for p in &pts {
        let z = bron.height(*p).unwrap_or(0.0) * f;
        if collection == "vegetatieobject_punt" {
            // A tree: trunk and an eight-sided crown.
            let mut m = MeshBuilder::default();
            let (r, zc0, zc1, zc2) = (2.5 * f, z + 0.35 * h, z + 0.65 * h, z + h);
            let ring: Vec<P3> = (0..8).map(|i| {
                let a = i as f64 * std::f64::consts::TAU / 8.0;
                [p[0] + r * a.cos(), p[1] + r * a.sin(), zc1]
            }).collect();
            for i in 0..8 {
                let (a, b) = (ring[i], ring[(i + 1) % 8]);
                m.tri([p[0], p[1], zc0], b, a);
                m.tri(a, b, [p[0], p[1], zc2]);
            }
            out.put(layer, m.finish(&layer.name, colour, Some(d)));
        }
        let mut e = EntityType::Polyline3D(Polyline3D::from_points(vec![v3(*p, z), v3(*p, z + if collection == "vegetatieobject_punt" { 0.35 * h } else { h })]));
        let c = e.common_mut();
        c.layer = layer.name.clone();
        c.color = if colour.is_some() { Color::Rgb { r: 96, g: 74, b: 56 } } else { Color::ByLayer };
        c.extended_data.add_record(d.clone());
        out.put(layer, [e]);
    }
    Made::Symbols(pts.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polygons_holes_and_draping() {
        let outer = vec![[0.0, 0.0], [20.0, 0.0], [20.0, 20.0], [0.0, 20.0]];
        let hole = vec![[5.0, 5.0], [5.0, 10.0], [10.0, 10.0], [10.0, 5.0]];
        let other = vec![[30.0, 0.0], [40.0, 0.0], [40.0, 10.0]];
        let p = polygons(&[hole.clone(), outer.clone(), other.clone()]);
        assert_eq!(p.len(), 2);
        assert_eq!(p[0].1.len(), 1, "the small square is the big one's hole");
        // Draped triangles cover the polygon minus its hole.
        let tris = draped_triangles(&p[0].0, &p[0].1, 3.0);
        let area: f64 = tris.iter().map(|t| ((t[1][0] - t[0][0]) * (t[2][1] - t[0][1]) - (t[2][0] - t[0][0]) * (t[1][1] - t[0][1])).abs() / 2.0).sum();
        assert!((area - 375.0).abs() < 1.0, "{area}");
        assert!(tris.len() > 20, "inner points make it follow the ground");
        assert_eq!(shape_of("waterdeel"), Shape::Level);
        assert_eq!(shape_of("pand"), Shape::Extruded);
    }

    #[test]
    fn rings_are_clipped_to_the_area() {
        let r = vec![[-10.0, -10.0], [10.0, -10.0], [10.0, 10.0], [-10.0, 10.0]];
        let c = clip_ring(&r, [0.0, 0.0, 20.0, 20.0]);
        assert!((ring_area(&c).abs() - 100.0).abs() < 1e-9, "{c:?}");
        assert!(clip_ring(&r, [50.0, 50.0, 60.0, 60.0]).is_empty());
        assert_eq!(clip_ring(&r, [-100.0, -100.0, 100.0, 100.0]).len(), 4);
    }

    #[test]
    fn meshes_share_corners_and_split() {
        let mut m = MeshBuilder::default();
        m.quad([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]);
        m.tri([1.0, 0.0, 0.0], [2.0, 0.0, 0.0], [1.0, 1.0, 0.0]);
        let e = m.finish("L", None, None);
        assert_eq!(e.len(), 1);
        let EntityType::Mesh(pm) = &e[0] else { panic!() };
        assert_eq!((pm.vertices.len(), pm.faces.len()), (5, 2));
        assert_eq!(pm.faces[1].vertices[0], 1, "shared corner");
        let mut big = MeshBuilder::default();
        for i in 0..40_000 {
            let x = i as f64;
            big.tri([x, 0.0, 0.0], [x, 1.0, 0.0], [x + 0.5, 2.0, 0.0]);
        }
        let parts = big.finish("L", None, None);
        assert!(parts.len() >= 2);
        assert!(parts.iter().all(|e| matches!(e, EntityType::Mesh(m) if m.vertices.len() <= MAX_VERTS)));
        let mut z = vec![1.0, f32::NAN, 3.0, f32::NAN];
        fill_gaps(&mut z, 2, 2);
        assert!(z.iter().all(|v| v.is_finite()));
        assert_eq!(z, vec![1.0, 0.5, 3.0, 0.5], "the lowest neighbour, half a metre down");
        assert_eq!(quantize([255, 0, 128]), (255, 0, 153));
    }
}
