//! A SLIDER BETWEEN FACES THAT LOOK AT EACH OTHER DOES ASSEMBLE.
//!
//! The most ordinary case in an assembly: two parts stand side by side and the joint is placed on the
//! faces that face each other. That is exactly what the scenario document did, and its slider came
//! out UNSATISFIABLE: `joint-miss#1.990`.
//!
//! MEASURING the anchor frames named the cause: the TRAVEL axis of both anchors coincides (cosine
//! 1.0000) while the ROLL axis is exactly opposite (cosine -1.0000), which gives a residual of
//! `|x_b - x_a| = 2`. And that is a point of ZERO DERIVATIVE: for exactly opposite vectors there is no
//! direction to turn towards, both are equally good, and the solver has nowhere to step.
//!
//! The anchors here are placed by the same call a face click uses (`joint_pick_face_click`) but
//! without a ray: the faces look AT EACH OTHER and both cannot be seen from one viewpoint — and what
//! is checked here is the choice of the MATING SIDE, not hitting them with the mouse (other checks
//! hold the hit-testing).
#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use super::super::App;
    use qymcad_core::feature::{FaceKey, JointKind};
    use qymcad_core::model::Id;

    /// A face of a part looking IN THE GIVEN DIRECTION (the largest such face).
    fn face_towards(app: &App, body: Id, dir: [f64; 3]) -> FaceKey {
        let faces = app.project.regen_faces.get(&body).expect("the body has faces");
        let f = faces
            .iter()
            .filter(|f| f.normal[0] * dir[0] + f.normal[1] * dir[1] + f.normal[2] * dir[2] > 0.9)
            .max_by(|x, y| x.area.total_cmp(&y.area))
            .expect("a face looking the required way");
        FaceKey { index: 0, centroid: [f.centroid.x, f.centroid.y, f.centroid.z], normal: f.normal, id: f.id }
    }

    #[test]
    fn a_slider_on_two_faces_that_look_at_each_other_is_assembled() {
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
        if let Some(o) = app.project.body_owner(mine[0]) {
            app.project.set_grounded(o, true);
        }

        // THE FACING FACES: the right one of the first part, the left one of the second.
        let (ka, kb) = (face_towards(&app, mine[0], [1.0, 0.0, 0.0]), face_towards(&app, mine[1], [-1.0, 0.0, 0.0]));

        let mut hand = Hand::new(&mut app);
        hand.look_at([40.0, 10.0, 5.0], 6.0).mate(JointKind::Slider).anchor(0);
        app.joint_pick_face_click_for_test(mine[0], ka);
        app.joint_pick_face_click_for_test(mine[1], kb);
        app.rebuild_if_dirty();

        let jid = app.project.joints.last().map(|j| j.id).expect("two face picks must create a joint");
        let faults = app.project.joint_faults();
        assert!(!faults.iter().any(|(id, _)| *id == jid), "the joint was born faulty: {faults:?}");
        assert!(
            !app.project.mates_violated.contains(&jid),
            "the slider between FACING faces did not assemble: the joint is violated while the arrangement is legitimate; status: {}",
            app.status
        );

        // AND IT MUST MOVE.
        let owner = app.project.body_owner(mine[1]).expect("the owner of the driven part");
        let was = app.project.world_transform(owner);
        let base = app.project.joints.iter().find(|x| x.id == jid).map(|x| x.offset).unwrap_or(0.0);
        if let Some(j) = app.project.joints.iter_mut().find(|x| x.id == jid) {
            j.drive[1] = Some(base + 14.0);
        }
        app.project.solve_joints();
        let now = app.project.world_transform(owner);
        let went = was.iter().zip(now.iter()).map(|(x, y)| (x - y).abs()).fold(0.0f64, f64::max);
        assert!((went - 14.0).abs() < 1e-3, "the slider must drive the part 14 mm, and it travelled {went:.4}");
    }
}
