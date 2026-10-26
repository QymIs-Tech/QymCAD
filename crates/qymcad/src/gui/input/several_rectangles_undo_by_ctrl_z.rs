//! SEVERAL RECTANGLES DRAWN ONE AFTER ANOTHER ARE TAKEN BACK BY CTRL+Z, ONE PRESS PER RECTANGLE.
//!
//! Reported behaviour: rectangles drawn in a sketch, and Ctrl+Z did not undo anything.
#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use super::super::{App, Sel};
    use qymcad_core::feature::SketchPlane;

    /// The sides of every figure of the sketch, as their ends.
    fn sides(app: &App, si: usize) -> Vec<((f64, f64), (f64, f64))> {
        let sk = &app.project.sketches[si];
        let at = |id: u64| sk.points.iter().find(|p| p.id == id).map(|p| (p.x, p.y));
        sk.entities
            .iter()
            .filter_map(|e| match e.kind.as_line() {
                Some(qymcad_core::model::LineParts { a, b }) => Some((at(a)?, at(b)?)),
                _ => None,
            })
            .collect()
    }

    /// THE STEPS OF UNDO THE HISTORY HOLDS, oldest first - said in every failure, so the report shows
    /// whether the steps were laid at all.
    fn history(app: &App) -> Vec<String> {
        app.disk.edits.undo.iter().map(|s| s.name.clone()).collect()
    }

    #[test]
    fn several_rectangles_are_taken_back_one_ctrl_z_at_a_time() {
        let mut app = App::default();
        let si = app.create_sketch_on(SketchPlane::default());
        let mut hand = Hand::new(&mut app);
        hand.sk_tool(qymcad_ui_state::SketchShape::Rect);
        // three rectangles, two clicks each, the next begun while the size box of the last stands
        hand.click2d(10.0, 10.0).click2d(30.0, 20.0);
        hand.click2d(50.0, 0.0).click2d(70.0, 15.0);
        hand.click2d(-40.0, -10.0).click2d(-20.0, -25.0);
        assert_eq!(sides(hand.app, si).len(), 12, "three rectangles stand in the sketch: {:?}", sides(hand.app, si));

        // ONE CTRL+Z PER RECTANGLE
        hand.undo();
        assert_eq!(sides(hand.app, si).len(), 8, "the first Ctrl+Z did not take the last rectangle away; history {:?}", history(hand.app));
        hand.undo();
        assert_eq!(sides(hand.app, si).len(), 4, "the second Ctrl+Z did not take the second rectangle away; history {:?}", history(hand.app));
        hand.undo();
        assert_eq!(sides(hand.app, si).len(), 0, "the third Ctrl+Z did not take the first rectangle away; history {:?}", history(hand.app));
    }

    /// RECTANGLES IN THE SKETCH OF THE FIRST START, REOPENED AS A PERSON REOPENS IT: the document of a first start
    /// holds a sketch of a square (the cube stands on it), and a double click on its row opens it for editing.
    #[test]
    fn rectangles_in_the_reopened_sample_sketch_are_taken_back() {
        let mut app = App::default();
        app.project.cube_sample(); // what a first start meets a person with
        app.disk.edits.committed_key = qymcad_ui_state::doc_key(&app.project);
        assert_eq!(app.project.sketches.len(), 1, "the first start holds the sketch of the cube");
        app.enter_sketch_edit(0);
        let mut hand = Hand::new(&mut app);
        hand.sk_tool(qymcad_ui_state::SketchShape::Rect);
        hand.click2d(40.0, 40.0).click2d(60.0, 55.0);
        hand.click2d(-50.0, 10.0).click2d(-30.0, 25.0);
        assert_eq!(sides(hand.app, 0).len(), 12, "the square of the sample plus two rectangles: {:?}", sides(hand.app, 0));

        hand.undo();
        assert_eq!(sides(hand.app, 0).len(), 8, "the first Ctrl+Z did not take the last rectangle away; history {:?}", history(hand.app));
        hand.undo();
        assert_eq!(sides(hand.app, 0).len(), 4, "the second Ctrl+Z did not take the second rectangle away; history {:?}", history(hand.app));
    }

    /// RECTANGLES SIZED BY TYPING, one after another: the width typed into the box of the shape, Enter laid,
    /// the next rectangle begun while the tool stays in hand - Ctrl+Z takes the last one back.
    #[test]
    fn rectangles_sized_by_typing_are_taken_back_by_ctrl_z() {
        let mut app = App::default();
        let si = app.create_sketch_on(SketchPlane::default());
        let mut hand = Hand::new(&mut app);
        hand.sk_tool(qymcad_ui_state::SketchShape::Rect);
        hand.click2d(10.0, 10.0).click2d(30.0, 20.0);
        // the width typed into the box of the shape and laid with Enter; the tool stays in hand
        hand.frame(vec![egui::Event::Text("40".into())]);
        hand.chord(egui::Modifiers::default(), egui::Key::Enter);
        hand.click2d(50.0, 0.0).click2d(70.0, 15.0);
        assert_eq!(sides(hand.app, si).len(), 8, "two rectangles stand in the sketch: {:?}", sides(hand.app, si));

        hand.undo();
        assert_eq!(sides(hand.app, si).len(), 4, "Ctrl+Z did not take the last rectangle away; history {:?}", history(hand.app));
    }

    /// THE WIDTH TYPED AND THE BOX LEFT WITHOUT ENTER - the next rectangle begun by the click itself. Whatever the
    /// person did with the box, the next Ctrl+Z must take the last rectangle back: an operation left open between
    /// actions would swallow every press of Ctrl+Z without a word.
    #[test]
    fn the_box_left_without_enter_leaves_no_open_operation() {
        let mut app = App::default();
        let si = app.create_sketch_on(SketchPlane::default());
        let mut hand = Hand::new(&mut app);
        hand.sk_tool(qymcad_ui_state::SketchShape::Rect);
        hand.click2d(10.0, 10.0).click2d(30.0, 20.0);
        // the width typed, and the box left by the click that begins the next rectangle
        hand.frame(vec![egui::Event::Text("40".into())]);
        hand.frame(Vec::new());
        hand.click2d(50.0, 0.0).click2d(70.0, 15.0);
        let open = hand.app.disk.edits.open.is_some();
        assert!(!open, "an operation is left open between actions: every Ctrl+Z would be swallowed");
        hand.undo();
        assert_eq!(sides(hand.app, si).len(), 4, "Ctrl+Z did not take the last rectangle away; history {:?}", history(hand.app));
    }

    /// THE VALUE FIELD OF A DIMENSION JUST PLACED, AND CTRL+Z: the field goes with the undo, as the field
    /// of a new shape does, and undo stays alive. Reported behaviour: the status said "Undone: Rectangle"
    /// and the rectangle stayed - the trial of the dimension's value was left open over the document, and
    /// every later press of Ctrl+Z was swallowed by it without a word.
    #[test]
    fn ctrl_z_with_the_dimension_field_open_leaves_undo_alive() {
        let mut app = App::default();
        let si = app.create_sketch_on(SketchPlane::default());
        app.chosen.sel = Sel::Sketch(si);
        let mut hand = Hand::new(&mut app);
        hand.sk_tool(qymcad_ui_state::SketchShape::Rect).click2d(10.0, 10.0).click2d(50.0, 40.0);
        // the dimension placed on the bottom side, its value field open - no Enter, no Escape
        hand.sk_select();
        assert!(hand.press_hint(&crate::i18n::tr("tb-dim-hint")), "the dimension tool");
        hand.click2d(30.0, 10.0);
        assert!(hand.app.tools.place.dim.is_some(), "a dimension being placed");
        hand.click2d(30.0, 2.0);
        hand.frame(vec![egui::Event::Text("20".into())]);
        assert!(hand.app.disk.edits.open.is_some() && hand.app.disk.edits.open_kind == qymcad_ui_state::OpenEdit::Trying, "setup: the value of the dimension holds no trial over the document");

        // CTRL+Z WITH THE FIELD OPEN: the field goes, and the undo stays alive
        hand.undo();
        let open = hand.app.disk.edits.open.is_some();
        assert!(!open, "Ctrl+Z left the trial of the value open over the document: every later press is swallowed");
        // THE NEXT PRESS TAKES THE RECTANGLE BACK
        hand.undo();
        assert_eq!(sides(hand.app, si).len(), 0, "Ctrl+Z did not take the rectangle away; history {:?}", history(hand.app));
    }
}
