//! THE DOORS OF THE TOOLS OF A PART: what Apply, Cancel and a step back do for each, over the narrow context it works in.
//! The frame goes through them (`qymcad_ui_state::go_through`) and says a refusal to apply itself.

use qymcad_ui_state::{Doors, NotApplied, Notice, Stepped};
use super::*;

/// THE DOORS OF A FEATURE COMMAND.
pub(crate) struct FeatDoors<'a, 'b> {
    pub(crate) pc: &'a mut qymcad_ui_state::PartCtx<'b>,
}

impl Doors for FeatDoors<'_, '_> {
    /// Enter of an extrusion or a revolution in the flat choice of its contours, the contours picked, takes the command
    /// into 3D to set its size there - the gizmo or the field; the next Enter applies it.
    fn apply(&mut self) -> Result<(), NotApplied> {
        let pc = &mut *self.pc;
        if matches!(pc.armed.cmd_kind(), 1 | 3) && !*pc.mode_3d && !pc.gsel.profiles.is_empty() {
            *pc.mode_3d = true;
            *pc.status = Notice::info(qymcad_i18n::tr("in-drag-or-type"));
            return Ok(());
        }
        apply_feat_cmd(pc)
    }

    fn cancel(&mut self) {
        qymcad_ui_state::cancel_feat_cmd(self.pc);
    }

    /// The choice of a contour of a sweep or a loft is let go first, returning to 3D; then a reference being picked -
    /// an axis, a line, the reference of a chamfer, the neutral face of a draft; then the command.
    fn back(&mut self) -> Stepped {
        let pc = &mut *self.pc;
        if pc.picking.contour().is_some() {
            pc.picking.clear();
            qymcad_ui_state::return_view(pc); // cancelling an action does not touch the camera
            *pc.status = Notice::info(qymcad_i18n::tr("in-contour-cancelled"));
            return Stepped::Back;
        }
        if pc.rev.pick_line {
            qymcad_ui_state::return_view(pc);
        }
        let picking = [&mut pc.arr.axis_pick, &mut pc.rev.pick_axis, &mut pc.rev.pick_line, &mut pc.chamfer.pick_ref, &mut pc.draft.pick_neutral];
        if !picking.iter().any(|p| **p) {
            return Stepped::NothingLeft;
        }
        for p in picking {
            *p = false;
        }
        *pc.status = Notice::info(qymcad_i18n::tr("in-axis-pick-cancelled"));
        Stepped::Back
    }
}

/// THE DOORS OF A BOOLEAN BEING MADE: body A picked, B and the kind awaited.
pub(crate) struct BoolPickDoors<'a, 'b> {
    pub(crate) pc: &'a mut qymcad_ui_state::PartCtx<'b>,
}

impl Doors for BoolPickDoors<'_, '_> {
    fn apply(&mut self) -> Result<(), NotApplied> {
        apply_boolean(self.pc)
    }

    fn cancel(&mut self) {
        (self.pc.boolean.pick, self.pc.boolean.b) = (None, None);
        *self.pc.status = Notice::info(qymcad_i18n::tr("in-bool-cancelled"));
    }

    fn back(&mut self) -> Stepped {
        Stepped::NothingLeft
    }
}

/// THE DOORS OF A BOOLEAN REOPENED: its kind is changed in the bar as it is chosen; Cancel puts back the kind it had.
pub(crate) struct BoolEditDoors<'a, 'b> {
    pub(crate) pc: &'a mut qymcad_ui_state::PartCtx<'b>,
}

impl Doors for BoolEditDoors<'_, '_> {
    fn apply(&mut self) -> Result<(), NotApplied> {
        self.pc.boolean.edit = None;
        *self.pc.status = Notice::info(qymcad_i18n::tr("in-bool-done"));
        Ok(())
    }

    fn cancel(&mut self) {
        bool_edit_cancel(self.pc);
    }

    fn back(&mut self) -> Stepped {
        Stepped::NothingLeft
    }
}

/// THE DOORS OF A PATTERN OF COMPONENTS.
pub(crate) struct CompArrayDoors<'a, 'b> {
    pub(crate) pc: &'a mut qymcad_ui_state::PartCtx<'b>,
}

impl Doors for CompArrayDoors<'_, '_> {
    fn apply(&mut self) -> Result<(), NotApplied> {
        apply_comp_array(self.pc)
    }

    fn cancel(&mut self) {
        *self.pc.carr = qymcad_ui_state::CompArrayCmd::default();
        self.pc.cmd.params.clear();
        *self.pc.status = Notice::info(qymcad_i18n::tr("msg-comp-array-cancelled"));
    }

    /// An axis being picked is let go first.
    fn back(&mut self) -> Stepped {
        if !self.pc.arr.axis_pick {
            return Stepped::NothingLeft;
        }
        self.pc.arr.axis_pick = false;
        *self.pc.status = Notice::info(qymcad_i18n::tr("in-axis-pick-cancelled"));
        Stepped::Back
    }
}

/// THE DOORS OF THE TOOLS OF THE PLACE OF THE SECTION: the measure, the wait for a sketch's plane, the pencil, the
/// mirror copy of a part, the section.
pub(crate) struct PlaneDoors<'a, 'b> {
    pub(crate) pb: &'a mut PlaneBarCtx<'b>,
    pub(crate) tool: PlaneTool,
}

impl Doors for PlaneDoors<'_, '_> {
    fn apply(&mut self) -> Result<(), NotApplied> {
        let pb = &mut *self.pb;
        match self.tool {
            PlaneTool::MirrorPart => mirror_part_apply(pb.mirror, pb.project, pb.edits, pb.regen, pb.active_path, pb.status),
            PlaneTool::Section => {
                pb.section.pick = false;
                *pb.status = Notice::info(qymcad_i18n::tr("vp-section-on"));
                Ok(())
            }
            // what is pointed at, not applied
            PlaneTool::Measure | PlaneTool::SketchFor(_) | PlaneTool::Pencil => Ok(()),
        }
    }

    fn cancel(&mut self) {
        let pb = &mut *self.pb;
        match self.tool {
            PlaneTool::Measure => {
                pb.m3.clear();
                *pb.status = Notice::info(qymcad_i18n::tr("in-measure-off"));
            }
            PlaneTool::SketchFor(_) | PlaneTool::Pencil => {
                *pb.status = Notice::info(pb.picking.cancel_key().map(qymcad_i18n::tr).unwrap_or_default());
                pb.picking.clear();
            }
            PlaneTool::MirrorPart => pb.mirror.drop_part(),
            // the status line said "the section is on" after it was switched off here: it says so now
            PlaneTool::Section => {
                if pb.section.pick {
                    section_cancel(pb.section, pb.regen);
                } else {
                    pb.section.plane = None;
                    qymcad_ui_state::invalidate(pb.regen);
                }
                *pb.status = Notice::info(qymcad_i18n::tr("tb-section-off"));
            }
        }
    }

    /// The measure lets go of what was clicked first, and only with nothing clicked leaves: things are usually measured
    /// several times in a row, and a miss does not throw one out of measuring. A section once on is no tool in hand:
    /// Esc there goes past it, to what Esc does with nothing in hand, and leaves the section on.
    fn back(&mut self) -> Stepped {
        let pb = &mut *self.pb;
        match self.tool {
            PlaneTool::Measure if !pb.m3.picks.is_empty() => {
                pb.m3.picks.clear();
                *pb.status = Notice::info(qymcad_i18n::tr("in-measure-hint"));
                Stepped::Back
            }
            PlaneTool::Section if !pb.section.pick => Stepped::Back,
            _ => Stepped::NothingLeft,
        }
    }
}

/// THE DOORS OF THE FIELD AT A GIZMO: the step typed is made, or the field goes.
pub(crate) struct GizmoNumDoors<'a, 'b> {
    pub(crate) pc: &'a mut qymcad_ui_state::PartCtx<'b>,
    pub(crate) on: qymcad_ui_state::GizmoOf,
    pub(crate) step: GizmoStep,
}

impl Doors for GizmoNumDoors<'_, '_> {
    fn apply(&mut self) -> Result<(), NotApplied> {
        gizmo_move(self.pc, self.on, self.step);
        self.pc.body_giz.num = None;
        Ok(())
    }

    fn cancel(&mut self) {
        self.pc.body_giz.num = None;
    }

    fn back(&mut self) -> Stepped {
        Stepped::NothingLeft
    }
}
