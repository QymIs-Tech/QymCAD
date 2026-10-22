//! A FILLET OF A SPLINE HOLDS WHEN DRAGGED, on the path of the report: a spline ending at (20, 20) and a line from there
//! drawn by hand, the corner rounded R3 by a click on its point, the far end of the line dragged - the arc still touches
//! the spline and the line where it ends on them, at its radius of 3.
//!
//! Reported behaviour: the end of the line dragged, a circle came up in the corner, points ran together, and constraints
//! nobody laid held them - the arc had shrunk to nothing, its sharp drawn to the end of the spline.
use crate::gui::hand::Hand;
use crate::gui::{App, Sel};
use qymcad_core::model::curve::Curve;

#[test]
fn a_fillet_of_a_spline_and_a_line_holds_when_the_line_is_dragged() {
    let mut app = App::default();
    let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
    app.chosen.sel = Sel::Sketch(si);
    let mut hand = Hand::new(&mut app);
    hand.sk_tool(qymcad_ui_state::SketchShape::Spline).click2d(-20.0, -10.0).click2d(5.0, 10.0).double_click2d(20.0, 20.0);
    hand.key(egui::Key::Escape).key(egui::Key::Escape);
    hand.sk_tool(qymcad_ui_state::SketchShape::Line).click2d(20.0, 20.0).click2d(30.0, 40.0).key(egui::Key::Escape).key(egui::Key::Escape);
    hand.sk_select();
    assert!(hand.press_hint(&crate::i18n::tr("tb-fillet-sketch-hint")), "no fillet button");
    hand.close_window();
    hand.click2d(20.0, 20.0);
    hand.frame(Vec::new());
    hand.chord(egui::Modifiers::COMMAND, egui::Key::A).type_text("3").key(egui::Key::Enter);
    hand.key(egui::Key::Escape).key(egui::Key::Escape);
    assert!(hand.app.project.sketches[si].entities.iter().any(|e| e.kind.as_arc().is_some()), "setup: the corner was not rounded; the status line says {:?}", hand.app.status);
    hand.sk_select().click2d(60.0, -30.0);
    hand.drag2d((30.0, 40.0), (38.0, 36.0));
    let s = &hand.app.project.sketches[si];
    let arc = s.entities.iter().find(|e| e.kind.as_arc().is_some()).and_then(|e| Curve::of(s, e)).expect("the arc");
    let Curve::Round { centre, r, .. } = arc else { panic!("the fillet is no arc") };
    let sides: Vec<Curve> = s.entities.iter().filter(|e| e.kind.as_arc().is_none()).filter_map(|e| Curve::of(s, e)).collect();
    // at each end of the arc a side ends there, running square to the radius
    let lean = arc
        .ends()
        .iter()
        .map(|q| {
            sides
                .iter()
                .filter_map(|c| [0.0, 1.0].into_iter().find(|t| c.at(*t).dist(*q) < 1e-4).map(|t| (c.tangent(t), *q - centre)))
                .map(|(u, radial)| (u.x * radial.x + u.y * radial.y).abs() / radial.x.hypot(radial.y))
                .fold(f64::MAX, f64::min)
        })
        .fold(0.0, f64::max);
    assert!(lean < 1e-3 && (r - 3.0).abs() < 1e-6, "the line dragged: the arc of radius {r:.4} leans {lean:.2e} off touching its sides");
}
