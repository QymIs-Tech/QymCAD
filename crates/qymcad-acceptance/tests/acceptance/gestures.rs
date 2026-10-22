//! THE GESTURES OF THE MOUSE REACH WHAT THEY ARE MADE ON: a click in the 3D view takes what is under it, a drag on the
//! sketch takes what was pressed.
use qymcad::{Key, Session, SketchPick};
use qymcad_acceptance::{build, probe};

/// Is `text` written in the properties, right of the canvas - not in the tree, which names every body of the part.
fn in_the_properties(s: &mut Session, text: &str) -> bool {
    let right = s.canvas().max.x;
    s.words_at().iter().any(|(w, r)| w == text && r.min.x >= right)
}

probe! {
    /// A DOUBLE CLICK IN THE 3D VIEW TAKES THE BODY UNDER IT (one click takes the face), and a click on empty space
    /// lets it go.
    fn a_double_click_in_3d_takes_the_body_under_it() {
        let mut s = Session::start();
        build::block(&mut s);
        let body = s.document().bodies[0].name.clone();
        let nothing = s.word("props-pick-in-tree");
        assert!(s.shows(&nothing) && !in_the_properties(&mut s, &body), "before the click the properties already show the body");
        let top = s.face_at([20.0, 15.0, 10.0]);
        s.double_click(top);
        assert!(in_the_properties(&mut s, &body) && !s.shows(&nothing), "a double click on the top face of the block did not take the block; on screen: {:?}", s.words());
        let canvas = s.canvas();
        s.click(canvas.min + (canvas.max - canvas.min) * 0.05);
        assert!(s.shows(&nothing), "a click on empty space kept the block taken");
    }
}

probe! {
    /// A DRAG ON THE SKETCH TAKES THE POINT PRESSED ON, and leaves it where the pointer is let go.
    fn a_drag_on_the_sketch_takes_the_point_pressed() {
        let mut s = Session::start();
        build::into_the_first_part(&mut s);
        build::rectangle_on_xy(&mut s);
        // the first Esc ends the box of the sizes of the rectangle, the second puts the tool that drew it down
        s.key(Key::Escape).key(Key::Escape);
        s.drag_on_sketch((40.0, 30.0), (50.0, 45.0));
        assert_eq!(s.sketch_under(50.0, 45.0), Some(SketchPick::Point { at: (50.0, 45.0) }), "the corner dragged from (40, 30) is not at (50, 45)");
        assert_eq!(s.sketch_under(40.0, 30.0), None, "the corner is still where the drag began");
    }
}
