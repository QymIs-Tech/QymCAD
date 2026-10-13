//! TANGENT POINTS ON CIRCLES/ARCS SNAP LIKE LINE ENDS WHEN DRAWING A LINE.
//!
//! When drawing a line (tool code 1), if the cursor is within snap distance of a tangency point
//! on an existing circle or arc, the new line should snap to that tangency point with type 6
//! ("on an edge"), just like it snaps to the end of another line.

use qymcad_core::geom::Point2;
use qymcad_ui_state::{Armed, Bench};

/// A sketch being edited, with a circle of radius 25 centred at (0, 0).
fn a_sketch_with_a_circle() -> Bench {
    let mut b = Bench::default();
    b.project.new_document();
    let si = b.project.new_sketch("S");
    let sid = b.project.sketches[si].id;
    // Circle at (0, 0) with radius 25
    b.project.add_circle_entity(si, 0.0, 0.0, 25.0, qymcad_core::feature::Purpose::Real);
    b.project.regen_sketch(si);
    b.sketch_ses.editing = Some(sid);
    b.view = qymcad_ui_state::View2d { center: egui::Vec2::ZERO, scale: 4.0, initialized: true, fit: 4.0 };
    // Set up line drawing tool (code 1) with one point already placed
    b.armed = Armed::Draw(1);
    b.tool_prefs.circ_mode = 0;
    b.tool.pts.push(Point2::new(-50.0, 0.0)); // Start point of the line being drawn
    b
}

/// Where the cursor lands when it is held near `world`.
fn snap_near(b: &mut Bench, world: Point2, off: (f64, f64)) -> (Point2, Option<u8>) {
    let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 700.0));
    let screen = qymcad_ui_state::Sheet { view: b.view, rect }.at(Point2::new(world.x + off.0, world.y + off.1));
    let p = qymcad_sketch::snap_world(&mut b.sketch_ctx(), rect, screen);
    (p, b.snap_hint.map(|(_, ty)| ty))
}

/// THE TANGENCY POINT ON A CIRCLE IS SNAPPED TO WHEN DRAWING A LINE FROM OUTSIDE.
#[test]
fn the_cursor_snaps_to_the_tangency_point_on_a_circle_when_drawing_a_line() {
    let mut b = a_sketch_with_a_circle();

    // For a circle at (0, 0) with radius 25, and line start point at (-50, 0),
    // the tangency points are where lines from (-50, 0) touch the circle.
    // Distance from (-50, 0) to center (0, 0) is d = 50.
    // Radius r = 25.
    // cos(alpha) = r/d = 25/50 = 0.5, so alpha = 60 degrees = PI/3
    // The vector from C to P is (-50, 0), which is along the negative x-axis (angle 180 deg).
    // The tangency points are at angles 180 - 60 = 120 deg and 180 + 60 = 240 deg.
    // Tangency point 1: angle = 120 deg => tp1 = (25*cos(120deg), 25*sin(120deg))
    // cos(120) = -0.5, sin(120) = sqrt(3)/2 ≈ 0.8660254
    let tp1 = Point2::new(-12.5, 25.0 * 3_f64.sqrt() / 2.0);

    // Hold cursor near the tangency point tp1
    let (p, ty) = snap_near(&mut b, tp1, (0.5, 0.5));

    assert!(
        (p.x - tp1.x).abs() < 1e-4 && (p.y - tp1.y).abs() < 1e-4,
        "the cursor is held near the tangency point on the circle at ({:.3}, {:.3}), and it landed on ({:.3}, {:.3}) instead",
        tp1.x,
        tp1.y,
        p.x,
        p.y
    );
    assert_eq!(ty, Some(5), "and the snap type must be 5 (tangent point on circle/arc)");
}
