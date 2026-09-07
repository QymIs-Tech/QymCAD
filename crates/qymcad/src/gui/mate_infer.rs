//! THE ANCHOR IS INFERRED UNDER THE CURSOR.
//!
//! A person USED TO BE OBLIGED TO SAY IN ADVANCE what they were about to point at: a face, an edge or
//! a vertex — with a switch in the tool bar. Grown-up CAD has no such switch at all: the cursor is
//! hovered and the anchor point is derived from the geometry itself. The difference is not cosmetic:
//! while the kind of anchor was chosen in advance, a click regularly gave an anchor other than the one
//! meant, and the joint "did not work" even when the solver was computing flawlessly.
//!
//! THE RULE OF INFERENCE: THE NEAREST SNAP POINT WINS rather than the most definite kind of geometry.
//!
//! The first edition went by seniority: the vertex first, then the edge, then the face. A measurement
//! refuted it: on a part that is small on screen (a machine scene — a part of 20 mm at a distance of
//! 800) the vertex falls into the grab EARLIER than the centre of a face, and aiming at the middle of
//! a face produced a corner. An order of seniority is the same guessing on a person's behalf that all
//! of this moves away from.
//!
//! So SNAP POINTS are collected — the centre of the face under the cursor, the midpoints of edges (on
//! a circular edge that is THE CENTRE OF THE HOLE, `Edge::axis_ref`), the ends of edges — and the one
//! nearest to the cursor is taken if it lies within the point grab threshold. If none is hit, the
//! ordinary pointing works: the edge under the cursor, otherwise the face under the cursor. The
//! thresholds are the ones adopted across the program, `Grab::Point` and `Grab::Curve`; no numbers of
//! its own are started here.
//!
//! Hence "a bolt into a hole" as well: click on an edge and you hit either the centre of the hole (if
//! close to it) or the circular edge itself, which the kernel resolves to the centre with the axis of
//! the circle anyway.
pub(crate) use qymcad_pick::{infer_axis_anchor, infer_mate_anchor};
use super::App;
use egui::{Pos2, Rect};
use qymcad_core::feature::AnchorRef;
use qymcad_core::model::Id;

impl App {
    /// THE DIRECTION UNDER THE CURSOR — for "point at the axis", the second pick.
    ///
    /// The rule is the same as for the anchor: what is pointed at is what is UNDER THE CURSOR. There
    /// used to be an order of its own here — `pick_edge_any` over the whole frame first, then the face
    /// — and it took the edge of ANY part as long as it came out closer to the cursor on screen.
    /// Reported behaviour: pointing at a rail guide that runs along the horizon drew the axis along Z.
    /// A measurement: the cursor was over body 18 while the edge belonged to body 6, a neighbour.
    ///
    /// An edge is preferred to a face: when showing an axis, a person aims at something extended.
    pub(super) fn infer_axis_anchor(&self, rect: Rect, pos: Pos2) -> Option<(Id, AnchorRef)> {
        infer_axis_anchor(&self.painting(), rect, pos)
    }

    /// THE ANCHOR UNDER THE CURSOR: the body and what was caught on it.
    ///
    /// `None` means there is no part under the cursor; only the caller has the right to call that a
    /// miss, and the caller is also the one who says so in words.
    pub(super) fn infer_mate_anchor(&self, rect: Rect, pos: Pos2) -> Option<(Id, AnchorRef)> {
        infer_mate_anchor(&self.painting(), rect, pos)
    }



    /// A click on the frame while choosing a mate anchor: infer the anchor and take it.
    pub(super) fn joint_pick_inferred_click(&mut self, rect: Rect, pos: Pos2) {
        // "BY ORIGINS" is not a way of pointing but a deliberate choice of A DIFFERENT anchor: the
        // origin of the part. It does not examine the geometry under the cursor at all; the body here
        // is only a finger showing which part is meant.
        if self.side.joint.anchor_mode == 3 {
            match crate::gui::pick::pick_body_at(&self.painting(), rect, pos).and_then(|mi| self.project.mesh_id(mi)) {
                Some(body) => qymcad_assembly::joint_pick_origin_click(&mut self.joint_ctx(), body),
                None => self.status = crate::i18n::tr("j-body-miss"),
            }
            return;
        }
        match self.infer_mate_anchor(rect, pos) {
            Some((body, anchor)) => qymcad_assembly::joint_pick_anchor_at(&mut self.joint_ctx(), body, anchor),
            None => self.status = crate::i18n::tr("vp-miss-face-cancel"),
        }
    }

    /// A click on the frame while RE-CHOOSING the anchor of a finished joint — the same inference.
    pub(super) fn joint_repick_inferred_click(&mut self, rect: Rect, pos: Pos2) {
        match self.infer_mate_anchor(rect, pos) {
            Some((body, anchor)) => qymcad_assembly::joint_edit_repick_apply(&mut self.joint_ctx(), body, anchor),
            None => self.status = crate::i18n::tr("vp-miss-face-cancel"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use super::super::App;
    use qymcad_core::feature::{AnchorRef, JointKind};
    use qymcad_core::model::Id;

    fn viewport() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 700.0))
    }

    /// ONE PART THAT HAS FACES, EDGES AND VERTICES ALIKE.
    fn a_part(app: &mut App) -> Id {
        let before: Vec<Id> = app.project.bodies.iter().map(|b| b.id).collect();
        super::super::joint_flow::tests::add_part_at(app, 0.0);
        let root = app.project.root;
        app.enter_component(root);
        qymcad_ui_state::rebuild_if_dirty(&mut app.rebuild_ctx());
        crate::gui::commands::refresh_edges(&mut app.part_ctx());
        app.project.bodies.iter().map(|b| b.id).find(|b| !before.contains(b)).expect("the part appeared")
    }

    /// A PERSON DOES NOT DECLARE IN ADVANCE WHAT THEY WILL POINT AT — IT IS PLAIN FROM WHERE THEY
    /// CLICKED.
    ///
    /// The same tool, the same kind of joint, NOT ONE mode switch: three clicks at three different
    /// places on one part must give three anchors of three DIFFERENT kinds.
    #[test]
    fn one_part_gives_three_different_anchors_by_where_you_point() {
        let mut app = App::default();
        let body = a_part(&mut app);
        let ctx = qymcad_ui_state::current_ctx_id(&app.active_path, &app.project);
        let wt = app.project.body_display_transform(body, ctx);
        let faces = app.project.regen_faces.get(&body).cloned().expect("the faces");
        let top = faces.iter().max_by(|a, b| a.centroid.z.total_cmp(&b.centroid.z)).expect("the top face");
        let centre = qymcad_core::feature::apply12(&wt, [top.centroid.x, top.centroid.y, top.centroid.z]);
        let edges = app.project.regen_edges.get(&body).cloned().expect("the edges");
        let e = edges.iter().find(|e| !e.is_circular()).expect("a straight edge");
        let mid = qymcad_core::feature::apply12(&wt, e.mid);
        // THE VERTEX is the end of that same edge; the midpoint of the edge cannot stand in for it.
        let vertex = qymcad_core::feature::apply12(&wt, e.a);

        let basis = app.viewing.cam.basis();
        let at = |app: &App, w: [f64; 3]| qymcad_ui_state::Screen { cam: &app.viewing.cam, set: &app.set, rect: viewport(), basis: &basis }.at(w).0;
        let kinds: Vec<(&str, AnchorRef)> = [("the centre of the face", centre), ("the middle of the edge", mid), ("the vertex", vertex)]
            .iter()
            .map(|(what, w)| {
                let p = at(&app, *w);
                let (_, a) = app.infer_mate_anchor(viewport(), p).unwrap_or_else(|| panic!("nothing was inferred under the cursor at {what}"));
                (*what, a)
            })
            .collect();
        assert!(matches!(kinds[0].1, AnchorRef::FaceCenter(..)), "at the centre of a face a FACE must be inferred, and what came out is {:?}", kinds[0].1);
        assert!(matches!(kinds[1].1, AnchorRef::EdgeMid(..)), "in the middle of an edge an EDGE must be inferred, and what came out is {:?}", kinds[1].1);
        assert!(matches!(kinds[2].1, AnchorRef::Vertex(..)), "at the end of an edge a VERTEX must be inferred, and what came out is {:?}", kinds[2].1);
    }

    /// AND IT WORKS BY HAND, WITHOUT A SINGLE MODE SWITCH.
    ///
    /// A joint is placed with two clicks on the frame: on an edge of one part and on an edge of
    /// another. This used to require choosing "anchor: edge" in advance, otherwise both clicks gave
    /// faces.
    #[test]
    fn a_mate_on_two_edges_needs_no_mode_switch() {
        let mut app = App::default();
        let before: Vec<Id> = app.project.bodies.iter().map(|b| b.id).collect();
        super::super::joint_flow::tests::add_part_at(&mut app, 0.0);
        super::super::joint_flow::tests::add_part_at(&mut app, 60.0);
        let root = app.project.root;
        app.enter_component(root);
        qymcad_ui_state::rebuild_if_dirty(&mut app.rebuild_ctx());
        crate::gui::commands::refresh_edges(&mut app.part_ctx());
        let mine: Vec<Id> = app.project.bodies.iter().map(|b| b.id).filter(|b| !before.contains(b)).collect();
        assert_eq!(mine.len(), 2, "setup: there should be two bodies of our own, and there are {}", mine.len());
        for (k, b) in mine.iter().enumerate() {
            if let Some(o) = app.project.body_owner(*b) {
                if let Some(i) = app.project.component_index(o) {
                    app.project.components[i].transform = [1.0, 0.0, 0.0, k as f64 * 60.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0];
                }
                if k == 0 {
                    app.project.set_grounded(o, true);
                }
            }
        }
        qymcad_ui_state::rebuild_if_dirty(&mut app.rebuild_ctx());
        crate::gui::commands::refresh_edges(&mut app.part_ctx());
        let ctx = qymcad_ui_state::current_ctx_id(&app.active_path, &app.project);
        let mut aim = Vec::new();
        for b in &mine {
            let wt = app.project.body_display_transform(*b, ctx);
            let edges = app.project.regen_edges.get(b).cloned().expect("the edges of the part");
            let e = edges.iter().find(|e| !e.is_circular()).expect("a straight edge");
            aim.push(qymcad_core::feature::apply12(&wt, e.mid));
        }

        let mut hand = Hand::new(&mut app);
        // THE MODE IS NOT TOUCHED AT ALL: no `anchor`, no switch — only the kind of joint and two
        // clicks.
        hand.look_at([30.0, 10.0, 5.0], 7.0).mate(JointKind::Slider).click(aim[0]).click(aim[1]);
        qymcad_ui_state::rebuild_if_dirty(&mut app.rebuild_ctx());

        let j = app.project.joints.last().cloned().expect("two clicks on edges must create a joint");
        for (side, cid) in [("A", j.a), ("B", j.b)] {
            let a = app.project.connector(cid).map(|c| c.anchor.clone());
            assert!(
                matches!(a, Some(AnchorRef::EdgeMid(..))),
                "the click was on the MIDDLE OF AN EDGE, and anchor {side} came out {a:?} — the inference under the cursor did not work"
            );
        }
        assert!(app.project.joint_faults().is_empty(), "a joint on two edges was born faulty: {:?}", app.project.joint_faults());
    }
}
