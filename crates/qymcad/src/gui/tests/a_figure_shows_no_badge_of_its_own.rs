//! A FIGURE SHOWS NO BADGE OF ITS OWN: a rectangle, a hexagon and a slot drawn by hand, and an angle dimension placed by
//! hand between a side of the rectangle and a slanted line, which takes the rectangle's turn - not one badge on the
//! canvas, as none of them was laid by the person.
//!
//! Reported behaviour: once an angle dimension took the turn of a rectangle, its parallel and square sides came out as
//! four badges, where a polygon and a slot showed none of theirs.
use crate::gui::hand::Hand;
use crate::gui::App;
use qymcad_core::feature::SketchPlane;
use qymcad_core::model::Constraint;

#[test]
fn a_rectangle_turned_by_an_angle_a_hexagon_and_a_slot_show_no_badge() {
    let mut app = App::default();
    let si = app.create_sketch_on(SketchPlane::default());
    app.enter_sketch_edit(si);
    let mut hand = Hand::new(&mut app);
    hand.sk_tool(qymcad_ui_state::SketchShape::Rect).click2d(5.0, 5.0).click2d(25.0, 15.0).key(egui::Key::Escape);
    hand.sk_tool(qymcad_ui_state::SketchShape::Polygon).click2d(-30.0, 20.0).click2d(-22.0, 20.0).key(egui::Key::Escape);
    hand.sk_tool(qymcad_ui_state::SketchShape::Slot).click2d(-30.0, -20.0).click2d(-10.0, -20.0).click2d(-10.0, -17.0).key(egui::Key::Escape);
    hand.sk_tool(qymcad_ui_state::SketchShape::Line).click2d(10.0, -30.0).click2d(30.0, -24.0).key(egui::Key::Escape);
    hand.sk_select();
    assert!(hand.press_hint(&crate::i18n::tr("tb-dim-angle-hint")), "the angle dimension tool");
    hand.click2d(16.0, -28.2).click2d(11.0, 5.0).click2d(40.0, -10.0).key(egui::Key::Escape);
    hand.sk_select();
    let s = &hand.app.project.sketches[si];
    let status = hand.app.status.clone();
    assert!(
        s.rects().len() == 1 && s.shapes_not_rects().count() == 2 && s.constraints.iter().any(|c| matches!(c, Constraint::AngleLines { .. })),
        "setup: {} rectangles, {} figures, no angle dimension in {:?}; the status line says {status:?}",
        s.rects().len(),
        s.shapes_not_rects().count(),
        s.constraints
    );
    assert!(!s.constraints.iter().any(|c| matches!(c, Constraint::Orientation { .. })), "setup: the angle did not take the rectangle's turn");
    let badges: Vec<String> = qymcad_pick::constraint_glyphs(&hand.app.pick_ctx(), hand.app.viewing.view_rect, si).into_iter().map(|(ci, _, g)| format!("{g:?} of {:?}", s.constraints[ci])).collect();
    assert!(badges.is_empty(), "badges of constraints no person laid:\n{}", badges.join("\n"));
}
