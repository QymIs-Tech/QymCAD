//! THE FRAME OF A TOOL: the one bar a tool in hand stands in, the one box of its fields at the geometry, the one place
//! its words are said, and what Enter, Esc and the two buttons of a command do - written once for every tool of every
//! workbench.
//!
//! A tool describes itself as data ([`Described`]): its name, its options - modes, switches, references to pick,
//! buttons, readouts - the words it says with their tone, and its gate: what it still waits for, what its trial build
//! says. The frame draws all of it and answers what the person did: an option touched ([`Did`]), and the [`Verdict`] -
//! apply, cancel, a step back on Esc, or why not yet. A tool draws no widget of its own in its bar or its box.
//!
//! Before it each bar and each box was drawn by its tool: the bar of a feature, of a boolean, of a section, of a
//! pattern of parts, of a joint and the boxes of a feature, of a pattern of parts and of a joint each read Enter and Esc
//! and laid Apply and Cancel by itself, and Enter with nothing picked made nothing and said nothing in some of them,
//! refused in words in others.

use super::*;
use egui_phosphor::regular as ph;

mod bar;
pub use bar::*;
mod doors;
pub use doors::*;
mod fields;
pub use fields::*;
mod layout;
pub use layout::*;
mod names;
pub use names::*;
mod opts;
pub use opts::*;
mod place;
pub use place::*;
mod tool;
pub use tool::*;
mod words;
pub use words::*;

/// WHAT THE BAR CALLS THE TOOL IN HAND - a glyph of the icon font and the tool's name - how the bar ends, and what is in
/// hand once the tool is applied.
pub struct Head {
    pub icon: &'static str,
    pub name: String,
    pub ends: Ends,
    pub after: AfterApply,
}

/// WHAT IS IN HAND ONCE THE TOOL IS APPLIED: the selection, or the same tool again. The frame reads it after an Apply
/// that went through and puts the tool down by the tool's own door ([`FramedTool::put_down`]); no tool puts itself down
/// in its Apply, and no list of tools in the frame says which does.
///
/// Before it each tool chose in its own Apply: a joint kept left the joint tool in hand for the next pair, grounding and
/// an anchor stayed after Enter, while a boolean, a turn and a pattern of a sketch left the arrow.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AfterApply {
    /// A command that does one thing: once it is done the hand holds the selection - an operation of a part, a tool of
    /// an assembly, the turn and the pattern of a sketch.
    Select,
    /// A tool that draws or sizes one thing after another stays in hand for the next: a shape of a sketch, a dimension,
    /// the corner, the offset. Esc puts it down.
    Stays,
}

/// HOW THE BAR OF A TOOL ENDS.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Ends {
    /// Apply and Cancel: a command that makes something.
    ApplyCancel,
    /// One button that puts the tool down, "Close (Esc)": a measure has nothing to apply, a section once on is turned off,
    /// grounding takes its clicks as they come. Pressed, it is the cancel of the tool. A plain button, not the tone of a
    /// refusal: nothing is refused by putting such a tool down.
    Close,
    /// No button: a hand with nothing in it - the selection of a sketch - has nothing to apply or put down.
    Nothing,
}

/// A VALUE THE TOOL CANNOT TAKE: the key of its field, the field's caption and why.
#[derive(Clone, Debug, PartialEq)]
pub struct Refusal {
    pub key: String,
    pub field: String,
    pub why: String,
}

/// CAN THE TOOL IN HAND BE APPLIED NOW, and when it cannot, what stands in the way.
#[derive(Clone, Debug, PartialEq)]
pub enum Gate {
    Open,
    /// It waits for a pick or a choice: the words that ask for it.
    Waiting(String),
    /// What is picked will not do: the words that say why.
    Wrong(String),
    /// The trial build of the values has not answered yet.
    Checking,
    /// A value cannot be taken.
    Refused(Refusal),
    /// A field of the bar holds a value it cannot take.
    BarField,
}

impl Gate {
    /// THE GATE OF A TOOL as its picks and its trial build leave it: a value the geometry refuses first, then what is
    /// still to be picked (`ask`, the words that ask for it; none once all is picked), then the trial still running.
    /// `fields` name the field a refusal is said beside.
    pub fn of(ask: Option<String>, trial: &Trial, fields: &[CmdParam]) -> Gate {
        if let Trial::Refused(key, why) = trial {
            let field = fields.iter().find(|p| p.key == *key).map(field_caption).unwrap_or_default();
            return Gate::Refused(Refusal { key: key.clone(), field, why: why.clone() });
        }
        match (ask, trial) {
            (Some(ask), _) => Gate::Waiting(ask),
            (None, Trial::Checking) => Gate::Checking,
            _ => Gate::Open,
        }
    }

    /// THE SAME GATE WITH THE FIELDS JUDGED IN FRONT: a value a field cannot take comes first - a typo concerns what is
    /// being typed right now, and there is nothing to wait for. A value the tool already refused stands: a formula the
    /// document refuses - one reading a circle of names - works out to a stale number, and the limits of its field
    /// refused that number instead. Reported behaviour: a count of copies typed as "a", where a reads b and b reads a,
    /// was refused as "past the limits: from 1 to 512".
    pub fn with_fields(self, fields: &[CmdParam], project: &Project) -> Gate {
        if matches!(self, Gate::Refused(_)) {
            return self;
        }
        let vars = project.param_map();
        match fields.iter().find_map(|p| field_value(p, &vars).err().map(|why| (p, why))) {
            Some((p, why)) => Gate::Refused(Refusal { key: p.key.clone(), field: field_caption(p), why }),
            None => self,
        }
    }

    /// May the tool be applied?
    pub fn open(&self) -> bool {
        *self == Gate::Open
    }

    /// WHAT ENTER IS ANSWERED WITH while the gate is shut: what is wanted, or which value and why - in the tone of a
    /// refusal; a trial still running is said as what it is. Nothing while it is open: Enter applies.
    pub fn answer(&self) -> Option<Notice> {
        match self {
            Gate::Open => None,
            Gate::Waiting(ask) => Some(Notice::refusal(qymcad_i18n::tr1("cmd-not-ready", "ask", ask))),
            Gate::Wrong(why) => Some(Notice::refusal(why.clone())),
            Gate::Checking => Some(Notice::info(qymcad_i18n::tr("cmd-trial-checking"))),
            Gate::Refused(r) => Some(Notice::refusal(format!("{}: {}", r.field, r.why))),
            Gate::BarField => Some(Notice::refusal(qymcad_i18n::tr("sk-fix-bar-field"))),
        }
    }
}

/// WHAT THE PERSON ASKED THE TOOL FOR in this frame.
#[derive(Clone, Debug, PartialEq)]
pub enum Verdict {
    Stay,
    Apply,
    Cancel,
    /// Esc with no field holding the keyboard: a step back - a pick being made let go first, the tool next. The tool
    /// knows its own steps; one with none to let go puts itself down.
    Back,
    /// Enter while the gate is shut: the words that say why, already said where the tool speaks.
    Refused(Notice),
}

/// WHAT A FIELD OF A COMMAND HOLDS: its expression worked out, or in words why it cannot be taken - an expression that
/// does not work out, or a value past the field's limits. A value past the limits used to be pressed to the nearest
/// one and applied without a word: a fillet of 0 came in green with a radius of 0.05.
pub fn field_value(p: &CmdParam, vars: &std::collections::HashMap<String, f64>) -> Result<f64, String> {
    let v = qymcad_core::expr::eval(&p.txt, vars).map_err(|e| qymcad_i18n::error_words::expr_error_text(&e))?;
    if p.nonzero && v.abs() < 1e-9 {
        return Err(qymcad_i18n::tr("cmd-value-zero"));
    }
    if p.number == Number::Whole && (v - v.round()).abs() > 1e-9 {
        return Err(qymcad_i18n::tr("cmd-value-whole"));
    }
    if v < p.lo - 1e-9 || v > p.hi + 1e-9 {
        return Err(qymcad_i18n::tr2("cmd-value-out-of-range", "lo", &qymcad_core::expr::fmt_num(p.lo), "hi", &qymcad_core::expr::fmt_num(p.hi)));
    }
    Ok(v)
}

/// THE CAPTION OF A FIELD in the words of the language in force: whose value it is, where it says, and what.
pub fn field_caption(p: &CmdParam) -> String {
    match p.of_key() {
        Some(of) => format!("{} {}", qymcad_i18n::tr(of), qymcad_i18n::tr(p.label_key())),
        None => qymcad_i18n::tr(p.label_key()),
    }
}

/// Whether every field holds a value it can take. One that cannot leaves `p.val` at its OLD value, and the tool must
/// not be applied then - a stale value would go through silently.
pub fn fields_valid(fields: &[CmdParam], project: &Project) -> bool {
    let vars = project.param_map();
    fields.iter().all(|p| field_value(p, &vars).is_ok())
}

/// WAS ENTER PRESSED FOR THE TOOL.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Enter {
    #[default]
    Not,
    Pressed,
}

/// WHICH BUTTON OF THE COMMAND WAS PRESSED.
#[derive(Clone, Copy, PartialEq)]
enum Pressed {
    Nothing,
    Apply,
    Cancel,
}

/// WAS Esc PRESSED FOR THE TOOL - with no field holding the keyboard as the frame began; a field takes Esc first.
#[derive(Clone, Copy, PartialEq)]
enum Esc {
    Not,
    Pressed,
}

/// The verdict of a frame: a button pressed, Enter answered by the gate, Esc a step back - once in a frame
/// (`first_answer`). A refusal is remembered to be said where the tool speaks.
fn verdict(ctx: &egui::Context, pressed: Pressed, keys: Keys, gate: &Gate) -> Verdict {
    let v = match pressed {
        Pressed::Cancel => Verdict::Cancel,
        Pressed::Apply => Verdict::Apply,
        Pressed::Nothing if keys.enter == Enter::Pressed => gate.answer().map_or(Verdict::Apply, Verdict::Refused),
        Pressed::Nothing if keys.esc == Esc::Pressed => Verdict::Back,
        Pressed::Nothing => Verdict::Stay,
    };
    if v != Verdict::Stay && !first_answer(ctx) {
        return Verdict::Stay;
    }
    if let Verdict::Refused(words) = &v {
        remember_refusal(ctx, gate, words);
    }
    v
}

/// THE KEYS OF A FRAME the tool answers.
#[derive(Clone, Copy)]
struct Keys {
    enter: Enter,
    esc: Esc,
}

/// THE FRAME ANSWERS A PRESS ONCE IN A FRAME. egui may run a frame twice over the same input - the first pass of a box
/// just come up is laid out and thrown away - and what a pass did to the document is not thrown away with it. Measured
/// on the split of a body: the box applied the command in the first pass, and the bar, the keyboard free by then,
/// answered the same Enter again in the second.
fn first_answer(ctx: &egui::Context) -> bool {
    let frame = ctx.cumulative_frame_nr();
    let id = egui::Id::new("the frame the tool in hand was last answered in");
    ctx.data_mut(|d| {
        let first = d.get_temp::<u64>(id) != Some(frame);
        d.insert_temp(id, frame);
        first
    })
}

/// THE BUTTON A COMMAND IS APPLIED WITH, in the colour the scheme gives it.
///
/// Reported behaviour: "highlight the Enter (apply) and Esc (cancel) buttons in the tool popups with different colours,
/// green and red say, so that it is clear". They used to differ by a tick glyph and a bold face - a difference one READS
/// rather than sees, and at the moment of pressing nobody is reading.
///
/// THE COLOUR GOES ON THE TEXT AND THE OUTLINE, not on the fill: text on the panel background is exactly the pair the
/// scheme's legibility check already measures.
pub fn confirm_button(pal: &qymcad_scheme::Palette, ui: &mut egui::Ui, can: Can, label: String) -> bool {
    let text = egui::RichText::new(label).color(pal.confirm()).strong();
    ui.add_enabled(can == Can::Yes, egui::Button::new(text).stroke(egui::Stroke::new(1.0, pal.confirm()))).clicked()
}

/// MAY A BUTTON OR A CHOICE BE PRESSED NOW.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Can {
    Yes,
    No,
}

/// THE BUTTON A COMMAND IS PUT DOWN WITH, in the colour the scheme gives it.
pub fn refuse_button(pal: &qymcad_scheme::Palette, ui: &mut egui::Ui, label: String) -> bool {
    let text = egui::RichText::new(label).color(pal.refuse());
    ui.add(egui::Button::new(text).stroke(egui::Stroke::new(1.0, pal.refuse()))).clicked()
}

/// The buttons the bar of a tool ends with, laid from the right: Apply, enabled by `can`, outermost and Cancel beside
/// it - or the one button that puts the tool down.
fn the_ends(pal: &qymcad_scheme::Palette, ui: &mut egui::Ui, ends: Ends, can: Can) -> Pressed {
    match ends {
        Ends::ApplyCancel => the_two_buttons(pal, ui, can),
        Ends::Close if ui.button(format!("{} {}", ph::X, qymcad_i18n::tr("cmd-close-esc"))).clicked() => Pressed::Cancel,
        Ends::Close | Ends::Nothing => Pressed::Nothing,
    }
}

/// The two buttons a command ends with: Apply, enabled by `can`, and Cancel. Laid from the right, Apply outermost.
fn the_two_buttons(pal: &qymcad_scheme::Palette, ui: &mut egui::Ui, can: Can) -> Pressed {
    let mut pressed = Pressed::Nothing;
    if confirm_button(pal, ui, can, format!("{} {}", ph::CHECK, qymcad_i18n::tr("cmd-apply-enter"))) {
        pressed = Pressed::Apply;
    }
    // THE BUTTON DOES WHAT THE KEY DOES. Reported behaviour: pressing it changes nothing - the popup stays, the command
    // stays. A button that answers a click with nothing says the way out is here, and it is not.
    if refuse_button(pal, ui, format!("{} {}", ph::X, qymcad_i18n::tr("cmd-cancel-esc"))) {
        pressed = Pressed::Cancel;
    }
    pressed
}

/// HOW TALL THE ROW OF THE BAR OF THE COMMAND STANDS, inside its frame: the tallest control a row holds - an outlined
/// button with its padding. Measured on the bars of the part and the assembly at 1280 points across: a row stood 24-26
/// tall with its frame, by what it held, and the bar came and went with the tool - the view under it jumped by as much
/// each time.
pub fn command_bar_row(ui: &egui::Ui) -> f32 {
    let s = ui.spacing();
    // the outlined Apply and Cancel stand 2 points taller than a plain button: their strokes are laid outside it
    s.interact_size.y.max(ui.text_style_height(&egui::TextStyle::Button) + 2.0 * s.button_padding.y) + 2.0
}

/// WHAT IS IN HAND as far as a click on the model goes: nothing - the click selects - or a tool, which takes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InHand {
    Nothing,
    Tool,
}
