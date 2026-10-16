//! THE DIMENSIONS OF A RECTANGLE TURN WITH IT, through the window: a width and a height put by the dimension tool -
//! horizontal and vertical as the sides stood - lie along their sides after the rectangle is turned by Rotate, read
//! the same width and height, and lie along them again after it is turned back.
//!
//! Reported behaviour (#71): turned by 30 deg, the rectangle went round and its dimensions stayed horizontal and
//! vertical where they stood, measuring along the axes rather than along the sides.
#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use super::super::{App, Sel};
    use qymcad_core::feature::SketchPlane;
    use qymcad_core::model::{Constraint, EntityKind};

    /// A rectangle 40 x 30 from (10, 10), its width dimensioned below it and its height to the right, by the dimension
    /// tool. Answers the app, the sketch, and the indices of the two dimensions. Off the origin: a corner drawn on it is
    /// held there, and Rotate lifting that tie would shift the indices the dimensions are kept by.
    fn dimensioned_rectangle() -> (App, usize, [usize; 2]) {
        let mut app = App::default();
        let si = app.create_sketch_on(SketchPlane::default());
        app.chosen.sel = Sel::Sketch(si);
        Hand::new(&mut app).sk_tool(2).click2d(10.0, 10.0).click2d(50.0, 40.0).key(egui::Key::Enter);
        let mut dims = [0usize; 2];
        for (k, (side, place)) in [((30.0, 10.0), (30.0, 2.0)), ((50.0, 25.0), (58.0, 25.0))].into_iter().enumerate() {
            let mut hand = Hand::new(&mut app);
            hand.sk_tool(0);
            assert!(hand.press_hint(&crate::i18n::tr("tb-dim-hint")), "the dimension tool");
            hand.click2d(side.0, side.1);
            dims[k] = hand.app.tools.place.dim.expect("a dimension being placed");
            hand.click2d(place.0, place.1);
            hand.key(egui::Key::Enter).key(egui::Key::Escape);
        }
        (app, si, dims)
    }

    /// Where the window draws dimension `ci`: the direction of its line on the sheet, and its value.
    fn drawn(app: &App, si: usize, ci: usize) -> (egui::Vec2, f64) {
        let c = &app.project.sketches[si].constraints[ci];
        let sheet = qymcad_ui_state::Sheet { view: app.viewing.view, rect: app.viewing.view_rect };
        let (a, b, _) = qymcad_ui_state::linear_dim_line(&app.project, si, c, &sheet).expect("the line of a dimension");
        let Constraint::Distance { d, .. } = *c else { panic!("a linear dimension") };
        ((b - a).normalized(), d)
    }

    /// The direction on the sheet of the side measured by dimension `ci`.
    fn side(app: &App, si: usize, ci: usize) -> egui::Vec2 {
        let Constraint::Distance { a, b, .. } = app.project.sketches[si].constraints[ci] else { panic!("a linear dimension") };
        let sheet = qymcad_ui_state::Sheet { view: app.viewing.view, rect: app.viewing.view_rect };
        let at = |id: u64| app.project.sketches[si].points.iter().find(|q| q.id == id).map(|q| sheet.at(qymcad_core::geom::Point2::new(q.x, q.y))).expect("a point");
        (at(b) - at(a)).normalized()
    }

    /// Whether the line of dimension `ci` stands outside the rectangle: farther from its middle `centre` than the side.
    fn outside(app: &App, si: usize, ci: usize, centre: (f64, f64)) -> bool {
        let c = &app.project.sketches[si].constraints[ci];
        let sheet = qymcad_ui_state::Sheet { view: app.viewing.view, rect: app.viewing.view_rect };
        let (a, b, _) = qymcad_ui_state::linear_dim_line(&app.project, si, c, &sheet).expect("the line of a dimension");
        let Constraint::Distance { a: pa, b: pb, .. } = *c else { return false };
        let at = |id: u64| app.project.sketches[si].points.iter().find(|q| q.id == id).map(|q| sheet.at(qymcad_core::geom::Point2::new(q.x, q.y))).expect("a point");
        let mid = sheet.at(qymcad_core::geom::Point2::new(centre.0, centre.1));
        (a + (b - a) / 2.0).distance(mid) > (at(pa) + (at(pb) - at(pa)) / 2.0).distance(mid)
    }

    fn along_their_sides(app: &App, si: usize, dims: [usize; 2], when: &str) -> Vec<String> {
        dims.iter()
            .zip([40.0, 30.0])
            .filter_map(|(&ci, want)| {
                let (line, d) = drawn(app, si, ci);
                let s = side(app, si, ci);
                let parallel = line.dot(s).abs() > 0.999;
                let out = outside(app, si, ci, (30.0, 25.0));
                (!parallel || !out || (d - want).abs() > 1e-6).then(|| format!("{when}: dimension {ci} reads {d} along {line:?}, outside {out}; its side runs {s:?}"))
            })
            .collect()
    }

    #[test]
    fn the_dimensions_of_a_rectangle_turn_with_it() {
        let (mut app, si, dims) = dimensioned_rectangle();
        let mut sins = along_their_sides(&app, si, dims, "as drawn");
        assert!(sins.is_empty(), "setup: {sins:?}");
        let side = app.project.sketches[si].entities.iter().find(|e| matches!(e.kind, EntityKind::Line { .. })).map(|e| e.id).expect("a side");
        let spot = Hand::new(&mut app).spot2d((1, side)).expect("a place to pick the side");
        Hand::new(&mut app).sk_rotate(spot, (30.0, 25.0), 30.0);
        sins.extend(along_their_sides(&app, si, dims, "turned by 30 deg"));
        let spot = Hand::new(&mut app).spot2d((1, side)).expect("a place to pick the side");
        Hand::new(&mut app).sk_rotate(spot, (30.0, 25.0), -30.0);
        sins.extend(along_their_sides(&app, si, dims, "turned back"));
        assert!(sins.is_empty(), "the dimensions did not go with their sides:\n{}", sins.join("\n"));
    }
}
