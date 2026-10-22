//! Applying the datums, arrays, mirrors and the gizmo of components.

use qymcad_ui_state::{NotApplied, Notice};
use super::*;

/// Apply the active datum command (Enter). Parametric: the offset and the coordinates are expressions
/// stored as feature dimensions.
pub fn apply_datum_cmd(
    armed: &qymcad_ui_state::Armed,
    cmd: &qymcad_ui_state::FeatCommand,
    datum: &qymcad_ui_state::DatumCommand,
    edges: &qymcad_ui_state::EdgeCache,
    project: &mut Project,
) -> Result<Id, NotApplied> {
    use qymcad_core::feature::SketchPlane;
    match armed.part_tool() {
        Some(qymcad_ui_state::PartTool::DatumPlane) => {
            let dist = qymcad_ui_state::cmd_val(cmd, "dist");
            let id = match datum.plane_pick {
                Some(SketchPlane::World(bp)) => project.add_offset_plane(bp, dist),
                Some(SketchPlane::Face(body, key)) => project.add_plane_from_face(body, key, dist),
                Some(SketchPlane::Datum(did)) => project.add_offset_from_plane(did, dist), // parametric
                None => return Err(refused("msg-click-base-plane")),
            };
            turn_datum_plane(cmd, project, id);
            if let Err(e) = store_cmd_exprs(cmd, project, id) {
                // the offset, the tilts and the turn stay parametric in all three cases
                return Err(formula_refused(&e));
            }
            Ok(id)
        }
        Some(qymcad_ui_state::PartTool::DatumPoint) => {
            // "at a vertex" gives an associative point that travels with the vertex; otherwise the
            // coordinates, which stay parametric
            if datum.pt_mode == 1 {
                let (body, edge, end, at) = datum.pt_vert.ok_or_else(|| refused("msg-click-vertex"))?;
                return Ok(project.add_point_at_vertex(at, body, edge, end));
            }
            let at = [qymcad_ui_state::cmd_val(cmd, "x"), qymcad_ui_state::cmd_val(cmd, "y"), qymcad_ui_state::cmd_val(cmd, "z")];
            let id = project.add_point_at(at);
            if let Err(e) = store_cmd_exprs(cmd, project, id) {
                // x, y and z stay parametric
                return Err(formula_refused(&e));
            }
            Ok(id)
        }
        Some(qymcad_ui_state::PartTool::DatumAxis) => {
            // mode 2, "two points": if BOTH are datum points the axis is a parametric TwoPoints;
            // otherwise it is a manual one built from the coordinates
            if datum.axis_mode == 2 {
                if datum.axis_pts.len() < 2 {
                    return Err(refused("msg-click-two-points"));
                }
                let (p0, p1) = (datum.axis_pts[0], datum.axis_pts[1]);
                if p0.0 != 0 && p1.0 != 0 {
                    return Ok(project.add_axis_two_points(p0.0, p1.0)); // associative to the points
                }
                let d = [p1.1[0] - p0.1[0], p1.1[1] - p0.1[1], p1.1[2] - p0.1[2]];
                if d[0].abs() + d[1].abs() + d[2].abs() < 1e-9 {
                    return Err(refused("msg-points-coincide"));
                }
                return Ok(project.add_axis_manual(p0.1, d));
            }
            // mode 0, "by an edge or a face", gives an ASSOCIATIVE axis that travels with its source
            if datum.axis_mode == 0 {
                return match datum.axis_hit {
                    Some(qymcad_ui_state::AxisHit::Edge(i)) => edges.axes.get(i).map(|&(body, edge, _)| project.add_axis_from_edge(body, edge)).ok_or_else(|| refused("msg-click-edge-axis")),
                    Some(qymcad_ui_state::AxisHit::Face(body, fid)) => Ok(project.add_axis_from_face(body, fid)),
                    Some(qymcad_ui_state::AxisHit::Datum(_)) | None => datum.axis_ref.map(|(o, d)| project.add_axis_manual(o, d)).ok_or_else(|| refused("msg-click-edge-axis")),
                };
            }
            let (o, d) = (
                [qymcad_ui_state::cmd_val(cmd, "ox"), qymcad_ui_state::cmd_val(cmd, "oy"), qymcad_ui_state::cmd_val(cmd, "oz")],
                [qymcad_ui_state::cmd_val(cmd, "dx"), qymcad_ui_state::cmd_val(cmd, "dy"), qymcad_ui_state::cmd_val(cmd, "dz")],
            );
            if d[0].abs() + d[1].abs() + d[2].abs() < 1e-9 {
                return Err(refused("msg-axis-not-set"));
            }
            Ok(project.add_axis_manual(o, d))
        }
        _ => Err(op_failed()),
    }
}

/// Create the array from the options picked by the command. Associative; the step and the angle stay
/// parametric.
pub fn apply_array_cmd(pc: &mut qymcad_ui_state::PartCtx) -> Result<Id, NotApplied> {
    let src =
        qymcad_ui_state::op_target_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, *pc.sel).ok_or_else(no_body)?;
    if pc.armed.part_tool() == Some(qymcad_ui_state::PartTool::CircularPattern) {
        let angle = if pc.arr.full { 360.0 } else { qymcad_ui_state::cmd_val(pc.cmd, "angle") };
        let body = pc.project.add_circular_array_axis(src, pc.arr.count.max(1), angle, pc.arr.axis);
        // the count and the angle are fields of the command, kept parametric through the one door of a feature's number
        if let Err(e) = store_cmd_exprs(pc.cmd, pc.project, body) {
            return Err(formula_refused(&e));
        }
        if pc.arr.full {
            pc.project.clear_feat_formula(&qymcad_core::model::FeatNumber { node: body, key: "angle" });
        }
        Ok(body)
    } else {
        let (dx, dy, dz) = qymcad_ui_state::arr_vec(pc.arr.dir, qymcad_ui_state::cmd_val(pc.cmd, "step"));
        let (dx2, dy2, dz2, c2) = if pc.arr.two {
            let (a, b, c) = qymcad_ui_state::arr_vec(pc.arr.dir2, qymcad_ui_state::cmd_val(pc.cmd, "step2"));
            (a, b, c, pc.arr.count2.max(1))
        } else {
            (0.0, 0.0, 0.0, 1)
        };
        let (dx3, dy3, dz3, c3) = if pc.arr.two && pc.arr.three {
            let (a, b, c) = qymcad_ui_state::arr_vec(pc.arr.dir3, qymcad_ui_state::cmd_val(pc.cmd, "step3"));
            (a, b, c, pc.arr.count3.max(1))
        } else {
            (0.0, 0.0, 0.0, 1)
        };
        let body = pc
            .project
            .add_linear_array_grid3(src, [ArrayAxis { d: [dx, dy, dz], count: pc.arr.count.max(1) }, ArrayAxis { d: [dx2, dy2, dz2], count: c2 }, ArrayAxis { d: [dx3, dy3, dz3], count: c3 }]);
        // the logical counts and steps, so that reopening returns the TEXT of the expression
        if let Err(e) = store_cmd_exprs(pc.cmd, pc.project, body) {
            return Err(formula_refused(&e));
        }
        // THE TEXTS ARE TAKEN FIRST, and the document is written afterwards. Reading the command and
        // writing the document in one expression borrows the application twice at once; taken apart,
        // the order of the two is also said out loud instead of being left to the compiler.
        let (d1, d2, d3) = (pc.arr.dir, pc.arr.dir2, pc.arr.dir3);
        let (t1, t2, t3) = (cmd_txt(pc.cmd, "step"), cmd_txt(pc.cmd, "step2"), cmd_txt(pc.cmd, "step3"));
        let (two, three) = (pc.arr.two, pc.arr.three);
        if let Err(e) = qymcad_ui_state::store_arr_component(pc.project, body, ["dx", "dy", "dz"], d1, t1) {
            return Err(formula_refused(&e));
        }
        if two {
            if let Err(e) = qymcad_ui_state::store_arr_component(pc.project, body, ["dx2", "dy2", "dz2"], d2, t2) {
                return Err(formula_refused(&e));
            }
        }
        if two && three {
            if let Err(e) = qymcad_ui_state::store_arr_component(pc.project, body, ["dx3", "dy3", "dz3"], d3, t3) {
                return Err(formula_refused(&e));
            }
        }
        Ok(body)
    }
}

/// Create the mirror from the plane picked by the command. Associative.
pub fn apply_mirror_cmd(pc: &mut qymcad_ui_state::PartCtx) -> Result<Id, NotApplied> {
    let src =
        qymcad_ui_state::op_target_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, *pc.sel).ok_or_else(no_body)?;
    let Some(sp) = pc.mirror.plane else {
        return Err(refused("msg-click-mirror-plane"));
    };
    let (plane, datum, face) = qymcad_ui_state::resolve_mirror_plane(sp);
    let node = pc.project.add_mirror(src, plane, pc.opts.mirror_keep, datum);
    if let Some((b, k)) = face {
        pc.project.set_op_face(node, b, k);
    }
    Ok(node)
}

/// Apply the accumulated drag of the component gizmo: `transform = accumulated * start` ->
/// `set_component_transform` (cheap, with no rebuild of bodies). The start is pinned, so the motion is
/// smooth; snapping comes from the panel or from Ctrl.
pub fn apply_comp_giz(pc: &mut qymcad_ui_state::PartCtx) {
    qymcad_ui_state::begin_edit(pc.edits, pc.project, qymcad_i18n::tr("status-move-component")); // THE BOUNDARY OF AN OPERATION
    let Some((comp, _, _, _)) = pc.comp_giz.drag else { return };
    if let Some(t) = qymcad_ui_state::comp_giz_accum(pc.comp_giz, pc.set, pc.comp_giz.snap) {
        // the parts grouped with it go along (a group holds them where they stood before the hand)
        if !pc.project.move_component_by_hand(comp, t) {
            *pc.status = Notice::info(qymcad_i18n::tr("j-group-grounded"));
        }
        qymcad_ui_state::invalidate_placement(pc.regen); // THE PLACEMENT moved; the shape of the bodies did not change
    }
    qymcad_ui_state::commit_edit(&mut pc.rebuild());
}

/// Apply the component array (Enter): create a new one or update the one being edited.
pub fn apply_comp_array(pc: &mut qymcad_ui_state::PartCtx) -> Result<(), NotApplied> {
    // A COUNT, A STEP OR AN ANGLE ITS FIELD REFUSED is not applied with the value the field held before: a row of 0
    // came out as a row of 3
    if let Some((label, msg)) = pc.cmd.params.iter().find_map(|p| qymcad_ui_state::field_value(p, &pc.project.param_map()).err().map(|m| (qymcad_i18n::tr(p.label_key()), m))) {
        return Err(NotApplied::Refused(format!("{label}: {msg}")));
    }
    let kind = qymcad_ui_state::comp_array_kind(*pc.arr, pc.carr, pc.cmd);
    // a layout past the limit is refused with the limit in words, before the "one instance" reading of a zero count
    if kind.count() == 0 {
        return Err(NotApplied::Refused(qymcad_i18n::error_words::error_text(&qymcad_core::feature::pattern_too_large(&kind.counts()))));
    }
    // a row of one instance is the source alone: nothing to make, and said so
    if kind.count() < 2 {
        return Err(refused("msg-comp-array-one"));
    }
    qymcad_ui_state::begin_edit(pc.edits, pc.project, if pc.carr.edit != 0 { qymcad_i18n::tr("status-edit-comp-array") } else { qymcad_i18n::tr("f-comp-array") });
    let ok = if pc.carr.edit != 0 { pc.project.set_comp_pattern(pc.carr.edit, kind) } else { pc.project.add_comp_pattern(pc.carr.src, kind) != 0 };
    if !ok {
        // nothing laid: the operation goes with no trace, the command stays in hand
        qymcad_ui_state::abort_edit(&mut pc.rebuild());
        return Err(refused("msg-array-no-body"));
    }
    *pc.status = Notice::info(qymcad_i18n::tr1("cmd-comp-array-done", "n", &pc.arr.count.max(1).to_string()));
    *pc.carr = qymcad_ui_state::CompArrayCmd::default();
    pc.cmd.params.clear();
    qymcad_ui_state::mark_dirty_for_rebuild(&mut pc.rebuild());
    qymcad_ui_state::commit_edit(&mut pc.rebuild());
    Ok(())
}
