//! A CORNER AT A CROSSING KEEPS ITS TWO SIDES: two lines crossing, the fillet or the chamfer tool taken, the two lines
//! clicked with Shift, a click in the lower quarter and a size typed - the corner is cut there, the two sides of that
//! quarter and the arc or the cut line are what is left, and the halves beyond the crossing are gone, as in the other
//! CAD of the trade. The corner dragged by the selected centre of its arc goes as one: the arc still touches both
//! sides.
//!
//! Reported behaviour: the halves beyond the crossing were left joined at the corner point, held to it, and the corner
//! dragged by the centre of its arc pulled them round - the drawing fell apart.
use crate::gui::hand::Hand;
use crate::gui::{App, Sel};
use qymcad_core::geom::Point2;
use qymcad_core::model::curve::Curve;

/// WHICH TOOL CUTS THE CORNER.
#[derive(Clone, Copy, Debug)]
enum Cut {
    Fillet,
    Chamfer,
}

/// Two lines crossing at (5, 10) drawn by hand, the corner tool taken, both clicked with Shift, the lower quarter clicked
/// and 3 typed.
fn cut_at_the_crossing(cut: Cut) -> App {
    let mut app = App::default();
    let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
    app.chosen.sel = Sel::Sketch(si);
    let mut hand = Hand::new(&mut app);
    hand.sk_tool(qymcad_ui_state::SketchShape::Line).click2d(-15.0, -5.0).double_click2d(25.0, 25.0);
    hand.click2d(20.0, -10.0).double_click2d(-5.0, 30.0);
    hand.key(egui::Key::Escape).sk_select();
    let hint = match cut {
        Cut::Fillet => "tb-fillet-sketch-hint",
        Cut::Chamfer => "tb-chamfer-sketch-hint",
    };
    assert!(hand.press_hint(&crate::i18n::tr(hint)), "no {cut:?} button");
    hand.close_window();
    hand.shift_click2d(-7.0, 1.0).shift_click2d(14.375, -2.5);
    hand.click2d(5.0, 6.0);
    hand.frame(Vec::new());
    hand.chord(egui::Modifiers::COMMAND, egui::Key::A).type_text("3").key(egui::Key::Enter);
    hand.key(egui::Key::Escape).key(egui::Key::Escape);
    app
}

#[test]
fn a_corner_cut_at_a_crossing_leaves_its_two_sides_alone() {
    let mut failures = Vec::new();
    for cut in [Cut::Fillet, Cut::Chamfer] {
        let app = cut_at_the_crossing(cut);
        let s = &app.project.sketches[0];
        let curves: Vec<Curve> = s.entities.iter().filter_map(|e| Curve::of(s, e)).collect();
        // what stands above the crossing at (5, 10): nothing of the halves beyond it
        let above = curves.iter().any(|c| (0..=40).map(|k| c.at(k as f64 / 40.0)).any(|q| q.y > 12.0));
        if curves.len() != 3 || above {
            failures.push(format!("{cut:?} at the crossing: {} curves left, a half beyond the crossing left {above}", curves.len()));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn a_fillet_at_a_crossing_goes_whole_with_the_centre_of_its_arc() {
    let mut app = cut_at_the_crossing(Cut::Fillet);
    let si = 0;
    let centre = {
        let s = &app.project.sketches[si];
        let c = s.entities.iter().find_map(|e| e.kind.as_arc()).map(|a| a.center).expect("the arc of the fillet");
        s.point(c).map(|q| (q.x, q.y)).expect("its centre")
    };
    let mut hand = Hand::new(&mut app);
    hand.sk_select().click2d(centre.0, centre.1);
    hand.drag2d(centre, (centre.0 + 2.0, centre.1 - 2.0));
    let s = &hand.app.project.sketches[si];
    let arc = s.entities.iter().find(|e| e.kind.as_arc().is_some()).and_then(|e| Curve::of(s, e)).expect("the arc");
    let lines: Vec<Curve> = s.entities.iter().filter(|e| e.kind.as_line().is_some()).filter_map(|e| Curve::of(s, e)).collect();
    // each end of the arc stands on a line, the line running the way the arc runs there
    let [a, b] = arc.ends();
    let touching = |p: Point2| lines.iter().any(|l| l.nearest(p).off < 1e-6);
    assert!(lines.len() == 2 && touching(a) && touching(b), "the fillet dragged by the centre of its arc: {} lines, the ends of the arc on them {} {}", lines.len(), touching(a), touching(b));
}
