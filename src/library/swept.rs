//! Swept paths: a vehicle driven along a path, the way swept-path software
//! draws it. The front axle centre follows the path; each rear axle and
//! each trailer follows as a tractrix (the low-speed model behind AutoTURN
//! and Vehicle Tracking). The result is the traces of the body corners and
//! the outer wheel edges, plus the vehicle's position every so often.
//!
//! Coordinates are metres in the plane; the path runs from its first point
//! to its last. The vehicle starts aligned with the first segment.

use super::draw::unit_fronts;
use super::{Unit, Vehicle};

type P = [f64; 2];

fn sub(a: P, b: P) -> P {
    [a[0] - b[0], a[1] - b[1]]
}
fn add(a: P, b: P) -> P {
    [a[0] + b[0], a[1] + b[1]]
}
fn mul(a: P, k: f64) -> P {
    [a[0] * k, a[1] * k]
}
fn len(a: P) -> f64 {
    a[0].hypot(a[1])
}
fn unit(a: P) -> P {
    let l = len(a).max(1e-12);
    [a[0] / l, a[1] / l]
}
fn left(d: P) -> P {
    [-d[1], d[0]]
}

/// Where a unit's guiding axle sits (from its front), and the effective
/// rear axle (the middle of the non-steering axles: a tandem acts as one).
fn axles(u: &Unit) -> Option<(f64, f64)> {
    let steer: Vec<f64> = u.axles.iter().filter(|a| a.steer).map(|a| a.x).collect();
    let fixed: Vec<f64> = u.axles.iter().filter(|a| !a.steer).map(|a| a.x).collect();
    let rear = if fixed.is_empty() { u.axles.iter().map(|a| a.x).fold(f64::MIN, f64::max) } else { fixed.iter().sum::<f64>() / fixed.len() as f64 };
    let front = steer.first().copied().or_else(|| u.axles.iter().map(|a| a.x).reduce(f64::min))?;
    (rear > front).then_some((front, rear))
}

/// One unit's pose: its front (bumper centre) and its forward direction.
#[derive(Clone, Copy, Debug)]
pub struct Pose {
    pub front: P,
    pub dir: P,
}

impl Pose {
    /// A point `back` metres behind the front and `side` metres to the left.
    pub fn at(&self, back: f64, side: f64) -> P {
        add(sub(self.front, mul(self.dir, back)), mul(left(self.dir), side))
    }
    pub fn angle(&self) -> f64 {
        self.dir[1].atan2(self.dir[0])
    }
}

#[derive(Clone, Debug, Default)]
pub struct Swept {
    /// Traces: body corners (front left/right, rear left/right per unit)
    /// and outer wheel edges, each a polyline.
    pub body: Vec<Vec<P>>,
    pub wheels: Vec<Vec<P>>,
    /// The vehicle's units every `ghost_every` metres: per position, a pose per unit.
    pub ghosts: Vec<Vec<Pose>>,
    /// Largest steering angle the path asks for (degrees).
    pub max_steer_deg: f64,
    /// Smallest radius of the outer front corner's trace (m), if the path turns.
    pub min_outer_radius: Option<f64>,
    pub length: f64,
}

/// Resample a polyline every `step` metres.
fn resample(path: &[P], step: f64) -> Vec<P> {
    let mut out = vec![path[0]];
    let mut carry = 0.0;
    for w in path.windows(2) {
        let (a, b) = (w[0], w[1]);
        let l = len(sub(b, a));
        if l < 1e-9 {
            continue;
        }
        let d = unit(sub(b, a));
        let mut s = step - carry;
        while s <= l + 1e-9 {
            out.push(add(a, mul(d, s)));
            s += step;
        }
        carry = l - (s - step);
    }
    if let (Some(last), Some(end)) = (out.last().copied(), path.last().copied()) {
        if len(sub(end, last)) > 1e-6 {
            out.push(end);
        }
    }
    out
}

/// Radius of the circle through three points (∞ when straight).
fn radius(a: P, b: P, c: P) -> f64 {
    let (ab, bc, ca) = (len(sub(b, a)), len(sub(c, b)), len(sub(a, c)));
    let cross = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
    if cross.abs() < 1e-9 {
        f64::INFINITY
    } else {
        ab * bc * ca / (2.0 * cross.abs())
    }
}

/// Drive `v` along `path` (metres). `step` is the simulation step, `ghost_every`
/// the distance between drawn vehicle positions (0: start and end only).
pub fn drive(v: &Vehicle, path: &[P], step: f64, ghost_every: f64) -> Result<Swept, String> {
    if path.len() < 2 {
        return Err("a swept path needs a path of at least two points".into());
    }
    let fronts = unit_fronts(v);
    let geo: Vec<(f64, f64)> = v.units.iter().map(|u| axles(u).ok_or_else(|| format!("{}: a unit needs two axles for a swept path", v.id))).collect::<Result<_, _>>()?;
    let step = step.max(0.02);
    let pts = resample(path, step);
    // Curvature over about a metre either side: the path's vertices would
    // otherwise read as sharp kinks.
    let m = ((1.0 / step).round() as usize).max(1);
    let d0 = unit(sub(pts[1.min(pts.len() - 1)], pts[0]));
    // Initial poses: everything straight behind the start, along the first segment.
    let mut rear: Vec<P> = Vec::with_capacity(v.units.len());
    let mut poses: Vec<Pose> = Vec::with_capacity(v.units.len());
    let (f0, r0) = geo[0];
    let start_front = add(pts[0], mul(d0, f0));
    for (i, u) in v.units.iter().enumerate() {
        let front = sub(start_front, mul(d0, fronts[i]));
        let pose = Pose { front, dir: d0 };
        rear.push(pose.at(if i == 0 { r0 } else { geo[i].1 }, 0.0));
        poses.push(pose);
        let _ = u;
    }
    let corners = |p: &Pose, u: &Unit| [p.at(0.0, u.width / 2.0), p.at(0.0, -u.width / 2.0), p.at(u.length, u.width / 2.0), p.at(u.length, -u.width / 2.0)];
    let n_body = v.units.len() * 4;
    let mut body: Vec<Vec<P>> = vec![Vec::new(); n_body];
    let wheel_keys: Vec<(usize, f64, f64)> = v
        .units
        .iter()
        .enumerate()
        .flat_map(|(i, u)| {
            u.axles.iter().flat_map(move |a| {
                let half = if a.track > 0.05 { a.track / 2.0 } else { (u.width / 2.0 - 0.2).max(0.0) } + if a.dual { 0.33 } else { 0.12 };
                [(i, a.x, half), (i, a.x, -half)]
            })
        })
        .collect();
    let mut wheels: Vec<Vec<P>> = vec![Vec::new(); wheel_keys.len()];
    let mut ghosts = Vec::new();
    let mut max_steer: f64 = 0.0;
    let mut travelled = 0.0;
    let mut next_ghost = 0.0;
    let record = |poses: &[Pose], body: &mut Vec<Vec<P>>, wheels: &mut Vec<Vec<P>>| {
        for (i, (p, u)) in poses.iter().zip(&v.units).enumerate() {
            for (k, c) in corners(p, u).into_iter().enumerate() {
                body[i * 4 + k].push(c);
            }
        }
        for (w, (i, x, y)) in wheel_keys.iter().enumerate() {
            wheels[w].push(poses[*i].at(*x, *y));
        }
    };
    record(&poses, &mut body, &mut wheels);
    ghosts.push(poses.clone());
    for k in 1..pts.len() {
        // The steer axle centre moves along the path; each rear axle follows.
        let fa = pts[k];
        let (fx0, rx0) = geo[0];
        let wb0 = rx0 - fx0;
        let r_new = add(fa, mul(unit(sub(rear[0], fa)), wb0));
        rear[0] = r_new;
        let d = unit(sub(fa, r_new));
        poses[0] = Pose { front: add(fa, mul(d, fx0)), dir: d };
        for i in 1..v.units.len() {
            let prev = poses[i - 1];
            let hitch = prev.at(crate::library::draw::hitch_x(&v.units[i - 1]), 0.0);
            let king = crate::library::draw::kingpin_x(&v.units[i]);
            let (_, rx) = geo[i];
            let wb = rx - king;
            let r_new = add(hitch, mul(unit(sub(rear[i], hitch)), wb.abs().max(0.1)));
            rear[i] = r_new;
            let d = unit(sub(hitch, r_new));
            poses[i] = Pose { front: add(hitch, mul(d, king)), dir: d };
        }
        // Steering angle from the turning circle of the steer axle.
        if k >= m && k + m < pts.len() {
            let rad = radius(pts[k - m], pts[k], pts[k + m]);
            if rad.is_finite() {
                max_steer = max_steer.max((wb0 / rad).atan().to_degrees());
            }
        }
        travelled += len(sub(pts[k], pts[k - 1]));
        record(&poses, &mut body, &mut wheels);
        if ghost_every > 0.0 && travelled >= next_ghost + ghost_every {
            next_ghost += ghost_every;
            ghosts.push(poses.clone());
        }
    }
    if ghosts.len() == 1 || ghost_every <= 0.0 || travelled - next_ghost > 1e-6 {
        ghosts.push(poses.clone());
    }
    // Tightest radius of the outer front corner: the corner that swings wide.
    let outer = |t: &Vec<P>| -> Option<f64> {
        let r: Vec<f64> = t.windows(2 * m + 1).step_by(m).map(|w| radius(w[0], w[m], w[2 * m])).filter(|r| r.is_finite()).collect();
        r.into_iter().reduce(f64::min)
    };
    let min_outer_radius = [&body[0], &body[1]].into_iter().filter_map(outer).reduce(f64::max);
    Ok(Swept { body, wheels, ghosts, max_steer_deg: max_steer, min_outer_radius, length: travelled })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn straight_run_keeps_the_vehicle_on_the_line() {
        let v = crate::library::vehicles().iter().find(|v| v.units.len() > 1).unwrap();
        let s = drive(v, &[[0.0, 0.0], [60.0, 0.0]], 0.1, 20.0).unwrap();
        for trace in &s.body {
            for p in trace {
                assert!(p[1].abs() < v.width() / 2.0 + 1e-6, "{p:?}");
            }
        }
        assert!(s.max_steer_deg < 1e-6);
        assert!(s.ghosts.len() >= 3);
    }

    #[test]
    fn a_quarter_turn_cuts_the_corner() {
        let v = crate::library::vehicles().iter().find(|v| v.category == "car").unwrap();
        // 10 m straight, a quarter circle of 8 m, 10 m straight up.
        let mut path = vec![[0.0, 0.0], [10.0, 0.0]];
        for i in 1..=30 {
            let a = -std::f64::consts::FRAC_PI_2 + i as f64 / 30.0 * std::f64::consts::FRAC_PI_2;
            path.push([10.0 + 8.0 * a.cos(), 8.0 + 8.0 * a.sin()]);
        }
        path.push([18.0, 18.0]);
        let s = drive(v, &path, 0.05, 0.0).unwrap();
        // The rear axle cuts inside the front axle's circle (off-tracking).
        let inner_rear = s.wheels.iter().flat_map(|w| w.iter()).map(|p| (p[0] - 10.0).hypot(p[1] - 8.0)).fold(f64::MAX, f64::min);
        assert!(inner_rear < 8.0 - 0.5, "inner wheel radius {inner_rear}");
        assert!(s.max_steer_deg > 10.0 && s.max_steer_deg < 30.0, "{}", s.max_steer_deg);
    }
}
