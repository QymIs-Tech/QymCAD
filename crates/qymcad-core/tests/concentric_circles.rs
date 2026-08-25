//! Two circles sharing a centre position must not collapse into one.
//!
//! Each circle has to own its centre node, since the radius variable of the solver is keyed by centre.
use qymcad_core::model::{EntityKind, Project};

#[test]
fn concentric_circles_get_distinct_centers_and_radii() {
    let mut p = Project::default();
    let _part = p.new_document();
    // an empty sketch, then two circles at the same point (0,0) with different radii
    let sid = p.add_sketch("concentric", vec![], None);
    p.add_sketch_node(sid, "Sketch");
    let si = p.sketch_index(sid).unwrap();

    let e_out = p.add_circle_entity(si, 0.0, 0.0, 10.0, qymcad_core::feature::Purpose::Real);
    let e_in = p.add_circle_entity(si, 0.0, 0.0, 5.0, qymcad_core::feature::Purpose::Real);

    let centers: Vec<_> = p.sketches[si]
        .entities
        .iter()
        .filter_map(|e| match e.kind {
            EntityKind::Circle { center, r } => Some((e.id, center, r)),
            _ => None,
        })
        .collect();
    eprintln!("circles: {centers:?}");
    assert_eq!(centers.len(), 2, "two circles");
    let c_out = centers.iter().find(|c| c.0 == e_out).unwrap().1;
    let c_in = centers.iter().find(|c| c.0 == e_in).unwrap().1;
    assert_ne!(c_out, c_in, "each circle owns its centre node");

    // the radii survive the solve independently
    p.solve_sketch(si);
    let radii: Vec<f64> = p.sketches[si]
        .entities
        .iter()
        .filter_map(|e| match e.kind {
            EntityKind::Circle { r, .. } => Some(r),
            _ => None,
        })
        .collect();
    eprintln!("radii after the solve: {radii:?}");
    assert!(radii.iter().any(|&r| (r - 10.0).abs() < 1e-3), "the outer r = 10 survived");
    assert!(radii.iter().any(|&r| (r - 5.0).abs() < 1e-3), "the inner r = 5 survived");
}
