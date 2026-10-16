//! Applying the commands that make or finish a solid.

use qymcad_ui_state::Notice;
use super::*;

/// Apply the SWEEP: the profile (`sweep_prof_sid`) along the path (`sweep_path_sid`). Both sketches are
/// entered into the timeline if they are not there yet, then the Sweep node follows. The contours come
/// from the selection in the bar (`sweep_prof_cid` and `sweep_path_cid`; 0 means the first suitable one
/// is taken automatically).
pub fn apply_sweep_cmd(feat: qymcad_ui_state::FeatTarget, project: &mut Project, status: &mut qymcad_ui_state::Notice, sweep: qymcad_ui_state::SweepParams) -> Option<Id> {
    if sweep.prof_sid == 0 || sweep.path_sid == 0 {
        *status = Notice::refusal(qymcad_i18n::tr("msg-need-profile-path"));
        return None;
    }
    if sweep.prof_sid == sweep.path_sid {
        *status = Notice::refusal(qymcad_i18n::tr("msg-profile-path-differ"));
        return None;
    }
    let prof_name = project.sketches.iter().find(|s| s.id == sweep.prof_sid).map(|s| qymcad_i18n::name(&s.name)).unwrap_or_default();
    let path_name = project.sketches.iter().find(|s| s.id == sweep.path_sid).map(|s| qymcad_i18n::name(&s.name)).unwrap_or_default();
    project.add_sketch_node(sweep.prof_sid, prof_name);
    project.add_sketch_node(sweep.path_sid, path_name);
    // the profile contours and the boolean go IN ONE node (see the revolve: the reason is the same).
    let profiles: Vec<Id> = if sweep.prof_cid != 0 { vec![sweep.prof_cid] } else { Vec::new() };
    // a sweep carries AN OPERATION: add (0) becomes a pad, plus cut (2) and intersect (3)
    let occt = match feat.op {
        2 => 0,
        3 => 2,
        _ => 1,
    };
    let part = project.sketch_owner(sweep.prof_sid).and_then(|o| project.active_body(o)).unwrap_or(0);
    let (src, op) = if part != 0 && matches!(feat.op, 1..=3) { (part, occt) } else { (0, 1) };
    let body = project.add_sweep_multi_op(sweep.prof_sid, profiles, sweep.path_sid, sweep.path_cid, src, op);
    (body != 0).then(|| if src == 0 { project.finish_base_body(body, 1) } else { body })
}

/// A hole: a cylinder cut at the centre of the picked face, perpendicular to it.
pub fn apply_hole_cmd(cmd: &qymcad_ui_state::FeatCommand, hole: qymcad_ui_state::HoleCommand, project: &mut Project, sel: qymcad_ui_state::Sel, status: &mut qymcad_ui_state::Notice) -> Option<Id> {
    // the kind plus the diameter and depth of the recess (for a counterbore or a countersink)
    let (dia2, depth2) = if hole.kind != 0 { (qymcad_ui_state::cmd_val(cmd, "dia2"), qymcad_ui_state::cmd_val(cmd, "depth2")) } else { (0.0, 0.0) };
    // "by sketch" mode drills many holes into the picked body, at the isolated points of a sketch
    if hole.mode == 1 {
        let Some(sid) = hole.sketch else {
            *status = Notice::info(qymcad_i18n::tr("msg-pick-sketch-points"));
            return None;
        };
        let Some(src) = qymcad_ui_state::selected_body(project, &sel) else {
            *status = Notice::info(qymcad_i18n::tr("msg-pick-body-drill"));
            return None;
        };
        if project.sketch_isolated_points(sid).is_empty() {
            *status = Notice::refusal(qymcad_i18n::tr("msg-no-isolated-points"));
            return None;
        }
        let body = project.add_hole_from_sketch(
            src,
            sid,
            qymcad_core::model::HoleTool { kind: hole.kind, diameter: qymcad_ui_state::cmd_val(cmd, "diameter"), depth: qymcad_ui_state::cmd_val(cmd, "depth"), dia2, depth2 },
            hole.flip,
        );
        store_cmd_exprs(cmd, project, body);
        return Some(body);
    }
    let qymcad_ui_state::Sel::Face(mi, fi) = sel else {
        *status = Notice::info(qymcad_i18n::tr("msg-pick-face-centre"));
        return None;
    };
    let src = project.mesh_id(mi)?;
    let face = project.bodies.get(mi).and_then(|b| b.faces.get(fi))?;
    // the face is referred to by its persistent id, so the hole holds on to it through a rebuild
    let key = qymcad_core::feature::FaceKey { index: fi as u32, centroid: [face.centroid.x, face.centroid.y, face.centroid.z], normal: face.normal, id: face.id };
    let at = hole_centre(key.centroid, key.normal, qymcad_ui_state::cmd_val(cmd, "off_u"), qymcad_ui_state::cmd_val(cmd, "off_v"));
    let body = project.add_hole_at(
        src,
        key,
        at,
        qymcad_core::model::HoleTool { kind: hole.kind, diameter: qymcad_ui_state::cmd_val(cmd, "diameter"), depth: qymcad_ui_state::cmd_val(cmd, "depth"), dia2, depth2 },
    );
    store_cmd_exprs(cmd, project, body);
    Some(body)
}

/// Apply the thread: `add_thread` on the picked rim, plus storing the parameter expressions.
pub fn apply_thread_cmd(cmd: &qymcad_ui_state::FeatCommand, project: &mut Project, status: &mut qymcad_ui_state::Notice, thread: qymcad_ui_state::ThreadParams) -> Option<Id> {
    let src = thread.src?;
    if thread.edge == 0 {
        *status = Notice::info(qymcad_i18n::tr("msg-pick-cyl-first"));
        return None;
    }
    // the input is a standard plus a size, as in a professional CAD; the geometry (the diameters, the
    // depth, the profile) is computed by the core from the formulas of the standard rather than
    // estimated by eye.
    let body = if thread.auger {
        let spec = qymcad_core::thread::AugerSpec {
            shaft_d: thread.radius * 2.0, // taken from the actual geometry; the regen refines it
            outer_d: qymcad_ui_state::cmd_val(cmd, "outer"),
            pitch: qymcad_ui_state::cmd_val(cmd, "pitch"),
            thickness: qymcad_ui_state::cmd_val(cmd, "thickness"),
            starts: thread.starts.max(1),
            left: thread.left,
            edge_r: qymcad_ui_state::cmd_val(cmd, "edge_r"),
        };
        project.add_auger(src, thread.edge, spec, qymcad_ui_state::cmd_val(cmd, "length"), qymcad_ui_state::cmd_val(cmd, "lead_in"), qymcad_ui_state::cmd_val(cmd, "lead_out"))
    } else {
        project.add_thread(src, thread.edge, thread_spec(cmd, thread), qymcad_ui_state::cmd_val(cmd, "length"), qymcad_ui_state::cmd_val(cmd, "lead_in"), qymcad_ui_state::cmd_val(cmd, "lead_out"))
    };
    store_cmd_exprs(cmd, project, body);
    Some(body)
}

/// APPLY THE RECOGNITION: the mesh piece the tool is aimed at becomes a body - the exact surfaces found on it, or,
/// where a person asks for it, a polyhedron of its flat faces. The tolerance multiplies the one the mesh itself
/// gives: 1 suits a mesh from a CAD, a coarser mesh needs more.
///
/// Both used to be buttons in the properties of a node that only shows after stepping into the part - that is, a
/// path nobody finds. Reported behaviour: an STL is imported, the part is stepped into, and there is no way at all
/// to turn the mesh into an object.
pub fn apply_recognise_cmd(pc: &mut qymcad_ui_state::PartCtx) -> Option<Id> {
    let Some(src) = pc.recognise.src else {
        *pc.status = Notice::info(qymcad_i18n::tr("msg-recognise"));
        return None;
    };
    let simplify = qymcad_ui_state::cmd_val(pc.cmd, "simplify");
    let body = if pc.feat.op == 1 {
        pc.project.add_mesh_solid(src, simplify)
    } else {
        pc.project.add_mesh_recognised(src, qymcad_ui_state::cmd_val(pc.cmd, "tol"), qymcad_ui_state::cmd_val(pc.cmd, "sharp"), simplify)
    };
    Some(body)
}

/// A fillet (4) or a chamfer (5) on the picked edges of the picked body (an empty pick means every edge).
pub fn apply_edge_cmd(pc: &mut qymcad_ui_state::PartCtx, cmd: u8) -> Option<Id> {
    // the target of a fillet or a chamfer is the body whose PERSISTENT edge ids actually sit in the edge
    // selection (`pc.edges.body`, kept in step by `refresh_edges`) rather than `qymcad_ui_state::selected_body()`:
    // otherwise, if `pc.sel` moved to another body between picking the edges and pressing Enter, the
    // chamfer would go to ids belonging to someone else or to none at all.
    // A part is one body, so when neither edges nor a body are picked, THE SINGLE body of the part is
    // taken (`active_body`), and the tool works straight away - a chamfer on EVERY edge, for instance -
    // without a "pick a body" step.
    let ctx = qymcad_ui_state::current_ctx_id(pc.active_path, pc.project);
    // the fallback through `qymcad_ui_state::current_body()` sees the body of the active part even WITHOUT entering it
    let Some(body) = pc
        .edges
        .body
        .or_else(|| qymcad_ui_state::selected_body(pc.project, pc.sel))
        .or_else(|| qymcad_ui_state::current_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }))
    else {
        *pc.status = Notice::refusal(qymcad_i18n::tr("msg-no-body"));
        return None;
    };
    // a hard gate: the body is alive and not consumed by a chain, or it would be a ghost branch...
    if pc.project.consumed_bodies().contains(&body) {
        *pc.status = Notice::refusal(qymcad_i18n::tr("msg-body-consumed"));
        return None;
    }
    // ...and belongs to the current context (a body of a neighbouring part must not be filleted from
    // inside this one).
    if pc.project.body_owner(body).is_some_and(|o| !pc.project.component_is_within(o, ctx)) {
        *pc.status = Notice::refusal(qymcad_i18n::tr("msg-edge-other-part"));
        return None;
    }
    let edges: Vec<u32> = pc.gsel.edges.iter().copied().collect();
    // IT IS RECORDED THE WAY IT WAS PICKED. If there is a description ("every edge of this face", "every
    // edge parallel to this one"), the description goes in: it survives an edit that adds elements.
    // Otherwise the list of picked ids goes in.
    let described = pc.gsel.described_ref();
    let last = if cmd == 4 {
        let r = qymcad_ui_state::cmd_val(pc.cmd, "radius");
        // THE "VERTEX -> RADIUS" TABLE. It works with a description and with a list alike: the radius is
        // set at a point rather than along an edge, so it needs no direction of an edge.
        let at = fillet_vertex_table(pc.cmd);
        if !at.is_empty() {
            let q = pc.gsel.recorded(&edges);
            pc.project.add_fillet_at_vertices(body, r, q, at)
        } else if let Some(q) = described {
            pc.project.add_fillet_ref(body, r, q)
        } else {
            pc.project.add_fillet(body, r, edges)
        }
    } else {
        use qymcad_core::feature::ChamferMode;
        // the asymmetric modes (two distances, or a leg plus an angle) work only on picked edges;
        // otherwise it is symmetric
        if pc.chamfer.mode != ChamferMode::Symmetric && !edges.is_empty() {
            pc.project.add_chamfer_ex(
                body,
                qymcad_ui_state::cmd_val(pc.cmd, "dist"),
                qymcad_core::model::ChamferShape { mode: pc.chamfer.mode, d2: qymcad_ui_state::cmd_val(pc.cmd, "d2"), flip: pc.chamfer.flip, ref_face: pc.chamfer.ref_face },
                edges,
            )
        } else {
            match described {
                Some(q) => pc.project.add_chamfer_ref(body, qymcad_ui_state::cmd_val(pc.cmd, "dist"), q),
                None => pc.project.add_chamfer(body, qymcad_ui_state::cmd_val(pc.cmd, "dist"), edges),
            }
        }
    };
    store_cmd_exprs(pc.cmd, pc.project, last); // radius, radius2, dist and d2 stay parametric
    Some(last)
}

/// Apply the LOFT: a body through a set of sketch sections (`loft_sids`, two or more). Every section is
/// entered into the timeline if it is not there yet, then the Loft node follows, with the picked contours
/// (`loft_cids`) and the kind of faces (`loft_ruled`).
pub fn apply_loft_cmd(pc: &mut qymcad_ui_state::PartCtx) -> Option<Id> {
    if pc.loft.sids.len() < 2 {
        *pc.status = Notice::refusal(qymcad_i18n::tr("msg-loft-needs-two"));
        return None;
    }
    // A cut, a pad or an intersection through sections: the target is the active body of the context.
    // "Surface" is a fourth kind of result rather than a separate tool. The question is the same one -
    // what should come out - and splitting it across two buttons would mean asking for a tool to be
    // chosen before that question has been answered.
    let surface = pc.loft.result == 4;
    let (mut src, mut op): (Id, u8) = (0, 0);
    if pc.loft.result != 0 && !surface {
        match qymcad_ui_state::current_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }) {
            Some(b) => {
                src = b;
                op = pc.loft.result - 1; // 1 becomes a cut (0), 2 a union (1), 3 an intersection (2)
            }
            None => {
                *pc.status = Notice::warning(qymcad_i18n::tr("msg-loft-no-body"));
            }
        }
    }
    let (sids, cids) = (pc.loft.sids.clone(), pc.loft.cids.clone());
    for &sid in &sids {
        let name = pc.project.sketches.iter().find(|s| s.id == sid).map(|s| qymcad_i18n::name(&s.name)).unwrap_or_default();
        pc.project.add_sketch_node(sid, name);
    }
    let body = pc.project.add_loft(sids, cids, pc.loft.ruled, src, op, surface);
    if body == 0 {
        return None;
    }
    // a SOLID loft (src == 0) is a material base and goes into the single body of the part; a cut loft
    // (src != 0) is already a boolean
    Some(if src == 0 { pc.project.finish_base_body(body, 1) } else { body })
}

/// Apply the thickening: one selected face plus a thickness.
pub fn apply_thicken_cmd(pc: &mut qymcad_ui_state::PartCtx) -> Option<Id> {
    // THE BODY IS TAKEN FROM THE FACE THAT WAS CLICKED rather than from the selection in the tree. Now
    // that surfaces exist, a sheet lives in the scene alongside the part, and it is the sheet that gets
    // thickened: with the part selected in the tree, the face of the sheet would be looked for on the
    // part and not found.
    let src = pc
        .gsel
        .faces_body
        .or_else(|| qymcad_ui_state::op_target_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, *pc.sel))?;
    let Some(fid) = pc.gsel.faces.iter().copied().next() else {
        *pc.status = Notice::info(qymcad_i18n::tr("msg-click-face-thicken"));
        return None;
    };
    let t = qymcad_ui_state::cmd_val(pc.cmd, "thickness");
    if t.abs() < 1e-9 {
        *pc.status = Notice::refusal(qymcad_i18n::tr("msg-zero-thickness"));
        return None;
    }
    let body = pc.project.add_thicken(src, fid, t);
    store_cmd_exprs(pc.cmd, pc.project, body); // the thickness stays parametric
    Some(body)
}

/// A shell: the picked faces become open and the body becomes hollow (a multiple selection by id, plus
/// a direction).
pub fn apply_shell_cmd(pc: &mut qymcad_ui_state::PartCtx) -> Option<Id> {
    // the source is the body the picked faces belong to; the faces are the multiple selection, by
    // persistent id. It falls back to the selection when the body of the faces is not pinned yet.
    let src = pc
        .gsel
        .faces_body
        .or_else(|| match *pc.sel {
            qymcad_ui_state::Sel::Face(mi, _) | qymcad_ui_state::Sel::Mesh(mi) => pc.project.mesh_id(mi),
            _ => qymcad_ui_state::selected_body(pc.project, pc.sel),
        })
        .or_else(|| qymcad_ui_state::current_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }))?; // a part is one body; `qymcad_ui_state::current_body` falls back without entering it
    let faces: Vec<u32> = pc.gsel.faces.iter().copied().collect();
    if faces.is_empty() {
        *pc.status = Notice::info(qymcad_i18n::tr("msg-click-face-open"));
        return None;
    }
    let body = pc.project.add_shell_mode(src, qymcad_ui_state::cmd_val(pc.cmd, "thickness"), faces, pc.opts.shell_side);
    store_cmd_exprs(pc.cmd, pc.project, body);
    Some(body)
}

/// A DRAFT: tilt the picked faces of the body relative to the neutral face (`draft_neutral`) by an
/// angle.
pub fn apply_draft_cmd(pc: &mut qymcad_ui_state::PartCtx) -> Option<Id> {
    let src = pc
        .gsel
        .faces_body
        .or_else(|| match *pc.sel {
            qymcad_ui_state::Sel::Face(mi, _) | qymcad_ui_state::Sel::Mesh(mi) => pc.project.mesh_id(mi),
            _ => qymcad_ui_state::selected_body(pc.project, pc.sel),
        })
        .or_else(|| qymcad_ui_state::current_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }))?; // a part is one body; `qymcad_ui_state::current_body` falls back without entering it
    let faces: Vec<u32> = pc.gsel.faces.iter().copied().collect();
    if faces.is_empty() {
        *pc.status = Notice::info(qymcad_i18n::tr("msg-click-face-draft"));
        return None;
    }
    if pc.draft.neutral == 0 {
        *pc.status = Notice::info(qymcad_i18n::tr("msg-pick-neutral"));
        return None;
    }
    let body = pc.project.add_draft(src, faces, pc.draft.neutral, qymcad_ui_state::cmd_val(pc.cmd, "angle"), pc.draft.flip);
    store_cmd_exprs(pc.cmd, pc.project, body); // the angle stays parametric
    Some(body)
}

/// Extrude or revolve along the picked contours. Returns the last body created.
pub fn apply_sketch_cmd(pc: &mut qymcad_ui_state::PartCtx, cmd: u8) -> Option<Id> {
    let si = pc.cmd.sketch.filter(|&si| si < pc.project.sketches.len())?;
    let sid = pc.project.sketches[si].id;
    // THE CONTEXT: a feature built from a sketch belongs to the component that OWNS the sketch (the part)
    // rather than to the active context. Extruding from AN ASSEMBLY (or without having entered the part)
    // would make `body_parent()` create a NEW part and put the feature there while the sketch stayed in
    // the old one - a cross-component reference is forbidden, and the sketch would not extrude. The owner
    // of the sketch is made active, so the feature lands in its part and the body builds.
    if let Some(owner) = pc.project.sketch_owner(sid) {
        pc.project.set_active_component(Some(owner));
    }
    let closed = qymcad_ui_state::sketch_closed_contours(pc.project, si);
    let mut targets: Vec<Id> = closed.iter().copied().filter(|c| pc.gsel.profiles.contains(c)).collect();
    if targets.is_empty() {
        // nothing picked: the one contour of the sketch is the profile; of several, none is chosen for the person -
        // Enter with nothing taken extruded them all at once, two squares apart into a part of two pieces
        if closed.is_empty() {
            *pc.status = Notice::refusal(qymcad_i18n::tr("msg-no-closed-contour"));
            return None;
        }
        if closed.len() > 1 {
            *pc.status = Notice::info(qymcad_i18n::tr("msg-pick-a-contour"));
            return None;
        }
        targets = closed.clone();
    }
    // NESTING IS NO LONGER FILTERED. Every picked contour is A REGION OF ITS OWN (itself minus its direct
    // children), and the regions are merged by the profile fuse in the core. qymcad_ui_state::Picking the outer and the
    // middle of three nested contours gives a plate with a hole the size of the small one; picking two
    // concentric circles gives a solid disc. It works at any depth. A nested picked contour used to be
    // thrown into the `fill` of the outer one, and everything deeper was lost - the part came out solid,
    // which was reported as not being able to extrude with a hole in the middle.
    let fill: Vec<Id> = Vec::new();
    // The target body is looked for IN THE CONTEXT OF THE SKETCH OWNER (where the feature will land)
    // rather than in the navigation context of the interface: without having entered the part by a double
    // click, a cut reported the part as empty and every extrude made a NEW body instead of the single
    // body of the part.
    let owner_body = pc.project.sketch_owner(sid).and_then(|o| pc.project.active_body(o));
    // A body is REQUIRED only for A CUT or AN INTERSECTION. A pad (op 1) with no body simply creates a new
    // one, as a join does in an empty part, rather than refusing silently with "build a body first".
    let need_target = (cmd == 1 || cmd == 3) && matches!(pc.feat.op, 2 | 3);
    let target = if need_target { owner_body.unwrap_or(0) } else { 0 };
    if need_target && target == 0 {
        *pc.status = Notice::refusal(qymcad_i18n::tr("cmd-cut-needs-body"));
        return None;
    }
    let reach = qymcad_ui_state::cmd_reach(pc.cmd, *pc.feat);
    // the second side is an expression field at the geometry (`qymcad_ui_state::cmd_val("down")`), not a drag value in the bar
    let down = if pc.cmd.extent.two_sided() { qymcad_ui_state::cmd_val(pc.cmd, "down").abs() } else { 0.0 };
    let through = pc.cmd.extent.through();
    let name = qymcad_i18n::name(&pc.project.sketches[si].name);
    pc.project.add_sketch_node(sid, name);
    let mut last = 0;
    // ALL the picked contours go through ONE operation and make ONE node (the core fuses N tools into a
    // single boolean against the body of the part). No chain of Extrude plus BodyBoolean or Combine, and
    // no collapsing.
    if cmd == 1 {
        let part = owner_body.unwrap_or(0); // the body of the part that OWNS the sketch (0 means an empty part, so create one)
        let h = qymcad_ui_state::cmd_val(pc.cmd, "height").abs();
        // console diagnostics ON REQUEST (`QYM_EXTRUDE_DEBUG=1`), as with the contour analysis. It used to
        // print unconditionally, and an ordinary session poured a line into stderr on every extrude.
        if std::env::var("QYM_EXTRUDE_DEBUG").is_ok() {
            eprintln!(
                "[extrude] sid={sid} targets={targets:?} fill={fill:?} part={part} h={h} op={} reach={reach:?} down={down} through={through} active={:?}",
                pc.feat.op, pc.project.active_component
            );
        }
        last = if part == 0 {
            // AN EMPTY part gets A NEW body: an Extrude node plus `finish_base_body` - THE SAME path that
            // revolve takes (in an empty part `finish_base_body` simply returns the seed body). That makes
            // an extrude in an empty part behave like a revolve, which works, and like a sketch on an
            // existing body.
            let e = pc.project.add_extrude_multi(sid, targets.clone(), h, reach, down, fill.clone());
            if e != 0 {
                pc.project.finish_base_body(e, 1)
            } else {
                0
            }
        } else {
            // with a body present: add (0) becomes a pad (1); pad (1) stays 1; cut (2) becomes a cut (0);
            // intersect (3) becomes an intersection (2)
            let occt = match pc.feat.op {
                2 => 0,
                3 => 2,
                _ => 1,
            };
            pc.project.add_combine_multi_op(part, sid, targets.clone(), qymcad_core::model::CombineSpan { height: h, down, extent: qymcad_core::feature::Extent { through, reach }, fill: &fill }, occt)
        };
        if last != 0 {
            store_cmd_exprs(pc.cmd, pc.project, last); // the dimensions (the height) go onto the node of the operation
        } else {
            qymcad_ui_state::cmd_fail(pc.cmd_failed, pc.status, qymcad_i18n::tr("cmd-op-failed"));
        }
    } else {
        // A REVOLVE puts ALL the contours and the boolean into ONE node, exactly as an extrude does.
        //
        // There used to be a loop over the contours here: a `Revolve` per contour (which is a NEW body,
        // that is, an add) plus a `BodyBoolean` per cut. Two contours produced four timeline nodes, which
        // read as the revolve falling apart into two features that add instead of cutting; editing opened
        // only one contour, and deleting either of them took the whole chain below with it.
        let part = owner_body.unwrap_or(0);
        let occt = match pc.feat.op {
            2 => 0,
            3 => 2,
            _ => 1,
        };
        // A pad in AN EMPTY part is simply a new body: there is nothing to boolean against.
        let (rev_src, rev_op) = if part != 0 && matches!(pc.feat.op, 1..=3) { (part, occt) } else { (0, 1) };
        let body = pc.project.add_revolve_multi_op(
            sid,
            targets.clone(),
            qymcad_core::model::RevolveAxis { axis: pc.rev.axis, datum: pc.rev.axis_datum, line: pc.rev.axis_line },
            qymcad_core::model::RevolveTurn { angle: qymcad_ui_state::cmd_val(pc.cmd, "angle"), reach: qymcad_ui_state::cmd_reach(pc.cmd, *pc.feat) },
            rev_src,
            rev_op,
        );
        if body != 0 {
            store_cmd_exprs(pc.cmd, pc.project, body);
            // a new body (nothing to boolean) is merged into the single body of the part, as with an extrude
            last = if rev_src == 0 { pc.project.finish_base_body(body, 1) } else { body };
        }
    }
    (last != 0).then_some(last)
}

/// Apply "push a face": one selected face plus an offset.
pub fn apply_push_face_cmd(pc: &mut qymcad_ui_state::PartCtx) -> Option<Id> {
    // THE BODY IS TAKEN FROM THE FACE THAT WAS CLICKED rather than from the selection in the tree. When
    // the two diverged, the search for the face key failed and the tool SILENTLY did nothing: no node, no
    // error, no message - it reads as "pressed it, and nothing happened". Thicken was mended the same
    // way.
    let src = pc
        .gsel
        .faces_body
        .or_else(|| qymcad_ui_state::op_target_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, *pc.sel))?;
    let fid = pc.gsel.faces.iter().copied().next();
    let Some(fid) = fid else {
        *pc.status = Notice::info(qymcad_i18n::tr("msg-click-face-push"));
        return None;
    };
    // the face key is taken from the built body: the offset is later resolved BY NAME, not by index
    let key = pc.project.mesh_index(src).and_then(|mi| pc.project.bodies.get(mi)).and_then(|b| b.faces.iter().find(|f| f.id == fid)).map(|f| qymcad_core::feature::FaceKey {
        index: 0,
        centroid: [f.centroid.x, f.centroid.y, f.centroid.z],
        normal: f.normal,
        id: f.id,
    });
    // AND IF THE FACE WAS NOT FOUND AFTER ALL, SAY SO. A silent return from here was exactly that
    // "pressed it, and nothing": silence is indistinguishable from a broken program.
    let Some(key) = key else {
        *pc.status = Notice::refusal(qymcad_i18n::tr("msg-faces-not-found"));
        return None;
    };
    let dist = qymcad_ui_state::cmd_val(pc.cmd, "dist");
    if dist.abs() < 1e-9 {
        *pc.status = Notice::refusal(qymcad_i18n::tr("msg-zero-offset"));
        return None;
    }
    let body = pc.project.add_push_face(src, key, dist);
    store_cmd_exprs(pc.cmd, pc.project, body); // the offset stays parametric
    Some(body)
}

/// Moved out of the frame because it is a decision about the DOCUMENT - which two bodies are combined -
/// and the frame's business is only where the click landed.
/// APPLY THE BOOLEAN whose two bodies are taken: one node, one step of undo, the tool put down.
pub fn apply_boolean(pc: &mut qymcad_ui_state::PartCtx) {
    let (Some((a, op)), Some(b)) = (pc.boolean.pick, pc.boolean.b) else { return };
    // a body taken and then taken away by Ctrl+Z is not combined: the node would only stand red
    if [a, b].iter().any(|&x| pc.project.mesh_index(x).is_none()) {
        (pc.boolean.pick, pc.boolean.b) = (None, None);
        *pc.status = Notice::refusal(qymcad_i18n::tr1("cmd-picks-gone", "ask", &qymcad_i18n::tr("tb-bool-bodies-hint")));
        return;
    }
    qymcad_ui_state::begin_edit(&mut *pc.edits, &*pc.project, qymcad_i18n::tr("f-operation"));
    let res = pc.project.add_body_boolean(a, b, op);
    (pc.boolean.pick, pc.boolean.b) = (None, None);
    qymcad_ui_state::mark_dirty_for_rebuild(&mut pc.rebuild()); // the document is marked; the scheduler does the computing
    let (project, sel, view) = (&mut *pc.project, &mut *pc.sel, &mut *pc.view);
    qymcad_ui_state::select_body(project, sel, view, res);
    *pc.status = Notice::info(qymcad_i18n::tr("vp-bool-created"));
    qymcad_ui_state::commit_edit(&mut pc.rebuild());
}

/// CREATE A PRIMITIVE from the fields of the running command.
///
/// The closure here used to take `&Self` for one reason: it needed `cmd` while `project` was borrowed
/// mutably in the same expression, and naming the whole application was the way round it. Over a context
/// the two are separate places, so the closure takes the command alone and the document stays free.
pub fn apply_prim_cmd(pc: &mut qymcad_ui_state::PartCtx) -> Option<Id> {
    let cmd = pc.cmd.clone();
    let v = |k: &str| qymcad_ui_state::cmd_val(&cmd, k);
    let body = match pc.armed.cmd_kind() {
        10 => pc.project.add_box(v("dx"), v("dy"), v("dz")),
        11 => pc.project.add_cylinder(v("r"), v("h")),
        12 => pc.project.add_sphere(v("r")),
        13 => pc.project.add_cone(v("r1"), v("r2"), v("h")),
        14 => pc.project.add_torus(v("major"), v("minor")),
        15 => pc.project.add_prism(v("r"), pc.prim.n.max(3), v("h")),
        _ => return None,
    };
    store_cmd_exprs(&cmd, pc.project, body);
    // placement: an oriented move of the primitive into the picked frame (the base sits on the surface)
    let placed = match pc.prim.frame {
        Some(m) if !qymcad_ui_state::is_identity12(&m) => pc.project.add_move(body, m),
        _ => body,
    };
    // A PART IS ONE BODY - the primitive is merged into the single body of the part (the first one seeds it)
    Some(pc.project.finish_base_body(placed, 1))
}
