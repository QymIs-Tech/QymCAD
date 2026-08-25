//! STITCHING SHEETS: pieces of surface become one, and a shell that closes becomes a solid.
//!
//! A surface is rarely born whole: a patch here, a copy of a face there, a third piece in between.
//! While they are separate bodies they can neither be worked with as one surface nor be given a
//! thickness — a thicken would take each piece on its own and produce a stack of plates instead of a
//! lid.
use qymcad_core::geom::Point2;
use qymcad_core::model::Project;
use qymcad_core::refs::{Fingerprint, Ref};

/// A 60x40x12 plate in a part. Returns (project, body).
fn plate() -> (Project, u64) {
    let mut p = Project::default();
    p.new_document();
    let sid = p.add_line_sketch(
        "Sketch 1",
        vec![Point2::new(0.0, 0.0), Point2::new(60.0, 0.0), Point2::new(60.0, 40.0), Point2::new(0.0, 40.0)],
        true,
    );
    let si = p.sketch_index(sid).unwrap();
    p.regen_sketch(si);
    if let Some(o) = p.sketch_owner(sid) {
        p.set_active_component(Some(o));
    }
    p.add_sketch_node(sid, "Sketch 1");
    let closed: Vec<u64> = p.sketches[si].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect();
    let body = p.add_extrude_multi(sid, closed, 12.0, qymcad_core::feature::Reach::Forward, 0.0, vec![]);
    qymcad_testkit::regenerate(&mut p);
    (p, body)
}

/// A copy of one face of a body as a separate sheet.
fn sheet_of(p: &mut Project, body: u64, face: u32) -> u64 {
    let id = p.add_face_copy(body, Ref::one(face, Fingerprint::default()));
    qymcad_testkit::regenerate(p);
    id
}

/// TWO NEIGHBOURING SHEETS BECOME ONE SURFACE.
#[test]
fn two_touching_sheets_become_one_surface() {
    let (mut p, body) = plate();
    let top = p.regen_faces[&body].iter().find(|f| f.normal[2] > 0.9).map(|f| f.id).expect("the top face");
    let side = p.regen_faces[&body].iter().find(|f| f.normal[1] < -0.9).map(|f| f.id).expect("the front face");
    let a = sheet_of(&mut p, body, top);
    let b = sheet_of(&mut p, body, side);
    let area_a: f64 = p.regen_faces[&a].iter().map(|f| f.area).sum();
    let area_b: f64 = p.regen_faces[&b].iter().map(|f| f.area).sum();

    let one = p.add_stitch(vec![a, b], 1e-4);
    let (rep, _) = qymcad_testkit::regenerate(&mut p);
    assert!(rep.errors.is_empty(), "neighbouring sheets must stitch: {:?}", rep.errors);

    let sewn = p.bodies.iter().find(|x| x.id == one).expect("the stitched surface became a body of the document");
    assert!(sewn.sheet, "two open faces do not close a shell — the output is a sheet");
    assert_eq!(p.regen_faces[&one].len(), 2, "the stitched surface must keep both faces");
    let area: f64 = p.regen_faces[&one].iter().map(|f| f.area).sum();
    assert!((area - (area_a + area_b)).abs() < 1.0, "the area must be the sum of the pieces: {area:.1} against {:.1}", area_a + area_b);
    assert!(p.consumed_bodies().contains(&a) && p.consumed_bodies().contains(&b), "the pieces are consumed: what lives on is one surface, not that surface plus its parts");
}

/// IT CLOSED, SO IT IS A SOLID, NOT AN "ALMOST SOLID".
///
/// This is what stitching is needed for most: a set of surfaces that has enclosed a volume from every
/// side already is a solid. Demanding a further step after that would mean asking a person to confirm
/// what the program already knows.
#[test]
fn a_closed_set_of_sheets_becomes_a_solid() {
    let (mut p, body) = plate();
    let faces: Vec<u32> = p.regen_faces[&body].iter().map(|f| f.id).collect();
    assert_eq!(faces.len(), 6, "the plate has six faces, and {} were found", faces.len());
    let parts: Vec<u64> = faces.into_iter().map(|f| sheet_of(&mut p, body, f)).collect();

    let solid = p.add_stitch(parts, 1e-4);
    let (rep, _) = qymcad_testkit::regenerate(&mut p);
    assert!(rep.errors.is_empty(), "six faces of the plate must stitch into a solid: {:?}", rep.errors);

    let out = p.bodies.iter().find(|x| x.id == solid).expect("the solid");
    assert!(!out.sheet, "the shell closed — this is already a SOLID, not a surface");
    assert!((out.mesh.volume() - 60.0 * 40.0 * 12.0).abs() < 1.0, "the volume must match the plate: {:.1} against 28800", out.mesh.volume());
}

/// EDGE NAMES MOVE ACROSS RATHER THAN BEING ISSUED ANEW.
///
/// An edge of a stitched surface is an anchor like any other: a patch is built along it, it gets
/// filleted. Give it a fresh positional number and a reference to it lasts exactly until the next
/// rebuild. This also checks that the names of the two pieces did not collide: two elements with one
/// name are indistinguishable, and a reference to one would mean both.
#[test]
fn edge_names_survive_the_stitch() {
    let (mut p, body) = plate();
    let top = p.regen_faces[&body].iter().find(|f| f.normal[2] > 0.9).map(|f| f.id).expect("the top");
    let side = p.regen_faces[&body].iter().find(|f| f.normal[1] < -0.9).map(|f| f.id).expect("the front");
    let a = sheet_of(&mut p, body, top);
    let b = sheet_of(&mut p, body, side);
    let ea: Vec<u32> = p.regen_edges[&a].iter().map(|e| e.id).collect();
    let eb: Vec<u32> = p.regen_edges[&b].iter().map(|e| e.id).collect();

    let one = p.add_stitch(vec![a, b], 1e-4);
    let (rep, _) = qymcad_testkit::regenerate(&mut p);
    assert!(rep.errors.is_empty(), "setup: {:?}", rep.errors);

    let got: Vec<u32> = p.regen_edges[&one].iter().map(|e| e.id).collect();
    let uniq: std::collections::HashSet<u32> = got.iter().copied().collect();
    assert_eq!(uniq.len(), got.len(), "two edges under one name — a reference to one would mean both: {got:?}");
    assert!(ea.iter().any(|x| uniq.contains(x)), "the edges of the first sheet must keep their names: were {ea:?}, now {got:?}");
    assert!(eb.iter().any(|x| uniq.contains(x)), "and those of the second too: were {eb:?}, now {got:?}");
}

/// SHEETS THAT DO NOT TOUCH EACH OTHER GET A NAMED REFUSAL.
///
/// A stitch would return a compound of two islands: formally "it worked", in substance the same two
/// sheets under one name. Further down the timeline such a surface behaves as debris.
#[test]
fn sheets_that_do_not_touch_are_refused() {
    let (mut p, body) = plate();
    let top = p.regen_faces[&body].iter().find(|f| f.normal[2] > 0.9).map(|f| f.id).expect("the top");
    let bot = p.regen_faces[&body].iter().find(|f| f.normal[2] < -0.9).map(|f| f.id).expect("the bottom");
    let a = sheet_of(&mut p, body, top);
    let b = sheet_of(&mut p, body, bot);

    let bad = p.add_stitch(vec![a, b], 1e-4);
    let (rep, _) = qymcad_testkit::regenerate(&mut p);
    assert!(
        // THE REFUSAL IS NAMED: not "the operation failed" but "there is nothing to stitch — the
        // surfaces do not touch". The test expects exactly that named cause: an anonymous refusal
        // here would be a step back, even though it too is formally a refusal.
        rep.errors.iter().any(|(id, e)| *id == bad && matches!(e, qymcad_core::errors::CoreError::StitchNothingJoined)),
        "the top and the bottom of the plate do not touch — the stitch must refuse with a NAMED cause: {:?}",
        rep.errors
    );
}

/// THE STITCHED SURFACE FOLLOWS THE BASE: stretch the sketch and the surface grows with the part.
#[test]
fn the_stitched_surface_follows_the_base() {
    let (mut p, body) = plate();
    let top = p.regen_faces[&body].iter().find(|f| f.normal[2] > 0.9).map(|f| f.id).expect("the top");
    let side = p.regen_faces[&body].iter().find(|f| f.normal[1] < -0.9).map(|f| f.id).expect("the front");
    let a = sheet_of(&mut p, body, top);
    let b = sheet_of(&mut p, body, side);
    let one = p.add_stitch(vec![a, b], 1e-4);
    let (rep, _) = qymcad_testkit::regenerate(&mut p);
    assert!(rep.errors.is_empty(), "setup: {:?}", rep.errors);
    let before: f64 = p.regen_faces[&one].iter().map(|f| f.area).sum();

    let sid = p.sketches[0].id;
    for pt in &mut p.sketches[0].points {
        if (pt.x - 60.0).abs() < 1e-9 {
            pt.x = 90.0;
        }
    }
    p.solve_sketch(0);
    p.regen_sketch(0);
    p.mark_sketch_dirty(sid);
    let (rep, _) = qymcad_testkit::regenerate(&mut p);
    assert!(rep.errors.is_empty(), "after the base edit the stitch must rebuild: {:?}", rep.errors);
    let after: f64 = p.regen_faces[&one].iter().map(|f| f.area).sum();
    // the top is 90x40 = 3600, the front is 90x12 = 1080
    assert!((after - 4680.0).abs() < 5.0, "the plate became 90 mm — the surface must grow to 4680 mm^2, and it became {after:.1} (was {before:.1})");
}
