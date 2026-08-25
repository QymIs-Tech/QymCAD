//! AN OPERATION THAT DID NO WORK MUST SAY SO.
//!
//! A green node with nothing behind it is worse than a failing one: a person does not see it. For a
//! cut it also comes at a LOSS — a body boolean CONSUMES the tool, so if the cut removed nothing the
//! tool body is gone for nothing.
//!
//! Measured before the fix (a 20x20x10 box, a 10x10x10 tool off to the side at 60,60):
//!
//! ```text
//! body cut past the base:      area 1600.00 -> 1600.00, tool CONSUMED, NO failures
//! contour cut past the body:   area 1600.00 -> 1600.00, NO failures
//! ```
//!
//! For comparison, the ones that answered honestly even before the fix and needed no changes:
//! intersecting disjoint bodies gives `EmptyResult`, and splitting by a plane outside the extent
//! gives `OpFailed(SplitBody)`.
use qymcad_core::geom::Point2;
use qymcad_core::model::{Id, Project, WorkPlane};

fn brick(p: &mut Project, name: &str, x0: f64, y0: f64, x1: f64, y1: f64, up: f64) -> Id {
    let sid = p.add_line_sketch(
        name,
        vec![Point2::new(x0, y0), Point2::new(x1, y0), Point2::new(x1, y1), Point2::new(x0, y1)],
        true,
    );
    let si = p.sketch_index(sid).unwrap();
    p.regen_sketch(si);
    if let Some(o) = p.sketch_owner(sid) {
        p.set_active_component(Some(o));
    }
    p.add_sketch_node(sid, name);
    let closed: Vec<Id> = p.sketches[si].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect();
    let b = p.add_extrude_multi(sid, closed, up, qymcad_core::feature::Reach::Forward, 0.0, vec![]);
    qymcad_testkit::regenerate(p);
    b
}

fn area(p: &Project, b: Id) -> f64 {
    p.regen_faces.get(&b).map(|f| f.iter().map(|x| x.area).sum()).unwrap_or(0.0)
}

#[test]
fn a_body_cut_that_removed_nothing_says_so() {
    let mut p = Project::default();
    p.new_document();
    let base = brick(&mut p, "Base", 0.0, 0.0, 20.0, 20.0, 10.0);
    let tool = brick(&mut p, "Tool", 60.0, 60.0, 70.0, 70.0, 10.0);
    let was = area(&p, base);
    let cut = p.add_body_boolean(base, tool, 0);
    qymcad_testkit::regenerate(&mut p);
    let now = area(&p, cut);
    assert!(
        !p.regen_errors.is_empty(),
        "a cut by a body standing off to the side removed nothing and must say so: the area was {was:.2}, is now {now:.2}"
    );
}

#[test]
fn a_contour_cut_that_removed_nothing_says_so() {
    let mut p = Project::default();
    p.new_document();
    let base = brick(&mut p, "Base", 0.0, 0.0, 20.0, 20.0, 10.0);
    let was = area(&p, base);
    let sid = p.add_line_sketch(
        "Window off to the side",
        vec![Point2::new(60.0, 60.0), Point2::new(70.0, 60.0), Point2::new(70.0, 70.0), Point2::new(60.0, 70.0)],
        true,
    );
    let si = p.sketch_index(sid).unwrap();
    p.regen_sketch(si);
    p.add_sketch_node(sid, "Window off to the side");
    let c = p.sketches[si].contour_ids.iter().copied().find(|c| p.contour_profile_xy(*c).is_some()).expect("the window contour");
    let cut = p.add_combine_on(base, sid, c, 20.0, 0, qymcad_core::feature::Extent { through: true, reach: qymcad_core::feature::Reach::Backward }, 0.0);
    qymcad_testkit::regenerate(&mut p);
    let now = area(&p, cut);
    assert!(
        !p.regen_errors.is_empty(),
        "a contour cut past the part removed nothing and must say so: the area was {was:.2}, is now {now:.2}"
    );
}

/// THE CURE DID NOT KILL THE PATIENT: a real body cut works as before.
#[test]
fn a_real_body_cut_still_works() {
    let mut p = Project::default();
    p.new_document();
    let base = brick(&mut p, "Base", 0.0, 0.0, 20.0, 20.0, 10.0);
    let tool = brick(&mut p, "Tool", 5.0, 5.0, 15.0, 15.0, 10.0);
    let was = area(&p, base);
    let cut = p.add_body_boolean(base, tool, 0);
    qymcad_testkit::regenerate(&mut p);
    assert!(p.regen_errors.is_empty(), "a real cut failed: {:?}", p.regen_errors);
    let now = area(&p, cut);
    assert!((now - was).abs() > 1.0, "the cut did not touch the material: was {was:.2}, now {now:.2}");
}

/// AND SO DOES A REAL CONTOUR CUT.
#[test]
fn a_real_contour_cut_still_works() {
    let mut p = Project::default();
    p.new_document();
    let base = brick(&mut p, "Base", 0.0, 0.0, 20.0, 20.0, 10.0);
    let was = area(&p, base);
    let sid = p.add_line_sketch(
        "Window",
        vec![Point2::new(5.0, 5.0), Point2::new(15.0, 5.0), Point2::new(15.0, 15.0), Point2::new(5.0, 15.0)],
        true,
    );
    let si = p.sketch_index(sid).unwrap();
    p.regen_sketch(si);
    p.add_sketch_node(sid, "Window");
    let c = p.sketches[si].contour_ids.iter().copied().find(|c| p.contour_profile_xy(*c).is_some()).expect("the window contour");
    let cut = p.add_combine_on(base, sid, c, 20.0, 0, qymcad_core::feature::Extent { through: true, reach: qymcad_core::feature::Reach::Backward }, 0.0);
    qymcad_testkit::regenerate(&mut p);
    assert!(p.regen_errors.is_empty(), "a real contour cut failed: {:?}", p.regen_errors);
    let now = area(&p, cut);
    assert!((now - was).abs() > 1.0, "the contour cut did not touch the material: was {was:.2}, now {now:.2}");
}

/// THE MIRROR PLANE WAS DELETED — THE PART DOES NOT MOVE SILENTLY.
///
/// The reference used to "degrade" to a world plane, on the argument that a mirror about another
/// plane is still a mirror. The measurement refuted that argument: a mirror about the datum x=50 gave
/// face centres from x=10 to x=90, and after deleting the datum from -30 to 30, with not one failing
/// node. The part moved to the other end of the world without a word.
///
/// A split in the same trouble has refused for a long time — better a failing node than a quietly
/// different part.
#[test]
fn a_mirror_whose_plane_was_deleted_says_so() {
    let mut p = Project::default();
    p.new_document();
    let body = brick(&mut p, "Base", 10.0, 0.0, 30.0, 20.0, 10.0);
    let pl = p.add_plane(WorkPlane {
        id: 0,
        name: "x50".into(),
        origin: [50.0, 0.0, 0.0],
        normal: [1.0, 0.0, 0.0],
        rot_deg: 0.0,
        def: Default::default(),
    });
    let m = p.add_mirror(body, 2, true, pl);
    qymcad_testkit::regenerate(&mut p);
    let span = |p: &Project, b: Id| -> (f64, f64) {
        let f = p.regen_faces.get(&b).cloned().unwrap_or_default();
        (
            f.iter().map(|x| x.centroid.x).fold(f64::INFINITY, f64::min),
            f.iter().map(|x| x.centroid.x).fold(f64::NEG_INFINITY, f64::max),
        )
    };
    let (lo, hi) = span(&p, m);
    assert!(p.regen_errors.is_empty(), "a mirror about a datum failed: {:?}", p.regen_errors);
    assert!(hi > 80.0, "a mirror about the datum x=50 must throw the copy past x=80: centres from {lo:.1} to {hi:.1}");

    assert!(p.delete_plane(pl), "the plane must be deleted");
    for n in p.timeline.iter_mut() {
        n.dirty = true;
    }
    qymcad_testkit::regenerate(&mut p);
    assert!(!p.regen_errors.is_empty(), "the mirror lost its plane and must say so");
    let (lo2, hi2) = span(&p, m);
    assert!(
        hi2 <= 30.5 && lo2 >= 9.5,
        "the part must stay where it is rather than move: centres from {lo2:.1} to {hi2:.1}"
    );
}

/// AND A MIRROR ABOUT A WORLD PLANE WORKS AS BEFORE — it has no datum, so there is nothing to lose.
#[test]
fn a_mirror_about_a_world_plane_still_works() {
    let mut p = Project::default();
    p.new_document();
    let body = brick(&mut p, "Base", 10.0, 0.0, 30.0, 20.0, 10.0);
    let m = p.add_mirror(body, 2, true, 0);
    qymcad_testkit::regenerate(&mut p);
    assert!(p.regen_errors.is_empty(), "a mirror about a world plane failed: {:?}", p.regen_errors);
    let n = p.regen_faces.get(&m).map(|f| f.len()).unwrap_or(0);
    assert!(n >= 12, "a mirror that keeps the original must give two boxes: {n} faces");
}
