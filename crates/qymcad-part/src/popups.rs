//! THE FIELDS AT THE GEOMETRY: where the box of a command stands, what shuts its gate, and the step typed at a gizmo.

use qymcad_ui_state::Notice;
use super::*;

/// WHAT THE BOX OF THE COMMAND IN HAND STANDS BY, in the world: the tip of the arrow of an extrusion or of a datum
/// plane's offset, the body a command works on, the point of a datum or of a primitive. The frame finds it on the screen
/// and lays the box by it (`qymcad_ui_state::box_corner`). None for a command with no box.
pub fn cmd_place(pc: &mut qymcad_ui_state::PartCtx) -> Option<qymcad_ui_state::Place> {
    use qymcad_ui_state::{By, Place};
    let dc = |pc: &qymcad_ui_state::PartCtx| {
        qymcad_ui_state::op_target_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, *pc.sel)
    };
    match pc.armed.part_tool() {
        Some(qymcad_ui_state::PartTool::Extrude) | Some(qymcad_ui_state::PartTool::Revolve) => {
            let (base, dir, h) = qymcad_ui_state::feat_cmd_axis(pc.cmd, pc.gsel, pc.project)?;
            let dir = if pc.armed.part_tool() == Some(qymcad_ui_state::PartTool::Extrude) && pc.feat.flip { [-dir[0], -dir[1], -dir[2]] } else { dir };
            if pc.armed.part_tool() == Some(qymcad_ui_state::PartTool::Revolve) {
                return Some(Place::by(By::Point(base)));
            }
            // clear of the whole arrow. Reported behaviour: the length field of an extrusion stood against the tip of its
            // arrow, and the arrow could hardly be seen or taken.
            Some(Place::by(By::Arrow { base, tip: [base[0] + dir[0] * h, base[1] + dir[1] * h, base[2] + dir[2] * h] }))
        }
        // THE STITCH STANDS BY THE SHEETS IT HOLDS: with none it stands where it stood before the first, not by the sheet
        // the selection still names after the last was clicked away
        Some(qymcad_ui_state::PartTool::Stitch) => pc.stitch_parts.first().map(|b| Place::by(By::Body(*b))),
        // EVERYTHING AIMED BY CLICKING A BODY KEEPS ITS POPUP BESIDE THE BODY.
        //
        // The fillet and the chamfer already did that, while the hole put its field right at the face
        // and the thread at the rim: that is, over what has to be aimed at next. The cuts had no anchor
        // at all - there was nowhere to show the offset field, although the command has that parameter.
        //
        // The shell, the draft, pushing and removing a face, thickening - all belong here. What helps
        // with aiming is not the popup but THE HANDLE: it sits on the geometry and covers nothing.
        Some(qymcad_ui_state::PartTool::Fillet)
        | Some(qymcad_ui_state::PartTool::Chamfer)
        | Some(qymcad_ui_state::PartTool::Shell)
        | Some(qymcad_ui_state::PartTool::Hole)
        | Some(qymcad_ui_state::PartTool::LinearPattern)
        | Some(qymcad_ui_state::PartTool::CircularPattern)
        | Some(qymcad_ui_state::PartTool::Draft)
        | Some(qymcad_ui_state::PartTool::Thread)
        | Some(qymcad_ui_state::PartTool::PushFace)
        | Some(qymcad_ui_state::PartTool::RemoveFace)
        | Some(qymcad_ui_state::PartTool::SplitBody)
        | Some(qymcad_ui_state::PartTool::Thicken)
        | Some(qymcad_ui_state::PartTool::SplitFace)
        | Some(qymcad_ui_state::PartTool::FaceCopy)
        | Some(qymcad_ui_state::PartTool::SurfaceReplace)
        | Some(qymcad_ui_state::PartTool::Patch)
        | Some(qymcad_ui_state::PartTool::Trim)
        | Some(qymcad_ui_state::PartTool::Recognise)
        | Some(qymcad_ui_state::PartTool::OffsetSurface) => {
            let b = pc.recognise.src.or_else(|| pc.stitch_parts.first().copied()).or(pc.trim.keep.map(|(b, _)| b)).or(pc.gsel.faces_body).or_else(|| dc(pc))?;
            Some(Place::by(By::Body(b)))
        }
        // datum plane: beside the arrow of its offset; point: at (x,y,z); axis: at the origin of the
        // reference or of the hand-typed one
        Some(qymcad_ui_state::PartTool::DatumPlane) => {
            // NOTHING PICKED YET: in the corner of the view, clear of everything a plane or a face is to be picked on - at
            // the origin it stood over the faces of a part drawn there
            let Some((o, n)) =
                pc.datum.plane_pick.as_ref().and_then(|sp| {
                    qymcad_ui_state::mirror_plane_world(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, sp)
                })
            else {
                return Some(Place::by(By::ViewCorner));
            };
            let d = qymcad_ui_state::cmd_val(pc.cmd, "dist");
            Some(Place::by(By::Arrow { base: o, tip: [o[0] + n[0] * d, o[1] + n[1] * d, o[2] + n[2] * d] }))
        }
        // A POINT AND THE ORIGIN OF AN AXIS BY HAND are moved by the arrows and rings out of them: the box stands clear of
        // those as of every handle (`keep_in_sight`)
        Some(qymcad_ui_state::PartTool::DatumPoint) => Some(Place::by(By::Point(["x", "y", "z"].map(|k| qymcad_ui_state::cmd_val(pc.cmd, k))))),
        Some(qymcad_ui_state::PartTool::DatumAxis) => {
            let p = if pc.datum.axis_mode == 1 {
                ["ox", "oy", "oz"].map(|k| qymcad_ui_state::cmd_val(pc.cmd, k))
            } else {
                pc.datum.axis_ref.map(|(o, _)| o).unwrap_or([0.0, 0.0, 0.0])
            };
            Some(Place::by(By::Point(p)))
        }
        // primitives: beside the body while editing (like the chamfer), by the placement point once it is clicked; before
        // the click in the corner of the view - by the origin the box lay over the place about to be clicked
        Some(k) if k.is_a_primitive() => match (qymcad_ui_state::selected_body(pc.project, pc.sel).filter(|_| pc.cmd.edit.is_some()), pc.prim.place) {
            (Some(b), _) => Some(Place::by(By::Body(b))),
            (None, Some(p)) => Some(Place::by(By::Point(p))),
            (None, None) => Some(Place::by(By::ViewCorner)),
        },
        _ => None,
    }
}

/// WHERE THE BOX OF THE COMMAND IN HAND STANDS BY ON THE SCREEN `rect`: the point the view comes nearer about while the
/// command is open.
pub fn cmd_spot(pc: &mut qymcad_ui_state::PartCtx, rect: Rect) -> Option<Pos2> {
    let place = cmd_place(pc)?;
    let basis = pc.cam.basis();
    let screen = qymcad_ui_state::Screen { cam: pc.cam, set: pc.set, rect, basis: &basis };
    Some(qymcad_ui_state::spot_of(place.by, pc.project, &screen, qymcad_ui_state::current_ctx_id(pc.active_path, pc.project)).at)
}

/// THE GATE OF THE PART COMMAND IN HAND: what it still waits for, and what the trial build of its values says. A
/// choice of a contour of a sweep or a loft in the flat half-sketcher waits for the click on the contour.
pub(crate) fn command_gate(pc: &mut qymcad_ui_state::PartCtx, ctx: &egui::Context) -> qymcad_ui_state::Gate {
    // A FORMULA THE DOCUMENT REFUSES - one coming back to the name of this very number - is said beside its field before
    // Enter and shuts the gate, asked of the door it is written through
    let refused = pc.cmd.params.iter().find_map(|p| field_refused_by_document(p, pc.project, pc.cmd.edit).map(|why| (p, why)));
    if let Some((p, why)) = refused {
        return qymcad_ui_state::Gate::Refused(qymcad_ui_state::Refusal { key: p.key.clone(), field: qymcad_ui_state::field_caption(p), why });
    }
    if pc.picking.contour().is_some() {
        return qymcad_ui_state::Gate::Waiting(pc.status.words.clone());
    }
    // EDGES OF A PATCH THAT DO NOT CLOSE are picked, and will not do: said as they are, not asked for again
    if pc.armed.part_tool() == Some(qymcad_ui_state::PartTool::Patch) && !pc.gsel.edges.is_empty() && !cmd_ready(pc) {
        return qymcad_ui_state::Gate::Wrong(qymcad_i18n::tr("msg-patch-needs-edges"));
    }
    let ask = (!cmd_ready(pc)).then(|| cmd_hint(&*pc.armed, &*pc.gsel, &*pc.trim));
    let trial = trial_refusal(pc, ctx);
    qymcad_ui_state::Gate::of(ask, &trial, &pc.cmd.params)
}

/// A STEP OF A GIZMO typed as a number: about or along axis `ax` (a turn when `rot`), by `by`, about the point `at`.
#[derive(Clone, Copy)]
pub(crate) struct GizmoStep {
    pub(crate) ax: u8,
    pub(crate) rot: bool,
    pub(crate) by: f64,
    pub(crate) at: [f64; 3],
}

/// THE BODY OR THE PART MOVED by a step typed at its gizmo: a parametric move of a body, or the part moved in the
/// assembly it stands in, its group going with it.
pub(crate) fn gizmo_move(pc: &mut qymcad_ui_state::PartCtx, on: qymcad_ui_state::GizmoOf, step: GizmoStep) {
    let t = if step.rot {
        qymcad_ui_state::rot_about_point(step.ax, step.by, step.at)
    } else {
        let mut t = qymcad_core::feature::PLACE_IDENTITY;
        t[[3, 7, 11][(step.ax as usize).min(2)]] = step.by;
        t
    };
    match on {
        qymcad_ui_state::GizmoOf::Body(mi) => apply_body_move(pc, mi, t),
        qymcad_ui_state::GizmoOf::Part(comp) => {
            qymcad_ui_state::begin_edit(pc.edits, pc.project, qymcad_i18n::tr("status-move-component"));
            let to = qymcad_core::feature::mat_mul12(&t, &pc.project.component_transform(comp));
            if !pc.project.move_component_by_hand(comp, to) {
                *pc.status = Notice::info(qymcad_i18n::tr("j-group-grounded"));
            }
            qymcad_ui_state::invalidate_placement(pc.regen);
            qymcad_ui_state::commit_edit(&mut pc.rebuild());
        }
    }
}
