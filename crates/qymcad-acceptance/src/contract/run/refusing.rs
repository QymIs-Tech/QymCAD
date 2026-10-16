//! Points 4-6: a wrong pick is refused, a valid value is taken, an invalid one is refused.

use super::*;

/// 4. A WRONG CLICK IS REFUSED IN WORDS, takes nothing and leaves the document as it was.
pub(crate) fn wrong_picks(tool: &Tool) {
    let mut problems = Vec::new();
    let tries: Vec<Option<&Pick>> = std::iter::once(None).chain(tool.wrong_picks.iter().map(Some)).collect();
    for p in tries {
        let mut s = tool.fixture.start();
        take(&mut s, tool);
        let (bar, status, doc) = (s.in_hand(), s.status(), s.document());
        match p {
            Some(p) => click(&mut s, p),
            None => {
                let at = empty_space(&mut s);
                s.click(at);
            }
        }
        let what = p.map(|p| format!("{p:?}")).unwrap_or_else(|| "empty space".to_string());
        if s.status() == status {
            problems.push(format!("{what}: no words of refusal - the status line still says {status:?}"));
        }
        if s.in_hand() != bar {
            problems.push(format!("{what}: the bar changed as if something was taken: {:?}", s.in_hand()));
        }
        if shape_of(&s.document()) != shape_of(&doc) {
            problems.push(format!("{what}: the document changed"));
        }
    }
    if !tool.picks.is_empty() {
        let mut s = tool.fixture.start();
        take(&mut s, tool);
        let (status, doc) = (s.status(), shape_of(&s.document()));
        enter_key(&mut s);
        if shape_of(&s.document()) != doc {
            problems.push("Enter with nothing picked changed the document".to_string());
        }
        if s.status() == status && holds(&mut s, tool) {
            problems.push(format!("Enter with nothing picked: no words say what is missing - the status line still says {status:?}"));
        }
    }
    // MORE PICKED THAN THE TOOL NEEDS: the ordinary picks, then one more of their kind, and Enter.
    // The tool takes it, trades it for one it held, or refuses it in words; it builds nothing red, and the document
    // holds together.
    // Of the kind the ordinary picks are: on the block, a face more for faces, the upper left edge, x 0, for edges; on
    // the other documents, the one named for the tool in `MORE`.
    let (front, left) = ([20.0, 0.0, 5.0], [0.0, 15.0, 10.0]);
    let on_block = matches!(tool.fixture, super::super::fixtures::Fixture::Block | super::super::fixtures::Fixture::BlockWithHole);
    let more = if let Some((_, p)) = MORE.iter().find(|(id, _)| *id == tool.id) {
        Some(*p)
    } else if on_block && !tool.picks.is_empty() && tool.picks.iter().all(|p| matches!(p, Pick::Face(at) if *at != front)) {
        Some(Pick::Face(front))
    } else if on_block && !tool.picks.is_empty() && tool.picks.iter().all(|p| matches!(p, Pick::Edge(at) if *at != left)) {
        Some(Pick::Edge(left))
    } else {
        None
    };
    let more_at = match more {
        Some(Pick::Face(at)) | Some(Pick::Edge(at)) | Some(Pick::Vertex(at)) => at,
        _ => front,
    };
    if let (true, Some(more)) = (matches!(tool.flow, Flow::Command), more) {
        let mut s = tool.fixture.start();
        take(&mut s, tool);
        for p in tool.picks {
            click(&mut s, p);
        }
        // a tool that its one pick finishes - a sketch opened on the face - has no more to be picked
        let still_in_space = s.seen_at(more_at).is_some();
        if still_in_space {
            click(&mut s, &more);
        }
        let before = s.document().features.len();
        enter_key(&mut s);
        let doc = s.document();
        let red: Vec<String> = doc.features.iter().skip(before).filter(|f| f.error.is_some()).map(|f| format!("{} {:?}: {:?}", f.kind, f.name, f.error)).collect();
        if !red.is_empty() {
            problems.push(format!("{more:?} picked past the ordinary picks, then Enter: built red: {red:?}"));
        }
        let torn = crate::oracles::whole(&doc);
        if !torn.is_empty() {
            problems.push(format!("{more:?} picked past the ordinary picks, then Enter: the document does not hold together: {torn:?}"));
        }
    }
    // A REFERENCE DELETED AFTER IT WAS PICKED: the picks held, Ctrl+Z takes away the step that made what they name,
    // and Enter. Whatever the tool does - puts itself down, lets the picks go, or builds on what is still there - it
    // builds nothing on what is gone: no node comes in red, and the document holds together.
    if matches!(tool.flow, Flow::Command) && !tool.picks.is_empty() {
        let mut s = tool.fixture.start();
        if !s.document().undo.is_empty() {
            take(&mut s, tool);
            for p in tool.picks {
                click(&mut s, p);
            }
            let held = s.document();
            s.chord(Modifiers::COMMAND, Key::Z);
            let undone = s.document();
            // the step taken off the list takes its work with it, the tool in hand or not: the same as Ctrl+Z on the
            // same document with empty hands
            let mut bare = tool.fixture.start();
            bare.chord(Modifiers::COMMAND, Key::Z);
            if undone.undo == bare.document().undo && shape_of(&undone) != shape_of(&bare.document()) {
                problems.push(format!("picked, then Ctrl+Z: {:?} left the list and its work stayed in the document", held.undo.last()));
            }
            let before = undone.features.len();
            enter_key(&mut s);
            let doc = s.document();
            let red: Vec<String> = doc.features.iter().skip(before).filter(|f| f.error.is_some()).map(|f| format!("{} {:?}: {:?}", f.kind, f.name, f.error)).collect();
            if !red.is_empty() {
                problems.push(format!("picked, then Ctrl+Z took what was picked away, then Enter: built red on it: {red:?}"));
            }
            let torn = crate::oracles::whole(&doc);
            if !torn.is_empty() {
                problems.push(format!("picked, then Ctrl+Z, then Enter: the document does not hold together: {torn:?}"));
            }
        }
    }
    assert!(problems.is_empty(), "{}: wrong picks:\n{}", tool.id, problems.join("\n"));
}

/// The values of a field a person may give, with the text they type: the ordinary one, the ends of the range, a
/// fraction, and an expression through a parameter.
pub(crate) fn valid_inputs(f: &Field) -> Vec<(String, f64)> {
    let mut v = vec![(fmt(f.typical), f.typical), (fmt(f.lo), f.lo), (fmt(f.hi), f.hi)];
    // a value between the ordinary one and the largest, where there is room for one
    if f.typical + 0.5 <= f.hi {
        v.push((fmt(f.typical + 0.5), f.typical + 0.5));
    }
    if f.class == Class::Count {
        v.retain(|(_, x)| x.fract() == 0.0);
    }
    if f.zero && f.lo > 0.0 {
        v.push(("0".into(), 0.0));
    }
    v.push((format!("{}*2/2", fmt(f.typical)), f.typical));
    v
}

/// A number as a person types it.
pub(crate) fn fmt(x: f64) -> String {
    let s = format!("{x}");
    s.strip_suffix(".0").map(str::to_string).unwrap_or(s)
}

/// The texts a field must refuse: out of range, of the wrong form, and the two a person types by habit - a comma for
/// the point and a unit after the number - which must give the right number or be refused, never another number.
pub(crate) fn invalid_inputs(f: &Field) -> Vec<String> {
    let mut v: Vec<String> = vec!["".into(), "abc".into(), "1..2".into(), "NaN".into(), "inf".into(), "nosuchname".into(), "1/0".into(), fmt(f.hi * 10.0 + 1.0)];
    if !f.zero {
        v.push("0".into());
    }
    if !f.negative {
        // below zero: the typical value turned round, or -1 where the typical is zero itself ("-0" is zero)
        v.push(fmt(if f.typical > 0.0 { -f.typical } else { -1.0 }));
    }
    if f.class == Class::Count {
        v.push(fmt(f.typical + 0.5));
    }
    v
}

/// Type `text` into the field of words `w`, over what it holds.
pub(crate) fn type_words(s: &mut Session, w: &super::super::Words, text: &str) {
    let caption = s.word(w.caption);
    if w.by_placeholder {
        s.fill_empty(&caption, text);
    } else {
        s.fill(&caption, text);
    }
    if w.enter {
        s.key(Key::Enter);
    }
}

/// 5. EVERY ACCEPTED VALUE GIVES ITS OWN BODY.
pub(crate) fn valid_values(tool: &Tool) {
    let mut problems = Vec::new();
    for f in tool.fields {
        for (text, value) in valid_inputs(f) {
            let mut s = value_ready(tool, f, &text);
            if f.when == When::After {
                finish(&mut s, tool);
                if field_if_there(&mut s, f).is_none() {
                    problems.push(format!("{:?}: the tool opens no field to type a value into", s.word(f.caption)));
                    break;
                }
                type_into_field(&mut s, f, &text);
                enter_key(&mut s);
            } else {
                finish(&mut s, tool);
            }
            if let Err(e) = matches(&mut s, &(f.outcome)(value)) {
                problems.push(format!("{:?} = {text:?}: {e}", s.word(f.caption)));
            }
        }
    }
    // THE WORDS A PERSON TYPES, each in its own session
    for w in tool.words {
        for text in w.valid {
            let mut s = ready(tool);
            type_words(&mut s, w, text);
            finish(&mut s, tool);
            if let Err(e) = matches(&mut s, &(w.outcome)(text)) {
                problems.push(format!("{:?} = {text:?}: {e}", s.word(w.caption)));
            }
        }
    }
    let Some(f) = tool.fields.first() else {
        assert!(problems.is_empty(), "{}: accepted values:\n{}", tool.id, problems.join("\n"));
        return;
    };
    let mut s = tool.fixture.start();
    build::parameter(&mut s, "pa", &fmt(f.typical));
    take(&mut s, tool);
    if !matches!(tool.flow, Flow::Command) {
        for w in tool.words {
            type_words(&mut s, w, w.typical);
        }
    }
    if let Flow::Command = tool.flow {
        for p in tool.picks {
            click(&mut s, p);
        }
        for w in tool.words {
            type_words(&mut s, w, w.typical);
        }
    } else if let Flow::Drawing(clicks, _) = tool.flow {
        for (x, y) in clicks.iter().take(clicks.len().saturating_sub(1)) {
            s.click_on_sketch(*x, *y);
        }
    }
    if f.when == When::After {
        finish(&mut s, tool);
    }
    if field_if_there(&mut s, f).is_none() {
        problems.push(format!("{:?}: the tool opens no field to type a parameter into", s.word(f.caption)));
        assert!(problems.is_empty(), "{}: accepted values:\n{}", tool.id, problems.join("\n"));
        return;
    }
    type_into_field(&mut s, f, "pa");
    // a name typed opens the list of names to finish it with, and the first Enter takes the name from it
    enter_key(&mut s);
    if matches!(tool.flow, Flow::Command) && holds(&mut s, tool) {
        enter_key(&mut s);
    }
    if matches!(tool.flow, Flow::Drawing(..)) && f.when == When::Before {
        finish(&mut s, tool);
    }
    if let Err(e) = matches(&mut s, &(f.outcome)(f.typical)) {
        problems.push(format!("a parameter pa = {} typed as \"pa\": {e}", f.typical));
    }
    // A COMMA FOR THE POINT AND A UNIT AFTER THE NUMBER: the right number, or refused in words - never another number
    let habit = f.typical + 0.5;
    // the unit as a Russian keyboard types it: "mm" in Cyrillic
    for text in [fmt(habit).replace('.', ","), format!("{}mm", fmt(habit)), format!("{} \u{43c}\u{43c}", fmt(habit))] {
        let mut s = value_ready(tool, f, &text);
        // THE DOCUMENT AS THE FIELD STANDS OPEN is what the value is judged against: a field filled after the last click
        // may stand on what that click made - a diameter laid on a circle that had none - and that is the click's work
        if f.when == When::After {
            finish(&mut s, tool);
        }
        let before = shape_of(&s.document());
        if f.when == When::After {
            type_into_field(&mut s, f, &text);
            enter_key(&mut s);
        } else {
            finish(&mut s, tool);
        }
        let doc = s.document();
        if shape_of(&doc) != before && matches(&mut s, &(f.outcome)(habit)).is_err() {
            problems.push(format!("{text:?} gave another number without a word: {:?}", the_body(&doc).map(|b| b.volume)));
        }
    }
    assert!(problems.is_empty(), "{}: accepted values:\n{}", tool.id, problems.join("\n"));
}

/// 6. A REFUSED VALUE STOPS THE TOOL: Apply cannot be pressed, the reason is in words, the document is untouched and
///    what was typed stays in the field.
pub(crate) fn invalid_values(tool: &Tool) {
    let mut problems = Vec::new();
    for w in tool.words {
        for text in w.invalid {
            let mut s = ready(tool);
            let before = shape_of(&s.document());
            // the words may refuse as they are typed - a letter the font lacks is turned away at once - or at the
            // click; either way the status line has said something by the end
            let status = s.status();
            type_words(&mut s, w, text);
            finish(&mut s, tool);
            if shape_of(&s.document()) != before {
                problems.push(format!("{:?} = {text:?}: it was taken and the document changed", s.word(w.caption)));
            } else if s.status() == status {
                problems.push(format!("{:?} = {text:?}: refused without a word - the status line still says {status:?}", s.word(w.caption)));
            }
        }
    }
    for f in tool.fields {
        for text in invalid_inputs(f) {
            let mut s = ready(tool);
            if f.when == When::After {
                finish(&mut s, tool);
                if field_if_there(&mut s, f).is_none() {
                    problems.push(format!("{:?}: the tool opens no field to refuse a value in", s.word(f.caption)));
                    break;
                }
            }
            // as the field stands open: what Enter does with a refused value is judged, not what the last click made
            let before = shape_of(&s.document());
            type_into_field(&mut s, f, &text);
            let caption = s.word(f.caption);
            let field = field_of(&mut s, f);
            let apply = s.word("cmd-apply-enter");
            let apply_enabled = s.widgets().iter().any(|w| w.label == apply && w.enabled);
            // WHY THE VALUE IS REFUSED IS READ WHERE A PERSON READS IT: the words drawn in the box beside the field, and the
            // hint that comes up over the mark next to it. The box was not read at all: a reason said only as a tooltip
            // exists while the pointer rests on the glyph beside it, and a person reading back what they typed has
            // their eyes on the field, not on a mark they have to find first - so the words stand in the box, and a
            // check that looked for a tooltip and found none reported a tool that says nothing.
            let mut reason = words_in_the_box(&mut s, &field);
            reason.extend(s.hint_at(pos2(field.rect.max.x + 10.0, field.rect.center().y)));
            if f.when == When::Before {
                finish(&mut s, tool);
            } else {
                enter_key(&mut s);
            }
            let what = format!("{caption:?} = {text:?}");
            if apply_enabled {
                problems.push(format!("{what}: Apply can still be pressed"));
            }
            if reason.is_empty() {
                problems.push(format!("{what}: no words beside the field say why"));
            }
            if shape_of(&s.document()) != before {
                problems.push(format!("{what}: Enter changed the document"));
            }
            match field_if_there(&mut s, f) {
                Some(still) if still.value != text => problems.push(format!("{what}: the field holds {:?}, not what was typed", still.value)),
                None => problems.push(format!("{what}: the field is gone, and what was typed with it")),
                _ => {}
            }
            let _ = field;
        }
    }
    assert!(problems.is_empty(), "{}: refused values:\n{}", tool.id, problems.join("\n"));
}

/// A second value for the picture to follow: twice the typical one, or half of it where twice is out of bounds (a full
/// turn of 360 deg, a thread the whole length of its face), or 10 where the typical one is nothing (a coordinate).
pub(crate) fn other_than_typical(f: &Field) -> f64 {
    // within the field's own range: twice the ordinary value, or half of it, or the middle of the range
    let inside = |v: f64| v >= f.lo && v <= f.hi;
    if f.typical == 0.0 {
        f.hi.min(10.0)
    } else if inside(f.typical * 2.0) {
        f.typical * 2.0
    } else if inside(f.typical / 2.0) {
        f.typical / 2.0
    } else {
        0.5 * (f.lo + f.hi)
    }
}
