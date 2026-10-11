//! THE TEXT OF A DIMENSION IS LED WITH THE MOUSE: a length's along its line and past an arrow onto a shelf, a
//! diameter's from its shelf onto the dimension line and back.
//!
//! Reported behaviour: the text of a linear dimension could not be moved along its line nor carried out on a shelf the
//! way a diameter's is; the text of a diameter or a radius could only stand on its shelf, never on the dimension line.
#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use super::super::App;
    use qymcad_core::model::Constraint;

    /// A sketch under edit: a line from (0, 0) to (40, 0) with its length dimensioned, and a circle of radius 10 about
    /// (80, 0) with its diameter. Answers the program, the sketch and the two dimensions.
    fn sketch_with_a_length_and_a_diameter() -> (App, usize, usize, usize) {
        let mut app = App::default();
        let part = app.project.add_part("Plate");
        app.enter_component(part);
        let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        qymcad_ui_state::begin_edit(&mut app.disk.edits, &app.project, "setup".to_string());
        let real = qymcad_core::feature::Purpose::Real;
        app.project.add_line_entity(si, 0.0, 0.0, 40.0, 0.0, real);
        app.project.add_circle_entity(si, 80.0, 0.0, 10.0, real);
        let pt = |app: &App, x: f64, y: f64| app.project.sketches[si].points.iter().find(|p| (p.x - x).abs() < 1e-6 && (p.y - y).abs() < 1e-6).map(|p| p.id).expect("a point of the sketch");
        let (a, b, c) = (pt(&app, 0.0, 0.0), pt(&app, 40.0, 0.0), pt(&app, 80.0, 0.0));
        app.project.sketches[si].constraints.push(Constraint::Distance { dimension: Default::default(), a, b, d: 40.0, off: 2.0, expr: String::new(), driven: true, axis: 0, at: None });
        let length = app.project.sketches[si].constraints.len() - 1;
        app.project.sketches[si].constraints.push(Constraint::Diameter { dimension: Default::default(), c, d: 20.0, off: 0.0, expr: String::new(), driven: true, diam: true, at: None });
        let diameter = app.project.sketches[si].constraints.len() - 1;
        app.project.solve_sketch(si);
        app.project.regen_sketch(si);
        qymcad_ui_state::commit_edit(&mut app.rebuild_ctx());
        (app, si, length, diameter)
    }

    fn sheet(app: &App) -> qymcad_ui_state::Sheet {
        qymcad_ui_state::Sheet { view: app.viewing.view, rect: app.viewing.view_rect }
    }

    /// Where the text of the length stands, in sketch units, and whether it stands on a shelf.
    fn length_text(app: &App, si: usize, ci: usize) -> ((f64, f64), bool) {
        let (place, _) = qymcad_ui_state::linear_text_of(&app.project, si, ci, &sheet(app), &app.set).expect("the length has a text");
        let w = qymcad_ui_state::to_world(&app.viewing.view, app.viewing.view_rect, place.center);
        ((w.x, w.y), place.shelf.is_some())
    }

    /// Where the text of the diameter stands, in sketch units, and whether it lies on the dimension line.
    fn diameter_text(app: &App, si: usize, ci: usize) -> ((f64, f64), bool) {
        let g = qymcad_ui_state::radial_dim_geom(&app.project, si, ci, &sheet(app), &app.set).expect("the diameter has a text");
        let w = qymcad_ui_state::to_world(&app.viewing.view, app.viewing.view_rect, g.text);
        ((w.x, w.y), g.shelf.is_none())
    }

    fn at_of(app: &App, si: usize, ci: usize) -> Option<f64> {
        match app.project.sketches[si].constraints[ci] {
            Constraint::Distance { at, .. } | Constraint::Diameter { at, .. } => at,
            _ => None,
        }
    }

    /// THE TEXT OF A LENGTH GOES ALONG ITS LINE, and past the arrow onto a shelf; Ctrl+Z takes one drag back; the place
    /// is kept in the file.
    #[test]
    fn the_text_of_a_length_goes_along_its_line_and_out_onto_a_shelf() {
        let (mut app, si, length, _) = sketch_with_a_length_and_a_diameter();
        let mut h = Hand::new(&mut app);
        h.look2d((40.0, 0.0)).frame(Vec::new());
        let ((x0, y0), shelf) = length_text(h.app, si, length);
        assert!(!shelf && (x0 - 20.0).abs() < 1.0, "setup: the text of the length stands in the middle of its line, and it stands at x {x0}, shelf {shelf}");
        // ALONG: a quarter of the line to the left
        h.drag2d((x0, y0), (x0 - 10.0, y0));
        let ((x1, y1), shelf) = length_text(h.app, si, length);
        assert!((x1 - 10.0).abs() < 1.5 && !shelf, "the text led 10 along its line stands at x {x1} (shelf {shelf}), not at 10");
        // PAST THE ARROW at the far end: onto a shelf
        h.drag2d((x1, y1), (52.0, y1));
        let ((x2, _), shelf) = length_text(h.app, si, length);
        assert!(x2 > 45.0 && shelf, "the text led past the arrow stands at x {x2}, on a shelf: {shelf}");
        assert!(at_of(h.app, si, length).is_some_and(|t| t > 1.0), "the place past the arrow is not kept: {:?}", at_of(h.app, si, length));
        h.undo();
        let ((x3, _), _) = length_text(h.app, si, length);
        assert!((x3 - x1).abs() < 1.5, "Ctrl+Z did not take the last drag back: the text stands at x {x3}, it stood at {x1}");
        let path = format!("{}/dim-text-along.qcad", std::env::temp_dir().display());
        qymcad_io::save_project(&h.app.project, &path).expect("saved");
        let read = qymcad_io::load_project(&path).expect("read back");
        let kept = match read.sketches[si].constraints[length] {
            Constraint::Distance { at, .. } => at,
            _ => None,
        };
        assert_eq!(kept, at_of(h.app, si, length), "the place of the text was not kept in the file");
    }

    /// THE TEXT OF A DIAMETER LIES ON THE DIMENSION LINE once led inside the circle, and goes back onto its shelf once
    /// led out past the rim.
    #[test]
    fn the_text_of_a_diameter_lies_on_its_line_and_goes_back_onto_the_shelf() {
        let (mut app, si, _, diameter) = sketch_with_a_length_and_a_diameter();
        let mut h = Hand::new(&mut app);
        h.look2d((70.0, 0.0)).frame(Vec::new());
        let (from, on_line) = diameter_text(h.app, si, diameter);
        assert!(!on_line, "setup: the text of the diameter stands on its shelf at first");
        // ONTO THE LINE: inside the circle, above its centre and to the right
        h.drag2d(from, (84.0, 3.0));
        let ((x, y), on_line) = diameter_text(h.app, si, diameter);
        assert!(on_line, "the text of the diameter led inside the circle does not lie on the dimension line");
        assert!((x - 80.0).hypot(y) < 10.0, "the text laid on the line stands at ({x}, {y}), outside the circle of radius 10 about (80, 0)");
        // BACK ONTO THE SHELF: past the rim
        h.drag2d((x, y), (100.0, 6.0));
        let (_, on_line) = diameter_text(h.app, si, diameter);
        assert!(!on_line && at_of(h.app, si, diameter).is_none(), "the text led out past the rim did not go back onto its shelf");
        h.undo();
        let (_, on_line) = diameter_text(h.app, si, diameter);
        assert!(on_line, "Ctrl+Z did not put the text back on the dimension line in one step");
    }
}
