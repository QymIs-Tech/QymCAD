//! A LOST REFERENCE SAYS SO OUT LOUD INSTEAD OF PASSING FOR "DO IT TO EVERYTHING".
//!
//! For a fillet and a chamfer an empty edge list means "the whole part" — a convenient convention,
//! and the right one for a "round everything" request. The trouble is that THE SAME empty list comes
//! out when a person picked one edge and the reference to it stopped resolving. Measured on a
//! 30x20x12 box:
//!
//! ```text
//! fillet on a non-existent edge: 26 faces, NO failures
//! fillet on all twelve edges:    26 faces
//! ```
//!
//! The same thing. One edge was asked for, the whole part got rounded, and the timeline said nothing.
//! That is worse than a failing node: a person does not see it.
//!
//! A shell has the same class of problem in a milder form: two faces were asked to be removed, one
//! reference did not resolve, one face was removed and the node stayed green (11 faces instead of
//! 10). The part comes out looking similar and closed on the side where an opening was expected.
//!
//! What is checked here is not the geometry but HONESTY: a lost reference must fail, and the body
//! must stay untouched. And separately — that the legitimate "do it to everything" request still
//! works, otherwise the cure would kill the patient.
use qymcad_core::geom::Point2;
use qymcad_core::model::{Id, Project};

/// An id the body certainly does not have: names are interned consecutively from the NAMED flag.
const FOREIGN_ID: u32 = 999_999;

fn box_body(p: &mut Project, w: f64, h: f64, up: f64) -> Id {
    let sid = p.add_line_sketch(
        "Sketch",
        vec![Point2::new(0.0, 0.0), Point2::new(w, 0.0), Point2::new(w, h), Point2::new(0.0, h)],
        true,
    );
    let si = p.sketch_index(sid).unwrap();
    p.regen_sketch(si);
    if let Some(o) = p.sketch_owner(sid) {
        p.set_active_component(Some(o));
    }
    p.add_sketch_node(sid, "Sketch");
    let closed: Vec<Id> = p.sketches[si].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect();
    let b = p.add_extrude_multi(sid, closed, up, qymcad_core::feature::Reach::Forward, 0.0, vec![]);
    qymcad_testkit::regenerate(p);
    b
}

#[test]
fn a_fillet_with_a_lost_edge_says_so_instead_of_rounding_everything() {
    let mut p = Project::default();
    p.new_document();
    let body = box_body(&mut p, 30.0, 20.0, 12.0);
    let was = p.regen_faces.get(&body).map(|f| f.len()).unwrap_or(0);
    assert_eq!(was, 6, "a box must give six faces, and it gave {was}");
    let f = p.add_fillet(body, 1.5, vec![FOREIGN_ID]);
    qymcad_testkit::regenerate(&mut p);
    let now = p.regen_faces.get(&f).map(|x| x.len()).unwrap_or(0);
    assert!(
        !p.regen_errors.is_empty(),
        "a fillet on a lost reference must fail rather than stay silent: the face count became {now}"
    );
    assert_eq!(now, 6, "the body must stay untouched instead of being rounded all over: {now} faces");
}

#[test]
fn a_chamfer_with_a_lost_edge_says_so_instead_of_cutting_everything() {
    let mut p = Project::default();
    p.new_document();
    let body = box_body(&mut p, 30.0, 20.0, 12.0);
    let ch = p.add_chamfer(body, 1.0, vec![FOREIGN_ID]);
    qymcad_testkit::regenerate(&mut p);
    let now = p.regen_faces.get(&ch).map(|x| x.len()).unwrap_or(0);
    assert!(!p.regen_errors.is_empty(), "a chamfer on a lost reference must fail: the face count became {now}");
    assert_eq!(now, 6, "the body must stay untouched: {now} faces");
}

#[test]
fn a_shell_that_found_fewer_faces_than_asked_says_so() {
    let mut p = Project::default();
    p.new_document();
    let body = box_body(&mut p, 30.0, 20.0, 12.0);
    let mut open: Vec<u32> = p.regen_faces[&body].iter().filter(|f| f.normal[2] > 0.9).map(|f| f.id).collect();
    assert_eq!(open.len(), 1, "the box has one top face");
    open.push(FOREIGN_ID); // ask for TWO; the second will not resolve
    let sh = p.add_shell_mode(body, 2.0, open, qymcad_core::feature::ShellSide::Inward);
    qymcad_testkit::regenerate(&mut p);
    let now = p.regen_faces.get(&sh).map(|x| x.len()).unwrap_or(0);
    assert!(
        !p.regen_errors.is_empty(),
        "the shell found fewer faces than were asked for and must say so: the face count became {now}"
    );
}

/// THE CURE DID NOT KILL THE PATIENT. A "round EVERYTHING" request names no descriptors at all and
/// must keep working as before. Measured: 6 faces of a box become 26 (twelve fillets and eight
/// patches).
#[test]
fn a_fillet_of_everything_still_works() {
    let mut p = Project::default();
    p.new_document();
    let body = box_body(&mut p, 30.0, 20.0, 12.0);
    let edges: Vec<u32> = p.regen_edges[&body].iter().map(|e| e.id).collect();
    assert_eq!(edges.len(), 12, "a box has twelve edges");
    let f = p.add_fillet(body, 1.5, edges);
    qymcad_testkit::regenerate(&mut p);
    assert!(p.regen_errors.is_empty(), "filleting every edge failed: {:?}", p.regen_errors);
    let now = p.regen_faces.get(&f).map(|x| x.len()).unwrap_or(0);
    assert_eq!(now, 26, "filleting all twelve edges must give 26 faces, and it gave {now}");
}

/// AND A SHELL WHOSE REFERENCES ARE ALL ALIVE BUILDS AS BEFORE. Measured: removing two faces of a box
/// gives 10 faces (four walls outside, four inside, two rims).
#[test]
fn a_shell_with_live_refs_still_works() {
    let mut p = Project::default();
    p.new_document();
    let body = box_body(&mut p, 30.0, 20.0, 12.0);
    let open: Vec<u32> = p.regen_faces[&body].iter().filter(|f| f.normal[2].abs() > 0.9).map(|f| f.id).collect();
    assert_eq!(open.len(), 2, "the top and the bottom of the box");
    let sh = p.add_shell_mode(body, 2.0, open, qymcad_core::feature::ShellSide::Inward);
    qymcad_testkit::regenerate(&mut p);
    assert!(p.regen_errors.is_empty(), "a shell with live references failed: {:?}", p.regen_errors);
    let now = p.regen_faces.get(&sh).map(|x| x.len()).unwrap_or(0);
    assert_eq!(now, 10, "a shell with the top and bottom removed must give 10 faces, and it gave {now}");
}

/// A DRAFT THAT LOST ONE WALL. Measured: a draft with four live walls and one unresolvable reference
/// drafted four of them and stayed GREEN (area 1172.00 -> 1258.82). The part comes out looking
/// similar, but one wall stands vertical — and a mould is cast against it.
#[test]
fn a_draft_that_lost_one_wall_says_so() {
    let mut p = Project::default();
    p.new_document();
    let body = box_body(&mut p, 20.0, 14.0, 9.0);
    let mut sides: Vec<u32> = p.regen_faces[&body].iter().filter(|f| f.normal[2].abs() < 0.1).map(|f| f.id).collect();
    assert_eq!(sides.len(), 4, "the box has four side faces");
    sides.push(FOREIGN_ID);
    let neutral = p.regen_faces[&body].iter().find(|f| f.normal[2] < -0.9).map(|f| f.id).expect("the base");
    let d = p.add_draft(body, sides, neutral, 5.0, false);
    qymcad_testkit::regenerate(&mut p);
    assert!(!p.regen_errors.is_empty(), "the draft lost a wall and must say so");
    let now: f64 = p.regen_faces.get(&d).map(|f| f.iter().map(|x| x.area).sum()).unwrap_or(0.0);
    assert!((now - 1172.0).abs() < 0.5, "the body must stay untouched: area {now:.2}");
}

/// AND A DRAFT WITH LIVE REFERENCES WORKS AS BEFORE. Measured: 1172.00 -> 1258.82.
#[test]
fn a_draft_with_live_refs_still_works() {
    let mut p = Project::default();
    p.new_document();
    let body = box_body(&mut p, 20.0, 14.0, 9.0);
    let sides: Vec<u32> = p.regen_faces[&body].iter().filter(|f| f.normal[2].abs() < 0.1).map(|f| f.id).collect();
    let neutral = p.regen_faces[&body].iter().find(|f| f.normal[2] < -0.9).map(|f| f.id).expect("the base");
    let d = p.add_draft(body, sides, neutral, 5.0, false);
    qymcad_testkit::regenerate(&mut p);
    assert!(p.regen_errors.is_empty(), "a draft with live references failed: {:?}", p.regen_errors);
    let now: f64 = p.regen_faces.get(&d).map(|f| f.iter().map(|x| x.area).sum()).unwrap_or(0.0);
    assert!(now > 1172.0 + 1.0, "the draft did not touch the material: area {now:.2}");
}

/// A SHELL THAT HOLLOWED NOTHING IS NOT A SHELL.
///
/// Measured on a 30x18x12 box: a thickness of 12 gave back the ORIGINAL body (6 faces, area 2232.00 —
/// exactly the stock) with the node GREEN, while a thickness of 9 gave the same thing but with an
/// honest refusal. The user saw "the shell built" while the part stayed solid.
#[test]
fn a_shell_that_hollowed_nothing_says_so() {
    let mut p = Project::default();
    p.new_document();
    let body = box_body(&mut p, 30.0, 18.0, 12.0);
    let solid: f64 = p.regen_faces[&body].iter().map(|x| x.area).sum();
    let open: Vec<u32> = p.regen_faces[&body].iter().filter(|f| f.normal[2] > 0.9).map(|f| f.id).collect();
    let sh = p.add_shell_mode(body, 12.0, open, qymcad_core::feature::ShellSide::Inward); // a wall the full height — there will be no cavity
    qymcad_testkit::regenerate(&mut p);
    let now: f64 = p.regen_faces.get(&sh).map(|f| f.iter().map(|x| x.area).sum()).unwrap_or(0.0);
    assert!(
        !p.regen_errors.is_empty(),
        "the shell hollowed nothing and must say so: stock area {solid:.2}, now {now:.2}"
    );
}

/// AND A SANE THICKNESS BUILDS AS BEFORE. Measured: 2232.00 for the stock, 3032.00 for the shell.
#[test]
fn a_shell_with_sane_thickness_still_works() {
    let mut p = Project::default();
    p.new_document();
    let body = box_body(&mut p, 30.0, 18.0, 12.0);
    let solid: f64 = p.regen_faces[&body].iter().map(|x| x.area).sum();
    let open: Vec<u32> = p.regen_faces[&body].iter().filter(|f| f.normal[2] > 0.9).map(|f| f.id).collect();
    let sh = p.add_shell_mode(body, 2.0, open, qymcad_core::feature::ShellSide::Inward);
    qymcad_testkit::regenerate(&mut p);
    assert!(p.regen_errors.is_empty(), "an ordinary shell failed: {:?}", p.regen_errors);
    let now: f64 = p.regen_faces.get(&sh).map(|f| f.iter().map(|x| x.area).sum()).unwrap_or(0.0);
    assert!(now > solid + 1.0, "the shell added no inner walls: was {solid:.2}, now {now:.2}");
}
