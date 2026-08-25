//! THE ASSEMBLY WORKBENCH DOES NOT SHOW INTERNAL KEYS INSTEAD OF WORDS.
//!
//! A guard against raw catalogue keys exists in the program (`screen_keys`), but its list of surfaces
//! is another hand-written enumeration, and the bars of the assembly workbench are NOT IN IT AT ALL.
//! They could not be checked there: that guard's scene is a single plate with no joints, the bars come
//! out empty, and damage with a raw key does not make it fail. A check that cannot be made to fail
//! checks nothing.
//!
//! So there is a scene OF ITS OWN here — a real assembly with joints — and every bar is painted WITH
//! ITS OWN tool in hand. A key such as `j-kind` reaching the screen instead of a word means one thing:
//! the string was left untranslated and a person is seeing an internal name of the program.
#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use super::super::App;
    use qymcad_core::feature::JointKind;
    use qymcad_core::model::Id;

    fn viewport() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1100.0, 800.0))
    }

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

    /// AN ASSEMBLY WITH A JOINT: without one, the edit and relation bars are not painted at all.
    fn an_assembly(app: &mut App) -> Id {
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
        hand.look_at([30.0, 10.0, 5.0], 6.0).mate(JointKind::Revolute).anchor(3).click(pa).click(pb);
        app.rebuild_if_dirty();
        app.project.joints.last().map(|j| j.id).expect("the joint was created")
    }

    #[test]
    fn no_catalogue_key_reaches_the_assembly_screen() {
        let keys = super::super::screen_keys::tests::catalogue_keys();
        assert!(keys.len() > 500, "GUARD: suspiciously few catalogue keys were collected: {}", keys.len());

        type Surface = (&'static str, fn(&mut App));
        let surfaces: &[Surface] = &[
            ("the mate bar", |a: &mut App| a.arm_joint_pick_for_test()),
            ("the anchor bar", |a: &mut App| a.start_conn_pick()),
            ("the group bar", |a: &mut App| a.start_group_pick()),
            ("the width bar", |a: &mut App| a.start_width_pick()),
            ("the tangency bar", |a: &mut App| a.start_tangent_pick()),
            ("the relation bar", |a: &mut App| a.start_relation_pick()),
            ("the ground bar", |a: &mut App| a.start_ground_pick()),
        ];

        let prev = crate::i18n::language();
        let mut leaks: Vec<String> = Vec::new();
        let mut drawn = 0usize;
        for code in ["ru", "en"] {
            crate::i18n::set_language(code);
            for (name, arm) in surfaces {
                let mut app = App::default();
                let jid = an_assembly(&mut app);
                app.workbench = super::super::Workbench::Assembly;
                app.mode_3d = true;
                app.joint.edit = Some(jid);
                arm(&mut app);

                let ctx = egui::Context::default();
                super::super::install_fonts(&ctx);
                let mut texts: Vec<String> = Vec::new();
                for _ in 0..2 {
                    let out = ctx.run(egui::RawInput { screen_rect: Some(viewport()), ..Default::default() }, |c| {
                        app.joint_tool_bar_for_test(c);
                        app.joint_popup_for_test(c, viewport());
                        egui::SidePanel::right("props").show(c, |ui| app.joints_panel_for_test(ui));
                    });
                    texts.clear();
                    for cs in &out.shapes {
                        super::super::screen_keys::tests::collect_text(&cs.shape, &mut texts);
                    }
                }
                // GUARD AGAINST A VACUOUS CHECK: the bar really was painted, otherwise silence means
                // nothing — that is exactly how the general guard "passed" with a scene that had no
                // joints.
                assert!(texts.len() > 3, "GUARD: {name} ({code}) painted {} strings, so there is nothing to look at", texts.len());
                drawn += texts.len();
                for t in texts {
                    if keys.iter().any(|k| k == t.trim()) {
                        leaks.push(format!("{name} ({code}): the internal name \"{t}\" instead of words"));
                    }
                }
            }
        }
        crate::i18n::set_language(&prev);
        assert!(drawn > 100, "GUARD: {drawn} strings were painted in total, so the screen is suspiciously empty");
        assert!(leaks.is_empty(), "the interface showed internal names ({}):\n{}", leaks.len(), leaks.join("\n"));
    }
}
