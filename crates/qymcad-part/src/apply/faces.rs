//! Applying the commands of faces and surfaces, and the split.

use qymcad_ui_state::Notice;
use super::*;

/// Apply the trim: the sheet with the "keep" point plus the tool.
pub fn apply_trim_cmd(project: &mut Project, status: &mut qymcad_ui_state::Notice, trim: &qymcad_ui_state::TrimTool) -> Option<Id> {
    let Some((src, keep)) = trim.keep else {
        *status = Notice::info(qymcad_i18n::tr("msg-trim-pick-sheet"));
        return None;
    };
    let Some(tool) = trim.tool else {
        *status = Notice::info(qymcad_i18n::tr("msg-trim-pick-tool"));
        return None;
    };
    Some(project.add_trim(src, tool, keep))
}

/// Apply the stitch: two or more sheets.
pub fn apply_stitch_cmd(cmd: &qymcad_ui_state::FeatCommand, project: &mut Project, status: &mut qymcad_ui_state::Notice, stitch_parts: &[Id]) -> Option<Id> {
    if stitch_parts.len() < 2 {
        *status = Notice::refusal(qymcad_i18n::tr("msg-stitch-needs-two"));
        return None;
    }
    let body = project.add_stitch(stitch_parts.to_vec(), qymcad_ui_state::cmd_val(cmd, "tol"));
    store_cmd_exprs(cmd, project, body); // the tolerance stays parametric, like every number of a feature
    Some(body)
}

/// Apply the removal of faces.
///
/// APPLY THE FACE COPY: the faces are recorded AS A QUERY - a description if there is one, otherwise a
/// list of picks. A copy must travel with its base exactly as everything else does.
/// APPLY THE OFFSET: the picked faces, as a description if one was given, and the distance of the field.
pub fn apply_offset_surface_cmd(pc: &mut qymcad_ui_state::PartCtx) -> Option<Id> {
    let src = pc
        .gsel
        .faces_body
        .or_else(|| qymcad_ui_state::op_target_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, *pc.sel))?;
    if pc.gsel.faces.is_empty() {
        *pc.status = Notice::info(qymcad_i18n::tr("msg-offset-surface"));
        return None;
    }
    let picks: Vec<u32> = pc.gsel.faces.iter().copied().collect();
    let q = pc.gsel.recorded(&picks);
    let body = pc.project.add_offset_surface(src, q, qymcad_ui_state::cmd_val(pc.cmd, "dist"));
    store_cmd_exprs(pc.cmd, pc.project, body);
    Some(body)
}

pub fn apply_face_copy_cmd(pc: &mut qymcad_ui_state::PartCtx) -> Option<Id> {
    let src = pc
        .gsel
        .faces_body
        .or_else(|| qymcad_ui_state::op_target_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, *pc.sel))?;
    if pc.gsel.faces.is_empty() {
        *pc.status = Notice::refusal(qymcad_i18n::tr("msg-faces-not-found"));
        return None;
    }
    let picks: Vec<u32> = pc.gsel.faces.iter().copied().collect();
    let q = pc.gsel.recorded(&picks);
    Some(pc.project.add_face_copy(src, q))
}

/// WHETHER THE PICKED EDGES BOUND A PATCH: two or more, or one closed edge - the rim of a hole is a whole circle, and
/// the patch over it a disc, as in the professional systems. Reported behaviour: the rule for straight edges refused
/// the circle too.
pub(crate) fn patch_boundary(project: &Project, body: Id, gsel: &qymcad_ui_state::GeomSelection) -> bool {
    let Some(es) = project.regen_edges.get(&body) else { return false };
    let picked: Vec<&qymcad_core::geom::MeshEdge> = gsel.edges.iter().filter_map(|id| es.iter().find(|e| e.id == *id)).collect();
    if picked.is_empty() || picked.len() != gsel.edges.len() {
        return false;
    }
    // A BOUNDARY CLOSES: a ring (a circle, an ellipse) closes on itself, and every end of an open edge meets exactly one
    // other end. An edge picked past the loop - one more than the boundary needs - leaves ends that meet nothing, and
    // a surface spans no such thing: the node came in red, the kernel saying so after Enter.
    let key = |p: [f64; 3]| p.map(|c| (c * 1e4).round() as i64);
    let mut ends: std::collections::HashMap<[i64; 3], usize> = std::collections::HashMap::new();
    for e in &picked {
        let d = [e.a[0] - e.b[0], e.a[1] - e.b[1], e.a[2] - e.b[2]];
        if (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt() < 1e-6 {
            continue; // a ring
        }
        *ends.entry(key(e.a)).or_default() += 1;
        *ends.entry(key(e.b)).or_default() += 1;
    }
    ends.values().all(|&n| n == 2)
}

/// APPLY THE PATCH: the edges go in as a query (a description if there is one), otherwise as a list of
/// picks.
pub fn apply_patch_cmd(pc: &mut qymcad_ui_state::PartCtx) -> Option<Id> {
    let src = pc
        .edges
        .body
        .or_else(|| qymcad_ui_state::op_target_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, *pc.sel))?;
    if !patch_boundary(pc.project, src, pc.gsel) {
        *pc.status = Notice::refusal(qymcad_i18n::tr("msg-patch-needs-edges"));
        return None;
    }
    let picks: Vec<u32> = pc.gsel.edges.iter().copied().collect();
    let q = pc.gsel.recorded(&picks);
    Some(pc.project.add_patch(src, q, pc.opts.patch_tangent))
}

/// Apply the cut: the plane (as a reference - a datum or a world one) plus the offset. The number of
/// pieces is counted BEFORE the feature is created: the timeline must create exactly as many bodies as
/// will come out.
pub fn apply_split_cmd(pc: &mut qymcad_ui_state::PartCtx) -> Option<Id> {
    let src = qymcad_ui_state::op_target_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, *pc.sel)?;
    if pc.split.plane.is_none() {
        *pc.status = Notice::info(qymcad_i18n::tr("msg-click-cut-plane"));
        return None;
    }
    let Some(pieces) = split_piece_count(pc, src).filter(|&n| n >= 2) else {
        *pc.status = Notice::refusal(qymcad_i18n::tr("msg-plane-cuts-nothing"));
        return None;
    };
    let sp = pc.split.plane?;
    let (plane, datum, face) = qymcad_ui_state::resolve_mirror_plane(sp); // the same reference the mirror uses
    let offset = qymcad_ui_state::cmd_val(pc.cmd, "offset");
    let parts = pc.project.add_split_body(src, plane, datum, offset, pieces);
    let first = *parts.first()?;
    if let Some((b, k)) = face {
        pc.project.set_op_face(first, b, k);
    }
    store_cmd_exprs(pc.cmd, pc.project, first); // the offset stays parametric
    *pc.status = Notice::info(qymcad_i18n::tr1("cmd-split-done", "n", &pieces.to_string()));
    Some(first)
}

/// The cutting plane in THE LOCAL FRAME of the body (where its B-rep lives), with the offset already
/// applied. The pick gives world coordinates, and without the conversion a cut inside an assembly would
/// drift by the transform of the component.
pub fn split_plane_local(pc: &mut qymcad_ui_state::PartCtx, src: Id) -> Option<([f64; 3], [f64; 3])> {
    use qymcad_core::feature::{apply12, apply12_dir, mat_inv12};
    let sp = pc.split.plane?;
    let (o, n) = qymcad_ui_state::mirror_plane_world(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, &sp)?;
    let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    if l < 1e-9 {
        return None;
    }
    let u = [n[0] / l, n[1] / l, n[2] / l];
    let d = qymcad_ui_state::cmd_val(pc.cmd, "offset");
    let ow = [o[0] + u[0] * d, o[1] + u[1] * d, o[2] + u[2] * d];
    let inv = mat_inv12(&pc.project.body_display_transform(src, qymcad_ui_state::current_ctx_id(pc.active_path, pc.project)));
    Some((apply12(&inv, ow), apply12_dir(&inv, u)))
}

/// How many pieces a cutting plane yields - counted by the LIVE B-rep. It cannot be derived from the
/// plane itself: one plane cuts a U-shaped part into three, and "always two halves" would lose a piece.
/// `None` means the plane does not cut the body at all.
pub fn split_piece_count(pc: &mut qymcad_ui_state::PartCtx, src: Id) -> Option<usize> {
    let (o, n) = split_plane_local(pc, src)?;
    qymcad_ui_state::ensure_brep(&mut pc.rebuild()); // there is nothing to cut with while there is no live B-rep
    qymcad_kernel::ask::split_pieces(pc.live.shapes.get(&src)?, qymcad_core::feature::PlaneAt { origin: o, normal: n })
}

/// Apply the division of faces.
pub fn apply_split_face_cmd(pc: &mut qymcad_ui_state::PartCtx) -> Option<Id> {
    let src = qymcad_ui_state::op_target_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, *pc.sel)?;
    if pc.split.plane.is_none() {
        *pc.status = Notice::info(qymcad_i18n::tr("msg-click-plane"));
        return None;
    }
    // WHAT WILL COME OUT IS CHECKED BEFORE THE FEATURE IS CREATED: a plane that misses the body divides
    // nothing, and a node in the timeline that is certain to go red is of no use to anyone.
    let (o, n) = split_plane_local(pc, src)?;
    qymcad_ui_state::ensure_brep(&mut pc.rebuild());
    if !pc.live.shapes.get(&src).is_some_and(|sh| qymcad_kernel::ask::splits_faces(sh, qymcad_core::feature::PlaneAt { origin: o, normal: n })) {
        *pc.status = Notice::refusal(qymcad_i18n::tr("msg-plane-splits-nothing"));
        return None;
    }
    let sp = pc.split.plane?;
    let (plane, datum, face) = qymcad_ui_state::resolve_mirror_plane(sp);
    let offset = qymcad_ui_state::cmd_val(pc.cmd, "offset");
    let body = pc.project.add_split_face(src, plane, datum, offset);
    if let Some((b, k)) = face {
        pc.project.set_op_face(body, b, k);
    }
    store_cmd_exprs(pc.cmd, pc.project, body);
    *pc.status = Notice::info(qymcad_i18n::tr("msg-faces-split"));
    Some(body)
}

/// APPLY THE FACE REPLACEMENT: the faces go in as a query (a description if there is one), the surface
/// as a body.
pub fn apply_surface_replace_cmd(pc: &mut qymcad_ui_state::PartCtx) -> Option<Id> {
    let src = pc
        .gsel
        .faces_body
        .or_else(|| qymcad_ui_state::op_target_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, *pc.sel))?;
    let surface = (*pc.repl_surface)?;
    if pc.gsel.faces.is_empty() {
        *pc.status = Notice::refusal(qymcad_i18n::tr("msg-faces-not-found"));
        return None;
    }
    let picks: Vec<u32> = pc.gsel.faces.iter().copied().collect();
    // THE SHEET MUST CLOSE WHAT IS TAKEN AWAY, and that is asked of the live bodies before Enter lays a node: a sheet
    // over the top taken for the top and a side left the side open - the node came in red, "4 edges left unpaired".
    if let (Some(b), Some(sheet)) = (pc.live.shapes.get(&src), pc.live.shapes.get(&surface)) {
        if let Err(free) = qymcad_kernel::ask::sheet_closes(b, &picks, sheet) {
            *pc.status = Notice::refusal(qymcad_i18n::error_words::error_text(&qymcad_core::errors::CoreError::SurfaceDoesNotClose { free }));
            return None;
        }
    }
    let q = pc.gsel.recorded(&picks);
    Some(pc.project.add_surface_replace(src, q, surface))
}

pub fn apply_remove_face_cmd(pc: &mut qymcad_ui_state::PartCtx) -> Option<Id> {
    let src = qymcad_ui_state::op_target_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, *pc.sel)?;
    if pc.gsel.faces.is_empty() {
        *pc.status = Notice::info(qymcad_i18n::tr("msg-click-faces-remove"));
        return None;
    }
    let keys: Vec<qymcad_core::feature::FaceKey> = pc
        .project
        .mesh_index(src)
        .and_then(|mi| pc.project.bodies.get(mi))
        .map(|b| {
            b.faces
                .iter()
                .filter(|f| pc.gsel.faces.contains(&f.id))
                .map(|f| qymcad_core::feature::FaceKey { index: 0, centroid: [f.centroid.x, f.centroid.y, f.centroid.z], normal: f.normal, id: f.id })
                .collect()
        })
        .unwrap_or_default();
    if keys.is_empty() {
        *pc.status = Notice::refusal(qymcad_i18n::tr("msg-faces-not-found"));
        return None;
    }
    Some(pc.project.add_remove_face(src, keys))
}
