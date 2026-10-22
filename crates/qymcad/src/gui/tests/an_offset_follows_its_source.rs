//! AN OFFSET FOLLOWS ITS SOURCE, on the path of a person: an ellipse 12 by 6 drawn, clicked, Offset taken, a click
//! outside it and 3 typed; then the half axis of the ellipse given 15 by the Dimension tool - the offset stands 3 from
//! the ellipse as it is now; then Ctrl+Z - the ellipse is 12 again and the offset 3 from it.
//!
//! Reported behaviour: the offset of an ellipse or a spline was a free spline - the source edited, the offset stayed
//! where it was.
use crate::gui::hand::Hand;
use crate::gui::{App, Sel};
use qymcad_core::geom::Point2;
use qymcad_core::model::curve::{Curve, OFFSET_TOLERANCE};

/// How far from `d` the splines of sketch `si` stand off its ellipse, the worst of 400 places of each; with the half axis
/// of the ellipse now. `None` where there is no ellipse or no spline.
fn offset_off(app: &App, si: usize, d: f64) -> Option<Standing> {
    let s = &app.project.sketches[si];
    let e = s.entities.iter().find(|e| e.kind.as_ellipse().is_some())?;
    let ellipse = Curve::of(s, e)?;
    let parts = e.kind.as_ellipse()?;
    let (c, ma) = (s.point(parts.c)?, s.point(parts.ma)?);
    let places: Vec<Point2> = s.entities.iter().filter(|e| e.kind.as_spline().is_some()).filter_map(|e| Curve::of(s, e)).flat_map(|c| (0..=400).map(move |k| c.at(k as f64 / 400.0))).collect();
    let off = places.iter().map(|q| (ellipse.nearest(*q).off - d).abs()).reduce(f64::max)?;
    Some(Standing { off, half_axis: (ma.x - c.x).hypot(ma.y - c.y) })
}

/// WHERE THE OFFSET STANDS: how far it is off the distance, and the half axis of the ellipse.
#[derive(Debug)]
struct Standing {
    off: f64,
    half_axis: f64,
}

#[test]
fn an_offset_of_an_ellipse_follows_it_and_comes_back_with_undo() {
    let mut app = App::default();
    let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
    app.chosen.sel = Sel::Sketch(si);
    let mut hand = Hand::new(&mut app);
    hand.sk_tool(qymcad_ui_state::SketchShape::Ellipse);
    for (x, y) in [(0.0, 0.0), (12.0, 0.0), (0.0, 6.0)] {
        hand.click2d(x, y);
    }
    hand.key(egui::Key::Escape).key(egui::Key::Escape);
    // the ellipse clicked on its way, offset 3 outwards
    let on_it = (12.0 * 0.6, 6.0 * 0.8);
    hand.sk_select().click2d(on_it.0, on_it.1);
    assert!(hand.press_hint(&crate::i18n::tr("tb-offset-hint")), "no sketch offset button");
    hand.click2d(20.0, 0.0);
    hand.type_text("3").key(egui::Key::Enter);
    hand.key(egui::Key::Escape).key(egui::Key::Escape);
    let laid = offset_off(hand.app, si, 3.0);
    assert!(laid.as_ref().is_some_and(|s| s.off <= OFFSET_TOLERANCE), "setup: the offset stands {laid:?}; the status line says {:?}", hand.app.status);
    // the half axis given 15 by the Dimension tool: the ellipse clicked, the dimension placed inside it - away from the
    // offset, which a click beside the ellipse takes as a second curve - and 15 typed
    assert!(hand.press_hint(&crate::i18n::tr("tb-dim-hint")), "no dimension button");
    hand.close_window();
    hand.click2d(on_it.0, on_it.1);
    hand.click2d(-4.0, 2.0);
    hand.frame(Vec::new());
    if hand.typing() {
        hand.chord(egui::Modifiers::COMMAND, egui::Key::A).type_text("15");
    } else {
        assert!(hand.type_into_hinted(&crate::i18n::tr("sk-expr-example"), 15.0), "no field of a dimension came up; the status line says {:?}", hand.app.status);
    }
    hand.key(egui::Key::Enter);
    hand.key(egui::Key::Escape).key(egui::Key::Escape);
    let edited = offset_off(hand.app, si, 3.0);
    assert!(
        edited.as_ref().is_some_and(|s| (s.half_axis - 15.0).abs() < 1e-6 && s.off <= OFFSET_TOLERANCE),
        "the half axis set to 15: the ellipse and its offset stand {edited:?}; the status line says {:?}",
        hand.app.status
    );
    // Ctrl+Z: the ellipse as it was, its offset with it
    hand.undo();
    let undone = offset_off(hand.app, si, 3.0);
    assert!(undone.as_ref().is_some_and(|s| (s.half_axis - 12.0).abs() < 1e-6 && s.off <= OFFSET_TOLERANCE), "Ctrl+Z: the ellipse and its offset stand {undone:?}");
}
