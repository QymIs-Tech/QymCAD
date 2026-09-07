//! JOINTS - picking them, solving them, their glyphs and their panel.

pub(crate) use qymcad_assembly::*;
use super::*;


impl App {










    /// The DOF gizmo handle under the cursor: (slot, whether it is a ring). Arrows take priority over rings,
    /// as in the six-degree gizmo.
    pub(super) fn joint_handle_hit(&mut self, jid: Id, rect: Rect, basis: &([f64; 3], [f64; 3], [f64; 3]), pp: Pos2) -> Option<(u8, bool)> {
        joint_handle_hit(&mut self.joint_ctx(), jid, rect, basis, pp)
    }
































    /// The one doorway into the assembly workbench: built in a single place so the borrows stay disjoint.
    pub(crate) fn joint_ctx(&mut self) -> JointCtx<'_> {
        JointCtx {
            view: &mut self.viewing.view,
            sel: &mut self.chosen.sel,
            joint: &mut self.side.joint,
            joint_anim: &mut self.joint_anim,
            comp_giz: &self.dragged.comp_giz,
            project: &mut self.project,
            status: &mut self.status,
            edits: &mut self.disk.edits,
            sel_conn: &mut self.chosen.sel_conn,
            live: &mut self.live,
            part_pull: &mut self.dragged.part_pull,
            regen: &mut self.regen,
            params_seen: &mut self.params_seen,
            active_path: &self.active_path,
            scheme: &self.scheme,
            cam: &self.viewing.cam,
            set: &self.set,
            workbench: self.workbench,
            mode_3d: self.viewing.mode_3d,
        }
    }

























    /// TOGGLE THE GROUNDING TOOL. Through one door, like joint picking: it now has two ways in - the panel
    /// button and the command search.
    pub(crate) fn start_ground_pick(&mut self) {
        // THE FLAG IS READ BEFORE THE CLEARING: `cancel_all_tools` wipes the very field it asks about.
        let on = !self.side.joint.ground_pick;
        self.cancel_all_tools(); // mutually exclusive with the create tools and the other joint tools
        start_ground_pick_armed(&mut self.joint_ctx(), on);
    }

    /// TOGGLE THE GROUP TOOL.
    ///
    /// A group fastens a set of parts TO ONE ANOTHER where they stand. It picks no anchors, so it is not
    /// gathered the way a joint is: click as many parts as needed and confirm.
    pub(crate) fn start_group_pick(&mut self) {
        // THE FLAG IS READ BEFORE THE CLEARING: `cancel_all_tools` wipes the very field it asks about.
        let on = self.side.joint.group_pick.is_none();
        self.cancel_all_tools(); // mutually exclusive with the other tools
        start_group_pick_armed(&mut self.joint_ctx(), on);
    }



    /// TOGGLE THE TANGENCY TOOL.
    ///
    /// Tangency needs no connectors - two surfaces are enough, so there is nothing to confirm: the second
    /// pick sets the condition straight away.
    pub(crate) fn start_tangent_pick(&mut self) {
        // THE FLAG IS READ BEFORE THE CLEARING: `cancel_all_tools` wipes the very field it asks about.
        let on = self.side.joint.tangent_pick.is_none();
        self.cancel_all_tools(); // mutually exclusive with the other tools
        start_tangent_pick_armed(&mut self.joint_ctx(), on);
    }


    /// TOGGLE THE WIDTH TOOL.
    ///
    /// Width puts a part MIDWAY between two walls. Three faces have to be shown: the two walls and the
    /// piece between them; the order matters.
    pub(crate) fn start_width_pick(&mut self) {
        // THE FLAG IS READ BEFORE THE CLEARING: `cancel_all_tools` wipes the very field it asks about.
        let on = self.side.joint.width_pick.is_none();
        self.cancel_all_tools(); // mutually exclusive with the other tools
        start_width_pick_armed(&mut self.joint_ctx(), on);
    }



    /// TOGGLE THE ANCHOR TOOL - creating a standalone connector.
    pub(crate) fn start_conn_pick(&mut self) {
        // THE FLAG IS READ BEFORE THE CLEARING: `cancel_all_tools` wipes the very field it asks about.
        let on = !self.side.joint.conn_pick;
        self.cancel_all_tools(); // mutually exclusive with the other tools
        start_conn_pick_armed(&mut self.joint_ctx(), on);
    }


    /// TOGGLE THE RELATION TOOL.
    ///
    /// A relation ties together the degrees of freedom of joints that already exist, so what is picked in
    /// it is THE JOINTS THEMSELVES, by clicking a row in the list of joints. There is no geometry to choose
    /// here: neither a face nor an edge concerns a relation.
    pub(crate) fn start_relation_pick(&mut self) {
        // THE FLAG IS READ BEFORE THE CLEARING: `cancel_all_tools` wipes the very field it asks about.
        let on = self.side.joint.relation_pick.is_none();
        self.cancel_all_tools(); // mutually exclusive with the other tools
        start_relation_pick_armed(&mut self.joint_ctx(), on);
    }





    /// LAUNCH A COMMAND BY ITS CATALOGUE CODE.
    ///
    /// The only entry point for the search - and it leads to EXACTLY the same calls the panel button makes.
    /// A second launch path would be worse than having no search at all: it would start doing what the
    /// button does not, and the divergence would surface for whoever uses the program rather than in a test.
    pub(crate) fn run_command(&mut self, code: &str) {
        use crate::command_catalog::Launch;
        let Some(cmd) = crate::command_catalog::by_code(code) else { return };
        match cmd.launch {
            Launch::Feat(n) => self.start_feat_cmd(n),
            Launch::Prim(n) => self.start_prim_cmd(n),
            Launch::SkTool(n) => self.set_sk_tool(n),
            Launch::Dim(n) => qymcad_ui_state::set_dim_tool(&mut qymcad_ui_state::tools_of!(self), &mut self.viewing.mode_3d, &self.project, self.chosen.sel, self.sketch_ses, &mut self.status, n),
            Launch::ClickOp(n) => qymcad_ui_state::set_click_op(&mut qymcad_ui_state::tools_of!(self), &mut self.viewing.mode_3d, n),
            Launch::Modify(n) => qymcad_ui_state::modify_button(qymcad_ui_state::editing_of!(self), &mut qymcad_ui_state::tools_of!(self), self.sk_pat, &self.tool_prefs, n),
            Launch::Action("joint") => self.start_joint_pick(),
            Launch::Action("ground") => self.start_ground_pick(),
            Launch::Action(_) => {}
        }
    }


    /// START PICKING A JOINT. One door for every way in: the workbench button, the button in the
    /// properties, the `J` key. The body of this launch used to be written straight into the panel button,
    /// and a second way in would have had to copy it - and a copy falls behind sooner or later.
    pub(crate) fn start_joint_pick(&mut self) {
        self.cancel_all_tools(); // mutually exclusive with the other tools
        self.side.joint.pick_faces = true;
        self.side.joint.ground_pick = false;
        self.side.joint.pick_first = None;
        // THE SORT OF ANCHOR IS INFERRED UNDER THE CURSOR. There used to be an "anchor chosen by kind"
        // here: rotation got an edge, everything else a face. The guess was half right (rotation also
        // happens about a cylindrical face) and cost dearly - aiming at a face produced an edge.
        self.side.joint.anchor_mode = 0;
        let what = crate::i18n::tr("j-place-lower");
        self.status = crate::i18n::tr2("jt-pick-a-then-b", "kind", &crate::i18n::tr(self.side.joint.new_kind.label()), "what", &what);
    }



    /// THE TEST FACADES. A test must walk the same path a person does - through the command and its picks
    /// - rather than poking at fields directly, or it only ever checks an invention of its own.
    #[cfg(test)]
    /// Take up the joint tool through the same door the button or the `J` key uses.
    #[cfg(test)]
    pub(crate) fn arm_joint_pick_for_test(&mut self) {
        if !self.side.joint.pick_faces {
            let mode = self.side.joint.anchor_mode;
            self.start_joint_pick();
            self.side.joint.anchor_mode = mode; // the check picks the anchor mode itself
        }
    }

































    /// GRABBING THE PART ITSELF: take a part in the frame and pull, and it moves along the degrees of
    /// freedom it has left.
    ///
    /// The degree handles existed before, but pulling was possible ONLY by a gizmo arrow: miss it, and the
    /// mechanism does not stir. A professional CAD lets a part be grabbed anywhere. The difference is not
    /// convenience: while a thin arrow must be hit, the mechanism is never handled, and so it is never
    /// noticed that it was assembled wrongly.
    ///
    /// WHICH DEGREE IS BEING DRIVEN is decided by THE DIRECTION OF THE GRAB: the one whose axis on screen is
    /// closest to the motion of the cursor. A joint with one freedom leaves no choice and no question.
    pub(crate) fn joint_grab_part_at(&mut self, rect: Rect, from: Pos2, towards: egui::Vec2, basis: &([f64; 3], [f64; 3], [f64; 3])) -> bool {
        if self.side.joint.giz_drag.is_some() {
            return false; // a handle is already being held
        }
        // THE PICK NEEDS THE WHOLE APPLICATION and happens once, at the top: it is handed in rather than
        // reached for, and everything after it is the assembly's own work.
        let Some((body, _)) = self.pick_part_face_at(rect, from) else { return false };
        joint_grab_part(&mut self.joint_ctx(), body, rect, towards, basis)
    }




















}
