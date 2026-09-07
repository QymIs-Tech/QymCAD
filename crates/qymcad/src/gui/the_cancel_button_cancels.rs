//! THE CANCEL BUTTON ON A TOOL'S POPUP MUST DO WHAT ESCAPE DOES.
//!
//! Reported behaviour: the popup of a feature tool carries a button beside Apply, and pressing it does
//! nothing - the popup stays open, the command stays running.
//!
//! The button was drawn with an EMPTY BODY. The click was read and thrown away under a comment saying the
//! cancel is handled below; below there was only the apply. A button that answers a click by doing nothing
//! is worse than no button: it says the way out is here, and it is not.
//!
//! Checked both ways round in one go: the key must close the command, and the button must close it the same
//! way. A check that only pressed the button would be just as green for a program where nothing closes
//! anything at all.
#[cfg(test)]
mod tests {
    use super::super::App;

    const RECT: egui::Rect = egui::Rect { min: egui::pos2(0.0, 0.0), max: egui::pos2(1400.0, 900.0) };

    /// A part with a sketch and an extrude already open - the state in which the popup is on screen.
    fn extruding() -> App {
        let mut app = super::super::screen_keys::tests::plate();
        let part = app.project.components.iter().rev().find(|c| c.parent.is_some()).map(|c| c.id).expect("the part");
        app.enter_component(part);
        app.chosen.sel = super::super::Sel::Sketch(0);
        app.viewing.mode_3d = true;
        app.start_feat_cmd(1);
        assert!(app.tools.armed.commanding(), "setup: the extrude did not open - there is nothing to check");
        app
    }

    /// One frame in the production order: the keys are handled first, the popup is drawn after. Returns
    /// every painted label with the place it was painted in.
    fn frame(app: &mut App, ctx: &egui::Context, events: Vec<egui::Event>) -> Vec<(String, egui::Rect)> {
        fn walk(s: &egui::epaint::Shape, out: &mut Vec<(String, egui::Rect)>) {
            match s {
                egui::epaint::Shape::Text(t) => {
                    // the phosphor icons live in the private-use block and are not part of a label
                    let text: String = t.galley.text().chars().filter(|c| !('\u{e000}'..='\u{f8ff}').contains(c)).collect();
                    out.push((text.trim().to_string(), egui::Rect::from_min_size(t.pos, t.galley.size())));
                }
                egui::epaint::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
                _ => {}
            }
        }
        let input = egui::RawInput { screen_rect: Some(RECT), events, ..Default::default() };
        let out = ctx.run_ui(input, |ui| {
            let ctx = &ui.ctx().clone();
            app.handle_key_commands(ctx);
            crate::gui::commands::feat_cmd_popup(&mut app.part_ctx(), ctx, RECT);
        });
        let mut texts = Vec::new();
        for cs in &out.shapes {
            walk(&cs.shape, &mut texts);
        }
        texts
    }

    fn click(spot: egui::Pos2) -> Vec<egui::Event> {
        vec![
            egui::Event::PointerMoved(spot),
            egui::Event::PointerButton { pos: spot, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() },
            egui::Event::PointerButton { pos: spot, button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() },
        ]
    }

    fn cancel_label() -> String {
        crate::i18n::tr("cmd-cancel-esc")
    }

    /// THE KEY CLOSES THE COMMAND. Without this the check below would pass against a program in which
    /// nothing closes anything.
    #[test]
    fn the_key_closes_the_command() {
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        let mut app = extruding();
        let _ = frame(&mut app, &ctx, vec![]);
        let key = |pressed| egui::Event::Key { key: egui::Key::Escape, physical_key: None, pressed, repeat: false, modifiers: Default::default() };
        let _ = frame(&mut app, &ctx, vec![key(true), key(false)]);
        assert!(!app.tools.armed.commanding(), "setup: Escape did not close the command, so the button cannot be compared with it");
    }

    /// THE BUTTON CLOSES IT THE SAME WAY. Exactly the reported complaint.
    #[test]
    fn the_button_closes_the_command_as_the_key_does() {
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        let mut app = extruding();

        // An egui area is placed on the frame after the one that first asked for it, so the popup is
        // painted from the second frame on. Looking at the first would find an empty screen.
        let _ = frame(&mut app, &ctx, vec![]);
        let painted = frame(&mut app, &ctx, vec![]);
        let label = cancel_label();
        let spot = painted
            .iter()
            .find(|(t, _)| *t == label)
            .map(|(_, r)| r.center())
            .unwrap_or_else(|| panic!("the popup has no `{label}` button; painted: {:?}", painted.iter().map(|(t, _)| t).collect::<Vec<_>>()));

        let _ = frame(&mut app, &ctx, click(spot));
        assert!(!app.tools.armed.commanding(), "the cancel button was pressed and the command is still running");
    }
}
