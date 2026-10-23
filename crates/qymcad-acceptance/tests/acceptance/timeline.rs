//! THE TIMELINE OF A PART: rolling back to a step, suppressing one, changing the order of two (where it may be
//! changed and where it may not), deleting a step others lean on, copying one, and opening one again.
use qymcad::{Key, Modifiers, Rect, Session};
use qymcad_acceptance::{build, probe};

/// THE BLOCK WITH TWO HOLES, one in its top and one in its front: three steps, of which the two holes lean on the
/// block and not on each other. Two holes in ONE face cannot be drilled (see the findings), so they are put in
/// two.
fn a_block_with_two_holes() -> Session {
    let mut s = Session::start();
    build::block(&mut s);
    for p in [[10.0, 15.0, 10.0], [20.0, 0.0, 5.0]] {
        s.key(Key::Escape); // the tool stays in hand after it is applied, and pressing its button again would put it down
        let hole = s.word("tb-hole-hint");
        s.press_hint(&hole);
        let at = s.face_at(p);
        s.click(at);
        s.key(Key::Enter);
    }
    s.key(Key::Escape);
    s
}

/// How much the one body of the part holds now - the body that is still there, not the ones an operation has
/// eaten on the way.
fn held(s: &mut Session) -> f64 {
    s.document().bodies.iter().find(|b| !b.consumed && !b.sheet).map(|b| b.volume).unwrap_or_else(|| panic!("the part holds no body"))
}

/// The names of the steps of the timeline, in their order.
fn steps(s: &mut Session) -> Vec<String> {
    s.document().features.iter().map(|f| f.name.clone()).collect()
}

/// The rows of the tree whose words begin as the catalogue line `key` does - one row per step of that kind, from
/// the top of the tree down.
fn rows_of(s: &mut Session, key: &str) -> Vec<Rect> {
    let line = s.word(key);
    let head = line.split(['{', '=', '\u{d7}']).next().unwrap_or(&line).trim().to_string();
    let mut found: Vec<Rect> = s.words_at().into_iter().filter(|(w, r)| w.starts_with(&head) && r.max.x < 500.0).map(|(_, r)| r).collect();
    found.sort_by(|a, b| a.min.y.total_cmp(&b.min.y));
    assert!(!found.is_empty(), "no row of the tree begins with {head:?}; on screen: {:?}", s.words());
    found
}

probe! {
    /// A ROLLBACK BUILDS THE TIMELINE ONLY UP TO THAT STEP - the steps below it are left out, and clearing the
    /// rollback brings them back. Nothing is thrown away either way.
    fn a_rollback_leaves_out_what_comes_after_it() {
        let mut s = a_block_with_two_holes();
        let drilled = held(&mut s);
        assert!(drilled < 12000.0, "two holes were drilled in a block of 12000 and it holds {drilled}");
        let first = rows_of(&mut s, "feat-hole")[0];
        build::press_in_the_menu(&mut s, first, "act-rollback-here");
        let one_hole = held(&mut s);
        assert!(one_hole > drilled && one_hole < 12000.0, "the timeline was built up to the first hole and the block holds {one_hole}: with both holes it held {drilled}, with none 12000");
        assert!(steps(&mut s).len() == 4, "the rollback threw the steps away: the timeline holds {:?}", steps(&mut s));
        let first = rows_of(&mut s, "feat-hole")[0];
        build::press_in_the_menu(&mut s, first, "act-clear-rollback");
        assert!((held(&mut s) - drilled).abs() < 1.0, "the rollback was cleared and the block holds {} instead of {drilled}", held(&mut s));
    }
}

probe! {
    /// A SUPPRESSED STEP IS LEFT OUT while the others hold, and enabling it again brings it back.
    fn a_suppressed_step_is_left_out_and_comes_back() {
        let mut s = a_block_with_two_holes();
        let drilled = held(&mut s);
        let second = rows_of(&mut s, "feat-hole")[1];
        build::press_in_the_menu(&mut s, second, "act-suppress");
        let one_hole = held(&mut s);
        assert!(one_hole > drilled && one_hole < 12000.0, "the second hole was suppressed and the block holds {one_hole}: it held {drilled} with both and 12000 with none");
        assert!(s.document().features.iter().filter(|f| f.suppressed).count() == 1, "one step was suppressed and the document marks {:?}", s.document().features.iter().map(|f| (f.name.clone(), f.suppressed)).collect::<Vec<_>>());
        let second = rows_of(&mut s, "feat-hole")[1];
        build::press_in_the_menu(&mut s, second, "act-unsuppress");
        assert!((held(&mut s) - drilled).abs() < 1.0, "the step was enabled again and the block holds {} instead of {drilled}", held(&mut s));
    }
}

probe! {
    /// TWO STEPS THAT LEAN ON THE SAME BODY AND NOT ON EACH OTHER CHANGE PLACES, and the body is what it was.
    fn two_steps_that_may_change_places_do() {
        let mut s = a_block_with_two_holes();
        // the two holes carry one name, so the order is read by which node stands where
        let order = |s: &mut Session| s.document().features.iter().map(|f| (f.name.clone(), f.key)).collect::<Vec<_>>();
        let (drilled, before) = (held(&mut s), order(&mut s));
        let first = rows_of(&mut s, "feat-hole")[0];
        build::press_in_the_menu(&mut s, first, "act-move-down");
        let after = order(&mut s);
        assert!(after != before, "the first hole was moved down and the timeline is in the same order: {before:?}");
        assert!(after.len() == before.len(), "moving a step changed how many there are: {before:?} became {after:?}");
        assert!((held(&mut s) - drilled).abs() < 1.0, "two holes changed places and the block holds {} instead of {drilled}", held(&mut s));
    }
}

probe! {
    /// A STEP THAT CANNOT MOVE ABOVE WHAT IT LEANS ON IS GREYED OUT, and the menu says why.
    fn a_step_that_cannot_change_places_is_greyed_out() {
        let mut s = a_block_with_two_holes();
        let first = rows_of(&mut s, "feat-hole")[0];
        let up = build::menu_item(&mut s, first, "act-move-up");
        assert!(!up.enabled, "the first hole leans on the block above it and the menu offers to move it up all the same");
    }
}

probe! {
    /// DELETING A STEP OTHERS LEAN ON ASKS FIRST, and what is left over is whole. What leaned on it stays, red with the
    /// reason, unless the tick asks for it to go too; with the tick nothing is left without a body.
    fn deleting_a_step_others_lean_on_asks_first() {
        let mut s = a_block_with_two_holes();
        let delete = |s: &mut qymcad::Session, with_dependents: bool| {
            let block = rows_of(s, "feat-extrude")[0];
            build::press_in_the_menu(s, block, "act-delete-feature");
            let yes = s.word("confirm-yes");
            assert!(s.shows(&yes), "a step two holes lean on was deleted and nothing asked first; on screen: {:?}", s.words());
            if with_dependents {
                let tick = s.word("confirm-with-dependents");
                s.press_word(&tick);
            }
            s.press_word(&yes);
        };
        delete(&mut s, false);
        let holes: Vec<qymcad::Feature> = s.document().features.iter().filter(|f| f.kind == "Hole").cloned().collect();
        assert!(holes.len() == 2 && holes.iter().all(|h| h.error.as_deref().is_some_and(|e| !e.is_empty())), "the block was deleted alone and the two holes did not stay red with the reason: {holes:?}");
        s.chord(Modifiers::COMMAND, Key::Z);
        delete(&mut s, true);
        let left: Vec<String> = s.document().bodies.iter().filter(|b| !b.consumed).map(|b| b.name.clone()).collect();
        assert!(left.is_empty(), "the block was deleted with what leaned on it and the part still holds {left:?}");
        // the sketch is a thing of its own and stays; the holes leaned on the body and go with it
        let leaning: Vec<String> = s.document().features.iter().filter(|f| f.kind != "Sketch").map(|f| f.name.clone()).collect();
        assert!(leaning.is_empty(), "the block was deleted with what leaned on it and the steps stayed: {leaning:?}");
        assert!(qymcad_acceptance::oracles::whole(&s.document()).is_empty(), "after the deletion the document is not whole: {:?}", qymcad_acceptance::oracles::whole(&s.document()));
    }
}

probe! {
    /// A STEP IS COPIED AND PASTED: the paste opens the step's tool with its values, a click puts the copy where it
    /// goes, Enter makes it - the timeline holds one step more and the body lost another hole.
    fn a_step_is_copied_and_pasted() {
        let mut s = a_block_with_two_holes();
        let (before, drilled) = (steps(&mut s).len(), held(&mut s));
        let second = rows_of(&mut s, "feat-hole")[1];
        s.click(second.center());
        s.copy().paste("");
        let asks = s.word("g-paste-feature");
        assert!(s.status() == asks, "the paste did not ask where the copy goes: the program says {:?}", s.status());
        let at = s.face_at([8.0, 0.0, 5.0]);
        s.click(at);
        s.key(Key::Enter);
        let after = steps(&mut s).len();
        assert!(after == before + 1, "a step was copied and pasted and the timeline holds {after} steps, it held {before}");
        assert!(held(&mut s) < drilled - 1.0, "the pasted hole took nothing out of the block: it holds {} as before", held(&mut s));
    }
}

probe! {
    /// A STEP IS OPENED AGAIN BY A DOUBLE CLICK on its row, with the numbers it was made with, and Esc leaves
    /// everything as it was.
    fn a_step_is_opened_again_by_a_double_click() {
        let mut s = a_block_with_two_holes();
        let paper = s.document();
        let second = rows_of(&mut s, "feat-hole")[1];
        s.double_click(second.center());
        assert!(!s.in_hand().is_empty(), "a double click on the row of a hole opened no command: the bar says {:?}", s.in_hand());
        s.key(Key::Escape);
        assert!(s.document() == paper, "a step was opened again and left with Esc, and the document changed");
    }
}

probe! {
    /// A SECOND HOLE GOES INTO THE SAME FACE: a plate with two holes in its top is the commonest thing there is.
    fn a_second_hole_goes_into_the_same_face() {
        let mut s = Session::start();
        build::block(&mut s);
        for p in [[10.0, 15.0, 10.0], [30.0, 15.0, 10.0]] {
            s.key(Key::Escape);
            let hole = s.word("tb-hole-hint");
            s.press_hint(&hole);
            let at = s.face_at(p);
            s.click(at);
            s.key(Key::Enter);
        }
        let holes = s.document().features.iter().filter(|f| f.kind == "Hole").count();
        assert!(holes == 2, "two holes were drilled in the top of the block and the timeline holds {holes}: the program says {:?}", s.status());
    }
}
