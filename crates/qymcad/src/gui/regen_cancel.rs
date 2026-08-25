//! A LONG REBUILD IS VISIBLE AND CAN BE STOPPED.
//!
//! The spinner said exactly one thing: busy. On an assembly that takes seconds to compute a person
//! needs something else: how much is left and whether they may change their mind. Both answers come
//! from ONE place — the loop over the timeline in the kernel (`RegenWatch`): it reports the step and
//! obeys the answer "stop".
//!
//! The stop happens BETWEEN nodes rather than inside a kernel operation: there is nothing to interrupt
//! an OCCT boolean halfway with, and pretending otherwise would yield half a body. On the boundary of
//! a node the document is still whole — a COPY is what is being computed — so cancelling costs exactly
//! nothing: the incomplete result is thrown away entirely.
#[cfg(test)]
mod tests {
    use super::super::App;
    use qymcad_core::feature::{Kernel, NoWatch, RegenWatch};

    /// A watcher that stops the rebuild at node `at` and counts the visits.
    struct StopAt {
        at: usize,
        seen: std::sync::atomic::AtomicUsize,
    }
    impl RegenWatch for StopAt {
        fn step(&self, done: usize, _total: usize) -> bool {
            self.seen.store(done, std::sync::atomic::Ordering::Relaxed);
            done < self.at
        }
    }

    /// THE KERNEL OBEYS: the rebuild stops where it is told and honestly marks the report as
    /// incomplete.
    #[test]
    fn the_core_stops_where_it_is_told_and_says_the_report_is_partial() {
        let mut app = super::super::screen_keys::tests::plate();
        let nodes = app.project.timeline.len();
        assert!(nodes >= 2, "setup: the timeline needs several nodes, and it holds {nodes}");
        for n in app.project.timeline.iter_mut() {
            n.dirty = true;
        }
        let kernel = qymcad_kernel::OcctKernel { shapes: std::cell::RefCell::new(Default::default()), ..Default::default() };
        let watch = StopAt { at: 1, seen: std::sync::atomic::AtomicUsize::new(usize::MAX) };
        let report = app.project.regenerate_watched(&kernel as &dyn Kernel, &watch);
        assert!(report.cancelled, "the report must be marked cancelled — otherwise an incomplete result is applied as a complete one");
        assert_eq!(watch.seen.load(std::sync::atomic::Ordering::Relaxed), 1, "the stop happened on the wrong node");
        assert!(report.built.len() < nodes, "after a stop on the first node {} of {nodes} were built — that is not a stop", report.built.len());
    }

    /// AND WITHOUT A WATCHER NOTHING CHANGES: the rebuild has a hundred and fifty callers, and they
    /// must not pay for cancellation where there is nobody to cancel.
    #[test]
    fn without_a_watcher_the_rebuild_is_never_cancelled() {
        let mut app = super::super::screen_keys::tests::plate();
        for n in app.project.timeline.iter_mut() {
            n.dirty = true;
        }
        let kernel = qymcad_kernel::OcctKernel { shapes: std::cell::RefCell::new(Default::default()), ..Default::default() };
        let report = app.project.regenerate_watched(&kernel as &dyn Kernel, &NoWatch);
        assert!(!report.cancelled, "a rebuild without a watcher cannot turn out to be cancelled");
    }

    /// CANCELLING DOES NOT START THE REBUILD AGAIN. The nodes stayed dirty after it, and the
    /// scheduler looks at exactly those: without a "stopped" mark the next frame would begin the very
    /// work a person had just interrupted, and the cancel button would become a flashing light.
    #[test]
    fn cancelling_does_not_immediately_start_the_same_rebuild_again() {
        let mut app = super::super::screen_keys::tests::plate();
        app.regen.ui_running = true;
        for n in app.project.timeline.iter_mut() {
            n.dirty = true;
        }
        // the way the arrival of a cancelled result does it
        app.finish_regen_checked(app.regen_doc_stamp(), app.project.clone(), Vec::new(), Vec::new(), Vec::new(), true);
        assert!(app.regen_paused_for_test(), "after a cancellation the rebuild must count as stopped");
        assert!(app.project.timeline.iter().any(|n| n.dirty), "setup: dirty nodes remain after a cancellation — the model really is not rebuilt");

        app.regen.wanted = false;
        app.rebuild_if_dirty_for_test();
        assert!(!app.regen.wanted, "the very next frame started that same rebuild again — the cancel button cancels nothing");

        // ...but an explicit request from a person ("rebuild everything") clears the mark
        app.mark_dirty_for_rebuild_for_test();
        assert!(app.regen.wanted || app.regen.busy.is_some(), "an explicit request must start the rebuild again");
        assert!(!app.regen_paused_for_test(), "after an explicit request the \"stopped\" mark must be cleared");
    }

    /// THE BUTTON AND THE COUNT REACH THE SCREEN. The overlay is drawn by a painter over the input
    /// barrier, and the button on it is the only live widget: create it BEFORE the barrier and the
    /// click would go to the barrier.
    #[test]
    fn the_overlay_shows_the_count_and_a_cancel_button() {
        let app = super::super::screen_keys::tests::plate();
        let screen = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1400.0, 900.0));
        let input = egui::RawInput { screen_rect: Some(screen), ..Default::default() };
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        let _ = ctx.run(input.clone(), |c| {
            app.draw_regen_overlay_for_test(c, 7, 40);
        });
        let out = ctx.run(input, |c| {
            app.draw_regen_overlay_for_test(c, 7, 40);
        });
        let mut texts = Vec::new();
        collect_text_from(&out, &mut texts);
        let want = crate::i18n::tr2("io-rebuild-progress", "done", "7", "total", "40");
        assert!(texts.iter().any(|t| t.contains(&want)), "the count \"{want}\" is not on the overlay: {texts:?}");
        let cancel = crate::i18n::tr("io-rebuild-cancel");
        assert!(texts.iter().any(|t| t.contains(&cancel)), "the \"{cancel}\" button is not on the overlay: {texts:?}");
    }

    fn collect_text_from(out: &egui::FullOutput, into: &mut Vec<String>) {
        fn walk(s: &egui::epaint::Shape, out: &mut Vec<String>) {
            match s {
                egui::epaint::Shape::Text(t) => out.push(t.galley.text().to_string()),
                egui::epaint::Shape::Vec(v) => v.iter().for_each(|x| walk(x, out)),
                _ => {}
            }
        }
        for cs in &out.shapes {
            walk(&cs.shape, into);
        }
    }

    /// THE REQUEST TO STOP REACHES THE THREAD — through the same handle the button pulls.
    #[test]
    fn the_cancel_button_reaches_the_running_rebuild() {
        let mut app = super::super::screen_keys::tests::plate();
        app.regen.ui_running = true;
        for n in app.project.timeline.iter_mut() {
            n.dirty = true;
        }
        app.spawn_regen();
        assert!(app.regen.busy.is_some(), "setup: the rebuild is running");
        app.cancel_regen_for_test();
        app.drain_busy_for_test();
        assert!(app.regen_paused_for_test(), "the request to stop did not arrive: the rebuild does not count as stopped; status: {}", app.status_for_test());
    }
    /// A FAILING NODE DOES NOT MAKE THE PROGRAM COMPUTE WITHOUT STOPPING.
    ///
    /// Reported behaviour: the rebuild window flashes like mad, 20 frames a second, reading "feature 0
    /// of 0".
    ///
    /// A node that failed to build stays DIRTY deliberately: the attempt must happen again once its
    /// input appears — a live B-rep after opening a file, for one. The mistake was not in the mark but
    /// in the scheduler reading it as "compute now" and taking on the node every frame. With a single
    /// failing feature the program became unusable.
    ///
    /// What is checked is the behaviour of the scheduler rather than the mark: a second pass in a row,
    /// when NOTHING has changed, has no right to ask for a rebuild.
    #[test]
    fn a_failing_node_does_not_keep_the_scheduler_running_every_frame() {
        let mut app = App::default();
        let part = app.project.add_part("part");
        app.enter_component(part);
        let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        app.project.add_rect_entity(si, 0.0, 0.0, 40.0, 30.0, qymcad_core::feature::Purpose::Real);
        app.project.regen_sketch(si);
        app.finish_sketch_edit();
        app.sel = super::super::Sel::Sketch(si);
        app.start_feat_cmd(1);
        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "height") {
            p.val = 10.0;
            p.txt = "10".into();
        }
        app.apply_feat_cmd();
        app.rebuild_if_dirty();
        let body = app.project.timeline.iter().rev().find_map(|n| n.kind.body()).expect("the body");

        // a patch over a rim the body does not have — the node fails for certain
        let bad = app.project.add_patch(body, qymcad_core::refs::Ref::picks(&[0xDEAD_BEEF, 0xDEAD_BEEE]), false);
        app.rebuild_if_dirty();
        assert!(app.project.regen_errors.contains_key(&bad), "setup: the node must turn red");

        // THE NEXT FRAME: nothing changed, so there is nothing to compute
        app.regen.ui_running = true; // as in a live window: a heavy rebuild is taken into a thread
        app.regen.wanted = false;
        app.rebuild_if_dirty();
        assert!(
            !app.regen.wanted,
            "nothing changed and the scheduler asks for a rebuild again — that is the window flashing on every frame"
        );

        // AND ON A REAL EDIT it does ask, otherwise the cure is worse than the illness
        app.project.mark_sketch_dirty(app.project.sketches[0].id);
        app.rebuild_if_dirty();
        assert!(app.regen.wanted, "after an edit of the document a rebuild must be asked for");
    }
}
