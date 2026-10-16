//! THE GIZMOS OF A BODY AND OF A COMPONENT: what a click hits on them and what a drag adds.

use super::*;

// ---------- helpers ----------

/// WHETHER TWO PLACEMENTS ARE ROTATED THE SAME WAY (the translation may be anything).
///
/// The scene buffer needs this: if a part has only moved, the vertices already built need the difference
/// added to them; if it was also rotated, the world normals became different, and both the colour and the
/// back-face culling are computed from them, so the block is assembled again. The translation columns (3,
/// 7, 11) say nothing about rotation, so they are skipped.
///
/// THE THRESHOLD COMES FROM MEASUREMENT, NOT FROM TASTE. Exact equality never happens here: the solver
/// derives the placements afresh every frame, and even on a PURE slider travel the rotating part still
/// breathes. Measured on a real assembly: 13.8 mm of translation per frame with a rotation discrepancy of
/// 1e-12 to 9e-10. With a threshold of 1e-12 the fast path was taken NOT ONCE (63 blocks out of 63 were
/// assembled again), that is, the fix was written and did not work.
///
/// 1e-7 is 6e-6 of a degree. On a part with a radius of a metre, a vertex moves 1e-4 mm from an
/// unrecognised turn of that size, which neither the eye nor an export will see. A real rotation (driving
/// a loop) goes in hundredths of a radian — five orders of magnitude away from the noise, and it honestly
/// rebuilds the block.
pub(crate) fn same_rotation12(a: &[f64; 12], b: &[f64; 12]) -> bool {
    (0..12).filter(|k| !matches!(k, 3 | 7 | 11)).all(|k| (a[k] - b[k]).abs() < 1e-7)
}

pub(crate) fn drag(ui: &mut egui::Ui, label: &str, v: &mut f64, speed: f64, range: std::ops::RangeInclusive<f64>) -> bool {
    ui.label(label);
    let changed = ui.add(egui::DragValue::new(v).speed(speed).range(range)).changed();
    ui.end_row();
    changed
}

/// The section's new offset while the gizmo is dragged — a DELTA from (`off0`, `p0`), the offset and the
/// screen cursor AT THE MOMENT THE GIZMO WAS GRABBED (the anchor). The offset used to be recomputed
/// ABSOLUTELY every frame, by reprojecting the CURRENT cursor relative to o0 (the world point at offset 0)
/// — but at the moment of the grab the cursor sits at the GIZMO ARROW'S TIP (`cp + n * diag * 0.35`, which
/// already includes both the current offset and the arrow's length) rather than at o0, so the very first
/// frame of the drag added the arrow's length to the offset in one jump. Reported behaviour: the plane
/// resets every time the gizmo is grabbed. A delta from the anchor gives 0 on the first frame (cur == p0)
/// and grows strictly in proportion to the mouse afterwards. `s0` and `s1` are the screen projections of o0
/// and o0 + normal (they set the direction and the pixels-per-millimetre scale along the normal); `None` is
/// the degenerate case where the normal points straight at the camera and its screen projection collapses
/// to a point.
pub(crate) fn section_drag_delta_offset(off0: f64, p0: Pos2, s0: Pos2, s1: Pos2, cur: Pos2) -> Option<f64> {
    let pd = s1 - s0;
    let den = (pd.x * pd.x + pd.y * pd.y) as f64;
    if den <= 1e-6 {
        return None;
    }
    let dt = ((cur.x - p0.x) * pd.x + (cur.y - p0.y) * pd.y) as f64 / den;
    Some(off0 + dt)
}

/// The local position (in the body's frame) of the vertex at an end of the persistent edge `edge`
/// (`end`: false is the start, true is the end). An initial snapshot for an associative datum point;
/// regeneration refines it through the kernel.
pub(crate) fn vertex_local_pos(live: &LiveGeom, body: Id, edge: u32, end: bool) -> Option<[f64; 3]> {
    let shape = live.shapes.get(&body)?;
    let (polys, ids) = shape.edges_with_ids();
    for (poly, id) in polys.iter().zip(ids) {
        if id == edge && poly.len() >= 2 {
            let v = if end { poly[poly.len() - 1] } else { poly[0] };
            return Some([v[0] as f64, v[1] as f64, v[2] as f64]);
        }
    }
    None
}

/// The axis of a gizmo with origin `o` and length `l` under the cursor — shared code for the COMPONENT
/// gizmo (Assembly) and the BODY gizmo (Part). The axes are the world X, Y and Z from `o`.
/// A CLICK (not a drag) on an arrow or a ring of the gizmo of what is chosen - a part's in an assembly, a body's in a
/// part: (what it stands on, axis 0/1/2, is it a rotation?). `None` when the click missed every handle.
pub(crate) fn gizmo_click_hit(pn: &qymcad_ui_state::Painting, rect: Rect, pos: Pos2, basis: &([f64; 3], [f64; 3], [f64; 3])) -> Option<(qymcad_ui_state::GizmoOf, u8, bool)> {
    use qymcad_ui_state::GizmoOf;
    let dc = qymcad_ui_state::DrawCtx { cam: &pn.cam, set: pn.set, scheme: pn.scheme, project: pn.project, active_path: pn.active_path };
    let (on, (o, l)) = match qymcad_ui_state::gizmo_component(pn.active_path, pn.project, pn.sel, pn.workbench).filter(|&c| matches!(qymcad_pick::comp_gizmo_mode(pn, c), CompGizmoMode::Free)) {
        Some(comp) => (GizmoOf::Part(comp), gizmo_geometry(pn.cam, pn.comp_giz, pn.project, comp)),
        None => {
            let (_, mi) = body_gizmo_target(pn.body_view(), pn.sel)?;
            (GizmoOf::Body(mi), body_gizmo_geometry(pn.body_giz, pn.cam, pn.project, pn.set, mi))
        }
    };
    if let Some(ax) = gizmo_axis_hit_at(&dc, o, l, rect, basis, pos) {
        return Some((on, ax, false));
    }
    gizmo_ring_hit_at(&dc, o, l, rect, basis, pos).map(|ax| (on, ax, true))
}

pub(crate) fn gizmo_axis_hit_at(dc: &qymcad_ui_state::DrawCtx, o: [f64; 3], l: f64, rect: Rect, basis: &([f64; 3], [f64; 3], [f64; 3]), pp: Pos2) -> Option<u8> {
    let scr = qymcad_ui_state::Screen { cam: dc.cam, set: dc.set, rect, basis };
    let s0 = scr.at(o).0;
    let mut best: Option<(f32, u8)> = None;
    for ax in 0..3u8 {
        let mut tip = o;
        tip[ax as usize] += l;
        let s1 = scr.at(tip).0;
        let d = screen_dist_seg(pp, s0, s1);
        if d <= 13.0 && best.is_none_or(|(bd, _)| d < bd) {
            best = Some((d, ax));
        }
    }
    best.map(|(_, a)| a)
}

/// The rotation ring of a gizmo with origin `o` and radius `l` under the cursor — shared code for the
/// Assembly and the Part.
pub(crate) fn gizmo_ring_hit_at(dc: &qymcad_ui_state::DrawCtx, o: [f64; 3], l: f64, rect: Rect, basis: &([f64; 3], [f64; 3], [f64; 3]), pp: Pos2) -> Option<u8> {
    let mut best: Option<(f32, u8)> = None;
    for ax in 0..3u8 {
        let (u, v) = ring_axes(ax);
        let mut prev: Option<Pos2> = None;
        let mut dmin = f32::MAX;
        for k in 0..=48 {
            let a = k as f64 / 48.0 * std::f64::consts::TAU;
            let p = [o[0] + l * (u[0] * a.cos() + v[0] * a.sin()), o[1] + l * (u[1] * a.cos() + v[1] * a.sin()), o[2] + l * (u[2] * a.cos() + v[2] * a.sin())];
            let s = qymcad_ui_state::Screen { cam: dc.cam, set: dc.set, rect, basis }.at(p).0;
            if let Some(pr) = prev {
                dmin = dmin.min(screen_dist_seg(pp, pr, s));
            }
            prev = Some(s);
        }
        if dmin <= 10.0 && best.is_none_or(|(bd, _)| dmin < bd) {
            best = Some((dmin, ax));
        }
    }
    best.map(|(_, a)| a)
}

/// Dragging a body gizmo's ring: accumulate the turn, in degrees, into the gizmo's own drag record.
///
/// WHICH BODY AND WHICH RING are read from the gizmo, not passed in. They used to be two more arguments,
/// filled at the call site out of the very same two fields - a second path by which they could disagree.
pub(crate) fn body_gizmo_ring_drag(body_giz: &mut BodyGizmo, scr: &Screen, project: &Project, regen: &mut Rebuilding, cursor: Pos2, d: egui::Vec2) {
    let (Some((mi, _, _)), Some(ax)) = (body_giz.drag, body_giz.ring) else { return };
    let (o, _) = body_gizmo_geometry(body_giz, *scr.cam, project, scr.set, mi);
    let center = scr.at(o).0;
    let radial = cursor - center;
    let r2 = (radial.x * radial.x + radial.y * radial.y) as f64;
    if r2 < 4.0 {
        return;
    }
    let ccw = -(radial.x * d.y - radial.y * d.x) as f64 / r2;
    let sign = ring_drag_sign(scr.basis.2[ax as usize]);
    if let Some(drag) = &mut body_giz.drag {
        drag.2 += ccw.to_degrees() * sign;
    }
    invalidate(regen);
}

/// Dragging a body gizmo's axis: accumulate the world shift along the grabbed axis into the gizmo's own
/// drag record. Which body and which axis are read from the gizmo, as above.
pub(crate) fn body_gizmo_axis_drag(body_giz: &mut BodyGizmo, scr: &Screen, project: &Project, regen: &mut Rebuilding, d: egui::Vec2) {
    let (Some((mi, _, _)), Some(ax)) = (body_giz.drag, body_giz.axis) else { return };
    let (o, l) = body_gizmo_geometry(body_giz, *scr.cam, project, scr.set, mi);
    let mut tip = o;
    tip[ax as usize] += l;
    let s0 = scr.at(o).0;
    let s1 = scr.at(tip).0;
    let pd = s1 - s0;
    let denom = (pd.x * pd.x + pd.y * pd.y) as f64;
    if denom < 1e-6 {
        return;
    }
    let inc = (d.x * pd.x + d.y * pd.y) as f64 * l / denom;
    if let Some(drag) = &mut body_giz.drag {
        drag.2 += inc;
    }
    invalidate(regen);
}
