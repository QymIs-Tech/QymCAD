//! THE BAR OF THE PATH: where the person stands - the steps from the top of the document down to what is open, each
//! a button that goes back to it - the button that leaves what is open, and the switches the workbenches give, group
//! by group with a line between. The host gives the words, the icons and the switches; the bar lays them out and says
//! what was asked.

/// ONE STEP OF THE PATH: its icon, its name and what it is to the person.
pub struct Crumb {
    pub icon: &'static str,
    pub name: String,
    pub step: Step,
}

/// WHAT A STEP OF THE PATH IS to the person.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Step {
    /// A step above where the person stands: a button that goes back to it.
    Above,
    /// The step where the person stands: a button, lit.
    Here,
    /// What is open for editing inside the last step: named in the colour of a hint, not a button - the button that
    /// leaves closes it.
    Open,
}

/// THE BUTTON THAT LEAVES what is open: its word and its hint.
pub struct Leave {
    pub word: String,
    pub hint: String,
}

/// THE BAR OF THE PATH to lay out: the steps, the button that leaves when there is something to leave, the colour of
/// what is open.
pub struct PathBar<'a> {
    pub crumbs: &'a [Crumb],
    pub leave: Option<Leave>,
    pub open_colour: egui::Color32,
}

/// WHAT THE PERSON ASKED of the bar of the path.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PathAsk {
    /// Go back to the step at this place of the path.
    GoTo(usize),
    /// Leave what is open.
    Leave,
}

/// Did a group of switches draw anything?
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Drew {
    Something,
    Nothing,
}

/// THE SWITCHES OF THE BAR, group by group: a line after every group that drew something, none after one that did not.
pub struct Switches<'u> {
    ui: &'u mut egui::Ui,
}

impl Switches<'_> {
    /// Lay one group of switches.
    pub fn group(&mut self, add: impl FnOnce(&mut egui::Ui) -> Drew) {
        if add(self.ui) == Drew::Something {
            self.ui.separator();
        }
    }
}

/// LAY THE BAR OF THE PATH, then the switches `add` gives, in lines that wrap: a bar put at a side lays its words in
/// lines rather than running out of its strip.
pub fn path_bar(ui: &mut egui::Ui, bar: PathBar, add: impl FnOnce(&mut Switches)) -> Option<PathAsk> {
    let mut asked = None;
    ui.horizontal_wrapped(|ui| {
        for (i, crumb) in bar.crumbs.iter().enumerate() {
            if i > 0 {
                ui.label("›");
            }
            let words = format!("{} {}", crumb.icon, crumb.name);
            match crumb.step {
                Step::Open => {
                    ui.label(egui::RichText::new(words).color(bar.open_colour));
                }
                step => {
                    if ui.selectable_label(step == Step::Here, words).clicked() {
                        asked = Some(PathAsk::GoTo(i));
                    }
                }
            }
        }
        if let Some(leave) = &bar.leave {
            if ui.button(&leave.word).on_hover_text(&leave.hint).clicked() {
                asked = Some(PathAsk::Leave);
            }
        }
        ui.separator();
        add(&mut Switches { ui });
    });
    asked
}
