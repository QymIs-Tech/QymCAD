//! The steps a point is made of: take the tool, aim, click, type into the field, press Enter.

use super::*;

/// TAKE THE TOOL the first way the description gives - for an action, on the selection its picks make first.
pub(crate) fn take(s: &mut Session, tool: &Tool) {
    preselect(s, tool);
    enter_for(s, tool, tool.entries.first().unwrap_or_else(|| panic!("{}: the description gives no way to take the tool", tool.id)));
}

/// THE SELECTION AN ACTION WORKS ON, made before its way in, as a person makes it: its picks in turn, each after the
/// first added with Shift. A constraint is two lines picked and then its button.
pub(crate) fn preselect(s: &mut Session, tool: &Tool) {
    if !matches!(tool.flow, Flow::Action) {
        return;
    }
    for (n, p) in tool.picks.iter().enumerate() {
        match *p {
            Pick::Sketch(x, y) => build::pick(s, x, y, n > 0),
            _ => click(s, p),
        }
    }
}

/// Take a tool by `entry`.
pub(crate) fn enter_for(s: &mut Session, tool: &Tool, entry: &Entry) {
    match entry {
        Entry::SearchByArticle => {
            let name = crate::completeness::article_title(&s.language(), tool.help);
            s.chord(Modifiers::COMMAND, Key::K).type_text(&name).key(Key::Enter);
        }
        other => enter(s, other),
    }
}

/// Take a tool by `entry`, where the entry says all there is to know about it.
pub(crate) fn enter(s: &mut Session, entry: &Entry) {
    match entry {
        Entry::Button(hint) => {
            let hint = s.word(hint);
            s.press_hint(&hint);
        }
        Entry::Key(modifiers, key) => {
            s.chord(*modifiers, *key);
        }
        Entry::Search(name) => {
            let name = s.word(name);
            s.chord(Modifiers::COMMAND, Key::K).type_text(&name).key(Key::Enter);
        }
        Entry::Menu(path) => {
            let words: Vec<String> = path.iter().map(|k| s.word(k)).collect();
            s.menu(&words.iter().map(String::as_str).collect::<Vec<_>>());
        }
        Entry::Label(word) => {
            let word = s.word(word);
            s.press_word(&word);
        }
        // the name comes from the article of the tool, which this way in does not carry - `enter_for` knows it
        Entry::SearchByArticle => panic!("the way in by the article's name needs the tool: take it with `enter_for`"),
    }
}

/// Does the bar name the tool in hand by its own word? This is the naming, checked by the way in (point 1).
pub(crate) fn in_hand(s: &mut Session, tool: &Tool) -> bool {
    let title = s.word(tool.title);
    s.in_hand().contains(&title)
}

/// Is the tool in hand at all? Its own word in the bar says so; a command whose bar heads it with a generic word is
/// still in hand, and its bar says that by the pair of buttons every command bar carries; a tool of the sketch, by the
/// dash its bar is headed with. The points other than the
/// way in ask this, not the naming: a bar that does not name its tool is one defect, not nineteen.
pub(crate) fn holds(s: &mut Session, tool: &Tool) -> bool {
    if in_hand(s, tool) {
        return true;
    }
    let (apply, cancel) = (s.word("cmd-apply-enter"), s.word("cmd-cancel-btn"));
    let bar = s.in_hand();
    match tool.flow {
        Flow::Command => bar.contains(&apply) && bar.contains(&cancel),
        // a tool of the sketch whose bar heads it with a dash in place of its name is in hand all the same
        Flow::Drawing(..) => bar.first().is_some_and(|w| w == "\u{2014}"),
        // an action holds nothing: its way in did the work
        Flow::Action => false,
    }
}

/// CLICK `pick` where the window drew it.
/// Find where the pick `pick` lands, turning the view to show it if it must, without clicking.
pub(crate) fn aim(s: &mut Session, pick: &Pick) {
    let _ = match *pick {
        Pick::Face(p) => s.face_at(p),
        Pick::Edge(p) => s.edge_at(p),
        Pick::Vertex(p) => s.vertex_at(p),
        Pick::Space(p) => s.in_space(p),
        _ => return,
    };
}

pub(crate) fn click(s: &mut Session, pick: &Pick) {
    let at = match *pick {
        Pick::Face(p) => s.face_at(p),
        Pick::Edge(p) => s.edge_at(p),
        Pick::Vertex(p) => s.vertex_at(p),
        Pick::Sketch(x, y) => s.on_sketch(x, y),
        Pick::Space(p) => s.in_space(p),
        Pick::Bar(word) => {
            let word = s.word(word);
            s.press_word_near(&word, TOP);
            return;
        }
        Pick::Row(word) => {
            let lead = s.word(word);
            let left = s.canvas().min.x;
            let rows: Vec<qymcad::Rect> = s.words_at().into_iter().filter(|(w, r)| r.max.x < left && w.starts_with(&lead)).map(|(_, r)| r).collect();
            match rows.as_slice() {
                [r] => r.center(),
                other => panic!("the row beginning {lead:?} is not one row of the tree: {other:?}"),
            }
        }
        Pick::Listed(word) => {
            let lead = s.word(word);
            let right = s.canvas().max.x;
            let rows: Vec<qymcad::Rect> = s.words_at().into_iter().filter(|(w, r)| r.min.x > right && w.starts_with(&lead)).map(|(_, r)| r).collect();
            match rows.as_slice() {
                [r] => r.center(),
                other => panic!("the row beginning {lead:?} is not one row of the list of mates: {other:?}"),
            }
        }
    };
    s.click(at);
}

/// The fixture, the tool taken, and everything done up to the step that finishes it: the ordinary picks of a command,
/// or all but the last click of a drawing.
pub(crate) fn ready(tool: &Tool) -> Session {
    ready_after(tool, |_| {})
}

/// THE TOOL READY, with `prepare` done in the document before the tool is taken - a parameter added to its table.
pub(crate) fn ready_after(tool: &Tool, prepare: impl FnOnce(&mut Session)) -> Session {
    let mut s = tool.fixture.start();
    prepare(&mut s);
    take(&mut s, tool);
    assert!(matches!(tool.flow, Flow::Action) || holds(&mut s, tool), "{}: the tool is not in hand after it was taken; bar: {:?}", tool.id, s.in_hand());
    // the words the tool needs to do anything at all - the string of a text - are typed first, as a person types them;
    // a command takes its words after its picks, since what is picked sets them afresh (a thread sizes itself to the
    // face it is put on)
    // words given with Enter are what finishes the tool, so they wait for the finish
    if !matches!(tool.flow, Flow::Command) {
        for w in tool.words.iter().filter(|w| !w.enter) {
            type_words(&mut s, w, w.typical);
        }
    }
    match tool.flow {
        Flow::Command => {
            for p in tool.picks {
                click(&mut s, p);
            }
            for w in tool.words {
                type_words(&mut s, w, w.typical);
            }
        }
        Flow::Drawing(clicks, _) => {
            for (x, y) in clicks.iter().take(clicks.len().saturating_sub(1)) {
                s.click_on_sketch(*x, *y);
            }
        }
        Flow::Action => {}
    }
    // A FIELD THE TOOL OPENS WITH THE ORDINARY VALUE IS LEFT ALONE - a person who takes a fillet and presses Enter
    // gets the radius of 2 the field already holds. Where the ordinary value is another one, a person types it: a cut
    // opens at an offset of 0, which is the picked face itself and cuts nothing.
    for f in tool.fields.iter().filter(|f| f.when == When::Before) {
        if field_holds(&mut s, f) != Some(f.typical) {
            type_into_field(&mut s, f, &fmt(f.typical));
        }
    }
    s
}

/// The number a field holds now, as far as it can be read.
pub(crate) fn field_holds(s: &mut Session, f: &Field) -> Option<f64> {
    let w = field_if_there(s, f)?;
    w.value.replace(',', ".").trim().parse::<f64>().ok()
}

/// DO THE WHOLE OF THE TOOL'S WORK from a tool already in hand: the picks and Enter of a command, or the clicks of a
/// drawing - `clicks` in place of the ordinary ones when a mode takes its own.
pub(crate) fn work(s: &mut Session, tool: &Tool, clicks: Option<&'static [(f64, f64)]>) {
    if !matches!(tool.flow, Flow::Command) {
        for w in tool.words {
            type_words(s, w, w.typical);
        }
    }
    match tool.flow {
        Flow::Command => {
            for p in tool.picks {
                click(s, p);
            }
            for w in tool.words {
                type_words(s, w, w.typical);
            }
            enter_key(s);
        }
        Flow::Drawing(ordinary, last) => {
            let clicks = clicks.unwrap_or(ordinary);
            for (x, y) in clicks.iter().take(clicks.len().saturating_sub(1)) {
                s.click_on_sketch(*x, *y);
            }
            if let Some((x, y)) = clicks.last() {
                finish_click(s, *x, *y, last);
            }
        }
        Flow::Action => {}
    }
}

/// FINISH THE TOOL the way it is finished: Enter for a command, the last click for a drawing.
pub(crate) fn finish(s: &mut Session, tool: &Tool) {
    match tool.flow {
        Flow::Command => enter_key(s),
        Flow::Drawing(clicks, last) => {
            if let Some((x, y)) = clicks.last() {
                finish_click(&mut *s, *x, *y, last);
            }
        }
        Flow::Action => {}
    }
}

/// The click that finishes a drawing at (x, y).
pub(crate) fn finish_click(s: &mut Session, x: f64, y: f64, last: Finish) {
    match last {
        Finish::LastClick | Finish::Placed => {
            s.click_on_sketch(x, y);
        }
        Finish::DoubleClick => {
            let at = s.on_sketch(x, y);
            s.double_click(at);
        }
        Finish::EnterAt((px, py)) => {
            s.click_on_sketch(x, y);
            let at = s.on_sketch(px, py);
            s.move_to(at);
            s.key(Key::Enter);
        }
    }
}

/// The first field of the tool.
pub(crate) fn first_field(tool: &Tool) -> &Field {
    tool.fields.first().unwrap_or_else(|| panic!("{}: the description gives no field", tool.id))
}

/// The tool ready to be finished, with `text` typed into `field`: a command takes its value at the geometry after its
/// picks, a drawing before the click that makes it.
pub(crate) fn value_ready(tool: &Tool, field: &Field, text: &str) -> Session {
    let mut s = ready(tool);
    if field.when == When::Before {
        type_into_field(&mut s, field, text);
    }
    s
}

/// FINISH THE TOOL AND GIVE EVERY FIELD FILLED AFTERWARDS ITS ORDINARY VALUE: a dimension is placed by a click and
/// becomes a dimension when its number is typed.
pub(crate) fn finish_all(s: &mut Session, tool: &Tool) {
    // an action taken by its way in alone still wants its words: the search of the tree does its work as it is typed
    if matches!(tool.flow, Flow::Action) {
        for w in tool.words {
            let caption = s.word(w.caption);
            // words given with Enter end the action: once given, their field is gone and there is nothing to give again
            if w.enter && !s.shows(&caption) {
                continue;
            }
            if !w.by_placeholder || s.widgets().iter().any(|x| x.placeholder == caption && x.value.is_empty()) {
                type_words(s, w, w.typical);
            }
        }
    }
    finish(s, tool);
    for f in tool.fields.iter().filter(|f| f.when == When::After) {
        // a tool that opens no field to type into is a matter for the points about values, not for every other point
        if field_if_there(s, f).is_some() {
            type_into_field(s, f, &fmt(f.typical));
            enter_key(s);
        }
    }
}

/// FINISH THE TOOL AND GIVE `field` ITS VALUE the way this field takes one: a dimension is placed first and typed into
/// after, so Enter ends the typing rather than the tool.
pub(crate) fn finish_with(s: &mut Session, tool: &Tool, field: &Field, text: &str) {
    finish(s, tool);
    if field.when == When::After {
        type_into_field(s, field, text);
        enter_key(s);
    }
}

/// THE WORDS DRAWN IN THE BOX A FIELD STANDS IN, below the field itself: what the box says about the value in it.
///
/// The search is bounded on purpose. Everything under the field on the sheet is not the box - the status line and the
/// coordinates stand there too - so only the words that lie within a hand's width below the field and no higher than
/// it are taken, and the field's own text is left out because the value is not a reason.
pub(crate) fn words_in_the_box(s: &mut Session, field: &qymcad::Widget) -> Vec<String> {
    let below = field.rect.max.y;
    let limit = below + 40.0;
    s.painted()
        .into_iter()
        .filter(|(r, text)| !text.is_empty() && r.min.y >= below - 2.0 && r.max.y <= limit && r.max.x >= field.rect.min.x - 40.0 && !text.trim().eq_ignore_ascii_case(field.value.trim()))
        .map(|(_, text)| text)
        .collect()
}

/// WHERE THE FIELD `f` STANDS on screen.
pub(crate) fn field_of(s: &mut Session, f: &Field) -> qymcad::Widget {
    field_if_there(s, f).unwrap_or_else(|| panic!("no field shows or stands under {:?}", s.word(f.caption)))
}

/// The field `f`, if it is on screen at all: one that was refused may have been taken away with what was typed in it.
pub(crate) fn field_if_there(s: &mut Session, f: &Field) -> Option<qymcad::Widget> {
    let caption = s.word(f.caption);
    if f.by_placeholder {
        // the first in reading order, as `type_into_field` types into it
        s.widgets().into_iter().filter(|w| w.placeholder == caption).min_by(|a, b| (a.rect.min.y, a.rect.min.x).partial_cmp(&(b.rect.min.y, b.rect.min.x)).unwrap_or(std::cmp::Ordering::Equal))
    } else {
        let inputs = s.widgets();
        let _ = &inputs;
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| s.field(&caption))).ok()
    }
}

/// Type `text` into the field `f`, over what it holds: found by its caption, or by the grey words it shows - the first
/// field showing them in reading order, where a bar holds two (a chamfer by two legs: its first leg).
pub(crate) fn type_into_field(s: &mut Session, f: &Field, text: &str) {
    let caption = s.word(f.caption);
    if f.by_placeholder {
        s.fill_first_hinted(&caption, text);
    } else {
        s.fill(&caption, text);
    }
}

/// Apply with Enter.
pub(crate) fn enter_key(s: &mut Session) {
    s.key(Key::Enter);
}

/// THE BODY OF THE PART: the one shown and not taken up into another. A part is one body.
pub(crate) fn the_body(doc: &Document) -> Result<&Solid, String> {
    let shown: Vec<&Solid> = doc.bodies.iter().filter(|b| b.visible && !b.consumed).collect();
    match shown.as_slice() {
        [one] => Ok(one),
        other => Err(format!("the part holds {} bodies shown: {:?}", other.len(), other.iter().map(|b| (&b.name, b.volume)).collect::<Vec<_>>())),
    }
}
