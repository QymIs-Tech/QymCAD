//! A MECHANISM CAN BE RUN EVEN WITHOUT LIMITS.
//!
//! Running a degree of freedom existed, but only for one with a limit SET; a rotation had a default
//! range (a full turn) while a travel did not, and the panel said "limits are required". Build a
//! slider, want to see how it travels — first be so kind as to invent the bounds. That is a crutch: a
//! travel simply has no "full turn", and it needs to be seen in motion just the same.
//!
//! THE DEFAULT TRAVEL IS NOT INVENTED BUT MEASURED: the length of the driven part itself along its own
//! axis of motion — that is how far it goes when it shifts by its own size. Proportionate to the
//! assembly rather than a hard-wired number.
#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use super::super::App;
    use qymcad_core::feature::JointKind;
    use qymcad_core::model::Id;

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

    /// A SLIDER WITHOUT A SINGLE LIMIT, assembled by hand.
    fn a_slider_without_limits(app: &mut App) -> (Id, Id) {
        let before: Vec<Id> = app.project.bodies.iter().map(|b| b.id).collect();
        super::super::joint_flow::tests::add_part_at(app, 0.0);
        super::super::joint_flow::tests::add_part_at(app, 60.0);
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
        let (pa, pb) = (aim(app, mine[0]), aim(app, mine[1]));
        let mut hand = Hand::new(app);
        hand.look_at([30.0, 10.0, 5.0], 6.0).mate(JointKind::Slider).anchor(3).click(pa).click(pb);
        app.rebuild_if_dirty();
        let jid = app.project.joints.last().map(|j| j.id).expect("two clicks must create a joint");
        let j = app.project.joints.iter().find(|x| x.id == jid).expect("the joint");
        assert!(
            j.limit_min.iter().chain(j.limit_max.iter()).all(|b| b.is_none()),
            "GUARD: the joint must have no limits, otherwise there is nothing to check"
        );
        (jid, mine[1])
    }

    /// A TRAVEL WITHOUT LIMITS STILL HAS A RANGE, proportionate to the part itself.
    #[test]
    fn a_slider_without_limits_still_has_a_range_to_run() {
        let mut app = App::default();
        let (jid, moving) = a_slider_without_limits(&mut app);
        let range = app.project.joint_anim_range(jid, 1);
        let Some((lo, hi)) = range else {
            panic!("a slider without limits has nowhere to run, so the mechanism cannot be seen in motion until bounds are invented");
        };
        assert!(hi > lo, "the run range is empty: from {lo:.4} to {hi:.4}");

        // THE TRAVEL MUST BE PROPORTIONATE TO THE PART rather than a hard-wired number: compare it with the part's extent.
        let owner = app.project.body_owner(moving).expect("the owner of the driven part");
        let ctx = app.current_ctx_id_for_test();
        let dir = app.project.joint_slot_axis(jid, 1, ctx).expect("the travel axis");
        let mut span: f64 = 0.0;
        for b in app.project.component_bodies(owner) {
            if let Some(mi) = app.project.mesh_index(b) {
                if let Some(bb) = app.project.bodies.get(mi).and_then(|x| x.mesh.bounds()) {
                    let d = [(bb.max.x - bb.min.x) * dir[0], (bb.max.y - bb.min.y) * dir[1], (bb.max.z - bb.min.z) * dir[2]];
                    span = span.max(d[0].abs() + d[1].abs() + d[2].abs());
                }
            }
        }
        assert!(span > 1e-6, "GUARD: the extent of the part along the travel axis is zero, so there is nothing to compare with");
        assert!(
            ((hi - lo) - span).abs() < 1e-6,
            "the default travel must equal the length of the part along the axis of motion ({span:.4}), and it is {:.4}",
            hi - lo
        );
    }

    /// AND THE RUN REALLY CARRIES THE PART: from the start of the range to its end it covers the whole travel.
    #[test]
    fn running_the_slider_carries_the_part_the_whole_way() {
        let mut app = App::default();
        let (jid, moving) = a_slider_without_limits(&mut app);
        let (lo, hi) = app.project.joint_anim_range(jid, 1).expect("the run range");
        let owner = app.project.body_owner(moving).expect("the owner of the driven part");

        let at = |app: &mut App, v: f64| -> [f64; 3] {
            if let Some(j) = app.project.joints.iter_mut().find(|x| x.id == jid) {
                j.drive[1] = Some(v);
            }
            app.project.solve_joints();
            let m = app.project.world_transform(owner);
            [m[3], m[7], m[11]]
        };
        let (start, end) = (at(&mut app, lo), at(&mut app, hi));
        let went = ((end[0] - start[0]).powi(2) + (end[1] - start[1]).powi(2) + (end[2] - start[2]).powi(2)).sqrt();
        assert!(
            (went - (hi - lo)).abs() < 1e-3,
            "a run from {lo:.4} to {hi:.4} must carry the part {:.4}, and it travelled {went:.4}",
            hi - lo
        );
    }
}
