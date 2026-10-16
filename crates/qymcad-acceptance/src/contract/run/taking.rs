//! Points 1-3: the tool is taken, its bar stands, and it picks what it should.

use super::*;

/// THE ROW OF `node` IN THE TREE: the row beginning with its word, or with its own name.
pub(crate) fn row_of(s: &mut Session, tool: &Tool, node: &Node) -> Pos2 {
    let doc = s.document();
    let lead = match node.row {
        Some(w) => s.word(w),
        None => doc.features.iter().find(|f| f.kind == node.kind).map(|f| f.name.clone()).unwrap_or_else(|| panic!("{}: no {} node in the timeline: {:?}", tool.id, node.kind, doc.features)),
    };
    let left = s.canvas().min.x;
    let mut rows: Vec<(String, qymcad::Rect)> = s.words_at().into_iter().filter(|(w, r)| r.max.x < left && w.starts_with(&lead)).collect();
    // a node the tree names by what it did - a boolean that cut is "Body cut" under the bar's "Body boolean" - is
    // found by the name the document gives it, when the word asked for heads no row
    if rows.is_empty() {
        if let Some(name) = doc.features.iter().find(|f| f.kind == node.kind).map(|f| f.name.clone()) {
            rows = s.words_at().into_iter().filter(|(w, r)| r.max.x < left && w.starts_with(&name)).collect();
        }
    }
    match rows.as_slice() {
        [(_, r)] => r.center(),
        other => panic!("{}: the row of the {} node, beginning {lead:?}, is not one row of the tree: {other:?}", tool.id, node.kind),
    }
}

/// TOOLS WHOSE BUTTON, PRESSED AGAIN WITH GEOMETRY SELECTED, MOVES ON RATHER THAN PUTTING THE TOOL DOWN - by design: the
/// mirror of a sketch takes what is selected and asks for the axis ("what, then about what"). Without a selection its
/// button puts it down as any other.
pub(crate) const PRESS_MOVES_ON: &[&str] = &["sketch.mirror"];

/// 1. EVERY WAY IN TAKES THE TOOL; its button a second time puts it down; taken over another tool, it puts that one
///    down.
pub(crate) fn entry(tool: &Tool) {
    let mut problems = Vec::new();
    for e in tool.entries {
        let mut s = tool.fixture.start();
        preselect(&mut s, tool);
        // THE HAND AT REST - the arrow of the sketch is in hand before any tool is taken, and pressing it again keeps it,
        // as in the professional systems: there is nothing to put down to. Taking it over another tool is checked below
        let at_rest = !matches!(tool.flow, Flow::Action) && in_hand(&mut s, tool);
        enter_for(&mut s, tool, e);
        // an action is not taken: each way in must do its work on what the fixture selected, with the numbers it asks
        // for typed - a mate taken by its row is turned by the angle typed into it
        if matches!(tool.flow, Flow::Action) {
            finish_all(&mut s, tool);
            if let Err(e2) = matches(&mut s, &tool.result) {
                problems.push(format!("{} did not do the work: {e2}", describe(e)));
            }
            continue;
        }
        if !holds(&mut s, tool) {
            problems.push(format!("{} did not take the tool; bar: {:?}", describe(e), s.in_hand()));
            continue;
        }
        if !in_hand(&mut s, tool) {
            problems.push(format!("{} took the tool, but the bar does not name it: {:?}", describe(e), s.in_hand()));
        }
        if let (Entry::Button(hint), false) = (e, at_rest || PRESS_MOVES_ON.contains(&tool.id)) {
            // a button that stays pressed may speak of itself another way while its tool is in hand ("click a part
            // ... Esc leaves"): it is pressed again where it stands, as a person presses it
            let word = s.word(hint);
            match s.find_hint(&word) {
                Some(at) => {
                    s.click(at);
                }
                None => {
                    let at = pressed_at(tool, e);
                    s.click(at);
                }
            }
            if in_hand(&mut s, tool) {
                problems.push(format!("{} a second time did not put the tool down; bar: {:?}", describe(e), s.in_hand()));
            }
        }
    }
    let mut s = tool.fixture.start();
    enter(&mut s, &tool.other.0);
    take(&mut s, tool);
    let other = s.word(tool.other.1);
    let bar = s.in_hand();
    // an action done with another tool in hand leaves that tool where it was: there is nothing to put it down
    if !matches!(tool.flow, Flow::Action) && (!in_hand(&mut s, tool) || bar.contains(&other)) {
        problems.push(format!("taken over {other:?}, the bar says {bar:?}"));
    }
    assert!(problems.is_empty(), "{}: taking the tool:\n{}", tool.id, problems.join("\n"));
}

/// Where the button of `e` stands before anything is taken: found on a fresh fixture.
pub(crate) fn pressed_at(tool: &Tool, e: &Entry) -> Pos2 {
    let Entry::Button(hint) = e else { panic!("{}: only a button stands somewhere", tool.id) };
    let mut s = tool.fixture.start();
    let word = s.word(hint);
    s.find_hint(&word).unwrap_or_else(|| panic!("{}: no button has the hint {word:?}", tool.id))
}

/// A way in, in words for a report.
pub(crate) fn describe(e: &Entry) -> String {
    match e {
        Entry::Button(h) => format!("the button {h}"),
        Entry::Key(m, k) => format!("the key {m:?} {k:?}"),
        Entry::Search(n) => format!("the command search for {n}"),
        Entry::SearchByArticle => "the command search by the title of its article".to_string(),
        Entry::Menu(p) => format!("the menu {p:?}"),
        Entry::Label(w) => format!("the label {w}"),
    }
}

/// 2. EVERY MODE OF THE BAR MAKES WHAT IT IS DESCRIBED TO MAKE - or refuses in words - and going through a group
///    and back leaves no mode hanging.
pub(crate) fn bar(tool: &Tool) {
    let mut problems = Vec::new();
    for mode in tool.modes.iter().flat_map(|g| g.iter()) {
        // THE MODE IS CHOSEN BEFORE THE WORK, as a person chooses it: changing it half-way through a drawing starts the
        // drawing again
        let mut s = tool.fixture.start();
        take(&mut s, tool);
        let (word, status, before) = (s.word(mode.word), s.status(), shape_of(&s.document()));
        s.press_word_near(&word, TOP);
        work(&mut s, tool, mode.clicks);
        for f in tool.fields.iter().filter(|f| f.when == When::After) {
            if field_if_there(&mut s, f).is_some() {
                type_into_field(&mut s, f, &fmt(f.typical));
                enter_key(&mut s);
            }
        }
        let doc = s.document();
        match &mode.outcome {
            Some(want) => {
                if let Err(e) = matches(&mut s, want) {
                    problems.push(format!("mode {word:?}: {e}"));
                }
            }
            None => {
                if shape_of(&doc) != before {
                    problems.push(format!("mode {word:?} must refuse and changed the document: {:?}", shape_of(&doc)));
                }
                if s.status() == status {
                    problems.push(format!("mode {word:?} must refuse in words and the status line still says {status:?}"));
                }
            }
        }
    }
    for group in tool.modes {
        let mut s = tool.fixture.start();
        take(&mut s, tool);
        for mode in group.iter().skip(1) {
            let word = s.word(mode.word);
            s.press_word_near(&word, TOP);
        }
        // a switch goes back with a second press; an option of a group, with its first option
        let word = s.word(group[0].word);
        s.press_word_near(&word, TOP);
        if group.len() == 1 {
            s.press_word_near(&word, TOP);
        }
        work(&mut s, tool, None);
        for f in tool.fields.iter().filter(|f| f.when == When::After) {
            if field_if_there(&mut s, f).is_some() {
                type_into_field(&mut s, f, &fmt(f.typical));
                enter_key(&mut s);
            }
        }
        if let Err(e) = matches(&mut s, &tool.result) {
            problems.push(format!("the group {:?} gone through and back: {e}", group.iter().map(|m| m.word).collect::<Vec<_>>()));
        }
    }
    assert!(problems.is_empty(), "{}: the modes of the bar:\n{}", tool.id, problems.join("\n"));
}

/// 3. A PICK IS TAKEN, LIT AND NAMED IN THE BAR; clicked again, it is let go.
pub(crate) fn picks(tool: &Tool) {
    let mut problems = Vec::new();
    for p in tool.pick_trial {
        let mut s = tool.fixture.start();
        take(&mut s, tool);
        // the place of the pick is found first: finding it may turn the view to show it, and a picture taken before
        // that turn differs from every picture after it by the turn alone
        aim(&mut s, p);
        let (bar, picture, mut fields) = (s.in_hand(), look(&mut s), fields_at(&mut s));
        click(&mut s, p);
        let (bar_taken, picture_taken, status) = (s.in_hand(), look(&mut s), s.status());
        fields.extend(fields_at(&mut s));
        if bar_taken == bar {
            problems.push(format!("{p:?}: the bar does not name what was taken: {bar_taken:?}; the status line says {status:?}"));
        }
        let canvas = s.canvas();
        if !crate::golden::changed_in(&picture, &picture_taken, canvas, &fields) {
            problems.push(format!("{p:?}: nothing on the canvas lit up"));
        }
        click(&mut s, p);
        let status = s.status();
        if s.in_hand() != bar {
            problems.push(format!("{p:?} clicked again: the bar still says {:?}, it said {bar:?} before", s.in_hand()));
        }
        let again = look(&mut s);
        fields.extend(fields_at(&mut s));
        if crate::golden::changed_in(&picture, &again, canvas, &fields) {
            problems.push(format!("{p:?} clicked again: the canvas did not go back to how it was; the status line says {status:?}"));
        }
    }
    assert!(problems.is_empty(), "{}: picks:\n{}", tool.id, problems.join("\n"));
}

/// ONE MORE PICK THAN THE ORDINARY ONES, for the tools whose documents are not the plain block: a face of the part
/// already taken, a third face for two, a vertex past the measured pair. A joint has none: its second click lays it,
/// and what stands over the part then is the choice of its anchor, not the model.
pub(crate) const MORE: &[(&str, Pick)] = &[
    ("assembly.ground", Pick::Face([20.0, 0.0, 5.0])),
    ("assembly.mirror-part", Pick::Face([20.0, 15.0, 10.0])),
    ("assembly.group", Pick::Face([20.0, 0.0, 5.0])),
    ("assembly.section", Pick::Face([20.0, 0.0, 5.0])),
    ("assembly.tangent", Pick::Face([20.0, 15.0, 10.0])),
    ("assembly.width", Pick::Face([25.0, 10.0, 5.0])),
    ("part.circular-array", Pick::Face([220.0, -10.0, 5.0])),
    ("part.draft", Pick::Face([40.0, 15.0, 5.0])),
    ("part.boolean", Pick::Face([20.0, 0.0, 7.5])),
    ("part.thread", Pick::Face([20.0, 15.0, 10.0])),
    ("part.thread-own-pitch", Pick::Face([20.0, 15.0, 10.0])),
    ("part.replace-face", Pick::Face([40.0, 15.0, 5.0])),
    ("part.trim-surface", Pick::Face([35.0, 25.0, 10.0])),
    ("part.measure-3d", Pick::Vertex([40.0, 30.0, 10.0])),
];
