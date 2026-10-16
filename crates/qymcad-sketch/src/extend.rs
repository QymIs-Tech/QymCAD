//! PROJECT AND EXTEND: an edge of a body taken into the sketch, a line extended to what it meets.

use qymcad_ui_state::Notice;
use super::*;

/// Project into the sketch whatever was clicked: an edge of the underlay, or - in "face contour" mode -
/// the whole contour of the face the sketch stands on.
pub fn project_clicked_edge(sk: &mut qymcad_ui_state::SketchCtx, si: usize, rect: Rect, pos: Pos2) {
    use qymcad_core::feature::SketchPlane;
    use qymcad_core::model::ProjSource;
    // "FACE CONTOUR" MODE works only for a sketch seated ON A FACE: a sketch on a world plane has no host
    // face, and projecting its contour would mean guessing.
    let face_mode = sk.tool.proj_face;
    let src = match (face_mode, sk.project.sketches.get(si).map(|s| s.plane)) {
        (true, Some(SketchPlane::Face(b, key))) if key.id != 0 => Some((sk.project.live_body(b), ProjSource::Face(key.id))),
        (true, _) => {
            *sk.status = Notice::refusal(qymcad_i18n::tr("sk-face-outline-only"));
            return;
        }
        _ => nearest_ref_edge(sk, si, rect, pos).map(|(b, e)| (b, ProjSource::Edge(e))),
    };
    let Some((body, src)) = src else {
        *sk.status = Notice::refusal(qymcad_i18n::tr("sk-miss-click-edge"));
        return;
    };
    qymcad_ui_state::begin_edit(sk.edits, sk.project, qymcad_i18n::tr("sk-project")); // THE BOUNDARY OF AN OPERATION, as with other sketch edits
    let id = qymcad_ui_state::with_kernel(&mut sk.rebuild(), |project, k| project.add_sketch_projection(si, body, src, k));
    *sk.status = if id == 0 { Notice::refusal(qymcad_i18n::tr("sk-not-projectable")) } else { Notice::info(qymcad_i18n::tr("sk-projected-hint")) };
    sk.project.solve_sketch(si);
    qymcad_ui_state::commit_edit(&mut sk.rebuild());
    qymcad_ui_state::invalidate(sk.regen);
}

/// A POINT PUT ON AN EDGE OF THE BODY UNDER THE SKETCH TAKES THE EDGE INTO IT: the edge is projected (`ProjSource::Edge`)
/// as construction geometry - it holds what is drawn and makes no contour - so the point put on its end is that end, and
/// a point put along it or at its middle is tied there (`tie_to_the_line_under`). The edge then moves with the body, and
/// what is drawn from it follows. Nothing when a curve of the sketch lies there already. Reported behaviour: "even
/// without projecting, tie to the edges of the face (the thin grey ones) - at a rebuild everything falls apart" - a point
/// put on a grey edge was held by nothing, and the body moved away from under it.
pub(crate) fn auto_project_under(sk: &mut qymcad_ui_state::SketchCtx, rect: Rect, si: usize, at: Point2) {
    use qymcad_core::model::ProjSource;
    let pos = (qymcad_ui_state::Sheet { view: *sk.view, rect }).at(at);
    if qymcad_pick::nearest_line_eid(&sk.pick(), rect, pos, si).is_some() || qymcad_pick::nearest_circle_entity(&sk.pick(), rect, pos, si).is_some() {
        return;
    }
    let Some((body, edge)) = nearest_ref_edge(sk, si, rect, pos) else { return };
    let id = qymcad_ui_state::with_kernel(&mut sk.rebuild(), |project, k| project.add_sketch_projection(si, body, ProjSource::Edge(edge), k));
    let Some(proj) = sk.project.sketches[si].projections.iter().find(|p| p.id == id) else { return };
    let curves = proj.entities.clone();
    for e in sk.project.sketches[si].entities.iter_mut().filter(|e| curves.contains(&e.id)) {
        e.construction = true;
    }
}

/// WHERE A CLICK OR AN ENTER OF EXTEND LANDS: the canvas, the pointer on it, the sketch, and the line under the pointer.
#[derive(Clone, Copy)]
pub(crate) struct ExtendAt {
    pub(crate) rect: Rect,
    pub(crate) pos: Pos2,
    pub(crate) si: usize,
    pub(crate) line: Option<Id>,
}

/// WHAT EXTEND ASKS OF ITS LINE with the pointer at `pos`: the pointer in the sketch, the curve under it, and the ends
/// "Both sides" gives.
pub(crate) fn extend_ask(sk: &mut qymcad_ui_state::SketchCtx, rect: Rect, pos: Pos2, si: usize) -> qymcad_core::model::ExtendAsk {
    let w = qymcad_ui_state::to_world(&*sk.view, rect, pos);
    let held = sk.tool.extend;
    let over = qymcad_pick::nearest_line_eid(&sk.pick(), rect, pos, si).filter(|e| Some(*e) != held).or_else(|| qymcad_pick::nearest_circle_entity(&sk.pick(), rect, pos, si));
    let sides = if sk.tool_prefs.extend_both { qymcad_core::model::ExtendSides::Both } else { qymcad_core::model::ExtendSides::Nearer };
    qymcad_core::model::ExtendAsk { pointer: Point2::new(w.x, w.y), over, sides }
}

/// A CLICK OF EXTEND: with no line held, the line clicked is taken and its extension previewed; with one held, the
/// extension the preview shows is made and the line let go. A line meeting nothing on the side asked says so.
pub(crate) fn extend_click(sk: &mut qymcad_ui_state::SketchCtx, at: ExtendAt) {
    let Some(held) = sk.tool.extend else {
        sk.tool.extend = at.line;
        *sk.status = Notice::info(qymcad_i18n::tr("opt-extend-to-hint"));
        return;
    };
    extend_apply(sk, at, held);
}

/// THE EXTENSION OF LINE `held` MADE as the pointer at `at` asks, and the line let go.
pub(crate) fn extend_apply(sk: &mut qymcad_ui_state::SketchCtx, at: ExtendAt, held: Id) {
    let ask = extend_ask(sk, at.rect, at.pos, at.si);
    let ext = sk.project.line_extension(at.si, held, &ask);
    if sk.project.extend_line_by(at.si, held, ext) {
        sk.tool.extend = None;
        sk.sel_sk.clear(); // the selection and whatever was waiting for it
        qymcad_ui_state::invalidate(&mut *sk.regen);
        *sk.status = Notice::info(qymcad_i18n::tr("sk-done"));
    } else if ext.kept == qymcad_core::model::EndKept::Joined {
        *sk.status = Notice::refusal(qymcad_i18n::tr("sk-extend-end-joined"));
    } else {
        *sk.status = Notice::refusal(qymcad_i18n::tr("sk-op-failed-no-intersection"));
    }
}
