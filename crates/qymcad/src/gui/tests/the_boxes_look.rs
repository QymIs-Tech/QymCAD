//! THE BOXES AT THE GEOMETRY, LOOKED AT: each box of a tool opened by a person's path and saved as a picture, so the
//! layout of every box of every workbench can be looked at side by side - the sizes of a shape drawn and opened again,
//! the edit of a dimension, the corner, the offset, the turn, the operations of a part, the mate of an assembly.
//!
//! The run: `QYMCAD_BOXES_LOOK=before cargo test -p qymcad --lib -- --ignored --nocapture the_boxes_look`. The pictures go
//! into `target/boxes-look/` as `<box>-<tag>.png`: not into the repository - this is a way to look, not a product.
use super::*;
use crate::gui::hand::Hand;
use qymcad_core::feature::SketchPlane;
use qymcad_ui_state::SketchShape;

/// Where the pictures go, named by the tag of the run.
fn save(hand: &mut Hand, name: &str, missing: &mut Vec<String>) {
    // a box just opened fades in over its first frames: it is looked at once it stands
    for _ in 0..12 {
        hand.frame(Vec::new());
    }
    let Some(rect) = hand.tool_box_id().and_then(|id| hand.area_rect(id)) else {
        missing.push(name.to_string());
        return;
    };
    let tag = std::env::var("QYMCAD_BOXES_LOOK").unwrap_or_else(|_| "now".to_string());
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/boxes-look");
    std::fs::create_dir_all(&dir).expect("the directory of the pictures");
    let png = crate::gui::color_image_to_png(&hand.picture(rect.expand(6.0))).expect("PNG");
    std::fs::write(dir.join(format!("{name}-{tag}.png")), png).expect("the write");
}

/// A sketch being edited, a hand on it.
fn sketching(app: &mut App) {
    let si = app.create_sketch_on(SketchPlane::default());
    app.chosen.sel = Sel::Sketch(si);
}

/// A shape drawn by its clicks, its box of sizes open.
fn drawn(name: &str, shape: SketchShape, clicks: &[(f64, f64)], missing: &mut Vec<String>) {
    let mut app = App::default();
    sketching(&mut app);
    let mut hand = Hand::new(&mut app);
    hand.sk_tool(shape);
    for c in clicks {
        hand.click2d(c.0, c.1);
    }
    save(&mut hand, name, missing);
}

/// A shape drawn, put down and opened again by a double click on it.
fn reopened(name: &str, shape: SketchShape, clicks: &[(f64, f64)], at: (f64, f64), missing: &mut Vec<String>) {
    let mut app = App::default();
    sketching(&mut app);
    let mut hand = Hand::new(&mut app);
    hand.sk_tool(shape);
    for c in clicks {
        hand.click2d(c.0, c.1);
    }
    hand.key(egui::Key::Escape).key(egui::Key::Escape).sk_select();
    hand.double_click2d(at.0, at.1);
    save(&mut hand, name, missing);
}

/// A TOOL TAKEN ON A LINE CHOSEN: the hint of its button, and the places clicked after it.
struct OnALine {
    hint: &'static str,
    then: &'static [(f64, f64)],
}

/// A line drawn and chosen, then the tool pressed by its button and its places clicked.
fn on_a_chosen_line(name: &str, tool: OnALine, missing: &mut Vec<String>) {
    let OnALine { hint: tool_hint, then } = tool;
    let mut app = App::default();
    sketching(&mut app);
    let mut hand = Hand::new(&mut app);
    hand.sk_tool(SketchShape::Line).click2d(0.0, 0.0).double_click2d(40.0, 0.0);
    hand.key(egui::Key::Escape).sk_select();
    hand.click2d(20.0, 0.0);
    if !hand.press_hint(&crate::i18n::tr(tool_hint)) {
        missing.push(format!("{name}: no button {tool_hint}"));
        return;
    }
    for c in then {
        hand.click2d(c.0, c.1);
    }
    save(&mut hand, name, missing);
}

#[test]
#[ignore = "pictures to look at, written into target/boxes-look"]
fn the_boxes_look() {
    let mut missing = Vec::new();
    drawn("sketch-rect", SketchShape::Rect, &[(0.0, 0.0), (40.0, 30.0)], &mut missing);
    drawn("sketch-circle", SketchShape::Circle, &[(0.0, 0.0), (20.0, 0.0)], &mut missing);
    drawn("sketch-arc", SketchShape::Arc, &[(0.0, 0.0), (20.0, 0.0), (0.0, 20.0)], &mut missing);
    drawn("sketch-slot", SketchShape::Slot, &[(0.0, 0.0), (40.0, 0.0), (40.0, 8.0)], &mut missing);
    drawn("sketch-ellipse", SketchShape::Ellipse, &[(0.0, 0.0), (30.0, 0.0), (0.0, 15.0)], &mut missing);
    drawn("sketch-polygon", SketchShape::Polygon, &[(0.0, 0.0), (20.0, 0.0)], &mut missing);
    reopened("sketch-rect-reopened", SketchShape::Rect, &[(0.0, 0.0), (40.0, 30.0)], (20.0, 0.0), &mut missing);
    reopened("sketch-circle-reopened", SketchShape::Circle, &[(0.0, 0.0), (20.0, 0.0)], (20.0, 0.0), &mut missing);
    reopened("sketch-polygon-reopened", SketchShape::Polygon, &[(0.0, 0.0), (20.0, 0.0)], (20.0, 0.0), &mut missing);
    on_a_chosen_line("sketch-dim-length", OnALine { hint: "tb-dim-hint", then: &[(20.0, 0.0), (20.0, 10.0)] }, &mut missing);
    on_a_chosen_line("sketch-offset", OnALine { hint: "tb-offset-hint", then: &[(20.0, 6.0)] }, &mut missing);
    {
        let mut app = App::default();
        sketching(&mut app);
        let mut hand = Hand::new(&mut app);
        hand.sk_tool(SketchShape::Rect).click2d(0.0, 0.0).click2d(40.0, 30.0).key(egui::Key::Enter);
        hand.sk_select();
        if hand.press_hint(&crate::i18n::tr("tb-fillet-sketch-hint")) {
            hand.mouse2d(40.0, 30.0);
            save(&mut hand, "sketch-fillet", &mut missing);
        } else {
            missing.push("sketch-fillet: no button".to_string());
        }
    }
    on_a_chosen_line("sketch-rotate", OnALine { hint: "tb-rotate-hint", then: &[(0.0, 0.0)] }, &mut missing);
    {
        let mut app = super::super::screen_keys::tests::plate();
        let part = app.project.components.iter().rev().find(|c| c.parent.is_some()).map(|c| c.id).expect("the part");
        app.enter_component(part);
        app.chosen.sel = Sel::Sketch(0);
        app.viewing.mode_3d = true;
        app.start_feat_cmd(qymcad_ui_state::PartTool::Extrude);
        let mut hand = Hand::as_it_stands(&mut app);
        save(&mut hand, "part-extrude", &mut missing);
    }
    {
        let mut app = App::default();
        let mut hand = Hand::new(&mut app);
        super::super::the_mate_window_stands_still_while_it_runs::tests::a_cylindrical_mate_with_its_window_open(&mut hand);
        save(&mut hand, "assembly-mate", &mut missing);
    }
    println!("boxes not opened: {missing:?}");
}
