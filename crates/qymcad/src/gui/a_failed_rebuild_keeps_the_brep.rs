//! A REBUILD THAT FAILS DOES NOT TAKE THE LIVE B-rep WITH IT. Reported behaviour (issue #119): the background rebuild
//! takes the whole live B-rep cache into its thread, and a panic there ended the thread with the cache in it - every
//! body of the document was left without its live B-rep, so picking a face, a sketch on a face or a fillet found the
//! body "not built" until something rebuilt it.
#[cfg(test)]
mod tests {
    use super::super::import_door::tests::{frame, running};
    use crate::gui::io_jobs::REBUILD_PANICS_FOR_TEST;

    /// A rebuild made to panic, driven through the window as a person's would be: the bodies keep their live B-rep,
    /// the window is free again, and the status says the rebuild failed and why.
    #[test]
    fn a_rebuild_that_panics_hands_the_live_brep_back() {
        let (mut app, ctx) = running();
        super::super::joint_flow::tests::add_part_at(&mut app, 0.0);
        // the part is built the way the window builds it: in the background, landing a few frames later
        let settle = |app: &mut crate::gui::App, ctx: &egui::Context| {
            for _ in 0..600 {
                frame(app, ctx, Vec::new());
                if app.regen.busy.is_none() && !app.regen.wanted {
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            panic!("the window is still rebuilding after half a minute; the status says: {}", app.status);
        };
        settle(&mut app, &ctx);
        let held = |app: &crate::gui::App| {
            let mut ids: Vec<_> = app.live.shapes.keys().copied().collect();
            ids.sort_unstable();
            ids
        };
        let before = held(&app);
        assert!(!before.is_empty(), "setup: the part has a live B-rep");

        app.project.meta.comment = REBUILD_PANICS_FOR_TEST.into();
        app.spawn_regen();
        assert!(app.live.shapes.is_empty(), "setup: the rebuild took the live B-rep into its thread");
        settle(&mut app, &ctx);
        for _ in 0..10 {
            frame(&mut app, &ctx, Vec::new());
        }
        assert_eq!(held(&app), before, "the live B-rep did not come back from the failed rebuild; the status says: {}", app.status);
        let failed = crate::i18n::tr1("io-rebuild-failed", "why", "a rebuild made to fail by a test");
        assert!(app.status.contains(&failed), "the status does not say the rebuild failed and why: {:?}, not {failed:?}", app.status);
    }

    /// A rebuild that fails for the last edit takes that edit back, as Cancel does: the document is the one before the
    /// edit, Redo holds the edit, and the status says what failed and what was taken back - still, after the rebuild of
    /// the restored document that taking it back starts. The edit moves a corner of the part's sketch, so that rebuild
    /// has a body to rebuild; the switch is set inside the edit, so the restored document rebuilds without it.
    #[test]
    fn a_rebuild_that_panics_for_the_last_edit_takes_the_edit_back() {
        let (mut app, ctx) = running();
        super::super::joint_flow::tests::add_part_at(&mut app, 0.0);
        let settle = |app: &mut crate::gui::App, ctx: &egui::Context| {
            for _ in 0..600 {
                frame(app, ctx, Vec::new());
                if app.regen.busy.is_none() && !app.regen.wanted {
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            panic!("the window is still rebuilding after half a minute; the status says: {}", app.status);
        };
        // a rebuild of everything, to its end: what is computed is the document as it stands
        qymcad_ui_state::regenerate_all(&mut app.rebuild_ctx());
        settle(&mut app, &ctx);
        let mut before: Vec<_> = app.live.shapes.keys().copied().collect();
        before.sort_unstable();
        assert!(!before.is_empty(), "setup: the part has a live B-rep");

        // the edit, and the rebuild it asks for, which panics
        let corner = app.project.sketches.last().and_then(|s| s.points.first()).map(|p| (p.id, p.x)).expect("a corner of the sketch");
        qymcad_ui_state::begin_edit(&mut app.disk.edits, &app.project, "move a corner");
        app.project.meta.comment = REBUILD_PANICS_FOR_TEST.into();
        if let Some(p) = app.project.sketches.last_mut().and_then(|s| s.points.iter_mut().find(|p| p.id == corner.0)) {
            p.x -= 2.0;
        }
        qymcad_ui_state::commit_edit(&mut app.rebuild_ctx());
        qymcad_ui_state::regenerate_all(&mut app.rebuild_ctx());
        settle(&mut app, &ctx);
        // taking the edit back started the rebuild of the restored document; let it land
        settle(&mut app, &ctx);

        let x = app.project.sketches.last().and_then(|s| s.points.iter().find(|p| p.id == corner.0)).map(|p| p.x);
        assert_eq!(x, Some(corner.1), "the edit whose rebuild failed was not taken back; the status says: {}", app.status);
        assert!(app.project.meta.comment.is_empty(), "setup: taking the edit back took the switch back with it");
        assert_eq!(app.disk.edits.redo.last().map(|s| s.name.as_str()), Some("move a corner"), "Redo does not hold the edit taken back");
        let mut after: Vec<_> = app.live.shapes.keys().copied().collect();
        after.sort_unstable();
        assert_eq!(after, before, "the live B-rep did not come back from the failed rebuild");
        let said = crate::i18n::trn("io-rebuild-failed-undone", &[("why", "a rebuild made to fail by a test"), ("what", "move a corner")]);
        assert!(app.status.contains(&said), "the status does not say the rebuild failed and what was taken back: {:?}, not {said:?}", app.status);
    }
}
