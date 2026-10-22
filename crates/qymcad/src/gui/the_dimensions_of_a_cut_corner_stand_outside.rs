//! THE DIMENSIONS OF A CUT CORNER STAND OUTSIDE THE SHAPE, through the window, in every mode of the tool: the legs of
//! a chamfer of two legs, the leg of a chamfer of a leg and an angle, the chord of a fillet - each drawn off to the side
//! away from the shape; the angle of a chamfer between the line run on to the sharp and the cut, its arc reaching both
//! past the end of the cut;
//! the radius of a fillet led from the centre out through the arc, its text clear of the sharp.
//!
//! Reported behaviour: on a square, one leg of a chamfer of two legs stood inside, the leg of a chamfer of a leg and an
//! angle stood inside, the angle was drawn now outside and now short of the line, the radius of a fillet was led from
//! the centre away from the arc into the shape, and the chord of a fillet stood inside.
#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use super::super::{App, Sel};
    use qymcad_core::feature::SketchPlane;
    use qymcad_core::geom::Point2;
    use qymcad_core::model::Constraint;

    /// A square of 40 x 30 drawn by the line tool from (0, 0), one closed chain, and the select tool taken.
    fn square() -> (App, usize) {
        let mut app = App::default();
        let si = app.create_sketch_on(SketchPlane::default());
        app.chosen.sel = Sel::Sketch(si);
        let mut hand = Hand::new(&mut app);
        hand.sk_tool(qymcad_ui_state::SketchShape::Line);
        for (x, y) in [(0.0, 0.0), (40.0, 0.0), (40.0, 30.0), (0.0, 30.0), (0.0, 0.0)] {
            hand.click2d(x, y);
        }
        hand.key(egui::Key::Escape);
        Hand::new(&mut app).sk_select();
        (app, si)
    }

    /// The middle of the square, inside it.
    const INSIDE: Point2 = Point2 { x: 20.0, y: 15.0 };

    /// The corner of the square at `at` cut by the tool of `key` in the mode of the word `mode`, with `value`.
    fn cut(app: &mut App, si: usize, at: (f64, f64), key: &str, mode: Option<&str>, value: f64) {
        let s = &app.project.sketches[si];
        let near = |id: u64| s.points.iter().any(|q| q.id == id && (q.x - at.0).hypot(q.y - at.1) < 1e-6);
        let lines: Vec<qymcad_ui_state::SketchItem> =
            s.entities.iter().filter(|e| e.kind.as_line().is_some_and(|qymcad_core::model::LineParts { a, b }| near(a) || near(b))).map(|e| qymcad_ui_state::SketchItem::curve(e.id)).collect();
        assert_eq!(lines.len(), 2, "setup: two lines meet at {at:?}");
        assert!(Hand::new(app).sk_corner_set_in(key, mode, &lines, value), "no place to click on the two lines");
    }

    fn sheet(app: &App) -> qymcad_ui_state::Sheet {
        qymcad_ui_state::Sheet { view: app.viewing.view, rect: app.viewing.view_rect }
    }

    fn on_screen(app: &App, si: usize, id: u64) -> egui::Pos2 {
        let q = app.project.sketches[si].points.iter().find(|q| q.id == id).expect("a point");
        sheet(app).at(Point2::new(q.x, q.y))
    }

    /// Whether the linear dimension from `a` to `b` is drawn off to the side away from `inside`: the left of its first
    /// end looking at the second, on screen, as the renderer lays it.
    fn stands_away(app: &App, si: usize, (a, b): (u64, u64), inside: Point2) -> bool {
        let (sa, sb) = (on_screen(app, si, a), on_screen(app, si, b));
        let dir = (sb - sa).normalized();
        let perp = egui::vec2(-dir.y, dir.x);
        perp.dot(sheet(app).at(inside) - (sa + (sb - sa) / 2.0)) < 0.0
    }

    /// Every linear dimension of the sketch of the value `d`, by its two ends.
    fn distances_of(app: &App, si: usize, d: f64) -> Vec<(u64, u64)> {
        app.project.sketches[si]
            .constraints
            .iter()
            .filter_map(|c| match *c {
                Constraint::Distance { a, b, d: v, .. } if (v - d).abs() < 1e-9 => Some((a, b)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn both_legs_of_a_chamfer_of_two_legs_stand_outside() {
        let (mut app, si) = square();
        cut(&mut app, si, (0.0, 0.0), "tb-chamfer-sketch-hint", Some("cmd-two-distances"), 3.0);
        let legs = distances_of(&app, si, 3.0);
        assert_eq!(legs.len(), 2, "two legs of 3: {:?}", app.project.sketches[si].constraints);
        for leg in legs {
            assert!(stands_away(&app, si, leg, INSIDE), "a leg {leg:?} of the chamfer is drawn into the square");
        }
    }

    #[test]
    fn the_leg_and_the_angle_of_a_chamfer_stand_outside() {
        let (mut app, si) = square();
        cut(&mut app, si, (40.0, 0.0), "tb-chamfer-sketch-hint", Some("cmd-leg-angle"), 3.0);
        let legs = distances_of(&app, si, 3.0);
        assert_eq!(legs.len(), 1, "one leg of 3: {:?}", app.project.sketches[si].constraints);
        assert!(stands_away(&app, si, legs[0], INSIDE), "the leg {:?} of the chamfer is drawn into the square", legs[0]);
        let s = &app.project.sketches[si];
        let (ci, (b, d)) = s
            .constraints
            .iter()
            .enumerate()
            .find_map(|(ci, c)| match *c {
                Constraint::AngleLines { b, d, .. } => Some((ci, (b, d))),
                _ => None,
            })
            .expect("the angle of the chamfer");
        let g = qymcad_ui_state::angle_dim_geom(&app.project, si, ci, &sheet(&app), &app.set).expect("the angle has a geometry");
        // the sides run from the end of the cut on the first line: on along that line to the sharp, and along the cut
        let (t1, t2) = (on_screen(&app, si, b), on_screen(&app, si, d));
        let sharp = sheet(&app).at(Point2::new(40.0, 0.0));
        let first = egui::vec2(g.a0.cos(), g.a0.sin());
        let second = egui::vec2((g.a0 + g.sweep).cos(), (g.a0 + g.sweep).sin());
        assert!(g.center.distance(t1) < 1.0, "the angle is not drawn at the end of the cut: {:?} against {t1:?}", g.center);
        assert!(first.dot((sharp - t1).normalized()) > 0.999, "the first side of the angle does not run on to the sharp: {first:?}");
        assert!(second.dot((t2 - t1).normalized()) > 0.999, "the second side of the angle does not run along the cut: {second:?}");
        assert!((g.sweep.abs().to_degrees() - 45.0).abs() < 0.5, "the arc does not span the angle of 45 deg: {} deg", g.sweep.to_degrees());
        // THE ARC STANDS PAST THE END OF THE CUT, where it reads: drawn at the radius of every angle, 24 px, it was a
        // tick tucked into the corner of the cut and the line
        let cut = t1.distance(t2);
        assert!(g.r > 1.2 * cut, "the arc of the angle does not stand past the end of the cut, {} px against a cut of {cut} px", g.r);
    }

    #[test]
    fn the_radius_of_a_fillet_is_led_out_through_its_arc() {
        let (mut app, si) = square();
        cut(&mut app, si, (0.0, 30.0), "tb-fillet-sketch-hint", None, 3.0);
        let ci = app.project.sketches[si].constraints.iter().position(|c| matches!(c, Constraint::Diameter { .. })).expect("the radius of the fillet");
        let g = qymcad_ui_state::radial_dim_geom(&app.project, si, ci, &sheet(&app), &app.set).expect("the radius has a geometry");
        let sharp = sheet(&app).at(Point2::new(0.0, 30.0));
        assert!((g.edge - g.start).dot(sharp - g.start) > 0.0, "the radius is led from the centre away from the arc: {:?} -> {:?}", g.start, g.edge);
        let text = egui::Rect::from_center_size(g.text, g.size).expand(2.0);
        assert!(!text.contains(sharp), "the text of the radius covers the sharp of the corner: {text:?} over {sharp:?}");
    }

    #[test]
    fn the_chord_of_a_fillet_stands_outside() {
        let (mut app, si) = square();
        cut(&mut app, si, (0.0, 0.0), "tb-fillet-sketch-hint", Some("opt-fillet-chord"), 3.0);
        let chords = distances_of(&app, si, 3.0);
        assert_eq!(chords.len(), 1, "one chord of 3: {:?}", app.project.sketches[si].constraints);
        assert!(stands_away(&app, si, chords[0], INSIDE), "the chord {:?} of the fillet is drawn into the square", chords[0]);
    }
}
