//! A PART IS TAKEN BY HAND AND LED ALONG ITS DEGREES.
//!
//! The handles of the degrees existed before, but pulling was possible ONLY by the thin arrow of the
//! gizmo: miss it and the mechanism did not stir. Grown-up CAD lets a part be grabbed anywhere, and
//! that is not about convenience: until a mechanism is handled, nobody notices it was assembled
//! wrongly.
//!
//! The checks go BY THE SAME PATH as the mouse: `joint_grab_part_at` is the very call the press pass
//! of a frame makes, and `joint_giz_drag_to` is the one that leads.
#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use super::super::App;
    use qymcad_core::feature::JointKind;
    use qymcad_core::model::Id;

    fn viewport() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 700.0))
    }

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

    /// A SLIDER BETWEEN TWO PARTS, assembled BY HAND; the first one is grounded.
    fn a_slider_by_hand(app: &mut App) -> (Id, Id) {
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
        let jid = app.project.joints.last().map(|j| j.id).expect("two clicks must create the joint");
        (jid, mine[1])
    }

    /// Where the degree goes ON SCREEN — a unit vector in pixels.
    fn screen_direction_of(app: &App, joint: Id, slot: usize) -> egui::Vec2 {
        let ctx = app.current_ctx_id_for_test();
        let basis = app.cam.basis();
        let m = app.project.joint_frame(joint, ctx).expect("the frame of the joint");
        let o = [m[3], m[7], m[11]];
        let d = app.project.joint_slot_axis(joint, slot, ctx).expect("the axis of the degree");
        let len = 60.0 / app.cam.scale as f64;
        let tip = [o[0] + d[0] * len, o[1] + d[1] * len, o[2] + d[2] * len];
        let v = app.project3(tip, viewport(), &basis).0 - app.project3(o, viewport(), &basis).0;
        let n = (v.x * v.x + v.y * v.y).sqrt();
        assert!(n > 1e-3, "GUARD: the axis of the degree points straight into the screen — it cannot be pulled from such a view");
        v / n
    }

    /// Lead the cursor `by` pixels from a point over the part, the way the mouse does.
    fn drag_the_part(app: &mut App, body: Id, by: egui::Vec2) -> bool {
        let basis = app.cam.basis();
        let at = app.project3(aim(app, body), viewport(), &basis).0;
        if !app.joint_grab_part_at_for_test(viewport(), at, by, &basis) {
            return false;
        }
        // lead in four steps — like a real drag rather than one jump
        for k in 1..=4 {
            let step = by * (k as f32 / 4.0);
            app.joint_giz_drag_to_for_test(at + step, by / 4.0, viewport(), &basis);
        }
        app.joint_giz_end_for_test();
        true
    }

    /// A PART CAN BE GRABBED AND IT GOES ALONG ITS OWN AXIS — AND ALONG NOTHING ELSE.
    #[test]
    fn grabbing_the_part_moves_it_along_its_own_axis_only() {
        let mut app = App::default();
        let (jid, moving) = a_slider_by_hand(&mut app);
        let owner = app.project.body_owner(moving).expect("the owner of the driven part");
        let ctx = app.current_ctx_id_for_test();
        let axis = app.project.joint_slot_axis(jid, 1, ctx).expect("the axis of travel of the slider");
        let m = app.project.world_transform(owner);
        let was = [m[3], m[7], m[11]];

        // LEAD WHERE THE TRAVEL GOES ON SCREEN — that is what a person does, rather than blindly
        // along X: in this assembly the slider runs along Z, and moving the cursor to the right is
        // almost across it.
        let by = screen_direction_of(&app, jid, 1) * 60.0;
        assert!(drag_the_part(&mut app, moving, by), "the part could not be grabbed — the mechanism cannot be handled");

        let m = app.project.world_transform(owner);
        let went = [m[3] - was[0], m[7] - was[1], m[11] - was[2]];
        let len = (went[0] * went[0] + went[1] * went[1] + went[2] * went[2]).sqrt();
        assert!(len > 1e-3, "the part was grabbed and led, and it did not move at all: it travelled {len:.4}");
        // ALONG THE AXIS AND NOWHERE ELSE: the transverse share must be zero.
        let along = went[0] * axis[0] + went[1] * axis[1] + went[2] * axis[2];
        let across = (len * len - along * along).max(0.0).sqrt();
        assert!(
            across < 1e-3,
            "a part on a slider went ACROSS its own axis by {across:.4} (of {len:.4} in all, {along:.4} along) — so it was led anywhere rather than along the degree"
        );
    }

    /// A GROUNDED PART IS NOT LED: it is the point of reference itself.
    #[test]
    fn a_grounded_part_does_not_follow_the_hand() {
        let mut app = App::default();
        let (_, moving) = a_slider_by_hand(&mut app);
        let stays: Vec<Id> = app.project.bodies.iter().map(|b| b.id).filter(|b| *b != moving).collect();
        let grounded = *stays.last().expect("the grounded part");
        let owner = app.project.body_owner(grounded).expect("the owner of the grounded part");
        assert!(app.project.is_grounded(owner), "GUARD: the part must be grounded, otherwise there is nothing to check");
        let m = app.project.world_transform(owner);
        let was = [m[3], m[7], m[11]];

        let took = drag_the_part(&mut app, grounded, egui::vec2(60.0, 0.0));
        let m = app.project.world_transform(owner);
        let went = ((m[3] - was[0]).powi(2) + (m[7] - was[1]).powi(2) + (m[11] - was[2]).powi(2)).sqrt();
        assert!(!took, "a grounded part was allowed to be grabbed — it must stand still");
        assert!(went < 1e-9, "a grounded part moved by {went:.6} — the ground does not hold");
    }
}
