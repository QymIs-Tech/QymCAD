//! EXTRUDE IS ONE TOOL, NOT TWO.
//!
//! The "Extrude" and "Cut" buttons invoked THE SAME command and differed only in the preset of the
//! "Add / Cut / Intersect" switch that sits in the same bar above. On top of that they lived in
//! different categories ("Sketch to 3D" and "Body") and looked like different concepts. A mature CAD
//! has one button with a choice of operation.
#[cfg(test)]
mod tests {
    use super::super::App;

    /// The workbench toolbar offers exactly one extrude button.
    #[test]
    fn the_toolbar_offers_one_extrude_button() {
        let src = crate::gui::panels_source::PANELS;
        let a = src.find("pub(super) fn wb_toolbar").expect("the workbench toolbar is there");
        let b = src[a..].find("\n    pub(super) fn ").map(|i| a + i).unwrap_or(src.len());
        let n = src[a..b].matches("self.start_feat_cmd(1)").count();
        assert_eq!(
            n, 1,
            "extrude must be ONE button (the operation is chosen in the bar), and {n} buttons \
             open the same command"
        );
    }

    /// The keyboard shortcut for a cut is kept: the same command, a different operation preset.
    #[test]
    fn the_cut_shortcut_opens_extrude_preset_to_cut() {
        let mut app = App::default();
        super::super::joint_flow::tests::add_part_at(&mut app, 0.0);
        app.rebuild_if_dirty();
        let body = app.project.mesh_id(0).expect("the body");
        if let Some(owner) = app.project.body_owner(body) {
            app.enter_component(owner);
        }
        let si = app.project.sketches.iter().position(|s| !s.entities.is_empty()).expect("there is a sketch");
        app.sel = super::super::Sel::Sketch(si);
        app.part_hotkey(egui::Key::Q);
        assert_eq!(app.cmd.kind, 1, "\"cut\" must open the extrude command");
        assert_eq!(app.feat.op, 2, "...with the operation preset to Cut");
    }
}
