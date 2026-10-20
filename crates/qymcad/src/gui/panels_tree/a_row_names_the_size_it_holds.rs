//! A ROW OF THE TREE NAMES WHAT THE FEATURE HOLDS, on the paths of the reports: a cylinder made by its button, a thread
//! of the standard pitch put on it by a click on its wall; a box mirrored across a face of its own; and the words the
//! tree then shows.
//!
//! Reported behaviour: a cylinder of 14 threaded with the standard pitch was listed as "Thread M14x1.75" - the pitch of
//! M12, which no M14 nut takes; a mirror across a side face was listed as "Mirror [XY]".
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

    /// A cylinder of 2.5 and its M2.5 thread are named with the half they hold. Reported behaviour: the rows read
    /// "Cylinder Ø2×10" and "Thread M2×0.45 ext×10" - a rounded nominal names another standard size.
    #[test]
    fn a_size_with_a_fraction_is_named_with_it() {
        let rows = threaded_cylinder(Cylinder { r: 1.25, h: 10.0 });
        let named: Vec<&String> = rows.iter().filter(|t| t.contains(&crate::i18n::tr("cmd-cylinder")) || t.contains("Thread")).collect();
        let mut bad = Vec::new();
        for want in ["Cylinder Ø2.5×10", "Thread M2.5×0.45"] {
            if !named.iter().any(|t| t.contains(want)) {
                bad.push(want);
            }
        }
        assert!(bad.is_empty(), "the tree does not show {bad:?}; it shows {named:?}");
    }

    /// A box mirrored across a side face of its own is named by the face, not by a plane of the origin. Reported
    /// behaviour: the row read "Mirror [XY]" - mirroring across XY would have put the copy under the table.
    #[test]
    fn a_mirror_across_a_face_is_named_by_the_face() {
        let mut app = App { project: qymcad_core::model::Project::default(), ..Default::default() };
        app.project.new_document();
        let part = app.project.add_part("Part");
        app.enter_component(part);
        let mut hand = Hand::new(&mut app);
        assert!(hand.press_hint(&crate::i18n::tr("tb-box-hint")), "GUARD: the panel has the button of the box");
        hand.enter().settle();
        // the side of the box facing +X, which the eye sees: the box of 20 stands about the origin
        hand.look_at([0.0, 0.0, 10.0], 10.0).tool(16).click([10.0, 4.0, 13.0]).enter().settle();
        let mirror = hand.app.project.timeline.iter().position(|n| matches!(n.kind, qymcad_core::feature::FeatureKind::Mirror { face: Some(_), .. }));
        let Some(ti) = mirror else { panic!("GUARD: a click on the side face and Enter mirror the box across it; status: {}", hand.app.status) };
        let label = crate::gui::panels_tree::feature_row_label(&hand.app.project, ti);
        let want = crate::i18n::tr1("feat-mirror", "plane", &crate::i18n::tr("ref-face"));
        assert!(label.ends_with(&want), "the mirror across a face is named {label:?}, not {want:?}");
        hand.frame(Vec::new());
        assert!(hand.words_drawn().iter().any(|(t, _)| t.ends_with(&want)), "the tree does not show {want:?}");
    }
}
