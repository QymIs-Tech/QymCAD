//! WHAT IS HIDDEN CANNOT BE PICKED.
//!
//! Reported behaviour: hide a part or a subassembly in an assembly and try to place a joint — the
//! part is not in the 3D viewport, yet its edges, faces and the rest hang in the air and can still be
//! picked.
//!
//! Hidden means unavailable: a click into its geometry must give neither a highlight nor an anchor.
//! Otherwise a person attaches to what they cannot see.
//!
//! The check measures THE SAME thing the mouse does: it takes the screen point of the midpoint of an
//! edge of the hidden part and asks the program what is there under the cursor.
#[cfg(test)]
mod tests {
    use super::super::App;
    use qymcad_core::model::Id;

    fn viewport() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 700.0))
    }

    /// An assembly of two parts standing apart. Returns (component A, body A, component B, body B).
    fn assembly_of_two(app: &mut App) -> (Id, Id, Id, Id) {
        super::super::joint_flow::tests::add_part_at(app, 0.0);
        super::super::joint_flow::tests::add_part_at(app, 60.0);
        // GO OUT TO THE ROOT OF THE ASSEMBLY, not "one level up".
        //
        // Two measurements in a row corrected the scenario. The first: creating a part ENTERS it, and
        // in the context of a part only its own body is visible ("shown=[(1,27)]" — one part out of
        // two). The second: one exit is not enough — the context stayed INSIDE the part, and inside
        // it its contents are legitimately shown by their own checkboxes however much the part itself
        // is hidden. A person places a joint while standing in the ASSEMBLY, so the check must stand
        // there too.
        for _ in 0..4 {
            if app.current_ctx_id_for_test() == app.project.root {
                break;
            }
            app.exit_context_for_test();
        }
        assert_eq!(app.current_ctx_id_for_test(), app.project.root, "setup: standing in the root of the assembly");
        app.rebuild_if_dirty_for_test();
        let a = app.project.mesh_id(0).expect("body A");
        let b = app.project.mesh_id(1).expect("body B");
        let ca = app.project.body_owner(a).expect("part A");
        let cb = app.project.body_owner(b).expect("part B");
        (ca, a, cb, b)
    }

    /// The screen point of the midpoint of some edge of the body — that is where the mouse aims.
    fn edge_mid_on_screen(app: &mut App, body: Id) -> (egui::Pos2, u32) {
        app.refresh_edges();
        app.ensure_brep_for_test();
        let e = app.project.regen_edges.get(&body).and_then(|es| es.first().cloned()).expect("the body has edges");
        let basis = app.cam.basis();
        (app.project3(e.mid, viewport(), &basis).0, e.id)
    }

    /// A HIDDEN PART OFFERS NEITHER EDGE, NOR VERTEX, NOR FACE.
    #[test]
    fn a_hidden_part_offers_nothing_to_the_pointer() {
        let mut app = App::default();
        let (ca, a, _cb, _b) = assembly_of_two(&mut app);
        app.mode_3d = true;
        app.cam.init = true;
        app.cam.scale = 6.0;
        app.cam.target = [30.0, 10.0, 5.0];

        let (at, _eid) = edge_mid_on_screen(&mut app, a);
        // SETUP: while the part is visible there is something to aim at.
        assert!(app.pick_edge_any(viewport(), at).is_some(), "setup: with a visible part an edge is found under the cursor");

        app.set_component_visible_for_test(ca, false);
        app.rebuild_if_dirty_for_test();

        assert!(
            app.pick_edge_any(viewport(), at).is_none(),
            "an edge of a HIDDEN part can still be picked — a person attaches to what they cannot see"
        );
        assert!(app.pick_vertex_any(viewport(), at).is_none(), "a vertex of a hidden part can be picked");
        assert!(
            !app.shown_bodies().iter().any(|(_, x)| *x == a),
            "a hidden body stayed in the list of visible ones — both the highlight and the picking will follow it"
        );
    }

    /// SHOW IT BACK AND IT IS PICKABLE AGAIN. Otherwise "hide" would have turned into "delete".
    #[test]
    fn showing_it_back_makes_it_pickable_again() {
        let mut app = App::default();
        let (ca, a, _cb, _b) = assembly_of_two(&mut app);
        app.mode_3d = true;
        app.cam.init = true;
        app.cam.scale = 6.0;
        app.cam.target = [30.0, 10.0, 5.0];
        let (at, _eid) = edge_mid_on_screen(&mut app, a);

        app.set_component_visible_for_test(ca, false);
        app.rebuild_if_dirty_for_test();
        assert!(app.pick_edge_any(viewport(), at).is_none(), "setup: what is hidden cannot be picked");

        app.set_component_visible_for_test(ca, true);
        app.rebuild_if_dirty_for_test();
        assert!(app.pick_edge_any(viewport(), at).is_some(), "a part shown back cannot be picked — \"hide\" has become \"delete\"");
    }
}
