//! Three circles at one coordinate keep three separate centres.
//!
//! Merging coincident points (`merge_close_points`, called automatically after a trim or a cut) must not
//! collapse those centres into one node: the radius variables would fuse and the sketch would break, with the
//! radii drifting.
use qymcad_core::model::{EntityKind, Project};

fn circle_centers(p: &Project, si: usize) -> Vec<u64> {
    let mut cs: Vec<u64> = p.sketches[si]
        .entities
        .iter()
        .filter_map(|e| match e.kind {
            EntityKind::Circle { center, .. } => Some(center),
            _ => None,
        })
        .collect();
    cs.sort_unstable();
    cs
}

#[test]
fn merge_close_points_keeps_concentric_circle_centers_distinct() {
    let mut p = Project::default();
    let _part = p.new_document();
    let sid = p.add_sketch("concentric", vec![], None);
    p.add_sketch_node(sid, "Sketch");
    let si = p.sketch_index(sid).unwrap();

    // three circles at one point (0,0): Ø3, Ø6 and Ø12, giving r of 1.5, 3 and 6
    p.add_circle_entity(si, 0.0, 0.0, 1.5, qymcad_core::feature::Purpose::Real);
    p.add_circle_entity(si, 0.0, 0.0, 3.0, qymcad_core::feature::Purpose::Real);
    p.add_circle_entity(si, 0.0, 0.0, 6.0, qymcad_core::feature::Purpose::Real);

    // a line below, as in the reported scenario, whose ends are not at the centre
    p.add_line_entity(si, -6.0, -6.0, 6.0, -6.0, qymcad_core::feature::Purpose::Real);

    let centers_before = circle_centers(&p, si);
    assert_eq!(centers_before.len(), 3, "three circles give three entities");
    assert_eq!(
        centers_before.iter().collect::<std::collections::HashSet<_>>().len(),
        3,
        "each circle owns its centre; `add_circle_entity` splits concentric ones"
    );

    // merging coincident points: what runs after a trim or a cut and used to break the sketch
    p.merge_close_points(si, 1e-3);

    let centers_after = circle_centers(&p, si);
    assert_eq!(
        centers_after.iter().collect::<std::collections::HashSet<_>>().len(),
        3,
        "after the merge the centres stayed distinct and the radii did not collapse"
    );

    // the radii survived, all three distinct
    let mut radii: Vec<f64> = p.sketches[si]
        .entities
        .iter()
        .filter_map(|e| match e.kind {
            EntityKind::Circle { r, .. } => Some(r),
            _ => None,
        })
        .collect();
    radii.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert_eq!(radii.len(), 3, "three circles");
    assert!((radii[0] - 1.5).abs() < 1e-6 && (radii[1] - 3.0).abs() < 1e-6 && (radii[2] - 6.0).abs() < 1e-6, "the radii 1.5, 3 and 6 survived rather than fusing: {radii:?}");
}
