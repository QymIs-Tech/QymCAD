//! A DIMENSION BEING PLACED FOLLOWS THE POINTER AS IT IS, through the window: led off the line it measures in steps of
//! 3 px, its line stands under the pointer at every step, none of them caught on the line; placed 10 mm below the line,
//! it stands 10 mm below it. Led off to the side of a horizontal line, it does not turn into a vertical dimension of
//! the line's height, which is nothing: it keeps measuring the length.
//!
//! Reported behaviour (#73): the dimension followed the snapped cursor, the snaps of the very line being dimensioned -
//! its middle, a point on it - caught it, and it stuck on the line, drawn red, until a firm pull tore it off. The side
//! of a rectangle's bottom, the pointer led up and to the left, showed a red vertical dimension of 0.0.
#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use super::super::{App, Sel};
    use qymcad_core::feature::SketchPlane;

    #[test]
    fn a_dimension_led_to_the_side_of_a_horizontal_line_keeps_its_length() {
        let mut app = App::default();
        let si = app.create_sketch_on(SketchPlane::default());
        app.chosen.sel = Sel::Sketch(si);
        Hand::new(&mut app).sk_tool(qymcad_ui_state::SketchShape::Line).click2d(0.0, 0.0).click2d(40.0, 0.0).key(egui::Key::Escape);
        let mut hand = Hand::new(&mut app);
        hand.sk_select();
        assert!(hand.press_hint(&crate::i18n::tr("tb-dim-hint")), "the dimension tool");
        hand.click2d(20.0, 0.0);
        let ci = hand.app.tools.place.dim.expect("the dimension of the line is being placed");
        // up and to the left of the line's left end, where the pointer stood when it read 0.0
        hand.hover2d(-5.0, 8.0);
        let d = match hand.app.project.sketches[si].constraints[ci] {
            qymcad_core::model::Constraint::Distance { d, .. } => d,
            ref c => panic!("not a linear dimension: {c:?}"),
        };
        assert!((d - 40.0).abs() < 1e-6, "led to the side of a horizontal line of 40, the dimension measures {d}");
    }

    #[test]
    fn a_dimension_being_placed_follows_the_pointer_off_its_line() {
        let mut app = App::default();
        let si = app.create_sketch_on(SketchPlane::default());
        app.chosen.sel = Sel::Sketch(si);
        Hand::new(&mut app).sk_tool(qymcad_ui_state::SketchShape::Line).click2d(0.0, 0.0).click2d(40.0, 0.0).key(egui::Key::Escape);
        let mut hand = Hand::new(&mut app);
        hand.sk_select();
        assert!(hand.press_hint(&crate::i18n::tr("tb-dim-hint")), "the dimension tool");
        hand.click2d(20.0, 0.0);
        let ci = hand.app.tools.place.dim.expect("the dimension of the line is being placed");
        let sheet = |hand: &Hand| qymcad_ui_state::Sheet { view: hand.app.viewing.view, rect: hand.app.viewing.view_rect };
        let line_y = |hand: &Hand| {
            let c = &hand.app.project.sketches[si].constraints[ci];
            qymcad_ui_state::linear_dim_line(&hand.app.project, si, c, &sheet(hand)).map(|(a, _, _)| a.y).expect("the line of the dimension")
        };
        // 3 px a step at the 6 px of a millimetre the hand looks at the sheet with
        let mut stuck = Vec::new();
        for k in 1..=24 {
            let y = -(k as f64) * 0.5;
            hand.hover2d(20.0, y);
            let (want, got) = (hand.on_screen2d((20.0, y)).y, line_y(&hand));
            if (want - got).abs() > 1.0 {
                stuck.push(format!("pointer at {y} mm: the line at {got} px, the pointer at {want} px"));
            }
        }
        assert!(stuck.is_empty(), "the dimension did not follow the pointer off its line:\n{}", stuck.join("\n"));
        hand.click2d(20.0, -10.0);
        let (want, got) = (hand.on_screen2d((20.0, -10.0)).y, line_y(&hand));
        assert!((want - got).abs() < 1.0, "placed 10 mm below the line, the dimension stands at {got} px, not at {want} px");
    }
}
