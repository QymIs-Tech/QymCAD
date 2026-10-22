use super::*;

/// Save and reopen - and check everything there as well.
pub(super) fn save_and_reopen(app: &mut App, step: &str, problems: &mut Vec<String>) -> App {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target").join("user-case.qcad").to_string_lossy().into_owned();
    crate::gui::set_project_path(&mut app.disk.project_path, &mut app.set, path.clone());
    app.save_project();
    app.drain_bg_for_test();
    preview_shows_the_model(std::path::Path::new(&path), step, problems);
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

/// THE FILE CARRIES ITS PREVIEW: at every save round of the scenario the document holds bodies on screen, so the
/// file manager must get a picture of them - 256x256, a transparent ground, the model drawn opaque.
fn preview_shows_the_model(path: &std::path::Path, step: &str, problems: &mut Vec<String>) {
    let Some(png) = qymcad_io::load_project_thumb(&path.to_string_lossy()) else {
        problems.push(format!("[{step}] the saved file holds no preview"));
        return;
    };
    let img = match image::load_from_memory(&png) {
        Ok(img) => img.to_rgba8(),
        Err(e) => {
            problems.push(format!("[{step}] the preview of the file does not decode: {e}"));
            return;
        }
    };
    if img.dimensions() != (256, 256) {
        problems.push(format!("[{step}] the preview is {:?}, not 256x256", img.dimensions()));
    }
    if img.get_pixel(0, 0)[3] != 0 {
        problems.push(format!("[{step}] the ground of the preview is not transparent"));
    }
    // the badge stands in the bottom right corner (64x64 and a margin of 8); the model is what is drawn past it.
    // An array of small parts spread over the scene fitted to the frame drew 2 622 opaque pixels there, a single box 34 930.
    let badge = 256 - 64 - 8;
    let opaque = img.enumerate_pixels().filter(|(x, y, p)| p[3] == 255 && (*x < badge || *y < badge)).count();
    if opaque < 16 * 16 {
        problems.push(format!("[{step}] the preview draws {opaque} opaque pixels past the badge: the model is not in it"));
    }
}
