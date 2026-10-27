//! Line work for a vehicle in plan (top) and side view.
//!
//! Coordinates are metres in the block: X is the driving direction with the
//! front of the vehicle at X = 0 (the block's base point) and the rear at
//! negative X; in plan Y is to the left of the centre line, in side view Y is
//! up from the road surface. The drawings are schematic, like the vehicle
//! blocks of swept-path software: true outline, axles, wheels and coupling
//! points, with a few details (cab, windscreen, mirrors) to read the vehicle.

use super::{Axle, Unit, Vehicle};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    Top,
    Side,
}

impl View {
    pub fn suffix(self) -> &'static str {
        match self {
            View::Top => "TOP",
            View::Side => "SIDE",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    /// A polyline; `bulges[i]` curves the segment from point i to i + 1.
    Poly { pts: Vec<[f64; 2]>, bulges: Vec<f64>, closed: bool },
    Circle { c: [f64; 2], r: f64 },
}

/// Bulge of a quarter circle.
const QUARTER: f64 = 0.414_213_562_373_095;

fn poly(pts: Vec<[f64; 2]>, closed: bool) -> Shape {
    let bulges = vec![0.0; pts.len()];
    Shape::Poly { pts, bulges, closed }
}

fn line(a: [f64; 2], b: [f64; 2]) -> Shape {
    poly(vec![a, b], false)
}

fn rect(x0: f64, x1: f64, y0: f64, y1: f64) -> Shape {
    poly(vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]], true)
}

/// A rectangle with rounded corners: `rf` at the front (`x1`), `rr` at the rear.
fn rrect(x0: f64, x1: f64, y0: f64, y1: f64, rr: f64, rf: f64) -> Shape {
    let (rr, rf) = (rr.min((y1 - y0) / 2.0).min((x1 - x0) / 2.0).max(0.0), rf.min((y1 - y0) / 2.0).min((x1 - x0) / 2.0).max(0.0));
    let mut pts = Vec::new();
    let mut bulges = Vec::new();
    let mut corner = |a: [f64; 2], b: [f64; 2], r: f64| {
        if r > 1e-6 {
            pts.push(a);
            bulges.push(QUARTER);
            pts.push(b);
            bulges.push(0.0);
        } else {
            pts.push(a);
            bulges.push(0.0);
        }
    };
    corner([x1 - rf, y0], [x1, y0 + rf], rf);
    corner([x1, y1 - rf], [x1 - rf, y1], rf);
    corner([x0 + rr, y1], [x0, y1 - rr], rr);
    corner([x0, y0 + rr], [x0 + rr, y0], rr);
    Shape::Poly { pts, bulges, closed: true }
}

/// Where each unit's front sits behind the vehicle's front (m), with the
/// units coupled in a straight line.
pub fn unit_fronts(v: &Vehicle) -> Vec<f64> {
    let mut out: Vec<f64> = Vec::with_capacity(v.units.len());
    for (i, u) in v.units.iter().enumerate() {
        if i == 0 {
            out.push(0.0);
            continue;
        }
        let prev = &v.units[i - 1];
        let d = out[i - 1] + hitch_of(prev) - kingpin_of(u);
        out.push(d);
    }
    out
}

fn rear_group_centre(u: &Unit) -> Option<f64> {
    let rear: Vec<f64> = u.axles.iter().filter(|a| a.x > u.length * 0.45).map(|a| a.x).collect();
    if rear.is_empty() {
        None
    } else {
        Some(rear.iter().sum::<f64>() / rear.len() as f64)
    }
}

/// The coupling for the next unit, from this unit's front (m).
pub fn hitch_x(u: &Unit) -> f64 {
    hitch_of(u)
}

/// Where this unit hangs on the previous one, from its front (m).
pub fn kingpin_x(u: &Unit) -> f64 {
    kingpin_of(u)
}

fn hitch_of(u: &Unit) -> f64 {
    if let Some(h) = u.hitch_x {
        return h;
    }
    match u.kind.as_str() {
        // Fifth wheel a little ahead of the drive axle(s).
        "tractor" => rear_group_centre(u).map(|x| x - 0.3).unwrap_or(u.length * 0.75),
        // A joint just behind the rear section's front wall.
        "bus" | "rail_car" | "car" => u.length,
        // Drawbar hitch behind the rear.
        _ => u.length + 0.3,
    }
}

fn kingpin_of(u: &Unit) -> f64 {
    if let Some(k) = u.kingpin_x {
        return k;
    }
    match u.kind.as_str() {
        "semitrailer" => 1.2,
        "trailer" | "dolly" => -1.6,
        "rail_car" => -0.4,
        _ => 0.0,
    }
}

/// Tyre diameter and width (m).
fn tyre(v: &Vehicle, u: &Unit, axle: &Axle) -> (f64, f64) {
    let id = v.id.to_ascii_lowercase();
    let has = |k: &str| id.contains(k) || v.name_en.to_ascii_lowercase().contains(k);
    match (v.category.as_str(), u.kind.as_str()) {
        (_, "bike") | ("two_wheeler", _) => {
            if has("scooter") && !has("e_scooter") && !has("kick") {
                (0.40, 0.11)
            } else if has("motor") {
                (0.62, 0.15)
            } else if has("wheelchair") {
                (0.60, 0.04)
            } else if has("kick") || has("step") || has("e_scooter") {
                (0.22, 0.06)
            } else {
                (0.70, 0.05)
            }
        }
        (_, "rail_car") | ("rail", _) => (0.85, 0.13),
        ("agricultural", _) => {
            if has("combine") {
                if axle.steer { (1.20, 0.45) } else { (1.85, 0.75) }
            } else if u.kind == "trailer" {
                (1.10, 0.45)
            } else if axle.steer {
                (1.10, 0.40)
            } else {
                (1.75, 0.60)
            }
        }
        _ if u.width < 2.15 && u.length < 6.6 => {
            if v.category == "van" || u.length > 5.4 { (0.70, 0.22) } else { (0.66, 0.21) }
        }
        _ => (1.05, 0.31),
    }
}

fn is_car_like(v: &Vehicle, u: &Unit) -> bool {
    u.kind == "car" || (matches!(v.category.as_str(), "car" | "van" | "pickup" | "emergency" | "special") && u.width < 2.15 && u.length < 7.6 && !matches!(u.kind.as_str(), "trailer" | "semitrailer" | "tractor"))
}

fn has_word(v: &Vehicle, words: &[&str]) -> bool {
    let id = format!("{} {}", v.id, v.name_en).to_ascii_lowercase();
    words.iter().any(|w| id.contains(w))
}

/// Overall height, or a typical one for the category.
pub fn height(v: &Vehicle) -> f64 {
    if let Some(h) = v.height.filter(|h| *h > 0.2) {
        return h;
    }
    let u = &v.units[0];
    match v.category.as_str() {
        "car" => 1.50,
        "van" => if u.length > 5.5 { 2.60 } else { 2.00 },
        "pickup" => 1.85,
        "two_wheeler" => if has_word(v, &["motor"]) { 1.25 } else { 1.10 },
        "bus" | "articulated_bus" => 3.10,
        "coach" => 3.70,
        "rail" => 3.60,
        "emergency" if u.length < 7.0 => 2.30,
        "agricultural" => 2.95,
        "construction" => 3.20,
        _ => 3.90,
    }
}

// ── Plan ────────────────────────────────────────────────────────────────

fn wheels_top(v: &Vehicle, u: &Unit, front: f64, out: &mut Vec<Shape>) {
    for a in &u.axles {
        let (dia, tw) = tyre(v, u, a);
        let x = front - a.x;
        let single_track = u.kind == "bike" || v.category == "two_wheeler" && u.width < 1.0 && !has_word(v, &["wheelchair", "cargo", "trike"]);
        if single_track {
            out.push(rect(x - dia / 2.0, x + dia / 2.0, -tw / 2.0, tw / 2.0));
            continue;
        }
        let track = if a.track > 0.05 { a.track } else { (u.width - tw - 0.05).max(tw) };
        for side in [-1.0, 1.0] {
            let yc = side * track / 2.0;
            if a.dual {
                for off in [-1.0, 1.0] {
                    let y = yc + off * (tw / 2.0 + 0.02);
                    out.push(rect(x - dia / 2.0, x + dia / 2.0, y - tw / 2.0, y + tw / 2.0));
                }
            } else {
                out.push(rect(x - dia / 2.0, x + dia / 2.0, yc - tw / 2.0, yc + tw / 2.0));
            }
        }
    }
}

fn mirrors(x: f64, w: f64, reach: f64, out: &mut Vec<Shape>) {
    for side in [-1.0, 1.0] {
        let y0 = side * w / 2.0;
        let y1 = side * (w / 2.0 + reach);
        out.push(poly(vec![[x, y0], [x - 0.08, y1], [x - 0.22, y1], [x - 0.18, y0]], false));
    }
}

fn car_top(v: &Vehicle, u: &Unit, front: f64, out: &mut Vec<Shape>) {
    let (l, w) = (u.length, u.width);
    let x = |f: f64| front - f * l;
    let van = v.category == "van" || has_word(v, &["van", "ambulance", "camper", "minibus", "transporter", "sprinter"]) || height(v) > 2.1;
    let pickup = v.category == "pickup" || has_word(v, &["pickup", "pick-up", "ute"]);
    out.push(rrect(front - l, front, -w / 2.0, w / 2.0, (w * 0.16).min(0.32), (w * 0.2).min(0.38)));
    let inset = w * 0.1;
    let (hood, glass, rear) = if van { (0.10, 0.07, 0.03) } else if pickup { (0.24, 0.07, 0.0) } else { (0.27, 0.10, 0.10) };
    // Windscreen and roof.
    let ws0 = x(hood);
    let ws1 = x(hood + glass);
    out.push(poly(vec![[ws0, -w / 2.0 + inset * 0.8], [ws0, w / 2.0 - inset * 0.8], [ws1, w / 2.0 - inset], [ws1, -w / 2.0 + inset]], true));
    let roof_end = if pickup { x(0.62) } else { x(1.0 - rear - if van { 0.0 } else { 0.08 }) };
    out.push(rrect(roof_end, ws1, -w / 2.0 + inset, w / 2.0 - inset, 0.08, 0.08));
    if pickup {
        out.push(rect(x(0.97), x(0.64), -w / 2.0 + inset * 0.7, w / 2.0 - inset * 0.7));
    } else if !van {
        // Rear window.
        out.push(poly(vec![[roof_end, -w / 2.0 + inset], [roof_end, w / 2.0 - inset], [x(0.93), w / 2.0 - inset * 1.3], [x(0.93), -w / 2.0 + inset * 1.3]], true));
    }
    mirrors(ws0 - 0.05, w, 0.18, out);
    wheels_top(v, u, front, out);
}

fn cab(front: f64, cab_len: f64, w: f64, conventional: bool, out: &mut Vec<Shape>) -> f64 {
    let (hood, c0) = if conventional { (1.6_f64.min(cab_len * 0.55), front - 1.6_f64.min(cab_len * 0.55)) } else { (0.0, front) };
    if conventional {
        out.push(rrect(c0, front, -w * 0.36, w * 0.36, 0.1, 0.2));
    }
    let c1 = front - cab_len;
    out.push(rrect(c1, c0, -w / 2.0, w / 2.0, 0.05, if conventional { 0.05 } else { 0.18 }));
    out.push(line([c0 - 0.25, -w / 2.0 + 0.12], [c0 - 0.25, w / 2.0 - 0.12]));
    mirrors(c0 - 0.35, w, 0.28, out);
    let _ = hood;
    c1
}

fn truck_top(v: &Vehicle, u: &Unit, front: f64, idx: usize, out: &mut Vec<Shape>) {
    let (l, w) = (u.length, u.width);
    let conventional = u.kind == "conventional_cab" || (u.kind == "tractor" && v.region.eq_ignore_ascii_case("US")) || (u.kind == "tractor" && v.region.eq_ignore_ascii_case("AU") && l > 7.0);
    match u.kind.as_str() {
        "tractor" => {
            let cab_len = if conventional { 4.0_f64.min(l * 0.6) } else { 2.3_f64.min(l * 0.45) };
            let c1 = cab(front, cab_len, w, conventional, out);
            // Chassis rails and the fifth wheel.
            let rear = front - l;
            for s in [-1.0, 1.0] {
                out.push(line([c1, s * 0.45], [rear, s * 0.45]));
            }
            out.push(line([rear, -w / 2.0 + 0.2], [rear, w / 2.0 - 0.2]));
            let fw = front - hitch_of(u);
            out.push(Shape::Circle { c: [fw, 0.0], r: 0.45 });
            out.push(line([fw - 0.55, 0.0], [fw + 0.55, 0.0]));
        }
        "semitrailer" | "trailer" | "dolly" => {
            if u.kind == "dolly" {
                out.push(rect(front - l, front, -w * 0.42, w * 0.42));
                out.push(Shape::Circle { c: [front - l * 0.5, 0.0], r: 0.45 });
            } else {
                out.push(rect(front - l, front, -w / 2.0, w / 2.0));
            }
            let k = kingpin_of(u);
            if u.kind == "semitrailer" {
                out.push(Shape::Circle { c: [front - k, 0.0], r: 0.12 });
                // Landing legs.
                for s in [-1.0, 1.0] {
                    out.push(rect(front - k - 1.4, front - k - 1.2, s * (w / 2.0 - 0.35) - 0.1, s * (w / 2.0 - 0.35) + 0.1));
                }
            } else if k < 0.0 || u.kind == "dolly" {
                let eye = front - k;
                out.push(poly(vec![[front, -w * 0.3], [eye + 0.25, 0.0], [front, w * 0.3]], false));
                out.push(Shape::Circle { c: [eye, 0.0], r: 0.12 });
            }
        }
        _ => {
            // Rigid truck: cab and body.
            let cab_len = if conventional { 3.6_f64.min(l * 0.45) } else { 2.25_f64.min(l * 0.4) };
            let c1 = cab(front, cab_len, w, conventional, out);
            let body_w = w;
            if l - cab_len > 0.6 {
                out.push(rect(front - l, c1 - 0.1, -body_w / 2.0, body_w / 2.0));
                if has_word(v, &["mixer", "concrete"]) {
                    out.push(Shape::Circle { c: [(front - l + c1) / 2.0, 0.0], r: w * 0.42 });
                } else if has_word(v, &["crane"]) {
                    out.push(line([front + 0.3, 0.0], [front - l + 0.5, 0.0]));
                    out.push(rect(front - l + 0.3, front - l + 2.3, -0.5, 0.5));
                } else if has_word(v, &["refuse", "garbage", "müll", "rubbish", "waste"]) {
                    out.push(line([front - l + 1.2, -w / 2.0], [front - l + 1.2, w / 2.0]));
                }
            }
            if idx == 0 && hitch_of(u) > l {
                out.push(Shape::Circle { c: [front - hitch_of(u), 0.0], r: 0.1 });
            }
        }
    }
    wheels_top(v, u, front, out);
}

fn bus_top(v: &Vehicle, u: &Unit, front: f64, idx: usize, n: usize, out: &mut Vec<Shape>) {
    let (l, w) = (u.length, u.width);
    let joint_front = idx > 0;
    let joint_rear = idx + 1 < n;
    out.push(rrect(front - l, front, -w / 2.0, w / 2.0, if joint_rear { 0.0 } else { 0.25 }, if joint_front { 0.0 } else { 0.3 }));
    if !joint_front {
        out.push(line([front - 0.45, -w / 2.0 + 0.1], [front - 0.45, w / 2.0 - 0.1]));
        mirrors(front - 0.15, w, 0.3, out);
    }
    // Roof hatches.
    let mid = front - l * 0.5;
    out.push(rect(mid - 0.45, mid + 0.45, -0.4, 0.4));
    if joint_rear {
        // Bellows over the joint.
        let j = front - hitch_of(u);
        let bw = w * 0.92;
        out.push(rect(j - 0.6, j + 0.6, -bw / 2.0, bw / 2.0));
        for k in 1..4 {
            let xk = j - 0.6 + 0.3 * k as f64;
            out.push(line([xk, -bw / 2.0], [xk, bw / 2.0]));
        }
    }
    wheels_top(v, u, front, out);
}

fn bike_top(v: &Vehicle, u: &Unit, front: f64, out: &mut Vec<Shape>) {
    let (l, w) = (u.length, u.width);
    let motor = has_word(v, &["motor", "scooter"]) && !has_word(v, &["kick", "e_scooter", "e-scooter"]);
    let ax: Vec<f64> = u.axles.iter().map(|a| front - a.x).collect();
    let (xf, xr) = (ax.first().copied().unwrap_or(front - 0.35), ax.last().copied().unwrap_or(front - l + 0.35));
    if has_word(v, &["wheelchair"]) {
        out.push(rrect(front - l, front, -w / 2.0, w / 2.0, 0.1, 0.1));
        out.push(rect(front - l * 0.75, front - l * 0.3, -w * 0.3, w * 0.3));
    } else if has_word(v, &["cargo", "bakfiets"]) {
        out.push(rrect(front - l * 0.42, front - 0.05, -w / 2.0, w / 2.0, 0.08, 0.12));
        out.push(line([front - l * 0.42, 0.0], [xr, 0.0]));
        out.push(line([front - l * 0.5, -0.3], [front - l * 0.5, 0.3]));
    } else if motor {
        out.push(rrect(xr - 0.1, xf - 0.15, -w * 0.28, w * 0.28, 0.15, 0.12));
        out.push(line([xf - 0.25, -w / 2.0], [xf - 0.25, w / 2.0]));
    } else {
        out.push(line([xf, 0.0], [xr, 0.0]));
        out.push(line([xf - 0.12, -w / 2.0], [xf - 0.12, w / 2.0]));
        out.push(rect(xr + 0.22, xr + 0.5, -0.07, 0.07));
    }
    wheels_top(v, u, front, out);
}

fn rail_top(v: &Vehicle, u: &Unit, front: f64, idx: usize, out: &mut Vec<Shape>) {
    let (l, w) = (u.length, u.width);
    let c = 0.35;
    out.push(poly(vec![[front, -w / 2.0 + c], [front, w / 2.0 - c], [front - c, w / 2.0], [front - l + c, w / 2.0], [front - l, w / 2.0 - c], [front - l, -w / 2.0 + c], [front - l + c, -w / 2.0], [front - c, -w / 2.0]], true));
    if idx == 0 {
        out.push(line([front - 0.6, -w / 2.0 + 0.1], [front - 0.6, w / 2.0 - 0.1]));
    }
    // Bogies around axle pairs.
    let xs: Vec<f64> = u.axles.iter().map(|a| a.x).collect();
    let mut i = 0;
    while i < xs.len() {
        let (a, b) = if i + 1 < xs.len() && xs[i + 1] - xs[i] < 3.0 { (xs[i], xs[i + 1]) } else { (xs[i], xs[i]) };
        out.push(rect(front - b - 0.8, front - a + 0.8, -1.1, 1.1));
        i += if a == b { 1 } else { 2 };
    }
    wheels_top(v, u, front, out);
}

fn machine_top(v: &Vehicle, u: &Unit, front: f64, out: &mut Vec<Shape>) -> bool {
    let (l, w) = (u.length, u.width);
    if has_word(v, &["excavator", "graafmachine", "bagger"]) && u.axles.len() < 2 {
        // Tracks, superstructure, boom forward.
        let tl = (l * 0.5).min(4.6);
        let t0 = front - l;
        for s in [-1.0, 1.0] {
            out.push(rect(t0, t0 + tl, s * (w / 2.0 - 0.6), s * w / 2.0));
        }
        let hx = t0 + tl * 0.5;
        out.push(rrect(t0, hx + 1.4, -w * 0.45, w * 0.45, 0.4, 0.1));
        out.push(rect(hx + 1.4, front, -0.35, 0.35));
        out.push(rect(front - 0.7, front, -0.6, 0.6));
        return true;
    }
    if v.category == "agricultural" && u.kind != "trailer" && !has_word(v, &["combine"]) {
        out.push(rrect(front - l * 0.45, front, -w * 0.22, w * 0.22, 0.05, 0.15));
        out.push(rrect(front - l, front - l * 0.45, -w * 0.32, w * 0.32, 0.1, 0.05));
        wheels_top(v, u, front, out);
        return true;
    }
    if has_word(v, &["combine"]) {
        out.push(rect(front - 1.8, front, -w / 2.0, w / 2.0));
        out.push(rrect(front - l, front - 1.8, -1.6_f64.min(w / 2.0), 1.6_f64.min(w / 2.0), 0.2, 0.0));
        wheels_top(v, u, front, out);
        return true;
    }
    false
}

fn unit_top(v: &Vehicle, i: usize, front: f64, out: &mut Vec<Shape>) {
    let u = &v.units[i];
    let n = v.units.len();
    if machine_top(v, u, front, out) {
        return;
    }
    match u.kind.as_str() {
        "bike" => bike_top(v, u, front, out),
        "rail_car" => rail_top(v, u, front, i, out),
        "bus" | "bus_rear" => bus_top(v, u, front, i, n, out),
        _ if v.category == "two_wheeler" => bike_top(v, u, front, out),
        _ if matches!(v.category.as_str(), "bus" | "coach" | "articulated_bus") && !matches!(u.kind.as_str(), "trailer" | "semitrailer") => bus_top(v, u, front, i, n, out),
        _ if v.category == "rail" => rail_top(v, u, front, i, out),
        _ if is_car_like(v, u) => car_top(v, u, front, out),
        _ => truck_top(v, u, front, i, out),
    }
}

/// Plan view of the whole vehicle.
pub fn top(v: &Vehicle) -> Vec<Shape> {
    let mut out = Vec::new();
    for (i, d) in unit_fronts(v).into_iter().enumerate() {
        unit_top(v, i, -d, &mut out);
    }
    out
}

/// Plan view of one unit with its own front at X = 0 (for swept paths,
/// where the units turn apart).
pub fn top_unit(v: &Vehicle, i: usize) -> Vec<Shape> {
    let mut out = Vec::new();
    unit_top(v, i, 0.0, &mut out);
    out
}

// ── Side ────────────────────────────────────────────────────────────────

fn wheels_side(v: &Vehicle, u: &Unit, front: f64, out: &mut Vec<Shape>) {
    for a in &u.axles {
        let (dia, _) = tyre(v, u, a);
        let r = dia / 2.0;
        out.push(Shape::Circle { c: [front - a.x, r], r });
        out.push(Shape::Circle { c: [front - a.x, r], r: r * 0.55 });
    }
}

fn profile(front: f64, l: f64, pts: &[(f64, f64)], h: f64) -> Shape {
    poly(pts.iter().map(|(fx, fz)| [front - fx * l, fz * h]).collect(), true)
}

/// Side view of the whole vehicle (driving to the right, ground at 0).
pub fn side(v: &Vehicle) -> Vec<Shape> {
    let mut out = Vec::new();
    let fronts = unit_fronts(v);
    let h = height(v);
    let n = v.units.len();
    for (i, (u, d)) in v.units.iter().zip(&fronts).enumerate() {
        let front = -d;
        let l = u.length;
        let kind = u.kind.as_str();
        if kind == "bike" || v.category == "two_wheeler" {
            let ax: Vec<f64> = u.axles.iter().map(|a| front - a.x).collect();
            let (xf, xr) = (ax.first().copied().unwrap_or(front - 0.35), ax.last().copied().unwrap_or(front - l + 0.35));
            let r = u.axles.first().map(|a| tyre(v, u, a).0 / 2.0).unwrap_or(0.35);
            let seat = [xr + (xf - xr) * 0.3, r + 0.55];
            let bar = [xf + 0.05, r + 0.65];
            out.push(poly(vec![[xr, r], seat, [xf - (xf - xr) * 0.3, r + 0.05], [xr, r]], false));
            out.push(line([xf, r], bar));
            out.push(line([seat[0], r + 0.45], [bar[0] - 0.1, r + 0.5]));
            out.push(line([bar[0] - 0.1, bar[1]], [bar[0] + 0.1, bar[1]]));
            out.push(rect(seat[0] - 0.12, seat[0] + 0.12, seat[1], seat[1] + 0.05));
            if has_word(v, &["cargo", "bakfiets"]) {
                out.push(rect(front - l * 0.42, front - 0.05, r + 0.05, r + 0.6));
            }
            wheels_side(v, u, front, &mut out);
            continue;
        }
        if is_car_like(v, u) && !matches!(kind, "bus" | "bus_rear" | "rail_car") {
            let van = v.category == "van" || has_word(v, &["van", "ambulance", "camper", "minibus", "transporter", "sprinter"]) || h > 2.1;
            let pickup = v.category == "pickup" || has_word(v, &["pickup", "pick-up", "ute"]);
            let s: &[(f64, f64)] = if van {
                &[(0.0, 0.12), (0.0, 0.42), (0.04, 0.52), (0.13, 0.98), (0.16, 1.0), (1.0, 1.0), (1.0, 0.12)]
            } else if pickup {
                &[(0.0, 0.2), (0.0, 0.5), (0.04, 0.56), (0.24, 0.6), (0.34, 1.0), (0.6, 1.0), (0.62, 0.62), (1.0, 0.62), (1.0, 0.2)]
            } else {
                &[(0.0, 0.2), (0.0, 0.45), (0.05, 0.55), (0.27, 0.64), (0.42, 1.0), (0.72, 1.0), (0.88, 0.66), (1.0, 0.6), (1.0, 0.2)]
            };
            out.push(profile(front, l, s, h));
            wheels_side(v, u, front, &mut out);
            continue;
        }
        match kind {
            "bus" | "bus_rear" => {
                let joint_front = i > 0;
                out.push(rrect(front - l, front, 0.3, h, 0.0, if joint_front { 0.0 } else { 0.35 }));
                out.push(line([front - l + 0.2, h * 0.55], [front - 0.3, h * 0.55]));
                out.push(line([front - l + 0.2, h * 0.88], [front - 0.3, h * 0.88]));
                if !joint_front {
                    out.push(rect(front - 1.6, front - 0.6, 0.45, h * 0.85));
                }
            }
            "rail_car" => {
                out.push(rrect(front - l, front, 0.95, h, 0.25, 0.25));
                out.push(line([front - l + 0.4, h * 0.55], [front - 0.4, h * 0.55]));
                out.push(line([front - l + 0.4, h * 0.82], [front - 0.4, h * 0.82]));
            }
            "tractor" => {
                let conventional = v.region.eq_ignore_ascii_case("US") || (v.region.eq_ignore_ascii_case("AU") && l > 7.0);
                if conventional {
                    out.push(poly(vec![[front, 0.6], [front, 1.85], [front - 1.6, 2.05], [front - 1.75, h * 0.92], [front - 4.0, h * 0.92], [front - 4.0, 1.0], [front - 1.0, 0.6]], true));
                } else {
                    out.push(poly(vec![[front, 0.55], [front, 2.4], [front - 0.25, h * 0.98], [front - 2.3, h * 0.98], [front - 2.3, 0.55]], true));
                }
                out.push(line([front - 1.0, 1.05], [front - l, 1.05]));
                let fw = front - hitch_of(u);
                out.push(rect(fw - 0.6, fw + 0.6, 1.05, 1.25));
            }
            "semitrailer" => {
                out.push(rect(front - l, front, 1.25, h));
                let k = front - kingpin_of(u);
                out.push(line([k, 1.25], [k, 1.1]));
                out.push(line([k - 1.3, 1.25], [k - 1.3, 0.25]));
            }
            "trailer" | "dolly" => {
                out.push(rect(front - l, front, 1.05, if kind == "dolly" { 1.3 } else { h }));
                let eye = front - kingpin_of(u);
                out.push(line([front, 0.95], [eye, 0.8]));
            }
            _ => {
                if v.category == "agricultural" && !has_word(v, &["combine"]) {
                    out.push(poly(vec![[front, 0.7], [front, 1.4], [front - l * 0.45, 1.6], [front - l * 0.45, h], [front - l * 0.95, h], [front - l, 1.0], [front - l, 0.7]], true));
                } else if has_word(v, &["excavator", "graafmachine", "bagger"]) && u.axles.len() < 2 {
                    out.push(rrect(front - l, front - l * 0.5, 0.0, 0.9, 0.4, 0.4));
                    out.push(rect(front - l, front - l * 0.42, 0.9, h * 0.55));
                    out.push(rect(front - l * 0.62, front - l * 0.42, h * 0.55, h));
                    out.push(poly(vec![[front - l * 0.45, 1.6], [front - l * 0.2, h * 0.95], [front, 0.6]], false));
                } else {
                    let conventional = kind == "conventional_cab";
                    let cab_len = if conventional { 3.6_f64.min(l * 0.45) } else { 2.25_f64.min(l * 0.4) };
                    let cab_h = (h * 0.85).clamp(2.4, 3.1);
                    if conventional {
                        out.push(poly(vec![[front, 0.6], [front, 1.8], [front - 1.6, 2.0], [front - 1.8, cab_h], [front - cab_len, cab_h], [front - cab_len, 0.6]], true));
                    } else {
                        out.push(poly(vec![[front, 0.55], [front, cab_h * 0.8], [front - 0.25, cab_h], [front - cab_len, cab_h], [front - cab_len, 0.55]], true));
                    }
                    if l - cab_len > 0.6 {
                        let top = if has_word(v, &["mixer", "concrete", "tipper", "dump", "kipper"]) { h * 0.95 } else { h };
                        out.push(rect(front - l, front - cab_len - 0.1, 1.05, top));
                    }
                }
            }
        }
        let _ = n;
        wheels_side(v, u, front, &mut out);
    }
    out
}

pub fn shapes(v: &Vehicle, view: View) -> Vec<Shape> {
    match view {
        View::Top => top(v),
        View::Side => side(v),
    }
}

/// Bounding box `[x0, y0, x1, y1]` of shapes (bulges ignored).
pub fn bounds(shapes: &[Shape]) -> [f64; 4] {
    let mut b = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
    let mut add = |p: [f64; 2]| {
        b[0] = b[0].min(p[0]);
        b[1] = b[1].min(p[1]);
        b[2] = b[2].max(p[0]);
        b[3] = b[3].max(p[1]);
    };
    for s in shapes {
        match s {
            Shape::Poly { pts, .. } => pts.iter().for_each(|p| add(*p)),
            Shape::Circle { c, r } => {
                add([c[0] - r, c[1] - r]);
                add([c[0] + r, c[1] + r]);
            }
        }
    }
    if b[0] > b[2] {
        [0.0; 4]
    } else {
        b
    }
}

/// Points along a shape for previews and the window (arcs flattened).
pub fn flatten(s: &Shape) -> Vec<[f64; 2]> {
    match s {
        Shape::Circle { c, r } => (0..=24).map(|i| {
            let a = i as f64 / 24.0 * std::f64::consts::TAU;
            [c[0] + r * a.cos(), c[1] + r * a.sin()]
        }).collect(),
        Shape::Poly { pts, bulges, closed } => {
            let n = pts.len();
            let mut out = Vec::with_capacity(n * 2);
            let segs = if *closed { n } else { n.saturating_sub(1) };
            if n > 0 {
                out.push(pts[0]);
            }
            for i in 0..segs {
                let (a, b) = (pts[i], pts[(i + 1) % n]);
                let bulge = bulges.get(i).copied().unwrap_or(0.0);
                if bulge.abs() > 1e-9 {
                    // Arc through the bulge: sagitta-based centre.
                    let theta = 4.0 * bulge.atan();
                    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
                    let chord = dx.hypot(dy);
                    let r = chord / (2.0 * (theta / 2.0).sin());
                    let mid = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
                    let h = r * (theta / 2.0).cos();
                    let nrm = [-dy / chord, dx / chord];
                    let c = [mid[0] + nrm[0] * h, mid[1] + nrm[1] * h];
                    let a0 = (a[1] - c[1]).atan2(a[0] - c[0]);
                    for k in 1..=6 {
                        let t = a0 + theta * k as f64 / 6.0;
                        out.push([c[0] + r.abs() * t.cos(), c[1] + r.abs() * t.sin()]);
                    }
                } else {
                    out.push(b);
                }
            }
            out
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_vehicle_draws_inside_its_length() {
        for v in super::super::vehicles() {
            for view in [View::Top, View::Side] {
                let s = shapes(v, view);
                assert!(!s.is_empty(), "{} {:?}: nothing drawn", v.id, view);
                let b = bounds(&s);
                let len = v.length();
                assert!(b[2] <= 0.6 && b[0] >= -len - 2.5, "{} {:?}: x {:?} for length {len}", v.id, view, b);
            }
        }
    }

    #[test]
    fn rounded_rectangle_flattens_to_its_box() {
        let s = rrect(-4.0, 0.0, -1.0, 1.0, 0.3, 0.3);
        let pts = flatten(&s);
        for p in pts {
            assert!(p[0] >= -4.001 && p[0] <= 0.001 && p[1].abs() <= 1.001, "{p:?}");
        }
    }
}
