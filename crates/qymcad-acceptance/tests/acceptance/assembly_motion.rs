//! WHAT A MATE LETS A PART DO: the limits of its degree of freedom, the number that drives it, the mechanism pulled
//! by the mouse, and the animation that runs the degree over its range.
use qymcad::{Key, Session};
use qymcad_acceptance::{build, probe};

/// Two parts, each holding a block: the first at the origin, the second 60 along X, joined by a revolute mate at
/// their corners - a mechanism with one degree of freedom. The mate is taken in the panel.
fn a_hinge() -> Session {
    let mut s = hinge_at(100.0);
    open_the_hinge(&mut s);
    s
}

/// THE HINGE ON THE NEAR CORNER of the second part (60, 0, 10): it hangs beside the block (40 to 80 along X) instead of
/// folding onto it, where nothing of it could be taken by the hand apart from the block.
fn a_hinge_beside() -> Session {
    hinge_at(60.0)
}

/// The block, a second part 60 along X, and a hinge from the block's corner (40, 0, 10) to the second part's corner at
/// `x` along X.
fn hinge_at(x: f64) -> Session {
    let mut s = Session::start();
    s.key(Key::Escape);
    build::block(&mut s);
    let assembly = s.word("wb-assembly");
    s.press_word_near(&assembly, qymcad::pos2(0.0, 0.0));
    let new_part = s.word("tb-new-part-hint");
    s.press_hint(&new_part);
    let second = s.document().parts.last().cloned().expect("the second part").name;
    build::rectangle_on_xy(&mut s);
    let finish = s.word("wb-finish");
    s.press_word(&finish);
    let extrude = s.word("tb-extrude-hint");
    s.press_hint(&extrude);
    s.key(Key::Enter);
    s.press_word_near(&assembly, qymcad::pos2(0.0, 0.0));
    let row = s.find(&second, qymcad::pos2(0.0, 300.0)).unwrap_or_else(|| panic!("the part {second:?} is not in the tree"));
    s.click(row.center());
    s.fill("X", "60").key(Key::Enter);
    let start = s.word("jp-start-joint");
    s.press_word_near(&start, qymcad::pos2(1100.0, 400.0));
    let caption = s.word("j-kind");
    let list = s.field(&caption);
    s.click(list.rect.center());
    let want = s.word("joint-kind-revolute");
    s.press_word_near(&want, list.rect.center());
    let a = s.vertex_at([40.0, 0.0, 10.0]);
    s.click(a);
    let b = s.vertex_at([x, 0.0, 10.0]);
    s.click(b);
    s.key(Key::Enter).key(Key::Escape); // Enter keeps the hinge the second pick made, Esc puts the tool down
    s
}

/// THE HINGE OPENED FOR EDITING as any joint kept is: by a double click on its row. Taken first, the row stands where
/// the panel lays it out for a taken joint; the pause keeps that click from counting into the double one.
fn open_the_hinge(s: &mut Session) {
    let joint = s.document().joints.last().cloned().unwrap_or_else(|| panic!("no hinge was made: the program says {:?}", s.status()));
    let row = s.find(&joint.name, qymcad::pos2(1100.0, 600.0)).unwrap_or_else(|| panic!("the hinge is not in the panel; on screen: {:?}", s.words()));
    s.click(row.center());
    let row = s.find(&joint.name, qymcad::pos2(1100.0, 600.0)).unwrap_or_else(|| panic!("the hinge left the panel; on screen: {:?}", s.words()));
    s.pause(std::time::Duration::from_millis(700));
    s.double_click(row.center());
}

/// Where the second part stands and which way it is turned.
fn second(s: &mut Session) -> qymcad::Part {
    s.document().parts.last().cloned().unwrap_or_else(|| panic!("the assembly holds no second part"))
}

probe! {
    /// AN ANIMATION WITHOUT LIMITS STILL RUNS, over the range it takes by default - a full turn for an angle, the
    /// size of the part for a travel - as in the professional systems: the button is there, and the part moves.
    fn an_animation_without_limits_runs_over_its_default_range() {
        let mut s = a_hinge();
        let before = second(&mut s);
        let run = s.word("j-anim-angle");
        s.press_word_near(&run, qymcad::pos2(1100.0, 600.0));
        let moved = |s: &mut Session| {
            let now = second(s);
            now.at != before.at || now.axes != before.axes
        };
        s.wait_for("the animation with no limits set to move the part", std::time::Duration::from_secs(30), moved);
    }
}

probe! {
    /// THE LIMITS ARE SET AND THE ANIMATION RUNS the degree over its range; stopping puts everything back.
    fn the_limits_are_set_and_the_animation_runs_and_stops() {
        let mut s = a_hinge();
        let (min, max) = (s.word("j-min"), s.word("j-max"));
        s.toggle(&min);
        s.fill(&min, "-30");
        s.toggle(&max);
        s.fill(&max, "30").key(Key::Enter);
        // what the animation must give back is what stood when it was started - the limits set before it solve the
        // assembly too
        let before = second(&mut s);
        let run = s.word("j-anim-angle");
        s.press_word_near(&run, qymcad::pos2(1100.0, 600.0));
        let moved = |s: &mut Session| {
            let during = second(s);
            during.axes != before.axes || during.at != before.at
        };
        s.wait_for("the animation within its limits to move the part", std::time::Duration::from_secs(30), moved);
        let stop = s.word("j-anim-stop");
        s.press_word_near(&stop, qymcad::pos2(1100.0, 600.0));
        let after = second(&mut s);
        // put back within the solver's noise: the view solves the assembly once more after the stop (1e-9 apart)
        let close = |a: &[f64], b: &[f64]| a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-6);
        let back = close(&after.at, &before.at) && (0..3).all(|k| close(&after.axes[k], &before.axes[k]));
        assert!(back, "stopping the animation did not put the part back: it stands at {:?} looking {:?}, it stood at {:?} looking {:?}", after.at, after.axes, before.at, before.axes);
    }
}

probe! {
    /// A DEGREE OF FREEDOM IS DRIVEN BY A NUMBER: the angle typed turns the part by it.
    fn a_degree_of_freedom_is_driven_by_a_number() {
        let mut s = a_hinge();
        let before = second(&mut s);
        let angle = s.word("j-angle-lower");
        s.fill(&angle, "90").key(Key::Enter);
        let now = second(&mut s);
        assert!(now.axes != before.axes, "the angle typed turned nothing: the part looks {:?} as before", now.axes);
    }
}

probe! {
    /// THE MECHANISM IS PULLED BY THE MOUSE: dragging the part that hangs on the hinge turns it about the hinge.
    fn the_mechanism_is_pulled_by_the_mouse() {
        let mut s = a_hinge_beside();
        let before = second(&mut s);
        let from = s.face_at([60.0, 15.0, 10.0]); // the part on the hinge, not the grounded block
        // aimed on screen from where the part was taken: turning the view to bring another point in would move the
        // part out from under the grab
        let to = from + qymcad::vec2(0.0, -120.0);
        s.drag(from, to, qymcad::PointerButton::Primary, qymcad::Modifiers::SHIFT);
        let now = second(&mut s);
        assert!(now.axes != before.axes || now.at != before.at, "the part on the hinge did not follow the mouse: it stands at {:?} looking {:?}", now.at, now.axes);
    }
}
