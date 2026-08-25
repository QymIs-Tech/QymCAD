//! A SURFACE THROUGH SECTIONS — the wiring.
//!
//! The kernel and the timeline are checked separately; checked here is that the choice in the top bar
//! reaches the node. An option that stays in the interface alone is a half-made tool: after the very
//! first rebuild the geometry comes out DIFFERENT, and there is no way to understand why.
//!
//! The surface deliberately has no button of its own: the question is the same as the loft's ("what
//! should come out of this"), and splitting it between two tools would mean asking a person to answer
//! it BEFORE they opened the command.
#[cfg(test)]
mod tests {
    use super::super::App;

    /// Two square sections at z=0 and z=30. Returns (app, sid+cid per section).
    ///
    /// The set of sections is put in AFTER the command opens: the command opens with a clean slate
    /// (the references are collected inside it), and filling them in earlier would mean checking
    /// something that never happens in real life.
    fn app_with_two_sections() -> (App, Vec<(u64, u64)>) {
        let mut app = App::default();
        let mut picks: Vec<(u64, u64)> = Vec::new();
        let part = app.project.add_part("part");
        app.enter_component(part);
        for (i, (half, z)) in [(20.0_f64, 0.0_f64), (10.0, 30.0)].into_iter().enumerate() {
            let plane = if z == 0.0 {
                qymcad_core::feature::SketchPlane::default()
            } else {
                let pl = app.project.add_plane(qymcad_core::model::WorkPlane { id: 0, name: format!("z{z}"), origin: [0.0, 0.0, z], normal: [0.0, 0.0, 1.0], rot_deg: 0.0, def: Default::default() });
                qymcad_core::feature::SketchPlane::Datum(pl)
            };
            let si = app.create_sketch_on(plane);
            app.project.add_rect_entity(si, -half, -half, half, half, qymcad_core::feature::Purpose::Real);
            app.project.regen_sketch(si);
            app.finish_sketch_edit();
            let sid = app.project.sketches[si].id;
            let cid = app.project.sketches[si].contour_ids.iter().copied().find(|c| app.project.contour_profile_xy(*c).is_some()).expect("the contour of the section");
            picks.push((sid, cid));
            let _ = i;
        }
        (app, picks)
    }

    /// The loft from the timeline: (node id, the "surface" flag, body).
    fn loft_node(app: &App) -> (u64, bool, u64) {
        app.project
            .timeline
            .iter()
            .find_map(|n| match n.kind {
                qymcad_core::feature::FeatureKind::Loft { surface, body, .. } => Some((n.id, surface, body)),
                _ => None,
            })
            .expect("a loft must appear in the timeline")
    }

    /// THE "SURFACE" CHOICE REACHES THE NODE AND YIELDS A SHEET.
    #[test]
    fn the_surface_choice_reaches_the_node() {
        let (mut app, picks) = app_with_two_sections();
        app.start_feat_cmd(9);
        // THE COMMAND MAY HAVE PICKED UP THE ACTIVE SKETCH as the first section — collect our own two
        // from scratch, otherwise a set of three would be checked that nobody chose.
        app.loft.sids.clear();
        app.loft.cids.clear();
        for (sid, cid) in picks {
            app.loft.sids.push(sid);
            app.loft.cids.push(cid);
        }
        app.loft.result = 4; // "Surface" in the top bar
        app.apply_feat_cmd();
        app.rebuild_if_dirty();

        let (id, surface, body) = loft_node(&app);
        assert!(surface, "\"Surface\" must be recorded in the node — otherwise a solid comes out after the rebuild");
        assert!(!app.project.regen_errors.contains_key(&id), "the surface must build: {:?}", app.project.regen_errors.get(&id));
        assert!(app.project.bodies.iter().any(|b| b.id == body && b.sheet), "the result must be a SHEET");
    }

    /// AND WITHOUT IT — STILL A SOLID. Otherwise the cure is worse than the illness.
    #[test]
    fn without_it_the_loft_is_still_a_solid() {
        let (mut app, picks) = app_with_two_sections();
        app.start_feat_cmd(9);
        // THE COMMAND MAY HAVE PICKED UP THE ACTIVE SKETCH as the first section — collect our own two
        // from scratch, otherwise a set of three would be checked that nobody chose.
        app.loft.sids.clear();
        app.loft.cids.clear();
        for (sid, cid) in picks {
            app.loft.sids.push(sid);
            app.loft.cids.push(cid);
        }
        app.apply_feat_cmd();
        app.rebuild_if_dirty();

        let (id, surface, body) = loft_node(&app);
        assert!(!surface, "by default a loft builds a SOLID");
        assert!(!app.project.regen_errors.contains_key(&id), "the solid must build: {:?}", app.project.regen_errors.get(&id));
        let solid = app.project.bodies.iter().find(|b| b.id == body).expect("the body");
        assert!(!solid.sheet && solid.mesh.volume() > 1.0, "a solid must have volume: sheet={} volume={:.2}", solid.sheet, solid.mesh.volume());
    }

    /// THE CHOICE IS VISIBLE IN THE TOP BAR, IN THE TREE, AND IS TRANSLATED.
    #[test]
    fn the_choice_is_visible_and_translated() {
        let src = crate::gui::panels_source::PANELS;
        assert!(src.contains("cmd-surface"), "the choice must be visible BEFORE Enter, in the top bar of the command");
        assert!(src.contains("feat-suffix-surface"), "in the tree a surface node must differ from a solid node");
        for (code, _) in crate::i18n::available() {
            let prev = crate::i18n::language();
            crate::i18n::set_language(&code);
            for key in ["cmd-surface", "feat-suffix-surface"] {
                let s = crate::i18n::tr(key);
                assert!(!s.is_empty() && s != key, "language {code} has no translation for {key}");
            }
            crate::i18n::set_language(&prev);
        }
    }
}
