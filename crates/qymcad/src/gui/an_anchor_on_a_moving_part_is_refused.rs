//! AN ANCHOR ON A MOVING PART IS REFUSED.
//!
//! The trouble this exists for: such an anchor makes an assembly unstable FOREVER — the joint holds
//! on to a part that travels inside the same assembly, and every recomputation carries it further. On
//! a real machine that cost 60 mm per solve, endlessly, and looked like "the assembly drifts apart by
//! itself"; the cause was hunted in the solver, in grounding and in how the problem was split — and it
//! was in the anchor.
//!
//! Learning about it AFTERWARDS from a mark in the timeline is not enough: a person places an anchor
//! deliberately, and they have to be stopped the second they point at the wrong part.
#[cfg(test)]
mod tests {
    use super::super::App;
    use qymcad_core::feature::{AnchorRef, ComponentKind, JointKind};
    use qymcad_core::model::Id;

    /// A SUBASSEMBLY WITH A PART TRAVELLING INSIDE IT, and the body of that part.
    ///
    /// Returns (subassembly, moving part, its body).
    fn a_subassembly_with_a_traveller(app: &mut App) -> (Id, Id, Id) {
        let root = app.project.ensure_root();
        let sub = app.project.add_component_kind("subassembly", ComponentKind::Assembly);
        let still = app.project.add_component_kind("stationary", ComponentKind::Part);
        let moving = app.project.add_component_kind("moving", ComponentKind::Part);
        for (c, parent) in [(sub, root), (still, sub), (moving, sub)] {
            if let Some(i) = app.project.component_index(c) {
                app.project.components[i].parent = Some(parent);
            }
        }
        // a slider INSIDE the subassembly; the reference part is grounded, otherwise which drives which is undefined
        app.project.set_grounded(still, true);
        let (ca, cb) = (app.project.add_connector(still, AnchorRef::Origin), app.project.add_connector(moving, AnchorRef::Origin));
        app.project.add_joint(ca, cb, JointKind::Slider);

        // THE BODY OF THE MOVING PART. A body belongs to a component through a TIMELINE NODE.
        let body = app.project.alloc_id();
        let node = app.project.alloc_id();
        app.project.timeline.push(qymcad_core::feature::FeatureNode {
            id: node,
            name: "body".into(),
            kind: qymcad_core::feature::FeatureKind::Import { body, source: 0, solid: 0 },
            parent: Some(moving),
            dirty: false,
            suppressed: false,
        });
        (sub, moving, body)
    }

    #[test]
    fn pointing_at_geometry_of_a_travelling_part_is_refused_with_words() {
        let mut app = App::default();
        let (sub, moving, body) = a_subassembly_with_a_traveller(&mut app);
        let key = qymcad_core::feature::FaceKey { index: 0, centroid: [0.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0], id: 1 };
        let anchor = AnchorRef::FaceCenter(body, key);

        // GUARD AGAINST A VACUOUS CHECK: the kernel really does consider this anchor to sit on a moving part.
        assert!(
            app.project.anchor_sits_on_moving_part(sub, &anchor),
            "GUARD: there is no trap — the kernel does not consider the anchor to sit on a moving part, so there is nothing to check"
        );
        assert!(app.project.drive_joint_for(moving).is_some(), "GUARD: the part inside must be a moving one");

        // THE PERSON POINTS EXACTLY THERE.
        let before = app.project.connectors.len();
        app.joint_pick_anchor_click_for_test(sub, anchor);

        assert_eq!(
            app.project.connectors.len(),
            before,
            "an anchor on a moving part was created after all: after such an anchor the assembly will never settle"
        );
        assert_eq!(
            app.status,
            crate::i18n::tr("j-anchor-on-moving-part-refused"),
            "the person was not told why the pick was not accepted: {}",
            app.status
        );
    }

    /// AND AN ORDINARY PICK IS ACCEPTED. Otherwise the refusal would turn into "nothing is allowed".
    #[test]
    fn pointing_at_stationary_geometry_is_taken_as_before() {
        let mut app = App::default();
        let before: Vec<Id> = app.project.bodies.iter().map(|b| b.id).collect();
        super::super::joint_flow::tests::add_part_at(&mut app, 0.0);
        let root = app.project.root;
        app.enter_component(root);
        app.rebuild_if_dirty();
        app.refresh_edges();
        let body = app.project.bodies.iter().map(|b| b.id).find(|b| !before.contains(b)).expect("the part appeared");
        let owner = app.project.body_owner(body).expect("the owner of the body");
        let f = app
            .project
            .regen_faces
            .get(&body)
            .and_then(|fs| fs.iter().max_by(|a, b| a.centroid.z.total_cmp(&b.centroid.z)))
            .cloned()
            .expect("a face");
        let key = qymcad_core::feature::FaceKey { index: 0, centroid: [f.centroid.x, f.centroid.y, f.centroid.z], normal: f.normal, id: f.id };

        let n = app.project.connectors.len();
        app.joint_pick_anchor_click_for_test(owner, AnchorRef::FaceCenter(body, key));
        assert!(
            app.project.connectors.len() > n || app.joint_pick_first_anchor_for_test().is_some(),
            "an ordinary pick on stationary geometry was not accepted: {}",
            app.status
        );
    }
}
