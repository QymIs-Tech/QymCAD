//! A CUT THROUGH ALL BOTH WAYS IS CHOSEN IN THE BAR AND REOPENS BOTH WAYS. "Through all" kept its direction, one way or
//! the other, and both ways was "symmetric" alone, which takes a depth: a cut through the whole part to both sides of a
//! sketch inside it could not be asked for. A node through all both ways, reopened with a double click, came back as
//! through all one way, and Enter wrote it so: the other side of the part kept its metal.
//!
//! And through all one way keeps to its side through the window: a plate drawn symmetric about XY, a square sketched on
//! XY in its middle, cut through all - half of the square's column is taken, not the whole (reported behaviour: a cut
//! through all one way took metal on the other side of a sketch lying inside the part).
use crate::gui::hand::Hand;
use crate::gui::App;
use qymcad_core::feature::{Extent, FeatureKind, Reach};
use qymcad_core::model::Id;

/// THE CUT OF THE DOCUMENT: its node and how far it goes.
struct Cut {
    node: Id,
    extent: Extent,
}

fn the_cut(app: &App) -> Option<Cut> {
    app.project.timeline.iter().find_map(|n| match &n.kind {
        FeatureKind::Combine { extent, .. } => Some(Cut { node: n.id, extent: *extent }),
        _ => None,
    })
}

/// A plate 40 x 40 x 20 drawn and extruded by hand symmetric about XY (z from -10 to 10), a square 10 x 10 drawn on
/// XY - in the middle of the plate - and cut through all, the extent picked in the bar by its word.
fn a_plate_cut_through(hand: &mut Hand, extent: &str) -> Id {
    assert!(hand.press_word(&qymcad_i18n::tr("plane-xy-table"), egui::pos2(0.0, 0.0)), "setup: a sketch is started on XY");
    hand.sk_tool(2).click2d(0.0, 0.0).click2d(40.0, 40.0);
    assert!(hand.press_word(&qymcad_i18n::tr("wb-finish"), egui::pos2(0.0, 0.0)), "setup: the sketch is finished");
    hand.close_window();
    hand.look_at([20.0, 20.0, 0.0], 8.0).tool(1);
    assert!(hand.press_word(&qymcad_i18n::tr("cmd-symmetric"), egui::pos2(0.0, 0.0)), "setup: the bar offers symmetric");
    // the rebuild of the plate waited for, as a person waits for its barrier to lift before the next sketch: pressed under
    // it, the plane XY was taken by the barrier - 3 frames of 4 ms each against a rebuild of 30 ms
    hand.set("height", 20.0).enter().close_window();
    assert!(hand.press_word(&qymcad_i18n::tr("plane-xy-table"), egui::pos2(0.0, 0.0)), "setup: a second sketch is started on XY");
    hand.sk_tool(2).click2d(15.0, 15.0).click2d(25.0, 25.0);
    assert!(hand.press_word(&qymcad_i18n::tr("wb-finish"), egui::pos2(0.0, 0.0)), "setup: the second sketch is finished");
    hand.close_window();
    hand.look_at([20.0, 20.0, 0.0], 8.0).tool(1);
    assert!(hand.press_word(&qymcad_i18n::tr("cmd-cut"), egui::pos2(0.0, 0.0)), "setup: the bar offers the cut");
    assert!(hand.press_word(extent, egui::pos2(0.0, 0.0)), "the bar offers {extent:?}");
    hand.enter();
    the_cut(hand.app).expect("setup: the cut is made").node
}

/// The node reopened with a double click on its row in the tree, and Enter.
fn reopened_and_entered(hand: &mut Hand, node: Id) {
    let ti = hand.app.project.timeline.iter().position(|n| n.id == node).expect("the cut in the timeline");
    let label = crate::gui::panels_tree::feature_row_label(&hand.app.project, ti);
    assert!(hand.double_click_word(&label), "the tree shows the cut as {label:?}");
    hand.look_at([20.0, 20.0, 0.0], 8.0).enter();
}

#[test]
fn a_cut_through_all_both_ways_reopens_both_ways() {
    let mut app = App::default();
    let mut hand = Hand::new(&mut app);
    let node = a_plate_cut_through(&mut hand, &qymcad_i18n::tr("cmd-through-all"));
    // the cut as a file holds it: through all, both ways
    for n in hand.app.project.timeline.iter_mut().filter(|n| n.id == node) {
        if let FeatureKind::Combine { extent, .. } = &mut n.kind {
            extent.reach = Reach::BothWays;
        }
    }
    reopened_and_entered(&mut hand, node);
    let extent = the_cut(hand.app).expect("the cut is still there").extent;
    assert!(extent.through && extent.reach == Reach::BothWays, "the cut through all both ways came back as {extent:?}");
}

#[test]
fn a_cut_through_all_both_ways_is_chosen_in_the_bar_and_reopens_so() {
    let mut app = App::default();
    let mut hand = Hand::new(&mut app);
    let node = a_plate_cut_through(&mut hand, &qymcad_i18n::tr("cmd-through-both"));
    let made = the_cut(hand.app).expect("the cut is made").extent;
    assert!(made.through && made.reach == Reach::BothWays, "the cut chosen through all both ways was made {made:?}");
    // the plate is cut through its whole height, to both sides of the sketch: 10 x 10 x 20 taken from 40 x 40 x 20
    let volume = cut_volume(&mut hand, node);
    assert!((volume - 30000.0).abs() < 1.0, "the plate cut through all both ways holds {volume:.1} mm^3, not 30000");
    reopened_and_entered(&mut hand, node);
    let extent = the_cut(hand.app).expect("the cut is still there").extent;
    assert!(extent.through && extent.reach == Reach::BothWays, "reopened and entered, the cut came back as {extent:?}");
}

/// The volume of the body the cut `node` makes, once the window has rebuilt it.
fn cut_volume(hand: &mut Hand, node: Id) -> f64 {
    hand.settle();
    let body = hand.app.project.timeline.iter().find(|n| n.id == node).and_then(|n| n.kind.body()).unwrap_or(0);
    hand.app.project.bodies.iter().find(|b| b.id == body).map_or(0.0, |b| b.mesh.volume())
}

#[test]
fn a_cut_through_all_one_way_takes_metal_on_its_side_only() {
    let mut app = App::default();
    let mut hand = Hand::new(&mut app);
    let node = a_plate_cut_through(&mut hand, &qymcad_i18n::tr("cmd-through-all"));
    let made = the_cut(hand.app).expect("the cut is made").extent;
    assert!(made.through && made.reach != Reach::BothWays, "the cut through all one way was made {made:?}");
    // half of the column under the square: 10 x 10 x 10 taken from 40 x 40 x 20
    let volume = cut_volume(&mut hand, node);
    assert!((volume - 31000.0).abs() < 1.0, "the plate cut through all one way holds {volume:.1} mm^3, not 31000");
}
