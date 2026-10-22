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
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Lit {
    On,
    #[default]
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
    /// A switch of one sign beside a value - the lock of a degree of a joint: a button that stays pressed while on.
    Toggle { key: OptKey, word: String, hint: Option<String>, lit: Lit },
    /// A reference picked in the view: what it asks for, what is taken, whether the pick is being made now. What is
    /// taken can be let go.
    Pick { key: OptKey, word: String, taken: Option<String>, lit: Lit, hint: Option<String> },
    /// A button that does one thing.
    Press { key: OptKey, word: String, hint: Option<String> },
    /// Words shown as they are: what is taken, how many, what was found.
    Readout(String),
    /// A count told at the end of the bar, beside its ends: how many things are selected.
    Tally(String),
    /// A number of the tool - the ratio of a relation, the sides of a polygon to draw: its caption by its catalogue key.
    /// The bar holds no field: the frame makes it a field of the box at the geometry (`numbers_into_the_box`), a formula
    /// judged as any field of a box is.
    Number { key: OptKey, label: &'static str, value: f64, limits: Limits },
    /// A line of text typed - the name a formula reads a value by, the words of a text drawn: the text as typed so far,
    /// kept by the tool, what the empty field shows, and why it will not do. A text that will not do keeps the keyboard
    /// on Enter, its words under the field.
    Text { key: OptKey, text: String, placeholder: String, unfit: Option<Unfit>, hint: Option<String>, focus: Focus },
}

/// WHAT A NUMBER OF THE BAR TAKES: its limits, whether it is whole, whether zero is a value, and the catalogue key of
/// its unit written after it, none for a plain count.
#[derive(Clone, Debug, PartialEq)]
pub struct Limits {
    pub range: std::ops::RangeInclusive<f64>,
    pub number: Number,
    pub zero: Zero,
    pub unit: Option<&'static str>,
}

impl Limits {
    /// Any number of `range`, zero among them, with no unit.
    pub fn between(range: std::ops::RangeInclusive<f64>) -> Limits {
        Limits { range, number: Number::Real, zero: Zero::Takes, unit: None }
    }
}

/// IS ZERO A VALUE of a number of the bar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Zero {
    Takes,
    Refused,
}

/// WHY A TEXT WILL NOT DO: the line said under it, and the longer explanation its hover shows.
#[derive(Clone, Debug, PartialEq)]
pub struct Unfit {
    pub said: String,
    pub hint: String,
}

/// The id of the field of text option `key`: a check reaches the field by it.
pub fn text_field_id(key: OptKey) -> egui::Id {
    egui::Id::new(("a text typed into a tool", key.name, key.at))
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

    /// A switch of one sign, a button that stays pressed while on.
    pub fn toggle(key: OptKey, word: impl Into<String>, lit: Lit) -> Opt {
        Opt::Toggle { key, word: word.into(), hint: None, lit }
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
            Opt::Switch { hint, .. } | Opt::Toggle { hint, .. } | Opt::Pick { hint, .. } | Opt::Press { hint, .. } | Opt::Text { hint, .. } => *hint = Some(text.into()),
            Opt::Modes { .. } | Opt::Readout(_) | Opt::Tally(_) | Opt::Number { .. } => {}
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
    /// A text was typed into, and holds this now.
    Retyped { key: OptKey, text: String },
    /// Enter was pressed in a name that will do.
    Entered { key: OptKey, text: String },
}

/// The most choices a group shows in a row; more are a list.
const ROW_OF_CHOICES: usize = 6;

/// DRAW THE OPTIONS in a row of the bar or of a box, a separator between them, and answer what was done to them.
pub(crate) fn draw_opts(ui: &mut egui::Ui, pal: &qymcad_scheme::Palette, opts: &[Opt]) -> Vec<Did> {
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
                        // the way already chosen, clicked again, is told too: a drawing begun in it starts anew
                        if r.clicked() {
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
            Opt::Toggle { key, word, hint, lit } => {
                if hinted(ui.add(egui::Button::selectable(*lit == Lit::On, word.as_str())), hint).clicked() {
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
            // WORDS BESIDE THE OPTIONS ARE READ LESS THAN THE OPTIONS, and are drawn so: a hint, a count, a value shown
            Opt::Readout(words) => {
                ui.label(egui::RichText::new(words.as_str()).weak());
            }
            // a count stands at the end of the bar, beside its ends (`bar`)
            Opt::Tally(_) => {}
            // a number is a field of the box, never drawn as an option (`numbers_into_the_box`)
            Opt::Number { .. } => {}
            Opt::Text { key, text, placeholder, unfit, hint, focus } => did.extend(text_opt(ui, pal, TextOpt { key: *key, text, placeholder, unfit: unfit.as_ref(), hint, focus: *focus })),
        }
    }
    did
}

/// A TEXT OPTION AS IT IS DRAWN: its key, the text the tool keeps, what the empty field shows, why it will not do, and
/// the longer explanation its hover shows.
struct TextOpt<'a> {
    key: OptKey,
    text: &'a str,
    placeholder: &'a str,
    unfit: Option<&'a Unfit>,
    hint: &'a Option<String>,
    focus: Focus,
}

/// THE FIELD OF A TEXT: the text the tool keeps, no list of names - a name is not a formula, and the list would take the
/// Enter the name is given with. A text that will not do keeps the keyboard and its letters on Enter, and the line under
/// it says why.
fn text_opt(ui: &mut egui::Ui, pal: &qymcad_scheme::Palette, o: TextOpt) -> Vec<Did> {
    let mut did = Vec::new();
    let mut typed = o.text.to_string();
    let r = ui.vertical(|ui| {
        let r = hinted(ui.add_sized([TEXT_WIDTH, ui.spacing().interact_size.y], egui::TextEdit::singleline(&mut typed).id(text_field_id(o.key)).hint_text(o.placeholder)), o.hint);
        // a text opened for editing takes the keyboard, all of it selected: what is typed stands instead of it
        if o.focus == Focus::Takes {
            r.request_focus();
            if let Some(mut st) = egui::TextEdit::load_state(ui.ctx(), r.id) {
                st.cursor.set_char_range(Some(egui::text::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(typed.chars().count()))));
                st.store(ui.ctx(), r.id);
            }
        }
        if let Some(u) = o.unfit {
            say(ui, pal, &Notice::warning(u.said.clone()), NoticeSize::Small).on_hover_text(&u.hint);
        }
        r
    });
    let r = r.inner;
    if typed != o.text {
        did.push(Did::Retyped { key: o.key, text: typed.clone() });
    }
    if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
        match o.unfit {
            Some(_) => r.request_focus(),
            None => did.push(Did::Entered { key: o.key, text: typed }),
        }
    }
    did
}

/// How wide the field of a text option is: the string of a text drawn, a name.
const TEXT_WIDTH: f32 = 160.0;

/// A widget with its longer explanation on hover, where it has one.
fn hinted(r: egui::Response, hint: &Option<String>) -> egui::Response {
    match hint {
        Some(h) => r.on_hover_text(h),
        None => r,
    }
}
