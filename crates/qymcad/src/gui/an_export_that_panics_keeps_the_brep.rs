//! AN EXPORT THAT FAILS DOES NOT TAKE THE LIVE B-rep WITH IT. An export moves the live B-rep of the bodies it writes into
//! its thread and gives it back only in its result, as a rebuild does with the whole cache; a panic there ended the
//! thread with the bodies in it, and they were left without their live B-rep - the same hole issue #119 found in the
//! rebuild (`a_failed_rebuild_keeps_the_brep`).
#[cfg(test)]
mod tests {
    use super::super::import_door::tests::{frame, running};
    use crate::gui::io_jobs::EXPORT_PANICS_FOR_TEST;
    use crate::gui::App;

    /// The window rebuilds in the background; this waits for it to land.
    fn settle(app: &mut App, ctx: &egui::Context) {
        for _ in 0..600 {
            frame(app, ctx, Vec::new());
            if app.regen.busy.is_none() && !app.regen.wanted {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        panic!("the window is still busy after half a minute; the status says: {}", app.status);
    }

    fn held(app: &App) -> Vec<qymcad_core::model::Id> {
        let mut ids: Vec<_> = app.live.shapes.keys().copied().collect();
        ids.sort_unstable();
        ids
    }

    /// The save chooser answered with a file whose name asks the export to panic, and the write waited out. `then` is
    /// what the menu hands the chooser.
    fn exported_to_a_panic(app: &mut App, ctx: &egui::Context, file: &str, then: impl FnOnce(&mut App, std::path::PathBuf) + 'static) {
        let dir = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/export-panics"));
        std::fs::create_dir_all(&dir).expect("a folder for the check");
        let (tx, rx) = std::sync::mpsc::channel();
        app.arm_file_ask(rx, then);
        tx.send(Some(dir.join(format!("{EXPORT_PANICS_FOR_TEST}-{file}")))).expect("the chooser's channel is open");
        settle(app, ctx);
        for _ in 0..10 {
            frame(app, ctx, Vec::new());
        }
    }

    /// A STEP and an STL export made to panic, each answered through the window's chooser: the part keeps its live
    /// B-rep, and the status names the format and the failure.
    #[test]
    fn an_export_that_panics_hands_the_live_brep_back() {
        let (mut app, ctx) = running();
        super::super::joint_flow::tests::add_part_at(&mut app, 0.0);
        settle(&mut app, &ctx);
        let before = held(&app);
        assert!(!before.is_empty(), "setup: the part has a live B-rep");
        let target = qymcad_ui_state::ExportTarget::Project;
        let mut failures = Vec::new();

        let plan = app.export_plan(target);
        let format = qymcad_kernel::ExactFormat::Step;
        let job = crate::gui::io_jobs::ExportJob { format, tree: crate::gui::io_jobs::export_tree_of(&app.project, format, target, &plan.brep), bodies: plan.brep.clone(), note: plan.note(true) };
        exported_to_a_panic(&mut app, &ctx, "part.step", move |app, path| crate::gui::io_jobs::write_exact_to(&mut app.live, &mut app.project, &mut app.regen, &mut app.status, &path, &job));
        let said = crate::i18n::tr2("io-export-failed", "format", crate::gui::exact_entry(format).name(), "why", "an export made to fail by a test");
        if held(&app) != before || !app.status.contains(&said) {
            failures.push(format!("STEP: live B-rep {:?} against {before:?} before; the status {:?}, not {said:?}", held(&app), app.status));
        }

        let plan = app.export_plan(target);
        let (bodies, note) = (plan.stl_bodies(), plan.note(false));
        let mesh = qymcad_ui_state::MeshFormat::Stl;
        exported_to_a_panic(&mut app, &ctx, "part.stl", move |app, path| {
            let job = crate::gui::io_jobs::mesh_job(&app.project, mesh, target, bodies, note, 0.1);
            crate::gui::io_jobs::write_mesh_to(qymcad_ui_state::editing_of!(app), &mut app.live, &path, &job)
        });
        let said = crate::i18n::tr2("io-export-failed", "format", crate::gui::mesh_entry(mesh).name(), "why", "an export made to fail by a test");
        if held(&app) != before || !app.status.contains(&said) {
            failures.push(format!("STL: live B-rep {:?} against {before:?} before; the status {:?}, not {said:?}", held(&app), app.status));
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }
}
