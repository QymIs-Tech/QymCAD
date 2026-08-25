//! A LIMIT HOLDS AND SAYS SO.
//!
//! A limit means "no further": a hinge opens 110 degrees and not 360, a carriage runs along a rail and
//! does not slide off it. In grown-up CAD the stop is visible in the frame as a dashed line with
//! marks, and a joint with limits is marked in the list.
//!
//! What is checked here is THE MOST IMPORTANT thing, and the most galling one when it is missing: a
//! limit must HOLD (the part stands at the stop rather than sailing past it) and must SAY that it
//! stopped — silently correcting a number a person entered is indistinguishable from "the program
//! does not obey me".
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

    /// A SLIDER BETWEEN TWO PARTS, assembled BY HAND.
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

    /// A LIMIT HOLDS: the part does not travel past the stop.
    #[test]
    fn a_limit_stops_the_part_at_the_stop() {
        let mut app = App::default();
        let (jid, moving) = a_slider_by_hand(&mut app);
        let owner = app.project.body_owner(moving).expect("the owner of the driven part");
        let zero = app.project.joints.iter().find(|x| x.id == jid).map(|x| x.offset).unwrap_or(0.0);

        // THE STOP IS 20 mm FROM THE ZERO OF THE JOINT, and the drive goes to 40 — twice as far.
        if let Some(j) = app.project.joints.iter_mut().find(|x| x.id == jid) {
            j.limit_min[1] = Some(zero - 5.0);
            j.limit_max[1] = Some(zero + 20.0);
        }
        let was = app.project.world_transform(owner);
        if let Some(j) = app.project.joints.iter_mut().find(|x| x.id == jid) {
            j.drive[1] = Some(zero + 40.0);
        }
        app.project.solve_joints();
        let now = app.project.world_transform(owner);
        let went = [3usize, 7, 11].iter().map(|&k| (now[k] - was[k]).powi(2)).sum::<f64>().sqrt();
        assert!(
            (went - 20.0).abs() < 1e-3,
            "the stop is at 20 mm and the drive went to 40 — the part must stand at 20.000, and it travelled {went:.4}"
        );
        // AND THE READING MUST AGREE WITH THE STOP rather than stay at forty: otherwise the field
        // says one thing and the part stands somewhere else.
        let shown = app.project.joints.iter().find(|x| x.id == jid).map(|x| x.offset).unwrap_or(f64::NAN);
        assert!((shown - (zero + 20.0)).abs() < 1e-3, "the part stands at the stop and the field reads {shown:.4} instead of {:.4}", zero + 20.0);
    }

    /// A LIMIT SAYS THAT IT STOPPED.
    ///
    /// Silently correcting a number a person entered is the worst kind of obedience: from the outside
    /// it reads as "the program does not obey me". Grown-up CAD marks a joint with limits and shows
    /// the stops in the frame; checked here is at least that the fact of the stop is SAID IN WORDS.
    #[test]
    fn hitting_a_limit_is_said_out_loud() {
        let mut app = App::default();
        let (jid, _) = a_slider_by_hand(&mut app);
        let zero = app.project.joints.iter().find(|x| x.id == jid).map(|x| x.offset).unwrap_or(0.0);
        if let Some(j) = app.project.joints.iter_mut().find(|x| x.id == jid) {
            j.limit_max[1] = Some(zero + 20.0);
            j.drive[1] = Some(zero + 40.0);
        }
        app.project.solve_joints();

        let stopped = app.project.joints_at_limit();
        assert!(
            stopped.iter().any(|(id, slot)| *id == jid && *slot == 1),
            "the part stands at the stop and the program says nothing about it: what counts as stopped is {stopped:?}"
        );

        // AND IT IS VISIBLE IN THE FRAME rather than merely recorded in memory.
        let words = panel_words(&mut app);
        let want = crate::i18n::tr("jp-at-limit");
        assert!(
            words.iter().any(|t| t.contains(&want)),
            "the joint stands at the stop and the panel says not a word about it: drawn {words:?}"
        );
    }

    /// "GO TO THE LIMIT" PUTS THE PART EXACTLY ON THE STOP.
    ///
    /// A mechanism with limits is examined in its EXTREME positions — that is where it runs into its
    /// neighbour. Typing the boundary number by hand, and the very same one as in the limit field at
    /// that, is work the program knows better than a person.
    #[test]
    fn apply_limit_position_puts_the_part_exactly_on_the_stop() {
        let mut app = App::default();
        let (jid, moving) = a_slider_by_hand(&mut app);
        let owner = app.project.body_owner(moving).expect("the owner of the driven part");
        let zero = app.project.joints.iter().find(|x| x.id == jid).map(|x| x.offset).unwrap_or(0.0);
        if let Some(j) = app.project.joints.iter_mut().find(|x| x.id == jid) {
            j.limit_min[1] = Some(zero - 8.0);
            j.limit_max[1] = Some(zero + 17.0);
        }
        let was = app.project.world_transform(owner);

        assert!(app.project.apply_limit_position(jid, 1, true), "go-to-the-limit refused to work with a limit set");
        app.project.solve_joints();
        let now = app.project.world_transform(owner);
        let went = [3usize, 7, 11].iter().map(|&k| (now[k] - was[k]).powi(2)).sum::<f64>().sqrt();
        assert!((went - 17.0).abs() < 1e-3, "the upper stop is at 17 mm and the part travelled {went:.4}");

        assert!(app.project.apply_limit_position(jid, 1, false), "go-to-the-lower-limit refused to work");
        app.project.solve_joints();
        let now = app.project.world_transform(owner);
        let went = [3usize, 7, 11].iter().map(|&k| (now[k] - was[k]).powi(2)).sum::<f64>().sqrt();
        assert!((went - 8.0).abs() < 1e-3, "the lower stop is at -8 mm and the part went {went:.4}");
    }

    /// AND WHERE THERE IS NO LIMIT THERE IS NOWHERE TO GO. Inventing a boundary is worse than
    /// refusing.
    #[test]
    fn apply_limit_position_refuses_where_there_is_no_limit() {
        let mut app = App::default();
        let (jid, _) = a_slider_by_hand(&mut app);
        assert!(!app.project.apply_limit_position(jid, 1, true), "there is no limit and go-to-the-limit agreed — so it invented a boundary");
        let driven = app.project.joints.iter().find(|x| x.id == jid).and_then(|j| j.drive[1]);
        assert_eq!(driven, None, "having refused, it still drove something: {driven:?}");
    }

    fn viewport() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 700.0))
    }

    /// The words DRAWN by the mates panel.
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
}
