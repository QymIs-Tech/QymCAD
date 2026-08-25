//! THE SKETCH TOOL IS GONE FROM ASSEMBLIES — it was inert there.
//!
//! The button was labelled "skeleton": the intent of top-down layout was there. But there is nothing
//! to reference such a skeleton with — a mate anchor understands `Origin`, `BasePlane`, `FaceCenter`,
//! `EdgeMid` and `Vertex`, there is no reference to a sketch, and there is no extrude in the Assembly
//! workbench either. It could be drawn but not used.
//!
//! DATUMS stay in an assembly: a mirrored component copy and a view section both consume them.
#[cfg(test)]
mod tests {
    use super::super::{App, Workbench};

    /// In an assembly a sketch starts from neither a button nor a key; in a part it does.
    #[test]
    fn a_sketch_can_be_started_in_a_part_but_not_in_an_assembly() {
        let mut app = App::default();
        super::super::joint_flow::tests::add_part_at(&mut app, 0.0);
        app.rebuild_if_dirty();

        // THE ASSEMBLY: the sketch key turns nothing on
        let root = app.project.root;
        app.enter_component(root);
        app.workbench = Workbench::Assembly;
        app.cancel_all_tools();
        app.assembly_hotkey(egui::Key::K);
        assert!(
            !app.picking.is_sketch_plane(),
            "there is nothing to start a sketch with in an assembly: it could not be referenced anyway"
        );
        // a datum, however, can be started: it works there (mirror, section)
        app.cancel_all_tools();
        app.assembly_hotkey(egui::Key::D);
        assert_ne!(app.cmd.kind, 0, "a datum must stay in an assembly: the mirror and the section consume it");

        // THE PART: a sketch does start
        let body = app.project.mesh_id(0).expect("the body");
        let owner = app.project.body_owner(body).expect("the owner");
        app.enter_component(owner);
        app.workbench = Workbench::Part;
        app.cancel_all_tools();
        app.part_hotkey(egui::Key::K);
        assert!(app.picking.is_sketch_plane(), "in a part a sketch must start");
    }

    /// The sketch button exists only in the Part toolbar.
    #[test]
    fn the_sketch_button_lives_only_in_the_part_toolbar() {
        let src = crate::gui::panels_source::PANELS;
        let a = src.find("Workbench::Assembly =>").expect("the assembly workbench is there");
        let b = src[a..].find("Workbench::Cam").map(|i| a + i).unwrap_or(src.len());
        assert!(
            !src[a..b].contains("create_panel_sketch_button"),
            "the Assembly toolbar must have no sketch button: a sketch is inert there"
        );
        let p = src.find("Workbench::Part =>").expect("the part workbench is there");
        let q = src[p..].find("Workbench::Assembly =>").map(|i| p + i).unwrap_or(src.len());
        assert!(src[p..q].contains("create_panel_sketch_button"), "the Part toolbar must have the sketch button");
    }
}
