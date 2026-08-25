//! The egui interface: the operation tree, the tool library, the 2D backplot, the projects.
//!
//! The model is `qymcad_core::model::Project`. A 2D top view (an egui `Painter`) with panning, zooming and
//! contour selection by click.

use eframe::egui;
use egui::{Color32, Pos2, Rect, Sense, Stroke, Vec2};
use egui_phosphor::regular as ph;

use qymcad_kernel::OcctKernel; // THE ONLY kernel implementation, shared with the repro harness
use qymcad_core::geom::{Contour, MeshFace, Point2};


use qymcad_core::ir::{CoolantMode, DrillKind, Move, Program, SpindleDir};
use qymcad_core::model::{Id, Machine, OpKind, OperationDef, PostKind, Project, SideMode};
use qymcad_core::ops::{Ramp, Tabs};
use qymcad_core::tool::{Tool, ToolType};
use qymcad_io::{import_dxf, import_stl, import_svg};
use qymcad_cam::{post_for, PostOptions};
use qymcad_verify::{verify_gcode, VerifyOptions, VerifyResult};

/// THE APPLICATION'S NAME for eframe. It is also the key of the directory eframe puts the settings file into, so
/// it lives as one constant: were those two places to diverge, the settings window would show the path of a
/// directory that does not exist.
pub(crate) const APP_ID: &str = "qymcad";

/// THE CURRENT TIME IN ISO-8601 (UTC, to the second).
///
/// By its own arithmetic, without a date crate: exactly one string is needed in the whole project, and a
/// dependency for it would have to be carried, updated and explained. The format chosen is machine-readable and
/// sortable - whoever wants to show it to a person may, but what is stored must be unambiguous.
pub(crate) fn now_iso8601() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    iso8601_from_unix(secs)
}

/// CONVERTING unix SECONDS INTO ISO-8601. Separate from "what time is it", because only this part is testable:
/// the current time has nothing to be compared against in a test, while a known stamp has.
pub(crate) fn iso8601_from_unix(secs: u64) -> String {
    let (days, rem) = ((secs / 86_400) as i64, secs % 86_400);
    let (h, mi, sec) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    // the calendar runs from 1970-01-01, civil Gregorian, with no time zones
    let (mut y, mut d) = (1970i64, days);
    loop {
        let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
        let len = if leap { 366 } else { 365 };
        if d < len {
            break;
        }
        d -= len;
        y += 1;
    }
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let ml = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut m = 0usize;
    while m < 12 && d >= ml[m] {
        d -= ml[m];
        m += 1;
    }
    format!("{y:04}-{:02}-{:02}T{h:02}:{mi:02}:{sec:02}Z", m + 1, d + 1)
}

pub(crate) fn settings_dir() -> Option<std::path::PathBuf> {
    eframe::storage_dir(APP_ID)
}

/// WHAT OPENS A FOLDER - as a separate PURE function rather than a line inside a click handler.
///
/// Every system has its own file manager, and there is no other way to check this: a real process must not be
/// started in a test, and "it works on my machine" proves nothing about the other two. The function answers WHAT
/// will be launched; a test compares that answer for all three systems, and the click simply carries it out.
pub(crate) fn reveal_command(os: egui::os::OperatingSystem, dir: &std::path::Path) -> (&'static str, Vec<String>) {
    open_command(os, dir.to_string_lossy().into_owned())
}

/// WHAT OPENS AN ADDRESS IN A BROWSER. The same way as for a folder, and that is no coincidence: on all three
/// systems "open this with whatever is proper" is one and the same command, which works it out for itself.
pub(crate) fn browse_command(os: egui::os::OperatingSystem, url: &str) -> (&'static str, Vec<String>) {
    open_command(os, url.to_string())
}

fn open_command(os: egui::os::OperatingSystem, arg: String) -> (&'static str, Vec<String>) {
    use egui::os::OperatingSystem as OS;
    match os {
        OS::Windows => ("explorer", vec![arg]),
        OS::Mac => ("open", vec![arg]),
        // Linux, BSD and everything else: `xdg-open` is the shared way across desktop environments
        _ => ("xdg-open", vec![arg]),
    }
}

pub fn launch() -> eframe::Result<()> {
    // EVERY SAVED FILE SAYS WHAT WROTE IT. The core is a library and has no build of its own to name, so
    // the application tells it once here, before anything can save. Placed before the window opens: an
    // autosave can fire on the very first seconds of a session.
    qymcad_core::model::set_producer(&crate::build_info::report_block().lines().next().unwrap_or_default());
    // AND THE CRASH STOPS DISAPPEARING. Installed before anything can panic, for the same reason.
    crate::crash::install();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1280.0, 800.0]),
        // the GPU viewport: egui on wgpu gives access to wgpu_render_state for the bodies' 3D paint callback.
        // glow stays compiled in as a fallback (the renderer is switched here).
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    eframe::run_native(
        APP_ID,
        options,
        Box::new(|cc| {
            // the Phosphor icon font (without it the icon glyphs render as empty boxes)
            install_fonts(&cc.egui_ctx);

            let mut app = App::default();
            // A CRASH FILE NOBODY IS TOLD ABOUT IS THE SAME AS NO CRASH FILE. The newest report from an
            // earlier run is picked up here and shown once.
            app.crash_report = crate::crash::unseen_reports().into_iter().next();
            // restore the previous session's settings (the machine, the view preferences)
            if let Some(storage) = cc.storage {
                if let Some(m) = eframe::get_value::<qymcad_core::model::Machine>(storage, "machine") {
                    app.project.machine = m;
                }
                // THE SETTINGS COME AS ONE RECORD: loaded and assigned. There is no separate "apply" step, and
                // there is nowhere to lose a setting along the way - except the theme, which egui does not remember itself.
                if let Some(v) = eframe::get_value::<Settings>(storage, "settings") {
                    app.adopt_settings(v, &cc.egui_ctx);
                } else {
                    app.adopt_settings(app.set.clone(), &cc.egui_ctx); // the factory defaults: the language from the locale, the theme, the scale
                }
            if let Some(lib) = eframe::get_value::<Vec<qymcad_core::model::Machine>>(storage, "machine_lib") {
                    if !lib.is_empty() {
                        app.cam_job.machines = lib;
                    }
                }
                // the previous session's project is reopened LAZILY: the splash with the logo is shown first (the
                // opening frames), and only then does the loading start, so a heavy STEP reparse does not hold an
                // empty window.
                if let Some(Some(path)) = eframe::get_value::<Option<String>>(storage, "last_project") {
                    if std::path::Path::new(&path).is_file() {
                        app.io.startup = Some(path); // loaded asynchronously on the first frame (the splash with the spinner)
                    }
                }
            }
            // The start is ALWAYS isometric: 3D mode with an isometric camera (init=false means it frames itself to
            // the contents on the first frame while keeping the orientation). Independent of the previous session.
            app.mode_3d = true;
            app.cam = Cam3::default();
            // the GPU viewport: when the wgpu backend is active, the bodies pass's GPU resources are installed.
            // Otherwise (on the glow fallback) the CPU raster remains.
            if let Some(rs) = &cc.wgpu_render_state {
                // ANTIALIASING COMES BEFORE THE PIPELINES ARE CREATED: they bake the sample count into themselves,
                // so the setting takes effect on a restart (which is what the window says).
                crate::viewport_gpu::set_msaa(app.set.msaa);
                app.gpu_ok = crate::viewport_gpu::install(rs);
                // WHICH ADAPTER IS DRAWING - half the complaints about a viewport are answered by this line
                // and by nothing else, and it cannot be guessed from a screenshot.
                let i = rs.adapter.get_info();
                crate::diagnostics::note_gpu(format!("wgpu {:?}, {} ({:?}), driver {} {}", i.backend, i.name, i.device_type, i.driver, i.driver_info));
            } else {
                // The glow fallback: no adapter to ask, but WHICH PATH is drawing is itself the answer to
                // "the viewport is slow" and "the viewport is black".
                crate::diagnostics::note_gpu("glow (the CPU fallback path)".into());
            }
            Ok(Box::new(app))
        }),
    )
}

#[derive(Clone, Copy)]
struct View2d {
    center: Vec2,
    scale: f32,
    initialized: bool,
}

impl Default for View2d {
    fn default() -> Self {
        Self { center: Vec2::ZERO, scale: 4.0, initialized: false }
    }
}

/// A CATALOGUE KEY WRAPPED SO THAT IT CANNOT BE DRAWN.
///
/// Reported behaviour: the thread popup showed `f-nominal-d`, `f-pitch-std` and `f-length` in place of the labels -
/// keys instead of words. The cause was exactly one character: the field was drawn as `ui.label(p.label)` instead
/// of `p.label()`. The key was a plain `&'static str`, and `ui.label` accepts a `&str` - the compiler said nothing
/// and the internal name went to the screen.
///
/// The wrapper does not implement `Into<WidgetText>`, so `ui.label(p.label)` no longer compiles at all. This is not
/// decoration: a translation that rests on attentiveness at every edit ends exactly the way it ended here.
#[derive(Clone, Copy)]
pub(crate) struct LabelKey(&'static str);

/// One editable dimension of the active Part command: the label + the key (for `feat_dims`) + the current value +
/// the input buffer (a number OR an expression like `w/2+3`). An on-screen field like the sketch dimensions have.
#[derive(Clone)]
pub(crate) struct CmdParam {
    /// A CATALOGUE KEY rather than a finished phrase: the field's label must follow a change of language by itself,
    /// without the command being recreated. The key is stable; the translation is taken at the moment of display.
    label: LabelKey,
    /// The dimension's key. A STRING rather than a `&'static str`: a variable fillet has as many fields as there
    /// are vertices in the table, and each has a key of its own of the form `at{vertex descriptor}`. That cannot be
    /// expressed as a list of constants, and a second field mechanism for one tool would fork into copies what
    /// already works: the popup, the expressions, the parametrics, Enter and Esc.
    key: String,
    val: f64,
    txt: String,
    lo: f64,
    hi: f64,
    /// WHERE THIS FIELD LIVES IN SPACE. `None` means the command's shared popup; a point means a small window of
    /// its own at that place (a radius at a vertex is shown AT THE VERTEX, otherwise six identical fields in a
    /// column cannot be told apart).
    at: Option<[f64; 3]>,
}
impl CmdParam {
    /// The field's label in the reader's language.
    pub(crate) fn label(&self) -> String {
        crate::i18n::tr(self.label.0)
    }

    /// Change the label on the fly (on a chamfer the second leg becomes an angle when the mode changes).
    pub(crate) fn set_label(&mut self, key: &'static str) {
        self.label = LabelKey(key);
    }

    fn new(label: &'static str, key: &str, val: f64, lo: f64, hi: f64) -> Self {
        Self { label: LabelKey(label), key: key.to_string(), val, txt: format!("{val:.2}"), lo, hi, at: None }
    }

    /// A field AT THE GEOMETRY: the same thing, but with a place of its own in space.
    fn at(mut self, p: [f64; 3]) -> Self {
        self.at = Some(p);
        self
    }
}

/// The mode for gathering geometry into an operation: press Add, then click a part or a contour in the viewport
/// and it is added to the active operation.
#[derive(Clone, Copy, PartialEq)]
enum OpPick {
    Body,
    Contour,
}

/// A READY PIECE OF THE SCENE BUFFER FOR ONE BODY.
///
/// THE KEY IS SPLIT IN TWO, AND THAT IS THE WHOLE POINT. `shape` is about the body's FORM and appearance (the
/// mesh, the highlight, the ghost state, the colour, the display settings, the cutting plane); `at` is where the
/// body stands. The position used to be part of one shared key, and MOVING a part declared the block stale
/// entirely: a drag rebuilt 63 blocks out of 138 (the driven side was a gantry subassembly), and rebuilding a
/// block means clipping every triangle by the section plane, world normals and three colours per triangle.
///
/// A move changes NEITHER THE FORM NOR THE COLOUR. If the part is oriented as before and merely shifted, the
/// ready vertices need only have the difference added - one addition per vertex instead of the whole
/// manufacture. A rotation and a section cannot be carried over that way (the colour is computed from the WORLD
/// normal and the section cuts in world space), so there the block is built anew.
struct SceneBlock {
    /// The key of the FORM and appearance - everything except the position.
    shape: u64,
    /// The position these vertices already stand at.
    at: [f64; 12],
    opaque: Vec<crate::viewport_gpu::GpuVert>,
    transp: Vec<crate::viewport_gpu::GpuVert>,
}

/// A workbench. It switches the left tool bar and the set of panels. Sketch, Part and Assembly are the model;
/// Machining is CAM.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Workbench {
    Sketch,
    Part,
    Assembly,
    Cam,
}

impl Workbench {
    /// The workbench's code for linking to the help - not its label: the label is translated, and a link must not
    /// depend on the language.
    fn code(self) -> &'static str {
        match self {
            Workbench::Sketch => "sketch",
            Workbench::Part => "part",
            Workbench::Assembly => "assembly",
            Workbench::Cam => "cam",
        }
    }

    fn is_cam(self) -> bool {
        self == Workbench::Cam
    }
}

/// The extent of an extrude or a cut (in place of the magic numbers 0 to 3). The discriminants are kept as the
/// former u8 in case of outside places, but the comparisons go by variant.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum ExtentMode {
    #[default]
    Length, // to a length (one side)
    Symmetric,
    TwoSided, // two sides (the "down" field is active)
    Through,  // through all (operations on a body only)
}

impl ExtentMode {
    fn symmetric(self) -> bool {
        matches!(self, ExtentMode::Symmetric)
    }
    fn two_sided(self) -> bool {
        matches!(self, ExtentMode::TwoSided)
    }
    fn through(self) -> bool {
        matches!(self, ExtentMode::Through)
    }
    /// Restore the mode from a saved feature's flags (through takes priority, then symmetry, then two-sided).
    fn from_extent(reach: qymcad_core::feature::Reach, down: f64, through: bool) -> Self {
        if through {
            ExtentMode::Through
        } else if reach == qymcad_core::feature::Reach::BothWays {
            ExtentMode::Symmetric
        } else if down.abs() > 1e-9 {
            ExtentMode::TwoSided
        } else {
            ExtentMode::Length
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Sel {
    None,
    Machine,
    Stock,
    Mesh(usize),
    /// A part's face: (the mesh index, the face index).
    Face(usize, usize),
    /// A contour object (an index into project.contours).
    Contour(usize),
    /// A sketch node (an index into project.sketches).
    Sketch(usize),
    /// A work plane (an index into project.planes).
    Plane(usize),
    /// A datum point (an index into project.datum_points).
    DatumPoint(usize),
    /// A datum axis (an index into project.datum_axes).
    DatumAxis(usize),
    Tool(usize),
    Op(usize),
    /// A setup (an index into project.setups).
    Setup(usize),
    /// A feature of the build tree (a node index into project.timeline).
    Feature(usize),
    /// A component or part (an index into project.components).
    Component(usize),
    /// A mate (by the joint's Id) - selecting the list row and the 3D glyph together, both ways.
    Joint(Id),
}

/// The TREE clipboard: what was copied or cut (by stable Id) and which of the two it was.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum TreeClip {
    /// A sketch node (by the sketch's Id).
    Sketch { sid: Id, cut: bool },
    /// A component - a Part or a subassembly (by the component's Id).
    Component { id: Id, cut: bool },
}

/// A reference for the dimension tool: a point or a straight line (a line or an axis). A dimension between two
/// references: point to point, point to line (a perpendicular), line to a parallel line, or to an axis.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum DimRef {
    Point(Id),
    Line(Id, Id),
}

/// A base edge for building tangents (a tangent arc or a tangent circle): either a straight line (with ends a
/// and b) or a circle or arc (a centre plus a radius).
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum EdgeRef {
    Line { a: Id, b: Id },
    Circle { center: Id, r: f64 },
}

/// The sketch anchor under the cursor - for dimensions and constraints between ANY geometry.
#[derive(Clone, Copy)]
enum SketchRef {
    /// An existing point (a circle's or an arc's centre included).
    Point(Id),
    /// The midpoint of segment (a,b) - materialised as a point with a Midpoint constraint.
    Midpoint(Id, Id),
    /// The origin (0,0) - materialised as a Fixed reference point.
    Origin,
}

/// A navigation that could throw away unsaved changes: it happens at once (when there are no edits) or after the
/// "save the changes?" dialogue has been answered.
#[derive(Clone)]
pub(crate) enum Nav {
    /// A new PART document: a root assembly plus one empty part, with the part active.
    New,
    /// A new ASSEMBLY document: the root only, and it is active. An empty part in an assembly document is litter
    /// that would have to be deleted by hand.
    NewAssembly,
    /// A new document FROM A TEMPLATE. It goes through the same guard against unsaved edits that Open does.
    NewFromTemplate(String),
    OpenDialog,
    /// Open A SPECIFIC path (the recent files, the start screen). It goes through the same guard against unsaved
    /// edits that Open does: otherwise a click on a recent file would silently lose the work.
    OpenPath(String),
    Exit,
}

/// What to export in 3D (STEP or STL): the selected component (with every body nested in its subtree) or the whole
/// project. In both cases ONLY the visible bodies are exported.
#[derive(Clone, Copy)]
enum ExportTarget {
    Component(Id),
    Project,
}

/// The parse of the export's target bodies by [`qymcad_core::model::ExportKind`] - ONE parse for both formats.
/// STEP writes only `brep`; STL writes EVERYTHING that is on the screen (`brep` by tessellation plus the
/// `mesh_only` and `stale` meshes). The difference must not be kept quiet: [`ExportPlan::note`] gives it in words,
/// meaning the same thing in both statuses.
#[derive(Default)]
struct ExportPlan {
    /// A live B-rep - the exact geometry.
    brep: Vec<Id>,
    /// An STL import: there never was a B-rep.
    mesh_only: Vec<Id>,
    /// A failed rebuild: the last good geometry is on the screen and there is no B-rep.
    stale: Vec<Id>,
}

impl ExportPlan {
    /// The bodies for STL: everything visible (a B-rep is re-tessellated, the rest go as their stored mesh).
    fn stl_bodies(&self) -> Vec<Id> {
        self.brep.iter().chain(&self.mesh_only).chain(&self.stale).copied().collect()
    }
    /// An honest note appended to the status: what exactly is missing and why. Empty means everything went out
    /// intact. `step=true` gives the wording for STEP (those bodies did NOT reach the file), otherwise for STL
    /// (they did, but as a mesh).
    fn note(&self, step: bool) -> String {
        let (m, s) = (self.mesh_only.len(), self.stale.len());
        if m + s == 0 {
            return String::new();
        }
        let mut what = Vec::new();
        if m > 0 {
            what.push(crate::i18n::tr1("note-mesh-bodies", "n", &m.to_string()));
        }
        if s > 0 {
            what.push(crate::i18n::tr1("note-failed-bodies", "n", &s.to_string()));
        }
        let what = what.join(", ");
        if step {
            crate::i18n::tr1("note-not-exported", "what", &what)
        } else {
            crate::i18n::tr1("note-as-mesh", "what", &what)
        }
    }
}

/// The target of an inline rename of any tree node (sharing the `rename_buf` buffer).
/// The names live in different places: a component's in `components[].name`; a datum's in `planes`,
/// `datum_points` or `datum_axes[].name`; a body's through `set_mesh_name(mi)` (by mesh index). Sketches and
/// features have fields of their own (rename_sketch and rename_target).
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum RenameNode {
    Component(Id),
    Plane(Id),
    DatumPoint(Id),
    DatumAxis(Id),
    Body(usize),
}

/// The result of a background (worker thread) import or export, arriving at the interface over a channel.
enum JobResult {
    /// A STEP was imported: the bodies (mesh plus B-rep faces) + the solids' live shapes + the file's path.
    StepImported { path: String, bodies: Vec<qymcad_kernel::Body>, shapes: Vec<qymcad_kernel::Shape> },
    /// An STL was imported: the mesh plus the detected faces (the detection runs in the worker too - it is heavy on large meshes).
    StlImported { path: String, mesh: qymcad_core::geom::Mesh, faces: Vec<qymcad_core::geom::MeshFace> },
    /// An export finished: the status plus the shapes moved into the worker (to be returned to the self.live.shapes cache).
    Exported { status: String, shapes_back: Vec<(Id, qymcad_kernel::Shape)> },
    /// A project was loaded (in a thread): the parsed timeline plus the faces. The bodies' geometry comes from the
    /// bundle rather than being rebuilt, so the window shows the model at once.
    ProjectLoaded { path: String, project: Box<Project>, shapes: Vec<(Id, qymcad_kernel::Shape)> },
    /// A REBUILD of the timeline carried out in a worker thread. Heavy operations (a thread takes seconds on a
    /// boolean) no longer hold the interface thread: the window draws a spinner, and the system does not consider
    /// the program hung nor offer to kill it.
    Regenerated {
        /// THE DOCUMENT'S FINGERPRINT AT THE MOMENT OF THE START. The result was computed from A COPY and replaces
        /// the document ENTIRELY: if the live document moved on in the meantime, applying such a result means
        /// silently erasing an edit. The fingerprint makes that noticeable rather than relying on there being
        /// nowhere for an edit to come from.
        stamp: u64,
        project: Box<Project>,
        shapes: Vec<(Id, qymcad_kernel::Shape)>,
        built: Vec<(Id, Vec<MeshFace>)>,
        errors: Vec<(Id, qymcad_core::errors::CoreError)>,
        /// THE REBUILD WAS STOPPED BY A PERSON. The result is incomplete by construction and must not be applied -
        /// it is thrown away whole and the document stays what it was.
        cancelled: bool,
    },
    /// The imported solids' B-rep, restored from the embedded STEP. `regen` says whether a full rebuild is needed
    /// afterwards (an older file with no saved geometry); otherwise this is a BACKGROUND top-up (the model is
    /// already on the screen and all that is awaited is that operations on the imports become available).
    ImportShapes { shapes: Vec<(Id, qymcad_kernel::Shape)>, regen: bool },
    /// The background write of the project finished (`error` is None on success).
    Saved { path: String, autosave: bool, error: Option<String> },
    /// An error - its text goes to the status line.
    Failed(String),
}

/// THE CONTROL PANEL OF A LONG REBUILD: how much is done, and the request to stop.
///
/// One record for both sides: the thread writes the progress and reads the request, the window reads the progress
/// and writes the request. Two different mechanisms ("progress over a channel, cancellation by a flag") would give
/// two different moments of truth about one piece of work.
#[derive(Default)]
pub(crate) struct RegenPulse {
    /// The node being computed right now, and the total number of nodes.
    done: std::sync::atomic::AtomicUsize,
    total: std::sync::atomic::AtomicUsize,
    stop: std::sync::atomic::AtomicBool,
}

impl RegenPulse {
    fn progress(&self) -> (usize, usize) {
        use std::sync::atomic::Ordering::Relaxed;
        (self.done.load(Relaxed), self.total.load(Relaxed))
    }
    fn ask_stop(&self) {
        self.stop.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    fn stop_asked(&self) -> bool {
        self.stop.load(std::sync::atomic::Ordering::Relaxed)
    }
}

impl qymcad_core::feature::RegenWatch for RegenPulse {
    fn step(&self, done: usize, total: usize) -> bool {
        use std::sync::atomic::Ordering::Relaxed;
        self.done.store(done, Relaxed);
        self.total.store(total, Relaxed);
        !self.stop_asked()
    }
}

struct Busy {
    label: String,
    /// The rebuild is quiet: the work is small, no window is shown - only the status line.
    quiet: bool,
    rx: std::sync::mpsc::Receiver<JobResult>,
    /// Only a rebuild has this: there is nothing to interrupt the other tasks with (a file write stopped halfway
    /// is a broken file, and parsing a STEP is all or nothing).
    pulse: Option<std::sync::Arc<RegenPulse>>,
    /// The kind of background task - it shows whether another one of the same kind may be queued (two writes of
    /// one file at once are a race between the temporary file and the rename) and what exactly to wait for on exit.
    kind: BgKind,
}

/// The kinds of background work. There used to be a SINGLE slot, and Save during the restoration of the imports'
/// B-rep overwrote its receiver: the thread finished computing and sent the shapes into a closed channel, so the
/// B-rep did not appear until a restart.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum BgKind {
    /// Writing the project to disk (by hand or by autosave).
    Save,
    /// Restoring the imported solids' B-rep from the embedded STEP.
    ImportShapes,
    /// A rebuild of the timeline by the kernel - modal (edits are forbidden while it runs).
    Regen,
}

/// Restore the B-rep shapes of the imported STEP solids from the embedded sources (`sources/`): parse each
/// source's embedded STEP once and lay its solids out across the bodies.
/// A free function rather than a method: it is called from a worker thread while a project loads, without
/// `&mut App`. The imported bodies are stripped on saving as outputs of the timeline; their geometry lives in the
/// original file.
fn restore_import_shapes_for(project: &Project) -> Vec<(Id, qymcad_kernel::Shape)> {
    use qymcad_core::feature::FeatureKind;
    use std::collections::HashMap;
    let imports: Vec<(Id, Id, u32)> = project
        .timeline
        .iter()
        .filter_map(|n| match n.kind {
            FeatureKind::Import { body, source, solid } => Some((body, source, solid)),
            _ => None,
        })
        .collect();
    let mut out = Vec::new();
    if imports.is_empty() {
        return out;
    }
    // group by source, so that each STEP is unpacked only once
    let mut by_src: HashMap<Id, Vec<(Id, u32)>> = HashMap::new();
    for (body, src, solid) in imports {
        by_src.entry(src).or_default().push((body, solid));
    }
    for (src, items) in by_src {
        let Some(sf) = project.sources.iter().find(|s| s.id == src) else { continue };
        if sf.data.is_empty() {
            continue;
        }
        let ext = if sf.ext.is_empty() { "step" } else { sf.ext.as_str() };
        let tmp = std::env::temp_dir().join(format!("qym_import_{src}.{ext}"));
        if std::fs::write(&tmp, &sf.data).is_err() {
            continue;
        }
        // the Option wrapper: a shape is moved (it is not Clone), and each solid is taken by index exactly once
        let mut shapes: Vec<Option<qymcad_kernel::Shape>> = qymcad_kernel::step_solids(tmp.to_string_lossy().as_ref()).unwrap_or_default().into_iter().map(Some).collect();
        let _ = std::fs::remove_file(&tmp);
        for (body, solid) in items {
            if let Some(s) = shapes.get_mut(solid as usize).and_then(|o| o.take()) {
                out.push((body, s));
            }
        }
    }
    out
}

/// DERIVED STATE, NOT TRUTH: everything here can be thrown away, and the next frame rebuilds it.
///
/// Fourteen caches used to sit among the fields of `App` looking exactly like the document itself, and the
/// difference matters: losing one of these costs a frame, losing a field of the document costs the part.
///
/// Each is keyed by the revision it was computed at, so it answers "still good?" by comparing numbers rather
/// than by somebody remembering to clear it. THE EMPTY KEY IS `u64::MAX`, NOT ZERO: zero is a real revision,
/// and a cache that starts at it claims to hold the first build of the document before anything is built.
pub(crate) struct Caches {
    /// THE TEXTURE CACHE OF THE ViewCube'S LABELS (keyed by text and size). Rasterising the font every frame means
    /// thousands of glyphs a second for nothing; a texture is prepared once and lives until the language or the
    /// cube's size changes, and both of those change the key.
    label_tex: std::cell::RefCell<std::collections::HashMap<String, egui::TextureHandle>>,
    /// THE SCENE BUFFER'S BLOCKS BY BODY - one per body, see `SceneBlock`.
    ///
    /// Dragging a part moves ONE body, while the buffer was assembled whole: 463,878 vertices per frame, 30 to 48
    /// ms on a real assembly. A block depends on the body's FORM, its highlight and the shared display settings;
    /// the position lives separately and a move does not invalidate the block.
    scene_blocks: std::cell::RefCell<std::collections::HashMap<usize, SceneBlock>>,
    /// WHAT BECAME OF THE BLOCKS ON THE LAST SCENE PASS: [rebuilt, shifted, taken ready-made].
    ///
    /// Not decoration but the only honest answer to "why is this frame expensive": without it an optimisation is
    /// verified with a stopwatch, and a stopwatch is off by whole multiples on a debug build. The
    /// `a_moved_part_does_not_rebuild_its_block` check asks for exactly these numbers.
    scene_stats: std::cell::Cell<[u32; 3]>,
    /// The 3D render's cache: (the view key, the texture). It is redrawn only when the view changes.
    view: std::cell::RefCell<Option<(u64, egui::TextureHandle)>>,
    /// The GPU viewport is on (a switch; the CPU raster is the fallback).
    /// A perspective projection instead of an orthographic one. It is applied by one formula in `project3`, so the
    /// overlays and both render paths agree. Orthographic by default, as is customary in CAD.
    /// Smooth (Gouraud) shading instead of flat. When true, the colour is computed at EVERY vertex from the
    /// smoothed normal (`Mesh::vertex_normals`) and interpolated across the triangle; sharp edges stay sharp (the
    /// mesh topology is split by face). False gives flat shading (the face normal).
    /// The cache of smoothed vertex normals, parallel to `project.meshes`, plus the `geom_rev` it was built at. It
    /// is recomputed only when the geometry changes, not every frame. The normals are local (before the world transform).
    norm: std::cell::RefCell<(u64, Vec<Vec<[f64; 3]>>)>,
    /// The key of the scene uploaded into the GPU vertex buffer (re-uploaded only on a change). It does NOT depend on the camera.
    gpu_scene_key: std::cell::Cell<u64>,
    /// The cache of the visible scene's world bounding sphere (a centre and a radius) keyed by the scene - for
    /// tight near and far planes in perspective (the z buffer's precision). Recomputed only when the scene changes,
    /// not every frame.
    bounds: std::cell::Cell<(u64, Option<([f64; 3], f64)>)>,
    /// THE CACHE OF CONSUMED BODIES: `(the revision, the bodies)`. The set is computed by a pass over the whole
    /// timeline while being asked for on every body on every frame - recomputing it is out of the question.
    consumed: std::cell::RefCell<(u64, std::collections::HashSet<Id>)>,
    /// THE LIST OF VISIBLE BODIES: `(the revision, the context, [(the mesh index, the body id)])`.
    ///
    /// For each body `body_shown` finds its owner by a LINEAR pass over the whole timeline and walks the visibility
    /// tree. In the pick loop that happens for EVERY body on EVERY frame: at 120 bodies the measurement gave 67 ms
    /// per frame, which is an application that does not respond. The list is computed once per rebuild and per
    /// change of context.
    shown_bodies: std::cell::RefCell<(u64, Id, Vec<(usize, Id)>)>,
    /// THE CACHE OF THE BODIES' WORLD EXTENTS: `(the revision, a body mapped to its 8 world corners)`.
    ///
    /// Rejecting by "the cursor is outside the extent" did not help by itself, because what was expensive was
    /// REACHING the body: finding the mesh by id and computing its display transform was done for every body on
    /// every frame. The measurement showed 0.56 ms per body - on an assembly that is seconds. The corners are
    /// computed once per rebuild.
    bbox_world: std::cell::RefCell<(u64, std::collections::HashMap<Id, [[f64; 3]; 8]>)>,
    /// THE CACHE OF THE BODIES' EDGES FOR PICKING: `(the geometry revision, a body mapped to (polylines, ids))`.
    ///
    /// Without it, picking an edge or a vertex anchor pulled the edges of EVERY body out of the kernel on EVERY
    /// mouse movement. On a couple of cubes that goes unnoticed; on an assembly of a thousand components the
    /// application stops responding. The key is the geometry revision: once the model is rebuilt, the cache throws
    /// itself away.
    pick_edges: std::cell::RefCell<(u64, std::collections::HashMap<Id, std::rc::Rc<(Vec<Vec<[f32; 3]>>, Vec<u32>)>>)>,
    /// The cache of the section CAPS: (a key of plane plus geom_rev plus visibility, the caps in WORLD coordinates).
    section_caps: std::cell::RefCell<(u64, std::rc::Rc<Vec<qymcad_core::geom::Mesh>>)>,
    /// The meshes' world extents by index plus the `geom_rev` they were computed at. A cheap way of rejecting
    /// bodies for the section caps (a Common boolean is expensive, and the plane cuts a handful of bodies out of a
    /// thousand).
    mesh_bounds: std::cell::RefCell<(u64, std::collections::HashMap<usize, ([f64; 3], [f64; 3])>)>,
    /// The sketch diagnostics cache. The key is a fingerprint of the geometry and the constraints. The rank
    /// analysis builds a FULL Jacobian and runs Gaussian elimination (O(m * nv^2)) - without a cache that was
    /// computed EVERY FRAME several times over (the panel, the list, the overlay, the glyphs, the tree), and on
    /// large sketches the interface hung. It is recomputed only on an edit.
    /// A `RefCell`, because drawing goes through `&self` and needs that very cache.
    sk_status: std::cell::RefCell<Option<(usize, u64, SketchDiag)>>,
}

impl Default for Caches {
    fn default() -> Self {
        Self {
            label_tex: std::cell::RefCell::new(std::collections::HashMap::new()),
            scene_blocks: Default::default(),
            scene_stats: Default::default(),
            view: std::cell::RefCell::new(None),
            norm: std::cell::RefCell::new((u64::MAX, Vec::new())),
            gpu_scene_key: std::cell::Cell::new(u64::MAX),
            bounds: std::cell::Cell::new((u64::MAX, None)),
            consumed: std::cell::RefCell::new((u64::MAX, std::collections::HashSet::new())),
            shown_bodies: std::cell::RefCell::new((u64::MAX, 0, Vec::new())),
            bbox_world: std::cell::RefCell::new((u64::MAX, std::collections::HashMap::new())),
            pick_edges: std::cell::RefCell::new((u64::MAX, std::collections::HashMap::new())),
            sk_status: std::cell::RefCell::new(None),
            section_caps: std::cell::RefCell::new((0, std::rc::Rc::new(Vec::new()))),
            mesh_bounds: std::cell::RefCell::new((0, std::collections::HashMap::new())),
        }
    }
}

/// WHAT IS OPEN ON SCREEN: the windows, the dialogues and the little state each of them keeps while open.
///
/// Two dozen `show_*` flags used to lie among the fields of the document, and a reader could not tell at a
/// glance which of them was part of the part and which was merely a window somebody had opened. They are all
/// the same kind of thing - "this is on screen right now" - and none of them belongs in a saved file.
#[derive(Default)]
pub(crate) struct Windows {
    /// The help window: whether it is open and which article is shown.
    help: bool,
    help_article: String,
    /// Where Back leads and what has been typed into the help's search.
    help_back: Vec<String>,
    help_query: String,
    /// Whether the "save as a template" dialogue is open, and the name typed into it.
    save_template: bool,
    tpl_name: String,
    /// The modal About window (Help -> About).
    /// THE START SCREEN is visible. It comes up at launch and from a menu item; any action dismisses it.
    /// It never returns by itself - a screen over someone's work is a modal that gets closed without a look.
    start: bool,
    /// THE START SCREEN WAS OPENED ON REQUEST - from the Windows menu rather than by itself.
    ///
    /// The rule "do not show it over someone's work" is written for what comes up BY ITSELF: an uninvited modal
    /// over somebody's part is what gets closed without a look. It has no right to cancel an explicit request:
    /// pressing the menu item gave NOTHING, not even a message.
    start_asked: bool,
    /// The document properties window.
    doc_props: bool,
    about: bool,
    /// "Report a problem" (Help -> Report a problem).
    report: bool,
    /// THE COMMAND SEARCH: whether it is open, the query, the selected row, a one-shot focus request.
    cmd_search_open: bool,
    cmd_search_query: String,
    cmd_search_sel: usize,
    cmd_search_focus: bool,
    /// The hotkeys window (under Help): a reference built from ONE source, see hotkeys.rs.
    hotkeys: bool,
    /// Whether to show the Machining (CAM) tab. CAM is under development and hidden by default; it is enabled in the settings.
    gcode: bool,
    sim: bool,
    /// "In context" (top-down): show the bodies of NEIGHBOURING parts as ghosts while working inside a part, so
    /// that their geometry can be referred to (a sketch on a neighbour's face becomes an automatic ExternalRef).
    context: bool,
    /// Show the tool manager window.
    tools: bool,
    /// Show the project parameters window (the named dimensions and formulas).
    params: bool,
    /// Whether to show the constraint glyphs in the viewport (a toggle). The dimensions are always shown.
    constraints: bool,
    /// The separate windows: Settings and Machine.
    settings: bool,
    machines: bool,
    /// The parts library: is the catalogue window open?
    parts_library: bool,
}

/// THE EDGES OF THE SELECTED BODY, ready for picking and for drawing.
///
/// Recomputed when the selection or the geometry changes, and keyed by the revision it was taken at, so a
/// stale set is never offered to a click.
#[derive(Default)]
pub(crate) struct EdgeCache {
    /// Whether the extrude gizmo's arrow is being dragged (the feature node's index).
    /// The body whose edges are currently shown for picking (chamfer or fillet), plus their polylines in world space.
    body: Option<Id>,
    polys: Vec<Vec<[f32; 3]>>,
    /// The edge's PERSISTENT id, parallel to `edge_polys` - for picking by an id that survives a rebuild.
    ids: Vec<u32>,
    /// The `geom_rev` at the moment the edge cache was built: when a body is rebuilt (the same Id, a new topology)
    /// the cache goes stale, so it is refreshed and any dangling picked edge ids are cleared.
    rev: u64,
    /// The straight edges of EVERY visible body (the body, the edge's persistent id, the polyline in its LOCAL
    /// frame) - for the axis click-pick (a pattern or a datum axis): an edge of ANY body may be picked, and the id
    /// is what makes the axis associative. Loaded on entry.
    axes: Vec<(Id, u32, Vec<[f32; 3]>)>,
}

/// TRIMMING A SURFACE: the stroke drawn by hand, what it has already cut, and which piece is being kept.
#[derive(Default)]
pub(crate) struct TrimTool {
    /// POWER TRIM: the on-screen trail of the drag and the spans already trimmed (an entity plus a span number).
    /// They live only within one drag - otherwise a second pass over the same line would cut nothing.
    path: Vec<Pos2>,
    done: std::collections::HashSet<(Id, u32)>,
    /// TRIM: what is being cut and which side is kept: (the sheet body, the click point) plus the tool body.
    ///
    /// The point arrives with the first click: a person points at THE part they are keeping, and that same movement
    /// chooses the sheet. Asking for the side as a separate step would split one gesture into two for the sake of
    /// data that has already been given.
    keep: Option<(Id, [f64; 3])>,
    tool: Option<Id>,
}

/// THE MODEL TREE AS DRAWN: what is being searched for, what is being dragged, and where the rows landed.
#[derive(Default)]
pub(crate) struct TreeUi {
    /// The last frame's tree row rectangles - the tests move the mouse over them.
    pub(super) row_rects: Vec<(Id, egui::Rect)>,
    /// The rectangles of the path labels in the parameters window: (the driver's number, where its path was drawn).
    /// Also for the tests - aiming at a label "roughly there" does not work, as a miss confirmed.
    pub(super) drv_path_rects: Vec<(usize, egui::Rect)>,
    /// The build tree search's query. Interface state, not a setting.
    search: String,
    /// The tree row grabbed for a move. `None` means nothing is being dragged.
    drag: Option<Id>,
}

/// THE PARTS LIBRARY: its tree, what is picked in it, and the thumbnails already drawn.
#[derive(Default)]
pub(crate) struct PartsLibrary {
    /// The cache of the parts catalogue's tree (the built-in one plus the user's). None means it is built on opening or refreshing.
    tree: Option<crate::parts_library::LibraryTree>,
    /// The category selected in the library's tree: (is it built in?, the path of indexes from the level's root).
    sel: Option<(bool, Vec<usize>)>,
    /// The search string over names and tags in the library window.
    search: String,
    /// The open "save as a standard part" dialogue (the component plus its metadata and preview).
    save: Option<SavePartDialog>,
    /// The cache of the parts' thumbnail textures for the library grid (keyed by source; None means there is no preview or it would not decode).
    thumbs: std::collections::HashMap<String, Option<egui::TextureHandle>>,
}

/// THE MACHINING SIDE: the program, its g-code, what the check said, and the machine it is meant for.
pub(crate) struct CamState {
    program: Option<Program>,
    gcode: Option<String>,
    verify: Option<VerifyResult>,
    /// The fraction of the toolpath shown (0 to 1) - the backplot's progress slider.
    progress: f32,
    /// The library of machine profiles (kept between sessions).
    machines: Vec<Machine>,
    /// The material chosen for the feeds and speeds (an index into feeds::materials()).
    material: usize,
    /// The mesh result of the material removal simulation plus the flag that shows it.
    sim_mesh: Option<qymcad_core::geom::Mesh>,
    /// The global tool library (in the OS config directory, shared across projects).
    tools: qymcad_core::tool::ToolLibrary,
}

impl Default for CamState {
    fn default() -> Self {
        Self {
            program: None,
            gcode: None,
            verify: None,
            progress: 1.0,
            machines: vec![Machine::default()],
            material: 1,
            sim_mesh: None,
            tools: crate::library::load_tool_library(),
        }
    }
}

/// UNDO, REDO AND WHAT IS SAVED: the history of the document and how far it has been written to disk.
pub(crate) struct Edits {
    /// Undo and redo: snapshots of the editor's state.
    undo: Vec<Step>,
    redo: Vec<Step>,
    /// The currently committed state (the baseline) and its key.
    baseline: Snapshot,
    /// the open edit: (its name, the state BEFORE it). Empty means there is no edit and a change is going past the boundary.
    open: Option<(String, Snapshot)>,
    /// the nesting depth of the edits: an inner `commit_edit` must not close the OUTER edit
    depth: usize,
    committed_key: u64,
    /// The places that change the document past `App::edit` - they are gathered during real work and converted one
    /// at a time. A field rather than a global flag: the debt belongs to the session, not to the process.
    ///
    /// Only the debug build keeps it: the record is written by the debug-only watch below, and a field
    /// nobody reads in release is a `dead_code` refusal, which is denied in the manifest.
    #[cfg(debug_assertions)]
    debt: std::collections::HashSet<String>,
    ready: bool,
    /// The state key as of the last save, load or new project. Being dirty (having unsaved changes) means
    /// `edit_key() != saved_key`.
    saved_key: u64,
    /// Allow the window to close (the dialogue has been answered), so that intercepting `close_requested` does not loop.
    allow_close: bool,
    /// AUTOSAVE: the timer and the key of the last autosave (so the same thing is not written twice).
    last_autosave: std::time::Instant,
    autosave_key: u64,
}

impl Default for Edits {
    fn default() -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
            baseline: Snapshot { project: Project::default() },
            open: None,
            depth: 0,
            committed_key: 0,
            #[cfg(debug_assertions)]
            debt: std::collections::HashSet::new(),
            ready: false,
            saved_key: 0,
            allow_close: false,
            last_autosave: std::time::Instant::now(),
            autosave_key: 0,
        }
    }
}

/// THE STATE OF THE REBUILD: what is pending, what is running in the background, and how far the geometry
/// has moved.
///
/// The revisions are the whole point: nearly every cache and every check compares against them rather than
/// waiting for somebody to remember to clear things.
pub(crate) struct Rebuilding {
    /// The active background operation (a STEP or STL import or export) - a modal overlay with a spinner while it runs.
    busy: Option<Busy>,
    /// A rebuild was requested: it runs in the background on the next frame, with an indicator shown.
    wanted: bool,
    /// THE REBUILD WAS STOPPED BY A PERSON - it no longer starts by itself.
    ///
    /// The dirty marks on the nodes remain after a cancellation (the document really has not been rebuilt), and the
    /// scheduler looks at exactly those. Without this mark the next frame would start precisely the work that was
    /// just stopped. It is cleared by ANY edit of the document and by an explicit "rebuild everything": someone
    /// changed their mind, so computing again is allowed.
    paused: bool,
    /// the application really is running frames (not a headless test) - only then does a rebuild go into the background.
    ui_running: bool,
    /// BACKGROUND (non-modal) work - it runs while the model is already being turned: topping up the imports'
    /// B-rep, saving the project. No overlay is shown, only the status line; the result comes over the same channel
    /// as `busy`.
    bg: Vec<Busy>,
    /// WHETHER THE IMPORTS' RESTORATION WAS CALLED - for the tests: "rebuild everything" must call it.
    import_asked: bool,
    /// The reference rebindings of the last rebuild - they are shown in the status line and mark the nodes in the
    /// tree. They live until the next rebuild and never reach the file.
    rebinds: Vec<qymcad_core::feature::Rebind>,
    /// THE STATE AS OF THE PREVIOUS REBUILD: the document's key, the list of dirty nodes and the number of live
    /// B-reps. The scheduler recomputes only if AT LEAST ONE of those has changed.
    ///
    /// A node that failed to build deliberately stays dirty - the attempt must happen again once its input appears.
    /// But the scheduler read "dirty" as "compute now" and took it up on every frame: with a single red feature the
    /// rebuild window flickered without stopping.
    last: (u64, Vec<Id>, usize),
    /// A counter of geometry changes (for invalidating the 3D render cache).
    geom_rev: u64,
    /// THE LAYOUT'S REVISION - where the parts stand. It moves on a drag and on any component move.
    ///
    /// It is kept separate from `geom_rev` for exactly the reason every other such pair was separated: A LAYOUT IS
    /// NOT GEOMETRY. Bodies do not change when they move, and everything that depends only on their form (the
    /// edges, the smoothed normals, the scene buffer's blocks) has no reason to be recomputed.
    place_rev: u64,
    /// a rebuild was requested inside an edit - it runs ONCE when that edit closes
    pending: bool,
}

impl Default for Rebuilding {
    fn default() -> Self {
        Self {
            busy: None,
            bg: Vec::new(),
            import_asked: false,
            wanted: false,
            paused: false,
            ui_running: false,
            rebinds: Vec::new(),
            last: (0, Vec::new(), 0),
            geom_rev: 0,
            place_rev: 0,
            pending: false,
        }
    }
}

/// THE LIVE GEOMETRY behind the meshes: the kernel's own solids and the faces they were tessellated into.
///
/// A bundle holds meshes and faces, not solids, so the live B-rep is fetched on demand rather than rebuilt
/// when a file is opened.
#[derive(Default)]
pub(crate) struct LiveGeom {
    /// THE BLOBS OF THE LIVE BODIES FOR WRITING, one per body. Serialising every body of a real file costs about
    /// 0.6 s, and without a cache EVERY autosave (once every three minutes) would pay it straight on the UI thread.
    /// An entry here is dropped for exactly those bodies that were rebuilt.
    blobs: std::collections::HashMap<Id, Vec<u8>>,
    /// AT WHICH STATE OF THE DOCUMENT the B-rep cache was last brought up. If nothing has changed since, repeating
    /// it is pointless (and the "ready" flag must not be raised - it would be a lie).
    ///
    /// THIS USED TO HOLD THE PICTURE'S REVISION (`geom_rev`), AND THAT WAS THE MAIN DOOR TO THE FLICKER. `geom_rev`
    /// is moved by `invalidate`, which is called on EVERY frame of a part drag - even though the live B-rep does
    /// not depend on where the part stands at all. In a document with joints on faces, a frame always brings the
    /// B-rep up (`needs_live_brep`), and while the live geometry is not fully raised, every frame of the drag
    /// started the preparation afresh: it marks the nodes dirty and DEMANDS a rebuild by an explicit request, and
    /// an explicit request bypasses every check the scheduler makes. Reported behaviour: after pressing "rebuild
    /// everything" and starting to drag, a modal window flickered unbearably (after a full rebuild nothing has a
    /// live B-rep).
    ///
    /// The key now describes the preparation's INPUTS: the timeline's state (`rebuild_key`) and how many bodies
    /// already have a live form. Dragging a part changes neither; new forms appearing does change it, and a
    /// legitimate second attempt still happens.
    tried_rev: Option<u64>,
    wait: Option<bool>,
    /// Does the cache of live `Shape`s match the timeline? After opening from a bundle the geometry is shown from
    /// the file while the B-rep is not yet built - the first operation that needs it brings it up through
    /// `ensure_brep`.
    ready: bool,
    /// The faces keyed by BODY Id (unlike the index-parallel `faces`, this survives deletions and reorderings of
    /// the meshes). The source for `rebuild_faces_from_cache` after a change of topology.
    faces: std::collections::HashMap<Id, Vec<MeshFace>>,
    /// A runtime cache of the live B-rep shapes keyed by body Id (for the shared booleans). It is not serialised
    /// and is filled in by extrude, revolve, STEP and the booleans.
    shapes: std::collections::HashMap<Id, qymcad_kernel::Shape>,
}

/// WHAT THE PERSON IS BEING MADE TO WAIT FOR, and since when - so that a wait shorter than an eye blink
/// never puts a spinner on screen.
pub(crate) struct Waiting {
    /// THE SPLASH AT START-UP: up to which moment it is held.
    ///
    /// Reported behaviour: there was no splash screen at start-up. Nor was there: the splash was shown ONLY while
    /// something was loading, and on an empty or small project there is nothing to load - it managed a blink, or
    /// never appeared at all. A program must say hello regardless of how fast it opened.
    ///
    /// The B-rep preparation is STARTED and awaiting a rebuild's result; inside is whether the project was clean
    /// before it. In a live window the rebuild is asynchronous, so "the attempt happened" means the rebuild
    /// ARRIVED rather than "the request was sent": marks set on the fact of the request lied (the revision was
    /// moved by a rebuild that had already come in), and the sketch plane pick restarted the preparation every
    /// frame - the overlay flickered and nothing could be done.
    splash_until: Option<std::time::Instant>,
    /// WHEN THE WRITE BEING WAITED FOR STARTED, and when the waiting card was first drawn.
    ///
    /// The two together keep the card from blinking: nothing is shown before `SAVE_WAIT_GRACE`, and what has
    /// been shown stays at least `SAVE_WAIT_MIN`.
    save_since: Option<std::time::Instant>,
    save_shown: Option<std::time::Instant>,
}

impl Default for Waiting {
    fn default() -> Self {
        Self {
            splash_until: Some(std::time::Instant::now() + SPLASH_MIN),
            save_since: None,
            save_shown: None,
        }
    }
}

/// THE COLOUR SCHEME AND THE SETTINGS WINDOW: the palette in force, the ones to choose from, and what is
/// being searched for or edited.
pub(crate) struct SchemeUi {
    /// THE CURRENT PALETTE, derived from the `scheme` setting. It is kept resolved rather than looked up by name
    /// for every colour: a colour is asked for thousands of times per frame.
    pal: crate::palette::Palette,
    /// EVERY KNOWN SCHEME: the built-in ones plus the user's from disk. Kept as a list rather than reading the
    /// directory every frame - the files are re-read only when the editor has changed them.
    all: Vec<crate::palette::Palette>,
    /// the state of the custom-scheme screen: which one is being edited and what to say about saving
    edit: SchemeEdit,
    /// THE OPEN SECTION of the settings window. INTERFACE state rather than a setting: `Settings` holds only the
    /// values that are saved whole, and "where I am in the window right now" is not one of them.
    section: settings_sections::SettingsSection,
    /// The settings search's query.
    search: String,
    /// What to say after a section is reset or after an attempt to open the config folder.
    note: String,
}

impl Default for SchemeUi {
    fn default() -> Self {
        Self {
            pal: crate::palette::dark(),
            all: crate::palette::builtin(),
            edit: SchemeEdit::default(),
            section: settings_sections::SettingsSection::General,
            search: String::new(),
            note: String::new(),
        }
    }
}

/// WHICH BODIES OCCUPY THE SAME SPACE, and at which revision that was last worked out.
pub(crate) struct Interference {
    /// Interference detection (bodies of different parts penetrating one another) in an assembly - a toggle.
    /// It is expensive (a pairwise OCCT boolean), so it is off by default and computed lazily while idle, never
    /// during a drag. The cache holds the pairs of intersecting bodies (Id, Id) plus the `geom_rev` it was computed
    /// at (it is recomputed when the scene changes).
    pairs: Vec<(Id, Id)>,
    rev: u64,
}

impl Default for Interference {
    fn default() -> Self {
        Self {
            pairs: Vec::new(),
            rev: u64::MAX,
        }
    }
}

pub(crate) struct App {
    /// WHICH BODIES OCCUPY THE SAME SPACE, and at which revision that was last worked out.; see [`Interference`].
    interference: Interference,
    /// THE COLOUR SCHEME AND THE SETTINGS WINDOW: the palette in force, the ones to choose from, and what is; see [`SchemeUi`].
    scheme: SchemeUi,
    /// WHAT THE PERSON IS BEING MADE TO WAIT FOR, and since when - so that a wait shorter than an eye blink; see [`Waiting`].
    waiting: Waiting,
    /// THE LIVE GEOMETRY behind the meshes: the kernel's own solids and the faces they were tessellated into.; see [`LiveGeom`].
    live: LiveGeom,
    /// THE STATE OF THE REBUILD: what is pending, what is running in the background, and how far the geometry; see [`Rebuilding`].
    regen: Rebuilding,
    /// UNDO, REDO AND WHAT IS SAVED: the history of the document and how far it has been written to disk.; see [`Edits`].
    edits: Edits,
    /// THE MACHINING SIDE: the program, its g-code, what the check said, and the machine it is meant for.; see [`CamState`].
    cam_job: CamState,
    /// THE PARTS LIBRARY: its tree, what is picked in it, and the thumbnails already drawn.; see [`PartsLibrary`].
    parts: PartsLibrary,
    /// THE MODEL TREE AS DRAWN: what is being searched for, what is being dragged, and where the rows landed.; see [`TreeUi`].
    tree: TreeUi,
    /// TRIMMING A SURFACE: the stroke drawn by hand, what it has already cut, and which piece is being kept.; see [`TrimTool`].
    trim: TrimTool,
    /// THE EDGES OF THE SELECTED BODY, ready for picking and for drawing.; see [`EdgeCache`].
    edges: EdgeCache,
    /// What is open on screen; see [`Windows`].
    win: Windows,
    /// Derived state that a frame may rebuild at will; see [`Caches`].
    cache: Caches,
    project: Project,
    dxf_path: Option<String>,
    project_path: Option<String>,
    /// A crash report left by an EARLIER run, waiting to be shown once. `None` means the last run ended
    /// normally - which is the usual case, and then nothing is drawn.
    crash_report: Option<std::path::PathBuf>,
    /// What has been typed into "Report a problem" and what is to travel with it.
    report: crate::gui::report_problem::ReportDraft,
    /// The splash and its progress. The logo for the splash (its texture is loaded lazily on the first frame).
    logo_tex: Option<egui::TextureHandle>,
    /// The previous frame's canvas rectangle - the window in the rebuild barrier (see `draw_dim_overlay_with`).
    view_rect: egui::Rect,
    /// The action a key is being assigned to right now (the hotkeys window is waiting for a press).
    hotkey_capture: Option<String>,
    /// Why the last assignment was refused - shown in that same window.
    hotkey_note: String,
    /// the dimension tool - one record (see DimTool)
    dim: DimTool,
    /// writing and opening a document - one record (see DocIo)
    io: DocIo,
    /// building a datum - one record (see DatumCommand)
    datum: DatumCommand,
    sel: Sel,
    /// THE VIEW A TOOL PUT ASIDE: `(the mode, the orbit, the flat view)` at the moment of entering a sub-mode that
    /// needs a flat view (choosing a contour, picking an axis by a line).
    ///
    /// The point of view belongs to the person. A tool may BORROW it for the duration of its work, but it must
    /// give it back rather than re-fit it from scratch. Leaving such sub-modes used to set
    /// `view.initialized = false`, and the next frame threw away everything that had been set up by hand -
    /// pressing Esc sent the viewport flying off somewhere.
    view_restore: Option<(bool, Cam3, View2d)>,
    /// THE TEXTS TYPED INTO THE BARS' FIELDS: the field's key mapped to what was typed.
    ///
    /// The values in the state are numbers (`tool_prefs.fillet`, `arr.count` and so on), and text cannot be put
    /// into them. So what was typed lives here and the number is recomputed from it. Without this the bars' fields
    /// would stay `DragValue`s, into which a formula CANNOT be typed: the sketch fillet radius, the offset, the
    /// copy counts and a polygon's number of sides could not be made parametric at all.
    bar_exprs: std::collections::HashMap<&'static str, String>,
    /// THE DRAG OF THE ARROW AT A FACE (push face): the offset at the moment of the grab. Dragging with the mouse
    /// is the main way of direct modelling; typing a number into a field is the fallback, not the only way.
    face_arrow_drag: Option<f64>,
    /// ALL THE PROGRAM'S SETTINGS in one record (see `Settings`). The single owner of the values: the settings
    /// window edits it, the store saves it whole, and the program reads from it.
    set: Settings,
    /// A SMOOTH TURN OF THE VIEW: (from, to, the moment it started). A jump of the view disorients - on an assembly
    /// one has to find one's bearings again afterwards. It lives outside the settings: this is the current frame's
    /// state, not a person's choice.
    view_anim: Option<((f64, f64), (f64, f64), std::time::Instant)>,
    /// THE COMMAND DID NOT APPLY - a flag, not a guess from the status text.
    ///
    /// This used to be decided by searching the status line for a substring. While the interface was in one
    /// language it worked; with a translation the status would stop matching, and a FAILED operation would land
    /// silently in the undo history as a successful one. Checking text is always a lie about what happened: the
    /// text is written for a person, while the decision is taken by the program.
    cmd_failed: bool,
    /// THE SNAP HINT, derived from the cursor and recomputed every frame. It used to live inside the snap settings
    /// and got in the way of saving them whole; it is not a setting.
    snap_hint: Option<(Point2, u8)>,

    status: String,
    view: View2d,
    cam: Cam3,
    mode_3d: bool,
    /// The cursor's coordinates in the part's frame (for the status line), in the 2D view.
    cursor: Option<Point2>,
    /// snapping while drawing - one record (see Snapping)
    /// the automatic constraints while drawing (horizontal, vertical, perpendicular, point-on-edge).
    /// an unfinished drawing - one record (see Placing)
    place: Placing,
    /// what is under the cursor - one record (see Hover)
    hover: Hover,
    /// the component gizmo - one record (see CompGizmo)
    comp_giz: CompGizmo,
    /// the mate command - one record (see JointCommand)
    joint: JointCommand,
    /// the measure tool - one record (see Measuring)
    measure: Measuring,
    /// THE 3D MEASURE TOOL (see measure3d): the elements clicked and the result.
    m3: measure3d::Measure3,
    /// what the command points at by a click - an enumeration (see Picking)
    picking: Picking,
    /// an import awaiting placement - one record (see PendingImport)
    pending_import: PendingImport,
    /// what is being dragged with the mouse in a sketch - one record (see Dragging)
    drag: Dragging,
    /// The parameters window's search: it filters both the variables and the drivers, by name and by path.
    par_search: String,
    /// A NAVIGATION THAT WAITED FOR THE WRITE. The background task's answer arrives where there is no `ctx`, and
    /// a navigation cannot be performed without one - so it is put here and done on the next frame.
    pending_nav: Option<Nav>,
    /// The graveyard of textures awaiting deferred release. A `TextureHandle` MUST NOT be dropped in a frame whose
    /// texture is still being drawn (wgpu reports "Texture ... has been destroyed" on submit). They are put here and
    /// cleared at the START of the next frame, before any drawing, so the texture is no longer used by anyone.
    tex_graveyard: Vec<egui::TextureHandle>,
    /// THE REBUILD GRAPH: the parameter values as of the last rebuild. When a name's value changes, only those
    /// that refer to it are marked dirty, not the project's whole parametrics.
    params_seen: std::collections::HashMap<String, f64>,
    /// The active workbench (Sketch, Part, Assembly or Machining).
    workbench: Workbench,
    /// THE SHEETS PICKED FOR STITCHING. The order matters only for the lineage (the first one carries the body on),
    /// hence a list rather than a set: what was picked is seen in the order it was clicked, and a second click
    /// removes it.
    stitch_parts: Vec<Id>,
    /// THE SURFACE that replaces the faces (in "replace face"). No separate pick mode is needed: a sheet in the
    /// scene cannot be confused with anything, and a click on it reads unambiguously as "that one".
    repl_surface: Option<Id>,
    /// The mode for gathering geometry into the active operation (a click in the viewport adds to it).
    op_pick: Option<OpPick>,
    /// the default values - one record (see Defaults)
    /// the sketch pattern tool - one record (see ArrayTool)
    array: ArrayTool,
    /// the core of the active Part command - one record (see FeatCommand)
    cmd: FeatCommand,
    /// The extrude's operation: 0 a new body, 1 join, 2 cut, 3 intersect.
    /// WHAT the current command builds and WHERE - one record instead of three independent fields (see FeatTarget)
    feat: FeatTarget,
    /// the revolve's parameters - one record (see RevolveParams)
    rev: RevolveParams,
    /// the geometry selected for the commands - one record (see GeomSelection)
    gsel: GeomSelection,
    /// the sketch working session - one record (see SketchSession)
    sketch_ses: SketchSession,
    /// the active sketch tool - one record (see SketchTool)
    tool: SketchTool,
    /// the sketch selection and the deferred action on it - one record (see SketchSelection)
    sel_sk: SketchSelection,
    /// The camera, view and mode saved before entering a sketch, so that the same viewpoint comes back.
    /// A stack of remembered viewpoints per drill-in level (a component or a sketch): pushed on entering, popped on leaving.
    nav_stash: Vec<(Cam3, View2d, bool)>,
    /// The active context's path: [the root, ..., the current one]. The breadcrumbs plus drilling in and out.
    active_path: Vec<Id>,
    /// The machining (CAM) mode - a global switch, outside the components' drill stack.
    cam_mode: bool,
    /// the body gizmo - one record (see BodyGizmo)
    body_giz: BodyGizmo,
    /// The sketches hidden in 3D (by Id) - the visibility checkbox in the tree (they are visible by default).
    sketch_hidden: std::collections::HashSet<Id>,
    /// the rename input - one record (see RenameInput)
    rename: RenameInput,
    /// the rollback line's drag - one record (see RollbackDrag)
    rollback: RollbackDrag,
    /// the Part tools' options - one record (see FeatOptions)
    opts: FeatOptions,
    /// the chamfer's parameters - one record (see ChamferParams)
    chamfer: ChamferParams,
    /// the sweep's parameters - one record (see SweepParams)
    sweep: SweepParams,
    /// the loft's parameters - one record (see LoftParams)
    loft: LoftParams,
    /// the draft's parameters - one record (see DraftParams)
    draft: DraftParams,
    /// the hole command - one record (see HoleCommand)
    hole: HoleCommand,
    /// the primitive's parameters - one record (see PrimParams)
    prim: PrimParams,
    /// the mirror's parameters - one record (see MirrorParams)
    mirror: MirrorParams,
    /// the body split's parameters - one record (see SplitParams)
    split: SplitParams,
    /// the Part pattern's parameters - one record (see ArrayParams)
    arr: ArrayParams,
    /// THE COMPONENT PATTERN (in an assembly): the command and its parameters - one record (see CompArrayCmd)
    carr: CompArrayCmd,
    /// the interface's deferred intentions - one record (see DeferredUi)
    deferred: DeferredUi,
    /// the body-to-body boolean - one record (see BoolCommand)
    boolean: BoolCommand,
    /// the thread's parameters - one record (see ThreadParams)
    thread: ThreadParams,
    /// editing an annotation or a text - one record (see AnnotEdit)
    annot: AnnotEdit,
    /// the inline editing of a sketch element - an enumeration (see InlineEdit)
    inline: InlineEdit,
    /// the sketch pattern's parameters - one record (see SketchPattern)
    sk_pat: SketchPattern,
    /// the drawing tools' preferences - one record (see SketchToolPrefs)
    tool_prefs: SketchToolPrefs,
    /// the rotation angle input - one record (see RotInput)
    rot: RotInput,
    /// the clipboard - one record (see Clipboard)
    clip: Clipboard,
    /// the tree selection - one record (see TreeSelection)
    tree_sel: TreeSelection,
    /// the sketch pattern tool - one record (see PatternTool)
    pat: PatternTool,
    font_cache: Option<Vec<u8>>,
    /// DRAGGING BY THE PART ITSELF: the component, a point on it, and where that point is being led.
    pub(crate) part_pull: Option<(Id, [f64; 3], [f64; 3])>,
    /// Whether an orbit or a zoom is in progress (drawn at a reduced resolution).
    view_dragging: bool,
    /// The GPU viewport is available (the wgpu backend is active and the resources are installed). Otherwise the CPU raster is used.
    gpu_ok: bool,

    /// The dialogue for choosing the STL export's quality is open (Some carries what is being exported).
    stl_export: Option<ExportTarget>,
    /// the section tool - one record (see SectionTool)
    section: SectionTool,
    /// a running sweep of a degree of freedom (a mate's animation)
    joint_anim: Option<JointAnim>,
    /// THE ANCHOR SELECTED in the connector list - its handles are the ones being edited
    sel_conn: Option<Id>,
    /// the corner fillet radius input - one record (see CornerInput)
    corner: CornerInput,
}

/// The sketch's diagnostics in one piece: the degrees of freedom and the redundancy, the free points, THE ARGUING
/// SET of constraints and the redundant ones. Computed in a single pass in `sketch_diag`.
#[derive(Clone, Default)]
pub(crate) struct SketchDiag {
    /// (the degrees of freedom, the redundancy)
    pub dof: (i32, i32),
    /// per sketch point: whether that point can move
    pub free: Vec<bool>,
    /// the indexes of the constraints that contradict one another TOGETHER - any one of the set may be removed
    pub conflicts: std::collections::HashSet<usize>,
    /// the indexes of the constraints whose removal does not raise the degrees of freedom (consistent redundancy)
    pub redundant: std::collections::HashSet<usize>,
}

/// AN UNDO SNAPSHOT IS ONE STATE OF THE DOCUMENT.
///
/// A snapshot used to be assembled from three pieces (the project, the faces and the visibility), while the live
/// B-rep was not part of it at all and was restored by a separate mechanism. While the state is smeared about, an
/// undo regularly under-rolls something: after Ctrl+Z the mesh showed the old geometry while the kernel's cache
/// held the new one. The faces and the visibility now live in the body itself, and a snapshot is the whole project.
#[derive(Clone)]
struct Snapshot {
    project: Project,
}

/// WHAT EXACTLY THE COMMAND BUILDS: the operation on the body and the direction. These used to be three
/// independent fields of `App`, and the ORDER in which they were set mattered more than their values: the direction
/// was computed when the command opened while the operation was chosen later - so a cut went outwards and removed
/// nothing. Gathered into one record they cannot drift apart: what is derived (`flip`) is deduced from `op` when
/// it is needed.
#[derive(Clone, Copy, Default, PartialEq)]
pub(crate) struct FeatTarget {
    /// 0 add (a new body), 1 a boss, 2 a cut, 3 an intersection
    pub op: u8,
    /// the direction: true means against the sketch's normal
    pub flip: bool,
    /// the direction is still automatic (nobody has touched it), so it is recomputed on apply
    pub flip_auto: bool,
}

impl FeatTarget {
    /// Open the command: the direction is automatic for now.
    fn opened(op: u8) -> Self {
        Self { op, flip: false, flip_auto: true }
    }

    /// The direction was set by hand - it is no longer recomputed.
    fn set_flip(&mut self, flip: bool) {
        self.flip = flip;
        self.flip_auto = false;
    }
}

/// THE SWEEP'S PARAMETERS: the profile and the path, each with its own sketch and chosen contour.
/// Five independent fields of `App` turned "which sketch is being picked right now" into implicit state:
/// `pick_path` decided where the next click would land and drifted apart from what had already been chosen.
#[derive(Clone, Copy, Default, PartialEq)]
pub(crate) struct SweepParams {
    /// the profile's sketch and the contour chosen in it (0 means the first suitable one)
    pub prof_sid: Id,
    pub prof_cid: Id,
    /// the path's sketch and the contour chosen in it
    pub path_sid: Id,
    pub path_cid: Id,
    /// the next click picks THE PATH (otherwise the profile)
    pub pick_path: bool,
}

/// THE LOFT'S PARAMETERS: an ordered set of sections (a sketch plus a contour in it) and how to join them.
#[derive(Clone, Default, PartialEq)]
pub(crate) struct LoftParams {
    /// the sections in order: the sketches and the contours chosen in them (parallel lists)
    pub sids: Vec<Id>,
    pub cids: Vec<Id>,
    /// ruled faces between the sections (otherwise a smooth surface)
    pub ruled: bool,
    /// what to do with the result: 0 a new body, 1 a boss, 2 a cut
    pub result: u8,
    /// the sections are being picked by clicks
    pub pick: bool,
    /// the section added last, so that a second click on it does not duplicate it
    pub pick_last: Option<Id>,
}

/// THE REVOLVE'S PARAMETERS: the axis and the angle. The axis is given in THREE mutually exclusive ways (a base
/// X or Y, a datum axis, a sketch centreline) plus two flags for "an axis or a line is being picked right now" - as
/// separate fields that state is easily driven into an impossible combination.
#[derive(Clone, Copy, Default, PartialEq)]
pub(crate) struct RevolveParams {
    /// the sketch's base axis: 0 for X, 1 for Y (used when neither `axis_datum` nor `axis_line` is set)
    pub axis: u8,
    /// a datum axis (it outranks the base one)
    pub axis_datum: Id,
    /// a sketch centreline (it outranks everything)
    pub axis_line: Id,
    /// a datum axis or a centreline is being picked by a click
    pub pick_axis: bool,
    pub pick_line: bool,
    pub angle: f64,
}

/// THE THREAD'S PARAMETERS: what it holds on to (a body and a circular edge) and what is being cut.
#[derive(Clone, Copy, Default, PartialEq)]
pub(crate) struct ThreadParams {
    /// the source body and the circular edge that gave the axis and the radius
    pub src: Option<Id>,
    pub edge: u32,
    pub axis: ([f64; 3], [f64; 3]),
    pub radius: f64,
    pub internal: bool,
    pub starts: u32,
    pub left: bool,
    /// the profile: 0 is metric, the rest follow the standards
    pub form: u8,
    /// an auger (a ribbon) rather than a thread
    pub auger: bool,
}

/// AN UNFINISHED DRAWING: which shape is being finished right now.
///
/// AN ENUMERATION RATHER THAN THREE `Option`s: exactly ONE THING is ever being drawn, and the type must say so.
/// While these were three independent fields, "a rectangle and an ellipse at once" remained a possible state -
/// held off by discipline in the code rather than by the type system.
#[derive(Clone, Default, PartialEq)]
pub(crate) enum PlacingShape {
    #[default]
    None,
    /// a rectangle: two corners and the entities it created
    Rect(Point2, Point2, Vec<Id>),
    /// a polygon by its inscribed or circumscribed circle
    Poly(Id),
    /// an ellipse: the entity and its centre
    Ellipse(Id, Point2),
}

/// The unfinished drawing as a whole: the shape plus the input of its dimensions.
#[derive(Clone, Default, PartialEq)]
pub(crate) struct Placing {
    shape: PlacingShape,
    /// the dimension is following the cursor and has not been placed yet (the constraint's index)
    pub dim: Option<usize>,
    /// the dimension input fields and the focus within them
    pub buf: [String; 2],
    pub focus: bool,
}

impl Placing {
    /// Are the unfinished shape's dimensions being typed in?
    pub(crate) fn active(&self) -> bool {
        self.shape != PlacingShape::None
    }

    /// Abandon the unfinished drawing.
    pub(crate) fn clear(&mut self) {
        *self = Self::default();
    }

    pub(crate) fn rect(&self) -> Option<(Point2, Point2, Vec<Id>)> {
        match &self.shape {
            PlacingShape::Rect(a, b, ids) => Some((*a, *b, ids.clone())),
            _ => None,
        }
    }

    pub(crate) fn poly(&self) -> Option<Id> {
        match self.shape {
            PlacingShape::Poly(id) => Some(id),
            _ => None,
        }
    }

    pub(crate) fn ellipse(&self) -> Option<(Id, Point2)> {
        match self.shape {
            PlacingShape::Ellipse(id, c) => Some((id, c)),
            _ => None,
        }
    }

    /// Start or update the shape. The previous one disappears - it is no longer being drawn.
    pub(crate) fn set(&mut self, shape: PlacingShape) {
        self.shape = shape;
    }
}

/// THE DRAWING TOOLS' PREFERENCES: each has a mode and numbers of its own. This is NOT the state of the current
/// action (that lives in `Placing`) but preferences proper: they survive a change of tool, as in any grown-up CAD -
/// come back to the polygon and the number of sides is the one left there.
#[derive(Clone, Default, PartialEq)]
pub(crate) struct SketchToolPrefs {
    /// the polygon: the number of sides, inscribed or circumscribed, the side's length
    pub poly_n: u32,
    pub poly_mode: u8,
    pub poly_edge: f64,
    /// the construction modes: an arc (by three points or by a centre), a rectangle (corners or centre), a circle
    pub arc_mode: u8,
    pub rect_mode: u8,
    pub circ_mode: u8,
    /// the corner fillet's radius and the offset's distance
    pub fillet: f64,
    pub offset: f64,
    /// a text in a sketch: its contents, its height, and whether it is an annotation rather than geometry
    pub text: String,
    pub text_h: f64,
    pub text_note: bool,
}

/// THE SKETCH PATTERN'S PARAMETERS: two directions with a step and a count each, plus the circular variant.
#[derive(Clone, Copy, Default, PartialEq)]
pub(crate) struct SketchPattern {
    pub dx: f64,
    pub dy: f64,
    pub count: u32,
    pub dx2: f64,
    pub dy2: f64,
    pub count2: u32,
    pub angle: f64,
}

/// THE ACTIVE SKETCH TOOL: what is chosen on the panel and what has already been clicked with it.
/// "Which tool" and "how many points it has gathered" used to be different fields of `App`, and changing the tool
/// was not obliged to clear what had been gathered - yet it must: a line's points mean nothing to an arc.
#[derive(Clone, Default, PartialEq)]
pub(crate) struct SketchTool {
    /// the drawing tool: 0 is Select, then line, rectangle, circle, arc and so on
    pub kind: u8,
    /// the points clicked with THE CURRENT tool (an arc has three, a line two, and so on)
    pub pts: Vec<Point2>,
    /// construction geometry is being drawn
    pub construction: bool,
    /// PROJECTION: take not one edge but THE WHOLE outline of the face the sketch stands on. By a switch of its
    /// own rather than a guess from the click: "clicked inside the face" is indistinguishable from a miss past an
    /// edge on outlines with complicated cut-outs.
    pub proj_face: bool,
    /// the editing tool (trim, offset, corner fillet and so on); 0 means none
    pub modify: u8,
    /// what a click does in editing mode
    pub click_op: u8,
    /// moving or rotating the selected geometry: 0 means none, otherwise the kind of operation and the base point
    pub move_op: u8,
    pub move_base: Option<Point2>,
    /// the tangent edge given for a circle (picked with THIS tool rather than globally)
    pub circ_tan: Option<EdgeRef>,
}

impl SketchTool {
    /// Choose a tool: whatever was clicked with the previous one MUST disappear - it has nothing to do with the new one.
    pub(crate) fn select(&mut self, kind: u8) {
        self.kind = if self.kind == kind { 0 } else { kind };
        self.pts.clear();
    }
}

/// THE CORE OF THE ACTIVE PART COMMAND: what is open, what it works on, with which parameters.
///
/// This is the very place where the order of assignments mattered more than the values. Gathered into one record
/// it now has A LIFE CYCLE: `open` (the command is opened) and `close` (it is closed). Opening clears the past by
/// itself - that used to be a scattering of assignments in `start_feat_cmd`, and nothing stopped one being
/// forgotten.
#[derive(Clone, Default)]
pub(crate) struct FeatCommand {
    /// which command is open: 0 none, 1 extrude, 3 revolve, 4 fillet, and so on
    pub kind: u8,
    /// the sketch the command works from
    pub sketch: Option<usize>,
    /// the parameters on the canvas (a height, a radius, a thickness) - their values and the expressions typed in
    pub params: Vec<CmdParam>,
    /// the extent (one side, symmetric, two sides, through all) and the second side
    pub extent: ExtentMode,
    pub down: f64,
    /// an EXISTING feature is being edited (its Id) rather than a new one created
    pub edit: Option<Id>,
    /// the dimension gizmo is being dragged
    pub drag: bool,
    /// the view was 3D before the command opened - restore it on Esc or on apply
    pub prev_3d: bool,
    /// the parameter field takes the focus on the first frame (so Enter works straight away)
    pub focus: bool,
    /// the body on whose face the command's sketch sits (an associative placement)
    pub ref_body: Option<Id>,
}

impl FeatCommand {
    /// Open command `kind` on sketch `sketch`: the previous state MUST disappear entirely.
    pub(crate) fn open(&mut self, kind: u8, prev_3d: bool) {
        *self = Self { kind, prev_3d, focus: true, ..Default::default() };
    }

    /// Close the command (applied or cancelled).
    pub(crate) fn close(&mut self) {
        *self = Self::default();
    }

    /// Is a command open?
    pub(crate) fn active(&self) -> bool {
        self.kind != 0
    }
}

/// TYPING A CORNER FILLET'S RADIUS IN A SKETCH: the corner is chosen and the radius is still being typed on the
/// canvas. Five fields described ONE unfinished input, and "where we are typing" could outlive "what we are typing".
#[derive(Clone, Default, PartialEq)]
pub(crate) struct CornerInput {
    /// which corner is being rounded: the sketch, the corner point, and whether this is a chamfer rather than a fillet
    pub at: Option<(usize, Id, bool)>,
    /// where on the canvas the input field stands
    pub pos: Option<Pos2>,
    pub buf: String,
    pub focus: bool,
    /// restrict the corners to this set (rounding THE SELECTED corners rather than all of them)
    pub only: Option<std::collections::HashSet<Id>>,
}

impl CornerInput {
    pub(crate) fn clear(&mut self) {
        *self = Self::default();
    }
}

/// THE BODY-TO-BODY BOOLEAN: what is chosen as an operand, and whether an existing node is being edited.
#[derive(Clone, Copy, Default, PartialEq)]
pub(crate) struct BoolCommand {
    /// the second operand of a 2D boolean (a contour index)
    pub other2d: usize,
    /// the chosen body and its role (0 the base, 1 the tool)
    pub pick: Option<(Id, u8)>,
    /// an existing boolean node is being edited (its index in the timeline)
    pub edit: Option<usize>,
}

/// THE MIRROR'S PARAMETERS: a given plane OR a given part - the mirroring goes either by a plane or over a whole
/// part, and as separate fields both could end up given at once.
#[derive(Clone, Default)]
pub(crate) struct MirrorParams {
    pub plane: Option<qymcad_core::feature::SketchPlane>,
    pub part: Option<Id>,
}

/// ALL THE PROGRAM'S SETTINGS IN ONE RECORD, WHICH IS ALSO THE SOLE OWNER OF THE VALUES.
///
/// The settings used to be held two ways at once: some as fields of `App`, some as a positional tuple in the
/// store, while the list of what gets saved was written by hand separately from the settings window. Forgetting to
/// add a setting to that list was THE NORM rather than an accident: the compiler says nothing, no test goes red,
/// and it is a person who finds out. And so it went - out of the window's thirteen settings, seven survived a
/// restart, and among those forgotten was THE THEME ITSELF.
///
/// A value now lives in exactly one place: here. The settings window edits this record, the store saves it whole,
/// and the program reads from it. So "the setting is in the window but is not saved" stopped being an expressible
/// state: to lose one, a field would have to be left uncreated.
///
/// **ONLY a person's choices go here.** Derived state (the snap hint, the highlight, the caches) does not: it is
/// recomputed and has no business in a settings file.
///
/// `serde(default)` on the record and on the nested ones: a setting added by a newer version is read from an older
/// file with its own default rather than bringing down THE WHOLE settings file.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub(crate) struct Settings {
    /// THE INTERFACE LANGUAGE (a code from the `i18n/` catalogue). Empty means nothing has been chosen yet, so it
    /// follows the system locale, and an undetermined locale gives English. An explicit choice overrides both.
    pub language: String,
    /// THE COLOUR SCHEME by its STABLE identifier (`dark`, `light`, or one of the user's). Not by its label: a
    /// built-in scheme's label is translated, and a settings record has no right to depend on the language.
    pub scheme: String,
    /// the ViewCube's size: 0 small, 1 medium, 2 large. On a 4K screen the former 36 px were unreadable, while
    /// "make it larger for everyone" would get in the way on a small screen - this is a person's choice.
    pub viewcube_size: u8,
    /// HOW OFTEN TO WRITE AN AUTOSAVE, in seconds. `0` means never.
    ///
    /// It used to be three minutes as a constant. On a heavy assembly the write is noticeable, and some want
    /// fifteen minutes while others want thirty seconds: the cost of losing work and the cost of a pause differ
    /// from person to person.
    pub autosave_secs: u64,
    /// HOW MANY UNDO STEPS TO KEEP. Memory against the history's length is a person's choice, not ours: a snapshot
    /// of an assembly weighs tens of megabytes, and some people roll back fifty steps.
    pub undo_cap: usize,
    /// THE GHOSTS' OPACITY (0 to 255): a part outside the context, an operation's preview. Some find it in the way
    /// and others cannot see it at all - a matter of taste rather than truth.
    pub ghost_alpha: u8,
    /// THE PERSPECTIVE FIELD OF VIEW, in degrees (the full vertical angle). Everyone is used to their own.
    pub persp_fov_deg: f64,
    /// GPU ANTIALIASING (the MSAA sample count): 1, 2, 4 or 8.
    ///
    /// IT TAKES EFFECT ON A RESTART, and the window says so. The sample count is baked into the wgpu pipelines when
    /// the renderer is created; changing it on the fly would mean rebuilding the pipelines and the render targets.
    /// Silently not applying it would be a lie, so the setting states its price plainly.
    pub msaa: u32,
    /// THE REASSIGNED KEYS: an action's code mapped to a key. ONLY the differences from the factory ones.
    ///
    /// Storing the full layout would be a mistake: a new tool added to the program would never appear for anyone
    /// who had ever touched the keys - their action simply would not be in the record. The differences, by
    /// contrast, survive any growth of the table.
    pub hotkeys: std::collections::BTreeMap<String, String>,
    /// THE HELP'S LANGUAGE, SEPARATE FROM THE INTERFACE'S. Empty means whatever the interface uses.
    ///
    /// Not a whim: CAD terminology is English, and someone working in a translated interface may well want to read
    /// `sweep` and `loft` rather than their translations, so that it matches what they see in manuals and on
    /// forums. The reverse holds too.
    pub help_lang: String,
    /// OPEN THE HELP IN A BROWSER rather than in the program's own window.
    ///
    /// The own window is the default: engineering software must explain itself without the internet. But some
    /// people have a second monitor, and keeping an article beside the program is easier in a browser.
    pub help_external: bool,
    /// show the rapid moves (CAM)
    pub show_rapids: bool,
    /// the Machining (CAM) tab is enabled
    pub cam_tab_enabled: bool,
    /// the GPU viewport (otherwise the CPU raster)
    pub gpu_viewport: bool,
    /// a perspective projection (otherwise orthographic)
    pub cam_perspective: bool,
    /// smooth shading (otherwise flat)
    pub smooth_shading: bool,
    /// THE SHARED TOGGLE FOR SKETCH OUTLINES - FOR THE ASSEMBLY ONLY.
    ///
    /// Inside a Part there is none and there must be none: every sketch has a visibility checkbox of its own there
    /// and a shared one would duplicate it. But in an ASSEMBLY the sketches number in the dozens across all the
    /// components, and hiding them one by one is impractical - there a shared one is needed.
    pub show_contours: bool,
    /// the mate glyphs in an assembly
    pub show_joints: bool,
    /// the interference check in an assembly (expensive, so off by default)
    pub show_interference: bool,
    /// the cursor snap: whether it is on, the grid step, the angle step
    pub snap: Snapping,
    /// the automatic constraints while drawing
    pub auto_constrain: bool,
    /// the values the commands open with
    pub defaults: Defaults,
    /// THE RECENT FILES, the newest first. They live in the settings record because they are saved by the same
    /// mechanism and survive a restart in exactly the same way; a separate store would have to be created, saved
    /// and tested all over again.
    #[serde(default)]
    pub recent: Vec<String>,
    /// HOW MANY FILES TO REMEMBER. A screen-long list is useless: it is searched by eye.
    #[serde(default = "default_recent_limit")]
    pub recent_limit: usize,
    /// THE INTERFACE SCALE. A multiplier for the size of everything `egui` draws: the fonts, the buttons, the
    /// margins. On a 4K screen the factory value gives an unreadably small interface, while on a small screen a
    /// large one eats the viewport - this is a person's choice, not ours.
    #[serde(default = "default_ui_scale")]
    pub ui_scale: f32,
    /// POINTING PRECISION: 0 precise, 1 ordinary, 2 coarse. It scales the grab radii (see `grab.rs`).
    /// On a 4K or a touch screen the former pixel radii are small, while with a mouse large ones make it hard to
    /// aim in tight geometry - this is a person's choice, not ours.
    #[serde(default = "default_pick_precision")]
    pub pick_precision: u8,
}

/// The factory interface scale - whatever the system decides.
fn default_ui_scale() -> f32 {
    1.0
}

/// The factory length of the recent-files list.
fn default_recent_limit() -> usize {
    10
}

/// The factory pointing precision - ordinary. As a function of its own, because `serde(default)` on a field wants
/// exactly that and the record's whole `Default` will not do here.
fn default_pick_precision() -> u8 {
    1
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            language: String::new(),
            scheme: "dark".into(),
            viewcube_size: 1,
            msaa: 4,
            autosave_secs: 180,
            undo_cap: 40,
            ghost_alpha: 115,
            persp_fov_deg: 35.5, // equals the former PERSP_FOV_HALF_TAN of 0.32: 2*atan(0.32)
            hotkeys: Default::default(),
            help_lang: String::new(),
            help_external: false,
            show_rapids: false,
            cam_tab_enabled: false,
            gpu_viewport: true,
            cam_perspective: false,
            smooth_shading: true,
            show_contours: true,
            show_joints: true,
            show_interference: false,
            snap: Snapping::default(),
            auto_constrain: true,
            defaults: Defaults::default(),
            ui_scale: default_ui_scale(),
            recent: Vec::new(),
            recent_limit: default_recent_limit(),
            pick_precision: default_pick_precision(),
        }
    }
}

/// THE CUSTOM SCHEME SCREEN: what is being edited and what to say about saving.
///
/// The edits go in LIVE - a colour is seen at once, with no Apply. Otherwise choosing a shade would be blind:
/// close the window, look, come back. So only the name of the scheme being edited and a note about the last save
/// live here; the colours themselves live in the scheme, which is already gathered in the list.
#[derive(Clone, Default)]
pub(crate) struct SchemeEdit {
    /// whether the colour editor is open
    pub open: bool,
    /// the name in the rename field (empty means no rename is in progress)
    pub rename: String,
    /// what to say about the last save or error
    pub note: String,
}

/// THE COMPONENT PATTERN COMMAND (in an assembly): what is being replicated and how.
///
/// Separate from the BODY pattern (`ArrayParams`): they have different sources (a component against a body),
/// different results (instance components against one merged body) and different edits. Shared state would mean
/// that editing one silently changes the other.
#[derive(Clone, Debug)]
pub(crate) struct CompArrayCmd {
    /// which command is open: 0 none, 1 linear, 2 circular
    pub mode: u8,
    /// the source component (chosen before the start or by a click)
    pub src: Id,
    /// linear: the direction, 0 for X, 1 for Y, 2 for Z
    pub dir: u8,
    /// circular: the axis, 0 for X, 1 for Y, 2 for Z
    pub axis: u8,
    /// an EXISTING pattern is being edited (its Id) rather than a new one created
    pub edit: Id,
}
impl Default for CompArrayCmd {
    fn default() -> Self {
        Self { mode: 0, src: 0, dir: 0, axis: 2, edit: 0 }
    }
}

/// THE SPLIT'S PARAMETERS: the plane a body is divided into independent pieces by. Separate from the mirror - the
/// tools live at the same time, and a shared field would mean that choosing for one erases the other's choice.
#[derive(Clone, Default)]
pub(crate) struct SplitParams {
    pub plane: Option<qymcad_core::feature::SketchPlane>,
}

/// THE OPTIONS OF THE INDIVIDUAL Part tools that have a switch or two each.
#[derive(Clone, Copy, Default, PartialEq)]
pub(crate) struct FeatOptions {
    /// the shell: which side of the opened face the wall goes to
    pub shell_side: qymcad_core::feature::ShellSide,
    /// the patch: meet the edges smoothly (tangent to the neighbouring faces) rather than by position alone
    pub patch_tangent: bool,
    /// the mirror: keep the original
    pub mirror_keep: bool,
    /// the rotation angle used when inserting
    pub rot_deg: f64,
}

/// THE SKETCH SELECTION AND THE DEFERRED ACTION ON IT.
///
/// "What is selected" and "what to do once enough has been gathered" are one state: a constraint or an editing
/// tool is pressed and then geometry is clicked. As separate fields this was expressed so that the deferred
/// action could outlive a cleared selection and fire on THE NEXT, unrelated set.
#[derive(Clone, Default, PartialEq)]
pub(crate) struct SketchSelection {
    /// what is selected: (the kind - a point, an entity and so on - plus its Id)
    pub items: Vec<(u8, Id)>,
    /// awaiting a set for a constraint (the constraint's code)
    pub constraint: Option<u8>,
    /// awaiting a set for an editing tool
    pub modify: Option<u8>,
}

impl SketchSelection {
    /// Clear the selection, together with whatever was waiting for it. A deferred action without a selection means nothing.
    pub(crate) fn clear(&mut self) {
        *self = Self::default();
    }
}

/// THE INTERFACE'S DEFERRED INTENTIONS: an action is decided within a frame but carried out at a safe point, not
/// in the middle of drawing. Keeping them as separate fields would mean that "delete" and "navigate" could drift
/// apart from whatever produced them.
#[derive(Clone, Default)]
pub(crate) struct DeferredUi {
    /// delete what is selected
    pub delete: Option<Sel>,
    /// navigate through the tree or the context
    pub nav: Option<Nav>,
    /// THE NAVIGATION WAITS FOR THE WRITE TO FINISH. Someone answered Save and is leaving for another document:
    /// while the file is written the window stays alive and shows a waiting card, and the navigation happens once
    /// the write lands.
    ///
    /// Reported behaviour: opening another project brought up the Save window, and what was wanted was a popup
    /// with a spinner so that the program would not seem silent. And silent it was: on Save the interface went
    /// into a blocking `wait_bg` and no frame was drawn at all.
    pub nav_after_save: bool,
}

/// THE STATE OF WRITING AND OPENING A DOCUMENT: what is awaiting a write, what the thread has already confirmed,
/// what to open at start-up. As separate fields, "the deferred request to write" and "the key it will confirm"
/// drifted apart: a failed save marked the project clean, because the key was applied without regard to the fact.
#[derive(Clone, Default)]
pub(crate) struct DocIo {
    /// a write was requested while the previous one was still running (the last one wins)
    pub save_request: Option<(String, bool)>,
    /// the state keys AWAITING the thread's confirmation: applied only on success
    pub saved_key: Option<u64>,
    pub autosave_key: Option<u64>,
    /// the file opened when the application starts
    pub startup: Option<String>,
    /// the operation chosen for export
    pub export_op: Option<usize>,
}

/// THE PART PATTERN'S PARAMETERS: up to three linear directions, or a circular one about an axis.
/// Eleven fields described ONE intention, and "two directions" could end up switched on with a count of 1, or an
/// axis chosen for a linear pattern.
#[derive(Clone, Copy, Default, PartialEq)]
pub(crate) struct ArrayParams {
    pub count: u32,
    pub dir: u8,
    /// the second direction
    pub two: bool,
    pub count2: u32,
    pub dir2: u8,
    /// the third direction
    pub three: bool,
    pub count3: u32,
    pub dir3: u8,
    /// a circular pattern about an axis: the axis itself, whether it is a full turn, and whether the axis is being picked by a click
    pub axis: Id,
    pub full: bool,
    pub axis_pick: bool,
}

/// BUILDING A DATUM: which mode it is in and what references have been gathered.
#[derive(Clone, Default)]
pub(crate) struct DatumCommand {
    /// the chosen base plane (for an offset plane)
    pub plane_pick: Option<qymcad_core::feature::SketchPlane>,
    /// the axis mode and the reference geometry gathered
    pub axis_mode: u8,
    pub axis_ref: Option<([f64; 3], [f64; 3])>,
    pub axis_pts: Vec<(Id, [f64; 3])>,
    pub axis_hit: Option<AxisHit>,
    /// the point mode and the chosen vertex
    pub pt_mode: u8,
    pub pt_vert: Option<(Id, u32, bool, [f64; 3])>,
    /// the datums hidden in the view
    pub hidden: std::collections::HashSet<Id>,
    /// the datum's consumers need rebuilding
    pub regen_pending: bool,
}

/// THE DIMENSION TOOL IN A SKETCH: what has been pointed at and what is being typed.
#[derive(Clone, Default, PartialEq)]
pub(crate) struct DimTool {
    /// kind of dimension: 0 means none
    pub kind: u8,
    /// the first reference pointed at, and what has been clicked
    pub first: Option<DimRef>,
    pub pick: Vec<Id>,
    /// typing the value: buffer, focus, editing an existing dimension
    pub buf: String,
    pub focus: bool,
    pub edit: Option<(Id, String, String)>,
    /// a hint drawn over the canvas
    pub overlay: Option<String>,
    /// A SILENT REBUILD IS RUNNING: a spinner in the middle of the canvas, no text and no dimming.
    ///
    /// While a background rebuild runs silently, the previous body is still on screen and people draw
    /// wrong conclusions from it. The status line is not enough — nobody looks at it while looking at
    /// the model. A spinner right on the canvas says one thing, but says it in time: what you see is
    /// STALE, a recompute is under way.
    pub spinner: bool,
    /// NODE COUNT of a long rebuild `(done, total)` — next to the caption and nowhere else.
    /// A count means there is also a "Cancel" button: exactly what reports its progress can be stopped.
    pub overlay_progress: Option<(usize, usize)>,
}

/// THE JOINT COMMAND: what has been pointed at, with which anchor, and whether an existing one is being edited.
#[derive(Clone)]
pub(crate) struct JointCommand {
    /// parameters of the joint being created
    pub new_kind: qymcad_core::feature::JointKind,
    pub new_offset: f64,
    pub new_angle: f64,
    /// KEEP IT WHERE IT STANDS RIGHT AT CREATION (an "as-built" joint).
    ///
    /// A "keep it where it stands" button in the joint's properties comes TOO LATE: the joint has
    /// already brought the anchors together and dragged the part away, and there is nothing left to
    /// declare "as it stands" — the spoiled placement would be baked in. Measured on a machine
    /// assembly: a slider joint carried an axis together with its spindle 1176.339 mm away. For an
    /// already assembled machine exactly the opposite is needed: the joint is born moving nothing.
    pub new_as_built: bool,
    /// picking mode: by faces, grounding, kind of anchor, the first anchor pointed at
    pub pick_faces: bool,
    pub ground_pick: bool,
    pub anchor_mode: u8,
    pub pick_first: Option<(Id, qymcad_core::feature::AnchorRef)>,
    /// editing an existing joint and re-picking its anchor
    pub edit: Option<Id>,
    pub edit_repick: Option<(Id, bool)>,
    /// THE SURFACES BEING POINTED AT FOR A TANGENT CONDITION.
    ///
    /// `Some` means the "Tangent" tool is in hand. Two surfaces are enough: a tangent condition has no
    /// connectors at all, and the second pick places it right away.
    pub tangent_pick: Option<Vec<(Id, qymcad_core::feature::AnchorRef)>>,
    /// THE ANCHORS BEING POINTED AT FOR A WIDTH CONDITION: two walls and the part between them.
    ///
    /// `Some` means the "Width" tool is in hand. Order matters: the first two anchors are the walls,
    /// the third is what sits between them.
    pub width_pick: Option<Vec<(Id, qymcad_core::feature::AnchorRef)>>,
    /// THE SET OF PARTS BEING GATHERED INTO A GROUP.
    ///
    /// `Some` means the "Group" tool is in hand; inside are the parts already clicked. Clicking a part
    /// again takes it back out of the set: the set is EDITED, not only appended to.
    pub group_pick: Option<Vec<Id>>,
    /// WHETHER A STANDALONE CONNECTOR IS BEING CREATED.
    ///
    /// A connector is a feature in its own right, like a sketch: it is made UP FRONT, and joints are
    /// attached to it afterwards. Without such a command a connector could only appear inside a joint,
    /// and there was no way to reuse it.
    pub conn_pick: bool,
    /// THE RELATION BETWEEN JOINTS BEING ASSEMBLED RIGHT NOW.
    ///
    /// `Some` means the "Relation" tool is in hand. What gets picked is not faces but THE JOINTS
    /// THEMSELVES: a relation ties their degrees of freedom together, and there is nothing else in it
    /// to point at.
    pub relation_pick: Option<RelationPick>,
    /// THE CONNECTOR WHOSE SECONDARY AXIS WE ARE WAITING TO BE POINTED AT (the "second pick").
    ///
    /// A square face has no long side, and the automatic choice answers arbitrarily; a person then
    /// points at an edge, and the axis will run along it. While this is `Some`, a click on geometry
    /// sets the axis of that connector and nothing else.
    pub axis_pick: Option<Id>,
    /// dragging the joint gizmo
    pub giz_drag: Option<JointGizDrag>,
    pub giz_handle: Option<(u8, bool)>,
}

impl Default for JointCommand {
    fn default() -> Self {
        Self {
            new_kind: qymcad_core::feature::JointKind::Rigid,
            new_offset: 0.0,
            new_angle: 0.0,
            new_as_built: false,
            pick_faces: false,
            ground_pick: false,
            anchor_mode: 0,
            pick_first: None,
            edit: None,
            edit_repick: None,
            axis_pick: None,
            conn_pick: false,
            relation_pick: None,
            group_pick: None,
            width_pick: None,
            tangent_pick: None,
            giz_drag: None,
            giz_handle: None,
        }
    }
}

/// A RELATION BEING PICKED: its kind, the joints already pointed at with their slots, and the number.
///
/// The slot is not picked separately by hand: the kind itself says what is what
/// (`RelationKind::slots_are_rotations`), and the matching degree of freedom is taken from the joint
/// pointed at. A separate dialogue for choosing the degree is only warranted where a joint has several
/// degrees of the matching sort.
#[derive(Clone)]
pub(crate) struct RelationPick {
    pub kind: qymcad_core::feature::RelationKind,
    /// already pointed at, as (joint, slot), in order
    pub picks: Vec<(Id, usize)>,
    /// the relation's number: a gear ratio or a travel per turn, depending on the kind
    pub value: f64,
    pub reversed: bool,
}

impl Default for RelationPick {
    fn default() -> Self {
        Self { kind: qymcad_core::feature::RelationKind::Gear, picks: Vec::new(), value: 1.0, reversed: false }
    }
}

/// SWEEPING A DEGREE OF FREEDOM (animating a joint).
///
/// A mechanism is checked by eye: assemble it, then sweep it and watch how it travels. Numbers do not
/// show that, and dragging a part by hand to convince yourself that it reaches the end and does not
/// pass through its neighbour is guesswork, not a check.
#[derive(Clone)]
pub(crate) struct JointAnim {
    pub joint: Id,
    pub slot: usize,
    /// the bounds of the sweep, taken from the joint's limits (`Project::joint_anim_range`)
    pub from: f64,
    pub to: f64,
    /// how far along from `from` to `to`, 0..1, and which way it travels
    pub t: f64,
    pub forward: bool,
    /// WHAT WAS SET BEFORE THE SWEEP — to put back when it is stopped.
    ///
    /// A sweep is a PREVIEW, not an edit: leaving the part wherever the stop caught it would silently
    /// change the document by pressing a "have a look" button.
    pub saved: Option<f64>,
}

/// THE SECTION TOOL: the cutting plane and its gizmo editing.
#[derive(Clone, Default)]
pub(crate) struct SectionTool {
    /// the plane, as (point, normal); None means the section is off
    pub plane: Option<([f64; 3], [f64; 3])>,
    /// the plane is being picked by a click
    pub pick: bool,
    /// shift along the normal and the tilts (deg about U and V)
    pub offset: f64,
    pub rot: [f64; 2],
    /// the gizmo arrow is being dragged; the drag anchor
    pub drag: bool,
    pub drag_anchor: Option<(f64, Pos2)>,
}

/// THE RENAME INPUT: what is being renamed and what has been typed.
#[derive(Clone, Default)]
pub(crate) struct RenameInput {
    /// a timeline node / a tree node / a sketch — three different targets of one input
    pub target: Option<Id>,
    pub node: Option<RenameNode>,
    pub sketch: Option<Id>,
    pub buf: String,
    pub focus: bool,
}

/// THE HOLE COMMAND: the kind, how it is placed, and the sketch holding the points.
#[derive(Clone, Copy, Default)]
pub(crate) struct HoleCommand {
    /// kind: simple / counterbore / countersink
    pub kind: u8,
    /// placement: on a face or on sketch points
    pub mode: u8,
    pub sketch: Option<Id>,
    /// drill the other way
    pub flip: bool,
}

/// CHAMFER PARAMETERS: an asymmetric chamfer is measured FROM A REFERENCE FACE, so "which face is the
/// reference" and "are we picking it right now" must live together with the mode itself — otherwise the
/// mode is asymmetric and there is no reference face.
#[derive(Clone, Copy, Default)]
pub(crate) struct ChamferParams {
    pub mode: qymcad_core::feature::ChamferMode,
    pub flip: bool,
    pub ref_face: u32,
    pub pick_ref: bool,
}

/// DRAFT PARAMETERS: the neutral face (the angle is measured from it) and the direction.
#[derive(Clone, Copy, Default)]
pub(crate) struct DraftParams {
    pub neutral: u32,
    pub pick_neutral: bool,
    pub flip: bool,
}

/// PRIMITIVE PARAMETERS: how many sides (for a prism) and where it goes.
#[derive(Clone, Copy, Default)]
pub(crate) struct PrimParams {
    pub n: u32,
    pub place: Option<[f64; 3]>,
    pub frame: Option<[f64; 12]>,
}

/// SKETCH PATTERN PARAMETERS (a tool, not a feature): the kind, editing an existing one, the centre.
#[derive(Clone, Copy, Default)]
pub(crate) struct PatternTool {
    pub op: u8,
    pub edit: Option<usize>,
    pub center: Option<Point2>,
}

/// SKETCH PATTERN PARAMETERS (the linear step and the count).
#[derive(Clone, Copy, Default)]
pub(crate) struct ArrayTool {
    pub n: u32,
    pub dx: f64,
    pub dy: f64,
}

/// THE COMPONENT GIZMO IN AN ASSEMBLY: what is being dragged, along which axis or ring.
#[derive(Clone, Copy, Default)]
pub(crate) struct CompGizmo {
    pub drag: Option<(Id, [f64; 12], [f64; 3], f64)>,
    pub snap: bool,
    pub axis: Option<u8>,
    pub ring: Option<u8>,
}

/// TYPING A ROTATION ANGLE: the value, the buffer and the focus — one unfinished input.
#[derive(Clone, Default)]
pub(crate) struct RotInput {
    pub angle: f64,
    pub buf: String,
    pub focus: bool,
}

/// WHAT IS BEING DRAGGED IN THE SKETCH RIGHT NOW.
///
/// AN ENUM, NOT SIX `Option`s: exactly ONE THING can be dragged. As separate fields a pair of them
/// could end up occupied at once, and the drag handlers fought over a single pointer — a state that
/// never happens, but which the type allowed.
#[derive(Clone, Default, PartialEq)]
pub(crate) enum Dragging {
    #[default]
    None,
    /// a sketch point, as (sketch index, point index)
    Point(usize, usize),
    /// a spline handle, as (sketch, spline, node)
    Handle(usize, usize, usize),
    /// moving the whole selected geometry
    Move(usize, Vec<Id>),
    /// a dimension label / a note / a text
    Dim(usize),
    Note(usize),
    Text(usize),
}

impl Dragging {
    /// Is anything being dragged?
    pub(crate) fn active(&self) -> bool {
        *self != Dragging::None
    }

    pub(crate) fn clear(&mut self) {
        *self = Dragging::None;
    }

    pub(crate) fn pt(&self) -> Option<(usize, usize)> {
        match *self {
            Dragging::Point(a, b) => Some((a, b)),
            _ => None,
        }
    }

    pub(crate) fn handle(&self) -> Option<(usize, usize, usize)> {
        match *self {
            Dragging::Handle(a, b, c) => Some((a, b, c)),
            _ => None,
        }
    }

    pub(crate) fn mov(&self) -> Option<(usize, Vec<Id>)> {
        match self {
            Dragging::Move(a, ids) => Some((*a, ids.clone())),
            _ => None,
        }
    }

    pub(crate) fn dim(&self) -> Option<usize> {
        match *self {
            Dragging::Dim(i) => Some(i),
            _ => None,
        }
    }

    pub(crate) fn note(&self) -> Option<usize> {
        match *self {
            Dragging::Note(i) => Some(i),
            _ => None,
        }
    }

    pub(crate) fn text(&self) -> Option<usize> {
        match *self {
            Dragging::Text(i) => Some(i),
            _ => None,
        }
    }
}

/// THE BODY GIZMO: moving or rotating a body with the mouse, plus typing the same value as a number.
/// The same pairing as `CompGizmo` has for a component: "what is being dragged" and "what is being
/// typed" are one intent, and as separate fields the input could outlive the release of the gizmo.
#[derive(Clone, Default)]
pub(crate) struct BodyGizmo {
    pub axis: Option<u8>,
    pub ring: Option<u8>,
    pub drag: Option<(usize, [f64; 3], f64)>,
    pub snap: bool,
    /// the gizmo itself is being dragged (an arrow or a ring has been grabbed)
    pub dragging: bool,
    /// typed input: (body, axis, is it a rotation?) plus its buffer and focus
    pub num: Option<(usize, u8, bool)>,
    pub num_buf: String,
    pub num_focus: bool,
}

/// IN-PLACE EDITING OF A SKETCH ELEMENT: what exactly is being edited right on the canvas. Exactly one
/// thing is edited at a time — hence an enum rather than four `Option`s.
#[derive(Clone, Copy, Default, PartialEq)]
pub(crate) enum InlineEdit {
    #[default]
    None,
    Note(usize),
    Text(usize),
    Dim(usize),
    Circle(Id),
}

impl InlineEdit {
    pub(crate) fn note(&self) -> Option<usize> {
        match *self { InlineEdit::Note(i) => Some(i), _ => None }
    }
    pub(crate) fn text(&self) -> Option<usize> {
        match *self { InlineEdit::Text(i) => Some(i), _ => None }
    }
    pub(crate) fn dim(&self) -> Option<usize> {
        match *self { InlineEdit::Dim(i) => Some(i), _ => None }
    }
    pub(crate) fn circle(&self) -> Option<Id> {
        match *self { InlineEdit::Circle(i) => Some(i), _ => None }
    }
    pub(crate) fn clear(&mut self) {
        *self = InlineEdit::None;
    }
}

/// WHAT THE COMMAND IS POINTING AT BY A CLICK RIGHT NOW. The modes are MUTUALLY EXCLUSIVE — in the
/// code that was held by hand, with comments next to it saying "exclusive: reset the others". An enum
/// makes it a property of the type: a second mode simply cannot be turned on without turning the first
/// one off.
#[derive(Clone, Copy, Default, PartialEq)]
pub(crate) enum Picking {
    #[default]
    None,
    /// the plane for a NEW sketch; inside it, the face, if a body was clicked
    SketchPlane(Option<Id>),
    /// replacing the plane of an existing sketch (by its index)
    ReplaceSketch(usize),
    /// all the edges of a body, for a fillet
    FilletAll,
    /// a contour into a slot of the command (profile / path / section)
    Contour(ContourSlot),
}

impl Picking {
    pub(crate) fn clear(&mut self) {
        *self = Picking::None;
    }
    pub(crate) fn is_sketch_plane(&self) -> bool {
        matches!(self, Picking::SketchPlane(_))
    }
    pub(crate) fn plane_face(&self) -> Option<Id> {
        match *self { Picking::SketchPlane(f) => f, _ => None }
    }
    pub(crate) fn set_plane_face(&mut self, f: Option<Id>) {
        if self.is_sketch_plane() {
            *self = Picking::SketchPlane(f);
        }
    }
    pub(crate) fn replace_sketch(&self) -> Option<usize> {
        match *self { Picking::ReplaceSketch(i) => Some(i), _ => None }
    }
    pub(crate) fn fillet_all(&self) -> bool {
        *self == Picking::FilletAll
    }
    pub(crate) fn contour(&self) -> Option<ContourSlot> {
        match *self { Picking::Contour(s) => Some(s), _ => None }
    }
}

/// EDITING AN ANNOTATION (a note or a text) in a sketch: what is being edited and what has been typed.
#[derive(Clone, Default)]
pub(crate) struct AnnotEdit {
    pub note: Option<usize>,
    pub note_buf: String,
    pub text: Option<usize>,
    pub text_buf: String,
    pub text_h: f64,
}

/// THE MEASURING TOOL: whether it is on, together with the points collected — one state, not a flag
/// apart from the points (otherwise the points outlive the switch-off and pop up the next time it is
/// turned on).
#[derive(Clone, Default)]
pub(crate) struct Measuring {
    pub on: bool,
    pub pts: Vec<Point2>,
}

impl Measuring {
    pub(crate) fn clear(&mut self) {
        *self = Self::default();
    }
}

/// DRAGGING THE TIMELINE ROLLBACK BAR: the accumulated shift and whether a rollback is still pending.
#[derive(Clone, Copy, Default)]
pub(crate) struct RollbackDrag {
    pub accum: f32,
    pub pending: bool,
}

/// THE SELECTION IN THE TREE: the multiple selection, the range anchor and the rubber-band box.
#[derive(Clone, Default)]
pub(crate) struct TreeSelection {
    pub multi: Vec<Id>,
    pub anchor: Option<Id>,
    pub box_start: Option<Pos2>,
}

/// DEFAULT VALUES — not the state of a command but SETTINGS: a command opens with them, and they
/// outlive its closing. They used to sit among the command's own fields and so looked like state.
#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub(crate) struct Defaults {
    /// the extrusion height the command opens with
    pub extrude_h: f64,
    /// the offset distance for 2D contour edits
    pub offset_2d: f64,
}

impl Default for Defaults {
    fn default() -> Self {
        Self { extrude_h: 10.0, offset_2d: 3.0 }
    }
}

/// WHAT IS UNDER THE CURSOR. Three independent fields could stay filled at once: move the cursor off
/// the sketch onto a joint, and the constraint highlight did not go out, because a different handler
/// was the one that cleared it.
#[derive(Clone, Copy, Default)]
pub(crate) struct Hover {
    /// sketch geometry, as (kind, Id)
    pub sketch: Option<(u8, Id)>,
    /// a constraint in the sketch list or glyph
    pub constraint: Option<usize>,
    /// an assembly joint
    pub joint: Option<Id>,
}

// `Hover` deliberately has NO shared reset: three places clear THEIR OWN kind of hover ("no constraint
// under the cursor here"), and the passes run every frame, so the mutual exclusion holds by itself. A
// shared reset would be wrong here — it would clear the hover a neighbouring pass had just computed.

/// SNAPPING WHILE DRAWING — SETTINGS ONLY.
///
/// The hint saying "where it snapped just now" used to live here too, and it is derived from the
/// cursor: it is recomputed every frame and has no place among settings. While it sat inside, this
/// record could not be a setting as a whole — and being a setting as a whole is exactly what makes
/// saving automatic (see `Settings`). The hint moved to `App::snap_hint`.
#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub(crate) struct Snapping {
    pub on: bool,
    /// grid step, mm
    pub grid: f64,
    /// angle snapping step, deg
    pub rot_deg: f64,
}

impl Default for Snapping {
    fn default() -> Self {
        Self { on: true, grid: 1.0, rot_deg: 15.0 }
    }
}

/// AN IMPORT WAITING TO BE PLACED: the curves have been read, but where they go has not been said yet.
/// Together with the drawing points that came from the same import, this is one unfinished action.
#[derive(Clone, Default)]
pub(crate) struct PendingImport {
    /// the curves read, their source and the file name
    pub curves: Option<(Vec<qymcad_core::geom::ProfEdge>, Option<Id>, String)>,
    /// the points to be drawn once it has been placed
    pub draw_pts: Option<Vec<Point2>>,
}

impl PendingImport {
    pub(crate) fn clear(&mut self) {
        *self = Self::default();
    }
}

/// THE SKETCH WORKING SESSION: which one is open for editing and which one was the last. Kept apart,
/// "open" could be cleared while "last" still held a different sketch — and the next command took hold
/// of the wrong one.
#[derive(Clone, Copy, Default)]
pub(crate) struct SketchSession {
    /// the sketch open for editing (None means not in sketch mode)
    pub editing: Option<Id>,
    /// the last one worked on (for commands that act "on the last sketch")
    pub last: Option<Id>,
}

/// THE GEOMETRY SELECTED FOR COMMANDS: profiles, faces, edges. "Faces of which body" used to live apart
/// from the faces themselves, and the set could outlive a change of body.
#[derive(Clone, Default)]
pub(crate) struct GeomSelection {
    pub profiles: std::collections::HashSet<Id>,
    pub faces: std::collections::HashSet<u32>,
    pub faces_body: Option<Id>,
    pub edges: std::collections::HashSet<u32>,
    /// A DESCRIPTION OF THE SELECTION instead of a snapshot.
    ///
    /// A reference remembers NOT ONLY WHAT was selected but also HOW. Click an edge and it is that one
    /// edge, a list. Click a face, or take the "expand selection" menu item, and the intent is a
    /// different one: "all the edges of this face", "everything parallel to this one" — and it must
    /// outlive an edit after which there are more elements than before. Then a query sits here, and the
    /// feature records the query rather than today's numbers.
    ///
    /// Touch even one element on its own and the description collapses and this becomes empty: "all the
    /// edges of the face except this one" is not something we can express yet, and a description must
    /// not lie.
    pub described: Option<qymcad_core::refs::Query>,
    /// THE LAST FACE CLICKED, together with its body.
    ///
    /// The "expand selection" menu needs it. In a fillet, clicking a face puts its EDGES into the set
    /// and the face itself is kept nowhere — so the menu, which asked `faces`, saw emptiness and did
    /// not open at all. Reported behaviour: pressing the right button did nothing.
    pub last_face: Option<(u32, Id)>,
    /// THE LAST EDGE UNDER THE CURSOR, together with its body. The same as `last_face`, but for the
    /// edge items of the menu: "the whole tangent chain" is asked for while hovering an edge, not a face.
    ///
    /// They are mutually exclusive: whatever was under the cursor last is what the menu offers.
    /// Otherwise we would have to guess what is being asked about — and it has already been pointed at.
    pub last_edge: Option<(u32, Id)>,
    /// WAITING FOR THE SECOND PICK of a "between": the FIRST face of it sits here.
    ///
    /// `Between` is the only query for which one element pointed at is not enough: a seam belongs to TWO
    /// sets. So the menu item does not create the reference straight away but puts the command into
    /// "now click the other side" mode.
    pub between_first: Option<u32>,
    /// the constraint selected in the sketch list
    pub constraint: Option<usize>,
}

impl GeomSelection {
    /// ADD A FACE TO THE DESCRIPTION "all the edges of these faces".
    ///
    /// It accumulates as a union: two faces can be clicked in a row, and both go into one description.
    /// If a description of a DIFFERENT kind was there before (say "everything parallel"), it is
    /// replaced — different intents must not be mixed silently.
    pub fn describe_edges_of_face(&mut self, fid: u32) {
        use qymcad_core::refs::Query;
        self.described = Some(match self.described.take() {
            Some(Query::Adjacent(inner)) => Query::Adjacent(Box::new(Query::Union(inner, Box::new(Query::Id(fid))))),
            _ => Query::Adjacent(Box::new(Query::Id(fid))),
        });
    }
}

/// THE CLIPBOARD: sketch geometry and tree nodes are different things, but they share one state of "what has been copied".
#[derive(Clone, Default)]
pub(crate) struct Clipboard {
    pub geom: Option<qymcad_core::model::GeomClip>,
    pub geom_pending: Option<(Vec<Id>, bool)>,
    pub geom_place: bool,
    pub tree: Option<TreeClip>,
    pub tree_multi: Option<(Vec<Id>, bool)>,
    pub os_ping: bool,
}

/// AN UNDO STEP IS AN OPERATION. Not "the document changed somehow" but "an Extrude was carried out":
/// a step has a name, and that name is shown. The stack used to hold nameless snapshots, because the
/// boundary of a step was decided by THE FRAME rather than by the command.
struct Step {
    name: String,
    snap: Snapshot,
}

/// THE BOUNDARY OF AN OPERATION ON THE DOCUMENT.
///
/// It opens under a name and closes either by a commit (the step goes onto the undo stack) or by an
/// abort (the document comes back as it was, leaving no trace). Nested operations merge into ONE: a
/// command touches the sketch, the timeline and the selection — that is one action and one undo step.
///
/// A forgotten `commit` does not lose the step: `Drop` commits. Losing an edit silently is worse than
/// recording one step too many, and this is easy to spot — the step appears under the name of the
/// operation that opened it.
pub(crate) struct Edit<'a> {
    app: &'a mut App,
    /// whether this is the OUTER operation (it is the one that creates the step) or a nested one (which does nothing on exit)
    outer: bool,
    done: bool,
}

impl<'a> Edit<'a> {

    /// The application as a whole — not every edit lives in `Project` yet (face caches, the selection).
    pub(crate) fn app(&mut self) -> &mut App {
        self.app
    }

    /// The operation happened: put the step onto the undo stack.
    pub(crate) fn commit(mut self) {
        self.finish(true);
    }

    /// The operation did not happen: bring the document back as it was, create no step.
    pub(crate) fn abort(mut self) {
        self.finish(false);
    }

    fn finish(&mut self, ok: bool) {
        if self.done {
            return;
        }
        self.done = true;
        let _ = self.outer;
        if ok {
            self.app.commit_edit();
        } else {
            // ABORT: a failed operation leaves no trace, neither in the document nor on the undo stack
            self.app.abort_edit();
        }
    }
}

impl Drop for Edit<'_> {
    fn drop(&mut self) {
        self.finish(true);
    }
}

/// INSTALL THE FONTS INTO A CONTEXT — IN ONE PLACE.
///
/// This used to be done right inside the application start-up, and the `qym-bold` family existed only
/// there. Any other `egui` context (and tests build their own) panicked on the first attempt to draw a
/// bold label: "FontFamily::Name(\"qym-bold\") is not bound to any fonts". That is, a test checking the
/// drawing was impossible precisely because the font setup was hidden inside `main`.
///
/// Now there is one setup, and everyone who needs a full context calls it.
/// THE NODE NAME FIELD: show it IN WORDS, store WHAT WAS TYPED.
///
/// A node's name is both document data and text on screen: the core stores a KEY (`name-plane`), and a
/// person may rename it to anything. A field bound straight to the stored value showed the key —
/// `name-plane` and `name-assembly` appeared in the right-hand panel.
///
/// The rule is simple and reversible: while the name has not been touched, the key stays in the
/// document (and the label follows a change of language); the very first edit stores the WORDS typed,
/// and they must not be translated any more.
pub(crate) fn name_edit(ui: &mut egui::Ui, stored: &mut String) -> egui::Response {
    let mut shown = crate::i18n::name(stored);
    let r = ui.text_edit_singleline(&mut shown);
    if r.changed() {
        *stored = shown;
    }
    r
}

pub(crate) fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    // the Phosphor icon font (otherwise icons render as "tofu" boxes)
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
    // A BOLD FACE. The egui set has none at all — only the ordinary proportional and monospace ones.
    // Labels drawn OVER geometry (the X/Y/Z axes, dimensions) get lost in a thin font.
    // Liberation Sans Bold: OFL (the licence sits next to the file), Latin plus Cyrillic.
    fonts.font_data.insert(
        BOLD_FONT.to_string(),
        egui::FontData::from_static(include_bytes!("../../../assets/fonts/LiberationSans-Bold.ttf")),
    );
    fonts.families.insert(egui::FontFamily::Name(BOLD_FONT.into()), vec![BOLD_FONT.to_string()]);
    ctx.set_fonts(fonts);
}

/// The name of the BOLD font family. One place: family names spelled out separately drift apart and
/// give a silent fallback to the default font — the text still draws, only not bold, and that is
/// invisible to the eye in the code.
pub(crate) const BOLD_FONT: &str = "qym-bold";

/// The bold font at a given size.
pub(crate) fn bold(size: f32) -> egui::FontId {
    egui::FontId::new(size, egui::FontFamily::Name(BOLD_FONT.into()))
}

/// HOW LONG THE SPLASH SCREEN IS HELD AT LEAST. Five seconds, even if it opened instantly; if loading
/// takes longer, it stays longer. The splash here is a greeting rather than an indicator: one that
/// flashes for a hundred milliseconds says nothing at all.
const SPLASH_MIN: std::time::Duration = std::time::Duration::from_secs(5);

/// HOW LONG A WRITE MUST RUN BEFORE THE WAITING CARD IS SHOWN AT ALL.
///
/// A small document is written faster than an eye can catch, and a card flashing for one frame reads as a
/// glitch rather than an answer. Below this the program simply gets on with it and says nothing.
const SAVE_WAIT_GRACE: std::time::Duration = std::time::Duration::from_millis(120);

/// AND HOW LONG IT STAYS ONCE SHOWN.
///
/// Without a floor the card could still blink: the write ends a moment after the grace has passed. What was
/// shown must be readable, so once up it stays for this long even if the write is already over.
const SAVE_WAIT_MIN: std::time::Duration = std::time::Duration::from_millis(400);


impl App {
    /// The active operation (for picking contours and for highlighting).
    fn active_op(&self) -> Option<usize> {
        match self.sel {
            Sel::Op(i) if i < self.project.operations.len() => Some(i),
            _ => None,
        }
    }
}

/// tan(half of the field of view) for perspective mode. ~0.32 is a vertical FOV of about 35 deg — a
/// moderate amount of depth, the usual default. Not used in orthographic mode.

/// The opacity of a ghosted body: straight alpha 0..255. Your own body or sketch is seen through it.

/// The orbit camera for the software 3D view (orthographic or perspective projection — see `App::cam_perspective`).
#[derive(Clone, Copy)]
struct Cam3 {
    yaw: f64,
    pitch: f64,
    scale: f32,
    target: [f64; 3],
    init: bool,
}

impl Default for Cam3 {
    fn default() -> Self {
        // Front isometric: the camera sits at +X -Y +Z (front = -Y towards the viewer), the usual choice.
        // +Y runs up and away, so the "top" of a sketch on the XY plane matches the "top" in 3D.
        Self { yaw: -0.7, pitch: 0.6, scale: 4.0, target: [0.0; 3], init: false }
    }
}

impl Cam3 {
    fn basis(&self) -> ([f64; 3], [f64; 3], [f64; 3]) {
        let dir = [self.pitch.cos() * self.yaw.cos(), self.pitch.cos() * self.yaw.sin(), self.pitch.sin()];
        let fwd = [-dir[0], -dir[1], -dir[2]];
        // TOP AND BOTTOM VIEWS (fwd parallel to Z): world Z as the "up" gives a zero cross product, so the
        // basis degenerates and the picture collapses. In that case world Y is taken as the reference
        // "up" (the ViewCube's top no longer breaks the view).
        let ref_up = if fwd[2].abs() > 0.999 { [0.0, 1.0, 0.0] } else { [0.0, 0.0, 1.0] };
        let right = v_norm(v_cross(fwd, ref_up));
        let up = v_norm(v_cross(right, fwd));
        (right, up, fwd)
    }
}

/// The state of the "Save as a standard part" dialogue (right-click on a component in the tree).
/// The manifest metadata, plus the chosen category (a path relative to the user's directory), plus a
/// pre-rendered PNG preview of the body. Writing it out goes through `subproject_of` and `save_part`.
struct SavePartDialog {
    /// The source component (a Part or a subassembly) the standard part is taken from.
    component: qymcad_core::model::Id,
    name: String,
    description: String,
    /// Tags separated by commas or spaces (parsed when written out).
    tags: String,
    /// The category path relative to `<data>/library/parts` (e.g. "Profiles/Aluminium"). Empty = the root.
    category: String,
    /// The user's existing categories (folders) — for picking one quickly from chips.
    known_cats: Vec<String>,
    /// The raw preview of the body (256^2, rendered at the moment it opened). Encoded to PNG when written out.
    preview: Option<egui::ColorImage>,
    /// The GPU texture of the preview (loaded lazily from `preview` to show it in the window).
    tex: Option<egui::TextureHandle>,
}

/// The contour slot of a sweep or a loft for which a contour is being picked in the half-sketcher (as in
/// Extrude: the sketch lies flat, a contour is clicked). The sweep profile, the sweep path, or the i-th
/// section of a loft.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ContourSlot {
    SweepProfile,
    SweepPath,
    LoftSection(usize),
}

impl Default for App {
    fn default() -> Self {
        let mut project = Project::default();
        project.new_document(); // the root assembly plus an active empty first Part right from the start
        project.tools.push(default_tool(1));
        project
            .operations
            .push(OperationDef::new(&crate::i18n::tr("g-profile"), 1, OpKind::Contour { side: SideMode::Auto, tabs: Tabs::default(), ramp: Ramp::default(), climb: true, finish: false }));
        let mut app = Self {
            set: Settings::default(),
            cmd_failed: false,
            snap_hint: None,
            view_anim: None,
            cache: Caches::default(),
            interference: Interference::default(),
            scheme: SchemeUi::default(),
            waiting: Waiting::default(),
            live: LiveGeom::default(),
            regen: Rebuilding::default(),
            edits: Edits::default(),
            cam_job: CamState::default(),
            parts: PartsLibrary::default(),
            tree: TreeUi::default(),
            trim: TrimTool::default(),
            edges: EdgeCache::default(),
            win: Windows { help_article: "index".into(), start: true, constraints: true, ..Default::default() },
            project,
            dxf_path: None,
            project_path: None,
            crash_report: None,
            report: Default::default(),
            logo_tex: None,
            io: DocIo::default(),
            view_rect: egui::Rect::NOTHING,
            hotkey_capture: None,
            hotkey_note: String::new(),
            dim: DimTool::default(),
            datum: DatumCommand::default(),
            sel: Sel::Op(0),
            view_restore: None,
            face_arrow_drag: None,
            bar_exprs: std::collections::HashMap::new(),
            status: crate::i18n::tr("g-cam-import-first"),
            view: View2d::default(),
            cam: Cam3::default(),
            mode_3d: false,
            cursor: None,
            picking: Picking::default(),
            pending_import: PendingImport::default(),
            drag: Dragging::default(),
            place: Placing::default(),
            hover: Hover::default(),
            comp_giz: CompGizmo::default(),
            // THE "ANGLE" FIELD IS EMPTY AT START-UP. It used to hold 90 deg, and that silently became a
            // REQUIREMENT: `joint_pick_anchor_click` writes a non-zero angle into `drive[0]` of the joint
            // being created. Every joint was born demanding a 90 deg turn that nobody had asked for —
            // against the contract stated in that same place ("a joint is born with its degrees of
            // freedom free"). On a slider that pinned a degree it does not even have; a gear relation
            // became unsatisfiable, the solve did not converge, and the WHOLE mechanism froze.
            joint: JointCommand::default(),
            measure: Measuring::default(),
            m3: measure3d::Measure3::default(),
            par_search: String::new(),
            pending_nav: None,
            tex_graveyard: Vec::new(),
            params_seen: std::collections::HashMap::new(),
            workbench: Workbench::Part,
            repl_surface: None,
            stitch_parts: Vec::new(),
            op_pick: None,
            array: ArrayTool { n: 3, dx: 20.0, dy: 0.0 },
            cmd: FeatCommand::default(),
            feat: FeatTarget::default(),
            rev: RevolveParams { angle: 360.0, ..Default::default() },
            gsel: GeomSelection::default(),
            sketch_ses: SketchSession::default(),
            tool: SketchTool::default(),
            sel_sk: SketchSelection::default(),
            nav_stash: Vec::new(),
            active_path: Vec::new(),
            cam_mode: false,
            sketch_hidden: std::collections::HashSet::new(),
            rename: RenameInput::default(),
            rollback: RollbackDrag::default(),
            opts: FeatOptions { mirror_keep: true, ..Default::default() },
            chamfer: ChamferParams::default(),
            sweep: SweepParams::default(),
            loft: LoftParams::default(),
            draft: DraftParams::default(),
            hole: HoleCommand::default(),
            prim: PrimParams { n: 6, ..Default::default() },
            mirror: MirrorParams::default(),
            split: SplitParams::default(),
            arr: ArrayParams { count: 3, count2: 2, count3: 2, dir2: 1, dir3: 2, full: true, ..Default::default() },
            carr: CompArrayCmd::default(),
            deferred: DeferredUi::default(),
            boolean: BoolCommand::default(),
            thread: ThreadParams { starts: 1, ..Default::default() },
            annot: AnnotEdit::default(),
            inline: InlineEdit::default(),
            sk_pat: SketchPattern { dx: 20.0, dy: 0.0, count: 3, dx2: 0.0, dy2: 20.0, count2: 1, angle: 360.0 },
            tool_prefs: SketchToolPrefs { poly_n: 6, poly_edge: 10.0, fillet: 3.0, offset: 3.0, text_h: 10.0, ..Default::default() },
            rot: RotInput::default(),
            clip: Clipboard::default(),
            tree_sel: TreeSelection::default(),
            pat: PatternTool::default(),
            font_cache: None,
            body_giz: BodyGizmo::default(),
            part_pull: None,
            view_dragging: false,
            gpu_ok: false,
            stl_export: None,
            section: SectionTool::default(),
            joint_anim: None,
            sel_conn: None,
            corner: CornerInput::default(),
        };
        // THE GUARD'S BASELINE COMES FROM THE DOCUMENT ITSELF, not from zero. With zero the very first
        // frame saw "the key does not match" and declared a fresh empty document changed outside an
        // operation.
        app.edits.committed_key = app.doc_key();
        app.edits.saved_key = app.edit_key();
        app
    }
}


fn default_tool(n: u32) -> Tool {
    Tool {
        number: n,
        name: crate::i18n::tr1("cam-tool-default", "n", &n.to_string()),
        kind: ToolType::FlatEnd,
        diameter: 3.0,
        corner_radius: 0.0,
        flutes: 2,
        v_angle: None,
    }
}

impl App {
    fn program_name(&self) -> String {
        self.dxf_path
            .as_deref()
            .map(|p| std::path::Path::new(p).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "program".into()))
            .unwrap_or_else(|| "program".into())
    }

    /// The file name out of a path (used as the name of a sketch or a source).
    fn file_name(path: &str) -> String {
        std::path::Path::new(path).file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| path.to_string())
    }

    /// Embed the original of an imported file into the document and return its Id.
    fn embed_source(&mut self, path: &str) -> Option<Id> {
        match std::fs::read(path) {
            Ok(bytes) => Some(self.project.add_source(Self::file_name(path), bytes)),
            Err(_) => None,
        }
    }

    fn open_dxf(&mut self, path: String) {
        match import_dxf(&path) {
            Ok(sk) => self.arm_sketch_import(sk.curves, &path),
            Err(e) => self.status = crate::i18n::tr1("g-dxf-error", "error", &e.to_string()),
        }
    }

    /// Load the imported curves (DXF or SVG) and switch to picking the PLACEMENT plane — a click on a
    /// plane (XY/XZ/YZ), on a datum, or on a face of a part in the viewport builds an EDITABLE sketch
    /// out of these curves in the active context (the assembly, or the part if the import was started
    /// inside one). The original file is embedded (for re-importing and for comparison). Esc cancels.
    fn arm_sketch_import(&mut self, curves: Vec<qymcad_core::geom::ProfEdge>, path: &str) {
        if curves.is_empty() {
            self.status = crate::i18n::tr("g-import-empty");
            return;
        }
        let n = curves.len();
        let src = self.embed_source(path);
        self.cancel_all_tools(); // drop the other picking modes and commands, so clicks do not conflict
        self.mode_3d = true; // placement planes and faces are picked in 3D (like picking a sketch plane)
        self.sel = Sel::None;
        self.pending_import.curves = Some((curves, src, Self::file_name(path)));
        self.dxf_path = Some(path.to_string());
        self.status = crate::i18n::tr1("g-import-place", "n", &n.to_string());
    }

    /// Place the waiting import on the chosen plane: build an editable sketch in the active context and
    /// enter its editing (visible and editable straight away — it can be drawn onto and dimensioned).
    fn place_pending_import(&mut self, plane: qymcad_core::feature::SketchPlane) {
        let Some((curves, source, name)) = self.pending_import.curves.take() else { return };
        let plane = self.resolve_placement_plane(plane);
        let si = self.project.import_sketch(name, curves, source, plane);
        let ents = self.project.sketches[si].entities.len();
        self.invalidate();
        self.view.initialized = false;
        self.enter_sketch_edit(si);
        self.status = crate::i18n::tr1("g-sketch-imported", "n", &ents.to_string());
    }

    /// Add bodies (a mesh plus B-rep faces) to the document ADDITIVELY (each body becomes its own part
    /// in the tree, which can be selected and hidden). The imported group is laid with its top at Z=0,
    /// keeping the bodies' positions relative to each other.
    fn add_bodies(&mut self, bodies: Vec<(qymcad_core::geom::Mesh, Vec<qymcad_core::geom::MeshFace>)>) {
        if bodies.is_empty() {
            return;
        }
        let max_z = bodies.iter().filter_map(|(m, _)| m.bounds()).map(|b| b.max.z).fold(f64::MIN, f64::max);
        let dz = if max_z > f64::MIN { -max_z } else { 0.0 };
        let mut first: Option<usize> = None;
        for (mut m, mut fs) in bodies {
            if dz != 0.0 {
                m.translate(0.0, 0.0, dz);
                for f in &mut fs {
                    f.centroid.z += dz;
                }
            }
            let bid = self.project.add_mesh(m);
            self.project.imported_bodies.insert(bid); // an import is a valid body with no node (prune leaves it alone)
            self.live.faces.insert(bid, fs.clone()); // the face cache keyed by body Id (for quick access)
            if let Some(b) = self.project.bodies.last_mut() {
                b.faces = fs;
            }
            if first.is_none() {
                first = Some(self.project.bodies.len() - 1);
            }
        }
        if let Some(i) = first {
            self.sel = Sel::Mesh(i);
        }
        self.invalidate();
        self.view.initialized = false;
        self.cam.init = false;
    }

    fn open_stl(&mut self, path: String) {
        // parsing the STL and detecting its faces run in a worker thread; the UI spins a spinner and the window does not freeze
        let (tx, rx) = std::sync::mpsc::channel();
        let p = path.clone();
        std::thread::spawn(move || {
            let res = match import_stl(&p) {
                Ok(mesh) => {
                    let faces = mesh.detect_faces(8.0);
                    JobResult::StlImported { path: p, mesh, faces }
                }
                Err(e) => JobResult::Failed(crate::i18n::tr1("g-stl-error", "error", &e.to_string())),
            };
            let _ = tx.send(res);
        });
        self.regen.busy = Some(Busy { label: crate::i18n::tr("g-import-stl"), rx, kind: BgKind::ImportShapes, pulse: None, quiet: false });
    }

    /// A new empty document (geometry and operations are reset; the global tool and machine libraries
    /// are kept, they live outside the document).
    /// A NEW DOCUMENT FROM A TEMPLATE.
    ///
    /// A template is read as an ordinary document — which is what it is — but its path is NOT
    /// REMEMBERED. Otherwise the very first Save would write the work over the template, and both would
    /// be lost at once: the template, and the confidence that files stay where they were left.
    ///
    /// The creation date is cleared too: the document is created NOW, not when the template was saved.
    pub(crate) fn new_from_template(&mut self, path: &str) {
        match qymcad_io::load_project(path) {
            Ok(mut project) => {
                project.ensure_document();
                project.meta.created.clear();
                self.finish_project_load(path.to_string(), project, Vec::new());
                // ...and forget straight away where it came from: this is a new document, not an opened file
                self.project_path = None;
                self.set.recent.retain(|p| p != path); // a template is not a "recent file"
                self.edits.saved_key = self.edit_key(); // a blank slate: no edits have been made yet
                self.status = crate::i18n::tr1("tpl-new-from", "name", &std::path::Path::new(path).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default());
            }
            Err(e) => self.status = format!("{} {}", ph::WARNING, crate::i18n::tr1("tpl-open-failed", "error", &e.to_string())),
        }
    }

    /// Save the CURRENT document as a template.
    pub(crate) fn save_as_template(&mut self, title: &str) {
        match crate::templates::save(&self.project, title) {
            Ok(path) => self.status = crate::i18n::tr1("tpl-saved", "path", &path),
            Err(e) => self.status = format!("{} {}", ph::WARNING, crate::i18n::tr1("tpl-save-failed", "error", &e)),
        }
    }

    fn new_project(&mut self) {
        self.project = Project::default();
        self.project.new_document();
        self.live.shapes.clear();
        self.sel = Sel::None;
        self.cam_job.program = None;
        self.cam_job.gcode = None;
        self.cam_job.sim_mesh = None;
        self.win.sim = false;
        self.project_path = None;
        self.dxf_path = None;
        self.pending_import.draw_pts = None;
        self.invalidate();
        self.view.initialized = false;
        self.cam.init = false;
        self.edits.saved_key = self.edit_key(); // a clean start — there are no edits
        self.status = crate::i18n::tr("g-new-project");
    }

    /// A NEW ASSEMBLY DOCUMENT: the root without an empty part, with the root active.
    ///
    /// "Create a part" and "create an assembly" are different intents, and starting an assembly with an
    /// empty part that then has to be deleted means making a person clean up after the program.
    fn new_assembly_project(&mut self) {
        self.new_project();
        // `new_document` created a part under the root — an assembly document does not need it
        let root = self.project.root;
        let parts: Vec<Id> = self.project.components.iter().filter(|c| c.parent == Some(root)).map(|c| c.id).collect();
        for id in parts {
            self.project.delete_component(id); // ask_delete-exempt: this is not deleting anyone's work but cleaning up while creating the document
        }
        self.project.set_active_component(Some(root));
        self.edits.saved_key = self.edit_key(); // a clean start — there are no edits
        self.status = crate::i18n::tr("g-new-assembly");
    }

    /// The autosave path sits next to the document (`name.autosave.qcad`); an unnamed document goes to temp.
    ///
    /// FROM THE STEM, NOT FROM THE WHOLE NAME. `format!("{path}.autosave.qcad")` wrote the extension twice -
    /// `Filter-v2.qcad.autosave.qcad` - and a second autosave over the first gave
    /// `Filter-v2.qcad.autosave.qcad.bak`. A name nobody can read is a name nobody checks.
    fn autosave_path(&self) -> String {
        match &self.project_path {
            Some(p) => {
                let base = std::path::Path::new(&p);
                base.with_extension("").to_string_lossy().into_owned() + ".autosave.qcad"
            }
            None => std::env::temp_dir().join("qymcad-unsaved.autosave.qcad").to_string_lossy().into_owned(),
        }
    }


    /// Whether there are unsaved changes (the state key has drifted from the moment of saving).
    fn is_dirty(&self) -> bool {
        self.edit_key() != self.edits.saved_key
    }

    /// Request navigation that could lose edits: with unsaved work it asks first, otherwise it goes ahead.
    fn request_nav(&mut self, nav: Nav, ctx: &egui::Context) {
        if self.is_dirty() {
            self.deferred.nav = Some(nav);
        } else {
            self.do_nav(nav, ctx);
        }
    }

    /// Carry out the navigation (there are no edits, or it was confirmed in the dialogue).
    fn do_nav(&mut self, nav: Nav, ctx: &egui::Context) {
        match nav {
            Nav::New => self.new_project(),
            Nav::NewAssembly => self.new_assembly_project(),
            Nav::OpenDialog => {
                if let Some(p) = rfd::FileDialog::new().add_filter("QymCAD", &["qcad", "ron"]).pick_file() {
                    self.spawn_project_load(p.to_string_lossy().into_owned());
                }
            }
            Nav::NewFromTemplate(path) => self.new_from_template(&path),
            Nav::OpenPath(path) => self.open_recent(path),
            Nav::Exit => {
                self.wait_bg(); // an unfinished background write must reach the disk before exiting
                self.edits.allow_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }


    /// The centre of the selected object (for the gizmo), in the part's XY coordinates.
    fn selected_centroid(&self) -> Option<Point2> {
        match self.sel {
            Sel::Contour(i) => self.project.contours.get(i).map(|c| c.centroid()),
            Sel::Mesh(i) => self.project.bodies.get(i).map(|b| &b.mesh).and_then(|m| m.bounds()).map(|b| Point2::new((b.min.x + b.max.x) / 2.0, (b.min.y + b.max.y) / 2.0)),
            _ => None,
        }
    }

    /// Move the selected object (a contour or a body) in XY.
    fn translate_selected(&mut self, dx: f64, dy: f64) {
        match self.sel {
            Sel::Contour(i) => {
                if let Some(c) = self.project.contours.get_mut(i) {
                    c.translate(dx, dy);
                    self.invalidate();
                }
            }
            Sel::Mesh(i) => {
                if i < self.project.bodies.len() {
                    self.move_body_at(i, mat_translate(dx, dy, 0.0)); // a B-rep gets a Move feature; a raw mesh is simply shifted
                }
            }
            _ => {}
        }
    }

    /// The geometry of the active sketch, for snapping: segments and circles (arcs count as circles).
    /// Returns (lines as [(A, B)], circles as [(centre, radius)]).
    fn active_edges(&self, si: usize) -> (Vec<(Point2, Point2)>, Vec<(Point2, f64)>) {
        use qymcad_core::model::EntityKind;
        let mut lines = Vec::new();
        let mut circs = Vec::new();
        if let Some(s) = self.project.sketches.get(si) {
            let pt = |id: Id| s.points.iter().find(|p| p.id == id).map(|p| Point2::new(p.x, p.y));
            for e in &s.entities {
                match e.kind {
                    EntityKind::Line { a, b } => {
                        if let (Some(pa), Some(pb)) = (pt(a), pt(b)) {
                            lines.push((pa, pb));
                        }
                    }
                    EntityKind::Circle { center, r } => {
                        if let Some(c) = pt(center) {
                            circs.push((c, r));
                        }
                    }
                    EntityKind::Arc { center, a, .. } => {
                        if let (Some(c), Some(pa)) = (pt(center), pt(a)) {
                            circs.push((c, ((pa.x - c.x).powi(2) + (pa.y - c.y).powi(2)).sqrt()));
                        }
                    }
                    EntityKind::Ellipse { .. } => {} // drawn by its own outline (as a profile)
                }
            }
        }
        (lines, circs)
    }

    /// The outline polyline of an ellipse entity in WORLD coordinates (for hit-testing and drawing).
    fn ellipse_outline_world(&self, si: usize, c: Id, ma: Id, mi: Id) -> Vec<Point2> {
        let Some(s) = self.project.sketches.get(si) else { return Vec::new() };
        let p = |id: Id| s.points.iter().find(|q| q.id == id).map(|q| (q.x, q.y));
        let (Some((cx, cy)), Some((max, may)), Some((mix, miy))) = (p(c), p(ma), p(mi)) else { return Vec::new() };
        let major = ((max - cx).powi(2) + (may - cy).powi(2)).sqrt().max(1e-6);
        let minor = ((mix - cx).powi(2) + (miy - cy).powi(2)).sqrt().max(1e-6);
        let (ux, uy) = ((max - cx) / major, (may - cy) / major);
        let (vx, vy) = (-uy, ux);
        let n = 72;
        (0..=n)
            .map(|k| {
                let t = std::f64::consts::TAU * k as f64 / n as f64;
                let (ct, st) = (t.cos(), t.sin());
                Point2::new(cx + major * ct * ux + minor * st * vx, cy + major * ct * uy + minor * st * vy)
            })
            .collect()
    }


    /// Test facades: the same two buttons the tool bar presses.
    #[cfg(test)]
    pub(crate) fn set_dim_tool_for_test(&mut self, k: u8) {
        self.set_dim_tool(k);
    }

    #[cfg(test)]
    pub(crate) fn set_sk_tool_for_test(&mut self, t: u8) {
        self.set_sk_tool(t);
    }

    /// Turn the dimension tool on or off (k: 1 = linear, 2 = angular, 3 = radius).
    fn set_dim_tool(&mut self, k: u8) {
        let cur = self.dim.kind;
        self.exit_draw_tools(); // entering a tool means leaving all the others, in one move
        self.dim.kind = if cur == k { 0 } else { k };
        if self.dim.kind != 0 {
            self.mode_3d = false;
            if self.edit_si().is_none() && !matches!(self.sel, Sel::Sketch(_)) {
                self.status = crate::i18n::tr("g-open-sketch-dim");
            } else {
                self.status = crate::i18n::tr("g-dim-hint");
            }
        }
    }

    /// The reference object under the cursor: a point or centre > the midpoint of a line > the origin.
    /// Used for dimensions and constraints between any geometry.
    fn resolve_sketch_ref(&self, rect: Rect, pos: Pos2, si: usize) -> Option<SketchRef> {
        use qymcad_core::model::EntityKind;
        // 1) the nearest point (the centres of circles and arcs are points too)
        if let Some(id) = self.nearest_sketch_point(rect, pos, si) {
            return Some(SketchRef::Point(id));
        }
        // 2) the midpoint of a segment
        if let Some(s) = self.project.sketches.get(si) {
            let mut best: Option<(f32, (Id, Id))> = None;
            for e in &s.entities {
                if let EntityKind::Line { a, b } = e.kind {
                    if let (Some(pa), Some(pb)) = (self.sketch_pt(si, a), self.sketch_pt(si, b)) {
                        let mid = Point2::new((pa.x + pb.x) * 0.5, (pa.y + pb.y) * 0.5);
                        let d = self.to_screen(rect, mid).distance(pos);
                        if d <= 10.0 && best.map_or(true, |(bd, _)| d < bd) {
                            best = Some((d, (a, b)));
                        }
                    }
                }
            }
            if let Some((_, ab)) = best {
                return Some(SketchRef::Midpoint(ab.0, ab.1));
            }
        }
        // 3) the origin
        if self.to_screen(rect, Point2::new(0.0, 0.0)).distance(pos) <= 11.0 {
            return Some(SketchRef::Origin);
        }
        None
    }

    /// Turn a reference object into a REAL point (creating the midpoint or the origin if needed) -> its Id.
    fn materialize_ref(&mut self, si: usize, r: SketchRef) -> Id {
        use qymcad_core::model::{Constraint, SketchPoint};
        match r {
            SketchRef::Point(id) => id,
            SketchRef::Origin => self.project.ensure_origin(si),
            SketchRef::Midpoint(a, b) => {
                // reuse an existing midpoint if a Midpoint constraint is already there
                if let Some(p) = self.project.sketches[si].constraints.iter().find_map(|c| match c {
                    Constraint::Midpoint { p, a: ca, b: cb } if (*ca == a && *cb == b) || (*ca == b && *cb == a) => Some(*p),
                    _ => None,
                }) {
                    return p;
                }
                let (pa, pb) = (self.sketch_pt(si, a), self.sketch_pt(si, b));
                let (mx, my) = match (pa, pb) {
                    (Some(pa), Some(pb)) => ((pa.x + pb.x) * 0.5, (pa.y + pb.y) * 0.5),
                    _ => (0.0, 0.0),
                };
                let id = self.project.alloc_id();
                self.project.sketches[si].points.push(SketchPoint { id, x: mx, y: my });
                self.project.sketches[si].constraints.push(Constraint::Midpoint { p: id, a, b });
                id
            }
        }
    }

    /// The index of the note under a screen point (a rough bounding box of the text).
    fn note_at(&self, rect: Rect, pos: Pos2, si: usize) -> Option<usize> {
        let s = self.project.sketches.get(si)?;
        for (i, n) in s.notes.iter().enumerate() {
            let sp = self.to_screen(rect, Point2::new(n.x, n.y));
            let w = (n.text.chars().count().max(1) as f32) * 8.0;
            let bb = Rect::from_min_max(sp + egui::vec2(-2.0, -17.0), sp + egui::vec2(w, 3.0));
            if bb.contains(pos) {
                return Some(i);
            }
        }
        None
    }

    /// The handle of a polygon under the cursor is the centre of its circumscribed (construction)
    /// circle, when the cursor is close to that "rim". A circle counts as a rim when vertices hang on it
    /// (PointOnCircle towards its centre). Returns the Id of the centre (so the radius can be retyped).
    fn polygon_under(&self, rect: Rect, pos: Pos2, si: usize) -> Option<Id> {
        use qymcad_core::model::{Constraint, EntityKind};
        let s = self.project.sketches.get(si)?;
        for e in &s.entities {
            let EntityKind::Circle { center, r } = e.kind else { continue };
            if !e.construction {
                continue;
            }
            let is_rim = s.constraints.iter().any(|c| matches!(c, Constraint::PointOnCircle { c: cc, .. } if *cc == center));
            if !is_rim {
                continue;
            }
            if let Some((cx, cy)) = s.points.iter().find(|q| q.id == center).map(|q| (q.x, q.y)) {
                let sc = self.to_screen(rect, Point2::new(cx, cy));
                let rp = (self.to_screen(rect, Point2::new(cx + r, cy)).x - sc.x).abs();
                if (sc.distance(pos) - rp).abs() <= 8.0 {
                    return Some(center);
                }
            }
        }
        None
    }

    /// The reference under the cursor for a dimension: a vertex or centre (Point) > a line entity (Line)
    /// > a coordinate axis (Line) > a midpoint or the origin (Point). Materialises the midpoint, the
    /// origin or the axis.
    fn resolve_dim_ref(&mut self, rect: Rect, pos: Pos2, si: usize) -> Option<DimRef> {
        // 1) a vertex or the centre of a circle (a real point)
        if let Some(id) = self.nearest_sketch_point(rect, pos, si) {
            return Some(DimRef::Point(id));
        }
        // 2) a line entity under the cursor
        if let Some((a, b)) = self.nearest_line_entity(rect, pos, si) {
            return Some(DimRef::Line(a, b));
        }
        // 2b) a circle or arc under the cursor (a click on the rim) resolves to its CENTRE as a point
        // reference, openly rather than silently: the dimension is taken from the centre. The status line
        // says so.
        if let Some(eid) = self.nearest_circle_entity(rect, pos, si) {
            let center = self.project.sketches.get(si).and_then(|s| s.entities.iter().find(|e| e.id == eid)).and_then(|e| match e.kind {
                qymcad_core::model::EntityKind::Circle { center, .. } | qymcad_core::model::EntityKind::Arc { center, .. } => Some(center),
                _ => None,
            });
            if let Some(c) = center {
                self.status = crate::i18n::tr("g-dim-from-centre");
                return Some(DimRef::Point(c));
            }
        }
        // 3) a coordinate axis
        let o = self.to_screen(rect, Point2::new(0.0, 0.0));
        if (pos.y - o.y).abs() <= 6.0 {
            let (a, b) = self.project.ensure_axis(si, 0);
            return Some(DimRef::Line(a, b));
        }
        if (pos.x - o.x).abs() <= 6.0 {
            let (a, b) = self.project.ensure_axis(si, 1);
            return Some(DimRef::Line(a, b));
        }
        // 4) the midpoint of a line or the origin (through the shared reference resolver)
        if let Some(r) = self.resolve_sketch_ref(rect, pos, si) {
            return Some(DimRef::Point(self.materialize_ref(si, r)));
        }
        None
    }

    /// A dependable unit SCREEN direction of the line a -> b. When the screen segment degenerates (a
    /// unit-long coordinate axis seen from far away puts both ends in one pixel), the direction is taken
    /// from THE MODEL (by projecting a far point of the line); otherwise a dimension to an axis comes
    /// out mangled, because its perpendicular degenerates.
    fn line_screen_dir(&self, si: usize, a: Id, b: Id, rect: Rect) -> Option<egui::Vec2> {
        let (pa, pb) = (self.sketch_pt(si, a)?, self.sketch_pt(si, b)?);
        let sa = self.to_screen(rect, pa);
        let v = self.to_screen(rect, pb) - sa;
        if v.length() > 0.5 {
            return Some(v.normalized());
        }
        let far = Point2::new(pa.x + (pb.x - pa.x) * 1e4, pa.y + (pb.y - pa.y) * 1e4);
        let d = self.to_screen(rect, far) - sa;
        (d.length() > 1e-4).then(|| d.normalized())
    }

    /// Whether the segments a1 -> b1 and a2 -> b2 are parallel (their normalised cross product is under 5%).
    fn lines_parallel(&self, si: usize, a1: Id, b1: Id, a2: Id, b2: Id) -> bool {
        match (self.sketch_pt(si, a1), self.sketch_pt(si, b1), self.sketch_pt(si, a2), self.sketch_pt(si, b2)) {
            (Some(p1), Some(p2), Some(p3), Some(p4)) => {
                let (u, v) = ((p2.x - p1.x, p2.y - p1.y), (p4.x - p3.x, p4.y - p3.y));
                let cross = u.0 * v.1 - u.1 * v.0;
                let (lu, lv) = ((u.0 * u.0 + u.1 * u.1).sqrt(), (v.0 * v.0 + v.1 * v.1).sqrt());
                cross.abs() / (lu * lv).max(1e-9) < 0.05
            }
            _ => false,
        }
    }

    /// Is the line (a, b) a coordinate axis of the sketch (one of its axis reference points)?
    fn is_axis_line(&self, si: usize, a: Id, b: Id) -> bool {
        self.project.sketches.get(si).is_some_and(|s| s.axis_pts.contains(&a) || s.axis_pts.contains(&b))
    }


    /// The base edge under a screen point (a line, a circle or an arc) — for a tangent circle.
    fn ref_edge_at(&self, rect: Rect, pos: Pos2, si: usize) -> Option<EdgeRef> {
        use qymcad_core::model::EntityKind;
        let (k, eid) = self.sketch_hit(rect, pos, si)?;
        if k != 1 {
            return None;
        }
        let s = self.project.sketches.get(si)?;
        let e = s.entities.iter().find(|e| e.id == eid)?;
        match e.kind {
            EntityKind::Line { a, b } => Some(EdgeRef::Line { a, b }),
            EntityKind::Circle { center, r } => Some(EdgeRef::Circle { center, r }),
            EntityKind::Arc { center, a, .. } => {
                let (pc, pa) = (self.sketch_pt(si, center)?, self.sketch_pt(si, a)?);
                Some(EdgeRef::Circle { center, r: ((pa.x - pc.x).powi(2) + (pa.y - pc.y).powi(2)).sqrt() })
            }
            EntityKind::Ellipse { .. } => None,
        }
    }

    /// The tangent direction at the point `p` (the end of an existing line or arc), plus the base edge.
    /// For a tangent arc: the new arc starts off smoothly from the end of the previous curve.
    fn arc_tangent_ref(&self, si: usize, p: Point2) -> Option<((f64, f64), EdgeRef)> {
        use qymcad_core::model::EntityKind;
        let s = self.project.sketches.get(si)?;
        let near = |q: Point2| (q.x - p.x).abs() < 1e-6 && (q.y - p.y).abs() < 1e-6;
        let norm = |x: f64, y: f64| {
            let l = (x * x + y * y).sqrt().max(1e-9);
            (x / l, y / l)
        };
        for e in &s.entities {
            match e.kind {
                EntityKind::Line { a, b } => {
                    if let (Some(pa), Some(pb)) = (self.sketch_pt(si, a), self.sketch_pt(si, b)) {
                        if near(pa) {
                            return Some((norm(pa.x - pb.x, pa.y - pb.y), EdgeRef::Line { a, b }));
                        }
                        if near(pb) {
                            return Some((norm(pb.x - pa.x, pb.y - pa.y), EdgeRef::Line { a, b }));
                        }
                    }
                }
                EntityKind::Arc { center, a, b, .. } => {
                    if let (Some(pc), Some(pa), Some(pb)) = (self.sketch_pt(si, center), self.sketch_pt(si, a), self.sketch_pt(si, b)) {
                        let r = ((pa.x - pc.x).powi(2) + (pa.y - pc.y).powi(2)).sqrt();
                        if near(pa) {
                            return Some((norm(-(pa.y - pc.y), pa.x - pc.x), EdgeRef::Circle { center, r }));
                        }
                        if near(pb) {
                            return Some((norm(-(pb.y - pc.y), pb.x - pc.x), EdgeRef::Circle { center, r }));
                        }
                    }
                }
                _ => {}
            }
        }
        None
    }

    /// The radius of a circle centred at `p` that touches the base edge (the perpendicular distance to a
    /// line, or |dist - R| to a circle).
    fn tangent_radius_to_edge(&self, si: usize, eref: EdgeRef, p: Point2) -> f64 {
        match eref {
            EdgeRef::Line { a, b } => match (self.sketch_pt(si, a), self.sketch_pt(si, b)) {
                (Some(pa), Some(pb)) => {
                    let (dx, dy) = (pb.x - pa.x, pb.y - pa.y);
                    let len = (dx * dx + dy * dy).sqrt().max(1e-9);
                    ((dx * (p.y - pa.y) - dy * (p.x - pa.x)) / len).abs()
                }
                _ => 0.0,
            },
            EdgeRef::Circle { center, r } => match self.sketch_pt(si, center) {
                Some(pc) => (((p.x - pc.x).powi(2) + (p.y - pc.y).powi(2)).sqrt() - r).abs(),
                None => 0.0,
            },
        }
    }

    /// Attach a tangency constraint between a new circle or arc (centre `cen`, radius `r`, at (cx, cy)) and the base edge.
    fn add_tangent_to_edge(&mut self, si: usize, eref: EdgeRef, cen: Id, cx: f64, cy: f64, r: f64) {
        use qymcad_core::model::Constraint;
        match eref {
            EdgeRef::Line { a, b } => {
                self.project.add_constraint_if_independent(si, Constraint::Tangent { a, b, c: cen, r });
            }
            EdgeRef::Circle { center, r: rr } => {
                if let Some(pc) = self.sketch_pt(si, center) {
                    let dist = ((cx - pc.x).powi(2) + (cy - pc.y).powi(2)).sqrt();
                    let external = (dist - (rr + r)).abs() <= (dist - (rr - r).abs()).abs();
                    self.project.add_constraint_if_independent(si, Constraint::CircleTangent { c1: center, c2: cen, external });
                }
            }
        }
    }

    /// Delete the current sketch selection (entities, primitives and points, apart from the system ones).
    fn delete_sketch_sel(&mut self, si: usize) {
        let eids: Vec<Id> = self.sel_sk.items.iter().filter(|(k, _)| *k == 1).map(|(_, id)| *id).collect();
        // DRIVEN GEOMETRY IS DELETED AS A WHOLE PROJECTION. One segment of a projected contour cannot be
        // removed on its own: it is not "geometry" but a view of an edge of the part — the record left
        // behind would bring it straight back.
        let doomed: Vec<Id> = {
            let s = &self.project.sketches[si];
            s.projections.iter().filter(|p| p.entities.iter().any(|e| eids.contains(e))).map(|p| p.id).collect()
        };
        for pid in doomed {
            self.project.remove_sketch_projection(si, pid);
        }
        let eids: Vec<Id> = {
            let s = &self.project.sketches[si];
            eids.into_iter().filter(|e| s.entities.iter().any(|x| x.id == *e)).collect()
        };
        let sys: std::collections::HashSet<Id> = {
            let s = &self.project.sketches[si];
            s.immovable_points().into_iter().collect() // system and driven points: they are not deleted one by one
        };
        let pids: Vec<Id> = self.sel_sk.items.iter().filter(|(k, id)| *k == 0 && !sys.contains(id)).map(|(_, id)| *id).collect();
        if !eids.is_empty() {
            self.project.delete_entities(si, &eids);
        }
        if !pids.is_empty() {
            self.project.delete_points(si, &pids);
        }
        self.project.solve_sketch(si);
        self.sel_sk.clear(); // the selection, and whatever was waiting on it
        self.invalidate();
        self.status = crate::i18n::tr("g-deleted");
    }

    /// Select all the geometry of a sketch: entities (lines, arcs, circles), primitives and free points
    /// (apart from the system ones — the origin and the axes). Used by Ctrl+A.
    fn select_all_sketch(&mut self, si: usize) {
        let Some(s) = self.project.sketches.get(si) else { return };
        let sys: std::collections::HashSet<Id> = s.system_ids().into_iter().collect();
        // points already taken by entities and primitives: they are not added as separate points
        let mut used: std::collections::HashSet<Id> = std::collections::HashSet::new();
        let mut sel: Vec<(u8, Id)> = Vec::new();
        for e in &s.entities {
            sel.push((1, e.id));
            match e.kind {
                qymcad_core::model::EntityKind::Line { a, b } => {
                    used.insert(a);
                    used.insert(b);
                }
                qymcad_core::model::EntityKind::Circle { center, .. } => {
                    used.insert(center);
                }
                qymcad_core::model::EntityKind::Arc { center, a, b, .. } => {
                    used.insert(center);
                    used.insert(a);
                    used.insert(b);
                }
                qymcad_core::model::EntityKind::Ellipse { c, ma, mi } => {
                    used.insert(c);
                    used.insert(ma);
                    used.insert(mi);
                }
            }
        }
        // lone points (not part of an entity and not system ones)
        for p in &s.points {
            if !used.contains(&p.id) && !sys.contains(&p.id) {
                sel.push((0, p.id));
            }
        }
        let n = sel.len();
        self.sel_sk.items = sel;
        self.gsel.constraint = None;
        self.annot.note = None;
        self.status = crate::i18n::tr1("g-selected-n", "n", &n.to_string());
    }

    fn sel_point_ids(&self) -> Vec<Id> {
        self.sel_sk.items.iter().filter(|(k, _)| *k == 0).map(|(_, id)| *id).collect()
    }

    fn sel_line_pts(&self, si: usize) -> Vec<(Id, Id)> {
        use qymcad_core::model::EntityKind;
        let Some(s) = self.project.sketches.get(si) else { return Vec::new() };
        self.sel_sk.items
            .iter()
            .filter(|(k, _)| *k == 1)
            .filter_map(|(_, eid)| s.entities.iter().find(|e| e.id == *eid).and_then(|e| match e.kind {
                EntityKind::Line { a, b } => Some((a, b)),
                _ => None,
            }))
            .collect()
    }

    /// The (centre, radius) of the first selected circle entity — for a tangency.
    fn sel_circle_cr(&self, si: usize) -> Option<(Id, f64)> {
        use qymcad_core::model::EntityKind;
        let s = self.project.sketches.get(si)?;
        let pos = |id: Id| s.points.iter().find(|q| q.id == id).map(|q| (q.x, q.y));
        self.sel_sk.items.iter().filter(|(k, _)| *k == 1).find_map(|(_, eid)| {
            s.entities.iter().find(|e| e.id == *eid).and_then(|e| match e.kind {
                EntityKind::Circle { center, r } => Some((center, r)),
                // an arc: the radius is the distance from the centre to an end (the tangency uses the arc's LIVE radius variable)
                EntityKind::Arc { center, a, .. } => match (pos(center), pos(a)) {
                    (Some((cx, cy)), Some((ax, ay))) => Some((center, ((ax - cx).powi(2) + (ay - cy).powi(2)).sqrt())),
                    _ => None,
                },
                _ => None,
            })
        })
    }

    fn sel_circle_centers(&self, si: usize) -> Vec<Id> {
        use qymcad_core::model::EntityKind;
        let Some(s) = self.project.sketches.get(si) else { return Vec::new() };
        self.sel_sk.items
            .iter()
            .filter(|(k, _)| *k == 1)
            .filter_map(|(_, eid)| s.entities.iter().find(|e| e.id == *eid).and_then(|e| match e.kind {
                EntityKind::Circle { center, .. } | EntityKind::Arc { center, .. } => Some(center),
                _ => None,
            }))
            .collect()
    }

    /// The radius of a curve, found by its CENTRE: a circle (r) or an arc (|centre to end|). The single
    /// source for radius and diameter dimensions — they work the same for a circle and for an arc (after
    /// trimming).
    fn center_radius(&self, si: usize, c: Id) -> Option<f64> {
        use qymcad_core::model::EntityKind;
        let s = self.project.sketches.get(si)?;
        s.entities.iter().find_map(|e| match e.kind {
            EntityKind::Circle { center, r } if center == c => Some(r),
            EntityKind::Arc { center, a, .. } if center == c => self
                .sketch_pt(si, a)
                .zip(self.sketch_pt(si, center))
                .map(|(pa, pc)| ((pa.x - pc.x).powi(2) + (pa.y - pc.y).powi(2)).sqrt()),
            _ => None,
        })
    }

    /// The centre of the curve whose PASSIVE radius or diameter label (a curve WITHOUT a Diameter
    /// constraint) is under the cursor. Returns (center, diam): a circle gives a diameter (diam = true),
    /// an arc gives a radius (diam = false). A circle's label sits in the off=0 style (to the right,
    /// beyond the rim); an arc's sits by the middle of the arc, where the passive leader draws it.
    fn passive_radius_label_at(&self, rect: Rect, pos: Pos2, si: usize) -> Option<(Id, bool)> {
        use qymcad_core::model::{Constraint, EntityKind};
        let s = self.project.sketches.get(si)?;
        for e in &s.entities {
            let (center, diam) = match e.kind {
                EntityKind::Circle { center, .. } => (center, true),
                EntityKind::Arc { center, .. } => (center, false),
                _ => continue,
            };
            if s.constraints.iter().any(|x| matches!(x, Constraint::Diameter { c, .. } if *c == center)) {
                continue;
            }
            let Some(cp) = self.sketch_pt(si, center) else { continue };
            let Some(r) = self.center_radius(si, center) else { continue };
            let sc = self.to_screen(rect, cp);
            let r_px = (self.to_screen(rect, Point2::new(cp.x + r, cp.y)) - sc).length();
            // the leader's direction: to the right for a circle (off=0), towards the middle of the arc for an arc, as it is drawn
            let dir = if diam {
                egui::vec2(1.0, 0.0)
            } else if let EntityKind::Arc { a, b, .. } = e.kind {
                match (self.sketch_pt(si, a), self.sketch_pt(si, b)) {
                    (Some(pa), Some(pb)) => {
                        let m = self.to_screen(rect, Point2::new((pa.x + pb.x) / 2.0, (pa.y + pb.y) / 2.0)) - sc;
                        if m.length() > 1e-3 { m.normalized() } else { egui::vec2(1.0, 0.0) }
                    }
                    _ => egui::vec2(1.0, 0.0),
                }
            } else {
                egui::vec2(1.0, 0.0)
            };
            let label_at = sc + dir * (r_px + 14.0);
            if label_at.distance(pos) <= 15.0 {
                return Some((center, diam));
            }
        }
        None
    }



    /// In-place editing of a note's text (a double click).
    /// Bake the glyph polylines of a text through the active font (world coordinates, baseline point x, y).
    fn bake_text_glyphs(&mut self, x: f64, y: f64, height: f64, text: &str) -> Vec<Vec<Point2>> {
        let Some(font) = self.default_font() else { return Vec::new() };
        qymcad_core::text::text_outline_contours(&font, text, height, x, y).into_iter().map(|c| c.points).collect()
    }

    /// The text object under a screen point (by the bounding box of its glyphs). For selecting, moving and editing.
    fn text_at(&self, rect: Rect, pos: Pos2, si: usize) -> Option<usize> {
        let s = self.project.sketches.get(si)?;
        for i in 0..s.texts.len() {
            if let Some((minx, miny, maxx, maxy)) = self.project.sketch_text_bbox(si, i) {
                let p0 = self.to_screen(rect, Point2::new(minx, miny));
                let p1 = self.to_screen(rect, Point2::new(maxx, maxy));
                if Rect::from_two_pos(p0, p1).expand(5.0).contains(pos) {
                    return Some(i);
                }
            }
        }
        None
    }

    /// The popup for editing a text object: the string plus the height. On apply the glyphs are re-baked and updated.
    fn text_obj_editor(&mut self, ctx: &egui::Context, rect: Rect) {
        let Sel::Sketch(si) = self.sel else {
            self.inline.clear();
            return;
        };
        let Some(ti) = self.inline.text() else { return };
        let Some((bx, _by, _, maxy)) = self.project.sketch_text_bbox(si, ti) else {
            self.inline.clear();
            return;
        };
        let at = self.to_screen(rect, Point2::new(bx, maxy));
        let (mut apply, mut close) = (false, false);
        egui::Area::new(egui::Id::new(("textedit", si, ti))).fixed_pos(self.clamp_popup(at, rect) + egui::vec2(0.0, -34.0)).order(egui::Order::Foreground).show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.horizontal(|ui| {
                    let r = ui.add(egui::TextEdit::singleline(&mut self.annot.text_buf).desired_width(160.0));
                    ui.label(&crate::i18n::tr("g-height-short"));
                    ui.add(egui::DragValue::new(&mut self.annot.text_h).speed(0.2).range(1.0..=1000.0).suffix(crate::i18n::tr("unit-mm-suffix")));
                    if (r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))) || ui.button(ph::CHECK).clicked() {
                        apply = true;
                    }
                });
            });
        });
        if apply {
            let (x, y, angle) = {
                let t = &self.project.sketches[si].texts[ti];
                (t.x, t.y, t.angle)
            };
            let (txt, h) = (self.annot.text_buf.clone(), self.annot.text_h);
            let glyphs = self.bake_text_glyphs(x, y, h, &txt);
            self.project.set_sketch_text(si, ti, x, y, h, angle, txt, glyphs);
            self.invalidate();
            close = true;
        }
        if close || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.inline.clear();
        }
    }

    fn note_editor(&mut self, ctx: &egui::Context, rect: Rect) {
        let Sel::Sketch(si) = self.sel else {
            self.inline.clear();
            return;
        };
        let Some(ni) = self.inline.note() else { return };
        let Some(at) = self.project.sketches.get(si).and_then(|s| s.notes.get(ni)).map(|n| self.to_screen(rect, Point2::new(n.x, n.y))) else {
            self.inline.clear();
            return;
        };
        let (mut apply, mut close) = (false, false);
        egui::Area::new(egui::Id::new(("noteedit", si, ni))).fixed_pos(self.clamp_popup(at, rect) + egui::vec2(0.0, -28.0)).order(egui::Order::Foreground).show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.horizontal(|ui| {
                    let r = ui.add(egui::TextEdit::singleline(&mut self.annot.note_buf).desired_width(160.0));
                    if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        apply = true;
                    }
                    if ui.button(ph::CHECK).clicked() {
                        apply = true;
                    }
                });
            });
        });
        if apply {
            if let Some(n) = self.project.sketches.get_mut(si).and_then(|s| s.notes.get_mut(ni)) {
                n.text = self.annot.note_buf.clone();
            }
            close = true;
        }
        if close || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.inline.clear();
        }
    }

    /// The TextEdit used by dimension popups: it SELECTS all the text when it gains focus (automatically,
    /// or by a click or Tab) — a new value overwrites the old one without Ctrl+A. `autofocus` is one-shot:
    /// it asks for focus on this frame. Returns the Response (for lost_focus and Enter). The same one is
    /// used in sketches, parts and assemblies.
    fn focus_edit(ui: &mut egui::Ui, text: &mut String, width: f32, hint: &str, autofocus: bool) -> egui::Response {
        let mut out = egui::TextEdit::singleline(text).desired_width(width).hint_text(hint).show(ui);
        if autofocus {
            out.response.request_focus();
        }
        if autofocus || out.response.gained_focus() {
            let end = text.chars().count();
            let range = egui::text::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(end));
            out.state.cursor.set_char_range(Some(range));
            out.state.store(ui.ctx(), out.response.id);
        }
        // ENTER RELEASES THE FOCUS.
        //
        // While the field holds it, `wants_keyboard_input` suppresses ALL the hotkeys and the command's
        // own Enter: a number is typed, Enter is pressed a second time — and nothing happens, leaving
        // the tick mark to be aimed at with the mouse. Releasing the focus gives the keyboard back to
        // the command: the next Enter applies it, and letters open the sub-modes again (re-pick the
        // contour and so on).
        if out.response.lost_focus() {
            // `lost_focus` also happens on a click elsewhere — then there is nothing to release, the focus has already gone
        } else if ui.input(|i| i.key_pressed(egui::Key::Enter)) && out.response.has_focus() {
            out.response.surrender_focus();
        }
        out.response
    }

    /// Keep the ANCHOR of a dimension input popup inside the viewport `rect`, so that with large values
    /// (where the geometry runs off the edge) the little window does not fly out of the frame. The popup
    /// is about 200x60 px, so room is left for it.
    fn clamp_popup(&self, p: Pos2, rect: Rect) -> Pos2 {
        let r = rect.shrink(4.0);
        Pos2::new(p.x.clamp(r.left(), (r.right() - 200.0).max(r.left())), p.y.clamp(r.top() + 32.0, (r.bottom() - 50.0).max(r.top() + 32.0)))
    }


    /// The index of the active sketch (while in editing mode).
    fn edit_si(&self) -> Option<usize> {
        self.sketch_ses.editing.and_then(|id| self.project.sketch_index(id))
    }

    /// The index of the sketch that defines the viewport's 2D projection: while editing it is the open
    /// sketch, otherwise (in the half-sketcher of a Part command) it is the command's profile sketch, so
    /// that the body is projected IN THE PLANE of the drawing rather than seen from above.
    fn active_2d_sketch(&self) -> Option<usize> {
        if let Some(si) = self.edit_si() {
            return Some(si);
        }
        if self.cmd.active() {
            if let Some(si) = self.cmd.sketch {
                return Some(si);
            }
            if let Sel::Sketch(si) = self.sel {
                return Some(si);
            }
        }
        None
    }

    /// Enter the editing mode of sketch `si` (the usual "open the sketch").
    fn enter_sketch_edit(&mut self, si: usize) {
        // Going from one sketch straight to another (a double click on a different sketch in the tree):
        // FINISH the current one first, so that its level comes off the `nav_stash` stack. Otherwise the
        // pushed mode_3d=false piles up and the flat view is restored on exit — 3D got stuck unrotatable
        // until the isometric button was pressed.
        if self.sketch_ses.editing.is_some() {
            self.finish_sketch_edit();
        }
        self.cancel_all_tools(); // entering a sketch CANCELS an active datum or Part command (two tools are never held at once)
        // the origin is a real fixed point from the very start: always selectable (for a coincidence or
        // a dimension from the origin), always grounding the sketch. It is created once.
        self.project.ensure_origin(si);
        if let Some(s) = self.project.sketches.get(si) {
            // remember the viewpoint and mode, to bring them back on leaving the sketch (the drill-in stack)
            self.nav_stash.push((self.cam, self.view, self.mode_3d));
            self.sketch_ses.editing = Some(s.id);
            self.workbench = Workbench::Sketch;
            self.mode_3d = false;
            self.sel = Sel::Sketch(si);
            self.sel_sk.clear(); // the selection, and whatever was waiting on it
            self.pending_import.draw_pts = None;
            self.dim.kind = 0;
        self.tool.click_op = 0;
            self.view.initialized = false; // fit the view to the sketch
            self.status = crate::i18n::tr1("g-editing-sketch", "name", &s.name);
        }
    }

    /// Create a new empty sketch and enter its editing straight away.
    fn create_new_sketch(&mut self) {
        self.create_sketch_on(qymcad_core::feature::SketchPlane::default());
    }

    /// Resolve the PLACEMENT plane (a shared step for a new sketch AND for an import).
    /// A sketch on a face of ANOTHER component's body is a LIVE external reference (top-down): an
    /// `ExternalRef` is registered and `Face(body, key)` is kept, so the sketch's frame resolves into the
    /// consumer's local space on EVERY regeneration and travels with the neighbour's face (editing the
    /// neighbour drives the part — exactly what in-context mode exists for). This used to take a ONE-OFF
    /// snapshot of the face into a fixed datum — the part stayed free, but the top-down associativity was
    /// lost silently. To break the link (freezing the geometry as a snapshot), use the part's properties
    /// and its external references.
    fn resolve_placement_plane(&mut self, plane: qymcad_core::feature::SketchPlane) -> qymcad_core::feature::SketchPlane {
        self.cmd.ref_body = None; // reset on every new placement
        if let qymcad_core::feature::SketchPlane::Face(body, key) = plane {
            let consumer = self.project.active_ctx();
            if self.project.body_owner(body).is_some_and(|bo| bo != consumer) {
                self.project.add_external_face_ref(consumer, body, key); // authorise the cross-reference (otherwise regeneration isolation blocks it)
                // the source is remembered for the session: to highlight its edges and snap to them
                self.cmd.ref_body = Some(body);
                let src = self.project.body_owner(body).and_then(|o| self.project.components.iter().find(|c| c.id == o)).map(|c| crate::i18n::name(&c.name)).unwrap_or_else(|| crate::i18n::tr("g-neighbour"));
                self.status = crate::i18n::tr1("g-sketch-on-foreign-face", "name", &src);
            }
        }
        plane
    }

    /// Create a sketch on a given plane (World, Datum or Face) and enter its editing.
    fn create_sketch_on(&mut self, plane: qymcad_core::feature::SketchPlane) -> usize {
        let plane = self.resolve_placement_plane(plane);
        let si = self.project.new_sketch(crate::i18n::tr1("g-sketch-n", "n", &(self.project.sketches.len() + 1).to_string()));
        self.project.sketches[si].plane = plane;
        let (sid, name) = (self.project.sketches[si].id, crate::i18n::name(&self.project.sketches[si].name));
        self.project.add_sketch_node(sid, name); // a sketch is a node of the timeline (owned by the active part)
        self.enter_sketch_edit(si);
        si
    }

    /// Apply a NEW plane to sketch `si` (the 2D geometry is kept and carried onto it) and rebuild the
    /// bodies built on that sketch (just as an ordinary sketch edit does, through `mark_sketch_dirty`).
    fn set_sketch_plane(&mut self, si: usize, plane: qymcad_core::feature::SketchPlane) {
        let plane = self.resolve_placement_plane(plane);
        let sid = self.project.sketches[si].id;
        self.project.sketches[si].plane = plane;
        self.project.mark_sketch_dirty(sid); // associativity: the bodies on this sketch will be rebuilt
        self.mark_dirty_for_rebuild(); // the document is marked; the scheduler does the computing
        self.picking.clear();
        self.view.initialized = false;
        self.status = crate::i18n::tr("g-sketch-moved");
    }

    /// The current context (the last one on the path) — the component being edited.
    fn current_ctx_id(&self) -> Id {
        self.active_path.last().copied().unwrap_or(self.project.root)
    }

    /// Make sure the active context path is sound: it starts at the root and holds no deleted nodes.
    fn ensure_active_path(&mut self) {
        let root = self.project.ensure_root();
        let comp_ids: std::collections::HashSet<Id> = self.project.components.iter().map(|c| c.id).collect();
        if self.active_path.first() != Some(&root) {
            self.active_path = vec![root];
        }
        self.active_path.retain(|id| *id == root || comp_ids.contains(id));
        if self.active_path.is_empty() {
            self.active_path = vec![root];
        }
    }

    /// Show or hide a component in 3D. Visibility is HIERARCHICAL: the flag is stored on the component
    /// itself (`c.visible`), and a body's visibility is computed along the chain of its owners' ticks
    /// (`component_chain_visible`). It does NOT cascade into `mesh_visible` — otherwise entering a hidden
    /// subassembly would show everything hidden despite the ticks of the child parts being on
    /// (`mesh_visible` means hiding an individual body by hand, and nothing else).
    fn set_component_visible(&mut self, cid: Id, vis: bool) {
        if let Some(c) = self.project.components.iter_mut().find(|c| c.id == cid) {
            c.visible = vis;
        }
        self.visibility_changed();
    }

    /// VISIBILITY HAS CHANGED — DROP THE CACHE OF "WHAT WE SHOW".
    ///
    /// Reported behaviour: a part is hidden, it is gone from 3D, and yet its edges and faces hang in the
    /// air and can still be picked. The visibility rules had nothing to do with it: the list of visible
    /// bodies is CACHED by the pair "rebuild plus context", and unticking a box changes neither. The
    /// cache returned yesterday's answer, and both the highlighting and the picking followed it.
    ///
    /// Called from EVERY place where visibility changes: the part's tick, the body's tick. Spread it
    /// across those places and the next tick will forget again.
    fn visibility_changed(&mut self) {
        self.regen.geom_rev = self.regen.geom_rev.wrapping_add(1);
    }

    /// Whether the chain of components from `owner` up through its parents is visible (every `visible`
    /// tick is on). The walk stops BEFORE `stop` (`stop` itself and its ancestors are not checked) — so
    /// on entering a hidden subassembly or part (the context being `stop`), its contents are shown
    /// according to the ticks of its own descendants.
    fn component_chain_visible(&self, owner: Id, stop: Option<Id>) -> bool {
        let mut cur = Some(owner);
        while let Some(id) = cur {
            if Some(id) == stop {
                break;
            }
            let Some(c) = self.project.components.iter().find(|c| c.id == id) else {
                break;
            };
            if !c.visible {
                return false;
            }
            cur = c.parent;
        }
        true
    }

    /// Enter a component (drill in): remember the camera, go one level down, fit the view.
    fn enter_component(&mut self, cid: Id) {
        if self.sketch_ses.editing.is_some() {
            self.finish_sketch_edit();
        }
        self.ensure_active_path();
        if self.current_ctx_id() == cid || !self.project.components.iter().any(|c| c.id == cid) {
            return;
        }
        self.cancel_all_tools(); // entering a component drops the active tool (nothing is carried in from the parent context)
        self.tree.search.clear(); // and the search query: it was about the PREVIOUS context, here it lies
        self.nav_stash.push((self.cam, self.view, self.mode_3d));
        self.active_path.push(cid);
        self.sel = Sel::None;
        self.view.initialized = false;
        self.cam.init = false;
        self.sync_workbench();
        let name = self.project.components.iter().find(|c| c.id == cid).map(|c| crate::i18n::name(&c.name)).unwrap_or_default();
        self.status = crate::i18n::tr1("g-context-is", "name", &name);
    }

    /// Go one level up (drill out): restore the camera.
    fn exit_context(&mut self) {
        if self.sketch_ses.editing.is_some() {
            self.finish_sketch_edit();
            return;
        }
        if self.active_path.len() <= 1 {
            return;
        }
        self.cancel_all_tools(); // leaving a component drops the active tool
        self.tree.search.clear(); // the query was about the context being left — outside it searches for the wrong thing
        self.active_path.pop();
        if let Some((cam, view, mode_3d)) = self.nav_stash.pop() {
            self.cam = cam;
            self.cam.init = true;
            self.view = view;
            self.mode_3d = mode_3d;
        }
        self.sel = Sel::None;
        self.sync_workbench();
    }

    /// Jump to an arbitrary context (the "make active" click in the tree): the path is built from the
    /// chain of ancestors. A jump does not restore the intermediate viewpoints — the view is fitted anew.
    fn set_context_to(&mut self, cid: Id) {
        if self.sketch_ses.editing.is_some() {
            self.finish_sketch_edit();
        }
        let root = self.project.ensure_root();
        let mut chain = vec![cid];
        let mut cur = cid;
        let mut guard = 0;
        while cur != root && guard < 256 {
            guard += 1;
            match self.project.components.iter().find(|c| c.id == cur).and_then(|c| c.parent) {
                Some(p) => {
                    chain.push(p);
                    cur = p;
                }
                None => break,
            }
        }
        if chain.last() != Some(&root) {
            chain.push(root);
        }
        chain.reverse();
        self.active_path = chain;
        self.nav_stash.clear();
        self.sel = Sel::None;
        self.view.initialized = false;
        self.cam.init = false;
        self.sync_workbench();
    }

    /// Go to level `i` of the path (a click on a breadcrumb), leaving one level at a time.
    fn goto_path_index(&mut self, i: usize) {
        if self.sketch_ses.editing.is_some() {
            self.finish_sketch_edit();
        }
        while self.active_path.len() > i + 1 {
            self.exit_context();
        }
    }

    /// ASK for a rebuild. In a running window it goes into a worker thread and runs behind an indicator:
    /// a boolean on a thread takes seconds, and the whole interface used to freeze for that time — the
    /// system showed "the application is not responding" and offered to kill it. In headless tests
    /// (there is no window) the rebuild runs straight away, so the result is available on the next line.
    /// Mark the model as needing a rebuild. It does NOT compute — that is done by the scheduler
    /// (`rebuild_if_dirty`) when an operation closes or once a frame. Needed where an edit did not set
    /// `dirty` itself (a change of context, the caches); such places should get fewer, not more.
    fn mark_dirty_for_rebuild(&mut self) {
        self.regen.pending = true;
        if self.edits.open.is_none() {
            self.rebuild_if_dirty();
        }
    }

    /// MARK THE CONSUMERS OF PARAMETERS WHOSE VALUES HAVE CHANGED SINCE THE LAST COMPLETED REBUILD.
    ///
    /// Computed against the `params_seen` snapshot. The snapshot is advanced by [`settle_params_seen`] —
    /// and by that alone, on the fact of a finished rebuild, whether synchronous or arriving from a thread.
    ///
    /// This loop used to stand as THREE copies, and only the synchronous branch updated the snapshot. In
    /// a live window the rebuild is asynchronous, which means the snapshot was NEVER updated: every
    /// parameter counted as changed forever, the scheduler marked the whole parametric model dirty, the
    /// frame asked for a rebuild — and round it went again. An open parametric document rebuilt WITHOUT
    /// END, and to no purpose: it has no live B-rep yet, so every such rebuild failed with "the source
    /// body has not been built" and left that error in the status line.
    fn mark_changed_params_dirty(&mut self) {
        let vars = self.project.param_map();
        let changed: Vec<String> = vars.iter().filter(|(k, v)| self.params_seen.get(*k).is_none_or(|old| (*old - **v).abs() > 1e-12)).map(|(k, _)| k.clone()).collect();
        for name in &changed {
            self.project.mark_param_dependents_dirty_for(name);
        }
    }

    /// WRITE THE SETTINGS TO A FILE — to carry them to another machine, to share them, to attach them to
    /// a bug report.
    ///
    /// The format is the same RON as in storage: a separate "export format" would have to be maintained
    /// as a second one, and it would drift from the first at the very first new setting.
    pub(crate) fn export_settings_to(&self, path: &str) -> Result<(), String> {
        let text = ron::ser::to_string_pretty(&self.set, ron::ser::PrettyConfig::default()).map_err(|e| e.to_string())?;
        std::fs::write(path, text).map_err(|e| e.to_string())
    }

    /// READ THE SETTINGS FROM A FILE and adopt them.
    ///
    /// Missing fields fall back to the factory ones (`serde(default)` on the record): a profile taken
    /// from a different version of the program must still read, not be rejected wholesale — otherwise it
    /// cannot be shared. A broken file does NOT touch the current settings: the change arrives whole or
    /// not at all.
    pub(crate) fn import_settings_from(&mut self, path: &str, ctx: &egui::Context) -> Result<(), String> {
        let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let s: Settings = ron::from_str(&text).map_err(|e| e.to_string())?;
        self.adopt_settings(s, ctx);
        Ok(())
    }

    /// ADOPT A WHOLE SET OF SETTINGS — the only path by which they take effect.
    ///
    /// The record alone is not enough: the theme, the language and the interface scale live not only in
    /// it but also in `egui`'s state, which is not remembered between runs. While this was done as a
    /// list of calls at start-up, the second path (importing a profile) would have had to repeat that
    /// same list — and would have drifted from it at the very first new setting, silently at that: the
    /// record is right while the screen shows the old thing.
    pub(crate) fn adopt_settings(&mut self, s: Settings, ctx: &egui::Context) {
        self.set = s;
        // CUSTOM SCHEMES COME FROM DISK BEFORE THE THEME IS APPLIED: otherwise the chosen custom scheme
        // will not be found in the list and will silently fall back to the dark one.
        self.reload_schemes();
        self.apply_theme(ctx);
        self.apply_language();
        self.apply_ui_scale(ctx);
        self.invalidate(); // the colours and the scale are part of the picture caches' keys
    }

    /// ASK THE REBUILD TO STOP.
    ///
    /// To ask, precisely: the thread will reach the boundary of the next node and come back by itself.
    /// There is nothing to kill it with in the middle of an OCCT boolean, and no reason to — a COPY is
    /// what is being computed, and the document on screen is intact. `finish_regen_checked` sums it up
    /// when the result arrives marked as cancelled.
    fn cancel_regen(&mut self) {
        if let Some(p) = self.regen.busy.as_ref().and_then(|b| b.pulse.as_ref()) {
            p.ask_stop();
            self.status = crate::i18n::tr("io-rebuild-cancelling");
        }
    }

    /// REMEMBER THE PARAMETER VALUES ON THE FACT OF A FINISHED REBUILD.
    ///
    /// The snapshot is taken AFTER it, not before: named dimensions are derived from the sketch
    /// geometry, and the sketches are solved by the rebuild itself (`settle_sketches`). A "before"
    /// snapshot would drift from the result for no reason at all, and the next frame would declare the
    /// parameter changed again.
    fn settle_params_seen(&mut self) {
        self.params_seen = self.project.param_map();
    }

    /// THE SINGLE REBUILD SCHEDULER.
    ///
    /// The UI does not rebuild the model — it CHANGES THE DOCUMENT. The document knows on its own that
    /// it is dirty (the `dirty` flags on the timeline nodes are set by `Project` methods), and deciding
    /// whether it is time to compute, and where — here or in a thread — must be one place's job. That is
    /// how professional CAD works: an edit marks the tree, and the engine carries out the regeneration
    /// itself.
    ///
    /// Called from two points and from those alone: when an operation closes (an edit made by hand) and
    /// once a frame (whatever was marked not by hand but by the system — a background task arriving,
    /// the B-rep being loaded in).
    fn rebuild_if_dirty(&mut self) {
        if self.edits.open.is_some() {
            return; // an operation is under way — its closing will sum it up (one rebuild per action)
        }
        self.mark_changed_params_dirty();
        let asked = std::mem::take(&mut self.regen.pending);
        // THE REBUILD WAS STOPPED BY HAND — it does not start again on its own. An explicit request
        // (`asked`) clears the mark: "Rebuild everything" and any edit of the document both mean
        // "compute again".
        if self.regen.paused && !asked {
            return;
        }
        self.regen.paused = false;
        if !asked && !self.project.timeline.iter().any(|n| n.dirty) {
            self.edits.committed_key = self.doc_key(); // the "dirty" marks set above are derived too
            return; // the document is clean and nobody asked — there is nothing to compute
        }
        // NOTHING HAS CHANGED SINCE THE LAST REBUILD — THERE IS NOTHING TO COMPUTE, EVEN WITH DIRTY NODES.
        //
        // A node that failed to build stays dirty DELIBERATELY: the attempt must happen again once its
        // input appears (a live B-rep after the file is opened). But the scheduler read "dirty" as
        // "compute now" — and took it on EVERY frame. Reported behaviour: the rebuild window flickers
        // wildly, twenty frames a second, saying "feature 0 of 0". A single red feature made the program
        // unusable.
        //
        // So what is asked is not the mark but THREE things at once: whether the document changed,
        // whether the set of dirty nodes changed, and whether a new live B-rep has appeared. Any one of
        // them is a reason to compute; none of them means there is nothing to repeat, the inputs are the
        // same.
        //
        // WE ASK ABOUT THE TIMELINE, NOT ABOUT THE WHOLE DOCUMENT (`rebuild_key`, not `doc_key`). The
        // full document key used to stand here, and it includes the PLACEMENT — where the components
        // stand. Driving a part along a degree of freedom moves it every frame while rebuilding no body
        // at all, and the scheduler read every frame of the drag as "the document has moved". Reported
        // behaviour: while a joint is being moved, the modal rebuild window flickers endlessly, saying
        // "parts 0 of 0". Measured along that path: eight steps of the drag, eight rebuild requests.
        let now = (self.project.rebuild_key(), self.project.timeline.iter().filter(|n| n.dirty).map(|n| n.id).collect::<Vec<_>>(), self.live.shapes.len());
        if !asked && now == self.regen.last {
            return;
        }
        self.regenerate_all();
        self.regen.last = (self.project.rebuild_key(), self.project.timeline.iter().filter(|n| n.dirty).map(|n| n.id).collect(), self.live.shapes.len());
        // A NODE THAT NEEDS A LIVE B-rep IS EXACTLY WHAT "ON DEMAND" MEANS.
        //
        // After a file is opened no body has a live B-rep: the geometry comes from the bundle, and the
        // kernel is raised lazily. For nodes built from a recipe that is enough; for those that need the
        // source's B-rep (thickening a sheet, replacing a face, trimming) it is not, and they were stuck
        // on "the source has not been built" FOREVER: there was nobody to raise the B-rep, and the
        // scheduler only computes on changes. This showed up as a red node that nothing would cure.
        //
        // The demand for a B-rep is now visible from the refusal itself: it is temporary (`retryable`),
        // so asking the kernel makes sense. We ask once — after that `ensure_brep` does not repeat itself.
        if !self.live.ready && self.project.regen_errors.values().any(|e| e.retryable()) {
            self.ensure_brep();
        }
        // ORDER MATTERS: the snapshot of "where we stopped" is written BEFORE the kernel is asked.
        // Otherwise it would remember nodes already marked for rebuilding, the scheduler would see no
        // changes and would not compute — that is, the very guard against endless computing would
        // suppress a legitimate repeat.
        // A REBUILD IS NOT AN EDIT MADE BY HAND. It touches the document with authority: it clears the
        // "dirty" marks, writes meshes and faces, assigns names. The guard on operation boundaries has
        // to account for that, otherwise it declares the scheduler's own work an edit made outside
        // `App::edit` — which is exactly why the application panicked on the FIRST frame: at start-up no
        // parameter has been seen yet, every dependent node was marked dirty, and the document key
        // changed before a single operation had taken place.
        self.edits.committed_key = self.doc_key();
    }

    fn regenerate_all(&mut self) {
        // ONE REBUILD PER ACTION. Inside an operation it is asked for several times (the command touches
        // the sketch, then the timeline, then the caches) — but the action is one, so the rebuild is one
        // too: the request is accumulated and carried out when the operation closes. Every request used
        // to recompute the model from scratch.
        if self.edits.open.is_some() {
            self.regen.pending = true;
            return;
        }
        if self.regen.ui_running {
            self.regen.wanted = true;
            return;
        }
        self.regenerate_now();
    }



    /// WHAT IS LEFT TO RAISE — the set of bodies with no live shape, as a fingerprint.
    ///
    /// It answers "we have tried this already and nothing came of it". The preparation asks exactly one
    /// question: IS THERE ANYTHING TO DO. So what must be compared is THE WORK, not the state of the
    /// document around it.
    ///
    /// THE COST OF THE PREVIOUS KEY WAS ESTABLISHED BY MEASUREMENT IN A LIVE WINDOW. It used to be the
    /// picture revision, then the state of the timeline; the rebuild MOVES BOTH ITSELF (topological
    /// naming rewrites the node names). The result was an endless round: the preparation asks for a
    /// rebuild, the rebuild arrives with a plan of "0 nodes" and changes nothing, yet it touches the
    /// document — the key differs, the guard lets it through, the preparation asks again. Reported
    /// behaviour: pressing "Rebuild everything" sent the CAD into a fever of endless flickering. The
    /// program printed "live shapes 0" on every round.
    ///
    /// A set of unfinished work cannot behave that way: if no body was raised, the set is the same and
    /// there is no second round; if even one was, the set changed and the attempt legitimately repeats.
    pub(super) fn brep_input_key(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        let mut missing: Vec<Id> = self.project.timeline.iter().filter_map(|n| n.kind.body()).filter(|b| !self.live.shapes.contains_key(b)).collect();
        missing.sort_unstable();
        missing.hash(&mut h);
        h.finish()
    }

    /// Sum up the B-rep preparation ON THE FACT of a rebuild having happened (synchronous or arrived
    /// from a thread). `was_clean` says whether the document was clean before the preparation.
    fn settle_brep_wait(&mut self, was_clean: bool) {
        self.live.tried_rev = Some(self.brep_input_key());
        // the flag follows THE FACT. If a body is left without a B-rep (an import waiting to be restored
        // from the embedded STEP), the cache is NOT ready, and the next attempt will happen once new data
        // appears.
        self.live.ready = self.project.timeline.iter().filter_map(|n| n.kind.body()).all(|b| self.live.shapes.contains_key(&b));
        self.fill_model_edges_for_anchors(); // anchors on edges are dead after opening otherwise (see io_jobs)
        if was_clean {
            self.edits.saved_key = self.edit_key(); // rebuilding the cache is not an edit made by hand
        }
    }

    /// After a DATUM is edited (its coordinates or its definition): datums are resolved unconditionally
    /// during regeneration, but their consumers (bodies on sketches that sit on a datum plane, axes
    /// through points) have to be rebuilt, otherwise they stay where they were.
    /// A FORCED regeneration of the whole document used to stand here, on the grounds that datums are
    /// rarely edited — on an assembly of a thousand bodies that is the same tens of seconds of freeze.
    /// Now only the nodes that depend on datums are marked dirty.
    fn regen_after_datum_change(&mut self) {
        self.ensure_brep(); // the kernel rebuilds the consumers' geometry, so a live B-rep is needed
        self.project.mark_datum_consumers_dirty();
        self.mark_dirty_for_rebuild(); // the document is marked; the scheduler does the computing
    }

    /// The axis of the active sketch-based command (extrude, cut and so on): the centroid of the
    /// profiles in world space, the normal, and the length.
    fn feat_cmd_axis(&self) -> Option<([f64; 3], [f64; 3], f64)> {
        let si = self.cmd.sketch?;
        let f = self.project.sketch_frame(si)?;
        let mut polys: Vec<Vec<f64>> = Vec::new();
        for cid in &self.gsel.profiles {
            if let Some(xy) = self.project.contour_profile_xy(*cid) {
                polys.push(xy);
            }
        }
        if polys.is_empty() {
            if let Some((_, xy)) = self.project.sketch_profile_xy(si) {
                polys.push(xy);
            }
        }
        let (mut sx, mut sy, mut cnt) = (0.0_f64, 0.0_f64, 0.0_f64);
        for xy in &polys {
            for k in 0..xy.len() / 2 {
                sx += xy[2 * k];
                sy += xy[2 * k + 1];
                cnt += 1.0;
            }
        }
        if cnt == 0.0 {
            return None;
        }
        let base = f.lift(Point2::new(sx / cnt, sy / cnt));
        Some(([base.x, base.y, base.z], f.normal(), self.cmd_val("height")))
    }


    /// The world contours of the loft's sections (in `loft_sids` order), each a closed loop of world points.
    fn loft_preview(&self) -> Vec<Vec<[f64; 3]>> {
        let mut out = Vec::with_capacity(self.loft.sids.len());
        for (i, &sid) in self.loft.sids.iter().enumerate() {
            let cid = self.project.loft_section_contour(sid, self.loft.cids.get(i).copied().unwrap_or(0));
            let (Some(cid), Some(pf)) = (cid, self.project.sketch_frame_by_id(sid)) else { continue };
            let Some(idx) = self.project.contour_index(cid) else { continue };
            let c = &self.project.contours[idx];
            if !c.closed || c.points.len() < 3 {
                continue;
            }
            let loop_w: Vec<[f64; 3]> = c
                .points
                .iter()
                .map(|p| {
                    let w = pf.lift(*p);
                    [w.x, w.y, w.z]
                })
                .collect();
            out.push(loop_w);
        }
        out
    }

    /// The screen anchor of the active command (where to stick the input field): for extrude and revolve
    /// it is at the tip of the arrow; for a fillet or a chamfer at the first selected edge; for a shell
    /// or a hole at the centre of the face.
    /// The screen anchor of a command popup TO THE SIDE of body `b`: the top-right corner of its
    /// projected bounding box, plus a margin. The popup used to be anchored at the CENTRE of the box, so
    /// it covered the part and what was needed could not be picked behind it. Now it grows to the right
    /// of the body rather than on top of it.
    fn body_side_anchor(&self, b: Id, rect: Rect, basis: &([f64; 3], [f64; 3], [f64; 3])) -> Option<Pos2> {
        let mi = self.project.mesh_index(b)?;
        let bb = self.project.bodies[mi].mesh.bounds()?;
        let (mut sx1, mut sy0) = (f32::MIN, f32::MAX);
        for &x in &[bb.min.x, bb.max.x] {
            for &y in &[bb.min.y, bb.max.y] {
                for &z in &[bb.min.z, bb.max.z] {
                    let p = self.project3([x, y, z], rect, basis).0;
                    sx1 = sx1.max(p.x);
                    sy0 = sy0.min(p.y);
                }
            }
        }
        (sx1 > f32::MIN).then_some(Pos2::new(sx1 + 14.0, sy0))
    }

    /// The TARGET body of a body operation (chamfer, fillet, shell, hole, pattern) — the body explicitly
    /// SELECTED, or, if none is, the part's SINGLE body (`active_body`). A part is one body, so pressing
    /// a tool button gets straight to work on that single body, without a "select a body first" step.
    /// Call the live B-rep preparation directly — for a check that looks at the program.
    /// "Rebuild everything" — for a check that looks at the program rather than at the harness.
    #[cfg(test)]
    pub(super) fn rebuild_everything_for_test(&mut self) {
        self.rebuild_everything();
    }

    /// Whether restoring the imports was called — "Rebuild everything" must call it.
    #[cfg(test)]
    pub(super) fn import_shapes_asked_for_test(&self) -> bool {
        self.regen.import_asked
    }

    /// Whether the preparation of the live geometry counts as finished.
    #[cfg(test)]
    pub(super) fn brep_ready_for_test(&self) -> bool {
        self.live.ready
    }

    #[cfg(test)]
    pub(super) fn ensure_brep_for_test(&mut self) {
        self.ensure_brep();
    }

    /// Whether a body has a live B-rep — for checks that look at the program rather than at the harness.
    #[cfg(test)]
    pub(super) fn has_shape_for_test(&self, body: Id) -> bool {
        self.live.shapes.contains_key(&body)
    }

    fn op_target_body(&self) -> Option<Id> {
        self.selected_body().or_else(|| self.current_body())
    }


    /// Whether to show a body (by mesh index) in the current context: the visibility tick AND the part's
    /// isolation — inside a Part ONLY its own bodies are visible (each part is its own space and frame);
    /// in the root assembly all of them are.
    fn body_shown(&self, mi: usize) -> bool {
        // THE BODY OF A RED NODE IS NOT SHOWN. A node that failed to build still has a body: it is created
        // together with the node and, until the first successful build, carries the source's mesh. Showing
        // it means drawing the part's ghost right next to it — two bodies on screen although the operation
        // failed. Caught by the fuzzer: a fillet applied to a surface goes red, and the part ends up with
        // two visible bodies.
        //
        // An empty body is hidden for the same reason: there is nothing to draw in it, yet it counts in
        // the lists of "what we show".
        if let Some(b) = self.project.bodies.get(mi) {
            if b.mesh.tris.is_empty() {
                return false;
            }
            // THE BODY OF A RED NODE IS A GHOST. Only that body ITSELF is hidden: the source must stay on
            // screen, otherwise a failed operation wipes out the whole part (its body having been consumed
            // by the node that failed to build). That is the rule in full: on failure we show what was
            // there BEFORE, not emptiness and not two bodies at once.
            let red = |id: Id| self.project.timeline.iter().any(|n| n.kind.bodies().contains(&id) && self.project.regen_errors.contains_key(&n.id));
            if red(b.id) {
                return false;
            }
            if self.body_is_consumed(b.id) && self.project.timeline.iter().any(|n| n.kind.consumed().contains(&b.id) && self.project.regen_errors.contains_key(&n.id)) {
                return true; // the consuming node is red, so the source stays visible
            }
        }
        let ctx = self.current_ctx_id();
        // A CONSUMED BODY IS NOT A BODY BUT A STEP OF HISTORY. A part is one body, and every operation
        // carries it along: an extrude, a cut, a fillet produce a NEW body and consume the previous one.
        // The previous ones stayed "visible": they were only out of sight because the resulting body
        // covers them. They were still drawn and still walked through while picking — and the cost grew
        // with the length of the timeline rather than with the number of parts. Hence the freeze while
        // picking an edge anchor: on a part with twenty operations, twenty copies of the same shape were
        // walked through.
        //
        // THERE IS ONE EXCEPTION — THE SOURCE OF THE FEATURE BEING EDITED. While editing a modifier, one
        // looks at the state BEFORE it: the feature's result and its descendant chain are hidden, and the
        // consumed source is shown — otherwise there is nothing to click on and nothing to edit. The
        // exception used to stand LOWER, in the list of visible bodies, and was dead code: control
        // reached this point first and threw the source away as consumed. It showed up as pressing Edit
        // on a fillet and the part disappearing entirely.
        if self.project.mesh_id(mi).is_some_and(|b| self.body_is_consumed(b) && Some(b) != self.edit_src_body()) {
            return false;
        }
        let owner = self.project.mesh_id(mi).and_then(|b| self.project.body_owner(b));
        // Hiding an INDIVIDUAL body by hand (the body's tick in the tree) always applies, whatever the context.
        if !self.project.bodies.get(mi).is_none_or(|b| b.visible) {
            return false;
        }
        let Some(owner) = owner else {
            return ctx == self.project.root; // a body with no owner is visible only in the root assembly
        };
        // The root assembly: visibility follows the chain of component ticks all the way to the root.
        if ctx == self.project.root {
            return self.component_chain_visible(owner, None);
        }
        // Inside a part, its own bodies; inside a subassembly, the bodies of all its parts (the subtree).
        // The ticks of the child parts hide their bodies, while the context component ITSELF (`stop`) is
        // excluded — on entering a hidden subassembly we see its contents according to its descendants' ticks.
        if self.project.component_is_within(owner, ctx) {
            return self.component_chain_visible(owner, Some(ctx));
        }
        // IN THE SKETCHER (while editing a sketch) the 3D bodies of in-context neighbours are NOT drawn at
        // all: they get in the way of building the part. The face being built on is shown by highlighting
        // its edges (`draw_sketch_face_edges`), not as a body.
        if self.sketch_ses.editing.is_some() {
            return false;
        }
        // In-context mode: the bodies of NEIGHBOURING parts (in the parent assembly) are shown as ghosts,
        // so that their geometry can be referred to (top-down). Otherwise isolation hides them.
        if self.win.context {
            if let Some(parent) = self.project.components.iter().find(|c| c.id == ctx).and_then(|c| c.parent) {
                if self.project.component_is_within(owner, parent) {
                    return self.component_chain_visible(owner, Some(parent));
                }
            }
        }
        false
    }

    /// The display transform of a datum (a point, an axis or a plane, by its Id) in the active context's
    /// frame — so that a part's datums travel with it in an assembly, just as its bodies do. None means
    /// another component's datum (isolation: we do not draw it).
    fn datum_render_transform(&self, datum_id: Id) -> Option<[f64; 12]> {
        use qymcad_core::feature::FeatureKind as FK;
        // the visibility tick in the tree (by a stable Id): a hidden datum is drawn nowhere
        if self.datum.hidden.contains(&datum_id) {
            return None;
        }
        let owner = self
            .project
            .timeline
            .iter()
            .find(|n| matches!(n.kind, FK::DatumPoint { point } if point == datum_id) || matches!(n.kind, FK::DatumAxis { axis } if axis == datum_id) || matches!(n.kind, FK::Plane { plane } if plane == datum_id))
            .and_then(|n| n.parent)?;
        // A datum is visible ONLY when the visibility context is the owner itself or one of its
        // descendants (we are "inside" the owner). So a Part's datums are NOT shown in the assembly or at
        // the root (owner is the Part, viz is the Assembly, and viz is not inside owner), but they appear
        // as soon as one enters the part. An ancestor assembly's datums, meanwhile, stay visible from a
        // nested part (ancestor reference geometry). `viz_ctx_id` accounts for sketch editing — the datums
        // of the sketch's owner are visible even when it was entered by a double click from the root.
        let viz = self.viz_ctx_id();
        if !self.project.component_is_within(viz, owner) {
            return None; // another component's datum, or a part's datum seen from the assembly: not shown
        }
        // An ANCESTOR's datum (the owner being a strict ancestor of the context: a plane or an axis of the
        // ASSEMBLY, seen from a nested subassembly or part) is shown ONLY with in-context mode on. The
        // context's own datums are always visible. This keeps the parent's reference geometry from
        // cluttering the part by default.
        if owner != viz && !self.win.context {
            return None;
        }
        // the placement is RELATIVE to the active context, just as bodies are, so that the datum travels with the part
        Some(self.project.relative_transform(owner, self.current_ctx_id()))
    }

    /// The body is shown only as CONTEXT (a neighbouring part while `show_context` is on) rather than as
    /// the active part, so it is drawn as a ghost and not picked as one's own (edit-in-context).
    fn body_is_ghost(&self, mi: usize) -> bool {
        let ctx = self.current_ctx_id();
        if ctx == self.project.root {
            return false;
        }
        match self.project.mesh_id(mi).and_then(|b| self.project.body_owner(b)) {
            Some(owner) => !self.project.component_is_within(owner, ctx),
            None => false,
        }
    }

    /// The VISIBILITY context: while a sketch is being edited it is the sketch's owner component (so
    /// isolation works however the sketch was entered, even by a double click from the root); otherwise
    /// it is the active context. `active_path` is NOT touched — that keeps the drill-in/drill-out stack
    /// invariant (every push of the path is paired with one on `nav_stash`).
    fn viz_ctx_id(&self) -> Id {
        if let Some(sid) = self.sketch_ses.editing {
            if let Some(owner) = self.project.sketch_owner(sid) {
                return owner;
            }
        }
        self.current_ctx_id()
    }

    /// The contours of sketches that do NOT belong to the active context (its subtree) — they are hidden
    /// both in the 2D sketcher and in the 3D overlay (one source for both drawing loops).
    pub(super) fn foreign_contour_ids(&self) -> std::collections::HashSet<Id> {
        self.project
            .sketches
            .iter()
            .filter(|s| !self.sketch_in_ctx(s.id))
            .flat_map(|s| s.contour_ids.iter().copied())
            .collect()
    }

    /// The Id of a body by its mesh index, IF it is a B-rep body (a live shape exists) — otherwise None (a raw import).
    fn brep_at(&self, mi: usize) -> Option<Id> {
        self.project.mesh_id(mi).filter(|id| self.live.shapes.contains_key(id))
    }

    /// A rigid move of a body (mesh index `mi`) by the matrix `mat` (3x4): a B-rep gets a PARAMETRIC
    /// `Move` feature (which moves the shape), a raw imported mesh is transformed directly. This keeps the
    /// mesh panel from putting a B-rep body out of step with its shape.
    fn move_body_at(&mut self, mi: usize, mat: [f64; 12]) {
        self.begin_edit(&crate::i18n::tr("status-move-body")); // THE OPERATION BOUNDARY
        if let Some(id) = self.brep_at(mi) {
            let nb = self.project.add_move(id, mat);
            self.mark_dirty_for_rebuild(); // the document is marked; the scheduler does the computing
            self.select_body(nb);
        } else {
            self.project.bodies[mi].mesh.transform(&mat);
            self.project.bodies[mi].faces = self.project.bodies[mi].mesh.detect_faces(8.0);
            self.invalidate();
        }
            self.commit_edit();
    }


    /// The Id of the selected body (through the part or through the feature).
    /// A test facade: a test must ask about the selection by the same path the tools use.
    #[cfg(test)]
    pub(crate) fn selected_body_for_test(&self) -> Option<Id> {
        self.selected_body()
    }

    fn selected_body(&self) -> Option<Id> {
        match self.sel {
            Sel::Mesh(mi) => self.project.mesh_id(mi),
            Sel::Feature(ti) => self.project.timeline.get(ti).and_then(|n| n.kind.body()),
            _ => None,
        }
    }

    /// The EFFECTIVE section plane: the base one, plus the tilts (deg about U and V), plus the shift along the normal.
    fn section_eff(&self) -> Option<([f64; 3], [f64; 3])> {
        let (o, n0) = self.section.plane?;
        let nn = v_norm(n0);
        let a = if nn[0].abs() < 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
        let u = v_norm(v_cross(a, nn));
        let v = v_cross(nn, u);
        let rot = |vec: [f64; 3], axis: [f64; 3], deg: f64| -> [f64; 3] {
            let (s, c) = deg.to_radians().sin_cos();
            let cr = v_cross(axis, vec);
            let d = v_dot(axis, vec);
            [
                vec[0] * c + cr[0] * s + axis[0] * d * (1.0 - c),
                vec[1] * c + cr[1] * s + axis[1] * d * (1.0 - c),
                vec[2] * c + cr[2] * s + axis[2] * d * (1.0 - c),
            ]
        };
        let n = v_norm(rot(rot(nn, u, self.section.rot[0]), v, self.section.rot[1]));
        let o = [o[0] + n[0] * self.section.offset, o[1] + n[1] * self.section.offset, o[2] + n[2] * self.section.offset];
        Some((o, n))
    }

    /// Is the point HIDDEN by the section? (the side along the normal is hidden; "Flip" changes the normal's sign)
    fn section_hidden(&self, p: [f64; 3]) -> bool {
        match self.section_eff() {
            Some((o, n)) => (p[0] - o[0]) * n[0] + (p[1] - o[1]) * n[1] + (p[2] - o[2]) * n[2] > 0.0,
            None => false,
        }
    }

    /// The triangle is hidden by the section (by its centroid — for PICKING; the renderer cuts it properly by clipping).
    fn section_tri_hidden(&self, a: [f64; 3], b: [f64; 3], c: [f64; 3]) -> bool {
        self.section.plane.is_some() && self.section_hidden([(a[0] + b[0] + c[0]) / 3.0, (a[1] + b[1] + c[1]) / 3.0, (a[2] + b[2] + c[2]) / 3.0])
    }

    /// A PROPER clip of a triangle by the section plane (Sutherland-Hodgman): the visible side (dist <= 0)
    /// is cut EXACTLY along the plane, giving 0 to 2 triangles. With no section, the original triangle
    /// comes back. A resulting vertex is (position, source weights [w0, w1, w2]) — for interpolating the
    /// colour on the GPU.
    fn section_clip_tri(&self, v: [[f64; 3]; 3]) -> smallvec_tris::ClipTris {
        use smallvec_tris::*;
        let Some((o, n)) = self.section_eff() else {
            return ClipTris::whole();
        };
        let d = [
            (v[0][0] - o[0]) * n[0] + (v[0][1] - o[1]) * n[1] + (v[0][2] - o[2]) * n[2],
            (v[1][0] - o[0]) * n[0] + (v[1][1] - o[1]) * n[1] + (v[1][2] - o[2]) * n[2],
            (v[2][0] - o[0]) * n[0] + (v[2][1] - o[1]) * n[1] + (v[2][2] - o[2]) * n[2],
        ];
        clip_by_dists(v, d)
    }

    /// The geometry of the section GIZMO: the centre of the quad on the plane, u, v, the half-size, and the arrow's tip.
    fn section_gizmo_geom(&self) -> Option<([f64; 3], [f64; 3], [f64; 3], f64, [f64; 3])> {
        let (o, n) = self.section_eff()?;
        // the bounding box of the visible scene, in world space
        let mut lo = [f64::MAX; 3];
        let mut hi = [f64::MIN; 3];
        for (_, _, _, _, mesh, wt) in self.visible_mesh_items() {
            if let Some(b) = mesh.bounds() {
                for c in [[b.min.x, b.min.y, b.min.z], [b.max.x, b.max.y, b.max.z]] {
                    let w = qymcad_core::feature::apply12(&wt, c);
                    for k in 0..3 {
                        lo[k] = lo[k].min(w[k]);
                        hi[k] = hi[k].max(w[k]);
                    }
                }
            }
        }
        if lo[0] > hi[0] {
            return None;
        }
        let c = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, (lo[2] + hi[2]) / 2.0];
        let d = (c[0] - o[0]) * n[0] + (c[1] - o[1]) * n[1] + (c[2] - o[2]) * n[2];
        let cp = [c[0] - d * n[0], c[1] - d * n[1], c[2] - d * n[2]]; // the scene's centre projected onto the plane
        let diag = ((hi[0] - lo[0]).powi(2) + (hi[1] - lo[1]).powi(2) + (hi[2] - lo[2]).powi(2)).sqrt().max(1.0);
        let a = if n[0].abs() < 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
        let u = v_norm(v_cross(a, n));
        let v = v_cross(n, u);
        let half = diag * 0.55;
        let tip = [cp[0] + n[0] * diag * 0.35, cp[1] + n[1] * diag * 0.35, cp[2] + n[2] * diag * 0.35];
        Some((cp, u, v, half, tip))
    }

    /// Whether the PLANE crosses the world bounding box of mesh `mi`. A cheap rejection before the
    /// expensive boolean for the section cap: a body lying entirely on one side has no cap by definition.
    fn mesh_crosses_plane(&self, mi: usize, o: [f64; 3], n: [f64; 3]) -> bool {
        let Some(b) = self.mesh_world_bounds(mi) else { return true }; // the box is unknown, so assume it cuts
        // the box's support point along the normal: if the WHOLE box is on one side, there is no intersection
        let (mut lo, mut hi) = (0.0, 0.0);
        for k in 0..3 {
            let (a, b2) = ((b.0[k] - o[k]) * n[k], (b.1[k] - o[k]) * n[k]);
            lo += a.min(b2);
            hi += a.max(b2);
        }
        lo <= 0.0 && hi >= 0.0
    }

    /// The world bounding box of mesh `mi` (cached by `geom_rev` — computed once per change of the scene).
    fn mesh_world_bounds(&self, mi: usize) -> Option<([f64; 3], [f64; 3])> {
        // the cache key accounts for VISIBILITY too (the map is built from the visible bodies): the key
        // used to be `geom_rev` alone, and a hidden body switched back on missed the cache.
        let key = {
            use std::hash::{Hash, Hasher};
            let mut h = std::collections::hash_map::DefaultHasher::new();
            // `geom_rev` IS IN ITS RIGHT PLACE HERE — and this is not the same thing as the document key.
            // A bounding box is derived from THE MESHES: rebuild a body and the old box lies, while a cache
            // miss is silent (the section cap simply does not get drawn). The drawing revision was taken out
            // of "is there anything unsaved" precisely because there it answered the wrong question; in a
            // picture cache it is exactly the right one.
            self.view_rev().hash(&mut h);
            for b in &self.project.bodies {
                b.visible.hash(&mut h);
            }
            h.finish()
        };
        {
            let c = self.cache.mesh_bounds.borrow();
            if c.0 == key {
                return c.1.get(&mi).copied();
            }
        }
        let mut map: std::collections::HashMap<usize, ([f64; 3], [f64; 3])> = std::collections::HashMap::new();
        for (i, _, _, _, _, wt) in self.visible_mesh_items() {
            let Some(m) = self.project.bodies.get(i).map(|b| &b.mesh) else { continue };
            let Some(bb) = m.bounds() else { continue };
            // the 8 corners of the local box taken into world space (wt is rigid), giving a world AABB
            let (mut mn, mut mx) = ([f64::MAX; 3], [f64::MIN; 3]);
            for cx in [bb.min.x, bb.max.x] {
                for cy in [bb.min.y, bb.max.y] {
                    for cz in [bb.min.z, bb.max.z] {
                        let p = qymcad_core::feature::apply12(&wt, [cx, cy, cz]);
                        for k in 0..3 {
                            mn[k] = mn[k].min(p[k]);
                            mx[k] = mx[k].max(p[k]);
                        }
                    }
                }
            }
            map.insert(i, (mn, mx));
        }
        let out = map.get(&mi).copied();
        *self.cache.mesh_bounds.borrow_mut() = (key, map);
        out
    }


    fn current_body(&self) -> Option<Id> {
        // THE CURRENT part: the last unconsumed body of THIS context (not the globally last one — otherwise
        // a cut in one part would catch hold of another part's body, and the second part appeared to
        // vanish). The logic lives in the core, under test.
        // A FALLBACK ON THE DOCUMENT'S ACTIVE COMPONENT: the GUI navigation (`active_path`) may stand at the
        // root when the part was never "entered" by a double click — the commands must still see the active
        // part's body, otherwise the part looks empty and new bodies start multiplying.
        self.project.active_body(self.current_ctx_id()).or_else(|| self.project.active_body(self.project.current_ctx()))
    }

    /// The bodies consumed by modifier features (hidden, so that only the result is seen). A delegate to
    /// the core (`Project::consumed_bodies`) — one source for the core and for the UI.
    fn consumed_bodies(&self) -> std::collections::HashSet<Id> {
        self.project.consumed_bodies()
    }

    /// Whether a body is consumed — cached, because it is asked for every body on every frame.
    fn body_is_consumed(&self, body: Id) -> bool {
        let mut c = self.cache.consumed.borrow_mut();
        if c.0 != self.view_rev() {
            c.0 = self.view_rev();
            c.1 = self.project.consumed_bodies();
        }
        c.1.contains(&body)
    }

    /// Select the feature that produced the body (so its parameters show up at once) and fit the view.
    fn select_body(&mut self, body: Id) {
        if let Some(ti) = self.project.timeline.iter().position(|n| n.kind.body() == Some(body)) {
            self.sel = Sel::Feature(ti);
        } else if let Some(mi) = self.project.mesh_index(body) {
            self.sel = Sel::Mesh(mi);
        }
        self.view.initialized = false;
    }

    /// The SOURCE body consumed by the feature being edited: while editing it is shown, so its face and edges are visible.
    fn edit_src_body(&self) -> Option<Id> {
        let fid = self.cmd.edit?;
        self.project.timeline.iter().find(|n| n.id == fid).and_then(|n| n.kind.consumed_body())
    }

    /// The bodies HIDDEN while a modifier feature is being edited: the feature itself plus its whole
    /// DESCENDANT chain (whatever consumes it, transitively). The source is shown, with its faces and
    /// edges visible and selectable — a temporary rollback to the feature being edited. It works IN THE
    /// MIDDLE of a chain too (mid-chain used to show the final model, with the face sitting on a hidden
    /// body). Empty when a base feature is being edited (it has no source), so its body is not hidden.
    fn edit_hidden_bodies(&self) -> std::collections::HashSet<Id> {
        let mut hide = std::collections::HashSet::new();
        let Some(fid) = self.cmd.edit else { return hide };
        let Some(node) = self.project.timeline.iter().find(|n| n.id == fid) else { return hide };
        if node.kind.consumed_body().is_none() {
            return hide; // a base feature: its body stays visible
        }
        let Some(b0) = node.kind.body() else { return hide };
        hide.insert(b0);
        // a forward closure over consumed(): everything that sits on b0, transitively
        let mut changed = true;
        while changed {
            changed = false;
            for n in &self.project.timeline {
                if n.kind.consumed().iter().any(|c| hide.contains(c)) {
                    if let Some(b) = n.kind.body() {
                        if hide.insert(b) {
                            changed = true;
                        }
                    }
                }
            }
        }
        hide
    }


    /// The candidate automatic constraints for the segment p1 -> p2 being drawn — for a live preview
    /// while drawing. Returns (glyph, the world point for the badge). Geometry only, nothing applied.
    fn infer_hints(&self, si: usize, prev: Option<Point2>, p1: Point2, p2: Point2) -> Vec<(Gly, Point2)> {
        let mut out: Vec<(Gly, Point2)> = Vec::new();
        let (dx, dy) = ((p2.x - p1.x).abs(), (p2.y - p1.y).abs());
        let mid = Point2::new((p1.x + p2.x) / 2.0, (p1.y + p2.y) / 2.0);
        let tol = 0.06;
        let mut axis = false;
        if dy <= dx * tol && dx > 1e-6 {
            out.push((Gly::Horiz, mid));
            axis = true;
        } else if dx <= dy * tol && dy > 1e-6 {
            out.push((Gly::Vert, mid));
            axis = true;
        }
        // perpendicular to the previous one
        if !axis {
            if let Some(pv) = prev {
                let (ux, uy) = (p1.x - pv.x, p1.y - pv.y);
                let (vx, vy) = (p2.x - p1.x, p2.y - p1.y);
                let (lu, lv) = ((ux * ux + uy * uy).sqrt(), (vx * vx + vy * vy).sqrt());
                if lu > 1e-6 && lv > 1e-6 && ((ux * vx + uy * vy) / (lu * lv)).abs() < tol {
                    out.push((Gly::Perp, p1));
                }
            }
            // parallel to the nearest non-axis line
            if self.nearest_parallel_line(si, p1, p2, 0, 0).is_some() {
                out.push((Gly::Parallel, mid));
            }
        }
        // tangent to a circle or an arc
        if self.nearest_tangent_circle(si, p1, p2).is_some() {
            out.push((Gly::Tangent, mid));
        }
        // equal in length to the nearest line
        if self.nearest_equal_line(si, p1, p2, 0, 0).is_some() {
            out.push((Gly::Equal, p2));
        }
        // an end coinciding with a vertex, or a point on an edge, is shown by the shared snap hint (`snap_infer_glyph`)
        out
    }

    /// The glyph of the automatic constraint implied by the cursor's current SNAP (coinciding with a
    /// vertex, centre, midpoint or intersection gives Coincident; on an edge it gives point-on-circle or
    /// point-on-line). For the preview in ALL the tools: any click snapped to geometry will attach a
    /// coincidence through the shared point.
    fn snap_infer_glyph(&self) -> Option<(Gly, Point2)> {
        let (p, kind) = self.snap_hint?;
        let g = match kind {
            0 | 3 | 4 | 5 => Gly::Coincident, // a vertex, a midpoint, a centre or an intersection
            6 => Gly::PointOnLine,            // on an edge
            _ => return None,                 // the grid or an axis: not a constraint
        };
        Some((g, p))
    }

    /// The ends of an existing LINE on which the point `p` lies (somewhere along it, not at an end) — for
    /// the automatic point-on-edge. Excludes the line (ea, eb) and any line where `p` is an end.
    fn line_under_point(&self, si: usize, p: Point2, ea: Id, eb: Id) -> Option<(Id, Id)> {
        use qymcad_core::model::EntityKind;
        let s = self.project.sketches.get(si)?;
        let pt = |id: Id| s.points.iter().find(|q| q.id == id).map(|q| Point2::new(q.x, q.y));
        for e in &s.entities {
            if let EntityKind::Line { a, b } = e.kind {
                if (a == ea && b == eb) || (a == eb && b == ea) {
                    continue; // our own line
                }
                let (Some(pa), Some(pb)) = (pt(a), pt(b)) else { continue };
                // p must not coincide with an end of the edge
                let near_end = ((p.x - pa.x).powi(2) + (p.y - pa.y).powi(2)).sqrt() < 1e-3 || ((p.x - pb.x).powi(2) + (p.y - pb.y).powi(2)).sqrt() < 1e-3;
                if near_end {
                    continue;
                }
                if let Some(proj) = project_on_seg(p, pa, pb) {
                    let dist_world = ((p.x - proj.x).powi(2) + (p.y - proj.y).powi(2)).sqrt();
                    // it is on the edge when it is within about 2 px on screen
                    if dist_world * self.view.scale as f64 <= 2.0 {
                        return Some((a, b));
                    }
                }
            }
        }
        None
    }

    /// Turn a click-driven editing operation on or off (1 = trim, 2 = extend, 3 = break).
    fn set_click_op(&mut self, op: u8) {
        let cur = self.tool.click_op;
        self.exit_draw_tools();
        self.tool.modify = 0;
        self.sel_sk.constraint = None;
        self.sel_sk.modify = None;
        self.tool.click_op = if cur == op { 0 } else { op };
        if self.tool.click_op != 0 {
            self.mode_3d = false;
        }
    }

    /// Whether the selection mode is active (no tool is in hand).
    fn in_select_mode(&self) -> bool {
        self.tool.kind == 0 && self.dim.kind == 0 && !self.measure.on
    }

    /// LEAVING ALL THE TOOLS — the single transition from a mode back to selection.
    ///
    /// The sketch modes are mutually exclusive: exactly one is active. This exit used to be written out
    /// by hand in six places, and the sets of fields in the copies DIFFERED — one reset forgot an
    /// unfinished import, another the pattern, a third the modify mode. So the exit exists in a single
    /// copy, and entering any tool begins with it. The selection (`sel_sk.items`) survives: what is
    /// cleared is the modes, not the work already done.
    fn exit_draw_tools(&mut self) {
        self.tool.kind = 0;
        self.tool.pts.clear();
        self.tool.circ_tan = None;
        self.tool.modify = 0;
        self.sel_sk.constraint = None;
        self.sel_sk.modify = None;
        self.dim.kind = 0;
        self.dim.pick.clear();
        self.dim.first = None;
        self.place.dim = None;
        self.pending_import.draw_pts = None;
        self.drag.clear();
        self.inline.clear();
        self.picking.clear(); // exclusivity: the shape pick of "Fillet all" does not survive a change of mode
        self.corner.clear();
        self.measure.clear(); // both the FLAG and the points collected: otherwise they outlived the exit
        self.tool.click_op = 0;
        self.tool.move_op = 0;
        self.tool.move_base = None;
        self.pat.op = 0;
        self.pat.edit = None;
        self.pat.center = None;
        self.cmd.close(); // the command is closed as a whole, not just a field zeroed out
        self.cmd.sketch = None;
        self.gsel.profiles.clear();
        // clear the viewport selection of a dimension, a note or a text (otherwise it stays highlighted while drawing)
        self.gsel.constraint = None;
        self.annot.note = None;
        self.annot.text = None;
    }

    /// Fillet ALL the corners of the current sketch's contour with the `sk_fillet` radius.
    fn fillet_all_corners(&mut self) {
        // A COMMAND. With a selection, the popup opens on it straight away; without one, the mode becomes "click a shape".
        if let Sel::Sketch(si) = self.sel {
            let only: std::collections::HashSet<Id> = self.sel_sk.items.iter().filter(|(k, _)| *k == 1).map(|(_, id)| *id).collect();
            if only.is_empty() {
                self.picking = Picking::FilletAll;
                self.status = crate::i18n::tr("g-fillet-all-hint");
            } else {
                self.corner.at = Some((si, 0, false));
                self.corner.only = Some(only);
                self.corner.pos = None; // the centre of the canvas
                self.corner.buf = format!("{}", (self.tool_prefs.fillet * 1000.0).round() / 1000.0);
                self.corner.focus = true;
            }
        }
    }

    /// The entity nearest to a screen point (within 12 px): for a line the distance to the segment, for
    /// a circle or an arc the distance to the rim.
    fn entity_near(&self, rect: Rect, pos: Pos2, si: usize) -> Option<Id> {
        let s = self.project.sketches.get(si)?;
        let scr = |id: Id| s.points.iter().find(|q| q.id == id).map(|q| self.to_screen(rect, Point2::new(q.x, q.y)));
        let seg_d = |p: Pos2, a: Pos2, b: Pos2| -> f32 {
            let (vx, vy) = (b.x - a.x, b.y - a.y);
            let l2 = vx * vx + vy * vy;
            if l2 < 1e-6 {
                return p.distance(a);
            }
            let t = ((p.x - a.x) * vx + (p.y - a.y) * vy) / l2;
            let t = t.clamp(0.0, 1.0);
            p.distance(Pos2::new(a.x + vx * t, a.y + vy * t))
        };
        use qymcad_core::model::EntityKind;
        let mut best: Option<(f32, Id)> = None;
        for e in &s.entities {
            let d = match e.kind {
                EntityKind::Line { a, b } => match (scr(a), scr(b)) {
                    (Some(pa), Some(pb)) => seg_d(pos, pa, pb),
                    _ => continue,
                },
                EntityKind::Circle { center, r } => match scr(center) {
                    Some(c) => (pos.distance(c) - (r * self.view.scale as f64) as f32).abs(),
                    _ => continue,
                },
                EntityKind::Arc { center, a, .. } => match (scr(center), scr(a)) {
                    (Some(c), Some(pa)) => (pos.distance(c) - c.distance(pa)).abs(),
                    _ => continue,
                },
                _ => continue,
            };
            if d < 12.0 && best.map_or(true, |(bd, _)| d < bd) {
                best = Some((d, e.id));
            }
        }
        best.map(|(_, id)| id)
    }

    /// A `PatternKind` built from the parameter fields (for a circular one the centre is the point picked,
    /// or the centroid of the selection).
    fn current_pattern_kind(&self, si: usize, eids: &[Id]) -> qymcad_core::model::PatternKind {
        use qymcad_core::model::PatternKind;
        if self.pat.op == 2 {
            // the centre in order of priority: the one explicitly clicked, then the stored one (while editing), then the selection's centroid
            let (cx, cy) = if let Some(c) = self.pat.center {
                (c.x, c.y)
            } else if let Some(PatternKind::Circular { cx, cy, .. }) = self.pat.edit.and_then(|pi| self.project.sketches.get(si).and_then(|s| s.patterns.get(pi)).map(|p| p.kind)) {
                (cx, cy)
            } else {
                self.project.entities_centroid(si, eids)
            };
            PatternKind::Circular { cx, cy, count: self.sk_pat.count, total_deg: self.sk_pat.angle }
        } else {
            PatternKind::Linear { dx: self.sk_pat.dx, dy: self.sk_pat.dy, count: self.sk_pat.count, dx2: self.sk_pat.dx2, dy2: self.sk_pat.dy2, count2: self.sk_pat.count2 }
        }
    }

    /// Apply or update the pattern on Enter.
    fn confirm_pattern(&mut self) {
        self.begin_edit(&crate::i18n::tr("g-sketch-array")); // THE OPERATION BOUNDARY
        let Sel::Sketch(si) = self.sel else { return };
        if let Some(pi) = self.pat.edit {
            let src = self.project.sketches.get(si).and_then(|s| s.patterns.get(pi)).map(|p| p.source.clone()).unwrap_or_default();
            let kind = self.current_pattern_kind(si, &src);
            self.project.update_pattern(si, pi, kind);
            self.status = crate::i18n::tr("g-array-updated");
        } else {
            let eids: Vec<Id> = self.sel_sk.items.iter().filter(|(k, _)| *k == 1).map(|(_, id)| *id).collect();
            if eids.is_empty() {
                self.status = crate::i18n::tr("g-select-for-array");
                return;
            }
            let kind = self.current_pattern_kind(si, &eids);
            self.project.add_pattern(si, &eids, kind);
            self.status = crate::i18n::tr("g-array-created");
        }
        self.pat.op = 0;
        self.pat.edit = None;
        self.pat.center = None;
        self.invalidate();
            self.commit_edit();
    }

    /// Ctrl+C and Ctrl+X. While editing a sketch with entities selected, this copies or cuts the
    /// GEOMETRY (waiting for the base point to be clicked). Otherwise it acts on a tree node (a sketch, a
    /// part or a subassembly).
    /// Whether there is anything to copy right now (used to enable the Edit menu items).
    fn clipboard_can_copy(&self) -> bool {
        if self.edit_si().is_some() {
            return self.sel_sk.items.iter().any(|(k, _)| *k == 1);
        }
        match self.sel {
            Sel::Sketch(_) => true,
            Sel::Component(ci) => self.project.components.get(ci).map(|c| c.id) != Some(self.project.root),
            _ => false,
        }
    }

    /// A test facade: the same Ctrl+C the frame's key handling calls.
    #[cfg(test)]
    pub(crate) fn clipboard_copy_for_test(&mut self, cut: bool) {
        self.clipboard_copy(cut);
    }

    /// Whether a geometry copy is armed and waiting for its base point.
    #[cfg(test)]
    pub(crate) fn clip_geom_pending_for_test(&self) -> bool {
        self.clip.geom_pending.is_some()
    }

    fn clipboard_copy(&mut self, cut: bool) {
        // while editing a sketch, the GEOMETRY is copied, whatever is selected in the tree
        if self.edit_si().is_some() {
            let eids: Vec<Id> = self.sel_sk.items.iter().filter(|(k, _)| *k == 1).map(|(_, id)| *id).collect();
            if eids.is_empty() {
                // A REFUSAL COSTS NOTHING. Nothing is selected, so there is no copy to make - and the tool
                // stays in hand: taking a half-drawn shape away over a mis-press would punish the person for
                // pressing the wrong key.
                self.status = crate::i18n::tr("g-pick-for-copy");
                return;
            }
            // THE TOOL GOES DOWN BEFORE THE COPY IS ARMED.
            //
            // Reported behaviour: copying with a tool active does not cancel the tool. Both then wait for
            // the same click - the copy for its base point, the tool for its next vertex - and whichever
            // gets it, the person did not ask for that; a half-built shape is left on the canvas with
            // nothing to finish it. Copying acts on what is SELECTED, so what is being BUILT has no part in
            // it.
            self.exit_draw_tools();
            self.clip.geom_place = false;
            self.clip.geom_pending = Some((eids, cut));
            self.status = if cut { crate::i18n::tr("g-cut-base-point") } else { crate::i18n::tr("g-copy-base-point") };
            return;
        }
        // outside a sketch, Ctrl+C acts on THE TREE (a part, a subassembly, a sketch node).
        // A call to ITSELF used to stand here — endless recursion, that is, a stack overflow on the very
        // first Ctrl+C outside sketch mode. A trace of a mechanical rename of the methods.
        self.tree_clipboard_copy(cut);
    }

    /// Ctrl+V. While editing a sketch with a non-empty geometry clipboard, this enters the placing mode
    /// (a ghost, then a click). Otherwise it pastes a tree node from the clipboard.
    fn clipboard_paste(&mut self) {
        // while editing a sketch with a non-empty geometry clipboard, enter the placing mode (a ghost, then a click)
        if self.edit_si().is_some() && self.clip.geom.as_ref().is_some_and(|c| !c.is_empty()) {
            self.clip.geom_pending = None;
            self.clip.geom_place = true;
            self.status = crate::i18n::tr("g-insert-click");
            return;
        }
        self.tree_clipboard_paste(); // outside a sketch, paste into the tree (see `clipboard_copy`)
    }

    /// Whether a multiple selection of components is active: the set holds more than one node AND the
    /// current `sel` is a component from that set. Otherwise the set counts as stale and clears itself: a
    /// click on a body or a sketch puts the multiple selection out without any explicit clean-up.
    fn is_multi(&self) -> bool {
        self.tree_sel.multi.len() > 1
            && matches!(self.sel, Sel::Component(ci) if self.project.components.get(ci).map(|c| self.tree_sel.multi.contains(&c.id)).unwrap_or(false))
    }

    /// A bulk move or copy of components from the multiple-selection clipboard into a target assembly.
    /// Every node goes through the SINGLE core method `reparent_component` or `clone_component` (by
    /// design, the UI only delegates).
    fn paste_components_multi(&mut self, ids: &[Id], cut: bool) {
        use qymcad_core::feature::ComponentKind;
        let root = self.project.root;
        // the target is the selected subassembly; if a Part is selected, its parent assembly; otherwise the active context
        let target = match self.sel {
            Sel::Component(ci) => match self.project.components.get(ci) {
                Some(c) if self.project.component_kind(c.id) == Some(ComponentKind::Assembly) => c.id,
                Some(c) => c.parent.unwrap_or(root),
                None => self.project.active_ctx(),
            },
            _ => self.project.active_ctx(),
        };
        if self.project.component_is_part(target) {
            self.status = crate::i18n::tr("g-paste-needs-assembly");
            return;
        }
        // Only the ROOTS of the selection: if the set holds both a subassembly and one of its
        // descendants, the descendant travels with the subassembly, as in a file manager. Otherwise the
        // reparent loop would pull the nested one out, and clone would duplicate it. This way the tree is
        // rebuilt predictably, with no orphans and no duplicates.
        let roots: Vec<Id> = ids.iter().copied().filter(|&id| !ids.iter().any(|&o| o != id && self.project.component_is_within(id, o))).collect();
        let (mut ok, mut fail) = (0u32, 0u32);
        if cut {
            for &id in &roots {
                if self.project.reparent_component(id, target) {
                    ok += 1;
                } else {
                    fail += 1;
                }
            }
            self.clip.tree_multi = None;
            self.tree_sel.multi.clear();
            self.invalidate();
        } else {
            for &id in &roots {
                if self.project.clone_component(id, target).is_some() {
                    ok += 1;
                } else {
                    fail += 1;
                }
            }
            self.mark_dirty_for_rebuild(); // the document is marked; the scheduler does the computing
        }
        self.status = if fail == 0 {
            crate::i18n::tr2("g-paste-result", "what", &if cut { crate::i18n::tr("g-body-moved") } else { crate::i18n::tr("g-copied") }, "n", &ok.to_string())
        } else {
            crate::i18n::tr2("g-paste-partial", "ok", &ok.to_string(), "fail", &fail.to_string())
        };
    }

    /// The editing button: with a ready selection it applies at once, otherwise it waits for one (Esc cancels).
    fn modify_button(&mut self, op: u8) {
        self.exit_draw_tools();
        self.sel_sk.constraint = None;
        self.tool.modify = match op {
            4 => 1,
            5 => 2,
            6 => 3,
            1 => 4,
            2 => 5,
            3 => 6,
            _ => 0,
        };
        if self.try_modify(op) {
            self.sel_sk.modify = None;
        } else {
            self.sel_sk.clear(); // the selection, and whatever was waiting on it: pick anew, with nothing stuck from before
            self.sel_sk.modify = Some(op);
            self.status = crate::i18n::tr("g-pick-for-op");
        }
    }


    /// Pick a sketch drawing tool (entering a sketch first, if not in one yet).
    fn set_sk_tool(&mut self, t: u8) {
        if self.edit_si().is_none() {
            self.create_new_sketch();
        }
        self.exit_draw_tools(); // entering a tool means leaving all the others, in one move
        self.tool.select(t); // changing the tool clears whatever the previous one had collected
        self.mode_3d = false;
        self.status = match self.tool.kind {
            1 => crate::i18n::tr("g-line-hint"),
            2 => crate::i18n::tr("g-rect-hint"),
            3 => crate::i18n::tr("g-circle-hint"),
            4 => crate::i18n::tr("g-arc-cse"),
            5 => crate::i18n::tr("g-point-hint"),
            6 => crate::i18n::tr("g-polygon-hint"),
            7 => crate::i18n::tr("g-slot-hint"),
            8 => crate::i18n::tr("g-ellipse-hint"),
            9 => crate::i18n::tr("g-spline-hint"),
            10 => crate::i18n::tr("g-circle-3pt"),
            11 => crate::i18n::tr("g-text-hint"),
            _ => crate::i18n::tr("g-select"),
        };
    }

    /// The bytes of the default system font (used for text); cached.
    fn default_font(&mut self) -> Option<Vec<u8>> {
        if let Some(f) = &self.font_cache {
            return Some(f.clone());
        }
        let mut paths: Vec<String> = vec![
            "/usr/share/fonts/TTF/DejaVuSans.ttf".into(),
            "/usr/share/fonts/TTF/OpenSans-Regular.ttf".into(),
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf".into(),
            "/usr/share/fonts/liberation/LiberationSans-Regular.ttf".into(),
            "/System/Library/Fonts/Supplemental/Arial.ttf".into(),
            "C:/Windows/Fonts/arial.ttf".into(),
        ];
        if let Ok(o) = std::process::Command::new("fc-match").args(["-f", "%{file}", "sans"]).output() {
            if let Ok(s) = String::from_utf8(o.stdout) {
                let s = s.trim().to_string();
                if !s.is_empty() {
                    paths.insert(0, s);
                }
            }
        }
        for p in paths {
            if let Ok(b) = std::fs::read(&p) {
                self.font_cache = Some(b.clone());
                return Some(b);
            }
        }
        None
    }

    fn post_options(&self) -> PostOptions {
        let c = &self.project.machine.post_cfg;
        PostOptions {
            output_comments: c.comments,
            output_header: c.header,
            output_line_numbers: c.line_numbers,
            axis_precision: c.axis_precision,
            feed_precision: c.feed_precision,
            spindle_decimals: 1,
            output_tool_length_offset: c.tlo,
            translate_drill_cycles: c.translate_cycles,
        }
    }

    /// APPLY THE THEME TO `egui`. The one setting for which sitting in the record is not enough: the
    /// library does not remember its palette between runs, so it has to be assigned explicitly — at
    /// start-up and on every change. The theme used to be assigned ONLY on a click and stored nowhere:
    /// pick the light one, restart, and it is dark again.
    /// APPLY THE LANGUAGE. An empty setting means "never chosen", and then the system locale decides.
    /// The resolution lives in `i18n`, and this only passes it on: two places deciding "which language"
    /// would drift apart at the very first edit.
    pub(crate) fn apply_language(&self) {
        let code = if self.set.language.is_empty() { crate::i18n::system_default() } else { self.set.language.clone() };
        crate::i18n::set_language(&code);
        // THE HELP LANGUAGE TRAVELS WITH THE INTERFACE LANGUAGE — otherwise "same as the interface" would
        // lie after a switch: the help would stay on the previous one until its own setting was touched.
        crate::help::set_lang(&self.set.help_lang);
    }

    /// OPEN A RECENT FILE. If it is gone, say so and DROP it from the list.
    ///
    /// A dead row must not be left there silently: the list exists so that one lands on the file at the
    /// first attempt, and a row where nothing happens on a click reads as a broken program.
    pub(crate) fn open_recent(&mut self, path: String) {
        if !std::path::Path::new(&path).exists() {
            self.status = crate::i18n::tr1("recent-missing", "path", &path);
            self.forget_recent(&path);
            return;
        }
        self.spawn_project_load(path);
    }

    /// THE DOCUMENT PATH HAS BECOME THE CURRENT ONE — AND WENT STRAIGHT INTO THE RECENT LIST.
    ///
    /// One point for both events, deliberately: "opened" or "saved as" and "remember it" are one action,
    /// and spread across the call sites, forgetting the second would only be a matter of time. A guard
    /// (`recent_files.rs`) keeps a direct assignment to `project_path` out of the working code.
    pub(crate) fn set_project_path(&mut self, path: String) {
        self.project_path = Some(path.clone());
        self.remember_recent(path);
        // A crash hook cannot reach the document, so it keeps the PATHS instead and the next start
        // offers them back. Told here, at the one point where the path can change.
        crate::crash::note_document(self.project_path.as_deref(), Some(&self.autosave_path()));
    }

    /// Put a path at the top of the recent list: no duplicates, trimmed to length.
    ///
    /// The freshest one goes first: the list is read from the top, and "the last thing worked on" must
    /// be right under the cursor.
    pub(crate) fn remember_recent(&mut self, path: String) {
        if path.trim().is_empty() {
            return;
        }
        self.set.recent.retain(|p| p != &path);
        self.set.recent.insert(0, path);
        let n = self.set.recent_limit.max(1);
        self.set.recent.truncate(n);
    }

    /// Drop a path from the recent list — the file would not open (renamed, moved away, the drive
    /// unmounted).
    ///
    /// A dead row must not be left there silently: it would be clicked again and again, while the list
    /// exists so that one lands on the file at the first attempt.
    pub(crate) fn forget_recent(&mut self, path: &str) {
        self.set.recent.retain(|p| p != path);
    }

    /// A test facade for "enter a component" (a double click on a part in the tree).
    #[cfg(test)]
    pub(crate) fn enter_ctx_for_test(&mut self, id: Id) {
        self.active_path = vec![self.project.root, id];
        self.project.set_active_component(Some(id));
        self.sync_workbench();
    }

    #[cfg(test)]
    pub(crate) fn tree_search_for_test(&self) -> String {
        self.tree.search.clone()
    }

    /// Test facades for changing the context: a test must travel the same path as a double click and as "up".
    #[cfg(test)]
    pub(crate) fn enter_component_for_test(&mut self, cid: Id) {
        self.enter_component(cid);
    }

    #[cfg(test)]
    pub(crate) fn exit_context_for_test(&mut self) {
        self.exit_context();
    }

    /// A test facade for the tree search: a test must edit the same field as the input line.
    #[cfg(test)]
    pub(crate) fn set_tree_search_for_test(&mut self, q: &str) {
        self.tree.search = q.to_string();
    }

    /// Test facades for the "save?" question: a test must pull the same handles as the menu and the dialogue.
    #[cfg(test)]
    pub(crate) fn open_for_test(&mut self, path: String) {
        self.spawn_project_load(path);
        // READING A FILE GOES THROUGH THE MODAL QUEUE (`self.regen.busy`), not the background one (`self.regen.bg`).
        // Only `wait_bg` used to stand here, and it waited on an EMPTY queue: nobody took the result of
        // the read, the document stayed FACTORY EMPTY, and tests of the form "opened a file and..."
        // checked emptiness and therefore always passed.
        self.drain_busy_for_test();
        self.wait_bg();
        self.rebuild_if_dirty();
    }

    /// A facade: carry a BACKGROUND job (a rebuild) through to the end, as a frame of the program does.
    #[cfg(test)]
    /// Wait for BACKGROUND work with no overlay (saving, loading in the B-rep) — in a live program a
    /// frame picks it up. A test cannot "wait for a frame", and without this the written file does not
    /// exist yet.
    #[cfg(test)]
    pub(crate) fn drain_bg_for_test(&mut self) {
        for _ in 0..8 {
            if self.regen.bg.is_empty() {
                break;
            }
            let jobs: Vec<_> = std::mem::take(&mut self.regen.bg);
            for b in jobs {
                if let Ok(res) = b.rx.recv_timeout(std::time::Duration::from_secs(120)) {
                    self.apply_job_result(res);
                }
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn drain_busy_for_test(&mut self) {
        for _ in 0..8 {
            let Some(busy) = self.regen.busy.take() else { break };
            match busy.rx.recv_timeout(std::time::Duration::from_secs(120)) {
                Ok(res) => self.apply_job_result(res),
                Err(_) => break,
            }
        }
    }

    /// A facade for "the opening asked for a rebuild" — what loading the B-rep in from the bundle does.
    #[cfg(test)]
    pub(crate) fn mark_dirty_for_rebuild_for_test(&mut self) {
        self.mark_dirty_for_rebuild();
    }

    #[cfg(test)]
    pub(crate) fn is_dirty_for_test(&self) -> bool {
        self.is_dirty()
    }

    #[cfg(test)]
    pub(crate) fn rebuild_if_dirty_for_test(&mut self) {
        self.rebuild_if_dirty();
    }

    /// The joints panel in a single call — the door for checks that look AT THE FRAME.
    #[cfg(test)]
    pub(crate) fn joints_panel_for_test(&mut self, ui: &mut egui::Ui) {
        self.joints_panel(ui);
    }

    /// THE BUILD TREE in a single call — the same door for checks that look at the frame.
    #[cfg(test)]
    pub(crate) fn build_tree_for_test(&mut self, ui: &mut egui::Ui) {
        self.build_tree(ui);
    }

    /// The handles of a joint's degree-of-freedom gizmo — the door for the check that the arrow points
    /// where the part will actually travel.
    #[cfg(test)]
    pub(crate) fn joint_giz_handles_for_test(&self, jid: Id) -> Option<([f64; 3], Vec<(u8, bool, [f64; 3])>)> {
        self.joint_giz_handles(jid)
    }

    /// Starting a joint pick — the door for checks of the path taken when the Joint button is pressed.
    #[cfg(test)]
    pub(crate) fn start_joint_pick_for_test(&mut self) {
        self.start_joint_pick();
    }

    /// The joint's degree-of-freedom gizmo into a frame — the door for checks of what is actually seen.
    #[cfg(test)]
    pub(crate) fn draw_joint_gizmo_for_test(&self, painter: &egui::Painter, rect: egui::Rect, jid: Id) {
        self.draw_joint_gizmo(painter, rect, jid);
    }

    /// Re-picking the anchor of an existing joint — the door for the check that a broken joint can be mended.
    #[cfg(test)]
    pub(crate) fn joint_edit_repick_apply_for_test(&mut self, body: Id, anchor: qymcad_core::feature::AnchorRef) {
        self.joint_edit_repick_apply(body, anchor);
    }

    /// The check doors for the Tangent command — the same path a person takes.
    #[cfg(test)]
    pub(crate) fn start_tangent_pick_for_test(&mut self) {
        self.start_tangent_pick();
    }
    #[cfg(test)]
    pub(crate) fn tangent_pick_active_for_test(&self) -> bool {
        self.joint.tangent_pick.is_some()
    }
    #[cfg(test)]
    pub(crate) fn tangent_pick_click_for_test(&mut self, body: Id, key: qymcad_core::feature::FaceKey) {
        self.tangent_pick_click(body, key);
    }

    /// The check doors for the Width command — the same path a person takes.
    #[cfg(test)]
    pub(crate) fn start_width_pick_for_test(&mut self) {
        self.start_width_pick();
    }
    #[cfg(test)]
    pub(crate) fn width_pick_active_for_test(&self) -> bool {
        self.joint.width_pick.is_some()
    }
    #[cfg(test)]
    pub(crate) fn width_pick_count_for_test(&self) -> usize {
        self.joint.width_pick.as_ref().map_or(0, |v| v.len())
    }
    #[cfg(test)]
    pub(crate) fn width_pick_click_for_test(&mut self, body: Id, key: qymcad_core::feature::FaceKey) {
        self.width_pick_click(body, key);
    }
    #[cfg(test)]
    pub(crate) fn width_pick_confirm_for_test(&mut self) {
        self.width_pick_confirm();
    }

    /// The check doors for the Group command — the same path a person takes.
    #[cfg(test)]
    pub(crate) fn start_group_pick_for_test(&mut self) {
        self.start_group_pick();
    }
    #[cfg(test)]
    pub(crate) fn group_pick_active_for_test(&self) -> bool {
        self.joint.group_pick.is_some()
    }
    #[cfg(test)]
    pub(crate) fn group_pick_members_for_test(&self) -> Vec<Id> {
        self.joint.group_pick.clone().unwrap_or_default()
    }
    #[cfg(test)]
    pub(crate) fn group_pick_click_for_test(&mut self, body: Id) {
        self.group_pick_click(body);
    }
    #[cfg(test)]
    pub(crate) fn group_pick_confirm_for_test(&mut self) {
        self.group_pick_confirm();
    }

    /// The check doors for the Relation command — the same path a person takes.
    #[cfg(test)]
    pub(crate) fn start_relation_pick_for_test(&mut self) {
        self.start_relation_pick();
    }
    #[cfg(test)]
    pub(crate) fn relation_pick_active_for_test(&self) -> bool {
        self.joint.relation_pick.is_some()
    }
    #[cfg(test)]
    pub(crate) fn relation_pick_set_for_test(&mut self, kind: qymcad_core::feature::RelationKind, value: f64) {
        if let Some(p) = self.joint.relation_pick.as_mut() {
            if p.kind != kind {
                p.picks.clear();
            }
            p.kind = kind;
            p.value = value;
        }
    }
    #[cfg(test)]
    pub(crate) fn relation_pick_count_for_test(&self) -> usize {
        self.joint.relation_pick.as_ref().map_or(0, |p| p.picks.len())
    }
    #[cfg(test)]
    pub(crate) fn relation_pick_click_for_test(&mut self, joint: Id) {
        self.relation_pick_click(joint);
    }
    #[cfg(test)]
    pub(crate) fn relation_pick_confirm_for_test(&mut self) {
        self.relation_pick_confirm();
    }

    /// Take the secondary axis being pointed at — the door for checks of the path taken by clicking an edge.
    #[cfg(test)]
    pub(crate) fn joint_axis_pick_apply_for_test(&mut self, anchor: qymcad_core::feature::AnchorRef) {
        self.joint_axis_pick_apply(anchor);
    }

    /// The joint editing popup (home to the anchors and the "Set the axis" button) — the door for frame checks.
    #[cfg(test)]
    pub(crate) fn joint_popup_for_test(&mut self, ctx: &egui::Context, rect: egui::Rect) {
        self.joint.edit = self.joint.edit.or_else(|| self.project.joints.first().map(|j| j.id));
        self.joint_popup(ctx, rect);
    }

    #[cfg(test)]
    pub(crate) fn request_nav_for_test(&mut self, nav: Nav) {
        let ctx = egui::Context::default();
        self.request_nav(nav, &ctx);
    }

    #[cfg(test)]
    pub(crate) fn deferred_nav_for_test(&self) -> bool {
        self.deferred.nav.is_some()
    }

    #[cfg(test)]
    pub(crate) fn save_project_for_test(&mut self) {
        self.save_project();
    }

    #[cfg(test)]
    pub(crate) fn wait_bg_for_test(&mut self) {
        self.wait_bg();
    }

    /// Test facades for saving: a test must take the same path as Save does.
    #[cfg(test)]
    pub(crate) fn save_for_test(&mut self, path: String) {
        self.spawn_save(path, false);
        self.wait_bg();
    }

    #[cfg(test)]
    pub(crate) fn autosave_for_test(&mut self, path: String) {
        self.spawn_save(path, true);
        self.wait_bg();
    }

    /// PUSH THE WAITING CARD BACK IN TIME — so that a check can watch the floor without sleeping.
    #[cfg(test)]
    pub(crate) fn age_save_wait_for_test(&mut self, by: std::time::Duration) {
        if let Some(t) = self.waiting.save_since {
            self.waiting.save_since = Some(t - by);
        }
        if let Some(t) = self.waiting.save_shown {
            self.waiting.save_shown = Some(t - by);
        }
    }

    /// Whether the waiting card is on screen right now (it was drawn on the last frame).
    #[cfg(test)]
    pub(crate) fn save_wait_shown_for_test(&self) -> bool {
        self.waiting.save_shown.is_some()
    }

    /// Facades for the splash screen: a test must run the same frame as the program does.
    #[cfg(test)]
    pub(crate) fn set_splash_for_test(&mut self, hold: std::time::Duration) {
        self.waiting.splash_until = Some(std::time::Instant::now() + hold);
    }

    #[cfg(test)]
    pub(crate) fn set_startup_for_test(&mut self, path: &str) {
        self.io.startup = Some(path.to_string());
    }

    #[cfg(test)]
    pub(crate) fn tick_async_for_test(&mut self, ctx: &egui::Context) -> bool {
        self.tick_async(ctx)
    }

    /// Test facades for the modal overlays: a test must draw them with the same code as the program.
    #[cfg(test)]
    pub(crate) fn draw_dim_overlay_for_test(&self, ctx: &egui::Context, label: &str) {
        self.draw_dim_overlay(ctx, label);
    }

    /// A facade: the overlay WITH A COUNT (and therefore with a cancel button). `true` means Cancel was pressed.
    #[cfg(test)]
    pub(crate) fn draw_regen_overlay_for_test(&self, ctx: &egui::Context, done: usize, total: usize) -> bool {
        self.draw_dim_overlay_with(ctx, &crate::i18n::tr("io-rebuilding"), Some((done, total)), egui::Rect::NOTHING)
    }

    /// A facade: the rebuild overlay WITH A LIVE CANVAS — by the same call the frame makes.
    #[cfg(test)]
    pub(crate) fn draw_regen_overlay_over(&self, ctx: &egui::Context, live: egui::Rect) -> bool {
        self.draw_dim_overlay_with(ctx, &crate::i18n::tr("io-rebuilding"), Some((1, 2)), live)
    }

    /// A facade: a sketch point in screen coordinates — by the same arithmetic the drawing uses.
    #[cfg(test)]
    pub(crate) fn to_screen_for_test(&self, rect: Rect, p: qymcad_core::geom::Point2) -> Pos2 {
        self.to_screen(rect, p)
    }

    /// A facade: the canvas rectangle of the previous frame.
    #[cfg(test)]
    pub(crate) fn view_rect_for_test(&self) -> egui::Rect {
        self.view_rect
    }

    /// A facade: the geometry of a component's gizmo (the origin and the length of the arrows).
    #[cfg(test)]
    pub(crate) fn gizmo_geometry_for_test(&self, comp: Id) -> ([f64; 3], f64) {
        self.gizmo_geometry(comp)
    }

    /// A facade: lay out and handle the 3D CANVAS — with a real `Response`.
    ///
    /// The mouse checks need it: the gizmo, the rubber band and the dimension drag all live on a
    /// `Response`, and faking one would mean checking the fake. This is the door that makes the mouse
    /// paths coverable.
    #[cfg(test)]
    pub(crate) fn viewport_for_test(&mut self, ctx: &egui::Context) {
        self.viewport(ctx);
    }

    /// A facade: whether a background rebuild is running right now.
    #[cfg(test)]
    pub(crate) fn regen_running_for_test(&self) -> bool {
        matches!(&self.regen.busy, Some(b) if b.kind == BgKind::Regen)
    }

    /// A facade: ask the rebuild to stop — by the same handle the button uses.
    #[cfg(test)]
    pub(crate) fn cancel_regen_for_test(&mut self) {
        self.cancel_regen();
    }

    #[cfg(test)]
    pub(crate) fn regen_paused_for_test(&self) -> bool {
        self.regen.paused
    }

    /// Facades for the question asked before deleting: a test pulls the same handles as the button and the Del key.
    #[cfg(test)]
    pub(crate) fn deferred_delete_for_test(&self) -> bool {
        self.deferred.delete.is_some()
    }

    #[cfg(test)]
    pub(crate) fn delete_cascade_names_for_test(&self, sel: Sel) -> Vec<String> {
        self.delete_cascade_names(sel)
    }

    #[cfg(test)]
    pub(crate) fn execute_deferred_delete_for_test(&mut self) {
        if let Some(sel) = self.deferred.delete.take() {
            self.execute_delete(sel);
        }
    }

    #[cfg(test)]
    pub(crate) fn draw_splash_for_test(&self, ctx: &egui::Context, label: &str) {
        self.draw_splash(ctx, label);
    }

    /// Test facades for the start screen: a test must pull the same handles as the interface.
    #[cfg(test)]
    pub(crate) fn set_show_start_for_test(&mut self, on: bool) {
        // THE FACADE TELLS TWO STATES APART, as the program does: `show_start` means "it comes up by
        // itself on an empty document", while a request from the menu is separate
        // (`ask_start_screen_for_test`). Closing puts out both: closed is closed, whatever raised it.
        self.win.start = on;
        if !on {
            self.win.start_asked = false;
        }
    }

    pub(crate) fn workbench_code(&self) -> &'static str {
        self.workbench.code()
    }

    /// Facades for the assembly modes: a test enters them through the same fields the buttons use.
    #[cfg(test)]
    pub(crate) fn start_rigid_joint_pick_for_test(&mut self) {
        self.joint.new_kind = qymcad_core::feature::JointKind::Rigid;
        self.joint.pick_faces = true;
    }

    #[cfg(test)]
    pub(crate) fn start_comp_array_mode_for_test(&mut self, mode: u8) {
        self.carr.mode = mode;
    }

    /// Facades for the help: a test uses the same handles as the window.
    #[cfg(test)]
    pub(crate) fn help_article_for_test(&self) -> String {
        self.win.help_article.clone()
    }

    #[cfg(test)]
    pub(crate) fn help_can_go_back_for_test(&self) -> bool {
        !self.win.help_back.is_empty()
    }

    #[cfg(test)]
    pub(crate) fn help_back_for_test(&mut self) {
        if let Some(prev) = self.win.help_back.pop() {
            self.win.help_article = prev;
        }
    }

    /// Facades for the settings that used to be constants: what is checked is that they are APPLIED, not that they are stored.
    #[cfg(test)]
    pub(crate) fn set_last_autosave_ago_for_test(&mut self, secs: u64) {
        self.edits.last_autosave = std::time::Instant::now() - std::time::Duration::from_secs(secs);
    }

    #[cfg(test)]
    pub(crate) fn maybe_autosave_for_test(&mut self) {
        self.maybe_autosave(false);
    }

    #[cfg(test)]
    pub(crate) fn undo_len_for_test(&self) -> usize {
        self.edits.undo.len()
    }

    /// The current context — for measuring in checks.
    #[cfg(test)]
    pub(crate) fn current_ctx_id_for_test(&self) -> Id {
        self.current_ctx_id()
    }

    /// Whether a file is being written right now — for a check.
    #[cfg(test)]
    pub(crate) fn saving_now_for_test(&self) -> bool {
        self.saving_now()
    }

    /// Answer Save in the unsaved-work dialogue — the same as pressing the button.
    #[cfg(test)]
    pub(crate) fn answer_save_for_test(&mut self) {
        self.save_project();
        if self.saving_now() {
            self.deferred.nav_after_save = true;
        }
    }

    /// Whether a navigation is waiting its turn.
    #[cfg(test)]
    pub(crate) fn deferred_nav_is_set_for_test(&self) -> bool {
        self.deferred.nav.is_some() || self.pending_nav.is_some()
    }

    /// Draw the dialogue or the card — the same thing a frame does.
    #[cfg(test)]
    pub(crate) fn nav_dialog_for_test(&mut self, ctx: &egui::Context) {
        self.nav_dialog(ctx);
    }

    /// Hide or show a component — by the same path the tree's tick takes.
    #[cfg(test)]
    pub(crate) fn set_component_visible_for_test(&mut self, cid: Id, vis: bool) {
        self.set_component_visible(cid, vis);
    }

    /// Type into the search field of the parameters window.
    #[cfg(test)]
    pub(crate) fn par_search_for_test(&mut self, q: &str) {
        self.par_search = q.to_string();
    }

    /// Undo the last step — by the same path Ctrl+Z takes.
    #[cfg(test)]
    pub(crate) fn undo_for_test(&mut self) {
        self.undo();
    }

    /// THE KEY OF THE WHOLE DOCUMENT. The checks use it to measure the main rule: while a value is being
    /// typed, the model does not change. The key is computed from the document's structure — unchanged
    /// means untouched.
    #[cfg(test)]
    pub(crate) fn doc_key_for_test(&self) -> u64 {
        self.doc_key()
    }

    #[cfg(test)]
    pub(crate) fn view_dir_at_for_test(&self, p: [f64; 3], fwd: [f64; 3], inv_d: f64) -> [f64; 3] {
        self.view_dir_at(p, fwd, inv_d)
    }

    #[cfg(test)]
    pub(crate) fn persp_inv_d_for_test(&self, half_h_points: f32) -> f64 {
        self.persp_inv_d_eye(half_h_points)
    }

    #[cfg(test)]
    pub(crate) fn shade_tri_for_test(pal: &crate::palette::Palette, ghost_alpha: u8, hot: bool, ghost: bool, base: [u8; 3], n: [f64; 3], light: [f64; 3]) -> Color32 {
        Self::shade_tri(pal, ghost_alpha, hot, ghost, base, n, light)
    }

    /// A facade for Windows -> Start screen — the same handle the menu item uses.
    #[cfg(test)]
    pub(crate) fn ask_start_screen_for_test(&mut self) {
        self.win.start_asked = true;
    }

    #[cfg(test)]
    pub(crate) fn new_assembly_project_for_test(&mut self) {
        self.new_assembly_project();
    }

    #[cfg(test)]
    pub(crate) fn new_project_for_test(&mut self) {
        self.new_project();
    }

    /// Test facades for the recent files: a test must look at the same record the menu does.
    #[cfg(test)]
    pub(crate) fn project_path_for_test(&self) -> Option<String> {
        self.project_path.clone()
    }

    #[cfg(test)]
    pub(crate) fn recent_for_test(&self) -> Vec<String> {
        self.set.recent.clone()
    }

    #[cfg(test)]
    pub(crate) fn set_recent_limit_for_test(&mut self, n: usize) {
        self.set.recent_limit = n;
    }

    /// How many bodies actually reach the viewport (the same list the CPU raster and the GPU pass draw).
    #[cfg(test)]
    pub(crate) fn visible_mesh_items_for_test(&self) -> usize {
        self.visible_mesh_items().len()
    }

    /// The Z extent of the tallest visible body — a number answering whether the part followed the parameter.
    #[cfg(test)]
    pub(crate) fn body_height_for_test(&self) -> f64 {
        self.visible_mesh_items()
            .iter()
            .filter_map(|(_, _, _, _, m, _)| m.bounds())
            .map(|bb| (bb.max.z - bb.min.z) as f64)
            .fold(0.0, f64::max)
    }

    /// A facade for the parameters window: editing a global parameter by the same handle the window uses.
    #[cfg(test)]
    pub(crate) fn set_param_for_test(&mut self, name: &str, expr: &str) {
        use qymcad_core::model::Param;
        match self.project.parameters.iter_mut().find(|p| p.name == name) {
            Some(p) => p.expr = expr.to_string(),
            None => self.project.parameters.push(Param { name: name.to_string(), expr: expr.to_string(), value: 0.0 }),
        }
        self.apply_param_edit();
    }

    #[cfg(test)]
    pub(crate) fn status_for_test(&self) -> String {
        self.status.clone()
    }

    /// A test facade for the picking precision: a test must take the same path as the settings window.
    #[cfg(test)]
    pub(crate) fn set_pick_precision_for_test(&mut self, level: u8) {
        self.set.pick_precision = level;
    }

    /// A test facade for the machining module's tick: the Machining section of the settings window is
    /// visible only while the module is on, and that has to be checked through the same setting the
    /// window edits.
    #[cfg(test)]
    pub(crate) fn set_cam_tab_for_test(&mut self, on: bool) {
        self.set.cam_tab_enabled = on;
    }

    /// Test facades for the language setting: a test must take the same path as the settings window.
    #[cfg(test)]
    pub(crate) fn settings_language_is_empty(&self) -> bool {
        self.set.language.is_empty()
    }

    #[cfg(test)]
    pub(crate) fn set_language_for_test(&mut self, code: &str) {
        self.set.language = code.to_string();
        self.apply_language();
    }

    /// A test facade for the current palette.
    #[cfg(test)]
    /// Test facades for the cache keys: a test must ask THE SAME thing a real frame asks.
    #[cfg(test)]
    pub(crate) fn view_key_pub(&self, rect: Rect, ppp: f32) -> u64 {
        self.view_key(rect, ppp)
    }
    #[cfg(test)]
    #[cfg(test)]
    /// A facade for the scene block counts: [rebuilt, moved, taken ready-made].
    #[cfg(test)]
    pub(crate) fn scene_stats_for_test(&self) -> [u32; 3] {
        self.cache.scene_stats.get()
    }

    #[cfg(test)]
    pub(crate) fn gpu_scene_for_test(&self) -> (usize, u32) {
        let (v, oc) = self.gpu_scene();
        (v.len(), oc)
    }

    #[cfg(test)]
    pub(crate) fn gpu_scene_key_pub(&self) -> u64 {
        self.gpu_scene_key()
    }

    /// A test facade for entering a sketch: a test must take THE SAME path as a double click in the tree.
    #[cfg(test)]
    pub(crate) fn enter_sketch_edit_pub(&mut self, si: usize) {
        self.enter_sketch_edit(si);
    }

    /// A test facade for editing a scheme live — by the same path the editor takes.
    #[cfg(test)]
    pub(crate) fn pal_mut_pub(&mut self) -> &mut crate::palette::Palette {
        &mut self.scheme.pal
    }

    /// The command did not apply: say so and record the fact, so the operation can be rolled back.
    pub(crate) fn cmd_fail(&mut self, text: String) {
        self.status = text;
        self.cmd_failed = true;
    }

    #[cfg(test)]
    pub(crate) fn palette_pub(&self) -> &crate::palette::Palette {
        &self.scheme.pal
    }

    /// Re-read the schemes from disk (at start-up and after a custom one is edited).
    pub(crate) fn reload_schemes(&mut self) {
        let (all, errs) = crate::palette::all();
        self.scheme.all = all;
        if !errs.is_empty() {
            self.status = crate::i18n::tr1("scheme-load-failed", "error", &errs.join("; "));
        }
    }

    /// The look of the egui panels according to the current scheme. Kept apart from [`Self::apply_theme`],
    /// because that one REBUILDS the scheme from the list — which, while a custom scheme is being edited
    /// live, would wipe out the unsaved work.
    pub(crate) fn sync_visuals(&self, ctx: &egui::Context) {
        ctx.set_visuals(crate::palette::visuals(&self.scheme.pal));
        // THE INTERFACE SCALE IS NOT APPLIED HERE. It has nothing to do with the theme, and the coupling
        // was a hidden one: because of it "adopt the settings" worked even without its own call to the
        // scale, and the guard stayed silent about that. The scale is applied by those whose business it
        // is: `adopt_settings` and the slider in the window.
    }

    /// APPLY THE INTERFACE SCALE TO `egui`.
    ///
    /// A method of its own, called from BOTH places where the appearance is applied (`apply_theme` at
    /// start-up and on a change of scheme, `sync_visuals` while a custom scheme is edited live). Exactly
    /// the lesson the theme taught: it was called on a click and applied nowhere at start-up, so the
    /// choice silently rolled back.
    ///
    /// The bounds are strict: with a zero or negative factor `egui` draws an interface one can neither
    /// leave nor use to put the setting back.
    pub(crate) fn apply_ui_scale(&self, ctx: &egui::Context) {
        ctx.set_zoom_factor(self.set.ui_scale.clamp(0.5, 3.0));
    }

    pub(crate) fn apply_theme(&mut self, ctx: &egui::Context) {
        // A SCHEME SETS BOTH THE CANVAS PALETTE AND THE LOOK OF `egui` ITSELF. These used to be two
        // unrelated things: the theme changed the buttons while the canvas stayed dark — exactly what was
        // reported.
        self.scheme.pal = self.scheme.all.iter().find(|p| p.id == self.set.scheme).cloned().unwrap_or_else(crate::palette::dark);
        // THE LOOK OF `egui` ASKS THE SCHEME rather than choosing between light and dark by itself:
        // otherwise a scheme that colours the interface would colour only the canvas, and half the window
        // would stay factory-coloured. Schemes with no interface colours of their own get exactly that
        // same factory look.
        ctx.set_visuals(crate::palette::visuals(&self.scheme.pal));
        // THE INTERFACE SCALE IS NOT APPLIED HERE: it has nothing to do with the theme. The coupling was
        // hidden and harmful — because of it "adopt the settings" would work even without its own call to
        // the scale, and the guard would stay silent. The scale is applied by those whose business it is:
        // `adopt_settings` and the slider in the window.
    }

    /// THE VIEW REVISION: what is on screen depends both on the shape of the bodies and on where they
    /// stand.
    ///
    /// Caches that depend on the placement are keyed by it (world bounding boxes, section caps, the scene
    /// buffer). Caches that depend ONLY on the shape (a body's edges) still look at `geom_rev`.
    fn view_rev(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        (self.regen.geom_rev, self.regen.place_rev).hash(&mut h);
        h.finish()
    }

    /// THE PLACEMENT CHANGED — THE SHAPE OF THE BODIES DID NOT.
    ///
    /// The door for driving a part and for moving a component. `invalidate` declares the GEOMETRY stale,
    /// and on a real assembly that cost 30-48 ms per frame: all 463,878 vertices of the scene buffer were
    /// assembled again although one body had moved. Reported behaviour: the part drags along as if on a
    /// rubber band.
    fn invalidate_placement(&mut self) {
        self.cam_job.program = None;
        self.cam_job.gcode = None;
        self.cam_job.verify = None;
        self.cam_job.sim_mesh = None;
        self.win.sim = false;
        self.regen.place_rev = self.regen.place_rev.wrapping_add(1);
    }

    fn invalidate(&mut self) {
        self.cam_job.program = None;
        self.cam_job.gcode = None;
        self.cam_job.verify = None;
        self.cam_job.sim_mesh = None;
        self.win.sim = false;
        self.regen.geom_rev = self.regen.geom_rev.wrapping_add(1); // invalidate the 3D render cache
    }

    // --- Undo and redo (snapshots) ---

    fn snapshot(&self) -> Snapshot {
        // THE BYTES OF THE EMBEDDED SOURCES DO NOT GO INTO A SNAPSHOT. They never change (they are the
        // original of a file imported once), and they weigh tens of megabytes: on a real assembly 89 MB
        // times 40 undo steps is gigabytes of memory and about 90 MB of memcpy for EVERY committed edit.
        // Restoring puts them back from the live document by id (see `restore`).
        let mut project = self.project.clone_without_source_data();
        project.regen_faces.clear(); // derived: the snapshot holds the faces itself (`faces`)
        project.regen_edges.clear();
        Snapshot { project }
    }

    /// THE DOCUMENT'S FINGERPRINT — the CONTENT alone, with nothing derived.
    ///
    /// One key for two questions: "did anything change outside an operation boundary" (the guard,
    /// `committed_key`) and "is there anything unsaved" ([`edit_key`], `saved_key`). There used to be two
    /// of them, and the second added `geom_rev` — the revision of the DRAWING cache. Any rebuild moves
    /// that, and so does a change of visibility, so "is there anything unsaved" answered "yes" after
    /// simply opening a file. A document key must describe the document; feature parameters now go into
    /// `Project::state_key`, and there is nothing left to prop it up with a picture revision.
    fn doc_key(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        // The model's structure goes through the core's cheap hand-written key (not a single
        // allocation), rather than a RON serialisation of half the document EVERY FRAME (the key is
        // computed in `maybe_commit` on every frame). It also finally SEES the structure: creating an
        // empty part, renaming, moving a component, joints and external references used to leave the
        // document "clean" — closing the window asked nothing about unsaved work, and it was lost
        // silently.
        self.project.state_key().hash(&mut h);
        // The CAM part (operations, tools, machine, stock) still goes through RON for now: those
        // structures stay untouched until CAM comes round, and on CAD documents these vectors are empty
        // and cost next to nothing.
        let p = &self.project;
        for s in [
            ron::ser::to_string(&p.operations).unwrap_or_default(),
            ron::ser::to_string(&p.tools).unwrap_or_default(),
            ron::ser::to_string(&p.machine).unwrap_or_default(),
            ron::ser::to_string(&p.stock).unwrap_or_default(),
        ] {
            s.hash(&mut h);
        }
        h.finish()
    }

    /// THE DOCUMENT CHANGED WITH AUTHORITY, but no undo step is created.
    ///
    /// A third kind of change to the document: neither an edit made by hand nor a rebuild, but
    /// NAVIGATION — entering a component changes `active_component`, and that is stored in the file.
    /// Nobody expects to undo a move like that ("Undo: entering a subassembly" is nonsense), yet it must
    /// not be declared an edit outside a boundary either: that is exactly why the application panicked on
    /// entering a subassembly. The method exists so that such places are VISIBLE and named, rather than
    /// hidden behind a silent update of the key.
    fn doc_touched_without_undo(&mut self) {
        self.edits.committed_key = self.doc_key();
    }

    /// Did the document change outside an operation? (The boundary guard; pulled out so that a test can
    /// check it rather than only a live run — that is precisely why the panic at start-up was not caught
    /// by the tests.)
    #[cfg(test)] // the guard is checked by a test; in production its role is played by the debt record itself
    pub(crate) fn doc_changed_outside_edit(&self) -> bool {
        self.edits.open.is_none() && self.doc_key() != self.edits.committed_key
    }

    /// "IS THERE ANYTHING UNSAVED" — the same document fingerprint the operation-boundary guard uses.
    ///
    /// The name is kept because the question is a different one (it is compared against `saved_key`, not
    /// `committed_key`), but the COMPUTATION must be single: two identical keys would drift apart on an
    /// edit to one of them, and would drift silently. Drifting here is expensive both ways — a needless
    /// question on closing, or lost work.
    fn edit_key(&self) -> u64 {
        self.doc_key()
    }

    fn restore(&mut self, snap: Snapshot) {
        // A snapshot carries THE MESHES but not the live B-rep (`Shape` is not cloneable). `shapes` used
        // to be left over from the undone state: old geometry on screen, new geometry in the kernel, and
        // the NEXT operation built on the undone shape. Silently, because nodes are not dirty after an
        // undo. What gets rebuilt is exactly the bodies whose RECIPE differs between the states (not a
        // forced pass over the whole document — on an assembly of a thousand imports that is tens of
        // seconds).
        let changed = self.project.changed_bodies_vs(&snap.project);
        let mut restored = snap.project;
        restored.take_source_data_from(&mut self.project); // the source bytes come from the live document
        // the derived topology caches never went into the snapshot — bring them back from the live state
        // for the bodies the edit did not touch (the changed ones are rebuilt below anyway)
        let (rf, re) = (std::mem::take(&mut self.project.regen_faces), std::mem::take(&mut self.project.regen_edges));
        self.project = restored;
        self.project.regen_faces = rf;
        self.project.regen_edges = re;
        for b in &changed {
            self.live.shapes.remove(b);
            self.live.faces.remove(b); // the face cache must not outlive an undo either
        }
        let dirty: Vec<Id> = self.project.timeline.iter().filter(|n| n.kind.body().is_some_and(|b| changed.contains(&b))).map(|n| n.id).collect();
        for n in &mut self.project.timeline {
            if dirty.contains(&n.id) {
                n.dirty = true;
            }
        }
        self.cam_job.program = None;
        self.cam_job.gcode = None;
        self.cam_job.verify = None;
        self.cam_job.sim_mesh = None;
        self.win.sim = false;
        self.sel = Sel::None;
        self.regen.geom_rev = self.regen.geom_rev.wrapping_add(1);
        // THE VIEW IS NOT TOUCHED. Restoring a snapshot is an operation on the DOCUMENT, and the point of
        // view belongs to the person: undoing an edit must not throw away the corner they had zoomed in on.
        //
        // Reported behaviour: in the sketcher, picking the measuring tools and the constraints resets the
        // camera. And so it did, by this line: a constraint button that does not fit the current selection
        // is the ordinary way of working - press the button, then pick - and it ends in `abort_edit`, that
        // is here. `view.initialized = false` made the next frame re-fit the whole sketch.
        if !changed.is_empty() {
            self.regenerate_all(); // bring the B-rep of the changed bodies up to the restored state
        }
    }

    /// Commit an undo step once the edit has settled (the pointer is released, nothing is being typed).
    /// A drag is coalesced into one step (committed only when it ends).
    fn maybe_commit(&mut self, ctx: &egui::Context) {
        // A dimension flying after the cursor (`placing_dim`), and typing the dimensions of a rectangle,
        // polygon or ellipse, are edits NOT YET finished: the offset or the value changes every frame, and
        // committing is not allowed (otherwise undo would step through the intermediate states instead of
        // removing the dimension or the shape as a whole). These states are cleared by any new action of
        // the tool, so they do not get stuck.
        let placing = self.place.dim.is_some() || self.place.active();
        let busy = ctx.is_using_pointer() || ctx.wants_keyboard_input() || placing || self.edits.open.is_some();
        if busy {
            return;
        }
        let k = self.doc_key();
        if k == self.edits.committed_key {
            return;
        }
        // THE BOUNDARY HAS BECOME THE ONLY PATH. A per-frame safety net used to stand here, and it was
        // the reason undo knew no names, depended on the state of the mouse and did not exist outside a
        // window. Now the step is created by the operation, and this place only WATCHES: if the document
        // changed outside `App::edit`, some place is editing it around the boundary. The edit is not
        // lost, but the place has to be found and moved onto the boundary rather than swept under a
        // snapshot.
        // THE GUARD DOES NOT BRING THE APPLICATION DOWN. A `debug_assert!(false)` used to stand here, and
        // that was a mistake: every UI path not yet moved onto `App::edit` turned into a panic of the
        // debug build — entering a subassembly, turning the gizmo. A tool that breaks someone's work in
        // order to tell a developer about their own debt is unacceptable. Now the place is RECORDED (once
        // per call site), the edit is kept by a snapshot, work carries on, and the list of offenders is
        // visible in the log and moved over one at a time.
        #[cfg(debug_assertions)]
        {
            let bt = std::backtrace::Backtrace::force_capture().to_string();
            let site: String = bt
                .lines()
                .filter(|l| l.contains("qymcad::gui"))
                .nth(2)
                .unwrap_or("undo-unknown")
                .trim()
                .to_string();
            if self.edits.debt.insert(site.clone()) {
                eprintln!("[operation boundary] the document was changed outside App::edit: {site}");
            }
        }
        let snap = self.snapshot();
        let cur = std::mem::replace(&mut self.edits.baseline, snap);
        self.edits.undo.push(Step { name: crate::i18n::tr("undo-edit"), snap: cur });
        if self.edits.undo.len() > self.set.undo_cap.max(1) {
            self.edits.undo.remove(0);
        }
        self.edits.redo.clear();
        self.edits.committed_key = k;
    }

    /// OPEN A LASTING OPERATION (one that lives across frames: a sketch editing session, a drag).
    /// The `Edit` guard holds `&mut App` and suits only a short operation inside a single call; a session
    /// is closed explicitly, by `commit_edit` or `abort_edit`.
    pub(crate) fn begin_edit(&mut self, name: impl Into<String>) {
        if self.edits.open.is_none() {
            let name = name.into();
            // THE TRAIL FOR A CRASH REPORT IS WRITTEN HERE, NOT AT THE COMMIT. A crash happens in the
            // middle of an operation, so a trail of committed steps is missing exactly the one that
            // killed the program - the only one worth having.
            crate::crash::note_step(&name);
            self.edits.open = Some((name, Snapshot { project: self.project.clone() }));
        }
        self.edits.depth += 1;
    }

    /// Close a lasting operation: a named step goes onto the undo stack.
    pub(crate) fn commit_edit(&mut self) {
        self.edits.depth = self.edits.depth.saturating_sub(1);
        if self.edits.depth > 0 {
            return; // a nested operation: the outer one will sum it up
        }
        let Some((name, before)) = self.edits.open.take() else { return };
        self.edits.undo.push(Step { name, snap: before });
        self.regen.pending = false;
        self.rebuild_if_dirty(); // ONE point: the scheduler decides for itself whether to compute
        if self.edits.undo.len() > self.set.undo_cap.max(1) {
            self.edits.undo.remove(0);
        }
        self.edits.redo.clear();
        self.edits.baseline = Snapshot { project: self.project.clone() };
        self.edits.committed_key = self.doc_key();
    }

    /// Abort a lasting operation: the document comes back as it was, and no step is left behind.
    pub(crate) fn abort_edit(&mut self) {
        self.edits.depth = self.edits.depth.saturating_sub(1);
        if self.edits.depth > 0 {
            return;
        }
        let Some((_, before)) = self.edits.open.take() else { return };
        self.regen.pending = false; // the operation was rolled back: there is nothing to rebuild
        self.restore(before);
        self.edits.committed_key = self.doc_key();
    }

    /// OPEN AN OPERATION on the document. Everything that changes the document must go through it.
    /// A nested call joins the operation already open: one action, one undo step.
    pub(crate) fn edit(&mut self, name: impl Into<String>) -> Edit<'_> {
        let outer = self.edits.open.is_none();
        self.begin_edit(name);
        Edit { app: self, outer, done: false }
    }

    fn undo(&mut self) {
        if let Some(step) = self.edits.undo.pop() {
            let cur = std::mem::replace(&mut self.edits.baseline, step.snap.clone());
            self.edits.redo.push(Step { name: step.name.clone(), snap: cur });
            self.restore(step.snap);
            self.edits.committed_key = self.doc_key();
            self.status = crate::i18n::tr1("g-undone", "what", &step.name);
        }
    }

    fn redo(&mut self) {
        if let Some(step) = self.edits.redo.pop() {
            let cur = std::mem::replace(&mut self.edits.baseline, step.snap.clone());
            self.edits.undo.push(Step { name: step.name.clone(), snap: cur });
            self.restore(step.snap);
            self.edits.committed_key = self.doc_key();
            self.status = crate::i18n::tr1("g-redone", "what", &step.name);
        }
    }

    fn toggle_sim(&mut self) {
        if self.win.sim {
            self.win.sim = false;
            return;
        }
        if self.project.contours.is_empty() && self.project.bodies.is_empty() {
            self.status = crate::i18n::tr("cam-no-geometry-sim");
            return;
        }
        // resolution: about the bounding box over 200, kept within sensible limits
        let cell = self
            .project
            .bounds()
            .map(|b| ((b.max.x - b.min.x).max(b.max.y - b.min.y) / 200.0).clamp(0.3, 2.0))
            .unwrap_or(0.5);
        match self.project.simulate(&self.program_name(), cell) {
            Some(mesh) => {
                let tris = mesh.tris.len();
                self.cam_job.sim_mesh = Some(mesh);
                self.win.sim = true;
                self.mode_3d = true;
                self.cam.init = false;
                self.status = crate::i18n::tr1("cam-sim-result", "n", &tris.to_string());
            }
            None => self.status = crate::i18n::tr("cam-sim-failed"),
        }
    }

    fn generate(&mut self) {
        if self.project.contours.is_empty() && self.project.bodies.is_empty() {
            self.status = crate::i18n::tr("cam-no-geometry");
            return;
        }
        let name = self.program_name();
        let program = self.project.build_program(&name);
        if program.toolpaths.is_empty() {
            self.status = crate::i18n::tr("cam-no-toolpaths");
            self.invalidate();
            return;
        }
        let m = &self.project.machine;
        let gcode = post_for(&program, m.post, &self.post_options());
        let vopts = VerifyOptions { rapid_rate: m.max_rapid, limits: Some((m.work_min, m.work_max)) };
        let verify = verify_gcode(&gcode, &vopts);
        let moves: usize = program.toolpaths.iter().map(|t| t.moves.len()).sum();
        let warn = if verify.errors.is_empty() {
            String::new()
        } else {
            format!("  {} {}", ph::WARNING, crate::i18n::tr1("cam-out-of-table-n", "n", &verify.errors.len().to_string()))
        };
        self.status = format!(
            "{}{warn}",
            crate::i18n::trn(
                "cam-program-summary",
                &[
                    ("ops", &program.toolpaths.len().to_string()),
                    ("moves", &moves.to_string()),
                    ("post", &crate::i18n::tr(m.post.label())),
                    (
                        "box",
                        &format!(
                            "X[{:.1}..{:.1}] Y[{:.1}..{:.1}] Z[{:.1}..{:.1}]",
                            verify.bounds_min[0],
                            verify.bounds_max[0],
                            verify.bounds_min[1],
                            verify.bounds_max[1],
                            verify.bounds_min[2],
                            verify.bounds_max[2]
                        )
                    ),
                    ("min", &format!("{:.1}", verify.seconds / 60.0)),
                ]
            )
        );
        self.cam_job.program = Some(program);
        self.cam_job.gcode = Some(gcode);
        self.cam_job.verify = Some(verify);
    }

    fn export(&mut self) {
        let Some(gcode) = &self.cam_job.gcode else {
            self.status = crate::i18n::tr("cam-generate-first");
            return;
        };
        let default = format!("{}.tap", self.program_name());
        if let Some(path) = rfd::FileDialog::new().set_file_name(default).add_filter("G-code", &["tap", "nc", "ngc"]).save_file() {
            match std::fs::write(&path, gcode) {
                Ok(()) => self.status = crate::i18n::tr1("cam-saved-path", "path", &path.display().to_string()),
                Err(e) => self.status = crate::i18n::tr1("io-write-error", "error", &e.to_string()),
            }
        }
    }

    // --- Exporting 3D (STEP, STL) and sketches (SVG, DXF) ------------------------------------
    /// The VISIBLE bodies of the target: for the whole document, every live body; for a component, the
    /// bodies of its subtree (itself plus its descendants). "Visible" means the body's own tick
    /// (`mesh_visible[idx]`) AND the owner's hierarchical visibility (`component_chain_visible` up to the
    /// root) — a hidden part or subassembly does not reach the export.
    fn visible_export_bodies(&self, target: ExportTarget) -> Vec<Id> {
        let subtree: Option<std::collections::HashSet<Id>> = match target {
            ExportTarget::Project => None,
            ExportTarget::Component(cid) => {
                let mut s: std::collections::HashSet<Id> = self.project.descendants(cid).into_iter().collect();
                s.insert(cid);
                Some(s)
            }
        };
        // CRITICAL: the CONSUMED bodies are excluded (the bases eaten by cuts, booleans and modifiers). In
        // 3D they are hidden by a separate filter, `consumed_bodies`, not by `mesh_visible`, so the export
        // used to pull them in TOGETHER with the final body — the bases covered the cuts, and the STEP or
        // STL came out as a solid blank with nothing cut away.
        let consumed = self.project.consumed_bodies();
        self.project
            .bodies
            .iter()
            .map(|b| b.id)
            .enumerate()
            .map(|(mi, b)| (mi, b))
            .filter(|&(mi, _)| self.project.bodies.get(mi).is_none_or(|b| b.visible))
            .filter(|&(_, b)| !consumed.contains(&b))
            .filter(|&(_, b)| self.project.body_owner(b).is_none_or(|o| self.component_chain_visible(o, None)))
            .filter(|&(_, b)| match &subtree {
                None => true,
                Some(s) => self.project.body_owner(b).is_some_and(|o| s.contains(&o)),
            })
            .map(|(_, b)| b)
            .collect()
    }

    /// The indices of the bodies highlighted as "selected" in 3D. Select a body and that body lights up;
    /// select a component (a part or a subassembly) in the tree or by a click in the assembly and its
    /// WHOLE subtree lights up, so that it is plain that this part or subassembly was selected entire.
    fn highlight_mesh_set(&self) -> std::collections::HashSet<usize> {
        let mut hl = std::collections::HashSet::new();
        match self.sel {
            Sel::Mesh(i) | Sel::Face(i, _) => {
                hl.insert(i);
            }
            Sel::Component(ci) => {
                if let Some(cid) = self.project.components.get(ci).map(|c| c.id) {
                    let mut sub: std::collections::HashSet<Id> = self.project.descendants(cid).into_iter().collect();
                    sub.insert(cid);
                    for (mi, b) in self.project.bodies.iter().map(|b| b.id).enumerate() {
                        if self.project.body_owner(b).is_some_and(|o| sub.contains(&o)) {
                            hl.insert(mi);
                        }
                    }
                }
            }
            _ => {}
        }
        hl
    }

}

impl eframe::App for App {
    /// Save the settings between sessions (the machine plus the view preferences).
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, "machine", &self.project.machine);
        eframe::set_value(storage, "machine_lib", &self.cam_job.machines);
        eframe::set_value(storage, "settings", &self.set);
        eframe::set_value(storage, "last_project", &self.project_path); // the path of the current document
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // The canvas of this frame, for a problem report: three atomic stores, so it costs nothing at
        // this rate. Taken before the prologue can return - a report is wanted most when the start-up
        // load is what went wrong.
        crate::diagnostics::note_viewport(ctx.screen_rect().size(), ctx.pixels_per_point());
        // THE FRAME PROLOGUE: until it says "carry on", there is nothing to draw (the start-up load is running).
        if !self.frame_prologue(ctx) {
            return;
        }
        // Ctrl+S saves (silently into the current file, or a dialogue for a new one); Ctrl+Shift+S is "save as".
        if !ctx.wants_keyboard_input() && ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::S)) {
            if ctx.input(|i| i.modifiers.shift) {
                self.save_project_as();
            } else {
                self.save_project();
            }
        }
        // the indicator of a background rebuild sits over the ordinary interface, with dimming
        if let Some(label) = self.dim.overlay.take() {
            let progress = self.dim.overlay_progress.take();
            // A LIVE VIEWPORT during a rebuild: the rectangle is taken from the PREVIOUS frame — the
            // overlay is drawn before the viewport itself, and the window size does not jump between frames.
            let live = if matches!(&self.regen.busy, Some(b) if b.kind == BgKind::Regen) { self.view_rect } else { egui::Rect::NOTHING };
            if self.draw_dim_overlay_with(ctx, &label, progress, live) {
                self.cancel_regen();
            }
        }
        // A SILENT REBUILD gets only a spinner on the canvas. No window, no dimming, no barrier: nobody is
        // kept from working, and nobody is left guessing whether what is on screen is still current.
        if std::mem::take(&mut self.dim.spinner) {
            self.draw_quiet_spinner(ctx);
        }
        self.sync_workbench(); // the workbench and the active context are derived from `active_path` (drill in and out)
        self.hover.joint = None; // the hovered joint is rebuilt every frame (by the panel and by 3D below)
        self.keep_selection_on_edited_sketch(); // the selection follows the sketch being edited — in one phase
        self.handle_key_commands(ctx); // the frame's keyboard commands — in one phase
        self.handle_tool_hotkeys(ctx); // the tool shortcuts (L/R/C/A/P/G/D/S, E)
        self.maybe_autosave(false); // a silent autosave every 3 minutes while there are unsaved edits
        self.help_window(ctx); // the help window
        self.save_template_dialog(ctx); // "save as a template" — the name and the confirmation
        self.confirm_delete_popup(ctx); // the popup confirming the deletion of a tree node
        self.menu_bar(ctx);
        self.take_screenshot(ctx); // the picture of the window for a report comes back as an event
        self.crash_notice(ctx); // "the last run ended in an error" - only after a crash
        self.report_window(ctx); // Help -> Report a problem
        self.about_dialog(ctx); // the About window
        self.doc_props_window(ctx); // the document properties
        self.start_screen(ctx); // where to begin — only on a blank slate
        self.nav_dialog(ctx); // the modal "save the changes?" over the menu
        self.stl_quality_dialog(ctx); // choosing the STL quality before exporting
        self.toolbar(ctx);
        self.section_bar(ctx); // the section control bar — drawing the UI, not a phase of the frame
        // "Finish" lives in one place: the button in the breadcrumbs (the toolbar). There is no separate banner.
        self.tick_view_anim(ctx); // the smooth turn of the view (the ViewCube)
        self.tick_joint_anim(ctx); // sweeping a joint's degree of freedom
        self.hotkeys_window(ctx); // Help -> Shortcuts
        self.command_search_window(ctx); // the command search (Space or Ctrl+K)
        self.comp_array_bar(ctx); // the component pattern bar (assembly)
        self.feat_command_bar(ctx); // the bar of the active Part command — a panel, not a phase of the frame
        self.tool_options_bar(ctx);
        self.joint_tool_bar(ctx); // the top bar for creating a joint
        self.joint_edit_bar(ctx); // the top bar for EDITING a joint (a double click on its glyph)
        self.bool_tool_bar(ctx); // the top bar for body-to-body booleans: the kind, and picking body B
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(&self.status);
                // THE SKETCH'S DEFINEDNESS is the sketcher's main state, and in CAD it belongs in the status
                // line: one looks at the drawing rather than hunting for it in a panel off to the side.
                if let Some(sid) = self.sketch_ses.editing {
                    if let Some(si) = self.project.sketches.iter().position(|s| s.id == sid) {
                        let (line, col) = self.sketch_dof_line(si);
                        ui.separator();
                        ui.label(egui::RichText::new(line).color(col));
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(&crate::i18n::tr("cam-mm"));
                    ui.separator();
                    match self.cursor {
                        Some(c) => ui.monospace(format!("X {:.2}  Y {:.2}", c.x, c.y)),
                        None => ui.monospace("X —  Y —"),
                    };
                    if self.cam_job.program.is_some() {
                        ui.separator();
                        ui.label(&crate::i18n::tr("cam-progress"));
                        ui.add(egui::Slider::new(&mut self.cam_job.progress, 0.0..=1.0).show_value(false));
                    }
                });
            });
        });
        self.wb_toolbar(ctx);
        // while a sketch is being edited the tree on the left is not needed — only the tools
        if self.edit_si().is_none() {
            self.tree_panel(ctx);
        }
        self.properties_panel(ctx);
        if let Some(i) = self.io.export_op.take() {
            self.export_op(i);
        }
        self.gcode_window(ctx);
        self.tools_window(ctx);
        self.parts_library_window(ctx);
        self.save_part_window(ctx);
        self.params_window(ctx);
        self.machines_window(ctx);
        self.settings_window(ctx);
        self.viewport(ctx);
        // commit an undo step if the edit has finished
        self.maybe_commit(ctx);
    }
}

impl App {
    fn gcode_window(&mut self, ctx: &egui::Context) {
        if !self.win.gcode || !self.set.cam_tab_enabled {
            return;
        }
        let mut open = self.win.gcode;
        egui::Window::new("G-code").open(&mut open).default_size([460.0, 560.0]).show(ctx, |ui| {
            if let Some(g) = &self.cam_job.gcode {
                ui.label(crate::i18n::tr1("cam-lines-count", "n", &g.lines().count().to_string()));
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let mut text = g.as_str();
                    ui.add(egui::TextEdit::multiline(&mut text).font(egui::TextStyle::Monospace).desired_width(f32::INFINITY).code_editor());
                });
            }
        });
        self.win.gcode = open;
    }

    /// Toggle the parts library window; on the first opening it builds the catalogue tree.
    fn toggle_parts_library(&mut self) {
        self.win.parts_library = !self.win.parts_library;
        if self.win.parts_library && self.parts.tree.is_none() {
            self.parts.tree = Some(crate::parts_library::LibraryTree::load());
        }
    }


    /// Draw a category node of the tree, recursively. `path` holds the indices from the root of the level
    /// (mutated by push and pop).
    fn parts_tree_node(
        ui: &mut egui::Ui,
        node: &crate::parts_library::CatNode,
        tier: bool,
        path: &mut Vec<usize>,
        sel: &mut Option<(bool, Vec<usize>)>,
    ) {
        let is_sel = matches!(sel, Some((t, p)) if *t == tier && p == path);
        let title = format!("{}  {} ({})", ph::FOLDER_OPEN, node.title, node.total_parts());
        if node.subcats.is_empty() {
            if ui.selectable_label(is_sel, title).clicked() {
                *sel = Some((tier, path.clone()));
            }
        } else {
            let id = ui.make_persistent_id(("parts_cat", tier, &*path));
            egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, path.is_empty())
                .show_header(ui, |ui| {
                    if ui.selectable_label(is_sel, title).clicked() {
                        *sel = Some((tier, path.clone()));
                    }
                })
                .body(|ui| {
                    for (i, c) in node.subcats.iter().enumerate() {
                        path.push(i);
                        Self::parts_tree_node(ui, c, tier, path, sel);
                        path.pop();
                    }
                });
        }
    }

    /// The category node at a path of indices from the root of the level.
    fn cat_at<'a>(root: &'a crate::parts_library::CatNode, path: &[usize]) -> Option<&'a crate::parts_library::CatNode> {
        let mut n = root;
        for &i in path {
            n = n.subcats.get(i)?;
        }
        Some(n)
    }

    /// Gather every part of the subtree whose name or tags contain `query` (in lower case).
    fn collect_matching<'a>(
        node: &'a crate::parts_library::CatNode,
        query: &str,
        out: &mut Vec<&'a crate::parts_library::PartEntry>,
    ) {
        for p in &node.parts {
            let name_hit = p.name.to_lowercase().contains(query);
            let tag_hit = p
                .manifest
                .as_ref()
                .map(|m| m.tags.iter().any(|t| t.to_lowercase().contains(query)))
                .unwrap_or(false);
            if name_hit || tag_hit {
                out.push(p);
            }
        }
        for c in &node.subcats {
            Self::collect_matching(c, query, out);
        }
    }

    /// The target assembly for an insertion: the active assembly; if a Part is active, its parent; otherwise the root.
    fn parts_insert_target(&mut self) -> qymcad_core::model::Id {
        let root = self.project.ensure_root();
        let ctx = self.project.current_ctx();
        if ctx != 0 && !self.project.component_is_part(ctx) {
            return ctx;
        }
        self.project
            .components
            .iter()
            .find(|c| c.id == ctx)
            .and_then(|c| c.parent)
            .unwrap_or(root)
    }

    /// Load a `.qpart` from a source and graft it into the target assembly (`graft`, then `regenerate_all`).
    fn insert_part_from(&mut self, src: crate::parts_library::PartSource) {
        let loaded = match &src {
            crate::parts_library::PartSource::User(p) => qymcad_io::load_part(&p.to_string_lossy()),
            crate::parts_library::PartSource::Embedded(rel) => match crate::parts_library::embedded_bytes(rel) {
                Some(b) => qymcad_io::load_part_bytes(b),
                None => Err(crate::i18n::tr("lib-part-not-found")),
            },
        };
        let loaded = match loaded {
            Ok(l) => l,
            Err(e) => {
                self.status = crate::i18n::tr1("lib-load-error", "error", &e.to_string());
                return;
            }
        };
        let target = self.parts_insert_target();
        match self.project.graft(&loaded.project, target) {
            Some(_) => {
                self.mark_dirty_for_rebuild(); // the document is marked; the scheduler does the computing
                self.invalidate();
                self.status = crate::i18n::tr1("lib-part-inserted", "name", &loaded.manifest.name);
            }
            None => {
                self.status = crate::i18n::tr("lib-no-assembly");
            }
        }
    }

    /// The thumbnail texture of a part for the library grid (`thumb.png` is loaded lazily from the
    /// `.qpart`, decoded, turned into a GPU texture, and cached by source). None means there is no preview
    /// (an older part with no thumbnail), and the grid shows a placeholder icon. The cache is cleared when
    /// the catalogue is rebuilt or something is saved.
    fn parts_thumb_texture(&mut self, ctx: &egui::Context, src: &crate::parts_library::PartSource) -> Option<egui::TextureHandle> {
        use crate::parts_library::PartSource;
        let key = match src {
            PartSource::Embedded(s) => format!("e:{s}"),
            PartSource::User(p) => format!("u:{}", p.to_string_lossy()),
        };
        if let Some(cached) = self.parts.thumbs.get(&key) {
            return cached.clone();
        }
        let png = match src {
            PartSource::User(p) => qymcad_io::load_part_thumb(&p.to_string_lossy()),
            PartSource::Embedded(rel) => crate::parts_library::embedded_bytes(rel).and_then(qymcad_io::load_part_thumb_bytes),
        };
        let tex = png
            .and_then(|bytes| image::load_from_memory(&bytes).ok())
            .map(|img| {
                let rgba = img.to_rgba8();
                let (w, h) = rgba.dimensions();
                let color = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], rgba.as_raw());
                ctx.load_texture(format!("part_thumb_{key}"), color, egui::TextureOptions::LINEAR)
            });
        self.parts.thumbs.insert(key, tex.clone());
        tex
    }

    /// Open the "Save as a standard part" dialogue for component `cid`. The name comes from the component,
    /// the preview is rendered right away (the body in its own frame), and the list of existing categories
    /// is there for picking one quickly.
    fn open_save_part_dialog(&mut self, cid: qymcad_core::model::Id) {
        let name = self.project.components.iter().find(|c| c.id == cid).map(|c| crate::i18n::name(&c.name)).unwrap_or_default();
        let preview = self.render_component_thumbnail(cid);
        self.parts.save = Some(SavePartDialog {
            component: cid,
            name,
            description: String::new(),
            tags: String::new(),
            category: String::new(),
            known_cats: crate::parts_library::user_category_paths(),
            preview,
            tex: None,
        });
        self.status = crate::i18n::tr("lib-save-hint");
    }

    /// Write a `.qpart` out of the open dialogue — `subproject_of(component)` plus `save_part`. Returns the path.
    fn commit_save_part(&mut self) -> Result<String, String> {
        let d = self.parts.save.as_ref().ok_or(&crate::i18n::tr("lib-no-dialog"))?;
        let name = d.name.trim().to_string();
        if name.is_empty() {
            return Err(crate::i18n::tr("lib-need-name"));
        }
        let root = crate::parts_library::user_parts_dir().ok_or(&crate::i18n::tr("lib-no-user-dir"))?;
        let cat = d.category.trim().trim_matches('/').trim();
        let dir = if cat.is_empty() { root } else { root.join(cat) };
        std::fs::create_dir_all(&dir).map_err(|e| crate::i18n::tr1("lib-category-error", "error", &e.to_string()))?;
        let stem = crate::parts_library::sanitize_part_stem(&name);
        let path = dir.join(format!("{stem}.qpart"));
        let sub = self.project.subproject_of(d.component).ok_or(&crate::i18n::tr("lib-no-root"))?;
        let tags: Vec<String> = d.tags.split([',', ';', '\n']).flat_map(|s| s.split_whitespace()).map(str::trim).filter(|s| !s.is_empty()).map(String::from).collect();
        let png = d.preview.as_ref().and_then(Self::color_image_to_png);
        let manifest = qymcad_core::part::PartManifest { schema_version: qymcad_core::part::PART_SCHEMA, name, description: d.description.trim().to_string(), tags, author: String::new() };
        qymcad_io::save_part(&sub, &manifest, &[], png.as_deref(), &path.to_string_lossy())?;
        // the catalogue is re-read, so the new part appears in the "My parts" tree at once
        self.parts.tree = Some(crate::parts_library::LibraryTree::load());
        // the old thumbnails go to the graveyard (freed later); textures must NOT be dropped in the current frame
        self.tex_graveyard.extend(self.parts.thumbs.drain().filter_map(|(_, v)| v));
        Ok(path.to_string_lossy().to_string())
    }

    /// A PNG out of a `ColorImage` (RGBA8) — for the `thumb.png` inside a `.qpart`. Uses `image` with its png feature.
    fn color_image_to_png(img: &egui::ColorImage) -> Option<Vec<u8>> {
        use image::ImageEncoder;
        let (w, h) = (img.size[0] as u32, img.size[1] as u32);
        let mut rgba: Vec<u8> = Vec::with_capacity(img.pixels.len() * 4);
        for p in &img.pixels {
            rgba.extend_from_slice(&p.to_array());
        }
        let mut png = Vec::new();
        image::codecs::png::PngEncoder::new(&mut png).write_image(&rgba, w, h, image::ExtendedColorType::Rgba8).ok()?;
        Some(png)
    }



    /// The separate Machine window — configuring the working area and the post-processor.
    fn machines_window(&mut self, ctx: &egui::Context) {
        if !self.win.machines || !self.set.cam_tab_enabled {
            return;
        }
        let mut open = self.win.machines;
        egui::Window::new(format!("{} {}", ph::WRENCH, crate::i18n::tr("cam-machine-title"))).open(&mut open).default_size([360.0, 420.0]).show(ctx, |ui| {
            self.machine_props(ui);
        });
        self.win.machines = open;
    }


    fn setup_sheet_html(&self, v: &VerifyResult) -> String {
        let p = &self.project;
        let m = &p.machine;
        let mut tools = String::new();
        for t in &p.tools {
            tools.push_str(&format!(
                "<tr><td>T{}</td><td>{}</td><td>{}</td><td>{:.2}</td><td>{}</td></tr>",
                t.number, t.name, crate::i18n::tr(tool_type_label(t.kind)), t.diameter, t.flutes
            ));
        }
        let mut ops = String::new();
        for (i, op) in p.operations.iter().enumerate() {
            ops.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>T{}</td><td>{:.2}</td><td>{}</td></tr>",
                i + 1,
                op.name,
                op.kind.label(),
                op.tool,
                op.heights.bottom,
                if op.enabled { crate::i18n::tr("cam-on") } else { crate::i18n::tr("cam-off") }
            ));
        }
        // THE SHEET IS A DOCUMENT FOR A PERSON, so its captions come from the catalogue like every other
        // line of the interface. The numbers are formatted here rather than by Fluent: this is an
        // engineering document, and a value has to read the same in every language.
        let name = self.program_name();
        let bounds = crate::i18n::trn(
            "cam-sheet-bounds",
            &[
                ("x0", &crate::i18n::num(v.bounds_min[0], 1)),
                ("x1", &crate::i18n::num(v.bounds_max[0], 1)),
                ("y0", &crate::i18n::num(v.bounds_min[1], 1)),
                ("y1", &crate::i18n::num(v.bounds_max[1], 1)),
                ("z0", &crate::i18n::num(v.bounds_min[2], 1)),
                ("z1", &crate::i18n::num(v.bounds_max[2], 1)),
            ],
        );
        let time = crate::i18n::tr2(
            "cam-sheet-time",
            "min",
            &crate::i18n::num(v.seconds / 60.0, 1),
            "cut",
            &crate::i18n::num(v.cut_length, 0),
        );
        format!(
            "<!doctype html><html><head><meta charset=\"utf-8\"><title>{title}</title>\
<style>body{{font-family:sans-serif;margin:24px;color:#222}}h1{{font-size:20px}}\
table{{border-collapse:collapse;margin:8px 0}}td,th{{border:1px solid #ccc;padding:4px 8px;font-size:13px}}\
th{{background:#f0f0f0;text-align:left}}.k{{color:#666}}</style></head><body>\
<h1>{heading}</h1>\
<p class=k>{machine}</p>\
<p>{bounds}<br>{time}</p>\
<h2>{h_tools}</h2><table><tr><th>T</th><th>{c_name}</th><th>{c_type}</th><th>Ø</th><th>{c_flutes}</th></tr>{tools}</table>\
<h2>{h_ops}</h2><table><tr><th>#</th><th>{c_name}</th><th>{c_type}</th><th>{c_tool}</th><th>{c_bottom}</th><th></th></tr>{ops}</table>\
</body></html>",
            title = crate::i18n::tr1("cam-sheet-title", "name", &name),
            heading = crate::i18n::tr1("cam-sheet-heading", "name", &name),
            machine = crate::i18n::tr2("cam-sheet-machine", "machine", &m.name, "post", m.post.label()),
            bounds = bounds,
            time = time,
            h_tools = crate::i18n::tr("cam-sheet-tools"),
            h_ops = crate::i18n::tr("cam-sheet-operations"),
            c_name = crate::i18n::tr("cam-sheet-col-name"),
            c_type = crate::i18n::tr("cam-sheet-col-type"),
            c_flutes = crate::i18n::tr("cam-sheet-col-flutes"),
            c_tool = crate::i18n::tr("cam-sheet-col-tool"),
            c_bottom = crate::i18n::tr("cam-sheet-col-bottom"),
            tools = tools,
            ops = ops,
        )
    }

    fn open_step(&mut self, path: String) {
        // the heavy STEP work (parsing, tessellation, `step_solids`) runs in a worker thread; the UI shows
        // an "importing STEP" spinner and does not freeze (large assemblies carry fat imports).
        let (tx, rx) = std::sync::mpsc::channel();
        let p = path.clone();
        std::thread::spawn(move || {
            let res = match qymcad_kernel::import_step(&p, 0.5) {
                Ok(bodies) if bodies.is_empty() => JobResult::Failed(crate::i18n::tr("io-step-no-solids")),
                Ok(bodies) => {
                    // the live B-rep shapes of the solids (in the same order as the bodies: both come from the file's solids)
                    let shapes = qymcad_kernel::step_solids(&p).unwrap_or_default();
                    JobResult::StepImported { path: p, bodies, shapes }
                }
                Err(e) => JobResult::Failed(crate::i18n::tr1("io-step-load-error", "error", &e.to_string())),
            };
            let _ = tx.send(res);
        });
        self.regen.busy = Some(Busy { label: crate::i18n::tr("io-step-importing"), rx, kind: BgKind::ImportShapes, pulse: None, quiet: false });
    }

    // ============ The splash screen and the progress of background work ============

    /// Load the logo lazily (a 256x256 PNG embedded in the binary) into a texture for the splash screen.
    fn ensure_logo(&mut self, ctx: &egui::Context) {
        if self.logo_tex.is_some() {
            return;
        }
        const LOGO: &[u8] = include_bytes!("../../../assets/icons/linux/256x256.png");
        if let Ok(img) = image::load_from_memory(LOGO) {
            let rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();
            let color = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], rgba.as_raw());
            self.logo_tex = Some(ctx.load_texture("app-logo", color, egui::TextureOptions::LINEAR));
        }
    }


    /// Recompute the WHOLE timeline from scratch (the Edit -> Rebuild everything item). Every node is
    /// marked dirty and the work goes into a background regeneration — the screen does not collapse, an
    /// overlay spins instead.
    fn rebuild_everything(&mut self) {
        for n in self.project.timeline.iter_mut() {
            n.dirty = true;
        }
        self.live.shapes.clear(); // the live B-rep is built anew as well, otherwise features would sit on old faces
        // AND ADMIT THAT IT IS GONE. Two flags used to be left as they were, describing exactly what the
        // command had just thrown away: "the preparation is finished" and "this revision has already been
        // tried". After the rebuild the program considered the live geometry ready although no body had
        // any, and a repeat preparation bailed out immediately on the "already tried" guard.
        //
        // Reported behaviour: pressing Rebuild everything changed nothing, the B-rep still would not
        // build. The one command that could have put things right declared the work done.
        self.live.ready = false;
        self.live.tried_rev = None;
        self.spawn_regen();
        // AND BRING BACK WHAT THE TIMELINE CANNOT.
        //
        // The live B-rep has been thrown away for ALL the bodies. Those built by the timeline get it back
        // from the rebuild; imported ones do not: their geometry lives in the embedded STEP rather than in
        // a recipe, and a separate path raises it. Without calling that path, the command took the live
        // geometry of a whole imported assembly away until the program was restarted — and left the B-rep
        // preparation with no hope of ever finishing.
        //
        // MEASURED IN A LIVE WINDOW on a document with 138 imported bodies: after the command there were
        // "0 live shapes", and every following rebuild arrived with a plan of "0 nodes out of 142" — there
        // was nothing for it to do. Reported behaviour: pressing Rebuild everything sent the CAD into a
        // fever of endless flickering.
        self.spawn_import_shapes(false); // quietly, in the background: the model on screen is intact, only the geometry is awaited
        self.regen.import_asked = true;
        self.status = crate::i18n::tr("io-rebuilding-all");
    }




    /// After a component's placement changes: with external references present, rebuild the consumers
    /// (top-down associativity — the source face travelled with the part); otherwise simply invalidate the cache.
    fn after_placement_change(&mut self) {
        // a mirrored part is placed FREELY by the gizmo, so moving either the source or the mirror needs no
        // regeneration of the shape (the plane is fixed in the source's local frame)
        if !self.project.external_refs.is_empty() {
            self.project.mark_external_consumers_dirty(); // the consumers' `sketch_frame` has moved, so rebuild
            self.mark_dirty_for_rebuild(); // the document is marked; the scheduler does the computing
        } else {
            self.invalidate();
        }
    }



    /// The datum's visibility tick in the tree (a stable Id against `datum_hidden`). Shared by planes, points and axes.
    fn datum_vis_checkbox(&mut self, ui: &mut egui::Ui, id: Id) {
        let mut vis = !self.datum.hidden.contains(&id);
        if ui.add(egui::Checkbox::without_text(&mut vis)).on_hover_text(&crate::i18n::tr("datum-vis-hint")).changed() {
            if vis {
                self.datum.hidden.remove(&id);
            } else {
                self.datum.hidden.insert(id);
            }
        }
    }

    /// The title of a tree node: the selected one is highlighted. A method rather than a free function:
    /// the selection colour belongs to the scheme, and a function without `&self` cannot ask the scheme —
    /// exactly the fault that kept the tool bar from being recoloured by the theme.
    fn sel_title(&self, text: String, selected: bool) -> egui::RichText {
        let rt = egui::RichText::new(text);
        if selected {
            rt.color(self.scheme.pal.tree_selected()).strong()
        } else {
            rt
        }
    }

    /// The icon of a feature in the tree (by its kind) — shared with the labels of `tree_feature_row` (which shows a custom name).
    fn feat_icon(kind: &qymcad_core::feature::FeatureKind) -> &'static str {
        use qymcad_core::feature::FeatureKind as FK;
        match kind {
            FK::Cylinder { .. } => ph::CYLINDER,
            FK::Sphere { .. } | FK::Torus { .. } | FK::Fillet { .. } | FK::Hole { .. } => ph::CIRCLE,
            FK::Combine { op, .. } => [ph::SCISSORS, ph::CUBE, ph::INTERSECT][(*op as usize).min(2)],
            FK::BodyBoolean { .. } => ph::INTERSECT,
            FK::PushFace { .. } => ph::ARROWS_OUT_LINE_VERTICAL,
            FK::RemoveFace { .. } => ph::ERASER,
            FK::SplitBody { .. } => ph::SQUARE_SPLIT_HORIZONTAL,
            FK::PartInstance { .. } => ph::CUBE_TRANSPARENT, // NOT the assembly icon: there is nothing to enter here (see panels.rs)
            FK::Thicken { .. } => ph::STACK_SIMPLE,
            FK::SplitFace { .. } => ph::GRID_FOUR,
            FK::Chamfer { .. } => ph::TRIANGLE,
            FK::Prism { .. } => ph::HEXAGON,
            FK::Shell { .. } => ph::BOUNDING_BOX,
            FK::LinearArray { .. } => ph::DOTS_THREE_OUTLINE,
            FK::CircularArray { .. } => ph::ARROWS_CLOCKWISE,
            FK::Mirror { .. } => ph::FLIP_HORIZONTAL,
            FK::Move { .. } => ph::ARROWS_OUT_CARDINAL,
            FK::Thread { .. } => ph::SPIRAL,
            _ => ph::CUBE,
        }
    }

    /// A feature's default name (as the `add_*` methods set it) — used to tell a RENAMED feature from a
    /// default one, so that the tree shows the name given by hand.
    /// A FEATURE'S DEFAULT NAME IS A CATALOGUE KEY, the same one the core puts in through `add_*`.
    ///
    /// The core writes a KEY into the node rather than a word: the default name is read by a person, and
    /// the core knows no languages. The same key is needed here to compare "has the feature been
    /// renamed" — what gets compared is KEYS, so a change of language does not affect that decision.
    fn feat_default_name(kind: &qymcad_core::feature::FeatureKind) -> String {
        use qymcad_core::feature::FeatureKind as FK;
        match kind {
            // THE SURFACE FEATURES. Without these lines a node counted as RENAMED (its name did not match
            // the "default"), and the tree showed the raw catalogue key instead of a word. Caught by a scenario.
            FK::FaceCopy { .. } => "feat-name-face-copy".into(),
            FK::SurfaceReplace { .. } => "feat-name-surface-replace".into(),
            FK::Patch { .. } => "feat-name-patch".into(),
            FK::Stitch { .. } => "feat-name-stitch".into(),
            FK::Trim { .. } => "feat-name-trim".into(),
            FK::Extrude { .. } => "feat-name-extrude".into(),
            FK::Revolve { .. } => "feat-name-revolve".into(),
            FK::Sweep { .. } => "feat-name-sweep".into(),
            FK::PushFace { .. } => "feat-name-push-face".into(),
            FK::RemoveFace { .. } => "feat-name-remove-face".into(),
            FK::SplitBody { .. } => "feat-name-split-body".into(),
            FK::PartInstance { .. } => "feat-name-instance".into(),
            // A MIRRORED PART WITH NO NAME. It had a row in the tree but no default name — so the node
            // looked nameless everywhere the name is taken from the timeline (properties, search, reports).
            FK::MirrorPart { .. } => "feat-mirror-part".into(),
            FK::Thicken { .. } => "feat-name-thicken".into(),
            FK::SplitFace { .. } => "feat-name-split-face".into(),
            FK::Loft { .. } => "feat-name-loft".into(),
            FK::Draft { .. } => "feat-name-draft".into(),
            FK::Box3 { .. } => "feat-name-box".into(),
            FK::Cylinder { .. } => "feat-name-cylinder".into(),
            FK::Sphere { .. } => "feat-name-sphere".into(),
            FK::Cone { .. } => "feat-name-cone".into(),
            FK::Torus { .. } => "feat-name-torus".into(),
            FK::Prism { .. } => "feat-name-prism".into(),
            FK::Combine { op, .. } => ["feat-name-combine-cut", "feat-name-combine-boss", "feat-name-combine-intersect"][(*op as usize).min(2)].into(),
            FK::Fillet { .. } => "feat-name-fillet".into(),
            FK::Chamfer { .. } => "feat-name-chamfer".into(),
            FK::Shell { .. } => "feat-name-shell".into(),
            FK::LinearArray { .. } => "feat-name-linear-array".into(),
            FK::CircularArray { .. } => "feat-name-circular-array".into(),
            FK::Mirror { .. } => "feat-name-mirror".into(),
            FK::Hole { sketch, .. } => if *sketch != 0 { "feat-name-holes-sketch".into() } else { "feat-name-hole".into() },
            FK::Thread { .. } => "feat-name-thread".into(),
            FK::Auger { .. } => "feat-name-auger".into(),
            FK::Move { .. } => "feat-name-move".into(),
            FK::BodyBoolean { op, .. } => ["feat-name-body-cut", "feat-name-body-union", "feat-name-body-intersect"][(*op as usize).min(2)].into(),
            _ => String::new(),
        }
    }


    /// The rollback bar: a horizontal line in the feature list that is dragged up and down. Everything
    /// BELOW the line is suppressed (not built), everything above it is active. `active_k` is the current
    /// number of active features (the position of the line). Dragging changes `Project::rollback` in steps
    /// of one row. `rollback = None` means everything is active (the line sits at the bottom);
    /// `Some(feat_tis[k])` means the first k features are active.
    fn rollback_bar(&mut self, ui: &mut egui::Ui, feat_tis: &[usize], active_k: usize) {
        let n = feat_tis.len();
        let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 14.0), egui::Sense::hover());
        // A STABLE id lets the drag survive a reflow of the row (the line visually follows the cursor between frames)
        let resp = ui.interact(rect, egui::Id::new("rollback_bar_drag"), egui::Sense::drag());
        let hot = resp.hovered() || resp.dragged();
        let col = if hot { self.scheme.pal.selected() } else { self.scheme.pal.rollback() };
        let y = rect.center().y;
        {
            let painter = ui.painter();
            painter.hline((rect.left() + 16.0)..=(rect.right() - 4.0), y, egui::Stroke::new(2.0, col));
            // the "drag me vertically" grip: two triangles drawn BY THE PAINTER, without a font (phosphor or
            // raw unicode inside painter.text comes out as "tofu" boxes).
            let gx = rect.left() + 8.0;
            let tri = |a: egui::Pos2, b: egui::Pos2, c: egui::Pos2| egui::Shape::convex_polygon(vec![a, b, c], col, egui::Stroke::NONE);
            painter.add(tri(egui::pos2(gx, y - 6.0), egui::pos2(gx - 4.0, y - 1.5), egui::pos2(gx + 4.0, y - 1.5)));
            painter.add(tri(egui::pos2(gx, y + 6.0), egui::pos2(gx - 4.0, y + 1.5), egui::pos2(gx + 4.0, y + 1.5)));
        }
        let tip = if active_k >= n { crate::i18n::tr1("rollback-all", "n", &n.to_string()) } else { crate::i18n::tr2("rollback-part", "k", &active_k.to_string(), "n", &n.to_string()) };
        let resp = resp.on_hover_text(tip).on_hover_cursor(egui::CursorIcon::ResizeVertical);
        if resp.dragged() {
            self.rollback.accum += resp.drag_delta().y;
            let step = 20.0; // roughly the height of a feature row
            let mut k = active_k as i32;
            while self.rollback.accum >= step {
                k += 1;
                self.rollback.accum -= step;
            }
            while self.rollback.accum <= -step {
                k -= 1;
                self.rollback.accum += step;
            }
            let k = k.clamp(0, n as i32) as usize;
            if k != active_k {
                // WHILE DRAGGING only the line moves (the tree preview follows instantly) plus a repaint
                // request; the heavy resync (a full 3D regeneration) happens ONCE on release, not on every step.
                self.project.set_rollback(if k >= n { None } else { Some(feat_tis[k]) });
                self.rollback.pending = true;
                ui.ctx().request_repaint();
            }
        }
        if resp.drag_stopped() {
            self.rollback.accum = 0.0;
            if std::mem::take(&mut self.rollback.pending) {
                self.resync_after_topology_change(); // the final 3D regeneration at the line's resulting position
            }
        }
    }

    /// Delete feature `ti` (together with its body), clear the caches and rebuild.
    fn delete_feature(&mut self, ti: usize) {
        self.begin_edit(&crate::i18n::tr("status-delete-feature")); // THE OPERATION BOUNDARY
        if ti >= self.project.timeline.len() {
            return;
        }
        if self.project.timeline[ti].kind.body().is_some() {
            // THE WHOLE OPERATION (its span) plus the consumers ahead of it, through the tested
            // `Project::delete_feature_op`: it also removes the sibling extrusions of a collapsed operation
            // (otherwise the part fell apart into orphan bodies and "deleted" nodes came back). Here we only
            // clear the shape cache by the list of what was removed.
            let nid = self.project.timeline[ti].id;
            for db in self.project.delete_feature_op(nid) {
                self.live.shapes.remove(&db);
            }
        } else {
            self.project.timeline.remove(ti); // a node with no body (a sketch or a datum in the timeline)
        }
        self.sel = Sel::None;
        self.resync_after_topology_change();
            self.commit_edit();
    }

    /// Delete a BODY by mesh index `mi` (whichever way the body was selected — in 3D, in the tree, in a
    /// panel): take the deepest existing node of the chain and cascade forward (`delete_feature`). An
    /// imported mesh with no node is removed directly. One path for every place (the panel, Del, the
    /// context menu), so nothing drifts apart.
    fn delete_body_mesh(&mut self, mi: usize) {
        match self.project.mesh_id(mi).and_then(|b| self.lineage_delete_ti(b)) {
            Some(ti) => self.delete_feature(ti), // ask_delete-exempt: this is the executor itself, called by `execute_delete`
            None => {
                // an imported mesh with no feature in the timeline is removed directly
                self.project.remove_mesh(mi); // the faces and the visibility go with the body — one record
                self.sel = Sel::None;
                self.invalidate();
            }
        }
    }

    /// Delete a SKETCH (one path for the tree and for the panel): the core's `delete_sketch` (cascading to
    /// the bodies built on it), then clearing the editing session and the selection, then a resync
    /// (regeneration plus prune).
    fn delete_sketch_full(&mut self, sid: Id) {
        self.begin_edit(&crate::i18n::tr("status-delete-sketch")); // THE OPERATION BOUNDARY
        let removed = self.project.delete_sketch(sid);
        if self.sketch_ses.editing == Some(sid) {
            self.sketch_ses.editing = None;
        }
        self.sel = Sel::None;
        self.resync_after_topology_change();
        self.status = if removed.is_empty() { crate::i18n::tr("sketch-deleted") } else { crate::i18n::tr1("sketch-deleted-n", "n", &removed.len().to_string()) };
            self.commit_edit();
    }

    /// THE SINGLE dispatcher for deleting the selection (the Del key or the buttons): a feature or a body
    /// cascades, a sketch cascades. Returns true when something was deleted (otherwise the caller handles
    /// its own case — sketch geometry, for instance).
    /// A human-readable name of the node being deleted, for the confirmation popup.
    /// ASK BEFORE DELETING — THE SINGLE ENTRY, whichever button was pressed.
    ///
    /// Only the tree asked, on Del and on "delete the part"; the very same feature, sketch, plane, axis,
    /// point, contour and body were removed SILENTLY by the button in the properties panel. That is,
    /// whether one was asked depended on the route taken to the same action — exactly the defect the
    /// right-hand panel's editors already had.
    ///
    /// The removal itself is done by `execute_delete`, one for every kind: the dialogue must not know how
    /// things are deleted.
    pub(super) fn ask_delete(&mut self, sel: Sel) {
        self.deferred.delete = Some(sel);
    }

    /// WHAT WILL GO WITH IT — by name, for the question asked before deleting.
    ///
    /// Deletion in the timeline cascades: with a fillet goes everything built on it. This used to be
    /// reported by one general line saying "together with its dependants" — true, but no answer to the
    /// question "what am I about to lose". The list comes from the same `Project::dependents_of` as the
    /// lineage in the properties card: two places must not answer one question differently.
    fn delete_cascade_names(&self, sel: Sel) -> Vec<String> {
        let subject = match sel {
            Sel::Feature(ti) => self.project.timeline.get(ti).map(|n| n.id),
            Sel::Mesh(mi) => self.project.mesh_id(mi),
            Sel::Sketch(si) => self.project.sketches.get(si).map(|s| s.id),
            Sel::Plane(i) => self.project.planes.get(i).map(|p| p.id),
            Sel::DatumPoint(i) => self.project.datum_points.get(i).map(|d| d.id),
            Sel::DatumAxis(i) => self.project.datum_axes.get(i).map(|d| d.id),
            _ => None,
        };
        let Some(id) = subject else { return Vec::new() };
        self.project
            .dependents_of(id)
            .into_iter()
            .filter_map(|nid| self.project.timeline.iter().find(|n| n.id == nid))
            .map(|n| crate::i18n::name(&n.name))
            .collect()
    }

    fn sel_delete_label(&self, sel: Sel) -> String {
        match sel {
            Sel::Feature(_) => crate::i18n::tr("del-feature"),
            Sel::Mesh(mi) => crate::i18n::tr1("del-body-named", "name", &crate::i18n::name(&self.project.mesh_name(mi))),
            Sel::Sketch(si) => self.project.sketches.get(si).map(|s| crate::i18n::tr1("del-sketch-named", "name", &crate::i18n::name(&s.name))).unwrap_or_else(|| crate::i18n::tr("del-sketch")),
            Sel::Plane(i) => self.project.planes.get(i).map(|p| crate::i18n::tr1("del-plane-named", "name", &crate::i18n::name(&p.name))).unwrap_or_else(|| crate::i18n::tr("del-plane")),
            Sel::DatumPoint(i) => self.project.datum_points.get(i).map(|d| crate::i18n::tr1("del-point-named", "name", &crate::i18n::name(&d.name))).unwrap_or_else(|| crate::i18n::tr("del-point")),
            Sel::DatumAxis(i) => self.project.datum_axes.get(i).map(|d| crate::i18n::tr1("del-axis-named", "name", &crate::i18n::name(&d.name))).unwrap_or_else(|| crate::i18n::tr("del-axis")),
            Sel::Joint(_) => crate::i18n::tr("del-joint"),
            Sel::Component(ci) => self.project.components.get(ci).map(|c| crate::i18n::tr1("del-part-named", "name", &crate::i18n::name(&c.name))).unwrap_or_else(|| crate::i18n::tr("del-part")),
            _ => crate::i18n::tr("del-item"),
        }
    }



    /// A large icon button for the left tool bar. Returns whether it was clicked.
    /// The icon button of the workbench's left panel. A FIXED size (`add_sized`, not `min_size`): wide
    /// glyphs (the move and expand arrows) must not inflate the button and break the grid, or the row
    /// stops holding exactly two columns. The size is THE SAME as `sym_button`'s (40x34) — both grids mix
    /// inside one `horizontal_wrapped`, and a mismatch of widths (34 against 38) broke the wrap onto two
    /// columns. The usable width of the tools panel is 108 - 16 (padding) - 6 (the floating scrollbar) =
    /// 86 px: two columns (40+3+40=83) fit with room to spare and hold at exactly two.
    fn icon_tool(ui: &mut egui::Ui, icon: &str, tip: &str, active: bool) -> bool {
        let btn = egui::Button::new(egui::RichText::new(icon).size(19.0)).selected(active);
        ui.add_sized(egui::vec2(40.0, 34.0), btn).on_hover_text(tip).clicked()
    }


    /// ONE style for the active tool's top bar across ALL the workbenches (the sketcher is the reference):
    /// the same background and padding, so the Part, Assembly and Sketch bars look alike. The tool's name
    /// is `strong()` in a neutral colour, the icon tells the tools apart, and the parameters follow a
    /// `separator()`.
    /// THE ACTIVE TOOL'S BAR. This used to be a function WITHOUT `&self` — it simply could not look at the
    /// theme, even had it wanted to. Not "somebody forgot to colour it", but structurally impossible.
    fn tool_bar_frame(&self) -> egui::Frame {
        egui::Frame::none().fill(self.scheme.pal.toolbar_bg()).inner_margin(egui::Margin::symmetric(8.0, 3.0))
    }


    /// The Sketch button — ONLY in the Part workbench (in an assembly a sketch is inert, see `create_panel_common`).
    fn create_panel_sketch_button(&mut self, ui: &mut egui::Ui) {
        if Self::icon_tool(ui, ph::PENCIL_SIMPLE, &crate::i18n::tr("g-sketch-pick-hint"), self.picking.is_sketch_plane()) {
            self.toggle_sketch_pick();
        }
    }

    /// Turn the plane pick for a new sketch on or off (the Sketch button and the K shortcut). It cancels
    /// an active datum or command (`cancel_all_tools` clears the sketch-plane pick, so the target state is
    /// remembered BEFORE the reset), so that two tools are never held at once.
    fn toggle_sketch_pick(&mut self) {
        let turning_on = !self.picking.is_sketch_plane();
        self.cancel_all_tools();
        if turning_on {
            self.picking = Picking::SketchPlane(None);
        }
        if self.picking.is_sketch_plane() {
            self.mode_3d = true; // planes and faces are picked in 3D
            self.sel = Sel::None;
            self.status = crate::i18n::tr("g-sketch-pick-plane");
        }
    }

    /// The shared Create buttons: THE DATUMS — they exist both in a Part and in an Assembly. A sketch is
    /// added separately and ONLY in a Part (see `create_panel_part`).
    ///
    /// SKETCHES ARE GONE FROM ASSEMBLIES. The button stood there marked "skeleton", so the intent of
    /// top-down layout was there, but nothing could refer to that skeleton: a joint anchor understands
    /// `Origin`, `BasePlane`, `FaceCenter`, `EdgeMid` and `Vertex` — there is no reference to a sketch, and
    /// there is no extrude in the Assembly workbench either. A sketch there was inert: it could be drawn
    /// but not used. Layout belongs in the part-modelling workbench.
    ///
    /// DATUMS STAY IN AN ASSEMBLY — they are not inert: a mirrored copy of a component and the view's
    /// section both consume them.
    fn create_panel_common(&mut self, ui: &mut egui::Ui) {
        use qymcad_core::feature::{BasePlane, SketchPlane};
        let _ = (BasePlane::XY, SketchPlane::default);
        // Datums are COMMANDS: an options bar, fields at the geometry, click-picked references, a preview, Enter and Esc
        if Self::icon_tool(ui, ph::SELECTION_ALL, &crate::i18n::tr("g-datum-plane-hint"), self.cmd.kind == 20) {
            self.start_feat_cmd(20);
        }
        if Self::icon_tool(ui, ph::DOT, &crate::i18n::tr("g-datum-point-hint"), self.cmd.kind == 21) {
            self.start_feat_cmd(21);
        }
        if Self::icon_tool(ui, ph::LINE_SEGMENT, &crate::i18n::tr("g-datum-axis-hint"), self.cmd.kind == 22) {
            self.start_feat_cmd(22);
        }
    }




    /// Enter the joint EDITING mode (a double click on its glyph): the top parameter bar plus the popup of
    /// anchors A and B. The competing picking and creating modes are cleared and the joint is selected, so
    /// its degree-of-freedom gizmo is visible.
    fn enter_joint_edit(&mut self, jid: Id) {
        self.joint.edit = Some(jid);
        self.joint.edit_repick = None;
        self.joint.pick_faces = false;
        self.joint.pick_first = None;
        self.joint.ground_pick = false;
        self.sel = Sel::Joint(jid);
        self.status = crate::i18n::tr("g-joint-edit-hint");
    }

    /// Leave the joint editing mode (Esc or Done).
    fn exit_joint_edit(&mut self) {
        self.joint.edit = None;
        self.joint.edit_repick = None;
        self.status = crate::i18n::tr("g-joint-edit-done");
    }

    /// The name of the PART that owns body `body` (for the anchor's label in the editing popup).
    fn body_comp_name(&self, body: Id) -> String {
        self.project
            .body_owner(body)
            .and_then(|o| self.project.components.iter().find(|c| c.id == o))
            .map(|c| crate::i18n::name(&c.name))
            .unwrap_or_else(|| "?".into())
    }

    /// A human-readable description of a connector's anchor: the kind (face, edge, vertex and so on) and the part.
    fn anchor_desc(&self, anchor: &qymcad_core::feature::AnchorRef) -> String {
        use qymcad_core::feature::AnchorRef;
        match anchor {
            AnchorRef::Origin => crate::i18n::tr("anchor-origin"),
            AnchorRef::BasePlane(_) => crate::i18n::tr("anchor-base-plane"),
            AnchorRef::FaceCenter(body, _) => crate::i18n::tr1("anchor-face", "what", &self.body_comp_name(*body)),
            AnchorRef::EdgeMid(body, _) => crate::i18n::tr1("anchor-edge", "what", &self.body_comp_name(*body)),
            AnchorRef::Vertex(body, _, _) => crate::i18n::tr1("anchor-vertex", "what", &self.body_comp_name(*body)),
        }
    }

    /// Change the KIND of an existing joint on the fly, KEEPING its anchors (without recreating the
    /// joint). Returns true when the kind changed (the caller then regenerates).
    fn change_joint_kind(&mut self, jid: Id, newk: qymcad_core::feature::JointKind) -> bool {
        // CHANGING THE KIND IS THE CORE'S BUSINESS: the mating side follows it (an anchor's frame is built
        // FOR A KIND), and so does the declared "as it stands". The interface here only wrote the `kind`
        // field, and after a change of kind the joint kept the side chosen for the previous kind's frame.
        if !self.project.set_joint_kind(jid, newk) {
            return false;
        }
        // ANY PAIR OF ANCHORS SUITS ANY KIND OF JOINT. A "compatibility" check used to stand here, with a
        // message saying the anchor kinds did not suit, but it always answered "they do": the rule existed
        // only in the message. It is not needed either — an anchor is a full coordinate system, and the
        // kind of joint only says which degrees of freedom stay free.
        self.status = crate::i18n::tr1("g-joint-kind-set", "kind", &crate::i18n::tr(newk.label()));
        true
    }

    /// The component the placement gizmo applies to: a component selected in the tree whose parent is the
    /// current context (so its transform is the display frame), in the Assembly workbench, and not the root.
    fn gizmo_component(&self) -> Option<Id> {
        if !matches!(self.workbench, Workbench::Assembly) {
            return None;
        }
        if let Sel::Component(ci) = self.sel {
            let c = self.project.components.get(ci)?;
            if c.id != self.project.root && c.parent == Some(self.current_ctx_id()) {
                return Some(c.id);
            }
        }
        None
    }

    /// The gizmo's origin (in the display frame, that is, the component's transform) and the length of an
    /// axis (about 60 px). During a drag the origin is FIXED, as it is for a body, so the axis projection
    /// does not float along with the component.
    fn gizmo_geometry(&self, comp: Id) -> ([f64; 3], f64) {
        let o = match self.comp_giz.drag {
            Some((dc, _, origin, _)) if dc == comp => origin,
            _ => {
                let t = self.project.component_transform(comp);
                [t[3], t[7], t[11]]
            }
        };
        (o, 60.0 / self.cam.scale as f64)
    }

    /// Which gizmo axis is under the cursor (0 = X, 1 = Y, 2 = Z), if it is close enough (within 8 px).
    fn gizmo_axis_hit(&self, comp: Id, rect: Rect, basis: &([f64; 3], [f64; 3], [f64; 3]), pp: Pos2) -> Option<u8> {
        let (o, l) = self.gizmo_geometry(comp);
        self.gizmo_axis_hit_at(o, l, rect, basis, pp)
    }

    /// The axis of a gizmo with origin `o` and length `l` under the cursor — shared code for the COMPONENT
    /// gizmo (Assembly) and the BODY gizmo (Part). The axes are the world X, Y and Z from `o`.
    fn gizmo_axis_hit_at(&self, o: [f64; 3], l: f64, rect: Rect, basis: &([f64; 3], [f64; 3], [f64; 3]), pp: Pos2) -> Option<u8> {
        let s0 = self.project3(o, rect, basis).0;
        let mut best: Option<(f32, u8)> = None;
        for ax in 0..3u8 {
            let mut tip = o;
            tip[ax as usize] += l;
            let s1 = self.project3(tip, rect, basis).0;
            let d = screen_dist_seg(pp, s0, s1);
            if d <= 13.0 && best.map_or(true, |(bd, _)| d < bd) {
                best = Some((d, ax));
            }
        }
        best.map(|(_, a)| a)
    }

    /// Dragging a component gizmo's AXIS: accumulate the world shift along the axis (projected onto the
    /// FIXED screen axis) into `comp_giz_drag.amt` and apply it through `apply_comp_giz`. Unified with the
    /// body gizmo.
    fn drag_component_axis(&mut self, _comp: Id, ax: u8, d: egui::Vec2, rect: Rect, basis: &([f64; 3], [f64; 3], [f64; 3])) {
        let Some((_, _, origin, _)) = self.comp_giz.drag else { return };
        let l = 60.0 / self.cam.scale as f64;
        let mut tip = origin;
        tip[ax as usize] += l;
        let s0 = self.project3(origin, rect, basis).0;
        let s1 = self.project3(tip, rect, basis).0;
        let pd = s1 - s0;
        let denom = (pd.x * pd.x + pd.y * pd.y) as f64;
        if denom < 1e-6 {
            return;
        }
        let inc = (d.x * pd.x + d.y * pd.y) as f64 * l / denom;
        if let Some(drag) = &mut self.comp_giz.drag {
            drag.3 += inc;
        }
        self.apply_comp_giz();
    }

    /// The accumulated transform of a component's gizmo (relative to the START transform): a shift along an
    /// axis, or a rotation about a ring through the fixed origin; `snap` rounds to the step (the grid or
    /// the angle). Unified with the body gizmo.
    fn comp_giz_accum(&self, snap: bool) -> Option<[f64; 12]> {
        let (_, start, origin, amt) = self.comp_giz.drag?;
        if let Some(ax) = self.comp_giz.axis {
            let step = self.set.snap.grid.max(0.01);
            let dmm = if snap { (amt / step).round() * step } else { amt };
            let mut t = qymcad_core::feature::PLACE_IDENTITY;
            t[[3, 7, 11][ax as usize]] = dmm;
            Some(compose12(&t, &start))
        } else if let Some(ax) = self.comp_giz.ring {
            let step = self.set.snap.rot_deg.max(0.1);
            let deg = if snap { (amt / step).round() * step } else { amt };
            Some(compose12(&rot_about_point(ax, deg, origin), &start))
        } else {
            None
        }
    }

    /// The readout of a component's gizmo (mm or degrees), taking snapping into account.
    fn comp_giz_readout(&self, snap: bool) -> Option<String> {
        let (_, _, _, amt) = self.comp_giz.drag?;
        if self.comp_giz.axis.is_some() {
            let step = self.set.snap.grid.max(0.01);
            let d = if snap { (amt / step).round() * step } else { amt };
            Some(crate::i18n::tr1("giz-shift-mm", "v", &format!("{d:+.2}")))
        } else if self.comp_giz.ring.is_some() {
            let step = self.set.snap.rot_deg.max(0.1);
            let g = if snap { (amt / step).round() * step } else { amt };
            Some(format!("{g:+.1}{}", crate::i18n::tr("unit-deg-suffix")))
        } else {
            None
        }
    }

    /// Which rotation ring of the gizmo is under the cursor (0 = X, 1 = Y, 2 = Z), if it is close enough
    /// (within 6 px). The ring of axis `ax` is a circle of radius L in the plane perpendicular to that
    /// axis, around the component's origin.
    fn gizmo_ring_hit(&self, comp: Id, rect: Rect, basis: &([f64; 3], [f64; 3], [f64; 3]), pp: Pos2) -> Option<u8> {
        let (o, l) = self.gizmo_geometry(comp);
        self.gizmo_ring_hit_at(o, l, rect, basis, pp)
    }

    /// The rotation ring of a gizmo with origin `o` and radius `l` under the cursor — shared code for the
    /// Assembly and the Part.
    fn gizmo_ring_hit_at(&self, o: [f64; 3], l: f64, rect: Rect, basis: &([f64; 3], [f64; 3], [f64; 3]), pp: Pos2) -> Option<u8> {
        let mut best: Option<(f32, u8)> = None;
        for ax in 0..3u8 {
            let (u, v) = ring_axes(ax);
            let mut prev: Option<Pos2> = None;
            let mut dmin = f32::MAX;
            for k in 0..=48 {
                let a = k as f64 / 48.0 * std::f64::consts::TAU;
                let p = [o[0] + l * (u[0] * a.cos() + v[0] * a.sin()), o[1] + l * (u[1] * a.cos() + v[1] * a.sin()), o[2] + l * (u[2] * a.cos() + v[2] * a.sin())];
                let s = self.project3(p, rect, basis).0;
                if let Some(pr) = prev {
                    dmin = dmin.min(screen_dist_seg(pp, pr, s));
                }
                prev = Some(s);
            }
            if dmin <= 10.0 && best.map_or(true, |(bd, _)| dmin < bd) {
                best = Some((dmin, ax));
            }
        }
        best.map(|(_, a)| a)
    }

    /// Dragging a component gizmo's RING: accumulate the angle (about the FIXED origin) into
    /// `comp_giz_drag.amt` and apply it through `apply_comp_giz` (the rotation is the accumulated one
    /// composed with the start, with snapping). Unified with the body gizmo.
    fn drag_component_ring(&mut self, _comp: Id, ax: u8, cursor: Pos2, d: egui::Vec2, rect: Rect, basis: &([f64; 3], [f64; 3], [f64; 3])) {
        let Some((_, _, origin, _)) = self.comp_giz.drag else { return };
        let center = self.project3(origin, rect, basis).0;
        let radial = cursor - center;
        let r2 = (radial.x * radial.x + radial.y * radial.y) as f64;
        if r2 < 4.0 {
            return;
        }
        let ccw = -(radial.x * d.y - radial.y * d.x) as f64 / r2;
        let sign = ring_drag_sign(basis.2[ax as usize]);
        if let Some(drag) = &mut self.comp_giz.drag {
            drag.3 += ccw.to_degrees() * sign;
        }
        self.apply_comp_giz();
    }

    // ===== The degree-of-freedom aware gizmo of a driven component. What gets dragged is the JOINT'S
    // FREEDOM (angle, offset, offset2) rather than a free 6-DOF transform, so the drag stays WITHIN the
    // joint and `solve_joints` works the rest out. =====

    /// The gizmo mode of the selected component: grounded gives None; driven by a joint that has a freedom
    /// gives Joint(jid); otherwise (free, or a seed with no freedoms of its own) gives Free, the plain 6-DOF one.
    fn comp_gizmo_mode(&self, comp: Id) -> CompGizmoMode {
        if self.project.is_grounded(comp) {
            return CompGizmoMode::None;
        }
        if let Some(jid) = self.project.drive_joint_for(comp) {
            // at least one handle is free and not driven by an expression, so the degree-of-freedom gizmo applies
            if self.joint_giz_handles(jid).map_or(false, |(_, hs)| !hs.is_empty()) {
                return CompGizmoMode::Joint(jid);
            }
            // there is a joint, but it has no freedoms (a rigid one) or every parameter is driven by an
            // expression, so the component is pinned
            return CompGizmoMode::None;
        }
        CompGizmoMode::Free
    }

    /// The joint whose degree-of-freedom gizmo is active in the viewport right now. Two ways in:
    /// 1) a COMPONENT is selected — a direct child of the context, driven by a joint that has freedoms;
    /// 2) the joint's glyph is selected DIRECTLY (`Sel::Joint`) — needed for a joint held at the ROOT whose
    ///    driven part lies inside a subassembly and is not a direct child (`gizmo_component()` gives None),
    ///    while the subassembly's slider still wants to be draggable by its handle straight from the root.
    fn active_dof_joint(&self) -> Option<Id> {
        if let Some(comp) = self.gizmo_component() {
            if let CompGizmoMode::Joint(jid) = self.comp_gizmo_mode(comp) {
                return Some(jid);
            }
        }
        if let Sel::Joint(jid) = self.sel {
            let ctx = self.current_ctx_id();
            if let Some(j) = self.project.joints.iter().find(|j| j.id == jid) {
                if self.project.joint_in_context(j, ctx)
                    && self.joint_giz_handles(jid).map_or(false, |(_, hs)| !hs.is_empty())
                {
                    return Some(jid);
                }
            }
        }
        None
    }

    // ===== The BODY gizmo in a Part: it reuses `gizmo_*_hit_at` and `draw_gizmo_at`; the movement itself
    // is a parametric Move feature. The drag state is `body_giz_axis`/`ring` plus the accumulated
    // `body_giz_drag`. =====

    /// The body under the gizmo in a Part: the selected body of the active part (`Sel::Mesh`, or
    /// `Sel::Feature` carrying a body). Returns (the body's Id, the mesh index). Only in the Part workbench.
    fn body_gizmo_target(&self) -> Option<(Id, usize)> {
        // A PART IS ONE BODY, so the Part workbench does NOT offer a gizmo for an INDIVIDUAL body. Dragging
        // a body would bake in a Move feature and tear the body away from its sketch. A part's body is moved
        // in an assembly by the COMPONENT gizmo (its placement). The body gizmo is kept ONLY for a "free"
        // imported body (an STL with no owning component) — that one cannot be moved by a component gizmo.
        if self.cmd.active() {
            return None; // during a command (extrude, cut and so on) the body gizmo stays out of the way
        }
        let body = self.visible_lineage_body(self.selected_body()?);
        // a body that BELONGS to a part (it has an owner) gets NO body gizmo, neither in the part nor in the
        // assembly: a part is a body and is moved by its component. The body gizmo is only for an imported
        // mesh with no owner.
        if self.project.body_owner(body).is_some() {
            return None;
        }
        let mi = self.project.mesh_index(body)?;
        self.body_shown(mi).then_some((body, mi))
    }

    /// The node index for DELETING the whole chain of body `body`: walk the lineage BACK towards the root,
    /// stopping at the DEEPEST body that still HAS a node (for a torus plus a Move that is the torus's
    /// node; for a dangling Move whose source is gone, the Move's own node). `delete_feature` then cascades
    /// forward and prune finishes the job.
    fn lineage_delete_ti(&self, body: Id) -> Option<usize> {
        let mut b = body;
        let mut best = self.project.timeline.iter().position(|n| n.kind.body() == Some(b));
        for _ in 0..256 {
            let Some(src) = self.project.timeline.iter().find(|n| n.kind.body() == Some(b)).and_then(|n| n.kind.consumed_body()) else { break };
            match self.project.timeline.iter().position(|n| n.kind.body() == Some(src)) {
                Some(ti) => {
                    best = Some(ti);
                    b = src;
                }
                None => break, // a source with no node (a dangling one): go no deeper
            }
        }
        best
    }

    /// The live body of a lineage — a delegate to the core (`Project::live_body`). The FORWARD walk lives
    /// there in a single definition: a copy in the interface layer would drift from the core silently.
    fn visible_lineage_body(&self, body: Id) -> Id {
        self.project.live_body(body)
    }

    /// The origin of a body's gizmo (the centre of the mesh's bounding box, in the Part's display frame,
    /// which is the local one) and the length of an axis (about 60 px). During a drag the origin is fixed
    /// (`body_giz_drag.1`); otherwise it is the current centre of the box.
    fn body_gizmo_geometry(&self, mi: usize) -> ([f64; 3], f64) {
        let o = if let Some((dmi, org, _)) = self.body_giz.drag {
            if dmi == mi {
                // THE GIZMO FOLLOWS the body during a drag (the preview moves the body by the same
                // accumulator). For a move the origin travels along; for a rotation
                // apply12(rot_about_org, org) = org, so the centre stays put. Otherwise the gizmo would
                // stand still while the body moved, and would jump to the body on release.
                match self.body_giz_accum(self.body_giz.snap) {
                    Some(acc) => qymcad_core::feature::apply12(&acc, org),
                    None => org,
                }
            } else {
                self.mesh_center(mi)
            }
        } else {
            self.mesh_center(mi)
        };
        (o, 60.0 / self.cam.scale as f64)
    }

    /// The centre of mesh `mi`'s bounding box (world, or the Part's local frame). [0, 0, 0] when the mesh is empty.
    fn mesh_center(&self, mi: usize) -> [f64; 3] {
        self.project
            .bodies
            .get(mi)
            .and_then(|b| b.mesh.bounds())
            .map(|b| [(b.min.x + b.max.x) / 2.0, (b.min.y + b.max.y) / 2.0, (b.min.z + b.max.z) / 2.0])
            .unwrap_or([0.0, 0.0, 0.0])
    }

    /// The snapping step for moving a body (mm): the grid step from the snapping panel.
    fn body_snap_mm(&self) -> f64 {
        self.set.snap.grid.max(0.01)
    }
    /// The snapping step for rotating a body (deg): the rotation field from the snapping panel.
    fn body_snap_deg(&self) -> f64 {
        self.set.snap.rot_deg.max(0.1)
    }

    /// The transform accumulated by a body gizmo over the current drag (in the Part's world frame): a shift
    /// along an axis OR a rotation about a ring through the fixed origin. `snap` rounds to the step (the
    /// grid, or the angle). None means there is no drag.
    fn body_giz_accum(&self, snap: bool) -> Option<[f64; 12]> {
        let (_, o, amt) = self.body_giz.drag?;
        if let Some(ax) = self.body_giz.axis {
            let step = self.body_snap_mm();
            let d = if snap { (amt / step).round() * step } else { amt };
            let mut t = qymcad_core::feature::PLACE_IDENTITY;
            t[[3, 7, 11][ax as usize]] = d; // a shift along the world axis `ax`
            Some(t)
        } else if let Some(ax) = self.body_giz.ring {
            let deg = if snap { (amt / self.body_snap_deg()).round() * self.body_snap_deg() } else { amt };
            Some(rot_about_point(ax, deg, o))
        } else {
            None
        }
    }

    /// The current number for a body gizmo's readout (mm for an axis, degrees for a ring), with snapping applied.
    fn body_giz_readout(&self, snap: bool) -> Option<String> {
        let (_, _, amt) = self.body_giz.drag?;
        if self.body_giz.axis.is_some() {
            let step = self.body_snap_mm();
            let d = if snap { (amt / step).round() * step } else { amt };
            Some(crate::i18n::tr1("giz-shift-mm", "v", &format!("{d:+.2}")))
        } else if self.body_giz.ring.is_some() {
            let g = if snap { (amt / self.body_snap_deg()).round() * self.body_snap_deg() } else { amt };
            Some(format!("{g:+.1}{}", crate::i18n::tr("unit-deg-suffix")))
        } else {
            None
        }
    }

    /// The world (origin, normal) of the plane the command has picked — for the PREVIEW, without creating a
    /// datum (unlike `resolve_mirror_plane`). For a face, the local centroid and normal are taken into
    /// world space through the body's display transform.
    fn mirror_plane_world(&self, sp: &qymcad_core::feature::SketchPlane) -> Option<([f64; 3], [f64; 3])> {
        use qymcad_core::feature::{apply12, BasePlane, SketchPlane};
        match sp {
            SketchPlane::World(BasePlane::XY) => Some(([0.0, 0.0, 0.0], [0.0, 0.0, 1.0])),
            SketchPlane::World(BasePlane::XZ) => Some(([0.0, 0.0, 0.0], [0.0, 1.0, 0.0])),
            SketchPlane::World(BasePlane::YZ) => Some(([0.0, 0.0, 0.0], [1.0, 0.0, 0.0])),
            SketchPlane::Datum(id) => self.project.planes.iter().find(|p| p.id == *id).map(|p| (p.origin, p.normal)),
            SketchPlane::Face(body, key) => {
                let ctx = self.current_ctx_id();
                let wt = self.project.body_display_transform(*body, ctx);
                let o = apply12(&wt, key.centroid);
                let z = apply12(&wt, [0.0, 0.0, 0.0]);
                let n = [apply12(&wt, key.normal)[0] - z[0], apply12(&wt, key.normal)[1] - z[1], apply12(&wt, key.normal)[2] - z[2]];
                Some((o, n))
            }
        }
    }

    /// The straight edges of ALL visible (unconsumed) bodies go into `axis_edges` (the body plus a local
    /// polyline), so that the axis of a pattern or a datum can be picked on an edge of ANY body rather than
    /// only the selected one. Called when the pick begins.
    fn refresh_axis_edges(&mut self) {
        self.ensure_brep(); // the candidates are edges of the LIVE B-rep; without it the list is empty and there is nothing to click
        self.edges.axes.clear();
        let consumed = self.consumed_bodies();
        for mi in 0..self.project.bodies.len() {
            if !self.body_shown(mi) {
                continue;
            }
            let Some(b) = self.project.mesh_id(mi) else { continue };
            if consumed.contains(&b) {
                continue;
            }
            let Some(shape) = self.live.shapes.get(&b) else { continue };
            let (polys, ids) = shape.edges_with_ids();
            for (poly, id) in polys.into_iter().zip(ids) {
                if id != 0 && is_straight_poly(&poly) {
                    self.edges.axes.push((b, id, poly));
                }
            }
        }
    }

    /// A click on an axis candidate for a REVOLVE: a STRAIGHT edge of a body, a CYLINDRICAL face, or a
    /// datum axis, each turned into an associative datum. Reported behaviour: the "pick an axis (3D)"
    /// button would not take a datum axis and simply never worked.
    ///
    /// A method of its own, because this branch used to live in the 2D half of the viewport while the
    /// candidates are drawn and hit-tested ONLY in 3D. The button did switch the view to 3D and the axes
    /// did light up, but there was NOBODY there to catch the click: the handler stayed in the flat branch
    /// and was never called. Now the branch is in 3D and the logic sits here, checked by a test without egui.
    fn rev_axis_pick_click(&mut self, rect: Rect, pos: Pos2) -> bool {
        let picked = match self.pick_axis_at(rect, pos) {
            Some(AxisHit::Datum(id)) => Some((id, "g-axis-datum")),
            Some(AxisHit::Edge(i)) => self.axis_from_edge(i).map(|id| (id, "g-axis-body-edge")),
            Some(AxisHit::Face(b, f)) => self.axis_from_face(b, f).map(|id| (id, "g-axis-cyl-face")),
            None => None,
        };
        match picked {
            Some((id, what)) => {
                self.rev.axis_datum = id;
                self.rev.axis_line = 0; // the axis was set in 3D, so the sketch's axis line no longer applies
                self.rev.pick_axis = false;
                self.status = format!("{} {}", ph::CHECK, crate::i18n::tr1("g-rev-axis", "what", &crate::i18n::tr(what)));
                true
            }
            None => {
                self.status = crate::i18n::tr("g-rev-axis-miss");
                false
            }
        }
    }

    /// Create a pattern's datum AXIS from the axis of a cylindrical or conical face — ASSOCIATIVELY, so it travels with the face.
    fn axis_from_face(&mut self, body: Id, face_id: u32) -> Option<Id> {
        self.axis_ref_world(AxisHit::Face(body, face_id))?; // check that there is an axis at all
        Some(self.project.add_axis_from_face(body, face_id))
    }

    /// Create a pattern's datum AXIS from the straight edge `axis_edges[i]` — ASSOCIATIVELY, so it travels with the edge.
    fn axis_from_edge(&mut self, i: usize) -> Option<Id> {
        let &(body, edge, _) = self.edges.axes.get(i)?;
        self.axis_ref_world(AxisHit::Edge(i))?; // check the geometry
        Some(self.project.add_axis_from_edge(body, edge))
    }

    /// A CLICK hit (not a drag) on a body gizmo's arrow or ring, giving (mi, axis 0/1/2, is it a rotation?).
    /// None means the click missed the gizmo.
    fn body_gizmo_click_hit(&self, rect: Rect, pos: Pos2, basis: &([f64; 3], [f64; 3], [f64; 3])) -> Option<(usize, u8, bool)> {
        let (_, mi) = self.body_gizmo_target()?;
        let (o, l) = self.body_gizmo_geometry(mi);
        if let Some(ax) = self.gizmo_axis_hit_at(o, l, rect, basis, pos) {
            return Some((mi, ax, false));
        }
        if let Some(ax) = self.gizmo_ring_hit_at(o, l, rect, basis, pos) {
            return Some((mi, ax, true));
        }
        None
    }


    /// THE ARROW ON THE SELECTED FACE (the "push the face" handle): its origin is the centre of the face,
    /// its direction is the face's normal, and its length is the current offset (at least visible enough).
    /// Returns the origin, the tip and the axis length in world space.
    /// WHAT THE HANDLE DRAGS: the name of a field of the active command. One handle serves every command
    /// whose number has ONE direction — pushing a face, thickening, shelling, splitting a body. Giving each
    /// tool its own handle would spread one and the same behaviour across copies, as the popups once were.
    ///
    /// FILLETS, CHAMFERS AND FACE SPLITS GET NO HANDLE, and that is not an unfinished corner. There one
    /// selects a batch of elements pointing in different directions: an arrow on one of them would show a
    /// direction the operation does not have — that is, it would lie.
    pub(super) fn face_arrow_key(&self) -> Option<&'static str> {
        match self.cmd.kind {
            25 => Some("dist"),
            6 | 28 => Some("thickness"),
            27 => Some("offset"),
            _ => None,
        }
    }

    /// WHERE THE HANDLE STARTS AND WHERE IT POINTS: the point it acts on and a unit direction.
    ///
    /// For face commands that is the centroid of the selected face and its normal; for splits it is the
    /// origin and normal of the chosen plane. Everything after that is shared: the drawing, the grabbing
    /// and the dragging.
    fn cmd_arrow_ref(&self) -> Option<([f64; 3], [f64; 3])> {
        match self.cmd.kind {
            6 | 25 | 28 => {
                let mi = self.project.mesh_index(self.gsel.faces_body?)?;
                let f = self.project.bodies.get(mi)?.faces.iter().find(|f| self.gsel.faces.contains(&f.id))?;
                Some(([f.centroid.x, f.centroid.y, f.centroid.z], f.normal))
            }
            27 => self.split.plane.as_ref().and_then(|sp| self.mirror_plane_world(sp)),
            _ => None,
        }
    }

    pub(super) fn face_arrow_geometry(&self) -> Option<([f64; 3], [f64; 3], [f64; 3])> {
        let key = self.face_arrow_key()?;
        let (o, n) = self.cmd_arrow_ref()?;
        let d = self.cmd_val(key);
        // AT ZERO THE LENGTH COMES FROM THE SIZE OF THE PART. Otherwise a zero value leaves the handle
        // invisible, and there is nothing to drag at exactly the moment one needs to.
        let span = self
            .op_target_body()
            .or(self.gsel.faces_body)
            .and_then(|b| self.project.mesh_index(b))
            .and_then(|mi| self.project.bodies[mi].mesh.bounds())
            .map(|b| (b.max.x - b.min.x).abs().max(1.0) * 0.25)
            .unwrap_or(5.0);
        let l = if d.abs() > 1e-6 { d } else { span };
        Some((o, [o[0] + n[0] * l, o[1] + n[1] * l, o[2] + n[2] * l], n))
    }

    /// Is the cursor on the face arrow? The threshold matches the body gizmo's, in pixels along the segment.
    pub(super) fn face_arrow_hit(&self, rect: Rect, pos: Pos2, basis: &([f64; 3], [f64; 3], [f64; 3])) -> bool {
        let Some((o, tip, _)) = self.face_arrow_geometry() else { return false };
        let (a, b) = (self.project3(o, rect, basis).0, self.project3(tip, rect, basis).0);
        screen_dist_seg(pos, a, b) <= 9.0
    }

    /// Dragging the face arrow: the offset grows along the face's NORMAL, just as it does for a body gizmo's axis.
    pub(super) fn face_arrow_drag_to(&mut self, d: egui::Vec2, rect: Rect, basis: &([f64; 3], [f64; 3], [f64; 3])) {
        let Some((o, _, n)) = self.face_arrow_geometry() else { return };
        let l = 60.0 / self.cam.scale as f64;
        let s0 = self.project3(o, rect, basis).0;
        let s1 = self.project3([o[0] + n[0] * l, o[1] + n[1] * l, o[2] + n[2] * l], rect, basis).0;
        let pd = s1 - s0;
        let denom = (pd.x * pd.x + pd.y * pd.y) as f64;
        if denom < 1e-6 {
            return;
        }
        let Some(key) = self.face_arrow_key() else { return };
        let inc = (d.x * pd.x + d.y * pd.y) as f64 * l / denom;
        let cur = self.cmd_val(key) + inc;
        if let Some(p) = self.cmd.params.iter_mut().find(|p| p.key == key) {
            p.val = cur;
            p.txt = format!("{:.2}", cur); // the field and the arrow are one value, not two independent ones
        }
        self.invalidate();
    }

    /// Dragging a body gizmo's axis: accumulate the world shift along axis `ax` into `body_giz_drag.amount`.
    fn body_gizmo_axis_drag(&mut self, mi: usize, ax: u8, d: egui::Vec2, rect: Rect, basis: &([f64; 3], [f64; 3], [f64; 3])) {
        let (o, l) = self.body_gizmo_geometry(mi);
        let mut tip = o;
        tip[ax as usize] += l;
        let s0 = self.project3(o, rect, basis).0;
        let s1 = self.project3(tip, rect, basis).0;
        let pd = s1 - s0;
        let denom = (pd.x * pd.x + pd.y * pd.y) as f64;
        if denom < 1e-6 {
            return;
        }
        let inc = (d.x * pd.x + d.y * pd.y) as f64 * l / denom;
        if let Some(drag) = &mut self.body_giz.drag {
            drag.2 += inc;
        }
        self.invalidate();
    }

    /// Dragging a body gizmo's ring: accumulate the angle (deg) about axis `ax` into `body_giz_drag.amount`.
    fn body_gizmo_ring_drag(&mut self, mi: usize, ax: u8, cursor: Pos2, d: egui::Vec2, rect: Rect, basis: &([f64; 3], [f64; 3], [f64; 3])) {
        let (o, _) = self.body_gizmo_geometry(mi);
        let center = self.project3(o, rect, basis).0;
        let radial = cursor - center;
        let r2 = (radial.x * radial.x + radial.y * radial.y) as f64;
        if r2 < 4.0 {
            return;
        }
        let ccw = -(radial.x * d.y - radial.y * d.x) as f64 / r2;
        let sign = ring_drag_sign(basis.2[ax as usize]);
        if let Some(drag) = &mut self.body_giz.drag {
            drag.2 += ccw.to_degrees() * sign;
        }
        self.invalidate();
    }

    /// Finish a body gizmo drag: apply the accumulated transform as a PARAMETRIC Move feature.
    fn commit_body_gizmo(&mut self, snap: bool) {
        let accum = self.body_giz_accum(snap);
        let drag = self.body_giz.drag;
        self.body_giz.drag = None;
        self.body_giz.axis = None;
        self.body_giz.ring = None;
        let (Some(accum), Some((mi, _, amt))) = (accum, drag) else { return };
        if amt.abs() < 1e-6 || qymcad_core::feature::is_identity12(&accum) {
            return; // a zero drag: nothing is committed
        }
        self.apply_body_move(mi, accum);
    }

    /// The bounding box of all the geometry in 3D (the contours at Z=0 plus the bodies).
    fn geometry_bbox3(&self) -> Option<([f64; 3], [f64; 3])> {
        let (mut mn, mut mx) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
        let mut any = false;
        for c in self.project.contours.iter() {
            if let Some(b) = c.bbox() {
                any = true;
                mn[0] = mn[0].min(b.min.x);
                mn[1] = mn[1].min(b.min.y);
                mn[2] = mn[2].min(0.0);
                mx[0] = mx[0].max(b.max.x);
                mx[1] = mx[1].max(b.max.y);
                mx[2] = mx[2].max(0.0);
            }
        }
        for m in self.project.bodies.iter().map(|b| &b.mesh) {
            if let Some(b) = m.bounds() {
                any = true;
                mn[0] = mn[0].min(b.min.x);
                mn[1] = mn[1].min(b.min.y);
                mn[2] = mn[2].min(b.min.z);
                mx[0] = mx[0].max(b.max.x);
                mx[1] = mx[1].max(b.max.y);
                mx[2] = mx[2].max(b.max.z);
            }
        }
        any.then_some((mn, mx))
    }

    /// The effective stock box (automatic means from the geometry, otherwise origin..origin+size).
    fn effective_stock(&self) -> Option<([f64; 3], [f64; 3])> {
        let st = self.project.stock;
        if st.auto {
            self.geometry_bbox3()
        } else {
            Some((st.origin, [st.origin[0] + st.size[0], st.origin[1] + st.size[1], st.origin[2] + st.size[2]]))
        }
    }

    fn stock_props(&mut self, ui: &mut egui::Ui) {
        ui.heading(format!("{} {}", ph::BOUNDING_BOX, crate::i18n::tr("cam-stock-title")));
        let mut st = self.project.stock;
        let mut changed = ui.checkbox(&mut st.auto, &crate::i18n::tr("cam-stock-auto")).changed();
        if !st.auto {
            ui.separator();
            ui.label(&crate::i18n::tr("cam-stock-size"));
            egui::Grid::new("stsize").num_columns(2).spacing([6.0, 4.0]).show(ui, |ui| {
                for (k, ax) in ["X", "Y", "Z"].iter().enumerate() {
                    ui.label(*ax);
                    changed |= ui.add(egui::DragValue::new(&mut st.size[k]).speed(1.0).range(0.1..=5000.0)).changed();
                    ui.end_row();
                }
            });
            ui.label(&crate::i18n::tr("cam-stock-zero"));
            egui::Grid::new("storig").num_columns(2).spacing([6.0, 4.0]).show(ui, |ui| {
                for (k, ax) in ["X", "Y", "Z"].iter().enumerate() {
                    ui.label(*ax);
                    changed |= ui.add(egui::DragValue::new(&mut st.origin[k]).speed(1.0)).changed();
                    ui.end_row();
                }
            });
        }
        ui.separator();
        if ui.button(format!("{} {}", ph::CORNERS_OUT, crate::i18n::tr("cam-stock-from-geometry"))).on_hover_text(&crate::i18n::tr("cam-stock-from-geometry-hint")).clicked() {
            if let Some((mn, mx)) = self.geometry_bbox3() {
                st.origin = mn;
                st.size = [mx[0] - mn[0], mx[1] - mn[1], (mx[2] - mn[2]).max(1.0)];
                st.auto = false;
                changed = true;
            }
        }
        if changed {
            self.project.stock = st;
            self.invalidate();
        }
        if let Some((mn, mx)) = self.effective_stock() {
            ui.label(egui::RichText::new(crate::i18n::tr2("cam-stock-box", "size", &format!("{:.0}×{:.0}×{:.0}", mx[0] - mn[0], mx[1] - mn[1], mx[2] - mn[2]), "at", &format!("{:.0}, {:.0}, {:.0}", mn[0], mn[1], mn[2]))).weak().small());
        } else {
            ui.label(egui::RichText::new(crate::i18n::tr("cam-stock-none")).weak().small());
        }
    }

    fn mesh_props(&mut self, ui: &mut egui::Ui, i: usize) {
        // The name in the header is READABLE rather than a catalogue key, and only a name that was touched
        // gets written back; otherwise merely opening the properties would freeze the automatic name in the
        // current language (that is what `NameSlot::Editable` takes care of).
        let lin = self.lineage_of(self.project.mesh_id(i));
        if let Some(n) = props_header(ui, ph::CUBE, "mesh-props-title", NameSlot::Editable(self.project.mesh_name(i)), &lin) {
            self.project.set_mesh_name(i, n);
        }
        // the part's colour (so an assembly reads clearly)
        ui.horizontal(|ui| {
            ui.label(&crate::i18n::tr("mesh-colour"));
            let mut rgb = self.project.mesh_color(i);
            if ui.color_edit_button_srgb(&mut rgb).changed() {
                self.project.set_mesh_color(i, rgb);
                self.regen.geom_rev = self.regen.geom_rev.wrapping_add(1); // redraw the 3D view
            }
            if ui.small_button(&crate::i18n::tr("mesh-colour-reset")).on_hover_text(&crate::i18n::tr("mesh-colour-reset-hint")).clicked() {
                self.project.reset_mesh_color(i); // drop the manual colour, back to the palette keyed by the lineage root
                self.regen.geom_rev = self.regen.geom_rev.wrapping_add(1);
            }
        });
        // INFORMATION, and only what is relevant. A body's size and position change PARAMETRICALLY, through
        // features in the timeline; moving and rotating go through the three-axis gizmo in 3D; booleans and
        // patterns are commands on the toolbar, and they are features too.
        ui.separator();
        if let Some(b) = self.project.bodies[i].mesh.bounds() {
            ui.label(crate::i18n::tr1("mesh-size", "v", &format!("{:.1} × {:.1} × {:.1}", b.max.x - b.min.x, b.max.y - b.min.y, b.max.z - b.min.z)));
            ui.label(egui::RichText::new(crate::i18n::tr1("mesh-tris-n", "n", &self.project.bodies[i].mesh.tris.len().to_string())).weak().small());
        } else {
            ui.label(egui::RichText::new(crate::i18n::tr("mesh-empty")).weak());
        }
        ui.separator();
        if ui.button(format!("{} {}", ph::TRASH, crate::i18n::tr("mesh-delete"))).clicked() {
            self.ask_delete(Sel::Mesh(i));
        }
    }


    /// The closed contour of sketch `si` under a screen point (point-in-polygon; the smaller area wins).
    fn contour_under_2d(&self, rect: Rect, screen: Pos2, si: usize) -> Option<Id> {
        let mut best: Option<(f64, Id)> = None;
        for cid in self.sketch_closed_contours(si) {
            let ci = self.project.contour_index(cid)?;
            let pts: Vec<Pos2> = self.project.contours[ci].points.iter().map(|p| self.to_screen(rect, *p)).collect();
            if pts.len() >= 3 && point_in_poly(screen, &pts) {
                let area = poly_area(&pts);
                if best.map_or(true, |(ba, _)| area < ba) {
                    best = Some((area, cid));
                }
            }
        }
        best.map(|(_, id)| id)
    }

    /// The contour under the cursor among the slot's CANDIDATES (for the half-sketcher of a sweep or a
    /// loft). A closed one with the cursor inside wins (the smaller area is the nearer); otherwise the
    /// NEAREST polyline within the threshold, which is how an OPEN path gets caught, since point-in-polygon
    /// will not take it. `cands` holds the slot's candidate contours.
    fn slot_contour_under_2d(&self, rect: Rect, screen: Pos2, cands: &[Id]) -> Option<Id> {
        let seg_d = |p: Pos2, a: Pos2, b: Pos2| -> f32 {
            let ab = b - a;
            let l2 = ab.length_sq();
            let t = if l2 <= 1e-6 { 0.0 } else { ((p - a).dot(ab) / l2).clamp(0.0, 1.0) };
            (p - (a + ab * t)).length()
        };
        let mut inside: Option<(f64, Id)> = None; // closed, cursor inside: ranked by area
        let mut near: Option<(f32, Id)> = None; // ranked by distance to the polyline
        for &cid in cands {
            let Some(ci) = self.project.contour_index(cid) else { continue };
            let c = &self.project.contours[ci];
            if c.points.len() < 2 {
                continue;
            }
            let pts: Vec<Pos2> = c.points.iter().map(|p| self.to_screen(rect, *p)).collect();
            if c.closed && pts.len() >= 3 && point_in_poly(screen, &pts) {
                let area = poly_area(&pts);
                if inside.map_or(true, |(ba, _)| area < ba) {
                    inside = Some((area, cid));
                }
            }
            let n = pts.len();
            let segs = if c.closed { n } else { n - 1 };
            let mut d = f32::INFINITY;
            for k in 0..segs {
                d = d.min(seg_d(screen, pts[k], pts[(k + 1) % n]));
            }
            if d < 8.0 && near.map_or(true, |(bd, _)| d < bd) {
                near = Some((d, cid));
            }
        }
        inside.map(|(_, id)| id).or(near.map(|(_, id)| id))
    }

    /// The world snapping point under the cursor for PLACING a primitive: a vertex > a datum point > the
    /// centre of a face.
    /// The cursor's ray in WORLD space. The camera is orthographic (`project3` gives sx = rel . right,
    /// sy = rel . up, depth = rel . fwd), so every ray is parallel to `fwd`; the ray's origin is taken in
    /// the plane of `target`, leaving the depth free.
    fn screen_ray(&self, rect: Rect, screen: Pos2) -> ([f64; 3], [f64; 3]) {
        let (right, up, fwd) = self.cam.basis();
        let c = rect.center();
        let sx = ((screen.x - c.x) / self.cam.scale) as f64;
        let sy = (-(screen.y - c.y) / self.cam.scale) as f64; // the screen Y is inverted (see `project3`)
        let t = self.cam.target;
        let o = [t[0] + right[0] * sx + up[0] * sy, t[1] + right[1] * sx + up[1] * sy, t[2] + right[2] * sx + up[2] * sy];
        (o, fwd)
    }

    /// Resolve the mirror plane the command has picked into (`plane` as a u8 world plane, `datum` Id): a
    /// world plane gives (0/1/2, 0); a datum gives (0, id); a face creates a datum plane from that face
    /// (offset 0) and gives (0, id). The same one is used both when creating and when editing.
    fn resolve_mirror_plane(&mut self, sp: qymcad_core::feature::SketchPlane) -> (u8, Id) {
        use qymcad_core::feature::{BasePlane, SketchPlane};
        match sp {
            SketchPlane::World(BasePlane::XY) => (0, 0),
            SketchPlane::World(BasePlane::XZ) => (1, 0),
            SketchPlane::World(BasePlane::YZ) => (2, 0),
            SketchPlane::Datum(id) => (0, id),
            SketchPlane::Face(body, key) => (0, self.project.add_plane_from_face(body, key, 0.0)),
        }
    }

    /// The step vector for direction `dir` (0 = X, 1 = Y, 2 = Z) of magnitude `step`.
    fn arr_vec(dir: u8, step: f64) -> (f64, f64, f64) {
        match dir {
            1 => (0.0, step, 0.0),
            2 => (0.0, 0.0, step),
            _ => (step, 0.0, 0.0),
        }
    }

    /// The direction of an offset vector, as (dir 0/1/2, a signed step) — for reopening a linear pattern.
    fn arr_dir_of(dx: f64, dy: f64, dz: f64) -> (u8, f64) {
        let (ax, ay, az) = (dx.abs(), dy.abs(), dz.abs());
        if az >= ax && az >= ay {
            (2, dz)
        } else if ay >= ax {
            (1, dy)
        } else {
            (0, dx)
        }
    }

    /// Store the pattern step's expression on the ACTIVE component of the vector (regeneration follows
    /// that one) and clear the others. A plain number clears the expression, and then the feature's stored
    /// number applies.
    fn store_arr_component(&mut self, body: Id, keys: [&str; 3], dir: u8, txt: String) {
        let t = txt.trim().to_string();
        let expr = !t.is_empty() && t.parse::<f64>().is_err();
        for (i, k) in keys.iter().enumerate() {
            if i as u8 == dir && expr {
                self.project.set_feat_dim(body, k, t.clone());
            } else {
                self.project.set_feat_dim(body, k, String::new());
            }
        }
    }

    /// The local position (in the body's frame) of the vertex at an end of the persistent edge `edge`
    /// (`end`: false is the start, true is the end). An initial snapshot for an associative datum point;
    /// regeneration refines it through the kernel.
    fn vertex_local_pos(&self, body: Id, edge: u32, end: bool) -> Option<[f64; 3]> {
        let shape = self.live.shapes.get(&body)?;
        let (polys, ids) = shape.edges_with_ids();
        for (poly, id) in polys.iter().zip(ids) {
            if id == edge && poly.len() >= 2 {
                let v = if end { poly[poly.len() - 1] } else { poly[0] };
                return Some([v[0] as f64, v[1] as f64, v[2] as f64]);
            }
        }
        None
    }

    /// The world (origin, dir) of the axis object caught by a click pick (an edge, a cylindrical face or a
    /// datum axis) — WITHOUT creating anything. One resolver for both the preview and the creation (of a
    /// pattern and of a datum axis).
    fn axis_ref_world(&self, hit: AxisHit) -> Option<([f64; 3], [f64; 3])> {
        use qymcad_core::feature::{apply12, is_identity12};
        let ctx = self.current_ctx_id();
        match hit {
            AxisHit::Datum(id) => self.project.datum_axes.iter().find(|d| d.id == id).map(|d| (d.origin(), d.dir())),
            AxisHit::Edge(i) => {
                let (body, _id, poly) = self.edges.axes.get(i)?;
                if poly.len() < 2 {
                    return None;
                }
                let wt = self.project.body_display_transform(*body, ctx);
                let w = |p: &[f32; 3]| -> [f64; 3] {
                    let v = [p[0] as f64, p[1] as f64, p[2] as f64];
                    if is_identity12(&wt) { v } else { apply12(&wt, v) }
                };
                let a = w(&poly[0]);
                let b = w(poly.last().unwrap());
                let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
                let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
                (l > 1e-9).then(|| (a, [d[0] / l, d[1] / l, d[2] / l]))
            }
            AxisHit::Face(body, fid) => {
                let (lo, ld) = self.live.shapes.get(&body)?.face_axis(fid)?;
                let wt = self.project.body_display_transform(body, ctx);
                let o = if is_identity12(&wt) { lo } else { apply12(&wt, lo) };
                let z = if is_identity12(&wt) { [0.0; 3] } else { apply12(&wt, [0.0, 0.0, 0.0]) };
                let d = if is_identity12(&wt) { ld } else { [apply12(&wt, ld)[0] - z[0], apply12(&wt, ld)[1] - z[1], apply12(&wt, ld)[2] - z[2]] };
                let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
                (l > 1e-9).then(|| (o, [d[0] / l, d[1] / l, d[2] / l]))
            }
        }
    }


    /// The command's fields for the current mode. A thread is specified the professional way — by standard
    /// and size; the depth, the diameters and the profile are computed by the model's core from the
    /// standard's formulas (the angle and the thread depth used to be typed by hand, and the result was
    /// whatever came out). An auger has a flight of its own: the outer diameter, the pitch, the thickness.
    fn set_thread_params(&mut self) {
        let d = if self.thread.radius > 1e-6 { self.thread.radius * 2.0 } else { 10.0 };
        self.cmd.params = if self.thread.auger {
            vec![
                CmdParam::new("th-outer-d", "outer", (d * 3.0).max(d + 10.0), 0.5, 2000.0),
                CmdParam::new("th-pitch", "pitch", (d * 2.0).max(10.0), 0.1, 1000.0),
                CmdParam::new("th-length", "length", (d * 5.0).max(30.0), 0.1, 10000.0),
                CmdParam::new("th-flight-thickness", "thickness", (d * 0.25).max(1.0), 0.1, 100.0),
                CmdParam::new("th-edge-fillet", "edge_r", 0.0, 0.0, 50.0),
                // THE FLIGHT'S RUN-OUT: without it the turn breaks off square, like a stump. Half a pitch by default.
                CmdParam::new("th-taper-in", "lead_in", (d * 1.0).max(5.0), 0.0, 10000.0),
                CmdParam::new("th-taper-out", "lead_out", (d * 1.0).max(5.0), 0.0, 10000.0),
            ]
        } else {
            vec![
                CmdParam::new("th-nominal-d", "nominal", d, 0.5, 1000.0),
                // 0 means the standard coarse pitch for this diameter (ISO 261 / ISO 2901), so choosing the
                // size is enough.
                CmdParam::new("th-pitch-std", "pitch", 0.0, 0.0, 100.0),
                CmdParam::new("th-length", "length", (d * 1.5).max(10.0), 0.1, 10000.0),
                // THE FIT CLEARANCE is what printed threads are made for: a bolt and a nut screw together.
                CmdParam::new("th-fit-clearance", "fit", 0.2, 0.0, 5.0),
                CmdParam::new("th-lead-in", "lead_in", 1.5, 0.0, 10000.0),
                CmdParam::new("th-lead-out", "lead_out", 1.5, 0.0, 10000.0),
                // FILLETS on both the crest and the root. 0 means follow the standard; a value of one's own
                // overrides the table — for printing the crest is rounded more, so the layers do not tear.
                CmdParam::new("th-crest-fillet", "crest_r", 0.0, 0.0, 100.0),
                CmdParam::new("th-root-fillet", "root_r", 0.0, 0.0, 100.0),
            ]
        };
        // A "custom" form is a profile NOT described by a standard, so the angle and the depth are given by
        // hand. Without these fields choosing "custom" changed nothing: the angle silently stayed at 60 deg
        // and the depth at 0.6 of the pitch.
        if !self.thread.auger && Self::thread_standard(self.thread.form) == qymcad_core::thread::ThreadStandard::Custom {
            let p = self.cmd_val("pitch");
            self.cmd.params.push(CmdParam::new("th-profile-angle", "angle", 60.0, 5.0, 170.0));
            self.cmd.params.push(CmdParam::new("th-depth", "depth", if p > 0.0 { p * 0.6 } else { 0.0 }, 0.0, 1000.0));
        }
    }

    /// Is the cylindrical face an INTERNAL one (a hole)? The core decides, from the face's triangles; here
    /// we only fetch the mesh and the face.
    fn cyl_face_is_internal(&self, body: Id, fid: u32, center: [f64; 3], axis: [f64; 3]) -> bool {
        let Some(mi) = self.project.mesh_index(body) else { return false };
        let Some(f) = self.project.bodies.get(mi).and_then(|b| b.faces.iter().find(|f| f.id == fid)) else { return false };
        qymcad_core::geom::cyl_face_is_internal(&self.project.bodies[mi].mesh, &f.triangles, center, axis)
    }

    /// Apply the active Part command (Enter). On success it finishes CLEANLY, with no arrow and no panel
    /// left behind.
    /// Re-enter the contour selection of the active sketch command (extrude, revolve) — back to the 2D
    /// half-sketcher, where a click on a contour changes the set of profiles. It works while editing too (a
    /// double click on a feature): change the contours, press Enter, and `update_feat` rebuilds the
    /// operation with the new profile.
    /// A NUMBER FIELD THAT ACCEPTS AN EXPRESSION — one widget for every tool bar.
    ///
    /// It returns the value (recomputed from the text) because a `&mut` to the state field cannot be taken:
    /// that field lives in the same `self`. A broken expression is marked as bad and does NOT change the
    /// value: better a field that turns red than rubbish travelling into the model.
    ///
    /// `integer` means the quantity is whole by its nature (a number of copies, a number of sides): the
    /// expression is evaluated and ROUNDED rather than rejected. "count = n_bolts" has to work.
    pub(super) fn num_or_expr(&mut self, ui: &mut egui::Ui, key: &'static str, cur: f64, lo: f64, hi: f64, integer: bool, suffix: &str) -> f64 {
        let vars = self.project.param_map();
        // THE TEXT IS BORROWED FOR A MOMENT. The drop-down list reads the whole document while the buffer
        // sits in the same `self`, and both cannot be borrowed at once. We work on a copy and put it back.
        let txt = self
            .bar_exprs
            .get(key)
            .cloned()
            .unwrap_or_else(|| if integer { format!("{cur:.0}") } else { qymcad_core::expr::fmt_num(cur) });
        let w = (60.0 + suffix.len() as f32 * 6.0).min(96.0);
        // ONE FIELD, AND IN THE FEATURES TOO, not only in sketches. This expression field is a single one
        // for ALL the tool bars (see `expr_fields.rs`), so the list of drivers and the typing rules arrive
        // everywhere at once: in extrude, fillet, chamfer, shell, hole, draft and pattern.
        //
        // A tool bar does not change the document — it shows a preview — so the text lives in `bar_exprs`
        // and is read every frame; the rule that editing text is not editing the model holds by itself here.
        let o = expr_field::expr_field(ui, &self.project, egui::Id::new(("bar_expr", key)), &txt, w, &crate::i18n::tr("g-expr-placeholder"));
        let txt = o.text;
        self.bar_exprs.insert(key, txt.clone());
        match qymcad_core::expr::eval(&txt, &vars) {
            Ok(v) => {
                let v = if integer { v.round() } else { v };
                v.clamp(lo, hi)
            }
            Err(_) => {
                ui.colored_label(self.scheme.pal.error_mild(), ph::X);
                cur
            }
        }
    }

    /// Borrow the view for a flat sub-mode: remember the state it was left in. A repeat call does not
    /// overwrite it — the first entry holds the state to come back to.
    pub(super) fn borrow_view(&mut self) {
        if self.view_restore.is_none() {
            self.view_restore = Some((self.mode_3d, self.cam, self.view));
        }
    }

    /// Restore the view to the state it was left in. With nothing to restore (the sub-mode was entered
    /// without `borrow_view`), the old behaviour applies: back to 3D, but WITHOUT refitting.
    pub(super) fn return_view(&mut self) {
        match self.view_restore.take() {
            Some((m, cam, view)) => {
                self.mode_3d = m;
                self.cam = cam;
                self.view = view;
            }
            None => self.mode_3d = true,
        }
    }

    /// A test facade: is the half-sketcher for re-picking contours open? It looks at the same thing a
    /// person does — a flat view inside an active sketch command.
    #[cfg(test)]
    pub(crate) fn contour_repick_active_for_test(&self) -> bool {
        matches!(self.cmd.kind, 1 | 3) && self.cmd.sketch.is_some() && !self.mode_3d
    }

    fn enter_contour_reselect(&mut self) {
        if !matches!(self.cmd.kind, 1 | 3) || self.cmd.sketch.is_none() {
            return;
        }
        self.borrow_view(); // the view is borrowed while contours are picked and restored on the way out
        self.mode_3d = false; // back to the flat half-sketcher: a click on a contour changes the set of profiles
        self.view.initialized = false;
        self.cmd.focus = false;
        self.status = crate::i18n::tr("g-contour-reselect");
    }

    /// Open the HALF-SKETCHER for picking the contour of a sweep or loft slot (`sid` is the slot's sketch).
    /// The same mechanism as in Extrude: the sketch is shown flat and a click on a contour fills the slot
    /// (see the 2D click handler and `set_contour_slot`). It replaces cycling through contours with arrows.
    fn begin_contour_pick(&mut self, slot: ContourSlot, sid: Id) {
        let Some(si) = self.project.sketch_index(sid) else {
            self.status = crate::i18n::tr("g-slot-sketch-missing");
            return;
        };
        self.picking = Picking::Contour(slot);
        self.cmd.sketch = Some(si);
        self.borrow_view(); // the view is borrowed while the contour is picked and restored on the way out
        self.mode_3d = false;
        self.view.initialized = false;
        self.cmd.focus = false;
        let what = match slot {
            ContourSlot::SweepProfile => "g-slot-profile",
            ContourSlot::SweepPath => "g-slot-path",
            ContourSlot::LoftSection(_) => "g-slot-section",
        };
        self.status = crate::i18n::tr1("g-contour-pick-of", "what", &crate::i18n::tr(what));
    }

    /// The slot's candidate contours (for hit-testing in the half-sketcher). A path allows open contours.
    fn slot_candidates(&self, slot: ContourSlot) -> Vec<Id> {
        match slot {
            ContourSlot::SweepProfile => self.project.sweep_profile_contours(self.sweep.prof_sid),
            ContourSlot::SweepPath => self.project.sweep_path_contours(self.sweep.path_sid),
            ContourSlot::LoftSection(i) => self.project.sweep_profile_contours(self.loft.sids.get(i).copied().unwrap_or(0)),
        }
    }

    /// Store the picked contour `cid` into the sweep or loft slot.
    fn set_contour_slot(&mut self, slot: ContourSlot, cid: Id) {
        match slot {
            ContourSlot::SweepProfile => self.sweep.prof_cid = cid,
            ContourSlot::SweepPath => self.sweep.path_cid = cid,
            ContourSlot::LoftSection(i) => {
                if let Some(c) = self.loft.cids.get_mut(i) {
                    *c = cid;
                }
            }
        }
    }

    /// The slot's current contour (for highlighting in the half-sketcher). 0 means the first suitable
    /// contour of the sketch, chosen automatically.
    fn slot_current_cid(&self, slot: ContourSlot) -> Id {
        let resolve = |sid: Id, cid: Id, path: bool| -> Id {
            if cid != 0 {
                return cid;
            }
            let cands = if path { self.project.sweep_path_contours(sid) } else { self.project.sweep_profile_contours(sid) };
            cands.first().copied().unwrap_or(0)
        };
        match slot {
            ContourSlot::SweepProfile => resolve(self.sweep.prof_sid, self.sweep.prof_cid, false),
            ContourSlot::SweepPath => resolve(self.sweep.path_sid, self.sweep.path_cid, true),
            ContourSlot::LoftSection(i) => {
                let sid = self.loft.sids.get(i).copied().unwrap_or(0);
                resolve(sid, self.loft.cids.get(i).copied().unwrap_or(0), false)
            }
        }
    }


    /// The label of a chamfer's second field: its meaning depends on the mode — the second leg, or the angle.
    fn chamfer_d2_label(mode: qymcad_core::feature::ChamferMode) -> &'static str {
        use qymcad_core::feature::ChamferMode;
        match mode {
            ChamferMode::DistAngle => "cmd-angle-deg",
            _ => "cmd-leg2",
        }
    }

    /// Cancel the active Part command (Esc) — a clean reset.
    /// CLEARING A FEATURE COMMAND'S PICKS — the single place where every subsystem's picks go out.
    ///
    /// A 3D feature command accumulates references in its own subsystem: a sweep's profile and path, a
    /// loft's sections, a draft's neutral face, a mirror's plane, a pattern's axis, a datum's references.
    /// This reset used to be written out separately in the apply and in the cancel, while the command's
    /// start did not reset at all — so going Sweep, then Loft, then Sweep again without cancelling showed
    /// the picks from the previous round. Now the start, the apply and the cancel all call ONE reset: a
    /// command always begins clean.
    fn clear_feat_picks(&mut self) {
        self.thread.src = None;
        self.thread.edge = 0;
        self.mirror.plane = None;
        self.arr.axis_pick = false;
        self.datum.plane_pick = None;
        self.datum.axis_ref = None;
        self.datum.axis_hit = None;
        self.datum.axis_pts.clear();
        self.datum.pt_vert = None;
        self.sweep.prof_sid = 0;
        self.sweep.path_sid = 0;
        self.sweep.pick_path = false;
        self.sweep.prof_cid = 0;
        self.sweep.path_cid = 0;
        self.loft.sids.clear();
        self.loft.cids.clear();
        self.loft.pick = false;
        self.loft.pick_last = None;
        self.loft.ruled = false;
        self.picking.clear();
        self.draft.neutral = 0;
        self.draft.pick_neutral = false;
        self.draft.flip = false;
        self.stitch_parts.clear(); // the sheets picked for stitching are a pick like everything above
        self.trim.keep = None;
        self.trim.tool = None;
    }

    /// FINISHING A FEATURE COMMAND (applied or cancelled) — both the aiming and the subject are cleared.
    ///
    /// The difference from `clear_feat_picks`: the command's subject (the sketch, the profiles, the
    /// parameters, the selected edges and faces) must NOT be cleared at the START — it is gathered BEFORE
    /// the button is pressed, as a pre-selection. But once the command is over it has to be forgotten, or
    /// the next one would pick up someone else's selection.
    fn end_feat_cmd_state(&mut self) {
        self.clear_feat_picks();
        self.cmd.sketch = None;
        self.cmd.params.clear();
        self.gsel.profiles.clear();
        self.gsel.edges.clear();
        self.gsel.faces.clear();
        self.gsel.faces_body = None; // the scope of the multiple face selection
        // UNFINISHED MENU GESTURES end together with the command too: waiting for the second face of a
        // "between", the memory of the face and edge last pointed at. A wait that outlived its command
        // would fire in the next one.
        self.gsel.between_first = None;
        self.gsel.last_face = None;
        self.gsel.last_edge = None;
        self.gsel.described = None;
    }

    fn cancel_feat_cmd(&mut self) {
        // THE MODE IS READ BEFORE THE CLOSE. `cmd.close()` resets the command to its default state, and
        // `prev_3d` along with it, to `false`. The old code read it AFTERWARDS and so compared the current
        // view against a false "it was flat": working in 3D, Esc out of ANY command threw you into a flat
        // projection with a refit. Reported behaviour: open the editing of an operation, press Esc, and the
        // viewport breaks into some two-dimensional projection — and so with everything.
        let prev_3d = self.cmd.prev_3d;
        self.cmd.close(); // the command is closed as a whole, not just a field zeroed out
        self.cmd.edit = None;
        self.end_feat_cmd_state();
        // The view borrowed for the command is RESTORED whole (see `borrow_view`): the camera belongs to
        // the person, and cancelling an action does not touch it. If nothing was borrowed, simply restore
        // the mode, WITHOUT refitting: refitting is exactly what throws away a carefully arranged view.
        if self.view_restore.is_some() {
            self.return_view();
        } else if self.mode_3d != prev_3d {
            self.mode_3d = prev_3d;
        }
        self.status = crate::i18n::tr("msg-cancelled");
    }

    /// Clear ALL the active tools and picking modes. Called on a change of workbench and on entering or
    /// leaving a component: a command belonging to one context must NOT leak into another (an assembly tool
    /// used to stay active after a double click into a part, and so on across every workbench).
    /// CANCEL EVERYTHING (Esc from the top bar, or the start of a new command): every active mode goes out.
    ///
    /// Assembled from the transitions that already exist rather than from a field list of its own. Such a
    /// copy is what it used to be — the seventh one — and its set differed from the others: it cleared the
    /// joint pick and the section but left the modify mode, the dimension's first reference, the dragging
    /// and the in-place editing alone. What is listed here is ONLY what is in neither the exit from the
    /// sketch tools nor the reset of a feature's aiming: the assembly, section and clipboard modes.
    fn cancel_all_tools(&mut self) {
        if self.cmd.active() {
            self.cancel_feat_cmd();
        }
        self.exit_draw_tools(); // every sketch mode, in one transition
        self.clear_feat_picks(); // the aiming of the feature's subsystems
        self.op_pick = None;
        self.boolean.pick = None;
        self.boolean.edit = None;
        self.joint.pick_faces = false;
        self.joint.pick_first = None;
        self.joint.ground_pick = false; // "Ground" is a tool too, and does not survive a change
        self.joint.group_pick = None; // an unfinished group set
        self.joint.width_pick = None; // an unfinished width set
        self.joint.tangent_pick = None; // an unfinished tangent set
        self.joint.axis_pick = None; // an unfinished pointing at an anchor's secondary axis
        self.joint.relation_pick = None; // an unfinished set of a relation between joints
        self.joint.conn_pick = false; // an unfinished creation of a standalone connector
        self.section.pick = false; // the section pick (the section itself lives until it is switched off)
        self.section.drag = false;
        self.section.drag_anchor = None;
        self.mirror.part = None; // an unfinished pick of the part to mirror
        self.pending_import.clear(); // the whole unfinished import (the curves plus the points)
        self.clip.geom_pending = None; // an unfinished copy or paste of geometry
        self.clip.geom_place = false;
        self.m3.clear(); // the 3D measuring tool is a tool too, exclusive with the rest
        self.carr = CompArrayCmd::default(); // an unfinished component pattern
    }


    /// The Part layout: K a new sketch, D a datum plane, E extrude, Q cut, R revolve, F fillet, C chamfer,
    /// H shell, O hole, M mirror, B box, Y cylinder.
    pub(super) fn part_hotkey(&mut self, key: egui::Key) {
        // WE MATCH ON THE ACTION, NOT ON THE KEY. While `Key::E` stood in the `match`, remapping was
        // inexpressible: the letter and the meaning were one and the same thing. Which key leads to which
        // action is decided by `hotkey_action` — one place for every workbench.
        let Some(action) = self.hotkey_action("part", key) else { return };
        match action {
            "part.sketch-pick" => self.toggle_sketch_pick(),
            "part.datum-plane" => self.start_feat_cmd(20),
            "part.extrude" => {
                self.feat.op = 0;
                self.start_feat_cmd(1);
            }
            "part.cut" => {
                self.feat.op = 2;
                self.start_feat_cmd(1);
            }
            "part.revolve" => self.start_feat_cmd(3),
            "part.fillet" => self.start_feat_cmd(4),
            "part.chamfer" => self.start_feat_cmd(5),
            "part.shell" => self.start_feat_cmd(6),
            "part.hole" => self.start_feat_cmd(7),
            "part.mirror" => self.start_feat_cmd(16),
            "part.box" => self.start_prim_cmd(10),
            "part.cylinder" => self.start_prim_cmd(11),
            "part.measure" => self.toggle_measure_3d(),
            // go back to picking the contours of the active sketch command (extrude, revolve) from its 3D step
            "part.contour-reselect" if matches!(self.cmd.kind, 1 | 3) && self.cmd.sketch.is_some() && self.mode_3d => self.enter_contour_reselect(),
            _ => {}
        }
    }

    /// The Assembly layout: K a new skeleton sketch, D a datum plane, N a new part, U a subassembly,
    /// I insert a component (STEP or STL), J a rigid joint (picking faces).
    pub(super) fn assembly_hotkey(&mut self, key: egui::Key) {
        let Some(action) = self.hotkey_action("assembly", key) else { return };
        match action {
            // there are NO sketch keys in an Assembly: a sketch is inert there (see `create_panel_common`)
            "assembly.datum-plane" => self.start_feat_cmd(20),
            "assembly.new-part" => {
                let id = self.project.add_part(crate::i18n::tr1("node-part-n", "n", &self.project.components.len().to_string()));
                self.enter_component(id);
            }
            "assembly.new-subassembly" => {
                let id = self.project.add_assembly(crate::i18n::tr1("node-assembly-n", "n", &self.project.components.len().to_string()));
                self.enter_component(id);
            }
            "assembly.insert" => self.pick_step(),
            "assembly.rigid-joint" => {
                self.cancel_all_tools();
                self.joint.new_kind = qymcad_core::feature::JointKind::Rigid;
                self.joint.pick_faces = true;
                self.joint.pick_first = None;
                self.status = crate::i18n::tr("g-rigid-joint-hint");
            }
            _ => {}
        }
    }


    /// Bring the application's caches (faces, visibility) into line with the current list of meshes after a
    /// change of topology (deleting a sketch or a feature) and rebuild the bodies from the timeline — the
    /// mesh indices may have shifted.
    fn resync_after_topology_change(&mut self) {
        // The faces are RE-HUNG by body Id rather than cleared. A `vec![Vec::new(); ...]` plus a FORCED
        // regeneration used to stand here (and only that regeneration filled the faces back in) — meaning
        // EVERY deletion of a node rebuilt and re-tessellated the WHOLE document. On an assembly of 1170
        // imported solids that is tens of seconds of freeze per Delete. The mesh indices shift after a
        // deletion, but body Ids are stable, so that is what they are laid out by; only what the deletion
        // actually made dirty needs rebuilding.
        self.rebuild_faces_from_cache();
        self.gsel.edges.clear();
        self.edges.body = None;
        // THE TOPOLOGY CHANGED — THE DERIVED CACHES ARE STALE, whether anything was rebuilt or not.
        //
        // Reported behaviour: delete "Push face" from the tree and the body disappears from the viewport
        // until Edit -> Rebuild everything. The cause: `geom_rev` (the key of every derived cache) only
        // ticks inside a rebuild that actually happened, and deleting a leaf modifier leaves no dirty node
        // at all — there is nothing to compute, and the scheduler honestly does nothing. The
        // `consumed_bodies` cache meanwhile stays as it was on the previous frame, where the source body was
        // still consumed by the deleted feature — and `body_shown` hides the ONLY remaining body. Bodies
        // would vanish the same way after deleting any modifying feature (remove face, split body, chamfer,
        // shell).
        //
        // So the counter is advanced HERE: a change of topology is itself the event "the derived data is
        // invalid", and it need not coincide with a rebuild of the geometry.
        self.invalidate();
        self.regenerate_all(); // only the dirty nodes (the deletion cascade has already marked them)
        self.detect_missing_faces(); // mesh-based detection ONLY for raw meshes with no B-rep
        self.view.initialized = false;
    }

    /// Lay the faces out of the `faces_by_body` cache onto the CURRENT mesh indices. The cache is keyed by
    /// body Id and so survives the deletions and reorderings that shift the indices — the faces used to be
    /// restored after a change of topology only by a full forced regeneration.
    fn rebuild_faces_from_cache(&mut self) {
        let live: std::collections::HashSet<Id> = self.project.bodies.iter().map(|b| b.id).collect();
        self.live.faces.retain(|b, _| live.contains(b));
        for b in self.project.bodies.iter_mut() {
            if let Some(f) = self.live.faces.get(&b.id) {
                b.faces = f.clone();
            }
        }
    }

    /// Remember a body's faces and put them into the index-parallel `self.faces` (the single point where
    /// these two representations cannot drift apart).
    /// REORDER A TIMELINE NODE — one handle for everyone.
    ///
    /// The core refuses by itself if the move would break the dependencies (a consumer ending up above its
    /// input); what is left here is keeping the caches in step. Pulled out of the menu item for the same
    /// reason as the section: logic inside a button is out of reach of a check, and the hand would use a
    /// different door from the person's.
    /// Returns whether the move happened.
    pub(crate) fn move_feature(&mut self, from: usize, to: usize) -> bool {
        let done = self.project.reorder_feature(from, to);
        if done {
            self.resync_after_topology_change();
        }
        done
    }

    /// TURN THE SECTION VIEW ON OR OFF — one handle for everyone.
    ///
    /// The logic used to live inside the panel's button, and there was no way to reach it by hand: a test
    /// either poked at the fields directly (checking itself, that is) or checked nothing at all. The rule
    /// that a hand must use the same doors a person does applies here too.
    pub(crate) fn toggle_section(&mut self) {
        if self.section.plane.is_some() || self.section.pick {
            self.section.plane = None;
            self.section.pick = false;
            self.section.drag = false;
            self.section.drag_anchor = None;
            self.invalidate();
            self.status = crate::i18n::tr("tb-section-off");
        } else {
            self.cancel_all_tools();
            self.section.pick = true;
            self.status = crate::i18n::tr("tb-section-pick");
        }
    }

    fn set_body_faces(&mut self, body: Id, faces: Vec<MeshFace>) {
        // THE BODY WAS REBUILT — THE STORED BLOB IS STALE. It is dropped here rather than in one of the
        // rebuild branches: the synchronous and the background one meet exactly at this point, and they must
        // not drift apart — a stale blob would go into the file and open as a body FROM THE PAST.
        self.live.blobs.remove(&body);
        if let Some(idx) = self.project.mesh_index(body) {
            if idx < self.project.bodies.len() {
                self.project.bodies[idx].faces = faces.clone();
            }
        }
        self.live.faces.insert(body, faces);
    }


    /// Remove the ghosts (orphan meshes and dangling features) — a delegate to the tested
    /// `Project::prune_dangling`; here we only clear the application's shape cache by the list of removed bodies.
    fn prune_dangling_features(&mut self) {
        for db in self.project.prune_dangling() {
            self.live.shapes.remove(&db);
        }
    }

    /// Fill in the faces by mesh detection ONLY for bodies WITHOUT a B-rep (a raw imported STL): bodies
    /// built by the timeline and those imported from STEP already have their faces from the B-rep topology.
    /// The principle: faces are derived from the B-rep, and mesh detection applies only where there is no
    /// B-rep at all.
    fn detect_missing_faces(&mut self) {
        for i in 0..self.project.bodies.len() {
            let has_brep = self.project.mesh_id(i).is_some_and(|id| self.live.shapes.contains_key(&id));
            let empty = self.project.bodies.get(i).map_or(true, |b| b.faces.is_empty());
            if !has_brep && empty {
                let f = self.project.bodies[i].mesh.detect_faces(8.0);
                self.project.bodies[i].faces = f.clone();
                if let Some(body) = self.project.mesh_id(i) {
                    self.live.faces.insert(body, f); // mesh detection goes into the cache by body Id as well
                }
            }
        }
    }

    fn delete_contour(&mut self, i: usize) {
        // drop the operations' references to this contour by Id (the indices do NOT shift: the other Ids
        // are stable and no reindexing is needed)
        let removed = self.project.contour_id(i);
        self.project.remove_contour(i);
        if let Some(rid) = removed {
            for op in &mut self.project.operations {
                op.selection.retain(|&x| x != rid);
            }
        }
        self.sel = Sel::None;
        self.invalidate();
        self.view.initialized = false;
    }

    fn face_props(&mut self, ui: &mut egui::Ui, mi: usize, fi: usize) {
        // INFORMATION ABOUT THE FACE, and nothing else. A face is only selected under the Shell or Hole
        // command (which is where this panel shows); the older buttons (sketch, plane, contour, projection,
        // Z0) are gone — a sketch on a face is made with the Sketch tool on the toolbar (a click on the
        // face), and a datum by the datum command.
        let Some(face) = self.project.bodies.get(mi).and_then(|b| b.faces.get(fi)) else { return };
        let n = face.normal;
        // THE LINEAGE IS TAKEN FROM THE BODY: a face has no Id of its own in the document, but "what made
        // the thing this face lies on" is exactly the question the face's properties are opened for.
        let lin = self.lineage_of(self.project.mesh_id(mi));
        props_header(ui, ph::SQUARE_HALF, "face-props-title", NameSlot::None, &lin);
        ui.label(crate::i18n::tr1("face-area", "v", &format!("{:.1}", face.area)));
        ui.label(crate::i18n::trn("face-normal", &[("n", &format!("[{}, {}, {}]", crate::i18n::num(n[0],2), crate::i18n::num(n[1],2), crate::i18n::num(n[2],2))), ("side", &crate::i18n::tr(normal_label(n)))]));
        ui.label(crate::i18n::tr1("face-center", "v", &format!("{:.1}, {:.1}, {:.1}", face.centroid.x, face.centroid.y, face.centroid.z)));
        ui.label(egui::RichText::new(crate::i18n::tr("face-picked-for-cmd")).weak().small());
    }

    /// Create (with target == 0) or re-target the face (target = plane_id) of an "offset from a face" datum plane.
    fn make_offset_plane_from_face(&mut self, target: Id, body: Id, key: qymcad_core::feature::FaceKey) {
        use qymcad_core::model::{PlaneDef, WorkPlane};
        if target == 0 {
            let wp = WorkPlane { name: crate::i18n::tr("plane-from-face"), def: PlaneDef::OffsetFace { body, face: key, dist: 10.0 }, ..Default::default() };
            let id = self.project.add_plane(wp);
            self.sel = self.project.planes.iter().position(|p| p.id == id).map(Sel::Plane).unwrap_or(Sel::None);
        } else if let Some(pi) = self.project.planes.iter().position(|p| p.id == target) {
            let dist = if let PlaneDef::OffsetFace { dist, .. } = self.project.planes[pi].def { dist } else { 10.0 };
            self.project.planes[pi].def = PlaneDef::OffsetFace { body, face: key, dist };
            self.sel = Sel::Plane(pi);
        }
        self.regen_after_datum_change();
        self.status = crate::i18n::tr("plane-from-face-hint");
    }


    /// The properties of a datum POINT: its name, its coordinates and a delete button. Parametric
    /// definitions live elsewhere.
    fn datum_point_props(&mut self, ui: &mut egui::Ui, i: usize) {
        let lin = self.lineage_of(Some(self.project.datum_points[i].id));
        if let Some(n) = props_header(ui, ph::DOT, "datum-point-title", NameSlot::Editable(self.project.datum_points[i].name.clone()), &lin) {
            self.project.datum_points[i].name = n;
        }
        let mut changed = false;
        {
            let d = &mut self.project.datum_points[i];
            ui.label(&crate::i18n::tr("datum-point-coords"));
            egui::Grid::new(("dpt", i)).num_columns(2).spacing([6.0, 3.0]).show(ui, |ui| {
                changed |= drag(ui, "X", &mut d.at[0], 1.0, -100000.0..=100000.0);
                changed |= drag(ui, "Y", &mut d.at[1], 1.0, -100000.0..=100000.0);
                changed |= drag(ui, "Z", &mut d.at[2], 1.0, -100000.0..=100000.0);
            });
        }
        if changed {
            self.datum.regen_pending = true; // the point moved, so an axis through two points and the bodies need rebuilding (debounced)
        }
        ui.separator();
        if ui.button(format!("{} {}", ph::TRASH, crate::i18n::tr("datum-point-delete"))).clicked() {
            self.ask_delete(Sel::DatumPoint(i));
        }
    }



    fn tools_tree(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(&crate::i18n::tr("cam-tools-tree")).strong());
            if ui.small_button(ph::PLUS).on_hover_text(&crate::i18n::tr("cam-add-tool")).clicked() {
                let next = self.project.tools.iter().map(|t| t.number).max().unwrap_or(0) + 1;
                self.project.tools.push(default_tool(next));
                self.sel = Sel::Tool(self.project.tools.len() - 1);
            }
        });
        for i in 0..self.project.tools.len() {
            let t = &self.project.tools[i];
            let label = format!("{} T{} · {} Ø{}", ph::SCREWDRIVER, t.number, t.name, t.diameter);
            if ui.selectable_label(self.sel == Sel::Tool(i), label).clicked() {
                self.sel = Sel::Tool(i);
            }
        }
    }

    fn tool_props(&mut self, ui: &mut egui::Ui, i: usize) {
        ui.heading(format!("{} {}", ph::SCREWDRIVER, crate::i18n::tr("cam-tool-title")));
        let mut do_remove = false;
        {
            let t = &mut self.project.tools[i];
            egui::Grid::new(("toolg", i)).num_columns(2).spacing([6.0, 4.0]).show(ui, |ui| {
                ui.label(&crate::i18n::tr("cam-tool-number"));
                ui.add(egui::DragValue::new(&mut t.number).range(1..=999));
                ui.end_row();
                ui.label(&crate::i18n::tr("cam-setup-name"));
                crate::gui::name_edit(ui, &mut t.name);
                ui.end_row();
                ui.label(&crate::i18n::tr("cam-tool-kind"));
                tool_type_combo(ui, i, &mut t.kind);
                ui.end_row();
                ui.label(&crate::i18n::tr("cam-tool-d"));
                ui.add(egui::DragValue::new(&mut t.diameter).speed(0.1).range(0.1..=50.0));
                ui.end_row();
                ui.label(&crate::i18n::tr("cam-tool-corner-r"));
                ui.add(egui::DragValue::new(&mut t.corner_radius).speed(0.1).range(0.0..=25.0));
                ui.end_row();
                ui.label(&crate::i18n::tr("cam-tool-flutes"));
                ui.add(egui::DragValue::new(&mut t.flutes).range(1..=12));
                ui.end_row();
            });
        }
        ui.add_space(6.0);
        if self.project.tools.len() > 1 && ui.button(format!("{} {}", ph::TRASH, crate::i18n::tr("cam-tool-delete"))).clicked() {
            do_remove = true;
        }
        if do_remove {
            self.project.tools.remove(i);
            self.sel = Sel::None;
        }
    }



    fn setup_props(&mut self, ui: &mut egui::Ui, si: usize) {
        use qymcad_core::model::Wcs;
        ui.heading(format!("{} {}", ph::STACK, crate::i18n::tr("cam-setup-title")));
        let mut changed = false;
        {
            let s = &mut self.project.setups[si];
            ui.horizontal(|ui| {
                ui.label(&crate::i18n::tr("cam-setup-name"));
                crate::gui::name_edit(ui, &mut s.name);
            });
            ui.horizontal(|ui| {
                ui.label(&crate::i18n::tr("cam-wcs"));
                egui::ComboBox::from_id_salt(("setupwcs", si)).selected_text(s.wcs.label()).show_ui(ui, |ui| {
                    for w in Wcs::ALL {
                        if ui.selectable_value(&mut s.wcs, w, w.label()).changed() {
                            changed = true;
                        }
                    }
                });
            });
        }
        let nops = self.project.operations.iter().filter(|o| o.setup == si).count();
        ui.label(egui::RichText::new(crate::i18n::tr1("cam-setup-ops-n", "n", &nops.to_string())).weak().small());
        ui.label(egui::RichText::new(&crate::i18n::tr("cam-setup-note")).weak().small());
        ui.separator();
        if ui.button(format!("{} {}", ph::TRASH, crate::i18n::tr("cam-setup-delete"))).clicked() {
            self.project.setups.remove(si);
            for op in &mut self.project.operations {
                if op.setup == si {
                    op.setup = 0;
                } else if op.setup > si {
                    op.setup -= 1;
                }
            }
            self.sel = Sel::None;
            changed = true;
        }
        if changed {
            self.invalidate();
        }
    }


    // `viewport` was 1509 lines and looked like a monolith, but it was in fact TWO unrelated viewports in
    // one if/else branch: the 3D one (537 lines) and the flat sketch one (959). While they lay together,
    // neither showed its phases — and neither could be taken apart on its own.




    /// THE FRAME PROLOGUE: free the previous frame's textures, show the splash screen, raise the undo
    /// baseline, finish a deferred datum regeneration, and intercept the closing of a window with unsaved
    /// work.
    ///
    /// Returns `false` when the rest of the frame must NOT be drawn (the start-up load is running and only
    /// the overlay is shown). This used to be a `return` in the middle of `update`, which made the prologue
    /// impossible to separate from the rest of the frame: the early exit hid inside the shared body.
    fn frame_prologue(&mut self, ctx: &egui::Context) -> bool {
        // free the previous frame's textures HERE, at the start and before any drawing, so that nothing is
        // still drawing them (otherwise wgpu panics with "Texture ... has been destroyed" on submit).
        self.tex_graveyard.clear();
        // the splash screen and the progress. While the start-up load of a document or a background job is
        // running, only the spinner overlay is drawn and we leave (the window with the logo is visible at
        // once and the UI does not appear frozen).
        if self.tick_async(ctx) {
            return false;
        }
        // the first initialisation of the undo baseline
        if !self.edits.ready {
            self.edits.baseline = self.snapshot();
            self.edits.committed_key = self.doc_key();
            self.edits.saved_key = self.edit_key(); // at start-up (an empty document, or one opened automatically) there are no edits
            self.edits.ready = true;
        }
        // Debouncing a datum edit: while a coordinate or a direction is being dragged (the pointer is held
        // down) the consumers are NOT force-regenerated every frame; the datum's own glyph moves live,
        // because the renderer reads the fields directly. Release the pointer and one regeneration runs at
        // the final position. Done BEFORE the panels, so the new geometry shows in this same frame.
        if self.datum.regen_pending && !ctx.input(|i| i.pointer.any_down()) {
            self.datum.regen_pending = false;
            self.regen_after_datum_change();
        }
        // Intercepting the closing of the window (the cross, or Alt+F4): with unsaved work, cancel the close and ask.
        if ctx.input(|i| i.viewport().close_requested()) {
            // a background write must reach the disk even when the document is "clean" (Save was pressed and
            // the window closed straight away) — otherwise one is sure it saved while the file stayed as it
            // was (the write is atomic: an interruption does not spoil the old file, but neither does it
            // carry the new edits).
            self.wait_bg();
            if !self.edits.allow_close && self.is_dirty() {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                if self.deferred.nav.is_none() {
                    self.deferred.nav = Some(Nav::Exit);
                }
            }
        }
        true
    }

    /// THE SELECTION FOLLOWS THE SKETCH BEING EDITED.
    ///
    /// While a sketch is being edited the selection stays on it (all the logic works through `Sel::Sketch`),
    /// and the sketch itself is remembered for projecting the part's geometry into it. Changing the active
    /// sketch clears the accumulated selection — otherwise it moves onto SOMEONE ELSE'S geometry. A phase
    /// of the frame rather than an implementation detail of a neighbouring block: this used to sit between
    /// the window-close interception and the keyboard, and the link "the sketch changed, so the selection
    /// was cleared" was invisible.
    fn keep_selection_on_edited_sketch(&mut self) {
        // sketch editing mode: the selection is kept on it (all the logic works through `Sel::Sketch`)
        match self.edit_si() {
            Some(si) => {
                self.sel = Sel::Sketch(si);
                self.workbench = Workbench::Sketch;
            }
            None => {
                if self.sketch_ses.editing.is_some() {
                    self.sketch_ses.editing = None; // the sketch is gone, so leave the mode
                }
            }
        }
        // remember the active sketch (for projecting the part's geometry into it)
        if let Sel::Sketch(si) = self.sel {
            if let Some(s) = self.project.sketches.get(si) {
                if self.sketch_ses.last != Some(s.id) {
                    self.sel_sk.clear(); // the selection, and whatever was waiting on it: the sketch changed, so the element selection goes
                }
                self.sketch_ses.last = Some(s.id);
            }
        } else {
            self.sel_sk.clear(); // the selection, and whatever was waiting on it
        }
    }










    fn viewport(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            let (resp, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
            let rect = resp.rect;
            self.view_rect = rect; // where the canvas stands: the rebuild overlay needs it so as not to blank it
            // THE CANVAS BACKGROUND COMES FROM THE SCHEME. A `from_gray(26)` used to stand here, asking the
            // theme nothing at all: switch to the light one and the viewport and the sketcher stayed black.
            painter.rect_filled(rect, 0.0, self.scheme.pal.viewport_bg());
            let has_geom = !self.project.contours.is_empty() || !self.project.bodies.is_empty();
            let scroll = ctx.input(|i| i.raw_scroll_delta.y);

            if self.mode_3d {
                self.viewport_3d(ctx, &resp, &painter, rect, has_geom, scroll);
            } else {
                self.viewport_2d(ctx, &resp, &painter, rect, has_geom, scroll);
            }
        });
    }

    /// A raycast on a click in 3D: the nearest triangle under the cursor gives its face.
    /// A click in the viewport while an operation's geometry is being gathered adds a body or a contour.
    fn op_pick_at(&mut self, rect: Rect, screen: Pos2) {
        let Some(op_i) = self.active_op() else {
            self.op_pick = None;
            return;
        };
        match self.op_pick {
            Some(OpPick::Body) => {
                if let Some(mi) = self.mesh_under_cursor(rect, screen) {
                    if let Some(id) = self.project.mesh_id(mi) {
                        if !self.project.operations[op_i].bodies.contains(&id) {
                            self.project.operations[op_i].bodies.push(id);
                        }
                        self.status = crate::i18n::tr1("cam-part-added-named", "name", &crate::i18n::name(&self.project.mesh_name(mi)));
                        self.invalidate();
                    }
                } else {
                    self.status = crate::i18n::tr("cam-part-not-found");
                }
            }
            Some(OpPick::Contour) => {
                if let Some(ci) = self.contour_under_cursor_3d(rect, screen) {
                    if let Some(id) = self.project.contour_id(ci) {
                        if !self.project.operations[op_i].selection.contains(&id) {
                            self.project.operations[op_i].selection.push(id);
                        }
                        self.status = crate::i18n::tr1("cam-contour-added-n", "n", &(ci + 1).to_string());
                        self.invalidate();
                    }
                } else {
                    self.status = crate::i18n::tr("cam-contour-not-found");
                }
            }
            None => {}
        }
        self.op_pick = None;
    }

    /// The index of the body under the cursor (in 3D), regardless of faces.
    fn mesh_under_cursor(&self, rect: Rect, screen: Pos2) -> Option<usize> {
        let basis = self.cam.basis();
        let mut best: Option<(f64, usize)> = None;
        for (mi, mesh) in self.project.bodies.iter().map(|b| &b.mesh).enumerate() {
            if !self.body_shown(mi) {
                continue;
            }
            for ti in 0..mesh.tris.len() {
                let t = mesh.triangle(ti);
                let (pa, da) = self.project3([t[0].x, t[0].y, t[0].z], rect, &basis);
                let (pb, db) = self.project3([t[1].x, t[1].y, t[1].z], rect, &basis);
                let (pc, dc) = self.project3([t[2].x, t[2].y, t[2].z], rect, &basis);
                if point_in_tri(screen, pa, pb, pc) {
                    let depth = tri_depth_at(screen, pa, da, pb, db, pc, dc);
                    if best.map_or(true, |(bd, _)| depth < bd) {
                        best = Some((depth, mi));
                    }
                }
            }
        }
        best.map(|(_, mi)| mi)
    }

    /// The index of the contour under the cursor in 3D (contours lie at Z=0).
    fn contour_under_cursor_3d(&self, rect: Rect, screen: Pos2) -> Option<usize> {
        let basis = self.cam.basis();
        let mut best: Option<(f32, usize)> = None;
        for (ci, c) in self.project.contours.iter().enumerate() {
            if c.points.len() < 2 {
                continue;
            }
            let n = c.points.len();
            let last = if c.closed { n } else { n - 1 };
            for k in 0..last {
                let a = c.points[k];
                let b = c.points[(k + 1) % n];
                let pa = self.project3([a.x, a.y, 0.0], rect, &basis).0;
                let pb = self.project3([b.x, b.y, 0.0], rect, &basis).0;
                let d = screen_dist_seg(screen, pa, pb);
                if best.map_or(true, |(bd, _)| d < bd) {
                    best = Some((d, ci));
                }
            }
        }
        best.filter(|(d, _)| *d <= 8.0).map(|(_, ci)| ci)
    }

    /// Reported behaviour: the base planes XY/XZ/YZ are impossible to hit — they flash for a moment and
    /// vanish as soon as the cursor moves by a pixel. A fixed half-size of 60 mm makes a clickable spot the
    /// size of a pinhead at the scale of a typical assembly (hundreds of millimetres across, with the camera
    /// far back) — the square is barely visible and cannot be hovered. So it is scaled by the actual
    /// bounding box of the visible scene, as the section gizmo is; an empty scene keeps the old default.
    fn plane_pick_half_size(&self) -> f64 {
        let mut lo = [f64::MAX; 3];
        let mut hi = [f64::MIN; 3];
        for (_, _, _, _, mesh, wt) in self.visible_mesh_items() {
            if let Some(b) = mesh.bounds() {
                for c in [[b.min.x, b.min.y, b.min.z], [b.max.x, b.max.y, b.max.z]] {
                    let w = qymcad_core::feature::apply12(&wt, c);
                    for k in 0..3 {
                        lo[k] = lo[k].min(w[k]);
                        hi[k] = hi[k].max(w[k]);
                    }
                }
            }
        }
        if lo[0] > hi[0] {
            return 60.0; // an empty scene
        }
        let diag = ((hi[0] - lo[0]).powi(2) + (hi[1] - lo[1]).powi(2) + (hi[2] - lo[2]).powi(2)).sqrt();
        (diag * 0.3).clamp(60.0, 500.0)
    }

    /// The WORLD frame of a candidate sketch plane, built the same way `Project::sketch_frame` builds it
    /// (World gives `b.frame()`, Datum and Face give `world_aligned`), so that the (u, v) snapping is
    /// computed in THE SAME axes the origin is later lifted in. For a face it is the local base frame
    /// carried into world space by `body_display_transform`.
    fn world_frame_of_plane(&self, sp: &qymcad_core::feature::SketchPlane) -> Option<qymcad_core::feature::PlaneFrame> {
        use qymcad_core::feature::{PlaneFrame, SketchPlane};
        match sp {
            SketchPlane::World(b) => Some(b.frame()),
            SketchPlane::Datum(id) => self.project.planes.iter().find(|p| p.id == *id).map(|p| PlaneFrame::world_aligned(p.origin, p.normal, p.rot_deg)),
            SketchPlane::Face(body, key) => {
                let (c, n) = self.project.resolve_face(*body, key);
                let wt = self.project.body_display_transform(*body, self.current_ctx_id());
                Some(PlaneFrame::world_aligned(c, n, 0.0).transformed(&wt))
            }
        }
    }

    fn project3(&self, p: [f64; 3], rect: Rect, basis: &([f64; 3], [f64; 3], [f64; 3])) -> (Pos2, f64) {
        let rel = v_sub(p, self.cam.target);
        let sx = v_dot(rel, basis.0);
        let sy = v_dot(rel, basis.1);
        let depth = v_dot(rel, basis.2);
        // Perspective: the eye sits a finite `d_eye` behind the target plane along -fwd, and the screen
        // offset is divided by (1 + depth/d_eye). In orthographic mode `inv_d_eye` is 0, so f = 1 and the
        // projection matches the previous one exactly. The denominator is clamped from below (a point in
        // front of the eye) — unreachable in a CAD orbit, but it guards against degeneracy.
        let inv = self.persp_inv_d_eye(rect.height() * 0.5);
        let f = (1.0 / (1.0 + depth * inv).max(0.05)) as f32;
        let c = rect.center();
        (Pos2::new(c.x + sx as f32 * f * self.cam.scale, c.y - sy as f32 * f * self.cam.scale), depth)
    }

    /// THE LINE OF SIGHT AT POINT `p` — the ray FROM THE EYE, not the camera's overall direction.
    ///
    /// In orthographic mode there is one ray for the whole frame, and it is `fwd`. In perspective every
    /// point has its own, diverging from `fwd` towards the edges of the frame the more, the wider the field
    /// of view. Back-face culling computed from `fwd` therefore lied both ways: visible faces were thrown
    /// away (gaps in the body) and invisible ones stayed (a ring fell apart into ribbons). One formula for
    /// the raster and for the shader.
    fn view_dir_at(&self, p: [f64; 3], fwd: [f64; 3], inv_d: f64) -> [f64; 3] {
        if inv_d <= 0.0 {
            return fwd;
        }
        let d_eye = 1.0 / inv_d;
        let eye = [self.cam.target[0] - fwd[0] * d_eye, self.cam.target[1] - fwd[1] * d_eye, self.cam.target[2] - fwd[2] * d_eye];
        v_norm(v_sub(p, eye))
    }

    /// `inv_d_eye = 1/d_eye` for perspective (0 in orthographic mode). `d_eye = world_half_h / tan(fov/2)`,
    /// where `world_half_h = half_h_points / scale`. The single source of the formula for `project3` and for
    /// the GPU shader.
    fn persp_inv_d_eye(&self, half_h_points: f32) -> f64 {
        if !self.set.cam_perspective {
            return 0.0;
        }
        // THE HALF-TANGENT FROM THE ANGLE: the setting holds an angle in degrees (which is what one
        // pictures), while the projection needs the half-tangent. The conversion lives here alone, at the
        // point of use.
        (self.set.persp_fov_deg.to_radians() / 2.0).tan() * self.cam.scale as f64 / (half_h_points.max(1.0) as f64)
    }

    /// The world bounding sphere (centre, radius) of the visible scene, taken from `visible_mesh_items`
    /// (exactly what gets drawn). Cached by the scene key: recomputed only when the scene changes, not
    /// every frame.
    fn scene_sphere_cached(&self, scene_key: u64) -> Option<([f64; 3], f64)> {
        let (k, cached) = self.cache.bounds.get();
        if k == scene_key {
            return cached;
        }
        let (mut mn, mut mx) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
        let mut any = false;
        for (_, _, _, _, mesh, wt) in self.visible_mesh_items() {
            let ident = qymcad_core::feature::is_identity12(&wt);
            for v in &mesh.verts {
                let p = if ident { [v.x, v.y, v.z] } else { qymcad_core::feature::apply12(&wt, [v.x, v.y, v.z]) };
                any = true;
                for a in 0..3 {
                    mn[a] = mn[a].min(p[a]);
                    mx[a] = mx[a].max(p[a]);
                }
            }
        }
        let sphere = any.then(|| {
            let c = [(mn[0] + mx[0]) * 0.5, (mn[1] + mx[1]) * 0.5, (mn[2] + mx[2]) * 0.5];
            let r = (((mx[0] - mn[0]).powi(2) + (mx[1] - mn[1]).powi(2) + (mx[2] - mn[2]).powi(2)).sqrt() * 0.5).max(1e-3);
            (c, r)
        });
        self.cache.bounds.set((scene_key, sphere));
        sphere
    }

    /// The frame's projection parameters: `(inv_d_eye, z_near, z_far, depth_half)`. Perspective takes TIGHT
    /// near and far planes from the scene's bounding box (the most z-buffer precision, so thin features do
    /// not z-fight); orthographic takes a linear depth from `depth_half`. `inv_d_eye = 0` means
    /// orthographic. The single source for the CPU (`depth_ndc`) and for the GPU (`CamRaw`).
    fn proj_params(&self, rect: Rect, scene_key: u64) -> (f64, f64, f64, f64) {
        let half_w = (rect.width() * 0.5) as f64;
        let half_h = (rect.height() * 0.5) as f64;
        let scale = (self.cam.scale as f64).max(1e-4);
        let depth_half = (half_w.max(half_h) / scale) * 50.0 + 1000.0; // the orthographic range (as in `CamRaw::new`)
        let inv_d = self.persp_inv_d_eye(rect.height() * 0.5);
        if inv_d <= 0.0 {
            return (0.0, 0.0, 0.0, depth_half);
        }
        let d_eye = 1.0 / inv_d;
        let (mut z_near, mut z_far) = (d_eye * 0.05, d_eye * 4.0); // the fallback when the scene is empty
        if let Some((c, r)) = self.scene_sphere_cached(scene_key) {
            let (_, _, fwd) = self.cam.basis();
            let dc = v_dot(v_sub(c, self.cam.target), fwd); // the world depth of the sphere's centre along the line of sight
            let margin = (r * 0.05).max(1e-3);
            z_near = (dc - r + d_eye - margin).max(d_eye * 0.02); // clamped above 0 (a point in front of the eye)
            z_far = (dc + r + d_eye + margin).max(z_near + d_eye * 0.01);
        }
        (inv_d, z_near, z_far, depth_half)
    }

    /// The screen-linear depth `ndc_z` for a world `depth = rel . fwd`. In perspective this is
    /// `clip_z/clip_w` (PERSPECTIVE-CORRECT, planar on the screen), so the linear interpolation in
    /// `raster_band` gives the right visibility; in orthographic mode it is linear in world space. The
    /// formula is THE SAME as in the GPU shader (`vs_mesh`), so both paths agree. `near` maps to 0 and
    /// `far` to 1, smaller is nearer (the z comparison is `<`). The parameters come from `proj_params`,
    /// computed once per frame rather than per vertex.
    fn depth_ndc(&self, world_depth: f64, inv_d: f64, z_near: f64, z_far: f64, depth_half: f64) -> f32 {
        if inv_d > 0.0 {
            let d_eye = 1.0 / inv_d;
            let zc = (world_depth + d_eye).max(z_near); // clamped to a point in front of the eye (the CPU path does not clip)
            let a = z_far / (z_far - z_near);
            let b = -z_near * z_far / (z_far - z_near);
            (a + b / zc) as f32
        } else {
            (0.5 + world_depth / (2.0 * depth_half)) as f32
        }
    }

    /// The faces of the ViewCube: (normal, four screen points, depth) in the corner of the viewport.
    /// TURN THE VIEW SMOOTHLY to the given angles (about 220 ms). An instant jump reads as something having
    /// broken: the eye has nothing to hold on to, and on a complex assembly the part has to be found again.
    pub(super) fn animate_view_to(&mut self, yaw: f64, pitch: f64) {
        self.mode_3d = true;
        // THE SHORTEST WAY ROUND IN YAW: without normalisation a turn from -170 deg to +170 deg would go
        // all the way round — 340 deg instead of 20.
        let mut from_yaw = self.cam.yaw;
        while yaw - from_yaw > std::f64::consts::PI {
            from_yaw += std::f64::consts::TAU;
        }
        while yaw - from_yaw < -std::f64::consts::PI {
            from_yaw -= std::f64::consts::TAU;
        }
        self.view_anim = Some(((from_yaw, self.cam.pitch), (yaw, pitch), std::time::Instant::now()));
    }

    /// Advance the turning animation. Called every frame; while it runs it asks for a repaint.
    pub(super) fn tick_view_anim(&mut self, ctx: &egui::Context) {
        let Some((from, to, t0)) = self.view_anim else { return };
        const DUR: f32 = 0.22;
        let t = (t0.elapsed().as_secs_f32() / DUR).clamp(0.0, 1.0);
        // easing in and out: a linear turn looks mechanical
        let e = (t * t * (3.0 - 2.0 * t)) as f64;
        self.cam.yaw = from.0 + (to.0 - from.0) * e;
        self.cam.pitch = from.1 + (to.1 - from.1) * e;
        if t >= 1.0 {
            self.view_anim = None;
            self.cam.init = false; // on arrival refit the scale and the target, as the instant jump used to
        } else {
            ctx.request_repaint();
        }
    }

    /// START SWEEPING A DEGREE OF FREEDOM: `false` means there is nothing to sweep, and the caller must say why.
    pub(crate) fn start_joint_anim(&mut self, joint: Id, slot: usize) -> bool {
        let Some((from, to)) = self.project.joint_anim_range(joint, slot) else { return false };
        let saved = self.project.joints.iter().find(|j| j.id == joint).and_then(|j| j.drive[slot]);
        self.joint_anim = Some(JointAnim { joint, slot, from, to, t: 0.0, forward: true, saved });
        true
    }

    /// STOP THE SWEEP AND PUT EVERYTHING BACK.
    ///
    /// A sweep is a preview, not an edit: leaving the part wherever the stop caught it would silently change
    /// the document by pressing a "have a look" button.
    pub(crate) fn stop_joint_anim(&mut self) {
        let Some(a) = self.joint_anim.take() else { return };
        if let Some(j) = self.project.joints.iter_mut().find(|j| j.id == a.joint) {
            j.drive[a.slot] = a.saved;
        }
        self.project.solve_joints();
        self.mark_dirty_for_rebuild(); // the document is marked; the scheduler does the computing
    }

    /// ONE STEP OF THE SWEEP: move the degree of freedom and re-solve the assembly.
    ///
    /// It travels there and back rather than round in a circle: a travel or an angle with limits has no
    /// circle to it, and jumping from one end to the other would read as a jerk rather than as a mechanism
    /// moving.
    pub(crate) fn step_joint_anim(&mut self, dt: f64) {
        let Some(a) = self.joint_anim.as_mut() else { return };
        let step = dt / 2.0; // a full pass in one direction takes two seconds
        if a.forward {
            a.t += step;
            if a.t >= 1.0 {
                a.t = 1.0;
                a.forward = false;
            }
        } else {
            a.t -= step;
            if a.t <= 0.0 {
                a.t = 0.0;
                a.forward = true;
            }
        }
        let (joint, slot, v) = (a.joint, a.slot, a.from + (a.to - a.from) * a.t);
        if let Some(j) = self.project.joints.iter_mut().find(|j| j.id == joint) {
            j.drive[slot] = Some(v);
        }
        self.project.solve_joints();
    }

    /// Advance the degree-of-freedom sweep. Called every frame; while it runs it asks for a repaint.
    pub(super) fn tick_joint_anim(&mut self, ctx: &egui::Context) {
        if self.joint_anim.is_none() {
            return;
        }
        // THE TIME COMES FROM THE FRAME rather than from counting frames: on a slow machine a frame count
        // would stretch the travel, and "two seconds" would turn into who knows how many.
        let dt = ctx.input(|i| i.stable_dt).clamp(0.0, 0.1) as f64;
        self.step_joint_anim(dt);
        ctx.request_repaint();
    }

    /// The check doors for a standalone connector — the same path a person takes.
    #[cfg(test)]
    pub(crate) fn start_conn_pick_for_test(&mut self) {
        self.start_conn_pick();
    }
    #[cfg(test)]
    pub(crate) fn conn_pick_active_for_test(&self) -> bool {
        self.joint.conn_pick
    }
    #[cfg(test)]
    pub(crate) fn delete_connector_asked_for_test(&mut self, cid: Id) {
        self.delete_connector_asked(cid);
    }

    /// The check doors for the degree-of-freedom sweep — the same path a person takes.
    #[cfg(test)]
    pub(crate) fn start_joint_anim_for_test(&mut self, joint: Id, slot: usize) -> bool {
        self.start_joint_anim(joint, slot)
    }
    #[cfg(test)]
    pub(crate) fn step_joint_anim_for_test(&mut self, dt: f64) {
        self.step_joint_anim(dt);
    }
    #[cfg(test)]
    pub(crate) fn stop_joint_anim_for_test(&mut self) {
        self.stop_joint_anim();
    }
    #[cfg(test)]
    pub(crate) fn joint_anim_active_for_test(&self) -> bool {
        self.joint_anim.is_some()
    }

    /// Test facades for the ViewCube: a test must take the same path the mouse does.
    #[cfg(test)]
    pub(crate) fn viewcube_size_pub(&self) -> f32 {
        self.viewcube_size()
    }

    #[cfg(test)]
    pub(crate) fn viewcube_zone_at_pub(&self, rect: Rect, pos: Pos2) -> Option<usize> {
        self.viewcube_zone_at(rect, pos)
    }

    #[cfg(test)]
    pub(crate) fn viewcube_click_pub(&mut self, rect: Rect, pos: Pos2) -> bool {
        self.viewcube_click(rect, pos)
    }

    /// The centre of a zone on screen — the point the mouse aims at.
    #[cfg(test)]
    pub(crate) fn viewcube_zone_center_pub(&self, rect: Rect, i: usize) -> Pos2 {
        let z = &super::gui::viewcube::zones()[i];
        let pts: Vec<Pos2> = z.poly.iter().map(|p| self.viewcube_project_pub(*p, rect)).collect();
        let n = pts.len() as f32;
        Pos2::new(pts.iter().map(|p| p.x).sum::<f32>() / n, pts.iter().map(|p| p.y).sum::<f32>() / n)
    }

    /// Run the turning animation to its end — a test has no reason to wait for real time.
    #[cfg(test)]
    pub(crate) fn finish_view_anim_pub(&mut self) {
        if let Some((_, to, _)) = self.view_anim.take() {
            self.cam.yaw = to.0;
            self.cam.pitch = to.1;
        }
    }

    /// The start and the end of the current animation — for checking the shortest way round.
    #[cfg(test)]
    pub(crate) fn view_anim_endpoints_pub(&self) -> Option<((f64, f64), (f64, f64))> {
        self.view_anim.map(|(a, b, _)| (a, b))
    }

    fn fit3d(&mut self, rect: Rect) {
        let mut mn = [f64::INFINITY; 3];
        let mut mx = [f64::NEG_INFINITY; 3];
        let mut acc = |p: [f64; 3]| {
            for a in 0..3 {
                if p[a] < mn[a] {
                    mn[a] = p[a];
                }
                if p[a] > mx[a] {
                    mx[a] = p[a];
                }
            }
        };
        for m in self.project.bodies.iter().map(|b| &b.mesh) {
            for v in &m.verts {
                acc([v.x, v.y, v.z]);
            }
        }
        for c in self.project.contours.iter() {
            for p in &c.points {
                acc([p.x, p.y, 0.0]);
            }
        }
        if !mn[0].is_finite() {
            return;
        }
        self.cam.target = [(mn[0] + mx[0]) / 2.0, (mn[1] + mx[1]) / 2.0, (mn[2] + mx[2]) / 2.0];
        let ext = (mx[0] - mn[0]).max(mx[1] - mn[1]).max(mx[2] - mn[2]).max(1.0) as f32;
        self.cam.scale = (rect.width().min(rect.height()) / ext) * 0.55;
        self.cam.init = true;
    }

    /// The key of the 3D render cache: the view (camera, size, pixels per point), the geometry revision and
    /// the visibility.
    fn view_key(&self, rect: Rect, ppp: f32) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.cam.yaw.to_bits().hash(&mut h);
        self.cam.pitch.to_bits().hash(&mut h);
        (self.cam.scale as f64).to_bits().hash(&mut h);
        for t in self.cam.target {
            t.to_bits().hash(&mut h);
        }
        ((rect.width() * ppp) as u32).hash(&mut h);
        ((rect.height() * ppp) as u32).hash(&mut h);
        self.view_dragging.hash(&mut h); // the low-resolution and full-resolution pictures are separate cache entries
        self.regen.geom_rev.hash(&mut h);
        // the components' placements: moving a part in an assembly changes the picture, and the raster cache must see it
        for c in &self.project.components {
            for v in c.transform {
                v.to_bits().hash(&mut h);
            }
            c.visible.hash(&mut h); // visibility is hierarchical (`body_shown` follows the chain of ticks), so it goes into the key
        }
        self.set.cam_perspective.hash(&mut h); // perspective against orthographic changes the projection, so it goes into the raster cache key
        self.set.smooth_shading.hash(&mut h); // smooth against flat shading changes the vertex colours, so it goes in too
        self.scheme.pal.fingerprint().hash(&mut h); // the raster is already coloured by the scheme: change the scheme and it is drawn anew
        (self.win.sim && self.cam_job.sim_mesh.is_some()).hash(&mut h);
        // The selection highlight affects the texture, so the WHOLE set of highlighted bodies goes into the
        // cache key (a body highlights itself; a component or subassembly highlights its whole subtree).
        // Only `Sel::Mesh` and `Sel::Face` used to be hashed, so a click on a body inside an assembly (which
        // is a `Sel::Component`) did not invalidate the raster, and the highlight appeared only after the
        // camera moved.
        let mut hl: Vec<usize> = self.highlight_mesh_set().into_iter().collect();
        hl.sort_unstable();
        hl.hash(&mut h);
        for (i, v) in self.project.bodies.iter().map(|b| b.visible).enumerate() {
            if v {
                (i as u32).hash(&mut h);
            }
        }
        // in-context mode and the active context change which bodies are visible and which are ghosted, so the raster cache must see them
        self.win.context.hash(&mut h);
        self.current_ctx_id().hash(&mut h);
        // EDITING A FEATURE CHANGES WHICH BODIES ARE DRAWN — AND THE KEY MUST SEE THAT.
        //
        // On entering the editing of a modifier, the program shows the state BEFORE it: the feature's result
        // and its whole descendant chain are hidden (`edit_hidden_bodies`), and the consumed source is
        // shown. On leaving, it puts everything back. Neither went into the key, and the picture changed
        // only by accident: if the camera happened to move as well, or `geom_rev` happened to tick.
        //
        // Reported on a freshly opened document: enter a fillet, press Esc — the fillet and the chamfer
        // under it are gone, and the features are not in error, they simply are not drawn. The model was
        // intact the whole time; the raster was left over from the editing mode: entering it had been
        // redrawn by a heavy rebuild (which ticked `geom_rev`), and on leaving there was nothing left to
        // change the key.
        self.cmd.edit.hash(&mut h);
        h.finish()
    }

    /// The visible bodies to draw: (highlighted, ghosted, base colour, mesh, world transform).
    /// The single source for the CPU raster and for the GPU pass (it accounts for the simulation, consumed
    /// bodies, feature editing, the assembly context and the body gizmo's preview).
    /// The first element of the tuple is the mesh's index in `project.meshes` (for the normals cache);
    /// `usize::MAX` marks the simulation (there is no body behind it, so smooth normals are not cached and
    /// it falls back to flat shading).
    fn visible_mesh_items(&self) -> Vec<(usize, bool, bool, [u8; 3], &qymcad_core::geom::Mesh, [f64; 12])> {
        let sim_active = self.win.sim && self.cam_job.sim_mesh.is_some();
        // the selection highlight: a body highlights itself; a component highlights its whole subtree (the part or subassembly entire)
        let hl = self.highlight_mesh_set();
        // every mesh carries the WORLD transform of its owning component: a body is built in the part's
        // local frame, and `world_transform` (composed up the assembly tree) places it into the world.
        if sim_active {
            self.cam_job.sim_mesh.iter().map(|m| (usize::MAX, false, false, [120, 135, 162], m, qymcad_core::feature::PLACE_IDENTITY)).collect()
        } else {
            // While a modifier feature (shell, hole, fillet, chamfer and so on) is being edited, the state
            // BEFORE it is shown: its RESULT and its whole descendant chain are hidden. The consumed SOURCE
            // is shown by `body_shown`, which is also what throws consumed bodies away — keeping a SECOND
            // check of the same thing here was a mistake: the exception for the source ended up behind an
            // earlier refusal and never worked.
            let edit_hide = self.edit_hidden_bodies();
            let ctx = self.current_ctx_id(); // the placement is RELATIVE to the active context (a part sits at the origin, an assembly in place)
            self.project
                .bodies
                .iter()
                .map(|b| &b.mesh)
                .enumerate()
                .filter(|(mi, _)| self.body_shown(*mi))
                .filter(|(mi, _)| !self.project.mesh_id(*mi).is_some_and(|b| edit_hide.contains(&b))) // the feature being edited plus its descendant chain
                .map(|(mi, m)| {
                    let mut wt = self.project.mesh_id(mi).map(|b| self.project.body_display_transform(b, ctx)).unwrap_or(qymcad_core::feature::PLACE_IDENTITY);
                    // THE BODY GIZMO'S PREVIEW: while dragging, the accumulated transform is laid over this body
                    // (with no B-rep rebuild until release). Ctrl snaps.
                    if let Some((dmi, _, _)) = self.body_giz.drag {
                        if dmi == mi {
                            if let Some(accum) = self.body_giz_accum(self.body_giz.snap) {
                                wt = compose12(&accum, &wt);
                            }
                        }
                    }
                    (mi, hl.contains(&mi), self.body_is_ghost(mi), self.project.mesh_color(mi), m, wt)
                })
                .collect()
        }
    }

    /// Shading a triangle, plus the branches for a highlighted body and for a ghosted one.
    /// One source of colour for the CPU raster and for the GPU pass (the normal and the light are in world
    /// space, so they do not depend on the camera).
    ///
    /// THE DEPTH OF THE SHADOW COMES FROM THE SCHEME. `0.4 + 0.6 * |n . light|` used to stand here, and the
    /// function had no `&self` — it simply could not look at the scheme. Against a dark background a shadow
    /// down to 40% brightness reads very well; against a light one that same shadow turns the part into a
    /// dark blot.
    /// `ghost_alpha` is an ARGUMENT for the same reason as the scheme: a function without `&self` can ask
    /// about neither. Hiding the setting in a constant here would mean a second source of truth about a
    /// ghost's opacity — exactly the case that once kept the scheme from reaching the shading at all.
    fn shade_tri(pal: &crate::palette::Palette, ghost_alpha: u8, hot: bool, ghost: bool, base: [u8; 3], n: [f64; 3], light: [f64; 3]) -> Color32 {
        let diff = v_dot(n, light).abs();
        let lit = crate::palette::lit(pal.shade_floor_body, diff as f32);
        // shading can only darken: without this the brightest a part gets is its own colour, which on a
        // light canvas is darker than the background. So the BRIGHTNESS is raised (lightness plus
        // saturation) rather than white being mixed in: white washes the colour out and gives whitewash
        // instead of a light-coloured part.
        let base = crate::palette::brighten(base, pal.body_lighten, pal.body_saturate);
        if hot {
            // a selected BODY or COMPONENT is LIGHTER than its own colour: lifted towards white plus a
            // slight cool tint, so that it reads as selected and highlighted. That shows the part or
            // subassembly was selected entire; NOT bright orange, which was confused with a face selection.
            let b = |c: f32, cool: f32| {
                let v = c * lit;
                (v + (255.0 - v) * 0.4 + cool).min(255.0) as u8
            };
            Color32::from_rgb(b(base[0] as f32, 0.0), b(base[1] as f32, 8.0), b(base[2] as f32, 22.0))
        } else if ghost {
            // a context ghost (a neighbouring part): a dim blue-grey and TRANSLUCENT, so that one's own body
            // or sketch shows through it. Real transparency (a two-pass render), not dimming.
            // A quarter of its own colour and three quarters of the colour the scheme leads a ghost towards:
            // on a dark background that is darkness, as it was; on a light one it is the canvas itself.
            let t = pal.ghost_target;
            let m = |c: u8, k: usize| (c as f32 * lit * 0.25 + t[k] as f32 * 0.75) as u8;
            Color32::from_rgba_unmultiplied(m(base[0], 0), m(base[1], 1), m(base[2], 2), ghost_alpha)
        } else {
            Color32::from_rgb((base[0] as f32 * lit) as u8, (base[1] as f32 * lit) as u8, (base[2] as f32 * lit) as u8)
        }
    }

    /// The scene key for the GPU buffer: the same as in `view_key`, BUT without the camera, the size or the
    /// dragging flag (the vertices and their colours do not depend on the camera). When it changes, the
    /// vertex buffer is uploaded anew.
    fn gpu_scene_key(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.view_rev().hash(&mut h);
        self.set.smooth_shading.hash(&mut h); // smooth against flat gives different vertex colours, so the buffer is re-uploaded
        self.scheme.pal.fingerprint().hash(&mut h); // the bodies' colours live IN THE BUFFER: a change of scheme must re-upload it
        if let Some((o, n)) = self.section_eff() {
            for v in o.iter().chain(n.iter()) {
                v.to_bits().hash(&mut h); // the section moved or turned, so the buffer is rebuilt
            }
        }
        for c in &self.project.components {
            for v in c.transform {
                v.to_bits().hash(&mut h);
            }
            c.visible.hash(&mut h);
        }
        (self.win.sim && self.cam_job.sim_mesh.is_some()).hash(&mut h);
        let mut hl: Vec<usize> = self.highlight_mesh_set().into_iter().collect();
        hl.sort_unstable();
        hl.hash(&mut h);
        for (i, v) in self.project.bodies.iter().map(|b| b.visible).enumerate() {
            if v {
                (i as u32).hash(&mut h);
            }
        }
        self.win.context.hash(&mut h);
        self.current_ctx_id().hash(&mut h);
        self.cmd.edit.hash(&mut h); // editing a feature hides its result and its chain — see `view_key`
        // the body gizmo's preview: while dragging, the body's transform changes frame by frame, and the scene must see it
        if let Some((dmi, _, _)) = self.body_giz.drag {
            (dmi as u64).hash(&mut h);
            if let Some(accum) = self.body_giz_accum(self.body_giz.snap) {
                for v in accum {
                    v.to_bits().hash(&mut h);
                }
            }
        }
        h.finish()
    }

    /// Refresh the cache of smoothed vertex normals for the current `geom_rev` (lazily, indexed as
    /// `project.meshes` is). The heavy pass over the triangles is done ONCE per change of geometry rather
    /// than every frame.
    fn ensure_vertex_normals(&self) {
        let mut c = self.cache.norm.borrow_mut();
        if c.0 == self.regen.geom_rev && c.1.len() == self.project.bodies.len() {
            return;
        }
        c.1 = self.project.bodies.iter().map(|b| &b.mesh).map(|m| m.vertex_normals()).collect();
        c.0 = self.regen.geom_rev;
    }

    /// Rotate a normal by the linear (3x3) part of a body's world transform, ignoring the translation. The
    /// components' transforms are rigid and uniform, so R * n is enough without an inverse transpose; the
    /// result is normalised.
    fn rotate_normal(wt: &[f64; 12], n: [f64; 3]) -> [f64; 3] {
        v_norm([
            wt[0] * n[0] + wt[1] * n[1] + wt[2] * n[2],
            wt[4] * n[0] + wt[5] * n[1] + wt[6] * n[2],
            wt[8] * n[0] + wt[9] * n[1] + wt[10] * n[2],
        ])
    }


    /// Recompute the pairs of bodies that INTERPENETRATE (interference) — lazily, while idle. Bodies of
    /// different components that are visible in the context are transformed into the context's frame ONCE,
    /// and then intersected pairwise for a common volume. It is expensive, so it runs only while
    /// `show_interference` is on, the scene is not being dragged, and the cache is stale.
    pub(super) fn refresh_interference(&mut self) {
        if !self.set.show_interference || !matches!(self.workbench, Workbench::Assembly) {
            if !self.interference.pairs.is_empty() {
                self.interference.pairs.clear();
            }
            self.interference.rev = u64::MAX;
            return;
        }
        // the scene is moving (a gizmo drag), or the cache is fresh: nothing to recompute
        if self.joint_drag_active() || self.comp_giz.drag.is_some() || self.body_giz.drag.is_some() {
            return;
        }
        if self.interference.rev == self.regen.geom_rev {
            return;
        }
        self.interference.rev = self.regen.geom_rev;
        let ctx = self.current_ctx_id();
        // the visible bodies that have an owning component become shapes in the context's frame (transformed once)
        let mut placed: Vec<(Id, Id, qymcad_kernel::Shape)> = Vec::new();
        for mi in 0..self.project.bodies.len() {
            if !self.body_shown(mi) {
                continue;
            }
            let Some(body) = self.project.mesh_id(mi) else { continue };
            let Some(owner) = self.project.body_owner(body) else { continue };
            let wt = self.project.body_display_transform(body, ctx);
            if let Some(ws) = self.live.shapes.get(&body).and_then(|s| s.transformed(&wt)) {
                placed.push((body, owner, ws));
            }
        }
        let mut pairs = Vec::new();
        for i in 0..placed.len() {
            for j in (i + 1)..placed.len() {
                if placed[i].1 == placed[j].1 {
                    continue; // bodies of the same part: their overlap is normal, not interference
                }
                // A PAIR THE KERNEL CANNOT MEASURE IS FLAGGED TOO. "Could not measure" is not "they are
                // clear of each other", and treating it as the second is how a part ends up sitting inside
                // another one with nothing said about it.
                if placed[i].2.interference_volume(&placed[j].2).is_none_or(|v| v > 1e-3) {
                    pairs.push((placed[i].0, placed[j].0));
                }
            }
        }
        self.interference.pairs = pairs;
    }

    /// Whether a body takes part in an interference (for the red highlight in `draw_mesh`).
    fn body_interferes(&self, body: Id) -> bool {
        self.interference.pairs.iter().any(|(a, b)| *a == body || *b == body)
    }

    /// A rubber-band selection: add to the active operation every contour whose bounding box crosses the
    /// rectangle (screen corners a to b).
    fn box_select(&mut self, rect: Rect, a: Pos2, b: Pos2) {
        // while a sketch is being edited, the band selects its entities into `sel_sk`
        if let Sel::Sketch(si) = self.sel {
            if self.sketch_ses.editing.is_some() {
                self.box_select_sketch(rect, a, b, si);
                return;
            }
        }
        let Some(op_i) = self.active_op() else { return };
        let w0 = self.to_world(rect, a);
        let w1 = self.to_world(rect, b);
        let (xmin, xmax) = (w0.x.min(w1.x), w0.x.max(w1.x));
        let (ymin, ymax) = (w0.y.min(w1.y), w0.y.max(w1.y));
        let mut added = Vec::new();
        for (i, c) in self.project.contours.iter().enumerate() {
            if let Some(bb) = c.bbox() {
                let overlap = bb.min.x <= xmax && bb.max.x >= xmin && bb.min.y <= ymax && bb.max.y >= ymin;
                if overlap {
                    if let Some(id) = self.project.contour_id(i) {
                        added.push(id);
                    }
                }
            }
        }
        let sel = &mut self.project.operations[op_i].selection;
        for id in added {
            if !sel.contains(&id) {
                sel.push(id);
            }
        }
    }

    /// A rubber-band selection of sketch entities: left to right means enclosure (wholly inside), right to
    /// left means crossing (merely touched) — the usual CAD convention.
    fn box_select_sketch(&mut self, rect: Rect, a: Pos2, b: Pos2, si: usize) {
        use qymcad_core::model::EntityKind;
        let crossing = b.x < a.x;
        let (x0, x1) = (a.x.min(b.x), a.x.max(b.x));
        let (y0, y1) = (a.y.min(b.y), a.y.max(b.y));
        let inside = |p: Pos2| p.x >= x0 && p.x <= x1 && p.y >= y0 && p.y <= y1;
        // gather the hits first (`s` holds a borrow of self, so they are pushed afterwards)
        let (mut add_ent, mut add_pt): (Vec<Id>, Vec<Id>) = (Vec::new(), Vec::new());
        {
            let Some(s) = self.project.sketches.get(si) else { return };
            let scr = |id: Id| s.points.iter().find(|q| q.id == id).map(|q| self.to_screen(rect, Point2::new(q.x, q.y)));
            for e in &s.entities {
                let ids: Vec<Id> = match e.kind {
                    EntityKind::Line { a, b } => vec![a, b],
                    EntityKind::Arc { a, b, .. } => vec![a, b],
                    EntityKind::Circle { center, .. } => vec![center],
                    EntityKind::Ellipse { c, ma, mi } => vec![c, ma, mi],
                };
                let pts: Vec<Pos2> = ids.iter().filter_map(|id| scr(*id)).collect();
                if pts.is_empty() {
                    continue;
                }
                let hit = if crossing { pts.iter().any(|p| inside(*p)) } else { pts.iter().all(|p| inside(*p)) };
                if hit {
                    add_ent.push(e.id);
                }
            }
            for p in &s.points {
                if inside(self.to_screen(rect, Point2::new(p.x, p.y))) {
                    add_pt.push(p.id);
                }
            }
        }
        for id in add_ent {
            if !self.sel_sk.items.contains(&(1, id)) {
                self.sel_sk.items.push((1, id));
            }
        }
        for id in add_pt {
            if !self.sel_sk.items.contains(&(0, id)) {
                self.sel_sk.items.push((0, id));
            }
        }
        self.status = crate::i18n::tr1("g-selected-n", "n", &self.sel_sk.items.len().to_string());
    }

    fn fit(&mut self, rect: Rect) {
        let mut b = bounds(&self.project.contours);
        for body in &self.project.bodies {
            if let Some(mb) = body.mesh.bounds() {
                let lo = Point2::new(mb.min.x, mb.min.y);
                let hi = Point2::new(mb.max.x, mb.max.y);
                b = Some(match b {
                    None => (lo, hi),
                    Some((bl, bh)) => (
                        Point2::new(bl.x.min(lo.x), bl.y.min(lo.y)),
                        Point2::new(bh.x.max(hi.x), bh.y.max(hi.y)),
                    ),
                });
            }
        }
        let Some(b) = b else { return };
        let w = (b.1.x - b.0.x).max(1.0) as f32;
        let h = (b.1.y - b.0.y).max(1.0) as f32;
        self.view.scale = (rect.width() / w).min(rect.height() / h) * 0.85;
        self.view.center = Vec2::new(((b.0.x + b.1.x) / 2.0) as f32, ((b.0.y + b.1.y) / 2.0) as f32);
        self.view.initialized = true;
    }

    /// A test facade for the screen mapping (the harness tests click exactly where the mouse would).
    #[cfg(test)]
    pub(crate) fn to_screen_pub(&self, rect: Rect, p: Point2) -> Pos2 {
        self.to_screen(rect, p)
    }

    /// A test facade for deleting the sketch selection.
    #[cfg(test)]
    pub(crate) fn delete_sketch_sel_pub(&mut self, si: usize) {
        self.delete_sketch_sel(si);
    }

    fn to_screen(&self, rect: Rect, p: Point2) -> Pos2 {
        let c = rect.center();
        Pos2::new(
            c.x + (p.x as f32 - self.view.center.x) * self.view.scale,
            c.y - (p.y as f32 - self.view.center.y) * self.view.scale,
        )
    }

    fn to_world(&self, rect: Rect, s: Pos2) -> Point2 {
        let c = rect.center();
        Point2::new(
            (self.view.center.x + (s.x - c.x) / self.view.scale) as f64,
            (self.view.center.y - (s.y - c.y) / self.view.scale) as f64,
        )
    }

    /// The contour under the cursor (for the hover highlight), within about 8 px.
    fn hovered_contour(&self) -> Option<usize> {
        let cur = self.cursor?;
        let thresh = 8.0 / self.view.scale as f64;
        let mut best: Option<(usize, f64)> = None;
        for (i, c) in self.project.contours.iter().enumerate() {
            let d = dist_to_contour(c, cur);
            if best.map_or(true, |(_, bd)| d < bd) {
                best = Some((i, d));
            }
        }
        best.filter(|(_, d)| *d <= thresh).map(|(i, _)| i)
    }

    /// The LINES of the profile sketch that are candidates for the axis of revolution (a centreline). They
    /// are chosen by a button or a combo box on the bar, NOT by a click in 3D: sketch geometry is not
    /// hit-tested in the command's 3D view.
    fn profile_axis_lines(&self, si: usize) -> Vec<Id> {
        // ANY straight line of the sketch can be the axis, not only a construction one — the usual CAD
        // behaviour. The core never minded; the restriction sat right here, in the list of candidates, and a
        // plain line drawn to serve as an axis simply did not appear on the command's bar. Reported
        // behaviour: only X or Y could be chosen, an axis of one's own could not.
        // Construction lines come FIRST: they are drawn precisely to serve as an axis, and that is the most
        // frequent choice.
        let Some(s) = self.project.sketches.get(si) else { return Vec::new() };
        let lines = |constr: bool| {
            s.entities
                .iter()
                .filter(move |e| e.construction == constr && matches!(e.kind, qymcad_core::model::EntityKind::Line { .. }))
                .map(|e| e.id)
        };
        lines(true).chain(lines(false)).collect()
    }

    /// Restore a thread operation's axis and radius from a circular edge while the feature is being EDITED
    /// (a double click). The axis is oriented into the body, because the kernel gives a rim an arbitrary normal.
    fn restore_thread_axis(&mut self, src: Id, edge: u32) {
        if let Some((c, ax0, r)) = self.project.regen_edges.get(&src).and_then(|es| es.iter().find(|e| e.id == edge && e.is_circular())).map(|e| (e.center, e.axis, e.radius)) {
            let mut ax = ax0;
            if let Some(bb) = self.project.mesh_index(src).and_then(|mi| self.project.bodies[mi].mesh.bounds()) {
                let cc = [(bb.min.x + bb.max.x) * 0.5, (bb.min.y + bb.max.y) * 0.5, (bb.min.z + bb.max.z) * 0.5];
                let tob = [cc[0] - c[0], cc[1] - c[1], cc[2] - c[2]];
                if tob[0] * ax[0] + tob[1] * ax[1] + tob[2] * ax[2] < 0.0 {
                    ax = [-ax[0], -ax[1], -ax[2]];
                }
            }
            self.thread.axis = (c, ax);
            self.thread.radius = r;
        }
    }

    /// The thread standard for the index of the command bar's switch.
    fn thread_standard(idx: u8) -> qymcad_core::thread::ThreadStandard {
        use qymcad_core::thread::ThreadStandard as S;
        match idx {
            1 => S::TrapezoidalTr,
            2 => S::Acme,
            3 => S::RoundRd,
            4 => S::Buttress,
            5 => S::Custom,
            _ => S::MetricIso,
        }
    }

    /// The switch index for a standard (the reverse mapping, used while editing a feature).
    fn thread_standard_idx(s: qymcad_core::thread::ThreadStandard) -> u8 {
        use qymcad_core::thread::ThreadStandard as S;
        match s {
            S::TrapezoidalTr => 1,
            S::Acme => 2,
            S::RoundRd => 3,
            S::Buttress => 4,
            S::Custom => 5,
            S::MetricIso => 0,
        }
    }

    /// The label of a candidate axis line for the command bar — construction lines are marked as such.
    fn axis_line_label(&self, si: usize, line: Id, n: usize) -> String {
        let constr = self
            .project
            .sketches
            .get(si)
            .and_then(|s| s.entities.iter().find(|e| e.id == line))
            .map(|e| e.construction)
            .unwrap_or(false);
        if constr {
            crate::i18n::tr1("sk-axis-line-n", "n", &n.to_string())
        } else {
            crate::i18n::tr1("sk-line-n", "n", &n.to_string())
        }
    }

    /// How many moves to show, according to the progress slider.
    fn progress_limit(&self) -> usize {
        let total: usize = self.cam_job.program.as_ref().map(|p| p.toolpaths.iter().map(|t| t.moves.len()).sum()).unwrap_or(0);
        (self.cam_job.progress.clamp(0.0, 1.0) * total as f32).ceil() as usize
    }

}

// ---------- helpers ----------

fn v_sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn v_cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn v_dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn v_norm(a: [f64; 3]) -> [f64; 3] {
    let l = v_dot(a, a).sqrt().max(1e-9);
    [a[0] / l, a[1] / l, a[2] / l]
}
/// WHETHER TWO PLACEMENTS ARE ROTATED THE SAME WAY (the translation may be anything).
///
/// The scene buffer needs this: if a part has only moved, the vertices already built need the difference
/// added to them; if it was also rotated, the world normals became different, and both the colour and the
/// back-face culling are computed from them, so the block is assembled again. The translation columns (3,
/// 7, 11) say nothing about rotation, so they are skipped.
///
/// THE THRESHOLD COMES FROM MEASUREMENT, NOT FROM TASTE. Exact equality never happens here: the solver
/// derives the placements afresh every frame, and even on a PURE slider travel the rotating part still
/// breathes. Measured on a real assembly: 13.8 mm of translation per frame with a rotation discrepancy of
/// 1e-12 to 9e-10. With a threshold of 1e-12 the fast path was taken NOT ONCE (63 blocks out of 63 were
/// assembled again), that is, the fix was written and did not work.
///
/// 1e-7 is 6e-6 of a degree. On a part with a radius of a metre, a vertex moves 1e-4 mm from an
/// unrecognised turn of that size, which neither the eye nor an export will see. A real rotation (driving
/// a loop) goes in hundredths of a radian — five orders of magnitude away from the noise, and it honestly
/// rebuilds the block.
fn same_rotation12(a: &[f64; 12], b: &[f64; 12]) -> bool {
    (0..12).filter(|k| !matches!(k, 3 | 7 | 11)).all(|k| (a[k] - b[k]).abs() < 1e-7)
}

/// A 3x4 row-major transform that is approximately the identity (no rotation and no translation).
fn is_identity12(m: &[f64; 12]) -> bool {
    const ID: [f64; 12] = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0];
    m.iter().zip(ID.iter()).all(|(a, b)| (a - b).abs() < 1e-9)
}

/// The intersection of a ray (o + t*d) with a plane (point p0, normal n). None means the ray is nearly
/// parallel to the plane.
fn ray_plane(o: [f64; 3], d: [f64; 3], p0: [f64; 3], n: [f64; 3]) -> Option<[f64; 3]> {
    let dn = v_dot(d, n);
    if dn.abs() < 1e-9 {
        return None;
    }
    let t = v_dot(v_sub(p0, o), n) / dn;
    Some([o[0] + d[0] * t, o[1] + d[1] * t, o[2] + d[2] * t])
}

/// The distance from a point to a segment (in screen coordinates).
fn screen_dist_seg(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let (vx, vy) = (b.x - a.x, b.y - a.y);
    let (wx, wy) = (p.x - a.x, p.y - a.y);
    let len2 = vx * vx + vy * vy;
    let t = if len2 > 1e-6 { ((wx * vx + wy * vy) / len2).clamp(0.0, 1.0) } else { 0.0 };
    let (cx, cy) = (a.x + t * vx, a.y + t * vy);
    ((p.x - cx).powi(2) + (p.y - cy).powi(2)).sqrt()
}

/// The projection of point `p` onto the SEGMENT a -> b (clamped to its ends). None means a degenerate segment.
fn project_on_seg(p: Point2, a: Point2, b: Point2) -> Option<Point2> {
    let (vx, vy) = (b.x - a.x, b.y - a.y);
    let len2 = vx * vx + vy * vy;
    if len2 < 1e-12 {
        return None;
    }
    let t = (((p.x - a.x) * vx + (p.y - a.y) * vy) / len2).clamp(0.0, 1.0);
    Some(Point2::new(a.x + t * vx, a.y + t * vy))
}

/// The point on the segment a -> b nearest to `p` that also lies on a GRID LINE (x = n*g or y = n*g) — the
/// intersection of an edge with the grid. For a vertical edge it ties Y to the grid (the edge holds X), for
/// a horizontal one it ties X, and for a slanted one it takes the nearest crossing with the grid. None when
/// there is no crossing within the segment.
fn grid_cross_on_seg(p: Point2, a: Point2, b: Point2, g: f64) -> Option<Point2> {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    if dx * dx + dy * dy < 1e-12 || g <= 0.0 {
        return None;
    }
    let mut best: Option<(f64, Point2)> = None;
    let mut consider = |q: Point2| {
        let dd = (q.x - p.x).powi(2) + (q.y - p.y).powi(2);
        if best.map_or(true, |(bd, _)| dd < bd) {
            best = Some((dd, q));
        }
    };
    if dx.abs() > 1e-9 {
        let gx = (p.x / g).round() * g; // the nearest vertical grid line
        let t = (gx - a.x) / dx;
        if (-1e-6..=1.0 + 1e-6).contains(&t) {
            consider(Point2::new(gx, a.y + dy * t));
        }
    }
    if dy.abs() > 1e-9 {
        let gy = (p.y / g).round() * g; // the nearest horizontal grid line
        let t = (gy - a.y) / dy;
        if (-1e-6..=1.0 + 1e-6).contains(&t) {
            consider(Point2::new(a.x + dx * t, gy));
        }
    }
    best.map(|(_, q)| q)
}

/// The intersection of two SEGMENTS a1 -> a2 and b1 -> b2, within both. None means there is none, or they are parallel.
fn seg_seg_intersect(a1: Point2, a2: Point2, b1: Point2, b2: Point2) -> Option<Point2> {
    let (rx, ry) = (a2.x - a1.x, a2.y - a1.y);
    let (sx, sy) = (b2.x - b1.x, b2.y - b1.y);
    let denom = rx * sy - ry * sx;
    if denom.abs() < 1e-12 {
        return None;
    }
    let (qpx, qpy) = (b1.x - a1.x, b1.y - a1.y);
    let t = (qpx * sy - qpy * sx) / denom;
    let u = (qpx * ry - qpy * rx) / denom;
    if (-1e-9..=1.0 + 1e-9).contains(&t) && (-1e-9..=1.0 + 1e-9).contains(&u) {
        Some(Point2::new(a1.x + t * rx, a1.y + t * ry))
    } else {
        None
    }
}

/// The intersection of two INFINITE lines (screen points a -> b and c -> d). None means they are parallel.
/// The arc of an angular dimension around `center`, from direction `u` to direction `v` the SHORT way (the
/// side the dimension is on, through the bisector), of radius `r` in screen pixels, with small arrowheads at
/// its ends. It shows which angle exactly the degree label refers to.
fn draw_dim_arc(painter: &egui::Painter, center: Pos2, u: egui::Vec2, v: egui::Vec2, r: f32, col: Color32) {
    let pi = std::f32::consts::PI;
    let a0 = u.y.atan2(u.x);
    let mut sweep = v.y.atan2(v.x) - a0;
    while sweep > pi {
        sweep -= 2.0 * pi;
    }
    while sweep < -pi {
        sweep += 2.0 * pi;
    }
    let n = 24;
    let at = |a: f32| center + egui::vec2(a.cos(), a.sin()) * r;
    let pts: Vec<Pos2> = (0..=n).map(|i| at(a0 + sweep * (i as f32 / n as f32))).collect();
    painter.add(egui::Shape::line(pts, Stroke::new(1.1, col)));
    // arrowheads at the ends of the arc, along the tangent, so that it reads as a dimension
    let head = |a: f32, dir: f32| {
        let p = at(a);
        let tan = egui::vec2(-a.sin(), a.cos()) * dir; // the tangent to the arc
        let back = (center - p).normalized();
        for s in [-1.0, 1.0] {
            painter.line_segment([p, p + (tan + back * s) * 5.0], Stroke::new(1.0, col));
        }
    };
    if sweep.abs() > 0.05 {
        head(a0, sweep.signum());
        head(a0 + sweep, -sweep.signum());
    }
}

/// The intersection of INFINITE lines (through the segments ab and cd) in the sketch's world coordinates.
/// None means they are parallel. Used to orient an angular dimension's directions outwards from the
/// intersection, so the angle shown is the visible opening rather than its supplement.
fn line_line_ix(a: Point2, b: Point2, c: Point2, d: Point2) -> Option<Point2> {
    let (rx, ry) = (b.x - a.x, b.y - a.y);
    let (sx, sy) = (d.x - c.x, d.y - c.y);
    let denom = rx * sy - ry * sx;
    if denom.abs() < 1e-9 {
        return None;
    }
    let t = ((c.x - a.x) * sy - (c.y - a.y) * sx) / denom;
    Some(Point2::new(a.x + t * rx, a.y + t * ry))
}

fn lines_intersect(a: Pos2, b: Pos2, c: Pos2, d: Pos2) -> Option<Pos2> {
    let (rx, ry) = (b.x - a.x, b.y - a.y);
    let (sx, sy) = (d.x - c.x, d.y - c.y);
    let denom = rx * sy - ry * sx;
    if denom.abs() < 1e-6 {
        return None;
    }
    let t = ((c.x - a.x) * sy - (c.y - a.y) * sx) / denom;
    Some(Pos2::new(a.x + t * rx, a.y + t * ry))
}

/// The points where the SEGMENT a -> b crosses a circle (centre c, radius r), within the segment.
fn seg_circle_intersect(a: Point2, b: Point2, c: Point2, r: f64) -> Vec<Point2> {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let (fx, fy) = (a.x - c.x, a.y - c.y);
    let aa = dx * dx + dy * dy;
    let bb = 2.0 * (fx * dx + fy * dy);
    let cc = fx * fx + fy * fy - r * r;
    let disc = bb * bb - 4.0 * aa * cc;
    if disc < 0.0 || aa < 1e-12 {
        return Vec::new();
    }
    let sq = disc.sqrt();
    let mut out = Vec::new();
    for t in [(-bb - sq) / (2.0 * aa), (-bb + sq) / (2.0 * aa)] {
        if (-1e-9..=1.0 + 1e-9).contains(&t) {
            out.push(Point2::new(a.x + t * dx, a.y + t * dy));
        }
    }
    out
}

/// A constraint's label for the list.
/// The circle through three points, giving (centre x, centre y, radius). None means they are collinear.
fn circumcircle(a: Point2, b: Point2, c: Point2) -> Option<(f64, f64, f64)> {
    let d = 2.0 * (a.x * (b.y - c.y) + b.x * (c.y - a.y) + c.x * (a.y - b.y));
    if d.abs() < 1e-9 {
        return None;
    }
    let (a2, b2, c2) = (a.x * a.x + a.y * a.y, b.x * b.x + b.y * b.y, c.x * c.x + c.y * c.y);
    let ux = (a2 * (b.y - c.y) + b2 * (c.y - a.y) + c2 * (a.y - b.y)) / d;
    let uy = (a2 * (c.x - b.x) + b2 * (a.x - c.x) + c2 * (b.x - a.x)) / d;
    let r = ((ux - a.x).powi(2) + (uy - a.y).powi(2)).sqrt();
    Some((ux, uy, r))
}

/// The arc tangent to the unit direction `t` at the point `s` and passing through `e`.
/// Returns (cx, cy, r, ccw). None when the end lies on the tangent line, which would make the radius infinite.
fn tangent_arc(s: Point2, t: (f64, f64), e: Point2) -> Option<(f64, f64, f64, bool)> {
    let (tx, ty) = t;
    let (nx, ny) = (-ty, tx); // the normal to the tangent: the centre lies on it
    let (sex, sey) = (s.x - e.x, s.y - e.y);
    let denom = 2.0 * (nx * sex + ny * sey);
    if denom.abs() < 1e-9 {
        return None;
    }
    let d = -(sex * sex + sey * sey) / denom; // the signed offset of the centre along the normal
    let (cx, cy) = (s.x + nx * d, s.y + ny * d);
    let r = d.abs();
    if r < 1e-9 {
        return None;
    }
    let (rx, ry) = ((s.x - cx) / r, (s.y - cy) / r); // the radial direction at s
    // the counter-clockwise tangent at s is the radial turned by +90 deg: (-ry, rx)
    let ccw = tx * (-ry) + ty * rx > 0.0;
    Some((cx, cy, r, ccw))
}

/// Font-independent glyphs for constraints, dimensions and tools (drawn as lines).
#[derive(Clone, Copy, PartialEq)]
enum Gly {
    Coincident,
    Horiz,
    Vert,
    Parallel,
    Perp,
    Equal,
    Collinear,
    Concentric,
    Fix,
    Construction,
    DimLin,
    DimAng,
    DimRad,
    Mirror,
    ArrayLin,
    ArrayCirc,
    Fillet,
    Chamfer,
    Offset,
    Tangent,
    Symmetric,
    Midpoint,
    PointOnLine,
    Trim,
    Extend,
    Break,
    Ellipse,
    Spline,
    Circle3,
    PointOnCircle,
    Text,
}

/// The two unit world axes that define the plane of a gizmo's rotation ring for axis `ax`.
fn ring_axes(ax: u8) -> ([f64; 3], [f64; 3]) {
    match ax {
        0 => ([0.0, 1.0, 0.0], [0.0, 0.0, 1.0]), // the ring about X lies in the YZ plane
        1 => ([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]), // about Y, in XZ
        _ => ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]), // about Z, in XY
    }
}

/// An orthonormal pair (u, v) spanning the plane perpendicular to an arbitrary axis `n` — for the ring of a
/// degree-of-freedom gizmo about a joint's axis, which, unlike `ring_axes`, does not line up with the world
/// X, Y and Z.
fn perp_basis(n: [f64; 3]) -> ([f64; 3], [f64; 3]) {
    let ln = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-9);
    let a = [n[0] / ln, n[1] / ln, n[2] / ln];
    // take the world axis least collinear with `a` as the seed
    let seed = if a[0].abs() <= a[1].abs() && a[0].abs() <= a[2].abs() {
        [1.0, 0.0, 0.0]
    } else if a[1].abs() <= a[2].abs() {
        [0.0, 1.0, 0.0]
    } else {
        [0.0, 0.0, 1.0]
    };
    let mut u = [a[1] * seed[2] - a[2] * seed[1], a[2] * seed[0] - a[0] * seed[2], a[0] * seed[1] - a[1] * seed[0]];
    let lu = (u[0] * u[0] + u[1] * u[1] + u[2] * u[2]).sqrt().max(1e-9);
    u = [u[0] / lu, u[1] / lu, u[2] / lu];
    let v = [a[1] * u[2] - a[2] * u[1], a[2] * u[0] - a[0] * u[2], a[0] * u[1] - a[1] * u[0]];
    (u, v)
}

/// The gizmo mode of the component selected in an assembly.
enum CompGizmoMode {
    None,      // grounded, or fully pinned by a joint: there is no gizmo
    Free,      // free of joints: the plain 6-DOF gizmo
    Joint(Id), // driven by a joint: the degree-of-freedom gizmo, offering only that joint's freedoms
}

/// The active drag of a joint gizmo's handle. The frame (`o` and `dir`) is fixed when the drag starts.
#[derive(Clone, Copy)]
pub(crate) struct JointGizDrag {
    jid: Id,
    slot: u8,   // 0 = angle, 1 = offset, 2 = offset2
    ring: bool, // true means a rotation about `dir`, false a translation along it
    start: f64, // the parameter's value when the drag started
    amt: f64,   // accumulated so far (degrees or millimetres)
    o: [f64; 3],
    dir: [f64; 3],
}

/// A VECTOR icon of a joint's kind inside its 3D badge — like the sketcher's constraint glyphs
/// (`paint_gly`), with no dependence on a font (phosphor inside `painter.text` came out as "tofu"). Drawn
/// over the badge's circle.
fn paint_joint_glyph(p: &egui::Painter, c: Pos2, r: f32, k: qymcad_core::feature::JointKind, col: Color32) {
    use egui::vec2 as v;
    use qymcad_core::feature::JointKind as J;
    let st = Stroke::new(1.8, col);
    let s = r * 0.62; // the symbol's half-size, kept large enough to read
    let head = |tip: Pos2, dir: egui::Vec2| {
        let d = dir.normalized();
        let n = v(-d.y, d.x);
        p.line_segment([tip, tip - d * (s * 0.55) + n * (s * 0.4)], st);
        p.line_segment([tip, tip - d * (s * 0.55) - n * (s * 0.4)], st);
    };
    match k {
        J::Rigid => {
            // a diamond for a fixed joint: recognisable and unlike a blank box
            let pts = vec![c + v(0.0, -s), c + v(s, 0.0), c + v(0.0, s), c + v(-s, 0.0)];
            p.add(egui::Shape::convex_polygon(pts, col, Stroke::NONE));
        }
        J::Revolute => {
            // an arc of about 290 deg with an arrowhead: a rotation
            let pts: Vec<Pos2> = (0..=24)
                .map(|i| {
                    let a = std::f64::consts::PI * (0.2 + 1.6 * i as f64 / 24.0);
                    c + v(a.cos() as f32 * s, a.sin() as f32 * s)
                })
                .collect();
            let n = pts.len();
            p.add(egui::Shape::line(pts.clone(), st));
            head(pts[n - 1], pts[n - 1] - pts[n - 2]);
        }
        J::Slider => {
            // a double-headed arrow: one translation
            let (l, rt) = (c + v(-s, 0.0), c + v(s, 0.0));
            p.line_segment([l, rt], st);
            head(l, l - rt);
            head(rt, rt - l);
        }
        J::Cylindrical => {
            // a circle plus an axis: one rotation and one translation
            p.circle_stroke(c, s * 0.7, st);
            p.line_segment([c + v(-s, 0.0), c + v(s, 0.0)], st);
        }
        J::Planar => {
            // a parallelogram for a plane: slanted, so it is not a square
            let pts = vec![c + v(-s, s * 0.55), c + v(s * 0.45, s * 0.55), c + v(s, -s * 0.55), c + v(-s * 0.45, -s * 0.55)];
            p.add(egui::Shape::convex_polygon(pts, Color32::TRANSPARENT, st));
        }
        J::Ball => {
            p.circle_filled(c, s * 0.85, col); // a ball: three rotations
        }
        J::PinSlot => {
            p.rect_stroke(egui::Rect::from_center_size(c, v(s * 2.0, s * 1.05)), s * 0.5, st); // the slot
            p.circle_filled(c + v(-s * 0.45, 0.0), s * 0.32, col); // the pin
        }
        J::Parallel => {
            // two parallel strokes: a condition rather than a fit
            p.line_segment([c + v(-s, -s * 0.45), c + v(s, -s * 0.45)], st);
            p.line_segment([c + v(-s, s * 0.45), c + v(s, s * 0.45)], st);
        }
    }
}

fn paint_gly(p: &egui::Painter, c: Pos2, h: f32, g: Gly, col: Color32) {
    use egui::vec2 as v;
    let st = Stroke::new(1.4, col);
    let thin = Stroke::new(1.0, col);
    match g {
        Gly::Coincident => {
            p.circle_stroke(c - v(2.0, 0.0), h * 0.6, st);
            p.circle_stroke(c + v(2.0, 0.0), h * 0.6, st);
        }
        Gly::Horiz => {
            let (l, r) = (c + v(-h, 0.0), c + v(h, 0.0));
            p.line_segment([l, r], st);
            p.line_segment([l + v(0.0, -3.0), l + v(0.0, 3.0)], thin);
            p.line_segment([r + v(0.0, -3.0), r + v(0.0, 3.0)], thin);
        }
        Gly::Vert => {
            let (t, b) = (c + v(0.0, -h), c + v(0.0, h));
            p.line_segment([t, b], st);
            p.line_segment([t + v(-3.0, 0.0), t + v(3.0, 0.0)], thin);
            p.line_segment([b + v(-3.0, 0.0), b + v(3.0, 0.0)], thin);
        }
        Gly::Parallel => {
            p.line_segment([c + v(-3.0, -h), c + v(-1.0, h)], st);
            p.line_segment([c + v(3.0, -h), c + v(5.0, h)], st);
        }
        Gly::Perp => {
            p.line_segment([c + v(0.0, -h), c + v(0.0, h)], st);
            p.line_segment([c + v(-h, h), c + v(h, h)], st);
        }
        Gly::Equal => {
            p.line_segment([c + v(-h, -2.5), c + v(h, -2.5)], st);
            p.line_segment([c + v(-h, 2.5), c + v(h, 2.5)], st);
        }
        Gly::Collinear => {
            p.line_segment([c + v(-h, 0.0), c + v(h, 0.0)], st);
            p.circle_filled(c, 1.6, col);
        }
        Gly::Concentric => {
            p.circle_stroke(c, h, st);
            p.circle_stroke(c, h * 0.5, st);
        }
        Gly::Midpoint => {
            // a line with a tick at its middle
            p.line_segment([c + v(-h, 0.0), c + v(h, 0.0)], st);
            p.line_segment([c + v(0.0, -h * 0.6), c + v(0.0, h * 0.6)], st);
        }
        Gly::PointOnLine => {
            // a line with a point on it
            p.line_segment([c + v(-h, 0.0), c + v(h, 0.0)], st);
            p.circle_filled(c, 2.0, col);
        }
        Gly::PointOnCircle => {
            // an arc of a circle with a point on it (a glyph of its own, not an alias of PointOnLine)
            p.circle_stroke(c + v(0.0, h * 0.7), h, thin);
            p.circle_filled(c + v(0.0, h * 0.7 - h), 2.0, col);
        }
        Gly::Fix => {
            let r = Rect::from_center_size(c, v(h * 1.5, h * 1.5));
            p.rect_stroke(r, 1.0, st);
            p.circle_filled(c, 1.6, col);
        }
        Gly::Construction => {
            p.add(egui::Shape::dashed_line(&[c + v(-h, 0.0), c + v(h, 0.0)], st, 3.0, 2.0));
        }
        Gly::DimLin => {
            let (l, r) = (c + v(-h, 0.0), c + v(h, 0.0));
            p.line_segment([l, r], st);
            p.line_segment([l, l + v(3.0, -2.0)], thin);
            p.line_segment([l, l + v(3.0, 2.0)], thin);
            p.line_segment([r, r + v(-3.0, -2.0)], thin);
            p.line_segment([r, r + v(-3.0, 2.0)], thin);
        }
        Gly::DimAng => {
            let o = c + v(-h * 0.6, h * 0.7);
            p.line_segment([o, o + v(h * 1.6, 0.0)], st);
            p.line_segment([o, o + v(h * 1.1, -h * 1.5)], st);
            p.circle_stroke(o, h * 0.7, thin);
        }
        Gly::DimRad => {
            p.circle_stroke(c, h * 0.85, st);
            let e = c + v(h * 0.85, 0.0);
            p.line_segment([c, e], thin);
            p.line_segment([e, e + v(-3.0, -2.0)], thin);
            p.line_segment([e, e + v(-3.0, 2.0)], thin);
        }
        Gly::Mirror => {
            p.add(egui::Shape::dashed_line(&[c + v(0.0, -h), c + v(0.0, h)], thin, 2.0, 2.0));
            for s in [-1.0_f32, 1.0] {
                let x = s * 2.0;
                let tip = s * (h - 1.0);
                p.line_segment([c + v(x, -3.0), c + v(tip, 0.0)], st);
                p.line_segment([c + v(tip, 0.0), c + v(x, 3.0)], st);
                p.line_segment([c + v(x, -3.0), c + v(x, 3.0)], st);
            }
        }
        Gly::ArrayLin => {
            for k in 0..3 {
                let x = -h + k as f32 * h;
                p.rect_stroke(Rect::from_center_size(c + v(x, 0.0), v(4.0, 4.0)), 0.0, st);
            }
        }
        Gly::ArrayCirc => {
            for k in 0..6 {
                let a = std::f32::consts::TAU * k as f32 / 6.0;
                p.circle_filled(c + v(h * 0.85 * a.cos(), h * 0.85 * a.sin()), 1.6, col);
            }
        }
        Gly::Fillet => {
            let cen = c + v(h * 0.5, h * 0.5);
            let pts: Vec<Pos2> = (0..=8).map(|i| { let t = std::f32::consts::PI + std::f32::consts::FRAC_PI_2 * (i as f32 / 8.0); cen + v(h * t.cos(), h * t.sin()) }).collect();
            p.add(egui::Shape::line(pts, st));
            p.line_segment([cen + v(-h, 0.0), cen + v(-h, h * 0.7)], st);
            p.line_segment([cen + v(0.0, -h), cen + v(-h * 0.7, -h)], st);
        }
        Gly::Chamfer => {
            let tl = c + v(-h, -h);
            p.line_segment([tl + v(0.0, h * 0.5), tl + v(0.0, h * 1.7)], st);
            p.line_segment([tl + v(h * 0.5, 0.0), tl + v(h * 1.7, 0.0)], st);
            p.line_segment([tl + v(0.0, h * 0.5), tl + v(h * 0.5, 0.0)], st);
        }
        Gly::Offset => {
            p.line_segment([c + v(-h, -h * 0.5), c + v(h, -h * 0.5)], st);
            p.add(egui::Shape::dashed_line(&[c + v(-h, h * 0.5), c + v(h, h * 0.5)], thin, 2.0, 2.0));
        }
        Gly::Tangent => {
            p.circle_stroke(c + v(0.0, 2.0), h * 0.7, st);
            p.line_segment([c + v(-h, -h * 0.6), c + v(h, -h * 0.6)], st); // the tangent
        }
        Gly::Symmetric => {
            p.add(egui::Shape::dashed_line(&[c + v(0.0, -h), c + v(0.0, h)], thin, 2.0, 2.0));
            p.circle_filled(c + v(-h * 0.7, 0.0), 1.8, col);
            p.circle_filled(c + v(h * 0.7, 0.0), 1.8, col);
            p.line_segment([c + v(-h * 0.7, 0.0), c + v(-2.0, 0.0)], thin);
            p.line_segment([c + v(2.0, 0.0), c + v(h * 0.7, 0.0)], thin);
        }
        Gly::Trim => {
            // scissors: a line with a piece cut out of it (dashed in the middle)
            p.line_segment([c + v(-h, 0.0), c + v(-h * 0.35, 0.0)], st);
            p.line_segment([c + v(h * 0.35, 0.0), c + v(h, 0.0)], st);
            p.add(egui::Shape::dashed_line(&[c + v(-h * 0.35, 0.0), c + v(h * 0.35, 0.0)], thin, 1.5, 1.5));
            p.line_segment([c + v(-3.0, -h), c + v(3.0, h)], st);
        }
        Gly::Extend => {
            // a line plus a dashed continuation with an arrow up to the barrier
            p.line_segment([c + v(-h, 0.0), c + v(2.0, 0.0)], st);
            p.add(egui::Shape::dashed_line(&[c + v(2.0, 0.0), c + v(h, 0.0)], thin, 1.5, 1.5));
            p.line_segment([c + v(h, -h), c + v(h, h)], st); // the barrier
            p.line_segment([c + v(h, 0.0), c + v(h - 3.0, -2.0)], thin);
            p.line_segment([c + v(h, 0.0), c + v(h - 3.0, 2.0)], thin);
        }
        Gly::Break => {
            // two lines with a break between them
            p.line_segment([c + v(-h, 0.0), c + v(-2.0, 0.0)], st);
            p.line_segment([c + v(2.0, 0.0), c + v(h, 0.0)], st);
            p.line_segment([c + v(0.0, -h), c + v(0.0, h)], thin);
        }
        Gly::Ellipse => {
            let n = 24;
            let pts: Vec<Pos2> = (0..=n).map(|i| { let a = std::f32::consts::TAU * i as f32 / n as f32; c + v(h * a.cos(), h * 0.6 * a.sin()) }).collect();
            p.add(egui::Shape::line(pts, st));
        }
        Gly::Spline => {
            let pts: Vec<Pos2> = (0..=16).map(|i| { let t = i as f32 / 16.0; let x = -h + 2.0 * h * t; let y = (t * std::f32::consts::TAU).sin() * h * 0.5; c + v(x, y) }).collect();
            p.add(egui::Shape::line(pts, st));
        }
        Gly::Circle3 => {
            p.circle_stroke(c, h, st);
            for k in 0..3 {
                let a = std::f32::consts::FRAC_PI_2 + std::f32::consts::TAU * k as f32 / 3.0;
                p.circle_filled(c + v(h * a.cos(), h * a.sin()), 1.6, col);
            }
        }
        Gly::Text => {
            p.line_segment([c + v(-h, -h), c + v(h, -h)], st); // the top bar
            p.line_segment([c + v(0.0, -h), c + v(0.0, h)], st); // the stem
        }
    }
}

/// A tool button with a drawn glyph (instead of raw unicode that renders as tofu).
fn sym_button(ui: &mut egui::Ui, g: Gly, tip: &str, active: bool) -> bool {
    // The size is THE SAME as `icon_tool`'s (40x34), otherwise a mismatch of widths breaks the wrap onto two columns.
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(40.0, 34.0), egui::Sense::click());
    let vis = ui.style().interact_selectable(&resp, active);
    ui.painter().rect(rect, 3.0, vis.bg_fill, vis.bg_stroke);
    paint_gly(ui.painter(), rect.center(), 9.0, g, vis.fg_stroke.color);
    resp.on_hover_text(tip).clicked()
}

fn constraint_label(c: &qymcad_core::model::Constraint) -> String {
    use qymcad_core::model::Constraint as C;
    // A DIMENSION'S VALUE: either a number, or "expression = number". The expression is typed by hand and is kept as it is.
    let value = |num: String, expr: &str| if expr.trim().is_empty() { num } else { format!("{expr} = {num}") };
    // "Dimension = 5.0" or "Dimension (5.0), driven" — one pair of messages for every kind of dimension.
    let dim = |key: &str, val: String, driven: bool| {
        if driven {
            crate::i18n::tr2("con-dim-driven", "what", &crate::i18n::tr(key), "value", &val)
        } else {
            crate::i18n::tr2("con-dim", "what", &crate::i18n::tr(key), "value", &val)
        }
    };
    match c {
        C::Fixed { .. } => crate::i18n::tr("con-fixed"),
        C::Horizontal { .. } => crate::i18n::tr("con-horizontal"),
        C::Vertical { .. } => crate::i18n::tr("con-vertical"),
        C::Coincident { .. } => crate::i18n::tr("con-coincident"),
        C::Distance { d, driven, expr, .. } => dim("con-name-distance", value(crate::i18n::num(*d, 1), expr), *driven),
        C::Parallel { .. } => crate::i18n::tr("con-parallel"),
        C::Perpendicular { .. } => crate::i18n::tr("con-perpendicular"),
        C::Equal { .. } => crate::i18n::tr("con-equal-length"),
        C::EqualRadius { .. } => crate::i18n::tr("con-equal-radius"),
        C::CircleTangent { external, .. } => crate::i18n::tr(if *external { "con-circle-tangent-out" } else { "con-circle-tangent-in" }),
        C::EdgeDistance { d, driven, expr, .. } => dim("con-name-edge-distance", value(crate::i18n::num(*d, 1), expr), *driven),
        C::PointOnCircle { .. } => crate::i18n::tr("con-point-on-circle"),
        C::Concentric { .. } => crate::i18n::tr("con-concentric"),
        C::Angle { deg, driven, .. } => dim("con-name-angle", format!("{}°", crate::i18n::num(*deg, 0)), *driven),
        C::AngleLines { deg, driven, expr, .. } => dim("con-name-angle", value(format!("{}°", crate::i18n::num(*deg, 0)), expr), *driven),
        C::ArcLength { len, driven, expr, .. } => dim("con-name-arc-length", value(crate::i18n::num(*len, 1), expr), *driven),
        C::Collinear { .. } => crate::i18n::tr("con-collinear"),
        C::Midpoint { .. } => crate::i18n::tr("con-midpoint"),
        C::Tangent { .. } => crate::i18n::tr("con-tangent"),
        C::Symmetric { .. } => crate::i18n::tr("con-symmetric"),
        C::PointOnLine { .. } => crate::i18n::tr("con-point-on-line"),
        // the diameter and radius signs are symbols rather than words: the same in every language
        C::Diameter { d, driven, diam, expr, .. } => {
            let val = value(crate::i18n::num(*d, 1), expr);
            let pfx = if *diam { "Ø" } else { "R" };
            if *driven { crate::i18n::tr2("con-dim-driven", "what", pfx, "value", &val) } else { crate::i18n::tr2("con-dim", "what", pfx, "value", &val) }
        }
        C::DistancePL { d, driven, expr, .. } => dim("con-name-distance-pl", value(crate::i18n::num(*d, 1), expr), *driven),
    }
}

/// The Id of the target body of a 3D operation (None for a 2.5D one).
fn mesh_of_kind(k: OpKind) -> Option<Id> {
    match k {
        OpKind::Surface3D { mesh } | OpKind::Rough3D { mesh } | OpKind::Waterline3D { mesh } | OpKind::Project3D { mesh } | OpKind::Flat3D { mesh } => Some(mesh),
        _ => None,
    }
}

/// The signed area (the edge function) used for barycentric coordinates and rasterisation.
fn edge(ax: f32, ay: f32, bx: f32, by: f32, px: f32, py: f32) -> f32 {
    (bx - ax) * (py - ay) - (by - ay) * (px - ax)
}

/// Rasterise triangles into the buffer band [y0, y0 + rows) with a per-pixel z test.
/// `color` and `zbuf` cover exactly that band (their indexing is local to it). A vertex is
/// `[px, py, depth]` in global pixel coordinates (the depth measured along fwd, smaller being nearer).
fn raster_band(color: &mut [Color32], zbuf: &mut [f32], w: usize, y0: usize, rows: usize, tris: &[([f32; 3], [f32; 3], [f32; 3], [Color32; 3])]) {
    let ymax = (y0 + rows) as i32;
    for (v0, v1, v2, cols) in tris {
        let minx = v0[0].min(v1[0]).min(v2[0]).floor().max(0.0) as i32;
        let maxx = v0[0].max(v1[0]).max(v2[0]).ceil().min(w as f32 - 1.0) as i32;
        let miny = (v0[1].min(v1[1]).min(v2[1]).floor() as i32).max(y0 as i32);
        let maxy = (v0[1].max(v1[1]).max(v2[1]).ceil() as i32).min(ymax - 1);
        if minx > maxx || miny > maxy {
            continue;
        }
        let area = edge(v0[0], v0[1], v1[0], v1[1], v2[0], v2[1]);
        if area.abs() < 1e-6 {
            continue;
        }
        let inv = 1.0 / area;
        let flat = cols[0] == cols[1] && cols[1] == cols[2]; // a flat-shaded triangle needs no interpolation
        for py in miny..=maxy {
            let row = (py as usize - y0) * w;
            for px in minx..=maxx {
                let (fx, fy) = (px as f32 + 0.5, py as f32 + 0.5);
                let w0 = edge(v1[0], v1[1], v2[0], v2[1], fx, fy);
                let w1 = edge(v2[0], v2[1], v0[0], v0[1], fx, fy);
                let w2 = edge(v0[0], v0[1], v1[0], v1[1], fx, fy);
                let inside = (w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0) || (w0 <= 0.0 && w1 <= 0.0 && w2 <= 0.0);
                if !inside {
                    continue;
                }
                let depth = (w0 * v0[2] + w1 * v1[2] + w2 * v2[2]) * inv;
                let idx = row + px as usize;
                if depth < zbuf[idx] {
                    zbuf[idx] = depth;
                    color[idx] = if flat { cols[0] } else { interp_color(cols, w0 * inv, w1 * inv, w2 * inv) };
                }
            }
        }
    }
}

/// Barycentric interpolation of the vertex colours (Gouraud). `b0` to `b2` are normalised weights summing
/// to about 1, paired with `cols[0..2]`. It works for premultiplied colour too, being linear.
#[inline]
fn interp_color(cols: &[Color32; 3], b0: f32, b1: f32, b2: f32) -> Color32 {
    let (a, b, c) = (cols[0].to_array(), cols[1].to_array(), cols[2].to_array());
    let ch = |i: usize| (b0 * a[i] as f32 + b1 * b[i] as f32 + b2 * c[i] as f32).clamp(0.0, 255.0) as u8;
    Color32::from_rgba_premultiplied(ch(0), ch(1), ch(2), ch(3))
}

/// The second pass: the translucent (ghost) triangles go OVER the opaque ones. The z test still uses the
/// shared `zbuf` (solid bodies occlude a ghost), but nothing is WRITTEN to it — translucent triangles do
/// not occlude one another, which avoids holes that depend on the drawing order. The blend is
/// premultiplied-over: `Color32` stores premultiplied colour, so out = src + dst * (1 - src_a).
/// It mirrors the GPU path (`mesh_pipeline_ghost`: depth writes off, premultiplied alpha blending).
fn raster_band_blend(color: &mut [Color32], zbuf: &[f32], w: usize, y0: usize, rows: usize, tris: &[([f32; 3], [f32; 3], [f32; 3], [Color32; 3])]) {
    let ymax = (y0 + rows) as i32;
    for (v0, v1, v2, cols) in tris {
        let minx = v0[0].min(v1[0]).min(v2[0]).floor().max(0.0) as i32;
        let maxx = v0[0].max(v1[0]).max(v2[0]).ceil().min(w as f32 - 1.0) as i32;
        let miny = (v0[1].min(v1[1]).min(v2[1]).floor() as i32).max(y0 as i32);
        let maxy = (v0[1].max(v1[1]).max(v2[1]).ceil() as i32).min(ymax - 1);
        if minx > maxx || miny > maxy {
            continue;
        }
        let area = edge(v0[0], v0[1], v1[0], v1[1], v2[0], v2[1]);
        if area.abs() < 1e-6 {
            continue;
        }
        let inv = 1.0 / area;
        let flat = cols[0] == cols[1] && cols[1] == cols[2];
        for py in miny..=maxy {
            let row = (py as usize - y0) * w;
            for px in minx..=maxx {
                let (fx, fy) = (px as f32 + 0.5, py as f32 + 0.5);
                let w0 = edge(v1[0], v1[1], v2[0], v2[1], fx, fy);
                let w1 = edge(v2[0], v2[1], v0[0], v0[1], fx, fy);
                let w2 = edge(v0[0], v0[1], v1[0], v1[1], fx, fy);
                let inside = (w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0) || (w0 <= 0.0 && w1 <= 0.0 && w2 <= 0.0);
                if !inside {
                    continue;
                }
                let depth = (w0 * v0[2] + w1 * v1[2] + w2 * v2[2]) * inv;
                let idx = row + px as usize;
                if depth >= zbuf[idx] {
                    continue; // occluded by an opaque body
                }
                let src = if flat { cols[0] } else { interp_color(cols, w0 * inv, w1 * inv, w2 * inv) }.to_array(); // premultiplied
                let ia = 255 - src[3] as u32; // 1 - src_a, on a 0..255 scale
                let dst = color[idx].to_array();
                let blend = |s: u8, d: u8| (s as u32 + (d as u32 * ia + 127) / 255).min(255) as u8;
                color[idx] = Color32::from_rgba_premultiplied(blend(src[0], dst[0]), blend(src[1], dst[1]), blend(src[2], dst[2]), blend(src[3], dst[3]));
            }
        }
    }
}

/// What the click pick of a circular pattern's axis caught: a datum AXIS, a STRAIGHT edge (an index into
/// `edge_polys`), or the axis of a CYLINDRICAL or conical face (the body plus the face's persistent id).
#[derive(Clone, Copy)]
pub(crate) enum AxisHit {
    Datum(Id),
    Edge(usize),
    Face(Id, u32),
}

/// The axis segment (origin plus or minus len * dir), for drawing and picking a datum axis.
fn axis_segment(origin: [f64; 3], dir: [f64; 3], len: f64) -> ([f64; 3], [f64; 3]) {
    let n = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt();
    let u = if n > 1e-9 { [dir[0] / n, dir[1] / n, dir[2] / n] } else { [0.0, 0.0, 1.0] };
    ([origin[0] - u[0] * len, origin[1] - u[1] * len, origin[2] - u[2] * len], [origin[0] + u[0] * len, origin[1] + u[1] * len, origin[2] + u[2] * len])
}

/// An edge polyline is nearly STRAIGHT (and so fit to be an axis): every point lies on the line from the
/// first to the last. An arc or a circle (a deviation above 1% of the length plus 0.05 mm) is rejected,
/// otherwise the chord of a circle would become an "axis".
fn is_straight_poly(poly: &[[f32; 3]]) -> bool {
    if poly.len() < 2 {
        return false;
    }
    let a = poly[0];
    let b = *poly.last().unwrap();
    let ab = [(b[0] - a[0]) as f64, (b[1] - a[1]) as f64, (b[2] - a[2]) as f64];
    let len = (ab[0] * ab[0] + ab[1] * ab[1] + ab[2] * ab[2]).sqrt();
    if len < 1e-4 {
        return false;
    }
    let u = [ab[0] / len, ab[1] / len, ab[2] / len];
    for p in poly {
        let ap = [(p[0] - a[0]) as f64, (p[1] - a[1]) as f64, (p[2] - a[2]) as f64];
        let t = ap[0] * u[0] + ap[1] * u[1] + ap[2] * u[2];
        let dev = ((ap[0] - t * u[0]).powi(2) + (ap[1] - t * u[1]).powi(2) + (ap[2] - t * u[2]).powi(2)).sqrt();
        if dev > len * 0.01 + 0.05 {
            return false;
        }
    }
    true
}

/// Rotate the point `p` about an axis (origin `o`, direction `dir`) by `ang` radians (Rodrigues) — for the
/// preview of a circular pattern. It matches the core's `rot_about_axis` (a right-handed frame).
fn rotate_pt_about_axis(o: [f64; 3], dir: [f64; 3], ang: f64, p: [f64; 3]) -> [f64; 3] {
    let n = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt();
    let k = if n > 1e-12 { [dir[0] / n, dir[1] / n, dir[2] / n] } else { [0.0, 0.0, 1.0] };
    let v = [p[0] - o[0], p[1] - o[1], p[2] - o[2]];
    let (c, s) = (ang.cos(), ang.sin());
    let kdv = k[0] * v[0] + k[1] * v[1] + k[2] * v[2];
    let kxv = [k[1] * v[2] - k[2] * v[1], k[2] * v[0] - k[0] * v[2], k[0] * v[1] - k[1] * v[0]];
    [
        v[0] * c + kxv[0] * s + k[0] * kdv * (1.0 - c) + o[0],
        v[1] * c + kxv[1] * s + k[1] * kdv * (1.0 - c) + o[1],
        v[2] * c + kxv[2] * s + k[2] * kdv * (1.0 - c) + o[2],
    ]
}

fn bounds(contours: &[Contour]) -> Option<(Point2, Point2)> {
    let mut min = Point2::new(f64::INFINITY, f64::INFINITY);
    let mut max = Point2::new(f64::NEG_INFINITY, f64::NEG_INFINITY);
    for c in contours {
        for p in &c.points {
            min.x = min.x.min(p.x);
            min.y = min.y.min(p.y);
            max.x = max.x.max(p.x);
            max.y = max.y.max(p.y);
        }
    }
    min.x.is_finite().then_some((min, max))
}

fn dist_to_contour(c: &Contour, p: Point2) -> f64 {
    let pts = &c.points;
    if pts.is_empty() {
        return f64::INFINITY;
    }
    let mut best = f64::INFINITY;
    let n = pts.len();
    let last = if c.closed { n } else { n - 1 };
    for i in 0..last {
        let a = pts[i];
        let b = pts[(i + 1) % n];
        best = best.min(dist_point_seg(p, a, b));
    }
    best
}

fn dist_point_seg(p: Point2, a: Point2, b: Point2) -> f64 {
    let ab = b.sub(a);
    let l2 = ab.x * ab.x + ab.y * ab.y;
    if l2 < 1e-12 {
        return p.dist(a);
    }
    let t = ((p.x - a.x) * ab.x + (p.y - a.y) * ab.y) / l2;
    let t = t.clamp(0.0, 1.0);
    p.dist(Point2::new(a.x + ab.x * t, a.y + ab.y * t))
}

fn drag(ui: &mut egui::Ui, label: &str, v: &mut f64, speed: f64, range: std::ops::RangeInclusive<f64>) -> bool {
    ui.label(label);
    let changed = ui.add(egui::DragValue::new(v).speed(speed).range(range)).changed();
    ui.end_row();
    changed
}

fn drag_opt(ui: &mut egui::Ui, label: &str, v: &mut f64, speed: f64, range: std::ops::RangeInclusive<f64>) -> bool {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(egui::DragValue::new(v).speed(speed).range(range)).changed()
    })
    .inner
}

fn tool_type_combo(ui: &mut egui::Ui, i: usize, kind: &mut ToolType) {
    egui::ComboBox::from_id_salt(("tt", i))
        .selected_text(crate::i18n::tr(tool_type_label(*kind)))
        .show_ui(ui, |ui| {
            for k in [ToolType::FlatEnd, ToolType::BallNose, ToolType::BullNose, ToolType::VBit, ToolType::Engraver, ToolType::Drill] {
                ui.selectable_value(kind, k, crate::i18n::tr(tool_type_label(k)));
            }
        });
}

fn tool_type_label(k: ToolType) -> &'static str {
    match k {
        ToolType::FlatEnd => "cam-tool-flat",
        ToolType::BallNose => "cam-tool-ball",
        ToolType::BullNose => "cam-tool-bull",
        ToolType::VBit => "cam-tool-vbit",
        ToolType::Engraver => "cam-tool-engraver",
        ToolType::Drill => "cam-tool-drill",
    }
}

/// The sign of a gizmo ring's rotation. `axis_depth` is the component of the rotation axis along the
/// INTO-THE-SCREEN direction (`basis.2`, where depth grows AWAY from the camera). With the axis pointing
/// TOWARDS the viewer (depth < 0), a visually counter-clockwise drag is a POSITIVE right-handed angle. The
/// sign used to be inverted — dragging to the right gave negative degrees and turned the part the other way.
fn ring_drag_sign(axis_depth: f64) -> f64 {
    if axis_depth >= 0.0 {
        -1.0
    } else {
        1.0
    }
}

/// The section's new offset while the gizmo is dragged — a DELTA from (`off0`, `p0`), the offset and the
/// screen cursor AT THE MOMENT THE GIZMO WAS GRABBED (the anchor). The offset used to be recomputed
/// ABSOLUTELY every frame, by reprojecting the CURRENT cursor relative to o0 (the world point at offset 0)
/// — but at the moment of the grab the cursor sits at the GIZMO ARROW'S TIP (`cp + n * diag * 0.35`, which
/// already includes both the current offset and the arrow's length) rather than at o0, so the very first
/// frame of the drag added the arrow's length to the offset in one jump. Reported behaviour: the plane
/// resets every time the gizmo is grabbed. A delta from the anchor gives 0 on the first frame (cur == p0)
/// and grows strictly in proportion to the mouse afterwards. `s0` and `s1` are the screen projections of o0
/// and o0 + normal (they set the direction and the pixels-per-millimetre scale along the normal); `None` is
/// the degenerate case where the normal points straight at the camera and its screen projection collapses
/// to a point.
fn section_drag_delta_offset(off0: f64, p0: Pos2, s0: Pos2, s1: Pos2, cur: Pos2) -> Option<f64> {
    let pd = s1 - s0;
    let den = (pd.x * pd.x + pd.y * pd.y) as f64;
    if den <= 1e-6 {
        return None;
    }
    let dt = ((cur.x - p0.x) * pd.x + (cur.y - p0.y) * pd.y) as f64 / den;
    Some(off0 + dt)
}

/// Clipping a triangle by a plane for the SECTION view: a pure module with no `self`, so it can be unit-tested.
mod smallvec_tris {
    /// A clipped vertex: its world position plus the barycentric weights of the original vertices (for interpolating colour).
    #[derive(Clone, Copy)]
    pub struct ClipV {
        pub pos: [f64; 3],
        pub w: [f64; 3],
    }
    /// 0 to 2 triangles (indices into `verts`, as a fan); `whole = true` means take the original triangle as it is.
    pub struct ClipTris {
        pub whole: bool,
        pub verts: Vec<ClipV>,
    }
    impl ClipTris {
        pub fn whole() -> Self {
            ClipTris { whole: true, verts: Vec::new() }
        }
        pub fn empty() -> Self {
            ClipTris { whole: false, verts: Vec::new() }
        }
    }
    /// Sutherland-Hodgman: keep the side where dist <= 0. Three inputs give a fan of 0, 3 or 4 vertices.
    pub fn clip_by_dists(v: [[f64; 3]; 3], d: [f64; 3]) -> ClipTris {
        let vis = [d[0] <= 0.0, d[1] <= 0.0, d[2] <= 0.0];
        if vis[0] && vis[1] && vis[2] {
            return ClipTris::whole();
        }
        if !vis[0] && !vis[1] && !vis[2] {
            return ClipTris::empty();
        }
        let mut out: Vec<ClipV> = Vec::with_capacity(4);
        for i in 0..3 {
            let j = (i + 1) % 3;
            if vis[i] {
                let mut w = [0.0; 3];
                w[i] = 1.0;
                out.push(ClipV { pos: v[i], w });
            }
            if vis[i] != vis[j] {
                let t = d[i] / (d[i] - d[j]); // the point EXACTLY on the plane
                let lerp = |a: f64, b: f64| a + (b - a) * t;
                let mut w = [0.0; 3];
                w[i] = 1.0 - t;
                w[j] = t;
                out.push(ClipV { pos: [lerp(v[i][0], v[j][0]), lerp(v[i][1], v[j][1]), lerp(v[i][2], v[j][2])], w });
            }
        }
        ClipTris { whole: false, verts: out }
    }
}

/// "hh:mm" without depending on chrono (for the autosave status). The UTC offset does not matter here — it is an indicator.
fn chrono_free_time() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    format!("{:02}:{:02}", (secs / 3600) % 24, (secs / 60) % 60)
}

fn point_in_tri(p: Pos2, a: Pos2, b: Pos2, c: Pos2) -> bool {
    let sign = |p: Pos2, a: Pos2, b: Pos2| (p.x - b.x) * (a.y - b.y) - (a.x - b.x) * (p.y - b.y);
    let d1 = sign(p, a, b);
    let d2 = sign(p, b, c);
    let d3 = sign(p, c, a);
    let has_neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let has_pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
    !(has_neg && has_pos)
}

/// The surface's depth AT THE CLICKED POINT `p` (a barycentric interpolation of the projected triangle's
/// vertex depths). CRITICAL for picking: the hit's depth used to be the AVERAGE of the triangle's vertex
/// depths — on a thin wall (2 mm) the large triangle of the outer face had its centroid DEEPER than the
/// small triangle of the inner one, so the wrong face won and a sketch landed on the inner wall seemingly
/// at random.
fn tri_depth_at(p: Pos2, a: Pos2, da: f64, b: Pos2, db: f64, c: Pos2, dc: f64) -> f64 {
    let (v0x, v0y) = ((b.x - a.x) as f64, (b.y - a.y) as f64);
    let (v1x, v1y) = ((c.x - a.x) as f64, (c.y - a.y) as f64);
    let (v2x, v2y) = ((p.x - a.x) as f64, (p.y - a.y) as f64);
    let den = v0x * v1y - v1x * v0y;
    if den.abs() < 1e-12 {
        return (da + db + dc) / 3.0; // degenerate (edge-on to the camera): fall back to the average
    }
    let u = (v2x * v1y - v1x * v2y) / den;
    let v = (v0x * v2y - v2x * v0y) / den;
    da * (1.0 - u - v) + db * u + dc * v
}

/// A 3x4 row-major translation matrix (for the Move feature and for a body's transform).
fn mat_translate(dx: f64, dy: f64, dz: f64) -> [f64; 12] {
    [1.0, 0.0, 0.0, dx, 0.0, 1.0, 0.0, dy, 0.0, 0.0, 1.0, dz]
}

/// The composition of two 3x4 affine transforms (row-major): the result is `a` after `b` (apply b, then a). Used by the body gizmo.
fn compose12(a: &[f64; 12], b: &[f64; 12]) -> [f64; 12] {
    let al = [a[0], a[1], a[2], a[4], a[5], a[6], a[8], a[9], a[10]];
    let bl = [b[0], b[1], b[2], b[4], b[5], b[6], b[8], b[9], b[10]];
    let (tb, ta) = ([b[3], b[7], b[11]], [a[3], a[7], a[11]]);
    let mut l = [0.0; 9];
    for r in 0..3 {
        for c in 0..3 {
            l[r * 3 + c] = al[r * 3] * bl[c] + al[r * 3 + 1] * bl[3 + c] + al[r * 3 + 2] * bl[6 + c];
        }
    }
    let t = [
        al[0] * tb[0] + al[1] * tb[1] + al[2] * tb[2] + ta[0],
        al[3] * tb[0] + al[4] * tb[1] + al[5] * tb[2] + ta[1],
        al[6] * tb[0] + al[7] * tb[1] + al[8] * tb[2] + ta[2],
    ];
    [l[0], l[1], l[2], t[0], l[3], l[4], l[5], t[1], l[6], l[7], l[8], t[2]]
}

/// A rotation by `deg` about the WORLD axis `ax` (0 = X, 1 = Y, 2 = Z) through the point `o`, as a 3x4
/// affine transform. Used by the body gizmo.
fn rot_about_point(ax: u8, deg: f64, o: [f64; 3]) -> [f64; 12] {
    let (s, c) = deg.to_radians().sin_cos();
    let r = match ax {
        0 => [1.0, 0.0, 0.0, 0.0, c, -s, 0.0, s, c],
        1 => [c, 0.0, s, 0.0, 1.0, 0.0, -s, 0.0, c],
        _ => [c, -s, 0.0, s, c, 0.0, 0.0, 0.0, 1.0],
    };
    // the translation is o - R*o, which keeps the centre still
    let ro = [r[0] * o[0] + r[1] * o[1] + r[2] * o[2], r[3] * o[0] + r[4] * o[1] + r[5] * o[2], r[6] * o[0] + r[7] * o[1] + r[8] * o[2]];
    [r[0], r[1], r[2], o[0] - ro[0], r[3], r[4], r[5], o[1] - ro[1], r[6], r[7], r[8], o[2] - ro[2]]
}

/// Whether a point lies inside a polygon (by ray casting). The contour is closed implicitly.
fn point_in_poly(p: Pos2, poly: &[Pos2]) -> bool {
    let n = poly.len();
    if n < 3 {
        return false;
    }
    let mut inside = false;
    let mut j = n - 1;
    for i in 0..n {
        let (a, b) = (poly[i], poly[j]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// The area of a polygon (the absolute value of the shoelace formula) — used to pick the inner contour.
/// WHAT A DROP AT THIS POINT OF A TREE ROW MEANS.
///
/// Two different gestures were asked for, and both are drags: reordering, and dropping onto a part to make
/// a new subassembly. They have to be told apart by WHERE the drop lands, otherwise one movement means two
/// different actions and there is no way to choose between them.
///
/// The convention is the usual one for trees: near the edges of a row it reorders (before or after), in the
/// middle it puts the item INSIDE. The middle band is made wide (half the row): it is easier to hit, and
/// grouping is both the more frequent action and the more meaningful one than an exact reordering.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum TreeDrop {
    /// Go BEFORE the target among its siblings.
    Before,
    /// Go AFTER the target among its siblings.
    After,
    /// Gather the selection and the target into a new subassembly.
    Onto,
}

pub(super) fn tree_drop_intent(rect: egui::Rect, pointer_y: f32) -> TreeDrop {
    let h = rect.height().max(1.0);
    let t = ((pointer_y - rect.top()) / h).clamp(0.0, 1.0);
    if t < 0.25 {
        TreeDrop::Before
    } else if t > 0.75 {
        TreeDrop::After
    } else {
        TreeDrop::Onto
    }
}

/// THE WORD UNDER THE CARET — that is what the driver is looked up by.
///
/// The field holds an expression (`w*2+len`), not a single name, so the suggestions follow one fragment
/// rather than the whole string: after `w*2+` the list must not empty out just as a new name is being
/// started.
///
/// The word is looked for on BOTH sides of the caret. A formula gets edited in its middle as often as at
/// its end, and searching from the end of the whole string answered the wrong question there: with the
/// caret after `le` in `10+le*2` the search ran on `2` and offered nothing at all.
///
/// `caret` is a BYTE offset. Returns (the start of the word, its end, the word itself).
pub(super) fn current_token(text: &str, caret: usize) -> (usize, usize, &str) {
    let caret = caret.min(text.len());
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    let start = text[..caret].rfind(|c: char| !is_word(c)).map(|i| i + c_len(text, i)).unwrap_or(0);
    let end = caret + text[caret..].find(|c: char| !is_word(c)).unwrap_or(text.len() - caret);
    (start, end, &text[start..end])
}

/// The length of a character in bytes, from its first byte — so that a string is never cut in the middle
/// of a letter (names may be written in any alphabet, and `i + 1` panics there).
fn c_len(text: &str, i: usize) -> usize {
    text[i..].chars().next().map(|c| c.len_utf8()).unwrap_or(1)
}

/// Put a driver's name in place of the word under the caret. Returns the new text and the byte offset
/// the caret lands on — right behind the name, so that typing carries on from there.
///
/// What comes after the word is KEPT. It used to be dropped: the result was assembled as head plus name,
/// so choosing `len` in `10+le*2` gave `10+len` and the `*2` was gone.
pub(super) fn insert_driver(text: &str, name: &str, caret: usize) -> (String, usize) {
    let (start, end, _) = current_token(text, caret);
    (format!("{}{}{}", &text[..start], name, &text[end..]), start + name.len())
}

/// The height limit of the parameter list: beyond it the list SCROLLS rather than growing the window off
/// the edge of the screen.
pub(super) const PARAM_ROWS_MAX_H: f32 = 420.0;

/// What drawing the parameter rows returned. The width and the height are REAL, taken from the frame: a
/// test uses them to check that the fields stretch and that a long list scrolls rather than growing without
/// end.
#[derive(Default)]
pub(super) struct ParamRowsOut {
    pub dirty: bool,
    pub remove: Option<usize>,
    pub name_w: f32,
    /// The height of the scrolling VIEWPORT — this is what the limit caps.
    pub height: f32,
    /// The height of the CONTENT. It grows with the number of rows, and shows that the list really is long
    /// and that the viewport scrolls it rather than showing it whole.
    pub content_h: f32,
    /// The expressions that did not evaluate, as (the parameter's name, the reason in words). They are shown
    /// BELOW the table, across its whole width: the value column is too narrow for a sentence, and a reason
    /// that has to be hunted for by hovering is a reason nobody reads.
    pub errors: Vec<(String, String)>,
}

/// THE FIELD WIDTHS IN THE PARAMETERS WINDOW ARE ELASTIC, NOT FIXED.
///
/// They used to be 90 points for the name and 120 for the formula, and the window could be stretched as far
/// as one liked while the fields stayed as they were. A long variable name was IMPOSSIBLE to type: the text
/// crept past the edge and one typed blind. Now both fields share the window's width, and the former
/// numbers became the lower bound — a narrow window does not collapse them to nothing.
///
/// The proportions are chosen so that the formula is wider than the name (expressions like `w*2+3` live in
/// it), and so that the third column with the value and the button still has room.
pub(super) fn param_field_widths(avail: f32) -> (f32, f32) {
    const NAME_MIN: f32 = 90.0;
    const EXPR_MIN: f32 = 120.0;
    const NAME_SHARE: f32 = 0.30;
    const EXPR_SHARE: f32 = 0.38;
    ((avail * NAME_SHARE).max(NAME_MIN), (avail * EXPR_SHARE).max(EXPR_MIN))
}

fn poly_area(poly: &[Pos2]) -> f64 {
    let n = poly.len();
    let mut s = 0.0;
    for i in 0..n {
        let a = poly[i];
        let b = poly[(i + 1) % n];
        s += (a.x as f64) * (b.y as f64) - (b.x as f64) * (a.y as f64);
    }
    (s * 0.5).abs()
}

fn normal_label(n: [f64; 3]) -> &'static str {
    if n[2] > 0.85 {
        "normal-top"
    } else if n[2] < -0.85 {
        "normal-bottom"
    } else if n[2].abs() < 0.25 {
        "normal-side"
    } else {
        "normal-tilted"
    }
}

fn side_label(s: SideMode) -> &'static str {
    match s {
        SideMode::Auto => "cam-side-auto",
        SideMode::Outside => "cam-side-out",
        SideMode::Inside => "cam-side-in",
        SideMode::On => "cam-side-online",
    }
}

fn drill_label(d: DrillKind) -> &'static str {
    match d {
        DrillKind::Drill => "G81",
        DrillKind::DwellDrill => "G82",
        DrillKind::Peck => "G83",
    }
}

fn coolant_label(c: CoolantMode) -> &'static str {
    match c {
        CoolantMode::Off => "cam-cool-off",
        CoolantMode::Mist => "cam-cool-mist",
        CoolantMode::Flood => "cam-cool-flood",
    }
}


/// The GUI tests live in a file of their own (`gui/tests.rs`) rather than at the end of this one.
#[cfg(test)]
mod audit;
mod behaviour_sweep;
mod described_picks;
mod expand_selection;
mod suppress_flow;
mod help_voice;
mod sketch_sweep;
mod input;
mod bg_rebuild;
mod font_coverage;
mod focus_keys;
mod frame_cost;
mod grab;
mod grab_tests;
mod joint_creation;
mod joint_flow;
mod confirm_once;
mod contours_visible;
mod dof_line;
mod expr_errors;
mod expr_fields;
mod driver_autocomplete;
mod copy_puts_the_tool_down;
mod thread_runout_look;
mod the_view_belongs_to_the_person;
mod escape_with_an_open_list;
mod expr_field_behaviour;
mod params_table_gesture;
mod dim_popup_gesture;
mod feature_driver_gesture;
mod lost_refs_on_edit;
mod hidden_is_not_pickable;
mod joint_pick_highlight;
mod a_broken_joint_says_so;
mod a_group_is_made_by_hand;
mod a_width_is_made_by_hand;
mod a_tangent_is_made_by_hand;
mod a_relation_is_made_by_hand;
mod a_joint_can_hold_what_it_finds;
mod a_mechanism_can_be_watched_moving;
mod a_connector_stands_on_its_own;
mod the_gizmo_pulls_where_it_points;
mod an_anchor_belongs_to_the_part_under_the_cursor;
mod an_edited_joint_shows_its_anchors;
mod changing_an_anchor_does_not_spin_the_part;
mod rebuilding_everything_asks_for_geometry_again;
mod f1_answers_for_every_assembly_tool;
mod no_raw_keys_on_the_assembly_screen;
mod a_dead_mate_can_be_revived;
mod a_mate_takes_the_face_you_click;
mod one_list_of_mates;
mod the_mate_hud;
mod limits_hold_and_say_so;
mod a_part_can_be_dragged;
mod a_mechanism_can_be_run;
mod an_anchor_on_a_moving_part_is_refused;
mod no_widget_id_clashes;
mod every_assembly_tool_speaks;
mod every_picking_tool_highlights;
mod no_silent_refusal_on_click;
mod escape_drops_every_assembly_tool;
mod tools_are_mutually_exclusive;
mod no_raw_keys_in_assembly;
mod a_slider_between_facing_faces;
mod a_stuck_assembly_says_so;
mod dragging_a_joint_does_not_flicker_the_rebuild_modal;
mod a_chain_of_parts_follows_the_hand;
mod a_global_mate_is_dragged_by_its_part;
mod preparing_the_brep_never_loops;
mod dragging_a_part_pulls_the_whole_chain;
mod a_part_is_dragged_by_real_mouse;
mod a_moved_part_does_not_rebuild_its_block;

mod an_edge_anchor_survives_reopening;
mod every_kind_is_made_by_hand;
mod every_relation_is_made_by_hand;
mod joint_limits_are_visible;
mod saving_is_not_silent;
mod tree_drag;
mod param_fields_stretch;
mod interference;
mod no_assembly_sketch;
mod one_extrude;
mod one_node_per_command;
mod open_keeps_bodies;
mod props_readonly;
mod recent_files;
mod redundant_flag;
mod reveal_folder;
mod regen_cancel;
mod root_name;
mod start_screen;
mod start_screen_tests;
mod delete_feature_view;
mod edit_keeps_body;
mod doc_props;
mod cam_hidden;
mod catalog_flow;
mod comp_array_flow;
mod help_flow;
mod help_general;
mod help_images;
mod help_pictures;
mod help_raster;
mod help_map_flow;
mod help_settings;
mod help_window;
mod hotkeys;
mod hotkeys_rebind;
mod perspective_cull;
mod power_trim_flow;
mod scheme_flow;
mod search_flow;
mod screen_keys;
mod settings_sections;
mod settings_sections_tests;
mod sketch_paint;
mod sketch_reopen;
mod settings_memory;
mod settings_profile;
mod measure3d;
mod dim_drag_mouse;
mod gizmo_mouse;
mod long_values;
mod modal_barrier;
mod open_keeps_brep;
mod regen_live_view;
mod measure_flow;
mod projection_flow;
mod push_face_flow;
mod split_flow;
mod templates_flow;
mod fillet_vertex_flow;
mod thicken_flow;
mod loft_surface_flow;
mod patch_flow;
mod stitch_flow;
mod hand;
mod layer_b;
mod trim_flow;
mod user_case;
mod tool_popup_sweep;
mod view_cache_sweep;
mod remove_face_flow;
mod sketch_ref;
mod view_state;
mod viewcube;
mod viewcube_flow;
mod fuzz;
mod geom_quality;
mod toolbar_dupes;
mod tree_row_identity;
mod tree_search;
mod tuned_constants;
mod ui_sweep;
mod unsaved_prompt;
mod clipped_text_sweep;
mod param_error_readable;
mod ghost_highlight;
mod release_build;
mod about_build_line;
mod crash_notice;
mod tests;

/// Picking and hit-testing live in `gui/pick.rs`.
mod report_problem;

mod pick;

/// Drawing the scene and the overlays lives in `gui/render.rs`.
mod render;

/// The Part commands (starting them, their parameters, applying them) live in `gui/commands.rs`.
mod command_search;
mod commands;

/// The assembly joints live in `gui/joints.rs`.
mod joints;

/// Inferring the anchor under the cursor lives in `gui/mate_infer.rs`.
mod mate_infer;

/// THE ONE list of assembly tools that everyone reads lives in `gui/assembly_tools.rs`.
mod assembly_tools;

/// The panels (the tree, the properties) live in `gui/panels.rs`.
mod panels_bars;
pub(crate) mod panels_source;
pub(crate) mod render_source;
mod render_scene;
mod viewport_3d;
mod panels_props;
mod panels_tree;
mod panels_windows;

/// The single expression field and its list of drivers. One for the whole project, so that a dimension's
/// field, a feature's field and a table cell all behave the same way.
mod expr_field;

/// The single header of a properties card lives in `gui/props_card.rs`.
mod props_card;
pub(crate) use props_card::{props_header, NameSlot};

/// The sketcher (geometry, dimensions, constraints) lives in `gui/sketching.rs`.
mod sketching;

/// Files and background jobs live in `gui/io_jobs.rs`.
mod io_jobs;
