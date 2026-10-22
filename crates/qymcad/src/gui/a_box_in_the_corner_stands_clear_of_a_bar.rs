//! A BOX IN THE CORNER OF THE VIEW STANDS CLEAR OF A BAR LAID OVER THE VIEW: the bar of tools laid along the top of the
//! view, seen through, and a tool whose box has nothing to stand by - a pattern of a sketch - opens it under the bar.
//! Reported behaviour: the box of a pattern stood over the buttons of the tools laid along the top, in every workbench.
#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use super::super::{App, Sel};
    use qymcad_core::feature::SketchPlane;

    #[test]
    fn the_box_of_a_pattern_opens_under_the_bar_of_tools_at_the_top() {
        use qymcad_shell::{Edge, Over, PanelPlace, Seen};
        let mut app = App::default();
        app.set.layout.insert("wbtools".into(), PanelPlace { edge: Edge::Top, strip: 0, order: 0, across: None, along: None, seen: Seen::Shown, over: Over::Over });
        let si = app.create_sketch_on(SketchPlane::default());
        app.chosen.sel = Sel::Sketch(si);
        let mut hand = Hand::new(&mut app);
        hand.sk_tool(qymcad_ui_state::SketchShape::Line).click2d(0.0, 0.0).click2d(20.0, 0.0).key(egui::Key::Escape).key(egui::Key::Escape);
        hand.sk_select();
        hand.click2d(10.0, 0.0);
        assert!(hand.press_hint(&crate::i18n::tr("tb-lin-array-hint")), "the linear pattern");
        hand.frame(Vec::new());
        hand.frame(Vec::new());
        let bar = hand.laid_out("wbtools").expect("the bar of tools is laid out");
        let title = qymcad_i18n::tr1("box-direction-n", "n", "1");
        let at = hand.written_at(&title).unwrap_or_else(|| panic!("the box of the pattern names no {title:?}"));
        assert!(at.min.y > bar.max.y, "the box of the pattern stands over the bar of tools: its title at {at:?}, the bar at {bar:?}");
    }
}
