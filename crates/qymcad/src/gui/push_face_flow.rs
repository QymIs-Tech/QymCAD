//! PUSH FACE — the whole path a person walks.
//!
//! The operation existed in the kernel and in the timeline while the button did not: for anybody using
//! the program it did not exist. The whole path is checked here — the command opened, a click on a face
//! selected it, the offset was accepted, Enter built the feature.
#[cfg(test)]
mod tests {
    use super::super::{App, Sel};

    /// The command opens, the face is selected by a click, Enter builds a feature of the right volume.
    #[test]
    fn a_face_can_be_pulled_from_the_toolbar() {
        let mut app = App::default();
        super::super::joint_flow::tests::add_part_at(&mut app, 0.0);
        app.rebuild_if_dirty();
        let body = app.project.mesh_id(0).expect("the body");
        if let Some(owner) = app.project.body_owner(body) {
            app.enter_component(owner);
        }
        let mi = app.project.mesh_index(body).expect("the mesh");
        app.sel = Sel::Mesh(mi);
        let v0: f64 = app.project.bodies[mi].mesh.volume();

        // THE BUTTON
        app.start_feat_cmd(25);
        assert_eq!(app.cmd.kind, 25, "the push-face command must open");
        assert!(app.cmd.params.iter().any(|p| p.key == "dist"), "the command must have an offset field");

        // A CLICK ON A FACE (the top one, normal +Z)
        let top = app.project.bodies[mi]
            .faces
            .iter()
            .filter(|f| f.normal[2] > 0.9)
            .max_by(|a, b| a.centroid.z.partial_cmp(&b.centroid.z).unwrap())
            .map(|f| f.id)
            .expect("the top face is there");
        // A CLICK ON THE FACE THROUGH A REAL PICK rather than by writing it into the selection: the former
        // test wrote the face in directly and so did not notice that a click did not select it at all.
        let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 700.0));
        app.mode_3d = true;
        app.cam.init = true;
        app.cam.scale = 8.0;
        app.cam.target = [10.0, 10.0, 5.0];
        let basis = app.cam.basis();
        let c = app.project.bodies[mi]
            .faces
            .iter()
            .find(|f| f.id == top)
            .map(|f| [f.centroid.x, f.centroid.y, f.centroid.z])
            .expect("the centre of the top face");
        let at = app.project3(c, rect, &basis).0;
        app.pick_face_3d(rect, at);
        assert!(
            app.gsel.faces.contains(&top),
            "a click on a face must SELECT it: what is selected is {:?}",
            app.gsel.faces
        );

        // THE OFFSET AND ENTER
        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "dist") {
            p.val = 6.0;
            p.txt = "6".into();
        }
        app.apply_feat_cmd();
        app.rebuild_if_dirty();

        let pushed = app
            .project
            .timeline
            .iter()
            .any(|n| matches!(n.kind, qymcad_core::feature::FeatureKind::PushFace { .. }));
        assert!(pushed, "a push-face feature must appear in the timeline; the status line: {}", app.status);

        let live = app.project.mesh_id(app.project.bodies.len() - 1).expect("the new body");
        let mi2 = app.project.mesh_index(live).expect("the mesh of the new body");
        let v1 = app.project.bodies[mi2].mesh.volume();
        assert!(
            v1 > v0 + 1.0,
            "with a face pulled outwards the volume must grow: it was {v0:.1}, it became {v1:.1}"
        );
    }

    /// The tool highlights the selected face and previews the offset.
    ///
    /// Without those it is blind: what is selected and where it will go are not clear. It was reported
    /// that there was no preview, no highlight and no choosing of faces at all.
    #[test]
    fn the_tool_shows_what_is_selected_and_where_it_goes() {
        let src = crate::gui::render_source::RENDER;
        let a = src.find("} else if self.cmd.kind == 25 {").expect("the tool must have a drawing block of its own");
        let b = src[a..].find("} else if self.cmd.kind == 23 {").map(|i| a + i).unwrap_or(src.len());
        let blk = &src[a..b];
        assert!(blk.contains("egui::Mesh::default()"), "the selected face must be highlighted with a fill");
        assert!(blk.contains("dashed_line"), "the offset outline must be shown as a preview");
        assert!(blk.contains("line_segment"), "the direction of the offset must be visible as an arrow");
    }

    /// The button is in the Part panel.
    #[test]
    fn the_tool_has_a_button() {
        let src = crate::gui::panels_source::PANELS;
        assert!(
            src.contains("self.start_feat_cmd(25)"),
            "without a button the operation does not exist for a person, however much of it there is in the kernel"
        );
    }

    /// The feature IS VISIBLE IN THE BUILD TREE.
    ///
    /// Reported: why does a face that was just pulled not appear under Bodies/Build? The cause: in the
    /// tree an unknown kind of feature fell into `_ => return` — the row simply was not drawn. A new kind
    /// has to be added to EVERY match, and the compiler does not warn about catch-all arms.
    #[test]
    fn the_feature_is_named_and_shown_in_the_tree() {
        let panels = crate::gui::panels_source::PANELS;
        assert!(
            panels.contains("FeatureKind::PushFace { dist, .. } =>"),
            "the build tree must know the kind of the feature, otherwise the row is not drawn at all"
        );
        let gui = include_str!("../gui.rs");
        assert!(gui.contains("FK::PushFace { .. } => ph::"), "the feature must have an icon");
        assert!(!crate::i18n::tr("feat-name-push-face").is_empty() && crate::i18n::tr("feat-name-push-face") != "feat-name-push-face", "the default name of the feature must have a translation");
        // A REFERENCE TO A FACE LIVES BY TWO DIFFERENT MECHANISMS, and the guard confused them. The
        // first is THE CARRYING OVER OF NAMES when a feature is copied (an array, a mirror): it lives in
        // `feature.rs` and it is essential. The second used to be the matching of a lost reference by
        // similarity in `regen.rs` — that is gone: a reference became a query and either finds the face
        // by its recipe or refuses with a reason.
        //
        // The former edition checked THE SECOND while its message described THE FIRST — and turned red
        // when the second was removed on purpose.
        let feature = include_str!("../../../qymcad-core/src/feature.rs");
        assert!(
            feature.contains("FeatureKind::Hole { face, .. } | FeatureKind::PushFace { face, .. } => face.remap_descs"),
            "the names in a reference must be translated when a feature is copied, otherwise the copy pulls somebody else's face"
        );
        // THE QUERY COMES FIRST, THE WITNESS LAST.
        //
        // The guard still forbids matching a face "by similarity", but now checks that more precisely.
        // The direct call to the resolver was replaced by the common path `face_by_ref`, and forbidding
        // that would mean forbidding the repair itself: a reference to a face gained a witness (a
        // snapshot of the place) precisely because without one EVERY improvement to the names broke a
        // live project.
        //
        // The condition is now twofold: push-face goes by the common path, and the common path itself
        // starts with THE QUERY. Should anybody put the geometry ahead of the name, the guard turns
        // red.
        let regen = include_str!("../../../qymcad-core/src/model/regen.rs");
        assert!(
            // The node being rebuilt travels in the pass context now that each kind of feature has a
            // branch of its own; the path itself is the same one.
            regen.contains("self.face_by_ref(p.node, src, &face, \"ref-what-pushed-face\")"),
            "the face must be looked for by the common path of a reference rather than by a special one of its own"
        );
        let common = regen.split("fn face_by_ref").nth(1).unwrap_or("");
        let by_name = common.find("self.resolve_face_ref(src, r, what)");
        let by_snap = common.find("self.resolve_face_id(node_id, src, *d)");
        assert!(
            by_name.is_some() && by_snap.is_some() && by_name < by_snap,
            "THE QUERY must come FIRST, and the snapshot of the place only when the name missed"
        );
    }

    /// A feature once created CAN BE CORRECTED: a double click opens the command, Enter applies the new
    /// offset.
    ///
    /// Without those two links the tool was a one-shot: built and done, with nothing to change it by. The
    /// gap was found not by poking but by comparison with A REFERENCE feature (the draft): where that one
    /// has something, this one must have it too.
    #[test]
    fn an_existing_pull_can_be_edited() {
        let mut app = App::default();
        super::super::joint_flow::tests::add_part_at(&mut app, 0.0);
        app.rebuild_if_dirty();
        let body = app.project.mesh_id(0).expect("the body");
        if let Some(owner) = app.project.body_owner(body) {
            app.enter_component(owner);
        }
        let mi = app.project.mesh_index(body).expect("the mesh");
        app.sel = Sel::Mesh(mi);
        let top = app.project.bodies[mi]
            .faces
            .iter()
            .filter(|f| f.normal[2] > 0.9)
            .max_by(|a, b| a.centroid.z.partial_cmp(&b.centroid.z).unwrap())
            .map(|f| f.id)
            .expect("the top face");

        app.start_feat_cmd(25);
        app.gsel.faces.insert(top);
        app.gsel.faces_body = Some(body);
        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "dist") {
            p.val = 4.0;
            p.txt = "4".into();
        }
        app.apply_feat_cmd();
        app.rebuild_if_dirty();
        let fid = app
            .project
            .timeline
            .iter()
            .find(|n| matches!(n.kind, qymcad_core::feature::FeatureKind::PushFace { .. }))
            .map(|n| n.id)
            .expect("the feature is created");

        // EDITING: a double click on the feature in the tree
        app.cancel_all_tools();
        app.start_feat_cmd_edit(fid);
        assert_eq!(app.cmd.kind, 25, "a double click must reopen the push-face command");
        assert!(app.gsel.faces.contains(&top), "editing must restore the selected face");
        let shown = app.cmd.params.iter().find(|p| p.key == "dist").map(|p| p.val);
        assert_eq!(shown, Some(4.0), "the field must hold the former offset rather than the default value");

        // the offset is changed and applied
        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "dist") {
            p.val = 9.0;
            p.txt = "9".into();
        }
        app.apply_feat_cmd();
        app.rebuild_if_dirty();
        let d = app.project.timeline.iter().find_map(|n| match n.kind {
            qymcad_core::feature::FeatureKind::PushFace { dist, .. } => Some(dist),
            _ => None,
        });
        assert_eq!(d, Some(9.0), "an edit must apply to THE EXISTING feature rather than create a second one");
        let n = app
            .project
            .timeline
            .iter()
            .filter(|n| matches!(n.kind, qymcad_core::feature::FeatureKind::PushFace { .. }))
            .count();
        assert_eq!(n, 1, "there must be one feature left — editing does not breed nodes");
    }

    /// THE FACE IS DRAGGED WITH THE MOUSE, not only entered as a number.
    ///
    /// Direct modelling is "grab it and pull". If there is nothing to pull with and all that is left is
    /// typing a number into a field, the tool is half a tool: it exists, and it cannot be worked with the
    /// way it is worked in grown-up CAD.
    #[test]
    fn the_face_can_be_dragged_by_its_arrow() {
        let mut app = App::default();
        super::super::joint_flow::tests::add_part_at(&mut app, 0.0);
        app.rebuild_if_dirty();
        let body = app.project.mesh_id(0).expect("the body");
        if let Some(owner) = app.project.body_owner(body) {
            app.enter_component(owner);
        }
        let mi = app.project.mesh_index(body).expect("the mesh");
        app.sel = Sel::Mesh(mi);
        app.start_feat_cmd(25);
        let top = app.project.bodies[mi]
            .faces
            .iter()
            .filter(|f| f.normal[2] > 0.9)
            .max_by(|a, b| a.centroid.z.partial_cmp(&b.centroid.z).unwrap())
            .map(|f| f.id)
            .expect("the top face");
        app.gsel.faces.insert(top);
        app.gsel.faces_body = Some(body);

        let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 700.0));
        app.mode_3d = true;
        app.cam.init = true;
        app.cam.scale = 9.0;
        app.cam.target = [10.0, 10.0, 5.0];
        let basis = app.cam.basis();

        // THE HANDLE IS THERE and can be hit with the cursor — even at a zero offset
        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "dist") {
            p.val = 0.0;
            p.txt = "0".into();
        }
        let (o, tip, _) = app.face_arrow_geometry().expect("the selected face must have a handle");
        let at = app.project3(tip, rect, &basis).0;
        assert!(app.face_arrow_hit(rect, at, &basis), "a cursor at the tip of the arrow must grab it");
        let far = app.project3([o[0] + 500.0, o[1] + 500.0, o[2]], rect, &basis).0;
        assert!(!app.face_arrow_hit(rect, far, &basis), "far from the arrow there must be no grab");

        // THE DRAG changes the offset, and THE FIELD follows the arrow (one value, not two independent ones)
        let before = app.cmd_val("dist");
        app.face_arrow_drag = Some(before);
        app.face_arrow_drag_to(egui::vec2(0.0, -40.0), rect, &basis);
        let after = app.cmd_val("dist");
        assert!((after - before).abs() > 0.5, "the drag must change the offset: it was {before}, it became {after}");
        let txt = app.cmd.params.iter().find(|p| p.key == "dist").map(|p| p.txt.clone()).unwrap_or_default();
        assert!(
            (txt.trim().parse::<f64>().unwrap_or(f64::NAN) - after).abs() < 0.01,
            "the field must show the same as the arrow: the field holds \"{txt}\", the arrow {after}"
        );
    }

    /// THE SECOND STRIP OF A PINCHED RIM PUSHES JUST LIKE THE FIRST — by the path of the mouse, not of
    /// the model.
    ///
    /// Reported: the opposite face pulls without trouble while this particular one does not — the face is
    /// no longer in the source body, the reference is stale. There is one difference between the strips:
    /// the first kept the name of the original face while the second has its name BUILT UP ("piece 1 of
    /// face N") after the kernel has already returned the result.
    ///
    /// The application lays out the faces of bodies BY THE REPORT of the rebuild — the pick goes by those
    /// and they go into the file — while references are resolved against `regen_faces`. While a
    /// PRE-RENAMING list was put into the report, the mouse got a positional number, the feature recorded
    /// it, and the very next rebuild answered "the face is no longer there". So the test takes the face
    /// FROM EXACTLY where a click takes it — from `bodies[..].faces`.
    #[test]
    fn the_renamed_half_of_a_pinched_rim_pushes_too() {
        let mut app = App::default();
        let part = app.project.add_part("part");
        app.enter_component(part);
        let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        app.project.add_rect_entity(si, 0.0, 0.0, 50.0, 50.0, qymcad_core::feature::Purpose::Real);
        app.project.regen_sketch(si);
        app.finish_sketch_edit();
        app.sel = Sel::Sketch(si);
        app.start_feat_cmd(1);
        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "height") {
            p.val = 50.0;
            p.txt = "50".into();
        }
        app.apply_feat_cmd();
        app.rebuild_if_dirty();
        let boxy = app.project.timeline.iter().rev().find_map(|n| n.kind.body()).expect("the box");

        // A SHELL 2 mm THICK, OPEN AT BOTH ENDS: the top and the bottom are removed
        app.select_body(boxy);
        app.start_feat_cmd(6);
        let mi = app.project.mesh_index(boxy).expect("the mesh");
        for f in app.project.bodies[mi].faces.clone().iter().filter(|f| f.normal[2].abs() > 0.9) {
            app.gsel.faces.insert(f.id);
        }
        app.gsel.faces_body = Some(boxy);
        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "thickness") {
            p.val = 2.0;
            p.txt = "2".into();
        }
        app.apply_feat_cmd();
        app.rebuild_if_dirty();
        let frame = app.project.timeline.iter().rev().find_map(|n| n.kind.body()).expect("the frame");

        // A FILLET R1 on the two front top edges — it eats the front strip of the rim right through and
        // the rim falls into two (see `face_is_one_island.rs` in the repro crate)
        app.select_body(frame);
        app.start_feat_cmd(4);
        let front: Vec<u32> = app.project.regen_edges[&frame]
            .iter()
            .filter(|e| (e.a[2] - 50.0).abs() < 1e-6 && (e.b[2] - 50.0).abs() < 1e-6 && e.mid[1] < 2.5)
            .map(|e| e.id)
            .collect();
        assert_eq!(front.len(), 2, "setup: there are two edges at the front top, and {} were found", front.len());
        app.gsel.edges = front.into_iter().collect();
        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "radius") {
            p.val = 1.0;
            p.txt = "1".into();
        }
        app.apply_feat_cmd();
        app.rebuild_if_dirty();
        let filleted = app.project.timeline.iter().rev().find_map(|n| n.kind.body()).expect("the filleted body");

        // A CHAMFER on the back top edge — it eats the back strip and the rim falls into two
        app.select_body(filleted);
        app.start_feat_cmd(5);
        let back: Vec<u32> = app.project.regen_edges[&filleted]
            .iter()
            .filter(|e| (e.a[2] - 50.0).abs() < 1e-6 && (e.b[2] - 50.0).abs() < 1e-6 && e.mid[1] > 49.5)
            .map(|e| e.id)
            .collect();
        assert_eq!(back.len(), 1, "setup: there is one outer edge at the back top, and {} were found", back.len());
        app.gsel.edges = back.into_iter().collect();
        for key in ["dist", "d2"] {
            if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == key) {
                p.val = 2.0;
                p.txt = "2".into();
            }
        }
        app.apply_feat_cmd();
        app.rebuild_if_dirty();
        let filleted = app.project.timeline.iter().rev().find_map(|n| n.kind.body()).expect("the chamfered body");

        // THE RIM LIES AS TWO STRIPS — they are taken THE SAME WAY A CLICK TAKES THEM
        let fmi = app.project.mesh_index(filleted).expect("the mesh");
        let mut strips: Vec<qymcad_core::geom::MeshFace> =
            app.project.bodies[fmi].faces.iter().filter(|f| f.normal[2] > 0.9 && f.centroid.z > 49.0).cloned().collect();
        assert_eq!(strips.len(), 2, "setup: the rim must lie as two strips, and out came {}", strips.len());
        strips.sort_by(|a, b| a.centroid.x.total_cmp(&b.centroid.x));

        // AND THE SAME THING AS A RULE: the name of a face is one across the whole program. What the
        // mouse clicks (`bodies[..].faces`) is what gets resolved (`regen_faces`). Let them diverge and a
        // click gives a name that does not exist for the resolver.
        let seen: Vec<u32> = app.project.bodies[fmi].faces.iter().map(|f| f.id).collect();
        let resolvable: Vec<u32> = app.project.regen_faces[&filleted].iter().map(|f| f.id).collect();
        assert_eq!(seen, resolvable, "the mouse sees one set of face names while references resolve against another");

        // THE SECOND ONE IS PULLED — the one whose name is built up
        let strip = strips.pop().expect("the right-hand strip");
        app.select_body(filleted);
        app.start_feat_cmd(25);
        app.gsel.faces.clear();
        app.gsel.faces.insert(strip.id);
        app.gsel.faces_body = Some(filleted);
        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "dist") {
            p.val = 5.0;
            p.txt = "5".into();
        }
        app.apply_feat_cmd();
        app.rebuild_if_dirty();

        let node = app
            .project
            .timeline
            .iter()
            .find(|n| matches!(n.kind, qymcad_core::feature::FeatureKind::PushFace { .. }))
            .map(|n| n.id)
            .expect("the push must appear in the timeline");
        assert!(
            !app.project.regen_errors.contains_key(&node),
            "pushing the second strip must build just as pushing the first does, and out came: {:?}",
            app.project.regen_errors.get(&node)
        );
    }
}
