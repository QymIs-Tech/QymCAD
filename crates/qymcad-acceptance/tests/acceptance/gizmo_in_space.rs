//! THE GIZMO: the arms that carry a thing and the rings that turn it, and the numbers that do the same by hand.
//! A part of an assembly carries one; a body carries one only where it belongs to no part.
use qymcad::{Key, Modifiers, PointerButton, Session};
use qymcad_acceptance::{build, probe};

/// THE BLOCK OF THE FIRST PART, 40 x 30 x 10 at the origin, seen in space with nothing in hand.
fn a_block() -> Session {
    let mut s = Session::start();
    build::block(&mut s);
    s.key(Key::Escape);
    s
}

/// AN ASSEMBLY OF TWO BLOCKS with the second part taken in the tree, so its gizmo stands at its origin.
fn a_second_part_taken() -> (Session, String) {
    let mut s = a_block();
    build::into_a_new_part(&mut s);
    let xy = s.word("plane-xy-table");
    s.press_word(&xy);
    let rect = s.word("tb-rect-hint");
    s.press_hint(&rect);
    s.click_on_sketch(60.0, 0.0).click_on_sketch(80.0, 20.0);
    let finish = s.word("wb-finish");
    s.press_word(&finish);
    let extrude = s.word("tb-extrude-hint");
    s.press_hint(&extrude);
    s.key(Key::Enter);
    let assembly = s.word("wb-assembly");
    s.press_word_near(&assembly, qymcad::pos2(0.0, 0.0));
    let second = s.document().parts.iter().filter(|p| !p.assembly).map(|p| p.name.clone()).next_back().expect("the second part");
    let row = s.find(&second, qymcad::pos2(0.0, 300.0)).unwrap_or_else(|| panic!("the part {second:?} is not in the tree; on screen: {:?}", s.words()));
    s.click(row.center());
    (s, second)
}

/// The gizmo of what is chosen, or the words of the window when there is none.
fn gizmo(s: &mut Session) -> qymcad::Gizmo {
    s.gizmo().unwrap_or_else(|| panic!("what is chosen carries no gizmo; on screen: {:?}", s.words()))
}

/// Where the part `name` stands and which way its own axes look.
fn placed(s: &mut Session, name: &str) -> ([f64; 3], [[f64; 3]; 3]) {
    s.document().parts.iter().find(|p| p.name == name).map(|p| (p.at, p.axes)).unwrap_or_else(|| panic!("the assembly holds no part {name:?}"))
}

probe! {
    /// A PART IS CARRIED ALONG THE ARM OF ITS GIZMO: the arm of Y is dragged and the part goes along Y and along
    /// nothing else, its axes left as they were.
    fn a_part_is_carried_along_the_arm_of_its_gizmo() {
        let (mut s, second) = a_second_part_taken();
        let (was, axes) = placed(&mut s, &second);
        let middle = s.seen_at([0.0, 0.0, 0.0]).expect("the origin of the part, where its gizmo stands");
        let arm = gizmo(&mut s).arms[1];
        s.drag(arm, arm + (arm - middle), PointerButton::Primary, Modifiers::default());
        let (now, turned) = placed(&mut s, &second);
        assert!((now[1] - was[1]).abs() > 1.0, "the part did not go along the arm of Y: it stood at {was:?} and stands at {now:?}");
        assert!((now[0] - was[0]).abs() < 1e-6 && (now[2] - was[2]).abs() < 1e-6, "the part went off the arm it was carried along: it stood at {was:?} and stands at {now:?}");
        assert!(turned == axes, "carrying the part turned it as well: its axes are {turned:?}");
    }
}

probe! {
    /// A PART IS TURNED BY THE RING OF ITS GIZMO: the ring about Z is dragged and the part's own axes come round
    /// with it, its Z left standing up.
    fn a_part_is_turned_by_the_ring_of_its_gizmo() {
        let (mut s, second) = a_second_part_taken();
        let (_, was) = placed(&mut s, &second);
        let middle = s.seen_at([0.0, 0.0, 0.0]).expect("the origin of the part, where its gizmo stands");
        let giz = gizmo(&mut s);
        // the ring is taken a quarter of the way round from the arm of X and carried to the tip of the arm of Y
        let tip_of_y = middle + (giz.arms[1] - middle) * 2.0;
        s.drag(giz.rings[2], tip_of_y, PointerButton::Primary, Modifiers::default());
        let (_, now) = placed(&mut s, &second);
        assert!(now != was, "the part was turned by the ring about Z and its axes are as they were: {was:?}");
        assert!((now[2][2] - 1.0).abs() < 1e-6, "the part was turned about Z and its own Z went off the world's: {now:?}");
    }
}

probe! {
    /// A PART IS TURNED BY A NUMBER: 90 typed into the rotation of its properties, and the button of Z, stands it
    /// the other way about - its X along the world's Y.
    fn a_part_is_turned_by_a_number() {
        let (mut s, second) = a_second_part_taken();
        let rotation = s.word("comp-rotation");
        s.fill(&rotation, "90").key(Key::Enter);
        let held = s.field(&rotation).value;
        assert!(held.starts_with("90"), "90 was typed into the rotation of the part and the field holds {held:?}");
        // the button of Z stands on the same row as the rotation, to the right of it - the placement above has a
        // Z of its own, and the axes drawn in the corner another
        let row = s.find(&rotation, qymcad::pos2(1280.0, 300.0)).unwrap_or_else(|| panic!("the properties of the part do not offer a rotation; on screen: {:?}", s.words()));
        let button = s
            .words_at()
            .into_iter()
            .filter(|(w, r)| w == "Z" && r.center().y > row.min.y && r.center().y < row.max.y && r.min.x > row.max.x)
            .map(|(_, r)| r)
            .min_by(|a, b| a.min.x.total_cmp(&b.min.x))
            .unwrap_or_else(|| panic!("the rotation of the part offers no button of Z; on screen: {:?}", s.words()));
        s.click(button.center());
        let (_, now) = placed(&mut s, &second);
        assert!((now[0][1] - 1.0).abs() < 1e-3, "the part was turned 90 degrees about Z and its X looks {:?} instead of along the world's Y", now[0]);
        assert!((now[2][2] - 1.0).abs() < 1e-6, "the part was turned about Z and its own Z went off the world's: {now:?}");
    }
}

probe! {
    /// A NUMBER AT THE GIZMO CARRIES THE BODY: 25 typed at the arm of X puts it 25 along.
    fn a_number_at_the_gizmo_carries_the_body() {
        let (mut s, second) = a_second_part_taken();
        let (was, _) = placed(&mut s, &second);
        let arm = gizmo(&mut s).arms[0];
        s.click(arm);
        let label = s.word("giz-offset-x");
        assert!(s.shows(&label), "a click on the arm of X asked for no {label:?}: there is no typing a number at the gizmo; on screen: {:?}", s.words());
        s.fill(&label, "25").key(Key::Enter);
        let (now, _) = placed(&mut s, &second);
        assert!((now[0] - was[0] - 25.0).abs() < 1e-3, "25 was typed at the arm of X and the part went from {was:?} to {now:?}");
    }
}

probe! {
    /// Esc AT THE GIZMO LEAVES THE FIELD FIRST, THE NUMBER NEXT: the box of the number stands in the frame of a tool,
    /// and the body stays where it was.
    fn escape_at_the_gizmo_leaves_the_field_then_the_number() {
        let (mut s, second) = a_second_part_taken();
        let (was, _) = placed(&mut s, &second);
        let arm = gizmo(&mut s).arms[0];
        s.click(arm);
        let label = s.word("giz-offset-x");
        assert!(s.shows(&label), "a click on the arm of X asked for no {label:?}; on screen: {:?}", s.words());
        s.key(Key::Escape);
        assert!(s.shows(&label), "the first Esc took the number away with the field it was typed in; on screen: {:?}", s.words());
        s.key(Key::Escape);
        assert!(!s.shows(&label), "the second Esc left the number at the gizmo; on screen: {:?}", s.words());
        let (now, _) = placed(&mut s, &second);
        assert!(now == was, "Esc moved the part from {was:?} to {now:?}");
    }
}
