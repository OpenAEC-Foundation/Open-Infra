//! AHN point clouds: the COPC tiles (1 × 1 km, Cloud Optimized Point Cloud)
//! on Het Waterschapshuis' object storage, read over HTTP range requests so
//! only the octree nodes in the import area are downloaded. Written out as
//! an indexed scan (.rcs) that Open CAD Studio attaches with
//! POINTCLOUDATTACH.

use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::sync::Arc;

use copc_rs::{BoundsSelection, CopcReader, LodSelection, Vector};

use super::http;

const BASE: &str = "https://fsn1.your-objectstorage.com/hwh-ahn";

/// AHN edition of the point cloud.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Edition {
    Ahn4,
    #[default]
    Ahn5,
}

impl Edition {
    pub const ALL: [Edition; 2] = [Edition::Ahn5, Edition::Ahn4];
    pub fn label(self) -> &'static str {
        match self {
            Edition::Ahn4 => "AHN4 (2020–2022)",
            Edition::Ahn5 => "AHN5 (2023–2025)",
        }
    }
    pub fn short(self) -> &'static str {
        match self {
            Edition::Ahn4 => "AHN4",
            Edition::Ahn5 => "AHN5",
        }
    }
    /// The COPC tile whose lower-left corner is (x, y) in RD (km grid).
    pub fn tile_url(self, x: i64, y: i64) -> String {
        let v = self.short();
        format!("{BASE}/{v}_KM/01_LAZ/{v}_C_{x:06}_{y:06}.COPC.LAZ")
    }
}

/// AHN point classes (ASPRS codes as AHN uses them).
pub const CLASSES: &[(u8, &str, [u8; 3])] = &[
    (2, "Maaiveld", [196, 160, 100]),
    (6, "Gebouwen", [214, 86, 64]),
    (9, "Water", [64, 128, 224]),
    (26, "Kunstwerken", [170, 90, 200]),
    (1, "Overig (o.a. begroeiing)", [86, 170, 72]),
    (14, "Hoogspanning", [240, 200, 40]),
];

/// How points are coloured in the scan.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Colouring {
    #[default]
    Classification,
    Height,
    /// The aerial-photo RGB the edition carries, else by class.
    Rgb,
}

impl Colouring {
    pub const ALL: [Colouring; 3] = [Colouring::Classification, Colouring::Height, Colouring::Rgb];
    pub fn label(self) -> &'static str {
        match self {
            Colouring::Classification => "Klasse",
            Colouring::Height => "Hoogte",
            Colouring::Rgb => "RGB (luchtfoto)",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Options {
    pub edition: Edition,
    /// Point spacing to aim for (m): the COPC level of detail.
    pub spacing: f64,
    /// ASPRS classes to keep (empty: all).
    pub classes: Vec<u8>,
    pub max_points: usize,
    pub colouring: Colouring,
}

impl Default for Options {
    fn default() -> Self {
        Self { edition: Edition::default(), spacing: 0.25, classes: Vec::new(), max_points: 8_000_000, colouring: Colouring::default() }
    }
}

pub struct Cloud {
    /// Tiles taken from the older edition because the chosen one lacks them.
    pub fallback_tiles: usize,
    /// RD + NAP.
    pub xyz: Vec<[f64; 3]>,
    pub class: Vec<u8>,
    pub intensity: Vec<u16>,
    pub rgb: Option<Vec<[u8; 3]>>,
    /// The point limit cut the result short.
    pub truncated: bool,
    pub tiles: usize,
}

/// A remote file as Read + Seek through cached HTTP range requests.
struct RangeReader {
    url: String,
    len: u64,
    pos: u64,
    blocks: HashMap<u64, Arc<Vec<u8>>>,
    order: Vec<u64>,
    fetched: Arc<std::sync::atomic::AtomicU64>,
}

const BLOCK: u64 = 1 << 20;
const KEEP_BLOCKS: usize = 48;

impl RangeReader {
    fn open(url: &str, fetched: Arc<std::sync::atomic::AtomicU64>) -> Result<Self, String> {
        let len = http::content_length(url)?;
        Ok(Self { url: url.to_string(), len, pos: 0, blocks: HashMap::new(), order: Vec::new(), fetched })
    }

    fn block(&mut self, index: u64) -> std::io::Result<Arc<Vec<u8>>> {
        if let Some(b) = self.blocks.get(&index) {
            return Ok(b.clone());
        }
        let start = index * BLOCK;
        let end = (start + BLOCK).min(self.len) - 1;
        let bytes = http::get_range(&self.url, start, end).map_err(std::io::Error::other)?;
        self.fetched.fetch_add(bytes.len() as u64, std::sync::atomic::Ordering::Relaxed);
        let b = Arc::new(bytes);
        self.blocks.insert(index, b.clone());
        self.order.push(index);
        if self.order.len() > KEEP_BLOCKS {
            let old = self.order.remove(0);
            self.blocks.remove(&old);
        }
        Ok(b)
    }
}

impl Read for RangeReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.pos >= self.len || buf.is_empty() {
            return Ok(0);
        }
        let index = self.pos / BLOCK;
        let block = self.block(index)?;
        let offset = (self.pos - index * BLOCK) as usize;
        let n = buf.len().min(block.len().saturating_sub(offset));
        buf[..n].copy_from_slice(&block[offset..offset + n]);
        self.pos += n as u64;
        Ok(n)
    }
}

impl Seek for RangeReader {
    fn seek(&mut self, to: SeekFrom) -> std::io::Result<u64> {
        let next = match to {
            SeekFrom::Start(p) => p as i64,
            SeekFrom::End(d) => self.len as i64 + d,
            SeekFrom::Current(d) => self.pos as i64 + d,
        };
        if next < 0 {
            return Err(std::io::Error::other("seek voor het begin"));
        }
        self.pos = next as u64;
        Ok(self.pos)
    }
}

/// The 1 km tiles (lower-left corners) an RD box touches.
pub fn tiles_for(bbox: [f64; 4]) -> Vec<(i64, i64)> {
    let k = |v: f64| (v / 1000.0).floor() as i64 * 1000;
    let mut out = Vec::new();
    let mut y = k(bbox[1]);
    while (y as f64) < bbox[3] {
        let mut x = k(bbox[0]);
        while (x as f64) < bbox[2] {
            out.push((x, y));
            x += 1000;
        }
        y += 1000;
    }
    out
}

/// Points of the AHN point cloud inside an RD box.
pub fn fetch(bbox: [f64; 4], opt: &Options, progress: &dyn Fn(String)) -> Result<Cloud, String> {
    let tiles = tiles_for(bbox);
    if tiles.is_empty() {
        return Err("leeg gebied".into());
    }
    if tiles.len() > 16 {
        return Err(format!("het gebied raakt {} AHN-tegels van 1 km²; kies een kleiner gebied", tiles.len()));
    }
    let fetched = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let mut cloud = Cloud { fallback_tiles: 0, xyz: Vec::new(), class: Vec::new(), intensity: Vec::new(), rgb: None, truncated: false, tiles: tiles.len() };
    let mut rgb: Vec<[u8; 3]> = Vec::new();
    let mut any_rgb = false;
    let keep = |c: u8| opt.classes.is_empty() || opt.classes.contains(&c);
    for (k, (tx, ty)) in tiles.iter().enumerate() {
        progress(format!("{} tegel {}/{} ({}_{})…", opt.edition.short(), k + 1, tiles.len(), tx, ty));
        // AHN5 is not complete yet: a tile it lacks comes from AHN4.
        let mut editions = vec![opt.edition];
        if opt.edition == Edition::Ahn5 {
            editions.push(Edition::Ahn4);
        }
        let mut reader = None;
        for (i, ed) in editions.iter().enumerate() {
            match RangeReader::open(&ed.tile_url(*tx, *ty), fetched.clone()) {
                Ok(r) => {
                    if i > 0 {
                        cloud.fallback_tiles += 1;
                    }
                    reader = Some(r);
                    break;
                }
                // A tile at sea or outside the edition simply isn't there.
                Err(e) if e.contains("404") || e.contains("403") => continue,
                Err(e) => return Err(e),
            }
        }
        let Some(reader) = reader else { continue };
        let mut copc = CopcReader::new(reader).map_err(|e| format!("{}: {e}", opt.edition.short()))?;
        let bounds = copc_rs::Bounds { min: Vector { x: bbox[0], y: bbox[1], z: -200.0 }, max: Vector { x: bbox[2], y: bbox[3], z: 2000.0 } };
        let points = copc
            .points(LodSelection::Resolution(opt.spacing.max(0.05)), BoundsSelection::Within(bounds))
            .map_err(|e| format!("{}: {e}", opt.edition.short()))?;
        // The COPC reader panics on a broken read; keep that inside this tile.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut n = 0usize;
            for p in points {
                let class = u8::from(p.classification);
                if !keep(class) {
                    continue;
                }
                if cloud.xyz.len() >= opt.max_points {
                    cloud.truncated = true;
                    break;
                }
                cloud.xyz.push([p.x, p.y, p.z]);
                cloud.class.push(class);
                cloud.intensity.push(p.intensity);
                match p.color {
                    Some(c) => {
                        any_rgb = true;
                        rgb.push([(c.red >> 8) as u8, (c.green >> 8) as u8, (c.blue >> 8) as u8]);
                    }
                    None => rgb.push([0, 0, 0]),
                }
                n += 1;
                if n % 250_000 == 0 {
                    progress(format!(
                        "{} tegel {}/{}: {} punten, {:.1} MB opgehaald…",
                        opt.edition.short(),
                        k + 1,
                        tiles.len(),
                        cloud.xyz.len(),
                        fetched.load(std::sync::atomic::Ordering::Relaxed) as f64 / 1e6
                    ));
                }
            }
        }));
        if result.is_err() {
            return Err(format!("{}: de puntenwolk kon niet worden gelezen (netwerk?)", opt.edition.short()));
        }
        if cloud.truncated {
            break;
        }
    }
    if any_rgb {
        cloud.rgb = Some(rgb);
    }
    if cloud.xyz.is_empty() {
        return Err(format!("{} heeft hier geen punten (buiten Nederland, of andere klassen kiezen)", opt.edition.short()));
    }
    Ok(cloud)
}

/// Point colours for the chosen scheme.
pub fn colours(cloud: &Cloud, scheme: Colouring) -> Vec<[u8; 3]> {
    let by_class = |c: u8| CLASSES.iter().find(|k| k.0 == c).map(|k| k.2).unwrap_or([160, 160, 160]);
    match scheme {
        Colouring::Rgb if cloud.rgb.is_some() => cloud.rgb.clone().unwrap_or_default(),
        Colouring::Height => {
            // Ground-level range: 2nd to 98th percentile, so a church tower
            // does not wash out the street.
            let mut z: Vec<f64> = cloud.xyz.iter().map(|p| p[2]).collect();
            z.sort_by(f64::total_cmp);
            let lo = z[z.len() * 2 / 100];
            let hi = z[(z.len() * 98 / 100).min(z.len() - 1)].max(lo + 0.1);
            cloud.xyz.iter().map(|p| ramp(((p[2] - lo) / (hi - lo)).clamp(0.0, 1.0))).collect()
        }
        _ => cloud.class.iter().map(|c| by_class(*c)).collect(),
    }
}

/// Blue – cyan – green – yellow – red.
fn ramp(t: f64) -> [u8; 3] {
    let stops = [[40.0, 70.0, 200.0], [40.0, 190.0, 220.0], [70.0, 190.0, 80.0], [240.0, 220.0, 60.0], [220.0, 60.0, 50.0]];
    let f = t * (stops.len() - 1) as f64;
    let i = (f.floor() as usize).min(stops.len() - 2);
    let u = f - i as f64;
    let mix = |k: usize| (stops[i][k] + (stops[i + 1][k] - stops[i][k]) * u).round() as u8;
    [mix(0), mix(1), mix(2)]
}

// ── Indexed scan (.rcs) writer ──────────────────────────────────────────────

/// Node cube edge (m): the point records hold 18-bit millimetre offsets,
/// so a node spans at most 262 m.
const NODE_EDGE: f64 = 200.0;
const NODE_RECORD: usize = 368;
const HEADER: usize = 0x200;

/// Write points (drawing coordinates, already scaled) as an indexed scan
/// ("ADOCT" version 3), the layout Open CAD Studio's point cloud reader
/// takes: one node per 200 m cube, every record carrying its own position.
pub fn write_rcs(path: &Path, xyz: &[[f64; 3]], colours: &[[u8; 3]], intensity: &[u16]) -> Result<(), String> {
    if xyz.is_empty() {
        return Err("geen punten".into());
    }
    let mut lo = [f64::MAX; 3];
    let mut hi = [f64::MIN; 3];
    for p in xyz {
        for k in 0..3 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    // Points by node cube, relative to the low corner.
    let key = |p: &[f64; 3]| -> (i64, i64, i64) {
        (((p[0] - lo[0]) / NODE_EDGE).floor() as i64, ((p[1] - lo[1]) / NODE_EDGE).floor() as i64, ((p[2] - lo[2]) / NODE_EDGE).floor() as i64)
    };
    let mut nodes: HashMap<(i64, i64, i64), Vec<usize>> = HashMap::new();
    for (i, p) in xyz.iter().enumerate() {
        nodes.entry(key(p)).or_default().push(i);
    }
    let mut keys: Vec<_> = nodes.keys().copied().collect();
    keys.sort();

    // Section 1: node blocks (index entries, then 16-byte point records).
    let mut data: Vec<u8> = Vec::with_capacity(xyz.len() * 20);
    let mut directory: Vec<u8> = Vec::with_capacity(8 + keys.len() * NODE_RECORD);
    directory.extend_from_slice(&(keys.len() as u64).to_le_bytes());
    for k in &keys {
        let members = &nodes[k];
        let cube = [lo[0] + k.0 as f64 * NODE_EDGE, lo[1] + k.1 as f64 * NODE_EDGE, lo[2] + k.2 as f64 * NODE_EDGE];
        let start = data.len();
        for i in 0..members.len() as u32 {
            data.extend_from_slice(&i.to_le_bytes());
        }
        for &i in members {
            let p = xyz[i];
            let q = |v: f64, o: f64| (((v - o) * 1000.0).round().clamp(0.0, 262_143.0)) as u64;
            let packed = q(p[0], cube[0]) | q(p[1], cube[1]) << 18 | q(p[2], cube[2]) << 36;
            data.extend_from_slice(&packed.to_le_bytes());
            data.push((intensity.get(i).copied().unwrap_or(0) >> 8) as u8);
            let c = colours.get(i).copied().unwrap_or([200, 200, 200]);
            data.extend_from_slice(&[c[2], c[1], c[0]]);
            // Normal code past the cube map: "up".
            data.extend_from_slice(&u16::MAX.to_le_bytes());
            data.extend_from_slice(&[0, 0]);
        }
        let size = data.len() - start;
        let mut record = vec![0u8; NODE_RECORD];
        for (j, v) in cube.iter().enumerate() {
            record[j * 8..j * 8 + 8].copy_from_slice(&v.to_le_bytes());
        }
        // One level, no octree cells: the index is one entry per point.
        record[96..100].copy_from_slice(&1u32.to_le_bytes());
        record[100..104].copy_from_slice(&(members.len() as u32).to_le_bytes());
        record[104..108].copy_from_slice(&(size as u32).to_le_bytes());
        record[236..240].copy_from_slice(&(members.len() as u32).to_le_bytes());
        directory.extend_from_slice(&record);
    }

    let mut header = vec![0u8; HEADER];
    header[0..5].copy_from_slice(b"ADOCT");
    header[5] = 3;
    let put3 = |h: &mut Vec<u8>, at: usize, v: [f64; 3]| {
        for (j, x) in v.iter().enumerate() {
            h[at + j * 8..at + j * 8 + 8].copy_from_slice(&x.to_le_bytes());
        }
    };
    put3(&mut header, 0x10, [0.0; 3]);
    put3(&mut header, 0x28, [0.0; 3]);
    put3(&mut header, 0x40, [1.0; 3]);
    put3(&mut header, 0xA0, lo);
    put3(&mut header, 0xB8, hi);
    header[0xE0] = 1; // RGB
    header[0xE1] = 0; // normals
    header[0xE2] = 1; // intensity
    let id = scan_id(xyz.len(), lo);
    header[0xE3..0xE3 + id.len()].copy_from_slice(id.as_bytes());
    let data_at = HEADER as u64;
    let dir_at = data_at + data.len() as u64;
    header[0x129..0x12D].copy_from_slice(&2u32.to_le_bytes());
    let entry = |h: &mut Vec<u8>, at: usize, kind: u64, offset: u64, size: u64| {
        h[at..at + 8].copy_from_slice(&kind.to_le_bytes());
        h[at + 8..at + 16].copy_from_slice(&offset.to_le_bytes());
        h[at + 16..at + 24].copy_from_slice(&size.to_le_bytes());
    };
    entry(&mut header, 0x12D, 1, data_at, data.len() as u64);
    entry(&mut header, 0x12D + 24, 2, dir_at, directory.len() as u64);

    let mut f = std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    for part in [&header, &data, &directory] {
        f.write_all(part).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(())
}

/// A stable "{…}" identifier for the scan from its size and position.
fn scan_id(n: usize, lo: [f64; 3]) -> String {
    let a = (lo[0] * 100.0) as u64 ^ (n as u64).rotate_left(17);
    let b = (lo[1] * 100.0) as u64 ^ (lo[2] * 100.0) as u64;
    format!("{{{:08X}-{:04X}-{:04X}-{:04X}-{:012X}}}", a as u32, (a >> 32) as u16, (b >> 48) as u16, b as u16, (b >> 16) & 0xFFFF_FFFF_FFFF)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiles_cover_the_box() {
        assert_eq!(tiles_for([104735.0, 425195.0, 105035.0, 425445.0]), vec![(104000, 425000), (105000, 425000)]);
        assert_eq!(Edition::Ahn5.tile_url(104000, 425000), "https://fsn1.your-objectstorage.com/hwh-ahn/AHN5_KM/01_LAZ/AHN5_C_104000_425000.COPC.LAZ");
    }

    #[test]
    fn rcs_layout_matches_the_reader() {
        // Re-read with the layout Open CAD Studio's decoder uses.
        let pts: Vec<[f64; 3]> = (0..1000).map(|i| [104000.0 + i as f64 * 0.37, 425000.0 + (i % 50) as f64, 2.0 + (i % 7) as f64]).collect();
        let cols: Vec<[u8; 3]> = (0..1000).map(|i| [i as u8, 0, 255]).collect();
        let path = std::env::temp_dir().join("open-infra-test.rcs");
        write_rcs(&path, &pts, &cols, &vec![65535; 1000]).unwrap();
        let b = std::fs::read(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(&b[0..5], b"ADOCT");
        let u64_at = |at: usize| u64::from_le_bytes(b[at..at + 8].try_into().unwrap());
        let u32_at = |at: usize| u32::from_le_bytes(b[at..at + 4].try_into().unwrap());
        let f64_at = |at: usize| f64::from_bits(u64_at(at));
        assert_eq!(u32_at(0x129), 2);
        let (data, dir) = (u64_at(0x12D + 8) as usize, u64_at(0x12D + 24 + 8) as usize);
        let nodes = u64_at(dir) as usize;
        let mut block = data;
        let mut read = Vec::new();
        for n in 0..nodes {
            let rec = dir + 8 + n * NODE_RECORD;
            let cube = [f64_at(rec), f64_at(rec + 8), f64_at(rec + 16)];
            let points = u32_at(rec + 100) as usize;
            let size = u32_at(rec + 104) as usize;
            let mut entries = points;
            for level in 0..32 {
                entries += u32_at(rec + 108 + level * 4) as usize;
            }
            assert_eq!(entries * 4 + points * 16, size);
            for r in b[block + entries * 4..block + size].chunks_exact(16) {
                let v = u64::from_le_bytes(r[0..8].try_into().unwrap());
                let q = |s: u32| ((v >> s) & 0x3FFFF) as f64 * 1e-3;
                read.push(([cube[0] + q(0), cube[1] + q(18), cube[2] + q(36)], [r[11], r[10], r[9]]));
            }
            block += size;
        }
        assert_eq!(read.len(), 1000);
        for (p, c) in &read {
            let i = pts.iter().position(|q| (q[0] - p[0]).abs() < 0.001 && (q[1] - p[1]).abs() < 0.001 && (q[2] - p[2]).abs() < 0.001).expect("point back");
            assert_eq!(*c, cols[i]);
        }
    }

    /// Against the object storage: a small box at the Grote Kerk, Dordrecht.
    #[test]
    #[ignore]
    fn reads_ahn5_copc() {
        let opt = Options { spacing: 1.0, ..Options::default() };
        let c = fetch([104880.0, 425300.0, 104920.0, 425340.0], &opt, &|m| println!("{m}")).unwrap();
        let mut classes: HashMap<u8, usize> = HashMap::new();
        for k in &c.class {
            *classes.entry(*k).or_default() += 1;
        }
        let zmax = c.xyz.iter().map(|p| p[2]).fold(f64::MIN, f64::max);
        println!("{} points, classes {classes:?}, rgb {}, z max {zmax:.1}", c.xyz.len(), c.rgb.is_some());
        assert!(c.xyz.len() > 100);
    }
}
