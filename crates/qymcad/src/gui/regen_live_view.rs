//! A REBUILD IS NOT OBLIGED TO PUT OUT THE WHOLE SCREEN.
//!
//! Reported behaviour: do an operation and a rebuild pops up with a modal window, so sit and wait. The
//! frame was drawn all the while (the scene stayed in place), but the input was muted across the WHOLE
//! screen: the barrier covered the panels and the canvas alike. There is one reason for the ban — an
//! edit would land on a stale copy of the document. But turning the view, zooming and selecting DO NOT
//! change the document: the `regen_doc_stamp` fingerprint is taken from the model, and neither the
//! camera nor the selection is part of it. So there was no reason to lock them.
//!
//! What is checked here is exactly that boundary: during a rebuild the canvas is alive and everything
//! around it is not.
#[cfg(test)]
mod tests {
    use super::super::App;

    /// A frame with two buttons: one inside the "canvas" and one outside it. Returns how far the click
    /// got.
    ///
    /// The click is spread over frames, as live input is (see `modal_barrier`): egui counts a press on
    /// the frame of the RELEASE, and gluing it all into one frame means checking the wrong thing.
    fn clicks_through(live: egui::Rect) -> (bool, bool) {
        let screen = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(800.0, 600.0));
        let app = App::default();
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);

        let inside = std::cell::Cell::new(false);
        let outside = std::cell::Cell::new(false);
        // the "canvas" is the lower two thirds of the screen and the "panel" is the top strip: that
        // is how a live window stands too
        let canvas = egui::Rect::from_min_max(egui::pos2(0.0, 200.0), egui::pos2(800.0, 600.0));
        let at_in = canvas.center();
        let at_out = egui::pos2(400.0, 100.0);
        let draw = |ui: &mut egui::Ui| {
            let ctx = &ui.ctx().clone();
            let put = |id: &str, r: egui::Rect, flag: &std::cell::Cell<bool>| {
                egui::Area::new(egui::Id::new(id)).fixed_pos(r.min).show(ctx, |ui| {
                    if ui.add_sized(r.size(), egui::Button::new(id)).clicked() {
                        flag.set(true);
                    }
                });
            };
            put("inside the canvas", egui::Rect::from_center_size(at_in, egui::vec2(60.0, 24.0)), &inside);
            put("on the panel", egui::Rect::from_center_size(at_out, egui::vec2(60.0, 24.0)), &outside);
            app.draw_regen_overlay_over(ctx, live);
        };
        let frame = |events: Vec<egui::Event>| egui::RawInput { screen_rect: Some(screen), events, ..Default::default() };
        let click = |at: egui::Pos2| {
            let _ = ctx.run_ui(frame(vec![egui::Event::PointerMoved(at)]), &draw);
            let press = egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() };
            let _ = ctx.run_ui(frame(vec![press]), &draw);
            let release = egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() };
            let _ = ctx.run_ui(frame(vec![release]), &draw);
        };
        let _ = ctx.run_ui(frame(vec![]), &draw); // lay out
        click(at_in);
        click(at_out);
        (inside.get(), outside.get())
    }

    /// SETUP: a full-screen barrier (as during loading) lets nothing through — otherwise the test
    /// below is green for nothing.
    #[test]
    fn a_full_screen_barrier_swallows_everything() {
        let (inside, outside) = clicks_through(egui::Rect::NOTHING);
        assert!(!inside && !outside, "a barrier with no window must mute the whole screen, and through went: canvas={inside}, panel={outside}");
    }

    /// THE CANVAS IS ALIVE, THE PANELS ARE NOT.
    #[test]
    fn during_a_rebuild_the_canvas_takes_input_and_the_panels_do_not() {
        let canvas = egui::Rect::from_min_max(egui::pos2(0.0, 200.0), egui::pos2(800.0, 600.0));
        let (inside, outside) = clicks_through(canvas);
        assert!(inside, "during a rebuild the canvas must accept input: turning the view and selecting do not change the document");
        assert!(!outside, "during a rebuild the panels must stay silent: an edit from there would land on a stale copy");
    }

    /// A frame is NOT aborted while a rebuild runs: the scene is drawn and the window lives.
    #[test]
    fn the_frame_is_not_aborted_while_a_rebuild_runs() {
        let mut app = App::default();
        super::super::joint_flow::tests::add_part_at(&mut app, 0.0);
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        // A REBUILD THAT IS STILL RUNNING: a real one on a little cube finishes inside the frame, and
        // there would be nothing to check. The channel is held open — the worker is "computing".
        let (_tx, rx) = std::sync::mpsc::channel();
        app.waiting.splash_until = None; // the startup splash is a separate case, it mutes the frame legitimately
        app.regen.busy = Some(super::super::Busy { label: "rebuild".into(), rx, kind: super::super::BgKind::Regen, pulse: None, quiet: false });
        let mut swallowed = true;
        let _ = ctx.run_ui(Default::default(), |c| swallowed = app.tick_async_for_test(c.ctx()));
        assert!(app.regen_running_for_test(), "setup: the rebuild must be running");
        assert!(!swallowed, "the frame was aborted during a rebuild — the window would seize up and the scene would vanish");
    }
}

/// THE REBUILD WINDOW APPEARS ONLY WHERE IT IS EARNED.
///
/// Reported behaviour: make a cut in one part and the rebuild window pops up, so sit and wait. A
/// measurement on the reported file (28 nodes, 18 bodies) showed the rebuild was ALREADY pinpoint:
/// editing one feature rebuilds 2 nodes in a second against 13 seconds for a cold build. What lied was
/// the count in the window — it counted the POSITION in the timeline rather than the work, and
/// therefore always got as far as "28 of 28".
///
/// The rule is now: a thread or a rebuild of the whole timeline gets a window (otherwise a person
/// decides nothing is happening), and a pinpoint edit gets one line in the status bar.
#[cfg(test)]
mod loud_only_when_earned {
    use super::super::App;

    fn two_parts() -> App {
        let mut app = App::default();
        super::super::joint_flow::tests::add_part_at(&mut app, 0.0);
        super::super::joint_flow::tests::add_part_at(&mut app, 100.0);
        app.regenerate_now();
        app
    }

    /// A PINPOINT EDIT GETS NO WINDOW.
    #[test]
    fn a_small_edit_does_not_raise_the_modal() {
        let mut app = two_parts();
        let first = app.project.timeline.iter().find(|n| n.kind.body().is_some()).map(|n| n.id).expect("the node of the body");
        app.project.mark_node_dirty(first);
        let plan = app.project.regen_plan();
        assert!(plan.nodes.len() * 2 < plan.total, "setup: the edit must be pinpoint, and it came out {} of {}", plan.nodes.len(), plan.total);
        app.spawn_regen();
        let quiet = matches!(&app.regen.busy, Some(b) if b.quiet);
        assert!(quiet, "a modal window popped up on an edit of one node — a person is held where the work takes a second");
    }

    /// A REBUILD OF THE WHOLE TIMELINE GETS A WINDOW: it is long, and that has to be said.
    #[test]
    fn a_full_rebuild_still_raises_the_modal() {
        let mut app = two_parts();
        for n in app.project.timeline.iter_mut() {
            n.dirty = true;
        }
        app.spawn_regen();
        let quiet = matches!(&app.regen.busy, Some(b) if b.quiet);
        assert!(!quiet, "a rebuild of the whole timeline went silently — a person would not understand why everything stopped");
    }
}

/// A QUIET REBUILD IS VISIBLE AND YET DOES NOT GET IN THE WAY.
///
/// Reported: while a background rebuild runs silently, the previous body is what is seen, and wrong
/// conclusions get drawn from it — and that is exactly how the reporter caught themselves out over
/// lost edges: at first they decided the edges had not been lost, when the bodies were simply still
/// rebuilding. A sign is needed, but an unobtrusive one: the part greys slightly and a spinner turns,
/// while nobody stops a person working.
#[cfg(test)]
mod quiet_is_visible {
    use super::super::App;

    #[test]
    fn a_quiet_rebuild_shows_the_veil_and_blocks_nothing() {
        let mut app = App::default();
        super::super::joint_flow::tests::add_part_at(&mut app, 0.0);
        app.regenerate_now();
        let first = app.project.timeline.iter().find(|n| n.kind.body().is_some()).map(|n| n.id).expect("the node of the body");
        let _ = first;
        // A REBUILD THAT IS STILL RUNNING: a real one on a little cube finishes inside the frame.
        let (_tx, rx) = std::sync::mpsc::channel();
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        app.waiting.splash_until = None;
        app.regen.busy = Some(super::super::Busy { label: "rebuild".into(), rx, kind: super::super::BgKind::Regen, pulse: None, quiet: true });
        let mut swallowed = true;
        let _ = ctx.run_ui(Default::default(), |c| swallowed = app.tick_async_for_test(c.ctx()));
        assert!(!swallowed, "a quiet rebuild has no right to eat the frame");
        assert!(app.dim.spinner, "a quiet rebuild must leave a sign on the canvas: otherwise a person takes what is shown for the truth");
        assert!(app.dim.overlay.is_none(), "a quiet rebuild has no right to raise a window — nobody is holding a person");
    }
}

/// THE PICTURE DOES NOT LAG BEHIND THE MODEL.
///
/// Reported: there is a discrepancy between when a body is built and what the 3D viewport shows — as
/// if the picture is not always redrawn when it should be.
///
/// An analysis of the code showed that both caches of the picture (the raster one and the vertex
/// buffer of the graphics card) include the geometry revision, and every path of the rebuild moves it.
/// No separate path of "the meshes changed and the picture did not" was found — the observation is
/// explained by the silent background rebuild, which is now visible (the part greys and a spinner
/// turns).
///
/// But the very first new line can easily INTRODUCE such a path: it is enough to change the geometry
/// past `invalidate`. So here is a guard on the property itself: the model changed, so BOTH keys
/// changed.
#[cfg(test)]
mod picture_follows_the_model {
    use super::super::App;

    #[test]
    fn changing_the_model_changes_both_picture_keys() {
        let mut app = App::default();
        super::super::joint_flow::tests::add_part_at(&mut app, 0.0);
        app.regenerate_now();
        let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 700.0));
        let (raster, gpu) = (app.view_key_pub(rect, 1.0), app.gpu_scene_key_pub());

        // an ordinary edit through the timeline: the body must move
        let node = app.project.timeline.iter().find(|n| n.kind.body().is_some()).map(|n| n.id).expect("the node of the body");
        app.project.mark_node_dirty(node);
        super::super::joint_flow::tests::add_part_at(&mut app, 120.0);
        app.regenerate_now();

        assert_ne!(raster, app.view_key_pub(rect, 1.0), "the model changed and the RASTER key is the same — the previous picture will stay on screen");
        assert_ne!(gpu, app.gpu_scene_key_pub(), "the model changed and the key of the GRAPHICS CARD buffer is the same — the previous picture will stay on screen");
    }
}
