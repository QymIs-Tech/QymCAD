//! ONE LIST OF THE ASSEMBLY TOOLS — AND EVERYBODY READS IT.
//!
//! WHY THIS EXISTS. In one sitting three troubles in a row turned out to be ONE illness: somewhere in
//! the code the modes are listed by hand, and the new tool was forgotten there.
//!
//! * the highlight stayed silent while pointing at a secondary axis — the mode was not named in the
//!   drawing condition;
//! * the highlight stayed silent for FOUR more (connector, group, width, tangent) — the same list;
//! * Esc did not release FIVE tools — a list of its own in `on_escape` that knew two out of nine.
//!
//! Fixing each list separately is pointless: the next tool will be forgotten in the next one. So
//! there is ONE list, and the compiler watches over its completeness: the `match` on `AssemblyTool`
//! is exhaustive, and a new kind simply will not build until it has been said whether it is taken up
//! and how it is put down.
pub(crate) use qymcad_ui_state::{AssemblyTool};
pub(crate) use qymcad_ui_state::{armed_assembly_tools};
use super::App;

impl App {
    /// WHICH ASSEMBLY TOOLS ARE TAKEN UP RIGHT NOW.
    pub(crate) fn armed_assembly_tools(&self) -> Vec<AssemblyTool> {
        armed_assembly_tools(&self.painting())
    }





}

/// RELEASE EVERY ASSEMBLY TOOL. Esc looks here too.
///
/// An unfinished selection goes with the tool: half of what was pointed at is not a document but
/// an intention, and it must not survive a cancellation.
pub(crate) fn drop_assembly_tools(joint: &mut super::JointCommand) {
    for t in AssemblyTool::ALL {
        match t {
            AssemblyTool::Mate => {
                joint.pick_faces = false;
                joint.pick_first = None;
            }
            AssemblyTool::Anchor => joint.conn_pick = false,
            AssemblyTool::Group => joint.group_pick = None,
            AssemblyTool::Width => joint.width_pick = None,
            AssemblyTool::Tangent => joint.tangent_pick = None,
            AssemblyTool::Relation => joint.relation_pick = None,
            AssemblyTool::Ground => joint.ground_pick = false,
            AssemblyTool::Axis => joint.axis_pick = None,
            AssemblyTool::Repick => joint.edit_repick = None,
        }
    }
}

/// TAKING A TOOL THE WAY A PERSON DOES, for every kind there is.
///
/// SHARED BECAUSE THERE IS A SECOND CALLER. Two checks need it - "F1 answers about the tool in hand" and
/// "taking one tool releases the previous" - and a door table written twice falls behind twice. It walks
/// `AssemblyTool::ALL`, so a tool added later cannot slip past either of them.
///
/// The axis pick and the re-pick have no door on the toolbar: they are switched on by a button IN THE POPUP
/// of an existing joint, and outside that popup they cannot be taken. So a joint is built first and the same
/// fields the popup sets are used.
#[cfg(test)]
pub(crate) mod doors {
    use super::super::App;
    use super::AssemblyTool;
    use qymcad_core::model::Id;

    /// A JOINT WITH SOMETHING TO EDIT: the axis pick and the anchor re-pick exist only alongside one.
    pub(crate) fn a_joint(app: &mut App) -> (Id, Id) {
        let ([ja, _jb], _wheels) = crate::gui::a_relation_is_made_by_hand::tests::two_hinges(app);
        let ca = app.project.joints.iter().find(|j| j.id == ja).map(|j| j.a).expect("the joint has an anchor");
        (ja, ca)
    }

    /// Take a tool through its own door.
    pub(crate) fn arm(app: &mut App, t: AssemblyTool) {
        match t {
            AssemblyTool::Mate => app.arm_joint_pick_for_test(),
            AssemblyTool::Anchor => app.start_conn_pick(),
            AssemblyTool::Group => app.start_group_pick(),
            AssemblyTool::Width => app.start_width_pick(),
            AssemblyTool::Tangent => app.start_tangent_pick(),
            AssemblyTool::Relation => app.start_relation_pick(),
            AssemblyTool::Ground => app.start_ground_pick(),
            AssemblyTool::Axis => {
                let (_, ca) = a_joint(app);
                app.side.joint.axis_pick = Some(ca);
            }
            AssemblyTool::Repick => {
                let (jid, _) = a_joint(app);
                app.side.joint.edit_repick = Some((jid, false));
            }
        }
    }
}
