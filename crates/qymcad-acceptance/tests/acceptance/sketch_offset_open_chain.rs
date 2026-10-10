//! AN OPEN CHAIN IS OFFSET by hand: a polyline drawn, its three lines chosen, Offset taken, the distance on its bar, the
//! pointer on the side the copy should go to, Enter. The copy runs 3 off the chain on that side, its joints kept, and
//! the window says what the tool takes where nothing it can offset is chosen.
//!
//! Reported behaviour: a polyline of three lines chosen, Offset, a distance - no copy, and the window said to choose
//! something, as though nothing were chosen.
use qymcad::{Key, Session};
use qymcad_acceptance::build::{draw, empty_sketch, pick};
use qymcad_acceptance::probe;

/// The polyline east, north, east: (0, 0) - (20, 0) - (20, 15) - (40, 15), its three lines chosen, Offset taken and its
/// distance made 3.
fn a_polyline_in_hand() -> Session {
    let mut s = empty_sketch();
    draw(&mut s, "tb-line-hint", &[(0.0, 0.0), (20.0, 0.0), (20.0, 15.0), (40.0, 15.0)]);
    for (k, (x, y)) in [(10.0, 0.0), (20.0, 7.5), (30.0, 15.0)].into_iter().enumerate() {
        pick(&mut s, x, y, k > 0);
    }
    let hint = s.word("tb-offset-hint");
    s.press_hint(&hint);
    let distance = s.word("opt-distance");
    s.fill(&distance, "3");
    s
}

fn stands(s: &mut Session, x: f64, y: f64) -> bool {
    s.document().sketches[0].places.iter().any(|p| (p[0] - x).abs() < 1e-6 && (p[1] - y).abs() < 1e-6)
}

probe! {
    /// THE COPY GOES TO THE SIDE OF THE POINTER: above the polyline it runs 3 to its left - the concave joint cut to
    /// (17, 3), the convex one rounded about (20, 15) - and below it, 3 to its right, the joint (20, 0) rounded about
    /// it and the joint (20, 15) cut to (23, 12).
    fn an_open_chain_is_offset_to_the_side_of_the_pointer() {
        let mut failures = Vec::new();
        for (pointer, want) in [((5.0, 6.0), [(0.0, 3.0), (17.0, 3.0), (17.0, 15.0), (20.0, 18.0), (40.0, 18.0)]), ((5.0, -6.0), [(0.0, -3.0), (20.0, -3.0), (23.0, 0.0), (23.0, 12.0), (40.0, 12.0)])] {
            let mut s = a_polyline_in_hand();
            let at = s.on_sketch(pointer.0, pointer.1);
            s.move_to(at);
            s.key(Key::Enter);
            let missing: Vec<(f64, f64)> = want.into_iter().filter(|&(x, y)| !stands(&mut s, x, y)).collect();
            let sk = s.document().sketches[0].clone();
            if !missing.is_empty() || sk.redundant != 0 {
                failures.push(format!("the pointer at {pointer:?}: nothing stands at {missing:?}, {} redundant; status {:?}; the sketch shows {:?}", sk.redundant, s.status(), sk.places));
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }
}

probe! {
    /// A POINT CHOSEN AND NOTHING THE OFFSET TAKES: the status says what it takes, not "pick the entities".
    fn offset_with_only_a_point_chosen_says_what_it_takes() {
        let mut s = empty_sketch();
        draw(&mut s, "tb-point-hint", &[(10.0, 10.0)]);
        pick(&mut s, 10.0, 10.0, false);
        let hint = s.word("tb-offset-hint");
        s.press_hint(&hint);
        let said = s.status();
        assert!(said == s.word("sk-offset-takes"), "Offset with a point chosen says {said:?}");
    }
}
