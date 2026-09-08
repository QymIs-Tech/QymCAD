//! THE AXES TAKE PART IN SNAPPING, not only the geometry.
//!
//! Reported behaviour: "in a sketch, a circle of construction geometry has a snap, but at the same time
//! the cursor is not pulled to the X and Y axes, which makes it awkward to put the centre of a new circle
//! on the construction circle - at an axis, for instance."
//!
//! WHERE IT WENT WRONG. Snapping has two halves. The first offers what it finds among the geometry - a
//! midpoint, an INTERSECTION, a point on an edge - and RETURNS as soon as it has anything. The axes X = 0
//! and Y = 0 are looked at only in the second half, after that return. So the moment the cursor came near
//! a circle, the circle answered with "a point on its edge" and the axes never got a turn.
//!
//! And the axes were nowhere among the things that can be INTERSECTED either: intersections were sought
//! between sketch lines, circles and the projected outline of a part. The one point a person is aiming at
//! - where the construction circle crosses the axis - was therefore not offered by either half.
//!
//! A CONSTRUCTION CIRCLE IS WHAT THE REPORT IS ABOUT and it is the ordinary case: construction geometry
//! exists to be aimed at.

use qymcad_core::geom::Point2;
use qymcad_ui_state::Bench;

/// A sketch being edited, with a construction circle of radius 25 centred on the X axis at x = 40.
///
/// Centred ON the axis deliberately: it crosses X at exactly (15, 0) and (65, 0) and does not reach the Y
/// axis at all, so there is one answer to aim at and no second one nearby to confuse the measurement.
fn a_sketch_with_a_construction_circle() -> Bench {
    let mut b = Bench::default();
    b.project.new_document();
    let si = b.project.new_sketch("S");
    let sid = b.project.sketches[si].id;
    b.project.add_circle_entity(si, 40.0, 0.0, 25.0, qymcad_core::feature::Purpose::Construction);
    b.project.regen_sketch(si);
    b.sketch_ses.editing = Some(sid);
    b.view = qymcad_ui_state::View2d { center: egui::Vec2::ZERO, scale: 4.0, initialized: true };
    b
}

/// Where the cursor lands when it is held near `world`.
fn snap_near(b: &mut Bench, world: Point2, off: (f64, f64)) -> (Point2, Option<u8>) {
    let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 700.0));
    let screen = qymcad_ui_state::Sheet { view: b.view, rect }.at(Point2::new(world.x + off.0, world.y + off.1));
    let p = qymcad_sketch::snap_world(&mut b.sketch_ctx(), rect, screen);
    (p, b.snap_hint.map(|(_, ty)| ty))
}

/// THE CROSSING OF A CONSTRUCTION CIRCLE AND THE X AXIS IS SNAPPED TO, exactly.
#[test]
fn the_cursor_sticks_where_a_construction_circle_crosses_the_x_axis() {
    let mut b = a_sketch_with_a_construction_circle();
    let cross = Point2::new(15.0, 0.0);
    let (p, ty) = snap_near(&mut b, cross, (0.6, 0.5));

    assert!(
        (p.x - cross.x).abs() < 1e-6 && (p.y - cross.y).abs() < 1e-6,
        "the cursor is held at the crossing of the construction circle and the X axis, and it landed on ({:.3}, {:.3}) instead of (15, 0) - the circle answered first and the axis never got a turn",
        p.x,
        p.y
    );
    assert_eq!(ty, Some(5), "and the crossing must be reported AS a crossing, so the glyph says what was caught");
}

/// AND THE OTHER CROSSING TOO - so the answer is not one hard-coded point.
#[test]
fn the_far_crossing_is_snapped_to_as_well() {
    let mut b = a_sketch_with_a_construction_circle();
    let cross = Point2::new(65.0, 0.0);
    let (p, _) = snap_near(&mut b, cross, (-0.6, 0.5));

    assert!((p.x - cross.x).abs() < 1e-6 && (p.y - cross.y).abs() < 1e-6, "the far crossing landed on ({:.3}, {:.3}) instead of (65, 0)", p.x, p.y);
}

/// A CONSTRUCTION LINE CROSSING AN AXIS IS SNAPPED TO AS WELL.
///
/// The same gap, and a line is the other half of it: without this the fix could be "special-case circles".
#[test]
fn the_cursor_sticks_where_a_construction_line_crosses_the_y_axis() {
    let mut b = Bench::default();
    b.project.new_document();
    let si = b.project.new_sketch("S");
    let sid = b.project.sketches[si].id;
    // A slanted line from (-30, 10) to (60, 55): it crosses the Y axis at (0, 25), and its MIDPOINT is
    // (15, 32.5) - well away. The first edition of this check used a line whose crossing WAS its midpoint,
    // and a midpoint outranks a crossing by the existing order of snaps, so the check was measuring that
    // order rather than the gap it is about.
    b.project.add_line_entity(si, -30.0, 10.0, 60.0, 55.0, qymcad_core::feature::Purpose::Construction);
    b.project.regen_sketch(si);
    b.sketch_ses.editing = Some(sid);
    b.view = qymcad_ui_state::View2d { center: egui::Vec2::ZERO, scale: 4.0, initialized: true };

    // Held CLOSE to the crossing: half a unit away the nearest answer is a different one - where the line
    // meets a grid line - and it is a legitimate snap of the same rank that simply happens to be nearer.
    let cross = Point2::new(0.0, 25.0);
    let (p, ty) = snap_near(&mut b, cross, (0.15, 0.1));

    assert!((p.x - cross.x).abs() < 1e-6 && (p.y - cross.y).abs() < 1e-6, "the line crosses the Y axis at (0, 25) and the cursor landed on ({:.3}, {:.3})", p.x, p.y);
    assert_eq!(ty, Some(5), "and it must be reported as a crossing");
}

/// AWAY FROM THE AXES NOTHING CHANGES: the circle still answers with a point on its edge.
///
/// Without this the fix could be "always return an axis crossing", which would take the ordinary snap to
/// the edge away.
#[test]
fn away_from_the_axes_the_edge_still_answers() {
    let mut b = a_sketch_with_a_construction_circle();
    let top = Point2::new(40.0, 25.0); // the top of the circle, far from both axes
    let (p, ty) = snap_near(&mut b, top, (0.0, 0.5));

    assert_eq!(ty, Some(6), "away from the axes the circle must go on answering with a point on its edge");
    assert!((p.y - 25.0).abs() < 0.2, "and that point is on the circle: y = {:.3} against 25", p.y);
}
