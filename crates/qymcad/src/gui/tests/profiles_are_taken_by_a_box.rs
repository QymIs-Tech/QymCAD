//! THE PROFILES OF AN EXTRUSION ARE TAKEN BY A BOX, as the curves of a sketch are: drawn left to right it takes the
//! contours wholly inside, right to left the ones it touches; without Shift it replaces what was gathered, with Shift
//! it adds - in the 3D view and on the sheet. Reported behaviour: the closed contours for an operation were taken by a
//! click only; a drag with the left button drew no box.
use crate::gui::hand::{Carry, Drag2d, Hand};
use crate::gui::App;
use qymcad_core::model::Id;

/// Three 20 x 20 squares side by side on the XY plane of a part - at x 0, 30 and 60 - and the extrusion taken with their
/// sketch selected. Answers the contours of the three, from left to right.
fn three_squares_and_an_extrusion(app: &mut App) -> Vec<Id> {
    let part = app.project.add_part("Plate");
    app.enter_component(part);
    let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
    let mut hand = Hand::new(app);
    hand.sk_tool(qymcad_ui_state::SketchShape::Rect);
    for x in [0.0, 30.0, 60.0] {
        hand.click2d(x, 0.0).click2d(x + 20.0, 20.0);
    }
    app.finish_sketch_edit();
    app.chosen.sel = qymcad_ui_state::Sel::Sketch(si);
    let mut contours = qymcad_ui_state::sketch_closed_contours(&app.project, si);
    let left = |app: &App, id: Id| app.project.contour_index(id).map_or(f64::MAX, |ci| app.project.contours[ci].points.iter().map(|p| p.x).fold(f64::MAX, f64::min));
    contours.sort_by(|a, b| left(app, *a).total_cmp(&left(app, *b)));
    assert_eq!(contours.len(), 3, "setup: three squares make three contours");
    Hand::new(app).look_at([40.0, 10.0, 0.0], 6.0).tool(qymcad_ui_state::PartTool::Extrude);
    contours
}

/// The profiles the extrusion holds, by the order of `contours`: "A", "B", "C".
fn held(app: &App, contours: &[Id]) -> String {
    let names = ["A", "B", "C"];
    (0..contours.len()).filter(|k| app.tools.gsel.profiles.contains(&contours[*k])).map(|k| names[k]).collect()
}

/// THE BOX ON SCREEN ROUND THE SQUARES `which` (their corners laid by the camera), `pad` points wider each way.
fn round(hand: &mut Hand, which: &[f64], pad: f32) -> egui::Rect {
    let mut r = egui::Rect::NOTHING;
    for x in which {
        for (dx, dy) in [(0.0, 0.0), (20.0, 0.0), (20.0, 20.0), (0.0, 20.0)] {
            r.extend_with(hand.on_screen3d([x + dx, dy, 0.0]));
        }
    }
    r.expand(pad)
}

#[test]
fn profiles_are_taken_by_a_box_in_3d() {
    let mut app = App::default();
    let contours = three_squares_and_an_extrusion(&mut app);
    let mut problems = Vec::new();
    let mut hand = Hand::new(&mut app);
    let turned = |hand: &Hand| [hand.app.viewing.cam.yaw, hand.app.viewing.cam.pitch];
    let before = turned(&hand);

    // a window left to right round A and B: those two, whatever was gathered before
    let ab = round(&mut hand, &[0.0, 30.0], 12.0);
    hand.carry(Carry { from: ab.left_top(), to: ab.right_bottom() }).let_go(ab.right_bottom());
    if held(hand.app, &contours) != "AB" {
        problems.push(format!("a window round A and B: the extrusion holds {:?}, \"AB\" expected", held(hand.app, &contours)));
    }
    // a crossing right to left over the right side of C, Shift pressed before the release: C added
    let c = round(&mut hand, &[60.0], 0.0);
    let from = egui::pos2(c.right() + 12.0, c.center().y - 4.0);
    let to = egui::pos2(c.right() - 6.0, c.center().y + 4.0);
    hand.carry(Carry { from, to }).let_go_holding(to, egui::Modifiers::SHIFT);
    if held(hand.app, &contours) != "ABC" {
        problems.push(format!("a crossing over the side of C with Shift: the extrusion holds {:?}, \"ABC\" expected", held(hand.app, &contours)));
    }
    // the same crossing without Shift: C alone
    hand.carry(Carry { from, to }).let_go(to);
    if held(hand.app, &contours) != "C" {
        problems.push(format!("a crossing over the side of C without Shift: the extrusion holds {:?}, \"C\" expected", held(hand.app, &contours)));
    }
    if turned(&hand) != before {
        problems.push(format!("the view turned under the boxes: yaw and pitch {before:?} -> {:?}", turned(&hand)));
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn profiles_are_taken_by_a_box_on_the_sheet() {
    let mut app = App::default();
    let contours = three_squares_and_an_extrusion(&mut app);
    let mut problems = Vec::new();
    let mut hand = Hand::new(&mut app);
    let plain = egui::Modifiers::NONE;
    let button = egui::PointerButton::Primary;

    // a window left to right round A and B
    hand.drag2d_held(Drag2d { from: (-6.0, 26.0), to: (56.0, -6.0), button, modifiers: plain });
    if held(hand.app, &contours) != "AB" {
        problems.push(format!("a window round A and B on the sheet: the extrusion holds {:?}, \"AB\" expected", held(hand.app, &contours)));
    }
    // a crossing right to left through C, Shift held: C added
    hand.drag2d_held(Drag2d { from: (86.0, 12.0), to: (56.0, 8.0), button, modifiers: egui::Modifiers::SHIFT });
    if held(hand.app, &contours) != "ABC" {
        problems.push(format!("a crossing through C with Shift on the sheet: the extrusion holds {:?}, \"ABC\" expected", held(hand.app, &contours)));
    }
    // a window round A alone, without Shift: A alone
    hand.drag2d_held(Drag2d { from: (-6.0, 26.0), to: (26.0, -6.0), button, modifiers: plain });
    if held(hand.app, &contours) != "A" {
        problems.push(format!("a window round A without Shift on the sheet: the extrusion holds {:?}, \"A\" expected", held(hand.app, &contours)));
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
