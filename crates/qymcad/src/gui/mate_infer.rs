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
use super::grab::Grab;
use super::App;
use egui::{Pos2, Rect};
use qymcad_core::feature::AnchorRef;
use qymcad_core::model::Id;

impl App {
    /// THE ANCHOR UNDER THE CURSOR: the body and what was caught on it.
    ///
    /// `None` means there is no part under the cursor; only the caller has the right to call that a
    /// miss, and the caller is also the one who says so in words.
    pub(super) fn infer_mate_anchor(&self, rect: Rect, pos: Pos2) -> Option<(Id, AnchorRef)> {
        let face = self.pick_part_face_at(rect, pos);
        let edge = self.pick_edge_any(rect, pos);
        // THE ANCHOR IS TAKEN ONLY FROM THE BODY UNDER THE CURSOR.
        //
        // Snap points used to be collected FROM EVERY body whose bounding box was near the cursor, and
        // the nearest one on screen won — anybody's. On a single part that goes unnoticed, but in a
        // machine there is always a neighbour next to it: the reported behaviour was that pointing at
        // the start of a face, with both faces horizontal, gave a gizmo handle along the Z axis. A
        // measurement on that document: the cursor stood over body 7 while the program offered
        // `EdgeMid(6, 39)` — an edge of the NEIGHBOURING part, and the axis of travel came out as that
        // part's rather than the one being pointed at. The joint glyph away from the click came from
        // the same place.
        let under = face.as_ref().map(|(b, _)| *b).or_else(|| edge.map(|(b, _)| b))?;
        let mut best: Option<(f32, Id, AnchorRef)> = None;
        let mut offer = |d: f32, body: Id, a: AnchorRef| {
            if best.as_ref().map_or(true, |(bd, _, _)| d < *bd) {
                best = Some((d, body, a));
            }
        };
        let basis = self.cam.basis();
        let ctx = self.current_ctx_id();
        // THE CENTRE OF THE FACE UNDER THE CURSOR is as much a snap point as a vertex and takes part
        // on equal terms.
        if let Some((body, key)) = face.clone() {
            let wt = self.project.body_display_transform(body, ctx);
            let w = qymcad_core::feature::apply12(&wt, key.centroid);
            offer(self.project3(w, rect, &basis).0.distance(pos), body, AnchorRef::FaceCenter(body, key));
        }
        for (_mi, body) in self.shown_bodies() {
            if body != under {
                continue; // another part gives up no anchor, however close its edge turns out to be
            }
            if !self.body_bbox_hit(body, rect, pos, &basis, 12.0) {
                continue;
            }
            let Some(edges) = self.body_edges_cached(body) else { continue };
            let wt = self.project.body_display_transform(body, ctx);
            let to_world = |p: &[f32; 3]| -> [f64; 3] {
                let v = [p[0] as f64, p[1] as f64, p[2] as f64];
                if qymcad_core::feature::is_identity12(&wt) { v } else { qymcad_core::feature::apply12(&wt, v) }
            };
            let model = self.project.regen_edges.get(&body);
            for (poly, id) in edges.0.iter().zip(edges.1.iter().copied()) {
                if id == 0 || poly.len() < 2 {
                    continue;
                }
                // THE MIDPOINT OF AN EDGE comes from the model if it is raised: on a circular edge
                // that is the centre of the hole, and the polyline will not restore it. With no model
                // the middle of the polyline is taken, which is the same thing for a straight edge.
                let mid = match model.and_then(|es| es.iter().find(|e| e.id == id)) {
                    Some(e) => {
                        let (p, _) = e.axis_ref();
                        if qymcad_core::feature::is_identity12(&wt) { p } else { qymcad_core::feature::apply12(&wt, p) }
                    }
                    None => to_world(&poly[poly.len() / 2]),
                };
                offer(self.project3(mid, rect, &basis).0.distance(pos), body, AnchorRef::EdgeMid(body, id));
                for (at_end, v) in [(false, &poly[0]), (true, &poly[poly.len() - 1])] {
                    offer(self.project3(to_world(v), rect, &basis).0.distance(pos), body, AnchorRef::Vertex(body, id, at_end));
                }
            }
        }
        if let Some((d, body, a)) = best {
            if d <= self.grab(Grab::Point) {
                return Some((body, a));
            }
        }
        // NO POINT WAS HIT — so something extended is being pointed at: an edge, and failing that a
        // face. The edge must be our own as well: another part's edge near the cursor gives no anchor.
        if let Some((body, e)) = edge.filter(|(b, _)| *b == under) {
            return Some((body, AnchorRef::EdgeMid(body, e)));
        }
        let (body, key) = face?;
        Some((body, AnchorRef::FaceCenter(body, key)))
    }

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
        let face = self.pick_part_face_at(rect, pos);
        let edge = self.pick_edge_any(rect, pos);
        let under = face.as_ref().map(|(b, _)| *b).or_else(|| edge.map(|(b, _)| b))?;
        if let Some((body, e)) = edge.filter(|(b, _)| *b == under) {
            return Some((body, AnchorRef::EdgeMid(body, e)));
        }
        let (body, key) = face?;
        Some((body, AnchorRef::FaceCenter(body, key)))
    }

    /// A click on the frame while choosing a mate anchor: infer the anchor and take it.
    pub(super) fn joint_pick_inferred_click(&mut self, rect: Rect, pos: Pos2) {
        // "BY ORIGINS" is not a way of pointing but a deliberate choice of A DIFFERENT anchor: the
        // origin of the part. It does not examine the geometry under the cursor at all; the body here
        // is only a finger showing which part is meant.
        if self.joint.anchor_mode == 3 {
            match self.pick_body_at(rect, pos).and_then(|mi| self.project.mesh_id(mi)) {
                Some(body) => self.joint_pick_origin_click(body),
                None => self.status = crate::i18n::tr("j-body-miss"),
            }
            return;
        }
        match self.infer_mate_anchor(rect, pos) {
            Some((body, anchor)) => self.joint_pick_anchor_at(body, anchor),
            None => self.status = crate::i18n::tr("vp-miss-face-cancel"),
        }
    }

    /// A click on the frame while RE-CHOOSING the anchor of a finished joint — the same inference.
    pub(super) fn joint_repick_inferred_click(&mut self, rect: Rect, pos: Pos2) {
        match self.infer_mate_anchor(rect, pos) {
            Some((body, anchor)) => self.joint_edit_repick_apply(body, anchor),
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
        app.rebuild_if_dirty();
        app.refresh_edges();
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
        let ctx = app.current_ctx_id_for_test();
        let wt = app.project.body_display_transform(body, ctx);
        let faces = app.project.regen_faces.get(&body).cloned().expect("the faces");
        let top = faces.iter().max_by(|a, b| a.centroid.z.total_cmp(&b.centroid.z)).expect("the top face");
        let centre = qymcad_core::feature::apply12(&wt, [top.centroid.x, top.centroid.y, top.centroid.z]);
        let edges = app.project.regen_edges.get(&body).cloned().expect("the edges");
        let e = edges.iter().find(|e| !e.is_circular()).expect("a straight edge");
        let mid = qymcad_core::feature::apply12(&wt, e.mid);
        // THE VERTEX is the end of that same edge; the midpoint of the edge cannot stand in for it.
        let vertex = qymcad_core::feature::apply12(&wt, e.a);

        let basis = app.cam.basis();
        let at = |app: &App, w: [f64; 3]| app.project3(w, viewport(), &basis).0;
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
        app.rebuild_if_dirty();
        app.refresh_edges();
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
        app.rebuild_if_dirty();
        app.refresh_edges();
        let ctx = app.current_ctx_id_for_test();
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
        app.rebuild_if_dirty();

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
