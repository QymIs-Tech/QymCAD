//! CHANGING AN ANCHOR ON A FINISHED JOINT DOES NOT TURN THE PART OVER.
//!
//! Reported behaviour: why does it flip by 180 degrees on the B face you pick? The path: the joint is
//! already there, the pult is opened, "change the anchor" -> B is pressed and another face is clicked
//! — and the part somersaults.
//!
//! The cause is the frozen SIDE OF THE MATING. It is chosen by proximity to the present placement and
//! remembered in the joint (`flip_decided`) so that the solver does not swing the part between two
//! solutions. But what was frozen was the answer chosen for the OLD pair of anchors: change the face
//! and the answer refers to geometry that no longer exists, and holds the part turned around.
//!
//! The check measures THE TURN OF THE PART, not the flag: the flag is the mechanism, and what a person
//! sees is the somersault.
#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use super::super::App;
    use qymcad_core::feature::{FaceKey, JointKind};
    use qymcad_core::model::Id;

    /// The face of the part facing A GIVEN DIRECTION (the largest of those).
    fn face_towards(app: &App, body: Id, dir: [f64; 3]) -> FaceKey {
        let faces = app.project.regen_faces.get(&body).expect("the body has faces");
        let f = faces
            .iter()
            .filter(|f| f.normal[0] * dir[0] + f.normal[1] * dir[1] + f.normal[2] * dir[2] > 0.9)
            .max_by(|x, y| x.area.total_cmp(&y.area))
            .expect("a face looking the right way");
        FaceKey { index: 0, centroid: [f.centroid.x, f.centroid.y, f.centroid.z], normal: f.normal, id: f.id }
    }

    /// THE ANGLE OF TURN between two placements of the part, in degrees.
    ///
    /// Computed from the trace of `transpose(R) * R'`: that is the very somersault a person sees with
    /// their eyes — unlike the side flag, which is never shown to them at all.
    fn turn_deg(was: &[f64; 12], now: &[f64; 12]) -> f64 {
        let col = |m: &[f64; 12], k: usize| [m[k], m[4 + k], m[8 + k]];
        let mut tr = 0.0;
        for k in 0..3 {
            let (a, b) = (col(was, k), col(now, k));
            tr += a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        }
        (0.5 * (tr - 1.0)).clamp(-1.0, 1.0).acos().to_degrees()
    }

    /// Two parts, a slider between faces looking AT EACH OTHER. Returns (joint, body A, body B).
    ///
    /// The kind is a SLIDER, as in the reported case: the frame of an anchor is built TO SUIT THE
    /// KIND, and for a slider the main axis is not the normal of the face but the direction of
    /// travel.
    fn a_slider_by_faces(app: &mut App) -> (Id, Id, Id) {
        let before: Vec<Id> = app.project.bodies.iter().map(|b| b.id).collect();
        super::super::joint_flow::tests::add_part_at(app, 0.0);
        super::super::joint_flow::tests::add_part_at(app, 60.0);
        let root = app.project.root;
        app.enter_component(root);
        app.rebuild_if_dirty();
        app.refresh_edges();
        let mine: Vec<Id> = app.project.bodies.iter().map(|b| b.id).filter(|b| !before.contains(b)).collect();
        assert_eq!(mine.len(), 2, "setup: there should be two bodies of our own, and there are {}", mine.len());
        if let Some(o) = app.project.body_owner(mine[0]) {
            app.project.set_grounded(o, true);
        }
        let (ka, kb) = (face_towards(app, mine[0], [1.0, 0.0, 0.0]), face_towards(app, mine[1], [-1.0, 0.0, 0.0]));
        let mut hand = Hand::new(app);
        hand.look_at([40.0, 10.0, 5.0], 6.0).mate(JointKind::Slider).anchor(0);
        app.joint_pick_face_click_for_test(mine[0], ka);
        app.joint_pick_face_click_for_test(mine[1], kb);
        app.rebuild_if_dirty();
        let jid = app.project.joints.last().map(|j| j.id).expect("pointing at two faces must create the joint");
        (jid, mine[0], mine[1])
    }

    /// THE B FACE WAS CHANGED — THE PART DID NOT SOMERSAULT.
    #[test]
    fn changing_the_b_anchor_does_not_flip_the_part_over() {
        let mut app = App::default();
        let (jid, _, b_body) = a_slider_by_faces(&mut app);
        app.project.solve_joints();
        let owner = app.project.body_owner(b_body).expect("the owner of the driven part");
        let was = app.project.world_transform(owner);

        // THE REPORTED PATH: the pult -> "change the anchor" -> B -> a click on another face of the
        // same part.
        let other = face_towards(&app, b_body, [1.0, 0.0, 0.0]);
        app.joint.edit_repick = Some((jid, true));
        app.joint_edit_repick_apply_for_test(b_body, qymcad_core::feature::AnchorRef::FaceCenter(b_body, other));
        app.rebuild_if_dirty();
        app.project.solve_joints();

        let now = app.project.world_transform(owner);
        let turn = turn_deg(&was, &now);
        assert!(
            turn < 1.0,
            "anchor B was changed and the part was turned by {turn:.3} deg, while the mating side was supposed to be chosen again by proximity"
        );
    }

    /// THE SAME FOR ANCHOR A: it changes on the fly too, and the freezing refers to the same pair.
    #[test]
    fn changing_the_a_anchor_does_not_flip_the_part_over() {
        let mut app = App::default();
        let (jid, a_body, b_body) = a_slider_by_faces(&mut app);
        app.project.solve_joints();
        let owner = app.project.body_owner(b_body).expect("the owner of the driven part");
        let was = app.project.world_transform(owner);

        let other = face_towards(&app, a_body, [-1.0, 0.0, 0.0]);
        app.joint.edit_repick = Some((jid, false));
        app.joint_edit_repick_apply_for_test(a_body, qymcad_core::feature::AnchorRef::FaceCenter(a_body, other));
        app.rebuild_if_dirty();
        app.project.solve_joints();

        let now = app.project.world_transform(owner);
        let turn = turn_deg(&was, &now);
        assert!(turn < 1.0, "anchor A was changed and the part was turned by {turn:.3} deg, while the side was supposed to be chosen again");
    }

    /// AND WHEN A PERSON ASKS FOR THE FLIP — THE PART MUST FLIP.
    ///
    /// The other side of the same change, and without it the cure is worse than the illness: the
    /// automation holds the part where it stands, so the "flip the axis" handle is the only way to
    /// demand the other side. There was NO check for it at all, and its earlier edition flipped the
    /// axis on the anchor, after which the side was chosen again by proximity and brought the part
    /// back: the handle did NOTHING, and there was nowhere to learn that from.
    #[test]
    fn the_flip_handle_really_turns_the_part_over() {
        let mut app = App::default();
        let (jid, _, b_body) = a_slider_by_faces(&mut app);
        app.project.solve_joints();
        let owner = app.project.body_owner(b_body).expect("the owner of the driven part");
        let was = app.project.world_transform(owner);

        app.joint_hud_flip_axis_for_test(jid);
        app.rebuild_if_dirty();
        app.project.solve_joints();

        let now = app.project.world_transform(owner);
        let turn = turn_deg(&was, &now);
        assert!((turn - 180.0).abs() < 1.0, "the flip-the-axis handle was pressed and the part turned by {turn:.3} deg instead of 180");

        // AND A SECOND PRESS BRINGS IT BACK: the handle must be reversible, otherwise it is a trap.
        app.joint_hud_flip_axis_for_test(jid);
        app.rebuild_if_dirty();
        app.project.solve_joints();
        let back = app.project.world_transform(owner);
        let turn = turn_deg(&was, &back);
        assert!(turn < 1.0, "a second press must put the part back as it was, and it left it turned by {turn:.3} deg");
    }
}
