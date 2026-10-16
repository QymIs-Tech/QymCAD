use super::*;

/// Save and reopen - and check everything there as well.
pub(super) fn save_and_reopen(app: &mut App, step: &str, problems: &mut Vec<String>) -> App {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target").join("user-case.qcad").to_string_lossy().into_owned();
    crate::gui::set_project_path(&mut app.disk.project_path, &mut app.set, path.clone());
    app.save_project();
    app.drain_bg_for_test();
    match qymcad_io::load_project(&path) {
        Ok(project) => {
            let mut fresh = App::default();
            fresh.finish_project_load(path, project, Vec::new());
            qymcad_ui_state::rebuild_if_dirty(&mut fresh.rebuild_ctx());
            let mut p2 = Vec::new();
            check_all(&mut fresh, &format!("{step} -> after opening"), &mut p2);
            problems.extend(p2);
            fresh
        }
        Err(e) => {
            problems.push(format!("[{step}] the saved file does not open: {e}"));
            App::default()
        }
    }
}
