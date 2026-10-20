//! A ROW OF THE TREE NAMES THE SIZE THE FEATURE HOLDS, on the path of the reports: a cylinder made by its button, a
//! thread of the standard pitch put on it by a click on its wall, and the words the tree then shows.
//!
//! Reported behaviour: a cylinder of 14 threaded with the standard pitch was listed as "Thread M14x1.75" - the pitch of
//! M12, which no M14 nut takes.
#[cfg(test)]
mod tests {
    use crate::gui::hand::Hand;
    use crate::gui::App;

    /// The size of a cylinder made by its button.
    struct Cylinder {
        r: f64,
        h: f64,
    }

    /// A new document with its part entered, a cylinder of `size` made by its button, and a thread of the standard pitch
    /// put on it by a click on its wall. Answers the rows of the tree as the window draws them.
    fn threaded_cylinder(size: Cylinder) -> Vec<String> {
        let mut app = App { project: qymcad_core::model::Project::default(), ..Default::default() };
        app.project.new_document();
        let part = app.project.add_part("Part");
        app.enter_component(part);
        let mut hand = Hand::new(&mut app);
        assert!(hand.press_hint(&crate::i18n::tr("tb-cylinder-hint")), "GUARD: the panel has the button of the cylinder");
        hand.set("r", size.r).set("h", size.h).enter().settle();
        // THE WALL A THIRD OF A TURN ASIDE from the point facing the eye: that point lies on the screen over the axis of
        // the part, and a click there takes the axis
        let (_, _, fwd) = hand.app.viewing.cam.basis();
        let aside = (-fwd[1]).atan2(-fwd[0]) - std::f64::consts::FRAC_PI_3;
        let wall = [size.r * aside.cos(), size.r * aside.sin(), size.h / 2.0];
        hand.look_at([0.0, 0.0, size.h / 2.0], 300.0 / size.h as f32).tool(24);
        hand.click(wall).set("pitch", 0.0).enter().settle();
        let threaded = hand.app.project.timeline.iter().any(|n| matches!(n.kind, qymcad_core::feature::FeatureKind::Thread { .. }));
        assert!(threaded, "GUARD: a click on the wall of the cylinder and Enter make a thread; status: {}", hand.app.status);
        hand.frame(Vec::new());
        hand.words_drawn().iter().map(|(t, _)| t.clone()).collect()
    }

    /// M14 is a second-choice size of ISO 261, coarse pitch 2. The row of its thread carries that pitch.
    #[test]
    fn a_thread_of_a_second_choice_size_is_named_with_its_own_pitch() {
        let rows = threaded_cylinder(Cylinder { r: 7.0, h: 30.0 });
        let thread_rows: Vec<&String> = rows.iter().filter(|t| t.contains("M14")).collect();
        assert!(thread_rows.iter().any(|t| t.contains("M14×2")), "the tree names the thread of 14 with the pitch of ISO 261, 2; it shows {thread_rows:?}");
    }
}
