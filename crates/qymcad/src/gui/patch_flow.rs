//! THE PATCH: the user's path end to end.
//!
//! The kernel and the timeline are checked separately; here it is the wiring: the button, clicking
//! edges with the real pick, the "smooth / by position" switch, Enter, and the fact that what was
//! chosen reached the timeline.
//!
//! The switch is checked separately from the kernel on purpose: "applied and forgotten" is the most
//! common kind of half-finished tool. An option that never reached the node gives a DIFFERENT surface
//! after the very first rebuild, with no way to understand why.
#[cfg(test)]
mod tests {
    use super::super::App;

    fn rect() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 700.0))
    }

    /// A box with the top removed. Returns (app, body).
    fn open_box() -> (App, u64) {
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
        app.mode_3d = true;
        app.cam.init = true;
        app.cam.scale = 9.0;
        app.cam.target = [10.0, 10.0, 5.0];
        (app, shell)
    }

    /// The outer edges of the opening: the boundary of the future lid.
    fn rim(app: &App, body: u64) -> Vec<u32> {
        let zmax = app.project.regen_edges[&body].iter().flat_map(|e| [e.a[2], e.b[2]]).fold(f64::MIN, f64::max);
        let (mut x0, mut x1, mut y0, mut y1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
        for e in &app.project.regen_edges[&body] {
            for q in [e.a, e.b] {
                x0 = x0.min(q[0]);
                x1 = x1.max(q[0]);
                y0 = y0.min(q[1]);
                y1 = y1.max(q[1]);
            }
        }
        let on_border = |q: [f64; 3]| (q[0] - x0).abs() < 1e-6 || (q[0] - x1).abs() < 1e-6 || (q[1] - y0).abs() < 1e-6 || (q[1] - y1).abs() < 1e-6;
        app.project.regen_edges[&body]
            .iter()
            .filter(|e| (e.a[2] - zmax).abs() < 1e-6 && (e.b[2] - zmax).abs() < 1e-6 && on_border(e.a) && on_border(e.b))
            .map(|e| e.id)
            .collect()
    }

    /// The button exists.
    #[test]
    fn the_tool_has_a_button() {
        assert!(crate::gui::panels_source::PANELS.contains("self.start_feat_cmd(32)"), "without a button the tool does not exist for a person");
    }

    /// THE WHOLE PATH: the button, clicks on edges with the real pick, Enter, the surface in the timeline.
    #[test]
    fn edges_can_be_picked_and_the_patch_reaches_the_timeline() {
        let (mut app, body) = open_box();
        app.start_feat_cmd(32);
        assert_eq!(app.cmd.kind, 32, "the command must open");
        app.refresh_edges();

        let want = rim(&app, body);
        assert_eq!(want.len(), 4, "setup: the opening has four outer edges, and {} were found", want.len());
        let basis = app.cam.basis();
        for id in &want {
            let mid = app.project.regen_edges[&body].iter().find(|e| e.id == *id).map(|e| e.mid).expect("the midpoint of the edge");
            app.pick_edge_3d(rect(), app.project3(mid, rect(), &basis).0);
        }
        assert_eq!(app.gsel.edges.len(), 4, "the clicks must SELECT four edges, and the selection is {:?}", app.gsel.edges);

        app.apply_feat_cmd();
        app.rebuild_if_dirty();
        let node = app
            .project
            .timeline
            .iter()
            .find_map(|n| match n.kind {
                qymcad_core::feature::FeatureKind::Patch { body, .. } => Some((n.id, body)),
                _ => None,
            })
            .expect("a patch must appear in the timeline");
        assert!(!app.project.regen_errors.contains_key(&node.0), "the patch must build: {:?}", app.project.regen_errors.get(&node.0));
        assert!(app.project.bodies.iter().any(|b| b.id == node.1 && b.sheet), "a patch is a sheet");
    }

    /// THE "SMOOTH" SWITCH REACHES THE NODE rather than staying in the interface.
    #[test]
    fn the_smooth_switch_reaches_the_node() {
        let (mut app, body) = open_box();
        app.start_feat_cmd(32);
        app.refresh_edges();
        for id in rim(&app, body) {
            app.gsel.edges.insert(id);
        }
        app.opts.patch_tangent = true;
        app.apply_feat_cmd();
        app.rebuild_if_dirty();

        let (id, tangent) = app
            .project
            .timeline
            .iter()
            .find_map(|n| match n.kind {
                qymcad_core::feature::FeatureKind::Patch { tangent, .. } => Some((n.id, tangent)),
                _ => None,
            })
            .expect("the patch is in the timeline");
        assert!(tangent, "\"smooth\" must be recorded in the node, otherwise the surface becomes a different one after a rebuild");
        assert!(!app.project.regen_errors.contains_key(&id), "and a smooth patch must build too: {:?}", app.project.regen_errors.get(&id));
    }

    /// THE SWITCH IS VISIBLE IN THE TOP BAR AND IS TRANSLATED.
    #[test]
    fn the_switch_is_in_the_top_bar_and_speaks_the_users_language() {
        let src = crate::gui::panels_source::PANELS;
        assert!(src.contains("cmd-patch-tangent") && src.contains("cmd-patch-flat"), "the choice must be visible BEFORE Enter, in the command bar at the top");
        for (code, _) in crate::i18n::available() {
            let prev = crate::i18n::language();
            crate::i18n::set_language(&code);
            for key in ["cmd-patch-flat", "cmd-patch-tangent"] {
                let s = crate::i18n::tr(key);
                assert!(!s.is_empty() && s != key, "language {code} has no translation for {key}");
            }
            crate::i18n::set_language(&prev);
        }
    }
}
