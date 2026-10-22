//! A TOOL OF A SKETCH TAKEN OUTSIDE A SKETCH SAYS SO AND LEAVES THE VIEW AS IT WAS.
//!
//! Reported behaviour: "Radius / diameter" taken from the command search (Ctrl+K) in a part, with no sketch open, turned
//! the part view flat with no view cube, held a tool with no bar that swallowed every click, and Esc left the view flat.
#[cfg(test)]
mod tests {
    use crate::command_catalog::Named;
    use crate::gui::hand::Hand;
    use crate::gui::App;

    /// IN A PART, "RADIUS / DIAMETER" FROM THE SEARCH IS REFUSED IN WORDS: no tool lands in hand, the view stays 3D, and
    /// the status line says in the tone of a refusal that the tool works inside a sketch.
    #[test]
    fn a_sketch_dimension_from_the_search_outside_a_sketch_is_refused_in_words() {
        let mut app = App::default();
        crate::gui::joint_flow::tests::add_part_at(&mut app, 0.0);
        qymcad_ui_state::rebuild_if_dirty(&mut app.rebuild_ctx());
        let body = app.project.mesh_id(0).expect("the body");
        if let Some(owner) = app.project.body_owner(body) {
            app.enter_component(owner);
        }
        let name = crate::command_catalog::by_code("sketch.dim-radius").expect("the command of a radius dimension").name();
        let refused = crate::i18n::tr("sk-tool-outside-sketch");
        let pal = app.scheme.pal.clone();
        let mut hand = Hand::new(&mut app);
        hand.chord(egui::Modifiers::COMMAND, egui::Key::K).type_text(&name).key(egui::Key::Enter);
        assert_eq!(hand.app.tools.armed, qymcad_ui_state::Armed::None, "a tool of a sketch landed in hand with no sketch open");
        assert!(hand.app.viewing.mode_3d, "the view of the part turned flat");
        let said = hand.status_painted().into_iter().find(|w| w.text.contains(&refused));
        assert!(
            said.as_ref().is_some_and(|w| w.colour == qymcad_ui_state::tone_colour(&pal, qymcad_ui_state::Tone::Refusal)),
            "the status line does not say in the tone of a refusal {refused:?}: {:?}",
            hand.status_painted().into_iter().map(|w| (w.text, w.colour)).collect::<Vec<_>>()
        );
    }
}
