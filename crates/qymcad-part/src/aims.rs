//! WHAT A TOOL OF THE PART AIMS AT UNDER THE POINTER, and what it takes of what the window found there. The window
//! asks the tool in hand what it aims at, finds that under the pointer - a plane, a vertex, an axis, a face - and hands
//! it here; what the tool does with it is the tool's own.

use super::*;
use qymcad_ui_state::Notice;

/// WHAT THE TOOL IN HAND AIMS AT under a click in the view.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Aim {
    /// A plane, a datum or a face: the mirror plane, the cutting plane, the base of a datum plane.
    Plane,
    /// A vertex of the part in hand: a datum point at a vertex, associative.
    VertexOfThePart,
    /// A point snapped to what is under the pointer: a datum point placed freely.
    PointSnap,
    /// A straight edge, a cylindrical face or a datum axis: a datum axis on a reference.
    Axis,
    /// Two datum points or vertices: a datum axis through them.
    TwoPoints,
    /// A vertex, a datum point, a plane or a face that places a primitive.
    Placement,
    /// The axis of a circular pattern.
    PatternAxis,
    /// The axis of a revolution.
    RevolveAxis,
    /// The face a chamfer of two legs or of a leg and an angle measures from.
    ReferenceFace,
    /// Edges, or a face for all of its edges.
    Edges,
    /// The cylindrical face a thread goes on.
    ThreadRim,
}

/// WHAT THE TOOL IN HAND AIMS AT, in the step it is at; `None` when a click goes to the body under it.
pub fn aim(pc: &qymcad_ui_state::PartCtx) -> Option<Aim> {
    use qymcad_ui_state::PartTool as T;
    Some(match pc.armed.part_tool()? {
        t if t.aims_at_a_plane() => Aim::Plane,
        T::DatumPoint if pc.datum.pt_mode == 1 => Aim::VertexOfThePart,
        T::DatumPoint => Aim::PointSnap,
        T::DatumAxis if pc.datum.axis_mode == 0 => Aim::Axis,
        T::DatumAxis if pc.datum.axis_mode == 2 => Aim::TwoPoints,
        t if t.is_a_primitive() && pc.cmd.edit.is_none() => Aim::Placement,
        T::CircularPattern if pc.arr.axis_pick => Aim::PatternAxis,
        T::Revolve if pc.rev.pick_axis => Aim::RevolveAxis,
        T::Chamfer if pc.chamfer.pick_ref && pc.chamfer.mode != qymcad_core::feature::ChamferMode::Symmetric => Aim::ReferenceFace,
        t if t.picks_edges() => Aim::Edges,
        T::Thread => Aim::ThreadRim,
        _ => return None,
    })
}

/// THE PLANE UNDER A CLICK taken by the tool that aims at one: the mirror plane, the cutting plane of a split - with
/// how many pieces will come out, said at once - or the base of a datum plane. The same plane again lets it go.
pub fn take_plane(pc: &mut qymcad_ui_state::PartCtx, plane: Option<qymcad_core::feature::SketchPlane>) {
    use qymcad_ui_state::PartTool as T;
    let Some(sp) = plane else {
        *pc.status = Notice::refusal(qymcad_i18n::tr("vp-miss-plane-short"));
        return;
    };
    match pc.armed.part_tool() {
        Some(T::Mirror) => {
            pc.mirror.plane = (pc.mirror.plane != Some(sp)).then_some(sp);
            *pc.status = Notice::info(qymcad_i18n::tr(if pc.mirror.plane.is_some() { "vp-mirror-plane-picked" } else { "msg-mirror-pick" }));
        }
        Some(T::DatumPlane) => {
            pc.datum.plane_pick = (pc.datum.plane_pick != Some(sp)).then_some(sp);
            *pc.status = Notice::info(qymcad_i18n::tr(if pc.datum.plane_pick.is_some() { "vp-ref-picked" } else { "hint-plane-offset" }));
        }
        _ => {
            pc.split.plane = (pc.split.plane != Some(sp)).then_some(sp);
            let src =
                qymcad_ui_state::op_target_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, *pc.sel).unwrap_or(0);
            *pc.status = match split_piece_count(pc, src) {
                // how many pieces will come out is visible AT ONCE: otherwise "it cuts nothing" would only be learnt
                // on Enter, after a click and an offset had already been spent
                Some(n) if n >= 2 => Notice::info(qymcad_i18n::tr1("vp-cut-plane-picked", "n", &n.to_string())),
                _ => Notice::refusal(qymcad_i18n::tr("vp-plane-cuts-nothing")),
            };
        }
    }
}

/// A VERTEX OF THE PART IN HAND: the end of a persistent edge and where it stands in the frame of its body.
pub struct VertexAt {
    pub body: Id,
    pub edge: u32,
    pub end: bool,
    pub at: [f64; 3],
}

/// A DATUM POINT AT A VERTEX: the vertex makes an ASSOCIATIVE reference.
pub fn take_vertex(pc: &mut qymcad_ui_state::PartCtx, vertex: Option<VertexAt>) {
    match vertex {
        Some(v) => {
            pc.datum.pt_vert = Some((v.body, v.edge, v.end, v.at));
            *pc.status = Notice::info(qymcad_i18n::tr("vp-vertex-picked"));
        }
        None => *pc.status = Notice::refusal(qymcad_i18n::tr("vp-miss-vertex-current")),
    }
}

/// AN AXIS UNDER A CLICK: what was hit, and the axis it gives in the world - none when the reference gives no axis.
pub struct AxisAt {
    pub hit: qymcad_ui_state::AxisHit,
    pub axis: Option<Line>,
}

/// A LINE IN THE WORLD: a point on it and its direction.
pub struct Line {
    pub origin: [f64; 3],
    pub dir: [f64; 3],
}

/// A DATUM AXIS ON A REFERENCE: a straight edge or a cylindrical face (Enter creates it ASSOCIATIVELY). The same edge or
/// face again lets it go.
pub fn take_axis(pc: &mut qymcad_ui_state::PartCtx, axis: Option<AxisAt>) {
    let Some(AxisAt { hit, axis }) = axis else {
        *pc.status = Notice::refusal(qymcad_i18n::tr("vp-miss-edge-or-cyl"));
        return;
    };
    let Some(Line { origin, dir }) = axis else {
        *pc.status = Notice::refusal(qymcad_i18n::tr("vp-ref-gives-no-axis"));
        return;
    };
    pc.datum.axis_hit = (pc.datum.axis_hit != Some(hit)).then_some(hit);
    pc.datum.axis_ref = pc.datum.axis_hit.map(|_| (origin, dir));
    *pc.status = Notice::info(qymcad_i18n::tr(if pc.datum.axis_hit.is_some() { "vp-axis-picked" } else { "msg-axis-pick" }));
}

/// A POINT A DATUM AXIS GOES THROUGH: a datum point (by its id) or a vertex (id 0), and where it stands.
pub struct PointAt {
    pub datum: Id,
    pub at: [f64; 3],
}

/// A DATUM AXIS BY TWO POINTS: two datum points or vertices are gathered; datum points make it parametric.
pub fn take_axis_point(pc: &mut qymcad_ui_state::PartCtx, point: Option<PointAt>) {
    let Some(PointAt { datum, at }) = point else {
        *pc.status = Notice::refusal(qymcad_i18n::tr("vp-miss-datum-point"));
        return;
    };
    if pc.datum.axis_pts.len() >= 2 {
        pc.datum.axis_pts.clear();
    }
    pc.datum.axis_pts.push((datum, at));
    if pc.datum.axis_pts.len() == 2 {
        let (a, b) = (pc.datum.axis_pts[0].1, pc.datum.axis_pts[1].1);
        let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        pc.datum.axis_ref = (l > 1e-9).then(|| (a, [d[0] / l, d[1] / l, d[2] / l]));
        *pc.status = Notice::info(qymcad_i18n::tr("vp-two-points-set"));
    } else {
        *pc.status = Notice::info(qymcad_i18n::tr("vp-point1-set"));
    }
}

/// A PRIMITIVE PLACED by a click on a vertex, a datum point, a plane or a face: its frame, with an orientation.
pub fn take_placement(pc: &mut qymcad_ui_state::PartCtx, frame: Option<[f64; 12]>) {
    match frame {
        Some(m) => {
            pc.prim.frame = Some(m);
            pc.prim.place = Some([m[3], m[7], m[11]]);
            *pc.status = Notice::info(qymcad_i18n::tr("vp-placement-set"));
        }
        None => *pc.status = Notice::refusal(qymcad_i18n::tr("vp-miss-placement")),
    }
}

/// THE CHAMFER'S REFERENCE FACE: a click on a face sets it, a second click on the same face clears it. A miss changes
/// nothing.
pub fn take_reference_face(pc: &mut qymcad_ui_state::PartCtx, face: Option<u32>) {
    match face {
        Some(fid) => {
            pc.chamfer.ref_face = if pc.chamfer.ref_face == fid { 0 } else { fid };
            pc.chamfer.pick_ref = false;
            *pc.status = Notice::info(if pc.chamfer.ref_face != 0 { qymcad_i18n::tr("vp-chamfer-ref-set") } else { qymcad_i18n::tr("vp-chamfer-ref-cleared") });
        }
        None => *pc.status = Notice::refusal(qymcad_i18n::tr("vp-miss-body-face")),
    }
}

/// DO SHEETS COME FIRST under the pointer: for the tools that take a sheet lying on the face of the part - stitching,
/// trimming, and a replacement whose faces are taken and whose surface is not yet.
pub fn sheets_come_first(armed: &qymcad_ui_state::Armed, faces_taken: usize, surface: Option<Id>) -> bool {
    use qymcad_ui_state::PartTool as T;
    match armed.part_tool() {
        Some(T::Stitch | T::Trim) => true,
        Some(T::SurfaceReplace) => faces_taken > 0 && surface.is_none(),
        _ => false,
    }
}

/// A CLICK ON A BODY IN THE VIEW of a part: the mesh under it, the face of it when the tool takes faces, where the
/// click was on the screen and where its ray met the body.
pub struct BodyClick {
    pub mesh: usize,
    pub face: Option<usize>,
    pub rect: Rect,
    pub screen: Pos2,
    pub ray_at: Option<[f64; 3]>,
}

/// WHAT THE TOOL IN HAND TAKES OF A CLICK ON A BODY - the faces of a set, one face, the steps of a trim, a whole body, the
/// surface of a replacement, the neutral face of a draft. The window says what was under the click; this says what it
/// means.
pub fn click_on_body(pc: &mut qymcad_ui_state::PartCtx, click: BodyClick) {
    use qymcad_ui_state::PartTool as T;
    let Some(tool) = pc.armed.part_tool() else { return };
    let BodyClick { mesh: mi, face: fi, rect, screen, ray_at } = click;
    let clicked = pc.project.mesh_id(mi);
    let is_sheet = pc.project.bodies.get(mi).is_some_and(|b| b.sheet);
    let face_id = fi.and_then(|fi| pc.project.bodies.get(mi).and_then(|b| b.faces.get(fi))).map(|f| f.id);
    // FACES OF ONE BODY: the ids of faces are local to a body (OCCT numbers them from zero in each), so a click on a
    // face of ANOTHER body starts the set afresh on it, or the ids of neighbouring bodies get confused and the
    // highlight leaks across. Replacing a face is not among them: there a click on another body names the surface.
    if tool.gathers_faces_of_one_body() && fi.is_some() && pc.gsel.faces_body != clicked {
        pc.gsel.faces.clear();
        pc.draft.neutral = 0;
        pc.gsel.faces_body = clicked;
    }
    // THE SHELL: a click on a face ADDS or REMOVES its id from the set (by the persistent id)
    if tool == T::Shell {
        toggle_face(pc, face_id, false);
    }
    // ONE FACE OR A SET BY A TOGGLE: pushing several faces by one offset would give a different result on each of them
    // (their normals differ), and one plate comes of one face - those take EXACTLY ONE, a click replaces the one before
    if tool.toggles_faces() {
        toggle_face(pc, face_id, tool.takes_one_face());
        place_hole_at(pc, rect, screen);
    }
    // TRIM: the first click on a SHEET says both WHAT is being cut and WHICH part is kept - the point of the hit is the
    // answer about the side. The second click, on a body, gives the tool.
    if tool == T::Trim {
        if is_sheet && pc.trim.keep.is_none() {
            if let (Some(id), Some(at)) = (clicked, ray_at) {
                pc.trim.keep = Some((id, at));
                *pc.status = Notice::info(qymcad_i18n::tr("msg-trim-pick-tool"));
            }
        } else if let Some(id) = clicked {
            if pc.trim.keep.map(|(b, _)| b) == Some(id) {
                (pc.trim.keep, *pc.sel) = (None, qymcad_ui_state::Sel::None); // a repeated click on the same sheet starts afresh, unlit
                *pc.status = Notice::info(qymcad_i18n::tr("msg-trim"));
            } else {
                pc.trim.tool = if pc.trim.tool == Some(id) { None } else { Some(id) };
                *pc.status = Notice::info(qymcad_i18n::tr(if pc.trim.tool.is_some() { "msg-trim-tool-picked" } else { "msg-trim-pick-tool" }));
            }
        }
    }
    // A WHOLE BODY, for the tools that take one - stitching sheets, recognising a mesh
    if tool.picks_bodies() {
        let is_mesh = clicked.is_some_and(|id| pc.project.timeline.iter().any(|n| matches!(n.kind, qymcad_core::feature::FeatureKind::MeshPiece { body, .. } if body == id)));
        pick_body_for_tool(pc, clicked, is_sheet, is_mesh);
    }
    // REPLACE FACE: a click on a SHEET chooses the surface, a click on a body collects the faces to be replaced. The
    // document itself tells them apart (`sheet`) rather than a mode switch.
    if tool == T::SurfaceReplace {
        if is_sheet {
            *pc.repl_surface = if *pc.repl_surface == clicked { None } else { clicked };
            *pc.status = Notice::info(qymcad_i18n::tr(if pc.repl_surface.is_some() { "msg-surface-picked" } else { "msg-surface-unpicked" }));
        } else if let Some(id) = face_id {
            if pc.gsel.faces_body != clicked {
                pc.gsel.faces.clear();
                pc.gsel.faces_body = clicked;
            }
            toggle_face(pc, Some(id), false);
        }
    }
    // THE DRAFT: in the neutral-face mode a click sets the neutral face (by the persistent id), otherwise a click ADDS or
    // REMOVES a face from the set of faces to be drafted
    if tool == T::Draft {
        if let Some(id) = face_id {
            if pc.draft.pick_neutral {
                pc.draft.neutral = if pc.draft.neutral == id { 0 } else { id };
                pc.draft.pick_neutral = false;
                pc.gsel.faces.remove(&id); // the neutral face cannot also be a drafted one
            } else {
                toggle_face(pc, Some(id), false);
            }
        }
    }
}
