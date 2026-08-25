//! FACE NAMES UNDER MODIFIERS: pattern and mirror.
//!
//! The result of these operations holds SEVERAL IMAGES of one source face. If the name is simply
//! inherited from the source, every copy gets THE SAME name — and a reference to a face of the
//! second copy resolves to the first. That is worse than no name at all: the error is silent and
//! looks like "the feature landed in the wrong place". Measured: how many faces there are and how
//! many DISTINCT names they carry.
use std::collections::HashSet;

fn plate_with_array(count: u32) -> (qymcad_core::model::Project, u64) {
    use qymcad_core::geom::{Contour, Point2};
    let mut p = qymcad_core::model::Project::default();
    p.new_document();
    let si = p.new_sketch("plate");
    let sid = p.sketches[si].id;
    p.add_sketch_node(sid, "plate");
    p.add_rect_entity(si, 0.0, 0.0, 20.0, 10.0, qymcad_core::feature::Purpose::Real);
    p.regen_sketch(si);
    let _ = Contour::closed(vec![Point2::new(0.0, 0.0)]);
    let body = p.add_extrude(sid, 5.0);
    let arr = p.add_linear_array(body, 30.0, 0.0, 0.0, count);
    let last = p.finish_base_body(arr, 1);
    (p, last)
}

/// MEASURED: 18 faces on 6 names — every instance carries the names of the ORIGINAL.
///
/// Why this cannot be fixed in place. The kernel has `Kernel::rename_faces(body, pairs)` — renaming
/// by pairs of "old id -> new". For a pattern that is INEXPRESSIBLE: `from` is the same for every
/// instance, so one pair would rename them all into a single name at once. What is needed is not a
/// change to the call but a way to address a SPECIFIC OCCURRENCE of a face in the result.
///
/// How it should work: the name is given at CONSTRUCTION time, not afterwards. `GeoName.split`
/// already exists for this — "the piece number, when one source entity gave rise to several faces".
/// So `linear_array`/`circular_array`/`mirror` must accept a per-instance name seeding (the way
/// `combine_region_multi` accepts `caps`), and the C++ side must hand those out to the images of the
/// source through `BRepTools_History` instead of copying the source number.
///
/// Scope: the signatures of three Kernel operations, the hand-out in occt_bridge, and name interning
/// in core. That is a separate pass and cannot be started "on the way": which is exactly why this
/// test is marked as a failure rather than deleted — the defect is measured, reproduces with one
/// command, and does not pass itself off as closed.
#[test]
fn array_instances_do_not_share_face_names() {
    let (mut p, last) = plate_with_array(3);
    let (report, _shapes) = qymcad_testkit::regenerate(&mut p);
    assert!(report.errors.is_empty(), "the pattern did not build: {:?}", report.errors);
    let faces = p.regen_faces.get(&last).cloned().unwrap_or_default();
    let ids: Vec<u32> = faces.iter().map(|f| f.id).collect();
    let uniq: HashSet<u32> = ids.iter().copied().collect();
    eprintln!("[pattern x3] faces {}, distinct names {}", ids.len(), uniq.len());
    assert_eq!(ids.len(), uniq.len(), "pattern instances share face names: {} faces on {} names — a reference to a copy will land on the original", ids.len(), uniq.len());
}

/// The halves of a MIRROR THAT KEEPS THE ORIGINAL must not share names either.
///
/// With `keep`, the body holds BOTH halves — the original and its image. If the image carries the
/// names of the source, a reference to a face of the mirrored half resolves to the original one: a
/// fillet or a hole lands on the opposite side of the part.
#[test]
fn mirror_halves_do_not_share_face_names() {
    use std::collections::HashSet;
    let mut p = qymcad_core::model::Project::default();
    p.new_document();
    let si = p.new_sketch("plate");
    let sid = p.sketches[si].id;
    p.add_sketch_node(sid, "plate");
    p.add_rect_entity(si, 10.0, 10.0, 30.0, 20.0, qymcad_core::feature::Purpose::Real); // OFF to the side of the mirror plane
    p.regen_sketch(si);
    let body = p.add_extrude(sid, 5.0);
    let mir = p.add_mirror(body, 2, true, 0); // the YZ plane, keep the original
    let last = p.finish_base_body(mir, 1);
    let (report, _s) = qymcad_testkit::regenerate(&mut p);
    assert!(report.errors.is_empty(), "the mirror did not build: {:?}", report.errors);
    let ids: Vec<u32> = p.regen_faces.get(&last).cloned().unwrap_or_default().iter().map(|f| f.id).collect();
    let uniq: HashSet<u32> = ids.iter().copied().collect();
    eprintln!("[mirror + original] faces {}, distinct names {}", ids.len(), uniq.len());
    assert_eq!(ids.len(), uniq.len(), "the mirror halves share names: {} faces on {} names", ids.len(), uniq.len());
}

/// SHELL: the inner walls are new faces and must have names of their own.
///
/// It does not work like a pattern: there are no copies, there are source faces and inner walls born
/// by offsetting them. If a wall carries the name of its outer face, a reference "fillet the inner
/// edge" lands on the outer one — the CAD appears to miss.
/// MEASURED: 11 faces on 6 names — the wall carries the name of its outer face.
///
/// Why the pattern/mirror approach does not apply. There the name was given BEFORE the union, while
/// the copy was still separate. Here there are no copies at all: OCCT gives birth to the inner walls
/// by OFFSETTING the source faces, and in the finished body an outer face and its wall are
/// indistinguishable by id. They can be told apart only through the operation history —
/// `BRepTools_History`: what came through `Modified` stays itself, and what came through `Generated`
/// is a new face and must get the name `Role::ShellWall` with `src` = the source face. The role
/// already exists in the types. THE PLACE TO FIX IS PINPOINTED: `occt_bridge.cpp`, the shell
/// function — after `MakeThickSolidByJoin` comes `propagate_ids(mk, s->shape, TopAbs_FACE, ...)`, and
/// it carries the source id to ALL images of the face, including the generated wall. The same
/// approach as for the pattern is needed: core seeds the pairs "source face -> the name of its wall"
/// and C++ hands them to what came through `Generated`, leaving `Modified` as itself. The signature
/// of `qym_shape_shell` gains three parameters (from/to/n), as already done for caps in
/// `combine_region_multi`.
///
/// Telling them apart geometrically (by normal direction) does not work here: on a shell of variable
/// thickness and on rounded bodies the normals of the outer and inner face are not opposite, and that
/// would bring back guessing about identity — the very thing the whole naming system moves away from.
#[test]
fn shell_inner_walls_have_their_own_names() {
    use std::collections::HashSet;
    let mut p = qymcad_core::model::Project::default();
    p.new_document();
    let si = p.new_sketch("box");
    let sid = p.sketches[si].id;
    p.add_sketch_node(sid, "box");
    p.add_rect_entity(si, 0.0, 0.0, 40.0, 30.0, qymcad_core::feature::Purpose::Real);
    p.regen_sketch(si);
    let body = p.add_extrude(sid, 20.0);
    // remove the top face -> an open box with walls
    let (r0, _s0) = qymcad_testkit::regenerate(&mut p);
    assert!(r0.errors.is_empty(), "the plate did not build: {:?}", r0.errors);
    let top = p.regen_faces.get(&body).and_then(|fs| fs.iter().find(|f| f.normal[2] > 0.9).map(|f| f.id)).expect("top face");
    let sh = p.add_shell(body, 2.0, vec![top], false);
    let last = p.finish_base_body(sh, 1);
    let (report, _s) = qymcad_testkit::regenerate(&mut p);
    assert!(report.errors.is_empty(), "the shell did not build: {:?}", report.errors);
    let ids: Vec<u32> = p.regen_faces.get(&last).cloned().unwrap_or_default().iter().map(|f| f.id).collect();
    let uniq: HashSet<u32> = ids.iter().copied().collect();
    eprintln!("[shell] faces {}, distinct names {}", ids.len(), uniq.len());
    assert_eq!(ids.len(), uniq.len(), "shell: {} faces on {} names", ids.len(), uniq.len());
}

/// A CIRCULAR pattern has the same collision as a linear one (EVERY path must be checked, not just a
/// "similar" one).
#[test]
fn circular_array_instances_do_not_share_face_names() {
    use std::collections::HashSet;
    let mut p = qymcad_core::model::Project::default();
    p.new_document();
    let si = p.new_sketch("blade");
    let sid = p.sketches[si].id;
    p.add_sketch_node(sid, "blade");
    p.add_rect_entity(si, 10.0, -2.0, 20.0, 2.0, qymcad_core::feature::Purpose::Real);
    p.regen_sketch(si);
    let body = p.add_extrude(sid, 4.0);
    let arr = p.add_circular_array(body, 4, 360.0);
    let last = p.finish_base_body(arr, 1);
    let (report, _s) = qymcad_testkit::regenerate(&mut p);
    assert!(report.errors.is_empty(), "the circular pattern did not build: {:?}", report.errors);
    let ids: Vec<u32> = p.regen_faces.get(&last).cloned().unwrap_or_default().iter().map(|f| f.id).collect();
    let uniq: HashSet<u32> = ids.iter().copied().collect();
    eprintln!("[circular x4] faces {}, distinct names {}", ids.len(), uniq.len());
    assert_eq!(ids.len(), uniq.len(), "the circular pattern shares names: {} faces on {} names", ids.len(), uniq.len());
}
