//! A CONSTRUCTION LINE IS NO SIDE OF A CORNER, through the window: with the fillet or the chamfer in hand a click on a
//! diagonal of a rectangle drawn from its centre chooses nothing, and a side and the diagonal picked with Shift make no
//! corner - nothing is previewed, nothing is cut.
//!
//! Reported behaviour: a side and the construction diagonal picked with the chamfer in hand stood lit, the box said
//! "corners in the set: 1", the preview drew a cut between them and Enter made it.
#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use super::super::{App, Sel};
    use qymcad_core::feature::SketchPlane;

    /// A rectangle (0, 0) - (40, 30) drawn from its centre (20, 15), the size window closed with Enter.
    fn from_its_centre() -> (App, usize) {
        let mut app = App::default();
        let si = app.create_sketch_on(SketchPlane::default());
        app.chosen.sel = Sel::Sketch(si);
        let mut hand = Hand::new(&mut app);
        hand.sk_tool(qymcad_ui_state::SketchShape::Rect);
        assert!(hand.press_word(&qymcad_i18n::tr("opt-rect-centre"), egui::pos2(0.0, 0.0)), "no centre + corner on the bar");
        hand.click2d(20.0, 15.0).click2d(40.0, 30.0).key(egui::Key::Enter);
        Hand::new(&mut app).sk_select();
        assert!(app.project.sketches[si].rects().first().is_some_and(|r| r.diagonals.is_some()), "the rectangle was not drawn from its centre with its diagonals");
        (app, si)
    }

    #[test]
    fn a_side_and_a_diagonal_picked_with_the_corner_tools_make_no_corner() {
        let mut sins = Vec::new();
        for (key, tool) in [("tb-chamfer-sketch-hint", "the chamfer"), ("tb-fillet-sketch-hint", "the fillet")] {
            let (mut app, si) = from_its_centre();
            let r = app.project.sketches[si].rects()[0].clone();
            let diagonal = r.diagonals.expect("the diagonals")[0];
            let entities = app.project.sketches[si].entities.len();
            if !Hand::new(&mut app).sk_corner_set(key, &[qymcad_ui_state::SketchItem::curve(r.sides[0]), qymcad_ui_state::SketchItem::curve(diagonal)], 3.0) {
                sins.push(format!("{tool}: no place to click on the side and the diagonal"));
                continue;
            }
            let now = app.project.sketches[si].entities.len();
            if now != entities {
                sins.push(format!("{tool}: a side and the diagonal were cut as a corner - {entities} entities became {now}"));
            }
            if app.tools.sel_sk.items.iter().any(|&qymcad_ui_state::SketchItem { kind: k, id }| k == qymcad_ui_state::ItemKind::Curve && id == diagonal) {
                sins.push(format!("{tool}: the diagonal stands chosen"));
            }
        }
        assert!(sins.is_empty(), "{}", sins.join("\n"));
    }

    /// THE DIAGONAL IS NOT LIT UNDER THE CURSOR with the corner tools in hand, as it is with the select tool: a line
    /// lit says that a click takes it.
    #[test]
    fn a_diagonal_under_the_cursor_is_not_lit_by_the_corner_tools() {
        let mut sins = Vec::new();
        for (key, tool) in [("tb-chamfer-sketch-hint", "the chamfer"), ("tb-fillet-sketch-hint", "the fillet")] {
            let (mut app, si) = from_its_centre();
            let diagonal = app.project.sketches[si].rects()[0].diagonals.expect("the diagonals")[0];
            let Some(spot) = Hand::new(&mut app).spot2d(qymcad_ui_state::SketchItem::curve(diagonal)) else {
                sins.push(format!("{tool}: no place on the diagonal the select tool picks"));
                continue;
            };
            Hand::new(&mut app).hover2d(spot.0, spot.1);
            if app.chosen.hover.sketch != Some(qymcad_ui_state::SketchItem::curve(diagonal)) {
                sins.push(format!("{tool}: the select tool does not light the diagonal at {spot:?} - the place is wrong, not the tool"));
                continue;
            }
            let mut hand = Hand::new(&mut app);
            assert!(hand.press_hint(&qymcad_i18n::tr(key)), "no button of {tool}");
            hand.hover2d(spot.0, spot.1);
            if app.chosen.hover.sketch == Some(qymcad_ui_state::SketchItem::curve(diagonal)) {
                sins.push(format!("{tool}: the diagonal stands lit under the cursor"));
            }
        }
        assert!(sins.is_empty(), "{}", sins.join("\n"));
    }
}
