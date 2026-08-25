//! The identity of contours across an edit. Matching by centroid and area substitutes one id for another when
//! loops swap places, and the feature then extrudes somebody else's contour. Correct matching goes by the
//! entities of the loop.

use qymcad_core::model::Project;

/// Two squares, A over 0..20 and B over 40..60, with A extruded. The edit moves A onto the former place of B,
/// 40..60, and B further on to 80..100. The feature has to follow A, so the column now stands over x in
/// [40, 60], rather than follow the other loop.
#[test]
fn feature_follows_its_loop_when_loops_swap_places() {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("s");
    let sid = p.sketches[si].id;
    p.add_sketch_node(sid, "s");
    let a_ids = p.add_rect_entity(si, 0.0, 0.0, 20.0, 20.0, qymcad_core::feature::Purpose::Real);
    let _b_ids = p.add_rect_entity(si, 40.0, 0.0, 60.0, 20.0, qymcad_core::feature::Purpose::Real);
    p.regen_sketch(si);
    // the contour of square A is the one whose centroid is at x = 10
    let cid_a = p.sketches[si]
        .contour_ids
        .iter()
        .copied()
        .find(|&c| p.contour_index(c).map(|ci| (p.contours[ci].centroid().x - 10.0).abs() < 1.0).unwrap_or(false))
        .expect("the contour of A");
    let e = p.add_extrude_multi(sid, vec![cid_a], 10.0, qymcad_core::feature::Reach::Forward, 0.0, vec![]);
    let body = p.finish_base_body(e, 1);
    let (_, shapes) = qymcad_testkit::regenerate(&mut p);
    assert!((shapes.get(&body).unwrap().volume() - 4000.0).abs() < 40.0, "at the start the column stands over A");
    // one edit: A moves by 40, onto the former place of B, and B moves 40 further on
    let a_set: std::collections::HashSet<u64> = a_ids.iter().copied().collect();
    let mut a_pts: std::collections::HashSet<u64> = std::collections::HashSet::new();
    for ent in &p.sketches[si].entities {
        if a_set.contains(&ent.id) {
            if let qymcad_core::model::EntityKind::Line { a, b } = ent.kind {
                a_pts.insert(a);
                a_pts.insert(b);
            }
        }
    }
    for pt in &mut p.sketches[si].points {
        if pt.x > -1.0 {
            pt.x += 40.0; // every point of both squares moves by 40
        }
    }
    p.regen_sketch(si);
    p.mark_sketch_dirty(sid);
    let (report, shapes) = qymcad_testkit::regenerate(&mut p);
    for (id, e) in &report.errors {
        panic!("the rebuild after the edit failed: {id}: {e}");
    }
    // the column has to stand over the new position of A, x in [40, 60], which a probe checks
    let s = shapes.get(&body).expect("there is a body");
    let outer = qymcad_core::geom::Contour::closed(vec![
        qymcad_core::geom::Point2::new(40.0, 0.0),
        qymcad_core::geom::Point2::new(60.0, 0.0),
        qymcad_core::geom::Point2::new(60.0, 20.0),
        qymcad_core::geom::Point2::new(40.0, 20.0),
    ]);
    let prof = qymcad_core::geom::encode_profile(&outer, &[]);
    let probe = qymcad_kernel::Shape::extrude_profile(&prof, 10.0).expect("the probe");
    let inter = s.boolean(&probe, 2).map(|x| x.volume()).unwrap_or(0.0);
    assert!(
        (inter - 4000.0).abs() < 40.0,
        "the feature did not follow its own loop A: in the zone of the new A there is {inter:.0} of 4000 of material, the body being {:.0}, so the id of the contour was substituted by another loop",
        s.volume()
    );
}
