//! THE JOINT PULT OPENS BY ITSELF AND WORKS.
//!
//! In grown-up CAD a pult appears on a joint after the second pointing: flip the main axis, turn the
//! secondary one, swap the roles of the parts, run the motion. Here a joint used to appear silently in
//! the list, and to fix the side or the order one first had to guess to look for it there — more often
//! a person simply deleted the joint and assembled it again, losing its drives, its limits and its
//! name.
//!
//! The checks are live: the hand assembles a joint by clicks on the frame and then presses the handles
//! of the pult — and what is measured is THE FACT, that is, which part moved afterwards.
#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use super::super::App;
    use qymcad_core::feature::JointKind;
    use qymcad_core::model::Id;

    /// A point ON THE BODY that can be clicked: the centre of the topmost face.
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

    /// TWO PARTS AND A SLIDER BETWEEN THEM, assembled BY HAND. Returns (joint, body A, body B).
    fn a_slider_by_hand(app: &mut App) -> (Id, Id, Id) {
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
            }
        }
        app.rebuild_if_dirty();
        app.refresh_edges();
        let (pa, pb) = (aim(app, mine[0]), aim(app, mine[1]));
        let mut hand = Hand::new(app);
        hand.look_at([30.0, 10.0, 5.0], 6.0).mate(JointKind::Slider).anchor(3).click(pa).click(pb);
        app.rebuild_if_dirty();
        let jid = app.project.joints.last().map(|j| j.id).expect("two clicks must create the joint");
        (jid, mine[0], mine[1])
    }

    /// WHERE the part went when the joint is driven by `how_far` — as a vector.
    ///
    /// The axis of travel is not known in advance (the anchor sets it), so measuring the projection
    /// onto a world axis picked at random is not allowed: the first attempt gave -0.0000 along X
    /// simply because the slider ran along Z.
    fn travel_vector(app: &mut App, joint: Id, body: Id, how_far: f64) -> [f64; 3] {
        let owner = app.project.body_owner(body).expect("the owner of the body");
        let m = app.project.world_transform(owner);
        let was = [m[3], m[7], m[11]];
        let base = app.project.joints.iter().find(|x| x.id == joint).map(|x| x.offset).unwrap_or(0.0);
        if let Some(j) = app.project.joints.iter_mut().find(|x| x.id == joint) {
            j.drive[1] = Some(base + how_far);
        }
        app.project.solve_joints();
        let m = app.project.world_transform(owner);
        let now = [m[3], m[7], m[11]];
        if let Some(j) = app.project.joints.iter_mut().find(|x| x.id == joint) {
            j.drive[1] = None;
        }
        app.project.solve_joints();
        [now[0] - was[0], now[1] - was[1], now[2] - was[2]]
    }

    fn length(v: [f64; 3]) -> f64 {
        (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
    }

    fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
        a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
    }

    /// How far the part travelled along its degree when the joint is driven by `how_far`.
    #[allow(dead_code)]
    fn travel(app: &mut App, joint: Id, body: Id, how_far: f64) -> f64 {
        let owner = app.project.body_owner(body).expect("the owner of the body");
        let was = app.project.world_transform(owner);
        let base = app.project.joints.iter().find(|x| x.id == joint).map(|x| x.offset).unwrap_or(0.0);
        if let Some(j) = app.project.joints.iter_mut().find(|x| x.id == joint) {
            j.drive[1] = Some(base + how_far);
        }
        app.project.solve_joints();
        let now = app.project.world_transform(owner);
        let went = was.iter().zip(now.iter()).map(|(x, y)| (x - y).abs()).fold(0.0f64, f64::max);
        if let Some(j) = app.project.joints.iter_mut().find(|x| x.id == joint) {
            j.drive[1] = None;
        }
        app.project.solve_joints();
        went
    }

    /// THE PULT OPENS BY ITSELF as soon as the joint is assembled.
    #[test]
    fn the_hud_opens_by_itself_when_the_mate_is_made() {
        let mut app = App::default();
        let (jid, _, _) = a_slider_by_hand(&mut app);
        assert_eq!(
            app.joint_edit_for_test(),
            Some(jid),
            "the joint is assembled and the pult did not open: there is nowhere to flip the axis and swap the roles without hunting for the joint in the list"
        );
    }

    /// SWAPPING THE ROLES REVERSES THE RECKONING OF THE TRAVEL.
    ///
    /// WHAT THE ROLES DECIDE AND WHAT THEY DO NOT was found out BY MEASUREMENT, not by reasoning. The
    /// first edition of this check claimed the swap changes WHICH PART MOVES, and grounded first one
    /// and then the other. Damaging the code (a swap that does nothing) did not make it fail, and an
    /// honest rewrite showed: after the swap it is still the ungrounded part that moves, all
    /// 14.0000 mm. Which part moves is decided by THE GROUND.
    ///
    /// The roles decide something else: which anchor the travel is reckoned from. So what has to be
    /// measured is THE SIGN: one and the same drive must lead the part the OPPOSITE way.
    #[test]
    fn swapping_roles_reverses_the_direction_of_travel() {
        let mut app = App::default();
        let (jid, first, second) = a_slider_by_hand(&mut app);
        if let Some(o) = app.project.body_owner(first) {
            app.project.set_grounded(o, true);
        }
        app.rebuild_if_dirty();

        let before = travel_vector(&mut app, jid, second, 14.0);
        assert!(length(before) > 1e-3, "GUARD: before the swap the part must move, and it travelled {:.4} ({before:?})", length(before));

        // THE HANDLE OF THE PULT AND NOTHING ELSE: neither the ground nor the anchors are touched.
        app.joint_hud_swap_roles_for_test(jid);
        app.rebuild_if_dirty();

        let after = travel_vector(&mut app, jid, second, 14.0);
        assert!(length(after) > 1e-3, "after the swap the part stopped moving at all: it travelled {:.4} ({after:?})", length(after));
        assert!(
            dot(before, after) < 0.0,
            "swapping the roles must reverse the reckoning of the travel: before the swap {before:?}, after it {after:?} — the side did not change"
        );
    }

    /// SWAPPING THE ROLES LOSES NOTHING BUT THE ORDER.
    ///
    /// That is what it exists for: the order used to be corrected by recreating the joint — along with
    /// its name, its drives and its limits.
    #[test]
    fn swapping_roles_keeps_the_name_the_limits_and_the_drives() {
        let mut app = App::default();
        let (jid, _, _) = a_slider_by_hand(&mut app);
        if let Some(j) = app.project.joints.iter_mut().find(|x| x.id == jid) {
            j.name = "my-joint".into();
            j.limit_min[1] = Some(-5.0);
            j.limit_max[1] = Some(25.0);
            j.drive[1] = Some(7.0);
        }
        let (a_was, b_was) = app.project.joints.iter().find(|x| x.id == jid).map(|j| (j.a, j.b)).expect("the joint");

        app.joint_hud_swap_roles_for_test(jid);

        let j = app.project.joints.iter().find(|x| x.id == jid).expect("the joint is there");
        assert_eq!((j.a, j.b), (b_was, a_was), "the anchors must swap places");
        assert_eq!(j.name, "my-joint", "the name of the joint was lost in the role swap");
        assert_eq!((j.limit_min[1], j.limit_max[1]), (Some(-5.0), Some(25.0)), "the limits were lost in the role swap");
        assert_eq!(j.drive[1], Some(7.0), "the drive was lost in the role swap");
        assert!(!j.flip_decided, "the mating side must be decided AFRESH: the previous answer was chosen for a different pair");
    }
}
