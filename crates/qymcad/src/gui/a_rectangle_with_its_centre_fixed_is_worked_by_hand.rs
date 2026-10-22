//! A RECTANGLE WITH ITS CENTRE FIXED, WORKED BY HAND: drawn from a corner, its centre picked and fixed with the button
//! of the panel. The sheet draws all four corners free - the width and the height move each of them - and the centre
//! defined; a corner dragged by the mouse resizes the rectangle about the centre, which stays.
//!
//! Reported behaviour: "only one corner is yellow, the rest green", and "a rectangle drawn from a corner, its centre
//! fixed - none of its corners can be dragged to change its size".
#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use crate::gui::import_door::tests::running;
    use qymcad_core::feature::SketchPlane;

    /// THE SIZE OF A PLAIN POINT on the sheet, as `draw_sketch_points` draws one neither picked nor under the pointer.
    const DOT: f32 = 3.5;

    #[test]
    fn a_rectangle_with_its_centre_fixed_shows_free_corners_and_resizes_about_it() {
        let (mut app, _ctx) = running();
        let si = app.create_sketch_on(SketchPlane::default());
        app.enter_sketch_edit(si);
        let mut hand = Hand::new(&mut app);
        hand.sk_tool(qymcad_ui_state::SketchShape::Rect).click2d(10.0, 10.0).click2d(50.0, 40.0).key(egui::Key::Escape).key(egui::Key::Escape);
        hand.sk_select();
        let centre = hand.app.project.sketches[0].rects().first().map(|r| r.centre).expect("the rectangle drawn");
        assert!(hand.select2d(&[qymcad_ui_state::SketchItem::point(centre)]), "the centre of the rectangle could not be picked");
        hand.constraint(qymcad_ui_state::ConstraintKind::Fix);
        hand.key(egui::Key::Escape);
        assert!(hand.app.project.sketches[0].constraints.iter().any(|c| matches!(c, qymcad_core::model::Constraint::Fixed { p } if *p == centre)), "GUARD: the centre is fixed");

        // THE COLOURS: the corners drawn free, the centre drawn defined
        hand.look2d((30.0, 25.0)).frame(Vec::new());
        let (free, ok) = (hand.app.scheme.pal.underdefined(), hand.app.scheme.pal.ok());
        let (yellow, green) = (hand.dots_in(DOT, free), hand.dots_in(DOT, ok));
        let drawn = |dots: &[egui::Pos2], at: (f64, f64)| dots.iter().any(|d| d.distance(hand.on_screen2d(at)) < 1.5);
        let corners = [(10.0, 10.0), (50.0, 10.0), (50.0, 40.0), (10.0, 40.0)];
        let held: Vec<(f64, f64)> = corners.iter().copied().filter(|&c| !drawn(&yellow, c)).collect();
        assert!(held.is_empty(), "with the centre fixed the corners {held:?} are not drawn free, though the width and the height move them");
        assert!(drawn(&green, (30.0, 25.0)), "the fixed centre is not drawn defined");

        // THE DRAG: the corner (50, 40) led to (56, 44) - the corner across goes to (4, 6), the centre stays
        hand.drag2d((50.0, 40.0), (56.0, 44.0));
        let sk = &hand.app.project.sketches[0];
        let at = |id: u64| sk.points.iter().find(|q| q.id == id).map(|q| (q.x, q.y)).expect("the point");
        let r = sk.rects()[0].clone();
        let now: Vec<(f64, f64)> = r.corners.iter().map(|&k| at(k)).collect();
        let near = |a: (f64, f64), b: (f64, f64)| (a.0 - b.0).abs() < 0.6 && (a.1 - b.1).abs() < 0.6;
        assert!(near(at(r.centre), (30.0, 25.0)), "the fixed centre moved to {:?}", at(r.centre));
        assert!(
            now.iter().any(|&c| near(c, (56.0, 44.0))) && now.iter().any(|&c| near(c, (4.0, 6.0))),
            "a corner dragged from (50, 40) to (56, 44) about the fixed centre left the corners at {now:?}"
        );
    }
}
