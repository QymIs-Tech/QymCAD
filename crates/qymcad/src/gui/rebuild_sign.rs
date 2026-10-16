//! THE SIGN OF A BACKGROUND REBUILD over the frame: the veil and spinner of a quiet one, the dimming and card of a loud
//! one, and the barrier a loud one puts over the panels.

use qymcad_ui_state::{RebuildLook, RebuildRunning, Rebuilding, SchemeUi};

/// THE SIGN OF A BACKGROUND REBUILD, drawn each frame over the interface; the canvas `view_rect` stays live under it.
///
/// A loud rebuild mutes the panels from its first frame: an edit made from there would land on a stale copy of the
/// document. The barrier is a layer over the panels rather than events cleared in the poll of the job: egui gathers the
/// input of a pass at its start, and clearing them there came too late. Only the paint waits
/// (`qymcad_ui_state::rebuild_sign`). Reported behaviour: the veil and the spinner went up on the first frame of every
/// rebuild, and one landing in 20-50 ms was on screen for a frame or two - the window blinked on every edit. A sign held
/// after its rebuild has landed is paint only: the panels answer again, and its Cancel has nothing left to stop.
pub(crate) fn draw_rebuild_sign(regen: &Rebuilding, scheme: &SchemeUi, view_rect: egui::Rect, ctx: &egui::Context, status: &mut qymcad_ui_state::Notice) {
    let running = RebuildRunning::of(regen.busy.as_ref());
    if matches!(running.as_ref().map(|r| &r.look), Some(RebuildLook::Loud { .. })) {
        crate::gui::render::modal_input_barrier_except(ctx, view_rect);
    }
    match qymcad_ui_state::rebuild_sign_now(ctx, running.as_ref()).map(|s| s.look) {
        Some(RebuildLook::Quiet) => crate::gui::render::draw_quiet_spinner(scheme, view_rect, ctx),
        Some(RebuildLook::Loud { label, pulse }) => {
            if crate::gui::render::paint_dim_overlay(scheme, ctx, &label, pulse.map(|p| p.progress()), view_rect) {
                crate::gui::cancel_regen(regen, status);
            }
        }
        None => {}
    }
}
