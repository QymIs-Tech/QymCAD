//! A PICKED CURVE OF ANY KIND IS LIT: a circle, an ellipse, an open spline and a line, each clicked with the arrow clear
//! of its points, is
//! drawn over in the colour of the selection - one highlight for every kind, laid out by the curve module.
use crate::gui::hand::Hand;
use crate::gui::App;
use qymcad_core::feature::SketchPlane;

/// A KIND OF CURVE: its name, how it is drawn, and a place on it to click.
struct Drawn {
    name: &'static str,
    draw: fn(&mut Hand),
    on: (f64, f64),
}

const KINDS: [Drawn; 4] = [
    Drawn {
        name: "circle",
        draw: |h| {
            h.sk_tool(qymcad_ui_state::SketchShape::Circle).click2d(40.0, 30.0).click2d(46.0, 30.0);
        },
        on: (46.0, 30.0),
    },
    Drawn {
        name: "ellipse",
        draw: |h| {
            h.sk_tool(qymcad_ui_state::SketchShape::Ellipse).click2d(40.0, 30.0).click2d(48.0, 30.0).click2d(40.0, 34.0);
        },
        // 45 deg round the ellipse, clear of the ends of its axes
        on: (45.657, 32.828),
    },
    Drawn {
        name: "open spline",
        draw: |h| {
            h.sk_tool(qymcad_ui_state::SketchShape::Spline).click2d(30.0, 30.0).click2d(40.0, 34.0).double_click2d(50.0, 30.0);
        },
        // between its first two nodes
        on: (35.0, 32.6),
    },
    Drawn {
        name: "line",
        draw: |h| {
            h.sk_tool(qymcad_ui_state::SketchShape::Line).click2d(30.0, 30.0).double_click2d(50.0, 30.0);
        },
        on: (37.0, 30.0),
    },
];

#[test]
fn every_kind_picked_is_lit_in_the_colour_of_the_selection() {
    let mut dark = Vec::new();
    for kind in &KINDS {
        let mut app = App::default();
        let si = app.create_sketch_on(SketchPlane::default());
        app.enter_sketch_edit(si);
        let mut hand = Hand::new(&mut app);
        (kind.draw)(&mut hand);
        hand.key(egui::Key::Escape).key(egui::Key::Escape);
        hand.sk_select().click2d(kind.on.0, kind.on.1);
        let colour = hand.app.scheme.pal.emphasis();
        if hand.strokes_near2d(kind.on, colour) == 0 {
            dark.push(format!("{}: picked {:?}", kind.name, hand.app.tools.sel_sk.items));
        }
    }
    assert!(dark.is_empty(), "picked and not lit:\n  {}", dark.join("\n  "));
}
