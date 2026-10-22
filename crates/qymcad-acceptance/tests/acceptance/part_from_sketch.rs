//! WHAT A SKETCH BECOMES: a body added, cut away or kept where it meets the part, and a body turned about an axis.
//!
//! The contract checks the extrusion of a first body point by point; what is here is the rest of the bullet - the
//! operations that need a body to work on, the extent that goes right through it, and the revolve with its axis and
//! angle. Every check reads the body it made: its volume and how far it reaches.
use qymcad::{pos2, Key, Session, Tone};
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
        // said where the tool speaks, in the tone of a refusal
        let why = s.word("cmd-cut-needs-body");
        let says = s.tool_says();
        assert!(says.iter().any(|n| n.words == why && n.tone == qymcad::Tone::Refusal), "nothing says a cut needs a body where the tool speaks: {says:?}");
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

/// A path of a sweep up the front plane, as drawn: from `from` to `to`, in the plane's own X and Z.
struct Path {
    label: &'static str,
    from: (f64, f64),
    to: (f64, f64),
}

probe! {
    /// A SWEEP STARTS AT ITS PROFILE WHERE IT IS DRAWN: a 6 x 4 rectangle drawn on the table away from the origin, a
    /// path up the front plane from its corner - drawn from the corner or down to it. The body stands over the
    /// rectangle, 30 tall. Reported behaviour: the body stood away from the profile, carried by its sketch's origin to
    /// the start of the path, and with the path drawn the other way it was turned over as well.
    fn a_sweep_starts_at_its_profile_where_it_is_drawn() {
        let mut fails = Vec::new();
        let paths = [Path { label: "up from the corner", from: (10.0, 0.0), to: (10.0, 30.0) }, Path { label: "down to the corner", from: (10.0, 30.0), to: (10.0, 0.0) }];
        for path in &paths {
            let mut s = Session::start();
            build::into_the_first_part(&mut s);
            on_plane(&mut s, "plane-xy-table");
            build::draw(&mut s, "tb-rect-hint", &[(10.0, 0.0), (16.0, 4.0)]);
            finish(&mut s);
            on_plane(&mut s, "plane-xz-front");
            build::line(&mut s, path.from, path.to);
            finish(&mut s);
            s.key(Key::Escape);
            let (profile, along) = (s.document().sketches[0].name.clone(), s.document().sketches[1].name.clone());
            tree_row(&mut s, &profile);
            let sweep = s.word("tb-sweep-hint");
            s.press_hint(&sweep);
            tree_row(&mut s, &along);
            s.key(Key::Enter);
            let now = body(&mut s);
            let (min, max) = ([10.0, 0.0, 0.0], [16.0, 4.0, 30.0]);
            if (0..3).any(|i| (now.min[i] - min[i]).abs() > 1e-3 || (now.max[i] - max[i]).abs() > 1e-3) {
                fails.push(format!("{}: the body stands in {:.2?} .. {:.2?}, not over the rectangle in {min:?} .. {max:?}", path.label, now.min, now.max));
            }
        }
        assert!(fails.is_empty(), "\n{}", fails.join("\n"));
    }
}

probe! {
    /// A SWEEP ALONG AN ARC FROM A TRIANGLE ON A FACE STANDS ON THE FACE (a private sample): a triangle drawn on the
    /// side face of a ring, an arc on its front face from the triangle's corner, around the ring. The body runs along the
    /// arc from the triangle, within the arc's 10 mm span across X and the triangle's 4 mm across Y. Reported behaviour:
    /// the body was built 32 mm off, by the profile sketch's origin.
    fn a_sweep_along_an_arc_from_a_triangle_on_a_face_stands_on_the_face() {
        let Some(path) = qymcad_acceptance::private_sample("issue-239/project.qcad") else { return };
        let mut s = Session::start();
        build::open_project(&mut s, &path);
        let part = s.document().parts.first().map(|p| p.name.clone()).unwrap_or_else(|| panic!("the sample holds no part"));
        let row = s.find(&part, qymcad::pos2(0.0, 300.0)).unwrap_or_else(|| panic!("the part {part:?} is not in the tree"));
        s.double_click(row.center());
        s.key(Key::Escape);
        // the last two sketches of the sample: the triangle on the side face, then the arc on the front face
        let sketches: Vec<String> = s.document().sketches.iter().map(|k| k.name.clone()).collect();
        let [.., triangle, arc] = sketches.as_slice() else { panic!("the sample holds fewer than two sketches: {sketches:?}") };
        tree_row(&mut s, triangle);
        let sweep = s.word("tb-sweep-hint");
        s.press_hint(&sweep);
        tree_row(&mut s, arc);
        let before: Vec<String> = s.document().bodies.iter().map(|b| b.name.clone()).collect();
        s.key(Key::Enter);
        let doc = s.document();
        let red: Vec<_> = doc.features.iter().filter(|f| matches!(f.kind.as_str(), "Sweep" | "BodyBoolean") && f.error.is_some()).collect();
        assert!(red.is_empty(), "the sweep did not build: {red:?}; the program says {:?}", s.status());
        let made = doc.bodies.iter().find(|b| !before.contains(&b.name)).unwrap_or_else(|| panic!("the sweep made no body: {:?}", doc.bodies));
        // The triangle stands on x = -5.05, y from -20 to -15.86, z from 31.73 to 32.73; the arc about the Y axis in
        // y = -20 turns it by 17.96 deg, from x = -5.05 to x = 4.98 at radius 32.13. Its outer corner, at radius 33.12,
        // ends at x = 33.12 sin(9.19 deg) = 5.29 and passes over the top at z = 33.12; its inner edge keeps to 31.73
        // at both ends. The body the old placement built stood at y 11.7 .. 12.7, z 12.0 .. 16.3.
        let (min, max) = ([-5.047, -20.0, 31.732], [5.288, -15.861, 33.117]);
        let off = (0..3).any(|i| (made.min[i] - min[i]).abs() > 0.01 || (made.max[i] - max[i]).abs() > 0.01);
        assert!(!off, "the body stands in {:.3?} .. {:.3?}, not along the arc from the triangle in {min:?} .. {max:?}", made.min, made.max);
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
        let said = s.tool_says();
        assert!(said.iter().any(|n| n.tone == Tone::Refusal && n.words.contains(&ask)), "Enter with no contour clicked was not refused in words where the extrusion speaks: {said:?}");
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

/// What a loft through the inner squares of the two rings of `bodies::loft_rings` holds: a square of 20 to a square of
/// 10, 30 apart - 30/6 * (400 + 4 * 225 + 100).
const INNER_LOFT: f64 = 7000.0;

/// A CLICK BESIDE A LINE, on one side of it: 3 px off the point `at` of the view, towards the point `toward`.
struct Beside {
    at: [f64; 3],
    toward: [f64; 3],
}

/// Where on screen the click [`Beside`] lands.
fn beside(s: &mut Session, b: Beside) -> qymcad::Pos2 {
    let at = s.in_space(b.at);
    let toward = s.projected(b.toward) - at;
    at + toward.normalized() * 3.0
}

probe! {
    /// LOFT, THE INNER CONTOURS CLICKED IN 3D: the sections are the two sketches picked in the tree, and a click on the
    /// line of the inner square of each takes that square as the contour of its section. Reported behaviour: a click on
    /// a section in 3D took nothing of it - the face behind it, or nothing - and the loft ran through the outer squares.
    fn a_loft_takes_the_inner_contours_clicked_in_3d() {
        let mut s = Session::start();
        qymcad_acceptance::bodies::loft_rings(&mut s);
        let loft = s.word("tb-loft-hint");
        s.press_hint(&loft);
        let second = s.document().sketches[1].name.clone();
        tree_row(&mut s, &second);
        let first_inner = s.in_space([20.0, 10.0, 0.0]);
        s.click(first_inner);
        let second_inner = s.in_space([20.0, 15.0, 30.0]);
        s.click(second_inner);
        s.key(Key::Enter);
        let now = body(&mut s);
        assert!((now.volume - INNER_LOFT).abs() < 1.0, "a loft through the inner squares holds {INNER_LOFT}, and the body holds {} from {:?} to {:?}", now.volume, now.min, now.max);
    }
}

probe! {
    /// A LOFT BEING EDITED TAKES ITS SECTIONS BY THEIR LINES, and stays in hand saying it is being edited: the loft
    /// through the outer squares is opened again from the tree, the inner line of the upper section clicked in 3D on the
    /// top of the body, the inner line of the lower one in its sketch laid flat. Reported behaviour: the click in 3D took
    /// the face and let go of the loft being edited, and a contour picked in the sketch said "Enter builds it", as if the
    /// loft were new.
    fn a_loft_being_edited_takes_its_sections_by_their_lines() {
        let mut s = Session::start();
        qymcad_acceptance::bodies::loft_rings(&mut s);
        let loft = s.word("tb-loft-hint");
        s.press_hint(&loft);
        let second = s.document().sketches[1].name.clone();
        tree_row(&mut s, &second);
        s.key(Key::Enter);
        let outer = body(&mut s).volume;
        assert!((outer - 48000.0).abs() < 1.0, "setup: the loft through the outer squares holds 48000, and the body holds {outer}");
        build::reopen(&mut s, "feat-loft");
        let editing = s.word("msg-edit-feature");
        let upper = s.in_space([20.0, 15.0, 30.0]);
        s.click(upper);
        let mut fails = Vec::new();
        if s.status() != editing {
            fails.push(format!("the upper section clicked in 3D: the loft being edited says {:?}, in hand: {:?}", s.status(), s.in_hand()));
        }
        section_by_its_line(&mut s, 0, 10.0);
        if s.status() != editing {
            fails.push(format!("the lower section picked in its sketch: the loft being edited says {:?}, in hand: {:?}", s.status(), s.in_hand()));
        }
        s.key(Key::Enter);
        let now = body(&mut s);
        if (now.volume - INNER_LOFT).abs() > 1.0 {
            fails.push(format!("the loft edited onto the inner squares holds {INNER_LOFT}, and the body holds {} from {:?} to {:?}", now.volume, now.min, now.max));
        }
        assert!(fails.is_empty(), "\n{}", fails.join("\n"));
    }
}

/// THE CONTOUR OF SECTION `section` PICKED IN ITS SKETCH LAID FLAT, 3 px below the line at y = `line` there: the button
/// of the section - "c<n>/<of>", the sections in order along the bar - lays the sketch flat, and the click falls just
/// outside the inner square, inside the outer one.
fn section_by_its_line(s: &mut Session, section: usize, line: f64) {
    let mut buttons: Vec<_> = s.words_at().into_iter().filter(|(t, _)| t.starts_with('c') && t.contains('/') && t.len() <= 6).collect();
    buttons.sort_by(|a, b| a.1.min.x.total_cmp(&b.1.min.x));
    let (_, at) = buttons.get(section).cloned().unwrap_or_else(|| panic!("no contour button of section {}: {buttons:?}", section + 1));
    s.click(at.center());
    let on = s.on_sheet(20.0, line);
    s.click(qymcad::pos2(on.x, on.y + 3.0));
}

probe! {
    /// THE CONTOUR OF A SECTION PICKED IN THE SKETCH BY ITS LINE: the button of the first section lays its sketch flat,
    /// and a click 3 px outside the inner square - inside the outer one - takes the inner square, whose line it is
    /// at. Reported behaviour: the outer square, around the click, was taken, and the inner one could not be.
    fn a_section_contour_is_picked_in_the_sketch_by_its_line() {
        let mut s = Session::start();
        qymcad_acceptance::bodies::loft_rings(&mut s);
        let loft = s.word("tb-loft-hint");
        s.press_hint(&loft);
        let second = s.document().sketches[1].name.clone();
        tree_row(&mut s, &second);
        // the bottom line of the inner square of each section: y 10 on the table, y 15 above
        section_by_its_line(&mut s, 0, 10.0);
        section_by_its_line(&mut s, 1, 15.0);
        s.key(Key::Enter);
        let now = body(&mut s);
        assert!((now.volume - INNER_LOFT).abs() < 1.0, "a loft through the inner squares picked in the sketch holds {INNER_LOFT}, and the body holds {} from {:?} to {:?}", now.volume, now.min, now.max);
    }
}

/// What a tool leaves of the ring once the inner square is let go.
struct Leaves {
    tool: &'static str,
    ring: f64,
}

probe! {
    /// AN EXTRUSION AND A REVOLUTION TAKE THE LINE UNDER A 3D CLICK FIRST, as the loft does: the ring of a square of 40
    /// about a square of 20 and the square in it picked, a click in 3D 3 px into the ring, beside the line of the inner
    /// square, lets go of the inner square, not of the ring around the click.
    fn an_extrusion_and_a_revolution_take_the_line_under_a_3d_click_first() {
        use std::f64::consts::PI;
        let mut fails = Vec::new();
        // the ring left once the inner square is let go: extruded 10, or turned about X with its middle 20 off the axis
        for Leaves { tool, ring } in [Leaves { tool: "tb-extrude-hint", ring: 1200.0 * 10.0 }, Leaves { tool: "tb-revolve-hint", ring: 1200.0 * 2.0 * PI * 20.0 }] {
            let mut s = Session::start();
            build::into_the_first_part(&mut s);
            on_plane(&mut s, "plane-xy-table");
            build::draw(&mut s, "tb-rect-hint", &[(0.0, 0.0), (40.0, 40.0)]);
            build::draw(&mut s, "tb-rect-hint", &[(10.0, 10.0), (30.0, 30.0)]);
            finish(&mut s);
            let hint = s.word(tool);
            s.press_hint(&hint);
            // both regions picked on the flat sheet - the ring and the square in it - and on to 3D with Enter
            for (x, y) in [(5.0, 5.0), (20.0, 20.0)] {
                let at = s.on_sheet(x, y);
                s.click(at);
            }
            s.key(Key::Enter);
            let click = beside(&mut s, Beside { at: [20.0, 30.0, 0.0], toward: [20.0, 40.0, 0.0] });
            s.click(click);
            if tool == "tb-extrude-hint" {
                let length = s.word("f-length");
                s.fill(&length, "10");
            }
            s.key(Key::Enter);
            let got = s.document().bodies.last().map(|b| b.volume);
            if got.is_none_or(|v| (v - ring).abs() > ring * 1e-3) {
                fails.push(format!("{tool}: the inner square let go leaves the ring, {ring:.1}, and the body holds {got:?}"));
            }
        }
        assert!(fails.is_empty(), "\n{}", fails.join("\n"));
    }
}

probe! {
    /// ON THE SHEET A CLICK TAKES THE LINE UNDER IT FIRST, AND THE REGION LIT UNDER THE POINTER IS THE ONE IT TAKES: the
    /// ring of a square of 40 about a square of 20, the pointer 3 px into the ring beside the line of the inner square.
    /// The inner square is lit - its middle, away from every line, changes - and the click picks it: extruded 10 it
    /// holds 4000. Reported behaviour: the smallest contour around the pointer was taken and lit, the ring.
    fn on_the_sheet_a_click_takes_the_line_under_it_and_lights_what_it_takes() {
        let mut s = Session::start();
        build::into_the_first_part(&mut s);
        on_plane(&mut s, "plane-xy-table");
        build::draw(&mut s, "tb-rect-hint", &[(0.0, 0.0), (40.0, 40.0)]);
        build::draw(&mut s, "tb-rect-hint", &[(10.0, 10.0), (30.0, 30.0)]);
        finish(&mut s);
        let hint = s.word("tb-extrude-hint");
        s.press_hint(&hint);
        // the sheet is fitted to the sketch as the tool lays it out: the places are read once it stands
        s.snapshot();
        // the pointer in the ring away from every line lights the ring, beside the line of the inner square the square
        let in_ring = s.on_sheet(35.0, 20.0);
        s.move_to(in_ring);
        let ring_lit = s.snapshot();
        let line = s.on_sheet(20.0, 30.0);
        let beside = line + (s.on_sheet(20.0, 40.0) - line).normalized() * 3.0;
        s.move_to(beside);
        let lit = s.snapshot();
        let middle = qymcad::Rect::from_center_size(s.on_sheet(20.0, 20.0), qymcad::vec2(16.0, 16.0));
        let mut fails = Vec::new();
        let (dark, light) = (brightness(&ring_lit, middle), brightness(&lit, middle));
        if light < dark + 10.0 {
            fails.push(format!("the pointer beside the line of the inner square does not light the inner square: its middle is {light:.0} bright, and {dark:.0} with the ring lit"));
        }
        s.click(beside);
        s.key(Key::Enter);
        let length = s.word("f-length");
        s.fill(&length, "10").key(Key::Enter);
        let got = s.document().bodies.last().map(|b| b.volume);
        if got.is_none_or(|v| (v - 4000.0).abs() > 1.0) {
            fails.push(format!("the click beside the line of the inner square picks it, 20 x 20 extruded 10 = 4000, and the body holds {got:?}"));
        }
        assert!(fails.is_empty(), "\n{}", fails.join("\n"));
    }
}

/// How bright `area` of a picture is, the mean of the channels of its pixels: a region lit under the pointer is filled a
/// shade lighter than the sheet, 25 steps of a channel, under the tolerance pictures are told apart by.
fn brightness(p: &qymcad::Picture, area: qymcad::Rect) -> f64 {
    let across = area.min.x as usize..(area.max.x as usize).min(p.width);
    let down = area.min.y as usize..(area.max.y as usize).min(p.height);
    let count = across.len() * down.len();
    let sum: f64 = down.flat_map(|y| across.clone().map(move |x| (y * p.width + x) * 4)).map(|i| p.rgba[i..i + 3].iter().map(|c| f64::from(*c)).sum::<f64>() / 3.0).sum();
    sum / count.max(1) as f64
}

/// The block with a square of 4 drawn on the side plane over its top front edge, y 0..4, z 10..14, the sketch left
/// selected: the profile of a rail swept along that edge.
fn a_block_with_a_square_over_its_edge() -> Session {
    let mut s = Session::start();
    build::block(&mut s);
    on_plane(&mut s, "plane-yz-side");
    build::draw(&mut s, "tb-rect-hint", &[(0.0, 10.0), (4.0, 14.0)]);
    finish(&mut s);
    s
}

/// A picture of the window before something and after it.
struct Before<'a> {
    was: &'a qymcad::Picture,
    now: &'a qymcad::Picture,
}

/// How many pixels of `area` are drawn otherwise now, past the tolerance a channel is told apart by: a thin line of a
/// preview crosses a few, under the share a whole picture is judged by.
fn pixels_changed(pictures: Before<'_>, area: qymcad::Rect) -> usize {
    let Before { was, now } = pictures;
    let across = area.min.x as usize..(area.max.x as usize).min(was.width);
    let down = area.min.y as usize..(area.max.y as usize).min(was.height);
    let tolerance = qymcad_acceptance::golden::CHANNEL_TOLERANCE;
    down.flat_map(|y| across.clone().map(move |x| (y * was.width + x) * 4)).filter(|&i| was.rgba[i..i + 4].iter().zip(&now.rgba[i..i + 4]).any(|(p, q)| p.abs_diff(*q) > tolerance)).count()
}

probe! {
    /// THE SWEEP ALONG AN EDGE IS PREVIEWED BEFORE ENTER, where its body will stand: the edge clicked, the rail of 4 x 4
    /// is drawn along it - the top of its far end, 4 above the edge lit, is drawn where nothing was with the edge not
    /// yet picked - and the document holds no sweep yet.
    fn a_sweep_along_an_edge_is_previewed_before_enter() {
        let mut s = a_block_with_a_square_over_its_edge();
        let sweep = s.word("tb-sweep-hint");
        s.press_hint(&sweep);
        // the top of the far end of the rail, 4 above the edge lit; the square of the profile stands at the near end
        let end = s.in_space([40.0, 2.0, 14.0]);
        let canvas = s.canvas();
        let away = qymcad::pos2(canvas.min.x + 20.0, canvas.max.y - 20.0);
        s.move_to(away);
        let bare = s.snapshot();
        let edge = s.edge_at([20.0, 0.0, 10.0]);
        s.click(edge);
        s.move_to(away);
        let shown = s.snapshot();
        let mut fails = Vec::new();
        let drawn = pixels_changed(Before { was: &bare, now: &shown }, qymcad::Rect::from_center_size(end, qymcad::vec2(8.0, 8.0)));
        if drawn < 4 {
            fails.push(format!("the edge picked, the far end of the rail is not drawn before Enter: {drawn} pixels changed at its top"));
        }
        if s.document().features.iter().any(|f| f.kind == "Sweep") {
            fails.push("the document holds the sweep before Enter".to_string());
        }
        s.key(Key::Escape);
        assert!(fails.is_empty(), "\n{}", fails.join("\n"));
    }
}

/// The block with a square of 4 drawn on the side plane across its top front edge, y 0..4, z 8..12, half of it in the
/// block, the sketch left selected; with `path`, a line along that edge on the front plane as well, the square then
/// picked in the tree.
fn a_block_with_a_square_across_its_edge(path: PathOf) -> Session {
    let mut s = Session::start();
    build::block(&mut s);
    on_plane(&mut s, "plane-yz-side");
    build::draw(&mut s, "tb-rect-hint", &[(0.0, 8.0), (4.0, 12.0)]);
    finish(&mut s);
    if path == PathOf::Sketch {
        on_plane(&mut s, "plane-xz-front");
        build::line(&mut s, (0.0, 10.0), (40.0, 10.0));
        finish(&mut s);
        s.key(Key::Escape);
        let square = s.document().sketches[1].name.clone();
        tree_row(&mut s, &square);
    }
    s
}

/// What the path of a sweep is laid along first.
#[derive(Clone, Copy, PartialEq)]
enum PathOf {
    Edge,
    Sketch,
}

/// An operation of the bar of a sweep and the body of the block with the rail across its top front edge it gives: the
/// rail adds 320 out of the block, cuts 320 from it, or is 320 in common with it.
struct Made {
    op: &'static str,
    volume: f64,
}

/// A body a way of laying a sweep gave.
struct Got {
    how: &'static str,
    volume: f64,
}

/// THE SWEEP LAID along `path` with the operation `op`: the tool, the operation in its bar, the path - the edge clicked
/// or the line's row in the tree - and Enter.
fn swept(s: &mut Session, path: PathOf, op: &str) {
    let sweep = s.word("tb-sweep-hint");
    s.press_hint(&sweep);
    match path {
        PathOf::Edge => {
            let edge = s.edge_at([20.0, 0.0, 10.0]);
            s.click(edge);
        }
        PathOf::Sketch => {
            let line = s.document().sketches[2].name.clone();
            tree_row(s, &line);
        }
    }
    bar_word(s, op);
    s.key(Key::Enter);
}

probe! {
    /// A SWEEP LAID AND A SWEEP EDITED GIVE ONE BODY, for each operation: laid along the edge with the operation; laid
    /// with Add and given the operation when opened again; laid along a line on the edge and given the edge when opened
    /// again. Reported behaviour: the operation chosen while a sweep was edited went nowhere, and a path turned from a
    /// sketch to edges kept the body it was laid with.
    fn a_sweep_laid_and_a_sweep_edited_give_one_body() {
        let mut fails = Vec::new();
        let made = [Made { op: "cmd-add", volume: 12320.0 }, Made { op: "cmd-cut", volume: 11680.0 }, Made { op: "cmd-intersect", volume: 320.0 }];
        for Made { op, volume } in made {
            let mut got = Vec::new();
            let mut s = a_block_with_a_square_across_its_edge(PathOf::Edge);
            swept(&mut s, PathOf::Edge, op);
            got.push(Got { how: "laid along the edge", volume: body(&mut s).volume });
            let mut s = a_block_with_a_square_across_its_edge(PathOf::Edge);
            swept(&mut s, PathOf::Edge, "cmd-add");
            build::reopen(&mut s, "feat-sweep");
            bar_word(&mut s, op);
            s.key(Key::Enter);
            got.push(Got { how: "laid with Add, the operation given when opened again", volume: body(&mut s).volume });
            if op != "cmd-add" {
                let mut s = a_block_with_a_square_across_its_edge(PathOf::Sketch);
                swept(&mut s, PathOf::Sketch, op);
                build::reopen(&mut s, "feat-sweep");
                let edge = s.edge_at([20.0, 0.0, 10.0]);
                s.click(edge);
                s.key(Key::Enter);
                got.push(Got { how: "laid along the line, the edge given when opened again", volume: body(&mut s).volume });
            }
            for Got { how, volume: v } in got {
                if (v - volume).abs() > 1.0 {
                    fails.push(format!("{op}, {how}: the body holds {v:.1}, not {volume}"));
                }
            }
        }
        // laid with Add along the line, its union joins it to the block after it: a cut asked when it is opened again is
        // refused in words, and the body stays the block with the rail on it
        let mut s = a_block_with_a_square_across_its_edge(PathOf::Sketch);
        swept(&mut s, PathOf::Sketch, "cmd-add");
        build::reopen(&mut s, "feat-sweep");
        bar_word(&mut s, "cmd-cut");
        s.key(Key::Enter);
        let refused = s.word("msg-sweep-laid-as-addition");
        let v = body(&mut s).volume;
        let said: Vec<String> = s.tool_says().into_iter().map(|n| n.words).chain(std::iter::once(s.status())).collect();
        if !said.iter().any(|w| w.contains(&refused)) || (v - 12320.0).abs() > 1.0 {
            fails.push(format!("a sweep laid with Add along a line, given Cut when opened again: says {said:?}, not {refused:?}; the body holds {v:.1}, not 12320"));
        }
        assert!(fails.is_empty(), "\n{}", fails.join("\n"));
    }
}

probe! {
    /// AN EDGE OF ANOTHER PART IS NOT A PATH: in an assembly of two blocks, a square drawn in the first part and the sweep
    /// taken there, the second block shown beside it in context and its top face clicked - its edges are not taken as
    /// the path, and Enter lays nothing that would stand red with an input of another part.
    fn an_edge_of_another_part_is_not_a_path() {
        let mut s = qymcad_acceptance::contract::fixtures::Fixture::TwoBlocksInAssembly.start();
        let first = s.document().parts.iter().find(|p| !p.assembly).map(|p| p.name.clone()).expect("the first part");
        let row = s.find(&first, qymcad::pos2(0.0, 300.0)).unwrap_or_else(|| panic!("{first:?} is not in the tree"));
        s.double_click(row.center());
        on_plane(&mut s, "plane-yz-side");
        build::draw(&mut s, "tb-rect-hint", &[(0.0, 10.0), (4.0, 14.0)]);
        finish(&mut s);
        let sweep = s.word("tb-sweep-hint");
        s.press_hint(&sweep);
        // the neighbouring part shown in the context of this one, as a person shows it to work beside it
        let beside = s.word("wb-in-context");
        s.press_word(&beside);
        let foreign = s.face_at([80.0, 15.0, 10.0]);
        s.click(foreign);
        let edges = s.word_with("cmd-path-edges-n", qymcad::Arg { name: "n", value: "4" });
        let before = s.document().features.len();
        s.key(Key::Enter);
        let laid: Vec<String> = s.document().features.iter().skip(before).map(|f| format!("{} {:?}", f.kind, f.error)).collect();
        assert!(laid.is_empty() && !s.in_hand().contains(&edges), "the top face of the other part clicked for the path, Enter: the bar {:?}, laid {laid:?}", s.in_hand());
    }
}

probe! {
    /// A SWEEP RUNS ALONG AN EDGE OF THE BODY clicked in the view, as along a path sketch: the square over the top front
    /// edge of the block, swept along that edge, adds a rail of 4 x 4 along it; the block made 12 tall, the edge rises
    /// with it and the rail still runs its length; opened again, the sweep lights the edge and says its path is one edge.
    fn a_sweep_runs_along_an_edge_clicked_in_the_view() {
        let mut s = a_block_with_a_square_over_its_edge();
        let sweep = s.word("tb-sweep-hint");
        s.press_hint(&sweep);
        let edge = s.edge_at([20.0, 0.0, 10.0]);
        s.click(edge);
        let one = s.word_with("cmd-path-edges-n", qymcad::Arg { name: "n", value: "1" });
        let mut fails = Vec::new();
        if !s.in_hand().contains(&one) {
            fails.push(format!("the edge clicked for the path is not said in the bar as {one:?}: {:?}", s.in_hand()));
        }
        s.key(Key::Enter);
        let now = body(&mut s);
        if (now.volume - 12640.0).abs() > 1.0 || (now.max[2] - 14.0).abs() > 1e-3 {
            fails.push(format!("the block with a rail of 4 x 4 along its top front edge holds 12640 up to z 14, and the body holds {} from {:?} to {:?}", now.volume, now.min, now.max));
        }
        build::reopen(&mut s, "feat-extrude");
        let length = s.word("f-length");
        s.fill(&length, "12").key(Key::Enter);
        let now = body(&mut s);
        let red: Vec<_> = s.document().features.into_iter().filter(|f| f.error.is_some()).map(|f| (f.name, f.error)).collect();
        if !red.is_empty() || (now.volume - 14720.0).abs() > 1.0 {
            fails.push(format!("the block made 12 tall with the rail standing 2 above it holds 14720, and the body holds {}; red: {red:?}", now.volume));
        }
        build::reopen(&mut s, "feat-sweep");
        if !s.in_hand().contains(&one) {
            fails.push(format!("the sweep opened again does not say its path is one edge: {:?}", s.in_hand()));
        }
        s.key(Key::Escape);
        assert!(fails.is_empty(), "\n{}", fails.join("\n"));
    }
}

probe! {
    /// THE AXIS OF A REVOLUTION IS CLICKED IN 3D WITH ITS SKETCH IN HAND: a rectangle beside a block revolved, the pick of
    /// the axis in 3D taken from the bar and the top front edge of the block clicked - the edge is the axis, said in the
    /// status line, and the revolution is built about it. Reported behaviour: with the sketch of the profile in hand a
    /// click in 3D went to the contours of the sketch, and the axis was never taken.
    fn the_axis_of_a_revolution_is_clicked_in_3d_with_its_sketch_in_hand() {
        let mut s = Session::start();
        build::block(&mut s);
        on_plane(&mut s, "plane-xy-table");
        build::draw(&mut s, "tb-rect-hint", &[(50.0, 0.0), (60.0, 10.0)]);
        finish(&mut s);
        let revolve = s.word("tb-revolve-hint");
        s.press_hint(&revolve);
        bar_word(&mut s, "cmd-pick-axis-3d");
        let edge = s.edge_at([20.0, 0.0, 10.0]);
        s.click(edge);
        let taken = s.word("g-axis-body-edge");
        let said = s.status();
        let mut fails = Vec::new();
        if !said.contains(&taken) {
            fails.push(format!("the top front edge of the block clicked for the axis: the status line says {said:?}, not that the axis is {taken:?}"));
        }
        let before = s.document().bodies.len();
        s.key(Key::Enter);
        if s.document().bodies.len() <= before && s.document().features.iter().all(|f| f.kind != "Revolve") {
            fails.push(format!("no revolution was built about the edge; the status line says {:?}", s.status()));
        }
        assert!(fails.is_empty(), "\n{}", fails.join("\n"));
    }
}

probe! {
    /// THE FIRST SWEEP OF A PART, OPENED AGAIN AND APPLIED, IS THE SAME BODY: a square of 10 swept up a line of 40 in an
    /// empty part, 4000, opened by a double click on its row and applied with Enter - still 4000, the node green; given Cut
    /// when opened again, it is refused in words, there being no body before it to cut. Reported behaviour: the edit took
    /// the active body of the part - the sweep's own - for the body it works on, and made a boolean of it with itself.
    fn the_first_sweep_of_a_part_opened_again_is_the_same_body() {
        let mut s = qymcad_acceptance::contract::fixtures::Fixture::SweepStraight.start();
        let sweep = s.word("tb-sweep-hint");
        s.press_hint(&sweep);
        let path = s.document().sketches[1].name.clone();
        tree_row(&mut s, &path);
        s.key(Key::Enter);
        let mut fails = Vec::new();
        let laid = body(&mut s).volume;
        build::reopen(&mut s, "feat-sweep");
        s.key(Key::Enter);
        let red: Vec<String> = s.document().features.iter().filter(|f| f.error.is_some()).map(|f| format!("{} {:?}", f.kind, f.error)).collect();
        let v = body(&mut s).volume;
        if (laid - 4000.0).abs() > 1.0 || (v - 4000.0).abs() > 1.0 || !red.is_empty() {
            fails.push(format!("the square of 10 swept up 40 holds {laid:.1}, opened again and applied {v:.1}, not 4000; red: {red:?}"));
        }
        build::reopen(&mut s, "feat-sweep");
        bar_word(&mut s, "cmd-cut");
        s.key(Key::Enter);
        let refused = s.word("cmd-cut-needs-body");
        let said: Vec<String> = s.tool_says().into_iter().map(|n| n.words).chain(std::iter::once(s.status())).collect();
        let v = body(&mut s).volume;
        if !said.iter().any(|w| w.contains(&refused)) || (v - 4000.0).abs() > 1.0 {
            fails.push(format!("the first sweep of a part given Cut when opened again: says {said:?}, not {refused:?}; the body holds {v:.1}, not 4000"));
        }
        assert!(fails.is_empty(), "\n{}", fails.join("\n"));
    }
}
