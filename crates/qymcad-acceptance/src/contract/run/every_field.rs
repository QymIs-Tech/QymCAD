//! Point 27: every field of the box of a tool works - its value lands in the document, a formula over a parameter
//! follows the parameter, its driver is read by another formula, and the node reopened holds it and takes another.
//! The fields are the box's own, as it stands on screen: every row with a driver, whatever the description lists.

use super::*;

/// The name given to the value in the row of its field.
const DRIVER: &str = "drvx";
/// The parameter the field is given as a formula.
const GIVEN: &str = "pgiv";
/// The parameter reading the driver: twice the value.
const READER: &str = "qread";

/// A ROW OF THE BOX WITH A DRIVER, told apart from another of the same caption by its place among them: the caption,
/// and how many rows of that caption stand above it.
#[derive(Clone, Debug)]
struct RowAt {
    caption: String,
    nth: usize,
}

/// THE FIELDS OF A ROW AS THEY STAND ON SCREEN: the value, and the name of its driver.
struct RowFields {
    value: qymcad::Widget,
    name: qymcad::Widget,
}

/// A LINE OF A BOX, by a place standing on it.
struct Line<'a> {
    of: &'a qymcad::Rect,
}

impl Line<'_> {
    /// Does place `r` stand on this line.
    fn holds(&self, r: &qymcad::Rect) -> bool {
        (self.of.center().y - r.center().y).abs() < self.of.height().max(r.height()) / 2.0
    }
}

/// A CAPTION OF A ROW and where it is written.
struct Caption {
    words: String,
    at: qymcad::Rect,
}

/// THE ROWS OF A BOX AS IT OPENS, and the value each held then.
struct Opened {
    rows: Vec<RowAt>,
    values: Vec<String>,
}

/// THE ROWS OF THE BOX WITH A DRIVER, top to bottom: the value of each is the last field on its line left of the words
/// "Driver", and its caption the word nearest left of the value - a tag of X, Y or Z, or the caption of the row. The words
/// of the tree and of the sheet stand on the same line further left, and are not it.
fn rows_of(s: &mut Session) -> Vec<RowAt> {
    let driver = s.word("sk-driver-label");
    let canvas = s.canvas();
    let words = s.words_at();
    let inputs: Vec<qymcad::Widget> = s.widgets().into_iter().filter(|w| w.kind == qymcad::Kind::TextField).collect();
    let mut rows: Vec<Caption> = Vec::new();
    // the boxes stand over the view; the properties beside it name values too, and are not a box of the tool
    for (_, d) in words.iter().filter(|(w, r)| *w == driver && canvas.contains(r.center())) {
        let Some(value) = inputs.iter().filter(|w| Line { of: d }.holds(&w.rect) && w.rect.max.x < d.min.x).max_by(|a, b| a.rect.min.x.total_cmp(&b.rect.min.x)) else { continue };
        let caption =
            words.iter().filter(|(w, r)| !w.trim().is_empty() && *w != driver && Line { of: &value.rect }.holds(r) && r.max.x <= value.rect.min.x + 1.0).max_by(|a, b| a.1.max.x.total_cmp(&b.1.max.x));
        if let Some((w, r)) = caption {
            rows.push(Caption { words: w.clone(), at: *r });
        }
    }
    rows.sort_by(|a, b| a.at.min.y.total_cmp(&b.at.min.y));
    let mut out: Vec<RowAt> = Vec::new();
    for Caption { words: caption, .. } in rows {
        let nth = out.iter().filter(|r| r.caption == caption).count();
        out.push(RowAt { caption, nth });
    }
    out
}

/// The value and the name of row `at`, as they stand: of the rows with a driver, the `nth` whose caption is the word
/// nearest left of its value.
fn fields_of(s: &mut Session, at: &RowAt) -> Option<RowFields> {
    let driver = s.word("sk-driver-label");
    let words = s.words_at();
    let inputs: Vec<qymcad::Widget> = s.widgets().into_iter().filter(|w| w.kind == qymcad::Kind::TextField).collect();
    let mut found: Vec<RowFields> = Vec::new();
    let canvas = s.canvas();
    let mut ds: Vec<qymcad::Rect> = words.iter().filter(|(w, r)| *w == driver && canvas.contains(r.center())).map(|(_, r)| *r).collect();
    ds.sort_by(|a, b| a.min.y.total_cmp(&b.min.y));
    for d in ds {
        let Some(value) = inputs.iter().filter(|w| Line { of: &d }.holds(&w.rect) && w.rect.max.x < d.min.x).max_by(|a, b| a.rect.min.x.total_cmp(&b.rect.min.x)) else { continue };
        let caption =
            words.iter().filter(|(w, r)| !w.trim().is_empty() && *w != driver && Line { of: &value.rect }.holds(r) && r.max.x <= value.rect.min.x + 1.0).max_by(|a, b| a.1.max.x.total_cmp(&b.1.max.x));
        if caption.is_none_or(|(w, _)| *w != at.caption) {
            continue;
        }
        let name = inputs.iter().filter(|w| Line { of: &d }.holds(&w.rect) && w.rect.min.x >= d.max.x).min_by(|a, b| a.rect.min.x.total_cmp(&b.rect.min.x));
        if let Some(name) = name {
            found.push(RowFields { value: value.clone(), name: name.clone() });
        }
    }
    found.into_iter().nth(at.nth)
}

/// The tool put down, so nothing in hand holds a word a check looks for elsewhere - the formula in a field of its box.
fn put_down(s: &mut Session) {
    s.key(Key::Escape).key(Key::Escape);
}

/// Type `text` into widget `w`, over what it holds.
fn type_into(s: &mut Session, w: &qymcad::Widget, text: &str) {
    s.click(w.rect.center()).chord(qymcad::Modifiers::COMMAND, Key::A).type_text(text);
}

/// The tool in hand with its box open: its picks made, a drawing finished to the shape whose sizes the box holds.
fn box_open(tool: &Tool, prepare: impl FnOnce(&mut Session)) -> Session {
    let mut s = ready_after(tool, prepare);
    if matches!(tool.flow, Flow::Drawing(..)) {
        finish(&mut s, tool);
    }
    s
}

/// 27. EVERY FIELD OF THE BOX WORKS, each in a session of its own: given as the formula `pgiv` over a parameter and named
///     `drvx` in the row of its field, Enter applies it; a parameter `qread = drvx*2` reads twice the value; `pgiv` changed,
///     the value and `qread` follow; the node reopened shows `pgiv` in the field, and a number typed over it is what
///     `qread` reads next. Answers what did not hold, by row.
pub(crate) fn every_field(tool: &Tool) {
    let problems = every_field_problems(tool);
    assert!(problems.is_empty(), "{}: fields of its box that do not work:\n{}", tool.id, problems.join("\n"));
}

/// The rows of the box of `tool` that do not work, each said with what did not hold.
fn every_field_problems(tool: &Tool) -> Vec<String> {
    let opened = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut s = box_open(tool, |_| {});
        let rows = rows_of(&mut s);
        let values: Vec<String> = rows.iter().map(|r| fields_of(&mut s, r).map(|f| f.value.value).unwrap_or_default()).collect();
        Opened { rows, values }
    }));
    let Ok(Opened { rows, values }) = opened else { return vec![format!("{}: the box could not be opened", tool.id)] };
    // A ROW IN A PROCESS OF ITS OWN: the sessions of a box of thirteen rows, one after another, held past the memory of a
    // check (2 GB); the row asked of a child is checked here
    let asked: Option<usize> = std::env::var(ROW_VAR).ok().and_then(|v| v.parse().ok());
    let own = std::env::args().find(|a| a.contains("::b27_every_field"));
    let mut problems = Vec::new();
    for (i, (at, held)) in rows.iter().zip(values).enumerate() {
        if asked.is_some_and(|a| a != i) {
            continue;
        }
        let found = match (&own, asked) {
            (Some(path), None) => {
                crate::isolation::apart(path, std::time::Duration::from_secs(120), ROW_MEMORY_MB, &[crate::isolation::Setting { name: ROW_VAR, value: i.to_string() }]).err().into_iter().collect()
            }
            _ => {
                let typical = held.trim().parse::<f64>().unwrap_or(1.0);
                let one = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| one_row(tool, at, typical)));
                one.unwrap_or_else(|e| vec![e.downcast_ref::<String>().cloned().or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default()])
            }
        };
        problems.extend(found.into_iter().map(|p| format!("{} {:?}#{}: {p}", tool.id, at.caption, at.nth)));
    }
    problems
}

/// HOW MUCH MEMORY THE CHECK OF ONE ROW MAY HOLD: a thread of its own pitch rebuilt four times over - applied, its
/// parameter changed, reopened, given a number - held 2072 MB. The rows of a box run one after another, never side by
/// side.
const ROW_MEMORY_MB: u64 = 3072;

/// The variable a check of every field is told which row of the box to check by, in a process of its own.
const ROW_VAR: &str = "QYMCAD_EVERY_FIELD_ROW";

/// The value the reader `q` holds, read from the document.
fn reader(s: &mut Session) -> Option<f64> {
    s.document().parameters.iter().find(|p| p.name == READER).map(|p| p.value)
}

/// Does the reader hold twice `value`, and if not, what it holds.
fn judge(s: &mut Session, value: f64, what: &str, problems: &mut Vec<String>) {
    let got = reader(s);
    if !got.is_some_and(|v| (v - 2.0 * value).abs() < 1e-6) {
        problems.push(format!("{what}: {READER} = {DRIVER}*2 reads {got:?}, the value is {value}"));
    }
}

/// The four checks of row `at` of the box of `tool`, its value standing at `typical` as the box opens, in one session.
fn one_row(tool: &Tool, at: &RowAt, typical: f64) -> Vec<String> {
    let mut problems = Vec::new();
    let mut s = box_open(tool, |s| build::parameter(s, GIVEN, &fmt(typical)));
    let Some(f) = fields_of(&mut s, at) else { return vec!["the row is not in the box".to_string()] };
    type_into(&mut s, &f.value, GIVEN);
    // found again: what a formula comes to stands under its row, and the box is laid anew by its new height
    let Some(f) = fields_of(&mut s, at) else { return vec!["the row went from the box when a formula was typed".to_string()] };
    type_into(&mut s, &f.name, DRIVER);
    // Enter applies the box; a name typed opens the list of names to finish it with, and the first Enter takes it
    enter_key(&mut s);
    if !tool.stays && holds(&mut s, tool) && matches!(tool.flow, Flow::Command) {
        enter_key(&mut s);
    }
    if matches!(tool.flow, Flow::Drawing(..)) && fields_of(&mut s, at).is_some() {
        enter_key(&mut s);
    }
    put_down(&mut s);
    build::parameter(&mut s, READER, &format!("{DRIVER}*2"));
    judge(&mut s, typical, &format!("given as {GIVEN} = {}", fmt(typical)), &mut problems);
    // the parameter changed: the value follows it, and so does what reads its name
    let other = typical + 1.0;
    build::set_parameter(&mut s, GIVEN, &fmt(other));
    judge(&mut s, other, &format!("{GIVEN} changed to {}", fmt(other)), &mut problems);
    // REOPENED BY A DOUBLE CLICK: the field holds the formula, and a number typed over it is the value
    if matches!(tool.flow, Flow::Command) && !tool.node.is_empty() && !tool.not_applicable.iter().any(|(p, _)| *p == 13) {
        let row = row_of(&mut s, tool, &Node { kind: tool.node, row: Some(tool.title) });
        s.double_click(row);
        let Some(f) = fields_of(&mut s, at) else {
            problems.push(format!("reopened, the row is not in the box; bar: {:?}", s.in_hand()));
            return problems;
        };
        if f.value.value.trim() != GIVEN {
            problems.push(format!("reopened, the field holds {:?}; it was given {GIVEN:?}", f.value.value));
        }
        // a number of its own inside the range the field takes: one under what it opened with, or two over it from zero
        let typed = if typical > 1.0 { typical - 1.0 } else { typical + 2.0 };
        type_into(&mut s, &f.value, &fmt(typed));
        enter_key(&mut s);
        put_down(&mut s);
        judge(&mut s, typed, &format!("reopened and given {}", fmt(typed)), &mut problems);
    }
    problems
}
