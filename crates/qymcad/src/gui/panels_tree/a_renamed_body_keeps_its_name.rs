//! A BODY KEEPS THE NAME A PERSON GAVE IT when a feature is added to it, on the path of the report: a box made by its
//! button, its row in the tree pressed, a name typed into its properties, and a fillet put on an edge of its top.
//!
//! Reported behaviour: a body renamed "Housing" read "Body 2" in the tree after a fillet; an edit of an existing
//! feature kept the name.
#[cfg(test)]
mod tests {
    use crate::gui::hand::Hand;
    use crate::gui::App;

    #[test]
    fn a_renamed_body_keeps_its_name_through_a_fillet() {
        let mut app = App { project: qymcad_core::model::Project::default(), ..Default::default() };
        app.project.new_document();
        let part = app.project.add_part("Part");
        app.enter_component(part);
        let mut hand = Hand::new(&mut app);
        assert!(hand.press_hint(&crate::i18n::tr("tb-box-hint")), "GUARD: the panel has the button of the box");
        hand.enter().settle();
        let row = crate::i18n::name(&hand.app.project.mesh_name(0));
        assert!(hand.press_word(&row, egui::pos2(0.0, 0.0)), "GUARD: the tree has the row {row:?} of the body");
        assert!(hand.type_after_word(&crate::i18n::tr("pp-name"), "Housing"), "GUARD: the properties of the body have a field for its name");
        hand.key(egui::Key::Enter).settle();
        assert!(hand.shows("Housing"), "GUARD: the tree names the body as typed");
        let body = hand.app.project.mesh_id(0).expect("the body");
        let edge = hand.app.project.regen_edges[&body].iter().filter(|e| (e.a[2] - e.b[2]).abs() < 1e-6).max_by(|x, y| x.mid[2].total_cmp(&y.mid[2])).cloned().expect("an edge of the top");
        hand.look_at([0.0, 0.0, 10.0], 9.0).tool(qymcad_ui_state::PartTool::Fillet).click(edge.mid).set("radius", 2.0).enter().settle();
        let rounded = hand.app.project.timeline.iter().any(|n| matches!(n.kind, qymcad_core::feature::FeatureKind::Fillet { .. }));
        assert!(rounded, "GUARD: a click on an edge and Enter round it; status: {}", hand.app.status);
        let consumed = hand.app.project.consumed_bodies();
        let shown: Vec<String> = hand.app.project.bodies.iter().filter(|b| !consumed.contains(&b.id)).map(|b| crate::i18n::name(&b.name)).collect();
        assert_eq!(shown, ["Housing"], "the rounded body does not carry the name it was given");
        assert!(hand.shows("Housing"), "the tree does not show the name given to the body");
    }
}
