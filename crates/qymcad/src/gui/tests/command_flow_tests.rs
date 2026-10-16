//! GUI PARITY: real user flows through App's COMMAND LAYER (headless, with the real OCCT kernel). Every fault
//! found by hand ("it does not extrude", stale states) lived exactly here - the model-level matrices never saw
//! them. Every new GUI fault becomes a flow in this module.
use super::{App, BgKind, Busy, ExportTarget, JobResult, Picking, Sel};
use qymcad_core::geom::Point3;
use qymcad_core::model::Id;

/// Draw a rectangle in a NEW sketch of the active part and leave editing.
fn sketch_rect(app: &mut App, x0: f64, y0: f64, x1: f64, y1: f64) -> usize {
    let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
    app.project.add_rect_entity(si, x0, y0, x1, y1, qymcad_core::feature::Purpose::Real);
    app.project.regen_sketch(si);
    app.finish_sketch_edit();
    si
}

fn total_volume(app: &App) -> f64 {
    app.live.shapes.values().map(|s| s.volume()).sum()
}

/// A cylinder of diameter `d` and height `h` through the primitive command (the full flow). Returns (the body, the id of the rim's circular edge).
fn build_shaft(app: &mut App, d: f64, h: f64) -> (u64, u32) {
    app.start_prim_cmd(11);
    for (k, v) in [("r", d * 0.5), ("h", h)] {
        if let Some(p) = app.tools.cmd.params.iter_mut().find(|p| p.key == k) {
            p.val = v;
            p.txt = format!("{v}");
        }
    }
    app.apply_feat_cmd();
    let consumed = qymcad_ui_state::consumed_bodies(&app.project);
    let body = *app.live.shapes.keys().find(|b| !consumed.contains(b)).expect("the shaft's body");
    let eid = app.project.regen_edges.get(&body).and_then(|es| es.iter().find(|e| (e.radius - d * 0.5).abs() < 0.05).map(|e| e.id)).expect("the shaft's circular rim");
    (body, eid)
}

/// The part's live volume (excluding the consumed bodies), MEASURED FROM THE MESH. The kernel's integrator is
/// off by whole multiples on helical surfaces (see `thread_profile_fidelity.rs`), so a thread cannot be
/// measured with it.
fn live_volume(app: &App) -> f64 {
    let consumed = qymcad_ui_state::consumed_bodies(&app.project);
    app.live.shapes.iter().filter(|(b, _)| !consumed.contains(b)).map(|(_, s)| qymcad_kernel::specimen::mesh(s, 0.02).iter().map(|b| b.mesh.volume()).sum::<f64>()).sum()
}

/// Run the thread command on a shaft through the FULL GUI flow and return the volume removed.
fn thread_on_shaft(d: f64, h: f64, form: u8, params: &[(&str, f64)], auger: bool) -> (f64, App) {
    let mut app = App::default();
    let (body, eid) = build_shaft(&mut app, d, h);
    let before = live_volume(&app);
    qymcad_ui_state::select_body(&mut app.project, &mut app.chosen.sel, &mut app.viewing.view, body);
    crate::gui::commands::start_thread_cmd(&mut app.part_ctx());
    app.params.thread.auger = auger;
    qymcad_ui_state::set_thread_params(&mut app.tools.cmd, app.params.thread);
    app.params.thread.form = form;
    app.params.thread.src = Some(body);
    app.params.thread.edge = eid;
    app.params.thread.radius = d * 0.5;
    app.params.thread.axis = ([0.0, 0.0, 0.0], [0.0, 0.0, 1.0]);
    for (k, v) in params {
        if let Some(p) = app.tools.cmd.params.iter_mut().find(|p| p.key == *k) {
            p.val = *v;
            p.txt = format!("{v}");
        }
    }
    app.apply_feat_cmd();
    assert_eq!(app.tools.armed.cmd_kind(), 0, "the command finished: {}", app.status);
    (before - live_volume(&app), app)
}

/// A 20-cubed block through the full flows (sketch, then extrude). Returns the body's id.
pub(super) fn build_cube(app: &mut App) -> u64 {
    let si = sketch_rect(app, 0.0, 0.0, 20.0, 20.0);
    app.chosen.sel = Sel::Sketch(si);
    app.feat.op = 0;
    app.start_feat_cmd(1);
    if let Some(p) = app.tools.cmd.params.iter_mut().find(|p| p.key == "height") {
        p.val = 20.0;
        p.txt = "20".into();
    }
    app.apply_feat_cmd();
    let consumed = qymcad_ui_state::consumed_bodies(&app.project);
    *app.live.shapes.keys().find(|b| !consumed.contains(b)).expect("the block's body")
}

mod extrude;
mod shaft;
mod edits;
mod saving;
mod buttons;
mod picks;
