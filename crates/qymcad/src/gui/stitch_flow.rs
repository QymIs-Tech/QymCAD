//! STITCHING SHEETS — the whole path a person takes.
//!
//! The kernel and the timeline are checked separately; this is the wiring: the button, a click on a
//! sheet through a real pick, a refusal on a click on a solid, Enter — and that the pieces are
//! consumed afterwards.
//!
//! A click on a SOLID must say the reason at once. Silence here reads exactly as "the tool does not
//! work": a person clicks a part, nothing happens, and they cannot tell "it was not picked" from "it
//! was picked and not drawn".
#[cfg(test)]
mod tests {
    use super::super::{App, Sel};

    fn rect() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 700.0))
    }

    /// A box with its top taken off and a patch OVER the cavity. Returns (app, body of the box,
    /// sheet).
    ///
    /// The patch is stretched over the INNER rim of the opening: it hangs above the cavity and
    /// coincides with no face, so a click on it is unambiguous — which is what checking a real pick
    /// requires.
    fn box_with_patch() -> (App, u64, u64) {
        let mut app = App::default();
        super::super::joint_flow::tests::add_part_at(&mut app, 0.0);
        app.rebuild_if_dirty();
        let cube = app.project.mesh_id(0).expect("the body");
        if let Some(owner) = app.project.body_owner(cube) {
            app.enter_component(owner);
        }
        let top: Vec<u32> = app.project.regen_faces[&cube].iter().filter(|f| f.normal[2] > 0.9).map(|f| f.id).collect();
        let shell = app.project.add_shell_mode(cube, 2.0, top, qymcad_core::feature::ShellSide::Inward);
        app.rebuild_if_dirty();

        // the INNER rim of the opening: the top edges that do NOT lie on the bounding box of the body
        let zmax = app.project.regen_edges[&shell].iter().flat_map(|e| [e.a[2], e.b[2]]).fold(f64::MIN, f64::max);
        let (mut x0, mut x1) = (f64::MAX, f64::MIN);
        for e in &app.project.regen_edges[&shell] {
            for q in [e.a, e.b] {
                x0 = x0.min(q[0]);
                x1 = x1.max(q[0]);
            }
        }
        let inner: Vec<u32> = app.project.regen_edges[&shell]
            .iter()
            .filter(|e| (e.a[2] - zmax).abs() < 1e-6 && (e.b[2] - zmax).abs() < 1e-6)
            .filter(|e| [e.a, e.b].iter().all(|q| (q[0] - x0).abs() > 1e-6 && (q[0] - x1).abs() > 1e-6))
            .map(|e| e.id)
            .collect();
        assert_eq!(inner.len(), 4, "setup: the inner rim has four edges, and {} were found", inner.len());
        let patch = app.project.add_patch(shell, qymcad_core::refs::Ref::picks(&inner), false);
        app.rebuild_if_dirty();
        assert!(app.project.bodies.iter().any(|b| b.id == patch && b.sheet), "setup: the patch must be a sheet");

        app.mode_3d = true;
        app.cam.init = true;
        app.cam.scale = 9.0;
        app.cam.target = [10.0, 10.0, 5.0];
        (app, shell, patch)
    }

    /// The button exists.
    #[test]
    fn the_tool_has_a_button() {
        assert!(crate::gui::panels_source::PANELS.contains("self.start_feat_cmd(33)"), "without a button the tool does not exist for a person");
    }

    /// A CLICK ON A SHEET ADDS IT, A CLICK ON A SOLID SAYS THE REASON.
    #[test]
    fn clicking_a_sheet_picks_it_and_clicking_a_solid_says_why() {
        let (mut app, boxy, patch) = box_with_patch();
        app.start_feat_cmd(33);
        assert_eq!(app.cmd.kind, 33, "the command must open");

        let basis = app.cam.basis();
        let mi = app.project.mesh_index(patch).expect("the mesh of the patch");
        let c = app.project.bodies[mi].faces.first().map(|f| [f.centroid.x, f.centroid.y, f.centroid.z]).expect("the centre of the patch");
        app.pick_face_3d(rect(), app.project3(c, rect(), &basis).0);
        assert_eq!(app.stitch_parts, vec![patch], "a click on a sheet must PICK it; status: {}", app.status);

        // a second click removes the sheet
        app.pick_face_3d(rect(), app.project3(c, rect(), &basis).0);
        assert!(app.stitch_parts.is_empty(), "a second click must remove the sheet rather than add it twice");

        // A CLICK ON A SOLID gives a named refusal rather than silence
        let bi = app.project.mesh_index(boxy).expect("the mesh of the box");
        let side = app.project.bodies[bi].faces.iter().find(|f| f.normal[2].abs() < 0.1).map(|f| [f.centroid.x, f.centroid.y, f.centroid.z]).expect("a side face");
        app.pick_face_3d(rect(), app.project3(side, rect(), &basis).0);
        assert!(app.stitch_parts.is_empty(), "there is nothing to stitch a solid with — it must not get into the selection");
        assert_eq!(app.status, crate::i18n::tr("msg-stitch-only-sheets"), "the reason must be said rather than silence");
    }

    /// THE WHOLE PATH TO THE TIMELINE: two sheets -> Enter -> one surface, the pieces consumed.
    ///
    /// The selection is put in directly: the face copies lie ON the faces of the part and a real pick
    /// there is ambiguous — the pick itself is checked above, and what is checked here is that the
    /// selection reaches the timeline.
    #[test]
    fn two_sheets_become_one_surface() {
        let mut app = App::default();
        super::super::joint_flow::tests::add_part_at(&mut app, 0.0);
        app.rebuild_if_dirty();
        let body = app.project.mesh_id(0).expect("the body");
        if let Some(owner) = app.project.body_owner(body) {
            app.enter_component(owner);
        }
        let top = app.project.regen_faces[&body].iter().find(|f| f.normal[2] > 0.9).map(|f| f.id).expect("the top");
        let side = app.project.regen_faces[&body].iter().find(|f| f.normal[1] < -0.9).map(|f| f.id).expect("the side");
        let a = app.project.add_face_copy(body, qymcad_core::refs::Ref::one(top, qymcad_core::refs::Fingerprint::default()));
        let b = app.project.add_face_copy(body, qymcad_core::refs::Ref::one(side, qymcad_core::refs::Fingerprint::default()));
        app.rebuild_if_dirty();

        app.sel = Sel::Mesh(0);
        app.start_feat_cmd(33);
        app.stitch_parts = vec![a, b];
        app.apply_feat_cmd();
        app.rebuild_if_dirty();

        let node = app
            .project
            .timeline
            .iter()
            .find_map(|n| match n.kind {
                qymcad_core::feature::FeatureKind::Stitch { ref parts, body, .. } => Some((n.id, parts.clone(), body)),
                _ => None,
            })
            .expect("a stitch must appear in the timeline");
        assert_eq!(node.1, vec![a, b], "the sheets that were picked must reach the timeline");
        assert!(!app.project.regen_errors.contains_key(&node.0), "the stitch must build: {:?}", app.project.regen_errors.get(&node.0));
        assert!(app.project.consumed_bodies().contains(&a) && app.project.consumed_bodies().contains(&b), "the pieces must be consumed");
        assert!(app.stitch_parts.is_empty(), "after applying, the selection must be cleared — otherwise it leaks into the next command");
    }

    /// THE TOOL SHOWS WHAT IS PICKED (a neighbouring guard catches this class by a sweep; what is
    /// checked here is this particular branch).
    #[test]
    fn the_tool_shows_what_is_picked() {
        let src = crate::gui::render_source::RENDER;
        let a = src.find("} else if self.cmd.kind == 33 {").expect("the stitch must have a drawing block of its own");
        let b = src[a..].find("} else if self.cmd.kind == 26 {").map(|i| a + i).unwrap_or(src.len());
        assert!(src[a..b].contains("egui::Mesh::default()"), "the picked sheets must be highlighted with a fill");
    }

    /// The fields and hints of the tool are translated — otherwise a person sees a catalogue key.
    #[test]
    fn the_tool_speaks_the_users_language() {
        for (code, _) in crate::i18n::available() {
            let prev = crate::i18n::language();
            crate::i18n::set_language(&code);
            for key in ["f-stitch", "f-stitch-tol", "tb-stitch-hint", "msg-stitch", "msg-stitch-only-sheets", "hint-stitch", "feat-name-stitch", "error-op-failed-stitch"] {
                let s = crate::i18n::tr(key);
                assert!(!s.is_empty() && s != key, "language {code} has no translation for {key}");
            }
            crate::i18n::set_language(&prev);
        }
    }
}
