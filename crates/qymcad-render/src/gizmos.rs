//! THE GIZMOS DRAWN: of a body, of a component, of a joint, of a section, the handles of a command.

use super::*;

/// Draw the gizmo (3 axis arrows + 3 rings) at origin `o` and scale `l`, with the hot axis/ring lit.
/// The shared renderer for the COMPONENT gizmo (Assembly) and the BODY gizmo (Part).
pub fn gizmo_at(dc: &DrawCtx, painter: &egui::Painter, rect: Rect, o: [f64; 3], l: f64, hot_axis: Option<u8>, hot_ring: Option<u8>) {
    let basis = dc.cam.basis();
    let scr = qymcad_ui_state::Screen { cam: dc.cam, set: dc.set, rect, basis: &basis };
    let s0 = scr.at(o).0;
    let cols = [dc.scheme.pal.axis(0), dc.scheme.pal.axis(1), dc.scheme.pal.axis(2)];
    // the rotation rings (under the arrows). THE INDEX IS AN AXIS NUMBER: it picks the ring's plane and
    // its colour, so it is the value the loop is about.
    #[allow(clippy::needless_range_loop)]
    for ax in 0..3usize {
        let (u, v) = qymcad_ui_state::ring_axes(ax as u8);
        let hot = hot_ring == Some(ax as u8);
        let pts: Vec<Pos2> = (0..=48)
            .map(|k| {
                let a = k as f64 / 48.0 * std::f64::consts::TAU;
                let p = [o[0] + l * (u[0] * a.cos() + v[0] * a.sin()), o[1] + l * (u[1] * a.cos() + v[1] * a.sin()), o[2] + l * (u[2] * a.cos() + v[2] * a.sin())];
                scr.at(p).0
            })
            .collect();
        painter.add(egui::Shape::line(pts, Stroke::new(if hot { 2.8 } else { 1.3 }, cols[ax])));
    }
    // the axis arrows (over the rings)
    for ax in 0..3usize {
        let mut tip = o;
        tip[ax] += l;
        let s1 = scr.at(tip).0;
        let hot = hot_axis == Some(ax as u8);
        painter.line_segment([s0, s1], Stroke::new(if hot { 4.0 } else { 2.5 }, cols[ax]));
        painter.circle_filled(s1, if hot { 7.5 } else { 5.0 }, cols[ax]);
    }
}

/// Draw the body gizmo: 3 axes + rings at the selected body + a readout of the value during a drag.
pub fn draw_body_gizmo(pn: &Painting, painter: &egui::Painter, rect: Rect) {
    let Some((_, mi)) = qymcad_ui_state::body_gizmo_target(pn.body_view(), pn.sel) else {
        return;
    };
    let (o, l) = qymcad_ui_state::body_gizmo_geometry(pn.body_giz, pn.cam, pn.project, pn.set, mi);
    gizmo_at(&DrawCtx { cam: &pn.cam, set: pn.set, scheme: pn.scheme, project: pn.project, active_path: pn.active_path }, painter, rect, o, l, pn.body_giz.axis, pn.body_giz.ring);
    if let Some(text) = qymcad_ui_state::body_giz_readout(pn.body_giz, pn.set, pn.body_giz.snap) {
        let s = qymcad_ui_state::Screen { cam: &pn.cam, set: pn.set, rect, basis: &pn.cam.basis() }.at(o).0;
        let suffix = if pn.body_giz.snap { "  snap" } else { "" };
        painter.text(s + egui::vec2(14.0, -14.0), egui::Align2::LEFT_BOTTOM, format!("{text}{suffix}"), egui::FontId::proportional(13.0), pn.scheme.pal.gizmo_label());
    }
}

/// Draw the DOF gizmo: only the handles of the joint's freedoms (rings/arrows along the motion axes) + a readout.
pub fn draw_joint_gizmo(pn: &Painting, painter: &egui::Painter, rect: Rect, jid: Id) {
    let Some(qymcad_ui_state::JointGizmo { origin: o, handles: hs }) =
        qymcad_ui_state::joint_giz_handles(&DrawCtx { cam: &pn.cam, set: pn.set, scheme: pn.scheme, project: pn.project, active_path: pn.active_path }, jid)
    else {
        return;
    };
    let basis = pn.cam.basis();
    let l = 60.0 / pn.cam.scale as f64;
    let scr = qymcad_ui_state::Screen { cam: &pn.cam, set: pn.set, rect, basis: &basis };
    let s0 = scr.at(o).0;
    let hot = pn.joint.giz_handle;
    let col_ring = pn.scheme.pal.active(); // yellow means "a joint freedom" (matching the selected joint)
    let col_arr = pn.scheme.pal.preview();
    // the rings
    for &qymcad_ui_state::JointHandle { slot, ring, dir } in hs.iter().filter(|h| h.ring) {
        if !ring {
            continue;
        }
        let is_hot = hot == Some((slot, true));
        let (u, v) = qymcad_ui_state::perp_basis(dir);
        let pts: Vec<Pos2> = (0..=48)
            .map(|k| {
                let a = k as f64 / 48.0 * std::f64::consts::TAU;
                let p = [o[0] + l * (u[0] * a.cos() + v[0] * a.sin()), o[1] + l * (u[1] * a.cos() + v[1] * a.sin()), o[2] + l * (u[2] * a.cos() + v[2] * a.sin())];
                scr.at(p).0
            })
            .collect();
        painter.add(egui::Shape::line(pts, Stroke::new(if is_hot { 3.2 } else { 1.8 }, col_ring)));
    }
    // the arrows
    for &qymcad_ui_state::JointHandle { slot, ring, dir } in hs.iter().filter(|h| !h.ring) {
        if ring {
            continue;
        }
        let is_hot = hot == Some((slot, false));
        let s1 = scr.at([o[0] + dir[0] * l, o[1] + dir[1] * l, o[2] + dir[2] * l]).0;
        painter.line_segment([s0, s1], Stroke::new(if is_hot { 4.0 } else { 2.5 }, col_arr));
        painter.circle_filled(s1, if is_hot { 7.5 } else { 5.0 }, col_arr);
    }
    // THE LIMITS ARE VISIBLE. The range of a degree of freedom is held by the solver and stops the drag,
    // but it used to exist only in the min/max fields: a person dragged the handle and hit an invisible
    // wall with no idea where it came from. Now a limited degree has its range drawn - dashed from the
    // minimum to the maximum - with cross ticks marking the stops at the ends.
    joint_limits(&DrawCtx { cam: &pn.cam, set: pn.set, scheme: pn.scheme, project: pn.project, active_path: pn.active_path }, painter, rect, jid, o, l, &hs);
    // THE PICKED AXIS IS VISIBLE. A person pointed at an edge and must see WHAT exactly they pointed at.
    // Otherwise "specify the axis" turns into an act of faith: something was picked, the part moved
    // somehow, and there is no way to check whether the one matches the other.
    joint_axis_refs(&DrawCtx { cam: &pn.cam, set: pn.set, scheme: pn.scheme, project: pn.project, active_path: pn.active_path }, painter, rect, jid, l);
    // the central marker of the joint axis
    painter.circle_stroke(s0, 3.0, Stroke::new(1.5, col_ring));
    // the readout of the current value during a drag
    if let Some((val, ring)) = qymcad_ui_state::joint_giz_value(pn.joint, pn.set, pn.comp_giz.snap) {
        let txt = if ring { format!("{val:+.1}{}", qymcad_i18n::tr("unit-deg-suffix")) } else { qymcad_i18n::tr1("unit-mm-value", "v", &qymcad_i18n::num_signed(val, 2)) };
        let suffix = if pn.comp_giz.snap { "  snap" } else { "" };
        painter.text(s0 + egui::vec2(14.0, -14.0), egui::Align2::LEFT_BOTTOM, format!("{txt}{suffix}"), egui::FontId::proportional(13.0), pn.scheme.pal.gizmo_label());
    }
}

/// Draw the component placement gizmo: 3 axis arrows X/Y/Z + 3 rotation rings.
pub fn draw_component_gizmo(pn: &Painting, painter: &egui::Painter, rect: Rect) {
    // the JOINT's DOF gizmo (a driven component OR a directly picked glyph, the root's GLOBAL included)
    if let Some(jid) = qymcad_pick::active_dof_joint(pn) {
        draw_joint_gizmo(pn, painter, rect, jid);
        return;
    }
    let Some(comp) = qymcad_ui_state::gizmo_component(pn.active_path, pn.project, pn.sel, pn.workbench) else {
        return;
    };
    // driven by a joint -> the DOF gizmo (the joint's freedoms only); grounded -> no gizmo; free -> 6 DOF.
    match qymcad_pick::comp_gizmo_mode(pn, comp) {
        CompGizmoMode::None => {}
        CompGizmoMode::Joint(jid) => draw_joint_gizmo(pn, painter, rect, jid),
        CompGizmoMode::Free => {
            let (o, l) = qymcad_ui_state::gizmo_geometry(pn.cam, pn.comp_giz, pn.project, comp);
            gizmo_at(&DrawCtx { cam: &pn.cam, set: pn.set, scheme: pn.scheme, project: pn.project, active_path: pn.active_path }, painter, rect, o, l, pn.comp_giz.axis, pn.comp_giz.ring);
            // the readout of the translation/rotation at the gizmo during a drag (as for a body)
            if let Some(text) = qymcad_ui_state::comp_giz_readout(&pn.comp_giz, pn.set, pn.comp_giz.snap) {
                let s = qymcad_ui_state::Screen { cam: &pn.cam, set: pn.set, rect, basis: &pn.cam.basis() }.at(o).0;
                let suffix = if pn.comp_giz.snap { "  snap" } else { "" };
                painter.text(s + egui::vec2(14.0, -14.0), egui::Align2::LEFT_BOTTOM, format!("{text}{suffix}"), egui::FontId::proportional(13.0), pn.scheme.pal.gizmo_label());
            }
        }
    }
}

/// Draw the section PLANE (a translucent quad across the scene) + the offset ARROW gizmo.
pub fn draw_section_gizmo(pn: &Painting, painter: &egui::Painter, rect: Rect) {
    let Some(qymcad_ui_state::SectionGizmo { centre: cp, u, v, half, tip }) = qymcad_ui_state::section_gizmo_geom(pn) else { return };
    let basis = pn.cam.basis();
    let pr = |p: [f64; 3]| qymcad_ui_state::Screen { cam: &pn.cam, set: pn.set, rect, basis: &basis }.at(p).0;
    let corners = [
        [cp[0] + (u[0] + v[0]) * half, cp[1] + (u[1] + v[1]) * half, cp[2] + (u[2] + v[2]) * half],
        [cp[0] + (u[0] - v[0]) * half, cp[1] + (u[1] - v[1]) * half, cp[2] + (u[2] - v[2]) * half],
        [cp[0] - (u[0] + v[0]) * half, cp[1] - (u[1] + v[1]) * half, cp[2] - (u[2] + v[2]) * half],
        [cp[0] - (u[0] - v[0]) * half, cp[1] - (u[1] - v[1]) * half, cp[2] - (u[2] - v[2]) * half],
    ];
    let pts: Vec<Pos2> = corners.iter().map(|c| pr(*c)).collect();
    let fill = qymcad_scheme::a(pn.scheme.pal.plane_fill(), 26);
    let edge = pn.scheme.pal.plane_face();
    painter.add(egui::Shape::convex_polygon(pts.clone(), fill, Stroke::new(1.5, edge)));
    // the normal arrow (dragging it = offsetting the section)
    let (a, b) = (pr(cp), pr(tip));
    painter.line_segment([a, b], Stroke::new(2.5, edge));
    let dirv = (b - a).normalized();
    let nn = egui::vec2(-dirv.y, dirv.x);
    painter.add(egui::Shape::convex_polygon(vec![b, b - dirv * 12.0 + nn * 5.0, b - dirv * 12.0 - nn * 5.0], edge, Stroke::NONE));
    let hot = pn.section.drag;
    painter.circle_filled(b, if hot { 7.0 } else { 5.5 }, if hot { pn.scheme.pal.active() } else { edge });
}

/// A HANDLE AT A FACE: an arrow along the normal that the mouse drags. It is visible for as long as
/// the face is selected - even at a zero value. Otherwise there would be nothing to drag: the arrow
/// would appear only after the number had been typed in, that is, exactly when it is no longer needed.
///
/// One drawing for every handle of a command (`handles`): the arrow of a value along a direction of a face pushed,
/// thickened or shelled, a split or a datum plane's offset, the three arrows of a datum point's or axis's position, and
/// the rings of a datum plane's tilts and turn and of a datum axis's direction. Where they stand is remembered for the
/// fields of the command to stand clear of them (`remember_handles`).
pub fn draw_face_arrow(pn: &Painting, painter: &egui::Painter, rect: Rect, basis: &([f64; 3], [f64; 3], [f64; 3])) {
    let scr = qymcad_ui_state::Screen { cam: &pn.cam, set: pn.set, rect, basis };
    let hs = qymcad_ui_state::handles(pn);
    // the rings under the arrows, as on a body's gizmo
    for ring in &hs.rings {
        let hot = pn.handle_drag == Some(qymcad_ui_state::Handle::Ring(ring.key));
        let col = if hot { pn.scheme.pal.highlight() } else { pn.scheme.pal.handle_face() };
        let pts: Vec<Pos2> = ring.points().into_iter().map(|p| scr.at(p).0).collect();
        painter.add(egui::Shape::line(pts, Stroke::new(if hot { 2.8 } else { 1.6 }, col)));
    }
    for arrow in &hs.arrows {
        let (a, b) = (scr.at(arrow.base).0, scr.at(arrow.tip()).0);
        let hot = pn.handle_drag == Some(qymcad_ui_state::Handle::Arrow(arrow.key));
        let col = if hot { pn.scheme.pal.highlight() } else { pn.scheme.pal.handle_face() };
        painter.add(egui::Shape::line_segment([a, b], Stroke::new(if hot { 3.5 } else { 2.5 }, col)));
        painter.circle_filled(b, if hot { 6.0 } else { 5.0 }, col);
    }
    qymcad_ui_state::remember_handles(painter.ctx(), qymcad_ui_state::handles_screen_rect(&hs, &scr));
}
