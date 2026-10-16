//! A FILLET WHOSE EDGES WERE ALL LOST, OPENED TO BE EDITED, WAITS FOR EDGES TO BE PICKED.
//!
//! Reported behaviour: a red rounding opened from the tree picked nothing, and its preview lit every edge of the part
//! green. An edit was taken as ready whatever was picked, the empty pick went to the node, and an empty list of edges is
//! "every edge" to the kernel: Enter would have rounded the whole part.
#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use super::super::App;
    use qymcad_core::feature::FeatureKind;

    #[test]
    fn a_fillet_with_its_edges_lost_waits_for_edges_to_be_picked() {
        let mut app = App::default();
        let mut hand = Hand::new(&mut app);
        // a plate 60 x 40 x 12 drawn and extruded by hand, one top edge rounded R1
        assert!(hand.press_word(&qymcad_i18n::tr("plane-xy-table"), egui::pos2(0.0, 0.0)), "setup: a sketch is started on XY");
        hand.sk_tool(2).click2d(0.0, 0.0).click2d(60.0, 40.0);
        assert!(hand.press_word(&qymcad_i18n::tr("wb-finish"), egui::pos2(0.0, 0.0)), "setup: the sketch is finished");
        hand.close_window();
        hand.look_at([30.0, 20.0, 6.0], 8.0).tool(1).set("height", 12.0).enter();
        hand.tool(4).set("radius", 1.0).click([30.0, 0.0, 12.0]);
        assert_eq!(hand.app.tools.gsel.edges.len(), 1, "setup: one top edge is picked; the program says {:?}", hand.app.status);
        hand.enter();
        let (ti, node) = hand.app.project.timeline.iter().enumerate().find(|(_, n)| matches!(n.kind, FeatureKind::Fillet { .. })).map(|(i, n)| (i, n.id)).expect("setup: the fillet");
        // its edge lost: a name no edge of the plate carries any more, as after an edit above renamed it
        let lost = qymcad_core::refs::Ref::picks(&[0x6000_ffff]);
        if let FeatureKind::Fillet { edges, .. } = &mut hand.app.project.timeline[ti].kind {
            *edges = lost.clone();
        }
        hand.app.project.timeline[ti].dirty = true;
        qymcad_ui_state::regenerate_now(&mut hand.app.rebuild_ctx());
        assert!(hand.app.project.regen_errors.contains_key(&node), "setup: the fillet with its edge lost is red");

        let label = crate::gui::panels_tree::feature_row_label(&hand.app.project, ti);
        assert!(hand.double_click_word(&label), "the tree shows the fillet as {label:?}");
        assert!(hand.app.tools.cmd.edit.is_some(), "the red fillet did not open to be edited; the program says {:?}", hand.app.status);
        assert!(hand.app.tools.gsel.edges.is_empty(), "GUARD: the fillet opened with edges picked: {:?}", hand.app.tools.gsel.edges);
        assert!(!qymcad_part::cmd_ready(&mut hand.app.part_ctx()), "the fillet with no edge picked is ready to apply: its preview rounds every edge of the part");
        hand.enter();
        let edges = hand.app.project.timeline.iter().find(|n| n.id == node).and_then(|n| match &n.kind {
            FeatureKind::Fillet { edges, .. } => Some(edges.clone()),
            _ => None,
        });
        assert_eq!(edges, Some(lost), "Enter with no edge picked wrote its empty pick into the fillet: every edge of the part rounded");
    }
}
