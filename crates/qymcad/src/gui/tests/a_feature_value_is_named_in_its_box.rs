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
        let input = egui::RawInput { screen_rect: Some(RECT), events, ..Default::default() };
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

    #[test]
    fn the_height_of_an_extrude_is_named_in_its_box() {
        let mut app = extruding();
        let ctx = egui::Context::default();
        crate::gui::install_fonts(&ctx);
        let mut drawn = Vec::new();
        for _ in 0..4 {
            drawn = frame(&mut app, &ctx, Vec::new());
        }
        let label = crate::i18n::tr("sk-driver-label");
        assert!(drawn.iter().any(|t| t.trim() == label.trim()), "the box of the extrude has no {label:?} row; it says {drawn:?}");
        let at = app.tools.cmd.params.iter().position(|p| p.key == "height").expect("the height field");
        let field = ctx
            .read_response(qymcad_ui_state::text_field_id(qymcad_ui_state::OptKey::at(qymcad_ui_state::FIELD_NAME, at)))
            .map(|r| r.rect.center())
            .expect("the name field of the height is not drawn");
        let button = |pressed| egui::Event::PointerButton { pos: field, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(field)]);
        frame(&mut app, &ctx, vec![button(true)]);
        frame(&mut app, &ctx, vec![button(false)]);
        frame(&mut app, &ctx, vec![egui::Event::Text("vysota".into())]);
        key(&mut app, &ctx, egui::Key::Enter); // the name is taken, the field lets the keyboard go
        key(&mut app, &ctx, egui::Key::Enter); // the command applies
        let node = app.project.timeline.last().map(|n| n.id).expect("the extrude node");
        let named = app.project.name_of_target(&qymcad_core::model::DimTarget::Feature { node, key: "height".into() });
        assert_eq!(named, "vysota", "the height of the extrude was not named by the row of its box");
    }
}
