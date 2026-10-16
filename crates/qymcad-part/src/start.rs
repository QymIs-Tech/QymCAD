//! A COMMAND TAKEN: what it asks for first and what it takes of the selection.

use qymcad_ui_state::Notice;
use super::*;

/// Turn on interactive moving (op=1), copying (op=2) or rotating (op=3): pick, then the base point or
/// centre, then the target (for a rotation, the angle in the popup).
pub fn start_move_tool(t: &mut qymcad_ui_state::Tools, status: &mut qymcad_ui_state::Notice, op: u8) {
    let cur = t.armed.move_op();
    qymcad_ui_state::exit_draw_tools(&mut t.reborrow());
    let qymcad_ui_state::Tools { armed, annot: _, cmd: _, corner: _, dim: _, drag: _, gsel: _, inline: _, measure: _, pat: _, pending_import: _, picking: _, place: _, sel_sk, tool } = t;
    sel_sk.modify = None;
    **armed = if cur == op { qymcad_ui_state::Armed::None } else { qymcad_ui_state::Armed::Move(op) };
    tool.move_base = None;
    let has_sel = sel_sk.items.iter().any(|(k, _)| *k == 1);
    let what = if op == 3 { qymcad_i18n::tr("f-rotation-centre") } else { qymcad_i18n::tr("f-base-point") };
    *status = if armed.move_op() == 0 {
        Notice::info(qymcad_i18n::tr("msg-cancelled"))
    } else {
        Notice::info(if has_sel { qymcad_i18n::tr1("cmd-click-what", "what", &what) } else { qymcad_i18n::tr1("cmd-select-then-click", "what", &what) })
    };
}

/// Turn on the array feature (op=1 linear, 2 circular): pick, then parameters, then Enter, with a
/// preview.
pub fn start_pattern(t: &mut qymcad_ui_state::Tools, status: &mut qymcad_ui_state::Notice, op: u8) {
    let cur = t.armed.pat_op();
    qymcad_ui_state::exit_draw_tools(&mut t.reborrow()); // drops the other modes (move, click-op) and `pat.op`
    let qymcad_ui_state::Tools { armed, annot: _, cmd: _, corner: _, dim: _, drag: _, gsel: _, inline: _, measure: _, pat, pending_import: _, picking: _, place: _, sel_sk, tool: _ } = t;
    sel_sk.modify = None;
    **armed = if cur == op { qymcad_ui_state::Armed::None } else { qymcad_ui_state::Armed::Pattern(op) };
    pat.edit = None;
    pat.center = None;
    *status = if armed.pat_op() == 0 {
        Notice::info(qymcad_i18n::tr("msg-cancelled"))
    } else {
        Notice::info(if armed.pat_op() == 2 { qymcad_i18n::tr("msg-circ-array-sketch") } else { qymcad_i18n::tr("msg-lin-array-sketch") })
    };
}

pub fn start_draft_cmd(pc: &mut qymcad_ui_state::PartCtx) {
    if qymcad_ui_state::op_target_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, *pc.sel).is_none() {
        *pc.status = Notice::refusal(qymcad_i18n::tr("msg-no-body"));
        return;
    }
    pc.cmd.open(pc.armed, 23, *pc.mode_3d); // a clean slate, then open

    *pc.mode_3d = true;
    pc.gsel.edges.clear();
    pc.gsel.faces.clear(); // the faces to tilt are picked from scratch: a draft needs no pre-selection
    pc.gsel.faces_body = None;
    pc.cmd.params = vec![qymcad_ui_state::CmdParam::new("f-angle", "angle", 3.0, -60.0, 60.0).nonzero()]; // a draft of 0 tilts nothing
    *pc.status = Notice::info(qymcad_i18n::tr("msg-draft"));
}

/// A command on a finished body (a fillet or a chamfer works on edges; a shell or a hole on a face).
pub fn start_body_cmd(pc: &mut qymcad_ui_state::PartCtx, cmd: u8) {
    // a part is one body, so a command on a body works on THE SINGLE body of the part without clicking it
    // first. It starts if there is a body at all (either selected or the active one); it refuses only
    // when there is no body whatsoever.
    if qymcad_ui_state::op_target_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, *pc.sel).is_none() {
        *pc.status = Notice::refusal(qymcad_i18n::tr("msg-no-body"));
        return;
    }
    pc.cmd.open(pc.armed, cmd, *pc.mode_3d); // a clean slate, then open
                                             // a new feature, not an edit
    *pc.mode_3d = true;
    pc.gsel.edges.clear();
    pc.gsel.faces.clear(); // a shell picks faces from scratch
    pc.gsel.faces_body = None; // the scope of the multiple face selection
    pc.opts.shell_side = qymcad_core::feature::ShellSide::Inward; // a shell goes INWARDS by default
    pc.chamfer.mode = qymcad_core::feature::ChamferMode::Symmetric; // a chamfer is symmetric by default
    pc.chamfer.flip = false;
    pc.chamfer.ref_face = 0; // the reference face is automatic until picked by hand
    pc.chamfer.pick_ref = false;
    pc.hole.kind = 0; // a hole is plain by default
    pc.hole.mode = 0; // by default "on a face"
    pc.hole.sketch = None;
    pc.hole.flip = false;
    pc.cmd.params = match cmd {
        // ONE field: the base radius. A variable fillet is set BY VERTICES: click a vertex of the
        // selection and it gets a field of its own. There is no second "radius 2" field any more: it
        // described a single edge with a direction and was incompatible with a selection in principle.
        4 => vec![qymcad_ui_state::CmdParam::new("f-radius", "radius", 2.0, 0.05, 1000.0)],
        // the leg d1 is always there; d2 is either the second leg (two distances) or the angle in
        // degrees (a leg plus an angle), enabled by the mode chosen in the top panel
        5 => vec![
            qymcad_ui_state::CmdParam::new("f-leg", "dist", 1.5, 0.05, 1000.0),
            qymcad_ui_state::CmdParam::new(qymcad_ui_state::chamfer_d2_label(qymcad_core::feature::ChamferMode::Symmetric), "d2", 1.5, 0.0, 1000.0),
        ],
        6 => vec![qymcad_ui_state::CmdParam::new("f-thickness", "thickness", 2.0, 0.1, 1000.0)],
        _ => vec![
            qymcad_ui_state::CmdParam::new("f-diameter", "diameter", 6.0, 0.1, 10000.0),
            qymcad_ui_state::CmdParam::new("f-depth", "depth", 15.0, 0.1, 10000.0),
            qymcad_ui_state::CmdParam::new("f-hole-u", "off_u", 0.0, -100000.0, 100000.0),
            qymcad_ui_state::CmdParam::new("f-hole-v", "off_v", 0.0, -100000.0, 100000.0),
        ],
    };
    *pc.status = match cmd {
        4 => Notice::info(qymcad_i18n::tr("msg-fillet")),
        5 => Notice::info(qymcad_i18n::tr("msg-chamfer")),
        6 => Notice::info(qymcad_i18n::tr("msg-shell")),
        _ => Notice::info(qymcad_i18n::tr("msg-hole")),
    };
}

/// Mirror as A COMMAND: pick a body, then the top bar (keep the original or not), then PICK BY CLICK the
/// mirror plane, datum or face in the viewport (as when picking a sketch plane), then the preview, then
/// Enter.
pub fn start_mirror_cmd(pc: &mut qymcad_ui_state::PartCtx) {
    if qymcad_ui_state::op_target_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, *pc.sel).is_none() {
        *pc.status = Notice::refusal(qymcad_i18n::tr("msg-no-body"));
        return;
    }
    pc.cmd.open(pc.armed, 16, *pc.mode_3d); // a clean slate, then open

    *pc.mode_3d = true;
    pc.opts.mirror_keep = true;
    pc.mirror.plane = None;
    pc.cmd.params.clear();
    *pc.status = Notice::info(qymcad_i18n::tr("msg-mirror-pick"));
}

pub fn start_remove_face_cmd(pc: &mut qymcad_ui_state::PartCtx) {
    if qymcad_ui_state::op_target_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, *pc.sel).is_none() {
        *pc.status = Notice::refusal(qymcad_i18n::tr("msg-no-body"));
        return;
    }
    pc.cmd.open(pc.armed, 26, *pc.mode_3d);
    qymcad_ui_state::borrow_view(*pc.cam, *pc.mode_3d, *pc.view, pc.view_restore);
    *pc.mode_3d = true;
    pc.gsel.edges.clear();
    pc.gsel.faces.clear();
    pc.gsel.faces_body = None;
    pc.cmd.params = vec![]; // no parameters: the operation is defined solely by which faces are picked
    *pc.status = Notice::info(qymcad_i18n::tr("msg-remove-face"));
}

/// STITCH SHEETS: click the surfaces, then Enter. The operation has no number other than the tolerance -
/// a stitch either meets along the edges or it does not.
pub fn start_stitch_cmd(pc: &mut qymcad_ui_state::PartCtx) {
    pc.cmd.open(pc.armed, 33, *pc.mode_3d);
    qymcad_ui_state::borrow_view(*pc.cam, *pc.mode_3d, *pc.view, pc.view_restore);
    *pc.mode_3d = true;
    pc.gsel.edges.clear();
    pc.gsel.faces.clear();
    pc.gsel.faces_body = None;
    pc.stitch_parts.clear();
    pc.cmd.params = vec![qymcad_ui_state::CmdParam::new("f-stitch-tol", "tol", 0.01, 1e-6, 10.0)];
    *pc.status = Notice::info(qymcad_i18n::tr("msg-stitch"));
}

/// WHAT A CLICK ON A WHOLE BODY MEANS FOR THE TOOLS THAT TAKE ONE: stitching sheets (33) and recognising a mesh (35).
/// The window says WHAT was clicked; which of those a tool wants, and what to answer when it is the wrong thing, is
/// the workbench's own business and not the window's.
pub fn pick_body_for_tool(pc: &mut qymcad_ui_state::PartCtx, clicked: Option<Id>, is_sheet: bool, is_mesh: bool) {
    match pc.armed.cmd_kind() {
        33 => match (is_sheet, clicked) {
            (true, Some(id)) => {
                if let Some(at) = pc.stitch_parts.iter().position(|x| *x == id) {
                    pc.stitch_parts.remove(at);
                } else {
                    pc.stitch_parts.push(id);
                }
                *pc.status = Notice::info(qymcad_i18n::tr1("msg-stitch-picked", "n", &pc.stitch_parts.len().to_string()));
            }
            _ => *pc.status = Notice::refusal(qymcad_i18n::tr("msg-stitch-only-sheets")),
        },
        35 => match (is_mesh, clicked) {
            (true, Some(id)) => {
                pc.recognise.src = if pc.recognise.src == Some(id) { None } else { Some(id) };
                *pc.status = Notice::info(qymcad_i18n::tr(if pc.recognise.src.is_some() { "msg-recognise-picked" } else { "msg-recognise" }));
                if let Some(mi) = pc.recognise.src.and_then(|src| pc.project.mesh_index(src)) {
                    suggest_simplify(pc.cmd, &pc.project.bodies[mi].mesh);
                }
            }
            _ => *pc.status = Notice::refusal(qymcad_i18n::tr("msg-recognise-only-mesh")),
        },
        _ => {}
    }
}

/// From how many triangles a mesh makes a heavy body: every triangle is a face of the body as it is, and a body of
/// hundreds of thousands of faces rebuilds for minutes after every operation (measured on a 178 032-triangle mesh).
pub const HEAVY_MESH: usize = 50_000;

/// A HEAVY MESH IS OFFERED ITS SIMPLIFICATION: where the field is still at zero, it gets a ten-thousandth of the mesh's
/// size - within what an export from a CAD holds its surface to - rounded to one figure. The person sees the number
/// and changes it or sets it back to zero.
pub fn suggest_simplify(cmd: &mut qymcad_ui_state::FeatCommand, mesh: &qymcad_core::geom::Mesh) {
    if mesh.tris.len() < HEAVY_MESH {
        return;
    }
    let Some(field) = cmd.params.iter_mut().find(|p| p.key == "simplify") else { return };
    if field.val != 0.0 {
        return;
    }
    let (lo, hi) = mesh.verts.iter().fold(([f64::MAX; 3], [f64::MIN; 3]), |(lo, hi), p| ([lo[0].min(p.x), lo[1].min(p.y), lo[2].min(p.z)], [hi[0].max(p.x), hi[1].max(p.y), hi[2].max(p.z)]));
    let size = ((hi[0] - lo[0]).powi(2) + (hi[1] - lo[1]).powi(2) + (hi[2] - lo[2]).powi(2)).sqrt();
    let raw = (size * 1e-4).max(1e-3);
    let unit = 10f64.powf(raw.log10().floor());
    let val = (raw / unit).round() * unit;
    field.val = val;
    field.txt = format!("{val}");
}

/// COUNT WHAT THE MESH IN HAND IS FOUND TO BE, in the background, whenever the mesh, the tolerance or the sharp angle
/// changes - the bar lists it and the viewport colours it before Enter. A count of a large mesh takes seconds, and a
/// frame must not wait for it.
pub fn recognise_preview(pc: &mut qymcad_ui_state::PartCtx, ctx: &egui::Context) {
    let Some(src) = pc.recognise.src else { return };
    let (tol, sharp) = (qymcad_ui_state::cmd_val(pc.cmd, "tol"), qymcad_ui_state::cmd_val(pc.cmd, "sharp"));
    let key = (src, tol.to_bits(), sharp.to_bits());
    if pc.recognise.asked == Some(key) {
        return;
    }
    let Some(mesh) = pc.project.mesh_index(src).map(|mi| pc.project.bodies[mi].mesh.clone()) else { return };
    pc.recognise.asked = Some(key);
    let (slot, ctx) = (pc.recognise.found.clone(), ctx.clone());
    std::thread::spawn(move || {
        let (kinds, counts) = qymcad_meshfit::classify(&mesh, tol, sharp);
        if let Ok(mut found) = slot.lock() {
            *found = Some(qymcad_ui_state::RecogniseFound { key, kinds, counts });
        }
        ctx.request_repaint();
    });
}

/// The recognition tool opened on a node already made of `src`: `op` 0 for exact surfaces, 1 for the polyhedron.
pub(crate) fn open_recognise_on(pc: &mut qymcad_ui_state::PartCtx, src: Id, was_3d: bool, op: u8) {
    pc.cmd.open(pc.armed, 35, was_3d);
    qymcad_ui_state::borrow_view(*pc.cam, *pc.mode_3d, *pc.view, &mut *pc.view_restore);
    *pc.mode_3d = true;
    pc.feat.op = op;
    pc.recognise.src = Some(src);
    pc.recognise.asked = None;
    pc.cmd.params = Vec::new();
    *pc.status = Notice::info(qymcad_i18n::tr("msg-recognise-picked"));
}

/// RECOGNISE A MESH: click the mesh, then Enter. The bar says what comes out - exact surfaces or the mesh as it is -
/// and the tolerance stands in the field at the geometry, as every other number of a command does.
pub fn start_recognise_cmd(pc: &mut qymcad_ui_state::PartCtx) {
    pc.cmd.open(pc.armed, 35, *pc.mode_3d);
    qymcad_ui_state::borrow_view(*pc.cam, *pc.mode_3d, *pc.view, pc.view_restore);
    *pc.mode_3d = true;
    pc.gsel.edges.clear();
    pc.gsel.faces.clear();
    pc.gsel.faces_body = None;
    pc.recognise.src = None;
    pc.feat.op = 0;
    pc.recognise.asked = None;
    pc.cmd.params = vec![
        qymcad_ui_state::CmdParam::new("f-recognise-tol", "tol", 1.0, 0.1, 1000.0),
        qymcad_ui_state::CmdParam::new("f-recognise-sharp", "sharp", 30.0, 5.0, 90.0),
        qymcad_ui_state::CmdParam::new("f-recognise-simplify", "simplify", 0.0, 0.0, 10.0),
    ];
    *pc.status = Notice::info(qymcad_i18n::tr("msg-recognise"));
}

/// TRIM A SURFACE: click THE part of the sheet that stays, then the tool body.
pub fn start_trim_cmd(pc: &mut qymcad_ui_state::PartCtx) {
    pc.cmd.open(pc.armed, 34, *pc.mode_3d);
    qymcad_ui_state::borrow_view(*pc.cam, *pc.mode_3d, *pc.view, pc.view_restore);
    *pc.mode_3d = true;
    pc.gsel.edges.clear();
    pc.gsel.faces.clear();
    pc.gsel.faces_body = None;
    pc.trim.keep = None;
    pc.trim.tool = None;
    pc.cmd.params = vec![];
    *pc.status = Notice::info(qymcad_i18n::tr("msg-trim"));
}

/// SPLIT A BODY: click a plane, a datum or a face, set the offset at the geometry, and Enter breaks the
/// body into independent pieces.
pub fn start_split_cmd(pc: &mut qymcad_ui_state::PartCtx) {
    if qymcad_ui_state::op_target_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, *pc.sel).is_none() {
        *pc.status = Notice::refusal(qymcad_i18n::tr("msg-no-body"));
        return;
    }
    pc.cmd.open(pc.armed, 27, *pc.mode_3d);
    qymcad_ui_state::borrow_view(*pc.cam, *pc.mode_3d, *pc.view, pc.view_restore); // the view is borrowed and returned on exit
    *pc.mode_3d = true;
    pc.split.plane = None;
    pc.cmd.params = vec![qymcad_ui_state::CmdParam::new("f-offset", "offset", 0.0, -100000.0, 100000.0)];
    *pc.status = Notice::info(qymcad_i18n::tr("msg-split-body"));
}

/// SPLIT FACES: the same reference plane a body cut uses, but the body stays ONE - only the faces are
/// divided. That is how an area is marked out for painting or machining without breaking the part
/// apart.
pub fn start_split_face_cmd(pc: &mut qymcad_ui_state::PartCtx) {
    if qymcad_ui_state::op_target_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, *pc.sel).is_none() {
        *pc.status = Notice::refusal(qymcad_i18n::tr("msg-no-body"));
        return;
    }
    pc.cmd.open(pc.armed, 29, *pc.mode_3d);
    qymcad_ui_state::borrow_view(*pc.cam, *pc.mode_3d, *pc.view, pc.view_restore);
    *pc.mode_3d = true;
    pc.split.plane = None;
    pc.cmd.params = vec![qymcad_ui_state::CmdParam::new("f-offset", "offset", 0.0, -100000.0, 100000.0)];
    *pc.status = Notice::info(qymcad_i18n::tr("msg-split-face"));
}

/// REPLACE A FACE WITH A SURFACE: the node that sews the design layer to the timeline. Two picks in one
/// gesture - the faces of the base and the sheet; neither can be mistaken for anything else, so there is
/// no mode to switch between them.
pub fn start_surface_replace_cmd(pc: &mut qymcad_ui_state::PartCtx) {
    if qymcad_ui_state::op_target_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, *pc.sel).is_none() {
        *pc.status = Notice::refusal(qymcad_i18n::tr("msg-no-body"));
        return;
    }
    pc.cmd.open(pc.armed, 31, *pc.mode_3d);
    qymcad_ui_state::borrow_view(*pc.cam, *pc.mode_3d, *pc.view, pc.view_restore);
    *pc.mode_3d = true;
    pc.gsel.edges.clear();
    pc.gsel.faces.clear();
    pc.gsel.faces_body = None;
    *pc.repl_surface = None;
    pc.cmd.params = vec![];
    *pc.status = Notice::info(qymcad_i18n::tr("msg-surface-replace"));
}

/// REMOVE A FACE AND HEAL: click the faces of an element (a hole, a boss), and Enter takes them away.
///
/// A FACE COPY: the faces of a body become a separate SURFACE while the body stays where it is. It is
/// the bridge from the parametric side into the design layer. There are no parameters - the operation is
/// defined solely by which faces are picked.
/// COPY FACES (30) OR OFFSET THEM (36): faces of a body taken out as a sheet - in place, or moved along their normals by
/// the distance in the field at the geometry. A face picked before the offset is taken at once.
pub fn start_face_copy_cmd(pc: &mut qymcad_ui_state::PartCtx, kind: u8) {
    if qymcad_ui_state::op_target_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, *pc.sel).is_none() {
        *pc.status = Notice::refusal(qymcad_i18n::tr("msg-no-body"));
        return;
    }
    let picked = match *pc.sel {
        qymcad_ui_state::Sel::Face(mi, fi) if kind == 36 => pc.project.mesh_id(mi).zip(pc.project.bodies.get(mi).and_then(|b| b.faces.get(fi)).map(|f| f.id)),
        _ => None,
    };
    pc.cmd.open(pc.armed, kind, *pc.mode_3d);
    qymcad_ui_state::borrow_view(*pc.cam, *pc.mode_3d, *pc.view, pc.view_restore);
    *pc.mode_3d = true;
    pc.gsel.edges.clear();
    pc.gsel.faces.clear();
    pc.gsel.faces_body = None;
    if let Some((b, fid)) = picked {
        pc.gsel.faces.insert(fid);
        pc.gsel.faces_body = Some(b);
    }
    pc.cmd.params = if kind == 36 { vec![qymcad_ui_state::CmdParam::new("f-distance", "dist", 5.0, -10000.0, 10000.0)] } else { vec![] };
    *pc.status = Notice::info(qymcad_i18n::tr(if kind == 36 { "msg-offset-surface" } else { "msg-face-copy" }));
}

/// THICKEN: click a face, set the thickness at the geometry, and Enter grows the face into A PLATE.
/// The plate is glued to the part, because a part is ONE body. As a separate body it was painted in a
/// different colour, and one part came out on screen as two.
pub fn start_thicken_cmd(pc: &mut qymcad_ui_state::PartCtx) {
    if qymcad_ui_state::op_target_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, *pc.sel).is_none() {
        *pc.status = Notice::refusal(qymcad_i18n::tr("msg-no-body"));
        return;
    }
    pc.cmd.open(pc.armed, 28, *pc.mode_3d);
    qymcad_ui_state::borrow_view(*pc.cam, *pc.mode_3d, *pc.view, pc.view_restore);
    *pc.mode_3d = true;
    pc.gsel.edges.clear();
    pc.gsel.faces.clear();
    pc.gsel.faces_body = None;
    pc.cmd.params = vec![qymcad_ui_state::CmdParam::new("f-thickness", "thickness", 2.0, -100000.0, 100000.0)];
    *pc.status = Notice::info(qymcad_i18n::tr("msg-thicken"));
}

/// DRAFT: tilt the faces of a body relative to a neutral face. With the body selected, click the faces
/// to tilt (a multiple selection) and pick the reference with the neutral-face button, then the angle,
/// then Enter.
///
/// PUSH A FACE - direct modelling: click a flat face, set the offset, Enter.
///
/// The same command experience as everywhere: the bar on top, a field at the geometry (with an
/// expression), the face picked by click, Enter/Esc. A positive offset adds material, a negative one
/// cuts it away.
pub fn start_push_face_cmd(pc: &mut qymcad_ui_state::PartCtx) {
    if qymcad_ui_state::op_target_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, *pc.sel).is_none() {
        *pc.status = Notice::refusal(qymcad_i18n::tr("msg-no-body"));
        return;
    }
    pc.cmd.open(pc.armed, 25, *pc.mode_3d);
    qymcad_ui_state::borrow_view(*pc.cam, *pc.mode_3d, *pc.view, pc.view_restore); // the view is borrowed and returned on exit
    *pc.mode_3d = true;
    pc.gsel.edges.clear();
    pc.gsel.faces.clear(); // the face is picked afresh: a pre-selection would only confuse here
    pc.gsel.faces_body = None;
    pc.cmd.params = vec![qymcad_ui_state::CmdParam::new("f-offset2", "dist", 5.0, -100000.0, 100000.0).nonzero()];
    *pc.status = Notice::info(qymcad_i18n::tr("msg-push-face"));
}

/// A THREAD: a real thread on a cylinder or inside a hole. With a body present, click a CYLINDRICAL
/// face; the top bar carries the side, the number of starts and the hand, while the popup at the
/// geometry carries the pitch, the length, the angle and the depth.
pub fn start_thread_cmd(pc: &mut qymcad_ui_state::PartCtx) {
    if qymcad_ui_state::op_target_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, *pc.sel).is_none() {
        *pc.status = Notice::refusal(qymcad_i18n::tr("msg-no-cylinder"));
        return;
    }
    pc.cmd.open(pc.armed, 24, *pc.mode_3d); // a clean slate, then open

    *pc.mode_3d = true;
    pc.thread.src = None;
    pc.thread.edge = 0;
    pc.thread.internal = false;
    pc.thread.starts = 1;
    pc.thread.left = false;
    pc.thread.form = 0;
    pc.thread.auger = false;
    qymcad_ui_state::set_thread_params(pc.cmd, *pc.thread);
    *pc.status = Notice::info(qymcad_i18n::tr("msg-thread"));
}

/// A primitive as A COMMAND: 10 box, 11 cylinder, 12 sphere, 13 cone, 14 torus, 15 prism. The sizes are
/// expression fields at the geometry (the popup) plus a wireframe PREVIEW; Enter creates, Esc cancels.
/// By default it sits at the origin; a click on a vertex, a datum point, a plane or a face PLACES it
/// (orienting the base along the normal of the plane).
pub fn start_prim_cmd(pc: &mut qymcad_ui_state::PartCtx, code: u8) {
    pc.cmd.focus = true;
    pc.boolean.edit = None;
    pc.cmd.open(pc.armed, code, *pc.mode_3d); // a clean slate, then open

    *pc.mode_3d = true;
    pc.prim.n = 6;
    pc.prim.place = None;
    pc.prim.frame = None;
    pc.gsel.faces.clear();
    pc.gsel.faces_body = None;
    pc.gsel.edges.clear();
    // THE KEYS match those of the regen and of the feature properties (r/r1/r2/major/minor/dx/dy/dz/h -
    // RADII), otherwise the expressions would not bind (the regen reads `r`, not `dia`) and reopening
    // would go out of step.
    pc.cmd.params = match code {
        10 => vec![
            qymcad_ui_state::CmdParam::new("f-length-x", "dx", 20.0, 0.1, 100000.0),
            qymcad_ui_state::CmdParam::new("f-width-y", "dy", 20.0, 0.1, 100000.0),
            qymcad_ui_state::CmdParam::new("f-height-z", "dz", 20.0, 0.1, 100000.0),
        ],
        11 => vec![qymcad_ui_state::CmdParam::new("f-radius", "r", 10.0, 0.05, 100000.0), qymcad_ui_state::CmdParam::new("f-height", "h", 20.0, 0.1, 100000.0)],
        12 => vec![qymcad_ui_state::CmdParam::new("f-radius", "r", 10.0, 0.05, 100000.0)],
        13 => vec![
            qymcad_ui_state::CmdParam::new("f-radius-bottom", "r1", 10.0, 0.01, 100000.0),
            qymcad_ui_state::CmdParam::new("f-radius-top", "r2", 0.0, 0.0, 100000.0),
            qymcad_ui_state::CmdParam::new("f-height", "h", 20.0, 0.1, 100000.0),
        ],
        14 => vec![qymcad_ui_state::CmdParam::new("f-ring-r", "major", 12.0, 0.1, 100000.0), qymcad_ui_state::CmdParam::new("f-tube-r", "minor", 4.0, 0.1, 100000.0)],
        15 => vec![qymcad_ui_state::CmdParam::new("f-radius-circ", "r", 10.0, 0.1, 100000.0), qymcad_ui_state::CmdParam::new("f-height", "h", 20.0, 0.1, 100000.0)],
        _ => vec![],
    };
    *pc.status = Notice::info(qymcad_i18n::tr("msg-primitive"));
}

/// LOFT (through sections) - a body through two or more sketch sections. The first section is the
/// selected sketch; after that, clicking sketches in the tree adds sections in order. Every section must
/// have a closed contour.
pub fn start_loft_cmd(pc: &mut qymcad_ui_state::PartCtx) {
    let si = match *pc.sel {
        qymcad_ui_state::Sel::Sketch(si) => Some(si),
        _ => pc.cmd.sketch,
    };
    let Some(si) = si.filter(|&si| si < pc.project.sketches.len()) else {
        // WAITS LIKE THE REST. It used to write the line and return, and a line in the status bar is not an
        // answer to a click.
        *pc.picking = qymcad_ui_state::Picking::SketchFor(9);
        *pc.status = Notice::info(qymcad_i18n::tr("msg-loft-pick-first"));
        return;
    };
    if qymcad_ui_state::sketch_closed_contours(pc.project, si).is_empty() {
        *pc.status = Notice::refusal(qymcad_i18n::tr("msg-section-no-contour"));
        return;
    }
    pc.cmd.open(pc.armed, 9, *pc.mode_3d); // a clean slate, then open

    pc.cmd.sketch = Some(si);
    pc.loft.sids = vec![pc.project.sketches[si].id];
    pc.loft.cids = vec![0];
    pc.loft.ruled = false;
    pc.loft.pick = true; // the next sections are expected to be added by clicking in the tree
    pc.loft.pick_last = Some(pc.project.sketches[si].id); // the first section is already in the set - no duplicate
    pc.loft.result = 0; // by default a separate new body
    pc.cmd.params.clear();
    qymcad_ui_state::borrow_view(*pc.cam, *pc.mode_3d, *pc.view, pc.view_restore); // the view is borrowed for the command and returned on exit - no refitting
    *pc.mode_3d = true;
    *pc.status = Notice::info(qymcad_i18n::tr("msg-loft-pick"));
}

/// SWEEP - a profile (the selected sketch with a closed contour) along a path (a second sketch, picked
/// in the tree). The profile usually lies on a plane at the start of the path, roughly perpendicular
/// to it.
pub fn start_sweep_cmd(pc: &mut qymcad_ui_state::PartCtx) {
    // the profile is the sketch selected in the tree (or the one captured earlier)
    let si = match *pc.sel {
        qymcad_ui_state::Sel::Sketch(si) => Some(si),
        _ => pc.cmd.sketch,
    };
    let Some(si) = si.filter(|&si| si < pc.project.sketches.len()) else {
        *pc.picking = qymcad_ui_state::Picking::SketchFor(8); // waits like the rest
        *pc.status = Notice::info(qymcad_i18n::tr("msg-sweep-pick-profile"));
        return;
    };
    if qymcad_ui_state::sketch_closed_contours(pc.project, si).is_empty() {
        *pc.status = Notice::refusal(qymcad_i18n::tr("msg-profile-no-contour"));
        return;
    }
    pc.cmd.open(pc.armed, 8, *pc.mode_3d); // a clean slate, then open

    pc.feat.op = 0; // a sweep starts as "add" (the op is chosen in the bar)
    pc.cmd.sketch = Some(si);
    pc.sweep.prof_sid = pc.project.sketches[si].id;
    pc.sweep.path_sid = 0;
    pc.sweep.pick_path = true; // the path pick is expected straight away
    pc.cmd.params.clear();
    qymcad_ui_state::borrow_view(*pc.cam, *pc.mode_3d, *pc.view, pc.view_restore); // the view is borrowed for the command and returned on exit - no refitting
    *pc.mode_3d = true;
    *pc.status = Notice::info(qymcad_i18n::tr("msg-sweep-pick-path"));
}

/// An array as A COMMAND: pick a body, then the top bar (the count, the direction, the axis), then the
/// STEP or ANGLE as an expression field AT THE GEOMETRY, then ghost previews of the copies, then Enter.
/// 17 is linear (a grid), 18 is circular.
pub fn start_array_cmd(pc: &mut qymcad_ui_state::PartCtx, cmd: u8) {
    if qymcad_ui_state::op_target_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, *pc.sel).is_none() {
        *pc.status = Notice::refusal(qymcad_i18n::tr("msg-no-body"));
        return;
    }
    pc.cmd.open(pc.armed, cmd, *pc.mode_3d); // a clean slate, then open

    *pc.mode_3d = true;
    pc.arr.count = if cmd == 18 { 6 } else { 3 };
    pc.arr.dir = 0;
    pc.arr.two = false;
    pc.arr.count2 = 2;
    pc.arr.dir2 = 1;
    pc.arr.three = false;
    pc.arr.count3 = 2;
    pc.arr.dir3 = 2;
    pc.arr.axis = 0;
    pc.arr.full = true;
    pc.arr.axis_pick = false;
    // a full circular array asks no angle (the sync adds the field once "full" is switched off)
    pc.cmd.params = if cmd == 18 { vec![] } else { vec![qymcad_ui_state::CmdParam::new("f-pitch", "step", 25.0, 0.01, 100000.0)] };
    *pc.status = Notice::info(if cmd == 18 { qymcad_i18n::tr("msg-circ-array") } else { qymcad_i18n::tr("msg-lin-array") });
}

/// THE CLICK THAT NAMES THE SKETCH a waiting tool asked for.
///
/// `hit` is what the click landed on, worked out by the caller: the answer needs the whole scene and the
/// act needs the document, and the two borrows cannot be held at once.
///
/// A MISS SAYS SO rather than putting the tool down. Pointing at nothing is an aim that missed, not a
/// change of mind; putting the tool down on it would make every stray click cancel the command.
pub fn name_the_sketch(pc: &mut qymcad_ui_state::PartCtx, hit: Option<usize>) {
    match hit {
        Some(si) => *pc.sel = qymcad_ui_state::Sel::Sketch(si), // the frame's own step continues the command
        None => *pc.status = Notice::info(qymcad_i18n::tr("msg-pick-sketch-first")),
    }
}

pub fn start_sketch_cmd(pc: &mut qymcad_ui_state::PartCtx, cmd: u8) {
    let si = match *pc.sel {
        qymcad_ui_state::Sel::Sketch(si) => Some(si),
        _ => pc.cmd.sketch,
    };
    let Some(si) = si.filter(|&si| si < pc.project.sketches.len()) else {
        // THE TOOL IS TAKEN IN HAND AND WAITS instead of not starting. It used to write the line below and
        // return, and a line in the status bar is not an answer to a click: what a person sees is that they
        // pressed and nothing happened. The wait is answered by `take_sketch_if_waiting`.
        *pc.picking = qymcad_ui_state::Picking::SketchFor(cmd);
        *pc.status = Notice::info(qymcad_i18n::tr("msg-pick-sketch-first"));
        return;
    };
    let closed = qymcad_ui_state::sketch_closed_contours(pc.project, si);
    if closed.is_empty() {
        // A SKETCH THAT CANNOT BE EXTRUDED IS NOT TAKEN, BUT THE TOOL IS: it waits for one that can, as with nothing
        // selected. Refusing put the tool down too, and the tool the hand was just moved on from was gone with nothing
        // in its place (a sweep's path picked, then Extrude pressed). The selection that does not fit is let go, or the
        // wait would take it again at once.
        *pc.sel = qymcad_ui_state::Sel::None;
        *pc.picking = qymcad_ui_state::Picking::SketchFor(cmd);
        *pc.status = Notice::refusal(qymcad_i18n::tr("msg-no-closed-contour"));
        return;
    }
    let was_3d = pc.cmd.prev_3d;
    pc.cmd.open(pc.armed, cmd, was_3d); // the checks passed - the command is open
    pc.cmd.edit = None; // a new feature, not an edit
    if cmd == 3 {
        pc.feat.op = 0; // revolve starts as "add" (for extrude the op is set by the tool button)
    }
    // CLEARING THE STALE STATE OF THE PREVIOUS COMMAND: "through", symmetry and the second side must NOT
    // travel silently into the new one (a through cut followed by an extrude in an empty part built a
    // 2000 mm body or broke silently). A new command means a clean one-sided extent.
    pc.cmd.extent = qymcad_ui_state::ExtentMode::default();
    // a sensible default for the direction: a CUT on a face of the part goes INTO the body (the negative
    // normal), otherwise it would cut outwards into the void. Everything else goes outwards (the
    // positive normal); dragging the gizmo or pressing flip reverses it, and the preview equals the
    // result. The command opens with the direction still automatic - it is computed on apply.
    *pc.feat = qymcad_ui_state::FeatTarget::opened(pc.feat.op);
    pc.cmd.sketch = Some(si);
    pc.gsel.profiles.clear();
    pc.rev.axis_datum = 0; // by default the X or Y of the sketch
    pc.rev.axis_line = 0; // no centre line is picked
    pc.rev.pick_axis = false; // the axis-picking sub-mode is off
    pc.rev.pick_line = false;
    pc.cmd.params = match cmd {
        3 => vec![qymcad_ui_state::CmdParam::new("f-angle", "angle", pc.rev.angle, 1.0, 360.0)],
        _ => vec![qymcad_ui_state::CmdParam::new("f-length", "height", pc.set.defaults.extrude_h.max(0.1), 0.1, 10000.0)],
    };
    // A SINGLE CONTOUR IS TAKEN AT ONCE, so the preview and the gizmo appear with the command. With two or more
    // nothing is taken: a click inside a contour adds its region, as a profile is picked in the professional
    // systems. Reported behaviour: every contour came in taken, so the first click in the ring round a hole let
    // the rectangle go and the circle alone was extruded (785 mm^3 for a plate of 11215), and a click on the second
    // of two squares let it go.
    if closed.len() == 1 {
        pc.gsel.profiles.insert(closed[0]);
    }
    qymcad_ui_state::borrow_view(*pc.cam, *pc.mode_3d, *pc.view, pc.view_restore); // the view is borrowed for the command and returned on exit - no refitting
    *pc.mode_3d = true;
    let what = if cmd == 3 { qymcad_i18n::tr("f-revolve") } else { qymcad_i18n::tr("f-extrude") };
    *pc.status = Notice::info(qymcad_i18n::tr1("cmd-pick-contours-hint", "what", &what));
    // WITH SOMETHING TO CHOOSE, THE COMMAND ASKS BEFORE IT SHOWS A SIZE.
    //
    // Reported behaviour: to change which contours are used a person had to find the "Pick contours"
    // button, or U/Alt+U, having already been shown a body built out of ALL of them. Now the creation of a
    // feature starts where the choice is - the flat view, where a click lands on a contour.
    //
    // ONLY WITH TWO OR MORE, and only when CREATING. A picker over a single candidate is a step that can
    // end one way, which is worse than no step at all; and an edit has already been told which contours the
    // feature is made of, so it opens on the size, with re-picking left on the button.
    if closed.len() > 1 {
        enter_contour_reselect(pc);
    }
}

/// A PATCH: stretch a surface over the picked edges. The first tool of this layer that creates a shape
/// the body did not have.
pub fn start_patch_cmd(pc: &mut qymcad_ui_state::PartCtx) {
    if qymcad_ui_state::op_target_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: &*pc.project, active_path: pc.active_path }, *pc.sel).is_none() {
        *pc.status = Notice::refusal(qymcad_i18n::tr("msg-no-body"));
        return;
    }
    pc.cmd.open(pc.armed, 32, *pc.mode_3d);
    qymcad_ui_state::borrow_view(*pc.cam, *pc.mode_3d, *pc.view, &mut *pc.view_restore);
    *pc.mode_3d = true;
    pc.gsel.faces.clear();
    pc.gsel.edges.clear();
    refresh_edges(pc);
    pc.cmd.params = vec![];
    *pc.status = Notice::info(qymcad_i18n::tr("msg-patch"));
}
