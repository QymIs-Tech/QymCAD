//! A PATCH: THE FIRST DESIGN-LAYER SHAPE THAT WAS NOT ON THE BODY BEFORE.
//!
//! A face copy only reproduced something that already existed; a patch closes an OPENING: the edges
//! give a boundary and the kernel stretches a surface over it. Hence the order of work: first what
//! creates a shape, then what edits it.
use qymcad_core::geom::Point2;
use qymcad_core::model::Project;
use qymcad_core::refs::Ref;

/// A 40x30x20 box with the TOP face removed: a 2 mm wall and an opening on top. Returns (project, body).
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

/// The OUTER edges of the opening — the boundary of the future lid.
///
/// The outer ones specifically: on a shell the opening is outlined twice (an outer outline and an
/// inner one, 2 mm inwards), and stretching a patch over both at once would be asking the kernel to
/// close a ring rather than a hole.
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

/// A PATCH SPANS THE OPENING WITH A SURFACE.
#[test]
fn a_patch_spans_the_opening() {
    let (mut p, body) = open_box();
    let rim = rim_edges(&p, body);
    assert_eq!(rim.len(), 4, "the box opening has four outer edges, and {} were found", rim.len());

    let patch = p.add_patch(body, Ref::picks(&rim), false);
    let (rep, _) = qymcad_testkit::regenerate(&mut p);
    assert!(rep.errors.is_empty(), "the patch must build: {:?}", rep.errors);

    let sheet = p.bodies.iter().find(|b| b.id == patch).expect("the patch became a body of the document");
    assert!(sheet.sheet, "a patch is a SHEET: it has no volume, it is a surface");
    assert!(!sheet.mesh.tris.is_empty(), "and it has geometry");
    let area: f64 = p.regen_faces[&patch].iter().map(|f| f.area).sum();
    assert!((area - 1200.0).abs() < 5.0, "the lid over a 40x30 opening is 1200 mm^2, and {area:.1} was stretched");
}

/// THE SOURCE BODY STAYS AN OPEN BOX: the patch lives beside it, not instead of it.
#[test]
fn the_box_stays_open_until_the_patch_is_sewn_in() {
    let (mut p, body) = open_box();
    let v0 = p.bodies.iter().find(|b| b.id == body).expect("the body").mesh.volume();
    let rim = rim_edges(&p, body);
    p.add_patch(body, Ref::picks(&rim), false);
    qymcad_testkit::regenerate(&mut p);

    let src = p.bodies.iter().find(|b| b.id == body).expect("the source is still there");
    assert!((src.mesh.volume() - v0).abs() < 1e-6, "the box must stay as it was: was {v0:.2}, now {:.2}", src.mesh.volume());
    assert!(!p.consumed_bodies().contains(&body), "a patch does NOT consume the body — for now it is just a surface beside it");
}

/// THE PATCH FOLLOWS THE BASE: stretch the sketch and the lid grows with the opening.
///
/// This is what separates a timeline feature from "draw a surface and forget it": the boundary is
/// taken from TODAY's geometry, not from yesterday's.
#[test]
fn the_patch_follows_the_opening() {
    let (mut p, body) = open_box();
    let rim = rim_edges(&p, body);
    let patch = p.add_patch(body, Ref::picks(&rim), false);
    let (rep, _) = qymcad_testkit::regenerate(&mut p);
    assert!(rep.errors.is_empty(), "setup: {:?}", rep.errors);
    let before: f64 = p.regen_faces[&patch].iter().map(|f| f.area).sum();

    let sid = p.sketches[0].id;
    for pt in &mut p.sketches[0].points {
        if (pt.x - 40.0).abs() < 1e-9 {
            pt.x = 60.0;
        }
    }
    p.solve_sketch(0);
    p.regen_sketch(0);
    p.mark_sketch_dirty(sid);
    let (rep, _) = qymcad_testkit::regenerate(&mut p);
    assert!(rep.errors.is_empty(), "after the base edit the patch must rebuild: {:?}", rep.errors);
    let after: f64 = p.regen_faces[&patch].iter().map(|f| f.area).sum();
    assert!((after - 1800.0).abs() < 5.0, "the opening became 60x30 — the lid must become 1800 mm^2, and it became {after:.1} (was {before:.1})");
}

/// TANGENCY BENDS THE SURFACE SO IT MEETS THE WALLS WITHOUT A SEAM.
///
/// "By position" the surface simply runs into the edges: over a rectangular opening that is a flat
/// lid. "Smooth" asks it to approach the same edges TANGENTIALLY to the neighbouring faces — the box
/// walls are vertical, so the lid has to bow. The difference is measured by area: over the same
/// boundary a curved surface is larger than a flat one, and that is a property rather than an
/// impression.
#[test]
fn tangency_bends_the_surface_to_meet_the_walls() {
    let (mut p, body) = open_box();
    let rim = rim_edges(&p, body);
    let flat = p.add_patch(body, Ref::picks(&rim), false);
    let (rep, _) = qymcad_testkit::regenerate(&mut p);
    assert!(rep.errors.is_empty(), "the patch by position: {:?}", rep.errors);
    let a_flat: f64 = p.regen_faces[&flat].iter().map(|f| f.area).sum();
    assert!((a_flat - 1200.0).abs() < 5.0, "by position, a flat 1200 mm^2 lid must land over a 40x30 opening, and it came out {a_flat:.1}");

    let (mut p2, body2) = open_box();
    let rim2 = rim_edges(&p2, body2);
    let smooth = p2.add_patch(body2, Ref::picks(&rim2), true);
    let (rep, _) = qymcad_testkit::regenerate(&mut p2);
    assert!(rep.errors.is_empty(), "the smooth patch: {:?}", rep.errors);
    let a_smooth: f64 = p2.regen_faces[&smooth].iter().map(|f| f.area).sum();
    assert!(a_smooth > a_flat + 5.0, "a smooth patch must BOW towards the walls: {a_smooth:.1} against the flat {a_flat:.1}");

    // and that is recorded in the timeline — the switch did not "apply and get forgotten"
    let stored = p2
        .timeline
        .iter()
        .find_map(|n| match n.kind {
            qymcad_core::feature::FeatureKind::Patch { tangent, .. } => Some(tangent),
            _ => None,
        })
        .expect("the patch is in the timeline");
    assert!(stored, "tangency must be stored in the node, otherwise the surface becomes a different one after a rebuild");
}

/// A LOST BOUNDARY IS A NAMED REFUSAL, NOT A SURFACE AT RANDOM.
#[test]
fn a_lost_boundary_is_a_named_refusal() {
    let (mut p, body) = open_box();
    let patch = p.add_patch(body, Ref::picks(&[0xDEAD_BEEF, 0xDEAD_BEEE]), false);
    let (rep, _) = qymcad_testkit::regenerate(&mut p);
    assert!(rep.errors.iter().any(|(id, _)| *id == patch), "a boundary that disappeared must fail: {:?}", rep.errors);
}
