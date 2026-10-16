//! LOOKING AT A POINT: a short click of the middle button, under the layouts whose programs have it, brings the point
//! under the pointer to the middle of the view and turns the view about it from then on - the point of the model hit
//! there, or, over empty space, the point of the plane through the present centre square to the view.
use egui::{Rect, Response};
use qymcad_core::model::{Id, Project};

/// Where the view is to look, once a middle click asks for it under `nav`.
pub(crate) fn look_at(pn: &qymcad_ui_state::Painting, nav: qymcad_ui_state::MouseNav, resp: &Response, rect: Rect) -> Option<[f64; 3]> {
    if !nav.middle_click_looks() || !resp.middle_clicked() {
        return None;
    }
    Some(point_under(pn, rect, resp.interact_pointer_pos()?))
}

/// THE POINT UNDER SCREEN `at` that the view looks at, turns about or zooms to: the point of the model hit there, or, over
/// empty space, the point of the ray there at the depth of the part seen nearest to `at` - and only with no part in front
/// of the eye at all, the point of the plane through the present centre square to the view.
///
/// The plane through the centre was the only answer over empty space, and after a view framed at 4 m and wheeled down
/// to a part that centre stood metres away from anything on screen: the view turned about a point far off the part.
pub(crate) fn point_under(pn: &qymcad_ui_state::Painting, rect: Rect, at: egui::Pos2) -> [f64; 3] {
    if let Some((_, _, w)) = qymcad_pick::pick_face_ray(pn, rect, at) {
        return w;
    }
    let depth = nearest_part_depth(pn, rect, at).unwrap_or(0.0);
    qymcad_ui_state::on_ray_at_depth(&pn.cam, pn.set, rect, at, depth)
}

/// The depth, from the plane of the centre along the line of sight, of the middle of the part whose box lands nearest to
/// screen `at` - of the parts standing wholly in front of the eye.
fn nearest_part_depth(pn: &qymcad_ui_state::Painting, rect: Rect, at: egui::Pos2) -> Option<f64> {
    let basis = pn.cam.basis();
    let scr = qymcad_ui_state::Screen { cam: &pn.cam, set: pn.set, rect, basis: &basis };
    let inv_d = qymcad_ui_state::persp_inv_d_eye(&pn.cam, pn.set, rect.height() * 0.5);
    let depth = |p: [f64; 3]| qymcad_ui_state::v_dot(qymcad_ui_state::v_sub(p, pn.cam.target), basis.2);
    let mut best: Option<[f64; 3]> = None;
    let mut best_gap = f32::INFINITY;
    for item in qymcad_ui_state::visible_mesh_items(pn) {
        let Some(b) = item.mesh.bounds() else { continue };
        let corners: Vec<[f64; 3]> =
            (0..8).map(|k| qymcad_core::feature::apply12(&item.world, [[b.min.x, b.max.x][k & 1], [b.min.y, b.max.y][(k >> 1) & 1], [b.min.z, b.max.z][(k >> 2) & 1]])).collect();
        if inv_d > 0.0 && corners.iter().any(|c| depth(*c) + 1.0 / inv_d <= 0.0) {
            continue; // behind the eye, or cut by it: not a part seen
        }
        let on = egui::Rect::from_points(&corners.iter().map(|c| scr.at(*c).0).collect::<Vec<_>>());
        let gap = (at - on.clamp(at)).length();
        if gap < best_gap {
            best_gap = gap;
            best = Some(std::array::from_fn(|a| (corners[0][a] + corners[7][a]) * 0.5));
        }
    }
    best.map(depth)
}

/// THE WHEEL OVER THE 3D VIEW: the camera after a zoom of `scroll` holding the place on screen the rule names
/// (`zoom_anchor`) and the point of the model seen there.
pub(crate) fn wheeled(pn: &qymcad_ui_state::Painting, rect: Rect, anchor: egui::Pos2, scroll: f32) -> qymcad_ui_state::Cam3 {
    let mut cam = pn.cam;
    qymcad_ui_state::wheel_zoom_3d(&mut cam, pn.set, rect, qymcad_ui_state::Held { anchor, point: point_under(pn, rect, anchor) }, scroll);
    cam
}

/// The status line of a context stepped into: its name, as the window names it.
pub(crate) fn context_is(project: &Project, active_path: &[Id]) -> String {
    let ctx = qymcad_ui_state::current_ctx_id(active_path, project);
    let name = project.components.iter().find(|c| c.id == ctx).map(|c| crate::i18n::name(&c.name)).unwrap_or_default();
    crate::i18n::tr1("g-context-is", "name", &name)
}
