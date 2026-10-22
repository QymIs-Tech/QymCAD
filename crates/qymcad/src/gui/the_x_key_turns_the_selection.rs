//! THE KEY X DOES WHAT THE CONSTRUCTION TOGGLE OF THE BAR DOES, through the window: with geometry selected it turns
//! that geometry into construction geometry and leaves the drawing mode alone; with nothing selected it switches the
//! drawing mode.
//!
//! Reported behaviour (#63): X always switched the drawing mode - with a line selected the line stayed as it was, the
//! toggle came up pressed and the next line was drawn as construction geometry.
#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use super::super::{App, Sel};
    use qymcad_core::feature::SketchPlane;

    /// Two lines drawn apart, the select tool taken.
    fn two_lines() -> (App, usize) {
        let mut app = App::default();
        let si = app.create_sketch_on(SketchPlane::default());
        app.chosen.sel = Sel::Sketch(si);
        Hand::new(&mut app).sk_tool(qymcad_ui_state::SketchShape::Line).click2d(0.0, 0.0).click2d(20.0, 0.0).key(egui::Key::Escape).click2d(0.0, 10.0).click2d(20.0, 10.0).key(egui::Key::Escape);
        Hand::new(&mut app).sk_select();
        assert_eq!(construction(&app, si).len(), 2, "setup: two lines");
        (app, si)
    }

    /// Whether each line of the sketch is construction geometry, in the order they were drawn.
    fn construction(app: &App, si: usize) -> Vec<bool> {
        app.project.sketches[si].entities.iter().filter(|e| e.kind.as_line().is_some()).map(|e| e.construction).collect()
    }

    #[test]
    fn x_with_a_line_selected_turns_that_line() {
        let (mut app, si) = two_lines();
        Hand::new(&mut app).sk_select().click2d(10.0, 0.0).key(egui::Key::X);
        assert_eq!(construction(&app, si), vec![true, false], "X did not turn the selected line alone");
        assert!(!app.tools.tool.construction, "X with a line selected switched the drawing mode as well");
        Hand::new(&mut app).key(egui::Key::Escape);
        Hand::new(&mut app).sk_tool(qymcad_ui_state::SketchShape::Line).click2d(0.0, 20.0).click2d(20.0, 20.0).key(egui::Key::Escape);
        assert_eq!(construction(&app, si).last(), Some(&false), "the line drawn after is construction geometry");
    }

    #[test]
    fn x_with_nothing_selected_switches_what_is_drawn_next() {
        let (mut app, si) = two_lines();
        Hand::new(&mut app).sk_select().key(egui::Key::X);
        assert!(app.tools.tool.construction, "X with nothing selected did not switch the drawing mode");
        assert_eq!(construction(&app, si), vec![false, false], "X with nothing selected turned a line");
        Hand::new(&mut app).sk_tool(qymcad_ui_state::SketchShape::Line).click2d(0.0, 20.0).click2d(20.0, 20.0).key(egui::Key::Escape);
        assert_eq!(construction(&app, si).last(), Some(&true), "the line drawn after is not construction geometry");
    }
}
