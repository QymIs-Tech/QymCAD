//! A CUT SKETCHED ON A DATUM PLANE FOLLOWS THE PARAMETER THAT PLACES THE PLANE, edited in the window of parameters.
//! Reported behaviour: a 40 x 40 plate of thickness `t`, a plane from XY at the distance `t` (its top), a 10 x 10
//! rectangle on it cut 3 down; `t` from 6 to 10 - the plate and the plane followed, the floor of the pocket stayed at
//! z = 3, deeper inside the thicker plate, with no word. The core's check of the same is
//! `crates/qymcad-testkit/tests/a_cut_on_a_datum_follows_its_parameter.rs`; this one goes the person's way.
use crate::gui::hand::{Hand, Typed};
use crate::gui::App;
use qymcad_core::feature::{BasePlane, FeatureKind, SketchPlane};
use qymcad_core::model::{Id, Param, PlaneDef, WorkPlane};
use qymcad_ui_state::Sel;

/// Where the floor of the pocket of `cut` stands: the faces 10 x 10 facing up.
fn floor(app: &App, cut: Id) -> Vec<f64> {
    let body = app.project.timeline.iter().find(|n| n.id == cut).and_then(|n| n.kind.body()).unwrap_or(0);
    app.project.regen_faces.get(&body).map(|faces| faces.iter().filter(|f| f.normal[2] > 0.99 && (f.area - 100.0).abs() < 1e-6).map(|f| f.centroid.z).collect()).unwrap_or_default()
}

#[test]
fn a_cut_on_a_datum_follows_its_parameter_edited_in_the_window() {
    let mut app = App::default();
    app.project.parameters.push(Param { name: "t".into(), expr: "6".into(), value: 6.0 });
    app.project.eval_parameters();
    let part = app.project.add_part("Plate");
    app.enter_component(part);
    // the plate, 6 thick by the formula t, drawn and extruded by hand
    let si = app.create_sketch_on(SketchPlane::default());
    Hand::new(&mut app).sk_tool(qymcad_ui_state::SketchShape::Rect).click2d(-20.0, -20.0).click2d(20.0, 20.0);
    app.finish_sketch_edit();
    app.chosen.sel = Sel::Sketch(si);
    Hand::new(&mut app).look_at([0.0, 0.0, 3.0], 8.0).tool(qymcad_ui_state::PartTool::Extrude).set("height", 6.0).formula(Typed { field: "height", text: "t" }).enter();
    // the plane from XY at t, the pocket sketched on it and cut 3 into the plate by hand
    let plane = app.project.add_plane(WorkPlane { origin: [0.0, 0.0, 6.0], def: PlaneDef::OffsetBase { base: BasePlane::XY, dist: 6.0 }, ..Default::default() });
    app.project.set_feat_dim(plane, "dist", "t".into());
    let sk = app.create_sketch_on(SketchPlane::Datum(plane));
    Hand::new(&mut app).sk_tool(qymcad_ui_state::SketchShape::Rect).click2d(-5.0, -5.0).click2d(5.0, 5.0);
    app.finish_sketch_edit();
    app.chosen.sel = Sel::Sketch(sk);
    let mut hand = Hand::new(&mut app);
    hand.look_at([0.0, 0.0, 6.0], 8.0).tool(qymcad_ui_state::PartTool::Extrude).op(2).set("height", 3.0);
    // the plane faces up, out of the plate: the cut is turned into it with the button of the direction
    assert!(hand.press_word(&crate::i18n::tr("cmd-flip-btn"), egui::pos2(0.0, 0.0)), "setup: the bar offers the direction");
    hand.enter();
    let cut = app.project.timeline.iter().rev().find(|n| matches!(n.kind, FeatureKind::Combine { .. })).map(|n| n.id).expect("setup: the cut is made");
    let mut hand = Hand::new(&mut app);
    hand.settle();
    assert!(hand.app.project.regen_errors.is_empty(), "setup: the plate and its pocket build: {:?}", hand.app.project.regen_errors);
    assert_eq!(floor(hand.app, cut), vec![3.0], "setup: the floor of the pocket stands at z = 6 - 3");

    // t typed 10 in the window of parameters, as a person types it
    assert!(hand.press_word(&crate::i18n::tr("wb-params"), egui::pos2(0.0, 0.0)), "the button of the parameters is not on screen");
    assert!(hand.type_after_word("t", "10"), "the formula of t is not on screen");
    hand.key(egui::Key::Enter).settle();
    assert!(hand.app.project.regen_errors.is_empty(), "the plate 10 thick does not build: {:?}", hand.app.project.regen_errors);
    assert_eq!(floor(hand.app, cut), vec![7.0], "t became 10 and the floor of the pocket stands where it was, not at z = 10 - 3");
}
