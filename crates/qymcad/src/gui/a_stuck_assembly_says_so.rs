//! THE ASSEMBLY IS STUCK — A PERSON MUST SEE THAT.
//!
//! The worst thing a CAD can do is silently do nothing. That is exactly what was reported: the joints
//! are placed, the parts stand still, and why is never said. While working on the relations it turned
//! out that in such cases the solve DOES NOT CONVERGE (`mates_conflict`), and by the rule about a
//! failed solve the parts stay where they are.
//!
//! Checked here is that this case IS SAID IN WORDS IN THE FRAME rather than merely marked by a flag in
//! memory. The check is live: something knowingly unsatisfiable is assembled, a real panel is drawn,
//! and a warning is looked for among its words.
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

    /// The words drawn by the mates panel.
    fn panel_words(app: &mut App) -> Vec<String> {
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        app.workbench = super::super::Workbench::Assembly;
        app.mode_3d = true;
        let mut texts = Vec::new();
        for _ in 0..2 {
            let out = ctx.run(egui::RawInput { screen_rect: Some(viewport()), ..Default::default() }, |c| {
                egui::CentralPanel::default().show(c, |ui| app.joints_panel_for_test(ui));
            });
            texts.clear();
            for cs in &out.shapes {
                super::super::screen_keys::tests::collect_text(&cs.shape, &mut texts);
            }
        }
        texts
    }

    /// TWO PARTS AND TWO ARGUING JOINTS BETWEEN THEM, placed by hand.
    ///
    /// The rigid one aligns the origins dead fast, while the slider with a SET travel demands the part
    /// be moved 25 mm away: together that is unsatisfiable, and the solve must fail to converge.
    ///
    /// The argument is taken over TRANSLATION on purpose. The first edition argued by rotation (rigid
    /// against a turn of 37 degrees) — and the trap guard showed there was no argument: with a "by
    /// origins" anchor the secondary axis is not derived from the geometry, the roll is undefined, and
    /// the rigid joint does not hold it at all. The rotation was legitimately free.
    fn two_mates_that_argue(app: &mut App) -> (Id, Id) {
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
        let (pa, pb) = (aim(app, mine[0]), aim(app, mine[1]));

        let mut hand = Hand::new(app);
        hand.look_at([30.0, 10.0, 5.0], 7.0).mate(JointKind::Rigid).anchor(3).click(pa).click(pb);
        let rigid = app.project.joints.last().map(|j| j.id).expect("the rigid joint");
        let mut hand = Hand::new(app);
        hand.mate(JointKind::Slider).anchor(3).click(pa).click(pb);
        let hinge = app.project.joints.last().map(|j| j.id).expect("the slider");
        // THE SLIDER HAS A TRAVEL SET — and the rigid joint holds the origins aligned: the two
        // cannot be satisfied together.
        if let Some(j) = app.project.joints.iter_mut().find(|x| x.id == hinge) {
            j.drive[1] = Some(25.0);
        }
        // AND SOLVE AGAIN. `rebuild_if_dirty` will not help here: a set value does not dirty the
        // document and there will be no rebuild — the solve has to be called. That caught this check
        // out twice: the trap guard said "the solve converged" while in fact there had been NO solve
        // at all, and the flag was left over from the previous time.
        app.project.solve_joints();
        (rigid, hinge)
    }

    /// A SICK JOINT FREEZES ONLY ITS OWN MECHANISM, NOT THE WHOLE DOCUMENT.
    ///
    /// The measurement it all started from: the scenario document held five joints and one sick one —
    /// and a slider in another corner of it travelled 0.000 mm instead of 15.000, a wheel 0.000 deg
    /// instead of 40.000. Remove ONE sick joint and the same mechanism moves. That is exactly what was
    /// hitting the reported machine: four dead joints in it, and NOTHING WORKED ANYWHERE.
    #[test]
    fn a_sick_mate_freezes_only_its_own_mechanism() {
        let mut app = App::default();
        // THE FIRST MECHANISM IS THE SICK ONE: the rigid joint holds the origins, the slider demands
        // a travel of 25 mm.
        let (_, _) = two_mates_that_argue(&mut app);
        assert!(app.project.mates_conflict, "GUARD: no argument came out — there is nothing to check");

        // THE SECOND MECHANISM IS HEALTHY and tied to the first by nothing.
        let before: Vec<Id> = app.project.bodies.iter().map(|b| b.id).collect();
        super::super::joint_flow::tests::add_part_at(&mut app, 300.0);
        super::super::joint_flow::tests::add_part_at(&mut app, 360.0);
        let root = app.project.root;
        app.enter_component(root);
        app.rebuild_if_dirty();
        app.refresh_edges();
        let mine: Vec<Id> = app.project.bodies.iter().map(|b| b.id).filter(|b| !before.contains(b)).collect();
        assert_eq!(mine.len(), 2, "setup: there should be two bodies of our own, and there are {}", mine.len());
        for (k, b) in mine.iter().enumerate() {
            if let Some(o) = app.project.body_owner(*b) {
                if let Some(i) = app.project.component_index(o) {
                    app.project.components[i].transform = [1.0, 0.0, 0.0, 300.0 + k as f64 * 60.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0];
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
        hand.look_at([330.0, 10.0, 5.0], 5.0).mate(JointKind::Slider).anchor(3).click(pa).click(pb);
        app.rebuild_if_dirty();
        let good = app.project.joints.last().map(|j| j.id).expect("the healthy joint was created");
        let moving = app.project.body_owner(mine[1]).expect("the owner of the driven part");

        // THE HEALTHY MECHANISM MUST MOVE, even though the document holds an argument.
        let was = app.project.world_transform(moving);
        let base = app.project.joints.iter().find(|x| x.id == good).map(|x| x.offset).unwrap_or(0.0);
        if let Some(j) = app.project.joints.iter_mut().find(|x| x.id == good) {
            j.drive[1] = Some(base + 15.0);
        }
        app.project.solve_joints();
        let now = app.project.world_transform(moving);
        let went = was.iter().zip(now.iter()).map(|(x, y)| (x - y).abs()).fold(0.0f64, f64::max);
        assert!(
            (went - 15.0).abs() < 1e-3,
            "a healthy mechanism must move despite an argument in another corner of the document: it travelled {went:.4} instead of 15"
        );
        assert!(app.project.mates_conflict, "and the argument must stay named rather than dissolve");
    }

    /// DURING AN ARGUMENT THE PANEL HAS NO RIGHT TO SAY "DEFINED".
    ///
    /// A snapshot of the frame revealed that BOTH arguing joints read "DOF of the driven part: 0 —
    /// defined" in green, right next to the red warning about the conflict. The number of degrees is
    /// counted over the stack of joints and never asks whether they are solvable together. For a
    /// person that is the worst possible answer: the program says "trouble" and "all is well" at the
    /// same time.
    #[test]
    fn a_part_held_by_arguing_mates_is_not_called_defined() {
        let mut app = App::default();
        let (rigid, hinge) = two_mates_that_argue(&mut app);
        assert!(app.project.mates_conflict, "GUARD: no argument came out — there is nothing to check");

        let words = panel_words(&mut app);
        let defined = crate::i18n::tr("jp-defined");
        let bad: Vec<&String> = words.iter().filter(|t| t.contains(&defined)).collect();
        assert!(
            bad.is_empty(),
            "joints {rigid} and {hinge} ARGUE and the panel calls the part defined: {bad:?}\neverything drawn: {words:?}"
        );
    }

    #[test]
    fn an_assembly_that_cannot_be_solved_says_so_in_words() {
        let mut app = App::default();
        let (rigid, hinge) = two_mates_that_argue(&mut app);
        assert_ne!(rigid, hinge, "setup: there should be two joints");

        // TRAP GUARD: the solve really did fail to converge — otherwise there is nothing to check.
        assert!(
            app.project.mates_conflict,
            "GUARD: there is no trap — the solve CONVERGED, so no argument came out and there is nothing to say words about"
        );

        let words = panel_words(&mut app);
        assert!(words.len() > 5, "GUARD: the panel drew suspiciously few lines ({})", words.len());
        let want = crate::i18n::tr("jp-conflict");
        let head = want.split(' ').take(2).collect::<Vec<_>>().join(" ");
        assert!(
            words.iter().any(|t| t.contains(&head)),
            "the assembly is stuck and not a word was said to the person: drawn {words:?}"
        );
    }
}
