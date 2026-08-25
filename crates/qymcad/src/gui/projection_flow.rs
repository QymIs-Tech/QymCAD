//! PROJECTING THE GEOMETRY OF A BODY INTO A SKETCH — the whole path a person takes.
//!
//! The kernel is checked separately (`qymcad-testkit/tests/sketch_projection.rs`); this is the wiring,
//! because it is exactly there that tools turned out to be "done" and yet unreachable: no button, the
//! click does not land, the driven geometry is dragged by the mouse.
#[cfg(test)]
mod tests {
    use super::super::{App, Sel};

    /// A part with a cube plus a sketch ON THE TOP FACE (that is where the backing lives). Returns
    /// (sketch index, body).
    fn part_with_sketch_on_face(app: &mut App) -> (usize, u64) {
        super::super::joint_flow::tests::add_part_at(app, 0.0);
        app.rebuild_if_dirty();
        let body = app.project.mesh_id(0).expect("the body");
        if let Some(owner) = app.project.body_owner(body) {
            app.enter_component(owner);
        }
        let mi = app.project.mesh_index(body).expect("the mesh");
        let key = app.project.bodies[mi]
            .faces
            .iter()
            .filter(|f| f.normal[2] > 0.9)
            .max_by(|a, b| a.centroid.z.partial_cmp(&b.centroid.z).unwrap())
            .map(|f| qymcad_core::feature::FaceKey { index: 0, centroid: [f.centroid.x, f.centroid.y, f.centroid.z], normal: f.normal, id: f.id })
            .expect("the top face is there");
        let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::Face(body, key));
        app.sel = Sel::Sketch(si);
        app.rebuild_if_dirty();
        (si, body)
    }

    /// The button exists, and it switches on exactly this tool.
    #[test]
    fn the_tool_has_a_button() {
        assert!(crate::gui::panels_source::PANELS.contains("self.set_click_op(6);"), "without a button the operation does not exist for a person");
        assert!(include_str!("sketching.rs").contains("} else if self.tool.click_op == 6 {"), "a click in a sketch must lead to the projection");
    }

    /// THE WHOLE FACE-OUTLINE PATH: button -> mode -> click -> driven geometry in the sketch.
    #[test]
    fn a_face_outline_can_be_projected_from_the_toolbar() {
        let mut app = App::default();
        let (si, _body) = part_with_sketch_on_face(&mut app);
        let ents_before = app.project.sketches[si].entities.len();

        app.set_click_op(6);
        assert_eq!(app.tool.click_op, 6, "the projection tool must switch on");
        app.tool.proj_face = true; // the face-outline toggle in the top bar

        let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 700.0));
        app.project_clicked_edge(si, rect, egui::pos2(450.0, 350.0));

        assert_eq!(app.project.sketches[si].projections.len(), 1, "the projection must appear; status: {}", app.status);
        let ents = app.project.sketches[si].entities.len();
        assert!(ents > ents_before, "driven entities must appear in the sketch: it was {ents_before}, it became {ents}");
        assert_eq!(app.project.sketches[si].projected_entities().len(), 4, "the outline of a square face is four edges");
    }

    /// THE WHOLE EDGE PATH: a click near an edge of the backing takes THAT EDGE and not everything
    /// at once.
    #[test]
    fn clicking_near_an_edge_projects_that_edge() {
        let mut app = App::default();
        let (si, _body) = part_with_sketch_on_face(&mut app);
        app.set_click_op(6);
        app.tool.proj_face = false;
        app.view.initialized = true;
        app.view.scale = 8.0;
        let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 700.0));

        // take a real point ON an edge of the backing and click exactly into it, as the mouse does
        let (_body, edges) = app.sketch_ref_edges_2d_ids(si);
        assert!(!edges.is_empty(), "the backing must give edges — otherwise there is nothing to click");
        let mid = {
            let poly = &edges[0].1;
            let (a, b) = (poly[0], poly[poly.len() - 1]);
            qymcad_core::geom::Point2::new((a.x + b.x) * 0.5, (a.y + b.y) * 0.5)
        };
        let at = app.to_screen_pub(rect, mid);
        app.project_clicked_edge(si, rect, at);

        assert_eq!(app.project.sketches[si].projections.len(), 1, "a click on an edge must project it; status: {}", app.status);
        assert_eq!(app.project.sketches[si].projected_entities().len(), 1, "ONE edge was projected, not the whole outline");
    }

    /// A miss past the edges gives an honest message rather than silent nothing.
    #[test]
    fn a_miss_says_so() {
        let mut app = App::default();
        let (si, _body) = part_with_sketch_on_face(&mut app);
        app.set_click_op(6);
        app.tool.proj_face = false;
        app.view.initialized = true;
        app.view.scale = 8.0;
        let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 700.0));
        app.project_clicked_edge(si, rect, egui::pos2(880.0, 690.0)); // deliberately past the part

        assert!(app.project.sketches[si].projections.is_empty(), "a miss means nothing is created");
        assert_eq!(app.status, crate::i18n::tr("sk-miss-click-edge"), "the reason must be said");
    }

    /// DRIVEN GEOMETRY IS NOT DRAGGED BY THE MOUSE — its position is set by the part.
    #[test]
    fn projected_geometry_is_not_draggable() {
        let mut app = App::default();
        let (si, _body) = part_with_sketch_on_face(&mut app);
        app.set_click_op(6);
        app.tool.proj_face = true;
        let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 700.0));
        app.project_clicked_edge(si, rect, egui::pos2(450.0, 350.0));

        // a group move: even if the driven points ended up in the selection, they must not move
        let driven: Vec<u64> = app.project.sketches[si].projected_points().into_iter().collect();
        assert!(!driven.is_empty(), "setup: the driven points are there");
        app.sel_sk.items = driven.iter().map(|id| (0u8, *id)).collect();
        let movable = app.sketch_sel_points(si);
        assert!(movable.is_empty(), "the driven points must drop out of the move, and it holds {movable:?}");
    }

    /// DELETING a driven entity removes the projection WHOLE — no half-alive record is left.
    #[test]
    fn deleting_projected_geometry_removes_the_whole_projection() {
        let mut app = App::default();
        let (si, _body) = part_with_sketch_on_face(&mut app);
        app.set_click_op(6);
        app.tool.proj_face = true;
        let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 700.0));
        app.project_clicked_edge(si, rect, egui::pos2(450.0, 350.0));
        let one: u64 = *app.project.sketches[si].projections[0].entities.first().expect("the entity is there");

        app.sel_sk.items = vec![(1u8, one)];
        app.delete_sketch_sel_pub(si);

        assert!(app.project.sketches[si].projections.is_empty(), "the projection must go whole");
        assert!(app.project.sketches[si].projected_entities().is_empty(), "no driven entities are left");
    }

    /// Driven geometry IS VISIBLY driven — otherwise it is not clear why it does not drag.
    #[test]
    fn projected_geometry_is_drawn_differently() {
        let src = crate::gui::render_source::RENDER;
        assert!(src.contains("pub(super) fn draw_projection_overlay"), "driven geometry must have a drawing layer of its own");
        assert!(include_str!("sketching.rs").contains("self.draw_projection_overlay(&painter, rect);"), "the layer must be called from the sketch frame");
        let a = src.find("pub(super) fn draw_projection_overlay").expect("the block");
        let b = src[a..].find("\n    /// The ghost of an insertion").map(|i| a + i).unwrap_or(src.len());
        assert!(src[a..b].contains("proj.lost"), "a lost source must be visible separately");
    }
}
