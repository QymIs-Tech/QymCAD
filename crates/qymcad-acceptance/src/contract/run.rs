//! THE STEPS OF THE CONTRACT, the same for every tool: each point takes a description and checks it through the
//! window. A point that does not hold fails with what a person would have seen.
use std::time::{Duration, Instant};

use qymcad::{pos2, Document, Key, Modifiers, Pos2, Session, Solid};

use super::{Class, Context, Entry, Field, Finish, Flow, Node, Outcome, Pick, Tool, Under, Upstream, When, POINTS};
use crate::build;

/// RUN POINT `n` OF THE CONTRACT OF `tool`. A point the description marks as not applying passes when it says why.
pub fn point(tool: &Tool, n: u8) {
    if let Some((_, why)) = tool.not_applicable.iter().find(|(p, _)| *p == n) {
        assert!(!why.trim().is_empty(), "{}: point {n} is marked as not applying without a reason", tool.id);
        return;
    }
    match n {
        1 => entry(tool),
        2 => bar(tool),
        3 => picks(tool),
        4 => wrong_picks(tool),
        5 => valid_values(tool),
        6 => invalid_values(tool),
        7 => preview(tool),
        8 => apply(tool),
        9 => cancel(tool),
        10 => result(tool),
        11 => undo(tool),
        12 => save_open(tool),
        13 => reopen(tool),
        14 => upstream(tool),
        15 => dependency(tool),
        16 => contexts(tool),
        17 => kernel_refusal(tool),
        18 => budget(tool),
        19 => help(tool),
        _ => panic!("the contract has no point {n}; it has {:?}", POINTS),
    }
}

/// The top left corner of the window, near which the bars stand.
const TOP: Pos2 = pos2(0.0, 0.0);

/// TAKE THE TOOL the first way the description gives - for an action, on the selection its picks make first.
fn take(s: &mut Session, tool: &Tool) {
    preselect(s, tool);
    enter_for(s, tool, tool.entries.first().unwrap_or_else(|| panic!("{}: the description gives no way to take the tool", tool.id)));
}

/// THE SELECTION AN ACTION WORKS ON, made before its way in, as a person makes it: its picks in turn, each after the
/// first added with Shift. A constraint is two lines picked and then its button.
fn preselect(s: &mut Session, tool: &Tool) {
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
fn in_hand(s: &mut Session, tool: &Tool) -> bool {
    let title = s.word(tool.title);
    s.in_hand().contains(&title)
}

/// Is the tool in hand at all? Its own word in the bar says so; a command whose bar heads it with a generic word is
/// still in hand, and its bar says that by the pair of buttons every command bar carries; a tool of the sketch, by the
/// dash its bar is headed with. The points other than the
/// way in ask this, not the naming: a bar that does not name its tool is one defect, not nineteen.
fn holds(s: &mut Session, tool: &Tool) -> bool {
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
fn aim(s: &mut Session, pick: &Pick) {
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
fn ready(tool: &Tool) -> Session {
    ready_after(tool, |_| {})
}

/// THE TOOL READY, with `prepare` done in the document before the tool is taken - a parameter added to its table.
fn ready_after(tool: &Tool, prepare: impl FnOnce(&mut Session)) -> Session {
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
fn field_holds(s: &mut Session, f: &Field) -> Option<f64> {
    let w = field_if_there(s, f)?;
    w.value.replace(',', ".").trim().parse::<f64>().ok()
}

/// DO THE WHOLE OF THE TOOL'S WORK from a tool already in hand: the picks and Enter of a command, or the clicks of a
/// drawing - `clicks` in place of the ordinary ones when a mode takes its own.
fn work(s: &mut Session, tool: &Tool, clicks: Option<&'static [(f64, f64)]>) {
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
fn finish(s: &mut Session, tool: &Tool) {
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
fn first_field(tool: &Tool) -> &Field {
    tool.fields.first().unwrap_or_else(|| panic!("{}: the description gives no field", tool.id))
}

/// The tool ready to be finished, with `text` typed into `field`: a command takes its value at the geometry after its
/// picks, a drawing before the click that makes it.
fn value_ready(tool: &Tool, field: &Field, text: &str) -> Session {
    let mut s = ready(tool);
    if field.when == When::Before {
        type_into_field(&mut s, field, text);
    }
    s
}

/// FINISH THE TOOL AND GIVE EVERY FIELD FILLED AFTERWARDS ITS ORDINARY VALUE: a dimension is placed by a click and
/// becomes a dimension when its number is typed.
fn finish_all(s: &mut Session, tool: &Tool) {
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
fn finish_with(s: &mut Session, tool: &Tool, field: &Field, text: &str) {
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
fn words_in_the_box(s: &mut Session, field: &qymcad::Widget) -> Vec<String> {
    let below = field.rect.max.y;
    let limit = below + 40.0;
    s.painted()
        .into_iter()
        .filter(|(r, text)| !text.is_empty() && r.min.y >= below - 2.0 && r.max.y <= limit && r.max.x >= field.rect.min.x - 40.0 && !text.trim().eq_ignore_ascii_case(field.value.trim()))
        .map(|(_, text)| text)
        .collect()
}

/// WHERE THE FIELD `f` STANDS on screen.
fn field_of(s: &mut Session, f: &Field) -> qymcad::Widget {
    field_if_there(s, f).unwrap_or_else(|| panic!("no field shows or stands under {:?}", s.word(f.caption)))
}

/// The field `f`, if it is on screen at all: one that was refused may have been taken away with what was typed in it.
fn field_if_there(s: &mut Session, f: &Field) -> Option<qymcad::Widget> {
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
fn enter_key(s: &mut Session) {
    s.key(Key::Enter);
}

/// THE BODY OF THE PART: the one shown and not taken up into another. A part is one body.
fn the_body(doc: &Document) -> Result<&Solid, String> {
    let shown: Vec<&Solid> = doc.bodies.iter().filter(|b| b.visible && !b.consumed).collect();
    match shown.as_slice() {
        [one] => Ok(one),
        other => Err(format!("the part holds {} bodies shown: {:?}", other.len(), other.iter().map(|b| (&b.name, b.volume)).collect::<Vec<_>>())),
    }
}

/// IS WHAT THE TOOL LEFT BEHIND WHAT IT SHOULD BE - the body of the part, or the geometry of the sketch?
fn matches(s: &mut Session, want: &Outcome) -> Result<(), String> {
    let doc = s.document();
    match *want {
        Outcome::Body { volume, faces, edges, min, max } => {
            let b = the_body(&doc)?;
            let close = |a: f64, b: f64| (a - b).abs() <= 1e-5 * b.abs().max(1.0);
            // the box of a body is read off its mesh, whose points on a curve lie inside the true surface by up to
            // the tessellation deflection - 0.0015 of the body's diagonal at the ordinary quality. A sphere of
            // radius 20 measures 19.953 across the mesh, one of radius 1000 measures 999.5: a fixed allowance
            // would have to be wrong at one size or the other, so the allowance follows the body.
            let diagonal = (0..3).map(|i| (max[i] - min[i]).powi(2)).sum::<f64>().sqrt();
            let slack = (0.0015 * diagonal).max(0.05);
            let box_close = |a: [f64; 3], b: [f64; 3]| a.iter().zip(b.iter()).all(|(x, y)| (x - y).abs() <= slack);
            let got = (b.volume, b.faces, b.edges.unwrap_or(usize::MAX), b.min, b.max);
            if close(got.0, volume) && got.1 == faces && got.2 == edges && box_close(got.3, min) && box_close(got.4, max) {
                Ok(())
            } else {
                Err(format!("the body is {got:?}, it should be {:?}", (volume, faces, edges, min, max)))
            }
        }
        Outcome::Pieces { pieces, volume } => {
            let (volume_got, at) = {
                let b = the_body(&doc)?;
                (b.volume, doc.bodies.iter().position(|x| x.name == b.name).unwrap_or(0))
            };
            let solids = s.inspect(at).map(|i| i.solids).unwrap_or(0);
            let close = |a: f64, b: f64| (a - b).abs() <= 1e-5 * b.abs().max(1.0);
            if solids == pieces && close(volume_got, volume) {
                Ok(())
            } else {
                Err(format!("the body holds {solids} separate pieces of {volume_got} mm^3 together, it should hold {pieces} of {volume}"))
            }
        }
        Outcome::Volume { volume, tol, min, max } => {
            let b = the_body(&doc)?;
            let diagonal = (0..3).map(|i| (max[i] - min[i]).powi(2)).sum::<f64>().sqrt();
            let slack = (0.0015 * diagonal).max(0.05);
            let in_box = b.min.iter().zip(min.iter()).chain(b.max.iter().zip(max.iter())).all(|(g, w)| (g - w).abs() <= slack);
            if (b.volume - volume).abs() <= tol && in_box {
                Ok(())
            } else {
                Err(format!("the body holds {} mm^3 in {:?}..{:?}; it should hold {volume} to within {tol} in {min:?}..{max:?}", b.volume, b.min, b.max))
            }
        }
        Outcome::Construction { count, lines } => {
            let sk = doc.sketches.last().ok_or_else(|| "the document holds no sketch".to_string())?;
            if sk.construction == count && sk.lines == lines {
                Ok(())
            } else {
                Err(format!("the sketch holds {} lines, {} of its geometry construction; it should hold {lines}, {count} construction", sk.lines, sk.construction))
            }
        }
        Outcome::Picked { count } => {
            let sk = doc.sketches.last().ok_or_else(|| "the document holds no sketch".to_string())?;
            if sk.picked == count {
                Ok(())
            } else {
                Err(format!("{} entities of the sketch are selected, it should be {count}", sk.picked))
            }
        }
        Outcome::Mates { kinds } => {
            if doc.mates.iter().map(String::as_str).eq(kinds.iter().copied()) {
                Ok(())
            } else {
                Err(format!("the assembly is tied by the mates {:?}, it should be by {kinds:?}", doc.mates))
            }
        }
        Outcome::Turned { part, degrees } => {
            let Some(p) = doc.parts.get(part) else { return Err(format!("the assembly holds {} parts, not {}", doc.parts.len(), part + 1)) };
            // the angle of the turn from the trace of its matrix: cos = (trace - 1) / 2
            let trace = p.axes[0][0] + p.axes[1][1] + p.axes[2][2];
            let turned = ((trace - 1.0) / 2.0).clamp(-1.0, 1.0).acos().to_degrees();
            let want = degrees.rem_euclid(360.0);
            let want = want.min(360.0 - want);
            if (turned - want).abs() < 0.5 {
                Ok(())
            } else {
                Err(format!("{:?} is turned by {turned:.2} deg from how it was built, it should be by {want:.2}; its axes look {:?}", p.name, p.axes))
            }
        }
        Outcome::TreeRows { shown, hidden } => {
            let canvas = s.canvas();
            let rows: Vec<String> = s.words_at().into_iter().filter(|(_, r)| r.max.x < canvas.min.x && r.min.y > canvas.min.y).map(|(w, _)| w).collect();
            let missing: Vec<&str> = shown.iter().copied().filter(|w| !rows.iter().any(|r| r.starts_with(w))).collect();
            let left_in: Vec<&str> = hidden.iter().copied().filter(|w| rows.iter().any(|r| r.starts_with(w))).collect();
            if missing.is_empty() && left_in.is_empty() {
                Ok(())
            } else {
                Err(format!("the tree should show {missing:?} and does not, should not show {left_in:?} and does; it shows {rows:?}"))
            }
        }
        Outcome::Fitted => {
            let canvas = s.canvas();
            let mut outside = Vec::new();
            for b in doc.bodies.iter().filter(|b| b.visible && !b.consumed) {
                for i in 0..8 {
                    let corner = [if i & 1 == 0 { b.min[0] } else { b.max[0] }, if i & 2 == 0 { b.min[1] } else { b.max[1] }, if i & 4 == 0 { b.min[2] } else { b.max[2] }];
                    if !canvas.contains(s.projected(corner)) {
                        outside.push(corner);
                    }
                }
            }
            if doc.bodies.is_empty() || outside.is_empty() {
                Ok(())
            } else {
                Err(format!("the corners {outside:?} fall outside the canvas {canvas:?}"))
            }
        }
        Outcome::OnScreen { word } => {
            let want = s.word(word);
            if s.shows(&want) {
                Ok(())
            } else {
                Err(format!("{want:?} is not on screen"))
            }
        }
        Outcome::Says { text } => {
            let said = s.status();
            if said.contains(text) {
                Ok(())
            } else {
                Err(format!("the status line says {said:?}; it should say {text:?}"))
            }
        }
        Outcome::Section { on } => {
            if doc.section == on {
                Ok(())
            } else {
                Err(format!("the view is {}cut by a section; it should {}be", if doc.section { "" } else { "not " }, if on { "" } else { "not " }))
            }
        }
        Outcome::Shade { dark } => {
            let pic = s.snapshot();
            let (mut sum, mut n) = (0u64, 0u64);
            for px in pic.rgba.as_chunks::<4>().0 {
                sum += (u64::from(px[0]) * 299 + u64::from(px[1]) * 587 + u64::from(px[2]) * 114) / 1000;
                n += 1;
            }
            let mean = sum.checked_div(n).unwrap_or(0);
            if (mean < 128) == dark {
                Ok(())
            } else {
                Err(format!("the window is drawn with a mean brightness of {mean} of 255; it should be {}", if dark { "dark" } else { "light" }))
            }
        }
        Outcome::Chooser { save } => {
            let want = if save { qymcad::Chooser::Save } else { qymcad::Chooser::Open };
            match s.chooser() {
                Some(k) if k == want => Ok(()),
                other => Err(format!("the chooser up is {other:?}, it should be {want:?}")),
            }
        }
        Outcome::Window { title } => {
            let want = s.word(title);
            let open = s.windows();
            if open.iter().any(|w| w.contains(&want)) {
                Ok(())
            } else {
                Err(format!("no window {want:?} is open; windows open: {open:?}"))
            }
        }
        Outcome::Parts { parts, grounded, joints, assemblies } => {
            let own: Vec<&qymcad::Part> = doc.parts.iter().filter(|p| !p.assembly).collect();
            let subs = doc.parts.iter().filter(|p| p.assembly).count();
            let fixed = own.iter().filter(|p| p.grounded).count();
            let held = doc.joints.iter().filter(|j| !j.violated).count();
            if own.len() == parts && fixed == grounded && doc.joints.len() == joints && held == joints && subs == assemblies {
                Ok(())
            } else {
                Err(format!("the assembly holds {} parts, {fixed} of them grounded, {} joints ({held} held), {subs} sub-assemblies; it should hold {parts}, {grounded} grounded, {joints} joints held, {assemblies} sub-assemblies", own.len(), doc.joints.len()))
            }
        }
        Outcome::Row { parts, step } => {
            let at: Vec<[f64; 3]> = doc.parts.iter().filter(|p| !p.assembly).map(|p| p.at).collect();
            let stepped = at.windows(2).all(|w| (0..3).all(|k| (w[1][k] - w[0][k] - step[k]).abs() < 1e-3));
            if at.len() == parts && stepped {
                Ok(())
            } else {
                Err(format!("the parts stand at {at:?}; there should be {parts} of them, each {step:?} on from the one before"))
            }
        }
        Outcome::Bodies { count, volume, largest } => {
            let got: Vec<f64> = doc.bodies.iter().filter(|b| b.visible && !b.consumed).map(|b| b.volume).collect();
            let whole: f64 = got.iter().sum();
            let biggest = got.iter().copied().fold(0.0, f64::max);
            let close = |a: f64, b: f64| (a - b).abs() <= 1e-5 * b.abs().max(1.0);
            if got.len() == count && close(whole, volume) && close(biggest, largest) {
                Ok(())
            } else {
                Err(format!("the part holds {} bodies of {whole} mm^3, the largest {biggest}; it should hold {count} of {volume}, the largest {largest}", got.len()))
            }
        }
        Outcome::Sheet { area, min, max } => {
            let near = |a: [f64; 3], b: [f64; 3]| (0..3).all(|k| (a[k] - b[k]).abs() < 1e-3);
            let sheets: Vec<&Solid> = doc.bodies.iter().filter(|b| b.visible && !b.consumed && b.sheet).collect();
            if sheets.iter().any(|b| (b.area - area).abs() <= 1e-3 * area.max(1.0) && near(b.min, min) && near(b.max, max)) {
                Ok(())
            } else {
                Err(format!("no sheet of {area} mm^2 from {min:?} to {max:?}; the sheets shown are {:?}", sheets.iter().map(|b| (b.area, b.min, b.max)).collect::<Vec<_>>()))
            }
        }
        Outcome::Sketch { points, lines, arcs, circles, ellipses, splines, texts, notes, constraints, dof, box_of, size_of, under } => {
            let sk = doc.sketches.last().ok_or_else(|| "the document holds no sketch".to_string())?.clone();
            let got = (sk.points, sk.lines, sk.arcs, sk.circles, sk.ellipses, sk.splines, sk.texts, sk.notes);
            if got != (points, lines, arcs, circles, ellipses, splines, texts, notes) {
                return Err(format!(
                    "the sketch holds {got:?} (points, lines, arcs, circles, ellipses, splines, texts, notes), it should hold {:?}",
                    (points, lines, arcs, circles, ellipses, splines, texts, notes)
                ));
            }
            if constraints.is_some_and(|want| sk.constraints != want) {
                return Err(format!("the sketch holds {} constraints and dimensions, it should hold {:?}", sk.constraints, constraints));
            }
            if dof.is_some_and(|want| sk.dof != want) {
                return Err(format!("the sketch has {} degrees of freedom left, it should have {:?}", sk.dof, dof));
            }
            if let Some(size) = size_of {
                let got = [sk.max[0] - sk.min[0], sk.max[1] - sk.min[1]];
                if got.iter().zip(size.iter()).any(|(a, b)| (a - b).abs() > 1e-3) {
                    return Err(format!("what is drawn is {got:?} across, it should be {size:?}"));
                }
            }
            if let Some((min, max)) = box_of {
                let close = |a: [f64; 2], b: [f64; 2]| a.iter().zip(b.iter()).all(|(x, y)| (x - y).abs() <= 1e-3);
                if !close(sk.min, min) || !close(sk.max, max) {
                    return Err(format!("what is drawn stands in the box {:?}..{:?}, it should stand in {min:?}..{max:?}", sk.min, sk.max));
                }
            }
            let mut wrong = Vec::new();
            for (x, y, what) in under {
                let there = s.sketch_under(*x, *y);
                let is = matches!(
                    (&there, what),
                    (None, Under::Nothing)
                        | (Some(qymcad::SketchPick::Point { .. }), Under::Point)
                        | (Some(qymcad::SketchPick::Line { .. }), Under::Line)
                        | (Some(qymcad::SketchPick::Arc { .. }), Under::Arc)
                        | (Some(qymcad::SketchPick::Circle { .. }), Under::Circle)
                        | (Some(qymcad::SketchPick::Ellipse { .. }), Under::Ellipse)
                        | (Some(qymcad::SketchPick::Spline { .. }), Under::Spline)
                );
                if !is {
                    wrong.push(format!("at ({x}, {y}) lies {there:?}, it should be {what:?}"));
                }
            }
            if wrong.is_empty() {
                Ok(())
            } else {
                Err(wrong.join("; "))
            }
        }
    }
}

/// What a check compares of a document: its timeline, its bodies and their numbers, its sketches.
#[derive(Debug, PartialEq)]
struct Counted {
    nodes: Vec<crate::oracles::NodeShape>,
    bodies: Vec<BodyCount>,
    sketches: Vec<crate::oracles::SketchShape>,
}

/// A body by its name, its volume in thousandths of a mm^3, its faces and its edges.
#[derive(Debug, PartialEq)]
struct BodyCount {
    name: String,
    volume: u64,
    faces: usize,
    edges: Option<usize>,
}

fn shape_of(doc: &Document) -> Counted {
    Counted {
        nodes: crate::oracles::node_shapes(doc),
        bodies: doc.bodies.iter().map(|b| BodyCount { name: b.name.clone(), volume: (b.volume * 1e3).round() as u64, faces: b.faces, edges: b.edges }).collect(),
        sketches: crate::oracles::sketch_shapes(doc),
    }
}

/// A place on the canvas with nothing drawn on it: a corner of the canvas.
pub(crate) fn empty_space(s: &mut Session) -> Pos2 {
    let c = s.canvas();
    c.min + (c.max - c.min) * 0.04
}

/// A PICTURE OF THE WINDOW WITH THE POINTER PUT ASIDE, so what is under it is not lit up by the hover alone.
fn look(s: &mut Session) -> qymcad::Picture {
    let aside = empty_space(s);
    s.move_to(aside);
    s.snapshot()
}

/// Where the fields for typing stand: a click elsewhere takes the keyboard from them and redraws them, which is not
/// something drawn on the canvas.
fn fields_at(s: &mut Session) -> Vec<qymcad::Rect> {
    s.widgets().into_iter().filter(|w| matches!(w.kind, qymcad::Kind::TextField | qymcad::Kind::Number)).map(|w| w.rect).collect()
}

/// THE ROW OF `node` IN THE TREE: the row beginning with its word, or with its own name.
fn row_of(s: &mut Session, tool: &Tool, node: &Node) -> Pos2 {
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
const PRESS_MOVES_ON: &[&str] = &["sketch.mirror"];

/// 1. EVERY WAY IN TAKES THE TOOL; its button a second time puts it down; taken over another tool, it puts that one
///    down.
fn entry(tool: &Tool) {
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
fn pressed_at(tool: &Tool, e: &Entry) -> Pos2 {
    let Entry::Button(hint) = e else { panic!("{}: only a button stands somewhere", tool.id) };
    let mut s = tool.fixture.start();
    let word = s.word(hint);
    s.find_hint(&word).unwrap_or_else(|| panic!("{}: no button has the hint {word:?}", tool.id))
}

/// A way in, in words for a report.
fn describe(e: &Entry) -> String {
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
fn bar(tool: &Tool) {
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
fn picks(tool: &Tool) {
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
const MORE: &[(&str, Pick)] = &[
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

/// 4. A WRONG CLICK IS REFUSED IN WORDS, takes nothing and leaves the document as it was.
fn wrong_picks(tool: &Tool) {
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
    let on_block = matches!(tool.fixture, super::fixtures::Fixture::Block | super::fixtures::Fixture::BlockWithHole);
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
pub(crate) fn type_words(s: &mut Session, w: &super::Words, text: &str) {
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
fn valid_values(tool: &Tool) {
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
fn invalid_values(tool: &Tool) {
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
fn other_than_typical(f: &Field) -> f64 {
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

/// 7. THE RESULT IS SHOWN BEFORE ENTER, follows the value, and the document does not hold it yet.
fn preview(tool: &Tool) {
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
fn apply(tool: &Tool) {
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
fn cancel(tool: &Tool) {
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
fn result(tool: &Tool) {
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
fn undo(tool: &Tool) {
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

/// A file of this check's own to save into.
fn scratch(tool: &Tool, what: &str) -> String {
    let dir = std::env::temp_dir().join(format!("qymcad-contract-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("the scratch folder is made");
    dir.join(format!("{}-{what}.qcad", tool.id.replace('.', "-"))).to_string_lossy().into_owned()
}

/// 12. SAVED AND OPENED, THE NODE AND THE BODY ARE THE SAME: names, numbers and the names of the faces.
fn save_open(tool: &Tool) {
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
fn reopen(tool: &Tool) {
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
fn upstream(tool: &Tool) {
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
fn by_a_parameter(tool: &Tool) {
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
fn dependency(tool: &Tool) {
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

/// 16. THE TOOL WORKS IN EVERY CONTEXT IT IS DESCRIBED FOR, or refuses in words.
fn contexts(tool: &Tool) {
    assert!(!tool.contexts.is_empty(), "{}: the description names no context besides a part of its own", tool.id);
    let mut problems = Vec::new();
    for c in tool.contexts {
        let mut s = Session::start();
        build::into_the_first_part(&mut s);
        let label = match c {
            Context::SecondPart => {
                build::into_a_new_part(&mut s);
                "a second part of the assembly".to_string()
            }
            Context::AfterTool(other) => {
                other.fixture.build_here(&mut s);
                take(&mut s, other);
                for p in other.picks {
                    click(&mut s, p);
                }
                enter_key(&mut s);
                format!("after {}", other.id)
            }
        };
        tool.fixture.build_here(&mut s);
        let status = s.status();
        take(&mut s, tool);
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
        let made = match tool.flow {
            Flow::Command if tool.node.is_empty() => matches(&mut s, &tool.result).is_ok(),
            Flow::Command => doc.features.iter().any(|f| f.kind == tool.node && f.error.is_none()),
            Flow::Drawing(..) | Flow::Action => matches(&mut s, &tool.result).is_ok(),
        };
        if !made && s.status() == status {
            problems.push(format!("{label}: nothing was made and nothing was said"));
        }
    }
    assert!(problems.is_empty(), "{}: contexts:\n{}", tool.id, problems.join("\n"));
}

/// 17. A VALUE OF THE RIGHT FORM THE KERNEL CANNOT BUILD IS REFUSED IN WORDS, and the document is whole.
fn kernel_refusal(tool: &Tool) {
    let value = tool.refusal.unwrap_or_else(|| panic!("{}: the description gives no value the kernel cannot build, and does not say why", tool.id));
    let f = first_field(tool);
    let mut s = value_ready(tool, f, &fmt(value));
    let body = the_body(&s.document()).map(|b| (b.volume, b.faces)).ok();
    let status = s.status();
    finish_with(&mut s, tool, f, &fmt(value));
    let doc = s.document();
    let mut problems = Vec::new();
    let red = doc.features.iter().find(|f| f.kind == tool.node).and_then(|f| f.error.clone());
    let node_made = doc.features.iter().any(|f| f.kind == tool.node);
    if node_made && red.is_none() {
        problems.push(format!("{value} was applied as if it built: {:?}", doc.features));
    }
    if !node_made && s.status() == status {
        problems.push(format!("{value}: nothing was made and nothing was said"));
    }
    if body.is_some() && the_body(&doc).map(|b| (b.volume, b.faces)).ok() != body {
        problems.push("the body it was applied to is not whole any more".to_string());
    }
    assert!(problems.is_empty(), "{}: a value the kernel cannot build:\n{}", tool.id, problems.join("\n"));
}

/// 18. APPLYING AND EVERY FRAME ARE WITHIN THEIR TIME.
fn budget(tool: &Tool) {
    let mut s = ready(tool);
    let _ = s.worst_frame();
    let began = Instant::now();
    finish_all(&mut s, tool);
    let _ = s.document();
    let (took, frame) = (began.elapsed(), s.worst_frame());
    let (apply_budget, frame_budget) = (Duration::from_secs(tool.budget.0), Duration::from_millis(tool.budget.1));
    assert!(took <= apply_budget && frame <= frame_budget, "{}: applying took {took:?} of {apply_budget:?}, the longest frame {frame:?} of {frame_budget:?}", tool.id);
}

/// 19. F1 WITH THE TOOL IN HAND OPENS ITS ARTICLE.
fn help(tool: &Tool) {
    let mut s = tool.fixture.start();
    take(&mut s, tool);
    s.key(Key::F1);
    let article = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../docs/help/{}/{}.md", s.language(), tool.help));
    let text = std::fs::read_to_string(&article).unwrap_or_else(|_| panic!("{}: there is no article {}", tool.id, article.display()));
    let title = text.lines().next().unwrap_or_default().trim_start_matches('#').trim().to_string();
    let window = s.word("help-title");
    let frame = s.widgets().into_iter().find(|w| w.kind == qymcad::Kind::Window && w.label == window).map(|w| w.rect);
    let shown_in_it = frame.is_some_and(|f| s.words_at().iter().any(|(w, r)| *w == title && f.contains(r.center())));
    assert!(
        shown_in_it,
        "{}: F1 with the tool in hand: the help window is {}, and the article {title:?} is not in it; windows open: {:?}",
        tool.id,
        if frame.is_some() { "open" } else { "not open" },
        s.windows()
    );
}
