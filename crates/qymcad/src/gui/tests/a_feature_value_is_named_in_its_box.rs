//! A NUMBER OF A FEATURE IS NAMED IN THE BOX OF ITS COMMAND, as a dimension of a sketch is: a row "driver:" under each
//! field, the name typed there given to the number as the command writes it.
//!
//! Reported behaviour: the boxes of the operations of a part had no driver field; a number of a feature could be named
//! only in the properties of the feature after it was made.
#[cfg(test)]
mod tests {
    use crate::gui::App;

    const RECT: egui::Rect = egui::Rect { min: egui::pos2(0.0, 0.0), max: egui::pos2(1400.0, 900.0) };

    /// A part with a sketch and an extrude open in 3D - its box at the geometry on screen.
    fn extruding() -> App {
        let mut app = crate::gui::screen_keys::tests::plate();
        let part = app.project.components.iter().rev().find(|c| c.parent.is_some()).map(|c| c.id).expect("the part");
        app.enter_component(part);
        app.chosen.sel = crate::gui::Sel::Sketch(0);
        app.viewing.mode_3d = true;
        app.start_feat_cmd(qymcad_ui_state::PartTool::Extrude);
        assert!(app.tools.armed.commanding(), "setup: the extrude did not open - there is nothing to check");
        app
    }

    /// One frame in the production order - the keys first, the tool after - and the words it painted.
    fn frame(app: &mut App, ctx: &egui::Context, events: Vec<egui::Event>) -> Vec<String> {
        frame_held(app, ctx, events, egui::Modifiers::default())
    }

    /// The same frame with `modifiers` held down: egui reads them from the input of the frame, not from a key event.
    fn frame_held(app: &mut App, ctx: &egui::Context, events: Vec<egui::Event>, modifiers: egui::Modifiers) -> Vec<String> {
        let input = egui::RawInput { screen_rect: Some(RECT), events, modifiers, ..Default::default() };
        let out = ctx.run_ui(input, |ui| {
            let ctx = &ui.ctx().clone();
            app.handle_key_commands(ctx);
            crate::gui::panel_places::part_tool(app, ui);
        });
        let mut drawn = Vec::new();
        for cs in &out.shapes {
            crate::gui::screen_keys::tests::collect_text(&cs.shape, &mut drawn);
        }
        drawn
    }

    /// A key pressed and let go in two frames.
    fn key(app: &mut App, ctx: &egui::Context, k: egui::Key) {
        let ev = |pressed| egui::Event::Key { key: k, physical_key: None, pressed, repeat: false, modifiers: Default::default() };
        frame(app, ctx, vec![ev(true)]);
        frame(app, ctx, vec![ev(false)]);
    }

    /// A key pressed with `modifiers` and let go in two frames.
    fn chord(app: &mut App, ctx: &egui::Context, k: egui::Key, modifiers: egui::Modifiers) {
        let ev = |pressed| egui::Event::Key { key: k, physical_key: None, pressed, repeat: false, modifiers };
        frame_held(app, ctx, vec![ev(true)], modifiers);
        frame_held(app, ctx, vec![ev(false)], modifiers);
    }

    /// The extrude applied with its height named `vysota` in the row of its box; answers the context the frames ran in.
    fn named_and_applied(app: &mut App) -> egui::Context {
        let ctx = egui::Context::default();
        crate::gui::install_fonts(&ctx);
        let mut drawn = Vec::new();
        for _ in 0..4 {
            drawn = frame(app, &ctx, Vec::new());
        }
        let label = crate::i18n::tr("sk-driver-label");
        assert!(drawn.iter().any(|t| t.trim() == label.trim()), "the box of the extrude has no {label:?} row; it says {drawn:?}");
        let field = ctx.read_response(qymcad_ui_state::name_field_id("height")).map(|r| r.rect.center()).expect("the name field of the height is not drawn");
        let button = |pressed| egui::Event::PointerButton { pos: field, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        frame(app, &ctx, vec![egui::Event::PointerMoved(field)]);
        frame(app, &ctx, vec![button(true)]);
        frame(app, &ctx, vec![button(false)]);
        frame(app, &ctx, vec![egui::Event::Text("vysota".into())]);
        key(app, &ctx, egui::Key::Enter); // the name is taken, the field lets the keyboard go
        key(app, &ctx, egui::Key::Enter); // the command applies
        ctx
    }

    #[test]
    fn the_height_of_an_extrude_is_named_in_its_box() {
        let mut app = extruding();
        named_and_applied(&mut app);
        let node = app.project.timeline.last().map(|n| n.id).expect("the extrude node");
        let named = app.project.name_of_target(&qymcad_core::model::DimTarget::Feature { node, key: "height".into() });
        assert_eq!(named, "vysota", "the height of the extrude was not named by the row of its box");
    }

    /// THE NAME IS PART OF THE STEP OF THE COMMAND: undone and done again, the extrude comes back named. Reported
    /// behaviour: the name was laid after the step of the command closed, so redo brought the node back without it and a
    /// parameter reading the name read 0.
    #[test]
    fn the_name_of_a_height_comes_back_with_redo() {
        let mut app = extruding();
        let ctx = named_and_applied(&mut app);
        let node = app.project.timeline.last().map(|n| n.id).expect("the extrude node");
        let target = qymcad_core::model::DimTarget::Feature { node, key: "height".into() };
        assert_eq!(app.project.name_of_target(&target), "vysota", "setup: the height was not named");
        chord(&mut app, &ctx, egui::Key::Z, egui::Modifiers::COMMAND);
        assert!(app.project.timeline.iter().all(|n| n.id != node), "undo did not take the extrude back");
        chord(&mut app, &ctx, egui::Key::Z, egui::Modifiers::COMMAND | egui::Modifiers::SHIFT);
        assert!(app.project.timeline.iter().any(|n| n.id == node), "redo did not bring the extrude back");
        assert_eq!(app.project.name_of_target(&target), "vysota", "redo brought the extrude back without the name of its height");
    }
}
