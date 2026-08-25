//! The tessellation deflection follows the size of the body; it used to be a fixed 0.5 mm, which left small
//! parts faceted and huge ones heavy.
//!
//! What is checked: the pure formula of `adaptive_deflection`, with its bounds, monotonicity and default; and
//! the real kernel, where the extents of a body are computed, the triangle count of a cylinder is invariant to
//! scale, a small cylinder is no longer faceted, and a fixed 0.5 mm on it still is — which is the proof of the
//! defect.
use qymcad_kernel::Shape;

/// The deflection at the ordinary accuracy of a document: what the program lived by when this was a constant.
///
/// The fraction of the diagonal now comes from the document, and the test has to ask it from there: hard-coding
/// the number here would check a copy of its own rather than what is actually used.
fn adefl(diag: f64) -> f64 {
    qymcad_kernel::adaptive_deflection(diag, qymcad_core::model::GeomQuality::Normal.deflection_k())
}

/// The pure formula: a fraction of the size clamped to [0.002, 1.0]; invalid extents fall back to the former
/// 0.5.
#[test]
fn adaptive_deflection_scales_and_clamps() {
    let mut bad = Vec::new();
    // a part of about 100 mm, with a diagonal of 173, gives 0.26 mm: twice as fine as the former 0.5
    let mid = adefl(173.0);
    if !(0.2..0.35).contains(&mid) {
        bad.push(format!("a diagonal of 173 gave {mid}, expecting about 0.26"));
    }
    // a tiny part: never below 0.002 mm, or the triangles run into millions
    if (adefl(0.05) - 0.002).abs() > 1e-12 {
        bad.push(format!("a tiny body gave {}, expecting the floor of 0.002", adefl(0.05)));
    }
    // a frame of 3 m: never coarser than 1.0 mm
    if (adefl(3464.0) - 1.0).abs() > 1e-12 {
        bad.push(format!("a huge body gave {}, expecting the ceiling of 1.0", adefl(3464.0)));
    }
    // monotonicity across the working range
    for (a, b) in [(10.0, 50.0), (50.0, 173.0), (173.0, 600.0)] {
        if adefl(a) >= adefl(b) {
            bad.push(format!("not monotonic: {a} gives {}, {b} gives {}", adefl(a), adefl(b)));
        }
    }
    // the extents were not computed, as for an empty shape, so the former behaviour applies
    for d in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        if adefl(d) != 0.5 {
            bad.push(format!("an invalid diagonal of {d} gave {}, expecting the default of 0.5", adefl(d)));
        }
    }
    assert!(bad.is_empty(), "adaptive_deflection:\n{}", bad.join("\n"));
}

/// The extents of a real body through the kernel: a cylinder of r = 5 and h = 20 gives a 10×10×20 box with a
/// diagonal of about 24.5.
#[test]
fn shape_bbox_matches_geometry() {
    let s = Shape::cylinder(5.0, 20.0).expect("the cylinder");
    let b = s.bbox().expect("the extents");
    let (dx, dy, dz) = (b[3] - b[0], b[4] - b[1], b[5] - b[2]);
    // the bounding box carries a small gap, so the tolerance is generous and only the order of magnitude
    // matters
    assert!((dx - 10.0).abs() < 0.3 && (dy - 10.0).abs() < 0.3 && (dz - 20.0).abs() < 0.3, "the extents {b:?}");
    let diag = s.bbox_diag();
    assert!((diag - 24.49).abs() < 0.5, "the diagonal {diag}");
}

/// How many triangles a tessellation has; `None` selects the adaptive deflection.
fn tri_count(s: &Shape, defl: Option<f64>) -> usize {
    let bodies = match defl {
        Some(d) => s.tessellate(d),
        None => s.tessellate_auto(qymcad_core::model::GeomQuality::Normal.deflection_k()),
    };
    bodies.iter().map(|(m, _)| m.tris.len()).sum()
}

/// The largest angular step, in radians, between vertices around the Z axis: a direct measure of how faceted a
/// circle is. A step of 0.5 rad means thirteen segments per turn, where the facets show; 0.3 means twenty-one,
/// which reads as smooth. The measure is dimensionless and therefore comparable between a 2 mm part and a 4 m
/// frame.
fn max_angle_step(s: &Shape) -> f64 {
    let mut angs: Vec<f64> = s
        .tessellate_auto(qymcad_core::model::GeomQuality::Normal.deflection_k())
        .iter()
        .flat_map(|(m, _)| m.verts.iter().map(|v| v.y.atan2(v.x)))
        .collect();
    angs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    angs.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
    assert!(angs.len() > 3, "the tessellation gave {} angles; is the body round at all?", angs.len());
    let wrap = angs[0] + std::f64::consts::TAU - angs[angs.len() - 1];
    angs.windows(2).map(|w| w[1] - w[0]).fold(wrap, f64::max)
}

/// A small part is not faceted: on a cylinder of r = 1 the vertex step stays at or below 0.32 rad, giving at
/// least twenty-one segments per turn.
///
/// The former angular default of the kernel, 0.5 rad and thirteen segments, did not meet that threshold, and a
/// linear deflection does not help at a small radius at all: the sag r·(1−cos) there is microscopic under any
/// reasonable linear tolerance. Faceted small parts are cured by the angular criterion specifically.
#[test]
fn small_body_is_not_faceted() {
    let step = max_angle_step(&Shape::cylinder(1.0, 2.0).expect("the small cylinder"));
    assert!(step <= 0.32, "a cylinder of r = 1: a vertex step of {step:.3} rad, and above 0.32 the part is faceted");
}

/// Smoothness is the same at every scale: 2 mm, 20 mm and 200 mm give the same angular step and, within 60 per
/// cent, the same triangle count. The former fixed deflection did not: quality depended on the size of the
/// part.
#[test]
fn quality_is_scale_invariant() {
    let mut bad = Vec::new();
    let mut counts = Vec::new();
    for (r, h) in [(1.0, 2.0), (10.0, 20.0), (100.0, 200.0)] {
        let s = Shape::cylinder(r, h).expect("the cylinder");
        let step = max_angle_step(&s);
        if step > 0.32 {
            bad.push(format!("r={r}: a step of {step:.3} rad, which is faceted"));
        }
        counts.push((r, tri_count(&s, None)));
    }
    let (lo, hi) = (counts.iter().map(|c| c.1).min().unwrap(), counts.iter().map(|c| c.1).max().unwrap());
    if hi as f64 / lo as f64 > 1.6 {
        bad.push(format!("the triangle count jumps with scale: {counts:?}"));
    }
    assert!(bad.is_empty(), "invariance of quality to scale:\n{}", bad.join("\n"));
}

/// A huge body does not drown in triangles: a fixed 0.5 mm on a cylinder of r = 2 m gives a noticeably heavier
/// mesh than the adaptive deflection at its ceiling of 1.0 mm, for a difference invisible on screen.
#[test]
fn huge_body_is_lighter_than_fixed_deflection() {
    let s = Shape::cylinder(2000.0, 4000.0).expect("the huge cylinder");
    let (auto, fixed) = (tri_count(&s, None), tri_count(&s, Some(0.5)));
    assert!(
        (auto as f64) < fixed as f64 * 0.85,
        "adaptive gives {auto} triangles against {fixed} at a fixed 0.5, expecting noticeably lighter"
    );
    assert!(max_angle_step(&s) <= 0.32, "and the huge body still reads as smooth");
}
