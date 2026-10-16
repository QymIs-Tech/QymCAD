//! EVERY FACE OF A CHAMFERED CONICAL HOLE HAS A NAME OF ITS OWN. Reported behaviour (#173): a conical hole cut by a
//! revolve through a 15 x 15 x 10 block, chamfered 0.5 on both its mouths, had 11 faces and 9 names - two chamfer faces
//! were each split in two by the seam of the cone, and both halves of each carried one name. A name is an address: a
//! fillet, a sketch or a mate on one half could land on the other. The profile lies on the -x side of an upward axis,
//! so the seam of the cone falls where the chamfer cuts it.
use qymcad_core::feature::{BasePlane, Purpose, Reach, SketchPlane};
use qymcad_core::geom::Point2;
use qymcad_core::model::{Project, RevolveAxis, RevolveTurn};

#[test]
fn every_face_of_a_chamfered_conical_hole_has_its_own_name() {
    let mut p = Project::default();
    p.new_document();
    let stock = p.add_box(15.0, 15.0, 10.0); // centred in XY, z from 0 to 10
                                             // the half profile of a cone, 6.75 across at z = -1 and 3.75 across at z = 11, on the -x side of an upward axis
    let sid = p.add_line_sketch("cone", vec![Point2::new(0.0, -1.0), Point2::new(-3.375, -1.0), Point2::new(-1.875, 11.0), Point2::new(0.0, 11.0)], true);
    let si = p.sketch_index(sid).expect("the sketch");
    p.sketches[si].plane = SketchPlane::World(BasePlane::XZ);
    p.add_sketch_node(sid, "cone");
    let axis = p.add_line_entity(si, 0.0, -2.0, 0.0, 12.0, Purpose::Construction);
    p.regen_sketch(si);
    let profiles = p.sketches[si].contour_ids.iter().copied().filter(|id| p.contour_profile_xy(*id).is_some()).collect();
    let cut = p.add_revolve_multi_op(sid, profiles, RevolveAxis { axis: 1, datum: 0, line: axis }, RevolveTurn { angle: 360.0, reach: Reach::Forward }, stock, 0);
    let (report, _) = qymcad_testkit::regenerate(&mut p);
    assert!(report.errors.is_empty(), "setup: the conical hole is cut: {:?}", report.errors);
    // the two mouths of the hole: circles of radius 3.25 at the bottom and 2 at the top
    let mouths: Vec<u32> = p.regen_edges[&cut].iter().filter(|e| e.radius > 1.9 && e.radius < 3.3).map(|e| e.id).collect();
    assert_eq!(mouths.len(), 2, "setup: the hole has two round mouths");
    let chamfer = p.add_chamfer(cut, 0.5, mouths);
    let (report, _) = qymcad_testkit::regenerate(&mut p);
    assert!(report.errors.is_empty(), "setup: both mouths are chamfered: {:?}", report.errors);
    let names: Vec<u32> = p.regen_faces[&chamfer].iter().map(|f| f.id).collect();
    let distinct: std::collections::BTreeSet<u32> = names.iter().copied().collect();
    assert_eq!(distinct.len(), names.len(), "two faces of the chamfered hole share a name: {:?}", names.iter().map(|n| p.names.describe(*n)).collect::<Vec<_>>());
}
