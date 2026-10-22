//! THE LAYOUT OF A BOX AT THE GEOMETRY, as its tool describes it: sections - a title, folded or not as the box opens,
//! and rows - each a caption and what stands after it: one field or several on one line (X, Y, Z), a group of choices,
//! a switch, a button. The frame draws the rows of a section in three aligned columns - the caption, the value with
//! its unit and its buttons, the name of a driver - with what a formula comes to, or why a value cannot be taken, on a
//! line under the row across the value and the name; the title of a section folds it.
//!
//! The box of a joint laid its nine fields one under another, then its switches and buttons wrapped in a heap after
//! them: what belonged to part A, to part B and to the limits could not be told apart.

use super::*;

/// THE LAYOUT OF A BOX: its sections, and the buttons at its foot over the ends of the tool. A field no row names stands
/// in a row of its own at the top, its caption its own.
#[derive(Default)]
pub struct Layout {
    pub sections: Vec<Section>,
    pub foot: Vec<Opt>,
}

/// ONE SECTION OF A BOX: its title, if it has one, whether it stands folded as the box opens, and its rows.
pub struct Section {
    pub title: Option<String>,
    pub folded: Folded,
    pub rows: Vec<Row>,
    /// A section that is a switch of its tool - the second direction of a pattern: opened, it is switched on, folded,
    /// off; its title tells the tool, which says whether it stands open (`folded`).
    pub switch: Option<OptKey>,
}

impl Section {
    /// A section with no title - the rows at the top of a box.
    pub fn plain(rows: Vec<Row>) -> Section {
        Section { title: None, folded: Folded::No, rows, switch: None }
    }

    /// A section under its title, folded or not as the box opens.
    pub fn titled(title: String, folded: Folded, rows: Vec<Row>) -> Section {
        Section { title: Some(title), folded, rows, switch: None }
    }

    /// A section under its title that switches what it holds on as it is opened: `key` told to the tool, `on` as the
    /// tool holds it.
    pub fn switched(title: String, key: OptKey, on: Lit, rows: Vec<Row>) -> Section {
        let folded = if on == Lit::On { Folded::No } else { Folded::Yes };
        Section { title: Some(title), folded, rows, switch: Some(key) }
    }
}

/// ONE DIRECTION OF A PATTERN IN ITS BOX: the rows of the workbench under it - an axis, the copies and the pitch of a
/// pattern of bodies or of parts, the copies and the steps along X and Y of a pattern of a sketch - and, for every direction
/// after the first, the switch its title is and whether it is on.
pub struct PatternDirection {
    pub rows: Vec<Row>,
    pub switch: Option<DirectionSwitch>,
}

/// The switch of a direction of a pattern after the first: the key told to the tool as its title is pressed, and whether
/// the tool holds it on.
#[derive(Clone, Copy)]
pub struct DirectionSwitch {
    pub key: OptKey,
    pub on: Lit,
}

/// THE DIRECTIONS OF A PATTERN, ONE DESCRIPTION FOR EVERY WORKBENCH: a section each under its title "Direction N" as the
/// parts of a joint are; the first open, every one after it folded until its title is pressed, which switches it on.
/// Nothing of them stands in the bar.
pub fn pattern_directions(directions: Vec<PatternDirection>) -> Vec<Section> {
    directions
        .into_iter()
        .enumerate()
        .map(|(k, d)| {
            let title = qymcad_i18n::tr1("box-direction-n", "n", &(k + 1).to_string());
            match d.switch {
                Some(sw) => Section::switched(title, sw.key, sw.on, d.rows),
                None => Section::titled(title, Folded::No, d.rows),
            }
        })
        .collect()
}

/// DOES A SECTION STAND FOLDED as its box opens.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Folded {
    No,
    Yes,
}

/// ONE ROW OF A SECTION: its caption - or a button in its place, the radius or the diameter a value is shown as - what
/// stands after it on one line, and the unit its numbers are in, after them.
#[derive(Clone)]
pub struct Row {
    pub caption: String,
    pub lead: Option<Opt>,
    pub cells: Vec<Cell>,
    pub unit: Option<String>,
}

/// WHERE A FIELD SAYS WHAT ITS FORMULA COMES TO: beside it, in a small box of a field at a point of its own, or on the
/// line under its row, in a box laid in rows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Note {
    #[default]
    Beside,
    Under,
}

/// WHAT STANDS IN A ROW: a field of the tool by its key, with a short tag before it where the row holds several (X, Y,
/// Z), or an option.
#[derive(Clone)]
pub enum Cell {
    Field { key: String, tag: Option<String> },
    Opt(Opt),
}

impl Row {
    /// A row with its caption and nothing after it yet.
    pub fn new(caption: impl Into<String>) -> Row {
        Row { caption: caption.into(), lead: None, cells: Vec::new(), unit: None }
    }

    /// The same row with button `o` in the place of its caption: "Ø" switching the value to a radius.
    pub fn lead(mut self, o: Opt) -> Row {
        self.lead = Some(o);
        self
    }

    /// The same row with field `key` after what it holds.
    pub fn field(mut self, key: &str) -> Row {
        self.cells.push(Cell::Field { key: key.to_string(), tag: None });
        self
    }

    /// The same row with field `key` after what it holds, `tag` before it.
    pub fn tagged(mut self, tag: impl Into<String>, key: &str) -> Row {
        self.cells.push(Cell::Field { key: key.to_string(), tag: Some(tag.into()) });
        self
    }

    /// The same row with option `o` after what it holds.
    pub fn opt(mut self, o: Opt) -> Row {
        self.cells.push(Cell::Opt(o));
        self
    }

    /// The same row with its numbers in the unit of catalogue key `key`, written after them: "mm", "°".
    pub fn unit(mut self, key: &str) -> Row {
        self.unit = Some(qymcad_i18n::tr(key).trim().to_string());
        self
    }
}

/// The id a section keeps its folding under, in box `b`.
fn fold_id(b: &FieldsBox, at: usize) -> egui::Id {
    b.id.with(("the folding of a section", at))
}

/// Is section `at` of box `b` open now - as it was left, or as it stands when the box opens.
fn open(ctx: &egui::Context, b: &FieldsBox, at: usize, s: &Section) -> bool {
    if s.switch.is_some() {
        return s.folded == Folded::No;
    }
    ctx.data(|d| d.get_temp::<bool>(fold_id(b, at))).unwrap_or(s.folded == Folded::No)
}

/// THE ROWS OF A BOX AS THEY STAND: the fields no row names first, one a row under its own caption, then the sections of
/// the layout; a field the tool does not hold now is left out, and a row or a section left with nothing goes with it.
pub(crate) fn sections_of(fields: &[CmdParam], b: &FieldsBox, layout: &Layout) -> Vec<Placed> {
    let named = |key: &str| layout.sections.iter().flat_map(|s| &s.rows).flat_map(|r| &r.cells).any(|c| matches!(c, Cell::Field { key: k, .. } if k == key));
    let held = |key: &str| fields.iter().any(|p| p.key == key && p.at.is_none() && !b.hidden.contains(&key));
    let loose: Vec<Placed> =
        fields.iter().filter(|p| p.at.is_none() && !b.hidden.contains(&p.key.as_str()) && !named(&p.key)).map(|p| Placed::Field { caption: field_caption(p), key: p.key.clone() }).collect();
    let mut out = loose;
    for (at, s) in layout.sections.iter().enumerate() {
        let rows: Vec<&Row> = s.rows.iter().filter(|r| r.cells.iter().any(|c| matches!(c, Cell::Opt(_)) || matches!(c, Cell::Field { key, .. } if held(key)))).collect();
        // a section that switches its fields on stands folded with none of them held
        if !rows.is_empty() || s.switch.is_some() {
            out.push(Placed::Section { at });
        }
    }
    out
}

/// WHAT STANDS IN A BOX, in order: a field of its own row, or a section of the layout with the rows it shows.
pub(crate) enum Placed {
    Field { caption: String, key: String },
    Section { at: usize },
}

/// THE KEYS OF THE FIELDS A PERSON SEES IN THE BOX NOW, in order: a folded section hides its own.
pub(crate) fn shown_keys(ctx: &egui::Context, fields: &[CmdParam], b: &FieldsBox, layout: &Layout) -> Vec<String> {
    let held = |key: &str| fields.iter().any(|p| p.key == key && p.at.is_none() && !b.hidden.contains(&key));
    let mut keys = Vec::new();
    for placed in sections_of(fields, b, layout) {
        match placed {
            Placed::Field { key, .. } => keys.push(key),
            Placed::Section { at } => {
                let s = &layout.sections[at];
                if s.title.is_none() || open(ctx, b, at, s) {
                    for c in s.rows.iter().flat_map(|r| &r.cells) {
                        if let Cell::Field { key, .. } = c {
                            if held(key) {
                                keys.push(key.clone());
                            }
                        }
                    }
                }
            }
        }
    }
    keys
}

/// WHAT DRAWING THE ROWS OF A BOX TAKES: the box, how it is drawn, its fields and the refusal of its trial build.
pub(crate) struct Drawing<'a, 'b> {
    pub b: &'a FieldsBox<'b>,
    pub look: &'a BoxLook<'b>,
    pub trial: Option<&'a Refusal>,
}

/// WHAT THE ROWS OF A BOX CAME TO: the fields typed, the options touched, and whether the first field shown has taken
/// the keyboard it was given.
pub(crate) struct Drawn {
    pub typed: Typed,
    pub did: Vec<Did>,
    pub focus: Focus,
    columns: Lined,
}

/// The columns of a box as they stood the frame before, and as they come out in this one.
#[derive(Clone, Copy, Default)]
struct Lined {
    was: Columns,
    now: Columns,
}

/// DRAW THE ROWS OF A BOX: the fields of their own rows and the sections, a grid of the caption and the value each, the
/// title of a section a button that folds it.
pub(crate) fn draw_rows(ui: &mut egui::Ui, d: &Drawing, fields: &mut [CmdParam], layout: &Layout, focus: Focus) -> Drawn {
    let columns_id = d.b.id.with("the columns of the rows");
    let was = ui.ctx().data(|m| m.get_temp::<Columns>(columns_id)).unwrap_or_default();
    let mut drawn = Drawn { typed: Typed::default(), did: Vec::new(), focus, columns: Lined { was, now: Columns::default() } };
    let placed = sections_of(fields, d.b, layout);
    let mut lead = Vec::new();
    for p in &placed {
        if let Placed::Field { caption, key } = p {
            lead.push(Row::new(caption.clone()).field(key));
        }
    }
    if !lead.is_empty() {
        grid(ui, d, fields, &lead, &mut drawn);
    }
    for p in placed {
        let Placed::Section { at } = p else { continue };
        let s = &layout.sections[at];
        if let Some(title) = &s.title {
            let is_open = open(ui.ctx(), d.b, at, s);
            let mark = if is_open { ph::CARET_DOWN } else { ph::CARET_RIGHT };
            if ui.add(egui::Button::new(egui::RichText::new(format!("{mark} {title}")).strong()).frame(false)).clicked() {
                match s.switch {
                    Some(key) => drawn.did.push(Did::Switched { key }),
                    None => {
                        ui.ctx().data_mut(|m| m.insert_temp(fold_id(d.b, at), !is_open));
                    }
                }
            }
            if !is_open {
                continue;
            }
        }
        grid(ui, d, fields, &s.rows, &mut drawn);
    }
    let now = drawn.columns.now;
    ui.ctx().data_mut(|m| m.insert_temp(columns_id, now));
    if !layout.foot.is_empty() {
        ui.separator();
        // the foot wraps at the width of the rows over it: a count said in a sentence widened the box of a corner to 320
        // points, and the box laid by the rule for its first size came to stand over the corner
        let wide = ui.min_rect().width().max(UNDER_WORDS);
        ui.horizontal_wrapped(|ui| {
            ui.set_max_width(wide);
            drawn.did.extend(draw_opts(ui, d.look.pal, &layout.foot));
        });
    }
    drawn
}

/// HOW WIDE THE COLUMNS OF A BOX STOOD the frame before: the caption, the value with its unit and buttons, the name of
/// a driver - one for every section of the box, so the rows of all of them stand lined up. A row is laid from the left and how wide it comes out is known once it is laid; the columns of the
/// frame before line the rows up in this one. A box opening is laid unseen in its first frame, so the rows a person
/// sees are lined up from the first.
#[derive(Clone, Copy, Default)]
struct Columns {
    caption: f32,
    value: f32,
    name: f32,
}

/// THE ROOM BETWEEN THE COLUMNS of a row, and between what stands in one column.
const GAP: f32 = 6.0;

/// The cursor of row `ui` carried on to `x`, where it stands short of it.
fn up_to(ui: &mut egui::Ui, x: f32) {
    let short = x - ui.cursor().left();
    if short > 0.0 {
        ui.add_space(short);
    }
}

/// WHAT `draw` LAYS IN A ROW, its pieces `GAP` apart; answers how wide it came out. The row itself lays its columns with
/// no room of its own between them, so a column starts where the row says, to the point.
fn piece(ui: &mut egui::Ui, draw: impl FnOnce(&mut egui::Ui)) -> f32 {
    let from = ui.cursor().left();
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.x = GAP;
        draw(ui);
    });
    ui.cursor().left() - from
}

/// THE ROWS GIVEN, in three columns lined up down the box: the caption - or the button in its place - the value with its
/// unit and its buttons, and the name of a driver where the value has one; what a formula comes to, or why the value or
/// its name cannot be taken, on a line of its own under the row, across the value and the name. Each column is as wide
/// as the widest of it, measured the frame before by where the cursor stood.
fn grid(ui: &mut egui::Ui, d: &Drawing, fields: &mut [CmdParam], rows: &[Row], drawn: &mut Drawn) {
    let held = |fields: &[CmdParam], key: &str| fields.iter().any(|p| p.key == key && p.at.is_none() && !d.b.hidden.contains(&key));
    let was = drawn.columns.was;
    let mut now = drawn.columns.now;
    for line in lines_of(rows, d.b.names) {
        let row = match line {
            Line::Heading(words) => {
                ui.label(egui::RichText::new(words).weak());
                continue;
            }
            Line::Row(row) => row,
        };
        let row: &Row = &row;
        if !row.cells.iter().any(|c| matches!(c, Cell::Opt(_)) || matches!(c, Cell::Field { key, .. } if held(fields, key))) {
            continue;
        }
        let mut under: Option<Result<f64, String>> = None;
        let mut name_refused: Option<Unfit> = None;
        let mut row_value = 0.0_f32;
        let named: Vec<&FieldName> = row.cells.iter().filter_map(|c| if let Cell::Field { key, .. } = c { d.b.names.iter().find(|n| n.field == *key) } else { None }).collect();
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            let x0 = ui.cursor().left();
            now.caption = now.caption.max(piece(ui, |ui| match &row.lead {
                Some(o) => drawn.did.extend(draw_opts(ui, d.look.pal, std::slice::from_ref(o))),
                None => {
                    let r = ui.label(row.caption.as_str());
                    if let Some(hint) = fields.iter().find(|p| row_holds(row, p)).and_then(|p| p.hint) {
                        r.on_hover_text(qymcad_i18n::tr(hint));
                    }
                }
            }));
            let v0 = x0 + was.caption + GAP;
            up_to(ui, v0);
            // THE NAME COLUMN IS LINED UP BY THE ROWS THAT HAVE ONE: a row of X, Y and Z, or of choices, runs as long as it
            // is and pushes no name to its far end
            let lined = !named.is_empty();
            let value = piece(ui, |ui| {
                // the unit stands right after the last field of the row, before a switch or a button after it
                let last = row.cells.iter().rposition(|c| matches!(c, Cell::Field { .. }));
                for (n, c) in row.cells.iter().enumerate() {
                    match c {
                        Cell::Field { key, tag } => {
                            let Some(p) = fields.iter_mut().find(|p| p.key == *key && p.at.is_none() && !d.b.hidden.contains(&key.as_str())) else { continue };
                            if let Some(tag) = tag {
                                ui.label(egui::RichText::new(tag.as_str()).weak());
                            }
                            // A REFERENCE SHOWS ITS VALUE, it drives nothing: its value written in place of the field, as wide
                            if named.iter().any(|n| n.field == *key && driven_now(ui.ctx(), d.b.id, n) == Lit::On) {
                                ui.add_sized([FIELD_WIDTH, ui.spacing().interact_size.y], egui::Label::new(egui::RichText::new(format!("({})", qymcad_i18n::num(p.val, 3))).weak()));
                            } else {
                                let said = field_input(ui, d.look, p, field_id(d.b.tool, &p.key), Input { focus: drawn.focus, note: Note::Under });
                                under = Some(said.value.clone());
                                drawn.focus = Focus::Waits;
                                drawn.typed.add(judge(ui, d.look.pal, p, said, d.trial));
                            }
                        }
                        Cell::Opt(o) => {
                            let did = draw_opts(ui, d.look.pal, std::slice::from_ref(o));
                            // Enter in a text of the box - the string of a label edited - applies it, as Enter in a value does
                            if did.iter().any(|x| matches!(x, Did::Entered { .. })) {
                                drawn.typed.add(Typed::entered());
                            }
                            drawn.did.extend(did);
                        }
                    }
                    if Some(n) == last {
                        if let Some(unit) = row.unit.clone().or_else(|| unit_of_last(row, fields)) {
                            ui.label(egui::RichText::new(unit).weak());
                        }
                    }
                }
            });
            row_value = value;
            // THE NAME AND THE REFERENCE OF THE VALUE, in a column of their own lined up down the box
            if lined {
                now.value = now.value.max(value);
                up_to(ui, v0 + was.value + GAP * 2.0);
                now.name = now.name.max(piece(ui, |ui| {
                    for n in &named {
                        let says = name_cells(ui, d, n);
                        if name_refused.is_none() {
                            name_refused = says.unfit;
                        }
                        if says.enter == Enter::Pressed {
                            drawn.typed.add(Typed::entered());
                        }
                    }
                }));
            }
        });
        // WHAT A FORMULA COMES TO, or the words of why the value - or its name - cannot be taken, on a line of its own
        // under the row: a plain number says itself, and a line under every field left a blank line under each row
        let said = match under {
            Some(Ok(v)) if fields.iter().any(|p| row_holds(row, p) && p.txt.trim().parse::<f64>().is_err()) => Some(Notice::info(format!("= {}", qymcad_i18n::num(v, 3)))),
            Some(Err(why)) => Some(Notice::refusal(why)),
            _ => name_refused.map(|u| Notice::refusal(u.said)),
        };
        if let Some(said) = said {
            let across = (was.value + if was.name > 0.0 { GAP * 2.0 + was.name } else { 0.0 }).max(row_value).max(UNDER_WORDS);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.add_space(was.caption + GAP);
                ui.vertical(|ui| {
                    ui.set_max_width(across);
                    if said.tone == Tone::Info {
                        ui.label(egui::RichText::new(said.words).weak().small());
                    } else {
                        // the words stand in the hint too: read whole under the pointer, where a narrow box wraps them
                        say(ui, d.look.pal, &said, NoticeSize::Small).on_hover_text(said.words.clone());
                    }
                });
            });
        }
    }
    drawn.columns.now = now;
}

/// ONE LINE OF A SECTION: a row, or the caption of a row whose values stand a line each under it.
enum Line<'a> {
    Row(std::borrow::Cow<'a, Row>),
    Heading(String),
}

/// THE LINES OF THE ROWS: a row of several values each with its driver - X, Y and Z of a point - is its caption on a line
/// of its own and each value on a line under it, its tag for a caption, then its driver. On one line the three drivers
/// stood in a heap at its far end, and which was whose could not be told.
fn lines_of<'a>(rows: &'a [Row], names: &[FieldName]) -> Vec<Line<'a>> {
    let mut lines = Vec::new();
    for row in rows {
        let named = row.cells.iter().filter(|c| matches!(c, Cell::Field { key, .. } if names.iter().any(|n| n.field == *key))).count();
        if named < 2 {
            lines.push(Line::Row(std::borrow::Cow::Borrowed(row)));
            continue;
        }
        if !row.caption.is_empty() {
            lines.push(Line::Heading(row.caption.clone()));
        }
        for c in &row.cells {
            let one = match c {
                Cell::Field { key, tag } => Row { caption: tag.clone().unwrap_or_default(), lead: None, cells: vec![Cell::Field { key: key.clone(), tag: None }], unit: row.unit.clone() },
                Cell::Opt(o) => Row { caption: String::new(), lead: None, cells: vec![Cell::Opt(o.clone())], unit: None },
            };
            lines.push(Line::Row(std::borrow::Cow::Owned(one)));
        }
    }
    lines
}

/// The unit of the last field of `row` as the field itself names it, where the row names none.
fn unit_of_last(row: &Row, fields: &[CmdParam]) -> Option<String> {
    let key = row.cells.iter().rev().find_map(|c| match c {
        Cell::Field { key, .. } => Some(key),
        Cell::Opt(_) => None,
    })?;
    fields.iter().find(|p| p.key == *key)?.unit_key().map(|u| qymcad_i18n::tr(u).trim().to_string())
}

/// Does `row` hold field `p`.
fn row_holds(row: &Row, p: &CmdParam) -> bool {
    row.cells.iter().any(|c| matches!(c, Cell::Field { key, .. } if *key == p.key))
}

/// THE NARROWEST THE WORDS UNDER A ROW AND AT THE FOOT OF A BOX WRAP AT; wider rows give them their own width. A
/// refusal of a sentence or two stood as one line of 632 points, the box grew to it in the frame after it was first laid
/// narrow, and the rule laid the box anew elsewhere.
const UNDER_WORDS: f32 = 200.0;

/// HOW WIDE A FIELD OF A BOX IS, and the value of a reference written in its place.
pub(crate) const FIELD_WIDTH: f32 = 74.0;
