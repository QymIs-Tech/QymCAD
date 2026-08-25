//! SPLIT A BODY — the whole path a person walks.
//!
//! What is checked is what the tool actually is to a person: the button opened the command, a click in
//! the viewport chose the cutting plane, the offset was accepted, Enter created TWO bodies, and both
//! are visible in the tree. The kernel and the timeline are checked separately
//! (`qymcad-testkit/tests/split_body_feature.rs`) — here it is the wiring, because that is exactly where
//! earlier tools turned out to be "done" yet unreachable.
#[cfg(test)]
mod tests {
    use super::super::{App, Sel};

    /// A cube inside a part, with that part entered; returns (the index of the mesh, the id of the body).
    fn part_with_cube(app: &mut App) -> (usize, u64) {
        super::super::joint_flow::tests::add_part_at(app, 0.0);
        app.rebuild_if_dirty();
        let body = app.project.mesh_id(0).expect("the body");
        if let Some(owner) = app.project.body_owner(body) {
            app.enter_component(owner);
        }
        let mi = app.project.mesh_index(body).expect("the mesh");
        app.sel = Sel::Mesh(mi);
        (mi, body)
    }

    /// The whole path: the button -> a click on the plane -> the offset -> Enter -> two bodies.
    #[test]
    fn a_body_can_be_split_from_the_toolbar() {
        let mut app = App::default();
        let (mi, body) = part_with_cube(&mut app);
        let v0 = app.project.bodies[mi].mesh.volume();
        let bodies_before = app.project.bodies.len();

        // THE BUTTON
        app.start_feat_cmd(27);
        assert_eq!(app.cmd.kind, 27, "the split-a-body command must open");
        assert!(app.cmd.params.iter().any(|p| p.key == "offset"), "the command must have an offset field");

        // A CLICK ON THE PLANE IN THE VIEWPORT — through a real pick rather than by assigning state:
        // earlier tools "worked" exactly until somebody tried them with a mouse.
        app.mode_3d = true;
        app.cam.init = true;
        app.cam.scale = 8.0;
        app.cam.target = [10.0, 10.0, 5.0];
        let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 700.0));
        // the face of the cube looking upwards — the cut goes along it (its datum imprint becomes the
        // cutting plane)
        let top = app.project.bodies[mi]
            .faces
            .iter()
            .filter(|f| f.normal[2] > 0.9)
            .max_by(|a, b| a.centroid.z.partial_cmp(&b.centroid.z).unwrap())
            .map(|f| [f.centroid.x, f.centroid.y, f.centroid.z])
            .expect("the top face is there");
        let basis = app.cam.basis();
        let at = app.project3(top, rect, &basis).0;
        let sp = app.pick_sketch_plane_at(rect, at).expect("a click on a face must give a plane");
        app.split.plane = Some(sp);

        // THE OFFSET DOWN BY HALF THE HEIGHT (the normal of the face looks upwards, so the offset is
        // negative)
        let h = app.project.bodies[mi].mesh.bounds().map(|b| b.max.z - b.min.z).expect("the height");
        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "offset") {
            p.val = -h * 0.5;
            p.txt = format!("{:.4}", -h * 0.5);
        }
        assert_eq!(app.split_piece_count(body), Some(2), "the tool must SEE that the plane cuts the body in two");

        // ENTER
        app.apply_feat_cmd();
        app.rebuild_if_dirty();

        let node = app
            .project
            .timeline
            .iter()
            .find(|n| matches!(n.kind, qymcad_core::feature::FeatureKind::SplitBody { .. }))
            .expect("a split feature must appear in the timeline");
        let parts = node.kind.bodies();
        assert_eq!(parts.len(), 2, "the split must give two bodies");
        assert!(app.project.bodies.len() > bodies_before, "there must be more bodies in the project; the status line: {}", app.status);

        let vs: Vec<f64> = parts.iter().filter_map(|b| app.project.mesh_index(*b)).map(|i| app.project.bodies[i].mesh.volume()).collect();
        assert_eq!(vs.len(), 2, "both pieces must have a mesh");
        assert!((vs[0] + vs[1] - v0).abs() < v0 * 0.01, "material does not vanish: there was {v0:.1}, there is {:.1}", vs[0] + vs[1]);
        assert!(vs.iter().all(|v| *v > v0 * 0.2), "both pieces must be real bodies, and what came out is {vs:?}");

        // THE SOURCE BODY IS HIDDEN — otherwise the material would be on screen twice
        assert!(app.project.consumed_bodies().contains(&body), "the source body must be consumed by the split");
    }

    /// A plane that does not cut the body is honestly refused — no feature is created.
    #[test]
    fn a_plane_that_misses_the_body_is_refused_with_a_message() {
        let mut app = App::default();
        let (mi, _body) = part_with_cube(&mut app);
        let before = app.project.timeline.len();
        app.start_feat_cmd(27);
        app.mode_3d = true;
        let top = app.project.bodies[mi].faces.iter().find(|f| f.normal[2] > 0.9).map(|f| f.id).expect("the top");
        let key = app.project.bodies[mi]
            .faces
            .iter()
            .find(|f| f.id == top)
            .map(|f| qymcad_core::feature::FaceKey { index: 0, centroid: [f.centroid.x, f.centroid.y, f.centroid.z], normal: f.normal, id: f.id })
            .expect("the key of the face");
        app.split.plane = Some(qymcad_core::feature::SketchPlane::Face(app.project.mesh_id(mi).unwrap(), key));
        // the offset goes UP — the plane moves away from the body
        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "offset") {
            p.val = 50.0;
            p.txt = "50".into();
        }
        app.apply_feat_cmd();
        assert_eq!(app.project.timeline.len(), before, "no feature must be created if the plane does not cut");
        assert_eq!(app.status, crate::i18n::tr("msg-plane-cuts-nothing"), "the reason must be said out loud");
    }

    /// The tool has a preview: the cutting plane itself and the line of the section across the body.
    #[test]
    fn the_tool_shows_where_the_cut_goes() {
        let src = crate::gui::render_source::RENDER;
        let a = src.find("pub(super) fn draw_split_preview").expect("the split must have a preview of its own");
        let b = src[a..].find("\n    /// THE PREVIEW OF THE MIRROR").map(|i| a + i).unwrap_or(src.len());
        let blk = &src[a..b];
        assert!(blk.contains("convex_polygon"), "the cutting plane must be visible as a square");
        assert!(blk.contains("line_segment"), "the line of the cut across the body must be visible");
        assert!(src.contains("self.draw_split_preview(painter, rect);"), "the preview must be called from the drawing of the frame");
    }

    /// The button is in the Part panel, and a click on the viewport inside this command chooses the
    /// plane.
    #[test]
    fn the_tool_has_a_button_and_a_pick() {
        assert!(crate::gui::panels_source::PANELS.contains("self.start_feat_cmd(27)"), "without a button the operation does not exist for a person");
        assert!(crate::gui::render_source::RENDER.contains("} else if self.cmd.kind == 27 || self.cmd.kind == 29 {"), "a click in the viewport must choose the cutting plane");
    }

    /// The feature is visible in the tree and reopens for editing — otherwise it is a one-shot.
    #[test]
    fn the_feature_is_visible_in_the_tree_and_editable() {
        assert!(crate::gui::panels_source::PANELS.contains("FeatureKind::SplitBody { ref bodies, offset, .. } =>"), "the row in the tree");
        let gui = include_str!("../gui.rs");
        assert!(gui.contains("FK::SplitBody { .. } => ph::"), "the icon");
        assert!(!crate::i18n::tr("feat-name-split-body").is_empty() && crate::i18n::tr("feat-name-split-body") != "feat-name-split-body", "the default name of the feature must have a translation");
        let cmds = include_str!("commands.rs");
        assert!(cmds.contains("FeatureKind::SplitBody { plane, datum, offset, .. } => {"), "reopening for editing");
        assert!(cmds.contains("FeatureKind::SplitBody { plane, datum, offset, bodies, .. } => {"), "applying the edit");
    }

    /// Editing the split moves the plane and does NOT recreate the bodies.
    #[test]
    fn editing_the_split_moves_the_plane_without_new_bodies() {
        let mut app = App::default();
        let (mi, body) = part_with_cube(&mut app);
        let h = app.project.bodies[mi].mesh.bounds().map(|b| b.max.z - b.min.z).expect("the height");
        let top = app.project.bodies[mi]
            .faces
            .iter()
            .filter(|f| f.normal[2] > 0.9)
            .max_by(|a, b| a.centroid.z.partial_cmp(&b.centroid.z).unwrap())
            .map(|f| qymcad_core::feature::FaceKey { index: 0, centroid: [f.centroid.x, f.centroid.y, f.centroid.z], normal: f.normal, id: f.id })
            .expect("the top face");
        app.start_feat_cmd(27);
        app.mode_3d = true;
        app.split.plane = Some(qymcad_core::feature::SketchPlane::Face(body, top));
        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "offset") {
            p.val = -h * 0.5;
            p.txt = format!("{:.4}", -h * 0.5);
        }
        app.apply_feat_cmd();
        app.rebuild_if_dirty();
        let fid = app
            .project
            .timeline
            .iter()
            .find(|n| matches!(n.kind, qymcad_core::feature::FeatureKind::SplitBody { .. }))
            .map(|n| n.id)
            .expect("the split feature");
        let parts: Vec<u64> = app.project.timeline.iter().find(|n| n.id == fid).map(|n| n.kind.bodies()).expect("the pieces");
        let thin_before = parts.iter().filter_map(|b| app.project.mesh_index(*b)).map(|i| app.project.bodies[i].mesh.volume()).fold(f64::MAX, f64::min);

        // A DOUBLE CLICK IN THE TREE -> editing
        app.start_feat_cmd_edit(fid);
        assert_eq!(app.cmd.kind, 27, "editing must open THE SAME command");
        assert!(app.split.plane.is_some(), "the cutting plane must be restored while editing");
        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "offset") {
            p.val = -h * 0.25;
            p.txt = format!("{:.4}", -h * 0.25);
        }
        app.apply_feat_cmd();
        app.rebuild_if_dirty();

        let parts_after: Vec<u64> = app.project.timeline.iter().find(|n| n.id == fid).map(|n| n.kind.bodies()).expect("the pieces after the edit");
        assert_eq!(parts, parts_after, "editing must move the cut rather than START new bodies");
        let thin_after = parts_after.iter().filter_map(|b| app.project.mesh_index(*b)).map(|i| app.project.bodies[i].mesh.volume()).fold(f64::MAX, f64::min);
        assert!(thin_after < thin_before * 0.75, "the plane has moved, so the thin piece must lose weight: it was {thin_before:.1}, it is {thin_after:.1}");
    }
    /// SPLITTING FACES is a separate tool next to splitting a body: the button, the pick, the tree, the
    /// editing.
    ///
    /// The difference is checked right here: after applying it, the body REMAINS ONE. Confusing the two
    /// tools is easy, and "marked out a patch and the part fell apart" is exactly the kind of mistake
    /// nobody expects.
    #[test]
    fn splitting_faces_keeps_the_body_whole() {
        let mut app = App::default();
        let (mi, body) = part_with_cube(&mut app);
        let face = app.project.bodies[mi]
            .faces
            .iter()
            .filter(|f| f.normal[2] > 0.9)
            .max_by(|a, b| a.centroid.z.partial_cmp(&b.centroid.z).unwrap())
            .map(|f| qymcad_core::feature::FaceKey { index: 0, centroid: [f.centroid.x, f.centroid.y, f.centroid.z], normal: f.normal, id: f.id })
            .expect("the top face");
        let h = app.project.bodies[mi].mesh.bounds().map(|b| b.max.z - b.min.z).expect("the height");
        let bodies_before = app.project.bodies.len();

        app.start_feat_cmd(29);
        assert_eq!(app.cmd.kind, 29, "the split-faces command must open");
        app.mode_3d = true;
        app.split.plane = Some(qymcad_core::feature::SketchPlane::Face(body, face));
        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "offset") {
            p.val = -h * 0.5;
            p.txt = format!("{:.4}", -h * 0.5);
        }
        app.apply_feat_cmd();
        app.rebuild_if_dirty();

        let node = app
            .project
            .timeline
            .iter()
            .find(|n| matches!(n.kind, qymcad_core::feature::FeatureKind::SplitFace { .. }))
            .expect("a split-faces feature must appear in the timeline");
        assert_eq!(node.kind.bodies().len(), 1, "there is EXACTLY ONE output — this is not a split of a body");
        assert_eq!(app.project.bodies.len(), bodies_before + 1, "the body is one: the result was added, not pieces; the status line: {}", app.status);
        let out = node.kind.body().expect("the body");
        let v = app.project.mesh_index(out).map(|i| app.project.bodies[i].mesh.volume()).unwrap_or(0.0);
        let v0 = app.project.mesh_index(body).map(|i| app.project.bodies[i].mesh.volume()).unwrap_or(0.0);
        assert!((v - v0).abs() < 1.0, "the volume must stay as it was ({v0}), and it became {v}");
    }

    /// The button is there, and it is A DIFFERENT button, not the split of a body.
    #[test]
    fn splitting_faces_has_its_own_button_and_row() {
        let panels = crate::gui::panels_source::PANELS;
        assert!(panels.contains("self.start_feat_cmd(29)"), "without a button the tool does not exist for a person");
        assert!(panels.contains("FeatureKind::SplitFace { offset, .. } =>"), "the row in the tree");
        let _gui = include_str!("../gui.rs");
        assert!(!crate::i18n::tr("feat-name-split-face").is_empty() && crate::i18n::tr("feat-name-split-face") != "feat-name-split-face", "the default name of the feature must have a translation");
        assert!(include_str!("commands.rs").contains("FeatureKind::SplitFace { plane, datum, offset, .. } => {"), "reopening for editing");
    }


}
