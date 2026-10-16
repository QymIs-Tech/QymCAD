//! THE AXIS OF A CIRCULAR PATTERN, PICKED IN THE VIEW - of bodies in a part and of parts in an assembly alike.
//!
//! A click lands on a datum axis, a straight edge or a cylindrical face, and the pattern turns about it; or on two
//! points one after the other - datum points or vertices - and the axis runs through them. Every one of them gives an
//! axis that follows its source: a vertex is taken as a datum point standing on it.
use qymcad_ui_state::Notice;
use super::*;

/// What a click in the view aims at while an axis is being picked.
pub(crate) enum ArrayAxisAim {
    Axis(AxisHit),
    Point(qymcad_ui_state::AxisPoint),
    Miss,
}

/// WHERE THE CLICK AT `pos` LANDS: a point under the cursor - a datum point, a vertex - before the edge it ends, as a
/// point is picked first everywhere; then an axis.
pub(crate) fn aim(pn: &Painting, rect: egui::Rect, pos: egui::Pos2) -> ArrayAxisAim {
    if let Some((id, at)) = crate::gui::pick::pick_datum_point_at(pn, rect, pos) {
        return ArrayAxisAim::Point(qymcad_ui_state::AxisPoint::Datum(id, at));
    }
    if let (Some((body, edge, end)), Some(at)) = (crate::gui::pick::pick_vertex_any(pn, rect, pos), crate::gui::pick::pick_vertex_pos(pn, rect, pos)) {
        return ArrayAxisAim::Point(qymcad_ui_state::AxisPoint::Vertex { body, edge, end, at });
    }
    match crate::gui::pick::pick_axis_at(pn, rect, pos) {
        Some(h) => ArrayAxisAim::Axis(h),
        None => ArrayAxisAim::Miss,
    }
}

/// TAKE WHAT THE CLICK AIMED AT as the axis of the pattern, or as the first of its two points.
pub(crate) fn take(pc: &mut PartCtx, at: ArrayAxisAim) {
    let axis = match at {
        ArrayAxisAim::Axis(AxisHit::Datum(id)) => Ok((id, "vp-array-axis-datum")),
        ArrayAxisAim::Axis(AxisHit::Edge(i)) => crate::gui::axis_from_edge(pc.active_path, pc.edges, pc.live, pc.project, i).map(|id| (id, "vp-array-axis-edge")).ok_or("vp-edge-not-axis"),
        ArrayAxisAim::Axis(AxisHit::Face(body, fid)) => {
            crate::gui::axis_from_face(pc.active_path, pc.edges, pc.live, pc.project, body, fid).map(|id| (id, "vp-array-axis-cyl")).ok_or("vp-face-has-no-axis")
        }
        ArrayAxisAim::Point(p) => match pc.arr.axis_first.take() {
            None => {
                pc.arr.axis_first = Some(p);
                *pc.status = Notice::info(crate::i18n::tr("vp-array-axis-point1"));
                return;
            }
            Some(first) => through_two_points(pc.project, pc.live, first, p).map(|id| (id, "vp-array-axis-points")).ok_or("msg-points-coincide"),
        },
        ArrayAxisAim::Miss => Err("vp-miss-axis"),
    };
    match axis {
        Ok((id, words)) => {
            (pc.arr.axis, pc.arr.axis_pick, pc.arr.axis_first) = (id, false, None);
            *pc.status = Notice::info(crate::i18n::tr(words));
        }
        Err(words) => *pc.status = Notice::refusal(crate::i18n::tr(words)),
    }
}

/// THE AXIS THROUGH TWO POINTS, following them: a datum point as it is, a vertex as a datum point standing on it (by
/// its edge and end, which survive a rebuild). Reported behaviour: an axis through two vertices stayed where they had
/// stood when the part changed.
fn through_two_points(project: &mut Project, live: &LiveGeom, a: qymcad_ui_state::AxisPoint, b: qymcad_ui_state::AxisPoint) -> Option<Id> {
    let (pa, pb) = (a.at(), b.at());
    let d = [pb[0] - pa[0], pb[1] - pa[1], pb[2] - pa[2]];
    if d[0].abs() + d[1].abs() + d[2].abs() < 1e-9 {
        return None;
    }
    let mut point = |p: qymcad_ui_state::AxisPoint| match p {
        qymcad_ui_state::AxisPoint::Datum(id, _) => id,
        qymcad_ui_state::AxisPoint::Vertex { body, edge, end, at } => {
            let local = crate::gui::vertex_local_pos(live, body, edge, end).unwrap_or(at);
            project.add_point_at_vertex(local, body, edge, end)
        }
    };
    let (ia, ib) = (point(a), point(b));
    Some(project.add_axis_two_points(ia, ib))
}
