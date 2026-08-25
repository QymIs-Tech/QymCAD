//! GIVING A SHEET A THICKNESS: a surface becomes an ordinary body.
//!
//! This is the way out of the design layer and back into the timeline. A patch on its own is a
//! surface: it can neither be added to a part nor printed. A thickness turns it into a body that is
//! worked with like everything else.
//!
//! Thickening a face of a PART used to glue a plate onto its source ("one part, one body"). A sheet
//! has nothing to glue to — it is the plate itself, so on a sheet the operation takes the surface
//! WHOLE and returns a solid. One tool, two cases: the difference is not in the button but in what
//! was pointed at.
use qymcad_core::geom::Point2;
use qymcad_core::model::Project;
use qymcad_core::refs::Ref;

/// A 40x30x20 box with the top face removed. Returns (project, body).
fn open_box() -> (Project, u64) {
    let mut p = Project::default();
    p.new_document();
    let sid = p.add_line_sketch(
        "Sketch 1",
        vec![Point2::new(0.0, 0.0), Point2::new(40.0, 0.0), Point2::new(40.0, 30.0), Point2::new(0.0, 30.0)],
        true,
    );
    let si = p.sketch_index(sid).unwrap();
    p.regen_sketch(si);
    if let Some(o) = p.sketch_owner(sid) {
        p.set_active_component(Some(o));
    }
    p.add_sketch_node(sid, "Sketch 1");
    let closed: Vec<u64> = p.sketches[si].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect();
    let boxy = p.add_extrude_multi(sid, closed, 20.0, qymcad_core::feature::Reach::Forward, 0.0, vec![]);
    qymcad_testkit::regenerate(&mut p);
    let top: Vec<u32> = p.regen_faces[&boxy].iter().filter(|f| f.normal[2] > 0.9).map(|f| f.id).collect();
    let shell = p.add_shell_mode(boxy, 2.0, top, qymcad_core::feature::ShellSide::Inward);
    qymcad_testkit::regenerate(&mut p);
    (p, shell)
}

/// The outer edges of the opening — the boundary of the lid.
fn rim_edges(p: &Project, body: u64) -> Vec<u32> {
    let top = p.regen_edges[&body].iter().flat_map(|e| [e.a[2], e.b[2]]).fold(f64::MIN, f64::max);
    let (mut x0, mut x1, mut y0, mut y1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for e in &p.regen_edges[&body] {
        for q in [e.a, e.b] {
            x0 = x0.min(q[0]);
            x1 = x1.max(q[0]);
            y0 = y0.min(q[1]);
            y1 = y1.max(q[1]);
        }
    }
    let on_border = |q: [f64; 3]| (q[0] - x0).abs() < 1e-6 || (q[0] - x1).abs() < 1e-6 || (q[1] - y0).abs() < 1e-6 || (q[1] - y1).abs() < 1e-6;
    p.regen_edges[&body]
        .iter()
        .filter(|e| (e.a[2] - top).abs() < 1e-6 && (e.b[2] - top).abs() < 1e-6 && on_border(e.a) && on_border(e.b))
        .map(|e| e.id)
        .collect()
}

/// A SHEET PLUS A THICKNESS IS A SOLID, AND IT GOES BACK INTO ITS OWN PART.
///
/// The lid used to stay a separate body, so the part held two of them — a differently coloured piece
/// on screen. Now the plate is glued to the live body of the part the surface was taken from, and the
/// volume of the result is the sum of the box and the lid.
#[test]
fn a_sheet_thickened_becomes_a_solid() {
    let (mut p, body) = open_box();
    let patch = p.add_patch(body, Ref::picks(&rim_edges(&p, body)), false);
    let (rep, _) = qymcad_testkit::regenerate(&mut p);
    assert!(rep.errors.is_empty(), "setup — the patch: {:?}", rep.errors);
    assert!(p.bodies.iter().any(|b| b.id == patch && b.sheet), "setup: the patch must be a sheet");

    let v_box = p.bodies.iter().find(|b| b.id == body).expect("the box").mesh.volume();
    let face = p.regen_faces[&patch][0].id;
    let lid = p.add_thicken(patch, face, 2.0);
    let (rep, _) = qymcad_testkit::regenerate(&mut p);
    assert!(rep.errors.is_empty(), "a sheet must thicken: {:?}", rep.errors);

    let solid = p.bodies.iter().find(|b| b.id == lid).expect("the lid became a body of the document");
    assert!(!solid.sheet, "after a thickness this is NO longer a sheet but a body — otherwise nothing further can be done with it");
    let lid = 40.0 * 30.0 * 2.0;
    assert!((solid.mesh.volume() - (v_box + lid)).abs() < 30.0, "the result is the box {v_box:.1} plus the lid {lid:.1}, and it came out {:.1}", solid.mesh.volume());
    assert!(p.consumed_bodies().contains(&patch), "the sheet is consumed: on screen there is a lid, not a lid on top of a surface");
    assert!(p.consumed_bodies().contains(&body), "and the part is consumed: what lives on is one body, not a part plus a lid");
}

/// AND THIS IS THE WAY BACK INTO THE TIMELINE: the box closes WITHOUT a separate union step.
///
/// The path used to be "patch, thicken, unite", and the last step had to be remembered. Forget it and
/// the part holds two bodies. Now the thicken returns the lid into the part by itself.
#[test]
fn the_thickened_lid_closes_the_box_without_a_separate_union() {
    let (mut p, body) = open_box();
    let patch = p.add_patch(body, Ref::picks(&rim_edges(&p, body)), false);
    qymcad_testkit::regenerate(&mut p);
    let v_box = p.bodies.iter().find(|b| b.id == body).expect("the box").mesh.volume();
    let face = p.regen_faces[&patch][0].id;
    let lid = p.add_thicken(patch, face, 2.0);
    let (rep, _) = qymcad_testkit::regenerate(&mut p);
    assert!(rep.errors.is_empty(), "setup — the thickness: {:?}", rep.errors);

    let v = p.bodies.iter().find(|b| b.id == lid).expect("the closed box").mesh.volume();
    assert!((v - (v_box + 2400.0)).abs() < 30.0, "the closed box is {v_box:.1} plus a lid of 2400, and it came out {v:.1}");
    let live: Vec<u64> = p.bodies.iter().map(|b| b.id).filter(|b| !p.consumed_bodies().contains(b) && !p.bodies.iter().any(|x| x.id == *b && x.sheet)).collect();
    assert_eq!(live.len(), 1, "the part must be left with ONE body, and there are {live:?}");
}

/// ZERO THICKNESS IS A NAMED REFUSAL, not an empty body.
#[test]
fn zero_thickness_is_a_named_refusal() {
    let (mut p, body) = open_box();
    let patch = p.add_patch(body, Ref::picks(&rim_edges(&p, body)), false);
    qymcad_testkit::regenerate(&mut p);
    let face = p.regen_faces[&patch][0].id;
    let bad = p.add_thicken(patch, face, 0.0);
    let (rep, _) = qymcad_testkit::regenerate(&mut p);
    assert!(
        rep.errors.iter().any(|(id, e)| *id == bad && matches!(e, qymcad_core::errors::CoreError::ZeroThickness)),
        "zero thickness must fail with exactly the zero-thickness cause: {:?}",
        rep.errors
    );
}
