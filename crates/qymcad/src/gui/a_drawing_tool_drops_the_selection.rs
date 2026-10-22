//! A DRAWING TOOL DROPS THE SELECTION: a line selected, then any drawing tool taken - by its button or by its key - and
//! nothing stays selected, neither when the tool is taken nor after a shape is drawn with it. The arrow of selection
//! keeps what is selected.
//!
//! Reported behaviour: a line selected, the circle taken from the panel and drawn, and the line stood selected the whole
//! time and after.
#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use crate::gui::import_door::tests::running;
    use qymcad_core::feature::SketchPlane;

    /// A sketch with a line drawn, open for editing, and the line's id.
    fn a_line(hand: &mut Hand) -> u64 {
        hand.sk_tool(qymcad_ui_state::SketchShape::Line).click2d(0.0, 0.0).click2d(30.0, 0.0).key(egui::Key::Escape).key(egui::Key::Escape);
        hand.sk_select();
        hand.app.project.sketches[0].entities[0].id
    }

    #[test]
    fn every_drawing_tool_drops_the_selection() {
        let mut failures = Vec::new();
        // by its button: line, rectangle, circle, arc, point, polygon, slot, ellipse, spline, circle by three points, text
        for kind in qymcad_ui_state::SketchShape::ALL {
            let (mut app, _ctx) = running();
            let si = app.create_sketch_on(SketchPlane::default());
            let mut hand = Hand::new(&mut app);
            hand.app.enter_sketch_edit(si);
            let line = a_line(&mut hand);
            assert!(hand.select2d(&[qymcad_ui_state::SketchItem::curve(line)]), "the line could not be picked");
            assert!(!hand.app.tools.sel_sk.items.is_empty(), "GUARD: the line is selected before the tool is taken");
            hand.sk_tool(kind);
            if !hand.app.tools.sel_sk.items.is_empty() {
                failures.push(format!("the drawing tool {kind:?} taken by its button left {:?} selected", hand.app.tools.sel_sk.items));
            }
        }
        // by its key, and a shape drawn after it
        for (key, name) in [(egui::Key::L, "line"), (egui::Key::R, "rectangle"), (egui::Key::C, "circle")] {
            let (mut app, _ctx) = running();
            let si = app.create_sketch_on(SketchPlane::default());
            let mut hand = Hand::new(&mut app);
            hand.app.enter_sketch_edit(si);
            let line = a_line(&mut hand);
            assert!(hand.select2d(&[qymcad_ui_state::SketchItem::curve(line)]), "the line could not be picked");
            hand.key(key);
            if hand.app.tools.sel_sk.items.contains(&qymcad_ui_state::SketchItem::curve(line)) {
                failures.push(format!("the {name} taken by its key left the line selected"));
            }
            hand.click2d(50.0, 10.0).click2d(60.0, 20.0).key(egui::Key::Escape);
            if hand.app.tools.sel_sk.items.contains(&qymcad_ui_state::SketchItem::curve(line)) {
                failures.push(format!("after a {name} drawn the line is selected again"));
            }
        }
        assert!(failures.is_empty(), "a drawing tool kept the selection made before it:\n{}", failures.join("\n"));
    }

    /// Two lines and a point selected together with Shift, then the circle taken by its button and the line by its key:
    /// nothing of the three stays selected.
    #[test]
    fn several_shapes_and_a_point_are_dropped_together() {
        let mut failures = Vec::new();
        for (by, take) in [("button", None), ("key", Some(egui::Key::L))] {
            let (mut app, _ctx) = running();
            let si = app.create_sketch_on(SketchPlane::default());
            let mut hand = Hand::new(&mut app);
            hand.app.enter_sketch_edit(si);
            let first = a_line(&mut hand);
            hand.sk_tool(qymcad_ui_state::SketchShape::Line).click2d(0.0, 20.0).click2d(30.0, 20.0).key(egui::Key::Escape).key(egui::Key::Escape);
            hand.sk_select();
            let sk = &hand.app.project.sketches[0];
            let second = sk.entities[1].id;
            let point = sk.points.iter().find(|p| p.x.abs() > 1.0 && p.y.abs() < 1e-6).expect("the far end of the first line").id;
            let picked = [qymcad_ui_state::SketchItem::curve(first), qymcad_ui_state::SketchItem::curve(second), qymcad_ui_state::SketchItem::point(point)];
            assert!(hand.select2d(&picked), "the two lines and the point could not be picked");
            assert_eq!(hand.app.tools.sel_sk.items.len(), 3, "GUARD: three things are selected before the tool is taken");
            match take {
                Some(key) => {
                    hand.key(key);
                }
                None => {
                    hand.sk_tool(qymcad_ui_state::SketchShape::Circle);
                }
            }
            if !hand.app.tools.sel_sk.items.is_empty() {
                failures.push(format!("a tool taken by its {by} left {:?} selected", hand.app.tools.sel_sk.items));
            }
        }
        assert!(failures.is_empty(), "a drawing tool kept a selection of several things:\n{}", failures.join("\n"));
    }

    #[test]
    fn the_arrow_of_selection_keeps_the_selection() {
        let (mut app, _ctx) = running();
        let si = app.create_sketch_on(SketchPlane::default());
        let mut hand = Hand::new(&mut app);
        hand.app.enter_sketch_edit(si);
        let line = a_line(&mut hand);
        assert!(hand.select2d(&[qymcad_ui_state::SketchItem::curve(line)]), "the line could not be picked");
        hand.sk_select();
        assert!(hand.app.tools.sel_sk.items.contains(&qymcad_ui_state::SketchItem::curve(line)), "the arrow of selection dropped the line selected");
    }
}
