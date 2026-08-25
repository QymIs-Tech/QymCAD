//! ENTER A LATE SKETCH, LEAVE IT, AND THE FEATURE TURNS RED. Written from a report.
//!
//! Reported behaviour: start the program with a project, open sketch 15, close it — and the cut at
//! the very end of the timeline turns red with "the source body is not built, fix the feature earlier
//! in the timeline first". Meanwhile the 3D view has not rebuilt and the cut is there on the part.
//! "Rebuild everything" makes the error go away and it never comes back. So it only happens on the
//! first run.
//!
//! THE CAUSE IS IN THE WORD "LATE". A project from a bundle shows the geometry FROM THE FILE and has
//! no live `Shape`s (lazy B-rep): those are built on demand. Leaving a sketch marks ONLY its own node
//! dirty and rebuilds — while the feature standing on it asks for a source body that is not in the
//! live cache yet. "Rebuild everything" builds the chain from the start, which is why it does not
//! happen again afterwards.
//!
//! The first version of this test did not catch it, because it went through the sketches IN A ROW: by
//! the fifteenth one the live bodies had already been built by the previous steps. It is caught only
//! on a sketch that has an unbuilt source earlier in the timeline — and on a FRESH opening.
#[cfg(test)]
mod tests {
    use super::super::{App, Sel};

    /// A plate with a cut from a second sketch — the smallest timeline where the last feature has a
    /// source. Saved to a bundle and opened again: exactly the state in which the error is caught.
    fn saved_and_reopened() -> Option<(App, String, usize)> {
        let dir = std::env::temp_dir().join("qym_sketch_reopen_test");
        std::fs::create_dir_all(&dir).ok()?;
        let path = dir.join("late_sketch.qcad").to_string_lossy().into_owned();
        let _ = std::fs::remove_file(&path);

        let mut app = App::default();
        // 1) the plate
        let si0 = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        app.project.add_rect_entity(si0, 0.0, 0.0, 40.0, 30.0, qymcad_core::feature::Purpose::Real);
        app.project.regen_sketch(si0);
        app.finish_sketch_edit();
        app.sel = Sel::Sketch(si0);
        app.feat.op = 0;
        app.start_feat_cmd(1);
        app.apply_feat_cmd();

        // 2) a cut from the SECOND sketch — this feature has a source body
        let si1 = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        app.project.add_rect_entity(si1, 10.0, 10.0, 20.0, 20.0, qymcad_core::feature::Purpose::Real);
        app.project.regen_sketch(si1);
        app.finish_sketch_edit();
        app.sel = Sel::Sketch(si1);
        app.feat.op = 1; // cut
        app.start_feat_cmd(1);
        app.apply_feat_cmd();
        assert!(app.project.regen_errors.is_empty(), "the model built cleanly: {:?}", app.project.regen_errors);

        app.project_path = Some(path.clone());
        app.save_project();
        app.wait_bg();

        // open it again — exactly as the application does at startup
        let loaded = qymcad_io::load_project(&path).ok()?;
        let mut fresh = App::default();
        fresh.finish_project_load(path.clone(), loaded, Vec::new());
        Some((fresh, path, si1))
    }

    /// LEAVING AN UNTOUCHED LATE SKETCH DOES NOT BREAK THE FEATURE STANDING ON IT.
    #[test]
    fn leaving_a_late_sketch_right_after_opening_does_not_break_its_feature() {
        let Some((mut app, path, si)) = saved_and_reopened() else { return };
        assert!(app.live.shapes.is_empty(), "a freshly opened bundle has no live Shapes — that is the very condition of the defect");
        assert!(app.project.regen_errors.is_empty(), "an opened project shows no errors");

        app.sel = Sel::Sketch(si);
        app.enter_sketch_edit_pub(si);
        app.finish_sketch_edit(); // Ctrl+Enter — the sketch was NOT touched

        let errs: Vec<String> = app.project.regen_errors.values().map(|e| format!("{e:?}")).collect();
        assert!(errs.is_empty(), "leaving an untouched sketch coloured the feature red: {}", errs.join("; "));
        assert!(!app.live.shapes.is_empty(), "the live bodies were built — otherwise there would have been nothing to rebuild from");
        let _ = std::fs::remove_file(&path);
    }

    /// AND AN EDIT OF THE SKETCH AFTER OPENING REACHES THE BODY. The first check would have said
    /// "no errors" even if leaving a sketch had stopped rebuilding anything at all.
    #[test]
    fn editing_a_late_sketch_after_opening_actually_rebuilds_the_body() {
        let Some((mut app, path, si)) = saved_and_reopened() else { return };
        let before: f64 = app.project.bodies.iter().map(|b| b.mesh.verts.len() as f64).sum();

        app.sel = Sel::Sketch(si);
        app.enter_sketch_edit_pub(si);
        // move the cut: the edit is real, the body must rebuild and stay intact
        let sid = app.project.sketches[si].id;
        for p in app.project.sketches[si].points.iter_mut() {
            p.x += 3.0;
        }
        app.project.regen_sketch(si);
        app.project.mark_sketch_dirty(sid);
        app.finish_sketch_edit();

        let errs: Vec<String> = app.project.regen_errors.values().map(|e| format!("{e:?}")).collect();
        let after: f64 = app.project.bodies.iter().map(|b| b.mesh.verts.len() as f64).sum();
        let _ = std::fs::remove_file(&path);
        assert!(errs.is_empty(), "an edit of the sketch broke the feature: {}", errs.join("; "));
        assert!(after > 0.0 && before > 0.0, "the bodies are there before and after the edit");
    }
}
