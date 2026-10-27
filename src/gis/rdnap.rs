//! ETRS89 ↔ RD/NAP following NSGI's 2018 procedure, implementation variant 1
//! (RDNAPTRANS2018_v220627.pdf, the specification NSGI publishes): a
//! seven-parameter transformation at a fixed height, the `rdcorr2018`
//! correction grid, the RD double stereographic projection, and the
//! `nlgeo2018` quasi-geoid for NAP heights.
//!
//! The grids are not part of this program. They come from NSGI's official
//! download (CC BY 4.0, © NSGI), fetched once into the configuration folder by
//! [`ensure_grids`]. Until then the correction is 0, which is what the
//! procedure prescribes outside the grid: the result is the seven-parameter
//! transformation, about 0.25 m off at worst.
//!
//! The name RDNAPTRANS™ is a trademark of NSGI and is only used after their
//! written consent; this module does not claim it.

use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

const PI: f64 = std::f64::consts::PI;
const DEG: f64 = PI / 180.0;
const SEC: f64 = PI / (180.0 * 3600.0);
/// Iteration threshold: 1e-9 degree (§2.2.3).
const EPS: f64 = 1e-9 * DEG;

// Ellipsoids (§2.2.1, §2.2.3).
const GRS80_A: f64 = 6_378_137.0;
const GRS80_F: f64 = 1.0 / 298.257_222_101;
const BESSEL_A: f64 = 6_377_397.155;
const BESSEL_F: f64 = 1.0 / 299.152_812_8;

/// Seven parameters, coordinate frame convention, rigorous rotation (§2.2.2, §3.3).
struct Helmert {
    t: [f64; 3],
    /// α, β, γ in radians.
    r: [f64; 3],
    delta: f64,
}

const TO_RD: Helmert = Helmert { t: [-565.7346, -50.4058, -465.2895], r: [-0.395_023 * SEC, 0.330_776 * SEC, -1.876_073 * SEC], delta: -4.072_42e-6 };
const TO_ETRS: Helmert = Helmert { t: [565.7381, 50.4018, 465.2904], r: [0.395_026 * SEC, -0.330_772 * SEC, 1.876_074 * SEC], delta: 4.072_44e-6 };

/// Fixed heights in the datum step: 43 m on ETRS89 going to RD, 0 m on Bessel coming back.
const H0_ETRS: f64 = 43.0;
const H0_BESSEL: f64 = 0.0;

// RD projection (§2.4.1).
const PHI0: f64 = (52.0 + 9.0 / 60.0 + 22.178 / 3600.0) * DEG;
const LAM0: f64 = (5.0 + 23.0 / 60.0 + 15.5 / 3600.0) * DEG;
const K: f64 = 0.999_907_9;
const X0: f64 = 155_000.0;
const Y0: f64 = 463_000.0;

fn e2(f: f64) -> f64 {
    f * (2.0 - f)
}

fn to_geocentric(phi: f64, lam: f64, h: f64, a: f64, f: f64) -> [f64; 3] {
    let e2 = e2(f);
    let rn = a / (1.0 - e2 * phi.sin().powi(2)).sqrt();
    [(rn + h) * phi.cos() * lam.cos(), (rn + h) * phi.cos() * lam.sin(), (rn * (1.0 - e2) + h) * phi.sin()]
}

fn from_geocentric(p: [f64; 3], a: f64, f: f64) -> (f64, f64) {
    let e2 = e2(f);
    let [x, y, z] = p;
    let rho = x.hypot(y);
    let lam = if x == 0.0 && y == 0.0 { 0.0 } else { y.atan2(x) };
    if rho < 1e-4 {
        return (if z >= 0.0 { 90.0 * DEG } else { -90.0 * DEG }, lam);
    }
    let mut phi = 0.0_f64;
    for _ in 0..100 {
        let rn = a / (1.0 - e2 * phi.sin().powi(2)).sqrt();
        let next = ((z + e2 * rn * phi.sin()) / rho).atan();
        let done = (next - phi).abs() < EPS;
        phi = next;
        if done {
            break;
        }
    }
    (phi, lam)
}

fn helmert(p: [f64; 3], h: &Helmert) -> [f64; 3] {
    let (a, b, g) = (h.r[0], h.r[1], h.r[2]);
    let (sa, ca, sb, cb, sg, cg) = (a.sin(), a.cos(), b.sin(), b.cos(), g.sin(), g.cos());
    let r = [
        [cg * cb, cg * sb * sa + sg * ca, -cg * sb * ca + sg * sa],
        [-sg * cb, -sg * sb * sa + cg * ca, sg * sb * ca + cg * sa],
        [sb, -cb * sa, cb * ca],
    ];
    let s = 1.0 + h.delta;
    let mut out = [0.0; 3];
    for i in 0..3 {
        out[i] = s * (r[i][0] * p[0] + r[i][1] * p[1] + r[i][2] * p[2]) + h.t[i];
    }
    out
}

// ── Grids ───────────────────────────────────────────────────────────────────

const LAT_MIN: f64 = 50.0;
const LAT_MAX: f64 = 56.0;
const LON_MIN: f64 = 2.0;
const LON_MAX: f64 = 8.0;
const D_LAT: f64 = 0.0125;
const D_LON: f64 = 0.02;
const N_LAT: usize = 481;
const N_LON: usize = 301;

/// The two grids, in the order of NSGI's text files (south-west first,
/// longitude running fastest), in degrees and metres.
pub struct Grids {
    corr_lat: Vec<f64>,
    corr_lon: Vec<f64>,
    geoid: Vec<f64>,
}

impl Grids {
    /// Parse NSGI's `rdcorr2018.txt` and `nlgeo2018.txt`.
    pub fn parse(rdcorr: &str, nlgeo: &str) -> Result<Grids, String> {
        let rows = |text: &str, cols: usize| -> Result<Vec<Vec<f64>>, String> {
            let v: Vec<Vec<f64>> = text
                .lines()
                .skip(1)
                .filter(|l| !l.trim().is_empty())
                .map(|l| l.split_whitespace().map(|t| t.parse::<f64>().map_err(|e| format!("grid: {e}"))).collect::<Result<Vec<f64>, String>>())
                .collect::<Result<_, _>>()?;
            if v.len() != N_LAT * N_LON || v.iter().any(|r| r.len() < cols) {
                return Err(format!("grid: {} rows, {} expected", v.len(), N_LAT * N_LON));
            }
            Ok(v)
        };
        let c = rows(rdcorr, 4)?;
        let g = rows(nlgeo, 3)?;
        Ok(Grids { corr_lat: c.iter().map(|r| r[2]).collect(), corr_lon: c.iter().map(|r| r[3]).collect(), geoid: g.iter().map(|r| r[2]).collect() })
    }

    /// Bilinear interpolation (§5.3, Formula 2.3.1); `None` outside the grid.
    fn interpolate(values: &[f64], lat: f64, lon: f64) -> Option<f64> {
        if !(LAT_MIN..=LAT_MAX).contains(&lat) || !(LON_MIN..=LON_MAX).contains(&lon) {
            return None;
        }
        let fl = (lat - LAT_MIN) / D_LAT;
        let fo = (lon - LON_MIN) / D_LON;
        let (lf, lc, of, oc) = (fl.floor() as usize, fl.ceil() as usize, fo.floor() as usize, fo.ceil() as usize);
        let g = |i: usize| values.get(i).copied();
        let nw = g(lc * N_LON + of)?;
        let ne = g(lc * N_LON + oc)?;
        let sw = g(lf * N_LON + of)?;
        let se = g(lf * N_LON + oc)?;
        let (tl, to) = (fl - fl.floor(), fo - fo.floor());
        Some(tl * (nw * (1.0 - to) + ne * to) + (1.0 - tl) * (sw * (1.0 - to) + se * to))
    }

    fn correction(&self, lat_deg: f64, lon_deg: f64) -> (f64, f64) {
        (Self::interpolate(&self.corr_lat, lat_deg, lon_deg).unwrap_or(0.0), Self::interpolate(&self.corr_lon, lat_deg, lon_deg).unwrap_or(0.0))
    }

    /// Quasi-geoid height N (m) at an ETRS89 position; `None` outside the grid.
    pub fn geoid(&self, lat_deg: f64, lon_deg: f64) -> Option<f64> {
        Self::interpolate(&self.geoid, lat_deg, lon_deg)
    }
}

/// RD correction in degrees, or 0 without grids or outside them.
fn correction(grids: Option<&Grids>, lat: f64, lon: f64) -> (f64, f64) {
    grids.map(|g| g.correction(lat, lon)).unwrap_or((0.0, 0.0))
}

// ── Projection (§2.4, §3.1) ─────────────────────────────────────────────────

struct Proj {
    e: f64,
    n: f64,
    m: f64,
    r: f64,
    big_phi0: f64,
}

fn proj() -> &'static Proj {
    static P: OnceLock<Proj> = OnceLock::new();
    P.get_or_init(|| {
        let e2 = e2(BESSEL_F);
        let e = e2.sqrt();
        let n = (1.0 + e2 * PHI0.cos().powi(4) / (1.0 - e2)).sqrt();
        let rn = BESSEL_A / (1.0 - e2 * PHI0.sin().powi(2)).sqrt();
        let rm = rn * (1.0 - e2) / (1.0 - e2 * PHI0.sin().powi(2));
        let r = (rm * rn).sqrt();
        let q0 = ((PHI0 + 90.0 * DEG) / 2.0).tan().ln() - e / 2.0 * ((1.0 + e * PHI0.sin()) / (1.0 - e * PHI0.sin())).ln();
        let big_phi0 = (rm.sqrt() / rn.sqrt() * PHI0.tan()).atan();
        let w0 = ((big_phi0 + 90.0 * DEG) / 2.0).tan().ln();
        Proj { e, n, m: w0 - n * q0, r, big_phi0 }
    })
}

fn project(phi: f64, lam: f64) -> [f64; 2] {
    let p = proj();
    let q = ((phi + 90.0 * DEG) / 2.0).tan().ln() - p.e / 2.0 * ((1.0 + p.e * phi.sin()) / (1.0 - p.e * phi.sin())).ln();
    let w = p.n * q + p.m;
    let big_phi = 2.0 * w.exp().atan() - 90.0 * DEG;
    let big_lam = LAM0 + p.n * (lam - LAM0);
    // Round the sphere coordinates to 1e-9° (2e-11 rad) so nothing divides by ~0.
    let r11 = |v: f64| (v / 2e-11).round() * 2e-11;
    let (big_phi, big_lam, phi0) = (r11(big_phi), r11(big_lam), r11(p.big_phi0));
    if (big_phi - phi0).abs() < 1e-15 && (big_lam - LAM0).abs() < 1e-15 {
        return [X0, Y0];
    }
    let s2 = (((big_phi - phi0) / 2.0).sin().powi(2) + ((big_lam - LAM0) / 2.0).sin().powi(2) * big_phi.cos() * phi0.cos()).sqrt();
    let c2 = (1.0 - s2 * s2).sqrt();
    let sin_a = (big_lam - LAM0).sin() * big_phi.cos() / (2.0 * s2 * c2);
    let cos_a = (big_phi.sin() - phi0.sin() + 2.0 * phi0.sin() * s2 * s2) / (2.0 * phi0.cos() * s2 * c2);
    let r = 2.0 * K * p.r * s2 / c2;
    [r * sin_a + X0, r * cos_a + Y0]
}

fn unproject(x: f64, y: f64) -> (f64, f64) {
    let p = proj();
    let (dx, dy) = (x - X0, y - Y0);
    let r = dx.hypot(dy);
    let (sin_a, cos_a) = if r > 0.0 { (dx / r, dy / r) } else { (0.0, 0.0) };
    let psi = 2.0 * (r / (2.0 * K * p.r)).atan();
    let phi0 = p.big_phi0;
    let (xb, yb, zb) = if r > 0.0 {
        (phi0.cos() * psi.cos() - cos_a * phi0.sin() * psi.sin(), sin_a * psi.sin(), cos_a * phi0.cos() * psi.sin() + phi0.sin() * psi.cos())
    } else {
        (phi0.cos(), 0.0, phi0.sin())
    };
    let big_phi = zb.clamp(-1.0, 1.0).asin();
    let big_lam = if xb > 0.0 {
        LAM0 + (yb / xb).atan()
    } else if xb < 0.0 {
        LAM0 + (yb / xb).atan() + if x >= X0 { PI } else { -PI }
    } else if x > X0 {
        LAM0 + PI / 2.0
    } else if x < X0 {
        LAM0 - PI / 2.0
    } else {
        LAM0
    };
    // Sphere → Bessel (§3.1.2), iterative.
    let w = ((big_phi + 90.0 * DEG) / 2.0).tan().ln();
    let q = (w - p.m) / p.n;
    let mut phi = 0.0_f64;
    for _ in 0..100 {
        let next = 2.0 * (q + p.e / 2.0 * ((1.0 + p.e * phi.sin()) / (1.0 - p.e * phi.sin())).ln()).exp().atan() - 90.0 * DEG;
        let done = (next - phi).abs() < EPS;
        phi = next;
        if done {
            break;
        }
    }
    let lam_n = (big_lam - LAM0) / p.n + LAM0;
    let lam = lam_n + 2.0 * PI * ((PI - lam_n) / (2.0 * PI)).floor();
    (phi, lam)
}

// ── The two directions ──────────────────────────────────────────────────────

/// ETRS89 latitude/longitude (degrees) → RD x, y (m).
pub fn etrs89_to_rd_with(grids: Option<&Grids>, lat: f64, lon: f64) -> [f64; 2] {
    let g = to_geocentric(lat * DEG, lon * DEG, H0_ETRS, GRS80_A, GRS80_F);
    let rd = helmert(g, &TO_RD);
    let (ps, ls) = from_geocentric(rd, BESSEL_A, BESSEL_F);
    // Pseudo-Bessel → real Bessel: iterate on the grid (§2.3.3).
    let (ps_d, ls_d) = (ps / DEG, ls / DEG);
    let (mut phi, mut lam) = (ps_d, ls_d);
    for _ in 0..20 {
        let (cp, cl) = correction(grids, phi, lam);
        let (np, nl) = (ps_d - cp, ls_d - cl);
        let done = (np - phi).abs() < 1e-9 && (nl - lam).abs() < 1e-9;
        phi = np;
        lam = nl;
        if done {
            break;
        }
    }
    project(phi * DEG, lam * DEG)
}

/// RD x, y (m) → ETRS89 latitude/longitude (degrees).
pub fn rd_to_etrs89_with(grids: Option<&Grids>, x: f64, y: f64) -> [f64; 2] {
    let r4 = |v: f64| (v * 1e4).round() / 1e4;
    let (phi, lam) = unproject(r4(x), r4(y));
    let (cp, cl) = correction(grids, phi / DEG, lam / DEG);
    let (ps, ls) = (phi + cp * DEG, lam + cl * DEG);
    let g = to_geocentric(ps, ls, H0_BESSEL, BESSEL_A, BESSEL_F);
    let e = helmert(g, &TO_ETRS);
    let (p, l) = from_geocentric(e, GRS80_A, GRS80_F);
    [p / DEG, l / DEG]
}

/// NAP height from an ETRS89 position and ellipsoidal height; `None` outside the geoid grid.
pub fn nap_with(grids: &Grids, lat: f64, lon: f64, h: f64) -> Option<f64> {
    grids.geoid(lat, lon).map(|n| h - n)
}

// ── Grids on disk ───────────────────────────────────────────────────────────

/// NSGI's official package (RDNAPTRANS2018_v220627.zip, about 7 MB).
pub const PACKAGE_URL: &str = "https://www.nsgi.nl/documents/d/nsgi/rdnaptrans2018_v220627-zip";
pub const ATTRIBUTION: &str = "RD correction grid and quasi-geoid: © NSGI. CC BY 4.0 according to the RDNAPTRANS2018 documentation (appendix 2 and 4); the package's license.txt names CC BY-ND 4.0. The files are kept unchanged.";

fn grid_dir() -> PathBuf {
    super::servers::config_dir().join("rdnap2018")
}

fn loaded() -> &'static Mutex<Option<&'static Grids>> {
    static G: OnceLock<Mutex<Option<&'static Grids>>> = OnceLock::new();
    G.get_or_init(|| {
        let dir = grid_dir();
        let g = match (std::fs::read_to_string(dir.join("rdcorr2018.txt")), std::fs::read_to_string(dir.join("nlgeo2018.txt"))) {
            (Ok(c), Ok(n)) => Grids::parse(&c, &n).ok().map(|g| &*Box::leak(Box::new(g))),
            _ => None,
        };
        Mutex::new(g)
    })
}

/// The grids, when they are on disk.
pub fn grids() -> Option<&'static Grids> {
    *loaded().lock().unwrap()
}

/// Download NSGI's package once and keep the two grid files of variant 1.
pub fn ensure_grids(progress: &dyn Fn(String)) -> Result<&'static Grids, String> {
    if let Some(g) = grids() {
        return Ok(g);
    }
    progress("NSGI RD-correctiegrid en quasi-geoïde downloaden (7 MB)…".into());
    let zip_bytes = super::http::get(PACKAGE_URL)?.body;
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(zip_bytes)).map_err(|e| format!("NSGI package: {e}"))?;
    let mut take = |suffix: &str| -> Result<String, String> {
        let name = (0..zip.len()).filter_map(|i| zip.by_index(i).ok().map(|f| f.name().to_string())).find(|n| n.ends_with(suffix) && n.to_ascii_lowercase().contains("variant 1")).ok_or_else(|| format!("NSGI package: no {suffix}"))?;
        let mut s = String::new();
        std::io::Read::read_to_string(&mut zip.by_name(&name).map_err(|e| e.to_string())?, &mut s).map_err(|e| e.to_string())?;
        Ok(s)
    };
    let (rdcorr, nlgeo) = (take("rdcorr2018.txt")?, take("nlgeo2018.txt")?);
    let g = Grids::parse(&rdcorr, &nlgeo)?;
    let dir = grid_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("rdcorr2018.txt"), rdcorr).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("nlgeo2018.txt"), nlgeo).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("NOTICE.txt"), format!("{ATTRIBUTION}\nSource: {PACKAGE_URL}\nUnchanged grid files of implementation variant 1.\n")).map_err(|e| e.to_string())?;
    let leaked: &'static Grids = Box::leak(Box::new(g));
    *loaded().lock().unwrap() = Some(leaked);
    Ok(leaked)
}

/// Start the download in the background once per session when it is
/// missing (called when a drawing works in RD).
pub fn prefetch() {
    static STARTED: OnceLock<()> = OnceLock::new();
    if cfg!(test) || grids().is_some() || STARTED.set(()).is_err() {
        return;
    }
    std::thread::spawn(|| {
        let _ = ensure_grids(&|_| {});
    });
}

pub fn etrs89_to_rd(lat: f64, lon: f64) -> [f64; 2] {
    etrs89_to_rd_with(grids(), lat, lon)
}

pub fn rd_to_etrs89(x: f64, y: f64) -> [f64; 2] {
    rd_to_etrs89_with(grids(), x, y)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The first rows of NSGI's self-validation file Z001 (values verbatim).
    const Z001: &[(f64, f64, f64, f64, f64, f64)] = &[
        (51.728601274, 4.712120126, 301.7981, 108360.8790, 415757.2745, 258.0057),
        (52.115330444, 7.684748554, 41.4160, 312352.6004, 461058.5812, -2.5206),
        (53.001308983, 3.360296728, 72.6882, 18940.4448, 559061.2153, 29.9951),
        (51.682439113, 2.739626538, 269.9401, -28072.8306, 413735.0451, 225.6753),
        (53.558631914, 6.829370962, 39.9510, 250569.5957, 620134.9245, -0.2625),
        (51.589888118, 3.160820284, -11.4426, 729.7127, 402466.5599, -55.6883),
        (53.957638355, 5.089553648, 301.2718, 135458.0914, 663630.6489, 260.6568),
        (52.175136373, 2.618267658, 77.9387, -34373.0132, 468835.5131, 33.8482),
        (51.593897609, 3.927663147, 106.9715, 53868.5138, 401568.0165, 62.7610),
    ];

    fn test_grids() -> Option<Grids> {
        let dir = std::env::var_os("NSGI_GRIDS").map(PathBuf::from)?;
        let c = std::fs::read_to_string(dir.join("rdcorr2018.txt")).ok()?;
        let n = std::fs::read_to_string(dir.join("nlgeo2018.txt")).ok()?;
        Grids::parse(&c, &n).ok()
    }

    #[test]
    fn projection_origin_and_round_trip_without_grids() {
        let p = project(PHI0, LAM0);
        // The prescribed rounding of the sphere coordinates leaves microns.
        assert!((p[0] - X0).abs() < 1e-4 && (p[1] - Y0).abs() < 1e-4, "{p:?}");
        // Without grids the correction is 0: the seven-parameter result,
        // within a few decimetres of the official one, and invertible.
        for &(lat, lon, _, x, y, _) in Z001 {
            let rd = etrs89_to_rd_with(None, lat, lon);
            assert!((rd[0] - x).abs() < 0.5 && (rd[1] - y).abs() < 0.5, "{lat} {lon}: {rd:?} vs {x} {y}");
            let back = rd_to_etrs89_with(None, rd[0], rd[1]);
            assert!((back[0] - lat).abs() < 2e-8 && (back[1] - lon).abs() < 2e-8, "{back:?}");
        }
    }

    /// With NSGI's grids (`NSGI_GRIDS=<folder with rdcorr2018.txt and
    /// nlgeo2018.txt>`): the validation tolerances of the self-validation set,
    /// 0.0010 m for x, y and NAP and 1e-8° for latitude and longitude.
    #[test]
    #[ignore]
    fn official_points_with_grids() {
        let Some(g) = test_grids() else { panic!("set NSGI_GRIDS") };
        for &(lat, lon, h, x, y, nap) in Z001 {
            let rd = etrs89_to_rd_with(Some(&g), lat, lon);
            assert!((rd[0] - x).abs() < 0.001 && (rd[1] - y).abs() < 0.001, "{lat} {lon}: {rd:?} vs {x} {y}");
            let hh = nap_with(&g, lat, lon, h).unwrap();
            assert!((hh - nap).abs() < 0.001, "NAP {hh} vs {nap}");
            let back = rd_to_etrs89_with(Some(&g), x, y);
            assert!((back[0] - lat).abs() < 1e-8 && (back[1] - lon).abs() < 1e-8, "{back:?} vs {lat} {lon}");
        }
    }

    /// The whole self-validation file (`NSGI_Z001=<path>`), inside the grid.
    #[test]
    #[ignore]
    fn whole_self_validation_set() {
        let Some(g) = test_grids() else { panic!("set NSGI_GRIDS") };
        let path = std::env::var("NSGI_Z001").expect("set NSGI_Z001");
        let text = std::fs::read_to_string(path).unwrap();
        let (mut n, mut fwd_bad, mut inv_bad, mut nap_bad) = (0, 0, 0, 0);
        let (mut outside, mut out_fwd, mut out_inv) = (0, 0, 0);
        for line in text.lines().skip(1) {
            // "*" marks a height outside the quasi-geoid.
            let v: Vec<f64> = line.split_whitespace().skip(1).map(|t| t.parse().unwrap_or(f64::NAN)).collect();
            if v.len() < 6 {
                continue;
            }
            let (lat, lon, h, x, y, nap) = (v[0], v[1], v[2], v[3], v[4], v[5]);
            // Netherlands + EEZ: the grid area. Outside it only reported.
            if !(LAT_MIN..=LAT_MAX).contains(&lat) || !(LON_MIN..=LON_MAX).contains(&lon) {
                let rd = etrs89_to_rd_with(Some(&g), lat, lon);
                let back = rd_to_etrs89_with(Some(&g), x, y);
                outside += 1;
                if (rd[0] - x).abs() >= 0.001 || (rd[1] - y).abs() >= 0.001 {
                    out_fwd += 1;
                }
                if (back[0] - lat).abs() >= 1e-8 || (back[1] - lon).abs() >= 1e-8 {
                    out_inv += 1;
                }
                continue;
            }
            n += 1;
            let rd = etrs89_to_rd_with(Some(&g), lat, lon);
            if (rd[0] - x).abs() >= 0.001 || (rd[1] - y).abs() >= 0.001 {
                fwd_bad += 1;
            }
            if let (Some(hh), true) = (nap_with(&g, lat, lon, h), nap.is_finite()) {
                if (hh - nap).abs() >= 0.001 {
                    nap_bad += 1;
                }
            }
            let back = rd_to_etrs89_with(Some(&g), x, y);
            if (back[0] - lat).abs() >= 1e-8 || (back[1] - lon).abs() >= 1e-8 {
                inv_bad += 1;
            }
        }
        eprintln!("{n} points in the grid area: {fwd_bad} forward, {inv_bad} inverse, {nap_bad} NAP outside tolerance");
        eprintln!("{outside} points outside the grid: {out_fwd} forward, {out_inv} inverse outside tolerance");
        assert!(n > 1000 && fwd_bad == 0 && inv_bad == 0 && nap_bad == 0);
    }
}
