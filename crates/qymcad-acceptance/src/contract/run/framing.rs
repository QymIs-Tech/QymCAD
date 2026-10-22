//! Points 20-26: what every command does alike, whatever its workbench - the selection made before it is taken, its
//! fields stand at the geometry, Tab goes on to the next field, Esc leaves a field before the command, Enter before the
//! picks is refused in words, the bar of the command keeps its place, a formula stays a formula.
//!
//! These points ask a command alone. An action holds nothing to ask them of; a drawing of the sketch keeps its own boxes
//! of fields and its own bar of options (`qymcad-sketch`), and is not held to them.

use super::*;

/// Is the tool a command - picks, fields at the geometry, Enter?
fn command(tool: &Tool) -> bool {
    matches!(tool.flow, Flow::Command)
}

/// The fields of the tool typed before it is finished: the ones of its box at the geometry.
fn fields_before(tool: &Tool) -> Vec<&Field> {
    tool.fields.iter().filter(|f| f.when == When::Before).collect()
}

/// Can `pick` be made with nothing in hand - a place of the model or a row of the tree - rather than a word of the bar
/// of the tool, which is there only while the tool is?
fn selectable(pick: &Pick) -> bool {
    !matches!(pick, Pick::Bar(_))
}

/// 20. WHAT IS SELECTED BEFORE THE COMMAND IS TAKEN: the picks made with nothing in hand - each after the first with
///     Shift, and the first too where the fixture has already selected the object of the tool - and then the command;
///     Enter alone makes the ordinary result. A command that does its work on the selection as it is taken (a sketch
///     on the face selected) is judged by the result at once.
pub(crate) fn taken_before(tool: &Tool) {
    if !command(tool) || tool.picks.is_empty() || !tool.picks.iter().all(selectable) {
        return; // nothing to select first, or a pick only the bar of the tool offers
    }
    let mut s = tool.fixture.start();
    for (n, p) in tool.picks.iter().enumerate() {
        click_adding(&mut s, p, if n > 0 || tool.fixture.selects() { Adding::Yes } else { Adding::No });
    }
    take(&mut s, tool);
    if holds(&mut s, tool) {
        for w in tool.words {
            type_words(&mut s, w, w.typical);
        }
        for f in fields_before(tool) {
            if field_holds(&mut s, f) != Some(f.typical) && field_if_there(&mut s, f).is_some() {
                type_into_field(&mut s, f, &fmt(f.typical));
            }
        }
        enter_key(&mut s);
    }
    if let Err(e) = matches(&mut s, &tool.result) {
        panic!("{}: the selection made before the tool was not taken: {e}; the bar says {:?}, the status line {:?}", tool.id, s.in_hand(), s.status());
    }
}

/// THE FIELD `f` IN THE BOX AT THE GEOMETRY: its caption, or the grey words it shows, on the canvas, and a field to type
/// into after it on the canvas as well. A field of the bar above the canvas is not one.
fn at_the_geometry(s: &mut Session, f: &Field) -> bool {
    let caption = s.word(f.caption);
    let canvas = s.canvas();
    let inputs: Vec<qymcad::Widget> = s.widgets().into_iter().filter(|w| matches!(w.kind, qymcad::Kind::TextField | qymcad::Kind::Number) && canvas.contains(w.rect.center())).collect();
    if f.by_placeholder {
        return inputs.iter().any(|w| w.placeholder == caption);
    }
    let captions: Vec<qymcad::Rect> = s.words_at().into_iter().filter(|(w, r)| *w == caption && canvas.contains(r.center())).map(|(_, r)| r).collect();
    captions.iter().any(|c| inputs.iter().any(|w| w.rect.min.y < c.max.y && w.rect.max.y > c.min.y && w.rect.min.x >= c.max.x - 1.0))
}

/// 21. EVERY FIELD OF THE COMMAND STANDS AT THE GEOMETRY, in its box on the canvas: not in the bar, not in a panel.
pub(crate) fn at_geometry(tool: &Tool) {
    let fields = fields_before(tool);
    if !command(tool) || fields.is_empty() {
        return;
    }
    let mut s = ready(tool);
    let mut problems = Vec::new();
    for f in fields {
        if !at_the_geometry(&mut s, f) {
            let place = field_if_there(&mut s, f).map(|w| format!("at {:?}", w.rect)).unwrap_or_else(|| "nowhere on screen".into());
            problems.push(format!("{:?} stands {place}, not on the canvas {:?}", s.word(f.caption), s.canvas()));
        }
    }
    assert!(problems.is_empty(), "{}: the fields at the geometry:\n{}", tool.id, problems.join("\n"));
}

/// 22. TAB GOES ON TO THE NEXT FIELD: a value typed into the first field, Tab, and what is typed then lands in another
///     field, the first one keeping its value.
pub(crate) fn tab(tool: &Tool) {
    let fields = fields_before(tool);
    if !command(tool) || fields.len() < 2 {
        return; // one field has no next one to go to
    }
    let mut s = ready(tool);
    let first = fields[0];
    type_into_field(&mut s, first, &fmt(first.typical));
    let held = field_of(&mut s, first);
    let others = |s: &mut Session, held: &qymcad::Widget| -> Vec<qymcad::Widget> {
        s.widgets().into_iter().filter(|w| matches!(w.kind, qymcad::Kind::TextField | qymcad::Kind::Number) && w.rect != held.rect).collect()
    };
    let before = others(&mut s, &held);
    s.key(Key::Tab).type_text("7");
    let after = others(&mut s, &held);
    let mut problems = Vec::new();
    if field_of(&mut s, first).value != held.value {
        problems.push(format!("after Tab the first field {:?} holds {:?}, it held {:?}", s.word(first.caption), field_of(&mut s, first).value, held.value));
    }
    let moved = after.iter().any(|w| w.value.contains('7') && !before.iter().any(|b| b.rect == w.rect && b.value == w.value));
    if !moved {
        problems.push(format!("what was typed after Tab landed in no other field; the fields hold {:?}", after.iter().map(|w| &w.value).collect::<Vec<_>>()));
    }
    assert!(problems.is_empty(), "{}: Tab:\n{}", tool.id, problems.join("\n"));
}

/// 23. Esc LEAVES THE FIELD FIRST, THE COMMAND NEXT: with the keyboard in a field the first Esc takes it out, the command
///     staying in hand; the second puts the command down; the document is as it was.
pub(crate) fn esc_steps(tool: &Tool) {
    let fields = fields_before(tool);
    if !command(tool) || fields.is_empty() {
        return; // with no field the first Esc is the command's own, which is point 9
    }
    let mut s = ready(tool);
    let doc = shape_of(&s.document());
    type_into_field(&mut s, fields[0], &fmt(fields[0].typical + 1.0));
    s.key(Key::Escape);
    let mut problems = Vec::new();
    if !holds(&mut s, tool) {
        problems.push(format!("the first Esc, the keyboard in a field, put the command down; bar: {:?}", s.in_hand()));
    } else {
        if s.typing() {
            problems.push("the first Esc left the keyboard in the field".to_string());
        }
        s.key(Key::Escape);
        if holds(&mut s, tool) {
            problems.push(format!("the second Esc did not put the command down; bar: {:?}", s.in_hand()));
        }
    }
    if shape_of(&s.document()) != doc {
        problems.push("Esc changed the document".to_string());
    }
    assert!(problems.is_empty(), "{}: Esc step by step:\n{}", tool.id, problems.join("\n"));
}

/// 24. ENTER BEFORE WHAT THE COMMAND NEEDS IS REFUSED IN WORDS: the command taken and nothing picked, Enter makes
///     nothing, keeps the command in hand and says in the tone of a refusal what is wanted.
pub(crate) fn enter_waits(tool: &Tool) {
    if !command(tool) || tool.picks.is_empty() {
        return; // a command picking nothing is ready as it is taken
    }
    let mut s = tool.fixture.start();
    take(&mut s, tool);
    let (doc, before) = (shape_of(&s.document()), refusals(&mut s));
    enter_key(&mut s);
    let mut problems = Vec::new();
    if shape_of(&s.document()) != doc {
        problems.push("Enter before the picks changed the document".to_string());
    }
    if !holds(&mut s, tool) {
        problems.push(format!("Enter before the picks put the command down; bar: {:?}", s.in_hand()));
    }
    // the answer is said where the tool speaks, in the tone of a refusal
    let said = refusals(&mut s);
    if said.is_empty() || said == before {
        problems.push(format!("Enter before the picks was not refused in words where the tool speaks: it says {:?}, it said {before:?} before", s.tool_says()));
    }
    assert!(problems.is_empty(), "{}: Enter before the picks:\n{}", tool.id, problems.join("\n"));
}

/// WHERE THE BAR OF THE COMMAND AND THE CANVAS UNDER IT STAND.
#[derive(Debug)]
struct Places {
    bar: Option<qymcad::Rect>,
    canvas: qymcad::Rect,
}

impl Places {
    fn of(s: &mut Session) -> Self {
        Places { bar: s.panel("command_bar"), canvas: s.canvas() }
    }

    /// The same, to a fraction of a point.
    fn same_as(&self, other: &Places) -> bool {
        let near = |a: qymcad::Rect, b: qymcad::Rect| (a.min - b.min).length() < 0.5 && (a.max - b.max).length() < 0.5;
        self.bar.zip(other.bar).is_some_and(|(a, b)| near(a, b)) && near(self.canvas, other.canvas)
    }
}

/// 25. THE BAR OF THE COMMAND KEEPS ITS PLACE: it stands with nothing in hand, and taking the command and putting it
///     down moves neither the bar nor the canvas under it.
pub(crate) fn bar_stays(tool: &Tool) {
    if !command(tool) {
        return;
    }
    let mut s = tool.fixture.start();
    let free = Places::of(&mut s);
    let mut problems = Vec::new();
    if free.bar.is_none() {
        problems.push("with nothing in hand the bar of the command is not there".to_string());
    }
    take(&mut s, tool);
    let taken = Places::of(&mut s);
    if !taken.same_as(&free) {
        problems.push(format!("taking the command moved them from {free:?} to {taken:?}"));
    }
    for _ in 0..3 {
        if !holds(&mut s, tool) {
            break;
        }
        s.key(Key::Escape);
    }
    let left = Places::of(&mut s);
    if !left.same_as(&free) {
        problems.push(format!("putting the command down moved them from {free:?} to {left:?}"));
    }
    assert!(problems.is_empty(), "{}: the place of the bar:\n{}", tool.id, problems.join("\n"));
}

/// 26. A FORMULA STAYS A FORMULA: the first field given the parameter `k`, the command finished, and the node reopened
///     by a double click shows `k` in the field, not the number it came to.
pub(crate) fn formula_kept(tool: &Tool) {
    let Some(f) = fields_before(tool).first().copied() else { return };
    if !command(tool) || tool.node.is_empty() || tool.not_applicable.iter().any(|(p, _)| *p == 13) {
        return; // no node to reopen: point 14 follows the parameter through the result
    }
    let mut s = ready_after(tool, |s| build::parameter(s, "k", &fmt(f.typical)));
    type_into_field(&mut s, f, "k");
    finish(&mut s, tool);
    if !tool.stays && holds(&mut s, tool) {
        enter_key(&mut s); // the first Enter took the name from the list of names
    }
    let row = row_of(&mut s, tool, &Node { kind: tool.node, row: Some(tool.title) });
    s.double_click(row);
    let problems = if !holds(&mut s, tool) {
        vec![format!("the double click did not open the command; bar: {:?}", s.in_hand())]
    } else {
        let held = field_of(&mut s, f).value;
        if held.trim() == "k" {
            vec![]
        } else {
            vec![format!("the reopened field {:?} holds {held:?}, the formula typed was \"k\"", s.word(f.caption))]
        }
    };
    assert!(problems.is_empty(), "{}: a formula in the field:\n{}", tool.id, problems.join("\n"));
}
