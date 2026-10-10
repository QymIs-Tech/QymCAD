//! THE CORNER OF A CROSSING FOLLOWS THE POINTER UNTIL A CLICK FIXES IT: two crossing lines chosen with Shift in the
//! chamfer tool - the pointer led into each quarter by the crossing in turn draws the cut there, as a corner not named
//! yet, and no box stands; a click in a quarter fixes the corner there, its preview turns the colour of the set, the box
//! opens, and the pointer led away leaves the corner where it was clicked.
//!
//! Reported behaviour: "the side is chosen by where the cursor is, with the preview drawn there; the box runs away from
//! the cursor - the corner must be fixed by a click, the preview changing colour, so the person can reach the box".
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
        let (named, fixed) = (qymcad_sketch::corner_colour(&hand.app.scheme, false), qymcad_sketch::corner_colour(&hand.app.scheme, true));
        let centre = hand.on_screen2d(o);
        // the cut drawn in `colour` stands in the quarter `q`: the screen runs down where the sheet runs up
        let cut_in = |hand: &Hand, colour: egui::Color32, q: (f64, f64)| {
            hand.segments_in(colour).iter().any(|[a, b]| {
                let m = egui::pos2((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
                (m.x - centre.x) * q.0 as f32 > 0.0 && (centre.y - m.y) * q.1 as f32 > 0.0
            })
        };
        let fields = |hand: &Hand| hand.app.tools.corner.at.is_some() && hand.app.status != crate::i18n::tr("sk-corner-click-to-fix");
        let mut failures = Vec::new();
        for q in [(1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)] {
            hand.hover2d(o.0 + 2.0 * q.0, o.1 + 2.0 * q.1);
            if !cut_in(&hand, named, q) || fields(&hand) {
                failures.push(format!("the pointer in the quarter {q:?} before a click: the preview of a corner not named yet is not there, or a box stands already"));
            }
        }
        // a click in the lower left quarter fixes it there: the preview turns the colour of the set, the box opens, and
        // the pointer led across leaves the corner where it was clicked
        hand.click2d(o.0 - 2.0, o.1 - 2.0);
        hand.hover2d(o.0 + 2.0, o.1 + 2.0);
        if !cut_in(&hand, fixed, (-1.0, -1.0)) || cut_in(&hand, fixed, (1.0, 1.0)) || !fields(&hand) {
            failures.push("clicked in the lower left quarter and the pointer led to the upper right: the corner did not stay fixed where clicked, or no box opened".into());
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }
}
