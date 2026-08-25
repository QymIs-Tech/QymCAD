//! Property tests for the two most numerical places: the solver and the decomposition into regions.
//!
//! Every defect found here so far — the barrier in |Δ|, angles near 0° and 180°, a drifting rank, tangencies,
//! collinear overlaps — was found by hand, from a complaint. A targeted case guards exactly what has already
//! broken; a property test checks an invariant over hundreds of random scenes and catches what nobody has
//! thought of yet.
//!
//! The generator is deterministic, with fixed seeds, so a failure always reproduces: the report prints the seed
//! that rebuilds the scene.

use qymcad_core::model::{Constraint, Project};

/// A minimal deterministic pseudo-random generator, xorshift64*, with no external dependencies.
struct Rng(u64);
impl Rng {
    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545F4914F6CDD1D)
    }
    /// Uniform over [lo, hi).
    fn f(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64 * (hi - lo)
    }
    fn usize(&mut self, n: usize) -> usize {
        (self.next_u64() % n.max(1) as u64) as usize
    }
}

/// A random sketch: segments, circles and rectangles within a square of side `span`.
fn random_sketch(rng: &mut Rng, span: f64, n: usize) -> (Project, usize) {
    let mut p = Project::default();
    let si = p.new_sketch("prop");
    for _ in 0..n {
        match rng.usize(3) {
            0 => {
                let (x, y) = (rng.f(-span, span), rng.f(-span, span));
                p.add_line_entity(si, x, y, x + rng.f(-span, span), y + rng.f(-span, span), qymcad_core::feature::Purpose::Real);
            }
            1 => {
                p.add_circle_entity(si, rng.f(-span, span), rng.f(-span, span), rng.f(span * 0.05, span * 0.6), qymcad_core::feature::Purpose::Real);
            }
            _ => {
                let (x, y) = (rng.f(-span, span), rng.f(-span, span));
                p.add_rect_entity(si, x, y, x + rng.f(span * 0.1, span), y + rng.f(span * 0.1, span), qymcad_core::feature::Purpose::Real);
            }
        }
    }
    p.regen_sketch(si);
    (p, si)
}

/// Invariant of the decomposition: on any scene the regions come out finite and meaningful — closed, with at
/// least three points, free of NaN, and with an area no larger than the scene. This is the place where
/// tangencies and collinear overlaps used to give either no regions at all or rubbish.
#[test]
fn arrangement_regions_are_always_sane() {
    let mut bad: Vec<String> = Vec::new();
    for seed in 1..=600u64 {
        let mut rng = Rng(seed.wrapping_mul(0x9E3779B97F4A7C15) | 1);
        let span = [0.05, 1.0, 50.0, 5000.0][(seed % 4) as usize]; // scales from tiny to metres
        let (p, si) = random_sketch(&mut rng, span, 2 + (seed % 5) as usize);
        let area_limit = (4.0 * span * span) * 1.5; // the area of a region cannot exceed the scene
        for cid in p.sketches[si].contour_ids.clone() {
            let Some(idx) = p.contour_index(cid) else { continue };
            let c = &p.contours[idx];
            if !c.points.iter().all(|q| q.x.is_finite() && q.y.is_finite()) {
                bad.push(format!("seed {seed}: NaN or infinity in the points of a region"));
                break;
            }
            if c.closed && c.points.len() < 3 {
                bad.push(format!("seed {seed}: a closed region of {} points", c.points.len()));
                break;
            }
            if c.area() > area_limit {
                bad.push(format!("seed {seed}: the area of a region, {:.3}, exceeds the scene, {:.3}", c.area(), area_limit));
                break;
            }
        }
    }
    assert!(bad.is_empty(), "the invariants of the decomposition are violated ({}):\n{}", bad.len(), bad.join("\n"));
}

/// Second invariant of the decomposition: it is deterministic, so the same sketch yields the same set of
/// regions. Non-determinism here would mean contour ids jumping between rebuilds, which is how an extrusion
/// ends up following someone else's contour.
#[test]
fn arrangement_is_deterministic() {
    let mut bad = Vec::new();
    for seed in 1..=200u64 {
        let mk = || {
            let mut rng = Rng(seed.wrapping_mul(0x9E3779B97F4A7C15) | 1);
            random_sketch(&mut rng, 20.0, 3)
        };
        let (p1, s1) = mk();
        let (p2, s2) = mk();
        let sig = |p: &Project, si: usize| -> Vec<(usize, u64)> {
            p.sketches[si]
                .contour_ids
                .iter()
                .filter_map(|c| p.contour_index(*c))
                .map(|i| (p.contours[i].points.len(), (p.contours[i].area() * 1e6) as u64))
                .collect()
        };
        if sig(&p1, s1) != sig(&p2, s2) {
            bad.push(format!("seed {seed}: two runs over one scene produced different regions"));
        }
    }
    assert!(bad.is_empty(), "the decomposition is not deterministic:\n{}", bad.join("\n"));
}

/// Invariant of the solver: a solve never corrupts the geometry — the coordinates stay finite and the points
/// do not fly orders of magnitude beyond the scene. That is what the past failures looked like: the solver did
/// not merely fail to solve, it drove the sketch into nonsense.
#[test]
fn solver_never_produces_wild_or_nan_geometry() {
    let mut bad: Vec<String> = Vec::new();
    for seed in 1..=600u64 {
        let mut rng = Rng(seed.wrapping_mul(0xD1B54A32D192ED03) | 1);
        let span = [0.05, 1.0, 50.0, 5000.0][(seed % 4) as usize];
        let (mut p, si) = random_sketch(&mut rng, span, 2 + (seed % 4) as usize);
        // a random set of constraints over random geometry
        let pts: Vec<u64> = p.sketches[si].points.iter().map(|q| q.id).collect();
        if pts.len() < 3 {
            continue;
        }
        for _ in 0..(1 + seed % 5) {
            let (a, b) = (pts[rng.usize(pts.len())], pts[rng.usize(pts.len())]);
            if a == b {
                continue;
            }
            let c = match rng.usize(5) {
                0 => Constraint::Horizontal { a, b },
                1 => Constraint::Vertical { a, b },
                2 => Constraint::Coincident { a, b },
                3 => Constraint::Distance { a, b, d: rng.f(span * 0.05, span), off: 0.0, expr: String::new(), driven: false, axis: rng.usize(3) as u8 },
                _ => Constraint::Fixed { p: a },
            };
            p.sketches[si].constraints.push(c);
        }
        let before = p.sketches[si].points.clone();
        p.solve_sketch(si);
        let limit = span * 1e4; // generous: the solver may move points, but not by four orders of magnitude
        for (q, was) in p.sketches[si].points.iter().zip(before.iter()) {
            if !q.x.is_finite() || !q.y.is_finite() {
                bad.push(format!("seed {seed}: NaN or infinity at point {}, which was at {:.3},{:.3}", q.id, was.x, was.y));
                break;
            }
            if q.x.abs() > limit || q.y.abs() > limit {
                bad.push(format!("seed {seed}: a point flew to ({:.1},{:.1}) at a scale of {span}", q.x, q.y));
                break;
            }
        }
    }
    assert!(bad.is_empty(), "the solver corrupts the geometry ({}):\n{}", bad.len(), bad.join("\n"));
}

/// Second invariant of the solver: a solve is stable, so calling it again on an already solved sketch changes
/// almost nothing. A sketch that drifts, where every solve moves the points, is what shows up on screen as
/// geometry trembling at every click.
#[test]
fn solving_twice_is_stable() {
    let mut bad: Vec<String> = Vec::new();
    for seed in 1..=400u64 {
        let mut rng = Rng(seed.wrapping_mul(0xA24BAED4963EE407) | 1);
        let span = 50.0;
        let (mut p, si) = random_sketch(&mut rng, span, 2 + (seed % 3) as usize);
        let pts: Vec<u64> = p.sketches[si].points.iter().map(|q| q.id).collect();
        if pts.len() < 3 {
            continue;
        }
        for _ in 0..(1 + seed % 4) {
            let (a, b) = (pts[rng.usize(pts.len())], pts[rng.usize(pts.len())]);
            if a != b {
                p.sketches[si].constraints.push(match rng.usize(3) {
                    0 => Constraint::Horizontal { a, b },
                    1 => Constraint::Vertical { a, b },
                    _ => Constraint::Distance { a, b, d: rng.f(5.0, span), off: 0.0, expr: String::new(), driven: false, axis: 0 },
                });
            }
        }
        p.solve_sketch(si);
        let first = p.sketches[si].points.clone();
        p.solve_sketch(si);
        let moved = p.sketches[si]
            .points
            .iter()
            .zip(first.iter())
            .map(|(a, b)| ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt())
            .fold(0.0_f64, f64::max);
        if moved > span * 1e-3 {
            bad.push(format!("seed {seed}: a repeated solve moved a point by {moved:.4}, so the sketch drifts"));
        }
    }
    assert!(bad.is_empty(), "the solve is unstable ({}):\n{}", bad.len(), bad.join("\n"));
}

/// Invariant of the degree-of-freedom count: it does not depend on the scale of the scene, the rank being
/// computed over a normalised Jacobian. The same sketch in microns and in metres has to give the same count.
#[test]
fn dof_is_scale_invariant_on_random_sketches() {
    let mut bad: Vec<String> = Vec::new();
    for seed in 1..=200u64 {
        let build = |k: f64| {
            let mut rng = Rng(seed.wrapping_mul(0x9E3779B97F4A7C15) | 1);
            let mut p = Project::default();
            let si = p.new_sketch("s");
            for _ in 0..3 {
                let (x, y) = (rng.f(-10.0, 10.0) * k, rng.f(-10.0, 10.0) * k);
                p.add_line_entity(si, x, y, x + rng.f(1.0, 10.0) * k, y + rng.f(1.0, 10.0) * k, qymcad_core::feature::Purpose::Real);
            }
            p.regen_sketch(si);
            let pts: Vec<u64> = p.sketches[si].points.iter().map(|q| q.id).collect();
            let mut r2 = Rng(seed | 1);
            for _ in 0..3 {
                let (a, b) = (pts[r2.usize(pts.len())], pts[r2.usize(pts.len())]);
                if a != b {
                    p.sketches[si].constraints.push(match r2.usize(3) {
                        0 => Constraint::Horizontal { a, b },
                        1 => Constraint::Vertical { a, b },
                        _ => Constraint::Distance { a, b, d: 5.0 * k, off: 0.0, expr: String::new(), driven: false, axis: 0 },
                    });
                }
            }
            p.sketch_dof(si)
        };
        let (small, normal, big) = (build(1e-3), build(1.0), build(1e3));
        if small != normal || big != normal {
            bad.push(format!("seed {seed}: the count depends on scale — ×0.001 {small:?}, ×1 {normal:?}, ×1000 {big:?}"));
        }
    }
    assert!(bad.is_empty(), "the degree-of-freedom count is not scale-invariant ({}):\n{}", bad.len(), bad.join("\n"));
}

/// Invariant of the sketch geometry: for a circle that reaches the decomposition, the area of its region
/// matches πr² within the tessellation. This catches skews such as a circle with a single cut disappearing.
#[test]
fn lone_circle_region_matches_its_radius() {
    let mut bad = Vec::new();
    for seed in 1..=200u64 {
        let mut rng = Rng(seed | 1);
        let r = rng.f(0.5, 100.0);
        let mut p = Project::default();
        let si = p.new_sketch("c");
        p.add_circle_entity(si, rng.f(-10.0, 10.0), rng.f(-10.0, 10.0), r, qymcad_core::feature::Purpose::Real);
        p.regen_sketch(si);
        let areas: Vec<f64> = p.sketches[si].contour_ids.iter().filter_map(|c| p.contour_index(*c)).map(|i| p.contours[i].area()).collect();
        let exp = std::f64::consts::PI * r * r;
        if !areas.iter().any(|a| ((a - exp) / exp).abs() < 0.02) {
            bad.push(format!("seed {seed}: a circle of r={r:.3} gave regions {areas:?}, expecting about {exp:.3}"));
        }
    }
    assert!(bad.is_empty(), "a lone circle is lost or distorted:\n{}", bad.join("\n"));
}
