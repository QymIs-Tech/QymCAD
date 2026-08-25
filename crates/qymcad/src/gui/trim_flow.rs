//! TRIMMING A SURFACE — the wiring.
//!
//! The kernel and the timeline are checked separately; checked here is that what was picked reaches
//! the node and that a person can see WHAT they picked. The selection is put in directly: a face copy
//! lies ON the face of the part, and a real pick at that spot is ambiguous — picking a sheet itself is
//! checked with the stitching, and what is checked here is the path to the timeline.
#[cfg(test)]
mod tests {
    use super::super::App;

    /// The bounding box of the body mesh: [xmin,ymin,zmin,xmax,ymax,zmax].
    fn bbox(app: &App, body: u64) -> [f64; 6] {
        let mi = app.project.mesh_index(body).expect("the mesh of the body");
        let mut bb = [f64::MAX, f64::MAX, f64::MAX, f64::MIN, f64::MIN, f64::MIN];
        for v in &app.project.bodies[mi].mesh.verts {
            for (k, c) in [v.x, v.y, v.z].into_iter().enumerate() {
                bb[k] = bb[k].min(c);
                bb[k + 3] = bb[k + 3].max(c);
            }
        }
        bb
    }

    /// A plate, a sheet copied from its top and a box tool across it. Returns (app, sheet, tool).
    fn plate_sheet_tool() -> (App, u64, u64) {
        let mut app = App::default();
        super::super::joint_flow::tests::add_part_at(&mut app, 0.0);
        app.rebuild_if_dirty();
        let body = app.project.mesh_id(0).expect("the body");
        if let Some(owner) = app.project.body_owner(body) {
            app.enter_component(owner);
        }
        let top = app.project.regen_faces[&body].iter().find(|f| f.normal[2] > 0.9).map(|f| f.id).expect("the top");
        let sheet = app.project.add_face_copy(body, qymcad_core::refs::Ref::one(top, qymcad_core::refs::Fingerprint::default()));
        app.rebuild_if_dirty();

        // the tool: a box covering half the sheet
        let bb = bbox(&app, sheet);
        let mid = (bb[0] + bb[3]) * 0.5;
        let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        app.project.add_rect_entity(si, mid, bb[1] - 10.0, bb[3] + 10.0, bb[4] + 10.0, qymcad_core::feature::Purpose::Real);
        app.project.regen_sketch(si);
        app.finish_sketch_edit();
        let sid = app.project.sketches[si].id;
        let cid = app.project.sketches[si].contour_ids.iter().copied().find(|c| app.project.contour_profile_xy(*c).is_some()).expect("the contour of the tool");
        let tool = app.project.add_extrude_multi(sid, vec![cid], bb[5] + 20.0, qymcad_core::feature::Reach::Forward, 0.0, vec![]);
        app.rebuild_if_dirty();
        (app, sheet, tool)
    }

    /// The button exists.
    #[test]
    fn the_tool_has_a_button() {
        assert!(crate::gui::panels_source::PANELS.contains("self.start_feat_cmd(34)"), "without a button the tool does not exist for a person");
    }

    /// THE WHOLE PATH TO THE TIMELINE: what is cut + where to keep + what cuts -> a node with that
    /// very point.
    #[test]
    fn what_is_picked_reaches_the_timeline() {
        let (mut app, sheet, tool) = plate_sheet_tool();
        app.start_feat_cmd(34);
        assert_eq!(app.cmd.kind, 34, "the command must open");

        let bb = bbox(&app, sheet);
        let keep = [bb[0] + 1.0, (bb[1] + bb[4]) * 0.5, bb[5]]; // by the left edge — the part that stays
        app.trim.keep = Some((sheet, keep));
        app.trim.tool = Some(tool);
        app.apply_feat_cmd();
        app.rebuild_if_dirty();

        let node = app
            .project
            .timeline
            .iter()
            .find_map(|n| match n.kind {
                qymcad_core::feature::FeatureKind::Trim { src, tool, keep, body } => Some((n.id, src, tool, keep, body)),
                _ => None,
            })
            .expect("a trim must appear in the timeline");
        assert_eq!(node.1, sheet, "the sheet cut is THE one that was clicked");
        assert_eq!(node.2, tool, "and by THE body that was chosen as the tool");
        assert!((node.3[0] - keep[0]).abs() < 1e-9, "the \"keep\" point must reach the node as it is: {:?} instead of {keep:?}", node.3);
        assert!(!app.project.regen_errors.contains_key(&node.0), "the trim must build: {:?}", app.project.regen_errors.get(&node.0));
        assert!(app.trim.keep.is_none() && app.trim.tool.is_none(), "after applying, the selection must be cleared — otherwise it leaks into the next command");
    }

    /// WITHOUT ONE OF THE TWO INPUTS — A NAMED REFUSAL rather than silent nothing.
    #[test]
    fn a_missing_input_is_refused_with_a_message() {
        let (mut app, sheet, _tool) = plate_sheet_tool();
        let before = app.project.timeline.len();
        app.start_feat_cmd(34);
        app.apply_feat_cmd();
        assert_eq!(app.status, crate::i18n::tr("msg-trim-pick-sheet"), "without a surface the reason must be said out loud");

        app.trim.keep = Some((sheet, [0.0, 0.0, 0.0]));
        app.apply_feat_cmd();
        assert_eq!(app.status, crate::i18n::tr("msg-trim-pick-tool"), "without a tool it is a reason too, not silence");
        assert_eq!(app.project.timeline.len(), before, "the feature must not be created without both inputs");
    }

    /// THE TOOL SHOWS WHAT IS PICKED AND WHERE IT WAS CLICKED.
    #[test]
    fn the_tool_shows_what_is_picked() {
        let src = crate::gui::render_source::RENDER;
        let a = src.find("} else if self.cmd.kind == 34 {").expect("the trim must have a drawing block of its own");
        let b = src[a..].find("} else if self.cmd.kind == 33 {").map(|i| a + i).unwrap_or(src.len());
        let blk = &src[a..b];
        assert!(blk.contains("egui::Mesh::default()"), "the picked bodies must be highlighted with a fill");
        assert!(blk.contains("circle_filled"), "the place of the click must be visible: it is what decides which piece stays");
        assert!(include_str!("pick.rs").contains("if self.cmd.kind == 34 {"), "a click must pick something");
    }

    /// The strings of the tool are translated.
    #[test]
    fn the_tool_speaks_the_users_language() {
        for (code, _) in crate::i18n::available() {
            let prev = crate::i18n::language();
            crate::i18n::set_language(&code);
            for key in ["f-trim", "tb-trim-surface-hint", "msg-trim", "msg-trim-pick-tool", "hint-trim", "hint-trim-tool", "feat-name-trim", "error-op-failed-trim"] {
                let s = crate::i18n::tr(key);
                assert!(!s.is_empty() && s != key, "language {code} has no translation for {key}");
            }
            crate::i18n::set_language(&prev);
        }
    }
}
