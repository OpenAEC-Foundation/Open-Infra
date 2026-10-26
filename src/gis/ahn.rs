//! AHN — Actueel Hoogtebestand Nederland. Height grids from the PDOK WCS
//! (AHN4, 0.5 m DTM and DSM, float32 GeoTIFF in RD/NAP), with contours and
//! profiles computed from them.

use std::collections::HashMap;
use std::io::Cursor;

use super::crs::XY;
use super::http;

pub const WCS: &str = "https://service.pdok.nl/rws/ahn/wcs/v1_0";
/// Native cell size of the AHN WCS coverages (m).
pub const NATIVE_CELL: f64 = 0.5;
/// Largest grid one WCS request returns (cells per side).
const MAX_REQUEST_CELLS: usize = 2000;

/// Height model.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Model {
    /// Digital terrain model: the ground surface (maaiveld); holes under
    /// buildings and water.
    #[default]
    Dtm,
    /// Digital surface model: everything except water (buildings, trees).
    Dsm,
}

impl Model {
    pub fn coverage(self) -> &'static str {
        match self {
            Model::Dtm => "dtm_05m",
            Model::Dsm => "dsm_05m",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Model::Dtm => "DTM — maaiveld",
            Model::Dsm => "DSM — oppervlak (gebouwen, bomen)",
        }
    }
}

/// A height grid in RD: row 0 is the top (north) row.
#[derive(Clone, Debug)]
pub struct Grid {
    /// Left edge (x) and top edge (y) of the grid.
    pub x0: f64,
    pub y1: f64,
    pub cell: f64,
    pub w: usize,
    pub h: usize,
    /// Heights in m NAP, NaN where AHN has no value.
    pub z: Vec<f32>,
}

impl Grid {
    fn blank(x0: f64, y1: f64, cell: f64, w: usize, h: usize) -> Self {
        Self { x0, y1, cell, w, h, z: vec![f32::NAN; w * h] }
    }

    /// The value of cell (col, row), if any.
    pub fn at(&self, col: usize, row: usize) -> Option<f64> {
        if col >= self.w || row >= self.h {
            return None;
        }
        let v = self.z[row * self.w + col];
        v.is_finite().then_some(v as f64)
    }

    /// Centre of cell (col, row) in RD.
    pub fn centre(&self, col: usize, row: usize) -> XY {
        [self.x0 + (col as f64 + 0.5) * self.cell, self.y1 - (row as f64 + 0.5) * self.cell]
    }

    /// Bilinear height at an RD position; the nearest cell where a
    /// neighbour has no value.
    pub fn sample(&self, x: f64, y: f64) -> Option<f64> {
        let fx = (x - self.x0) / self.cell - 0.5;
        let fy = (self.y1 - y) / self.cell - 0.5;
        if fx < -0.5 || fy < -0.5 || fx > self.w as f64 - 0.5 || fy > self.h as f64 - 0.5 {
            return None;
        }
        let (c0, r0) = (fx.floor().max(0.0) as usize, fy.floor().max(0.0) as usize);
        let (c1, r1) = ((c0 + 1).min(self.w - 1), (r0 + 1).min(self.h - 1));
        let (tx, ty) = ((fx - c0 as f64).clamp(0.0, 1.0), (fy - r0 as f64).clamp(0.0, 1.0));
        match (self.at(c0, r0), self.at(c1, r0), self.at(c0, r1), self.at(c1, r1)) {
            (Some(a), Some(b), Some(c), Some(d)) => {
                let top = a + (b - a) * tx;
                let bottom = c + (d - c) * tx;
                Some(top + (bottom - top) * ty)
            }
            _ => {
                let col = (fx.round().max(0.0) as usize).min(self.w - 1);
                let row = (fy.round().max(0.0) as usize).min(self.h - 1);
                self.at(col, row)
            }
        }
    }

    /// Lowest and highest height in the grid.
    pub fn range(&self) -> Option<(f64, f64)> {
        let mut r: Option<(f64, f64)> = None;
        for v in self.z.iter().filter(|v| v.is_finite()) {
            let v = *v as f64;
            r = Some(r.map_or((v, v), |(lo, hi)| (lo.min(v), hi.max(v))));
        }
        r
    }

    /// Share of cells with a value.
    pub fn coverage(&self) -> f64 {
        if self.z.is_empty() {
            return 0.0;
        }
        self.z.iter().filter(|v| v.is_finite()).count() as f64 / self.z.len() as f64
    }
}

/// Decode a WCS GeoTIFF (float32, georeferenced by tie point and pixel scale).
pub fn decode_geotiff(bytes: &[u8]) -> Result<Grid, String> {
    use tiff::decoder::{Decoder, DecodingResult};
    use tiff::tags::Tag;
    let err = |e: tiff::TiffError| format!("AHN GeoTIFF: {e}");
    let mut dec = Decoder::new(Cursor::new(bytes)).map_err(err)?;
    let (w, h) = dec.dimensions().map_err(err)?;
    let scale = dec.get_tag_f64_vec(Tag::ModelPixelScaleTag).map_err(err)?;
    let tie = dec.get_tag_f64_vec(Tag::ModelTiepointTag).map_err(err)?;
    let nodata: Option<f32> = dec.get_tag_ascii_string(Tag::GdalNodata).ok().and_then(|s| s.trim().trim_end_matches('\0').parse().ok());
    if scale.len() < 2 || tie.len() < 6 {
        return Err("AHN GeoTIFF zonder georeferentie".into());
    }
    let values: Vec<f32> = match dec.read_image().map_err(err)? {
        DecodingResult::F32(v) => v,
        DecodingResult::F64(v) => v.into_iter().map(|x| x as f32).collect(),
        _ => return Err("AHN GeoTIFF: geen float-raster".into()),
    };
    let z = values
        .into_iter()
        .map(|v| if !v.is_finite() || nodata.is_some_and(|n| v == n) || v.abs() > 1.0e4 { f32::NAN } else { v })
        .collect();
    // Tie point (i, j, k, x, y, z): raster (0, 0) is the top-left corner.
    Ok(Grid { x0: tie[3] - tie[0] * scale[0], y1: tie[4] + tie[1] * scale[1], cell: scale[0], w: w as usize, h: h as usize, z })
}

fn request(bbox: [f64; 4], model: Model, cell: f64) -> Result<Grid, String> {
    let mut url = format!(
        "{WCS}?SERVICE=WCS&VERSION=2.0.1&REQUEST=GetCoverage&COVERAGEID={}&SUBSET=x({},{})&SUBSET=y({},{})&FORMAT=image/tiff",
        model.coverage(),
        bbox[0], bbox[2], bbox[1], bbox[3],
    );
    if cell > NATIVE_CELL * 1.01 {
        url.push_str(&format!("&SCALEFACTOR={}", NATIVE_CELL / cell));
    }
    let f = http::get(&url)?;
    if !f.content_type.contains("tiff") {
        let text = String::from_utf8_lossy(&f.body);
        return Err(http::service_error(&text).unwrap_or_else(|| "AHN WCS gaf geen GeoTIFF terug".into()));
    }
    decode_geotiff(&f.body)
}

/// Heights of `model` for an RD box at `cell` metres (0.5 = full AHN
/// resolution), in tiles the WCS accepts, put together into one grid.
/// The AHN coverage in RD (the Netherlands with a margin).
pub const EXTENT: [f64; 4] = [-10_000.0, 290_000.0, 290_000.0, 630_000.0];

pub fn fetch(bbox: [f64; 4], model: Model, cell: f64, progress: &dyn Fn(String)) -> Result<Grid, String> {
    if bbox[2] < EXTENT[0] || bbox[0] > EXTENT[2] || bbox[3] < EXTENT[1] || bbox[1] > EXTENT[3] || !bbox.iter().all(|v| v.is_finite()) {
        return Err("buiten het AHN-gebied: AHN heeft alleen hoogtes voor Nederland".into());
    }
    let cell = cell.max(NATIVE_CELL);
    let snap = |v: f64, up: bool| if up { (v / cell).ceil() * cell } else { (v / cell).floor() * cell };
    let (x0, y0, x1, y1) = (snap(bbox[0], false), snap(bbox[1], false), snap(bbox[2], true), snap(bbox[3], true));
    let (w, h) = (((x1 - x0) / cell).round() as usize, ((y1 - y0) / cell).round() as usize);
    if w == 0 || h == 0 {
        return Err("leeg gebied".into());
    }
    if w * h > 36_000_000 {
        return Err(format!("{w} × {h} cellen is te veel; kies een grovere celgrootte of een kleiner gebied"));
    }
    let mut grid = Grid::blank(x0, y1, cell, w, h);
    let step = MAX_REQUEST_CELLS as f64 * cell;
    let (nx, ny) = (((x1 - x0) / step).ceil() as usize, ((y1 - y0) / step).ceil() as usize);
    for ty in 0..ny {
        for tx in 0..nx {
            progress(format!("AHN {} ophalen: tegel {}/{}…", model.coverage(), ty * nx + tx + 1, nx * ny));
            let b = [x0 + tx as f64 * step, y0 + ty as f64 * step, (x0 + (tx + 1) as f64 * step).min(x1), (y0 + (ty + 1) as f64 * step).min(y1)];
            let tile = request(b, model, cell)?;
            // Copy by position: the WCS snaps to its own grid.
            for r in 0..tile.h {
                for c in 0..tile.w {
                    let [x, y] = tile.centre(c, r);
                    let (gc, gr) = (((x - x0) / cell).floor(), ((y1 - y) / cell).floor());
                    if gc >= 0.0 && gr >= 0.0 && (gc as usize) < w && (gr as usize) < h {
                        grid.z[gr as usize * w + gc as usize] = tile.z[r * tile.w + c];
                    }
                }
            }
        }
    }
    Ok(grid)
}

/// Heights along lines that may run far: AHN tiles of 250 m fetched on
/// demand and kept.
pub struct Sampler {
    pub model: Model,
    pub cell: f64,
    tiles: HashMap<(i64, i64), Option<Grid>>,
    pub error: Option<String>,
}

const SAMPLER_TILE: f64 = 250.0;

impl Sampler {
    pub fn new(model: Model, cell: f64) -> Self {
        Self { model, cell: cell.max(NATIVE_CELL), tiles: HashMap::new(), error: None }
    }

    pub fn height(&mut self, x: f64, y: f64) -> Option<f64> {
        let key = ((x / SAMPLER_TILE).floor() as i64, (y / SAMPLER_TILE).floor() as i64);
        if !self.tiles.contains_key(&key) {
            // A margin of a few cells keeps bilinear sampling at tile edges.
            let m = 4.0 * self.cell;
            let b = [key.0 as f64 * SAMPLER_TILE - m, key.1 as f64 * SAMPLER_TILE - m, (key.0 + 1) as f64 * SAMPLER_TILE + m, (key.1 + 1) as f64 * SAMPLER_TILE + m];
            let grid = match fetch(b, self.model, self.cell, &|_| {}) {
                Ok(g) => Some(g),
                Err(e) => {
                    self.error = Some(e);
                    None
                }
            };
            self.tiles.insert(key, grid);
        }
        self.tiles.get(&key)?.as_ref()?.sample(x, y)
    }
}

// ── Profiles ────────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct Station {
    /// Distance along the route (m).
    pub chainage: f64,
    pub at: XY,
    pub ground: Option<f64>,
    pub surface: Option<f64>,
}

/// Stations every `step` m along a route in RD (plus every vertex), with
/// the DTM and, when asked, the DSM height.
pub fn profile(route: &[XY], step: f64, surface: bool, cell: f64, progress: &dyn Fn(String)) -> Result<Vec<Station>, String> {
    if route.len() < 2 {
        return Err("een profiel heeft minstens twee punten nodig".into());
    }
    let positions = stations(route, step);
    let mut dtm = Sampler::new(Model::Dtm, cell);
    let mut dsm = Sampler::new(Model::Dsm, cell);
    let n = positions.len();
    let mut out = Vec::with_capacity(n);
    for (k, (chainage, at)) in positions.into_iter().enumerate() {
        if k % 200 == 0 {
            progress(format!("AHN-hoogtes langs het profiel: {k}/{n}…"));
        }
        let ground = dtm.height(at[0], at[1]);
        let surface = if surface { dsm.height(at[0], at[1]) } else { None };
        out.push(Station { chainage, at, ground, surface });
    }
    if out.iter().all(|s| s.ground.is_none() && s.surface.is_none()) {
        return Err(dtm.error.or(dsm.error).unwrap_or_else(|| "AHN heeft geen hoogtes langs deze lijn (buiten Nederland?)".into()));
    }
    Ok(out)
}

/// Chainage and position of the stations: every `step` m from the start,
/// and every vertex of the route.
pub fn stations(route: &[XY], step: f64) -> Vec<(f64, XY)> {
    let step = step.max(0.1);
    let mut positions: Vec<(f64, XY)> = Vec::new();
    let mut run = 0.0;
    for w in route.windows(2) {
        let (a, b) = (w[0], w[1]);
        let len = (b[0] - a[0]).hypot(b[1] - a[1]);
        if len <= 0.0 {
            continue;
        }
        if positions.is_empty() {
            positions.push((0.0, a));
        }
        // Next multiple of the step after the start of this segment.
        let mut s = ((run / step).floor() + 1.0) * step - run;
        while s < len - 1e-9 {
            let t = s / len;
            positions.push((run + s, [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]));
            s += step;
        }
        run += len;
        positions.push((run, b));
    }
    positions.dedup_by(|a, b| (a.0 - b.0).abs() < 1e-6);
    positions
}

// ── Contours ────────────────────────────────────────────────────────────────

/// Contour levels between the grid's lowest and highest value.
pub fn levels(range: (f64, f64), interval: f64) -> Vec<f64> {
    let interval = interval.max(0.01);
    let mut l = (range.0 / interval).ceil() * interval;
    let mut out = Vec::new();
    while l <= range.1 && out.len() < 2000 {
        out.push((l * 1000.0).round() / 1000.0);
        l += interval;
    }
    out
}

/// Contour lines of the grid (cell centres) for each level: polylines in
/// RD, closed when they come round. Marching squares with the saddle cases
/// decided by the cell average.
/// Lines shorter than `min_length` (m) are left out: in built-up areas the
/// AHN surface is full of tiny rings around kerbs and planters.
pub fn contours(grid: &Grid, levels: &[f64], min_length: f64) -> Vec<(f64, Vec<(Vec<XY>, bool)>)> {
    levels
        .iter()
        .map(|l| {
            let lines: Vec<(Vec<XY>, bool)> = contour_level(grid, *l).into_iter().filter(|(pts, closed)| length(pts, *closed) >= min_length).collect();
            (*l, lines)
        })
        .filter(|(_, v)| !v.is_empty())
        .collect()
}

fn length(pts: &[XY], closed: bool) -> f64 {
    let open: f64 = pts.windows(2).map(|w| (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1])).sum();
    match (closed, pts.first(), pts.last()) {
        (true, Some(a), Some(b)) => open + (a[0] - b[0]).hypot(a[1] - b[1]),
        _ => open,
    }
}

impl Grid {
    /// A 3 × 3 mean over the cells with a value; cells without one stay
    /// empty. Gives calmer contours.
    pub fn smoothed(&self) -> Grid {
        let mut out = self.clone();
        for r in 0..self.h {
            for c in 0..self.w {
                if self.at(c, r).is_none() {
                    continue;
                }
                let (mut sum, mut n) = (0.0, 0.0);
                for dr in -1i64..=1 {
                    for dc in -1i64..=1 {
                        let (cc, rr) = (c as i64 + dc, r as i64 + dr);
                        if cc >= 0 && rr >= 0 {
                            if let Some(v) = self.at(cc as usize, rr as usize) {
                                sum += v;
                                n += 1.0;
                            }
                        }
                    }
                }
                out.z[r * self.w + c] = (sum / n) as f32;
            }
        }
        out
    }
}

/// Edge key: (col, row, 0) the horizontal edge from centre (col,row) to
/// (col+1,row); (col, row, 1) the vertical edge from (col,row) to (col,row+1).
type Edge = (usize, usize, u8);

fn contour_level(g: &Grid, level: f64) -> Vec<(Vec<XY>, bool)> {
    if g.w < 2 || g.h < 2 {
        return Vec::new();
    }
    // Values exactly on the level count as above it.
    let above = |v: f64| v >= level;
    let cross = |e: Edge| -> XY {
        let (c, r, d) = e;
        let (c2, r2) = if d == 0 { (c + 1, r) } else { (c, r + 1) };
        let (a, b) = (g.at(c, r).unwrap_or(level), g.at(c2, r2).unwrap_or(level));
        let t = if (b - a).abs() < 1e-12 { 0.5 } else { ((level - a) / (b - a)).clamp(0.0, 1.0) };
        let (p, q) = (g.centre(c, r), g.centre(c2, r2));
        [p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t]
    };
    let mut segments: Vec<(Edge, Edge)> = Vec::new();
    for r in 0..g.h - 1 {
        for c in 0..g.w - 1 {
            let (Some(tl), Some(tr), Some(bl), Some(br)) = (g.at(c, r), g.at(c + 1, r), g.at(c, r + 1), g.at(c + 1, r + 1)) else { continue };
            let idx = (above(tl) as u8) << 3 | (above(tr) as u8) << 2 | (above(br) as u8) << 1 | above(bl) as u8;
            let top: Edge = (c, r, 0);
            let bottom: Edge = (c, r + 1, 0);
            let left: Edge = (c, r, 1);
            let right: Edge = (c + 1, r, 1);
            let centre_above = above((tl + tr + bl + br) / 4.0);
            match idx {
                0 | 15 => {}
                1 | 14 => segments.push((left, bottom)),
                2 | 13 => segments.push((bottom, right)),
                3 | 12 => segments.push((left, right)),
                4 | 11 => segments.push((top, right)),
                6 | 9 => segments.push((top, bottom)),
                7 | 8 => segments.push((left, top)),
                5 => {
                    // tr and bl above.
                    if centre_above {
                        segments.push((left, top));
                        segments.push((bottom, right));
                    } else {
                        segments.push((left, bottom));
                        segments.push((top, right));
                    }
                }
                10 => {
                    // tl and br above.
                    if centre_above {
                        segments.push((left, bottom));
                        segments.push((top, right));
                    } else {
                        segments.push((left, top));
                        segments.push((bottom, right));
                    }
                }
                _ => {}
            }
        }
    }
    // Chain the segments through their shared edges.
    let mut at_edge: HashMap<Edge, Vec<usize>> = HashMap::new();
    for (i, (a, b)) in segments.iter().enumerate() {
        at_edge.entry(*a).or_default().push(i);
        at_edge.entry(*b).or_default().push(i);
    }
    let mut used = vec![false; segments.len()];
    let mut lines = Vec::new();
    for start in 0..segments.len() {
        if used[start] {
            continue;
        }
        used[start] = true;
        let mut chain: std::collections::VecDeque<Edge> = [segments[start].0, segments[start].1].into_iter().collect();
        // Grow forward from the back, then backward from the front.
        for forward in [true, false] {
            loop {
                let end = if forward { *chain.back().unwrap() } else { *chain.front().unwrap() };
                let next = at_edge.get(&end).and_then(|v| v.iter().copied().find(|i| !used[*i]));
                let Some(i) = next else { break };
                used[i] = true;
                let (a, b) = segments[i];
                let other = if a == end { b } else { a };
                if forward {
                    chain.push_back(other);
                } else {
                    chain.push_front(other);
                }
            }
        }
        let closed = chain.len() > 3 && chain.front() == chain.back();
        let mut pts: Vec<XY> = chain.iter().map(|e| cross(*e)).collect();
        if closed {
            pts.pop();
        }
        let pts = simplify(&pts, g.cell * 0.25, closed);
        if pts.len() >= 2 {
            lines.push((pts, closed));
        }
    }
    lines
}

/// Douglas–Peucker simplification.
pub fn simplify(pts: &[XY], tol: f64, closed: bool) -> Vec<XY> {
    if pts.len() < 3 {
        return pts.to_vec();
    }
    fn dp(pts: &[XY], tol: f64, keep: &mut [bool], a: usize, b: usize) {
        if b <= a + 1 {
            return;
        }
        let (p, q) = (pts[a], pts[b]);
        let (dx, dy) = (q[0] - p[0], q[1] - p[1]);
        let len = dx.hypot(dy);
        let (mut best, mut at) = (0.0, a);
        for (i, r) in pts.iter().enumerate().take(b).skip(a + 1) {
            let d = if len < 1e-12 { (r[0] - p[0]).hypot(r[1] - p[1]) } else { ((r[0] - p[0]) * dy - (r[1] - p[1]) * dx).abs() / len };
            if d > best {
                best = d;
                at = i;
            }
        }
        if best > tol {
            keep[at] = true;
            dp(pts, tol, keep, a, at);
            dp(pts, tol, keep, at, b);
        }
    }
    let n = pts.len();
    let mut keep = vec![false; n];
    keep[0] = true;
    keep[n - 1] = true;
    if closed {
        // Split a ring at its farthest point from the start.
        let far = (1..n).max_by(|i, j| {
            let d = |k: usize| (pts[k][0] - pts[0][0]).hypot(pts[k][1] - pts[0][1]);
            d(*i).total_cmp(&d(*j))
        });
        if let Some(f) = far {
            keep[f] = true;
            dp(pts, tol, &mut keep, 0, f);
            dp(pts, tol, &mut keep, f, n - 1);
        }
    } else {
        dp(pts, tol, &mut keep, 0, n - 1);
    }
    pts.iter().zip(keep).filter(|(_, k)| *k).map(|(p, _)| *p).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cone() -> Grid {
        // A 21 × 21 grid with a 10 m high cone in the middle.
        let (w, h) = (21, 21);
        let mut g = Grid::blank(0.0, 21.0, 1.0, w, h);
        for r in 0..h {
            for c in 0..w {
                let [x, y] = g.centre(c, r);
                g.z[r * w + c] = (10.0 - (x - 10.5).hypot(y - 10.5)).max(0.0) as f32;
            }
        }
        g
    }

    #[test]
    fn samples_and_contours() {
        let g = cone();
        assert!((g.sample(10.5, 10.5).unwrap() - 10.0).abs() < 1e-6);
        let ls = levels(g.range().unwrap(), 2.5);
        assert_eq!(ls, vec![0.0, 2.5, 5.0, 7.5, 10.0]);
        let c = contours(&g, &[5.0], 0.0);
        assert_eq!(c.len(), 1);
        let (_, lines) = &c[0];
        // One closed ring of radius ~5 around the top.
        assert_eq!(lines.len(), 1);
        assert!(lines[0].1);
        for p in &lines[0].0 {
            let r = (p[0] - 10.5).hypot(p[1] - 10.5);
            assert!((r - 5.0).abs() < 0.3, "radius {r}");
        }
    }

    #[test]
    fn profile_stations_follow_the_route() {
        let route = [[0.0, 0.0], [10.0, 0.0], [10.0, 5.0]];
        let st = stations(&route, 4.0);
        let ch: Vec<f64> = st.iter().map(|s| s.0).collect();
        // Every 4 m, plus the bend at 10 m and the end at 15 m.
        assert_eq!(ch, vec![0.0, 4.0, 8.0, 10.0, 12.0, 15.0]);
        assert_eq!(st[4].1, [10.0, 2.0]);
        assert_eq!(levels((0.2, 1.1), 0.5), vec![0.5, 1.0]);
    }

    /// Against the PDOK WCS: the Grote Kerk square in Dordrecht.
    #[test]
    #[ignore]
    fn fetches_ahn_around_the_grote_kerk() {
        let g = fetch([104850.0, 425300.0, 104950.0, 425360.0], Model::Dtm, 0.5, &|_| {}).unwrap();
        assert_eq!((g.w, g.h), (200, 120));
        let (lo, hi) = g.range().unwrap();
        println!("DTM {lo:.2} .. {hi:.2} m NAP, coverage {:.0} %", g.coverage() * 100.0);
        let coarse = fetch([104000.0, 424000.0, 106000.0, 426000.0], Model::Dtm, 5.0, &|_| {}).unwrap();
        assert_eq!((coarse.w, coarse.h), (400, 400));
        let st = profile(&[[104860.0, 425330.0], [104940.0, 425330.0]], 1.0, true, 0.5, &|_| {}).unwrap();
        let n = st.iter().filter(|s| s.ground.is_some()).count();
        println!("profile: {} stations, {n} with ground, max surface {:?}", st.len(), st.iter().filter_map(|s| s.surface).fold(None, |m: Option<f64>, v| Some(m.map_or(v, |m| m.max(v)))));
        let c = contours(&coarse.smoothed(), &levels(coarse.range().unwrap(), 1.0), 50.0);
        println!("contours: {} levels, {} lines", c.len(), c.iter().map(|(_, l)| l.len()).sum::<usize>());
    }
}
