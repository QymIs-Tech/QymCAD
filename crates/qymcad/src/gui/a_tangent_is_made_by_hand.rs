//! A TANGENCY IS PLACED BY HAND.
//!
//! Tangency already works in the kernel — a shaft settles onto a plate — but there was no way to
//! create one. Here is the whole path: take the tool, point at two surfaces, see the row in the panel,
//! delete it.
//!
//! Tangency is the one mate that needs no connectors: two selected surfaces are enough.
#[cfg(test)]
mod tests {
    use super::super::App;
    use qymcad_core::feature::{ConstraintKind, FaceKey};
    use qymcad_core::model::Id;

    fn viewport() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 700.0))
    }

    /// A plate and a shaft: the plate has planar faces, the shaft a cylindrical one.
    fn plate_and_shaft(app: &mut App) -> (Id, Id) {
        super::super::joint_flow::tests::add_part_at(app, 0.0);
        // the shaft: a circle extruded, which gives it a real cylindrical face
        let root = app.project.root;
        app.project.set_active_component(Some(root));
        let part = app.project.add_part("shaft");
        app.enter_component_for_test(part);
        let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        app.project.add_circle_entity(si, 80.0, 10.0, 5.0, qymcad_core::feature::Purpose::Real);
        app.project.regen_sketch(si);
        app.finish_sketch_edit();
        app.sel = super::super::Sel::Sketch(si);
        app.start_feat_cmd(1);
        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "height") {
            p.val = 40.0;
            p.txt = "40".into();
        }
        app.apply_feat_cmd();
        for _ in 0..4 {
            if app.current_ctx_id_for_test() == app.project.root {
                break;
            }
            app.exit_context_for_test();
        }
        app.rebuild_if_dirty_for_test();
        (app.project.mesh_id(0).expect("the plate"), app.project.mesh_id(1).expect("the shaft"))
    }

    fn flat_face(app: &App, body: Id) -> FaceKey {
        let f = app
            .project
            .regen_faces
            .get(&body)
            .and_then(|fs| fs.iter().find(|f| f.normal[2] > 0.99).cloned())
            .expect("a planar face");
        FaceKey { index: 0, centroid: [f.centroid.x, f.centroid.y, f.centroid.z], normal: f.normal, id: f.id }
    }

    fn round_face(app: &App, body: Id) -> FaceKey {
        app.project
            .regen_faces
            .get(&body)
            .expect("the faces")
            .iter()
            .find_map(|f| {
                let k = FaceKey { index: 0, centroid: [f.centroid.x, f.centroid.y, f.centroid.z], normal: f.normal, id: f.id };
                app.project.face_cylinder(body, &k).map(|_| k)
            })
            .expect("a cylindrical face")
    }

    fn panel_text(app: &mut App) -> Vec<String> {
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        let mut texts = Vec::new();
        for _ in 0..2 {
            let out = ctx.run(egui::RawInput { screen_rect: Some(viewport()), ..Default::default() }, |c| {
                egui::CentralPanel::default().show(c, |ui| app.joints_panel_for_test(ui));
            });
            texts.clear();
            for cs in &out.shapes {
                super::super::screen_keys::tests::collect_text(&cs.shape, &mut texts);
            }
        }
        texts
    }

    /// TWO SURFACES, AND THE TANGENCY EXISTS AND IS VISIBLE.
    #[test]
    fn a_person_can_point_at_a_plane_and_a_cylinder() {
        let mut app = App::default();
        let (plate, shaft) = plate_and_shaft(&mut app);
        app.start_tangent_pick_for_test();
        assert!(app.tangent_pick_active_for_test(), "the Tangency tool was not taken");

        app.tangent_pick_click_for_test(plate, flat_face(&app, plate));
        app.tangent_pick_click_for_test(shaft, round_face(&app, shaft));

        let c = app.project.mate_constraints.first().cloned().expect("the tangency was not created");
        assert_eq!(c.kind, ConstraintKind::Tangent, "the wrong constraint was created: {:?}", c.kind);
        assert_eq!(c.faces.len(), 2, "a tangency must have two surfaces: {:?}", c.faces);
        assert!(!app.tangent_pick_active_for_test(), "the tool was not released after the second pick");

        let texts = panel_text(&mut app);
        let want = crate::i18n::name(&c.name);
        assert!(texts.iter().any(|t| t.contains(&want)), "the tangency is not in the panel: expected \"{want}\", in the frame {texts:?}");
    }

    /// TWO PLANES CANNOT BE TANGENT, AND THAT IS SAID OUT LOUD.
    ///
    /// A tangency holds a distance equal to a radius; a pair of planes has no radius, and "tangency"
    /// between them merely means "they coincide", for which there is a planar mate. Accepting such a
    /// pair silently would mean creating a constraint that does nothing.
    #[test]
    fn two_planes_are_refused_out_loud() {
        let mut app = App::default();
        let (plate, _shaft) = plate_and_shaft(&mut app);
        let f = flat_face(&app, plate);
        app.start_tangent_pick_for_test();
        app.tangent_pick_click_for_test(plate, f.clone());
        app.tangent_pick_click_for_test(plate, f);

        assert!(app.project.mate_constraints.is_empty(), "a tangency was assembled from two planes");
        assert!(app.tangent_pick_active_for_test(), "the tool was dropped instead of letting a cylinder be pointed at");
        assert_eq!(app.status, crate::i18n::tr("j-tangent-need-cylinder"), "the person was not told what is wrong: {}", app.status);
    }
}
