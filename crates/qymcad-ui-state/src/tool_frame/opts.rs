//! THE OPTIONS OF A TOOL, DESCRIBED: modes, switches, references to pick, buttons, readouts and numbers of the bar - as
//! data the tool gives and the frame draws. The tool is told what was touched ([`Did`]) and changes its own state; it
//! draws nothing itself, so every option of every tool looks and answers alike.

use super::*;

/// WHICH OPTION OF A TOOL: its name, and its place among the options of one name - the third section of a loft.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OptKey {
    pub name: &'static str,
    pub at: usize,
}

impl OptKey {
    /// The one option of this name.
    pub const fn of(name: &'static str) -> Self {
        OptKey { name, at: 0 }
    }

    /// The option of this name at place `at` among the ones of its name.
    pub const fn at(name: &'static str, at: usize) -> Self {
        OptKey { name, at }
    }
}

/// ONE MODE OF A GROUP: its words, the longer explanation the hover shows, and whether it can be chosen now - a
/// choice that cannot is dimmed, not hidden, so the group keeps its place.
#[derive(Clone, Debug, PartialEq)]
pub struct Mode {
    pub word: String,
    pub hint: Option<String>,
    pub can: Can,
}

impl Mode {
    /// A choice that can be made, with no hint.
    pub fn new(word: impl Into<String>) -> Self {
        Mode { word: word.into(), hint: None, can: Can::Yes }
    }

    /// The same, with the longer explanation the hover shows.
    pub fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// The same, made or not as `can` says.
    pub fn can(mut self, can: Can) -> Self {
        self.can = can;
        self
    }
}

/// IS A SWITCH ON, OR A PICK BEING MADE.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lit {
    On,
    Off,
}

/// A FLAG OF A TOOL READ AS A SWITCH OF ITS BAR: `pc.feat.flip.lit()`.
pub trait AsLit {
    fn lit(self) -> Lit;
}

impl AsLit for bool {
    fn lit(self) -> Lit {
        if self {
            Lit::On
        } else {
            Lit::Off
        }
    }
}

/// ONE OPTION OF A TOOL.
#[derive(Clone, Debug, PartialEq)]
pub enum Opt {
    /// A group of named choices, one in force (or none, where nothing is chosen yet). More choices than a row holds
    /// well are drawn as a list.
    Modes { key: OptKey, caption: Option<String>, choices: Vec<Mode>, current: Option<usize> },
    /// A switch, on or off: a tick box.
    Switch { key: OptKey, word: String, hint: Option<String>, lit: Lit },
    /// A reference picked in the view: what it asks for, what is taken, whether the pick is being made now. What is
    /// taken can be let go.
    Pick { key: OptKey, word: String, taken: Option<String>, lit: Lit, hint: Option<String> },
    /// A button that does one thing.
    Press { key: OptKey, word: String, hint: Option<String> },
    /// Words shown as they are: what is taken, how many, what was found.
    Readout(String),
    /// A number of the bar, an expression, for a tool with no place in the view to set it at - the ratio of a relation.
    Number { key: OptKey, caption: String, value: f64, lo: f64, hi: f64 },
}

impl Opt {
    /// A group of choices by their catalogue keys, `current` in force.
    pub fn modes(key: OptKey, caption: Option<&str>, words: &[&str], current: usize) -> Opt {
        Opt::Modes { key, caption: caption.map(qymcad_i18n::tr), choices: words.iter().map(|w| Mode::new(qymcad_i18n::tr(w))).collect(), current: Some(current) }
    }

    /// A switch by its words, lit or not.
    pub fn switch(key: OptKey, word: impl Into<String>, lit: Lit) -> Opt {
        Opt::Switch { key, word: word.into(), hint: None, lit }
    }

    /// A reference picked in the view by its words, what it has taken, and whether the pick is being made.
    pub fn pick(key: OptKey, word: impl Into<String>, taken: Option<String>, lit: Lit) -> Opt {
        Opt::Pick { key, word: word.into(), taken, lit, hint: None }
    }

    /// A button by its words.
    pub fn press(key: OptKey, word: impl Into<String>) -> Opt {
        Opt::Press { key, word: word.into(), hint: None }
    }

    /// The same option with the longer explanation the hover shows - a switch, a pick or a button.
    pub fn hint(mut self, text: impl Into<String>) -> Opt {
        match &mut self {
            Opt::Switch { hint, .. } | Opt::Pick { hint, .. } | Opt::Press { hint, .. } => *hint = Some(text.into()),
            Opt::Modes { .. } | Opt::Readout(_) | Opt::Number { .. } => {}
        }
        self
    }
}

/// WHAT WAS DONE TO AN OPTION IN A FRAME.
#[derive(Clone, Debug, PartialEq)]
pub enum Did {
    /// A choice of a group was taken.
    Chose { key: OptKey, index: usize },
    /// A switch was turned over, or a pick begun or given up.
    Switched { key: OptKey },
    /// A button was pressed.
    Pressed { key: OptKey },
    /// What a pick has taken was let go.
    Cleared { key: OptKey },
    /// A number of the bar was given a value it can take.
    Typed { key: OptKey, value: f64 },
}

/// The most choices a group shows in a row; more are a list.
const ROW_OF_CHOICES: usize = 6;

/// DRAW THE OPTIONS in a row of the bar or of a box, a separator between them, and answer what was done to them.
/// `numbers` is where a number of the bar keeps its text; a box has none, and shows a number as a readout.
pub(crate) fn draw_opts(ui: &mut egui::Ui, opts: &[Opt], mut numbers: Option<&mut ExprBarCtx>) -> Vec<Did> {
    let mut did = Vec::new();
    for (i, o) in opts.iter().enumerate() {
        if i > 0 {
            ui.separator();
        }
        match o {
            Opt::Modes { key, caption, choices, current } => {
                if let Some(c) = caption {
                    ui.label(c);
                }
                if choices.len() > ROW_OF_CHOICES {
                    let shown = current.and_then(|c| choices.get(c)).map(|c| c.word.clone()).unwrap_or_default();
                    egui::ComboBox::from_id_salt(("the choices of a tool", key.name, key.at)).selected_text(shown).show_ui(ui, |ui| {
                        for (n, c) in choices.iter().enumerate() {
                            if ui.add_enabled(c.can == Can::Yes, egui::Button::selectable(*current == Some(n), c.word.as_str())).clicked() {
                                did.push(Did::Chose { key: *key, index: n });
                            }
                        }
                    });
                } else {
                    for (n, c) in choices.iter().enumerate() {
                        let r = ui.add_enabled(c.can == Can::Yes, egui::Button::selectable(*current == Some(n), c.word.as_str()));
                        let r = match &c.hint {
                            Some(h) => r.on_hover_text(h),
                            None => r,
                        };
                        if r.clicked() && *current != Some(n) {
                            did.push(Did::Chose { key: *key, index: n });
                        }
                    }
                }
            }
            // a switch is a tick box: on or off, read at a glance as every tick box of the program
            Opt::Switch { key, word, hint, lit } => {
                let mut on = *lit == Lit::On;
                if hinted(ui.checkbox(&mut on, word.as_str()), hint).clicked() {
                    did.push(Did::Switched { key: *key });
                }
            }
            // a pick names what it has taken, or what it asks for while nothing is
            Opt::Pick { key, word, taken, lit, hint } => {
                let words = format!("{} {}", ph::CROSSHAIR, taken.as_deref().unwrap_or(word));
                if hinted(ui.add(egui::Button::selectable(*lit == Lit::On, words)), hint).clicked() {
                    did.push(Did::Switched { key: *key });
                }
                if taken.is_some() && ui.small_button(ph::X).on_hover_text(qymcad_i18n::tr("cmd-reset")).clicked() {
                    did.push(Did::Cleared { key: *key });
                }
            }
            Opt::Press { key, word, hint } => {
                if hinted(ui.button(word.as_str()), hint).clicked() {
                    did.push(Did::Pressed { key: *key });
                }
            }
            Opt::Readout(words) => {
                ui.label(words.as_str());
            }
            Opt::Number { key, caption, value, lo, hi } => {
                ui.label(caption.as_str());
                match numbers.as_deref_mut() {
                    Some(n) => {
                        let v = num_or_expr(n, ui, key.name, *value, NumFormat { lo: *lo, hi: *hi, integer: false, suffix: "", nonzero: false });
                        if (v - value).abs() > 1e-12 {
                            did.push(Did::Typed { key: *key, value: v });
                        }
                    }
                    None => {
                        ui.label(qymcad_core::expr::fmt_num(*value));
                    }
                }
            }
        }
    }
    did
}

/// A widget with its longer explanation on hover, where it has one.
fn hinted(r: egui::Response, hint: &Option<String>) -> egui::Response {
    match hint {
        Some(h) => r.on_hover_text(h),
        None => r,
    }
}
