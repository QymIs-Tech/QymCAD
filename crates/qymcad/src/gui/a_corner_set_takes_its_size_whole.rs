//! A SET OF CORNERS TAKES ITS SIZE WHOLE, through the window: a fillet given by its chord or by its arc length is
//! previewed as the fillet Enter makes, a set of several corners is rounded by that chord or arc length on every corner,
//! and a set one corner of which cannot take the size is not cut at all.
//!
//! Reported behaviour: the preview of a fillet by its chord or its arc length drew the value as a radius, a set of
//! several corners was rounded by the value taken as a radius whatever the bar said, and a set was cut on the corners
//! that took the value and left whole on the one that did not.
#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use super::super::{App, Sel};
    use qymcad_core::feature::SketchPlane;
    use qymcad_core::model::Constraint;

    /// A chain of lines drawn by the line tool through `clicks`, and the select tool taken.
    fn drawn(clicks: &[(f64, f64)]) -> (App, usize) {
        let mut app = App::default();
        let si = app.create_sketch_on(SketchPlane::default());
        app.chosen.sel = Sel::Sketch(si);
        let mut hand = Hand::new(&mut app);
        hand.sk_tool(qymcad_ui_state::SketchShape::Line);
        for &(x, y) in clicks {
            hand.click2d(x, y);
        }
        hand.key(egui::Key::Escape);
        Hand::new(&mut app).sk_select();
        (app, si)
    }

    const SQUARE: [(f64, f64); 5] = [(0.0, 0.0), (40.0, 0.0), (40.0, 30.0), (0.0, 30.0), (0.0, 0.0)];

    /// The lines of the sketch with both ends at the places `ends`, as items to pick, in the order of `ends`.
    fn lines(app: &App, si: usize, ends: &[[(f64, f64); 2]]) -> Vec<qymcad_ui_state::SketchItem> {
        let s = &app.project.sketches[si];
        let at = |id: u64, p: (f64, f64)| s.points.iter().any(|q| q.id == id && (q.x - p.0).hypot(q.y - p.1) < 1e-6);
        ends.iter()
            .map(|[p, q]| {
                let e = s
                    .entities
                    .iter()
                    .find(|e| e.kind.as_line().is_some_and(|qymcad_core::model::LineParts { a, b }| (at(a, *p) && at(b, *q)) || (at(a, *q) && at(b, *p))))
                    .expect("a line between the two places");
                qymcad_ui_state::SketchItem::curve(e.id)
            })
            .collect()
    }

    /// The radii of the arcs of the sketch.
    fn arc_radii(app: &App, si: usize) -> Vec<f64> {
        let s = &app.project.sketches[si];
        let at = |id: u64| s.points.iter().find(|q| q.id == id).map(|q| (q.x, q.y)).expect("a point");
        s.entities
            .iter()
            .filter_map(|e| match e.kind.as_arc() {
                Some(qymcad_core::model::ArcParts { center, a, .. }) => {
                    let (c, p) = (at(center), at(a));
                    Some((p.0 - c.0).hypot(p.1 - c.1))
                }
                _ => None,
            })
            .collect()
    }

    /// The preview of the corner at the origin of the square in the mode `mode` with the value 3: the rings it draws
    /// where the lines will be cut stand at `along` from the corner on both lines.
    fn previewed_at(mode: &str, along: f64) {
        let (mut app, si) = drawn(&SQUARE);
        let set = lines(&app, si, &[[(0.0, 30.0), (0.0, 0.0)], [(0.0, 0.0), (40.0, 0.0)]]);
        let mut hand = Hand::new(&mut app);
        assert!(hand.sk_corner_typed("tb-fillet-sketch-hint", Some(mode), &set, 3.0), "no place to click on the two lines");
        let rings = hand.rings_drawn(3.0);
        for end in [(along, 0.0), (0.0, along)] {
            let want = hand.on_screen2d(end);
            assert!(rings.iter().any(|r| r.distance(want) < 1.5), "{mode}: no mark of the cut at {end:?} ({want:?}), the marks stand at {rings:?}");
        }
    }

    #[test]
    fn the_preview_of_a_fillet_by_its_chord_is_the_fillet_enter_makes() {
        // a chord of 3 on a square corner is a radius of 3 / (2 sin 45 deg), the arc touching each line that far out
        previewed_at("opt-fillet-chord", 3.0 / (2.0 * std::f64::consts::FRAC_PI_4.sin()));
    }

    #[test]
    fn the_preview_of_a_fillet_by_its_arc_length_is_the_fillet_enter_makes() {
        // an arc of 3 on a square corner is a radius of 3 / (pi / 2)
        previewed_at("opt-fillet-arc-length", 3.0 / std::f64::consts::FRAC_PI_2);
    }

    #[test]
    fn a_set_of_corners_is_rounded_by_its_chord() {
        let (mut app, si) = drawn(&SQUARE);
        let set = lines(&app, si, &[[(0.0, 30.0), (0.0, 0.0)], [(0.0, 0.0), (40.0, 0.0)], [(40.0, 0.0), (40.0, 30.0)]]);
        assert!(Hand::new(&mut app).sk_corner_set_in("tb-fillet-sketch-hint", Some("opt-fillet-chord"), &set, 3.0), "no place to click on the lines");
        let radii = arc_radii(&app, si);
        let r = 3.0 / (2.0 * std::f64::consts::FRAC_PI_4.sin());
        assert!(radii.len() == 2 && radii.iter().all(|x| (x - r).abs() < 1e-6), "two corners of a chord of 3 have arcs of radius {r}: {radii:?}");
        let s = &app.project.sketches[si];
        let chords = s.constraints.iter().filter(|c| matches!(c, Constraint::Distance { d, .. } if (d - 3.0).abs() < 1e-9)).count();
        let held_by_radius = s.constraints.iter().filter(|c| matches!(c, Constraint::Diameter { .. })).count();
        assert_eq!((chords, held_by_radius), (2, 0), "each corner keeps its chord of 3 and no radius: {:?}", s.constraints);
    }

    #[test]
    fn a_set_one_corner_of_which_cannot_take_the_size_is_not_cut() {
        // a square corner and a corner of 135 deg on a short line: a chord of 18 is a radius of 12.7 on the first and
        // of 23.5 on the second, which takes no more than 8.49 tan(67.5 deg) = 20.5
        let (mut app, si) = drawn(&[(0.0, 30.0), (0.0, 0.0), (40.0, 0.0), (46.0, 6.0)]);
        let set = lines(&app, si, &[[(0.0, 30.0), (0.0, 0.0)], [(0.0, 0.0), (40.0, 0.0)], [(40.0, 0.0), (46.0, 6.0)]]);
        let before = app.project.sketches[si].entities.len();
        assert!(Hand::new(&mut app).sk_corner_set_in("tb-fillet-sketch-hint", Some("opt-fillet-chord"), &set, 18.0), "no place to click on the lines");
        let s = &app.project.sketches[si];
        assert!(arc_radii(&app, si).is_empty() && s.entities.len() == before, "a set one corner of which cannot take the chord was cut: {:?}", s.entities);
    }
}
