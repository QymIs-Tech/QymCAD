//! THICKEN — the whole path a person walks.
//!
//! The kernel and the timeline are checked separately; here it is the wiring: the button, a click on a
//! face through a real pick, the thickness, Enter — and the fact that the part stays ONE body after all
//! that.
//!
//! The plate used to stay a body of its own and the part became two: a piece on screen was painted a
//! different colour, as if a second part had been added, and that is how it was caught. The law that a
//! Part is ONE body does not allow that — the plate is glued to the source and the source is
//! consumed.
#[cfg(test)]
mod tests {
    use super::super::{App, Sel};

    /// A cube inside a part; returns (the index of the mesh, the id of the body).
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

    /// The button is there.
    #[test]
    fn the_tool_has_a_button() {
        assert!(crate::gui::panels_source::PANELS.contains("self.start_feat_cmd(28)"), "without a button the tool does not exist for a person");
    }

    /// THE WHOLE PATH: the button -> a click on a face -> the thickness -> Enter -> the part has grown
    /// and stayed ONE body.
    #[test]
    fn a_face_can_be_thickened_from_the_toolbar() {
        let mut app = App::default();
        let (mi, body) = part_with_cube(&mut app);
        let bodies_before = (0..app.project.bodies.len()).filter(|i| app.body_shown(*i)).count();
        let before_v = app.project.mesh_index(body).map(|i| app.project.bodies[i].mesh.volume()).unwrap_or(0.0);

        app.start_feat_cmd(28);
        assert_eq!(app.cmd.kind, 28, "the command must open");
        assert!(app.cmd.params.iter().any(|p| p.key == "thickness"), "the command must have a thickness field");

        // A CLICK ON A FACE THROUGH A REAL PICK
        app.mode_3d = true;
        app.cam.init = true;
        app.cam.scale = 8.0;
        app.cam.target = [10.0, 10.0, 5.0];
        let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 700.0));
        let top = app.project.bodies[mi]
            .faces
            .iter()
            .filter(|f| f.normal[2] > 0.9)
            .max_by(|a, b| a.centroid.z.partial_cmp(&b.centroid.z).unwrap())
            .expect("the top face");
        let (fid, c) = (top.id, [top.centroid.x, top.centroid.y, top.centroid.z]);
        let basis = app.cam.basis();
        app.pick_face_3d(rect, app.project3(c, rect, &basis).0);
        assert!(app.gsel.faces.contains(&fid), "the click must SELECT the face: what is selected is {:?}", app.gsel.faces);

        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "thickness") {
            p.val = 3.0;
            p.txt = "3".into();
        }
        app.apply_feat_cmd();
        app.rebuild_if_dirty();

        let node = app
            .project
            .timeline
            .iter()
            .find(|n| matches!(n.kind, qymcad_core::feature::FeatureKind::Thicken { .. }))
            .expect("a thicken feature must appear in the timeline");
        let out = node.kind.body().expect("the body of the result");
        let v = app.project.mesh_index(out).map(|i| app.project.bodies[i].mesh.volume()).unwrap_or(0.0);
        assert!(v > before_v + 1.0, "the part must grow by the plate: it was {before_v:.1}, it became {v:.1}; the status line: {}", app.status);

        // A PART IS ONE BODY: the source is consumed, there is one piece on screen and not two of
        // different colours
        assert!(app.project.consumed_bodies().contains(&body), "thicken must consume the source, otherwise the part becomes two bodies");
        let shown = (0..app.project.bodies.len()).filter(|i| app.body_shown(*i)).count();
        assert_eq!(shown, bodies_before, "the number of visible bodies must stay the same ({bodies_before}), and it became {shown}");
    }

    /// A SHEET IS THICKENED BY THE SAME TOOL — and the body is taken from THE FACE THAT WAS CLICKED.
    ///
    /// This is the way out of the design layer back into the timeline: a surface can neither be added to
    /// a part nor printed, while a body can. No separate button is needed for it: the difference is not
    /// in the tool but in what was pointed at.
    ///
    /// The main trap of the wiring is checked along the way: the PART is selected in the tree while the
    /// SHEET is clicked. The body used to be taken from the selection — the face of the sheet would be
    /// looked for on the part and would not be found.
    #[test]
    fn a_sheet_is_thickened_by_the_same_tool() {
        let mut app = App::default();
        let (mi, body) = part_with_cube(&mut app);
        let top = app.project.bodies[mi].faces.iter().filter(|f| f.normal[2] > 0.9).max_by(|a, b| a.centroid.z.total_cmp(&b.centroid.z)).expect("the top face").id;
        let sheet = app.project.add_face_copy(body, qymcad_core::refs::Ref::one(top, qymcad_core::refs::Fingerprint::default()));
        app.rebuild_if_dirty();
        assert!(app.project.bodies.iter().any(|b| b.id == sheet && b.sheet), "setup: a copy of a face must be a sheet");
        let sface = app.project.regen_faces[&sheet].first().map(|f| f.id).expect("a face of the sheet");
        let area = app.project.regen_faces[&sheet].iter().map(|f| f.area).sum::<f64>();

        // THE PART IS SELECTED IN THE TREE WHILE THE SHEET IS CLICKED. The selection is set directly:
        // the sheet lies ON a face of the part, and a real pick there is ambiguous — what is checked here
        // is not the pick but whose face the command takes the clicked one to be.
        app.sel = Sel::Mesh(mi);
        app.start_feat_cmd(28);
        app.gsel.faces.insert(sface);
        app.gsel.faces_body = Some(sheet);
        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "thickness") {
            p.val = 3.0;
            p.txt = "3".into();
        }
        app.apply_feat_cmd();
        app.rebuild_if_dirty();

        let node = app
            .project
            .timeline
            .iter()
            .find_map(|n| match n.kind {
                qymcad_core::feature::FeatureKind::Thicken { src, body, .. } => Some((n.id, src, body)),
                _ => None,
            })
            .expect("a thicken feature must appear in the timeline");
        assert_eq!(node.1, sheet, "what must be thickened is THE SHEET whose face was clicked, not the part selected in the tree");
        assert!(!app.project.regen_errors.contains_key(&node.0), "the sheet must thicken: {:?}", app.project.regen_errors.get(&node.0));
        let out = app.project.bodies.iter().find(|b| b.id == node.2).expect("the body of the result");
        assert!(!out.sheet, "after a thickness this is a body already and not a surface");
        // THE PLATE RETURNS INTO THE PART: the result is the part PLUS the plate rather than the plate on
        // its own. Otherwise two bodies are left in the part — a piece of another colour on screen.
        let v_part = app.project.bodies.iter().find(|b| b.id == body).map(|b| b.mesh.volume()).unwrap_or(0.0);
        assert!((out.mesh.volume() - (v_part + area * 3.0)).abs() < area * 0.1, "the result = the part {v_part:.1} + the plate {:.1}, and out came {:.1}", area * 3.0, out.mesh.volume());
        assert!(app.project.consumed_bodies().contains(&body), "the part must be consumed: one body lives on from here");
    }

    /// A thickness of zero is refused with an explanation and no feature is created.
    #[test]
    fn zero_thickness_is_refused_with_a_message() {
        let mut app = App::default();
        let (mi, _body) = part_with_cube(&mut app);
        let before = app.project.timeline.len();
        app.start_feat_cmd(28);
        let fid = app.project.bodies[mi].faces.iter().find(|f| f.normal[2] > 0.9).map(|f| f.id).expect("a face");
        app.gsel.faces.insert(fid);
        app.gsel.faces_body = app.project.mesh_id(mi);
        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "thickness") {
            p.val = 0.0;
            p.txt = "0".into();
        }
        app.apply_feat_cmd();
        assert_eq!(app.project.timeline.len(), before, "no feature must be created at a thickness of zero");
        assert_eq!(app.status, crate::i18n::tr("msg-zero-thickness"), "the reason must be said out loud");
    }

    /// The tool shows what is selected and where the material will go.
    #[test]
    fn the_tool_shows_the_face_and_where_the_plate_goes() {
        let src = crate::gui::render_source::RENDER;
        let a = src.find("} else if self.cmd.kind == 28 {").expect("the tool must have a drawing block of its own");
        let b = src[a..].find("} else if self.cmd.kind == 26 {").map(|i| a + i).unwrap_or(src.len());
        let blk = &src[a..b];
        assert!(blk.contains("egui::Mesh::default()"), "the selected face must be highlighted with a fill");
        assert!(blk.contains("line_segment"), "the offset outline of the plate must be visible");
    }

    /// The feature is visible in the tree and reopens for editing.
    #[test]
    fn the_feature_is_in_the_tree_and_editable() {
        assert!(crate::gui::panels_source::PANELS.contains("FeatureKind::Thicken { thickness, .. } =>"), "the row in the tree");
        let gui = include_str!("../gui.rs");
        assert!(gui.contains("FK::Thicken { .. } => ph::"), "the icon");
        assert!(!crate::i18n::tr("feat-name-thicken").is_empty() && crate::i18n::tr("feat-name-thicken") != "feat-name-thicken", "the default name of the feature must have a translation");
        let cmds = include_str!("commands.rs");
        assert!(cmds.contains("FeatureKind::Thicken { face, thickness, .. } => {"), "reopening for editing");
        assert!(cmds.contains("FeatureKind::Thicken { face, thickness, .. } => {"), "applying the edit");
    }

    /// THE THICKNESS FIELD MUST APPEAR AT THE GEOMETRY.
    ///
    /// The command had the parameter from the very start, and the anchor of the popup did not: the list
    /// of command kinds in `cmd_anchor_screen` knew nothing of thicken, the anchor came out `None`, and
    /// there was nowhere to enter the thickness — the reported behaviour was that the popup with the
    /// thickness is not there. The guard asks for the anchor rather than for the presence of the
    /// parameter — the parameter was precisely what was there.
    #[test]
    fn the_thickness_field_has_a_place_at_the_geometry() {
        let mut app = App::default();
        let (mi, _body) = part_with_cube(&mut app);
        app.start_feat_cmd(28);
        app.mode_3d = true;
        app.cam.init = true;
        app.cam.scale = 8.0;
        app.cam.target = [10.0, 10.0, 5.0];
        let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 700.0));
        let top = app.project.bodies[mi].faces.iter().filter(|f| f.normal[2] > 0.9).max_by(|a, b| a.centroid.z.partial_cmp(&b.centroid.z).unwrap()).expect("the top face");
        let (fid, c) = (top.id, [top.centroid.x, top.centroid.y, top.centroid.z]);
        let basis = app.cam.basis();
        app.pick_face_3d(rect, app.project3(c, rect, &basis).0);
        assert!(app.gsel.faces.contains(&fid), "the face must get selected by a click");

        let anchor = app.cmd_anchor_screen(rect).expect("the command must have an anchor for the popup — otherwise there is nowhere to show the thickness field");
        assert!(rect.contains(anchor), "the anchor must be inside the viewport, and it is {anchor:?}");
        // and it is at THE FACE ITSELF rather than in some random corner
        let face = app.project3(c, rect, &basis).0;
        assert!(anchor.distance(face) < 200.0, "the anchor must stand at the selected face: it is {:.0} px away", anchor.distance(face));
    }

    /// THE THICKNESS IS DRAGGED WITH THE MOUSE, not only entered as a number.
    ///
    /// The tool is one for one like push-face: pick a face, set a distance along its normal. There the
    /// handle existed, here it did not — and the same action was done in two different ways. A handle
    /// was asked for in so many words.
    #[test]
    fn the_thickness_can_be_dragged_by_a_handle() {
        let mut app = App::default();
        let (mi, body) = part_with_cube(&mut app);
        app.start_feat_cmd(28);
        let top = app.project.bodies[mi].faces.iter().filter(|f| f.normal[2] > 0.9).max_by(|a, b| a.centroid.z.partial_cmp(&b.centroid.z).unwrap()).map(|f| f.id).expect("the top face");
        app.gsel.faces.insert(top);
        app.gsel.faces_body = Some(body);

        let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 700.0));
        app.mode_3d = true;
        app.cam.init = true;
        app.cam.scale = 9.0;
        app.cam.target = [10.0, 10.0, 5.0];
        let basis = app.cam.basis();

        let (_, tip, _) = app.face_arrow_geometry().expect("the selected face must have a handle");
        let at = app.project3(tip, rect, &basis).0;
        assert!(app.face_arrow_hit(rect, at, &basis), "a cursor at the tip of the arrow must grab it");
        assert!(!app.face_arrow_hit(rect, at + egui::vec2(200.0, 200.0), &basis), "far from the arrow there must be no grab");

        let before = app.cmd_val("thickness");
        app.face_arrow_drag = Some(before);
        app.face_arrow_drag_to(egui::vec2(0.0, -40.0), rect, &basis);
        let after = app.cmd_val("thickness");
        assert!((after - before).abs() > 0.1, "dragging the handle must change THE THICKNESS: it was {before:.2}, it became {after:.2}");
        assert!(
            app.cmd.params.iter().find(|p| p.key == "thickness").map(|p| p.txt.clone()).unwrap_or_default().starts_with(&format!("{after:.2}")),
            "the field and the handle are one value and not two independent ones"
        );
    }

    /// THE POPUP DOES NOT LIE ON THE FACE BEING PICKED.
    ///
    /// The anchor stood at the centroid of the selected face — that is, exactly on top of what gets
    /// dragged and aimed at next; the reported behaviour was that the popups cover the geometry and make
    /// picking faces awkward. The check goes by the screen box of the face: the popup must be OUTSIDE it
    /// yet nearby.
    #[test]
    fn the_popup_does_not_cover_the_face_being_picked() {
        for kind in [25u8, 28] {
            let mut app = App::default();
            let (mi, body) = part_with_cube(&mut app);
            app.start_feat_cmd(kind);
            app.mode_3d = true;
            app.cam.init = true;
            app.cam.scale = 9.0;
            app.cam.target = [10.0, 10.0, 5.0];
            let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 700.0));
            let face = app.project.bodies[mi].faces.iter().filter(|f| f.normal[2] > 0.9).max_by(|a, b| a.centroid.z.partial_cmp(&b.centroid.z).unwrap()).cloned().expect("the top face");
            app.gsel.faces.insert(face.id);
            app.gsel.faces_body = Some(body);

            let basis = app.cam.basis();
            let mesh = &app.project.bodies[mi].mesh;
            let mut bbox: Option<egui::Rect> = None;
            for &ti in &face.triangles {
                for v in mesh.triangle(ti as usize) {
                    let p = app.project3([v.x, v.y, v.z], rect, &basis).0;
                    bbox = Some(bbox.map_or(egui::Rect::from_min_max(p, p), |r| r.union(egui::Rect::from_min_max(p, p))));
                }
            }
            let bbox = bbox.expect("the face is visible on screen");
            let anchor = app.cmd_anchor_screen(rect).expect("the anchor of the popup must exist");
            assert!(!bbox.contains(anchor), "command {kind}: the popup lay on the selected face — the very thing aimed at next (the face {bbox:?}, the anchor {anchor:?})");
            assert!(rect.contains(anchor), "command {kind}: the anchor must be inside the viewport, and it is {anchor:?}");
            assert!(anchor.distance(bbox.center()) < 400.0, "command {kind}: the anchor must stay AT THE GEOMETRY, and it is {:.0} px from the face", anchor.distance(bbox.center()));
        }
    }
}
