//! WHERE A SKETCH SITS: on a plane of the world, on a datum plane, on a face of its own part, and on a face of a
//! neighbouring part - which makes a live reference between the two.
use qymcad::{Key, Session};
use qymcad_acceptance::build;
use qymcad_acceptance::probe;

/// Where the last sketch of the document sits, as the door names the seat.
fn seat(s: &mut Session) -> String {
    s.document().sketches.last().map(|sk| sk.seat.clone()).unwrap_or_else(|| panic!("the document holds no sketch"))
}

/// Draw a 10 x 10 square and extrude it with the height the tool offers (10), leaving the sketch.
fn a_block_of_the_open_sketch(s: &mut Session) {
    build::draw(s, "tb-rect-hint", &[(0.0, 0.0), (10.0, 10.0)]);
    let finish = s.word("wb-finish");
    s.press_word(&finish);
    let extrude = s.word("tb-extrude-hint");
    s.press_hint(&extrude);
    s.key(Key::Enter);
}

probe! {
    /// A SKETCH ON A PLANE OF THE WORLD: the plane is picked by its name and the sketch sits on it.
    fn a_sketch_sits_on_the_plane_of_the_world_it_was_started_on() {
        let mut s = Session::start();
        build::into_the_first_part(&mut s);
        let xy = s.word("plane-xy-table");
        s.press_word(&xy);
        assert!(seat(&mut s) == "XY", "the sketch does not sit on the XY plane: it sits on {:?}", seat(&mut s));
        let sketch = s.document().editing.clone();
        assert!(sketch.is_some(), "the sketch did not open for drawing");
    }
}

probe! {
    /// A SKETCH ON A DATUM PLANE stands where the datum stands: a datum 20 above the table carries the body drawn on
    /// it up with it.
    fn a_sketch_sits_on_a_datum_plane_and_the_body_stands_on_it() {
        let mut s = Session::start();
        build::into_the_first_part(&mut s);
        let datum = s.word("g-datum-plane-hint");
        s.press_hint(&datum);
        let middle = s.in_space([0.0, 0.0, 0.0]);
        s.click(middle); // the table plane, which the datum is measured from
        // the offset typed into its own field, by its caption: the plane has its tilts and its turn beside it
        let offset = s.word("f-offset");
        s.fill(&offset, "20").key(Key::Enter);
        let pencil = s.word("g-sketch-pick-hint");
        s.press_hint(&pencil);
        let on_datum = s.in_space([0.0, 0.0, 20.0]);
        s.click(on_datum);
        assert!(seat(&mut s) == "datum", "the sketch does not sit on the datum plane: it sits on {:?}", seat(&mut s));
        a_block_of_the_open_sketch(&mut s);
        let body = s.document().bodies.last().cloned().unwrap_or_else(|| panic!("the sketch on the datum made no body"));
        assert!((body.min[2] - 20.0).abs() < 1e-6, "the body does not stand on the datum 20 above the table: it starts at {}", body.min[2]);
    }
}

probe! {
    /// A SKETCH ON A FACE OF ITS OWN PART sits on that face, and what is built from it stands on it.
    fn a_sketch_sits_on_a_face_of_its_own_part() {
        let mut s = Session::start();
        build::block(&mut s);
        let part = s.document().parts[0].name.clone();
        build::sketch_on_face(&mut s, [20.0, 15.0, 10.0]);
        assert!(seat(&mut s) == format!("face of {part}"), "the sketch does not sit on a face of {part:?}: it sits on {:?}", seat(&mut s));
        a_block_of_the_open_sketch(&mut s);
        let body = s.document().bodies.last().cloned().unwrap_or_else(|| panic!("the sketch on the face made no body"));
        assert!((body.max[2] - 20.0).abs() < 1e-6, "what was built on the 10 tall block, itself 10 tall, does not reach 20: it reaches {}", body.max[2]);
    }
}

probe! {
    /// A SKETCH ON A FACE OF A NEIGHBOURING PART is a live reference: the program says so, and when the neighbour
    /// grows the sketch drawn on it follows.
    fn a_sketch_on_the_face_of_a_neighbour_is_a_live_reference() {
        let mut s = Session::start();
        build::block(&mut s);
        let first = s.document().parts[0].name.clone();
        build::into_a_new_part(&mut s);
        let second = s.document().parts[1].name.clone();
        assert!(second != first, "the second part was not made: the parts are {:?}", s.document().parts);
        // the neighbour is only there to be seen and picked while "in context" is on
        let context = s.word("wb-in-context");
        let switch = s.widgets().into_iter().find(|w| w.label.ends_with(&context)).unwrap_or_else(|| panic!("there is no switch for working in context; on screen: {:?}", s.words()));
        if switch.checked == Some(false) {
            s.click(switch.rect.center());
        }
        build::sketch_on_face(&mut s, [20.0, 15.0, 10.0]);
        assert!(seat(&mut s) == format!("face of {first}"), "the sketch does not sit on a face of the neighbour {first:?}: it sits on {:?}", seat(&mut s));
        // the projected outline of that face follows the neighbour: made 60 wide, it is 60 in the sketch too
        let project = s.word("tb-project-body-hint");
        s.press_hint(&project);
        let outline = s.word("opt-face-outline");
        s.press_word_near(&outline, qymcad::pos2(640.0, 24.0));
        s.click_on_sketch(20.0, 15.0);
        let sk = s.document().sketches.last().cloned().expect("the sketch");
        assert!((sk.max[0] - sk.min[0] - 40.0).abs() < 1e-3, "the outline taken from the neighbour is not 40 across: it is {}", sk.max[0] - sk.min[0]);
        let finish = s.word("wb-finish");
        s.press_word(&finish);
        // back to the neighbour, made wider by its own sketch: up to the assembly first, as the path at the top leads
        let assembly = s.word("wb-assembly");
        s.press_word_near(&assembly, qymcad::pos2(0.0, 0.0));
        let row = s.find(&first, qymcad::pos2(0.0, 300.0)).unwrap_or_else(|| panic!("the neighbour {first:?} is not in the tree; on screen: {:?}", s.words()));
        s.double_click(row.center());
        let sketch = s.document().sketches[0].name.clone();
        let row = s.find(&sketch, qymcad::pos2(0.0, 300.0)).unwrap_or_else(|| panic!("the neighbour's sketch is not in the tree; on screen: {:?}", s.words()));
        s.double_click(row.center());
        s.drag_on_sketch((40.0, 30.0), (60.0, 30.0));
        let finish = s.word("wb-finish");
        s.press_word(&finish);
        let sk = s.document().sketches.last().cloned().expect("the sketch of the second part");
        assert!((sk.max[0] - sk.min[0] - 60.0).abs() < 1e-3, "the sketch on the neighbour's face did not follow it from 40 to 60: it is {} across", sk.max[0] - sk.min[0]);
    }
}

probe! {
    /// THE PROGRAM SAYS WHOSE FACE THE SKETCH WAS PUT ON: a sketch on a neighbour's face binds the two parts
    /// together, and a person has to be told by whose face they are now driven.
    fn the_program_says_whose_face_the_sketch_was_put_on() {
        let mut s = Session::start();
        build::block(&mut s);
        let first = s.document().parts[0].name.clone();
        build::into_a_new_part(&mut s);
        let context = s.word("wb-in-context");
        let switch = s.widgets().into_iter().find(|w| w.label.ends_with(&context)).unwrap_or_else(|| panic!("there is no switch for working in context; on screen: {:?}", s.words()));
        if switch.checked == Some(false) {
            s.click(switch.rect.center());
        }
        build::sketch_on_face(&mut s, [20.0, 15.0, 10.0]);
        let seen = s.words().iter().any(|w| w.contains(&first) && w.len() > first.len() + 8);
        assert!(s.status().contains(&first) || seen, "nothing on screen says the sketch was put on the face of {first:?} and now follows it; the status line says {:?}", s.status());
    }
}
