//! A VARIABLE RADIUS BY WAY OF THE MOUSE: click a corner and it gets a field of its own.
//!
//! The kernel and the model are checked separately; this is the wiring. The way of setting it must be
//! the same gesture as everything else in a Part: click the geometry and get a field AT IT, with an
//! expression and Enter/Esc. A list in the right panel would be a separate mechanic for one tool.
#[cfg(test)]
mod tests {
    use super::super::{App, Sel};

    fn rect() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 700.0))
    }

    /// A plate in a part, the fillet open, the edges of the top face picked. Returns (app, body).
    fn plate_in_fillet() -> (App, u64) {
        let mut app = App::default();
        let part = app.project.add_part("part");
        app.enter_component(part);
        let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        app.project.add_rect_entity(si, 0.0, 0.0, 60.0, 40.0, qymcad_core::feature::Purpose::Real);
        app.project.regen_sketch(si);
        app.finish_sketch_edit();
        app.sel = Sel::Sketch(si);
        app.start_feat_cmd(1);
        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "height") {
            p.val = 12.0;
            p.txt = "12".into();
        }
        app.apply_feat_cmd();
        app.rebuild_if_dirty();
        let body = app.project.timeline.iter().rev().find_map(|n| n.kind.body()).expect("the body");

        app.select_body(body);
        app.start_feat_cmd(4);
        app.refresh_edges();
        app.mode_3d = true;
        app.cam.init = true;
        app.cam.scale = 8.0;
        app.cam.target = [30.0, 20.0, 6.0];
        // the edges of the top face
        let top: Vec<u32> = app.project.regen_edges[&body].iter().filter(|e| (e.a[2] - 12.0).abs() < 1e-6 && (e.b[2] - 12.0).abs() < 1e-6).map(|e| e.id).collect();
        assert_eq!(top.len(), 4, "setup: the top face has four edges, and {} were found", top.len());
        app.gsel.edges = top.into_iter().collect();
        (app, body)
    }

    /// A CLICK ON A CORNER GIVES IT A FIELD OF ITS OWN — AND THE FIELD STANDS AT THAT CORNER.
    #[test]
    fn clicking_a_corner_gives_it_its_own_radius_field() {
        let (mut app, body) = plate_in_fillet();
        let r = rect();
        let corner = app.project.vertex_spots(body).into_iter().find(|(p, _)| p[2] > 11.0 && p[0] < 1.0 && p[1] < 1.0).map(|(p, _)| p).expect("the top corner at (0,0)");
        let basis = app.cam.basis();
        let at = app.project3(corner, r, &basis).0;

        assert!(app.pick_fillet_vertex(r, at), "a click on a corner must land on the vertex");
        let extra: Vec<_> = app.cmd.params.iter().filter(|p| p.key.starts_with("at")).collect();
        assert_eq!(extra.len(), 1, "a clicked corner must get EXACTLY one field of its own");
        assert!(extra[0].at.is_some(), "the field must stand AT THE GEOMETRY — otherwise six identical fields in a column cannot be told apart");
        let placed = extra[0].at.expect("the place of the field");
        assert!((placed[0] - corner[0]).abs() < 1e-6 && (placed[2] - corner[2]).abs() < 1e-6, "the field must stand at THE corner that was clicked");

        // a second click — the corner goes back to the common radius
        assert!(app.pick_fillet_vertex(r, at), "a second click must land in the same place");
        assert!(!app.cmd.params.iter().any(|p| p.key.starts_with("at")), "a second click must REMOVE the field, not add a second one");
    }

    /// AND THE VALUE REACHES THE TIMELINE AS A VERTEX TABLE instead of being lost in the interface.
    #[test]
    fn the_corner_radius_reaches_the_timeline() {
        let (mut app, body) = plate_in_fillet();
        let r = rect();
        let corner = app.project.vertex_spots(body).into_iter().find(|(p, _)| p[2] > 11.0 && p[0] < 1.0 && p[1] < 1.0).map(|(p, _)| p).expect("the top corner");
        let basis = app.cam.basis();
        app.pick_fillet_vertex(r, app.project3(corner, r, &basis).0);
        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key.starts_with("at")) {
            p.val = 2.0;
            p.txt = "2".into();
        }
        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "radius") {
            p.val = 1.0;
            p.txt = "1".into();
        }
        app.apply_feat_cmd();
        app.rebuild_if_dirty();

        let node = app
            .project
            .timeline
            .iter()
            .find_map(|n| match n.kind {
                qymcad_core::feature::FeatureKind::Fillet { src, ref at_vertices, .. } => Some((n.id, src, at_vertices.clone())),
                _ => None,
            })
            .expect("the fillet must appear in the timeline");
        assert_eq!(node.2.len(), 1, "the vertex table must reach the timeline");
        assert!(!app.project.regen_errors.contains_key(&node.0), "and the body must build: {:?}", app.project.regen_errors.get(&node.0));

        // the reference leads to THE corner that was clicked
        let found = app.project.resolve_vertex_refs(node.1, &node.2[0].0, "ref-what-fillet-vertex").expect("the reference must resolve");
        let pt = app.project.vertex_point(node.1, found[0]).expect("the place of the vertex");
        assert!((pt[0] - corner[0]).abs() < 1e-6 && (pt[1] - corner[1]).abs() < 1e-6, "the radius was recorded on the wrong corner: {pt:?} instead of {corner:?}");
    }

    /// THE VERTEX RADIUS FIELD IS TRANSLATED — otherwise a person sees a catalogue key.
    #[test]
    fn the_field_speaks_the_users_language() {
        for (code, _) in crate::i18n::available() {
            let prev = crate::i18n::language();
            crate::i18n::set_language(&code);
            let s = crate::i18n::tr("f-radius-at-vertex");
            crate::i18n::set_language(&prev);
            assert!(!s.is_empty() && s != "f-radius-at-vertex", "in language {code} the vertex radius field has no translation");
        }
    }
}
