//! A CUT CORNER DRAWS CLEAN, through the window: a corner cut by a chamfer or rounded by a fillet holds itself by
//! constraints of its own - the virtual sharp on the extensions of the sides, the legs of a symmetric chamfer kept equal
//! on it, the arc touching the lines - and none of them is drawn as a badge. The one dimension of a symmetric chamfer
//! stands outside the shape, on the side of the sharp.
//!
//! Reported behaviour: two "=" and a point-on-line badge stood out in the air beside the cut of a chamfer, a fillet
//! carried a tangency badge on each line, and the dimension of a chamfer on one corner of a triangle dipped into it.
#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use super::super::{App, Sel};
    use qymcad_core::feature::SketchPlane;
    use qymcad_core::model::Constraint;

    /// A sketch with the lines drawn through `clicks` by the line tool, one chain, and the select tool taken.
    fn drawn(clicks: &[(f64, f64)]) -> (App, usize) {
        let mut app = App::default();
        let si = app.create_sketch_on(SketchPlane::default());
        app.chosen.sel = Sel::Sketch(si);
        let mut hand = Hand::new(&mut app);
        hand.sk_tool(qymcad_ui_state::SketchShape::Line);
        for &(x, y) in clicks {
            hand.click2d(x, y);
        }
        hand.key(egui::Key::Escape);
        Hand::new(&mut app).sk_select();
        (app, si)
    }

    /// The lines of the sketch that end at `at`, as items to pick.
    fn lines_at(app: &App, si: usize, at: (f64, f64)) -> Vec<qymcad_ui_state::SketchItem> {
        let s = &app.project.sketches[si];
        let near = |id: u64| s.points.iter().any(|q| q.id == id && (q.x - at.0).hypot(q.y - at.1) < 1e-6);
        s.entities.iter().filter(|e| e.kind.as_line().is_some_and(|qymcad_core::model::LineParts { a, b }| near(a) || near(b))).map(|e| qymcad_ui_state::SketchItem::curve(e.id)).collect()
    }

    /// The constraints the corner holds itself by that are drawn as badges anyway.
    fn badges_of_holders(app: &App, si: usize) -> Vec<String> {
        let holders = app.project.sketches[si].corner_holders();
        qymcad_pick::constraint_glyphs(&app.pick_ctx(), app.viewing.view_rect, si)
            .into_iter()
            .filter(|(ci, _, _)| holders.contains(ci))
            .map(|(ci, _, _)| format!("{:?}", app.project.sketches[si].constraints[ci]))
            .collect()
    }

    #[test]
    fn a_chamfered_corner_shows_no_badge_of_its_sharp() {
        let (mut app, si) = drawn(&[(30.0, 0.0), (0.0, 0.0), (0.0, 30.0)]);
        let lines = lines_at(&app, si, (0.0, 0.0));
        assert!(Hand::new(&mut app).sk_corner_set("tb-chamfer-sketch-hint", &lines, 3.0), "no place to click on the two lines");
        assert_eq!(app.project.sketches[si].corner_holders().len(), 3, "the sharp is held on both lines and its legs kept equal: {:?}", app.project.sketches[si].constraints);
        let shown = badges_of_holders(&app, si);
        assert!(shown.is_empty(), "what holds the sharp is drawn as badges: {shown:?}");
    }

    #[test]
    fn a_rounded_corner_shows_no_badge_of_its_tangency() {
        let (mut app, si) = drawn(&[(30.0, 0.0), (0.0, 0.0), (0.0, 30.0)]);
        let lines = lines_at(&app, si, (0.0, 0.0));
        assert!(Hand::new(&mut app).sk_corner_set("tb-fillet-sketch-hint", &lines, 3.0), "no place to click on the two lines");
        let tangents = app.project.sketches[si].constraints.iter().filter(|c| matches!(c, Constraint::Tangent { .. })).count();
        assert_eq!(tangents, 2, "the arc touches both lines");
        let shown = badges_of_holders(&app, si);
        assert!(shown.is_empty(), "the tangencies of the fillet are drawn as badges: {shown:?}");
    }

    /// THE CURSOR DOES NOT STICK TO THE PART OF A CIRCLE AN ARC DOES NOT RUN OVER: the corner of two lines rounded R20
    /// leaves an arc of a quarter turn about (20, 20), and the cursor rested on the same circle at 20 deg - its far side,
    /// off the grid nodes - finds no snap to the circle there.
    ///
    /// Reported behaviour: a green snap mark showed, and the cursor stuck to it, beside a fillet where no arc is drawn.
    #[test]
    fn the_hidden_part_of_a_fillet_circle_is_no_snap() {
        let (mut app, si) = drawn(&[(40.0, 0.0), (0.0, 0.0), (0.0, 40.0)]);
        let lines = lines_at(&app, si, (0.0, 0.0));
        assert!(Hand::new(&mut app).sk_corner_set("tb-fillet-sketch-hint", &lines, 20.0), "no place to click on the two lines");
        let mut hand = Hand::new(&mut app);
        hand.sk_tool(qymcad_ui_state::SketchShape::Line); // a drawing tool: the cursor snaps while drawing
        let far = (20.0 + 20.0 * 20f64.to_radians().cos(), 20.0 + 20.0 * 20f64.to_radians().sin());
        hand.hover2d(far.0, far.1);
        let hint = app.snap_hint;
        assert!(hint.is_none_or(|(p, _)| (p.x - far.0).hypot(p.y - far.1) > 1e-3), "the cursor snapped to the hidden part of the fillet circle at {far:?}: {hint:?}");
    }

    /// THE CORNER OF A ROUNDED RECTANGLE IS A POINT ONE CAN PICK: its virtual sharp is drawn and picked like any point,
    /// so a dimension or a constraint can be measured to it.
    #[test]
    fn the_sharp_of_a_rounded_rectangle_is_picked() {
        let mut app = App::default();
        let si = app.create_sketch_on(SketchPlane::default());
        app.chosen.sel = Sel::Sketch(si);
        Hand::new(&mut app).sk_tool(qymcad_ui_state::SketchShape::Rect).click2d(0.0, 0.0).click2d(40.0, 30.0).key(egui::Key::Enter);
        Hand::new(&mut app).sk_select();
        let sides: Vec<qymcad_ui_state::SketchItem> = app.project.sketches[si].rects()[0].sides.iter().map(|&id| qymcad_ui_state::SketchItem::curve(id)).collect();
        assert!(Hand::new(&mut app).sk_corner_set("tb-fillet-sketch-hint", &sides, 3.0), "no place to click on the four sides");
        let corner = app.project.sketches[si].rects()[0].corners[2];
        assert!(!app.project.sketches[si].unseen_points().contains(&corner), "the corner of the rounded rectangle is hidden from the drawing");
        let at = app.project.sketches[si].points.iter().find(|q| q.id == corner).map(|q| (q.x, q.y)).expect("the corner stays");
        let mut hand = Hand::new(&mut app);
        hand.hover2d(at.0, at.1);
        assert_eq!(app.chosen.hover.sketch, Some(qymcad_ui_state::SketchItem::point(corner)), "the cursor on the sharp of the rounded corner does not take it");
    }

    /// THE DIMENSION OF A SYMMETRIC CHAMFER STANDS OUTSIDE: on a corner of a triangle the dimension line is drawn off to
    /// the side of the sharp - the left of its first end looking at the second, on screen, as the renderer lays it - and
    /// not into the triangle.
    #[test]
    fn the_dimension_of_a_chamfer_on_a_triangle_stands_outside() {
        let (mut app, si) = drawn(&[(0.0, 0.0), (60.0, 0.0), (30.0, -40.0), (0.0, 0.0)]);
        let lines = lines_at(&app, si, (0.0, 0.0));
        assert_eq!(lines.len(), 2, "setup: two lines meet at (0, 0)");
        assert!(Hand::new(&mut app).sk_corner_set("tb-chamfer-sketch-hint", &lines, 3.0), "no place to click on the two lines");
        let s = &app.project.sketches[si];
        let Some((a, b)) = s.constraints.iter().find_map(|c| match *c {
            Constraint::Distance { a, b, d, .. } if (d - 3.0).abs() < 1e-9 => Some((a, b)),
            _ => None,
        }) else {
            panic!("no dimension of 3 on the cut: {:?}", s.constraints)
        };
        let sheet = qymcad_ui_state::Sheet { view: app.viewing.view, rect: app.viewing.view_rect };
        let at = |id: u64| s.points.iter().find(|q| q.id == id).map(|q| sheet.at(qymcad_core::geom::Point2::new(q.x, q.y))).expect("a point");
        let (sa, sb) = (at(a), at(b));
        let dir = (sb - sa).normalized();
        let perp = egui::vec2(-dir.y, dir.x); // as the renderer offsets a linear dimension
        let mid = sa + (sb - sa) / 2.0;
        let inside = sheet.at(qymcad_core::geom::Point2::new(30.0, -13.0)); // the centroid of the triangle
        assert!(perp.dot(inside - mid) < 0.0, "the dimension of the chamfer is drawn into the triangle: from {sa:?} to {sb:?}, off towards {perp:?}");
    }
}
