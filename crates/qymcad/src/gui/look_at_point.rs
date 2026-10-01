//! LOOKING AT A POINT: a short click of the middle button, under the layouts whose programs have it, brings the point
//! under the pointer to the middle of the view and turns the view about it from then on.
//!
//! THE SAME POINT IS THE ORBIT PIVOT: a turn latches it for the gesture and keeps it still on screen (no yank of the
//! camera centre to the cursor). Preference: a face hit, else the world/datum/sketch plane under the pointer, else
//! the view plane through the present centre.
use egui::{Pos2, Rect, Response};
use qymcad_core::model::{Id, Project};

/// THE POINT UNDER THE POINTER that an orbit or a look-at turns about.
pub(crate) fn orbit_pivot(pn: &qymcad_ui_state::Painting, rect: Rect, at: Pos2) -> [f64; 3] {
    // 1) a face of a body: the hit on the triangle, not a stand-in for the face
    if let Some((_, _, w)) = qymcad_pick::pick_face_ray(pn, rect, at) {
        return w;
    }
    // 2) a world / datum plane (or a vertex / datum-point snap) under the cursor — same plane the place-tool uses
    if let Some(frame) = qymcad_pick::pick_place_frame_at(pn, rect, at) {
        return [frame[3], frame[7], frame[11]];
    }
    // 3) the plane through the present centre, square to the view
    let basis = pn.cam.basis();
    let centre = qymcad_ui_state::Screen { cam: &pn.cam, set: pn.set, rect, basis: &basis }.at(pn.cam.target).0;
    let (right, up, _) = basis;
    let (dx, dy, k) = ((at.x - centre.x) as f64, (at.y - centre.y) as f64, 1.0 / pn.cam.scale as f64);
    let t = pn.cam.target;
    [t[0] + (right[0] * dx - up[0] * dy) * k, t[1] + (right[1] * dx - up[1] * dy) * k, t[2] + (right[2] * dx - up[2] * dy) * k]
}

/// Where the view is to look, once a middle click asks for it under `nav`.
pub(crate) fn look_at(pn: &qymcad_ui_state::Painting, nav: qymcad_ui_state::MouseNav, resp: &Response, rect: Rect) -> Option<[f64; 3]> {
    if !nav.middle_click_looks() || !resp.middle_clicked() {
        return None;
    }
    let at = resp.interact_pointer_pos()?;
    Some(orbit_pivot(pn, rect, at))
}

/// The status line of a context stepped into: its name, as the window names it.
pub(crate) fn context_is(project: &Project, active_path: &[Id]) -> String {
    let ctx = qymcad_ui_state::current_ctx_id(active_path, project);
    let name = project.components.iter().find(|c| c.id == ctx).map(|c| crate::i18n::name(&c.name)).unwrap_or_default();
    crate::i18n::tr1("g-context-is", "name", &name)
}
