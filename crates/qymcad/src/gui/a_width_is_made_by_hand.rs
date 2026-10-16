//! A WIDTH IS MADE BY HAND.
//!
//! In the model, width appeared before the interface did — the same "visible progress" state as the
//! group: the ability exists, there is nothing to reach it with. Checked here is the whole path:
//! take the tool, point at two walls and a tab, confirm, see the line in the panel, delete it.
//!
//! A width has exactly two supporting walls and a tab between them; it ties the parts symmetrically.
#[cfg(test)]
mod tests {
    use super::super::App;
    use qymcad_core::feature::{AnchorRef, ConstraintKind, FaceKey};
    use qymcad_core::model::Id;

    fn viewport() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 700.0))
    }

    /// An assembly of three parts in the root. Returns their bodies.
    fn three_parts(app: &mut App) -> Vec<Id> {
        for x in [0.0, 60.0, 120.0] {
            super::super::joint_flow::tests::add_part_at(app, x);
        }
        for _ in 0..4 {
            if qymcad_ui_state::current_ctx_id(&app.active_path, &app.project) == app.project.root {
                break;
            }
            app.exit_context();
        }
        qymcad_ui_state::rebuild_if_dirty(&mut app.rebuild_ctx());
        (0..3).filter_map(|i| app.project.mesh_id(i)).collect()
    }

    /// The key of some face of the body — what a click on the geometry yields.
    fn a_face(app: &App, body: Id) -> FaceKey {
        let f = app.project.regen_faces.get(&body).and_then(|fs| fs.first().cloned()).expect("a face of the body");
        FaceKey { index: 0, centroid: [f.centroid.x, f.centroid.y, f.centroid.z], normal: f.normal, id: f.id }
    }

    fn panel_text(app: &mut App) -> Vec<String> {
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        let mut texts = Vec::new();
        for _ in 0..2 {
            let out = ctx.run_ui(egui::RawInput { screen_rect: Some(viewport()), ..Default::default() }, |c| {
                egui::CentralPanel::default().show(c, |ui| app.joints_panel_for_test(ui));
            });
            texts.clear();
            for cs in &out.shapes {
                super::super::screen_keys::tests::collect_text(&cs.shape, &mut texts);
            }
        }
        texts
    }

    /// THE WIDTH MOVES THE TAB AT ONCE. Reported behaviour: "Width applied" was said and the part stood where it
    /// stood - the mate went into the document and nothing solved the assembly. Walls: the +X faces of two grounded
    /// parts 60 apart; the tab: the +X face of the third, 120 from the first, which has to come halfway, to 30.
    #[test]
    fn the_width_moves_the_tab_halfway_at_once() {
        let mut app = App::default();
        let bodies = three_parts(&mut app);
        let comps: Vec<Id> = bodies.iter().map(|b| app.project.body_owner(*b).expect("the owner")).collect();
        app.project.set_grounded(comps[0], true);
        app.project.set_grounded(comps[1], true);
        let plus_x = |app: &App, body: Id| {
            let f = app.project.regen_faces.get(&body).and_then(|fs| fs.iter().find(|f| f.normal[0] > 0.99).cloned()).expect("a face looking along +X");
            FaceKey { index: 0, centroid: [f.centroid.x, f.centroid.y, f.centroid.z], normal: f.normal, id: f.id }
        };
        app.start_width_pick();
        for &b in &bodies {
            let face = plus_x(&app, b);
            qymcad_assembly::width_pick_click(&app.active_path, &mut app.side.joint, &mut app.project, &mut app.status, b, face);
        }
        let was = app.project.component_transform(comps[2])[3];
        qymcad_assembly::width_pick_confirm(&mut app.joint_ctx());
        let at = |app: &App, c: Id| qymcad_core::feature::apply12(&app.project.world_transform(c), [0.0; 3])[0];
        // the bodies are built 60 apart inside parts standing at zero: the tab's face goes from 120 to 30 past the first
        // wall's, so its part goes by -90
        let tab = at(&app, comps[2]);
        assert!((tab - was + 90.0).abs() < 1e-3, "the tab's part stood at {was} and after the width stands at {tab}: -90 was expected; the program says {:?}", app.status);
    }

    /// TWO WALLS AND A TAB — THE WIDTH EXISTS AND IT IS VISIBLE.
    #[test]
    fn a_person_can_point_at_two_walls_and_a_tab() {
        let mut app = App::default();
        let bodies = three_parts(&mut app);
        assert!(app.project.mate_constraints.is_empty(), "setup: there are no constraints yet");

        app.start_width_pick();
        assert!(app.side.joint.width_pick.is_some(), "the width tool was not taken up");

        let face = a_face(&app, bodies[0]);
        qymcad_assembly::width_pick_click(&app.active_path, &mut app.side.joint, &mut app.project, &mut app.status, bodies[0], face);
        let face = a_face(&app, bodies[1]);
        qymcad_assembly::width_pick_click(&app.active_path, &mut app.side.joint, &mut app.project, &mut app.status, bodies[1], face);
        let face = a_face(&app, bodies[2]);
        qymcad_assembly::width_pick_click(&app.active_path, &mut app.side.joint, &mut app.project, &mut app.status, bodies[2], face);
        assert_eq!(app.side.joint.width_pick.as_ref().map_or(0, |v| v.len()), 3, "{} supports picked instead of three", app.side.joint.width_pick.as_ref().map_or(0, |v| v.len()));

        qymcad_assembly::width_pick_confirm(&mut app.joint_ctx());
        let c = app.project.mate_constraints.first().cloned().expect("the width was not created");
        assert_eq!(c.kind, ConstraintKind::Width, "the wrong constraint was created: {:?}", c.kind);
        assert_eq!(c.anchors.len(), 3, "a width must have two walls and a tab: {:?}", c.anchors);
        assert!(!app.side.joint.width_pick.is_some(), "the tool was not released after confirmation");

        let texts = panel_text(&mut app);
        let want = crate::i18n::name(&c.name);
        assert!(texts.iter().any(|t| t.contains(&want)), "the width is not in the panel: expected \"{want}\", in the frame {texts:?}");
    }

    /// AN INCOMPLETE SET — A REFUSAL OUT LOUD, NOT SILENCE.
    #[test]
    fn two_picks_are_not_enough_and_it_is_said() {
        let mut app = App::default();
        let bodies = three_parts(&mut app);

        app.start_width_pick();
        let face = a_face(&app, bodies[0]);
        qymcad_assembly::width_pick_click(&app.active_path, &mut app.side.joint, &mut app.project, &mut app.status, bodies[0], face);
        let face = a_face(&app, bodies[1]);
        qymcad_assembly::width_pick_click(&app.active_path, &mut app.side.joint, &mut app.project, &mut app.status, bodies[1], face);
        qymcad_assembly::width_pick_confirm(&mut app.joint_ctx());

        assert!(app.project.mate_constraints.is_empty(), "the width was assembled from two supports — there is no tab");
        assert!(app.side.joint.width_pick.is_some(), "the tool was dropped instead of letting the tab be pointed at");
        assert_eq!(app.status, qymcad_ui_state::Notice::refusal(crate::i18n::tr("j-width-need-three")), "the person was not told what is missing: {}", app.status);
    }

    /// THE WALLS MUST FACE THE SAME WAY: otherwise "in the middle" means nothing.
    #[test]
    fn walls_that_face_different_ways_are_refused() {
        let mut app = App::default();
        let bodies = three_parts(&mut app);
        let faces = app.project.regen_faces.get(&bodies[0]).cloned().unwrap_or_default();
        let top = faces.iter().find(|f| f.normal[2] > 0.99).expect("the top").clone();
        let side = faces.iter().find(|f| f.normal[0] > 0.99).expect("the side").clone();
        let key = |f: &qymcad_core::geom::MeshFace| FaceKey { index: 0, centroid: [f.centroid.x, f.centroid.y, f.centroid.z], normal: f.normal, id: f.id };

        app.start_width_pick();
        qymcad_assembly::width_pick_click(&app.active_path, &mut app.side.joint, &mut app.project, &mut app.status, bodies[0], key(&top));
        qymcad_assembly::width_pick_click(&app.active_path, &mut app.side.joint, &mut app.project, &mut app.status, bodies[0], key(&side));
        assert_eq!(app.side.joint.width_pick.as_ref().map_or(0, |v| v.len()), 1, "the second wall faces the other way — it must not be taken");
        assert_eq!(app.status, qymcad_ui_state::Notice::refusal(crate::i18n::tr("j-width-walls-differ")), "the person was not told why the support was not taken: {}", app.status);
        let _ = AnchorRef::Origin;
    }
}
