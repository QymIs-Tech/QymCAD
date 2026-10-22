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
pub(crate) fn drop_assembly_tools(jc: &mut qymcad_ui_state::JointCtx) {
    // a joint just made and not yet kept goes with the tool, as a command cancelled
    qymcad_assembly::joint_create_finish(jc, false);
    qymcad_assembly::tangent_finish(jc, false);
    let joint = &mut *jc.joint;
    // THE EDIT OF A MATE GOES TOO, unless one of its own picks (axis, anchor re-pick) is what is being put down: its
    // popup stands over the canvas, and a tool taken after it had its clicks land on the popup's fields.
    // Reported behaviour: the corner of the third part lay under the first hinge's popup when the second was started.
    if joint.axis_pick.is_none() && joint.edit_repick.is_none() {
        joint.edit = None;
    }
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
/// TAKE THE "MIRROR A WHOLE PART" TOOL, in the order that works: release the others FIRST.
///
/// The button used to push the release and set the field straight after. The release is carried out
/// AFTER the frame, so it cleared the field again and the button did nothing - the same shape as the
/// ruler that switched the measuring tool on and off within one frame.
///
/// Beside the application rather than on it: nothing here needs to see the whole of it.
pub(crate) fn mirror_part_door(app: &mut App, comp: Option<qymcad_core::model::Id>) {
    // the button pressed again with the tool in hand puts it down, as every tool button does
    let held = app.params.mirror.in_hand();
    app.cancel_all_tools();
    if held {
        return;
    }
    // with no part picked before the button the tool is in hand all the same and waits for a click on one
    (app.params.mirror.part, app.params.mirror.waiting) = (comp, comp.is_none());
    if comp.is_some() {
        app.tools.gsel.before.spend_object();
    }
    app.status = qymcad_ui_state::Notice::info(crate::i18n::tr(if comp.is_some() { "tb-mirror-pick-plane" } else { "tb-mirror-pick-part" }));
}

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
            // the pick of an axis and the re-pick of an anchor are made in the edit of a joint, opened as its row opens it
            AssemblyTool::Axis => {
                let (jid, ca) = a_joint(app);
                crate::gui::enter_joint_edit(&mut app.side.joint, &mut app.chosen.sel, &mut app.status, jid);
                app.side.joint.axis_pick = Some(ca);
            }
            AssemblyTool::Repick => {
                let (jid, _) = a_joint(app);
                crate::gui::enter_joint_edit(&mut app.side.joint, &mut app.chosen.sel, &mut app.status, jid);
                app.side.joint.edit_repick = Some((jid, false));
            }
        }
    }
}
