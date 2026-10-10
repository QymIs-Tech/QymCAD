//! AN OPEN CHAIN IS OFFSET by hand: a polyline drawn, its three lines chosen, Offset taken, the pointer on the side the
//! copy should go to - Enter there, or a click fixing the side and the distance typed beside the copy. The copy runs off
//! the chain on that side, its corners sharp, and the window says what the tool takes where nothing it can offset is
//! chosen.
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
    /// THE COPY GOES TO THE SIDE OF THE POINTER, ITS CORNERS SHARP. Above the polyline, Enter with the pointer there: 3 to
    /// its left, the corners at (17, 3) and (17, 18). Below it, a click fixing the side and 4 typed into the box beside the
    /// copy, Enter: 4 to its right, the corners at (24, -4) and (24, 11). A corner of two lines is a corner of the copy,
    /// not an arc - reported behaviour: "the polyline has no fillets, and the copy is rounded".
    fn an_open_chain_is_offset_to_the_side_of_the_pointer() {
        let mut failures = Vec::new();
        let mut s = a_polyline_in_hand();
        let at = s.on_sketch(5.0, 6.0);
        s.move_to(at);
        s.key(Key::Enter);
        let above = [(0.0, 3.0), (17.0, 3.0), (17.0, 18.0), (40.0, 18.0)];
        let missing: Vec<(f64, f64)> = above.into_iter().filter(|&(x, y)| !stands(&mut s, x, y)).collect();
        let sk = s.document().sketches[0].clone();
        if !missing.is_empty() || sk.redundant != 0 || sk.arcs != 0 {
            failures.push(format!("Enter with the pointer above: nothing stands at {missing:?}, {} redundant, {} arcs; status {:?}; the sketch shows {:?}", sk.redundant, sk.arcs, s.status(), sk.places));
        }
        let mut s = a_polyline_in_hand();
        s.click_on_sketch(5.0, -6.0);
        s.type_text("4");
        s.key(Key::Enter);
        let below = [(0.0, -4.0), (24.0, -4.0), (24.0, 11.0), (40.0, 11.0)];
        let missing: Vec<(f64, f64)> = below.into_iter().filter(|&(x, y)| !stands(&mut s, x, y)).collect();
        let sk = s.document().sketches[0].clone();
        if !missing.is_empty() || sk.redundant != 0 || sk.arcs != 0 {
            failures.push(format!("the side fixed below by a click, 4 typed beside the copy: nothing stands at {missing:?}, {} redundant, {} arcs; status {:?}; the sketch shows {:?}", sk.redundant, sk.arcs, s.status(), sk.places));
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
