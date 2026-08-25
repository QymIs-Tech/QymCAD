//! A tangency is a tangency, not an approximation.
//!
//! Three lines tangent to a circle form a closed contour together with it, yet it could not be extruded: only
//! the circles themselves were selectable.
//!
//! The cause: whether something touches was decided twice, and neither time in units of length — once by the
//! discriminant being zero, at a threshold of 1e-12 in its own scale, and once by the parameter along the
//! segment, at ±1e-9. In the file the solver had brought the tangency to within 2e-7 mm while the point of
//! tangency fell 5e-5 mm before the start of the segment: both thresholds rejected the cut, the circle stayed
//! whole and the region never assembled.
use qymcad_core::model::Project;

/// The failing geometry: a circle and a tangent line whose ends sit fractions of a micron from the rim.
fn scene(gap: f64) -> (Project, usize) {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("t");
    let sid = p.sketches[si].id;
    p.add_sketch_node(sid, "Sketch");
    p.add_circle_entity(si, 0.0, 0.0, 9.0, qymcad_core::feature::Purpose::Real);
    p.add_circle_entity(si, 0.0, 0.0, 7.0, qymcad_core::feature::Purpose::Real); // a concentric hole, as in the part
    p.add_line_entity(si, 0.0, 9.0 + gap, 16.0, 9.0 + gap, qymcad_core::feature::Purpose::Real);
    p.add_line_entity(si, 16.0, 9.0 + gap, 16.0, -9.0 - gap, qymcad_core::feature::Purpose::Real);
    p.add_line_entity(si, 16.0, -9.0 - gap, 0.0, -9.0 - gap, qymcad_core::feature::Purpose::Real);
    p.regen_sketch(si);
    (p, si)
}

fn closed_areas(p: &Project, si: usize) -> Vec<f64> {
    let mut v: Vec<f64> = p.sketches[si]
        .contour_ids
        .iter()
        .filter_map(|c| p.contour_index(*c))
        .map(|i| &p.contours[i])
        .filter(|c| c.closed)
        .map(|c| c.area().abs())
        .collect();
    v.sort_by(f64::total_cmp);
    v
}

#[test]
fn tangent_lines_close_a_region_even_with_solver_slack() {
    // solver tolerances that really occur in files: from an exact zero to fractions of a micron
    for gap in [0.0, 1.6e-10, 2.1e-7, 5.0e-5] {
        let (p, si) = scene(gap);
        let areas = closed_areas(&p, si);
        assert!(areas.len() >= 3, "gap {gap:.1e}: circle, hole and box give three regions, got {areas:?}");
        assert!(areas.iter().any(|a| (*a - 161.0).abs() < 3.0), "gap {gap:.1e}: the box region of about 161 mm² is present: {areas:?}");
        assert!(areas.iter().any(|a| (*a - 153.9).abs() < 2.0), "gap {gap:.1e}: the hole is in place: {areas:?}");
    }
}

/// A line that plainly does not touch closes no region, or the tolerance would catch too much.
#[test]
fn a_line_that_clearly_misses_the_circle_closes_nothing() {
    let (p, si) = scene(0.2); // 0.2 mm is a designed clearance rather than solver accuracy
    let areas = closed_areas(&p, si);
    assert!(!areas.iter().any(|a| (*a - 161.0).abs() < 3.0), "a gap of 0.2 mm does not close a region: {areas:?}");
}
