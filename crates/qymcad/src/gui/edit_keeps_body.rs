//! EDITING A FEATURE DOES NOT TAKE THE PART OFF THE SCREEN.
//!
//! Reported behaviour: entering a part, choosing to edit a Fillet, a rebuild window appearing and
//! standing for a long time, then vanishing — with no part at all left. Edit -> Rebuild Everything
//! brought the body back.
//!
//! That timeline held TWO fillets in a row and the first was being edited. While a modifier is edited
//! the program shows the state BEFORE it: the result of the edited feature and its whole descendant
//! chain are hidden, while the consumed SOURCE is shown. The second part did not work: the list of
//! visible bodies first discards EVERYTHING consumed (`body_shown`), and the source never survived as
//! far as the line that makes an exception for the edit source. That exception was dead code from the
//! start.
#[cfg(test)]
mod tests {
    use super::super::{App, Sel};

    /// A plate with TWO fillets in a row: a chain one can stand in the middle of.
    fn plate_with_two_fillets() -> (App, u64, u64) {
        let mut app = super::super::screen_keys::tests::plate();
        let body = app.project.timeline.iter().rev().find_map(|n| n.kind.body()).expect("the body of the plate");
        let edges: Vec<u32> = app.project.regen_edges.get(&body).map(|es| es.iter().take(4).map(|e| e.id).collect()).unwrap_or_default();
        assert!(!edges.is_empty(), "setup: the plate must have edges");
        let f1 = app.project.add_fillet(body, 1.0, edges);
        app.rebuild_if_dirty();
        let b1 = app.project.timeline.iter().rev().find_map(|n| n.kind.body()).expect("the body after the first fillet");
        let e2: Vec<u32> = app.project.regen_edges.get(&b1).map(|es| es.iter().take(2).map(|e| e.id).collect()).unwrap_or_default();
        let f2 = app.project.add_fillet(b1, 0.5, e2);
        app.rebuild_if_dirty();
        let ids: Vec<u64> = app.project.timeline.iter().filter(|n| matches!(n.kind, qymcad_core::feature::FeatureKind::Fillet { .. })).map(|n| n.id).collect();
        assert_eq!(ids.len(), 2, "setup: the timeline must hold two fillets");
        let _ = (f1, f2);
        (app, ids[0], ids[1])
    }

    /// THE MAIN POINT: while a fillet is being edited there is something on screen to edit.
    #[test]
    fn editing_a_fillet_leaves_the_part_on_screen() {
        for which in ["in the MIDDLE of the chain", "at the TOP of the chain"] {
            let (mut app, first, last) = plate_with_two_fillets();
            let fid = if which.contains("MIDDLE") { first } else { last };
            assert!(app.visible_mesh_items_for_test() > 0, "setup: before the edit the part is visible");
            app.sel = Sel::Feature(app.project.timeline.iter().position(|n| n.id == fid).expect("the node is there"));
            app.start_feat_cmd_edit(fid);
            let during = app.visible_mesh_items_for_test();
            assert!(during > 0, "editing {which}: EVERYTHING vanished from the screen, so there is nothing to edit and nothing to click");
            app.cancel_all_tools();
            app.rebuild_if_dirty();
            assert!(app.visible_mesh_items_for_test() > 0, "after cancelling the edit {which} the part must come back");
        }
    }

    /// AND AFTER APPLYING TOO. Leaving the edit left no part until Rebuild Everything.
    #[test]
    fn applying_an_edit_brings_the_part_back() {
        let (mut app, first, _) = plate_with_two_fillets();
        app.sel = Sel::Feature(app.project.timeline.iter().position(|n| n.id == first).expect("the node is there"));
        app.start_feat_cmd_edit(first);
        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "radius") {
            p.val = 1.5;
            p.txt = "1.5".into();
        }
        app.apply_feat_cmd();
        app.rebuild_if_dirty();
        app.drain_busy_for_test();
        assert!(app.visible_mesh_items_for_test() > 0, "after applying the edit the part must be on screen, not only after Rebuild Everything; status: {}", app.status_for_test());
    }
}
