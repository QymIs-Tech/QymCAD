//! THE WORKBENCH DOES NOT COMPLAIN ABOUT ITSELF IN THE VIEWPORT.
//!
//! Reported behaviour: the joint popup showed in red "First use of widget ID... / Second use of
//! widget ID D898...". That is how egui reports that two different widgets took ONE id: it paints the
//! complaint over the interface, and one of the fields stops responding. That trouble was found by
//! eye — 771 checks missed it, because all of them looked at numbers rather than at the frame.
//!
//! This guard is a general one: the real assembly screen is painted — the right-hand panel AND the
//! joint popup at once (which is exactly how they collided) — and then the PAINTED text is searched
//! for egui's words about widget ids. Let the next such trouble fail here rather than in a person's
//! face.
#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use super::super::App;
    use qymcad_core::feature::{JointKind, RelationKind};
    use qymcad_core::model::Id;

    fn viewport() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1100.0, 800.0))
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

    /// AN ASSEMBLY WITH ALL THREE KINDS: two joints, a group and a relation between the joints.
    fn a_document_with_everything(app: &mut App) -> Id {
        let before: Vec<Id> = app.project.bodies.iter().map(|b| b.id).collect();
        for x in [0.0, 60.0, 200.0, 260.0] {
            super::super::joint_flow::tests::add_part_at(app, x);
        }
        let root = app.project.root;
        app.enter_component(root);
        app.rebuild_if_dirty();
        app.refresh_edges();
        let mine: Vec<Id> = app.project.bodies.iter().map(|b| b.id).filter(|b| !before.contains(b)).collect();
        assert_eq!(mine.len(), 4, "setup: there should be four bodies of our own, and there are {}", mine.len());
        for (k, b) in mine.iter().enumerate() {
            if let Some(o) = app.project.body_owner(*b) {
                if let Some(i) = app.project.component_index(o) {
                    let x = [0.0, 60.0, 200.0, 260.0][k];
                    app.project.components[i].transform = [1.0, 0.0, 0.0, x, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0];
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
        let first = app.project.joints.last().map(|j| j.id).expect("the first joint");
        let (pc, pd) = (aim(app, mine[2]), aim(app, mine[3]));
        let mut hand = Hand::new(app);
        hand.look_at([230.0, 10.0, 5.0], 6.0).mate(JointKind::Revolute).anchor(3).click(pc).click(pd);
        let second = app.project.joints.last().map(|j| j.id).expect("the second joint");
        let members: Vec<Id> = mine.iter().filter_map(|b| app.project.body_owner(*b)).collect();
        app.project.add_group(&members[..2]);
        app.start_relation_pick_for_test();
        app.relation_pick_set_for_test(RelationKind::Gear, 2.0);
        app.relation_pick_click_for_test(first);
        app.relation_pick_click_for_test(second);
        app.relation_pick_confirm_for_test();
        app.rebuild_if_dirty();
        first
    }

    #[test]
    fn the_assembly_screen_never_paints_egui_id_complaints() {
        let mut app = App::default();
        let jid = a_document_with_everything(&mut app);
        // THE JOINT POPUP IS OPEN AT THE SAME TIME AS THE PANEL — exactly the arrangement in which
        // two fields took one id.
        app.joint.edit = Some(jid);
        app.sel = super::super::Sel::Joint(jid);
        app.workbench = super::super::Workbench::Assembly;
        app.mode_3d = true;

        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        let mut texts: Vec<String> = Vec::new();
        for _ in 0..3 {
            let out = ctx.run(egui::RawInput { screen_rect: Some(viewport()), ..Default::default() }, |c| {
                // THE TREE AND THE PANEL AND THE POPUP TOGETHER, as a person sees them: the id
                // collision is born precisely from simultaneity; each piece alone is flawless.
                egui::SidePanel::left("tree").show(c, |ui| app.build_tree_for_test(ui));
                egui::SidePanel::right("props").show(c, |ui| app.joints_panel_for_test(ui));
                app.joint_popup_for_test(c, viewport());
            });
            texts.clear();
            for cs in &out.shapes {
                super::super::screen_keys::tests::collect_text(&cs.shape, &mut texts);
            }
        }

        assert!(texts.len() > 5, "GUARD: the screen painted suspiciously few strings ({}), so there is nothing to look at", texts.len());
        // egui always writes its complaint with the words: widget ID.
        let complaints: Vec<&String> = texts.iter().filter(|t| t.contains("widget ID") || t.contains("Widget ID")).collect();
        assert!(
            complaints.is_empty(),
            "the assembly screen complains about itself in the viewport: two widgets took one id and one of them stopped responding:\n{complaints:?}"
        );
    }
}
