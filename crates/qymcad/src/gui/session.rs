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
    /// How many vertices of its mesh carry the surface's own normal - what its smooth light is shaded by. None: a mesh
    /// saved before the normals were kept, or one with no surface behind it.
    pub lit_by_the_surface: usize,
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

    /// The colour of the picture at a point of the screen.
    pub fn colour_at(&self, at: Pos2) -> [u8; 3] {
        let (x, y) = (at.x.round() as usize, at.y.round() as usize);
        let o = (y * self.width + x) * 4;
        [self.rgba[o], self.rgba[o + 1], self.rgba[o + 2]]
    }

    /// How far apart two colours are, the largest of the three channels.
    pub fn apart(a: [u8; 3], b: [u8; 3]) -> i32 {
        (0..3).map(|k| (a[k] as i32 - b[k] as i32).abs()).max().unwrap_or(0)
    }
}

/// THE GLYPH OF A SNAP drawn at the pointer while a sketch tool is in hand: the tie a click there is to lay (its name, as
/// the glyph is named), and where on the sketch it stands.
#[derive(Clone, Debug, PartialEq)]
pub struct SnapGlyph {
    pub glyph: String,
    pub at: (f64, f64),
}

/// A BADGE OF A CONSTRAINT drawn on the sketch: its glyph, as the glyph is named, and where on the screen it stands.
#[derive(Clone, Debug, PartialEq)]
pub struct Badge {
    pub glyph: String,
    pub at: Pos2,
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

impl Session {}

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

mod life;
mod pointer;
mod keys;
mod finding;
mod space;
mod reading;
mod frames;
