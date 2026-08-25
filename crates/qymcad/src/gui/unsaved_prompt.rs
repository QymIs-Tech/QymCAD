//! "SAVE?" IS ASKED ONLY WHEN THERE IS SOMETHING TO SAVE. Written from reports.
//!
//! 1. Open a project, do nothing in it, close it — and the save-or-not message pops up at once. The
//!    question was asked honestly by the edit key, but that key includes `geom_rev`, the revision of
//!    the DRAWING cache, which any rebuild moves. And opening a project is what causes a rebuild.
//!
//!    The rule "a rebuild is not a person's edit" had already been derived in the code, but applied
//!    only to the guard of operation boundaries (`committed_key`). For "is it saved?" it was
//!    forgotten — and it stayed quiet exactly as long as the rebuild after opening ran
//!    synchronously.
//!
//! 2. Press save — and that is it, the window for opening a new project never appears. The write goes
//!    in the BACKGROUND, and the dirtiness was checked straight after the request to save: by that
//!    moment the file had not yet landed on the disk, the project counted as dirty, and the
//!    navigation was thrown away. A person has already said "save it and move on" — their command
//!    must not be lost.
#[cfg(test)]
mod tests {
    use super::super::{App, Nav, Sel};
    use qymcad_core::feature::SketchPlane;

    /// A plate and a save to a file; returns the path.
    fn saved_project(name: &str) -> (App, String) {
        let dir = std::env::temp_dir().join("qym_unsaved_prompt_test");
        std::fs::create_dir_all(&dir).expect("the directory for the check");
        let path = dir.join(name).to_string_lossy().into_owned();
        let _ = std::fs::remove_file(&path);

        let mut app = App::default();
        let si = app.create_sketch_on(SketchPlane::default());
        app.project.add_rect_entity(si, -20.0, -20.0, 20.0, 20.0, qymcad_core::feature::Purpose::Real);
        app.project.regen_sketch(si);
        app.finish_sketch_edit();
        app.sel = Sel::Sketch(si);
        app.start_feat_cmd(1);
        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "height") {
            p.val = 10.0;
            p.txt = "10".into();
        }
        app.apply_feat_cmd();
        // THE PATH IS REMEMBERED, as "save as" does it: without it `save_project` goes into a file
        // dialogue, which a test does not have, and the save quietly writes nothing.
        app.set_project_path(path.clone());
        // THROUGH A REAL SAVE: `spawn_save` only writes the file, while it is `save_project` that
        // marks the project saved — a fixture calling the write directly would leave it dirty and
        // check the wrong thing.
        app.save_project_for_test();
        app.wait_bg_for_test();
        (app, path)
    }

    /// OPENED IT AND DID NOTHING — THERE IS NOTHING TO SAVE.
    #[test]
    fn a_freshly_opened_project_is_not_dirty() {
        let (_src, path) = saved_project("clean.qcad");

        let mut app = App::default();
        app.open_for_test(path.clone());
        assert!(!app.is_dirty_for_test(), "right after opening there is nothing to save");

        // THE POINT IS THE REBUILD THE OPENING ITSELF SCHEDULES. A project from a bundle carries no
        // live B-rep: it is loaded in the background and a rebuild is asked for
        // (`mark_dirty_for_rebuild`). Without this step the test goes down the synchronous path, where
        // there is no rebuild at all, and stays green even with broken logic — that is exactly what
        // caught this check out during an honesty pass.
        app.mark_dirty_for_rebuild_for_test();
        for _ in 0..3 {
            app.rebuild_if_dirty_for_test();
            app.drain_busy_for_test(); // the rebuild may have gone into a thread — carried through, as a frame does
            assert!(!app.is_dirty_for_test(), "a rebuild is not a person's edit: after it the project must stay saved");
        }
        let _ = std::fs::remove_file(&path);
    }

    /// A REAL EDIT AFTER OPENING IS DIRT. Otherwise the first test is green for "never ask" as well.
    #[test]
    fn a_real_edit_after_opening_is_still_dirty() {
        let (_src, path) = saved_project("edited.qcad");

        let mut app = App::default();
        app.open_for_test(path.clone());
        assert!(!app.is_dirty_for_test(), "setup: opened, and it is clean");

        let si = app.create_sketch_on(SketchPlane::default());
        app.project.add_rect_entity(si, 0.0, 0.0, 5.0, 5.0, qymcad_core::feature::Purpose::Real);
        app.project.regen_sketch(si);
        app.finish_sketch_edit();
        app.rebuild_if_dirty_for_test();
        assert!(app.is_dirty_for_test(), "an edit after opening must count as unsaved — otherwise the work is lost silently");
        let _ = std::fs::remove_file(&path);
    }

    /// "SAVE AND MOVE ON" DOES NOT LOSE THE COMMAND.
    ///
    /// What is checked is the CONSEQUENCE of choosing to save: the write landed on the disk and the
    /// deferred navigation stayed doable rather than being thrown away. Exactly what used to break: the
    /// background write did not make it in time and the navigation silently vanished.
    #[test]
    fn choosing_save_keeps_the_pending_navigation() {
        let (mut app, path) = saved_project("nav.qcad");

        // an edit makes the project dirty, so the navigation is deferred
        let si = app.create_sketch_on(SketchPlane::default());
        app.project.add_rect_entity(si, 0.0, 0.0, 5.0, 5.0, qymcad_core::feature::Purpose::Real);
        app.project.regen_sketch(si);
        app.finish_sketch_edit();
        app.rebuild_if_dirty_for_test();
        assert!(app.is_dirty_for_test(), "setup: after the edit the project is dirty");

        app.request_nav_for_test(Nav::New);
        assert!(app.deferred_nav_for_test(), "a dirty project must defer the navigation rather than carry it out silently");

        // the save: a background write plus the waiting — exactly what the dialogue does
        app.save_project_for_test();
        app.wait_bg_for_test();
        assert!(!app.is_dirty_for_test(), "after a COMPLETED write the project must be clean — otherwise the dialogue throws the navigation away");
        let _ = std::fs::remove_file(&path);
    }

    /// THE DIALOGUE NEITHER LOSES A PERSON'S COMMAND NOR FREEZES THE WINDOW.
    ///
    /// A guard over the source used to stand here: "between the request to save and the check that the
    /// dirt is gone there must be a `wait_bg()`". The waiting really was there — and it was BLOCKING:
    /// no frame was drawn while the file was landing on the disk, and to a person that is
    /// indistinguishable from a hung program. The requirement was stated plainly: a person must not
    /// think the program has gone quiet.
    ///
    /// Now the navigation WAITS for the end of the write without freezing the window, and that is
    /// checked by behaviour rather than by the text of the source: see `saving_is_not_silent.rs` —
    /// while the write runs there is a waiting card in the frame, and after the write the navigation
    /// happens.
    #[test]
    fn the_dialog_does_not_freeze_the_window_while_saving() {
        let src = crate::gui::panels_source::PANELS;
        let code = src.split("#[cfg(test)]\nmod ").next().expect("the working part");
        let at = code.find("fn nav_dialog").expect("the dialogue is there");
        let end = code[at + 10..].find("\n    pub(super) fn ").map(|i| at + 10 + i).unwrap_or(code.len());
        let body = &code[at..end];
        assert!(body.contains("self.save_project();"), "the save branch is there");
        assert!(
            !body.contains("self.wait_bg();"),
            "a blocking wait for the write is back in the dialogue — the window will freeze and a person will decide the program has hung"
        );
        assert!(body.contains("nav_after_save"), "the navigation must WAIT for the end of the write rather than be thrown away");
    }

    /// EDITING A FEATURE PARAMETER IS UNSAVED WORK. The main check after the key was moved.
    ///
    /// `geom_rev` was taken out of the document key, and `state_key` must take over its role. Were it
    /// blind to the parameters, "changed the height of an extrude and closed the window" would pass
    /// SILENTLY — a loss of work, that is, far worse trouble than one extra question.
    #[test]
    fn changing_a_feature_parameter_counts_as_unsaved() {
        use qymcad_core::feature::FeatureKind as FK;
        let (mut app, path) = saved_project("param.qcad");
        app.rebuild_if_dirty_for_test();
        app.drain_busy_for_test();
        assert!(!app.is_dirty_for_test(), "setup: a saved project is clean");

        // change the HEIGHT of the extrude right in the recipe — the way the gizmo does it
        let mut touched = false;
        for n in app.project.timeline.iter_mut() {
            if let FK::Extrude { height, .. } = &mut n.kind {
                *height += 3.0;
                touched = true;
                break;
            }
        }
        assert!(touched, "setup: the timeline must hold an extrude");
        assert!(app.is_dirty_for_test(), "editing a PARAMETER of a feature must count as unsaved — otherwise the work is lost silently");
        let _ = std::fs::remove_file(&path);
    }

    /// AND A REBUILD IS NOT, however many frames go by. Exactly the reported complaint, in full.
    #[test]
    fn no_number_of_rebuilds_ever_makes_an_untouched_project_dirty() {
        let (_src, path) = saved_project("stable.qcad");
        let mut app = App::default();
        app.open_for_test(path.clone());
        for i in 0..10 {
            app.mark_dirty_for_rebuild_for_test();
            app.rebuild_if_dirty_for_test();
            app.drain_busy_for_test();
            assert!(!app.is_dirty_for_test(), "after rebuild {} an untouched project became dirty", i + 1);
        }
        let _ = std::fs::remove_file(&path);
    }
}
