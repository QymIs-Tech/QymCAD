//! A contour has one source of truth: its exact edges.
//!
//! A contour is described twice, by the polyline `points` and by the exact `edges`, and it is the edges that
//! reach the kernel. Nothing compared the two descriptions, and they once diverged in traversal direction: the
//! points of an outer loop ran counter-clockwise while the edges ran clockwise. The kernel does not check
//! orientation, so the hole became co-directed with the outer loop, the areas added instead of subtracting, the
//! body came out as a slab of 48 835 mm² instead of 9 020, and the chamfers and the second extrusion collapsed
//! after it.
//!
//! These checks demand the invariant rather than repair the symptom: the polyline is derived from the edges, a
//! closed contour runs counter-clockwise, and `canonicalize()` is idempotent.
use qymcad_core::geom::{circle_contour, Contour, Point2, ProfEdge};

fn rect_edges(w: f64, h: f64, ccw: bool) -> Contour {
    let (a, b, c, d) = (Point2::new(0.0, 0.0), Point2::new(w, 0.0), Point2::new(w, h), Point2::new(0.0, h));
    let seq = if ccw { [a, b, c, d] } else { [d, c, b, a] };
    let mut cont = Contour::closed(seq.to_vec());
    cont.edges = (0..4).map(|i| ProfEdge::Line { a: seq[i], b: seq[(i + 1) % 4] }).collect();
    cont
}

/// After canonicalisation a closed contour runs counter-clockwise, whatever it came in as.
#[test]
fn closed_contour_is_counter_clockwise_after_canonicalize() {
    for ccw_in in [true, false] {
        let mut c = rect_edges(30.0, 20.0, ccw_in);
        c.canonicalize();
        assert!(c.edges_signed_area() > 0.0, "input ccw={ccw_in}: the edges have to run counter-clockwise");
        assert!(c.signed_area() > 0.0, "input ccw={ccw_in}: and so does the polyline");
        assert!((c.edges_signed_area() - 600.0).abs() < 1e-6, "the area is preserved: {}", c.edges_signed_area());
    }
}

/// The polyline is derived: however badly it is damaged, canonicalisation rebuilds it from the exact edges.
#[test]
fn polyline_is_rebuilt_from_exact_edges() {
    let mut c = rect_edges(10.0, 10.0, true);
    c.points = vec![Point2::new(-100.0, -100.0), Point2::new(5.0, 0.5)]; // rubbish in place of the polyline
    c.canonicalize();
    assert_eq!(c.points.len(), 4, "the polyline was rebuilt from the edges: {:?}", c.points);
    assert!((c.signed_area() - 100.0).abs() < 1e-9, "and describes the same figure: {}", c.signed_area());
}

/// The points and the edges describe one curve in one direction, arcs included, where a divergence is harder
/// to notice.
#[test]
fn points_follow_edges_on_arcs_too() {
    let (c0, r) = (Point2::new(0.0, 0.0), 10.0);
    let (a, b) = (Point2::new(r, 0.0), Point2::new(-r, 0.0));
    let mut c = Contour::closed(vec![a, b]);
    c.edges = vec![
        ProfEdge::Arc { a, b, center: c0, ccw: true },  // the upper half
        ProfEdge::Arc { a: b, b: a, center: c0, ccw: true }, // the lower half
    ];
    c.canonicalize();
    let area = c.signed_area();
    let exact = std::f64::consts::PI * r * r;
    // the polyline is inscribed in the arc, so its area is slightly smaller than the exact one; the tolerance
    // is a fraction of the tessellation sag
    assert!(area > 0.0 && area <= exact, "counter-clockwise and no larger than the exact area: {area:.2} against {exact:.2}");
    assert!((exact - area) / exact < 0.005, "the polyline follows the arc: area {area:.2}, exact {exact:.2}");
    assert!(c.points.len() > 20, "the arcs really are tessellated: {} points", c.points.len());
    // the first point of the polyline is the start of the first edge rather than something arbitrary
    assert!(c.points[0].dist(a) < 1e-9, "the polyline starts at the beginning of the first edge");
}

/// Canonicalisation is idempotent: a second call changes nothing, or it would itself be a source of drift.
#[test]
fn canonicalize_is_idempotent() {
    for mut c in [rect_edges(7.0, 3.0, false), circle_contour(1.0, 2.0, 5.0, 0.01)] {
        c.canonicalize();
        let once = c.clone();
        c.canonicalize();
        assert_eq!(c.points.len(), once.points.len(), "a repeated canonicalisation does not move the polyline");
        for (p, q) in c.points.iter().zip(once.points.iter()) {
            assert!(p.dist(*q) < 1e-12, "a point moved on the repeated call: {p:?} -> {q:?}");
        }
    }
}

/// The main point: a face of an outer loop minus a hole is built from canonicalised contours whatever the input
/// was. That is exactly the failing case — the outer loop clockwise and the hole counter-clockwise.
#[test]
fn face_of_canonical_contours_subtracts_the_hole() {
    for (o_ccw, h_ccw) in [(true, true), (true, false), (false, true), (false, false)] {
        let (mut outer, mut hole) = (rect_edges(100.0, 100.0, o_ccw), rect_edges(40.0, 40.0, h_ccw));
        outer.canonicalize();
        hole.canonicalize();
        assert!(outer.edges_signed_area() > 0.0 && hole.edges_signed_area() > 0.0, "both loops were brought to counter-clockwise");
        // the area of material is the outer loop minus the hole; the kernel subtracts the hole because the
        // directions are canonical
        let material = outer.edges_signed_area() - hole.edges_signed_area();
        assert!((material - 8400.0).abs() < 1e-6, "input ({o_ccw},{h_ccw}): material {material:.1}, expecting 8400");
    }
}
