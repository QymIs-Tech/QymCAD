//! The shots of a sketch: at a scale set in advance for the animations, fitted for a single picture, and the empty
//! sketch they start from.

use super::*;

/// A SKETCH AT A SCALE SET IN ADVANCE - for the animations.
///
/// Fitting the frame to the bounds does not do here: after a trim the geometry gets smaller and the
/// frame closes in - to the eye it looks as if the line did not shorten but came nearer.
pub(super) fn shot_sketch_fixed(app: &mut App, si: usize, scale: f32) -> ColorImage {
    app.viewing.mode_3d = false;
    app.project.regen_sketch(si);
    app.viewing.view.scale = scale;
    app.viewing.view.center = crate::gui::Vec2::new(0.0, 0.0);
    app.viewing.view.initialized = true;
    let bg = app.scheme.pal.viewport_bg();
    let a = &*app;
    crate::gui::help_raster::shot_ui([640, 400], bg, |ui| {
        let ctx = &ui.ctx().clone();
        ctx.set_visuals(if a.scheme.pal.light { egui::Visuals::light() } else { egui::Visuals::dark() });
        egui::CentralPanel::default().show(ui, |ui| {
            let painter = ui.painter().clone();
            let r = ui.available_rect_before_wrap();
            painter.rect_filled(r, 0.0, bg);
            a.draw_contours(&painter, r);
            a.draw_sketch_constraints(&painter, r, si);
            a.draw_sketch_dims(&painter, r, si);
        });
    })
}

/// An empty sketch in a part of its own - the basis of the scenes in the sketch section.
pub(super) fn empty_sketch() -> (App, usize) {
    let mut app = in_one_part();
    let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
    (app, si)
}

/// A SHOT OF A SKETCH: the geometry, the constraint glyphs and the dimension captions - by the same
/// code that draws the sketcher on screen (`draw_contours` plus `draw_sketch_constraints` plus
/// `draw_sketch_dims`).
///
/// The background is OPAQUE, unlike the pictures of bodies: the canvas of the sketcher is part of the
/// picture, and without it the page would carry bare lines in a colour meant for a dark canvas.
pub(super) fn shot_sketch(app: &mut App, si: usize, w: usize, h: usize) -> ColorImage {
    app.viewing.mode_3d = false;
    app.project.regen_sketch(si);
    qymcad_ui_state::fit(&app.project, &mut app.viewing.view, Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(w as f32, h as f32)));
    let bg = app.scheme.pal.viewport_bg();
    let a = &*app;
    crate::gui::help_raster::shot_ui([w, h], bg, |ui| {
        egui::CentralPanel::default().show(ui, |ui| {
            let painter = ui.painter().clone();
            let r = ui.available_rect_before_wrap();
            painter.rect_filled(r, 0.0, bg);
            a.draw_contours(&painter, r);
            a.draw_sketch_constraints(&painter, r, si);
            a.draw_sketch_dims(&painter, r, si);
        });
    })
}

/// One frame of a sketch animation (sketches fit their own camera - the scenes are all the same size).
pub(super) fn sketch_frame(app: &mut App, si: usize, dir: &str, i: usize) {
    std::fs::create_dir_all(img_dir().join(dir)).expect("the frame directory");
    let img = shot_sketch(app, si, 640, 400);
    save(&format!("{dir}/{i:02}.png"), &img);
}

/// A single picture of a sketch.
pub(super) fn sketch_still(app: &mut App, si: usize, name: &str) {
    let img = shot_sketch(app, si, 640, 400);
    save(&format!("{name}.png"), &img);
}
