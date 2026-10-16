//! WHAT A SKETCH BECOMES: a body added, cut away or kept where it meets the part, and a body turned about an axis.
//!
//! The contract checks the extrusion of a first body point by point; what is here is the rest of the bullet - the
//! operations that need a body to work on, the extent that goes right through it, and the revolve with its axis and
//! angle. Every check reads the body it made: its volume and how far it reaches.
use qymcad::{pos2, Key, Session};
use qymcad_acceptance::build;
use qymcad_acceptance::probe;

/// The body of the part, measured.
fn body(s: &mut Session) -> qymcad::Solid {
    s.document().bodies.last().cloned().unwrap_or_else(|| panic!("the part holds no body"))
}

/// Press the word `key` names in the bar of options at the top.
fn bar_word(s: &mut Session, key: &str) {
    let word = s.word(key);
    s.press_word_near(&word, qymcad::pos2(640.0, 24.0));
}

/// Start a sketch on the plane of the world `key` names, with nothing selected first - the panel offers the planes
/// only when nothing is picked.
fn on_plane(s: &mut Session, key: &str) {
    s.key(Key::Escape);
    let plane = s.word(key);
    s.press_word(&plane);
}

/// Leave the open sketch by its button.
fn finish(s: &mut Session) {
    let finish = s.word("wb-finish");
    s.press_word(&finish);
}

/// Pick the sketch `name` by its row in the tree.
fn tree_row(s: &mut Session, name: &str) {
    let row = s.find(name, qymcad::pos2(0.0, 300.0)).unwrap_or_else(|| panic!("{name:?} is not in the tree; on screen: {:?}", s.words()));
    s.click(row.center());
}

/// A 40 x 30 x 10 block with a 10 x 10 square drawn on its top face, ready for the tool to be taken.
fn a_block_with_a_square_on_top() -> Session {
    let mut s = Session::start();
    build::block(&mut s);
    build::sketch_on_face(&mut s, [20.0, 15.0, 10.0]);
    build::draw(&mut s, "tb-rect-hint", &[(10.0, 10.0), (20.0, 20.0)]);
    let finish = s.word("wb-finish");
    s.press_word(&finish);
    s
}

probe! {
    /// CUT: the prism of the sketch is taken out of the body to the depth that was set.
    fn a_cut_takes_the_prism_of_the_sketch_out_of_the_body() {
        let mut s = a_block_with_a_square_on_top();
        let before = body(&mut s).volume;
        let extrude = s.word("tb-extrude-hint");
        s.press_hint(&extrude);
        bar_word(&mut s, "cmd-cut");
        let length = s.word("f-length");
        s.fill(&length, "5").key(Key::Enter);
        let now = body(&mut s);
        assert!((before - now.volume - 500.0).abs() < 1.0, "a 10 by 10 pocket 5 deep takes 500 out of the block: {before} became {}", now.volume);
        assert!((now.max[2] - 10.0).abs() < 1e-6, "the cut changed the height of the block: it reaches {}", now.max[2]);
    }
}

probe! {
    /// CUT THROUGH ALL: the extent that goes right through leaves a hole from the top face to the bottom.
    fn a_cut_through_all_goes_right_through_the_body() {
        let mut s = a_block_with_a_square_on_top();
        let before = body(&mut s).volume;
        let extrude = s.word("tb-extrude-hint");
        s.press_hint(&extrude);
        bar_word(&mut s, "cmd-cut");
        bar_word(&mut s, "cmd-through-all");
        s.key(Key::Enter);
        let now = body(&mut s);
        assert!((before - now.volume - 1000.0).abs() < 1.0, "a 10 by 10 hole through the 10 thick block takes 1000 out of it: {before} became {}", now.volume);
        assert!((now.min[2]).abs() < 1e-6 && (now.max[2] - 10.0).abs() < 1e-6, "the block is no longer 10 thick: it runs from {} to {}", now.min[2], now.max[2]);
    }
}

probe! {
    /// INTERSECT: only what the prism and the body have in common is kept.
    fn an_intersection_keeps_only_what_is_common() {
        let mut s = a_block_with_a_square_on_top();
        let extrude = s.word("tb-extrude-hint");
        s.press_hint(&extrude);
        bar_word(&mut s, "cmd-intersect");
        bar_word(&mut s, "cmd-flip-btn"); // into the body: upwards from the top face there is nothing to meet
        let length = s.word("f-length");
        s.fill(&length, "5").key(Key::Enter);
        let now = body(&mut s);
        let common = 10.0 * 10.0 * 5.0;
        assert!((now.volume - common).abs() < 1.0, "what the 10 by 10 prism 5 deep and the block have in common is {common}, and the body is {}", now.volume);
        assert!((now.max[2] - 10.0).abs() < 1e-6 && (now.min[2] - 5.0).abs() < 1e-6, "the piece left is not the top 5 of the block: it runs from {} to {}", now.min[2], now.max[2]);
    }
}

probe! {
    /// A CUT WITH NOTHING TO CUT IS REFUSED IN WORDS: the first feature of an empty part cannot take material away.
    fn a_cut_in_an_empty_part_is_refused_in_words() {
        let mut s = Session::start();
        build::into_the_first_part(&mut s);
        build::rectangle_on_xy(&mut s);
        let finish = s.word("wb-finish");
        s.press_word(&finish);
        let extrude = s.word("tb-extrude-hint");
        s.press_hint(&extrude);
        bar_word(&mut s, "cmd-cut");
        s.key(Key::Enter);
        assert!(s.status() == s.word("cmd-cut-needs-body"), "nothing says a cut needs a body: the status line says {:?}", s.status());
        assert!(s.document().bodies.is_empty(), "the cut in an empty part made a body: {:?}", s.document().bodies);
    }
}

probe! {
    /// REVOLVE ABOUT THE X AXIS: the 40 by 30 rectangle turned all the way round makes a cylinder of radius 30 and
    /// length 40.
    fn a_revolve_about_x_makes_a_cylinder() {
        let mut s = Session::start();
        build::into_the_first_part(&mut s);
        build::rectangle_on_xy(&mut s);
        let finish = s.word("wb-finish");
        s.press_word(&finish);
        let revolve = s.word("tb-revolve-hint");
        s.press_hint(&revolve);
        s.key(Key::Enter);
        let now = body(&mut s);
        let want = std::f64::consts::PI * 30.0 * 30.0 * 40.0;
        assert!((now.volume - want).abs() < want * 1e-3, "a cylinder of radius 30 and length 40 holds {want}, and the body holds {}", now.volume);
        assert!((now.max[0] - 40.0).abs() < 1e-3 && (now.min[1] + 30.0).abs() < 0.1, "the body does not stand about the X axis: it runs {:?} to {:?}", now.min, now.max);
    }
}

probe! {
    /// REVOLVE ABOUT THE Y AXIS: the same rectangle about the other axis makes a cylinder of radius 40 and height 30.
    fn a_revolve_about_y_makes_a_wider_cylinder() {
        let mut s = Session::start();
        build::into_the_first_part(&mut s);
        build::rectangle_on_xy(&mut s);
        let finish = s.word("wb-finish");
        s.press_word(&finish);
        let revolve = s.word("tb-revolve-hint");
        s.press_hint(&revolve);
        // the axes of a sketch are named by the letter they point along - X, Y, Z - and letters are the same in
        // every language, so this one is not a word of the catalogue
        s.press_word_near("Y", qymcad::pos2(640.0, 24.0));
        s.key(Key::Enter);
        let now = body(&mut s);
        let want = std::f64::consts::PI * 40.0 * 40.0 * 30.0;
        assert!((now.volume - want).abs() < want * 1e-3, "a cylinder of radius 40 and height 30 holds {want}, and the body holds {}", now.volume);
    }
}

probe! {
    /// AN ANGLE SHORT OF THE WHOLE TURN makes that share of the body: a quarter turn, a quarter of the cylinder.
    fn a_revolve_of_a_quarter_turn_makes_a_quarter_of_the_body() {
        let mut s = Session::start();
        build::into_the_first_part(&mut s);
        build::rectangle_on_xy(&mut s);
        let finish = s.word("wb-finish");
        s.press_word(&finish);
        let revolve = s.word("tb-revolve-hint");
        s.press_hint(&revolve);
        let angle = s.word("cmd-angle");
        s.fill(&angle, "90").key(Key::Enter);
        let now = body(&mut s);
        let want = std::f64::consts::PI * 30.0 * 30.0 * 40.0 / 4.0;
        assert!((now.volume - want).abs() < want * 1e-3, "a quarter of the cylinder holds {want}, and the body holds {}", now.volume);
    }
}

probe! {
    /// AN OPERATION THAT MAKES NOTHING SAYS SO and leaves the part as it was: the prism above the top face has
    /// nothing in common with the block.
    fn an_operation_that_makes_nothing_says_so() {
        let mut s = a_block_with_a_square_on_top();
        let before = body(&mut s);
        let extrude = s.word("tb-extrude-hint");
        s.press_hint(&extrude);
        bar_word(&mut s, "cmd-intersect");
        let length = s.word("f-length");
        s.fill(&length, "5").key(Key::Enter);
        let said = s.status();
        assert!(said.contains(&s.word("mesh-empty")), "nothing says the operation made nothing: the status line says {said:?}");
        let now = body(&mut s);
        assert!((now.volume - before.volume).abs() < 1e-6, "the part did not keep its body: {} became {}", before.volume, now.volume);
        let node = s.document().features.last().cloned().unwrap_or_else(|| panic!("the timeline is empty"));
        assert!(node.error.is_some(), "the node that made nothing stands in the timeline without a word of why: {node:?}");
    }
}

probe! {
    /// SWEEP: the profile is carried along the path, and the body it leaves is the profile times the length.
    fn a_sweep_carries_the_profile_along_the_path() {
        let mut s = Session::start();
        build::into_the_first_part(&mut s);
        on_plane(&mut s, "plane-xy-table");
        build::draw(&mut s, "tb-rect-hint", &[(0.0, 0.0), (10.0, 10.0)]);
        finish(&mut s);
        on_plane(&mut s, "plane-xz-front");
        build::draw(&mut s, "tb-line-hint", &[(0.0, 0.0), (0.0, 40.0)]);
        finish(&mut s);
        s.key(Key::Escape);
        let (profile, path) = (s.document().sketches[0].name.clone(), s.document().sketches[1].name.clone());
        tree_row(&mut s, &profile);
        let sweep = s.word("tb-sweep-hint");
        s.press_hint(&sweep);
        tree_row(&mut s, &path);
        s.key(Key::Enter);
        let now = body(&mut s);
        assert!((now.volume - 4000.0).abs() < 1.0, "a 10 by 10 profile carried 40 along leaves 4000, and the body holds {}", now.volume);
        assert!((now.max[2] - 40.0).abs() < 1e-3, "the body does not reach the end of the path at 40: it reaches {}", now.max[2]);
    }
}

probe! {
    /// LOFT: the body runs from one section to the other, and its volume is the one the two sections and the distance
    /// between them give.
    fn a_loft_runs_from_one_section_to_the_other() {
        let mut s = Session::start();
        build::into_the_first_part(&mut s);
        on_plane(&mut s, "plane-xy-table");
        build::draw(&mut s, "tb-rect-hint", &[(0.0, 0.0), (10.0, 10.0)]);
        finish(&mut s);
        // a datum 30 above the table, and the second section on it, about the same middle
        s.key(Key::Escape);
        let datum = s.word("g-datum-plane-hint");
        s.press_hint(&datum);
        let middle = s.in_space([0.0, 0.0, 0.0]);
        s.click(middle);
        // the offset typed into its own field, by its caption: the plane has its tilts and its turn beside it
        let offset = s.word("f-offset");
        s.fill(&offset, "30").key(Key::Enter);
        let pencil = s.word("g-sketch-pick-hint");
        s.press_hint(&pencil);
        let on_datum = s.in_space([0.0, 0.0, 30.0]);
        s.click(on_datum);
        build::draw(&mut s, "tb-rect-hint", &[(-5.0, -5.0), (15.0, 15.0)]);
        finish(&mut s);
        s.key(Key::Escape);
        let (first, second) = (s.document().sketches[0].name.clone(), s.document().sketches[1].name.clone());
        tree_row(&mut s, &first);
        let loft = s.word("tb-loft-hint");
        s.press_hint(&loft);
        tree_row(&mut s, &second);
        s.key(Key::Enter);
        let now = body(&mut s);
        // a body between a square of 10 and a square of 20, 30 apart: 30/6 * (100 + 4*225 + 400)
        let want = 30.0 / 6.0 * (100.0 + 4.0 * 225.0 + 400.0);
        assert!((now.volume - want).abs() < want * 1e-3, "the body between the two sections holds {want}, and it holds {}", now.volume);
        assert!((now.min[2]).abs() < 1e-3 && (now.max[2] - 30.0).abs() < 1e-3, "the body does not run from one section to the other: it goes from {} to {}", now.min[2], now.max[2]);
    }
}

probe! {
    /// WHAT WAS MADE IS SAID WHEN IT IS MADE: once the extrusion is applied and built, the status line names it - a
    /// bare "Done" tells a person nothing of what the Enter did.
    fn the_status_line_names_what_was_made() {
        let mut s = Session::start();
        build::block(&mut s);
        let made = s.document().features.last().map(|f| f.name.clone()).expect("the node the block was made by");
        let (tool, said) = (s.word("cmd-extrude"), s.status());
        assert!(said.contains(&made) || said.contains(&tool), "the extrusion was applied and the status line says {said:?}, naming neither {made:?} nor {tool:?}");
    }
}

probe! {
    /// ADDING WHAT DOES NOT TOUCH THE BODY IS REFUSED IN WORDS: a part is one solid, and a boss standing off the block
    /// would make its body two pieces. A circle is drawn on the top face beyond the back of the block and added 5
    /// tall.
    fn an_addition_that_does_not_touch_the_body_is_refused_in_words() {
        let mut s = Session::start();
        build::block(&mut s);
        build::sketch_on_face(&mut s, [20.0, 15.0, 10.0]);
        build::circle(&mut s, (20.0, 45.0), (26.0, 45.0));
        finish(&mut s);
        let status = s.status();
        let extrude = s.word("tb-extrude-hint");
        s.press_hint(&extrude);
        let length = s.word("f-length");
        s.fill(&length, "5").key(Key::Enter);
        let doc = s.document();
        let (i, now) = qymcad_acceptance::matrix::working(&mut s).unwrap_or_else(|| panic!("the part holds no body"));
        let pieces = s.inspect(i).map(|k| k.solids);
        let red = doc.features.iter().rfind(|f| f.kind != "Sketch").and_then(|f| f.error.clone());
        assert!(pieces == Some(1), "the body of the part is {pieces:?} pieces now, {} mm^3; the timeline: {:?}", now.volume, doc.features.iter().map(|f| (&f.name, &f.error)).collect::<Vec<_>>());
        assert!(red.is_some() || s.status() != status, "the addition that could not be made was refused without a word");
    }
}

probe! {
    /// A PIECE BECOMES A PART BY HAND: a slot cut right through the block leaves two pieces of 18 x 30 x 10 in the one
    /// part; the right button on the right piece and "Make a part" ask the name - the part it came from with "piece"
    /// after it - and Enter puts that piece into a part of that name, each part then one piece of 5400.
    fn a_piece_of_a_block_cut_through_is_made_a_part() {
        let mut s = Session::start();
        build::block(&mut s);
        build::sketch_on_face(&mut s, [20.0, 15.0, 10.0]);
        build::draw(&mut s, "tb-rect-hint", &[(18.0, -5.0), (22.0, 35.0)]);
        finish(&mut s);
        let extrude = s.word("tb-extrude-hint");
        s.press_hint(&extrude);
        bar_word(&mut s, "cmd-cut");
        bar_word(&mut s, "cmd-through-all");
        s.key(Key::Enter);
        let parts = s.document().parts.iter().filter(|p| !p.assembly).count();
        let at = s.seen_at([35.0, 15.0, 10.0]).unwrap_or_else(|| panic!("the top of the right piece is not in view"));
        s.click_with(at, qymcad::PointerButton::Secondary, qymcad::Modifiers::NONE);
        let make = s.word("act-piece-to-part");
        s.press_word(&make);
        // the name is asked beside the cursor: the part it came from with "piece" after it, to keep or retype
        let from = s.document().parts.iter().find(|p| !p.assembly).map(|p| p.name.clone()).unwrap_or_default();
        let offered = s.word("piece-part-name-offered").replace("{$name}", &from);
        assert!(s.words().contains(&offered), "the name {offered:?} is not offered; on screen: {:?}", s.words());
        s.key(Key::Enter);
        let doc = s.document();
        assert!(doc.parts.iter().any(|p| p.name == offered), "no part is named {offered:?}: {:?}", doc.parts.iter().map(|p| &p.name).collect::<Vec<_>>());
        let now = doc.parts.iter().filter(|p| !p.assembly).count();
        assert_eq!(now, parts + 1, "no new part came of the piece; the status line says {:?}", s.status());
        let bodies: Vec<_> = doc.bodies.iter().filter(|b| !b.consumed && !b.sheet).collect();
        let keys: std::collections::HashSet<_> = bodies.iter().map(|b| b.part_key).collect();
        assert!(bodies.len() == 2 && keys.len() == 2, "two parts of one body each were expected: {:?}", bodies.iter().map(|b| (b.part_key, b.volume)).collect::<Vec<_>>());
        for b in &bodies {
            assert!((b.volume - 5400.0).abs() < 1.0, "a piece is {:.1} mm^3, 18 x 30 x 10 = 5400", b.volume);
        }
        assert!(qymcad_acceptance::oracles::whole(&doc).is_empty(), "the document does not hold together: {:?}", qymcad_acceptance::oracles::whole(&doc));
    }
}

probe! {
    /// WITH SEVERAL CONTOURS NONE IS TAKEN FOR THE PERSON: the extrusion opens on the choice, and Enter with nothing
    /// clicked is refused in words and makes nothing. Reported behaviour: every contour came in taken, and Enter
    /// extruded two squares apart at once.
    fn enter_with_no_contour_clicked_is_refused_in_words() {
        let mut s = Session::start();
        qymcad_acceptance::bodies::two_apart(&mut s);
        let extrude = s.word("tb-extrude-hint");
        s.press_hint(&extrude);
        s.key(Key::Enter);
        let ask = s.word("hint-closed-contour");
        assert!(s.status().contains(&ask), "Enter with no contour clicked was not refused in words: the status line says {:?}", s.status());
        assert!(s.document().bodies.is_empty(), "Enter with no contour clicked made a body: {:?}", s.document().bodies);
    }
}

/// THE ROWS OF THE BODIES the part left of the canvas shows under its "Bodies" heading, by the names they are drawn with.
fn body_rows(s: &mut Session) -> Vec<(String, qymcad::Rect)> {
    let left = s.canvas().min.x;
    let names: Vec<String> = s.document().bodies.iter().filter(|b| !b.consumed && !b.sheet).map(|b| b.name.clone()).collect();
    s.words_at().into_iter().filter(|(w, r)| r.max.x < left && names.iter().any(|n| w.ends_with(n.as_str()))).collect()
}

probe! {
    /// THE BODIES OF A PART ARE ROWS OF ITS TREE: a slot cut right through the block leaves two bodies, and the tree of
    /// the part lists them under "Bodies (2)"; a click on a row picks that body - the canvas lights it - and the right
    /// button on the other row makes it a part of its own, as the item on the canvas does.
    fn the_pieces_a_cut_left_are_rows_of_the_part_tree() {
        let mut s = Session::start();
        build::block(&mut s);
        build::sketch_on_face(&mut s, [20.0, 15.0, 10.0]);
        build::draw(&mut s, "tb-rect-hint", &[(18.0, -5.0), (22.0, 35.0)]);
        finish(&mut s);
        let extrude = s.word("tb-extrude-hint");
        s.press_hint(&extrude);
        bar_word(&mut s, "cmd-cut");
        bar_word(&mut s, "cmd-through-all");
        s.key(Key::Enter);
        let heading = s.word("tree-part-bodies").replace("{$n}", "2");
        assert!(s.words().iter().any(|w| w.ends_with(heading.as_str())), "the tree does not say {heading:?}; on screen: {:?}", s.words());
        let rows = body_rows(&mut s);
        assert_eq!(rows.len(), 2, "the two pieces are not two rows of the tree: {rows:?}");
        // a click on the first row picks its body: the canvas lights it
        let canvas = s.canvas();
        let aside = pos2(canvas.max.x - 10.0, canvas.max.y - 10.0);
        s.move_to(aside);
        let before = s.snapshot();
        s.click(rows[0].1.center());
        s.move_to(aside);
        let after = s.snapshot();
        assert!(qymcad_acceptance::golden::changed_in(&before, &after, canvas, &[]), "a click on the row {:?} lit nothing on the canvas", rows[0].0);
        // the right button on the second row makes it a part
        let parts = s.document().parts.iter().filter(|p| !p.assembly).count();
        s.click_with(rows[1].1.center(), qymcad::PointerButton::Secondary, qymcad::Modifiers::NONE);
        let make = s.word("act-piece-to-part");
        s.press_word(&make);
        s.key(Key::Enter);
        let doc = s.document();
        assert_eq!(doc.parts.iter().filter(|p| !p.assembly).count(), parts + 1, "no part came of the row; the status line says {:?}", s.status());
        let bodies: Vec<_> = doc.bodies.iter().filter(|b| !b.consumed && !b.sheet).collect();
        let keys: std::collections::HashSet<_> = bodies.iter().map(|b| b.part_key).collect();
        assert!(bodies.len() == 2 && keys.len() == 2, "two parts of one body each were expected: {:?}", bodies.iter().map(|b| (&b.name, b.part_key, b.volume)).collect::<Vec<_>>());
    }
}

probe! {
    /// "MAKE A PART" IS FOUND BY THE COMMAND SEARCH: taken with nothing picked, it says to click a piece; the click on
    /// the right piece of a block cut through asks the name, and Enter makes the part.
    fn a_piece_is_made_a_part_through_the_command_search() {
        let mut s = Session::start();
        build::block(&mut s);
        build::sketch_on_face(&mut s, [20.0, 15.0, 10.0]);
        build::draw(&mut s, "tb-rect-hint", &[(18.0, -5.0), (22.0, 35.0)]);
        finish(&mut s);
        let extrude = s.word("tb-extrude-hint");
        s.press_hint(&extrude);
        bar_word(&mut s, "cmd-cut");
        bar_word(&mut s, "cmd-through-all");
        s.key(Key::Enter);
        s.key(Key::Escape);
        let canvas = s.canvas();
        s.click(pos2(canvas.max.x - 10.0, canvas.max.y - 10.0)); // nothing picked
        let parts = s.document().parts.iter().filter(|p| !p.assembly).count();
        let make = s.word("act-piece-to-part");
        s.chord(qymcad::Modifiers::COMMAND, Key::K).type_text(&make).key(Key::Enter);
        let pick = s.word("piece-part-pick");
        assert_eq!(s.status(), pick, "the command taken from the search does not say what to click");
        let at = s.seen_at([35.0, 15.0, 10.0]).unwrap_or_else(|| panic!("the top of the right piece is not in view"));
        s.click(at);
        s.key(Key::Enter);
        let doc = s.document();
        assert_eq!(doc.parts.iter().filter(|p| !p.assembly).count(), parts + 1, "no part came of the piece; the status line says {:?}", s.status());
        let bodies: Vec<_> = doc.bodies.iter().filter(|b| !b.consumed && !b.sheet).collect();
        let keys: std::collections::HashSet<_> = bodies.iter().map(|b| b.part_key).collect();
        assert!(bodies.len() == 2 && keys.len() == 2, "two parts of one body each were expected: {:?}", bodies.iter().map(|b| (&b.name, b.part_key, b.volume)).collect::<Vec<_>>());
    }
}
