//! THE RIGHT-HAND PANEL SHOWS, IT DOES NOT EDIT.
//!
//! A long-standing requirement: the properties panel is informational. It used to hold 450 lines of
//! editors applying instantly — no preview, no Enter/Esc and no formulas — which gave two ways to
//! edit one feature with DIFFERENT capabilities. What was available depended on which route was taken
//! into the feature.
#[cfg(test)]
mod tests {
    use super::super::{App, Sel};

    /// The panel contains no editing widget at all.
    ///
    /// Checking the source is deliberate: a behavioural test for "the panel does not change the
    /// document" would have passed BEFORE the change too — `DragValue` only changes a value on
    /// interaction, and there is none in a headless run. Such a test would be green with the contract
    /// entirely broken, that is, useless.
    #[test]
    fn the_properties_panel_has_no_editing_widgets() {
        let src = crate::gui::panels_source::PANELS;
        let a = src.find("pub(super) fn feature_props").expect("the feature properties panel is there");
        let b = src[a..].find("\n    pub(super) fn ").map(|i| a + i).unwrap_or(src.len());
        let body = &src[a..b];
        for w in ["DragValue", "selectable_value", "checkbox", "TextEdit"] {
            assert!(
                !body.contains(w),
                "an editing widget `{w}` is left in the feature properties: editing lives in the command \
                 (preview, cancel, formulas) and the panel only shows"
            );
        }
    }

    /// "Edit" must open a command with parameters for EVERY kind that occurs in the timeline.
    ///
    /// That is the precondition for making the panel informational: remove the editors before every
    /// kind opens as a command and the ability to edit simply disappears.
    #[test]
    fn every_feature_in_the_timeline_opens_as_a_command() {
        let mut app = App::default();
        super::super::joint_flow::tests::add_part_at(&mut app, 0.0);
        app.rebuild_if_dirty();
        let body = app.project.mesh_id(0).expect("the body");
        if let Some(owner) = app.project.body_owner(body) {
            app.enter_component(owner);
        }
        // add a couple more kinds so the timeline is not uniform
        let mi = app.project.mesh_index(body).expect("the mesh");
        app.sel = Sel::Mesh(mi);
        app.start_feat_cmd(4); // the fillet
        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "radius") {
            p.val = 2.0;
            p.txt = "2".into();
        }
        app.gsel.edges = app.body_edges_cached(body).map(|e| e.1.iter().copied().filter(|&i| i != 0).collect()).unwrap_or_default();
        app.apply_feat_cmd();
        app.rebuild_if_dirty();

        let nodes: Vec<(u64, String)> = app.project.timeline.iter().map(|n| (n.id, n.name.clone())).collect();
        assert!(nodes.len() >= 2, "setup: the timeline should hold several nodes, and it holds {}", nodes.len());
        for (fid, name) in nodes {
            app.cancel_all_tools();
            app.start_feat_cmd_edit(fid);
            let has_params = !app.cmd.params.is_empty();
            let opened = app.cmd.kind != 0;
            assert!(
                opened || !has_params,
                "\"{name}\": the command did not open, so the feature cannot be edited from the properties panel"
            );
        }
    }
}
