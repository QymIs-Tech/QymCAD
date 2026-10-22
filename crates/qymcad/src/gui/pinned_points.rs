//! A PIN HOLDS THE POINT, AND THE POINT DOES NOT TRAVEL.
//!
//! `Constraint::Fixed` carries no coordinates: it pins the point to where it IS at the moment of the solve.
//! Every door that moved the point first and asked the solver afterwards therefore carried the pin along -
//! the drawing moved, the constraint came out satisfied by the new place, and the glyph stayed green. The
//! two doors a person uses are the mouse and the move tool, and both are driven here.
#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use super::super::{App, Sel};
    use qymcad_core::feature::SketchPlane;
    use qymcad_core::model::Constraint;

    /// A line drawn by hand from (10,0) to (30,0), with its left end pinned.
    fn a_pinned_line() -> (App, usize, u64) {
        let mut app = App::default();
        let si = app.create_sketch_on(SketchPlane::default());
        app.chosen.sel = Sel::Sketch(si);
        Hand::new(&mut app).sk_tool(qymcad_ui_state::SketchShape::Line).click2d(10.0, 0.0).click2d(30.0, 0.0);
        let (a, _) = app.project.sketches[si]
            .entities
            .iter()
            .find_map(|e| match e.kind.as_line() {
                Some(qymcad_core::model::LineParts { a, b }) => Some((a, b)),
                _ => None,
            })
            .expect("setup: the line was not drawn");
        app.project.sketches[si].constraints.push(Constraint::Fixed { p: a });
        (app, si, a)
    }

    fn at(app: &App, si: usize, id: u64) -> (f64, f64) {
        let p = app.project.sketches[si].points.iter().find(|p| p.id == id).expect("the point is still there");
        (p.x, p.y)
    }

    /// THE MOUSE DOES NOT PICK UP A PINNED POINT AT ALL.
    ///
    /// Not "picks it up and puts it back": the drag used to carry the point to the cursor and the pin with
    /// it, so a pinned point moved as freely as any other and said nothing.
    #[test]
    fn the_mouse_does_not_drag_a_pinned_point() {
        let (mut app, si, pinned) = a_pinned_line();
        Hand::canvas(&mut app).drag2d((10.0, 0.0), (40.0, 20.0));
        app.project.regen_sketch(si);
        let (x, y) = at(&app, si, pinned);
        assert!((x - 10.0).abs() < 1e-6 && y.abs() < 1e-6, "the pinned point was dragged to ({x}, {y})");
    }

    /// AND NEITHER DOES THE MOVE TOOL, used the way a person uses it: pick the shape, the base point, the
    /// target.
    #[test]
    fn the_move_tool_does_not_carry_a_pinned_point_along() {
        let (mut app, si, pinned) = a_pinned_line();
        Hand::new(&mut app).sk_move(qymcad_ui_state::MoveKind::Move, (20.0, 0.0), (20.0, 0.0), (24.0, 4.0));
        app.project.regen_sketch(si);
        let (x, y) = at(&app, si, pinned);
        assert!((x - 10.0).abs() < 1e-6 && y.abs() < 1e-6, "the move tool carried the pinned point to ({x}, {y})");
    }
}
