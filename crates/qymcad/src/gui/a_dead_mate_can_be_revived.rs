//! A DEAD MATE CAN BE REVIVED BY HAND.
//!
//! In a real document four joints are dead the moment it opens: their anchors were lost to an old
//! deletion defect (`j-fault-connector-lost`, the document holds fewer anchors than the joints ask
//! for). The defect is closed, but a DAMAGED file could not be helped: the joint is in the list, the
//! part does not move, and nothing but deletion could repair it.
//!
//! The way out already existed in the interface — "Change anchor" — but it did not work on a lost
//! anchor: `set_connector_anchor` looks the anchor up by id and, finding none, silently does nothing.
//! What is checked here is the whole path: the joint is dead, a person re-picks the anchor with a
//! click, and the joint comes alive and moves.
#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use super::super::App;
    use qymcad_core::feature::JointKind;
    use qymcad_core::model::Id;

    fn viewport() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 700.0))
    }

    /// A point ON THE BODY that can be clicked: the centre of its topmost face.
    fn aim(app: &App, body: Id) -> [f64; 3] {
        let wt = app.project.body_display_transform(body, app.current_ctx_id_for_test());
        let f = app
            .project
            .regen_faces
            .get(&body)
            .and_then(|fs| fs.iter().max_by(|a, b| a.centroid.z.total_cmp(&b.centroid.z)))
            .expect("the body has faces");
        qymcad_core::feature::apply12(&wt, [f.centroid.x, f.centroid.y, f.centroid.z])
    }

    #[test]
    fn a_mate_whose_anchor_was_lost_is_revived_by_repicking_it() {
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
        let (pa, pb) = (aim(&app, mine[0]), aim(&app, mine[1]));

        let mut hand = Hand::new(&mut app);
        hand.look_at([30.0, 10.0, 5.0], 7.0).mate(JointKind::Slider).anchor(0).click(pa).click(pb);
        app.rebuild_if_dirty();
        let jid = app.project.joints.last().map(|j| j.id).expect("the joint was created");
        let lost = app.project.joints.iter().find(|x| x.id == jid).map(|x| x.a).expect("anchor A");

        // LOSE THE ANCHOR — exactly what the old deletion defect did to a real file: the joint
        // stayed, the anchor did not.
        app.project.connectors.retain(|c| c.id != lost);
        app.rebuild_if_dirty();
        assert!(
            app.project.joint_faults().iter().any(|(id, why)| *id == jid && *why == "j-fault-connector-lost"),
            "GUARD: the joint must become dead, otherwise there is nothing to revive: {:?}",
            app.project.joint_faults()
        );

        // THE PERSON RE-PICKS THE ANCHOR: "Change anchor -> A", then a click on a face of the part.
        app.joint.edit = Some(jid);
        app.joint.edit_repick = Some((jid, false));
        app.set_joint_anchor_mode_for_test(0);
        let basis = app.cam.basis();
        let at = app.project3(pa, viewport(), &basis).0;
        app.refresh_edges();
        app.viewport_3d_click_at(at, viewport(), &basis);
        app.rebuild_if_dirty();

        let faults = app.project.joint_faults();
        assert!(
            !faults.iter().any(|(id, _)| *id == jid),
            "the joint must COME ALIVE after the anchor is re-picked, and it is still dead: {faults:?}; status: {}",
            app.status
        );

        // AND ONCE ALIVE IT MUST MOVE, not merely count as healthy.
        let owner = app.project.body_owner(mine[1]).expect("the owner of the driven part");
        let was = app.project.world_transform(owner);
        let base = app.project.joints.iter().find(|x| x.id == jid).map(|x| x.offset).unwrap_or(0.0);
        if let Some(j) = app.project.joints.iter_mut().find(|x| x.id == jid) {
            j.drive[1] = Some(base + 12.0);
        }
        app.project.solve_joints();
        let now = app.project.world_transform(owner);
        let went = was.iter().zip(now.iter()).map(|(x, y)| (x - y).abs()).fold(0.0f64, f64::max);
        assert!((went - 12.0).abs() < 1e-3, "a revived joint must drive the part 12 mm, and it travelled {went:.4}");
    }
}
