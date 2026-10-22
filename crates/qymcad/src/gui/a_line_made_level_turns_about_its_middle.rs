//! A LINE MADE HORIZONTAL OR VERTICAL TURNS ABOUT ITS MIDDLE AND KEEPS ITS LENGTH, through the window: a line standing
//! at 80 deg, picked and given Horizontal, lies level with the length it had - turned to the direction, not squashed onto it.
//!
//! Reported behaviour: a vertical line whose Vertical was deleted, given Horizontal, was refused with "it would pull the
//! line into a point". The solve takes the least movement of the points: bringing both ends to one height is less than
//! turning the line by 90 deg, so it shrank the line to its projection - to nothing for a vertical line, which the
//! guard refused, and to 5 mm of 30 for a line at 80 deg, which nothing refused.
#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use super::super::{App, Sel};
    use qymcad_core::feature::SketchPlane;

    #[test]
    fn a_steep_line_given_horizontal_lies_level_with_its_length() {
        let mut app = App::default();
        let si = app.create_sketch_on(SketchPlane::default());
        app.chosen.sel = Sel::Sketch(si);
        // 30 long at 80 deg: past the auto-constraints' 3.4 deg, so it is drawn with no Vertical on it
        let (dx, dy) = (30.0 * 80f64.to_radians().cos(), 30.0 * 80f64.to_radians().sin());
        Hand::new(&mut app).sk_tool(qymcad_ui_state::SketchShape::Line).click2d(10.0, 0.0).click2d(10.0 + dx, dy).key(egui::Key::Escape);
        let ends = |app: &App| {
            let s = &app.project.sketches[si];
            let (a, b) = s.entities.iter().find_map(|e| if let Some(qymcad_core::model::LineParts { a, b }) = e.kind.as_line() { Some((a, b)) } else { None }).expect("the line");
            let at = |id: u64| s.points.iter().find(|q| q.id == id).map(|q| (q.x, q.y)).expect("an end");
            (at(a), at(b))
        };
        let (a0, b0) = ends(&app);
        let before = (b0.0 - a0.0).hypot(b0.1 - a0.1);
        Hand::new(&mut app).sk_select().click2d((a0.0 + b0.0) / 2.0, (a0.1 + b0.1) / 2.0);
        Hand::new(&mut app).constraint(qymcad_ui_state::ConstraintKind::Horizontal);
        let (a, b) = ends(&app);
        let after = (b.0 - a.0).hypot(b.1 - a.1);
        assert!((a.1 - b.1).abs() < 1e-6 && (after - before).abs() < 1e-3, "given Horizontal, the line {a:?}-{b:?} is {after:.3} long, it was {before:.3}; status: {}", app.status);
    }

    /// The case reported: a vertical line, its Vertical (laid by auto-constraints) deleted by its glyph and Delete, then
    /// given Horizontal - it lies level with its length, not refused as pulled into a point.
    #[test]
    fn a_vertical_line_without_its_vertical_given_horizontal_lies_level() {
        let mut app = App::default();
        let si = app.create_sketch_on(SketchPlane::default());
        app.chosen.sel = Sel::Sketch(si);
        Hand::new(&mut app).sk_tool(qymcad_ui_state::SketchShape::Line).click2d(10.0, 0.0).click2d(10.0, 30.0).key(egui::Key::Escape);
        let vertical = app.project.sketches[si].constraints.iter().position(|c| matches!(c, qymcad_core::model::Constraint::Vertical { .. })).expect("setup: auto-constraints laid Vertical");
        let mut hand = Hand::new(&mut app);
        hand.sk_select();
        let glyph =
            qymcad_pick::constraint_glyphs(&hand.app.pick_ctx(), hand.app.viewing.view_rect, si).into_iter().find(|(ci, _, _)| *ci == vertical).map(|(_, at, _)| at).expect("the glyph of Vertical");
        hand.click_screen(glyph).key(egui::Key::Delete);
        assert!(!hand.app.project.sketches[si].constraints.iter().any(|c| matches!(c, qymcad_core::model::Constraint::Vertical { .. })), "setup: Vertical deleted by its glyph");
        Hand::new(&mut app).sk_select().click2d(10.0, 15.0);
        Hand::new(&mut app).constraint(qymcad_ui_state::ConstraintKind::Horizontal);
        let s = &app.project.sketches[si];
        let (a, b) = s.entities.iter().find_map(|e| if let Some(qymcad_core::model::LineParts { a, b }) = e.kind.as_line() { Some((a, b)) } else { None }).expect("the line");
        let at = |id: u64| s.points.iter().find(|q| q.id == id).map(|q| (q.x, q.y)).expect("an end");
        let (pa, pb) = (at(a), at(b));
        let len = (pb.0 - pa.0).hypot(pb.1 - pa.1);
        assert!((pa.1 - pb.1).abs() < 1e-6 && (len - 30.0).abs() < 1e-3, "given Horizontal, the line {pa:?}-{pb:?} is {len:.3} long; status: {}", app.status);
    }
}
