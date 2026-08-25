//! RUNNING A DEGREE OF FREEDOM — A MECHANISM IS CHECKED BY EYE.
//!
//! Once it is assembled, run it and watch how it moves. Numbers do not show that, and dragging a part
//! with the mouse to make sure it reaches the end and does not fall through its neighbour is guessing,
//! not checking.
//!
//! A RUN IS A VIEWING, NOT AN EDIT. Leaving the part wherever the stop caught it would mean silently
//! changing the document by pressing a "watch" button — and checked here is that this does not
//! happen.
#[cfg(test)]
mod tests {
    use super::super::App;
    use qymcad_core::feature::apply12;
    use qymcad_core::model::Id;

    fn viewport() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 700.0))
    }

    /// The turn of the part about the world Z, in degrees.
    fn spin(app: &App, comp: Id) -> f64 {
        let m = app.project.world_transform(comp);
        let o = apply12(&m, [0.0, 0.0, 0.0]);
        let x = apply12(&m, [1.0, 0.0, 0.0]);
        (x[1] - o[1]).atan2(x[0] - o[0]).to_degrees()
    }

    /// THE RUN WALKS THE PART THROUGH ITS RANGE AND TURNS BACK AT THE END.
    #[test]
    fn animating_a_hinge_walks_the_part_through_its_range_and_turns_back_at_the_end() {
        let mut app = App::default();
        let ([hinge, _], [wheel, _]) = super::super::a_relation_is_made_by_hand::tests::two_hinges(&mut app);
        // set the limits: the run must go EXACTLY between them and not around an invented circle
        if let Some(j) = app.project.joints.iter_mut().find(|j| j.id == hinge) {
            j.limit_min[0] = Some(0.0);
            j.limit_max[0] = Some(40.0);
        }
        assert!(app.start_joint_anim_for_test(hinge, 0), "the run of the angle must start: the limits are set");

        // step through the frames and watch THE TURN OF THE PART, not the field of the joint
        let mut seen: Vec<f64> = Vec::new();
        for _ in 0..40 {
            app.step_joint_anim_for_test(0.1); // 0.1 s per frame — twenty steps for the way there
            seen.push(spin(&app, wheel));
        }
        assert_eq!(seen.len(), 40, "GUARD: there should be forty steps, and it came out {}", seen.len());
        let hi = seen.iter().cloned().fold(f64::MIN, f64::max);
        let lo = seen.iter().cloned().fold(f64::MAX, f64::min);
        assert!((hi - 40.0).abs() < 0.5, "the part must reach the upper limit of 40 deg, and it reached {hi:.3} deg");
        assert!(lo > -0.5 && lo < 2.5, "the part must not go below the lower limit of 0 deg, and it went to {lo:.3} deg");
        // IT TURNED BACK AT THE END rather than jumping: somewhere in the middle the travel
        // changed sign
        let up = seen.windows(2).filter(|w| w[1] > w[0] + 1e-9).count();
        let down = seen.windows(2).filter(|w| w[1] < w[0] - 1e-9).count();
        assert!(up > 5 && down > 5, "the travel must go there AND back, and it came out {up} up, {down} down");
        // and not one jerk: a jump from end to end would look like twitching, not motion
        let jump = seen.windows(2).map(|w| (w[1] - w[0]).abs()).fold(0.0f64, f64::max);
        assert!(jump < 5.0, "the travel must be smooth, and the largest step of {jump:.3} deg is a jerk");
    }

    /// STOPPING PUTS EVERYTHING BACK: a run is a viewing, not an edit of the document.
    #[test]
    fn stopping_the_animation_puts_everything_back_the_way_it_was() {
        let mut app = App::default();
        let ([hinge, _], [wheel, _]) = super::super::a_relation_is_made_by_hand::tests::two_hinges(&mut app);
        if let Some(j) = app.project.joints.iter_mut().find(|j| j.id == hinge) {
            j.limit_min[0] = Some(0.0);
            j.limit_max[0] = Some(90.0);
            j.drive[0] = Some(15.0); // a person has already set the angle — that is what must come back
        }
        app.project.solve_joints();
        let before = spin(&app, wheel);
        assert!((before - 15.0).abs() < 1e-2, "setup: the part must stand at the given 15 deg, and it stands at {before:.4}");

        app.start_joint_anim_for_test(hinge, 0);
        for _ in 0..7 {
            app.step_joint_anim_for_test(0.1);
        }
        assert!((spin(&app, wheel) - before).abs() > 5.0, "setup: in seven frames the part must move noticeably");

        app.stop_joint_anim_for_test();
        assert!(!app.joint_anim_active_for_test(), "the run must stop");
        let after = spin(&app, wheel);
        assert!((after - before).abs() < 1e-2, "after the stop the part must come back to {before:.4} deg, and it stands at {after:.4} deg");
        let drive = app.project.joints.iter().find(|j| j.id == hinge).and_then(|j| j.drive[0]);
        assert_eq!(drive, Some(15.0), "the value a person set must come back, and it became {drive:?}");
    }

    /// A DEGREE WITH NOWHERE TO GO DOES NOT PRETEND TO BE RUNNING.
    ///
    /// A translation without limits has no natural end, and running it to a hundred millimetres would
    /// be a number out of thin air: the part would travel who knows where.
    #[test]
    fn a_degree_with_nowhere_to_go_refuses_instead_of_pretending() {
        let mut app = App::default();
        let ([hinge, _], _) = super::super::a_relation_is_made_by_hand::tests::two_hinges(&mut app);
        // a hinge has no travel at all
        assert!(!app.start_joint_anim_for_test(hinge, 1), "a hinge has no travel — the run must not start");
        assert!(!app.joint_anim_active_for_test(), "a run that never started must not count as running");
    }

    /// THE RUN BUTTON IS THERE IN THE JOINT POPUP.
    ///
    /// An ability with no button does not exist for a person.
    #[test]
    fn the_animate_button_is_there_in_the_joint_popup() {
        let mut app = App::default();
        let ([hinge, _], _) = super::super::a_relation_is_made_by_hand::tests::two_hinges(&mut app);
        if let Some(j) = app.project.joints.iter_mut().find(|j| j.id == hinge) {
            j.limit_min[0] = Some(0.0);
            j.limit_max[0] = Some(90.0);
        }
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        app.workbench = super::super::Workbench::Assembly;
        app.mode_3d = true;
        app.joint.edit = Some(hinge);
        let mut texts = Vec::new();
        // an egui popup settles on the SECOND frame — draw it twice
        for _ in 0..2 {
            let out = ctx.run(egui::RawInput { screen_rect: Some(viewport()), ..Default::default() }, |c| {
                app.joint_popup_for_test(c, viewport());
            });
            texts.clear();
            for cs in &out.shapes {
                super::super::screen_keys::tests::collect_text(&cs.shape, &mut texts);
            }
        }
        let want = crate::i18n::tr("j-anim-angle");
        assert!(texts.iter().any(|t| t.contains(&want)), "the joint popup has no \"{want}\" button: drawn {texts:?}");
    }
}
