//! Points 7-11: the preview, Enter, Esc, the result and the step of undo.

use super::*;

/// 7. THE RESULT IS SHOWN BEFORE ENTER, follows the value, and the document does not hold it yet.
pub(crate) fn preview(tool: &Tool) {
    let mut s = tool.fixture.start();
    let (doc, picture) = (shape_of(&s.document()), look(&mut s));
    drop(s);
    // everything up to the step that finishes the tool: its picks, or all the clicks but the last
    let mut s = ready(tool);
    // A CHAIN LAYS WHAT EACH CLICK FINISHES: the first segment of a chain of lines is in the document once its end is
    // clicked, one undo step each; what must not be there yet is what the last click makes
    let doc = if matches!(tool.flow, Flow::Drawing(..)) { shape_of(&s.document()) } else { doc };
    let (shown, followed, fields) = match tool.flow {
        Flow::Command => match tool.fields.first() {
            Some(f) => {
                type_into_field(&mut s, f, &fmt(f.typical));
                let shown = look(&mut s);
                let fields = fields_at(&mut s);
                type_into_field(&mut s, f, &fmt(other_than_typical(f)));
                (shown, look(&mut s), fields)
            }
            // a tool that takes no value - a mirror, a copy - shows its result as soon as its picks are made, and
            // there is nothing for the picture to follow
            None => {
                let shown = look(&mut s);
                let fields = fields_at(&mut s);
                (shown.clone(), shown, fields)
            }
        },
        // A DRAWING FOLLOWS THE POINTER: the rubber band is drawn where the last click would go, so the pointer stays
        // where it is for the picture - putting it aside would take the drawing with it
        Flow::Drawing(clicks, _) => {
            let (last, half) = (clicks[clicks.len() - 1], clicks[0]);
            let at = s.on_sketch(last.0, last.1);
            s.move_to(at);
            let shown = s.snapshot();
            let fields = fields_at(&mut s);
            // a tool of one click has no first click to go half-way to: the pointer goes 10 mm aside
            let midway = if clicks.len() == 1 { s.on_sketch(last.0 + 10.0, last.1 + 5.0) } else { s.on_sketch((last.0 + half.0) / 2.0, (last.1 + half.1) / 2.0) };
            s.move_to(midway);
            (shown, s.snapshot(), fields)
        }
        Flow::Action => {
            let shown = look(&mut s);
            let fields = fields_at(&mut s);
            (shown.clone(), shown, fields)
        }
    };
    let mut problems = Vec::new();
    let canvas = s.canvas();
    if !crate::golden::changed_in(&picture, &shown, canvas, &fields) {
        problems.push("nothing new is drawn before Enter".to_string());
    }
    let follows = tool.fields.first().is_some_and(|f| f.class != Class::Tolerance);
    if follows && !crate::golden::changed_in(&shown, &followed, canvas, &fields) {
        problems.push(match tool.flow {
            Flow::Command => "the picture did not follow the value typed into the field".to_string(),
            Flow::Drawing(..) => "the picture did not follow the pointer before the click that makes the shape".to_string(),
            Flow::Action => "an action has nothing to follow".to_string(),
        });
    }
    if shape_of(&s.document()) != doc {
        problems.push("the document holds the result before Enter".to_string());
    }
    assert!(problems.is_empty(), "{}: preview:\n{}", tool.id, problems.join("\n"));
}

/// 8. ENTER AND THE CHECK BUTTON APPLY: one node of the timeline more, and the tool put down or kept as described.
pub(crate) fn apply(tool: &Tool) {
    let mut problems = Vec::new();
    for by_button in [false, true] {
        let mut s = ready(tool);
        let before = s.document().features.len();
        let how = match (tool.flow, by_button) {
            (Flow::Command, true) => {
                let word = s.word("cmd-apply-enter");
                s.press_word_near(&word, TOP);
                "the Apply button"
            }
            (Flow::Command, false) => {
                enter_key(&mut s);
                "Enter"
            }
            (Flow::Drawing(..), true) => continue, // a drawing has no button to apply with: the last click is what makes it
            (Flow::Drawing(..), false) => {
                finish_all(&mut s, tool);
                "the last click"
            }
            (Flow::Action, true) => continue, // an action has no button to apply with: its way in is what does it
            (Flow::Action, false) => {
                finish_all(&mut s, tool);
                "the way in"
            }
        };
        let doc = s.document();
        let new: Vec<&str> = doc.features.iter().skip(before).map(|f| f.kind.as_str()).collect();
        // a command that changes the assembly rather than a part's timeline (grounding) names no node, and adds none
        let want: &[&str] = if tool.node.is_empty() { &[] } else { std::slice::from_ref(&tool.node) };
        if matches!(tool.flow, Flow::Command) && new != want {
            problems.push(format!("{how} added {new:?} to the timeline, not one {:?}", tool.node));
        }
        if let Err(e) = matches(&mut s, &tool.result) {
            problems.push(format!("{how}: {e}"));
        }
        if holds(&mut s, tool) != tool.stays {
            problems.push(format!("{how}: the tool is {} in hand", if tool.stays { "not" } else { "still" }));
        }
    }
    assert!(problems.is_empty(), "{}: applying:\n{}", tool.id, problems.join("\n"));
}

/// 9. Esc, THE CROSS AND ANOTHER TOOL CANCEL WITHOUT A TRACE: the document, the bar and the status line as before.
pub(crate) fn cancel(tool: &Tool) {
    let mut problems = Vec::new();
    // a drawing has no button to cancel with: Esc and another tool are its ways
    let ways: &[&str] = if matches!(tool.flow, Flow::Command) { &["Esc", "Cancel", "another tool"] } else { &["Esc", "another tool"] };
    for how in ways {
        let how = *how;
        let mut s = tool.fixture.start();
        let (mut doc, bar) = (shape_of(&s.document()), s.in_hand());
        take(&mut s, tool);
        for p in tool.picks {
            click(&mut s, p);
        }
        if let (Flow::Command, Some(f)) = (tool.flow, tool.fields.first()) {
            type_into_field(&mut s, f, &fmt(f.typical + 1.0));
        }
        // A SHAPE IN THE MAKING, to have something to cancel: every click of a drawing but the last. What a chain of
        // lines has laid down by then is its own matter (one figure, one step); what is cancelled is what is still
        // being drawn, so the document is taken from here.
        if let Flow::Drawing(places, finish) = tool.flow {
            for (x, y) in places.iter().take(places.len().saturating_sub(1)) {
                s.click_on_sketch(*x, *y);
            }
            // a shape PLACED by its last click has laid nothing down before it - what the earlier clicks show (a
            // dimension's provisional length) is the shape being made, and cancelling takes it away too
            if !matches!(finish, Finish::Placed) {
                doc = shape_of(&s.document());
            }
        }
        match how {
            // THE Esc LADDER: with a value just typed the focus is in a field, and the first Esc takes it out of the
            // field (a list of names open is closed first); the command goes at the next. At most three steps
            "Esc" => {
                for _ in 0..3 {
                    s.key(Key::Escape);
                    if !holds(&mut s, tool) {
                        break;
                    }
                }
            }
            "Cancel" => {
                let word = s.word("cmd-cancel-esc");
                s.press_word_near(&word, TOP);
            }
            _ => enter(&mut s, &tool.other.0),
        }
        if how == "another tool" {
            let other = s.word(tool.other.1);
            if in_hand(&mut s, tool) || !s.in_hand().contains(&other) {
                problems.push(format!("{how}: the bar says {:?}", s.in_hand()));
            }
            s.key(Key::Escape);
        } else if matches!(tool.flow, Flow::Command) && s.in_hand() != bar {
            problems.push(format!("{how}: the bar says {:?}, it said {bar:?} before the tool", s.in_hand()));
        }
        // WHAT WAS CANCELLED CANNOT BE FINISHED: the click that would have made the shape makes none of it now - it
        // begins a new one at most
        if let (true, Flow::Drawing([_, .., (x, y)], Finish::LastClick | Finish::Placed | Finish::EnterAt(_))) = (how == "Esc", tool.flow) {
            s.click_on_sketch(*x, *y);
        }
        if shape_of(&s.document()) != doc {
            problems.push(format!("{how}: the document changed"));
        }
    }
    assert!(problems.is_empty(), "{}: cancelling:\n{}", tool.id, problems.join("\n"));
}

/// 10. THE ORDINARY RESULT HAS THE DESCRIBED NUMBERS.
pub(crate) fn result(tool: &Tool) {
    let mut s = ready(tool);
    finish_all(&mut s, tool);
    let doc = s.document();
    if let Err(e) = matches(&mut s, &tool.result) {
        panic!("{}: the result: {e}; the timeline: {:?}", tool.id, doc.features);
    }
    // a chooser up waits for the system to answer, as a file dialog does: nothing else is done until it is answered,
    // so the round trips belong to the file it writes or reads, not to the chooser
    // and so does a question up - "save the changes?" - which holds the window until it is answered: the menus behind
    // it are not reached, and the round trips are those of what the answer leads to
    if matches!(tool.result, Outcome::Chooser { .. } | Outcome::Window { .. }) {
        return;
    }
    // THE ROUND TRIPS OF THE ORACLES on the result: undo and redo, rebuilding everything, saving and opening
    let problems: Vec<String> =
        [crate::oracles::undo_redo(&mut s), crate::oracles::rebuild_everything(&mut s), crate::oracles::save_open(&mut s, &scratch(tool, "round-trip"))].into_iter().filter_map(Result::err).collect();
    assert!(problems.is_empty(), "{}: the result does not survive its round trips:\n{}", tool.id, problems.join("\n"));
}

/// 11. ONE STEP OF UNDO NAMED AFTER THE TOOL; undo gives back the document before it, redo the one after.
pub(crate) fn undo(tool: &Tool) {
    let mut s = tool.fixture.start();
    // an action does its work as it is taken, so the document before it is read before it is taken
    let early = matches!(tool.flow, Flow::Action).then(|| (shape_of(&s.document()), s.document().undo.len()));
    take(&mut s, tool);
    // the words a drawing needs to make anything - the string of a text - are typed first, as `ready` types them
    if matches!(tool.flow, Flow::Drawing(..)) {
        for w in tool.words {
            type_words(&mut s, w, w.typical);
        }
    }
    let (before, steps) = early.unwrap_or_else(|| (shape_of(&s.document()), s.document().undo.len()));
    match tool.flow {
        Flow::Command => {
            for p in tool.picks {
                click(&mut s, p);
            }
        }
        Flow::Drawing(clicks, _) => {
            for (x, y) in clicks.iter().take(clicks.len().saturating_sub(1)) {
                s.click_on_sketch(*x, *y);
            }
        }
        Flow::Action => {}
    }
    finish_all(&mut s, tool);
    let doc = s.document();
    let after = shape_of(&doc);
    let mut problems = Vec::new();
    let name = s.word(tool.undo);
    let new: Vec<&String> = doc.undo.iter().skip(steps).collect();
    if new.len() != tool.undo_steps || new.iter().any(|n| **n != name) {
        problems.push(format!("using the tool added the steps {new:?}, not {} named {name:?}", tool.undo_steps));
    }
    for _ in 0..tool.undo_steps {
        s.chord(Modifiers::COMMAND, Key::Z);
    }
    if shape_of(&s.document()) != before {
        problems.push(format!("undo did not give back the document before the tool: {:?}", shape_of(&s.document())));
    }
    for _ in 0..tool.undo_steps {
        s.chord(Modifiers::COMMAND, Key::Y);
    }
    if shape_of(&s.document()) != after {
        problems.push(format!("redo did not give back the document after the tool: {:?}", shape_of(&s.document())));
    }
    assert!(problems.is_empty(), "{}: undo and redo:\n{}", tool.id, problems.join("\n"));
}
