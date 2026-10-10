//! THE CONTRACT OF A TOOL, as data: how a person takes it, what it works on, what it accepts and refuses, what it
//! makes. One description gives every point of the contract as a check of its own - see [`contract!`].
//!
//! A description names what a person reads (the keys of the catalogue), where they click (points of the sketch and
//! of space) and what they expect (the numbers of the body). It holds no steps: the steps are the same for every tool
//! and live in `run`.
pub mod fixtures;
pub mod run;

use qymcad::Key;

/// Whatever a word is written with: a key of the catalogue, read in the session's language.
pub type Word = &'static str;

/// THE POINTS OF THE CONTRACT, by number, as the checks are named.
pub const POINTS: [(u8, &str); 19] = [
    (1, "entry"),
    (2, "bar"),
    (3, "picks"),
    (4, "wrong_picks"),
    (5, "valid_values"),
    (6, "invalid_values"),
    (7, "preview"),
    (8, "apply"),
    (9, "cancel"),
    (10, "result"),
    (11, "undo"),
    (12, "save_open"),
    (13, "reopen"),
    (14, "upstream"),
    (15, "dependency"),
    (16, "contexts"),
    (17, "kernel_refusal"),
    (18, "budget"),
    (19, "help"),
];

/// A tool described.
pub struct Tool {
    /// A name of its own, for the report.
    pub id: &'static str,
    /// How the tool is used: a command applied with Enter, or a drawing made by clicks.
    pub flow: Flow,
    /// The word its bar of options names it by.
    pub title: Word,
    /// Every way a person takes it.
    pub entries: &'static [Entry],
    /// Another tool, taken first to see this one put it down, and the word its bar names it by.
    pub other: (Entry, Word),
    /// What is in the window before the tool is taken.
    pub fixture: fixtures::Fixture,
    /// What is clicked for the ordinary result, in order.
    pub picks: &'static [Pick],
    /// What is clicked to see a pick taken and let go - the ordinary picks, unless the tool takes something by
    /// itself (a sketch of one contour).
    pub pick_trial: &'static [Pick],
    /// Clicks the tool must refuse, in words, taking nothing. A click on empty space is always tried as well.
    pub wrong_picks: &'static [Pick],
    /// The fields at the geometry; the first is the one the ordinary result is changed through.
    pub fields: &'static [Field],
    /// The fields a person types words into - a string of text, a name.
    pub words: &'static [Words],
    /// The modes of its bar, group by group: the options of a group put each other down, and the first is the one
    /// the tool starts in; a group of one is a switch.
    pub modes: &'static [&'static [Mode]],
    /// The ordinary result: the body of the part, or the geometry of the sketch, after the ordinary picks, clicks and
    /// values.
    pub result: Outcome,
    /// The kind of timeline node it makes (`Extrude`, `Fillet`).
    pub node: &'static str,
    /// The name of its step of undo, and how many such steps the ordinary use makes.
    pub undo: Word,
    pub undo_steps: usize,
    /// Does the tool stay in hand once applied?
    pub stays: bool,
    /// A change made above it in the timeline, and what the body becomes.
    pub upstream: Option<Upstream>,
    /// The node it stands on, deleted to see it turn red with a reason.
    pub dependency: Option<Node>,
    /// The contexts it works in, besides a part of its own.
    pub contexts: &'static [Context],
    /// A value of the right form the kernel cannot build, typed into the first field.
    pub refusal: Option<f64>,
    /// How long applying may take, in seconds, and the longest frame, in milliseconds.
    pub budget: (u64, u64),
    /// Its article of help, by its path under `docs/help/<language>/` without `.md`.
    pub help: &'static str,
    /// Points of the contract that do not apply, each with why.
    pub not_applicable: &'static [(u8, &'static str)],
}

/// HOW A TOOL IS USED.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Flow {
    /// Picks, then values, then Enter: the tools of the part and the assembly.
    Command,
    /// Clicks on the sheet make the geometry, in these places, and this gesture finishes it.
    Drawing(&'static [(f64, f64)], Finish),
    /// The way in does the work itself, on what the fixture has selected: deleting, grounding - nothing is held.
    Action,
}

/// WHAT FINISHES A DRAWING.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Finish {
    /// The last click of the list.
    LastClick,
    /// A double click on the last place: a shape of as many points as a person wants ends by saying so.
    DoubleClick,
    /// The last click PLACES what the earlier ones only picked - a dimension on a line: nothing of it is laid down
    /// before it (what shows meanwhile follows the cursor), so cancelling on the way leaves the sketch as it was
    /// before the tool.
    Placed,
    /// The clicks pick what the tool works on, then the pointer stands at this place of the sheet - the side the
    /// result goes to - and Enter makes it: the offset.
    EnterAt((f64, f64)),
}

/// A way to take a tool.
#[derive(Clone, Copy)]
pub enum Entry {
    /// The button whose hint is this word.
    Button(Word),
    /// A key, with these modifiers held.
    Key(qymcad::Modifiers, Key),
    /// The command search, by the command's name.
    Search(Word),
    /// The command search, by the name it shows for a command that has no name of its own: the title of the article
    /// of help the tool points at.
    SearchByArticle,
    /// A path down the menus.
    Menu(&'static [Word]),
    /// The box, or the word, of the window labelled with this word - a check of the tree that has no hint of its own.
    Label(Word),
}

/// A click.
#[derive(Clone, Copy, Debug)]
pub enum Pick {
    /// On a face through this point of space.
    Face([f64; 3]),
    /// On an edge through this point of space.
    Edge([f64; 3]),
    /// On the vertex at this point of space.
    Vertex([f64; 3]),
    /// On this point of the open sketch.
    Sketch(f64, f64),
    /// On this point of space, whatever is drawn there - a contour of a sketch, a plane.
    Space([f64; 3]),
    /// On the row of the tree beginning with this word.
    Row(Word),
    /// On the row of the list of mates, right of the canvas, beginning with this word.
    Listed(Word),
    /// On this word of the bar, between picks: a tool that takes two kinds of reference says which kind comes next.
    Bar(Word),
}

/// The kind of value a field takes: what is tried in it follows from the kind.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Class {
    /// A length, a depth, a height, an offset.
    Length,
    /// A radius, a thickness, a leg: bounded by the geometry it is made on.
    Radius,
    /// An angle, in degrees.
    Angle,
    /// A count.
    Count,
    /// A tolerance: it decides whether the pieces meet, not the shape they make - the picture has nothing to follow.
    Tolerance,
}

/// WHEN A FIELD IS FILLED: before the tool is finished, or after - a dimension is placed first and typed into then.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum When {
    Before,
    After,
}

/// A field at the geometry.
pub struct Field {
    /// The caption beside it, or the grey words it shows while empty.
    pub caption: Word,
    /// Is the caption the grey words of an empty field rather than a caption beside it?
    pub by_placeholder: bool,
    /// Is it filled before the tool is finished, or after?
    pub when: When,
    pub class: Class,
    /// The value of the ordinary result.
    pub typical: f64,
    /// The smallest and the largest value the tool accepts.
    pub lo: f64,
    pub hi: f64,
    /// Is zero a value it accepts? A negative one?
    pub zero: bool,
    pub negative: bool,
    /// The body for a value of this field, the other fields at their ordinary values.
    pub outcome: fn(f64) -> Outcome,
}

/// A FIELD OF WORDS: what a person types into it, and what comes of it.
pub struct Words {
    /// The caption beside it, or the grey words it shows while empty.
    pub caption: Word,
    /// Is the caption the grey words of an empty field rather than a caption beside it?
    pub by_placeholder: bool,
    /// The words of the ordinary result.
    pub typical: &'static str,
    /// Words it must take: an ordinary string, one of another alphabet, a long one.
    pub valid: &'static [&'static str],
    /// Words it must refuse: empty, spaces alone, letters the font cannot draw.
    pub invalid: &'static [&'static str],
    /// What the sketch holds for a string.
    pub outcome: fn(&str) -> Outcome,
    /// Are the words given with Enter, as a name asked beside the cursor is, rather than taken as they are typed?
    pub enter: bool,
}

/// A mode of the bar.
pub struct Mode {
    /// The word of its switch.
    pub word: Word,
    /// The clicks this mode takes, when they are not the ordinary ones - a rectangle by three points takes three.
    pub clicks: Option<&'static [(f64, f64)]>,
    /// What it makes of the ordinary picks and values; `None` when it must refuse them in words.
    pub outcome: Option<Outcome>,
}

/// A change made above the tool in the timeline.
pub enum Upstream {
    /// The node of this kind reopened with a double click on its row, the field under this caption set to a value.
    Reopen { node: Node, caption: Word, value: f64, then: Outcome },
    /// The sketch reopened with a double click on its row, the dimension written as `shown` double-clicked and set to
    /// a value, the sketch finished.
    SketchDimension { shown: &'static str, value: f64, then: Outcome },
}

/// A node of the timeline, by its kind, and how its row in the tree is found.
#[derive(Clone, Copy, Debug)]
pub struct Node {
    /// The kind (`Sketch`, `Extrude`).
    pub kind: &'static str,
    /// The word its row begins with; `None` when the row shows the node's own name.
    pub row: Option<Word>,
}

/// A context a tool is used in.
#[derive(Clone, Copy)]
pub enum Context {
    /// A second part of the assembly, the fixture built in it.
    SecondPart,
    /// The fixture built after this other tool was used on the first part.
    AfterTool(&'static Tool),
}

/// WHAT THE TOOL LEAVES BEHIND: a body of the part, or the geometry of the sketch.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Outcome {
    /// The numbers of the body of the part.
    Body { volume: f64, faces: usize, edges: usize, min: [f64; 3], max: [f64; 3] },
    /// A body of several separate pieces and what they hold together: a pattern whose copies stand apart leaves them
    /// in one body of the part, and of such a body only the number of pieces and the volume can be said.
    Pieces { pieces: u32, volume: f64 },
    /// SEVERAL BODIES OF THE PART: how many, what they hold together, and the largest of them - where a cut fell is
    /// read off the largest piece. What more than one body does to the rule "a part is one body" is a matter for the
    /// oracles, not for a description.
    Bodies { count: usize, volume: f64, largest: f64 },
    /// A SHEET OF SURFACE the part shows beside its solid: its area and where it reaches - an offset surface is told by
    /// how far it stands from the face it came from.
    Sheet { area: f64, min: [f64; 3], max: [f64; 3] },
    /// THE PARTS OF THE ASSEMBLY: how many, how many of them are grounded - fixed where they stand - how many joints
    /// hold them, none of them violated, and how many sub-assemblies stand in it.
    Parts { parts: usize, grounded: usize, joints: usize, assemblies: usize },
    /// A ROW OF PARTS: this many parts of the assembly, each standing `step` further on than the one before, in the
    /// order they were made.
    Row { parts: usize, step: [f64; 3] },
    /// A WINDOW OF THE PROGRAM OPEN, by the words of its title: what a menu item that shows something leaves.
    Window { title: Word },
    /// ONE BODY OF THIS VOLUME, to within `tol`, in this box: for a shape whose faces are not worth counting - a thread
    /// is a helix cut into as many faces as the kernel lays - but whose volume the standard gives.
    Volume { volume: f64, tol: f64, min: [f64; 3], max: [f64; 3] },
    /// HOW MANY ENTITIES OF THE OPEN SKETCH ARE CONSTRUCTION GEOMETRY, the rest of it counted as before.
    Construction { count: usize, lines: usize },
    /// HOW MANY ENTITIES OF THE OPEN SKETCH ARE SELECTED.
    Picked { count: usize },
    /// THE MATES THAT TIE PARTS WITHOUT A JOINT - a group, a width, a tangent - by their kinds, in order.
    Mates { kinds: &'static [&'static str] },
    /// EVERY BODY WHOLE IN THE VIEW: each corner of each body's box falls inside the canvas - what fitting the view to
    /// the model gives.
    Fitted,
    /// THE PART AT THIS PLACE OF THE ASSEMBLY TURNED BY THIS ANGLE from how it was built, in degrees, whichever axis it
    /// turned about - what driving a hinge by a number gives.
    Turned { part: usize, degrees: f64 },
    /// THE TREE SHOWS THE ROWS BEGINNING WITH THESE WORDS AND NONE BEGINNING WITH THOSE - what a search of the tree
    /// leaves of it.
    TreeRows { shown: &'static [&'static str], hidden: &'static [&'static str] },
    /// THIS WORD WRITTEN SOMEWHERE ON SCREEN: what a submenu opened shows.
    OnScreen { word: Word },
    /// THE STATUS LINE SAYS THIS, among its words: what a measure answers, the document left as it was.
    Says { text: &'static str },
    /// A VIEW SECTION CUTTING THE MODEL, or none: what the view shows, the document left as it was.
    Section { on: bool },
    /// THE WINDOW DRAWN DARK OR LIGHT: the mean brightness of its whole picture below or above the middle - what a
    /// theme changes, everything else about the document left as it was.
    Shade { dark: bool },
    /// THE CHOOSER OF FILES PUT UP and waiting: to write a file (`save`), or to pick one to read.
    Chooser { save: bool },
    /// The geometry of the sketch, counted, and what lies at places of the sheet.
    Sketch {
        points: usize,
        lines: usize,
        arcs: usize,
        circles: usize,
        ellipses: usize,
        splines: usize,
        texts: usize,
        notes: usize,
        /// The constraints and dimensions a person put there, when they are part of what the tool makes.
        constraints: Option<usize>,
        /// The degrees of freedom left, when the tool is about taking them away.
        dof: Option<i32>,
        /// The corners of the box around what is drawn, when the tool decides where it stands.
        box_of: Option<([f64; 2], [f64; 2])>,
        /// The width and height of that box, when the tool decides the size but not the place - a dimension moves both
        /// ends of an unanchored line.
        size_of: Option<[f64; 2]>,
        under: &'static [(f64, f64, Under)],
    },
}

/// WHAT LIES AT A PLACE OF THE SHEET, as a click there would take it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Under {
    Point,
    Line,
    Arc,
    Circle,
    Ellipse,
    Spline,
    Nothing,
}

/// The geometry of an empty sketch: nothing drawn, nothing under the places named.
pub const EMPTY_SKETCH: Outcome =
    Outcome::Sketch { points: 0, lines: 0, arcs: 0, circles: 0, ellipses: 0, splines: 0, texts: 0, notes: 0, constraints: None, dof: None, box_of: None, size_of: None, under: &[] };

/// EVERY POINT OF THE CONTRACT OF A TOOL AS A CHECK OF ITS OWN, in a module named for it:
/// `contract!(extrude, qymcad_acceptance::tools::part::EXTRUDE);` gives `extrude::b01_entry` to `extrude::b19_help`.
#[macro_export]
macro_rules! contract {
    ($module:ident, $tool:path) => {
        mod $module {
            $crate::probe! { fn b01_entry() { $crate::contract::run::point(&$tool, 1) } }
            $crate::probe! { fn b02_bar() { $crate::contract::run::point(&$tool, 2) } }
            $crate::probe! { fn b03_picks() { $crate::contract::run::point(&$tool, 3) } }
            $crate::probe! { fn b04_wrong_picks() { $crate::contract::run::point(&$tool, 4) } }
            $crate::probe! { fn b05_valid_values() { $crate::contract::run::point(&$tool, 5) } }
            $crate::probe! { fn b06_invalid_values() { $crate::contract::run::point(&$tool, 6) } }
            $crate::probe! { fn b07_preview() { $crate::contract::run::point(&$tool, 7) } }
            $crate::probe! { fn b08_apply() { $crate::contract::run::point(&$tool, 8) } }
            $crate::probe! { fn b09_cancel() { $crate::contract::run::point(&$tool, 9) } }
            $crate::probe! { fn b10_result() { $crate::contract::run::point(&$tool, 10) } }
            $crate::probe! { fn b11_undo() { $crate::contract::run::point(&$tool, 11) } }
            $crate::probe! { fn b12_save_open() { $crate::contract::run::point(&$tool, 12) } }
            $crate::probe! { fn b13_reopen() { $crate::contract::run::point(&$tool, 13) } }
            $crate::probe! { fn b14_upstream() { $crate::contract::run::point(&$tool, 14) } }
            $crate::probe! { fn b15_dependency() { $crate::contract::run::point(&$tool, 15) } }
            $crate::probe! { fn b16_contexts() { $crate::contract::run::point(&$tool, 16) } }
            $crate::probe! { fn b17_kernel_refusal() { $crate::contract::run::point(&$tool, 17) } }
            $crate::probe! { fn b18_budget() { $crate::contract::run::point(&$tool, 18) } }
            $crate::probe! { fn b19_help() { $crate::contract::run::point(&$tool, 19) } }
        }
    };
}
