//! TWO FILLETS MEET ON A SIDE: a fillet as big as the side between two corners allows is taken, and the side between
//! them goes. A rectangle 40 x 20 with every corner rounded to 10 is a slot - two sides of 20 and four arcs of 10.
//!
//! Reported behaviour: two fillets of 10 on a side of 20 were refused as too big, where the two arcs meet.
use crate::gui::hand::Hand;
use crate::gui::App;
use qymcad_core::feature::SketchPlane;

#[test]
fn every_corner_of_a_rectangle_rounded_to_half_its_short_side_is_a_slot() {
    let mut app = App::default();
    let si = app.create_sketch_on(SketchPlane::default());
    app.enter_sketch_edit(si);
    let mut hand = Hand::new(&mut app);
    hand.sk_tool(qymcad_ui_state::SketchShape::Rect).click2d(0.0, 0.0).click2d(40.0, 20.0);
    hand.key(egui::Key::Escape);
    hand.sk_select();
    hand.click2d(20.0, 0.0).shift_click2d(40.0, 10.0).shift_click2d(20.0, 20.0).shift_click2d(0.0, 10.0);
    assert!(hand.press_hint(&qymcad_i18n::tr("tb-fillet-sketch-hint")), "no sketch fillet button");
    hand.close_window();
    hand.frame(Vec::new());
    hand.type_text("10").key(egui::Key::Enter);
    let s = &hand.app.project.sketches[si];
    let at = |id: u64| s.points.iter().find(|q| q.id == id).map(|q| (q.x, q.y)).unwrap_or((f64::NAN, f64::NAN));
    let lines: Vec<f64> = s
        .entities
        .iter()
        .filter(|e| !e.construction)
        .filter_map(|e| match e.kind.as_line() {
            Some(qymcad_core::model::LineParts { a, b }) => Some((at(a).0 - at(b).0).hypot(at(a).1 - at(b).1)),
            _ => None,
        })
        .collect();
    let radii: Vec<f64> = s
        .entities
        .iter()
        .filter_map(|e| match e.kind.as_arc() {
            Some(qymcad_core::model::ArcParts { center, a, .. }) => Some((at(center).0 - at(a).0).hypot(at(center).1 - at(a).1)),
            _ => None,
        })
        .collect();
    let status = hand.app.status.clone();
    assert!(radii.len() == 4 && radii.iter().all(|r| (r - 10.0).abs() < 1e-6), "the arcs are {radii:?}, not four of 10; the status line says {status:?}");
    assert!(lines.len() == 2 && lines.iter().all(|l| (l - 20.0).abs() < 1e-6), "the sides left are {lines:?} long, not two of 20; the status line says {status:?}");
}
