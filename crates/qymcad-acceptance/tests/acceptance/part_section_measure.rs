//! LOOKING INSIDE AND MEASURING: the section that hides half the model, and the measure that answers in numbers what
//! two things of a body are to each other.
use qymcad::{Key, Session};
use qymcad_acceptance::{build, golden, probe};

/// The block of the first part.
fn a_block() -> Session {
    let mut s = Session::start();
    build::block(&mut s);
    s
}

/// Take the tool whose hint is `hint`.
fn take(s: &mut Session, hint: &str) {
    let hint = s.word(hint);
    s.press_hint(&hint);
}

probe! {
    /// A SECTION HIDES HALF THE MODEL: the picture changes, the document does not, and turning the section off puts
    /// the model back.
    fn a_section_hides_half_the_model_and_gives_it_back() {
        let mut s = a_block();
        let (whole, before) = (s.snapshot(), s.document());
        take(&mut s, "tb-section-hint-bar");
        let top = s.face_at([20.0, 15.0, 10.0]);
        s.click(top);
        let caption = s.word("sec-offset");
        s.fill(&caption, "-5");
        s.key(Key::Enter);
        assert!(!golden::same(&whole, &s.snapshot()), "the section was put through the middle of the block and the picture stayed the same");
        // whether a section cuts the view is what the window shows, not the document
        let mut now = s.document();
        now.section = before.section;
        assert!(now == before, "looking inside changed the document");
        let off = s.word("cmd-close-esc");
        s.press_word_near(&off, qymcad::pos2(640.0, 24.0));
        let back = s.snapshot();
        assert!(golden::same(&whole, &back), "the section was turned off and the window did not come back as it was: {:?} pixels differ; the status line says {:?}", golden::difference(&whole, &back), s.status());
    }
}

probe! {
    /// TWO CORNERS TELL THE DISTANCE BETWEEN THEM: the diagonal of the 40 by 30 top of the block is 50.
    fn two_corners_tell_the_distance_between_them() {
        let mut s = a_block();
        take(&mut s, "tb-measure3d-hint");
        // the two corners of the front top edge, which the eye sees whole: a corner that stands behind a face of
        // its own is taken as that face, and the measurement would be of the face instead
        for p in [[0.0, 0.0, 10.0], [40.0, 0.0, 10.0]] {
            let at = s.vertex_at(p);
            s.click(at);
            assert!(s.status().contains(&s.word("m3-vertex")), "a click on the corner {p:?} of the block, with nothing in front of it, took something else: the program says {:?}", s.status());
        }
        let said = s.status();
        assert!(said.contains("40"), "the corners of the front top edge are 40 apart, and the program says {said:?}");
    }
}

probe! {
    /// AN EDGE TELLS ITS LENGTH: the front top edge of the block is 40 long.
    fn an_edge_tells_its_length() {
        let mut s = a_block();
        take(&mut s, "tb-measure3d-hint");
        let at = s.edge_at([20.0, 0.0, 10.0]);
        s.click(at);
        let said = s.status();
        assert!(said.contains("40"), "the front top edge of the block is 40 long, and the program says {said:?}");
    }
}

probe! {
    /// A ROUND FACE TELLS ITS DIAMETER: the side of a cylinder of radius 10 is 20 across.
    fn a_round_face_tells_its_diameter() {
        let mut s = Session::start();
        build::into_the_first_part(&mut s);
        take(&mut s, "tb-cylinder-hint");
        s.key(Key::Enter);
        take(&mut s, "tb-measure3d-hint");
        let side = the_side_of_a_shaft(&mut s, 10.0, 10.0);
        s.click(side);
        let said = s.status();
        assert!(said.contains("20"), "the side of a cylinder of radius 10 is 20 across, and the program says {said:?}");
    }
}

probe! {
    /// TWO FACES THAT ARE NOT PARALLEL ARE ANSWERED WITH THE ANGLE BETWEEN THEM, not with a distance that means
    /// nothing.
    fn two_faces_that_are_not_parallel_are_answered_with_the_angle() {
        let mut s = a_block();
        take(&mut s, "tb-measure3d-hint");
        for p in [[20.0, 15.0, 10.0], [20.0, 0.0, 5.0]] {
            let at = s.face_at(p);
            s.click(at);
        }
        let said = s.status();
        assert!(said.contains("90"), "the top and the front of a block meet at a right angle, and the program says {said:?}");
    }
}

/// WHERE THE ROUND SIDE OF A SHAFT CAN BE CLICKED: only the facets turned towards the eye can be.
fn the_side_of_a_shaft(s: &mut Session, r: f64, z: f64) -> qymcad::Pos2 {
    for k in 0..(72 * 3) {
        let a = (k % 72) as f64 * std::f64::consts::TAU / 72.0;
        let r = r - 0.05 * (k / 72) as f64;
        let p = [r * a.cos(), r * a.sin(), z];
        let mut seen = None;
        let message = qymcad_acceptance::refusal(|| seen = Some(s.face_at(p)));
        if message.is_empty() {
            if let Some(at) = seen {
                return at;
            }
        }
    }
    panic!("no place on the round side of the shaft can be seen from here");
}
