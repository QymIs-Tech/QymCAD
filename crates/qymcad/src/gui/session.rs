//! THE PROGRAM FROM OUTSIDE: a session is the whole program in a window with no screen, and the only door the
//! acceptance checks have.
//!
//! What a session gives is what a person has - the mouse, the keyboard, the wheel, the answer to a file chooser -
//! and what it shows is what a person sees: the words on screen, the status line, the document as the program
//! reports it. The application behind it stays private to the crate, so a check cannot set a field, call a
//! handler or start a rebuild the window would not have started: it does not compile.
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

pub use super::window::{Kind, Widget};
use super::window::Window;
use super::App;

pub use crate::system::Chooser;
pub use egui::{pos2, vec2, Key, Modifiers, PointerButton, Pos2, Rect, Vec2};

/// THE MACHINE THE PROGRAM IS STARTED ON.
pub struct Machine {
    /// The size of the window, in points. The program opens itself at 1280x800.
    pub screen: (f32, f32),
    /// The locale the system is set to (`en`, `ru-RU`): a first start picks its language by it.
    pub locale: String,
    /// What earlier runs of the program kept on this machine - its settings and the last project.
    pub kept: Kept,
    /// WHERE THE PROGRAM KEEPS ITS OWN FILES on this machine: settings, schemes, templates, the parts library,
    /// crash reports. One folder for the whole process - a person's own is never written.
    pub home: std::path::PathBuf,
    /// THE GREETING IS WAITED OUT in real time, five seconds of it. Otherwise the session comes to the program as
    /// its greeting ends.
    pub greeting: bool,
}

impl Default for Machine {
    /// A MACHINE THE PROGRAM HAS RUN ON BEFORE: its factory settings kept, so a start is not the first one (a first
    /// start opens the sample cube - `Machine::first_run`).
    fn default() -> Self {
        let mut kept = Kept::default();
        eframe::set_value(&mut kept, "settings", &qymcad_ui_state::Settings::default());
        Self {
            screen: (1280.0, 800.0),
            // `QYM_LOCALE` runs the whole acceptance in another language, to see what it shows in that one
            locale: std::env::var("QYM_LOCALE").unwrap_or_else(|_| "en".into()),
            kept,
            home: std::env::temp_dir().join(format!("qymcad-sessions-{}", std::process::id())),
            greeting: false,
        }
    }
}

impl Machine {
    /// A MACHINE THE PROGRAM HAS NEVER RUN ON: nothing kept, the first start.
    pub fn first_run() -> Self {
        Self { kept: Kept::default(), ..Self::default() }
    }
}

/// WHAT THE PROGRAM KEEPS BETWEEN ITS RUNS, as the window's framework keeps it: named records of text.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Kept(BTreeMap<String, String>);

impl eframe::Storage for Kept {
    fn get_string(&self, key: &str) -> Option<String> {
        self.0.get(key).cloned()
    }

    fn set_string(&mut self, key: &str, value: String) {
        self.0.insert(key.to_string(), value);
    }

    fn remove_string(&mut self, key: &str) {
        self.0.remove(key);
    }

    fn flush(&mut self) {}
}

/// WHAT IS LOOKED AT AFTER EVERY STEP OF EVERY SESSION of this process, once set: a check of its own that answers the
/// problems it saw.
static WATCH: std::sync::OnceLock<fn(&mut Session) -> Vec<String>> = std::sync::OnceLock::new();

/// The whole program, driven from outside.
pub struct Session {
    app: Box<App>,
    win: Window,
    kept: Kept,
    /// How long one wait for the program may take before the check fails with what is still running.
    budget: Duration,
    /// Where the pointer stands, once it has come over the window.
    pointer: Option<Pos2>,
    /// The keys held down across gestures - Shift, Ctrl, Alt.
    held: Modifiers,
    /// The clock of the last press, so two clicks meant apart are never taken for a double click.
    last_press: f64,
    /// The mouse buttons held down: nobody waits for the screen to rest in the middle of a drag.
    down: Vec<PointerButton>,
    /// The locale of the machine, said again before every frame: the language is kept per thread, and two sessions
    /// may take turns on one.
    locale: String,
    /// The watch is being run: what it does is not itself watched.
    watching: bool,
}

/// THE DOCUMENT AS THE PROGRAM HOLDS IT, read and never written: what a person could find out by looking at the
/// tree, the properties and the measuring tool.
#[derive(Clone, Debug, PartialEq)]
pub struct Document {
    /// The file the document is kept in, once it has one.
    pub path: Option<String>,
    /// Does the program take it for changed since it was last saved?
    pub unsaved: bool,
    /// The parts and assemblies of the tree, in the order they were made.
    pub parts: Vec<Part>,
    /// The sketches.
    pub sketches: Vec<SketchInfo>,
    /// The sketch open for editing, by its name, when the person is inside one.
    pub editing: Option<String>,
    /// Where the person stands: the name of the part or assembly whose inside is open, as the path at the top says.
    pub context: String,
    /// The datums: the planes, axes and points a person has put in, in the order they were made.
    pub datums: Vec<Datum>,
    /// The timeline, in its order: sketches, planes and features alike.
    pub features: Vec<Feature>,
    /// The bodies.
    pub bodies: Vec<Solid>,
    /// The parameters.
    pub parameters: Vec<Parameter>,
    /// The joints between parts.
    pub joints: Vec<JointInfo>,
    /// THE MATES THAT TIE PARTS WITHOUT A JOINT - a group, a width, a tangent, a relation - by their kind, in order.
    pub mates: Vec<String>,
    /// Is a view section cutting the model - a plane set that hides half of it? Not the document, but what the window
    /// shows of it.
    pub section: bool,
    /// The steps Undo would take back, oldest first, by their names.
    pub undo: Vec<String>,
    /// The steps Redo would do again, by their names.
    pub redo: Vec<String>,
}

/// A DATUM a person has put in: a plane, an axis or a point, where it stands and which way it looks.
#[derive(Clone, Debug, PartialEq)]
pub struct Datum {
    /// Its name in the tree.
    pub name: String,
    /// What it is: "plane", "axis" or "point".
    pub kind: String,
    /// Where it stands, in the coordinates of the part.
    pub at: [f64; 3],
    /// Which way it looks: the normal of a plane, the direction of an axis; all zero for a point.
    pub dir: [f64; 3],
}

/// THE GIZMO OF WHAT IS CHOSEN, as the eye sees it: three arms to carry it along and three rings to turn it about.
#[derive(Clone, Debug, PartialEq)]
pub struct Gizmo {
    /// Where the pointer takes hold of the arm of X, Y and Z - halfway along it.
    pub arms: [Pos2; 3],
    /// A place the ring about X, Y and Z runs through - away from the arms, so nothing else is taken instead.
    pub rings: [Pos2; 3],
}

/// A part or an assembly of the tree.
#[derive(Clone, Debug, PartialEq)]
pub struct Part {
    pub name: String,
    /// WHICH ONE IT IS, when two carry the same name - a clone or a part brought in from a file named as one already
    /// there. The number the program keeps it by.
    pub key: u64,
    /// An assembly rather than a part.
    pub assembly: bool,
    /// The name of the assembly it stands in; `None` at the top.
    pub parent: Option<String>,
    pub visible: bool,
    pub grounded: bool,
    /// The part this one is a clone of - the same part once more, standing and mated on its own; `None` for a part of
    /// its own.
    pub clone_of: Option<String>,
    /// Where it stands in the assembly that holds it - the placement the properties show as X, Y and Z.
    pub at: [f64; 3],
    /// Which way it is turned: where its own X, Y and Z look in the assembly.
    pub axes: [[f64; 3]; 3],
}

/// A sketch, counted.
#[derive(Clone, Debug, PartialEq)]
pub struct SketchInfo {
    pub name: String,
    /// The part it belongs to.
    pub part: Option<String>,
    pub points: usize,
    pub lines: usize,
    /// How many of the lines, arcs, circles and ellipses are construction geometry - dashed, outside every profile.
    pub construction: usize,
    pub arcs: usize,
    pub circles: usize,
    pub ellipses: usize,
    pub splines: usize,
    pub texts: usize,
    /// Notes: words on the sheet that are not geometry.
    pub notes: usize,
    /// The font each text is written in, as the bar names it.
    pub text_fonts: Vec<String>,
    /// The kind of each constraint and dimension a person put there, by the program's own name for the kind.
    pub constraint_kinds: Vec<String>,
    /// WHERE THE SKETCH SITS: "XY", "XZ", "YZ", "datum", or "face of <part>" for a sketch seated on the face of a
    /// body - the part named being the one that owns that body, whether it is this part or a neighbour.
    pub seat: String,
    /// How many things of this sketch a person has picked - the count the panel writes out.
    pub picked: usize,
    /// Where every point of a person's own geometry stands, in the coordinates of the sheet - the ends of the lines,
    /// the centres of the circles, the points put by hand. The origin and the axes are not among them.
    pub places: Vec<[f64; 2]>,
    /// The corners of the box around everything drawn, in the coordinates of the sheet; both zero when nothing is.
    pub min: [f64; 2],
    pub max: [f64; 2],
    pub constraints: usize,
    /// Degrees of freedom left.
    pub dof: i32,
    /// Constraints that say what others already say.
    pub redundant: i32,
}

/// A node of the timeline.
#[derive(Clone, Debug, PartialEq)]
pub struct Feature {
    pub name: String,
    /// What kind of node it is, by the program's own name for the kind (`Sketch`, `Extrude`, `Fillet`): the same
    /// in every language.
    pub kind: String,
    /// The part it belongs to.
    pub part: Option<String>,
    pub suppressed: bool,
    /// Why it did not build, in the words the properties give; `None` when it built.
    pub error: Option<String>,
    /// Built, but not all of what was asked - the words of the warning the tree shows in yellow.
    pub warning: Option<String>,
    /// Which node it is, whatever it is called: two holes are two keys (the program's own id, stable across undo).
    pub key: u64,
    /// How many bodies it makes: one, or several - the pieces a split or a cut through left, the copies of a pattern
    /// of components.
    pub bodies: usize,
}

/// A body, measured.
#[derive(Clone, Debug, PartialEq)]
pub struct Solid {
    pub name: String,
    /// The part it belongs to.
    pub part: Option<String>,
    /// Which part it belongs to, when two carry the same name (see [`Part::key`]).
    pub part_key: Option<u64>,
    /// The colour it is drawn in, as red, green and blue: the one the person chose, or the one of the palette.
    pub colour: [u8; 3],
    /// In mm^3: of the exact body when the program holds one, of its mesh otherwise.
    pub volume: f64,
    /// In mm^2, of the mesh.
    pub area: f64,
    pub faces: usize,
    /// The names the program keeps its faces by, which references to them hold on to.
    pub face_names: Vec<u32>,
    /// Where each of those faces lies - the middle of its mesh, in the same order: what the eye tells a face by, and
    /// so what tells a name kept from a name moved to another face.
    pub face_centres: Vec<[f64; 3]>,
    /// The edges of the exact body; `None` while the program holds none.
    pub edges: Option<usize>,
    /// The corners of the box around it, in mm.
    pub min: [f64; 3],
    pub max: [f64; 3],
    pub visible: bool,
    /// Taken up into another body - by a boolean, a pattern - and not a body of its own any more.
    pub consumed: bool,
    /// A surface rather than a solid.
    pub sheet: bool,
}

/// WHAT AN EXACT BODY IS MADE OF, read on demand ([`Session::inspect`]): checking a large body takes time, so it is
/// not part of every reading of the document.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Inspection {
    /// The body passes the kernel's check of a correct solid: closed, not self-crossing, every face bounded.
    pub valid: bool,
    /// How many solids and how many shells it holds: one and one for an ordinary part.
    pub solids: u32,
    pub shells: u32,
    /// Its faces by the kind of surface they lie on.
    pub kinds: FaceKinds,
}

/// FACES BY THE KIND OF SURFACE: a rounded straight edge adds a cylinder, a cut one a plane.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FaceKinds {
    pub plane: u32,
    pub cylinder: u32,
    pub cone: u32,
    pub sphere: u32,
    pub torus: u32,
    /// B-spline and Bezier surfaces.
    pub free: u32,
    /// Any other: an extrusion, a revolution, an offset of a curve or a surface.
    pub other: u32,
}

/// A parameter.
#[derive(Clone, Debug, PartialEq)]
pub struct Parameter {
    pub name: String,
    pub expr: String,
    pub value: f64,
}

/// A joint between two parts.
#[derive(Clone, Debug, PartialEq)]
pub struct JointInfo {
    pub name: String,
    /// The kind of joint, as the program names it in code.
    pub kind: String,
    /// Held apart from what it asks for.
    pub violated: bool,
}

/// A PICTURE OF THE WINDOW, a pixel to a point, as rows of RGBA bytes from the top left.
#[derive(Clone, Debug, PartialEq)]
pub struct Picture {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
}

impl Picture {
    /// A picture read from a PNG file; `None` when the bytes are not one.
    pub fn from_png(bytes: &[u8]) -> Option<Picture> {
        let img = image::load_from_memory_with_format(bytes, image::ImageFormat::Png).ok()?.to_rgba8();
        Some(Picture { width: img.width() as usize, height: img.height() as usize, rgba: img.into_raw() })
    }

    /// The picture as a PNG file.
    pub fn png(&self) -> Vec<u8> {
        let mut out = Vec::new();
        let img = image::RgbaImage::from_raw(self.width as u32, self.height as u32, self.rgba.clone()).expect("the bytes fill the picture");
        img.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png).expect("a picture in memory writes");
        out
    }
}

/// WHAT A CLICK ON THE SKETCH WOULD TAKE, in the coordinates of the sketch.
#[derive(Clone, Debug, PartialEq)]
pub enum SketchPick {
    /// A point.
    Point { at: (f64, f64) },
    /// A segment.
    Line { from: (f64, f64), to: (f64, f64) },
    /// An arc, counter-clockwise from `from` to `to` when `ccw`.
    Arc { centre: (f64, f64), from: (f64, f64), to: (f64, f64), ccw: bool },
    /// A circle.
    Circle { centre: (f64, f64), radius: f64 },
    /// An ellipse.
    Ellipse { centre: (f64, f64), major: (f64, f64), minor: (f64, f64) },
    /// A spline through its nodes.
    Spline { nodes: Vec<(f64, f64)> },
}

impl Session {
    /// START THE PROGRAM on a machine of its defaults, and wait until it is ready for a person.
    pub fn start() -> Session {
        Session::start_on(Machine::default())
    }

    /// START THE PROGRAM on `machine`, the way the window starts it, and wait until it is ready for a person.
    ///
    /// # Panics
    /// When `machine.home` is not the home of the sessions started before in this process: the program's own
    /// folders are one per process.
    pub fn start_on(machine: Machine) -> Session {
        assert!(qymcad_paths::keep_under(machine.home.clone()), "one process keeps one home for the program; {} was asked for after another", machine.home.display());
        super::prepare_the_process();
        qymcad_i18n::stand_for_system_locale(&machine.locale);
        crate::system::stand_in(true);
        let win = Window::new(egui::vec2(machine.screen.0, machine.screen.1));
        let app = Box::new(super::start_the_program(&win.ctx, Some(&machine.kept)));
        let mut session = Session {
            app,
            win,
            kept: machine.kept,
            budget: Duration::from_secs(120),
            pointer: None,
            held: Modifiers::default(),
            last_press: f64::NEG_INFINITY,
            down: Vec::new(),
            locale: machine.locale,
            watching: false,
        };
        if !machine.greeting {
            // the greeting has been on screen for its time: it goes on the first frame, through its own branch
            session.app.waiting.splash_until = Some(Instant::now());
        }
        session.settle();
        session
    }

    /// A MACHINE LIKE THIS SESSION'S: its locale and its settings as they are now, for a second start of the program
    /// that reads what this one wrote in the same language.
    pub fn same_machine(&self) -> Machine {
        let mut kept = Kept::default();
        eframe::set_value(&mut kept, "settings", &self.app.set);
        Machine { locale: self.locale.clone(), kept, ..Machine::default() }
    }

    /// CLOSE THE WINDOW as a person does, with its cross, and keep what the program keeps on the way out.
    ///
    /// A program with unsaved work refuses to close and asks what to do with it; then the session is given back,
    /// still running, with the question on screen.
    pub fn quit(mut self) -> Result<Kept, Box<Session>> {
        if !self.win.closed {
            self.settle();
            self.speak();
            self.win.run(&mut self.app, egui::Modifiers::default(), Vec::new(), true);
            if self.win.close_refused && !self.win.closed {
                self.settle();
                return Err(Box::new(self));
            }
        }
        eframe::App::save(&mut *self.app, &mut self.kept);
        Ok(std::mem::take(&mut self.kept))
    }

    /// HAS THE PROGRAM CLOSED ITS WINDOW by itself - Quit, or an answer to the question it asked on closing?
    /// What it keeps is then taken by [`Session::quit`].
    pub fn closed(&self) -> bool {
        self.win.closed
    }

    /// LOOK AT EVERY STEP OF EVERY SESSION OF THIS PROCESS with `watch`: after each gesture - a click, a key, a drag,
    /// typing, an answer to a chooser - the watch is run on the session, and a problem it answers fails the check at
    /// that step. Set once per process; `false` when another watch was set first.
    pub fn watch_every_step(watch: fn(&mut Session) -> Vec<String>) -> bool {
        WATCH.set(watch).is_ok()
    }

    /// HOW LONG ONE WAIT MAY TAKE before the check fails, naming what was still running. Two minutes unless said.
    pub fn budget(&mut self, budget: Duration) -> &mut Self {
        self.budget = budget;
        self
    }

    /// WAIT UNTIL THE PROGRAM IS QUIET, as a person waits: frames go on being drawn until no rebuild, load or
    /// write is under way and nothing on screen is still moving.
    ///
    /// # Panics
    /// When the budget runs out first, with what was still running.
    pub fn settle(&mut self) -> &mut Self {
        if !self.down.is_empty() || self.win.closed {
            // a button held is a gesture not over, the screen meant to move under it; a closed window has no frames
            return self;
        }
        let began = Instant::now();
        let mut quiet_frames = 0;
        while quiet_frames < 2 {
            self.frame(Vec::new());
            let still = self.still_running();
            quiet_frames = if still.is_empty() { quiet_frames + 1 } else { 0 };
            if !still.is_empty() {
                assert!(began.elapsed() < self.budget, "the program did not come to rest in {:?}: {}", self.budget, still.join("; "));
                std::thread::sleep(self.win.repaint.min(Duration::from_millis(16)));
            }
        }
        self
    }

    /// LET `span` OF REAL TIME PASS with the window drawing its frames, as a person waits with the program in front
    /// of them: what the program times by the clock on the wall - autosave, the turn of the view - moves on too.
    pub fn pause(&mut self, span: Duration) -> &mut Self {
        let began = Instant::now();
        while began.elapsed() < span {
            std::thread::sleep(Duration::from_millis(16).min(span.saturating_sub(began.elapsed())));
            if self.win.closed {
                break;
            }
            self.frame(Vec::new());
        }
        self.settle()
    }

    /// BRING THE POINTER TO `at`.
    pub fn move_to(&mut self, at: Pos2) -> &mut Self {
        self.settle();
        self.go(at);
        self
    }

    /// PRESS `button` where the pointer stands.
    ///
    /// # Panics
    /// When the pointer has not come over the window yet.
    pub fn press(&mut self, button: PointerButton) -> &mut Self {
        self.settle();
        self.button(button, true);
        self
    }

    /// LET GO OF `button` where the pointer stands.
    ///
    /// # Panics
    /// When the pointer has not come over the window yet.
    pub fn release(&mut self, button: PointerButton) -> &mut Self {
        self.settle();
        self.button(button, false);
        self.stepped("a button let go");
        self
    }

    /// CLICK THE LEFT BUTTON at `at`.
    pub fn click(&mut self, at: Pos2) -> &mut Self {
        self.click_with(at, PointerButton::Primary, Modifiers::default())
    }

    /// CLICK `button` at `at` with `modifiers` held for the click.
    pub fn click_with(&mut self, at: Pos2, button: PointerButton, modifiers: Modifiers) -> &mut Self {
        self.settle();
        self.go(at);
        let before = self.held;
        self.held = before | modifiers;
        self.button(button, true);
        self.button(button, false);
        self.held = before;
        self.stepped("a click");
        self
    }

    /// DOUBLE-CLICK THE LEFT BUTTON at `at`: two clicks a few frames apart, well inside the time a double click
    /// is allowed.
    pub fn double_click(&mut self, at: Pos2) -> &mut Self {
        self.settle();
        self.go(at);
        self.button(PointerButton::Primary, true);
        self.button(PointerButton::Primary, false);
        self.frame(vec![Self::pressing(at, PointerButton::Primary, true, self.held)]);
        self.frame(vec![Self::pressing(at, PointerButton::Primary, false, self.held)]);
        self.stepped("a double click");
        self
    }

    /// DRAG WITH `button` from `from` to `to`, `modifiers` held: pressed, carried over in even steps a frame
    /// each, and let go.
    pub fn drag(&mut self, from: Pos2, to: Pos2, button: PointerButton, modifiers: Modifiers) -> &mut Self {
        self.settle();
        self.drag_holding(from, to, &[button], modifiers);
        self.stepped("a drag");
        self
    }

    /// DRAG WITH SEVERAL BUTTONS AT ONCE, all of them held from `from` to `to`; no button at all is a movement
    /// with only `modifiers` down, the way a touchpad moves the view.
    pub fn drag_with(&mut self, from: Pos2, to: Pos2, buttons: &[PointerButton], modifiers: Modifiers) -> &mut Self {
        self.settle();
        self.drag_holding(from, to, buttons, modifiers);
        self.stepped("a drag");
        self
    }

    /// TURN THE WHEEL with the pointer at `at`, by `delta` points (positive `y` is away from the person, which
    /// scrolls up), `modifiers` held.
    pub fn wheel(&mut self, at: Pos2, delta: Vec2, modifiers: Modifiers) -> &mut Self {
        self.settle();
        self.go(at);
        let before = self.held;
        self.held = before | modifiers;
        let event = egui::Event::MouseWheel { unit: egui::MouseWheelUnit::Point, delta, phase: egui::TouchPhase::Move, modifiers: self.held };
        self.frame(vec![event]);
        self.held = before;
        self.stepped("a turn of the wheel");
        self
    }

    /// HOLD `modifiers` DOWN until [`Session::let_go`], across every gesture in between.
    pub fn hold(&mut self, modifiers: Modifiers) -> &mut Self {
        self.held |= modifiers;
        self
    }

    /// Let go of every key held down.
    pub fn let_go(&mut self) -> &mut Self {
        self.held = Modifiers::default();
        self
    }

    /// PRESS AND RELEASE `key`, with whatever is held.
    pub fn key(&mut self, key: Key) -> &mut Self {
        self.chord(Modifiers::default(), key)
    }

    /// PRESS AND RELEASE `key` with `modifiers` down: Ctrl+K is `chord(Modifiers::COMMAND, Key::K)`.
    pub fn chord(&mut self, modifiers: Modifiers, key: Key) -> &mut Self {
        self.settle();
        let before = self.held;
        self.held = before | modifiers;
        let event = |pressed| egui::Event::Key { key, physical_key: None, pressed, repeat: false, modifiers: before | modifiers };
        self.frame(vec![event(true)]);
        self.frame(vec![event(false)]);
        self.held = before;
        self.stepped(&format!("the key {key:?}"));
        self
    }

    /// TYPE `text` on the keyboard, into whatever holds the keyboard.
    pub fn type_text(&mut self, text: &str) -> &mut Self {
        self.settle();
        self.frame(vec![egui::Event::Text(text.to_string())]);
        self.stepped(&format!("typing {text:?}"));
        self
    }

    /// PASTE `text` from the clipboard, as Ctrl+V does.
    pub fn paste(&mut self, text: &str) -> &mut Self {
        self.settle();
        self.frame(vec![egui::Event::Paste(text.to_string())]);
        self.stepped("a paste");
        self
    }

    /// COPY, as Ctrl+C does; what reached the clipboard is in [`Session::clipboard`].
    pub fn copy(&mut self) -> &mut Self {
        self.settle();
        self.frame(vec![egui::Event::Copy]);
        self.stepped("a copy");
        self
    }

    /// CUT, as Ctrl+X does.
    pub fn cut(&mut self) -> &mut Self {
        self.settle();
        self.frame(vec![egui::Event::Cut]);
        self.stepped("a cut");
        self
    }

    /// WHICH CHOOSER THE PROGRAM HAS PUT UP and waits on, if any.
    pub fn chooser(&mut self) -> Option<Chooser> {
        self.settle();
        crate::system::with_stand_in(|s| s.asked.as_ref().map(|(kind, _)| *kind)).flatten()
    }

    /// ANSWER THE CHOOSER with `path`, as a person picks a file in it.
    ///
    /// # Panics
    /// When no chooser is up.
    pub fn answer_file(&mut self, path: impl AsRef<std::path::Path>) -> &mut Self {
        self.answer(Some(path.as_ref().to_path_buf()))
    }

    /// CLOSE THE CHOOSER without choosing.
    ///
    /// # Panics
    /// When no chooser is up.
    pub fn cancel_file(&mut self) -> &mut Self {
        self.answer(None)
    }

    /// WHAT THE PROGRAM ASKED THE SYSTEM TO START - a file manager, a browser - with its arguments, in order.
    pub fn started(&self) -> Vec<(String, Vec<String>)> {
        crate::system::with_stand_in(|s| s.started.clone()).unwrap_or_default()
    }

    /// WHAT REACHED THE CLIPBOARD, oldest first.
    pub fn clipboard(&self) -> &[String] {
        &self.win.copied
    }

    /// The pages the program asked the system to open, oldest first.
    pub fn opened_pages(&self) -> &[String] {
        &self.win.urls
    }

    /// RESIZE THE WINDOW to `size` points.
    pub fn resize(&mut self, size: Vec2) -> &mut Self {
        self.win.screen = size;
        self.settle();
        self
    }

    /// The size of the window, in points.
    pub fn screen(&self) -> Vec2 {
        self.win.across() // in points, which the interface scale changes
    }

    /// WHERE `text` IS WRITTEN ON SCREEN, whole - the place nearest to `near` when it is written in several.
    pub fn find(&mut self, text: &str, near: Pos2) -> Option<Rect> {
        self.settle();
        self.win.drawn.iter().filter(|(t, _)| t == text).map(|(_, r)| *r).min_by(|a, b| a.center().distance(near).total_cmp(&b.center().distance(near)))
    }

    /// PRESS WHERE `text` IS WRITTEN - a menu item, a tree row, a button with a caption.
    ///
    /// # Panics
    /// When `text` is not on screen, or is written in more than one place: then [`Session::press_word_near`] says
    /// which one.
    pub fn press_word(&mut self, text: &str) -> &mut Self {
        let places: Vec<Rect> = self.places_of(text);
        match places.as_slice() {
            [one] => self.click(one.center()),
            [] => panic!("{text:?} is not on screen; on screen: {:?}", self.words()),
            many => panic!("{text:?} is written in {} places, {many:?}: say which with press_word_near", many.len()),
        }
    }

    /// PRESS WHERE `text` IS WRITTEN, the place nearest to `near` when it is written in several.
    ///
    /// # Panics
    /// When `text` is not on screen.
    pub fn press_word_near(&mut self, text: &str, near: Pos2) -> &mut Self {
        match self.find(text, near) {
            Some(r) => self.click(r.center()),
            None => panic!("{text:?} is not on screen; on screen: {:?}", self.words()),
        }
    }

    /// WHERE THE SIDE PANEL `id` STANDS, as the last frame laid it out: its outer rectangle, the divider at its edge.
    /// `None` when no such panel was drawn.
    pub fn panel(&mut self, id: &str) -> Option<Rect> {
        self.settle();
        egui::PanelState::load(&self.win.ctx, egui::Id::new(id)).map(|p| p.outer_rect)
    }

    /// WHAT THE LAST FRAME PAINTED that a person reads as content: every run of words, and every filled background of
    /// one row's height - a highlight, a button - named "a filled background". The ground of a whole area is left out.
    pub fn painted(&mut self) -> Vec<(Rect, String)> {
        fn collect(s: &egui::Shape, out: &mut Vec<(Rect, String)>) {
            match s {
                egui::Shape::Text(t) => out.push((Rect::from_min_size(t.pos, t.galley.size()), t.galley.text().to_string())),
                egui::Shape::Rect(r) if r.fill.a() > 0 && r.rect.height() <= 40.0 => out.push((r.rect, "a filled background".to_string())),
                egui::Shape::Vec(v) => v.iter().for_each(|x| collect(x, out)),
                _ => {}
            }
        }
        self.settle();
        let mut out = Vec::new();
        for cs in &self.win.shapes {
            collect(&cs.shape, &mut out);
        }
        out
    }

    /// GO DOWN A MENU: press each of `path` in turn, each nearest to the one pressed before it.
    ///
    /// # Panics
    /// When an item of the path is not on screen.
    pub fn menu(&mut self, path: &[&str]) -> &mut Self {
        let mut near = pos2(0.0, 0.0);
        for item in path {
            // an item that names what it acts on reads "Undo: Extrusion" - it is still the item "Undo"
            let found = match self.find(item, near) {
                Some(r) => Some(r),
                None => {
                    let lead = format!("{item}:");
                    self.win.drawn.iter().filter(|(t, _)| t.starts_with(&lead)).map(|(_, r)| *r).min_by(|a, b| a.center().distance(near).total_cmp(&b.center().distance(near)))
                }
            };
            let Some(r) = found else { panic!("{item:?} of the menu path {path:?} is not on screen; on screen: {:?}", self.words()) };
            self.click(r.center());
            near = r.center();
        }
        self
    }

    /// PRESS THE BUTTON WHOSE HINT IS `hint`, found as a person finds an icon they do not know: the pointer goes
    /// over the buttons and rests until the hint comes up; a panel longer than the window is scrolled with the
    /// wheel.
    ///
    /// # Panics
    /// When no button of the window has that hint.
    pub fn press_hint(&mut self, hint: &str) -> &mut Self {
        match self.find_hint(hint) {
            Some(at) => self.click(at),
            None => panic!("no button of the window has the hint {hint:?}"),
        }
    }

    /// WHERE THE BUTTON WITH THE HINT `hint` IS, looked for as [`Session::press_hint`] looks; `None` when no
    /// button has it.
    pub fn find_hint(&mut self, hint: &str) -> Option<Pos2> {
        if let Some(at) = self.look_for_hint(hint) {
            return Some(at);
        }
        // A PANEL LEFT SCROLLED DOWN by a button pressed before hides what stands above: a person scrolls back up and
        // looks again
        for _ in 0..12 {
            let Some(over) = self.win.plates.first().copied() else { break };
            let before = self.win.plates.clone();
            self.wheel(over, vec2(0.0, 200.0), Modifiers::default());
            self.settle();
            if self.win.plates == before {
                break;
            }
        }
        // the wheel was turned over the first plate, and the pointer left resting there raises its hint before it
        // is read: a hint already up reads as none. Measured on the part with a block: the sketch pencil, first of the
        // panel, went unread under the oracles
        self.pointer_off_the_plates();
        if let Some(at) = self.look_for_hint(hint) {
            return Some(at);
        }
        // AN ICON BUTTON OUTSIDE THE TOOL PLATES - the cross beside a line of the properties: a person rests the
        // pointer over the small buttons in turn, as over the plates
        let small = self.small_buttons();
        small.into_iter().find(|&at| self.hint_comes_up(at, hint))
    }

    /// Look for the button with the hint `hint` from where the panel stands now, scrolling downwards.
    fn look_for_hint(&mut self, hint: &str) -> Option<Pos2> {
        self.settle();
        let mut scrolled = 0;
        loop {
            for at in self.win.plates.clone() {
                if self.hint_comes_up(at, hint) {
                    return Some(at);
                }
            }
            // down the panel the buttons stand in, a notch of the wheel at a time
            let over = self.win.plates.first().copied()?;
            let before = self.win.plates.clone();
            self.wheel(over, vec2(0.0, -200.0), Modifiers::default());
            self.settle();
            scrolled += 1;
            if self.win.plates == before || scrolled > 10 {
                return None; // the panel went no further: there is no such button
            }
        }
    }

    /// THE HINTS OF THE SMALL ICON BUTTONS OUTSIDE THE TOOL PLATES - the cross beside a line of the properties - read
    /// as the plates are: the pointer rests over each in turn. A button without a hint is left out.
    pub fn icon_hints(&mut self) -> Vec<String> {
        let small = self.small_buttons();
        let mut hints: Vec<String> = Vec::new();
        for at in small {
            let words = self.hint_at(at).join(" ");
            if !words.is_empty() && !hints.contains(&words) {
                hints.push(words);
            }
        }
        hints
    }

    /// Take the pointer off the buttons so no hint is up when they are read: to the empty right end of the menu bar,
    /// not onto the canvas - there the readout of the coordinates changes as the pointer leaves it again, and its words
    /// would be read as part of the next hint.
    fn pointer_off_the_plates(&mut self) {
        let c = self.canvas();
        self.move_to(pos2(c.max.x - 8.0, 10.0));
    }

    /// Where the small buttons of the window stand: an icon, not a tool plate and not a word.
    fn small_buttons(&mut self) -> Vec<Pos2> {
        self.widgets().into_iter().filter(|w| matches!(w.kind, crate::gui::window::Kind::Button) && w.rect.width() <= 32.0 && w.rect.height() <= 32.0).map(|w| w.rect.center()).collect()
    }

    /// THE HINT OF EVERY TOOL BUTTON OF THE WINDOW, read as a person reads them: the pointer rests over each button in
    /// turn, and a panel longer than the window is scrolled with the wheel. A button without a hint is given as empty.
    pub fn button_hints(&mut self) -> Vec<String> {
        self.settle();
        // a panel left scrolled down by what was done before hides its first buttons: back to the top first, as
        // `find_hint` goes
        for _ in 0..12 {
            let Some(over) = self.win.plates.first().copied() else { break };
            let before = self.win.plates.clone();
            self.wheel(over, vec2(0.0, 200.0), Modifiers::default());
            self.settle();
            if self.win.plates == before {
                break;
            }
        }
        // the wheel was turned over the first plate, and the pointer left resting there raises its hint before it
        // is read: a hint already up reads as none. Measured on the part with a block: the sketch pencil, first of the
        // panel, went unread under the oracles
        self.pointer_off_the_plates();
        let mut hints: Vec<String> = Vec::new();
        let mut scrolled = 0;
        loop {
            for at in self.win.plates.clone() {
                let words = self.hint_at(at).join(" ");
                if !hints.contains(&words) {
                    hints.push(words);
                }
            }
            let Some(over) = self.win.plates.first().copied() else { return hints };
            let before = self.win.plates.clone();
            self.wheel(over, vec2(0.0, -200.0), Modifiers::default());
            self.settle();
            scrolled += 1;
            if self.win.plates == before || scrolled > 10 {
                return hints;
            }
        }
    }

    /// THE HINT OF WHAT IS UNDER `at`, read as it comes up after the pointer rests there; `None` when nothing
    /// new comes up.
    pub fn hint_at(&mut self, at: Pos2) -> Vec<String> {
        self.settle();
        let before: Vec<String> = self.win.drawn.iter().map(|(t, _)| t.clone()).collect();
        self.rest_over(at);
        // the frames of the rest each draw the hint once: its words count once
        let mut came: Vec<String> = Vec::new();
        for (t, _) in &self.win.drawn {
            if !t.is_empty() && !before.contains(t) && !came.contains(t) {
                came.push(t.clone());
            }
        }
        came
    }

    /// EVERY WIDGET ON SCREEN once it is at rest, as the window describes them to a screen reader.
    pub fn widgets(&mut self) -> Vec<Widget> {
        self.settle();
        self.win.widgets.clone()
    }

    /// THE FIELD A PERSON FILLS UNDER THE CAPTION `label`: the text field, spin box, list or checkbox that stands
    /// nearest after the caption - on its row to the right first, below it otherwise.
    ///
    /// # Panics
    /// When `label` is not on screen or nothing to fill stands after it.
    pub fn field(&mut self, label: &str) -> Widget {
        self.settle();
        let captions = self.places_of(label);
        let inputs: Vec<Widget> = self.win.widgets.iter().filter(|w| matches!(w.kind, Kind::TextField | Kind::Number | Kind::ComboBox | Kind::Slider)).cloned().collect();
        let after = |caption: &Rect, w: &Widget| {
            let same_row = w.rect.min.y < caption.max.y && w.rect.max.y > caption.min.y && w.rect.min.x >= caption.max.x - 1.0;
            let below = w.rect.min.y >= caption.max.y - 1.0 && w.rect.min.x < caption.max.x;
            if same_row {
                Some(w.rect.min.x - caption.max.x)
            } else if below {
                Some(1000.0 + w.rect.min.y - caption.max.y)
            } else {
                None
            }
        };
        let best = captions.iter().flat_map(|c| inputs.iter().filter_map(move |w| after(c, w).map(|d| (d, w)))).min_by(|a, b| a.0.total_cmp(&b.0));
        match best {
            Some((_, w)) => w.clone(),
            None => panic!("nothing to fill stands after {label:?} (at {captions:?}); fields on screen: {inputs:?}"),
        }
    }

    /// FILL THE FIELD under the caption `label` with `text`: click into it, select what is there, type over it.
    /// Nothing is pressed after - Enter and Tab are the check's to press.
    pub fn fill(&mut self, label: &str, text: &str) -> &mut Self {
        let field = self.field(label);
        self.click(field.rect.center()).chord(Modifiers::COMMAND, Key::A).type_over(text)
    }

    /// TYPE `text` OVER WHAT IS SELECTED - nothing typed clears it, as Backspace does: typing no letters over a
    /// selection leaves it standing, which is not what a person emptying a field does.
    fn type_over(&mut self, text: &str) -> &mut Self {
        if text.is_empty() {
            self.key(Key::Backspace)
        } else {
            self.type_text(text)
        }
    }

    /// FILL THE EMPTY FIELD that shows `placeholder` in grey - the name of a new row, say - with `text`.
    ///
    /// # Panics
    /// When no empty field shows it, or more than one does.
    pub fn fill_empty(&mut self, placeholder: &str, text: &str) -> &mut Self {
        self.settle();
        let fields: Vec<Widget> = self.win.widgets.iter().filter(|w| w.kind == Kind::TextField && w.placeholder == placeholder && w.value.is_empty()).cloned().collect();
        match fields.as_slice() {
            [one] => self.click(one.rect.center()).chord(Modifiers::COMMAND, Key::A).type_over(text),
            [] => panic!("no empty field shows {placeholder:?}"),
            many => panic!("{} empty fields show {placeholder:?}: {many:?}", many.len()),
        }
    }

    /// FILL THE FIELD THAT SHOWS `placeholder` in grey - a field of a dimension holds a number and still shows what it
    /// would take - with `text`, over what it holds.
    ///
    /// # Panics
    /// When no field shows it, or more than one does.
    pub fn fill_hinted(&mut self, placeholder: &str, text: &str) -> &mut Self {
        self.settle();
        let fields: Vec<Widget> = self.win.widgets.iter().filter(|w| w.kind == Kind::TextField && w.placeholder == placeholder).cloned().collect();
        match fields.as_slice() {
            [one] => self.click(one.rect.center()).chord(Modifiers::COMMAND, Key::A).type_over(text),
            [] => panic!("no field shows {placeholder:?}"),
            many => panic!("{} fields show {placeholder:?}: {many:?}", many.len()),
        }
    }

    /// FILL THE FIRST FIELD, in reading order, THAT SHOWS `placeholder` in grey with `text`, over what it holds: the
    /// first value of a bar that holds two fields of the same grey words - the size of a chamfer and its second leg.
    ///
    /// # Panics
    /// When no field shows it.
    pub fn fill_first_hinted(&mut self, placeholder: &str, text: &str) -> &mut Self {
        self.settle();
        let first = self
            .win
            .widgets
            .iter()
            .filter(|w| w.kind == Kind::TextField && w.placeholder == placeholder)
            .min_by(|a, b| (a.rect.min.y, a.rect.min.x).partial_cmp(&(b.rect.min.y, b.rect.min.x)).unwrap_or(std::cmp::Ordering::Equal))
            .cloned();
        match first {
            Some(one) => self.click(one.rect.center()).chord(Modifiers::COMMAND, Key::A).type_over(text),
            None => panic!("no field shows {placeholder:?}"),
        }
    }

    /// TICK OR UNTICK THE CHECKBOX whose words are `label`.
    ///
    /// # Panics
    /// When no checkbox has those words.
    pub fn toggle(&mut self, label: &str) -> &mut Self {
        self.settle();
        let boxes: Vec<Widget> = self.win.widgets.iter().filter(|w| w.kind == Kind::CheckBox && w.label == label).cloned().collect();
        match boxes.as_slice() {
            [one] => self.click(one.rect.center()),
            [] => panic!("no checkbox says {label:?}; checkboxes on screen: {:?}", self.win.widgets.iter().filter(|w| w.kind == Kind::CheckBox).map(|w| &w.label).collect::<Vec<_>>()),
            many => panic!("{} checkboxes say {label:?}: {many:?}", many.len()),
        }
    }

    /// THE CANVAS - the sketch sheet or the 3D view - where the window laid it out.
    pub fn canvas(&mut self) -> Rect {
        self.settle();
        self.app.viewing.view_rect
    }

    /// WHERE THE POINT (x, y) OF THE SKETCH OPEN FOR EDITING STANDS ON SCREEN, on the canvas as the window laid it
    /// out. A point out of view is brought into it first, the way a person does it: the sheet is dragged with the
    /// middle button.
    ///
    /// # Panics
    /// When no sketch is open on the flat canvas, or the point cannot be brought into view.
    pub fn on_sketch(&mut self, x: f64, y: f64) -> Pos2 {
        self.settle();
        assert!(!self.app.viewing.mode_3d && qymcad_ui_state::edit_si(&self.app.project, &self.app.sketch_ses).is_some(), "no sketch is open on the flat canvas");
        let at = |s: &Session| qymcad_ui_state::Sheet { view: s.app.viewing.view, rect: s.app.viewing.view_rect }.at(qymcad_core::geom::Point2::new(x, y));
        self.bring_into_view(&at, &[PointerButton::Middle], Modifiers::default());
        at(self)
    }

    /// WHERE THE POINT (x, y) OF THE FLAT SHEET IS CLICKED, whatever sketch the sheet shows - one open for editing, or
    /// a finished one a tool lays flat for its contours to be picked - brought into view as [`Session::on_sketch`]
    /// brings it.
    ///
    /// # Panics
    /// When the canvas is not the flat sheet, or the point cannot be brought into view.
    pub fn on_sheet(&mut self, x: f64, y: f64) -> Pos2 {
        self.settle();
        assert!(!self.app.viewing.mode_3d, "the canvas is not the flat sheet");
        let at = |s: &Session| qymcad_ui_state::Sheet { view: s.app.viewing.view, rect: s.app.viewing.view_rect }.at(qymcad_core::geom::Point2::new(x, y));
        self.bring_into_view(&at, &[PointerButton::Middle], Modifiers::default());
        at(self)
    }

    /// WHERE THE POINT (x, y) OF THE OPEN SKETCH IS SEEN NOW, with the sheet left as it stands - what the eye reads
    /// off the screen while the sheet is moved or scaled. `None` when it falls outside the canvas or no sketch is open
    /// on the flat canvas. Unlike [`Session::on_sketch`] it moves nothing.
    pub fn seen_on_sketch(&mut self, x: f64, y: f64) -> Option<Pos2> {
        self.settle();
        if self.app.viewing.mode_3d || qymcad_ui_state::edit_si(&self.app.project, &self.app.sketch_ses).is_none() {
            return None;
        }
        let at = qymcad_ui_state::Sheet { view: self.app.viewing.view, rect: self.app.viewing.view_rect }.at(qymcad_core::geom::Point2::new(x, y));
        self.app.viewing.view_rect.contains(at).then_some(at)
    }

    /// CLICK THE POINT (x, y) OF THE OPEN SKETCH, brought into view as [`Session::on_sketch`] brings it.
    pub fn click_on_sketch(&mut self, x: f64, y: f64) -> &mut Self {
        let at = self.on_sketch(x, y);
        self.click(at)
    }

    /// DRAG ON THE OPEN SKETCH from the point `from` to the point `to` with the left button: both are brought into
    /// view together first - a place worked out before the view moved is a place on another part of the sheet.
    ///
    /// # Panics
    /// When no sketch is open, or the two points do not fit on the canvas at once.
    pub fn drag_on_sketch(&mut self, from: (f64, f64), to: (f64, f64)) -> &mut Self {
        let mid = ((from.0 + to.0) / 2.0, (from.1 + to.1) / 2.0);
        self.on_sketch(mid.0, mid.1);
        let sheet = qymcad_ui_state::Sheet { view: self.app.viewing.view, rect: self.app.viewing.view_rect };
        let (a, b) = (sheet.at(qymcad_core::geom::Point2::new(from.0, from.1)), sheet.at(qymcad_core::geom::Point2::new(to.0, to.1)));
        let canvas = self.app.viewing.view_rect;
        assert!(canvas.contains(a) && canvas.contains(b), "{from:?} and {to:?} do not fit on the canvas at once ({a:?}, {b:?} on {canvas:?}): zoom out first");
        self.drag(a, b, PointerButton::Primary, Modifiers::default())
    }

    /// WHAT A CLICK AT (x, y) OF THE OPEN SKETCH WOULD TAKE, brought into view as [`Session::on_sketch`] does.
    pub fn sketch_under(&mut self, x: f64, y: f64) -> Option<SketchPick> {
        let pos = self.on_sketch(x, y);
        let si = qymcad_ui_state::edit_si(&self.app.project, &self.app.sketch_ses)?;
        let (kind, id) = qymcad_sketch::sketch_hit(&self.app.pick_ctx(), self.app.viewing.view_rect, pos, si)?;
        let sk = &self.app.project.sketches[si];
        let pt = |id: u64| sk.points.iter().find(|p| p.id == id).map(|p| (p.x, p.y)).unwrap_or((f64::NAN, f64::NAN));
        if kind == 0 {
            return Some(SketchPick::Point { at: pt(id) });
        }
        use qymcad_core::model::EntityKind;
        if let Some(e) = sk.entities.iter().find(|e| e.id == id) {
            return Some(match e.kind {
                EntityKind::Line { a, b } => SketchPick::Line { from: pt(a), to: pt(b) },
                EntityKind::Arc { center, a, b, ccw } => SketchPick::Arc { centre: pt(center), from: pt(a), to: pt(b), ccw },
                EntityKind::Circle { center, r } => SketchPick::Circle { centre: pt(center), radius: r },
                EntityKind::Ellipse { c, ma, mi } => SketchPick::Ellipse { centre: pt(c), major: pt(ma), minor: pt(mi) },
            });
        }
        let spline = sk.splines.iter().find(|sp| sp.points.contains(&id))?;
        Some(SketchPick::Spline { nodes: spline.points.iter().map(|p| pt(*p)).collect() })
    }

    /// WHERE THE POINT `p` OF THE 3D VIEW FALLS ON SCREEN AS THE VIEW STANDS NOW - no gesture, nothing brought into view,
    /// covered or not: to see whether the view shows the whole of something.
    pub fn projected(&mut self, p: [f64; 3]) -> Pos2 {
        self.settle();
        let basis = self.app.viewing.cam.basis();
        qymcad_ui_state::Screen { cam: &self.app.viewing.cam, set: &self.app.set, rect: self.app.viewing.view_rect, basis: &basis }.at(p).0
    }

    /// WHERE THE POINT `p` OF THE 3D VIEW STANDS ON SCREEN, brought into view first with the gesture that moves
    /// the view under the person's mouse settings.
    ///
    /// # Panics
    /// When the view is not 3D, the point cannot be brought into view, or something nearer to the eye covers it.
    pub fn in_space(&mut self, p: [f64; 3]) -> Pos2 {
        self.settle();
        assert!(self.app.viewing.mode_3d, "the canvas is not the 3D view");
        let at = move |s: &Session| {
            let basis = s.app.viewing.cam.basis();
            qymcad_ui_state::Screen { cam: &s.app.viewing.cam, set: &s.app.set, rect: s.app.viewing.view_rect, basis: &basis }.at(p).0
        };
        let pan = self.app.set.mouse_nav.pan();
        // any button of ours moves the view with Shift, and the middle one does it anywhere: Shift and the left one, begun
        // on a part in an assembly, carries the part
        let buttons: Vec<PointerButton> = if pan.any_button { vec![PointerButton::Middle] } else { pan.buttons.to_vec() };
        let modifiers = Modifiers { shift: pan.shift, ctrl: pan.ctrl, alt: pan.alt, command: pan.ctrl, mac_cmd: false };
        self.bring_into_view(&at, &buttons, modifiers);
        let pos = at(self);
        let (tol, depth) = self.pixel_and_depth(p);
        if let Some((hit, _, face)) = qymcad_pick::face_under_cursor(&self.app.painting(), self.app.viewing.view_rect, pos) {
            assert!(hit >= depth - 4.0 * tol, "{p:?} is hidden: a face around {:?} lies {} mm in front of it; turn the view first", face.centroid, depth - hit);
        }
        if let Some(w) = self.covering(pos) {
            panic!("{p:?} is covered on screen by {:?} {:?}: a click there lands on it, not on the model", w.kind, w.label);
        }
        pos
    }

    /// WHAT OF THE WINDOW STANDS OVER THE CANVAS AT `pos` - a button, a field, a panel - and so takes a click aimed at
    /// the model there. A widget takes a click that falls within its reach of it, not only on it: 1.4 px above the
    /// Apply button of a tool's popup the click went to the popup, and the edge under it was not picked.
    fn covering(&mut self, pos: Pos2) -> Option<Widget> {
        let canvas = self.app.viewing.view_rect;
        let reach = self.win.ctx.global_style().interaction.interact_radius;
        self.win.widgets.iter().find(|w| w.rect.expand(reach).contains(pos) && w.rect != canvas).cloned()
    }

    /// WHERE THE POINT `p` OF THE WORLD IS SEEN NOW, with the view left as it stands - what the eye reads off the
    /// screen while the view is being turned, moved or scaled. `None` when it falls outside the canvas or the
    /// canvas is the flat sheet. Unlike [`Session::in_space`] it moves nothing.
    pub fn seen_at(&mut self, p: [f64; 3]) -> Option<Pos2> {
        self.settle();
        if !self.app.viewing.mode_3d {
            return None;
        }
        let basis = self.app.viewing.cam.basis();
        let pos = qymcad_ui_state::Screen { cam: &self.app.viewing.cam, set: &self.app.set, rect: self.app.viewing.view_rect, basis: &basis }.at(p).0;
        self.app.viewing.view_rect.contains(pos).then_some(pos)
    }

    /// WHERE THE SIDE OF THE NAVIGATION CUBE THAT READS `side` IS CLICKED - TOP, FRONT and the rest. The captions
    /// of the cube are baked into the drawing of it, so they stand on no word of the screen; the eye reads them off
    /// the cube all the same. `None` when no side reads that.
    pub fn cube_side(&mut self, side: &str) -> Option<Pos2> {
        self.settle();
        let cube = self.app.cube_ctx();
        let rect = self.app.viewing.view_rect;
        let zones = super::viewcube::zones();
        let i = zones.iter().position(|z| z.label.is_some_and(|k| crate::i18n::tr(k) == side))?;
        Some(super::viewcube::zone_center(&cube, rect, i))
    }

    /// WHERE THE NAVIGATION CUBE IS CLICKED TO LOOK FROM `dir` - the way from the model to the eye, each coordinate
    /// -1, 0 or 1: a side of the cube for one axis, an edge of it for two, a corner for three. `None` when that part
    /// of the cube is on its far side, out of the eye's reach, as it stands now: a person turns the cube to it first.
    pub fn cube_toward(&mut self, dir: [i8; 3]) -> Option<Pos2> {
        self.settle();
        let cube = self.app.cube_ctx();
        let rect = self.app.viewing.view_rect;
        let zones = super::viewcube::zones();
        let sign = |v: f64| {
            if v > 1e-9 {
                1
            } else if v < -1e-9 {
                -1
            } else {
                0
            }
        };
        let i = zones.iter().position(|z| z.dir.map(sign) == dir)?;
        let at = super::viewcube::zone_center(&cube, rect, i);
        (super::viewcube::zone_at(&cube, rect, at) == Some(i)).then_some(at)
    }

    /// CAN THE POINT `p` OF SPACE BE SEEN AS THE VIEW STANDS: on the canvas, and no face nearer to the eye covers it.
    /// The view is not moved.
    pub fn sees(&mut self, p: [f64; 3]) -> bool {
        let Some(pos) = self.seen_at(p) else { return false };
        if self.covering(pos).is_some() {
            return false;
        }
        let (tol, depth) = self.pixel_and_depth(p);
        qymcad_pick::face_under_cursor(&self.app.painting(), self.app.viewing.view_rect, pos).is_none_or(|(hit, _, _)| hit >= depth - 4.0 * tol)
    }

    /// WHERE THE GIZMO OF WHAT IS CHOSEN STANDS: the arms to carry it and the rings to turn it. `None` when what
    /// is chosen carries no gizmo, or the canvas is the flat sheet.
    pub fn gizmo(&mut self) -> Option<Gizmo> {
        self.settle();
        if !self.app.viewing.mode_3d {
            return None;
        }
        let app = &self.app;
        let (o, l) = match app.chosen.sel {
            qymcad_ui_state::Sel::Component(i) => {
                let comp = app.project.components.get(i)?.id;
                qymcad_ui_state::gizmo_geometry(app.viewing.cam, app.dragged.comp_giz, &app.project, comp)
            }
            sel => {
                let (_, mi) = qymcad_ui_state::body_gizmo_target(qymcad_ui_state::body_view_of!(app), sel)?;
                qymcad_ui_state::body_gizmo_geometry(&app.dragged.body_giz, app.viewing.cam, &app.project, &app.set, mi)
            }
        };
        let basis = app.viewing.cam.basis();
        let scr = qymcad_ui_state::Screen { cam: &app.viewing.cam, set: &app.set, rect: app.viewing.view_rect, basis: &basis };
        // halfway along the arm, and the ring a quarter of the way round from its first axis - the place on it
        // furthest from both arms it lies between
        let arm = |ax: usize| {
            let mut tip = o;
            tip[ax] += l / 2.0;
            scr.at(tip).0
        };
        let ring = |ax: u8| {
            let (u, v) = qymcad_ui_state::ring_axes(ax);
            let (c, s) = (std::f64::consts::FRAC_PI_4.cos(), std::f64::consts::FRAC_PI_4.sin());
            scr.at([o[0] + l * (u[0] * c + v[0] * s), o[1] + l * (u[1] * c + v[1] * s), o[2] + l * (u[2] * c + v[2] * s)]).0
        };
        Some(Gizmo { arms: [arm(0), arm(1), arm(2)], rings: [ring(0), ring(1), ring(2)] })
    }

    /// IS THE CANVAS THE FLAT SHEET? - the sketch drawn flat before the eye, rather than the model in space.
    pub fn flat(&mut self) -> bool {
        self.settle();
        !self.app.viewing.mode_3d
    }

    /// WHERE A FACE THROUGH `p` IS CLICKED: the point in view, and a click there takes a face through `p`.
    ///
    /// # Panics
    /// As [`Session::in_space`] does, and when the surface under the pointer does not pass through `p`.
    pub fn face_at(&mut self, p: [f64; 3]) -> Pos2 {
        let pos = self.in_space(p);
        let (tol, depth) = self.pixel_and_depth(p);
        let hit = qymcad_pick::face_under_cursor(&self.app.painting(), self.app.viewing.view_rect, pos).map(|(d, _, _)| d);
        assert!(hit.is_some_and(|d| (d - depth).abs() < 4.0 * tol), "under the pointer at {pos:?} there is no face through {p:?}: the face there stands at depth {hit:?}, the point at {depth}");
        pos
    }

    /// WHERE AN EDGE THROUGH `p` IS CLICKED: the point in view, and the edge the pointer takes passes through `p`.
    ///
    /// # Panics
    /// As [`Session::in_space`] does, and when the edge under the pointer does not pass through `p`.
    pub fn edge_at(&mut self, p: [f64; 3]) -> Pos2 {
        let pos = self.in_space(p);
        let (tol, _) = self.pixel_and_depth(p);
        let hit = qymcad_pick::pick_edge_point(&self.app.painting(), self.app.viewing.view_rect, pos);
        assert!(hit.is_some_and(|h| dist(h, p) < 4.0 * tol), "under the pointer at {pos:?} there is no edge through {p:?}: the nearest edge point is {hit:?}");
        pos
    }

    /// WHERE THE VERTEX `p` IS CLICKED: the point in view, and the vertex the pointer takes is `p`.
    ///
    /// # Panics
    /// As [`Session::in_space`] does, and when the vertex under the pointer is not `p`.
    pub fn vertex_at(&mut self, p: [f64; 3]) -> Pos2 {
        let pos = self.in_space(p);
        let (tol, _) = self.pixel_and_depth(p);
        let hit = qymcad_pick::pick_vertex_pos(&self.app.painting(), self.app.viewing.view_rect, pos);
        assert!(hit.is_some_and(|h| dist(h, p) < 4.0 * tol), "under the pointer at {pos:?} there is no vertex at {p:?}: the vertex there is {hit:?}");
        pos
    }

    /// `f` DONE ASIDE: what is chosen and the status line are as they were when it returns - for a check that makes
    /// steps of its own between the steps of a person (taking a step back and putting it again, rebuilding
    /// everything) and must leave the next step of the person where the last one left it.
    pub fn aside<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
        let (sel, multi, status) = (self.app.chosen.sel, self.app.chosen.tree_sel.multi.clone(), self.app.status.clone());
        let out = f(self);
        (self.app.chosen.sel, self.app.chosen.tree_sel.multi, self.app.status) = (sel, multi, status);
        out
    }

    /// HOW MANY THINGS OF THE SKETCH STAND LIT - the count a person would count by eye.
    pub fn lit_in_sketch(&self) -> usize {
        self.app.tools.sel_sk.items.len()
    }

    /// WHICH EDITING BUTTON IS IN HAND, by its own number: 4 the corner fillet, 5 the chamfer, 0 none of them.
    pub fn editing_tool(&self) -> u8 {
        self.app.tools.armed.click_op()
    }

    /// Is a text field taking the keys - a person in the middle of typing, where Ctrl+Z is the field's own?
    pub fn typing(&self) -> bool {
        self.win.ctx.egui_wants_keyboard_input()
    }

    /// Is the program asking for a file - a chooser up and waiting for its answer?
    pub fn asking_for_a_file(&self) -> bool {
        self.app.asking_for_a_file()
    }

    /// A COPY OF THE DOCUMENT WRITTEN TO `path`, the document keeping its own place and name - what a check of saving and
    /// opening needs in the middle of a session without changing the session it checks.
    pub fn save_copy(&mut self, path: &str) -> Result<(), String> {
        self.settle();
        // the live bodies go with it, as File -> Save writes them
        let live = &self.app.live;
        let breps: Vec<(qymcad_core::model::Id, Vec<u8>)> = live.shapes.iter().filter_map(|(id, sh)| live.blobs.get(id).cloned().or_else(|| sh.to_brep_bytes()).map(|b| (*id, b))).collect();
        let thumb = crate::gui::render_scene::render_project_file_preview(&self.app.draw_ctx(), self.app.project.root).as_ref().and_then(crate::gui::color_image_to_png);
        qymcad_io::save_project_guarded_bundle(&self.app.project, path, &breps, thumb.as_deref()).map_err(|e| e.to_string())
    }

    /// THE DOCUMENT once the program is at rest.
    pub fn document(&mut self) -> Document {
        self.settle();
        let app = &self.app;
        let p = &app.project;
        let named = |stored: &str| crate::i18n::name(stored);
        let comp_name = |id: Option<u64>| id.and_then(|id| p.components.iter().find(|c| c.id == id)).map(|c| named(&c.name));
        let parts = p
            .components
            .iter()
            .filter(|c| c.id != p.root)
            .map(|c| Part {
                name: named(&c.name),
                key: c.id,
                assembly: c.kind == qymcad_core::feature::ComponentKind::Assembly,
                parent: c.parent.filter(|id| *id != p.root).and_then(|id| comp_name(Some(id))),
                visible: c.visible,
                grounded: c.grounded,
                clone_of: Some(p.instance_origin(c.id)).filter(|o| *o != c.id).and_then(|o| p.components.iter().find(|q| q.id == o)).map(|q| named(&q.name)),
                at: {
                    let t = p.component_transform(c.id);
                    [t[3], t[7], t[11]]
                },
                axes: {
                    let t = p.component_transform(c.id);
                    [[t[0], t[4], t[8]], [t[1], t[5], t[9]], [t[2], t[6], t[10]]]
                },
            })
            .collect();
        use qymcad_core::model::EntityKind;
        let sketches = p
            .sketches
            .iter()
            .enumerate()
            .map(|(si, sk)| {
                let count = |f: fn(&EntityKind) -> bool| sk.entities.iter().filter(|e| f(&e.kind)).count();
                // counted as the sketch's own panel counts them: the origin, the axes and what holds them are not a
                // person's geometry
                let system: std::collections::HashSet<u64> = sk.system_ids().into_iter().collect();
                let own = |c: &qymcad_core::model::Constraint| !matches!(c, qymcad_core::model::Constraint::Fixed { p } if system.contains(p));
                let (dof, redundant) = p.sketch_dof(si);
                SketchInfo {
                    name: named(&sk.name),
                    part: comp_name(p.sketch_owner(sk.id)),
                    points: sk.points.iter().filter(|q| !system.contains(&q.id)).count(),
                    places: sk.points.iter().filter(|q| !system.contains(&q.id)).map(|q| [q.x, q.y]).collect(),
                    picked: if qymcad_ui_state::edit_si(p, &self.app.sketch_ses) == Some(si) { self.app.tools.sel_sk.items.len() } else { 0 },
                    seat: {
                        use qymcad_core::feature::{BasePlane, SketchPlane};
                        match sk.plane {
                            SketchPlane::World(BasePlane::XY) => "XY".to_string(),
                            SketchPlane::World(BasePlane::XZ) => "XZ".to_string(),
                            SketchPlane::World(BasePlane::YZ) => "YZ".to_string(),
                            SketchPlane::Datum(_) => "datum".to_string(),
                            SketchPlane::Face(body, _) => format!("face of {}", comp_name(p.body_owner(body)).unwrap_or_default()),
                        }
                    },
                    lines: count(|k| matches!(k, EntityKind::Line { .. })),
                    construction: sk.entities.iter().filter(|e| e.construction && !system.contains(&e.id)).count(),
                    arcs: count(|k| matches!(k, EntityKind::Arc { .. })),
                    circles: count(|k| matches!(k, EntityKind::Circle { .. })),
                    ellipses: count(|k| matches!(k, EntityKind::Ellipse { .. })),
                    splines: sk.splines.len(),
                    texts: sk.texts.len(),
                    notes: sk.notes.len(),
                    text_fonts: sk.texts.iter().map(|t| qymcad_ui_state::font_label(&t.font, "opt-font")).collect(),
                    min: box_of_sketch(sk, &system).0,
                    max: box_of_sketch(sk, &system).1,
                    constraints: sk.constraints.iter().filter(|c| own(c)).count(),
                    constraint_kinds: sk.constraints.iter().filter(|c| own(c)).map(|c| format!("{c:?}").split([' ', '{', '(']).next().unwrap_or_default().to_string()).collect(),
                    dof,
                    redundant,
                }
            })
            .collect();
        let features = p
            .timeline
            .iter()
            .map(|n| Feature {
                name: named(&n.name),
                kind: format!("{:?}", n.kind).split([' ', '{', '(']).next().unwrap_or_default().to_string(),
                part: comp_name(n.parent),
                suppressed: n.suppressed,
                error: p.regen_errors.get(&n.id).map(super::error_words::error_text),
                warning: p.regen_warnings.get(&n.id).map(super::error_words::error_text),
                key: n.id,
                bodies: n.kind.bodies().len(),
            })
            .collect();
        let consumed = p.consumed_bodies();
        let bodies = p
            .bodies
            .iter()
            .enumerate()
            .map(|(i, b)| {
                let m = &b.mesh;
                let shape = app.live.shapes.get(&b.id);
                let area = (0..m.tris.len())
                    .map(|ti| {
                        let t = m.triangle(ti);
                        let (u, v) = ([t[1].x - t[0].x, t[1].y - t[0].y, t[1].z - t[0].z], [t[2].x - t[0].x, t[2].y - t[0].y, t[2].z - t[0].z]);
                        0.5 * ((u[1] * v[2] - u[2] * v[1]).powi(2) + (u[2] * v[0] - u[0] * v[2]).powi(2) + (u[0] * v[1] - u[1] * v[0]).powi(2)).sqrt()
                    })
                    .sum();
                let bounds = m.bounds();
                Solid {
                    name: named(&b.name),
                    part: comp_name(p.body_owner(b.id)),
                    part_key: p.body_owner(b.id),
                    colour: p.mesh_color(i),
                    volume: shape.map(|s| s.volume()).unwrap_or_else(|| m.volume()),
                    area,
                    faces: b.faces.len(),
                    face_names: b.faces.iter().map(|f| f.id).collect(),
                    face_centres: b.faces.iter().map(|f| [f.centroid.x, f.centroid.y, f.centroid.z]).collect(),
                    edges: shape.map(|s| s.edges().len()),
                    min: bounds.as_ref().map(|bb| [bb.min.x, bb.min.y, bb.min.z]).unwrap_or([f64::NAN; 3]),
                    max: bounds.as_ref().map(|bb| [bb.max.x, bb.max.y, bb.max.z]).unwrap_or([f64::NAN; 3]),
                    visible: b.visible,
                    consumed: consumed.contains(&b.id),
                    sheet: b.sheet,
                }
            })
            .collect();
        Document {
            path: app.disk.project_path.clone(),
            unsaved: qymcad_ui_state::edit_key(&app.draw_ctx()) != app.disk.edits.saved_key,
            parts,
            sketches,
            editing: qymcad_ui_state::edit_si(p, &app.sketch_ses).map(|si| named(&p.sketches[si].name)),
            context: app.active_path.last().and_then(|id| p.components.iter().find(|c| c.id == *id)).map(|c| named(&c.name)).unwrap_or_else(|| qymcad_i18n::tr("wb-assembly")),
            datums: p
                .planes
                .iter()
                .map(|q| Datum { name: named(&q.name), kind: "plane".into(), at: q.origin, dir: q.normal })
                .chain(p.datum_axes.iter().map(|q| Datum { name: named(&q.name), kind: "axis".into(), at: q.origin(), dir: q.dir() }))
                .chain(p.datum_points.iter().map(|q| Datum { name: named(&q.name), kind: "point".into(), at: q.at, dir: [0.0; 3] }))
                .collect(),
            features,
            bodies,
            parameters: p.parameters.iter().map(|q| Parameter { name: q.name.clone(), expr: q.expr.clone(), value: q.value }).collect(),
            joints: p.joints.iter().map(|j| JointInfo { name: named(&j.name), kind: format!("{:?}", j.kind), violated: p.mates_violated.contains(&j.id) }).collect(),
            mates: p.mate_constraints.iter().map(|c| format!("{:?}", c.kind)).chain(p.relations.iter().map(|r| format!("{:?}", r.kind))).collect(),
            section: self.app.side.section.plane.is_some(),
            undo: app.disk.edits.undo.iter().map(|s| s.name.clone()).collect(),
            redo: app.disk.edits.redo.iter().map(|s| s.name.clone()).collect(),
        }
    }

    /// WHAT THE EXACT BODY NUMBERED `body` IN [`Document::bodies`] IS MADE OF; `None` while the program holds no exact
    /// body for it (a mesh brought in as it is) or the kernel cannot read it.
    pub fn inspect(&mut self, body: usize) -> Option<Inspection> {
        self.settle();
        let id = self.app.project.bodies.get(body)?.id;
        let shape = self.app.live.shapes.get(&id)?;
        let k = shape.face_kinds()?;
        Some(Inspection {
            valid: shape.is_valid(),
            solids: shape.solid_count(),
            shells: shape.shell_count(),
            kinds: FaceKinds { plane: k[0], cylinder: k[1], cone: k[2], sphere: k[3], torus: k[4], free: k[5], other: k[6] },
        })
    }

    /// HOW THE 3D VIEW DRAWS EACH PIECE OF WHAT IT SHOWS on a screen with a graphics card: the colour of the row of
    /// the table of looks its vertices name, and whether that row is painted as selected. It is what a person sees -
    /// the picture of [`Session::snapshot`] draws the space another way and cannot show what the card is handed.
    pub fn drawn(&mut self) -> Vec<([u8; 3], bool)> {
        self.settle();
        let scene = super::render_scene::gpu_scene(&self.app.painting());
        scene
            .pieces
            .iter()
            .filter_map(|p| p.verts.first().map(|v| v.body))
            .map(|row| {
                let look = scene.looks.get(row as usize).copied().unwrap_or(qymcad_ui_state::BodyLook { tint: 0, state: 0 });
                let [r, g, b, _] = look.tint.to_le_bytes();
                ([r, g, b], look.state & qymcad_ui_state::LOOK_HOT != 0)
            })
            .collect()
    }

    /// THE STATUS LINE once the program is at rest.
    pub fn status(&mut self) -> String {
        self.settle();
        // the icons of the interface are letters of a private range of the icon font, not words
        self.app.status.chars().filter(|c| !('\u{e000}'..='\u{f8ff}').contains(c)).collect::<String>().trim().to_string()
    }

    /// THE WINDOWS OPEN INSIDE THE PROGRAM, by their titles.
    pub fn windows(&mut self) -> Vec<String> {
        self.settle();
        self.win.widgets.iter().filter(|w| w.kind == Kind::Window).map(|w| w.label.clone()).collect()
    }

    /// WHAT IS IN HAND, as the bars of options across the top say it: the words written in every bar a tool or a
    /// command has put up. Empty when nothing is.
    pub fn in_hand(&mut self) -> Vec<String> {
        use qymcad_shell::Fills;
        self.settle();
        let shell = super::shell(&self.app.set);
        let bars: Vec<Rect> = shell
            .keys()
            .into_iter()
            .filter(|k| shell.slot_of(k) == Some(qymcad_shell::Slot::Top) && *k != "toolbar" && self.app.live(k))
            .filter_map(|k| egui::containers::panel::PanelState::load(&self.win.ctx, egui::Id::new(k)).map(|s| s.outer_rect))
            .collect();
        self.win.drawn.iter().filter(|(t, r)| !t.is_empty() && bars.iter().any(|b| b.contains(r.center()))).map(|(t, _)| t.clone()).collect()
    }

    /// EVERY WORD ON SCREEN AND WHERE IT STANDS, once the screen is at rest.
    pub fn words_at(&mut self) -> Vec<(String, Rect)> {
        self.settle();
        self.win.drawn.iter().filter(|(t, _)| !t.is_empty()).cloned().collect()
    }

    /// A PICTURE OF THE WINDOW once it is at rest, drawn on the processor from what the last frame drew.
    pub fn snapshot(&mut self) -> Picture {
        self.settle();
        let ppp = self.win.ctx.pixels_per_point();
        let size = [(self.win.screen.x * ppp).round() as usize, (self.win.screen.y * ppp).round() as usize];
        let prims = self.win.ctx.tessellate(self.win.shapes.clone(), ppp);
        let img = super::help_raster::paint(&prims, &self.win.textures, size, egui::Color32::BLACK, ppp);
        Picture { width: size[0], height: size[1], rgba: img.pixels.iter().flat_map(|c| c.to_array()).collect() }
    }

    /// CLOSE THE WINDOW TITLED `title` INSIDE THE PROGRAM with the cross on its title bar.
    ///
    /// # Panics
    /// When no such window is open, or it has no cross.
    pub fn close_window(&mut self, title: &str) -> &mut Self {
        self.settle();
        let Some(window) = self.win.widgets.iter().find(|w| w.kind == Kind::Window && w.label == title).map(|w| w.rect) else {
            panic!("no window titled {title:?} is open; open: {:?}", self.windows());
        };
        // the cross is the framework's own button, and it tells a screen reader what it does in these words
        let cross = self
            .win
            .widgets
            .iter()
            .find(|w| w.kind == Kind::Button && w.label == "Close window" && window.contains(w.rect.center()) && w.rect.center().y < window.min.y + 40.0)
            .map(|w| w.rect.center());
        match cross {
            Some(at) => self.click(at),
            None => panic!("the window {title:?} has no cross to close it with"),
        }
    }

    /// THE LONGEST A FRAME TOOK SINCE THE LAST STEP WAS WATCHED, in wall time; asking starts the count again.
    pub fn step_frame(&mut self) -> Duration {
        std::mem::take(&mut self.win.worst_step_frame)
    }

    /// THE WORDS ON SCREEN THAT ARE KEYS OF THE CATALOGUE, not words: a key is what the program shows when it has no
    /// string for it, and a person reads `feat-name-extrude` where a name should be.
    pub fn words_that_are_keys(&mut self) -> Vec<String> {
        let keys: std::collections::HashSet<String> = crate::i18n::reference_keys().into_iter().collect();
        self.words().into_iter().filter(|w| keys.contains(w.split('#').next().unwrap_or(w))).collect()
    }

    /// THE WORDS DRAWN WITH A LETTER THE FONT DOES NOT HAVE - a person sees a box in its place - once the screen is at
    /// rest.
    pub fn words_without_glyphs(&mut self) -> Vec<String> {
        self.settle();
        let mut out = Vec::new();
        let mut texts = Vec::new();
        fn gather(s: &egui::epaint::Shape, out: &mut Vec<std::sync::Arc<egui::Galley>>) {
            match s {
                egui::epaint::Shape::Text(t) => out.push(t.galley.clone()),
                egui::epaint::Shape::Vec(v) => v.iter().for_each(|s| gather(s, out)),
                _ => {}
            }
        }
        for shape in &self.win.shapes {
            gather(&shape.shape, &mut texts);
        }
        // A LETTER THE FONT LACKS IS DRAWN AS THE REPLACEMENT GLYPH, and so takes that glyph's cell of the atlas: the
        // cell is what tells it, not the font's own answer - which says "missing" for every letter of a family whose
        // first font also holds the replacement square.
        let mut replacement: std::collections::HashMap<String, ([u16; 2], [u16; 2])> = std::collections::HashMap::new();
        for g in texts {
            let job = &g.job;
            let mut cells = Vec::new();
            for section in &job.sections {
                let font = section.format.font_id.clone();
                let key = format!("{font:?}");
                let cell = *replacement.entry(key).or_insert_with(|| {
                    let square = self.win.ctx.fonts_mut(|f| f.layout_no_wrap("\u{25fb}".to_string(), font, egui::Color32::WHITE));
                    square.rows.first().and_then(|r| r.row.glyphs.first()).map(|g| (g.uv_rect.min, g.uv_rect.max)).unwrap_or_default()
                });
                cells.push(cell);
            }
            let lost = g
                .rows
                .iter()
                .flat_map(|r| r.row.glyphs.iter())
                .any(|glyph| !glyph.chr.is_whitespace() && !matches!(glyph.chr, '\u{25fb}' | '?') && !glyph.uv_rect.is_nothing() && cells.contains(&(glyph.uv_rect.min, glyph.uv_rect.max)));
            if lost {
                out.push(job.text.clone());
            }
        }
        out
    }

    /// WHAT WAITS FOR A CLICK WITH NO BAR OF OPTIONS SAYING SO: a tool in hand, or a pick a button started, while no
    /// bar a tool or a command puts up is on screen. `None` when there is nothing, or its bar is up.
    pub fn waiting_without_bar(&mut self) -> Option<String> {
        let bar = self.in_hand();
        let waiting = self.waiting_for_a_click();
        (bar.is_empty() && !waiting.is_empty()).then(|| waiting.join(", "))
    }

    /// A BAR OF OPTIONS WITH NOTHING IN HAND: its words, while no tool, no pick, no sketch open, no section, no pattern
    /// of components and no joint being edited stands behind it. `None` when there is no such bar.
    pub fn bar_with_nothing_in_hand(&mut self) -> Option<Vec<String>> {
        let bar = self.in_hand();
        if bar.is_empty() || !self.waiting_for_a_click().is_empty() {
            return None;
        }
        let app = &self.app;
        let behind = qymcad_ui_state::edit_si(&app.project, &app.sketch_ses).is_some() || app.side.section.plane.is_some() || app.side.carr.mode != 0 || app.side.joint.edit.is_some();
        (!behind).then_some(bar)
    }

    /// Everything that waits for a click - a tool in hand, a pick a button started - in words.
    fn waiting_for_a_click(&self) -> Vec<String> {
        let app = &self.app;
        let mut waiting = Vec::new();
        if app.tools.armed != qymcad_ui_state::Armed::None {
            waiting.push(format!("the tool {:?}", app.tools.armed));
        }
        let joint = &app.side.joint;
        for (on, what) in [
            (app.params.mirror.in_hand(), "the plane of a mirrored copy"),
            (app.side.section.pick, "the plane of a section"),
            (app.tools.picking.sketch_for().is_some(), "a sketch for a command"),
            (app.tools.picking.is_sketch_plane(), "the plane of a new sketch"),
            (app.side.m3.on, "the elements to measure"),
            (app.tools.picking.fillet_all(), "the shape whose corners are rounded"),
            (app.side.clip.geom_pending.is_some() || app.side.clip.geom_place, "the base point or the place of a copy"),
            (app.params.boolean.pick.is_some(), "the second body of a boolean"),
            (app.params.boolean.edit.is_some(), "the kind of a boolean being edited"),
            (joint.pick_faces || joint.conn_pick, "an anchor of a joint"),
            (joint.ground_pick, "a part to ground"),
            (joint.group_pick.is_some(), "the parts of a group"),
            (joint.width_pick.is_some(), "the walls of a width"),
            (joint.tangent_pick.is_some(), "the surfaces of a tangent"),
            (joint.axis_pick.is_some(), "the axis of an anchor"),
            (joint.edit_repick.is_some(), "a new anchor of a joint"),
            (joint.relation_pick.is_some(), "the joints of a relation"),
        ] {
            if on {
                waiting.push(what.to_string());
            }
        }
        waiting
    }

    /// THE LONGEST A FRAME TOOK since this was last asked, in wall time; asking starts the count again.
    pub fn worst_frame(&mut self) -> Duration {
        std::mem::take(&mut self.win.worst_frame)
    }

    /// THE TITLE OF THE WINDOW, as the program last set it.
    pub fn title(&mut self) -> String {
        self.settle();
        self.win.title.clone().unwrap_or_default()
    }

    /// The words a person reads in the program's language for the catalogue entry `key`.
    pub fn word(&self, key: &str) -> String {
        self.speak();
        crate::i18n::tr(key)
    }

    /// THE CODE OF THE LANGUAGE the program speaks (`en`, `ru`).
    pub fn language(&self) -> String {
        self.speak();
        crate::i18n::language()
    }

    /// THIS SESSION'S LANGUAGE ON THIS THREAD again: the machine's locale and the program's own setting.
    fn speak(&self) {
        qymcad_i18n::stand_for_system_locale(&self.locale);
        super::apply_language(&self.app.set);
    }

    /// THE WORDS ON SCREEN once it is at rest.
    pub fn words(&mut self) -> Vec<String> {
        self.settle();
        self.win.drawn.iter().map(|(t, _)| t.clone()).filter(|t| !t.is_empty()).collect()
    }

    /// Is `text` written anywhere on screen, whole, once it is at rest?
    pub fn shows(&mut self, text: &str) -> bool {
        self.settle();
        self.win.drawn.iter().any(|(t, _)| t == text)
    }

    /// A STEP IS DONE: the watch of the process, if one is set, looks at the session now.
    fn stepped(&mut self, what: &str) {
        let Some(watch) = WATCH.get() else { return };
        if self.watching || self.win.closed || !self.down.is_empty() {
            return;
        }
        self.watching = true;
        let problems = watch(self);
        self.watching = false;
        assert!(problems.is_empty(), "after {what}, the program is not right:\n{}", problems.join("\n"));
    }

    /// One frame with `events` in it and whatever is held, in a window that is still open.
    fn frame(&mut self, events: Vec<egui::Event>) {
        assert!(!self.win.closed, "the program has closed its window: there is nothing left to act on");
        self.speak();
        self.win.run(&mut self.app, self.held, events, false);
    }

    /// BRING WHAT `at` POINTS TO INTO THE MIDDLE PART OF THE CANVAS, as a person does: what is far off the canvas
    /// is first brought nearer by turning the wheel over the middle of it, and then the view is dragged with
    /// `buttons` and `modifiers` held, a part of the canvas at a time. A wheel turn or a drag that took the place
    /// further away is made the other way next time.
    fn bring_into_view(&mut self, at: &dyn Fn(&Session) -> Pos2, buttons: &[PointerButton], modifiers: Modifiers) {
        let (mut drag_sign, mut wheel_sign) = (1.0, -1.0);
        for _ in 0..48 {
            let canvas = self.app.viewing.view_rect;
            let inner = canvas.shrink2(canvas.size() * 0.1);
            let before = at(self);
            if inner.contains(before) {
                return;
            }
            let centre = canvas.center();
            if before.distance(centre) > canvas.size().length() * 1.5 {
                self.wheel(centre, vec2(0.0, 120.0 * wheel_sign), Modifiers::default());
                self.settle();
                if at(self).distance(centre) >= before.distance(centre) {
                    wheel_sign = -wheel_sign;
                }
                continue;
            }
            let step = ((centre - before) * drag_sign).clamp(-canvas.size() * 0.35, canvas.size() * 0.35);
            self.drag_holding(centre, centre + step, buttons, modifiers);
            self.settle();
            if at(self).distance(centre) >= before.distance(centre) {
                drag_sign = -drag_sign;
            }
        }
        panic!("the point at {:?} could not be brought into the canvas {:?}", at(self), self.app.viewing.view_rect);
    }

    /// A drag from `from` to `to` with every one of `buttons` held and `modifiers` down; no button at all is a
    /// movement with the modifiers held, as a touchpad moves the view.
    fn drag_holding(&mut self, from: Pos2, to: Pos2, buttons: &[PointerButton], modifiers: Modifiers) {
        self.go(from);
        let before = self.held;
        self.held = before | modifiers;
        for b in buttons {
            self.button(*b, true);
        }
        const STEPS: usize = 12;
        for k in 1..=STEPS {
            self.go(from + (to - from) * (k as f32 / STEPS as f32));
        }
        for b in buttons.iter().rev() {
            self.button(*b, false);
        }
        self.held = before;
    }

    /// How long a pixel of the screen is in the 3D view at `p`, and how deep `p` stands.
    fn pixel_and_depth(&self, p: [f64; 3]) -> (f64, f64) {
        let basis = self.app.viewing.cam.basis();
        let scr = qymcad_ui_state::Screen { cam: &self.app.viewing.cam, set: &self.app.set, rect: self.app.viewing.view_rect, basis: &basis };
        let (a, depth) = scr.at(p);
        let (b, _) = scr.at([p[0] + basis.0[0], p[1] + basis.0[1], p[2] + basis.0[2]]);
        (1.0 / (a.distance(b) as f64).max(1e-9), depth)
    }

    /// Every place `text` is written, whole, once the screen is at rest.
    fn places_of(&mut self, text: &str) -> Vec<Rect> {
        self.settle();
        self.win.drawn.iter().filter(|(t, _)| t == text).map(|(_, r)| *r).collect()
    }

    /// Does `hint` come up with the pointer resting over `at`? Only a hint that was not on screen before counts:
    /// the same words written elsewhere say nothing about this button.
    fn hint_comes_up(&mut self, at: Pos2, hint: &str) -> bool {
        let already = self.win.drawn.iter().any(|(t, _)| t == hint);
        self.rest_over(at);
        !already && self.win.drawn.iter().any(|(t, _)| t == hint)
    }

    /// THE POINTER COMES OVER `at` AND RESTS until a hint would be up.
    ///
    /// THREE FRAMES AFTER A SECOND OF REST, measured: the hint of the button left behind closes a frame after the
    /// pointer leaves it, egui opens no second hint while one is open, and a hint that opens is laid out unseen in
    /// its first frame and drawn in the next. The words of all three frames count.
    fn rest_over(&mut self, at: Pos2) {
        self.go(at);
        self.win.clock += 1.0;
        let mut seen: Vec<(String, Rect)> = Vec::new();
        for _ in 0..3 {
            self.frame(Vec::new());
            seen.extend(self.win.drawn.iter().cloned());
        }
        self.win.drawn = seen;
    }

    /// The pointer comes to `at`, in a frame.
    fn go(&mut self, at: Pos2) {
        self.pointer = Some(at);
        self.frame(vec![egui::Event::PointerMoved(at)]);
    }

    /// `button` goes down or up where the pointer stands, in a frame of its own. A press follows the one before
    /// by at least a second: two gestures meant apart are never one double click.
    fn button(&mut self, button: PointerButton, pressed: bool) {
        let at = self.pointer.expect("the pointer has not come over the window: move it first");
        if pressed {
            self.win.clock = self.win.clock.max(self.last_press + 1.0);
            self.last_press = self.win.clock;
        }
        self.down.retain(|b| *b != button);
        if pressed {
            self.down.push(button);
        }
        let event = Self::pressing(at, button, pressed, self.held);
        self.frame(vec![event]);
    }

    /// The event of `button` going down or up at `at`.
    fn pressing(at: Pos2, button: PointerButton, pressed: bool, modifiers: Modifiers) -> egui::Event {
        egui::Event::PointerButton { pos: at, button, pressed, modifiers }
    }

    /// Answer the chooser that is up.
    fn answer(&mut self, path: Option<std::path::PathBuf>) -> &mut Self {
        self.settle();
        let asked = crate::system::with_stand_in(|s| s.asked.take()).flatten();
        let (_, tx) = asked.expect("the program has put up no chooser to answer");
        tx.send(path).expect("the program stopped waiting for the chooser");
        self.settle();
        self.stepped("an answer to the chooser");
        self
    }

    /// WHAT KEEPS THE PROGRAM FROM REST, in words; empty when it is quiet.
    fn still_running(&self) -> Vec<String> {
        let app = &self.app;
        let mut still = Vec::new();
        if app.waiting.splash_until.is_some() {
            still.push("the greeting".to_string());
        }
        if let Some(path) = &app.disk.io.startup {
            still.push(format!("the start-up load of {path}"));
        }
        if let Some(busy) = &app.regen.busy {
            still.push(format!("{:?} '{}' for {:?}", busy.kind, busy.label, busy.started.elapsed()));
        }
        still.extend(app.regen.bg.iter().map(|b| format!("in the background {:?} '{}' for {:?}", b.kind, b.label, b.started.elapsed())));
        if app.regen.wanted {
            still.push("a rebuild asked for and not started".to_string());
        }
        // THE FONTS OF THE MACHINE ARE WALKED ON A THREAD: an open list with nothing in it is still being filled
        if app.font_cache.picker.open && app.font_cache.picker.faces.is_empty() {
            still.push("the fonts of the machine are being looked for".to_string());
        }
        // A CHOOSER WAITING FOR ITS ANSWER keeps the frames coming by design: that is rest, with a question open.
        // AN ANIMATION IS THE SAME: a degree of freedom running over its range asks for a frame every time, and
        // waiting for it to stop would mean waiting for ever - the check looks at the program while it moves.
        if self.win.repaint < Duration::from_millis(100) && !app.asking_for_a_file() && app.joint_anim.is_none() {
            still.push(format!("the screen is still moving (a frame asked again in {:?})", self.win.repaint));
        }
        still
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        crate::system::stand_in(false);
    }
}

/// THE BOX AROUND WHAT IS DRAWN IN A SKETCH, in the coordinates of the sheet: the points a person put there, and the
/// reach of the round shapes, which have no point on their rims.
fn box_of_sketch(sk: &qymcad_core::model::Sketch, system: &std::collections::HashSet<u64>) -> ([f64; 2], [f64; 2]) {
    use qymcad_core::model::EntityKind;
    let mut min = [f64::MAX, f64::MAX];
    let mut max = [f64::MIN, f64::MIN];
    let mut take = |x: f64, y: f64| {
        min = [min[0].min(x), min[1].min(y)];
        max = [max[0].max(x), max[1].max(y)];
    };
    let at = |id: u64| sk.points.iter().find(|p| p.id == id).map(|p| (p.x, p.y));
    for p in sk.points.iter().filter(|q| !system.contains(&q.id)) {
        take(p.x, p.y);
    }
    for e in &sk.entities {
        match e.kind {
            EntityKind::Circle { center, r } => {
                if let Some((cx, cy)) = at(center) {
                    take(cx - r, cy - r);
                    take(cx + r, cy + r);
                }
            }
            // AN ARC STANDS IN THE BOX OF ITS OWN SWEEP - its ends, and of the circle's four extremes only those it passes
            // through. Boxed as its whole circle, the upper half left by a trim of a circle of radius 10 read as reaching
            // down to y = -10, the half that had been cut away.
            EntityKind::Arc { center, a, b, ccw } => {
                if let (Some((cx, cy)), Some((ax, ay)), Some((bx, by))) = (at(center), at(a), at(b)) {
                    let r = (ax - cx).hypot(ay - cy);
                    take(ax, ay);
                    take(bx, by);
                    let (a0, a1) = ((ay - cy).atan2(ax - cx), (by - cy).atan2(bx - cx));
                    let sweep = if ccw { (a1 - a0).rem_euclid(std::f64::consts::TAU) } else { (a0 - a1).rem_euclid(std::f64::consts::TAU) };
                    for k in 0..4 {
                        let t = k as f64 * std::f64::consts::FRAC_PI_2;
                        let from = if ccw { (t - a0).rem_euclid(std::f64::consts::TAU) } else { (a0 - t).rem_euclid(std::f64::consts::TAU) };
                        if from <= sweep {
                            take(cx + r * t.cos(), cy + r * t.sin());
                        }
                    }
                }
            }
            EntityKind::Ellipse { c, ma, mi } => {
                if let (Some((cx, cy)), Some((mx, my)), Some((nx, ny))) = (at(c), at(ma), at(mi)) {
                    let (a, b) = ((mx - cx).hypot(my - cy), (nx - cx).hypot(ny - cy));
                    take(cx - a.max(b), cy - a.max(b));
                    take(cx + a.max(b), cy + a.max(b));
                }
            }
            EntityKind::Line { .. } => {}
        }
    }
    if min[0] == f64::MAX {
        return ([0.0, 0.0], [0.0, 0.0]);
    }
    (min, max)
}

/// The distance between two points of space.
fn dist(a: [f64; 3], b: [f64; 3]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}
