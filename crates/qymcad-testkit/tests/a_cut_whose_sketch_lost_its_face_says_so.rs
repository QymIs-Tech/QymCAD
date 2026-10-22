//! A CUT WHOSE SKETCH LOST THE FACE IT STOOD ON SAYS SO, not what followed from it. A block 40 x 40 x 20, a pocket 20 x
//! 20 cut `d` = 5 down from its top, a sketch on the floor of the pocket and a cut 4 x 4 from it 2 down. `d` typed 25 in
//! the table of parameters takes the pocket through the block: the floor is gone, the sketch on it is placed by the
//! nearest face that faces the same way, and the cut, drawn where nothing is left to cut, said that it removed nothing -
//! the person was sent to look at the cut, not at the face its sketch had lost.
//!
//! Live OCCT, headless.
use qymcad_core::errors::CoreError;
use qymcad_core::feature::{Extent, FaceKey, Purpose, SketchPlane};
use qymcad_core::model::{CombineSpan, Id, Param, PlaneDef, Project, WorkPlane};

/// A CUT AND THE SKETCH IT IS DRAWN ON.
struct Cut {
    node: Id,
    sketch: Id,
}

/// A cut of the rectangle between `corners` drawn on `plane`, `down` into `body`.
fn cut(p: &mut Project, body: Id, plane: SketchPlane, corners: [[f64; 2]; 2], down: f64) -> Cut {
    let si = p.new_sketch("cut");
    p.sketches[si].plane = plane;
    let sid = p.sketches[si].id;
    p.add_sketch_node(sid, "cut");
    let [[x0, y0], [x1, y1]] = corners;
    p.add_rect_entity(si, x0, y0, x1, y1, Purpose::Real);
    p.regen_sketch(si);
    let regions: Vec<Id> = p.sketches[si].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect();
    let node = p.add_combine_multi_op(body, sid, regions, CombineSpan { height: 0.0, down, extent: Extent::default(), fill: &[] }, 0);
    Cut { node, sketch: sid }
}

#[test]
fn the_cut_says_its_sketch_lost_its_face_when_the_face_is_gone() {
    let mut p = Project::default();
    p.new_document();
    p.parameters.push(Param { name: "d".into(), expr: "5".into(), value: 5.0 });
    let block = p.add_box(40.0, 40.0, 20.0);
    let top = p.add_plane(WorkPlane { origin: [0.0, 0.0, 20.0], def: PlaneDef::OffsetBase { base: qymcad_core::feature::BasePlane::XY, dist: 20.0 }, ..Default::default() });
    let pocket = cut(&mut p, block, SketchPlane::Datum(top), [[-10.0, -10.0], [10.0, 10.0]], 5.0);
    p.set_feat_dim(pocket.node, "down", "d".into());
    let _ = qymcad_testkit::regenerate(&mut p);
    let floor = p
        .regen_faces
        .get(&pocket.node)
        .and_then(|fs| fs.iter().find(|f| f.normal[2] > 0.99 && (f.centroid.z - 15.0).abs() < 1e-6 && (f.area - 400.0).abs() < 1e-6))
        .cloned()
        .expect("setup: the floor of the pocket");
    let key = FaceKey { index: 0, centroid: [floor.centroid.x, floor.centroid.y, floor.centroid.z], normal: floor.normal, id: floor.id };
    let hole = cut(&mut p, pocket.node, SketchPlane::Face(pocket.node, key), [[-2.0, -2.0], [2.0, 2.0]], 2.0);
    let (report, shapes) = qymcad_testkit::regenerate(&mut p);
    assert!(report.errors.is_empty(), "setup: the block, the pocket and the cut on its floor do not build: {:?}", report.errors);

    // d typed 25 in the table of parameters
    p.parameters[0].expr = "25".into();
    assert!(p.eval_parameters().is_empty(), "the parameter did not take 25");
    p.mark_param_dependents_dirty_for("d");
    let _ = qymcad_testkit::regenerate_dirty_with_shapes(&mut p, shapes);
    assert!(!p.regen_errors.contains_key(&pocket.node), "the pocket through the block does not build: {:?}", p.regen_errors.get(&pocket.node));
    assert!(
        matches!(p.regen_errors.get(&hole.node), Some(CoreError::SketchFaceRefLost { sketch, .. }) if *sketch == hole.sketch),
        "the cut whose sketch lost the floor it stood on says {:?}, not that the floor is gone",
        p.regen_errors.get(&hole.node)
    );
}
