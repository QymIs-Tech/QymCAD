//! WHAT A SKETCH TAKES FROM THE BODY UNDER IT, and what it draws that is not part of the shape: an edge or the whole
//! outline of a face brought in as driven geometry, and construction lines that stay outside the profile.
use qymcad::{Key, PointerButton, Session};
use qymcad_acceptance::build;
use qymcad_acceptance::build::pick;
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

/// WHOSE SKETCH STANDS ON THE TOP OF THE BLOCK.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Whose {
    /// a sketch of the part of the block
    Own,
    /// a sketch of a second part of the assembly, put on the face of the first
    Neighbour,
}

/// A block 40 x 30 x 10 whose sides are the parameters w and h, a sketch open on its top.
fn a_block_of_parameters_and_a_sketch_on_its_top() -> Session {
    a_block_of_parameters_and_a_sketch_of(Whose::Own)
}

/// A block 40 x 30 x 10 whose sides are the parameters w and h, a sketch of `whose` open on its top.
fn a_block_of_parameters_and_a_sketch_of(whose: Whose) -> Session {
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
    if whose == Whose::Neighbour {
        // a second part, and the switch that lets it see and take the faces of its neighbours turned on
        build::into_a_new_part(&mut s);
        let context = s.word("wb-in-context");
        let switch = s.widgets().into_iter().find(|w| w.label.ends_with(&context)).unwrap_or_else(|| panic!("there is no switch for working in context"));
        if switch.checked == Some(false) {
            s.click(switch.rect.center());
        }
    }
    build::sketch_on_face(&mut s, [20.0, 15.0, 10.0]);
    s
}

/// The chain of the report with the outline of the top taken `taken`.
fn the_chain_taken(taken: Taken) -> Session {
    the_chain_of(taken, Whose::Own)
}

/// The chain of the report with the outline of the top taken `taken` into a sketch of `whose`.
fn the_chain_of(taken: Taken, whose: Whose) -> Session {
    let mut s = a_block_of_parameters_and_a_sketch_of(whose);
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

/// The block of parameters with the outline of its top projected whole and made construction, the sketch left open.
fn an_outline_on_the_top() -> Session {
    let mut s = a_block_of_parameters_and_a_sketch_on_its_top();
    take(&mut s, "tb-project-body-hint");
    bar_word(&mut s, "opt-face-outline");
    s.click_on_sketch(20.0, 15.0);
    s.key(Key::Escape);
    let arrow = s.word("tb-select-hint");
    s.press_hint(&arrow);
    s
}

/// Press the constraint whose hint is `hint` on what is picked, and put the selection down.
fn constrain(s: &mut Session, hint: &str) {
    let hint = s.word(hint);
    s.press_hint(&hint);
    s.key(Key::Escape);
}

probe! {
    /// WHAT IS TIED TO A PROJECTION BY THE CONSTRAINT BUTTONS FOLLOWS THE BODY: a line drawn on the top, one end made
    /// coincident with a corner of the outline, the other end put on its far side; a second line put parallel to the
    /// lower side, its start on the right side. The parameters changed, undone; then the sketch finished and extruded,
    /// and changed again.
    fn what_is_tied_to_a_projection_by_constraints_follows_the_parameters() {
        let mut failures = Vec::new();
        let mut s = an_outline_on_the_top();
        build::line(&mut s, (8.0, 8.0), (25.0, 14.0));
        build::line(&mut s, (12.0, 20.0), (32.0, 23.0));
        // the end (8, 8) on the corner (0, 0); the end (25, 14) on the upper side
        pick(&mut s, 8.0, 8.0, false);
        pick(&mut s, 0.0, 0.0, true);
        constrain(&mut s, "con-coincident-hint");
        pick(&mut s, 25.0, 14.0, false);
        pick(&mut s, 30.0, 30.0, true);
        constrain(&mut s, "con-coincident-hint");
        // the second line parallel to the lower side, its end (32, 23) on the right side
        pick(&mut s, 22.0, 21.5, false);
        pick(&mut s, 10.0, 0.0, true);
        constrain(&mut s, "con-parallel-hint");
        let sk = s.document().sketches.last().cloned().expect("the sketch");
        let end = sk.places.iter().find(|p| (p[0] - 32.0).abs() < 1.0 && (p[1] - 22.0).abs() < 1.5).copied();
        let Some(end) = end else { panic!("GUARD: the end of the second line is lost after Parallel: {:?}", sk.places) };
        pick(&mut s, end[0], end[1], false);
        pick(&mut s, 40.0, 5.0, true);
        constrain(&mut s, "con-coincident-hint");

        // what a person sees: (0, 0) held, an end on the upper side, an end on the right side and the second line level
        let check = |s: &mut Session, w: f64, h: f64, when: &str| -> Option<String> {
            let sk = s.document().sketches.last().cloned().expect("the sketch");
            let on_top = sk.places.iter().any(|p| (p[1] - h).abs() < 1e-3 && p[0] > 1.0 && p[0] < w - 1.0);
            // a point on a line stands on the line drawn on, past its ends too, as in every CAD
            let on_right = sk.places.iter().filter(|p| (p[0] - w).abs() < 1e-3 && p[1].abs() > 1e-3 && (p[1] - h).abs() > 1e-3).copied().collect::<Vec<_>>();
            let level = on_right.iter().any(|r| sk.places.iter().any(|p| (p[1] - r[1]).abs() < 1e-3 && p[0] < w - 1.0 && p[0] > 1.0));
            let ok = a_point_at(&sk, 0.0, 0.0) && a_point_at(&sk, w, h) && on_top && !on_right.is_empty() && level && sk.redundant == 0;
            (!ok).then(|| format!("{when}, {w} x {h}: corner {} far corner {} end on top {on_top} end on right {on_right:?} level {level}, {} redundant; the sketch shows {:?}", a_point_at(&sk, 0.0, 0.0), a_point_at(&sk, w, h), sk.redundant, sk.places))
        };
        failures.extend(check(&mut s, 40.0, 30.0, "GUARD, as tied"));
        let finish = s.word("wb-finish");
        s.press_word(&finish);
        build::set_parameter(&mut s, "w", "30");
        build::set_parameter(&mut s, "h", "20");
        failures.extend(check(&mut s, 30.0, 20.0, "the parameters changed"));
        s.chord(qymcad::Modifiers::COMMAND, Key::Z);
        s.chord(qymcad::Modifiers::COMMAND, Key::Z);
        failures.extend(check(&mut s, 40.0, 30.0, "the change undone"));
        build::set_parameter(&mut s, "w", "50");
        failures.extend(check(&mut s, 50.0, 30.0, "w made 50"));
        assert!(failures.is_empty(), "what is tied to a projection by constraints:\n{}", failures.join("\n"));
    }
}

probe! {
    /// A PROJECTION MADE ORDINARY GEOMETRY LETS GO OF THE BODY: the outline of the top with a rectangle held 5 inside it,
    /// the four sides picked and made ordinary from the menu of the right button; the parameters changed - the outline
    /// stays 40 x 30 and the rectangle inside it, nothing red. The same done and undone: the outline is a projection
    /// again and follows the parameters.
    fn a_projection_made_ordinary_stays_where_it_was() {
        let mut failures = Vec::new();
        for undone in [false, true] {
            let mut s = an_outline_on_the_top();
            build::draw(&mut s, "tb-rect-hint", &[(6.0, 6.0), (34.0, 24.0)]);
            a_distance_between(&mut s, (20.0, 0.0), (20.0, 6.0), "5");
            a_distance_between(&mut s, (40.0, 15.0), (34.0, 15.0), "5");
            a_distance_between(&mut s, (20.0, 30.0), (20.0, 24.0), "5");
            a_distance_between(&mut s, (0.0, 15.0), (6.0, 15.0), "5");
            for (k, (x, y)) in [(20.0, 0.0), (40.0, 15.0), (20.0, 30.0), (0.0, 15.0)].into_iter().enumerate() {
                pick(&mut s, x, y, k > 0);
            }
            let at = s.on_sketch(0.0, 15.0);
            s.click_with(at, PointerButton::Secondary, qymcad::Modifiers::default());
            let word = s.word("sk-make-ordinary");
            s.press_word(&word);
            if undone {
                s.chord(qymcad::Modifiers::COMMAND, Key::Z);
            }
            let finish = s.word("wb-finish");
            s.press_word(&finish);
            build::set_parameter(&mut s, "w", "30");
            build::set_parameter(&mut s, "h", "20");
            let (w, h, when) = if undone { (30.0, 20.0, "made ordinary and undone") } else { (40.0, 30.0, "made ordinary") };
            failures.extend(tied_to(&mut s, w, h).map(|e| format!("{when}, the parameters changed: {e}")));
            let red: Vec<String> = s.document().features.iter().filter_map(|f| f.error.clone().map(|e| format!("{}: {e}", f.name))).collect();
            if !red.is_empty() {
                failures.push(format!("{when}: red in the tree: {red:?}"));
            }
        }
        assert!(failures.is_empty(), "a projection made ordinary:\n{}", failures.join("\n"));
    }
}

probe! {
    /// THE HEIGHT OF THE BLOCK CHANGED UNDER THE SKETCH ON ITS TOP: the chain of the report, the first extrusion reopened
    /// by a double click in the tree and made 25. The sketch rides up with the face, the outline and the rectangle 5
    /// inside it stay; the body on top stands on the new top, 25 to 35.
    fn the_height_of_the_block_changed_under_the_projection() {
        let mut s = the_chain_of_the_report();
        let lead = s.word("cmd-extrude");
        let left = s.canvas().min.x;
        let first = s.words_at().into_iter().filter(|(w, r)| r.max.x < left && w.starts_with(&lead)).map(|(_, r)| r).min_by(|a, b| a.min.y.total_cmp(&b.min.y));
        let first = first.unwrap_or_else(|| panic!("no extrusion in the tree; on screen: {:?}", s.words()));
        s.double_click(first.center());
        let caption = s.word("f-length");
        s.fill(&caption, "25");
        s.key(Key::Enter);
        let mut failures = Vec::new();
        failures.extend(tied_to(&mut s, 40.0, 30.0).map(|e| format!("the block made 25: {e}")));
        let doc = s.document();
        // the block joined by the body on top is the last body; the block alone stays listed under it, consumed
        let volume = doc.bodies.last().map(|b| b.volume).unwrap_or(0.0);
        let top = doc.bodies.iter().flat_map(|b| b.face_centres.iter().map(|c| c[2])).fold(f64::MIN, f64::max);
        if (volume - (40.0 * 30.0 * 25.0 + 30.0 * 20.0 * 10.0)).abs() > 1.0 || (top - 35.0).abs() > 1e-3 {
            failures.push(format!("the block made 25: the bodies hold {volume:.1} mm^3, their top at {top:.3}; want 36000 and 35"));
        }
        let red: Vec<String> = doc.features.iter().filter_map(|f| f.error.clone().map(|e| format!("{}: {e}", f.name))).collect();
        if !red.is_empty() {
            failures.push(format!("the block made 25: red in the tree: {red:?}"));
        }
        assert!(failures.is_empty(), "the height under the projection:\n{}", failures.join("\n"));
    }
}

/// Press what stands under `key` in the menu of the right button on the first row of the tree whose words begin as
/// the catalogue line `row` does.
fn from_the_menu_of(s: &mut Session, row: &str, key: &str) {
    let lead = s.word(row);
    let left = s.canvas().min.x;
    let at = s.words_at().into_iter().filter(|(w, r)| r.max.x < left && w.starts_with(&lead)).map(|(_, r)| r).min_by(|a, b| a.min.y.total_cmp(&b.min.y));
    let at = at.unwrap_or_else(|| panic!("no row {lead:?} in the tree; on screen: {:?}", s.words()));
    s.click_with(at.center(), PointerButton::Secondary, qymcad::Modifiers::default());
    let word = s.word(key);
    s.press_word_near(&word, at.center());
}

probe! {
    /// A FILLET PUT ABOVE THE SKETCH ON THE TOP: the chain of the report rolled back to the block, the corner edge (40,
    /// 0) of the block rounded 3, the rollback cleared. The top face has five edges now; the projection takes them, the
    /// sides the rectangle is held to are still its sides, and the rectangle 5 inside stands where it stood.
    fn a_fillet_put_above_the_projection_keeps_what_is_tied_to_it() {
        let mut s = the_chain_of_the_report();
        from_the_menu_of(&mut s, "cmd-extrude", "act-rollback-here");
        let hint = s.word("tb-fillet-body-hint");
        s.press_hint(&hint);
        let edge = s.edge_at([40.0, 0.0, 5.0]);
        s.click(edge);
        let radius = s.word("f-radius");
        s.fill(&radius, "3").key(Key::Enter);
        s.key(Key::Escape);
        from_the_menu_of(&mut s, "cmd-extrude", "act-clear-rollback");

        let mut failures = Vec::new();
        let sk = s.document().sketches.last().cloned().expect("the sketch on the top");
        // the corner rounded is an arc of the sketch around (37, 3), its ends where the sides end
        let missing: Vec<(f64, f64)> = [(0.0, 0.0), (0.0, 30.0), (40.0, 30.0), (37.0, 0.0), (40.0, 3.0), (37.0, 3.0), (5.0, 5.0), (35.0, 5.0), (35.0, 25.0), (5.0, 25.0)].into_iter().filter(|&(x, y)| !a_point_at(&sk, x, y)).collect();
        if !missing.is_empty() || sk.redundant != 0 || sk.places.len() != 11 {
            failures.push(format!("nothing stands at {missing:?}, {} redundant; the sketch shows {} points {:?}", sk.redundant, sk.places.len(), sk.places));
        }
        // the outline was made construction: the arc the fillet brought into it is construction as its sides are
        if sk.arcs != 1 || sk.construction != 5 {
            failures.push(format!("the outline made construction, then rounded: {} arcs, {} construction curves; want 1 arc and the 4 sides with it", sk.arcs, sk.construction));
        }
        let doc = s.document();
        let red: Vec<String> = doc.features.iter().filter_map(|f| f.error.clone().map(|e| format!("{}: {e}", f.name))).collect();
        if !red.is_empty() {
            failures.push(format!("red in the tree: {red:?}"));
        }
        let want = 40.0 * 30.0 * 10.0 - (9.0 - std::f64::consts::PI * 9.0 / 4.0) * 10.0 + 30.0 * 20.0 * 10.0;
        let volume = doc.bodies.iter().rfind(|b| !b.consumed).map(|b| b.volume).unwrap_or(0.0);
        if (volume - want).abs() > 1.0 {
            failures.push(format!("the part holds {volume:.1} mm^3, want {want:.1}: {:?}", doc.bodies.iter().map(|b| (b.name.clone(), b.volume, b.consumed)).collect::<Vec<_>>()));
        }
        assert!(failures.is_empty(), "a fillet above the projection:\n{}", failures.join("\n"));
    }
}

probe! {
    /// A POINT PUT ON A SIDE OF THE PROJECTION STAYS ON THAT SIDE WHEN A FILLET ABOVE PARTS ITS CORNER: on the top, a line
    /// level with the lower side, its end put on the right side by Coincident; the corner (40, 0) of the block rounded 3
    /// above the sketch. The right side runs from (40, 3) now, and the end stays on it, at X 40.
    fn a_point_on_a_side_stays_on_it_when_a_fillet_parts_the_corner() {
        let mut s = an_outline_on_the_top();
        build::line(&mut s, (12.0, 20.0), (32.0, 20.0));
        pick(&mut s, 22.0, 20.0, false);
        pick(&mut s, 10.0, 0.0, true);
        constrain(&mut s, "con-parallel-hint");
        pick(&mut s, 32.0, 20.0, false);
        pick(&mut s, 40.0, 10.0, true);
        constrain(&mut s, "con-coincident-hint");
        let finish = s.word("wb-finish");
        s.press_word(&finish);
        let before = s.document().sketches.last().cloned().expect("the sketch");
        assert!(a_point_at(&before, 40.0, 20.0), "GUARD: the end put on the right side: {:?}", before.places);
        from_the_menu_of(&mut s, "cmd-extrude", "act-rollback-here");
        let hint = s.word("tb-fillet-body-hint");
        s.press_hint(&hint);
        let edge = s.edge_at([40.0, 0.0, 5.0]);
        s.click(edge);
        let radius = s.word("f-radius");
        s.fill(&radius, "3").key(Key::Enter);
        s.key(Key::Escape);
        from_the_menu_of(&mut s, "cmd-extrude", "act-clear-rollback");
        let sk = s.document().sketches.last().cloned().expect("the sketch");
        assert!(a_point_at(&sk, 37.0, 3.0) && a_point_at(&sk, 40.0, 20.0) && a_point_at(&sk, 12.0, 20.0) && sk.redundant == 0, "the corner rounded above: the end put on the right side left it, or the fillet is not in the sketch; {} redundant, the sketch shows {:?}", sk.redundant, sk.places);
    }
}

probe! {
    /// THE BLOCK UNDER THE SKETCH DELETED: the chain of the report, the first extrusion deleted alone from the menu of
    /// its row. The sketch on its top goes red with the reason, and what was drawn there stays as it stood - the outline
    /// let go as ordinary geometry, the rectangle 5 inside it. Undone, the projection is back and follows the
    /// parameters.
    fn the_block_under_the_projection_deleted() {
        let mut s = the_chain_of_the_report();
        from_the_menu_of(&mut s, "cmd-extrude", "act-delete-feature");
        let yes = s.word("confirm-yes");
        s.press_word(&yes);
        let mut failures = Vec::new();
        failures.extend(tied_to(&mut s, 40.0, 30.0).map(|e| format!("the block deleted: {e}")));
        let doc = s.document();
        let name = doc.sketches.last().map(|k| k.name.clone()).unwrap_or_default();
        if !doc.features.iter().any(|f| f.name == name && f.error.as_deref().is_some_and(|e| !e.is_empty())) {
            failures.push(format!("the block deleted: the sketch {name:?} it stood on is not red with the reason: {:?}", doc.features.iter().map(|f| (f.name.clone(), f.error.clone())).collect::<Vec<_>>()));
        }
        s.chord(qymcad::Modifiers::COMMAND, Key::Z);
        failures.extend(tied_to(&mut s, 40.0, 30.0).map(|e| format!("the deletion undone: {e}")));
        build::set_parameter(&mut s, "w", "30");
        build::set_parameter(&mut s, "h", "20");
        failures.extend(tied_to(&mut s, 30.0, 20.0).map(|e| format!("the deletion undone, then the parameters changed: {e}")));
        assert!(failures.is_empty(), "the block under the projection deleted:\n{}", failures.join("\n"));
    }
}

probe! {
    /// A SIDE OF THE OUTLINE CUT, THE OTHER SIDES STILL FOLLOW: the outline of the top projected; its upper side picked
    /// and deleted with Delete, or the left half of its lower side trimmed off at a line crossing it. The parameters
    /// changed: the sides not cut stand on the new top, as a projected curve in every CAD is tied to its own edge.
    fn a_side_of_the_outline_cut_and_the_others_still_follow() {
        let mut failures = Vec::new();
        let corners = |s: &mut Session, extra: &[(f64, f64)], when: &str| -> Option<String> {
            let sk = s.document().sketches.last().cloned().expect("the sketch");
            let missing: Vec<(f64, f64)> = [(0.0, 0.0), (30.0, 0.0), (30.0, 20.0), (0.0, 20.0)].iter().chain(extra).copied().filter(|&(x, y)| !a_point_at(&sk, x, y)).collect();
            (!missing.is_empty() || sk.redundant != 0).then(|| format!("{when}: nothing stands at {missing:?}, {} redundant; the sketch shows {:?}", sk.redundant, sk.places))
        };

        let mut s = an_outline_on_the_top();
        pick(&mut s, 20.0, 30.0, false);
        s.key(Key::Delete);
        let finish = s.word("wb-finish");
        s.press_word(&finish);
        build::set_parameter(&mut s, "w", "30");
        build::set_parameter(&mut s, "h", "20");
        failures.extend(corners(&mut s, &[], "the upper side deleted"));

        let mut s = an_outline_on_the_top();
        build::line(&mut s, (20.0, -5.0), (20.0, 5.0));
        let trim = s.word("tb-trim-hint");
        s.press_hint(&trim);
        s.click_on_sketch(10.0, 0.0);
        s.key(Key::Escape);
        let finish = s.word("wb-finish");
        s.press_word(&finish);
        build::set_parameter(&mut s, "w", "30");
        build::set_parameter(&mut s, "h", "20");
        failures.extend(corners(&mut s, &[(20.0, 0.0)], "the lower side trimmed"));
        assert!(failures.is_empty(), "a side of the outline cut:\n{}", failures.join("\n"));
    }
}

probe! {
    /// LINES STARTED BY THE CURSOR ON THE PROJECTION FOLLOW IT: one from the middle of the lower side, one from a place on
    /// the right side, nothing laid by hand. The parameters changed: the first still starts in the middle of the lower
    /// side, the second on the right side.
    fn lines_started_on_the_projection_by_the_cursor_follow_it() {
        let mut s = an_outline_on_the_top();
        build::line(&mut s, (20.0, 0.0), (20.0, 12.0));
        build::line(&mut s, (40.0, 10.0), (28.0, 14.0));
        let finish = s.word("wb-finish");
        s.press_word(&finish);
        build::set_parameter(&mut s, "w", "30");
        build::set_parameter(&mut s, "h", "20");
        let sk = s.document().sketches.last().cloned().expect("the sketch");
        let on_right = sk.places.iter().any(|p| (p[0] - 30.0).abs() < 1e-3 && p[1] > 1e-3 && (p[1] - 20.0).abs() > 1e-3);
        assert!(a_point_at(&sk, 15.0, 0.0) && on_right && sk.redundant == 0, "w 30, h 20: a line started in the middle of the lower side starts at (15, 0): {}, one started on the right side starts on it: {on_right}; {} redundant, the sketch shows {:?}", a_point_at(&sk, 15.0, 0.0), sk.redundant, sk.places);
    }
}

probe! {
    /// A RECTANGLE AND A CIRCLE PUT ON GREY CORNERS OF THE FACE, nothing projected: a rectangle from the corner (40, 30),
    /// a circle around the corner (40, 0). The parameters changed: the rectangle still starts at the corner, now (30, 20),
    /// the circle still stands around it, now (30, 0).
    ///
    /// Reported behaviour: "even if you do not project, but tie to the edges of the face (the thin grey ones), at a
    /// rebuild everything falls apart".
    fn a_rectangle_and_a_circle_on_grey_corners_follow_the_parameters() {
        let mut s = a_block_of_parameters_and_a_sketch_on_its_top();
        build::draw(&mut s, "tb-rect-hint", &[(40.0, 30.0), (25.0, 20.0)]);
        build::draw(&mut s, "tb-circle-hint", &[(40.0, 0.0), (44.0, 0.0)]);
        let finish = s.word("wb-finish");
        s.press_word(&finish);
        build::set_parameter(&mut s, "w", "30");
        build::set_parameter(&mut s, "h", "20");
        let sk = s.document().sketches.last().cloned().expect("the sketch on the top");
        assert!(
            a_point_at(&sk, 30.0, 20.0) && a_point_at(&sk, 30.0, 0.0) && sk.redundant == 0,
            "w 30, h 20: the rectangle from the corner (40, 30) starts at (30, 20): {}, the circle around the corner (40, 0) stands around (30, 0): {}; {} redundant, the sketch shows {:?}",
            a_point_at(&sk, 30.0, 20.0),
            a_point_at(&sk, 30.0, 0.0),
            sk.redundant,
            sk.places
        );
    }
}

probe! {
    /// THE CHAIN OF THE REPORT IN AN ASSEMBLY: the block in the first part; a second part made, its sketch put on the top
    /// of the block, the outline of that face of the neighbour projected, the rectangle held 5 inside it, extruded. The
    /// parameters of the block changed: the sketch of the second part follows - the outline on the new top, the rectangle
    /// 5 inside it; undone, back.
    fn what_is_tied_to_a_projection_of_a_neighbour_follows_it() {
        let mut failures = Vec::new();
        let mut s = the_chain_of(Taken::FaceOutline, Whose::Neighbour);
        let doc = s.document();
        if doc.sketches.last().is_none_or(|k| !k.seat.contains(&doc.parts.iter().find(|p| !p.assembly).map(|p| p.name.clone()).unwrap_or_default())) {
            failures.push(format!("GUARD: the sketch of the second part does not sit on the face of the first: {:?}", doc.sketches.iter().map(|k| k.seat.clone()).collect::<Vec<_>>()));
        }
        failures.extend(tied_to(&mut s, 40.0, 30.0).map(|e| format!("GUARD, as built: {e}")));
        build::set_parameter(&mut s, "w", "30");
        build::set_parameter(&mut s, "h", "20");
        failures.extend(tied_to(&mut s, 30.0, 20.0).map(|e| format!("the parameters changed: {e}")));
        s.chord(qymcad::Modifiers::COMMAND, Key::Z);
        s.chord(qymcad::Modifiers::COMMAND, Key::Z);
        failures.extend(tied_to(&mut s, 40.0, 30.0).map(|e| format!("the change undone: {e}")));
        assert!(failures.is_empty(), "a projection of a neighbour:\n{}", failures.join("\n"));
    }
}

probe! {
    /// A CIRCLE TANGENT TO TWO SIDES OF THE PROJECTION AND A LINE COLLINEAR WITH ONE, by the constraint buttons: the
    /// circle drawn near the far corner, made tangent to the right side and to the upper side; a line drawn below the
    /// upper side, made collinear with it. The parameters changed: the circle still touches both sides - its centre as
    /// far from the one as from the other - and the line lies on the upper side.
    fn tangent_and_collinear_to_a_projection_follow_the_parameters() {
        let mut s = an_outline_on_the_top();
        build::circle(&mut s, (33.0, 23.0), (37.0, 23.0));
        build::line(&mut s, (6.0, 25.0), (16.0, 26.0));
        // the circle picked by its rim where it stands - the first tangency moves it - its lowest point, away from both sides
        for side in [(40.0, 10.0), (10.0, 30.0)] {
            let c = s.document().sketches.last().and_then(|k| k.places.iter().find(|p| p[0] > 25.0 && p[0] < 39.5 && p[1] > 15.0 && p[1] < 29.5).copied()).expect("the centre of the circle");
            let r = (2..40)
                .map(|k| f64::from(k) * 0.25)
                .find_map(|d| match s.sketch_under(c[0], c[1] - d) {
                    Some(qymcad::SketchPick::Circle { radius, .. }) => Some(radius),
                    _ => None,
                })
                .unwrap_or_else(|| panic!("GUARD: no rim of a circle below its centre {c:?}"));
            pick(&mut s, c[0], c[1] - r, false);
            pick(&mut s, side.0, side.1, true);
            constrain(&mut s, "con-tangent-hint");
        }
        pick(&mut s, 11.0, 25.5, false);
        pick(&mut s, 30.0, 30.0, true);
        constrain(&mut s, "con-collinear-hint");
        let finish = s.word("wb-finish");
        s.press_word(&finish);
        let check = |s: &mut Session, w: f64, h: f64, when: &str| -> Option<String> {
            let sk = s.document().sketches.last().cloned().expect("the sketch");
            // the centre of the circle: the one point well inside the outline
            let centre = sk.places.iter().find(|p| p[0] > w / 2.0 && p[0] < w - 0.5 && p[1] > h / 2.0 && p[1] < h - 0.5).copied();
            let touches = centre.is_some_and(|c| ((w - c[0]) - (h - c[1])).abs() < 1e-3);
            let on_top = sk.places.iter().filter(|p| (p[1] - h).abs() < 1e-3 && p[0].abs() > 1e-3 && (p[0] - w).abs() > 1e-3).count();
            (!touches || on_top < 2 || sk.redundant != 0).then(|| format!("{when}, {w} x {h}: the circle touches both sides {touches} (centre {centre:?}), {on_top} ends of the line on the upper side, {} redundant; the sketch shows {:?}", sk.redundant, sk.places))
        };
        let mut failures = Vec::new();
        failures.extend(check(&mut s, 40.0, 30.0, "GUARD, as tied"));
        build::set_parameter(&mut s, "w", "30");
        build::set_parameter(&mut s, "h", "20");
        failures.extend(check(&mut s, 30.0, 20.0, "the parameters changed"));
        assert!(failures.is_empty(), "tangent and collinear to a projection:\n{}", failures.join("\n"));
    }
}

probe! {
    /// A DIMENSION FROM A GREY EDGE OF THE FACE, nothing projected: a line drawn upright inside the top, the dimension
    /// tool clicked on the grey right edge and on the line, 10 typed. The edge is taken into the sketch and the
    /// dimension holds the line 10 off it; the parameters changed, the line stands 10 off the new edge.
    fn a_dimension_from_a_grey_edge_follows_the_parameters() {
        let mut s = a_block_of_parameters_and_a_sketch_on_its_top();
        build::line(&mut s, (25.0, 8.0), (25.0, 22.0));
        a_distance_between(&mut s, (40.0, 15.0), (25.0, 15.0), "10");
        let sk = s.document().sketches.last().cloned().expect("the sketch");
        assert!(a_point_at(&sk, 30.0, 8.0) || a_point_at(&sk, 30.0, 22.0), "GUARD: the line held 10 off the grey edge stands at X 30; the sketch shows {:?}, the status says {:?}", sk.places, s.status());
        let finish = s.word("wb-finish");
        s.press_word(&finish);
        build::set_parameter(&mut s, "w", "30");
        let sk = s.document().sketches.last().cloned().expect("the sketch");
        let line_x: Vec<f64> = sk.places.iter().filter(|p| p[1] > 1.0 && p[1] < 29.0).map(|p| p[0]).collect();
        assert!(line_x.len() == 2 && line_x.iter().all(|x| (x - 20.0).abs() < 1e-3) && sk.redundant == 0, "w 30: the line held 10 off the right edge stands at X {line_x:?}, not 20; {} redundant, the sketch shows {:?}", sk.redundant, sk.places);
    }
}
