//! THE LAYOUT OF A BOX AT THE GEOMETRY, as its tool describes it: sections - a title, folded or not as the box opens,
//! and rows - each a caption and what stands after it: one field or several on one line (X, Y, Z), a group of choices,
//! a switch, a button. The frame draws the rows of a section as a grid of two columns, the caption and the value, and
//! the title of a section folds it.
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
}

impl Section {
    /// A section with no title - the rows at the top of a box.
    pub fn plain(rows: Vec<Row>) -> Section {
        Section { title: None, folded: Folded::No, rows }
    }

    /// A section under its title, folded or not as the box opens.
    pub fn titled(title: String, folded: Folded, rows: Vec<Row>) -> Section {
        Section { title: Some(title), folded, rows }
    }
}

/// DOES A SECTION STAND FOLDED as its box opens.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Folded {
    No,
    Yes,
}

/// ONE ROW OF A SECTION: its caption, what stands after it on one line, and the unit its numbers are in, after them.
pub struct Row {
    pub caption: String,
    pub cells: Vec<Cell>,
    pub unit: Option<String>,
    /// where what the formula of its field comes to is said: beside a formula, or under the row, the number included
    pub note: Note,
}

/// WHERE A ROW SAYS WHAT ITS FORMULA COMES TO, or why it cannot be taken: beside a formula, a plain number saying it
/// already - or on a line of its own under the row, always, as the box of a dimension says it (`= 42.500`, or the words
/// of the error where a mark beside the field would hide them under the pointer).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Note {
    #[default]
    Beside,
    Under,
}

/// WHAT STANDS IN A ROW: a field of the tool by its key, with a short tag before it where the row holds several (X, Y,
/// Z), or an option.
pub enum Cell {
    Field { key: String, tag: Option<String> },
    Opt(Opt),
}

impl Row {
    /// A row with its caption and nothing after it yet.
    pub fn new(caption: impl Into<String>) -> Row {
        Row { caption: caption.into(), cells: Vec::new(), unit: None, note: Note::Beside }
    }

    /// The same row saying what its formula comes to - or why it cannot be taken - on a line under it.
    pub fn said_under(mut self) -> Row {
        self.note = Note::Under;
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
        if !rows.is_empty() {
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
}

/// DRAW THE ROWS OF A BOX: the fields of their own rows and the sections, a grid of the caption and the value each, the
/// title of a section a button that folds it.
pub(crate) fn draw_rows(ui: &mut egui::Ui, d: &Drawing, fields: &mut [CmdParam], layout: &Layout, focus: Focus) -> Drawn {
    let mut drawn = Drawn { typed: Typed::default(), did: Vec::new(), focus };
    let placed = sections_of(fields, d.b, layout);
    let mut lead = Vec::new();
    for p in &placed {
        if let Placed::Field { caption, key } = p {
            lead.push(Row::new(caption.clone()).field(key));
        }
    }
    if !lead.is_empty() {
        grid(ui, d, fields, &lead, d.b.id.with("the rows of their own"), &mut drawn);
    }
    for p in placed {
        let Placed::Section { at } = p else { continue };
        let s = &layout.sections[at];
        if let Some(title) = &s.title {
            let is_open = open(ui.ctx(), d.b, at, s);
            let mark = if is_open { ph::CARET_DOWN } else { ph::CARET_RIGHT };
            if ui.add(egui::Button::new(egui::RichText::new(format!("{mark} {title}")).strong()).frame(false)).clicked() {
                ui.ctx().data_mut(|m| m.insert_temp(fold_id(d.b, at), !is_open));
            }
            if !is_open {
                continue;
            }
        }
        grid(ui, d, fields, &s.rows, d.b.id.with(("the rows of a section", at)), &mut drawn);
    }
    if !layout.foot.is_empty() {
        ui.separator();
        // the foot wraps at the width of the words under a field: a count said in a sentence widened the box of a corner
        // to 320 points, and the box laid by the rule for its first size came to stand over the corner
        ui.horizontal_wrapped(|ui| {
            ui.set_max_width(UNDER_WORDS);
            drawn.did.extend(draw_opts(ui, d.look.pal, &layout.foot));
        });
    }
    drawn
}

/// The rows given, as a grid of two columns: the caption, and the fields and options after it on one line.
fn grid(ui: &mut egui::Ui, d: &Drawing, fields: &mut [CmdParam], rows: &[Row], id: egui::Id, drawn: &mut Drawn) {
    let held = |fields: &[CmdParam], key: &str| fields.iter().any(|p| p.key == key && p.at.is_none() && !d.b.hidden.contains(&key));
    egui::Grid::new(id).num_columns(2).spacing(egui::vec2(10.0, 4.0)).show(ui, |ui| {
        for row in rows {
            if !row.cells.iter().any(|c| matches!(c, Cell::Opt(_)) || matches!(c, Cell::Field { key, .. } if held(fields, key))) {
                continue;
            }
            ui.label(row.caption.as_str());
            // the unit stands right after the last field of the row, before a switch or a button after it
            let last = row.cells.iter().rposition(|c| matches!(c, Cell::Field { .. }));
            let mut under: Option<Result<f64, String>> = None;
            ui.horizontal(|ui| {
                for (n, cell) in row.cells.iter().enumerate() {
                    match cell {
                        Cell::Field { key, tag } => {
                            let Some(p) = fields.iter_mut().find(|p| p.key == *key && p.at.is_none() && !d.b.hidden.contains(&key.as_str())) else { continue };
                            if let Some(tag) = tag {
                                ui.label(egui::RichText::new(tag.as_str()).weak());
                            }
                            let said = field_input(ui, d.look, p, field_id(d.b.tool, &p.key), Input { focus: drawn.focus, note: row.note });
                            if row.note == Note::Under {
                                under = Some(said.value.clone());
                            }
                            drawn.focus = Focus::Waits;
                            drawn.typed.add(judge(ui, d.look.pal, p, said, d.trial));
                        }
                        Cell::Opt(o) => drawn.did.extend(draw_opts(ui, d.look.pal, std::slice::from_ref(o))),
                    }
                    if Some(n) == last {
                        if let Some(unit) = &row.unit {
                            ui.label(egui::RichText::new(unit.as_str()).weak());
                        }
                    }
                }
            });
            ui.end_row();
            // WHAT A FORMULA COMES TO, or the words of why the value cannot be taken, on a line of its own under the row - a
            // plain number says itself, and a line under every field left the box with a blank line under each row
            let said = match under {
                Some(Ok(v)) if fields.iter().any(|p| row_holds(row, p) && p.txt.trim().parse::<f64>().is_err()) => Some(Ok(v)),
                Some(Err(why)) => Some(Err(why)),
                _ => None,
            };
            if let Some(said) = said {
                ui.label("");
                match said {
                    Ok(v) => {
                        ui.label(egui::RichText::new(format!("= {}", qymcad_i18n::num(v, 3))).weak().small());
                    }
                    Err(why) => under_words(ui, |ui| say(ui, d.look.pal, &Notice::refusal(why), NoticeSize::Small)),
                }
                ui.end_row();
            }
            // THE NAME OF A FIELD OF THE ROW, on a row of its own under it - a field hidden now keeps its name: a reference
            // dimension shows its value and is still read by its name
            for cell in &row.cells {
                let Cell::Field { key, .. } = cell else { continue };
                let Some(n) = d.b.names.iter().find(|n| n.field == *key) else { continue };
                let Some(at) = fields.iter().position(|p| p.key == *key) else { continue };
                let named = name_row(at, n);
                ui.label(named.caption.as_str());
                ui.horizontal(|ui| {
                    for c in &named.cells {
                        if let Cell::Opt(o) = c {
                            drawn.did.extend(draw_opts(ui, d.look.pal, std::slice::from_ref(o)));
                        }
                    }
                });
                ui.end_row();
                // WHY THE NAME WILL NOT DO, on a line of its own under it, as what a formula comes to stands under its field
                if let Some(u) = &n.unfit {
                    ui.label("");
                    under_words(ui, |ui| say(ui, d.look.pal, &Notice::warning(u.said.clone()), NoticeSize::Small).on_hover_text(&u.hint));
                    ui.end_row();
                }
            }
        }
    });
}

/// Does `row` hold field `p`.
fn row_holds(row: &Row, p: &CmdParam) -> bool {
    row.cells.iter().any(|c| matches!(c, Cell::Field { key, .. } if *key == p.key))
}

/// HOW WIDE THE WORDS UNDER A FIELD AND AT THE FOOT OF A BOX RUN before they wrap: a refusal of a sentence or two stood
/// as one line of 632 points, the box grew to it in the frame after it was first laid narrow, and the rule laid the box
/// anew elsewhere - under the pointer resting on the words there came the canvas. At 260 the box of a corner, 264 points
/// wide, found no side of the corner it stood clear on in a view of 310 points between the tree and the properties.
const UNDER_WORDS: f32 = 200.0;

/// Words under a field, wrapped at the one width every box keeps them to.
fn under_words(ui: &mut egui::Ui, words: impl FnOnce(&mut egui::Ui) -> egui::Response) {
    ui.vertical(|ui| {
        ui.set_max_width(UNDER_WORDS);
        words(ui);
    });
}
