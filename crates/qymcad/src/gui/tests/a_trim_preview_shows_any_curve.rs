//! THE PREVIEW OF TRIM AND EXTEND SHOWS ANY CURVE: with Trim in hand, the pointer over the piece of an ellipse a line cuts
//! off lights that piece in red; with Extend in hand, the pointer by the end of a spline draws in green the spline as it
//! will run to the line on its way. Both come from the functions of the core the tools cut and stretch by.
//!
//! Reported behaviour: the preview knew a line, a circle and an arc; over an ellipse or a spline it showed nothing, and
//! the click did what nothing had shown.
use crate::gui::hand::Hand;
use crate::gui::App;
use qymcad_core::feature::SketchPlane;

fn sketch() -> App {
    let mut app = App::default();
    let si = app.create_sketch_on(SketchPlane::default());
    app.enter_sketch_edit(si);
    app
}

#[test]
fn trim_lights_the_piece_of_an_ellipse_it_takes_away() {
    let mut app = sketch();
    let mut hand = Hand::new(&mut app);
    // an ellipse 8 by 4 about (40, 30), and a line across it at y = 29 cutting off its lower piece
    hand.sk_tool(qymcad_ui_state::SketchShape::Ellipse).click2d(40.0, 30.0).click2d(48.0, 30.0).click2d(40.0, 34.0);
    hand.key(egui::Key::Escape).key(egui::Key::Escape);
    hand.sk_tool(qymcad_ui_state::SketchShape::Line).click2d(28.0, 29.0).double_click2d(52.0, 29.0);
    hand.key(egui::Key::Escape).key(egui::Key::Escape);
    assert!(hand.press_hint(&qymcad_i18n::tr("tb-trim-hint")), "no trim button");
    hand.close_window();
    let red = hand.app.scheme.pal.error();
    let lit = hand.strokes_near2d((40.0, 26.0), red);
    assert!(lit > 0, "the pointer over the lower piece of the ellipse lit nothing in red");
}

#[test]
fn extend_draws_the_spline_as_it_will_run() {
    let mut app = sketch();
    let mut hand = Hand::new(&mut app);
    // a spline ending at (30, 30), and a line upright at x = 40 on its way
    hand.sk_tool(qymcad_ui_state::SketchShape::Spline).click2d(20.0, 30.0).click2d(25.0, 33.0).double_click2d(30.0, 30.0);
    hand.key(egui::Key::Escape).key(egui::Key::Escape);
    hand.sk_tool(qymcad_ui_state::SketchShape::Line).click2d(40.0, 15.0).double_click2d(40.0, 45.0);
    hand.key(egui::Key::Escape).key(egui::Key::Escape);
    assert!(hand.press_hint(&qymcad_i18n::tr("tb-extend-hint")), "no extend button");
    hand.close_window();
    let green = hand.app.scheme.pal.add();
    let drawn = hand.strokes_near2d((29.0, 30.5), green);
    assert!(drawn > 0, "the pointer by the end of the spline drew nothing in green");
}
