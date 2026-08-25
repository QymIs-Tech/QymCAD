//! TRIMMING A SURFACE: a sheet is cut by a body and the piece under the click stays.
//!
//! Trimming is the other half of working with surfaces: first a surface is stretched deliberately
//! LARGER than needed, then the excess is cut away against neighbouring geometry. Without it the
//! outline has to be fitted with sections in advance — that is, guessed.
//!
//! The piece is given by a POINT rather than by a number: a number is a property of today's traversal
//! order, and after an edit of the base it points somewhere else. A point survives both a shift and a
//! stretch, because the question "which piece is nearest to this place" has the same answer as before.
use qymcad_core::geom::Point2;
use qymcad_core::model::Project;
use qymcad_core::refs::{Fingerprint, Ref};

/// A 60x40x12 plate and a sheet copy of its top face. Returns (project, body, sheet).
fn plate_with_sheet() -> (Project, u64, u64) {
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
    let top = p.regen_faces[&body].iter().find(|f| f.normal[2] > 0.9).map(|f| f.id).expect("the top face");
    let sheet = p.add_face_copy(body, Ref::one(top, Fingerprint::default()));
    qymcad_testkit::regenerate(&mut p);
    (p, body, sheet)
}

/// A box tool standing across the sheet: it occupies x in [30, 90].
///
/// Along y it is deliberately LONGER than the plate: a tool that stops reaching the edge no longer
/// cuts the sheet all the way through, and the piece beyond the end of the cut legitimately turns out
/// to be the same one. That is correct behaviour, but associativity cannot be checked with it: what
/// would be measured is the length of the tool, not whether the trim follows the base.
fn cutter(p: &mut Project) -> u64 {
    let sid = p.add_line_sketch(
        "tool",
        vec![Point2::new(30.0, -50.0), Point2::new(90.0, -50.0), Point2::new(90.0, 150.0), Point2::new(30.0, 150.0)],
        true,
    );
    let si = p.sketch_index(sid).unwrap();
    p.regen_sketch(si);
    p.add_sketch_node(sid, "tool");
    let closed: Vec<u64> = p.sketches[si].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect();
    let id = p.add_extrude_multi(sid, closed, 30.0, qymcad_core::feature::Reach::Forward, 0.0, vec![]);
    qymcad_testkit::regenerate(p);
    id
}

/// THE PIECE UNDER THE CLICK IS WHAT STAYS.
#[test]
fn the_piece_under_the_click_is_what_stays() {
    let (mut p, _body, sheet) = plate_with_sheet();
    let tool = cutter(&mut p);
    let whole: f64 = p.regen_faces[&sheet].iter().map(|f| f.area).sum();
    assert!((whole - 2400.0).abs() < 1.0, "setup: a 60x40 sheet is 2400 mm^2, and it came out {whole:.1}");

    // clicked to the LEFT of the tool (x < 30) — the left piece, 30x40, stays
    let left = p.add_trim(sheet, tool, [10.0, 20.0, 12.0]);
    let (rep, _) = qymcad_testkit::regenerate(&mut p);
    assert!(rep.errors.is_empty(), "the trim must build: {:?}", rep.errors);
    let a: f64 = p.regen_faces[&left].iter().map(|f| f.area).sum();
    assert!((a - 1200.0).abs() < 5.0, "the piece under the click must stay: 30x40 = 1200 mm^2, and it came out {a:.1}");
    assert!(p.bodies.iter().any(|b| b.id == left && b.sheet), "what was trimmed is still a surface");
    assert!(p.consumed_bodies().contains(&sheet), "the original sheet is consumed: what lives on is the trimmed one");
    assert!(!p.consumed_bodies().contains(&tool), "the tool is NOT consumed — it goes on cutting");
}

/// CLICK ON THE OTHER SIDE AND THE OTHER PIECE STAYS. Otherwise the point decides nothing.
#[test]
fn clicking_the_other_side_keeps_the_other_piece() {
    let (mut p, _body, sheet) = plate_with_sheet();
    let tool = cutter(&mut p);
    let right = p.add_trim(sheet, tool, [45.0, 20.0, 12.0]); // under the tool (x > 30)
    let (rep, _) = qymcad_testkit::regenerate(&mut p);
    assert!(rep.errors.is_empty(), "the trim must build: {:?}", rep.errors);
    let a: f64 = p.regen_faces[&right].iter().map(|f| f.area).sum();
    assert!((a - 1200.0).abs() < 5.0, "the RIGHT piece must stay: 30x40 = 1200 mm^2, and it came out {a:.1}");

    // and it really is on the right: the centroid of the faces lies beyond the cut line
    let cx = p.regen_faces[&right].iter().map(|f| f.centroid.x).sum::<f64>() / p.regen_faces[&right].len().max(1) as f64;
    assert!(cx > 30.0, "the remaining piece must lie on the far side of the cut, and its centre is at x={cx:.1}");
}

/// A TOOL THAT MISSES IS A NAMED REFUSAL, NOT "IT WORKED AND NOTHING CHANGED".
///
/// A feature that silently does nothing is worse than a failing node: you never learn about it.
#[test]
fn a_tool_that_misses_is_a_named_refusal() {
    let (mut p, _body, sheet) = plate_with_sheet();
    let sid = p.add_line_sketch(
        "off target",
        vec![Point2::new(200.0, 200.0), Point2::new(240.0, 200.0), Point2::new(240.0, 240.0), Point2::new(200.0, 240.0)],
        true,
    );
    let si = p.sketch_index(sid).unwrap();
    p.regen_sketch(si);
    p.add_sketch_node(sid, "off target");
    let closed: Vec<u64> = p.sketches[si].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect();
    let far = p.add_extrude_multi(sid, closed, 30.0, qymcad_core::feature::Reach::Forward, 0.0, vec![]);
    qymcad_testkit::regenerate(&mut p);

    let bad = p.add_trim(sheet, far, [10.0, 20.0, 12.0]);
    let (rep, _) = qymcad_testkit::regenerate(&mut p);
    assert!(
        rep.errors.iter().any(|(id, e)| *id == bad && matches!(e, qymcad_core::errors::CoreError::OpFailed(qymcad_core::errors::Op::Trim))),
        "the tool went past — the trim must fail: {:?}",
        rep.errors
    );
}

/// A SURFACE CAN BE THE CUTTING TOOL TOO, NOT ONLY A SOLID.
///
/// The tool description said so, but there was no guard for it — it rested on "the kernel accepts the
/// input". It does: the tool is the same kind of shape, and the split does not ask whether it is a
/// solid or a sheet. But something declared without a check is an intention, not a property.
#[test]
fn a_sheet_can_be_the_cutting_tool() {
    let (mut p, body, sheet) = plate_with_sheet();
    let solid_tool = cutter(&mut p);
    // the sheet tool: a copy of a side face of the box tool, standing across the sheet
    let side = p.regen_faces[&solid_tool].iter().find(|f| f.normal[0] < -0.9).map(|f| f.id).expect("a side face of the tool");
    let sheet_tool = p.add_face_copy(solid_tool, Ref::one(side, Fingerprint::default()));
    let (rep, _) = qymcad_testkit::regenerate(&mut p);
    assert!(rep.errors.is_empty(), "setup — the sheet tool: {:?}", rep.errors);
    assert!(p.bodies.iter().any(|b| b.id == sheet_tool && b.sheet), "setup: the tool must be a SHEET");
    let _ = body;

    let left = p.add_trim(sheet, sheet_tool, [10.0, 20.0, 12.0]);
    let (rep, _) = qymcad_testkit::regenerate(&mut p);
    assert!(rep.errors.is_empty(), "a surface must be cuttable by a surface: {:?}", rep.errors);
    let a: f64 = p.regen_faces[&left].iter().map(|f| f.area).sum();
    assert!((a - 1200.0).abs() < 5.0, "the piece under the click must stay: 30x40 = 1200 mm^2, and it came out {a:.1}");
    assert!(!p.consumed_bodies().contains(&sheet_tool), "a sheet tool, like a solid tool, is not consumed");
}

/// THE TRIMMED SURFACE FOLLOWS THE BASE: stretch the plate and the same piece stays, only larger.
#[test]
fn the_trimmed_surface_follows_the_base() {
    let (mut p, _body, sheet) = plate_with_sheet();
    let tool = cutter(&mut p);
    let left = p.add_trim(sheet, tool, [10.0, 20.0, 12.0]);
    let (rep, _) = qymcad_testkit::regenerate(&mut p);
    assert!(rep.errors.is_empty(), "setup: {:?}", rep.errors);

    // the plate got deeper: 60x60 instead of 60x40 — the piece left of the cut must grow to 30x60
    let sid = p.sketches[0].id;
    for pt in &mut p.sketches[0].points {
        if (pt.y - 40.0).abs() < 1e-9 {
            pt.y = 60.0;
        }
    }
    p.solve_sketch(0);
    p.regen_sketch(0);
    p.mark_sketch_dirty(sid);
    let (rep, _) = qymcad_testkit::regenerate(&mut p);
    assert!(rep.errors.is_empty(), "after the base edit the trim must rebuild: {:?}", rep.errors);
    let a: f64 = p.regen_faces[&left].iter().map(|f| f.area).sum();
    assert!((a - 1800.0).abs() < 10.0, "the piece must grow with the plate: 30x60 = 1800 mm^2, and it came out {a:.1}");
}
