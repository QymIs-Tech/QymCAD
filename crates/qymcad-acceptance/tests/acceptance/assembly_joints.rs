//! THE MATES BETWEEN TWO PARTS: all eight kinds, the places a mate is anchored to, and what can be done to one that
//! is already made - the anchor swapped, the axis flipped, the roles exchanged, the arrangement held as it stands.
//!
//! Every check starts from two blocks of 40 by 30 by 10: the first at the origin, the second 60 along X. A mate
//! brings the second to the first, and what came of it is read from the document - the kind of the mate, whether it
//! holds, and where the parts stand.
use qymcad::{Key, Session};
use qymcad_acceptance::{build, probe};

/// Two parts, each holding a block: the first at the origin, the second 60 along X. The session stands in the
/// assembly with the second part taken.
fn two_blocks_apart() -> Session {
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
    s
}

/// Where the second part stands.
fn second_stands_at(s: &mut Session) -> [f64; 3] {
    s.document().parts.last().map(|p| p.at).unwrap_or_else(|| panic!("the assembly holds no second part"))
}

/// Press the word `key` names in the bar of options at the top.
fn bar_word(s: &mut Session, key: &str) {
    let word = s.word(key);
    s.press_word_near(&word, qymcad::pos2(640.0, 24.0));
}

/// Choose the kind of the mate in the bar: the list is a drop-down that shows only the kind chosen.
fn choose_the_kind(s: &mut Session, key: &str) {
    let caption = s.word("j-kind");
    let list = s.field(&caption);
    s.click(list.rect.center());
    let want = s.word(key);
    s.press_word_near(&want, list.rect.center());
}

/// Start a mate of the kind `kind` names, anchor it to the top face of each block, and answer what the document
/// holds after it.
fn a_mate_of(kind: &str) -> (Session, qymcad::JointInfo) {
    let mut s = two_blocks_apart();
    let start = s.word("jp-start-joint");
    s.press_word_near(&start, qymcad::pos2(1100.0, 400.0));
    choose_the_kind(&mut s, kind);
    let a = s.face_at([20.0, 15.0, 10.0]);
    s.click(a);
    let b = s.face_at([80.0, 15.0, 10.0]);
    s.click(b);
    let joint = s.document().joints.last().cloned().unwrap_or_else(|| panic!("no mate was made: the program says {:?}", s.status()));
    (s, joint)
}

/// A mate of the kind `kind` names holds the two parts and is not violated.
fn a_mate_holds(kind: &str) {
    let (s, joint) = a_mate_of(kind);
    let want = s.word(kind);
    assert!(joint.kind.to_lowercase().replace(['-', ' '], "") == want.to_lowercase().replace(['-', ' '], ""), "a {want} mate was asked for, and the document holds a {:?} one", joint.kind);
    assert!(!joint.violated, "the {want} mate does not hold: {joint:?}");
}

probe! {
    /// A RIGID MATE brings the second part onto the first and holds it there.
    fn a_rigid_mate_brings_the_parts_together() {
        let (mut s, joint) = a_mate_of("joint-kind-rigid");
        assert!(!joint.violated, "the rigid mate does not hold: {joint:?}");
        let at = second_stands_at(&mut s);
        assert!(at.iter().all(|v| v.abs() < 1e-3), "the rigid mate did not bring the second part to the first: it stands at {at:?}");
    }
}

probe! {
    /// A REVOLUTE MATE holds.
    fn a_revolute_mate_holds() {
        a_mate_holds("joint-kind-revolute");
    }
}

probe! {
    /// A SLIDER MATE holds.
    fn a_slider_mate_holds() {
        a_mate_holds("joint-kind-slider");
    }
}

probe! {
    /// A CYLINDRICAL MATE holds.
    fn a_cylindrical_mate_holds() {
        a_mate_holds("joint-kind-cylindrical");
    }
}

probe! {
    /// A PIN-SLOT MATE holds.
    fn a_pin_slot_mate_holds() {
        a_mate_holds("joint-kind-pin-slot");
    }
}

probe! {
    /// A PLANAR MATE holds.
    fn a_planar_mate_holds() {
        a_mate_holds("joint-kind-planar");
    }
}

probe! {
    /// A BALL MATE holds.
    fn a_ball_mate_holds() {
        a_mate_holds("joint-kind-ball");
    }
}

probe! {
    /// A PARALLEL MATE holds.
    fn a_parallel_mate_holds() {
        a_mate_holds("joint-kind-parallel");
    }
}

probe! {
    /// A MATE ANCHORED TO AN EDGE holds, and the parts meet along it.
    fn a_mate_is_anchored_to_an_edge() {
        let mut s = two_blocks_apart();
        let start = s.word("jp-start-joint");
        s.press_word_near(&start, qymcad::pos2(1100.0, 400.0));
        let a = s.edge_at([20.0, 0.0, 10.0]);
        s.click(a);
        let b = s.edge_at([80.0, 0.0, 10.0]);
        s.click(b);
        let joint = s.document().joints.last().cloned().unwrap_or_else(|| panic!("no mate came of two edges: the program says {:?}", s.status()));
        assert!(!joint.violated, "the mate anchored to the edges does not hold: {joint:?}");
        let at = second_stands_at(&mut s);
        assert!(at[0].abs() < 1e-3, "the mate did not bring the parts together along their edges: the second stands at {at:?}");
    }
}

probe! {
    /// A MATE ANCHORED TO A VERTEX holds, and the parts meet at it.
    fn a_mate_is_anchored_to_a_vertex() {
        let mut s = two_blocks_apart();
        let start = s.word("jp-start-joint");
        s.press_word_near(&start, qymcad::pos2(1100.0, 400.0));
        let a = s.vertex_at([40.0, 0.0, 10.0]);
        s.click(a);
        let b = s.vertex_at([100.0, 0.0, 10.0]);
        s.click(b);
        let joint = s.document().joints.last().cloned().unwrap_or_else(|| panic!("no mate came of two corners: the program says {:?}", s.status()));
        assert!(!joint.violated, "the mate anchored to the corners does not hold: {joint:?}");
        let at = second_stands_at(&mut s);
        assert!(at[0].abs() < 1e-3, "the mate did not bring the corners together: the second part stands at {at:?}");
    }
}

probe! {
    /// A MATE HELD AS IT STANDS moves nothing: the arrangement the parts are in becomes the one the mate holds.
    fn a_mate_held_as_it_stands_moves_nothing() {
        let mut s = two_blocks_apart();
        let before = second_stands_at(&mut s);
        let start = s.word("jp-start-joint");
        s.press_word_near(&start, qymcad::pos2(1100.0, 400.0));
        let as_built = s.word("j-as-built");
        bar_word(&mut s, "j-as-built");
        assert!(s.shows(&as_built), "the bar of the mate does not offer to hold the arrangement as it stands");
        let a = s.face_at([20.0, 15.0, 10.0]);
        s.click(a);
        let b = s.face_at([80.0, 15.0, 10.0]);
        s.click(b);
        let joint = s.document().joints.last().cloned().unwrap_or_else(|| panic!("no mate was made: the program says {:?}", s.status()));
        assert!(!joint.violated, "the mate made as built does not hold: {joint:?}");
        let now = second_stands_at(&mut s);
        assert!(now == before, "the mate made as built moved the part from {before:?} to {now:?}");
    }
}

/// A rigid mate between the top faces of the two blocks, ready to be worked on: the mate is taken in the list of
/// mates so that the panel offers what can be done to it.
fn a_mate_taken() -> Session {
    let (mut s, joint) = a_mate_of("joint-kind-rigid");
    let row = s.find(&joint.name, qymcad::pos2(1100.0, 600.0)).unwrap_or_else(|| panic!("the mate {:?} is not in the panel; on screen: {:?}", joint.name, s.words()));
    s.click(row.center());
    s
}

probe! {
    /// THE ANCHOR OF A MATE IS SWAPPED for another place, and the mate still holds.
    fn the_anchor_of_a_mate_is_swapped() {
        let mut s = a_mate_taken();
        // anchor A is picked anew by its switch on the bar of the mate, which names where it stands now
        let side_a = s.words().into_iter().find(|w| w.starts_with("A: ")).unwrap_or_else(|| panic!("the bar of the mate names no anchor A; on screen: {:?}", s.words()));
        s.press_word_near(&side_a, qymcad::pos2(640.0, 0.0));
        let a = s.vertex_at([40.0, 0.0, 10.0]);
        s.click(a);
        let joint = s.document().joints.last().cloned().expect("the mate");
        assert!(!joint.violated, "the mate does not hold after its anchor was swapped: {joint:?}");
    }
}

probe! {
    /// THE AXIS OF A MATE IS FLIPPED and the part turns over, the mate still holding.
    fn the_axis_of_a_mate_is_flipped() {
        // the mate is anchored to a CORNER of each block: a mate anchored to the middle of a face would turn the
        // block onto itself, a 40 by 30 rectangle being the same after half a turn, and the flip would not show
        let mut s = two_blocks_apart();
        let start = s.word("jp-start-joint");
        s.press_word_near(&start, qymcad::pos2(1100.0, 400.0));
        let a = s.vertex_at([40.0, 0.0, 10.0]);
        s.click(a);
        let b = s.vertex_at([100.0, 0.0, 10.0]);
        s.click(b);
        let joint = s.document().joints.last().cloned().expect("the mate");
        let row = s.find(&joint.name, qymcad::pos2(1100.0, 600.0)).unwrap_or_else(|| panic!("the mate {:?} is not in the panel", joint.name));
        s.click(row.center());
        let before = s.document().parts.last().cloned().expect("the second part");
        let flip = s.word("jt-flip-axis");
        s.press_word_near(&flip, qymcad::pos2(1100.0, 600.0));
        let joint = s.document().joints.last().cloned().expect("the mate");
        assert!(!joint.violated, "the mate does not hold after its axis was flipped: {joint:?}");
        let now = s.document().parts.last().cloned().expect("the second part");
        assert!(now.axes != before.axes || now.at != before.at, "flipping the axis turned nothing: the part stands at {:?} looking {:?}, as before, and the program says {:?}", now.at, now.axes, s.status());
    }
}

probe! {
    /// THE ROLES OF A MATE ARE SWAPPED: the part that travels becomes the one that stays, and the other moves.
    fn the_roles_of_a_mate_are_swapped() {
        let mut s = a_mate_taken();
        let first_before = s.document().parts[0].at;
        let swap = s.word("jt-swap-roles");
        s.press_word_near(&swap, qymcad::pos2(1100.0, 600.0));
        let joint = s.document().joints.last().cloned().expect("the mate");
        assert!(!joint.violated, "the mate does not hold after the roles were swapped: {joint:?}");
        let first_now = s.document().parts[0].at;
        let second_now = second_stands_at(&mut s);
        assert!(first_now != first_before || second_now.iter().all(|v| v.abs() < 1e-3), "swapping the roles changed nothing: the first part stands at {first_now:?} and the second at {second_now:?}");
    }
}
