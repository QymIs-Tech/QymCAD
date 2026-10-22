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

    /// The block with its top front edge in hand for a fillet.
    fn a_fillet_in_hand() -> App {
        let mut app = App::default();
        super::super::joint_flow::tests::add_part_at(&mut app, 0.0);
        qymcad_ui_state::rebuild_if_dirty(&mut app.rebuild_ctx());
        Hand::new(&mut app).look_at([10.0, 10.0, 5.0], 8.0).tool(qymcad_ui_state::PartTool::Fillet).click([10.0, 0.0, 10.0]);
        assert!(!app.tools.gsel.edges.is_empty(), "setup: the edge was not taken; the program says {:?}", app.status);
        app
    }

    /// The trial of the command in hand, as the frame keeps it.
    fn slot(ctx: &egui::Context) -> qymcad_ui_state::TrialSlot {
        ctx.data(|d| d.get_temp::<qymcad_ui_state::TrialSlot>(qymcad_ui_state::trial_slot_id())).expect("no trial is kept for the command in hand")
    }

    /// A TRIAL A NEWER VALUE MAKES OF NO USE IS STOPPED, not left to build for the lock of the kernel it holds. Reported
    /// behaviour: two edges of a propeller clicked in a row and Apply waited for the trials of every state before.
    #[test]
    fn a_new_value_stops_the_trial_of_the_old() {
        let mut app = a_fillet_in_hand();
        let ctx = egui::Context::default();
        Hand::new(&mut app).set("radius", 3.0);
        crate::gui::session::waiting::until("the trial of R3 to start", || {
            let _ = qymcad_part::trial_refusal(&mut app.part_ctx(), &ctx);
            slot(&ctx).run == qymcad_ui_state::TrialRun::Started
        });
        let old = slot(&ctx);
        Hand::new(&mut app).set("radius", 2.0);
        let _ = qymcad_part::trial_refusal(&mut app.part_ctx(), &ctx);
        assert!(old.stop.load(std::sync::atomic::Ordering::Relaxed), "the trial of R3 goes on building after R2 was typed");
        let (_, last) = answer(&mut app, &ctx, 2.0);
        assert_eq!(last, Trial::Clear, "the trial of R2 does not answer after R3 was given up: {last:?}");
    }

    /// VALUES TYPED IN A ROW ARE NOT TRIED ONE BY ONE: a state is tried once it holds, the ones it went through never.
    #[test]
    fn values_typed_in_a_row_are_tried_once() {
        let mut app = a_fillet_in_hand();
        let ctx = egui::Context::default();
        for r in [1.0, 1.5, 1.8] {
            Hand::new(&mut app).set("radius", r);
            assert_eq!(qymcad_part::trial_refusal(&mut app.part_ctx(), &ctx), Trial::Checking, "R{r} typed is not said as being checked");
            assert_eq!(slot(&ctx).run, qymcad_ui_state::TrialRun::Waiting, "R{r}, a value held for no time, is tried at once");
        }
        let (_, last) = answer(&mut app, &ctx, 1.8);
        assert_eq!(last, Trial::Clear, "R1.8 held is not answered: {last:?}");
    }

    /// A GREY APPLY SAYS WHY UNDER THE POINTER while the trial runs: the words the corner of the view says, as the hint of
    /// the button. Reported behaviour: Apply stood grey for minutes and nothing at it said what it waited for.
    #[test]
    fn a_grey_apply_says_it_checks_under_the_pointer() {
        let mut app = a_fillet_in_hand();
        let mut h = Hand::new(&mut app);
        h.set("radius", 3.0);
        h.frame(Vec::new());
        let apply = qymcad_i18n::tr("cmd-apply-enter");
        let at = h.words_drawn().iter().find(|(t, _)| t.contains(&apply)).map(|(_, r)| r.center()).expect("Apply is not on screen");
        let checking = qymcad_i18n::tr("cmd-trial-checking");
        let hints = h.hint_at(at);
        assert!(hints.iter().any(|t| t == &checking), "under the pointer over the grey Apply came {hints:?}, not that the value is being checked");
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
