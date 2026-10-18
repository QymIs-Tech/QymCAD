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
//!
//! THE LAYOUT. At each edge of the window the panels stand in STRIPS, counted from the edge inward; in a
//! strip they stand one after another ALONG the edge - top to bottom at a side, left to right at the top
//! and the bottom - with a border between them that is dragged. So two panels at the left are either
//! side by side (two strips) or one under the other (one strip), as the person puts them. A panel may be
//! hidden; one at the top or the bottom may stand over the view instead of beside it. The menu and the
//! middle are not in the layout: the menu is always above everything, the middle is what is left.
use std::collections::BTreeMap;

mod bar;
mod draw;

pub use bar::{note_button, noted_buttons, noted_more, plan_bar, BarAlign, BarCaption, BarItem, BarLines, BarLook, BarPlace, BarPlan, Button, Captions, MoreSlot};
pub use draw::{clear_of_overs, keyed, laid_out};

/// The kinds of place an interface has. Not a list of panels: a KIND, of which there may be many.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, serde::Serialize, serde::Deserialize)]
pub enum Slot {
    /// The menu, above everything. Not in the layout: nobody moves or hides it.
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
    /// The order the frame goes in. The middle is last.
    pub const ORDER: [Slot; 6] = [Slot::Menu, Slot::Top, Slot::Bottom, Slot::Left, Slot::Right, Slot::Centre];

    /// The edge this kind stands at; the menu and the middle stand at none.
    pub fn edge(self) -> Option<Edge> {
        match self {
            Slot::Top => Some(Edge::Top),
            Slot::Left => Some(Edge::Left),
            Slot::Right => Some(Edge::Right),
            Slot::Bottom => Some(Edge::Bottom),
            Slot::Menu | Slot::Centre => None,
        }
    }
}

/// THE EDGES OF THE WINDOW a panel can stand at.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default, Hash, serde::Serialize, serde::Deserialize)]
pub enum Edge {
    #[default]
    Left,
    Right,
    Top,
    Bottom,
}

impl Edge {
    /// Every edge, in the order a list of them is shown.
    pub const ALL: [Edge; 4] = [Edge::Left, Edge::Right, Edge::Top, Edge::Bottom];

    /// The catalogue key holding this edge's name.
    pub fn key(self) -> &'static str {
        match self {
            Edge::Left => "slot-left",
            Edge::Right => "slot-right",
            Edge::Top => "slot-top",
            Edge::Bottom => "slot-bottom",
        }
    }

    /// Do the panels of a strip at this edge stand one under another? At a side they do; at the top and the
    /// bottom they stand side by side.
    pub fn upright(self) -> bool {
        matches!(self, Edge::Left | Edge::Right)
    }

    /// The kind of place of this edge.
    pub fn slot(self) -> Slot {
        match self {
            Edge::Left => Slot::Left,
            Edge::Right => Slot::Right,
            Edge::Top => Slot::Top,
            Edge::Bottom => Slot::Bottom,
        }
    }
}

/// Is a panel shown?
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, serde::Serialize, serde::Deserialize)]
pub enum Seen {
    #[default]
    Shown,
    Hidden,
}

/// Does a panel at the top or the bottom take a strip of its own, or stand over the view?
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, serde::Serialize, serde::Deserialize)]
pub enum Over {
    /// A strip of its own: the view is narrowed by it.
    #[default]
    Beside,
    /// Over the edge of the view, which is not narrowed; only at the top and the bottom.
    Over,
}

/// WHERE ONE PANEL STANDS. Kept in the settings by the key of the panel, and only when it differs from where the
/// workbench registered it.
#[derive(Clone, Copy, PartialEq, Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct PanelPlace {
    pub edge: Edge,
    /// Which strip at the edge, from the edge inward. Only the order of the numbers counts, not their values.
    pub strip: u32,
    /// Where in its strip, along the edge. Again only the order counts.
    pub order: u32,
    /// The thickness of its strip across the edge, once the person dragged the border of the strip.
    pub across: Option<f32>,
    /// Its length along the strip, once the person dragged the border to its neighbour.
    pub along: Option<f32>,
    pub seen: Seen,
    pub over: Over,
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
    /// May the person hide it?
    pub hiding: Hiding,
    /// What its content takes across the strip, without the frame, when that is known ahead - a bar of buttons.
    pub content: Option<ContentSize>,
    /// May the person move it to another edge?
    pub pin: Pin,
    /// The margin inside its frame on every side, when it is not the frame's own.
    pub margin: Option<f32>,
    /// HOW OPAQUE ITS GROUND IS, 0 to 1; one seen through lies over the view. Said when the shell is built for a frame and
    /// held for the whole frame: asked of the host as each container opened, a slider moved in the settings drawn
    /// between them put the bar in its strip and over the view in one frame - one widget on two layers, and egui stopped
    /// the program.
    pub opacity: f32,
}

/// MAY A PLACE BE MOVED? One a person looks for in one place - the status line at the bottom, as in every program -
/// stays at its edge; it is still shown on the diagram, locked, and may still be hidden.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pin {
    Movable,
    Pinned,
}

/// WHAT THE CONTENT OF A PLACE TAKES ACROSS ITS STRIP: at a side, and at the top or the bottom.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ContentSize {
    pub side: f32,
    pub flat: f32,
}

/// MAY A PLACE BE HIDDEN? One that answers for what the person is doing right now - the bar of the command in hand -
/// is not: a tool with its bar hidden waits for something nobody can see it asking for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Hiding {
    Allowed,
    Refused,
}

impl Place {
    pub fn new(key: &'static str, slot: Slot) -> Self {
        Self { key, slot, size: None, resizable: false, framed: false, separator: true, hiding: Hiding::Allowed, content: None, pin: Pin::Movable, margin: None, opacity: 1.0 }
    }
    /// How opaque the ground is, 0 to 1.
    pub fn opacity(mut self, o: f32) -> Self {
        self.opacity = o;
        self
    }
    /// The place stays at the edge it is registered at.
    pub fn pinned(mut self) -> Self {
        self.pin = Pin::Pinned;
        self
    }
    /// The same margin inside the frame on every side.
    pub fn margin(mut self, m: f32) -> Self {
        self.margin = Some(m);
        self
    }
    /// The strip is as thick as the content says, plus its frame: neither resized by hand nor started at a guess.
    pub fn content(mut self, size: ContentSize) -> Self {
        self.content = Some(size);
        self
    }
    pub fn never_hidden(mut self) -> Self {
        self.hiding = Hiding::Refused;
        self
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

/// A NEW SIZE A PERSON DRAGGED a panel to: across its strip, or along it.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Resize {
    Across(f32),
    Along(f32),
}

/// WHERE A PANEL IS TAKEN TO at an edge.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Target {
    /// A strip of its own, before the strip `before` counted from the edge (the count of strips puts it innermost).
    NewStrip { edge: Edge, before: usize },
    /// Into the strip `strip` counted from the edge, before its panel `before` (the count of them puts it last).
    Into { edge: Edge, strip: usize, before: usize },
}

/// What the shell needs from whoever owns the panels. None of it is about workbenches.
pub trait Fills {
    /// Is this place wanted at all this frame? A command bar exists only while its command is running,
    /// and an empty container still eats a strip of the window, so the question is asked before opening.
    fn live(&self, key: &'static str) -> bool;
    /// Fill it. The `Ui` is already bounded by the container; `edge` is where it stands, none for the menu and the
    /// middle, so that a bar lays itself out in a row or a column.
    fn fill(&mut self, key: &'static str, ui: &mut egui::Ui, edge: Option<Edge>);
    /// The frame a bar-like place is drawn in. It carries the colours, which belong to the host.
    fn bar_frame(&self) -> egui::Frame;
    /// The person dragged the place to a new size: the host keeps it.
    fn resized(&mut self, key: &'static str, size: Resize);
}

/// The layout: the places as registered, plus wherever the person has since moved them.
#[derive(Default)]
pub struct Shell {
    places: Vec<Place>,
    /// The person's changes, kept apart from the registration so that "put it back" is a deletion and
    /// not a second copy of the defaults that would drift from them.
    moved: BTreeMap<String, PanelPlace>,
}

impl Shell {
    /// Register a place. Called by a workbench when it starts, not by the shell.
    pub fn put(&mut self, p: Place) {
        self.places.push(p);
    }

    /// Every registered key, in registration order.
    pub fn keys(&self) -> Vec<&'static str> {
        self.places.iter().map(|p| p.key).collect()
    }

    /// THE KEYS IN THE LAYOUT: every place at an edge, in registration order. The menu and the middle are not
    /// among them. The layout editor and the menu of panels walk THIS rather than a list of their own: a place
    /// added by any workbench shows up there without that code being touched.
    pub fn movable(&self) -> Vec<&'static str> {
        self.places.iter().filter(|p| p.slot.edge().is_some() && p.pin == Pin::Movable).map(|p| p.key).collect()
    }

    /// THE KEYS SHOWN ON THE DIAGRAM OF THE LAYOUT: every place at an edge, the pinned ones among them.
    pub fn at_edges(&self) -> Vec<&'static str> {
        self.places.iter().filter(|p| p.slot.edge().is_some()).map(|p| p.key).collect()
    }

    /// May the place be moved to another edge?
    pub fn pinned(&self, key: &str) -> bool {
        self.registered(key).is_some_and(|p| p.pin == Pin::Pinned)
    }

    fn registered(&self, key: &str) -> Option<&Place> {
        self.places.iter().find(|p| p.key == key)
    }

    /// WHERE A PLACE WAS REGISTERED: its edge, and a strip of its own after the places registered at that edge
    /// before it. `None` for the menu, the middle and a key nobody registered.
    pub fn home(&self, key: &str) -> Option<PanelPlace> {
        let p = self.registered(key)?;
        let edge = p.slot.edge()?;
        let strip = self.places.iter().take_while(|q| q.key != key).filter(|q| q.slot.edge() == Some(edge)).count() as u32;
        Some(PanelPlace { edge, strip, ..PanelPlace::default() })
    }

    /// Where a place stands now: what the person chose, or what was registered.
    pub fn place_of(&self, key: &str) -> Option<PanelPlace> {
        let home = self.home(key)?;
        Some(self.moved.get(key).copied().unwrap_or(home))
    }

    /// The kind of place a key stands in now.
    pub fn slot_of(&self, key: &str) -> Option<Slot> {
        match self.place_of(key) {
            Some(p) => Some(p.edge.slot()),
            None => self.registered(key).map(|p| p.slot),
        }
    }

    /// PUT A PLACE where `place` says. The registration is untouched, so "put it back" stays possible; a place put
    /// back where it was registered leaves no record.
    pub fn set_place(&mut self, key: &str, place: PanelPlace) {
        let Some(home) = self.home(key) else { return };
        // a pinned place keeps where it was registered; only whether it is seen is the person's
        let place = if self.pinned(key) { PanelPlace { seen: place.seen, ..home } } else { place };
        if place == home {
            self.moved.remove(key);
        } else {
            self.moved.insert(key.to_string(), place);
        }
    }

    /// May the place be hidden?
    pub fn hideable(&self, key: &str) -> bool {
        self.registered(key).is_some_and(|p| p.hiding == Hiding::Allowed && p.slot.edge().is_some())
    }

    /// Show or hide a place. One that may not be hidden stays shown.
    pub fn set_seen(&mut self, key: &str, seen: Seen) {
        if seen == Seen::Hidden && !self.hideable(key) {
            return;
        }
        if let Some(p) = self.place_of(key) {
            self.set_place(key, PanelPlace { seen, ..p });
        }
    }

    /// Let a place at the top or the bottom stand over the view, or beside it.
    pub fn set_over(&mut self, key: &str, over: Over) {
        if let Some(p) = self.place_of(key) {
            self.set_place(key, PanelPlace { over, ..p });
        }
    }

    /// KEEP A SIZE the person dragged a place to. Across, it is the thickness of the whole strip, so every panel of
    /// the strip takes it: the strip is as thick as its first panel says, and a panel moved out keeps the size.
    pub fn resize(&mut self, key: &str, size: Resize) {
        let Some(p) = self.place_of(key) else { return };
        match size {
            Resize::Along(v) => self.set_place(key, PanelPlace { along: Some(v), ..p }),
            Resize::Across(v) => {
                for k in self.strips(p.edge).into_iter().find(|s| s.contains(&key_of(&self.places, key))).unwrap_or_default() {
                    if let Some(q) = self.place_of(k) {
                        self.set_place(k, PanelPlace { across: Some(v), ..q });
                    }
                }
            }
        }
    }

    /// THE STRIPS AT AN EDGE, from the edge inward, each with its panels along the edge - every panel placed there,
    /// shown or hidden. Two places with the same numbers stand in the order they were registered.
    pub fn strips(&self, edge: Edge) -> Vec<Vec<&'static str>> {
        let mut at: Vec<Ranked> =
            self.places.iter().enumerate().filter_map(|(i, p)| self.place_of(p.key).filter(|q| q.edge == edge).map(|q| Ranked { key: p.key, strip: q.strip, order: q.order, registered: i })).collect();
        at.sort_by_key(|r| (r.strip, r.order, r.registered));
        let mut out: Vec<Vec<&'static str>> = Vec::new();
        let mut last = None;
        for r in at {
            if last == Some(r.strip) {
                out.last_mut().expect("a strip was opened for the one before").push(r.key);
            } else {
                out.push(vec![r.key]);
                last = Some(r.strip);
            }
        }
        out
    }

    /// TAKE A PLACE TO `to`. The strips of the edge it comes to are numbered afresh, so the numbers kept say the
    /// order the person sees; its sizes are dropped, as they belonged to where it stood before.
    pub fn move_to(&mut self, key: &str, to: Target) {
        if self.pinned(key) {
            return;
        }
        let Some(now) = self.place_of(key) else { return };
        let key = key_of(&self.places, key);
        let edge = match to {
            Target::NewStrip { edge, .. } | Target::Into { edge, .. } => edge,
        };
        let mut strips: Vec<Vec<&'static str>> = self.strips(edge).into_iter().map(|s| s.into_iter().filter(|k| *k != key).collect::<Vec<_>>()).collect();
        // a strip left empty by taking the place out of it is no longer a strip, but the indices of the target count
        // it as the person saw it
        match to {
            Target::NewStrip { before, .. } => strips.insert(before.min(strips.len()), vec![key]),
            Target::Into { strip, before, .. } => match strips.get_mut(strip) {
                Some(s) => s.insert(before.min(s.len()), key),
                None => strips.push(vec![key]),
            },
        }
        let over = if edge.upright() { Over::Beside } else { now.over };
        for (i, strip) in strips.iter().filter(|s| !s.is_empty()).enumerate() {
            for (j, k) in strip.iter().enumerate() {
                let Some(q) = self.place_of(k) else { continue };
                let q = if *k == key { PanelPlace { edge, across: None, along: None, over, ..q } } else { q };
                self.set_place(k, PanelPlace { strip: i as u32, order: j as u32, ..q });
            }
        }
    }

    /// Put everything back as the workbenches registered it.
    pub fn reset(&mut self) {
        self.moved.clear();
    }

    /// The person's layout, for saving. Only the CHANGES: the defaults come from the code and would
    /// otherwise be frozen into a file and go stale the moment a panel is added.
    pub fn saved(&self) -> BTreeMap<String, PanelPlace> {
        self.moved.clone()
    }

    /// Restore a saved layout. Keys nobody registered are dropped - a panel may have been removed since - and so
    /// are the menu and the middle, which no layout moves.
    pub fn restore(&mut self, saved: BTreeMap<String, PanelPlace>) {
        self.moved = saved
            .into_iter()
            .filter(|(k, _)| self.home(k).is_some())
            .map(|(k, p)| {
                let seen = if self.hideable(&k) { p.seen } else { Seen::Shown };
                let kept = match self.home(&k).filter(|_| self.pinned(&k)) {
                    Some(home) => PanelPlace { seen, ..home },
                    None => PanelPlace { seen, ..p },
                };
                (k, kept)
            })
            .collect();
    }

    /// Draw one kind of place. The frame calls this six times, in `Slot::ORDER`.
    pub fn run_slot(&self, slot: Slot, ui: &mut egui::Ui, host: &mut dyn Fills) {
        draw::run_slot(self, slot, ui, host);
    }
}

/// The key as registered - the `&'static str` the host is handed - for a key given as any string.
fn key_of(places: &[Place], key: &str) -> &'static str {
    places.iter().find(|p| p.key == key).map(|p| p.key).unwrap_or("")
}

/// A place with what puts it in order at its edge.
struct Ranked {
    key: &'static str,
    strip: u32,
    order: u32,
    registered: usize,
}

#[cfg(test)]
mod tests;
