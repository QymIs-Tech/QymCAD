//! ARGUING CONSTRAINTS IN THE SKETCHER: what a person sees. The core has long been able to name the arguing
//! set, but it reached the viewport and the panel only through the dimensions - the geometric constraints
//! argued silently while their glyph stayed green. What runs here is exactly the functions the highlight is
//! drawn from.
use super::{App, Rect};
use qymcad_core::model::Constraint;

/// A segment with two incompatible lengths: 30 and 50.
fn app_with_conflict() -> (App, usize) {
    let mut app = App::default();
    let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
    app.project.add_line_entity(si, 0.0, 0.0, 30.0, 0.0, qymcad_core::feature::Purpose::Real);
    app.project.regen_sketch(si);
    // THE ENDS OF THE LINE, asked of the line itself. Taken by position in `points` these used to be
    // the same thing by luck; a sketch also holds the frame of reference, and its points are in that
    // list too - see `Sketch::frame`.
    let (a, b) = match app.project.sketches[si].entities[0].kind {
        qymcad_core::model::EntityKind::Line { a, b } => (a, b),
        ref other => panic!("setup: the first entity is not a line: {other:?}"),
    };
    let s = &mut app.project.sketches[si];
    s.constraints.push(Constraint::Fixed { p: a });
    s.constraints.push(Constraint::Horizontal { a, b });
    s.constraints.push(Constraint::Distance { a, b, d: 30.0, off: 0.0, expr: String::new(), driven: false, axis: 0, at: None });
    s.constraints.push(Constraint::Distance { a, b, d: 50.0, off: 0.0, expr: String::new(), driven: false, axis: 0, at: None });
    app.project.solve_sketch(si);
    (app, si)
}

/// A segment whose GEOMETRIC constraints argue: horizontal plus vertical against a given length.
/// Those are the ones with glyphs - the dimensions are always drawn in the viewport and the toggle does not hide them.
fn app_with_geometric_conflict() -> (App, usize) {
    let mut app = App::default();
    let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
    app.project.add_line_entity(si, 0.0, 0.0, 30.0, 0.0, qymcad_core::feature::Purpose::Real);
    app.project.regen_sketch(si);
    // THE ENDS OF THE LINE, asked of the line itself. Taken by position in `points` these used to be
    // the same thing by luck; a sketch also holds the frame of reference, and its points are in that
    // list too - see `Sketch::frame`.
    let (a, b) = match app.project.sketches[si].entities[0].kind {
        qymcad_core::model::EntityKind::Line { a, b } => (a, b),
        ref other => panic!("setup: the first entity is not a line: {other:?}"),
    };
    let s = &mut app.project.sketches[si];
    s.constraints.push(Constraint::Fixed { p: a });
    s.constraints.push(Constraint::Distance { a, b, d: 30.0, off: 0.0, expr: String::new(), driven: false, axis: 0, at: None });
    s.constraints.push(Constraint::Horizontal { a, b });
    s.constraints.push(Constraint::Vertical { a, b });
    app.project.solve_sketch(si);
    (app, si)
}

/// The arguing set reaches the UI, and the geometry it holds is known by name - the red highlight in the
/// viewport is drawn from it.
#[test]
fn conflict_set_reaches_the_ui_with_its_geometry() {
    let (app, si) = app_with_conflict();
    let diag = qymcad_ui_state::sketch_diag(&app.cache, &app.project, si);
    assert!(diag.conflicts.len() >= 2, "the arguing set is visible to the UI: {:?}", diag.conflicts);
    let mut hot: std::collections::HashSet<qymcad_core::model::Id> = std::collections::HashSet::new();
    for &ci in &diag.conflicts {
        hot.extend(app.project.sketch_constraint_points(si, ci));
    }
    assert_eq!(hot.len(), 2, "both ends of the arguing segment are lit: {hot:?}");
}

/// AN ERROR CANNOT BE SWITCHED OFF BY A CHECKBOX: the "show constraints" toggle hides the ordinary glyphs, but not the arguing ones.
#[test]
fn conflicting_glyphs_stay_visible_with_the_toggle_off() {
    let (mut app, si) = app_with_geometric_conflict();
    let rect = Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(800.0, 600.0));
    app.win.constraints = true;
    let all = qymcad_pick::visible_constraint_glyphs(&app.painting(), rect, si).len();
    app.win.constraints = false;
    let shown = qymcad_pick::visible_constraint_glyphs(&app.painting(), rect, si);
    assert!(all > shown.len(), "the toggle hides the ordinary glyphs: there were {all}, now {}", shown.len());
    assert!(!shown.is_empty(), "but the arguing ones stay visible with the toggle off");
    let conflicts = qymcad_ui_state::sketch_diag(&app.cache, &app.project, si).conflicts;
    assert!(shown.iter().all(|(ci, _, _)| conflicts.contains(ci)), "EXACTLY the arguing ones are shown");
}

/// WITH NO ARGUMENT, a switched-off toggle shows nothing (the highlight does not stick).
#[test]
fn no_conflict_means_nothing_is_forced_on_screen() {
    let (mut app, si) = app_with_geometric_conflict();
    // remove ANY constraint from the arguing set - by contract that is enough
    let ci = *qymcad_ui_state::sketch_diag(&app.cache, &app.project, si).conflicts.iter().next().expect("there is an argument");
    app.project.delete_sketch_constraint(si, ci);
    app.project.solve_sketch(si);
    assert!(qymcad_ui_state::sketch_diag(&app.cache, &app.project, si).conflicts.is_empty(), "the argument is resolved");
    app.win.constraints = false;
    let rect = Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(800.0, 600.0));
    assert!(qymcad_pick::visible_constraint_glyphs(&app.painting(), rect, si).is_empty(), "with no argument the toggle is in charge again");
}

/// A ONE-CLICK RESOLUTION: the "make it a reference" button ends the argument and leaves the dimension on the drawing.
#[test]
fn making_the_conflicting_dimension_driven_from_the_panel_resolves_it() {
    let (mut app, si) = app_with_conflict();
    let ci = *qymcad_ui_state::sketch_diag(&app.cache, &app.project, si).conflicts.iter().max().expect("there is an argument");
    let before = app.project.sketches[si].constraints.len();
    assert!(crate::gui::sketching::make_dim_driven(&mut app.project, &mut app.regen, &mut app.status, si, ci), "the button worked: {}", app.status);
    assert!(qymcad_ui_state::sketch_diag(&app.cache, &app.project, si).conflicts.is_empty(), "the argument is over: {:?}", qymcad_ui_state::sketch_diag(&app.cache, &app.project, si).conflicts);
    assert_eq!(app.project.sketches[si].constraints.len(), before, "the dimension stayed on the drawing rather than being deleted");
    assert!(app.project.sketches[si].constraints[ci].is_driven(), "it is a reference dimension now");
}

/// THE DIAGNOSTICS CACHE DOES NOT LIE: editing the constraints changes the fingerprint, so the set is recomputed rather than sticking.
#[test]
fn the_diagnostic_cache_follows_edits() {
    let (mut app, si) = app_with_conflict();
    assert!(!qymcad_ui_state::sketch_diag(&app.cache, &app.project, si).conflicts.is_empty(), "there is an argument");
    assert!(!qymcad_ui_state::sketch_diag(&app.cache, &app.project, si).conflicts.is_empty(), "a second call (from the cache) answers the same");
    // THE ARGUING DIMENSION ITSELF, not whatever sits at index 3. The sketch also carries the points
    // and constraints of the frame of reference, so a position in the list is not an identity.
    let ci = *qymcad_ui_state::sketch_diag(&app.cache, &app.project, si).conflicts.iter().max().expect("there is an argument to end");
    app.project.delete_sketch_constraint(si, ci);
    app.project.solve_sketch(si);
    assert!(qymcad_ui_state::sketch_diag(&app.cache, &app.project, si).conflicts.is_empty(), "after the edit the cache was recomputed rather than handing back the old answer");
}
