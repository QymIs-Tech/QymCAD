//! THE DOORS OF THE TOOLS OF AN ASSEMBLY: what Apply, Cancel and a step back do for each, over the context of the joints.
//! The frame goes through them (`qymcad_ui_state::go_through`) and says a refusal to apply itself.

use qymcad_ui_state::{Doors, NotApplied, Notice, Stepped};
use super::*;

/// THE DOORS OF A JOINT MADE OR REOPENED: its bar and its box alike.
pub(crate) struct JointEditDoors<'a, 'b> {
    pub(crate) jc: &'a mut qymcad_ui_state::JointCtx<'b>,
}

impl Doors for JointEditDoors<'_, '_> {
    fn apply(&mut self) -> Result<(), NotApplied> {
        joint_edit_keep(self.jc);
        Ok(())
    }

    fn cancel(&mut self) {
        joint_edit_leave(self.jc);
    }

    /// A pick being made is let go first - the axis of an anchor, an anchor picked anew - and the editing stays.
    fn back(&mut self) -> Stepped {
        let jc = &mut *self.jc;
        if jc.joint.axis_pick.take().is_some() {
            *jc.status = Notice::info(qymcad_i18n::tr("in-axis-pick-cancelled"));
            return Stepped::Back;
        }
        if jc.joint.edit_repick.take().is_some() {
            *jc.status = Notice::info(qymcad_i18n::tr("in-anchor-swap-cancelled"));
            return Stepped::Back;
        }
        Stepped::NothingLeft
    }
}

/// THE DOORS OF THE ASSEMBLY TOOL IN HAND.
pub(crate) struct AsmToolDoors<'a, 'b> {
    pub(crate) jc: &'a mut qymcad_ui_state::JointCtx<'b>,
    pub(crate) tool: AsmTool,
}

impl Doors for AsmToolDoors<'_, '_> {
    /// The tangent, the width, the group and the relation are made of what was picked; grounding, an anchor and a mate
    /// take their clicks as they come, and Enter has nothing more to make.
    fn apply(&mut self) -> Result<(), NotApplied> {
        let jc = &mut *self.jc;
        match self.tool {
            AsmTool::Tangent => {
                if !jc.joint.tangent_pick.as_ref().is_some_and(|s| s.len() >= 2) {
                    return Err(NotApplied::Refused(qymcad_i18n::tr("j-tangent-pick")));
                }
                tangent_finish(jc, true);
                Ok(())
            }
            AsmTool::Width => width_pick_confirm(jc),
            AsmTool::Group => group_pick_confirm(jc),
            AsmTool::Relation => relation_pick_confirm(jc),
            AsmTool::Ground | AsmTool::Anchor | AsmTool::Joint => Ok(()),
        }
    }

    /// The tool is put down; a tangent already made of two picks is taken away with it.
    fn cancel(&mut self) {
        let jc = &mut *self.jc;
        let off = match self.tool {
            AsmTool::Tangent if jc.joint.tangent_pick.as_ref().is_some_and(|s| s.len() >= 2) => {
                tangent_finish(jc, false);
                return;
            }
            AsmTool::Tangent => {
                jc.joint.tangent_pick = None;
                "j-tangent-off"
            }
            AsmTool::Width => {
                jc.joint.width_pick = None;
                "j-width-off"
            }
            AsmTool::Group => {
                jc.joint.group_pick = None;
                "j-group-off"
            }
            AsmTool::Ground => {
                jc.joint.ground_pick = false;
                "j-ground-off"
            }
            AsmTool::Anchor => {
                jc.joint.conn_pick = false;
                "j-conn-off"
            }
            AsmTool::Relation => {
                jc.joint.relation_pick = None;
                "j-relation-off"
            }
            AsmTool::Joint => {
                (jc.joint.pick_faces, jc.joint.pick_first) = (false, None);
                "j-cancelled"
            }
        };
        *jc.status = Notice::info(qymcad_i18n::tr(off));
    }

    fn back(&mut self) -> Stepped {
        Stepped::NothingLeft
    }
}
