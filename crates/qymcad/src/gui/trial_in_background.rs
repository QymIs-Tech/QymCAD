//! THE TRIAL BUILD RUNS ON A WORKER: the frame that starts it does not wait for the kernel - it says the value is being
//! checked - and the answer comes on a later frame, a good radius clear and one the block cannot take refused. Reported
//! behaviour: on a heavy part the trial stood the window still for as long as the kernel took.
#[cfg(test)]
mod tests {
    use super::super::App;
    use crate::gui::hand::Hand;
    use qymcad_part::Trial;

    fn answer(app: &mut App, ctx: &egui::Context, radius: f64) -> (Trial, Trial) {
        Hand::new(app).set("radius", radius);
        let first = qymcad_part::trial_refusal(&mut app.part_ctx(), ctx);
        let mut last = first.clone();
        // the worker waits for the kernel's lock, which the other checks of a full run hold in turn
        crate::gui::session::waiting::until("the trial of the fillet to answer", || {
            last = qymcad_part::trial_refusal(&mut app.part_ctx(), ctx);
            last != Trial::Checking
        });
        (first, last)
    }

    #[test]
    fn the_trial_answers_on_a_later_frame() {
        let mut app = App::default();
        super::super::joint_flow::tests::add_part_at(&mut app, 0.0);
        qymcad_ui_state::rebuild_if_dirty(&mut app.rebuild_ctx());
        Hand::new(&mut app).look_at([10.0, 10.0, 5.0], 8.0).tool(qymcad_ui_state::PartTool::Fillet).click([10.0, 0.0, 10.0]);
        assert!(!app.tools.gsel.edges.is_empty(), "setup: the edge was not taken; the program says {:?}", app.status);
        let ctx = egui::Context::default();
        let (first, last) = answer(&mut app, &ctx, 3.0);
        assert_eq!(first, Trial::Checking, "the frame that starts the trial waited for the kernel instead of saying it checks");
        assert_eq!(last, Trial::Clear, "a fillet of 3 on a block 10 thick is refused: {last:?}");
        // THE PREVIEW IS THE BUILT SURFACE: the faces the fillet adds, and only there - along the top front edge (y 0,
        // z 10), within its radius of it
        let faces = qymcad_ui_state::trial_faces(&ctx).expect("the trial built the fillet and gives no faces to show");
        let off = faces.iter().flatten().map(|v| (v[1].powi(2) + (v[2] - 10.0).powi(2)).sqrt()).fold(0.0, f64::max);
        assert!(off <= 3.0 + 1e-6, "the faces shown stand {off:.2} mm from the edge, past the radius of 3: not the fillet alone");
        let (first, last) = answer(&mut app, &ctx, 96.0);
        assert_eq!(first, Trial::Checking, "a new value is not checked afresh on the worker");
        assert!(matches!(last, Trial::Refused(..)), "a fillet of 96 on a block 10 thick is taken: {last:?}");
        assert!(qymcad_ui_state::trial_faces(&ctx).is_none(), "a refused fillet still shows faces");
    }

    /// THE PREVIEW OF A FILLET, SEEN: the block with R3 on its top front edge before Enter, into
    /// `target/look/trial-preview.png`.
    #[test]
    #[ignore = "a picture to look at"]
    fn the_trial_preview_is_looked_at() {
        let mut app = App::default();
        super::super::joint_flow::tests::add_part_at(&mut app, 0.0);
        qymcad_ui_state::rebuild_if_dirty(&mut app.rebuild_ctx());
        Hand::new(&mut app).look_at([10.0, 10.0, 5.0], 8.0).tool(qymcad_ui_state::PartTool::Fillet).click([10.0, 0.0, 10.0]);
        let ctx = egui::Context::default();
        let _ = answer(&mut app, &ctx, 3.0);
        app.viewing.mode_3d = true;
        let dir = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/look"));
        std::fs::create_dir_all(&dir).expect("a folder for the picture");
        let bg = app.scheme.pal.viewport_bg();
        let slot = ctx.data(|d| d.get_temp::<qymcad_ui_state::TrialSlot>(qymcad_ui_state::trial_slot_id()));
        let img = crate::gui::help_raster::shot_ui([1200, 800], bg, |ui| {
            if let Some(s) = slot.clone() {
                ui.ctx().data_mut(|d| d.insert_temp(qymcad_ui_state::trial_slot_id(), s));
            }
            let ctx = &ui.ctx().clone();
            crate::gui::apply_theme(&mut app.scheme, &app.set, ctx);
            let shell = crate::gui::shell(&app.set);
            for slot in qymcad_shell::Slot::ORDER {
                shell.run_slot(slot, ui, &mut app);
            }
        });
        std::fs::write(dir.join("trial-preview.png"), crate::gui::color_image_to_png(&img).expect("PNG")).expect("writing");
    }
}
