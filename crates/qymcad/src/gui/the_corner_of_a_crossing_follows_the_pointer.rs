//! THE CORNER OF A CROSSING FOLLOWS THE POINTER, and its preview with it: two crossing lines chosen with Shift in the
//! chamfer tool, a size typed - the pointer led into each quarter by the crossing in turn draws the cut in that
//! quarter.
//!
//! Reported behaviour: "the side of the fillet or the chamfer is chosen by where the cursor is, and the preview is drawn
//! there".
#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use super::super::{App, Sel};
    use qymcad_core::feature::SketchPlane;

    #[test]
    fn the_preview_of_the_corner_of_a_crossing_stands_in_the_quarter_of_the_pointer() {
        let mut app = App::default();
        let si = app.create_sketch_on(SketchPlane::default());
        app.chosen.sel = Sel::Sketch(si);
        let o = (5.0, 10.0);
        let mut hand = Hand::new(&mut app);
        hand.sk_tool(1).click2d(o.0 - 20.0, o.1).double_click2d(o.0 + 20.0, o.1);
        hand.click2d(o.0, o.1 - 20.0).double_click2d(o.0, o.1 + 20.0);
        hand.key(egui::Key::Escape).sk_tool(0);
        assert!(hand.press_hint(&crate::i18n::tr("tb-chamfer-sketch-hint")), "no sketch chamfer button");
        hand.shift_click2d(o.0 - 12.0, o.1).shift_click2d(o.0, o.1 - 12.0);
        hand.key(egui::Key::A).type_text("7.0710678");
        let colour = qymcad_sketch::corner_colour(&hand.app.scheme, true);
        let mut failures = Vec::new();
        for q in [(1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)] {
            hand.hover2d(o.0 + 2.0 * q.0, o.1 + 2.0 * q.1);
            let centre = hand.on_screen2d(o);
            let cut = hand.segments_in(colour);
            // the screen runs down where the sheet runs up
            let in_quarter = cut.iter().any(|[a, b]| {
                let m = egui::pos2((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
                (m.x - centre.x) * q.0 as f32 > 0.0 && (centre.y - m.y) * q.1 as f32 > 0.0
            });
            if !in_quarter {
                failures.push(format!("the pointer in the quarter {q:?}: the preview of the cut is drawn at {cut:?}, the crossing on screen at {centre:?}"));
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }
}
