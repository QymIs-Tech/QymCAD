//! A PERSON'S HAND: the only way for a test to touch the program.
//!
//! The tests of the application used to cheat — they reached straight into the fields
//! (`app.tools.gsel.edges.insert(...)`). Such a test checks what a person cannot do and skips what they do
//! every time: did the click land, what was under the cursor, which tool is open. Hence the class of
//! breakages that was caught by hand and not by the run.
//!
//! The hand can do exactly what a person can: press a tool button, click a point in the scene, press
//! Enter or Esc. It has nothing else — and that is its main property.
#[cfg(test)]
pub(super) struct Hand<'a> {
    pub app: &'a mut super::App,
    pub rect: egui::Rect,
}

#[cfg(test)]
impl<'a> Hand<'a> {
    /// Take the program in hand: a 900x700 frame, a 3D view, the camera fitted to the scene.
    pub fn new(app: &'a mut super::App) -> Self {
        app.viewing.mode_3d = true;
        app.viewing.cam.init = true;
        let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 700.0));
        Self { app, rect }
    }

    /// Aim the camera so that the whole body is visible — otherwise a click outside the frame means
    /// nothing.
    pub fn look_at(&mut self, target: [f64; 3], scale: f32) -> &mut Self {
        self.app.viewing.cam.target = target;
        self.app.viewing.cam.scale = scale;
        self
    }

    /// Press a tool button in the panel.
    pub fn tool(&mut self, kind: u8) -> &mut Self {
        self.app.start_feat_cmd(kind);
        // WAIT FOR THE LIVE B-rep TO BE PREPARED, the way a person waits for it. After a file is
        // opened the kernel comes up in the background, and until then the body has neither edges nor
        // faces: in the program a frame waits and shows an overlay, while a test without the wait would
        // decide the tool "cannot be clicked with".
        crate::gui::commands::refresh_edges(&mut self.app.part_ctx());
        self.app.drain_bg_for_test();
        qymcad_ui_state::rebuild_if_dirty(&mut self.app.rebuild_ctx());
        crate::gui::commands::refresh_edges(&mut self.app.part_ctx());
        self
    }

    /// CLICK A PLACE IN THE SCENE. The point is given in world coordinates — that way the test says
    /// "on this face" rather than "at these pixels"; the screen point is computed by the same
    /// projection the drawing uses.
    pub fn click(&mut self, world: [f64; 3]) -> &mut Self {
        let basis = self.app.viewing.cam.basis();
        let pos = qymcad_ui_state::Screen { cam: &self.app.viewing.cam, set: &self.app.set, rect: self.rect, basis: &basis }.at(world).0;
        crate::gui::commands::refresh_edges(&mut self.app.part_ctx()); // the same thing a frame does before accepting a click
        self.app.viewport_3d_click_at(pos, self.rect, &basis);
        self
    }

    /// Type a number into a field of the command — the same as typing it in the popup at the
    /// geometry.
    pub fn set(&mut self, key: &str, v: f64) -> &mut Self {
        if let Some(p) = self.app.tools.cmd.params.iter_mut().find(|p| p.key == key) {
            p.val = v;
            p.txt = format!("{v}");
        }
        self
    }

    /// TURN THE PART to see a face that is not visible from the current side.
    ///
    /// That is what a person does: cannot click the bottom, so they turn the model. Without this part
    /// of the scenarios were out of reach of the checks, and a limitation of the checks was taken for a
    /// breakage of the program: the bottom face "could not be picked" simply because it was behind the
    /// part.
    pub fn orbit(&mut self, yaw: f64, pitch: f64) -> &mut Self {
        self.app.viewing.cam.yaw = yaw;
        self.app.viewing.cam.pitch = pitch;
        self
    }

    /// LOOK FROM BELOW — the commonest turn: there is no other way to get at the bottom of a part.
    pub fn look_from_below(&mut self) -> &mut Self {
        self.orbit(-0.7, -0.9)
    }

    /// TAKE A SKETCHER TOOL (the same panel button): 1 line, 2 rectangle, 3 circle, 4 arc, 5 point,
    /// 6 polygon.
    pub fn sk_tool(&mut self, t: u8) -> &mut Self {
        self.app.viewing.mode_3d = false;
        self.app.viewing.view.scale = 6.0;
        self.app.viewing.view.center = super::Vec2::new(0.0, 0.0);
        self.app.viewing.view.initialized = true;
        self.app.set_sk_tool(t);
        // THE SKETCH BEING EDITED IS THE SELECTION, as it is on entering an edit: part of the handling
        // (finishing a spline, dragging a point) simply does not fire without it.
        if let Some(si) = qymcad_ui_state::edit_si(&self.app.project, &self.app.sketch_ses) {
            self.app.chosen.sel = super::Sel::Sketch(si);
        }
        self
    }

    /// CLICK THE SKETCH CANVAS in its own coordinates — that way the test says "right here" rather
    /// than "at these pixels".
    pub fn click2d(&mut self, x: f64, y: f64) -> &mut Self {
        let pos = (qymcad_ui_state::Sheet { view: self.app.viewing.view, rect: self.rect }).at(qymcad_core::geom::Point2::new(x, y));
        let ctx = egui::Context::default();
        crate::gui::sketching::sketch_click_at(&mut self.app.sketch_ctx(), &ctx, pos, self.rect);
        self
    }

    /// SELECTION MODE in a sketch — the same as putting down the drawing tool.
    pub fn sk_select(&mut self) -> &mut Self {
        qymcad_ui_state::exit_draw_tools(&mut qymcad_ui_state::tools_of!(self.app));
        // THE SKETCH BEING EDITED IS THE SELECTION. The drag handling pulls a point only of the
        // SELECTED sketch; in the program that state is set by entering the edit, and the hand must be
        // in the same state, otherwise it pulls a canvas belonging to nobody.
        if let Some(si) = qymcad_ui_state::edit_si(&self.app.project, &self.app.sketch_ses) {
            self.app.chosen.sel = super::Sel::Sketch(si);
        }
        self
    }

    /// PRESS A CONSTRAINT BUTTON: 0 coincidence, 1 horizontal, 2 vertical, 3 parallel,
    /// 4 perpendicular, 5 equal, 6 fix, 7 collinear, 8 concentric, 9 tangent, 10 symmetry,
    /// 11 midpoint.
    pub fn constraint(&mut self, code: u8) -> &mut Self {
        crate::gui::sketching::constraint_button(&mut self.app.sketch_ctx(), code);
        self
    }

    /// DRAG A POINT with the mouse from one place of the canvas to another.
    ///
    /// THE SAME handling is called as in a real drag: take the point under the cursor, lead it along
    /// the path, release. The `egui` event is not faked — a fake would be checking itself.
    pub fn drag2d(&mut self, from: (f64, f64), to: (f64, f64)) -> &mut Self {
        let sh = qymcad_ui_state::Sheet { view: self.app.viewing.view, rect: self.rect };
        let Some(si) = qymcad_ui_state::edit_si(&self.app.project, &self.app.sketch_ses) else { return self };
        let a = sh.at(qymcad_core::geom::Point2::new(from.0, from.1));
        let skip = std::collections::HashSet::new();
        if !crate::gui::sketching::begin_point_drag(&mut self.app.tools.drag, &self.app.project, &self.app.set, qymcad_ui_state::Sheet { view: self.app.viewing.view, rect: self.rect }, si, a, &skip) {
            return self; // there is nothing to drag under the cursor — let the check show that
        }
        let Some((si, pi)) = self.app.tools.drag.pt() else { return self };
        for k in 1..=4 {
            let t = k as f64 / 4.0;
            let p = qymcad_core::geom::Point2::new(from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t);
            let sp = sh.at(p);
            crate::gui::sketching::drag_point_to(&mut self.app.sketch_ctx(), si, pi, self.rect, sp);
        }
        crate::gui::sketching::finish_point_drag(&mut self.app.sketch_ctx());
        self
    }

    /// A DOUBLE CLICK IN A SKETCH means "the shape is finished" (a spline, a chain of lines).
    pub fn finish_shape(&mut self) -> &mut Self {
        if self.app.tools.armed.draw_kind() == 9 {
            crate::gui::sketching::finish_spline(&mut self.app.project, &mut self.app.regen, self.app.chosen.sel, &mut self.app.tools.tool, &mut self.app.viewing.view);
        } else {
            self.app.tools.tool.pts.clear();
        }
        self
    }

    /// WHAT THE OPERATION DOES TO THE BODY: 0 add, 2 cut — the same as the switch in the top bar of
    /// the extrude.
    pub fn op(&mut self, op: u8) -> &mut Self {
        self.app.feat.op = op;
        self
    }

    /// TAKE THE JOINT TOOL AND CHOOSE THE KIND — the same door the workbench button and the `J` key
    /// use.
    ///
    /// The hand had NO assembly actions at all, and that cost dearly: the mates workbench was never
    /// once touched the way a person touches it. The checks called `add_joint` directly and therefore
    /// saw neither that a click on a part was declared a miss nor that an anchor on an edge was born
    /// dead — both were found by hand in five minutes.
    pub fn mate(&mut self, kind: qymcad_core::feature::JointKind) -> &mut Self {
        self.app.workbench = super::Workbench::Assembly;
        self.app.viewing.mode_3d = true;
        self.app.side.joint.new_kind = kind;
        self.app.arm_joint_pick_for_test();
        crate::gui::commands::refresh_edges(&mut self.app.part_ctx());
        self
    }

    /// THE ANCHOR MODE — the switch in the assembling bar: 0 face, 1 edge, 2 vertex, 3 origin of the
    /// part.
    ///
    /// Taken AFTER the tool: taking the tool sets the mode by the kind of joint (an edge for the
    /// coaxial ones, a face for the rest), and a person's choice must lie on top.
    pub fn anchor(&mut self, mode: u8) -> &mut Self {
        qymcad_assembly::set_joint_anchor_mode_for_test(&mut self.app.joint_ctx(), mode);
        self
    }

    /// Enter applies the command.
    pub fn enter(&mut self) -> &mut Self {
        self.app.apply_feat_cmd();
        qymcad_ui_state::rebuild_if_dirty(&mut self.app.rebuild_ctx());
        self
    }

}

#[cfg(test)]
mod tests {
    use super::Hand;
    use super::super::App;

    /// THE HAND REALLY WORKS AS A HAND: tool button -> click on an edge -> Enter -> a fillet in the
    /// timeline.
    ///
    /// Not one reach into a field: had the click missed, or had the click handling not understood that
    /// the edge tool was open, no node would have appeared — and the test would show that, as a person
    /// would.
    #[test]
    fn a_fillet_is_made_by_clicking_like_a_person() {
        let mut app = App::default();
        super::super::joint_flow::tests::add_part_at(&mut app, 0.0);
        qymcad_ui_state::rebuild_if_dirty(&mut app.rebuild_ctx());
        let body = app.project.mesh_id(0).expect("the body");
        if let Some(owner) = app.project.body_owner(body) {
            app.enter_component(owner);
        }
        let edge = app.project.regen_edges[&body]
            .iter()
            .filter(|e| (e.a[2] - e.b[2]).abs() < 1e-6)
            .max_by(|x, y| x.mid[2].total_cmp(&y.mid[2]))
            .cloned()
            .expect("the top edge");

        let mut hand = Hand::new(&mut app);
        hand.look_at([10.0, 10.0, 5.0], 9.0).tool(4).click(edge.mid).enter();

        let made = app.project.timeline.iter().any(|n| matches!(n.kind, qymcad_core::feature::FeatureKind::Fillet { .. }));
        assert!(made, "a click on an edge and Enter must create a fillet; status: {}", app.status);
        assert!(app.project.regen_errors.is_empty(), "and it must build: {:?}", app.project.regen_errors);
    }
}
