//! A POLYGON OF A SKETCH REOPENED: its sides, its size and the names given to them, as a person changes them after it
//! is drawn.
use qymcad::{Key, Session};
use qymcad_acceptance::build::{draw, empty_sketch};
use qymcad_acceptance::probe;

/// A hexagon about the origin, its corner at (10, 0), drawn with the polygon tool and put down: the arrow in hand.
fn a_hexagon() -> Session {
    let mut s = empty_sketch();
    draw(&mut s, "tb-polygon-hint", &[(0.0, 0.0), (10.0, 0.0)]);
    s.key(Key::Escape).key(Key::Escape);
    s
}

/// The box of the polygon opened by a double click on its side between the corners (10, 0) and (5, 8.66).
fn reopened(s: &mut Session) {
    let side = s.on_sketch(7.5, 4.33);
    s.double_click(side);
}

probe! {
    /// THE SIDES OF A POLYGON ARE IN ITS BOX WHEN IT IS REOPENED: a double click on a side of a hexagon opens its box
    /// with the number of its sides, and 8 typed there makes it an octagon. Reported behaviour: the box reopened held
    /// the radius and the turn, and the sides could not be changed.
    fn the_sides_of_a_polygon_reopened_are_in_its_box() {
        let mut s = a_hexagon();
        assert!(s.document().sketches[0].lines == 6, "setup: the hexagon has {} sides", s.document().sketches[0].lines);
        reopened(&mut s);
        let sides = s.word("opt-sides");
        s.fill(&sides, "8").key(Key::Enter);
        let lines = s.document().sketches[0].lines;
        assert!(lines == 8, "8 typed into the sides of the hexagon reopened: it has {lines} sides");
    }
}
