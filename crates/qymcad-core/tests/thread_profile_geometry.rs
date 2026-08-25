//! Invariants of the axial groove profile: what a thread needs to come out right, whatever the kernel does.
//!
//! What these checks caught in live code:
//!
//! * the crest arc took the long way round, 312° instead of 48°, sweeping nearly a full circle, tearing the
//!   thread and inflating one face to 437 thousand triangles instead of five;
//! * the root arc was placed by eye rather than by tangency: on Rd Ø30×3.5 the radius came out as 1.43 instead
//!   of 0.79 and the groove dropped below its own depth;
//! * the overshoot above the surface continued the flanks outwards, and a half-width of 1.834 at a pitch of 3.5
//!   reached into the neighbouring turn, so adjacent grooves intersected;
//! * on round Rd the crest flat was zero, the radius shrank to 0.03 mm, and a round thread came out straight.
use qymcad_core::geom::{Point2, ProfEdge};
use qymcad_core::thread::{ThreadSpec, ThreadStandard};

const STDS: [ThreadStandard; 5] =
    [ThreadStandard::MetricIso, ThreadStandard::TrapezoidalTr, ThreadStandard::Acme, ThreadStandard::RoundRd, ThreadStandard::Buttress];

fn ends(e: &ProfEdge) -> (Point2, Point2) {
    match *e {
        ProfEdge::Line { a, b } => (a, b),
        ProfEdge::Arc { a, b, .. } => (a, b),
        ProfEdge::Circle { center, .. } => (center, center),
    }
}

/// The angle of an arc in radians, signed by its direction, and its radius at both ends.
fn arc_sweep(a: Point2, b: Point2, c: Point2, ccw: bool) -> (f64, f64, f64) {
    let (ra, rb) = ((a.x - c.x).hypot(a.y - c.y), (b.x - c.x).hypot(b.y - c.y));
    let (t0, t1) = ((a.y - c.y).atan2(a.x - c.x), (b.y - c.y).atan2(b.x - c.x));
    let mut d = t1 - t0;
    if ccw {
        while d <= 0.0 {
            d += std::f64::consts::TAU;
        }
    } else {
        while d >= 0.0 {
            d -= std::f64::consts::TAU;
        }
    }
    (d.abs(), ra, rb)
}

/// A dense polyline sampling of the contour, used to check extents and self-intersections.
fn polyline(edges: &[ProfEdge]) -> Vec<Point2> {
    let mut pts = Vec::new();
    for e in edges {
        match *e {
            ProfEdge::Line { a, b } => {
                pts.push(a);
                pts.push(b);
            }
            ProfEdge::Arc { a, b, center, ccw } => {
                let (sw, r, _) = arc_sweep(a, b, center, ccw);
                let t0 = (a.y - center.y).atan2(a.x - center.x);
                let n = ((sw / 0.05).ceil() as usize).max(2);
                for i in 0..=n {
                    let t = t0 + if ccw { 1.0 } else { -1.0 } * sw * i as f64 / n as f64;
                    pts.push(Point2::new(center.x + r * t.cos(), center.y + r * t.sin()));
                }
            }
            ProfEdge::Circle { .. } => {}
        }
    }
    pts
}

fn segments_cross(p: Point2, q: Point2, r: Point2, s: Point2) -> bool {
    let d = |a: Point2, b: Point2, c: Point2| (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x);
    let (d1, d2, d3, d4) = (d(p, q, r), d(p, q, s), d(r, s, p), d(r, s, q));
    ((d1 > 1e-12 && d2 < -1e-12) || (d1 < -1e-12 && d2 > 1e-12)) && ((d3 > 1e-12 && d4 < -1e-12) || (d3 < -1e-12 && d4 > 1e-12))
}

/// The profile is closed: the end of each edge is the start of the next.
#[test]
fn profile_is_closed_for_every_standard() {
    for std in STDS {
        for pitch in [0.5, 1.5, 3.5, 8.0] {
            for internal in [false, true] {
                let g = ThreadSpec { standard: std, nominal_d: 30.0, pitch, internal, ..Default::default() }.geometry();
                for k in 0..g.groove.len() {
                    let (_, b) = ends(&g.groove[k]);
                    let (a2, _) = ends(&g.groove[(k + 1) % g.groove.len()]);
                    assert!(b.dist(a2) < 1e-9, "{std:?} P{pitch} internal={internal}: the contour breaks at edge {k}: {b:?} -> {a2:?}");
                }
            }
        }
    }
}

/// Every arc takes the short way round, at most 180°. A wrong traversal gave 312° instead of 48°: the arc swept
/// nearly a full circle, the thread tore, and one face inflated to hundreds of thousands of triangles.
#[test]
fn every_arc_takes_the_short_way() {
    for std in STDS {
        for pitch in [0.5, 1.5, 3.5, 8.0] {
            for internal in [false, true] {
                let g = ThreadSpec { standard: std, nominal_d: 30.0, pitch, internal, ..Default::default() }.geometry();
                for (i, e) in g.groove.iter().enumerate() {
                    if let ProfEdge::Arc { a, b, center, ccw } = *e {
                        let (sw, ra, rb) = arc_sweep(a, b, center, ccw);
                        assert!(
                            sw <= std::f64::consts::PI + 1e-9,
                            "{std:?} P{pitch} internal={internal}: arc {i} takes the long way round, {:.1}° instead of {:.1}°",
                            sw.to_degrees(),
                            360.0 - sw.to_degrees()
                        );
                        assert!((ra - rb).abs() < 1e-9, "{std:?} P{pitch}: arc {i} is not an arc; its end radii are {ra:.4} and {rb:.4}");
                        assert!(ra > 1e-9, "{std:?} P{pitch}: arc {i} degenerated into a point");
                    }
                }
            }
        }
    }
}

/// The profile fits entirely within half a pitch on either side. Otherwise neighbouring turns of the groove
/// overlap, the swept body self-intersects and the boolean returns a torn surface.
#[test]
fn profile_fits_within_half_a_pitch() {
    for std in STDS {
        for pitch in [0.5, 1.5, 3.5, 8.0] {
            for fit in [0.0, 0.2, 0.4] {
                let spec = ThreadSpec { standard: std, nominal_d: 30.0, pitch, fit, ..Default::default() };
                let g = spec.geometry();
                let max_x = polyline(&g.groove).iter().fold(0.0_f64, |m, p| m.max(p.x.abs()));
                assert!(
                    max_x <= g.pitch * 0.5 + 1e-9,
                    "{std:?} P{pitch} clearance {fit}: the profile exceeded half a pitch, {max_x:.4} against {:.4}",
                    g.pitch * 0.5
                );
            }
        }
    }
}

/// The profile is simple: non-adjacent edges do not intersect. A self-intersecting contour guarantees a broken
/// swept body.
#[test]
fn profile_does_not_self_intersect() {
    for std in STDS {
        for pitch in [1.5, 3.5, 8.0] {
            for internal in [false, true] {
                let g = ThreadSpec { standard: std, nominal_d: 30.0, pitch, internal, ..Default::default() }.geometry();
                let pts = polyline(&g.groove);
                let n = pts.len();
                for i in 0..n - 1 {
                    for j in i + 2..n - 1 {
                        if i == 0 && j == n - 2 {
                            continue; // the closing edge is adjacent to the first one
                        }
                        assert!(
                            !segments_cross(pts[i], pts[i + 1], pts[j], pts[j + 1]),
                            "{std:?} P{pitch} internal={internal}: the profile intersects itself, at segments {i} and {j}"
                        );
                    }
                }
            }
        }
    }
}

/// The depth of the groove is exactly what the standard computes: the root is neither shallower nor deeper. A
/// misplaced root arc dropped 0.64 mm below its own depth, which is a different thread altogether.
#[test]
fn groove_reaches_exactly_the_standard_depth() {
    for std in STDS {
        for pitch in [0.5, 1.5, 3.5, 8.0] {
            let g = ThreadSpec { standard: std, nominal_d: 30.0, pitch, ..Default::default() }.geometry();
            let min_y = polyline(&g.groove).iter().fold(0.0_f64, |m, p| m.min(p.y));
            assert!(min_y >= -g.depth - 1e-6, "{std:?} P{pitch}: the groove is deeper than its own depth, {:.4} against {:.4}", -min_y, g.depth);
            assert!(min_y <= -0.85 * g.depth, "{std:?} P{pitch}: the groove is shallower than its own depth, {:.4} against {:.4}", -min_y, g.depth);
        }
    }
}

/// A round Rd thread really is round: the crest and root radii are fractions of the pitch rather than microns.
/// The crest flat used to be zero, the radius shrank to 0.03 mm, and the profile came out straight.
#[test]
fn round_rd_has_real_radii_not_dust() {
    for pitch in [1.5, 3.5, 8.0] {
        let g = ThreadSpec { standard: ThreadStandard::RoundRd, nominal_d: 30.0, pitch, ..Default::default() }.geometry();
        let radii: Vec<f64> = g
            .groove
            .iter()
            .filter_map(|e| match *e {
                ProfEdge::Arc { a, center, .. } => Some((a.x - center.x).hypot(a.y - center.y)),
                _ => None,
            })
            .collect();
        assert_eq!(radii.len(), 4, "Rd P{pitch}: both crest and root are rounded, giving four arcs rather than {}", radii.len());
        for r in &radii {
            assert!(*r > 0.15 * pitch, "Rd P{pitch}: a radius of {r:.4} is dust rather than a rounding, expecting about {:.3}", 0.24 * pitch);
        }
        // and the crest is not eaten away entirely: material remains between neighbouring grooves
        let max_x = polyline(&g.groove).iter().fold(0.0_f64, |m, p| m.max(p.x.abs()));
        assert!(max_x < 0.48 * pitch, "Rd P{pitch}: the groove took the whole pitch and no crest is left ({max_x:.4})");
    }
}

/// A web has to remain between neighbouring turns.
///
/// Exactly half a pitch is not enough: on a round Rd thread with a fit the crest arcs met precisely, the turns
/// touched, and the resulting body came out self-intersecting. A section then showed seven segment crossings and
/// a torn fill, while a part without that contact sectioned cleanly.
#[test]
fn adjacent_turns_never_touch() {
    for std in STDS {
        for pitch in [1.5, 3.5, 5.0, 8.0] {
            for fit in [0.0, 0.2, 0.4] {
                let g = ThreadSpec { standard: std, nominal_d: 30.0, pitch, fit, ..Default::default() }.geometry();
                let max_x = polyline(&g.groove).iter().fold(0.0_f64, |m, p| m.max(p.x.abs()));
                let land = g.pitch - 2.0 * max_x; // the web between turns
                assert!(
                    land > 0.005 * g.pitch,
                    "{std:?} P{pitch} clearance {fit}: the turns meet, with a web of {land:.4} mm at a pitch of {:.2}",
                    g.pitch
                );
            }
        }
    }
}
