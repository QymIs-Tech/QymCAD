//! RENAMING STARTS WITH F2, not only from the right-click menu.
//!
//! Reported behaviour: "renaming parts/subassemblies/sketches/features not only by right-click -> Rename
//! but also by pressing F2."
//!
//! F2 is the key every tree a person has ever used renames by, and it was the one thing the tree did not
//! answer. The menu item existed for five different kinds of node, each starting the rename its own way -
//! a component through `RenameNode::Component`, a body through `RenameNode::Body`, a sketch through
//! `rename.sketch`, a feature through `rename.target` - so the key needed one door rather than a sixth
//! copy of that fan-out. That door is `rename_selected`.
//!
//! PRESSED IN A REAL FRAME. A key that reaches the program only through a direct call to its handler
//! proves nothing about a person pressing it: the frame decides whether the key is swallowed by a focused
//! field, by an open list or by the system dialogue before any handler sees it.
#[cfg(test)]
mod tests {
    use crate::gui::{App, Sel};

    const SCREEN: egui::Vec2 = egui::vec2(1400.0, 900.0);

    /// One whole frame with `events` delivered into it.
    fn frame(app: &mut App, ctx: &egui::Context, events: Vec<egui::Event>) {
        let raw = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), SCREEN)), events, ..Default::default() };
        let _ = ctx.run_ui(raw, |ui| app.draw_frame(ui));
    }

    fn press(key: egui::Key) -> Vec<egui::Event> {
        vec![
            egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() },
            egui::Event::Key { key, physical_key: None, pressed: false, repeat: false, modifiers: Default::default() },
        ]
    }

    /// A part with a sketch and a body built on it, past the splash, with frames already running.
    fn a_part_with_something_to_rename() -> (App, egui::Context, usize) {
        let mut app = App::default();
        app.set.gpu_viewport = false;
        let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        app.project.add_rect_entity(si, 0.0, 0.0, 60.0, 40.0, qymcad_core::feature::Purpose::Real);
        app.project.regen_sketch(si);
        app.finish_sketch_edit();
        app.chosen.sel = Sel::Sketch(si);
        app.start_feat_cmd(1);
        app.apply_feat_cmd();

        let ctx = egui::Context::default();
        crate::gui::install_fonts(&ctx);
        app.waiting.splash_until = None;
        for _ in 0..3 {
            frame(&mut app, &ctx, Vec::new());
        }
        (app, ctx, si)
    }

    /// F2 OVER A SELECTED SKETCH OPENS ITS NAME FOR EDITING.
    #[test]
    fn f2_renames_the_selected_sketch() {
        let (mut app, ctx, si) = a_part_with_something_to_rename();
        let sid = app.project.sketches[si].id;
        app.chosen.sel = Sel::Sketch(si);

        frame(&mut app, &ctx, press(egui::Key::F2));

        assert_eq!(app.side.rename.sketch, Some(sid), "F2 is the key every tree renames by, and this one does not answer it");
        assert!(!app.side.rename.buf.is_empty(), "and the field must open on the name that is there, not empty");
    }

    /// F2 OVER A SELECTED FEATURE OPENS ITS NAME.
    ///
    /// A feature is renamed through a different field than a sketch, so one of the two passing says nothing
    /// about the other.
    #[test]
    fn f2_renames_the_selected_feature() {
        let (mut app, ctx, _) = a_part_with_something_to_rename();
        let fi = app.project.timeline.iter().position(|n| !matches!(n.kind, qymcad_core::feature::FeatureKind::Sketch { .. })).expect("a feature in the timeline");
        let fid = app.project.timeline[fi].id;
        app.chosen.sel = Sel::Feature(fi);

        frame(&mut app, &ctx, press(egui::Key::F2));

        assert_eq!(app.side.rename.target, Some(fid), "a feature is renamed through its own field, and F2 does not reach it");
    }

    /// WITH NOTHING SELECTED F2 DOES NOTHING.
    ///
    /// Without this the fix could be "always start renaming something", which would put a field on screen
    /// over whatever the last selection happened to be.
    #[test]
    fn f2_with_nothing_selected_starts_no_rename() {
        let (mut app, ctx, _) = a_part_with_something_to_rename();
        app.chosen.sel = Sel::None;

        frame(&mut app, &ctx, press(egui::Key::F2));

        assert!(app.side.rename.sketch.is_none() && app.side.rename.target.is_none() && app.side.rename.node.is_none(), "nothing is selected, so there is nothing to rename");
    }

    /// F2 PRESSED AGAIN DOES NOT WIPE THE NAME BEING TYPED.
    ///
    /// The first edition of this check leaned on the keyboard focus - the guard every other key in the
    /// frame uses - and could not be built: in a headless frame the tree row that carries the field is not
    /// drawn, so nothing ever took the focus and the check measured a scene that did not exist. The GUARD
    /// said so instead of the check passing over it.
    ///
    /// So the property is held where it belongs: starting a rename that is ALREADY under way on the same
    /// node leaves what has been typed. That holds however the second F2 arrives - focus or no focus, from
    /// the key or from the menu item beside it.
    #[test]
    fn f2_pressed_again_keeps_the_name_being_typed() {
        let (mut app, ctx, si) = a_part_with_something_to_rename();
        app.chosen.sel = Sel::Sketch(si);
        frame(&mut app, &ctx, press(egui::Key::F2));
        app.side.rename.buf = "the name being typed".into();

        frame(&mut app, &ctx, press(egui::Key::F2));

        assert_eq!(app.side.rename.buf, "the name being typed", "F2 pressed again wiped what was typed and started over");
    }

    /// BUT F2 ON A DIFFERENT NODE DOES START AFRESH.
    ///
    /// Without this, "leave the buffer alone" could be implemented as "never touch the buffer again", and
    /// renaming the next node would open on the previous node's name.
    #[test]
    fn f2_on_another_node_opens_on_that_node_s_name() {
        let (mut app, ctx, si) = a_part_with_something_to_rename();
        app.chosen.sel = Sel::Sketch(si);
        frame(&mut app, &ctx, press(egui::Key::F2));
        app.side.rename.buf = "half-typed".into();

        let fi = app.project.timeline.iter().position(|n| !matches!(n.kind, qymcad_core::feature::FeatureKind::Sketch { .. })).expect("a feature in the timeline");
        app.chosen.sel = Sel::Feature(fi);
        frame(&mut app, &ctx, press(egui::Key::F2));

        assert!(app.side.rename.sketch.is_none(), "the sketch is no longer the one being renamed");
        assert_ne!(app.side.rename.buf, "half-typed", "the field opened on the previous node's half-typed name");
    }
}
