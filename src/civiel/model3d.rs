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
    index: HashMap<(i64, i64, i64, i64), usize>,
    /// Vertices are shared within a group only: one group per plane gives
    /// crisp, evenly lit planes; one group for a whole surface, smooth ones.
    group: i64,
    faces: Vec<[usize; 4]>,
    done: Vec<Mesh>,
}

const NO: usize = usize::MAX;
const MAX_VERTS: usize = 100_000;

impl MeshBuilder {
    fn vertex(&mut self, p: P3) -> usize {
        let key = ((p[0] * 1000.0).round() as i64, (p[1] * 1000.0).round() as i64, (p[2] * 1000.0).round() as i64, self.group);
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

    /// Start a new group of shared vertices (a new plane).
    pub fn new_group(&mut self) {
        self.group += 1;
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

/// How 3D BAG buildings look.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BagStyle {
    /// ByLayer: the NLCS colours.
    Nlcs,
    /// One roof colour (pitched or flat) and one wall colour.
    Realistisch,
    /// Roofs in their colour on the aerial photo, walls by building year,
    /// monuments in stone, windows per storey, crisp planes, a plinth into
    /// the ground.
    Mooi,
    /// A white architectural model.
    Maquette,
}

impl BagStyle {
    pub fn parse(s: &str) -> Option<BagStyle> {
        Some(match s.to_ascii_lowercase().as_str() {
            "nlcs" | "laag" => BagStyle::Nlcs,
            "realistisch" | "standaard" | "foto" => BagStyle::Realistisch,
            "mooi" | "detail" | "fotorealistisch" => BagStyle::Mooi,
            "maquette" | "wit" | "massa" => BagStyle::Maquette,
            _ => return None,
        })
    }
}

/// Wall colour for a building year, varied a little per building.
fn wall_colour(year: Option<i64>, id: &str) -> (u8, u8, u8) {
    let base: (i32, i32, i32) = match year {
        Some(y) if y < 1850 => (176, 104, 82),
        Some(y) if y < 1920 => (152, 84, 66),
        Some(y) if y < 1945 => (136, 80, 60),
        Some(y) if y < 1970 => (198, 178, 142),
        Some(y) if y < 1990 => (170, 144, 122),
        Some(y) if y < 2010 => (214, 206, 196),
        Some(_) => (230, 228, 222),
        None => (206, 192, 176),
    };
    let h = id.bytes().fold(2166136261u32, |h, b| (h ^ b as u32).wrapping_mul(16777619));
    let j = |k: u32| ((h >> k) & 15) as i32 - 8;
    let c = |v: i32| v.clamp(0, 255) as u8;
    (c(base.0 + j(0)), c(base.1 + j(4)), c(base.2 + j(8)))
}

fn tri_area(a: &[f64; 3], b: &[f64; 3], c: &[f64; 3]) -> f64 {
    let (u, v) = ([b[0] - a[0], b[1] - a[1], b[2] - a[2]], [c[0] - a[0], c[1] - a[1], c[2] - a[2]]);
    let n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
    0.5 * (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt()
}

fn inside_xy(p: [f64; 2], ring: &[[f64; 3]]) -> bool {
    let mut inside = false;
    let n = ring.len();
    let mut j = n.wrapping_sub(1);
    for i in 0..n {
        let (a, b) = (ring[i], ring[j]);
        if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0] {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Average colour of the roofs on the aerial photo, a little lighter (the
/// photo has shadows) and mixed with a plain roof colour.
fn roof_colour(photo: &image::RgbaImage, gb: [f64; 4], px: f64, roofs: &[&Vec<[f64; 3]>], pitched: bool) -> Option<(u8, u8, u8)> {
    let (mut s, mut n) = ([0f64; 3], 0f64);
    for ring in roofs {
        let (x0, y0, x1, y1) = ring.iter().fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |b, p| (b.0.min(p[0]), b.1.min(p[1]), b.2.max(p[0]), b.3.max(p[1])));
        let step = (((x1 - x0) * (y1 - y0)).sqrt() / 10.0).max(px);
        let mut y = y0 + step / 2.0;
        while y < y1 {
            let mut x = x0 + step / 2.0;
            while x < x1 {
                if inside_xy([x, y], ring) {
                    let (u, v) = (((x - gb[0]) / px) as i64, ((gb[3] - y) / px) as i64);
                    if u >= 0 && v >= 0 && (u as u32) < photo.width() && (v as u32) < photo.height() {
                        let p = photo.get_pixel(u as u32, v as u32);
                        s = [s[0] + p[0] as f64, s[1] + p[1] as f64, s[2] + p[2] as f64];
                        n += 1.0;
                    }
                }
                x += step;
            }
            y += step;
        }
    }
    if n < 1.0 {
        return None;
    }
    // The average is greyed by shadows and mixed pixels: give back some
    // light and saturation, then lean a little on a plain roof colour.
    let avg = [s[0] / n, s[1] / n, s[2] / n];
    let grey = (avg[0] + avg[1] + avg[2]) / 3.0;
    let plain = if pitched { [156.0, 78.0, 58.0] } else { [104.0, 106.0, 112.0] };
    let mix = |k: usize| {
        let sat = grey + (avg[k] - grey) * 1.7;
        (sat * 1.15 * 0.75 + plain[k] * 0.25).clamp(0.0, 255.0) as u8
    };
    Some((mix(0), mix(1), mix(2)))
}

/// Windows on a wall: rows per storey of 3 m, 1.2 × 1.5 m, every 2.4 m,
/// just in front of the wall (`out` points outward).
fn windows(ring: &[[f64; 3]], out: [f64; 2], ground: f64, eaves: f64, mb: &mut MeshBuilder, p3: &dyn Fn(&[f64; 3]) -> Option<P3>) -> usize {
    let (n, mut made) = (ring.len(), 0);
    if n < 3 {
        return 0;
    }
    // The wall's own axes: u along it (horizontal), z up.
    let u = [-out[1], out[0]];
    let s_of = |p: &[f64; 3]| p[0] * u[0] + p[1] * u[1];
    let (s0, s1) = ring.iter().fold((f64::MAX, f64::MIN), |b, p| (b.0.min(s_of(p)), b.1.max(s_of(p))));
    let (z0, z1) = ring.iter().fold((f64::MAX, f64::MIN), |b, p| (b.0.min(p[2]), b.1.max(p[2])));
    if s1 - s0 < 2.2 || z1 - z0 < 2.0 {
        return 0;
    }
    let flat: Vec<[f64; 3]> = ring.iter().map(|p| [s_of(p), p[2], 0.0]).collect();
    let base = ring[0];
    let d0 = base[0] * out[0] + base[1] * out[1];
    let at = |s: f64, z: f64| -> [f64; 3] {
        // The point on the wall plane at (s, z), 6 cm in front of it.
        let d = d0 + 0.06;
        [u[0] * s + out[0] * d, u[1] * s + out[1] * d, z]
    };
    let top = eaves.min(z1) - 0.4;
    let count = (((s1 - s0) - 1.2) / 2.4).floor().max(0.0) as usize + 1;
    let pad = ((s1 - s0) - ((count - 1) as f64 * 2.4 + 1.2)) / 2.0;
    let mut z = ground.max(z0) + 0.9;
    while z + 1.5 <= top {
        for k in 0..count {
            let a = s0 + pad + k as f64 * 2.4;
            let corners = [(a, z), (a + 1.2, z), (a + 1.2, z + 1.5), (a, z + 1.5)];
            if corners.iter().all(|(cs, cz)| inside_xy([*cs, *cz], &flat)) {
                let q: Vec<Option<P3>> = corners.iter().map(|(cs, cz)| p3(&at(*cs, *cz))).collect();
                if let (Some(a), Some(b), Some(c), Some(d)) = (q[0], q[1], q[2], q[3]) {
                    mb.new_group();
                    mb.quad(a, b, c, d);
                    made += 1;
                }
            }
        }
        z += 3.0;
    }
    made
}

/// `CIV_BAG3D`: the buildings of the project area from the 3D BAG, one roof
/// mesh (OG-BEBOUWING) and one wall mesh (OG-BEBOUWING_HOOFDGEBOUW) each;
/// with `BagStyle::Mooi` also a window mesh.
pub fn bag3d(opt: &super::CivielOptions, lod: Lod, max: usize, style: BagStyle, progress: &dyn Fn(String)) -> Result<String, String> {
    let ctx = Ctx::new(opt)?;
    let bbox = project_rd_bbox(&ctx)?;
    let (buildings, cut) = bag3d::fetch(bbox, lod, max, progress)?;
    if buildings.is_empty() {
        return Err("3D BAG: geen panden in het projectgebied (alleen Nederland)".into());
    }
    // The aerial photo for the roof colours.
    let px = 0.5;
    let photo = if style == BagStyle::Mooi { Some(aerial_mosaic(bbox, px, progress)?) } else { None };
    progress(format!("3D BAG: {} panden omzetten…", buildings.len()));
    let to = rd_to_drawing(&ctx)?;
    let f = ctx.f;
    let roofs = spec(&ctx, "OG", "BEBOUWING")?;
    let walls = spec(&ctx, "OG", "BEBOUWING_HOOFDGEBOUW")?;
    let mut out = Out::default();
    let (mut faces, mut panes) = (0, 0);
    let p3 = |p: &[f64; 3]| to([p[0], p[1]]).map(|q| [q[0], q[1], p[2] * f]);
    let crisp = matches!(style, BagStyle::Mooi | BagStyle::Maquette);
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
        let (mut rm, mut wm, mut win) = (MeshBuilder::default(), MeshBuilder::default(), MeshBuilder::default());
        let ground = b.ground.unwrap_or_else(|| b.surfaces.iter().flat_map(|(_, s)| s.iter().flatten()).map(|p| p[2]).fold(f64::MAX, f64::min));
        let roof_rings: Vec<&Vec<[f64; 3]>> = b.surfaces.iter().filter(|(k, _)| *k == Kind::Roof).filter_map(|(_, s)| s.first()).collect();
        let eaves = roof_rings.iter().flat_map(|r| r.iter()).map(|p| p[2]).fold(f64::MAX, f64::min);
        let pitched = roof_rings.iter().any(|r| {
            let zs = r.iter().map(|p| p[2]);
            zs.clone().fold(f64::MIN, f64::max) - zs.fold(f64::MAX, f64::min) > 1.0
        });
        let tall_old = b.year.is_some_and(|y| y < 1700) && b.roof.zip(b.ground).is_some_and(|(r, g)| r - g > 20.0);
        let centre = {
            let pts: Vec<&[f64; 3]> = b.surfaces.iter().flat_map(|(_, s)| s.iter().flatten()).collect();
            let n = pts.len().max(1) as f64;
            [pts.iter().map(|p| p[0]).sum::<f64>() / n, pts.iter().map(|p| p[1]).sum::<f64>() / n]
        };
        for (k, surf) in &b.surfaces {
            if *k == Kind::Ground {
                // A plinth: the outline 1 m down, so no gap shows where the
                // terrain lies lower than the 3D BAG ground level.
                if crisp {
                    if let Some(r) = surf.first() {
                        for i in 0..r.len() {
                            let (a, c) = (r[i], r[(i + 1) % r.len()]);
                            let (a2, c2) = ([a[0], a[1], a[2] - 1.0], [c[0], c[1], c[2] - 1.0]);
                            if let (Some(p), Some(q), Some(rr), Some(t)) = (p3(&a), p3(&c), p3(&c2), p3(&a2)) {
                                wm.new_group();
                                wm.quad(t, rr, q, p);
                            }
                        }
                    }
                }
                continue;
            }
            let target = if *k == Kind::Roof { &mut rm } else { &mut wm };
            if crisp {
                target.new_group();
            }
            for t in bag3d::triangulate(surf) {
                if crisp && tri_area(&t[0], &t[1], &t[2]) < 0.02 {
                    continue;
                }
                if let (Some(a), Some(bb), Some(c)) = (p3(&t[0]), p3(&t[1]), p3(&t[2])) {
                    target.tri(a, bb, c);
                }
            }
            // Windows on vertical walls.
            if style == BagStyle::Mooi && *k == Kind::Wall && !tall_old {
                if let Some(r) = surf.first() {
                    let (mut nx, mut ny, mut nz) = (0.0, 0.0, 0.0);
                    for i in 0..r.len() {
                        let (a, c) = (r[i], r[(i + 1) % r.len()]);
                        nx += (a[1] - c[1]) * (a[2] + c[2]);
                        ny += (a[2] - c[2]) * (a[0] + c[0]);
                        nz += (a[0] - c[0]) * (a[1] + c[1]);
                    }
                    let len = (nx * nx + ny * ny + nz * nz).sqrt();
                    if len > 0.0 && (nz / len).abs() < 0.1 {
                        let h = (nx * nx + ny * ny).sqrt();
                        let mut o = [nx / h, ny / h];
                        // Outward: away from the building's centre.
                        let m = r.iter().fold([0.0, 0.0], |s, p| [s[0] + p[0], s[1] + p[1]]);
                        let m = [m[0] / r.len() as f64, m[1] / r.len() as f64];
                        if (m[0] - centre[0]) * o[0] + (m[1] - centre[1]) * o[1] < 0.0 {
                            o = [-o[0], -o[1]];
                        }
                        panes += windows(r, o, ground, eaves, &mut win, &p3);
                    }
                }
            }
        }
        faces += rm.faces() + wm.faces();
        let rgb = |c: (u8, u8, u8)| Some(Color::Rgb { r: c.0, g: c.1, b: c.2 });
        let (roof_c, wall_c) = match style {
            BagStyle::Nlcs => (None, None),
            BagStyle::Realistisch => (rgb(if pitched { (142, 78, 62) } else { (104, 104, 108) }), rgb((214, 200, 184))),
            BagStyle::Maquette => (rgb((214, 214, 210)), rgb((238, 238, 234))),
            BagStyle::Mooi if tall_old => (rgb((74, 78, 86)), rgb((196, 182, 156))),
            BagStyle::Mooi => {
                let roof = photo.as_ref().and_then(|ph| roof_colour(ph, bbox, px, &roof_rings, pitched)).unwrap_or(if pitched { (142, 78, 62) } else { (104, 104, 108) });
                (rgb(roof), rgb(wall_colour(b.year, &b.id)))
            }
        };
        if !rm.is_empty() {
            out.put(&roofs, rm.finish(&roofs.name, roof_c, Some(&d)));
        }
        if !wm.is_empty() {
            out.put(&walls, wm.finish(&walls.name, wall_c, Some(&d)));
        }
        if !win.is_empty() {
            out.put(&walls, win.finish(&walls.name, rgb((44, 54, 68)), Some(&d)));
        }
    }
    progress("3D BAG tekenen…".into());
    let n = out.write(&format!("3D BAG LoD {}", lod.code()))?;
    crate::bridge::note_source("3D BAG (TU Delft 3D geoinformation, CC BY 4.0)");
    if photo.is_some() {
        crate::bridge::note_source("Luchtfoto (Beeldmateriaal.nl / PDOK, CC BY 4.0)");
    }
    super::api::note("panden", buildings.len());
    super::api::note("afgekapt", cut);
    super::api::note("vlakken", faces);
    super::api::note("ramen", panes);
    super::api::note("lod", lod.code());
    Ok(format!(
        "3D BAG LoD {}: {} panden{} met {faces} driehoeken{} — {n} elementen op {} en {}.",
        lod.code(),
        buildings.len(),
        if cut { format!(" (maximum {max} bereikt; verhoog MAX=)") } else { String::new() },
        if panes > 0 { format!(" en {panes} ramen") } else { String::new() },
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
/// The PDOK aerial photo of an RD box at `px` m a pixel, in tiles the
/// service accepts. Pixel (0, 0) is the top left corner.
fn aerial_mosaic(gb: [f64; 4], px: f64, progress: &dyn Fn(String)) -> Result<image::RgbaImage, String> {
    let (pw, ph) = (((gb[2] - gb[0]) / px).ceil() as u32, ((gb[3] - gb[1]) / px).ceil() as u32);
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
    Ok(photo)
}

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
    // The photo at about four pixels a cell.
    let px = (cell / 4.0).max(0.25);
    let gb = [grid.x0, grid.y1 - grid.h as f64 * grid.cell, grid.x0 + grid.w as f64 * grid.cell, grid.y1];
    let photo = aerial_mosaic(gb, px, progress)?;
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

// ── Water level (flood picture) ─────────────────────────────────────────────

/// The AHN ground of the last `water_level`, so a series of levels (a film,
/// a what-if) fetches it once.
fn water_ground() -> &'static std::sync::Mutex<Option<([f64; 4], crate::gis::ahn::Grid)>> {
    static G: std::sync::OnceLock<std::sync::Mutex<Option<([f64; 4], crate::gis::ahn::Grid)>>> = std::sync::OnceLock::new();
    G.get_or_init(Default::default)
}

/// Share of the project area below `level` (m NAP) on the AHN ground; cells
/// without a height (water) count as below.
fn share_below(ctx: &Ctx, b: [f64; 4], level: f64, progress: &dyn Fn(String)) -> Result<f64, String> {
    let mut cache = water_ground().lock().unwrap();
    if cache.as_ref().is_none_or(|(bb, _)| *bb != b) {
        let bron = super::hoogtebron(ctx)?;
        progress(format!("{} ophalen…", bron.label()));
        let cell = (((b[2] - b[0]) * (b[3] - b[1])) / 100_000.0).sqrt().max(2.0);
        *cache = Some((b, bron.grid(b, cell, progress)?));
    }
    let grid = &cache.as_ref().unwrap().1;
    let wet = grid.z.iter().filter(|z| !z.is_finite() || (**z as f64) < level).count();
    Ok(wet as f64 / grid.z.len().max(1) as f64)
}

/// `CIV_WATERPEIL PEIL=<m NAP>`: a water surface at that level over the
/// project area, to see what floods (a mesh: the terrain and the buildings
/// above it stay visible). Again with another level moves the same surface.
/// Says which share of the area lies lower (AHN ground).
pub fn water_level(opt: &super::CivielOptions, level: f64, progress: &dyn Fn(String)) -> Result<String, String> {
    let ctx = Ctx::new(opt)?;
    let b = project_rd_bbox(&ctx)?;
    let z = level * ctx.f;
    let known: Vec<_> = crate::bridge::index_read().into_iter().filter(|(k, _, _)| k == "waterpeil").flat_map(|(_, _, h)| h).collect();
    let mut moved = 0;
    if !known.is_empty() {
        let meshes: Vec<EntityType> = crate::bridge::entities(&known)?
            .into_iter()
            .flatten()
            .filter_map(|e| match e {
                EntityType::Mesh(mut m) => {
                    for v in &mut m.vertices {
                        v.z = z;
                    }
                    Some(EntityType::Mesh(m))
                }
                _ => None,
            })
            .collect();
        if !meshes.is_empty() {
            moved = crate::bridge::update_entities("Waterpeil", meshes)?;
        }
    }
    if moved == 0 {
        let to = rd_to_drawing(&ctx)?;
        let corner = |x: f64, y: f64| -> Result<P3, String> { to([x, y]).map(|q| [q[0], q[1], z]).ok_or_else(|| "projectgebied buiten het CRS".to_string()) };
        let mut mb = MeshBuilder::default();
        mb.quad(corner(b[0], b[1])?, corner(b[2], b[1])?, corner(b[2], b[3])?, corner(b[0], b[3])?);
        // A temporary layer: this is a what-if, not existing or new work.
        let obj = nlcs::object_by_name("OG", "WATER").ok_or("NLCS-object OG WATER ontbreekt")?;
        let layer = nlcs::layer(obj, nlcs::Status::T, &ctx.opt.nlcs.discipline, Element::G);
        let d = record(&[("bron", "waterpeil".into()), ("peil_m_nap", format!("{level:.2}"))]);
        let mut out = Out::default();
        out.put(&layer, mb.finish(&layer.name, Some(Color::Rgb { r: 38, g: 112, b: 168 }), Some(&d)));
        let handles = out.write_handles("Waterpeil")?;
        crate::bridge::index_add("waterpeil", "vlak", &handles, true);
    }
    let share = share_below(&ctx, b, level, progress)?;
    super::api::note("peil_m_nap", level);
    super::api::note("aandeel_lager", (share * 1000.0).round() / 1000.0);
    Ok(format!(
        "Waterpeil {} m: {:.0} % van het projectgebied ligt lager (AHN-maaiveld){}.",
        nap(level, 2),
        share * 100.0,
        if moved > 0 { " — vlak verschoven" } else { " — watervlak geplaatst op T-…-OG-WATER-G" }
    ))
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
            // A tree: trunk and a round crown (a flattened sphere whose
            // corners are shared, so the host shades it smooth).
            let mut m = MeshBuilder::default();
            let (r, rv, zc) = (2.5 * f, 0.33 * h, z + 0.67 * h);
            let (seg, rings) = (12usize, 7usize);
            let at = |i: usize, j: usize| -> P3 {
                let lat = std::f64::consts::PI * (j as f64 / rings as f64 - 0.5);
                let lon = std::f64::consts::TAU * (i % seg) as f64 / seg as f64;
                [p[0] + r * lat.cos() * lon.cos(), p[1] + r * lat.cos() * lon.sin(), zc + rv * lat.sin()]
            };
            for j in 0..rings {
                for i in 0..seg {
                    let (a, b, c2, d2) = (at(i, j), at(i + 1, j), at(i + 1, j + 1), at(i, j + 1));
                    if j == 0 {
                        m.tri(a, c2, d2);
                    } else if j == rings - 1 {
                        m.tri(a, b, d2);
                    } else {
                        m.quad(a, b, c2, d2);
                    }
                }
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
    fn building_styles() {
        assert_eq!(BagStyle::parse("Mooi"), Some(BagStyle::Mooi));
        assert_eq!(BagStyle::parse("maquette"), Some(BagStyle::Maquette));
        assert_eq!(BagStyle::parse("iets"), None);
        // Old brick before 1850, light after 2010; the same building always alike.
        let (old, new) = (wall_colour(Some(1700), "a"), wall_colour(Some(2015), "a"));
        assert!(old.0 > old.2 + 50 && new.0 > 200 && new.2 > 200, "{old:?} {new:?}");
        assert_eq!(wall_colour(Some(1700), "NL.IMBAG.Pand.1"), wall_colour(Some(1700), "NL.IMBAG.Pand.1"));
    }

    #[test]
    fn windows_per_storey() {
        // A wall 10 m long, 9 m high, facing south (y = 0 plane, outward −y).
        let ring = vec![[0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [10.0, 0.0, 9.0], [0.0, 0.0, 9.0]];
        let mut mb = MeshBuilder::default();
        let p3 = |p: &[f64; 3]| Some(*p);
        let n = windows(&ring, [0.0, -1.0], 0.0, 9.0, &mut mb, &p3);
        // Four windows a row (10 m / 2.4 m), storeys at 0.9, 3.9 and 6.9 m.
        assert_eq!(n, 12, "{n}");
        // A gable: no window sticks out above the roof line.
        let gable = vec![[0.0, 0.0, 0.0], [8.0, 0.0, 0.0], [8.0, 0.0, 3.0], [4.0, 0.0, 7.0], [0.0, 0.0, 3.0]];
        let mut mb = MeshBuilder::default();
        let n = windows(&gable, [0.0, -1.0], 0.0, 3.0, &mut mb, &p3);
        assert!(n <= 3, "{n}");
    }

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
