//! PICKING A TOOL IN THE SKETCHER DOES NOT MOVE THE VIEW.
//!
//! Reported behaviour: in the sketcher, picking the measuring tools and the constraints resets the camera,
//! and so does picking "construction" among the tool options.
//!
//! The view belongs to whoever is working: they zoomed in on a corner to place a dimension there, and the
//! program has no business deciding that the moment is right for a refit. What is measured here is every
//! field the flat view is made of, plus the flat-or-3D mode itself: from a person's seat a snap from a
//! turned view to a flat one is the same "the camera jumped".
#[cfg(test)]
mod tests {
    use super::super::{App, Sel, Vec2};
    use qymcad_core::feature::SketchPlane;

    /// A sketch being edited, with geometry, and a view the person has set up by hand.
    fn sketching_with_a_chosen_view() -> App {
        let mut app = App::default();
        let si = app.create_sketch_on(SketchPlane::default());
        app.project.add_rect_entity(si, -20.0, -20.0, 20.0, 20.0, qymcad_core::feature::Purpose::Real);
        app.project.regen_sketch(si);
        app.sel = Sel::Sketch(si);
        // THE PERSON'S OWN VIEW: zoomed into a corner, not the fit the program would choose.
        app.mode_3d = false;
        app.view.scale = 12.5;
        app.view.center = Vec2::new(18.0, -17.0);
        app.view.initialized = true;
        app
    }

    /// Everything the flat view is made of, plus the mode.
    fn view_of(app: &App) -> (bool, f32, f32, f32, bool) {
        (app.mode_3d, app.view.scale, app.view.center.x, app.view.center.y, app.view.initialized)
    }

    /// EVERY MEASURING TOOL LEAVES THE VIEW ALONE.
    #[test]
    fn taking_a_measuring_tool_does_not_move_the_view() {
        let mut bad: Vec<String> = Vec::new();
        for k in 1u8..=3 {
            let mut app = sketching_with_a_chosen_view();
            let before = view_of(&app);
            app.set_dim_tool_for_test(k);
            let after = view_of(&app);
            if after != before {
                bad.push(format!("  dimension tool {k}: {before:?} -> {after:?}"));
            }
        }
        assert!(bad.is_empty(), "taking a measuring tool moved the view:\n{}", bad.join("\n"));
    }

    /// EVERY CONSTRAINT LEAVES THE VIEW ALONE.
    #[test]
    fn pressing_a_constraint_does_not_move_the_view() {
        let mut bad: Vec<String> = Vec::new();
        for code in 0u8..=11 {
            let mut app = sketching_with_a_chosen_view();
            let before = view_of(&app);
            app.constraint_button(code);
            let after = view_of(&app);
            if after != before {
                bad.push(format!("  constraint {code}: {before:?} -> {after:?}"));
            }
        }
        assert!(bad.is_empty(), "pressing a constraint moved the view:\n{}", bad.join("\n"));
    }

    /// THE CONSTRUCTION SWITCH LEAVES THE VIEW ALONE.
    #[test]
    fn switching_construction_does_not_move_the_view() {
        let mut app = sketching_with_a_chosen_view();
        let before = view_of(&app);
        app.run_command("sketch.construction");
        assert_eq!(view_of(&app), before, "switching to construction moved the view");
        app.run_command("sketch.construction");
        assert_eq!(view_of(&app), before, "switching construction back moved the view");
    }

    /// A DRAWING TOOL LEAVES THE VIEW ALONE TOO — the same law, and it is the commonest press of all.
    #[test]
    fn taking_a_drawing_tool_does_not_move_the_view() {
        let mut bad: Vec<String> = Vec::new();
        for k in 1u8..=11 {
            let mut app = sketching_with_a_chosen_view();
            let before = view_of(&app);
            app.set_sk_tool_for_test(k);
            let after = view_of(&app);
            if after != before {
                bad.push(format!("  drawing tool {k}: {before:?} -> {after:?}"));
            }
        }
        assert!(bad.is_empty(), "taking a drawing tool moved the view:\n{}", bad.join("\n"));
    }
}
