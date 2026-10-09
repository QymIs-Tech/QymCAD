//! WHAT A SKETCH TAKES FROM THE BODY UNDER IT, and what it draws that is not part of the shape: an edge or the whole
//! outline of a face brought in as driven geometry, and construction lines that stay outside the profile.
use qymcad::{Key, PointerButton, Session};
use qymcad_acceptance::build;
use qymcad_acceptance::probe;

/// Take the tool whose hint is `hint`.
fn take(s: &mut Session, hint: &str) {
    let hint = s.word(hint);
    s.press_hint(&hint);
}

/// Press the word of the bar of options `key` names.
fn bar_word(s: &mut Session, key: &str) {
    let word = s.word(key);
    s.press_word_near(&word, qymcad::pos2(640.0, 24.0));
}

/// How many lines the sketch holds, and how far it reaches.
fn lines_and_box(s: &mut Session) -> (usize, [f64; 2], [f64; 2]) {
    let sk = s.document().sketches.last().cloned().unwrap_or_else(|| panic!("the document holds no sketch"));
    (sk.lines, sk.min, sk.max)
}

/// The 40 x 30 x 10 block of the first part with a sketch open on its top face.
fn a_sketch_on_the_top_of_a_block() -> Session {
    let mut s = Session::start();
    build::block(&mut s);
    build::sketch_on_face(&mut s, [20.0, 15.0, 10.0]);
    s
}

probe! {
    /// AN EDGE OF THE BODY IS BROUGHT INTO THE SKETCH by a click, and the program says it is driven.
    fn an_edge_of_the_body_is_projected_into_the_sketch() {
        let mut s = a_sketch_on_the_top_of_a_block();
        assert!(lines_and_box(&mut s).0 == 0, "the new sketch is not empty: {:?}", lines_and_box(&mut s));
        take(&mut s, "tb-project-body-hint");
        s.click_on_sketch(20.0, 0.0); // the front edge of the top face, under the sheet
        assert!(s.status() == s.word("sk-projected-hint"), "nothing says the geometry is driven: the status line says {:?}", s.status());
        let (lines, min, max) = lines_and_box(&mut s);
        assert!(lines == 1, "the edge did not come into the sketch: it holds {lines} line(s)");
        let (w, h) = (max[0] - min[0], max[1] - min[1]);
        assert!((w.max(h) - 40.0).abs() < 1e-3 && w.min(h) < 1e-3, "what came in is not the 40 long edge of the block: it is {w} by {h}");
    }
}

probe! {
    /// A PROJECTED EDGE IS DRIVEN: it does not follow the mouse, as the program says.
    fn a_projected_edge_does_not_follow_the_mouse() {
        let mut s = a_sketch_on_the_top_of_a_block();
        take(&mut s, "tb-project-body-hint");
        s.click_on_sketch(20.0, 0.0); // the front edge of the top face, under the sheet
        let before = s.document().sketches.last().cloned().expect("the sketch").places;
        let arrow = s.word("tb-select-hint");
        s.press_hint(&arrow);
        let (from, to) = (before[0], [before[0][0] + 5.0, before[0][1] + 5.0]);
        s.drag_on_sketch((from[0], from[1]), (to[0], to[1]));
        let after = s.document().sketches.last().cloned().expect("the sketch").places;
        assert!(after == before, "the projected edge followed the mouse: {before:?} became {after:?}");
    }
}

probe! {
    /// THE WHOLE OUTLINE OF THE FACE UNDER THE SKETCH comes in at once when the bar is set to it.
    fn the_outline_of_the_face_under_the_sketch_is_projected() {
        let mut s = a_sketch_on_the_top_of_a_block();
        take(&mut s, "tb-project-body-hint");
        bar_word(&mut s, "opt-face-outline");
        s.click_on_sketch(20.0, 15.0);
        let (lines, min, max) = lines_and_box(&mut s);
        assert!(lines == 4, "the outline of the top face is four lines, and {lines} came in");
        let (w, h) = (max[0] - min[0], max[1] - min[1]);
        assert!((w.max(h) - 40.0).abs() < 1e-3 && (w.min(h) - 30.0).abs() < 1e-3, "the outline is not the 40 by 30 top of the block: it is {w} by {h}");
    }
}

probe! {
    /// THE OUTLINE OF A FACE IS REFUSED IN WORDS for a sketch that sits on a plane and not on a face, and nothing is
    /// drawn.
    fn the_outline_of_a_face_is_refused_for_a_sketch_on_a_plane() {
        let mut s = Session::start();
        build::into_the_first_part(&mut s);
        let xy = s.word("plane-xy-table");
        s.press_word(&xy);
        take(&mut s, "tb-project-body-hint");
        bar_word(&mut s, "opt-face-outline");
        s.click_on_sketch(10.0, 10.0);
        assert!(s.status() == s.word("sk-face-outline-only"), "nothing says the outline needs a sketch on a face: the status line says {:?}", s.status());
        assert!(lines_and_box(&mut s).0 == 0, "something was drawn anyway: {:?}", lines_and_box(&mut s));
    }
}

probe! {
    /// A PROJECTED EDGE FOLLOWS THE PART: the block is made wider by its own sketch, and the line that was taken from
    /// its edge grows with it.
    fn a_projected_edge_follows_the_part() {
        let mut s = a_sketch_on_the_top_of_a_block();
        take(&mut s, "tb-project-body-hint");
        s.click_on_sketch(20.0, 0.0); // the front edge of the top face, under the sheet
        let finish = s.word("wb-finish");
        s.press_word(&finish);
        // the rectangle the block was made of, made 60 wide by dragging its corner
        let first = s.document().sketches[0].name.clone();
        let row = s.find(&first, qymcad::pos2(0.0, 300.0)).unwrap_or_else(|| panic!("the first sketch is not in the tree; on screen: {:?}", s.words()));
        s.double_click(row.center());
        s.drag_on_sketch((40.0, 30.0), (60.0, 30.0));
        let finish = s.word("wb-finish");
        s.press_word(&finish);
        let (lines, min, max) = lines_and_box(&mut s);
        let (w, h) = (max[0] - min[0], max[1] - min[1]);
        assert!(lines == 1, "the projected line is gone: the sketch holds {lines} line(s)");
        assert!((w.max(h) - 60.0).abs() < 1e-3, "the projected line did not follow the part from 40 to 60: it is {w} by {h}");
    }
}

probe! {
    /// CONSTRUCTION GEOMETRY STAYS OUT OF THE PROFILE: a rectangle drawn as construction is in the sketch and makes no
    /// body.
    fn construction_geometry_makes_no_body() {
        let mut s = build::empty_sketch();
        let switch = s.word("opt-construction-short");
        s.toggle(&switch);
        build::draw(&mut s, "tb-rect-hint", &[(0.0, 0.0), (40.0, 30.0)]);
        assert!(lines_and_box(&mut s).0 == 4, "the rectangle was not drawn: {:?}", lines_and_box(&mut s));
        let finish = s.word("wb-finish");
        s.press_word(&finish);
        let extrude = s.word("tb-extrude-hint");
        s.press_hint(&extrude);
        s.key(Key::Enter);
        assert!(s.document().bodies.is_empty(), "construction geometry was extruded into a body: {:?}", s.document().bodies);
    }
}

probe! {
    /// GEOMETRY ALREADY DRAWN IS TURNED INTO CONSTRUCTION AND BACK, and the profile goes and comes back with it.
    fn geometry_is_turned_into_construction_and_back() {
        let mut s = build::empty_sketch();
        build::draw(&mut s, "tb-rect-hint", &[(0.0, 0.0), (40.0, 30.0)]);
        for (x, y) in [(20.0, 0.0), (40.0, 15.0), (20.0, 30.0), (0.0, 15.0)] {
            build::pick(&mut s, x, y, (x, y) != (20.0, 0.0));
        }
        let at = s.on_sketch(20.0, 0.0);
        s.click_with(at, PointerButton::Secondary, qymcad::Modifiers::default());
        let toggle = s.word("sk-construction-toggle");
        s.press_word_near(&toggle, at);
        let finish = s.word("wb-finish");
        s.press_word(&finish);
        let extrude = s.word("tb-extrude-hint");
        s.press_hint(&extrude);
        s.key(Key::Enter);
        assert!(s.document().bodies.is_empty(), "the rectangle turned into construction still made a body: {:?}", s.document().bodies);
    }
}

/// A DIMENSION BETWEEN TWO PARALLEL LINES laid by hand with the dimension tool: a click on each line, a click to put it
/// down between them, the value typed, Enter, the tool put down.
fn a_distance_between(s: &mut Session, one: (f64, f64), other: (f64, f64), value: &str) {
    let (hint, field) = (s.word("tb-dim-hint"), s.word("sk-expr-example"));
    s.press_hint(&hint);
    s.click_on_sketch(one.0, one.1);
    s.click_on_sketch(other.0, other.1);
    // the dimension put down between the two, a little along them
    s.click_on_sketch((one.0 + other.0) / 2.0 + 3.0, (one.1 + other.1) / 2.0 + 3.0);
    s.fill_hinted(&field, value).key(Key::Enter);
    s.key(Key::Escape);
    let arrow = s.word("tb-select-hint");
    s.press_hint(&arrow);
}

/// Whether the sketch shows a point of its own at (`x`, `y`).
fn a_point_at(sk: &qymcad::SketchInfo, x: f64, y: f64) -> bool {
    sk.places.iter().any(|p| (p[0] - x).abs() < 1e-3 && (p[1] - y).abs() < 1e-3)
}

/// HOW THE OUTLINE OF THE TOP COMES INTO THE SKETCH.
#[derive(Clone, Copy, Debug)]
enum Taken {
    /// the projection tool with "Face outline"
    FaceOutline,
    /// the projection tool, a click on each of the four edges
    EdgeByEdge,
}

/// THE CHAIN OF THE REPORT, by hand: a block 40 x 30 whose sides are the parameters w and h; on its top face the outline
/// projected, made construction, a rectangle inside it held 5 off each side by the dimension tool, extruded 10 on.
fn the_chain_of_the_report() -> Session {
    the_chain_taken(Taken::FaceOutline)
}

/// A block 40 x 30 x 10 whose sides are the parameters w and h, a sketch open on its top.
fn a_block_of_parameters_and_a_sketch_on_its_top() -> Session {
    let mut s = Session::start();
    build::into_the_first_part(&mut s);
    build::parameter(&mut s, "w", "40");
    build::parameter(&mut s, "h", "30");
    build::rectangle_on_xy(&mut s);
    s.key(Key::Escape);
    // the lower side given w, the right side given h, by the dimension tool
    for (on, off, value) in [((20.0, 0.0), (20.0, -10.0), "w"), ((40.0, 15.0), (50.0, 15.0), "h")] {
        let (hint, field) = (s.word("tb-dim-hint"), s.word("sk-expr-example"));
        s.press_hint(&hint);
        s.click_on_sketch(on.0, on.1);
        s.click_on_sketch(off.0, off.1);
        s.fill_hinted(&field, value).key(Key::Enter);
        s.key(Key::Escape);
    }
    let finish = s.word("wb-finish");
    s.press_word(&finish);
    take(&mut s, "tb-extrude-hint");
    s.key(Key::Enter);
    s.key(Key::Escape);
    build::sketch_on_face(&mut s, [20.0, 15.0, 10.0]);
    s
}

/// The chain of the report with the outline of the top taken `taken`.
fn the_chain_taken(taken: Taken) -> Session {
    let mut s = a_block_of_parameters_and_a_sketch_on_its_top();
    // on the top: the outline of the face projected and made construction
    take(&mut s, "tb-project-body-hint");
    match taken {
        Taken::FaceOutline => {
            bar_word(&mut s, "opt-face-outline");
            s.click_on_sketch(20.0, 15.0);
        }
        Taken::EdgeByEdge => {
            for (x, y) in [(20.0, 0.0), (40.0, 15.0), (20.0, 30.0), (0.0, 15.0)] {
                s.click_on_sketch(x, y);
            }
        }
    }
    s.key(Key::Escape);
    let arrow = s.word("tb-select-hint");
    s.press_hint(&arrow);
    for (k, (x, y)) in [(20.0, 0.0), (40.0, 15.0), (20.0, 30.0), (0.0, 15.0)].into_iter().enumerate() {
        let at = s.on_sketch(x, y);
        if k == 0 {
            s.click(at);
        } else {
            s.click_with(at, PointerButton::Primary, qymcad::Modifiers::SHIFT);
        }
    }
    s.key(Key::X);
    s.key(Key::Escape);
    // a rectangle inside, each side held 5 off the side of the outline beside it
    build::draw(&mut s, "tb-rect-hint", &[(6.0, 6.0), (34.0, 24.0)]);
    a_distance_between(&mut s, (20.0, 0.0), (20.0, 6.0), "5");
    a_distance_between(&mut s, (40.0, 15.0), (34.0, 15.0), "5");
    a_distance_between(&mut s, (20.0, 30.0), (20.0, 24.0), "5");
    a_distance_between(&mut s, (0.0, 15.0), (6.0, 15.0), "5");
    let finish = s.word("wb-finish");
    s.press_word(&finish);
    take(&mut s, "tb-extrude-hint");
    s.key(Key::Enter);
    s.key(Key::Escape);
    s
}

/// What is wrong with the sketch on the top, against an outline `w` x `h` with the rectangle 5 inside it: the points
/// that stand nowhere.
fn tied_to(s: &mut Session, w: f64, h: f64) -> Option<String> {
    let sk = s.document().sketches.last().cloned().expect("the sketch on the top");
    let want = [(0.0, 0.0), (w, 0.0), (w, h), (0.0, h), (5.0, 5.0), (w - 5.0, 5.0), (w - 5.0, h - 5.0), (5.0, h - 5.0)];
    let missing: Vec<(f64, f64)> = want.into_iter().filter(|&(x, y)| !a_point_at(&sk, x, y)).collect();
    (!missing.is_empty()).then(|| format!("for {w} x {h} nothing stands at {missing:?}; the sketch shows {:?}, {} redundant", sk.places, sk.redundant))
}

/// A dimension of the first sketch, the one whose label reads `label`, given `value` by a double click on it, the
/// sketch opened from the tree and finished again.
fn the_first_sketch_given(s: &mut Session, sizes: &[(&str, &str)]) {
    let first = s.document().sketches[0].name.clone();
    let row = s.find(&first, qymcad::pos2(0.0, 300.0)).unwrap_or_else(|| panic!("the first sketch is not in the tree; on screen: {:?}", s.words()));
    s.double_click(row.center());
    for (label, value) in sizes {
        let at = s.find(label, qymcad::pos2(640.0, 400.0)).unwrap_or_else(|| panic!("no dimension {label:?} on the sheet; on screen: {:?}", s.words()));
        s.double_click(at.center());
        let field = s.word("sk-expr-example");
        s.fill_hinted(&field, value).key(Key::Enter);
    }
    let finish = s.word("wb-finish");
    s.press_word(&finish);
}

probe! {
    /// WHAT IS TIED TO A PROJECTION BY DIMENSIONS FOLLOWS THE BODY, on the chain of the report, whatever way the body
    /// above it is changed: the parameters in their table; the dimensions of the first sketch by a double click; the
    /// parameters changed and the change undone and done again; the document saved, opened and then changed. The outline
    /// of the projection stands on the new top, and the rectangle 5 inside it.
    ///
    /// Reported behaviour: "tie to the projection by dimensions, change the body above in the tree: does not work at all".
    fn what_is_tied_to_a_projection_by_dimensions_follows_the_body_changed_every_way() {
        let mut failures = Vec::new();
        // the parameters in their table
        let mut s = the_chain_of_the_report();
        if let Some(e) = tied_to(&mut s, 40.0, 30.0) {
            failures.push(format!("GUARD, as built: {e}"));
        }
        build::set_parameter(&mut s, "w", "30");
        build::set_parameter(&mut s, "h", "20");
        failures.extend(tied_to(&mut s, 30.0, 20.0).map(|e| format!("the parameters changed: {e}")));
        // undone, and done again
        s.chord(qymcad::Modifiers::COMMAND, Key::Z);
        s.chord(qymcad::Modifiers::COMMAND, Key::Z);
        failures.extend(tied_to(&mut s, 40.0, 30.0).map(|e| format!("the change undone: {e}")));
        s.chord(qymcad::Modifiers::COMMAND | qymcad::Modifiers::SHIFT, Key::Z);
        s.chord(qymcad::Modifiers::COMMAND | qymcad::Modifiers::SHIFT, Key::Z);
        failures.extend(tied_to(&mut s, 30.0, 20.0).map(|e| format!("the change done again: {e}")));

        // the dimensions of the first sketch by a double click
        let mut s = the_chain_of_the_report();
        the_first_sketch_given(&mut s, &[("40.0", "30"), ("30.0", "20")]);
        failures.extend(tied_to(&mut s, 30.0, 20.0).map(|e| format!("the first sketch given 30 and 20: {e}")));

        // saved, opened, then changed
        let mut s = the_chain_of_the_report();
        let path = std::env::temp_dir().join(format!("projection-tied-{}.qcad", std::process::id()));
        let path = path.to_string_lossy().to_string();
        build::save_as(&mut s, &path);
        build::open_project(&mut s, &path);
        build::set_parameter(&mut s, "w", "30");
        build::set_parameter(&mut s, "h", "20");
        failures.extend(tied_to(&mut s, 30.0, 20.0).map(|e| format!("saved, opened, then changed: {e}")));
        let _ = std::fs::remove_file(&path);

        assert!(failures.is_empty(), "what is tied to a projection, the body changed:\n{}", failures.join("\n"));
    }
}

probe! {
    /// WHAT IS TIED TO AN OUTLINE TAKEN EDGE BY EDGE FOLLOWS THE BODY as when it is taken whole: the parameters changed,
    /// undone and done again.
    fn what_is_tied_to_an_outline_taken_edge_by_edge_follows_the_parameters() {
        let mut failures = Vec::new();
        let mut s = the_chain_taken(Taken::EdgeByEdge);
        failures.extend(tied_to(&mut s, 40.0, 30.0).map(|e| format!("GUARD, as built: {e}")));
        build::set_parameter(&mut s, "w", "30");
        build::set_parameter(&mut s, "h", "20");
        failures.extend(tied_to(&mut s, 30.0, 20.0).map(|e| format!("the parameters changed: {e}")));
        s.chord(qymcad::Modifiers::COMMAND, Key::Z);
        s.chord(qymcad::Modifiers::COMMAND, Key::Z);
        failures.extend(tied_to(&mut s, 40.0, 30.0).map(|e| format!("the change undone: {e}")));
        assert!(failures.is_empty(), "an outline taken edge by edge:\n{}", failures.join("\n"));
    }
}

probe! {
    /// A LINE DRAWN FROM A GREY CORNER OF THE FACE, with nothing projected, follows the body: the edge it starts on is
    /// taken into the sketch, and the parameters changed carry the line's start with the corner.
    ///
    /// Reported behaviour: "even if you do not project, but tie to the edges of the face (the thin grey ones), at a
    /// rebuild everything falls apart".
    fn a_line_from_a_grey_corner_follows_the_parameters() {
        let mut s = a_block_of_parameters_and_a_sketch_on_its_top();
        build::draw(&mut s, "tb-line-hint", &[(40.0, 30.0), (20.0, 20.0)]);
        let finish = s.word("wb-finish");
        s.press_word(&finish);
        build::set_parameter(&mut s, "w", "30");
        build::set_parameter(&mut s, "h", "20");
        let sk = s.document().sketches.last().cloned().expect("the sketch on the top");
        assert!(a_point_at(&sk, 30.0, 20.0), "w 30, h 20: the line drawn from the corner (40, 30) does not start at the corner (30, 20); the sketch shows {:?}", sk.places);
    }
}
