//! Points 12-15: saving and opening, reopening, a change above it and what depends on it.

use super::*;

/// A file of this check's own to save into.
pub(crate) fn scratch(tool: &Tool, what: &str) -> String {
    let dir = std::env::temp_dir().join(format!("qymcad-contract-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("the scratch folder is made");
    dir.join(format!("{}-{what}.qcad", tool.id.replace('.', "-"))).to_string_lossy().into_owned()
}

/// 12. SAVED AND OPENED, THE NODE AND THE BODY ARE THE SAME: names, numbers and the names of the faces.
pub(crate) fn save_open(tool: &Tool) {
    let mut s = ready(tool);
    finish_all(&mut s, tool);
    // a question up holds the window until it is answered: saving is what one of its answers does, not a step past it
    if matches!(tool.result, Outcome::Window { .. }) {
        return;
    }
    let doc = s.document();
    let path = scratch(tool, "saved");
    build::save_as(&mut s, &path);
    drop(s);
    let mut t = Session::start();
    build::open_project(&mut t, &path);
    let opened = t.document();
    let faces = |d: &Document| d.bodies.iter().map(|b| (b.name.clone(), b.face_names.clone())).collect::<Vec<_>>();
    let mut problems = Vec::new();
    if shape_of(&opened) != shape_of(&doc) {
        problems.push(format!("the document opened is {:?}, the one saved was {:?}", shape_of(&opened), shape_of(&doc)));
    }
    if faces(&opened) != faces(&doc) {
        problems.push("the faces are named otherwise after opening".to_string());
    }
    let _ = std::fs::remove_file(&path);
    assert!(problems.is_empty(), "{}: save and open:\n{}", tool.id, problems.join("\n"));
}

/// 13. A DOUBLE CLICK ON THE NODE OPENS THE TOOL WITH ITS VALUES; a changed value moves the body; Esc leaves it as it
///     was.
pub(crate) fn reopen(tool: &Tool) {
    // made with a value of its own, not the one the tool starts with: a field that only shows its default would pass.
    // One more than the ordinary, or one less when the ordinary is already the most the tool takes - a revolution
    // starts at a whole turn, and 361 degrees would be a value out of range rather than a value of its own.
    // a tool that takes no value is reopened all the same: the double click must put it back in hand with its
    // picks, and Esc must leave the document as it was
    let Some(f) = tool.fields.first() else {
        let mut s = ready(tool);
        finish(&mut s, tool);
        let made = shape_of(&s.document());
        let row = row_of(&mut s, tool, &Node { kind: tool.node, row: Some(tool.title) });
        s.double_click(row);
        let mut problems = Vec::new();
        if !holds(&mut s, tool) {
            problems.push(format!("the double click did not open the tool; bar: {:?}", s.in_hand()));
        }
        s.key(Key::Escape);
        if shape_of(&s.document()) != made {
            problems.push("reopened and left with Esc, the document changed".to_string());
        }
        assert!(problems.is_empty(), "{}: reopening:\n{}", tool.id, problems.join("\n"));
        return;
    };
    let made = if f.typical + 1.0 <= f.hi { f.typical + 1.0 } else { (f.typical - 1.0).max(f.lo) };
    let mut s = value_ready(tool, f, &fmt(made));
    finish_with(&mut s, tool, f, &fmt(made));
    let row = row_of(&mut s, tool, &Node { kind: tool.node, row: Some(tool.title) });
    s.double_click(row);
    let mut problems = Vec::new();
    if !holds(&mut s, tool) {
        problems.push(format!("the double click did not open the tool; bar: {:?}", s.in_hand()));
    } else {
        let caption = s.word(f.caption);
        let held = s.field(&caption).value;
        if held.parse::<f64>().ok() != Some(made) {
            problems.push(format!("the reopened field holds {held:?}, the node was made with {made}"));
        }
        // twice the ordinary value, kept inside the range the tool accepts: doubling a negative offset would walk out
        // of it, and a value the tool refuses says nothing about reopening
        let other = (f.typical * 2.0).clamp(f.lo, f.hi);
        type_into_field(&mut s, f, &fmt(other));
        enter_key(&mut s);
        if let Err(e) = matches(&mut s, &(f.outcome)(other)) {
            problems.push(format!("changed to {other}: {e}"));
        }
        let changed = shape_of(&s.document());
        let row = row_of(&mut s, tool, &Node { kind: tool.node, row: Some(tool.title) });
        s.double_click(row);
        type_into_field(&mut s, f, &fmt(f.typical));
        s.key(Key::Escape);
        if shape_of(&s.document()) != changed {
            problems.push("reopened, changed and left with Esc, the document changed".to_string());
        }
    }
    assert!(problems.is_empty(), "{}: reopening:\n{}", tool.id, problems.join("\n"));
}

/// 14. A CHANGE ABOVE IN THE TIMELINE REBUILDS THE NODE with the right numbers.
pub(crate) fn upstream(tool: &Tool) {
    let change = tool.upstream.as_ref().unwrap_or_else(|| panic!("{}: the description gives no change above the tool in the timeline, and does not say why not", tool.id));
    let mut s = ready(tool);
    finish_all(&mut s, tool);
    let (node, then) = match change {
        Upstream::Reopen { node, then, .. } => (*node, then),
        Upstream::SketchDimension { then, .. } => (Node { kind: "Sketch", row: None }, then),
    };
    let row = row_of(&mut s, tool, &node);
    s.double_click(row);
    match change {
        Upstream::Reopen { caption, value, .. } => {
            let caption = s.word(caption);
            s.fill(&caption, &fmt(*value));
            enter_key(&mut s);
        }
        Upstream::SketchDimension { shown, value, .. } => {
            let canvas = s.canvas();
            let label = s.words_at().into_iter().find(|(w, r)| w == shown && canvas.contains(r.center())).map(|(_, r)| r.center());
            let label = label.unwrap_or_else(|| panic!("{}: the dimension {shown:?} is not on the sketch; on screen: {:?}", tool.id, s.words()));
            s.double_click(label).chord(Modifiers::COMMAND, Key::A).type_text(&fmt(*value)).key(Key::Enter);
            let finish = s.word("wb-finish");
            s.press_word_near(&finish, TOP);
        }
    }
    let doc = s.document();
    if let Err(e) = matches(&mut s, then) {
        panic!("{}: after the change above: {e}; the timeline: {:?}", tool.id, doc.features);
    }
    by_a_parameter(tool);
}

/// THE FIRST VALUE GIVEN BY A PARAMETER, and the parameter changed and deleted later: `k` holds the ordinary value and
/// the field reads `k`; `k` changed, the node follows it; `k` deleted, the node turns red naming it and keeps its last
/// body. Every tool with a field, whether it is typed before the tool is finished or after (a dimension).
pub(crate) fn by_a_parameter(tool: &Tool) {
    let Some(f) = tool.fields.first().filter(|_| !matches!(tool.flow, Flow::Action)) else { return };
    let mut s = ready_after(tool, |s| build::parameter(s, "k", &fmt(f.typical)));
    if f.when == When::Before {
        type_into_field(&mut s, f, "k");
    }
    finish_with(&mut s, tool, f, "k");
    if !tool.stays && holds(&mut s, tool) {
        enter_key(&mut s); // the first Enter took the name from the list of names
    }
    if let Err(e) = matches(&mut s, &(f.outcome)(f.typical)) {
        panic!("{}: the field given k = {}: {e}", tool.id, fmt(f.typical));
    }
    s.key(Key::Escape);
    let other = other_than_typical(f);
    build::set_parameter(&mut s, "k", &fmt(other));
    if let Err(e) = matches(&mut s, &(f.outcome)(other)) {
        panic!("{}: k changed to {} after the node was laid: {e}", tool.id, fmt(other));
    }
    build::delete_the_parameter(&mut s);
    let doc = s.document();
    let node = doc.features.iter().rev().find(|x| x.kind == tool.node);
    if !node.is_some_and(|n| n.error.as_deref().is_some_and(|e| e.contains('k'))) {
        panic!("{}: k deleted from under the field, and the node says {:?}", tool.id, node.map(|n| &n.error));
    }
}

/// 15. DELETING WHAT THE NODE STANDS ON TURNS IT RED WITH A REASON; the program keeps working; undo gives it all back.
pub(crate) fn dependency(tool: &Tool) {
    let node = tool.dependency.unwrap_or_else(|| panic!("{}: the description names nothing the tool stands on, and does not say why", tool.id));
    let mut s = ready(tool);
    finish_all(&mut s, tool);
    // A SKETCH LEFT OPEN by the tool (the pencil opens one on the face) is finished first: the tree stands hidden while
    // a sketch is edited, and a person leaves the sketch to reach it
    if s.document().editing.is_some() {
        let finish = s.word("wb-finish");
        s.press_word_near(&finish, pos2(0.0, 0.0));
    }
    let built = shape_of(&s.document());
    let row = row_of(&mut s, tool, &node);
    let name = node.kind;
    s.click(row).key(Key::Delete);
    let yes = s.word("confirm-yes");
    if let Some(r) = s.find(&yes, pos2(640.0, 400.0)) {
        s.click(r.center());
    }
    let doc = s.document();
    let mut problems = Vec::new();
    // the node the tool made is the last of its kind: a sketch on a face stands after the block's own sketch
    let mine = doc.features.iter().rfind(|f| f.kind == tool.node);
    match mine {
        None => problems.push(format!("deleting the {name} node took this one away with it: {:?}", doc.features)),
        Some(f) if f.error.as_deref().is_none_or(str::is_empty) => problems.push(format!("the node stands on nothing and is not red with a reason: {f:?}")),
        _ => {}
    }
    s.chord(Modifiers::COMMAND, Key::Z);
    if shape_of(&s.document()) != built {
        problems.push(format!("undo did not give everything back: {:?}", shape_of(&s.document())));
    }
    assert!(problems.is_empty(), "{}: deleting what it stands on:\n{}", tool.id, problems.join("\n"));
}
