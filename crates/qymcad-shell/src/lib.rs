//! THE INTERFACE SHELL: it knows KINDS OF PLACES, never a list of panels.
//!
//! Before this, every panel opened its own container in the middle of the frame - `Panel::left("tree")`
//! inside `tree_panel`, `Panel::bottom("status")` written straight into `draw_frame`, and so on for
//! fourteen of them, each with its own size, framing and separator. Two things follow from that, and
//! both are bad.
//!
//! A panel that opens its own container cannot be MOVED: where it sits is written inside it, so a
//! layout the person chooses is impossible without editing the panel. And the frame becomes a flat run
//! of forty calls whose ORDER is load-bearing (egui wants the central panel last) while nothing says
//! so - the order lives in the sequence of statements and nowhere else.
//!
//! Here the shell owns the places and the workbench REGISTERS into them. The shell is handed a key and
//! the shape of the container wanted; it opens the container and hands back a `Ui` bounded by it. It
//! never learns what the panel draws, and - checked by a test over this file - it never learns the name
//! of a single workbench.
use std::collections::BTreeMap;

/// The kinds of place an interface has. Not a list of panels: a KIND, of which there may be many.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, serde::Serialize, serde::Deserialize)]
pub enum Slot {
    /// The menu, above everything.
    Menu,
    /// A bar across the top: the toolbar, the bars of an active command.
    Top,
    /// A strip down the left: the tree, the workbench's own tools.
    Left,
    /// A strip down the right: properties.
    Right,
    /// A bar across the bottom: the status line.
    Bottom,
    /// What is left in the middle. Opened LAST, which is egui's rule and the reason the order of the
    /// frame used to matter without saying so.
    Centre,
}

impl Slot {
    /// The catalogue key holding this kind's name.
    pub fn key(self) -> &'static str {
        match self {
            Slot::Menu => "slot-menu",
            Slot::Top => "slot-top",
            Slot::Left => "slot-left",
            Slot::Right => "slot-right",
            Slot::Bottom => "slot-bottom",
            Slot::Centre => "slot-centre",
        }
    }

    /// The order the frame goes in. The middle is last.
    pub const ORDER: [Slot; 6] = [Slot::Menu, Slot::Top, Slot::Bottom, Slot::Left, Slot::Right, Slot::Centre];
}

/// A place asked for by a workbench: where it goes and what the container looks like.
///
/// The size, the resizing and the separator live HERE rather than inside the panel, because they are
/// properties of the container and not of what is drawn in it. That is the whole point: the same panel
/// in a different place gets a different container and does not notice.
pub struct Place {
    /// What the host will be asked to fill. Opaque to the shell - it never reads it, only hands it back.
    pub key: &'static str,
    pub slot: Slot,
    /// The size across the strip: exact when `resizable` is off, the starting width when it is on.
    pub size: Option<f32>,
    pub resizable: bool,
    /// Drawn in the host's bar frame (a toolbar look) rather than the plain one.
    pub framed: bool,
    /// The hairline between this place and its neighbour.
    pub separator: bool,
}

impl Place {
    pub fn new(key: &'static str, slot: Slot) -> Self {
        Self { key, slot, size: None, resizable: false, framed: false, separator: true }
    }
    pub fn sized(mut self, w: f32, resizable: bool) -> Self {
        self.size = Some(w);
        self.resizable = resizable;
        self
    }
    pub fn framed(mut self) -> Self {
        self.framed = true;
        self
    }
    pub fn bare(mut self) -> Self {
        self.separator = false;
        self
    }
}

/// What the shell needs from whoever owns the panels. Three questions, none of them about workbenches.
pub trait Fills {
    /// Is this place wanted at all this frame? A command bar exists only while its command is running,
    /// and an empty container still eats a strip of the window, so the question is asked before opening.
    fn live(&self, key: &'static str) -> bool;
    /// Fill it. The `Ui` is already bounded by the container.
    fn fill(&mut self, key: &'static str, ui: &mut egui::Ui);
    /// The frame a bar-like place is drawn in. It carries the colours, which belong to the host.
    fn bar_frame(&self) -> egui::Frame;
}

/// The layout: the places as registered, plus wherever the person has since moved them.
#[derive(Default)]
pub struct Shell {
    places: Vec<Place>,
    /// The person's changes, kept apart from the registration so that "put it back" is a deletion and
    /// not a second copy of the defaults that would drift from them.
    moved: BTreeMap<String, Slot>,
}

impl Shell {
    /// Register a place. Called by a workbench when it starts, not by the shell.
    pub fn put(&mut self, p: Place) {
        self.places.push(p);
    }

    /// Every registered key, in registration order. The layout editor walks THIS rather than a list of
    /// its own: a place added by any workbench shows up in the settings without that code being touched.
    pub fn keys(&self) -> Vec<&'static str> {
        self.places.iter().map(|p| p.key).collect()
    }

    /// Where a place stands now: what the person chose, or what was registered.
    pub fn slot_of(&self, key: &str) -> Option<Slot> {
        if let Some(s) = self.moved.get(key) {
            return Some(*s);
        }
        self.places.iter().find(|p| p.key == key).map(|p| p.slot)
    }

    /// Move a place. The registration is untouched, so "put it back" stays possible.
    pub fn move_to(&mut self, key: &str, slot: Slot) {
        if self.places.iter().any(|p| p.key == key) {
            self.moved.insert(key.to_string(), slot);
        }
    }

    /// Put everything back as the workbenches registered it.
    pub fn reset(&mut self) {
        self.moved.clear();
    }

    /// The person's layout, for saving. Only the CHANGES: the defaults come from the code and would
    /// otherwise be frozen into a file and go stale the moment a panel is added.
    pub fn saved(&self) -> Vec<(String, Slot)> {
        self.moved.iter().map(|(k, s)| (k.clone(), *s)).collect()
    }

    /// Restore a saved layout. Keys nobody registered are dropped - a panel may have been removed since.
    pub fn restore(&mut self, saved: Vec<(String, Slot)>) {
        self.moved = saved.into_iter().filter(|(k, _)| self.places.iter().any(|p| p.key == *k)).collect();
    }

    /// Draw one kind of place, in registration order. The frame calls this six times, in `Slot::ORDER`.
    pub fn run_slot(&self, slot: Slot, ui: &mut egui::Ui, host: &mut dyn Fills) {
        let keys: Vec<&'static str> = self
            .places
            .iter()
            .filter(|p| self.moved.get(p.key).copied().unwrap_or(p.slot) == slot)
            .map(|p| p.key)
            .collect();
        for key in keys {
            if !host.live(key) {
                continue;
            }
            let p = self.places.iter().find(|p| p.key == key).expect("the key came from the same list");
            let bar = host.bar_frame();
            match slot {
                Slot::Centre => {
                    egui::CentralPanel::default().show(ui, |ui| host.fill(key, ui));
                }
                _ => {
                    let mut panel = match slot {
                        Slot::Menu | Slot::Top => egui::Panel::top(key),
                        Slot::Bottom => egui::Panel::bottom(key),
                        Slot::Left => egui::Panel::left(key),
                        Slot::Right => egui::Panel::right(key),
                        Slot::Centre => unreachable!("handled above"),
                    };
                    if let Some(w) = p.size {
                        panel = if p.resizable { panel.default_size(w).resizable(true) } else { panel.exact_size(w).resizable(false) };
                    }
                    if p.framed {
                        panel = panel.frame(bar);
                    }
                    if !p.separator {
                        panel = panel.show_separator_line(false);
                    }
                    panel.show(ui, |ui| host.fill(key, ui));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// THE SHELL DOES NOT KNOW A SINGLE WORKBENCH BY NAME.
    ///
    /// This is the whole rule of the file, and it can only be held by reading the file: a shell that
    /// mentions a workbench is a shell that will grow a second mention, and then the workbench cannot be
    /// taken out without editing it. The keys travel through as opaque strings for exactly this reason.
    #[test]
    fn the_shell_names_no_workbench() {
        let src = include_str!("lib.rs");
        // THE CODE, WITHOUT THE PROSE. The doc comments name a tree and a toolbar on purpose - as examples
        // of what MIGHT stand in a place - and forbidding that would forbid explaining the file. And below
        // the test module the panels are named deliberately, to check that a key travels through untouched.
        let code: String = src
            .split("\n#[cfg(test)]")
            .next()
            .expect("the working part is the file above its tests")
            .lines()
            .map(|l| l.trim_start())
            .filter(|l| !l.starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        // WHOLE WORDS. A plain `contains("part")` found it inside "apart" and reported the shell for a word
        // it had not said - a needle that fires on prose teaches everyone to ignore it.
        let words: Vec<String> = code.split(|c: char| !c.is_alphanumeric() && c != '_').map(|w| w.to_lowercase()).collect();
        for name in ["sketch", "part", "assembly", "cam", "joint", "tree", "viewport", "toolbar", "properties", "sketcher"] {
            assert!(
                !words.iter().any(|w| w == name),
                "the shell names `{name}` - a place is a KIND of container, and what stands in it is not the shell's business"
            );
        }
    }

    fn two() -> Shell {
        let mut s = Shell::default();
        s.put(Place::new("alpha", Slot::Left).sized(200.0, true));
        s.put(Place::new("beta", Slot::Right));
        s
    }

    /// A place stands where it was registered until somebody moves it.
    #[test]
    fn a_place_stands_where_it_was_registered() {
        let s = two();
        assert_eq!(s.slot_of("alpha"), Some(Slot::Left));
        assert_eq!(s.slot_of("beta"), Some(Slot::Right));
        assert_eq!(s.slot_of("nobody"), None, "an unregistered key has no place, rather than a default one");
    }

    /// Moving changes where it stands and nothing else; putting back returns exactly the registration.
    #[test]
    fn moving_and_putting_back() {
        let mut s = two();
        s.move_to("alpha", Slot::Bottom);
        assert_eq!(s.slot_of("alpha"), Some(Slot::Bottom));
        assert_eq!(s.slot_of("beta"), Some(Slot::Right), "moving one place must not disturb another");
        s.reset();
        assert_eq!(s.slot_of("alpha"), Some(Slot::Left), "putting back must give the registered place");
        assert!(s.saved().is_empty(), "nothing was moved, so nothing is worth saving");
    }

    /// ONLY THE CHANGES ARE SAVED. Saving the defaults too would freeze them: a place added by a later
    /// version would never reach anybody who had opened the settings once.
    #[test]
    fn only_the_changes_are_saved() {
        let mut s = two();
        s.move_to("alpha", Slot::Top);
        assert_eq!(s.saved(), vec![("alpha".to_string(), Slot::Top)]);

        let mut fresh = two();
        fresh.restore(s.saved());
        assert_eq!(fresh.slot_of("alpha"), Some(Slot::Top), "the layout must survive a restart");
        assert_eq!(fresh.slot_of("beta"), Some(Slot::Right));
    }

    /// A saved place that nobody registers any more is dropped rather than kept as a ghost.
    #[test]
    fn a_saved_place_of_a_panel_that_is_gone_is_dropped() {
        let mut s = two();
        s.restore(vec![("alpha".to_string(), Slot::Top), ("gone".to_string(), Slot::Left)]);
        assert_eq!(s.slot_of("alpha"), Some(Slot::Top));
        assert_eq!(s.slot_of("gone"), None, "a place nobody registered must not come back from the file");
        assert_eq!(s.saved().len(), 1, "and it must not be written out again either");
    }

    /// Moving an unregistered key does nothing: the file cannot invent a panel.
    #[test]
    fn an_unknown_key_cannot_be_placed() {
        let mut s = two();
        s.move_to("nobody", Slot::Top);
        assert!(s.saved().is_empty(), "a key nobody registered must not enter the layout");
    }

    /// THE MIDDLE IS LAST. egui requires the central panel after every side one, and until now that was
    /// held only by the order of statements in the frame.
    #[test]
    fn the_middle_comes_last() {
        assert_eq!(*Slot::ORDER.last().expect("the order is not empty"), Slot::Centre);
        assert_eq!(Slot::ORDER[0], Slot::Menu, "the menu is above everything");
    }
}
