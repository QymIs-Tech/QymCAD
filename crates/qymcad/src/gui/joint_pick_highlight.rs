//! HIGHLIGHTING AN EDGE AND A VERTEX WHILE ASSEMBLING A MATE.
//!
//! Reported behaviour: picking edges and vertices in the tools is broken AGAIN — it simply is not
//! drawn.
//!
//! The word "again" is what decides what the check must be: the trouble kept coming back, so what has
//! to be looked at is the FRAME — what is drawn under the cursor — and not the internal flags. The
//! flags were fine the previous times too.
#[cfg(test)]
mod tests {
    use super::super::App;
    use qymcad_core::model::Id;

    fn viewport() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 700.0))
    }

    /// An assembly of two parts; standing in the root, as a person does when placing a joint.
    fn assembly_of_two(app: &mut App) -> (Id, Id) {
        super::super::joint_flow::tests::add_part_at(app, 0.0);
        super::super::joint_flow::tests::add_part_at(app, 60.0);
        for _ in 0..4 {
            if qymcad_ui_state::current_ctx_id(&app.active_path, &app.project) == app.project.root {
                break;
            }
            app.exit_context();
        }
        qymcad_ui_state::rebuild_if_dirty(&mut app.rebuild_ctx());
        let a = app.project.mesh_id(0).expect("body A");
        let b = app.project.mesh_id(1).expect("body B");
        app.viewing.mode_3d = true;
        app.viewing.cam.init = true;
        app.viewing.cam.scale = 6.0;
        app.viewing.cam.target = [30.0, 10.0, 5.0];
        (a, b)
    }

    /// The screen point of the midpoint of an edge of the body.
    fn edge_mid_on_screen(app: &mut App, body: Id) -> egui::Pos2 {
        crate::gui::commands::refresh_edges(&mut app.part_ctx());
        crate::gui::io_jobs::ensure_brep(&mut app.rebuild_ctx());
        let e = app.project.regen_edges.get(&body).and_then(|es| es.first().cloned()).expect("the body has edges");
        let basis = app.viewing.cam.basis();
        qymcad_ui_state::Screen { cam: &app.viewing.cam, set: &app.set, rect: viewport(), basis: &basis }.at(e.mid).0
    }

    /// How many shapes the highlight drew with the cursor at `at`.
    ///
    /// The SHAPES OF THE FRAME are counted, not the flags: a highlight is what a person sees. An empty
    /// frame means they are aiming blind.
    fn highlight_shapes(app: &mut App, at: Option<egui::Pos2>, mode: u8) -> usize {
        qymcad_assembly::set_joint_anchor_mode_for_test(&mut app.joint_ctx(), mode);
        app.arm_joint_pick_for_test();
        crate::gui::commands::refresh_edges(&mut app.part_ctx());
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        let mut count = 0;
        // TWO FRAMES: egui learns the cursor position from an event, and until the next frame there
        // is no hover yet — this has already cost time in the popup checks.
        for _ in 0..2 {
            let mut input = egui::RawInput { screen_rect: Some(viewport()), ..Default::default() };
            if let Some(p) = at {
                input.events.push(egui::Event::PointerMoved(p));
            }
            let out = ctx.run_ui(input, |c| {
                egui::CentralPanel::default().show(c, |ui| {
                    let painter = ui.painter().clone();
                    crate::gui::render::draw_joint_pick_highlight(&app.painting(), &painter, viewport());
                });
            });
            count = out.shapes.len();
        }
        count
    }

    /// AN EDGE UNDER THE CURSOR IS HIGHLIGHTED.
    #[test]
    fn an_edge_under_the_cursor_is_highlighted() {
        let mut app = App::default();
        let (a, _b) = assembly_of_two(&mut app);
        let at = edge_mid_on_screen(&mut app, a);

        let empty = highlight_shapes(&mut app, None, 1);
        let hovered = highlight_shapes(&mut app, Some(at), 1);
        assert!(
            hovered > empty,
            "an edge under the cursor is not highlighted: {empty} shapes without the cursor, {hovered} with it — a person is aiming blind"
        );
    }

    /// A VERTEX UNDER THE CURSOR IS HIGHLIGHTED.
    #[test]
    fn a_vertex_under_the_cursor_is_highlighted() {
        let mut app = App::default();
        let (a, _b) = assembly_of_two(&mut app);
        // Aim at the END of the edge — that is where the vertex lives.
        crate::gui::commands::refresh_edges(&mut app.part_ctx());
        crate::gui::io_jobs::ensure_brep(&mut app.rebuild_ctx());
        let e = app.project.regen_edges.get(&a).and_then(|es| es.first().cloned()).expect("an edge");
        let basis = app.viewing.cam.basis();
        let at = qymcad_ui_state::Screen { cam: &app.viewing.cam, set: &app.set, rect: viewport(), basis: &basis }.at(e.a).0;

        let empty = highlight_shapes(&mut app, None, 2);
        let hovered = highlight_shapes(&mut app, Some(at), 2);
        assert!(hovered > empty, "a vertex under the cursor is not highlighted: {empty} shapes without the cursor, {hovered} with it");
    }

    /// AWAY FROM THE GEOMETRY NOTHING LIGHTS UP. Otherwise the highlight lies about a hit.
    #[test]
    fn pointing_at_empty_space_highlights_nothing() {
        let mut app = App::default();
        let (a, _b) = assembly_of_two(&mut app);
        let _ = edge_mid_on_screen(&mut app, a);

        let empty = highlight_shapes(&mut app, None, 1);
        let far = highlight_shapes(&mut app, Some(egui::pos2(880.0, 680.0)), 1);
        assert_eq!(far, empty, "the highlight lit up where there is no geometry");
    }

    /// THE HIGHLIGHT SURVIVES PLAYING WITH THE VISIBILITY CHECKBOXES.
    ///
    /// This appears to have been the reported trouble, and the same one as the hidden-parts case: the
    /// list of visible bodies was cached by the pair "rebuild + context", and a checkbox changed
    /// neither of them. Hide a part and show it back — the cache stayed yesterday's, and BOTH THE
    /// PICKING AND THE HIGHLIGHT went by it. From the outside that is exactly "picking edges and
    /// vertices simply is not drawn".
    ///
    /// This check stands apart from the hidden-parts one: there the PICKING was measured, here it is
    /// the FRAME.
    #[test]
    fn the_highlight_survives_a_visibility_toggle() {
        let mut app = App::default();
        let (a, _b) = assembly_of_two(&mut app);
        let at = edge_mid_on_screen(&mut app, a);
        let owner = app.project.body_owner(a).expect("the part");

        let empty = highlight_shapes(&mut app, None, 1);
        assert!(highlight_shapes(&mut app, Some(at), 1) > empty, "setup: the edge is highlighted");

        crate::gui::set_component_visible(&mut app.project, &mut app.regen, owner, false);
        qymcad_ui_state::rebuild_if_dirty(&mut app.rebuild_ctx());
        assert_eq!(highlight_shapes(&mut app, Some(at), 1), empty, "an edge of a HIDDEN part lit up");

        crate::gui::set_component_visible(&mut app.project, &mut app.regen, owner, true);
        qymcad_ui_state::rebuild_if_dirty(&mut app.rebuild_ctx());
        assert!(
            highlight_shapes(&mut app, Some(at), 1) > empty,
            "after hiding and showing the highlight is gone — exactly what is seen as \"picking edges is not drawn\""
        );
    }
}
