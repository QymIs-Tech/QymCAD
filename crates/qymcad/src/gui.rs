//! The egui interface: the operation tree, the tool library, the 2D backplot, the projects.
//!
//! The model is `qymcad_core::model::Project`. A 2D top view (an egui `Painter`) with panning, zooming and
//! contour selection by click.

use eframe::egui;
use egui::{Color32, Pos2, Rect, Sense, Stroke};
#[allow(unused_imports)] // the child modules reach it through `super::Vec2`
pub(crate) use egui::Vec2;
use egui_phosphor::regular as ph;

use qymcad_kernel::OcctKernel; // THE ONLY kernel implementation, shared with the repro harness
use qymcad_core::geom::{MeshFace, Point2};


use qymcad_core::model::{Id, Project};
use qymcad_io::{import_dxf, import_stl, import_svg};

/// THE APPLICATION'S NAME for eframe. It is also the key of the directory eframe puts the settings file into, so
/// it lives as one constant: were those two places to diverge, the settings window would show the path of a
/// directory that does not exist.
pub(crate) const APP_ID: &str = "qymcad";

/// THE NAME A PERSON READS, as against the identifier above that a machine stores things under. The
/// window bar, the task bar and the About window all say this one.
pub(crate) const APP_NAME: &str = "QymCAD";

/// The window icon, decoded from the 256x256 PNG built into the binary.
///
/// The same image the splash screen shows. A window with no icon of its own is given the framework's,
/// and a program that wears someone else's face in the task bar looks like a stray tool rather than the
/// one that was installed.
fn app_icon() -> egui::IconData {
    const ICON: &[u8] = include_bytes!("../../../assets/icons/linux/256x256.png");
    match image::load_from_memory(ICON) {
        Ok(img) => {
            let rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();
            egui::IconData { rgba: rgba.into_raw(), width: w, height: h }
        }
        // An icon is worth nothing next to starting at all: a window without one still works.
        Err(_) => egui::IconData::default(),
    }
}

/// THE CURRENT TIME IN ISO-8601 (UTC, to the second).
///
/// By its own arithmetic, without a date crate: exactly one string is needed in the whole project, and a
/// dependency for it would have to be carried, updated and explained. The format chosen is machine-readable and
/// sortable - whoever wants to show it to a person may, but what is stored must be unambiguous.
pub(crate) fn now_iso8601() -> String {
    iso8601_from_unix(unix_secs())
}

/// SECONDS SINCE THE EPOCH, in one place.
///
/// The same incantation stood in four files - the name of a crash file, the name of a report directory,
/// the stamp of a document and the autosave clock - each spelling out its own fall back to zero.
pub(crate) fn unix_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// CONVERTING unix SECONDS INTO ISO-8601. Separate from "what time is it", because only this part is testable:
/// the current time has nothing to be compared against in a test, while a known stamp has.
pub(crate) fn iso8601_from_unix(secs: u64) -> String {
    // THE LAYOUT IS BUILT HERE, THE CALENDAR IS NOT. Leap years, the rule of centuries and the length of
    // February are a solved problem; the layout is four numbers and two separators and is held by the table
    // of known stamps beside it. Only the hard half was handed over.
    let t = time::OffsetDateTime::from_unix_timestamp(secs as i64).unwrap_or(time::OffsetDateTime::UNIX_EPOCH);
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", t.year(), u8::from(t.month()), t.day(), t.hour(), t.minute(), t.second())
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
    qymcad_core::model::set_producer(crate::build_info::report_block().lines().next().unwrap_or_default());
    // AND THE CRASH STOPS DISAPPEARING. Installed before anything can panic, for the same reason.
    crate::crash::install();
    // THE BREADCRUMB SINK. Opening an edit records what a person just did; the crate of records does not
    // know what a crash report is, so the application plugs its trail in here, once.
    qymcad_ui_state::set_step_recorder(crate::crash::note_step);

    let mut options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            // WITHOUT THESE THE WINDOW BORROWS SOMEBODY ELSE'S FACE: the framework's own icon and the
            // storage identifier `qymcad` as a title. Both are what a person sees first, in a task bar
            // beside three other windows, and neither said anything about this program or its document.
            .with_title(APP_NAME)
            .with_icon(app_icon()),
        // the GPU viewport: egui on wgpu gives access to wgpu_render_state for the bodies' 3D paint callback.
        // glow stays compiled in as a fallback (the renderer is switched here).
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    choose_the_adapter_ourselves(&mut options);
    let started = eframe::run_native(
        APP_ID,
        options,
        Box::new(|cc| {
            // the Phosphor icon font (without it the icon glyphs render as empty boxes)
            install_fonts(&cc.egui_ctx);

            let mut app = App::default();
            // A CRASH FILE NOBODY IS TOLD ABOUT IS THE SAME AS NO CRASH FILE. The newest report from an
            // earlier run is picked up here and shown once.
            app.disk.crash_report = crate::crash::unseen_reports().into_iter().next();
            // restore the previous session's settings (the view preferences)
            if let Some(storage) = cc.storage {
                // THE SETTINGS COME AS ONE RECORD: loaded and assigned. There is no separate "apply" step, and
                // there is nowhere to lose a setting along the way - except the theme, which egui does not remember itself.
                if let Some(v) = eframe::get_value::<Settings>(storage, "settings") {
                    adopt_settings(&mut app.regen, &mut app.scheme, &mut app.set, &mut app.status, v, &cc.egui_ctx);
                } else {
                    let factory = app.set.clone(); // the factory defaults: the language from the locale, the theme, the scale
                    adopt_settings(&mut app.regen, &mut app.scheme, &mut app.set, &mut app.status, factory, &cc.egui_ctx);
                }
                // the previous session's project is reopened LAZILY: the splash with the logo is shown first (the
                // opening frames), and only then does the loading start, so a heavy STEP reparse does not hold an
                // empty window.
                if let Some(Some(path)) = eframe::get_value::<Option<String>>(storage, "last_project") {
                    if std::path::Path::new(&path).is_file() {
                        app.disk.io.startup = Some(path); // loaded asynchronously on the first frame (the splash with the spinner)
                    }
                }
            }
            // The start is ALWAYS isometric: 3D mode with an isometric camera (init=false means it frames itself to
            // the contents on the first frame while keeping the orientation). Independent of the previous session.
            app.viewing.mode_3d = true;
            app.viewing.cam = Cam3::default();
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
            // DRAWING ON THE PROCESSOR IS SAID OUT LOUD, once, in the status line. Everything works and
            // nothing is broken - but turning a model feels like wading, and a person owed no explanation
            // reports it as a fault in the program rather than as a graphics driver they have not installed.
            if let Some(name) = crate::diagnostics::drawing_on_the_processor() {
                app.status = crate::i18n::tr1("gpu-on-the-processor", "name", &name);
            }
            Ok(Box::new(app))
        }),
    );
    // A START THAT NEVER HAPPENED USED TO LEAVE NOTHING BEHIND. The framework hands this back as an
    // ordinary error rather than a panic, so the crash hook never saw it, and the message went to standard
    // error - which a windowed build on Windows has nowhere to print to. What a person saw was a window
    // that blinked and closed, and the crash folder was empty.
    if let Err(e) = &started {
        let path = crate::crash::note_failed_start(&e.to_string());
        crate::diagnostics::note_start_failure(&e.to_string(), path.as_deref());
    }
    started
}

/// WHICH CARD DRAWS, DECIDED HERE AND WRITTEN DOWN.
///
/// Left to itself the framework asks for an adapter and, finding none, fails with an error nobody sees.
/// Two things are wanted instead, and both need the list of adapters in hand: every one of them recorded
/// (a report from a machine that would not start is otherwise a blank page), and the ONE ON THE PROCESSOR
/// accepted rather than refused when it is all there is.
///
/// Reported behaviour: on Windows 11 with an old card and no driver installed the program would not start;
/// there was no crash file and no falling back to drawing on the processor either. On the same version a
/// virtual machine and a modern card both started without trouble.
///
/// Software rendering is slow, and on a big assembly it is very slow - but it is a program that runs and
/// says why it is slow, against one that closes without a word.
fn choose_the_adapter_ourselves(options: &mut eframe::NativeOptions) {
    use eframe::wgpu;
    // THE FRAMEWORK'S OWN SETTINGS ARE KEPT and only two of them changed. Building the record from scratch
    // would drop the display handle it fills in for us, which some systems will not open a window without.
    let eframe::egui_wgpu::WgpuSetup::CreateNew(setup) = &mut options.wgpu_options.wgpu_setup else { return };
    // THE BACKENDS ARE LEFT AS THEY COME: the framework already asks for the primary ones plus GL, and on
    // a desktop there is nothing else to add. What changes here is WHICH of the adapters they enumerate is
    // taken, and that the list is written down either way.
    setup.native_adapter_selector = Some(std::sync::Arc::new(|adapters: &[wgpu::Adapter], surface: Option<&wgpu::Surface<'_>>| {
        let seen: Vec<String> = adapters.iter().map(describe_adapter).collect();
        crate::diagnostics::note_adapters(&seen);
        // a card that cannot draw into THIS window is no use, whatever else it can do
        let best = adapters.iter().filter(|a| surface.is_none_or(|s| a.is_surface_supported(s))).max_by_key(|a| rank_adapter(a));
        match best {
            Some(a) => {
                let info = a.get_info();
                crate::diagnostics::note_gpu(format!("wgpu {:?}, {} ({:?}), driver {} {}", info.backend, info.name, info.device_type, info.driver, info.driver_info));
                // A CPU ADAPTER IS A WORKING PROGRAM, and the person is told rather than left to wonder
                // why turning a model feels like wading.
                if info.device_type == wgpu::DeviceType::Cpu {
                    crate::diagnostics::note_drawing_on_the_processor(&info.name);
                }
                Ok(a.clone())
            }
            // THE MESSAGE IS THE REPORT. This string is what reaches the file and the message box, so it
            // says what was looked for and what was found rather than "no suitable adapter".
            None => Err({
                crate::diagnostics::note_no_adapter();
                format!(
                "no graphics adapter this window can draw on. Offered {}: {}",
                adapters.len(),
                if seen.is_empty() { "none at all - the machine has no working graphics driver".to_string() } else { seen.join("; ") }
                )
            }),
        }
    }));
}

/// One adapter in one line, for the report.
fn describe_adapter(a: &eframe::wgpu::Adapter) -> String {
    let i = a.get_info();
    format!("{:?}/{:?} {} (driver {} {})", i.backend, i.device_type, i.name, i.driver, i.driver_info)
}

/// HOW GOOD AN ADAPTER IS, highest first. A real card beats a shared one, a shared one beats a virtual one,
/// and everything beats the processor - which is taken only when nothing else answers.
fn rank_adapter(a: &eframe::wgpu::Adapter) -> u8 {
    rank_device_type(a.get_info().device_type)
}

/// The ranking itself, apart from any adapter: a machine with no graphics is exactly the machine a test
/// cannot be run on, so the order is checked here instead.
///
/// NOTHING SCORES ZERO. The processor is the worst choice and the one that must still be TAKEN when it is
/// the only one - refusing it is the program that would not start.
pub(crate) fn rank_device_type(t: eframe::wgpu::DeviceType) -> u8 {
    use eframe::wgpu::DeviceType;
    match t {
        DeviceType::DiscreteGpu => 5,
        DeviceType::IntegratedGpu => 4,
        DeviceType::VirtualGpu => 3,
        DeviceType::Other => 2,
        DeviceType::Cpu => 1,
    }
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


/// Toggle the parts library window; on the first opening it builds the catalogue tree.
pub(crate) fn toggle_parts_library(parts: &mut PartsLibrary, win: &mut Windows) {
    win.toggle(WinKind::PartsLibrary);
    if win.is(WinKind::PartsLibrary) && parts.tree.is_none() {
        parts.tree = Some(crate::parts_library::load_library_tree());
    }
}

pub(crate) use qymcad_i18n::error_words;
pub(crate) use qymcad_ui_state::settings_sections;
pub(crate) use qymcad_ui_state::*;

/// THE DOCUMENT ON DISK: where it came from, what is unsaved, and what is being written or read.
///
/// Ten fields of `App` about ONE subject - the file. They were spread through a hundred, so "is there
/// anything unsaved" and "where does Save write" were questions one answered by remembering which field
/// held what.
pub(crate) struct OnDisk {
    /// The path of the open document; `None` for one that has never been saved.
    pub(crate) project_path: Option<String>,
    /// The last DXF a drawing was imported from - the next import opens there.
    pub(crate) dxf_path: Option<String>,
    /// Reading and writing: the background jobs and what they came back with.
    pub(crate) io: DocIo,
    /// What has changed since the last save, and when the last autosave was.
    pub(crate) edits: Edits,
    /// A file chooser that is open, and what to do with the answer.
    pub(crate) file_ask: Option<file_ask::FileAsk>,
    /// A mesh export waiting for its deflection.
    pub(crate) stl_export: Option<ExportTarget>,
    /// A crash report left by an earlier run, to be shown once.
    pub(crate) crash_report: Option<std::path::PathBuf>,
    /// What the person is writing into "report a problem".
    pub(crate) report: crate::gui::report_problem::ReportDraft,
    /// The title as it stands in the window bar - kept so it is not set on every frame.
    pub(crate) title_shown: String,
    /// Somewhere to go once the unsaved-work question has been answered.
    pub(crate) pending_nav: Option<Nav>,
}

impl Default for OnDisk {
    /// THE TITLE STARTS AS THE PROGRAM'S NAME, not empty: an empty title bar at launch reads as a window
    /// that failed to open rather than as one with no document.
    fn default() -> Self {
        OnDisk {
            project_path: None,
            dxf_path: None,
            io: DocIo::default(),
            edits: Edits::default(),
            file_ask: None,
            stl_export: None,
            crash_report: None,
            report: crate::gui::report_problem::ReportDraft::default(),
            title_shown: APP_NAME.to_string(),
            pending_nav: None,
        }
    }
}


pub(crate) struct App {
    /// THE DRAWING TOOLS, all fourteen in one record (see `DrawState`). They only ever move together, and
    /// as fourteen fields among a hundred "the tools in hand" was something one had to know rather than read.
    tools: qymcad_ui_state::DrawState,
    /// THE PARAMETERS OF THE FEATURE BEING BUILT, all fourteen in one record (see `FeatParams`).
    params: qymcad_ui_state::FeatParams,
    /// HOW THE SCENE IS BEING LOOKED AT: the camera, the flat view, the mode, and what a turn or a drag
    /// of them needs while it lasts (see `Viewing`).
    viewing: qymcad_ui_state::Viewing,
    /// WHAT IS BEING DRAGGED in the scene, and by which handle (see `Dragged`).
    dragged: qymcad_ui_state::Dragged,
    /// THE TOOLS THAT ARE NOT DRAWING TOOLS: trimming, datums, mates, measuring, the section, the
    /// component array, the clipboard, the sketch array, a rotation and a name being typed (see `SideTools`).
    side: qymcad_ui_state::SideTools,
    /// THE DOCUMENT ON DISK: where it came from, what is unsaved, what is being written (see `OnDisk`).
    disk: OnDisk,
    /// WHAT IS CHOSEN and what is under the cursor (see `Chosen`).
    chosen: qymcad_ui_state::Chosen,
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
    /// THE PARTS LIBRARY: its tree, what is picked in it, and the thumbnails already drawn.; see [`PartsLibrary`].
    parts: PartsLibrary,
    /// THE MODEL TREE AS DRAWN: what is being searched for, what is being dragged, and where the rows landed.; see [`TreeUi`].
    tree: TreeUi,
    /// THE EDGES OF THE SELECTED BODY, ready for picking and for drawing.; see [`EdgeCache`].
    edges: EdgeCache,
    /// What is open on screen; see [`Windows`].
    win: Windows,
    /// Derived state that a frame may rebuild at will; see [`Caches`].
    cache: Caches,
    project: Project,
    /// The splash and its progress. The logo for the splash (its texture is loaded lazily on the first frame).
    logo_tex: Option<egui::TextureHandle>,
    /// The hotkey window's own state while it waits for a press (see `HotkeyCapture`).
    hotkeys: qymcad_ui_state::HotkeyCapture,
    /// THE TEXTS TYPED INTO THE BARS' FIELDS: the field's key mapped to what was typed.
    ///
    /// The values in the state are numbers (`tool_prefs.fillet`, `arr.count` and so on), and text cannot be put
    /// into them. So what was typed lives here and the number is recomputed from it. Without this the bars' fields
    /// would stay `DragValue`s, into which a formula CANNOT be typed: the sketch fillet radius, the offset, the
    /// copy counts and a polygon's number of sides could not be made parametric at all.
    bar_exprs: std::collections::HashMap<&'static str, String>,
    /// ALL THE PROGRAM'S SETTINGS in one record (see `Settings`). The single owner of the values: the settings
    /// window edits it, the store saves it whole, and the program reads from it.
    set: Settings,
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
    /// The cursor's coordinates in the part's frame (for the status line), in the 2D view.
    cursor: Option<Point2>,
    /// what the command points at by a click - an enumeration (see Picking)
    /// an import awaiting placement - one record (see PendingImport)
    /// what is being dragged with the mouse in a sketch - one record (see Dragging)
    /// The parameters window's search: it filters both the variables and the drivers, by name and by path.
    par_search: String,
    /// The graveyard of textures awaiting deferred release. A `TextureHandle` MUST NOT be dropped in a frame whose
    /// texture is still being drawn (wgpu reports "Texture ... has been destroyed" on submit). They are put here and
    /// cleared at the START of the next frame, before any drawing, so the texture is no longer used by anyone.
    tex_graveyard: Vec<egui::TextureHandle>,
    /// THE REBUILD GRAPH: the parameter values as of the last rebuild. When a name's value changes, only those
    /// that refer to it are marked dirty, not the project's whole parametrics.
    params_seen: std::collections::HashMap<String, f64>,
    /// The active workbench (Sketch, Part, Assembly or Machining).
    workbench: Workbench,
    /// the core of the active Part command - one record (see FeatCommand)
    /// The extrude's operation: 0 a new body, 1 join, 2 cut, 3 intersect.
    /// WHAT the current command builds and WHERE - one record instead of three independent fields (see FeatTarget)
    feat: FeatTarget,
    /// the geometry selected for the commands - one record (see GeomSelection)
    /// the sketch working session - one record (see SketchSession)
    sketch_ses: SketchSession,
    /// The active context's path: [the root, ..., the current one]. The breadcrumbs plus drilling in and out.
    active_path: Vec<Id>,
    /// The sketches hidden in 3D (by Id) - the visibility checkbox in the tree (they are visible by default).
    sketch_hidden: std::collections::HashSet<Id>,
    /// the Part tools' options - one record (see FeatOptions)
    opts: FeatOptions,
    /// the interface's deferred intentions - one record (see DeferredUi)
    deferred: DeferredUi,
    /// editing an annotation or a text - one record (see AnnotEdit)
    /// the inline editing of a sketch element - an enumeration (see InlineEdit)
    /// the sketch pattern's parameters - one record (see SketchPattern)
    sk_pat: SketchPattern,
    /// the drawing tools' preferences - one record (see SketchToolPrefs)
    tool_prefs: SketchToolPrefs,
    /// the sketch pattern tool - one record (see PatternTool)
    font_cache: Option<Vec<u8>>,
    /// The GPU viewport is available (the wgpu backend is active and the resources are installed). Otherwise the CPU raster is used.
    gpu_ok: bool,

    /// a running sweep of a degree of freedom (a mate's animation)
    joint_anim: Option<JointAnim>,
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
}

// `Hover` deliberately has NO shared reset: three places clear THEIR OWN kind of hover ("no constraint
// under the cursor here"), and the passes run every frame, so the mutual exclusion holds by itself. A
// shared reset would be wrong here — it would clear the hover a neighbouring pass had just computed.

pub(crate) fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    // the Phosphor icon font (otherwise icons render as "tofu" boxes)
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
    // A BOLD FACE. The egui set has none at all — only the ordinary proportional and monospace ones.
    // Labels drawn OVER geometry (the X/Y/Z axes, dimensions) get lost in a thin font.
    // Liberation Sans Bold: OFL (the licence sits next to the file), Latin plus Cyrillic.
    fonts.font_data.insert(
        BOLD_FONT.to_string(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!("../../../assets/fonts/LiberationSans-Bold.ttf"))),
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



    /// THE ONE DOORWAY OF THE WINDOWS.
    pub(crate) fn win_ctx<'a>(&'a mut self, ask: &'a mut Vec<WinAsk>) -> WinCtx<'a> {
        let file_ask_open = self.asking_for_a_file();
        WinCtx {
            cache: &mut self.cache,
            deferred: &mut self.deferred,
            edits: &mut self.disk.edits,
            gpu_ok: &mut self.gpu_ok,
            logo_tex: &mut self.logo_tex,
            par_search: &mut self.par_search,
            parts: &mut self.parts,
            pending_nav: &mut self.disk.pending_nav,
            project: &mut self.project,
            project_path: &mut self.disk.project_path,
            regen: &mut self.regen,
            scheme: &mut self.scheme,
            sel: &mut self.chosen.sel,
            set: &mut self.set,
            status: &mut self.status,
            stl_export: &mut self.disk.stl_export,
            tex_graveyard: &mut self.tex_graveyard,
            tree: &mut self.tree,
            waiting: &mut self.waiting,
            win: &mut self.win,
            live: &mut self.live,
            params_seen: &mut self.params_seen,
            view: &mut self.viewing.view,
            cam: &mut self.viewing.cam,
            active_path: &mut self.active_path,
            file_ask_open,
            hotkeys: &mut self.hotkeys,
            workbench: self.workbench,
            ask,
        }
    }

    /// Carry out what a window asked for, after it has drawn.
    pub(crate) fn do_win_asks(&mut self, asks: Vec<WinAsk>, ctx: &egui::Context) {
        for a in asks {
            match a {
                WinAsk::Nav(n) => self.do_nav(n, ctx),
                WinAsk::NavGuarded(n) => self.request_nav(n, ctx),
                WinAsk::Save => self.save_project(),
                WinAsk::GoTo { owner, sel } => {
                    if let Some(o) = owner {
                        self.enter_component(o);
                    }
                    self.chosen.sel = sel;
                }
                WinAsk::InsertPart(src) => self.insert_part_from(src),
                WinAsk::ExportSettings => self.ask_save_file(
                    rfd::AsyncFileDialog::new().set_file_name("qym-cad-settings.ron").add_filter("qym-cad settings", &["ron"]),
                    |app, p| {
                        let path = p.to_string_lossy().into_owned();
                        app.scheme.note = match crate::gui::export_settings_to(&app.set, &path) {
                            Ok(()) => crate::i18n::tr1("settings-profile-saved", "path", &path),
                            Err(e) => format!("{} {}", ph::WARNING, crate::i18n::tr1("settings-profile-failed", "error", &e)),
                        };
                    },
                ),
                WinAsk::ImportSettings => {
                    let ctx = ctx.clone();
                    self.ask_open_file(rfd::AsyncFileDialog::new().add_filter("qym-cad settings", &["ron"]), move |app, p| {
                        let path = p.to_string_lossy().into_owned();
                        app.scheme.note = match app.import_settings_from(&path, &ctx) {
                            Ok(()) => crate::i18n::tr("settings-profile-loaded"),
                            // A BROKEN FILE DOES NOT TOUCH THE CURRENT SETTINGS: it is said that it did not
                            // work, and what was there stays.
                            Err(e) => format!("{} {}", ph::WARNING, crate::i18n::tr1("settings-profile-failed", "error", &e)),
                        };
                    });
                }
                WinAsk::ExportStl(target, defl) => self.export_stl(target, defl),
                WinAsk::Delete(sel) => self.execute_delete(sel),
                WinAsk::RegenerateAll => qymcad_ui_state::regenerate_all(&mut self.rebuild_ctx()),
                // THE ONE DOOR a button uses. The search must not have a launch path of its own: a second one
                // sooner or later starts doing something other than the button, and nothing says so.
                WinAsk::RunCommand(code) => self.run_command(code),
            }
        }
    }

    /// THE ONE DOORWAY OF THE BARS.
    pub(crate) fn bar_ctx<'a>(&'a mut self, ask: &'a mut Vec<BarAsk>) -> BarCtx<'a> {
        BarCtx {
            armed: &mut self.tools.armed,
            active_path: &mut self.active_path,
            annot: &mut self.tools.annot,
            bar_exprs: &mut self.bar_exprs,
            boolean: &mut self.params.boolean,
            cam: &mut self.viewing.cam,
            carr: &mut self.side.carr,
            clip: &mut self.side.clip,
            cmd: &mut self.tools.cmd,
            corner: &mut self.tools.corner,
            dim: &mut self.tools.dim,
            drag: &mut self.tools.drag,
            dxf_path: &mut self.disk.dxf_path,
            edits: &mut self.disk.edits,
            feat: &mut self.feat,
            gsel: &mut self.tools.gsel,
            inline: &mut self.tools.inline,
            joint: &mut self.side.joint,
            m3: &mut self.side.m3,
            measure: &mut self.tools.measure,
            mirror: &mut self.params.mirror,
            mode_3d: &mut self.viewing.mode_3d,
            parts: &mut self.parts,
            pat: &mut self.tools.pat,
            pending_import: &mut self.tools.pending_import,
            picking: &mut self.tools.picking,
            place: &mut self.tools.place,
            project: &mut self.project,
            regen: &mut self.regen,
            scheme: &mut self.scheme,
            section: &mut self.side.section,
            sel: &mut self.chosen.sel,
            sel_sk: &mut self.tools.sel_sk,
            set: &mut self.set,
            sk_pat: &mut self.sk_pat,
            sketch_ses: &mut self.sketch_ses,
            status: &mut self.status,
            stl_export: &mut self.disk.stl_export,
            tool: &mut self.tools.tool,
            tool_prefs: &mut self.tool_prefs,
            view: &mut self.viewing.view,
            win: &mut self.win,
            workbench: &mut self.workbench,
            ask,
        }
    }

    /// Carry out what a bar asked for, after it has drawn.
    pub(crate) fn do_bar_asks(&mut self, asks: Vec<BarAsk>, ctx: &egui::Context) {
        for a in asks {
            match a {
                BarAsk::Nav(n) => self.request_nav(n, ctx),
                BarAsk::Save => self.save_project(),
                BarAsk::SaveAs => self.save_project_as(),
                BarAsk::PickDxf => self.pick_dxf(),
                BarAsk::PickStl => self.pick_stl(),
                BarAsk::PickStep => self.pick_step(),
                BarAsk::PickSvg => self.pick_svg(),
                BarAsk::PickFont => self.pick_font(),
                BarAsk::ExportStep(t) => self.export_step(t),
                BarAsk::Undo => self.undo(),
                BarAsk::Redo => self.redo(),
                BarAsk::Clipboard { cut } => self.clipboard_copy(cut),
                BarAsk::Paste => self.clipboard_paste(),
                BarAsk::RebuildEverything => self.rebuild_everything(),
                BarAsk::Help(article) => self.open_help(&article),
                BarAsk::GotoPath(i) => self.goto_path_index(i),
                BarAsk::ExitContext => self.exit_context(),
                BarAsk::SketchTool(t) => self.set_sk_tool(t),
                BarAsk::FeatCmd(c) => self.start_feat_cmd(c),
                BarAsk::PrimCmd(c) => crate::gui::commands::start_prim_cmd(&mut self.part_ctx(), c),
                BarAsk::ToggleSection => self.toggle_section(),
                BarAsk::MirrorPart(comp) => mirror_part_door(self, comp),
                BarAsk::ToggleMeasure3d => self.toggle_measure_3d(),
                BarAsk::EnterComponent(cid) => self.enter_component(cid),
                BarAsk::CompArray(m) => self.start_comp_array(m),
                BarAsk::CancelAllTools => self.cancel_all_tools(),
                BarAsk::ToggleLibrary => toggle_parts_library(&mut self.parts, &mut self.win),
                BarAsk::ToggleSketchPick => self.toggle_sketch_pick(),
                BarAsk::SketchSelectMode => crate::gui::sketching::sketch_select_mode(&mut self.sketch_ctx()),
                BarAsk::Constraint(code) => crate::gui::sketching::constraint_button(&mut self.sketch_ctx(), code),
                BarAsk::JointPick => self.start_joint_pick(),
                BarAsk::GroundPick => self.start_ground_pick(),
                BarAsk::GroupPick => self.start_group_pick(),
                BarAsk::WidthPick => self.start_width_pick(),
                BarAsk::TangentPick => self.start_tangent_pick(),
                BarAsk::RelationPick => self.start_relation_pick(),
            }
        }
    }

    /// THE ONE DOORWAY OF THE FEATURE TREE. The borrows are disjoint by field, so the panel gets exactly
    /// what it touches and nothing else.
    pub(crate) fn tree_ctx<'a>(&'a mut self, ask: &'a mut Vec<TreeAsk>) -> TreeCtx<'a> {
        TreeCtx {
            project: &mut self.project,
            view: &mut self.viewing.view,
            sel: &mut self.chosen.sel,
            tree_sel: &mut self.chosen.tree_sel,
            tree: &mut self.tree,
            rename: &mut self.side.rename,
            clip: &mut self.side.clip,
            status: &mut self.status,
            edits: &mut self.disk.edits,
            regen: &mut self.regen,
            live: &mut self.live,
            params_seen: &mut self.params_seen,
            datum: &mut self.side.datum,
            deferred: &mut self.deferred,
            interference: &mut self.interference,
            sketch_hidden: &mut self.sketch_hidden,
            sketch_ses: &mut self.sketch_ses,
            stl_export: &mut self.disk.stl_export,
            rollback: &mut self.dragged.rollback,
            workbench: self.workbench,
            set: &mut self.set,
            active_path: &self.active_path,
            cam: &self.viewing.cam,
            scheme: &self.scheme,
            ask,
        }
    }

    /// Carry out what the tree asked for, after it has drawn.
    pub(crate) fn do_tree_asks(&mut self, asks: Vec<TreeAsk>) {
        for a in asks {
            match a {
                TreeAsk::EditFeature(id) => crate::gui::commands::start_feat_cmd_edit(&mut self.part_ctx(), id),
                TreeAsk::EnterSketch(si) => self.enter_sketch_edit(si),
                TreeAsk::Clipboard { cut } => self.clipboard_copy(cut),
                TreeAsk::Paste => self.clipboard_paste(),
                TreeAsk::ExportSketch { si, flat } => self.export_sketch(si, flat),
                TreeAsk::ReplaceSketchPlane(si) => self.start_replace_sketch_plane(si),
                TreeAsk::Action { act, ti, nid, prev_feat, next_feat } => self.tree_action(act, ti, nid, prev_feat, next_feat),
                TreeAsk::SketchOnBasePlane(b) => {
                    self.create_sketch_on(qymcad_core::feature::SketchPlane::World(b));
                }
                TreeAsk::EnterComponent(cid) => self.enter_component(cid),
                TreeAsk::EditCompArray(pid) => self.start_comp_array_edit(pid),
                TreeAsk::ExportStep(cid) => self.export_step(ExportTarget::Component(cid)),
                TreeAsk::SavePart(cid) => self.open_save_part_dialog(cid),
                TreeAsk::Drop { dragged, target, how } => {
                    self.tree_apply_drop(dragged, target, how);
                }
                TreeAsk::Resync => crate::gui::commands::resync_after_topology_change(&mut self.part_ctx()),
            }
        }
    }

    pub(crate) fn props_ctx<'a>(&'a mut self, ask: &'a mut Vec<PropsAsk>) -> PropsCtx<'a> {
        PropsCtx {
            project: &mut self.project,
            datum: &mut self.side.datum,
            deferred: &mut self.deferred,
            hover: &mut self.chosen.hover,
            joint: &mut self.side.joint,
            picking: &mut self.tools.picking,
            sel_conn: &mut self.chosen.sel_conn,
            status: &mut self.status,
            edits: &mut self.disk.edits,
            regen: &mut self.regen,
            live: &mut self.live,
            params_seen: &mut self.params_seen,
            sel: &mut self.chosen.sel,
            array: &mut self.side.array,
            boolean: &mut self.params.boolean,
            opts: &mut self.opts,
            cache: &mut self.cache,
            win: &mut self.win,
            view: &mut self.viewing.view,
            gsel: &mut self.tools.gsel,
            sketch_ses: &mut self.sketch_ses,
            set: &mut self.set,
            active_path: &self.active_path,
            cam: &self.viewing.cam,
            scheme: &self.scheme,
            workbench: self.workbench,
            mode_3d: &mut self.viewing.mode_3d,
            ask,
        }
    }

    /// Carry out what the properties panel asked for, after it has drawn.
    pub(crate) fn do_props_asks(&mut self, asks: Vec<PropsAsk>) {
        for a in asks {
            match a {
                PropsAsk::EditFeature(id) => crate::gui::commands::start_feat_cmd_edit(&mut self.part_ctx(), id),
                PropsAsk::SketchOnDatum(id) => {
                    self.create_sketch_on(qymcad_core::feature::SketchPlane::Datum(id));
                }
                PropsAsk::JointPick => self.start_joint_pick(),
                PropsAsk::ConnPick => self.start_conn_pick(),
                PropsAsk::DeleteConnector(cid) => qymcad_assembly::delete_connector_asked(&mut self.joint_ctx(), cid),
                PropsAsk::RelationPick(j) => qymcad_assembly::relation_pick_click(&mut self.joint_ctx(), j),
                PropsAsk::SketchOnBasePlane(b) => {
                    self.create_sketch_on(qymcad_core::feature::SketchPlane::World(b));
                }
                PropsAsk::EnterSketch(si) => self.enter_sketch_edit(si),
                PropsAsk::ExitContext => self.exit_context(),
                PropsAsk::SetContext(cid) => self.set_context_to(cid),
            }
        }
    }
    /// The label of a candidate axis line for the command bar — construction lines are marked as such.

    /// Is the cursor on the face arrow? The threshold matches the body gizmo's, in pixels along the segment.
    pub(super) fn face_arrow_hit(&self, rect: Rect, pos: Pos2, basis: &([f64; 3], [f64; 3], [f64; 3])) -> bool {
        face_arrow_hit(&self.painting(), rect, pos, basis)
    }









    pub(super) fn face_arrow_geometry(&self) -> Option<([f64; 3], [f64; 3], [f64; 3])> {
        face_arrow_geometry(&self.painting())
    }

    /// The joint whose degree-of-freedom gizmo is active in the viewport right now. Two ways in:
    /// 1) a COMPONENT is selected — a direct child of the context, driven by a joint that has freedoms;
    /// 2) the joint's glyph is selected DIRECTLY (`Sel::Joint`) — needed for a joint held at the ROOT whose
    ///    driven part lies inside a subassembly and is not a direct child (`gizmo_component()` gives None),
    ///    while the subassembly's slider still wants to be draggable by its handle straight from the root.
    fn active_dof_joint(&self) -> Option<Id> {
        qymcad_pick::active_dof_joint(&self.painting())
    }

    /// The gizmo mode of the selected component: grounded gives None; driven by a joint that has a freedom
    /// gives Joint(jid); otherwise (free, or a seed with no freedoms of its own) gives Free, the plain 6-DOF one.
    fn comp_gizmo_mode(&self, comp: Id) -> CompGizmoMode {
        qymcad_pick::comp_gizmo_mode(&self.painting(), comp)
    }


    /// The geometry of the section GIZMO: the centre of the quad on the plane, u, v, the half-size, and the arrow's tip.
    fn section_gizmo_geom(&self) -> Option<qymcad_ui_state::SectionGizmo> {
        section_gizmo_geom(&self.painting())
    }


    /// The display transform of a datum (a point, an axis or a plane, by its Id) in the active context's
    /// frame — so that a part's datums travel with it in an assembly, just as its bodies do. None means
    /// another component's datum (isolation: we do not draw it).
    fn datum_render_transform(&self, datum_id: Id) -> Option<[f64; 12]> {
        datum_render_transform(&self.painting(), datum_id)
    }


    /// The one doorway into drawing. Shared throughout, so it costs nothing to build and can be taken from
    /// a `&self` method - which every drawing method is.
    pub(crate) fn painting(&self) -> Painting<'_> {
        Painting {
            armed: &self.tools.armed,
            active_path: &self.active_path,
            arr: self.params.arr,
            body_giz: &self.dragged.body_giz,
            cache: &self.cache,
            cam: self.viewing.cam,
            carr: &self.side.carr,
            clip: &self.side.clip,
            cmd: &self.tools.cmd,
            comp_giz: self.dragged.comp_giz,
            cursor: self.cursor,
            datum: &self.side.datum,
            draft: self.params.draft,
            edges: &self.edges,
            face_arrow_drag: self.dragged.face_arrow_drag,
            feat: self.feat,
            gpu_ok: self.gpu_ok,
            gsel: &self.tools.gsel,
            hole: self.params.hole,
            hover: self.chosen.hover,
            inline: self.tools.inline,
            interference: &self.interference,
            joint: &self.side.joint,
            live: &self.live,
            loft: &self.params.loft,
            m3: &self.side.m3,
            mirror: &self.params.mirror,
            mode_3d: self.viewing.mode_3d,
            pat: self.tools.pat,
            pending_import: &self.tools.pending_import,
            picking: self.tools.picking,
            prim: self.params.prim,
            project: &self.project,
            regen: &self.regen,
            repl_surface: self.params.repl_surface,
            rev: self.params.rev,
            rot: &self.side.rot,
            scheme: &self.scheme,
            section: &self.side.section,
            snap_hint: self.snap_hint,
            sel: self.chosen.sel,
            sel_sk: &self.tools.sel_sk,
            set: &self.set,
            sk_pat: self.sk_pat,
            sketch_hidden: &self.sketch_hidden,
            sketch_ses: self.sketch_ses,
            split: &self.params.split,
            stitch_parts: &self.params.stitch_parts,
            sweep: self.params.sweep,
            thread: self.params.thread,
            tool: &self.tools.tool,
            tool_prefs: &self.tool_prefs,
            trim: &self.side.trim,
            view: self.viewing.view,
            view_dragging: self.viewing.view_dragging,
            win: &self.win,
            workbench: self.workbench,
        }
    }

    /// The one doorway into the sketch workbench: built in a single place so the borrows stay disjoint.
    pub(crate) fn sketch_ctx(&mut self) -> SketchCtx<'_> {
        SketchCtx {
            armed: &mut self.tools.armed,
            project: &mut self.project,
            regen: &mut self.regen,
            status: &mut self.status,
            place: &mut self.tools.place,
            cmd: &mut self.tools.cmd,
            tool: &mut self.tools.tool,
            sel: &mut self.chosen.sel,
            sel_sk: &mut self.tools.sel_sk,
            drag: &mut self.tools.drag,
            edits: &mut self.disk.edits,
            dim: &mut self.tools.dim,
            pending_import: &mut self.tools.pending_import,
            gsel: &mut self.tools.gsel,
            measure: &mut self.tools.measure,
            inline: &mut self.tools.inline,
            pat: &mut self.tools.pat,
            picking: &mut self.tools.picking,
            corner: &mut self.tools.corner,
            annot: &mut self.tools.annot,
            live: &mut self.live,
            params_seen: &mut self.params_seen,
            view: &mut self.viewing.view,
            cursor: &mut self.cursor,
            cache: &self.cache,
            cam: &self.viewing.cam,
            set: &self.set,
            scheme: &self.scheme,
            active_path: &self.active_path,
            sk_pat: &mut self.sk_pat,
            tool_prefs: &mut self.tool_prefs,
            sketch_ses: &mut self.sketch_ses,
            font_cache: &mut self.font_cache,
            mode_3d: &mut self.viewing.mode_3d,
            snap_hint: &mut self.snap_hint,
            workbench: &mut self.workbench,
            body_giz: &mut self.dragged.body_giz,
            tree_sel: &mut self.chosen.tree_sel,
            trim: &mut self.side.trim,
            clip: &mut self.side.clip,
            loft: &mut self.params.loft,
            rev: &mut self.params.rev,
            rot: &mut self.side.rot,
            sweep: &mut self.params.sweep,
        }
    }

    /// The one doorway into the Part workbench: built in a single place so the borrows stay disjoint.
    pub(crate) fn part_ctx(&mut self) -> PartCtx<'_> {
        PartCtx {
            armed: &mut self.tools.armed,
            status: &mut self.status,
            cmd: &mut self.tools.cmd,
            project: &mut self.project,
            gsel: &mut self.tools.gsel,
            sel: &mut self.chosen.sel,
            split: &mut self.params.split,
            opts: &mut self.opts,
            feat: &mut self.feat,
            mirror: &mut self.params.mirror,
            loft: &mut self.params.loft,
            chamfer: &mut self.params.chamfer,
            arr: &mut self.params.arr,
            datum: &mut self.side.datum,
            stitch_parts: &mut self.params.stitch_parts,
            trim: &mut self.side.trim,
            prim: &mut self.params.prim,
            draft: &mut self.params.draft,
            hole: &mut self.params.hole,
            sweep: &mut self.params.sweep,
            repl_surface: &mut self.params.repl_surface,
            thread: &mut self.params.thread,
            edges: &mut self.edges,
            edits: &mut self.disk.edits,
            live: &mut self.live,
            regen: &mut self.regen,
            params_seen: &mut self.params_seen,
            boolean: &mut self.params.boolean,
            bar_exprs: &mut self.bar_exprs,
            rev: &mut self.params.rev,
            cmd_failed: &mut self.cmd_failed,
            carr: &mut self.side.carr,
            view: &mut self.viewing.view,
            view_restore: &mut self.viewing.view_restore,
            cam: &mut self.viewing.cam,
            picking: &mut self.tools.picking,
            set: &self.set,
            active_path: &self.active_path,
            scheme: &self.scheme,
            comp_giz: &self.dragged.comp_giz,
            mode_3d: &mut self.viewing.mode_3d,
            body_giz: &mut self.dragged.body_giz,
            joint: &self.side.joint,
            workbench: self.workbench,
            cache: &mut self.cache,
            sketch_ses: &mut self.sketch_ses,
            win: &mut self.win,
        }
    }
    /// The one doorway into the rebuild: built in a single place so the borrows stay disjoint.
    pub(crate) fn rebuild_ctx(&mut self) -> RebuildCtx<'_> {
        RebuildCtx {
            project: &mut self.project,
            view: &mut self.viewing.view,
            edits: &mut self.disk.edits,
            regen: &mut self.regen,
            live: &mut self.live,
            params_seen: &mut self.params_seen,
            status: &mut self.status,
            sel: &mut self.chosen.sel,
            active_path: &self.active_path,
            cam: &self.viewing.cam,
            scheme: &self.scheme,
            set: &self.set,
        }
    }









}

/// tan(half of the field of view) for perspective mode. ~0.32 is a vertical FOV of about 35 deg — a
/// moderate amount of depth, the usual default. Not used in orthographic mode.

/// The opacity of a ghosted body: straight alpha 0..255. Your own body or sketch is seen through it.

impl Default for App {
    fn default() -> Self {
        let mut project = Project::default();
        project.new_document(); // the root assembly plus an active empty first Part right from the start
        let mut app = Self {
            set: Settings::default(),
            cmd_failed: false,
            snap_hint: None,
            cache: Caches::default(),
            interference: Interference::default(),
            scheme: SchemeUi::default(),
            waiting: Waiting::default(),
            live: LiveGeom::default(),
            regen: Rebuilding::default(),
            parts: PartsLibrary::default(),
            tree: TreeUi::default(),
            edges: EdgeCache::default(),
            // The start screen is open at launch; the constraint glyphs are on. Built and then opened, because
            // the open set is private - one door in, so nobody can invent a second way of saying "it is open".
            win: {
                let mut w = Windows::default();
                w.help.article = "index".into();
                w.constraints = true;
                w.open(WinKind::Start);
                w
            },
            tools: qymcad_ui_state::DrawState::default(),
            params: qymcad_ui_state::FeatParams::default(),
            viewing: qymcad_ui_state::Viewing::default(),
            dragged: qymcad_ui_state::Dragged::default(),
            side: qymcad_ui_state::SideTools::default(),
            disk: OnDisk::default(),
            chosen: qymcad_ui_state::Chosen::default(),
            project,
            logo_tex: None,
            hotkeys: qymcad_ui_state::HotkeyCapture::default(),
            bar_exprs: std::collections::HashMap::new(),
            status: crate::i18n::tr("g-start-hint"),
            cursor: None,
            // THE "ANGLE" FIELD IS EMPTY AT START-UP. It used to hold 90 deg, and that silently became a
            // REQUIREMENT: `joint_pick_anchor_click` writes a non-zero angle into `drive[0]` of the joint
            // being created. Every joint was born demanding a 90 deg turn that nobody had asked for —
            // against the contract stated in that same place ("a joint is born with its degrees of
            // freedom free"). On a slider that pinned a degree it does not even have; a gear relation
            // became unsatisfiable, the solve did not converge, and the WHOLE mechanism froze.
            par_search: String::new(),
            tex_graveyard: Vec::new(),
            params_seen: std::collections::HashMap::new(),
            workbench: Workbench::Part,
            feat: FeatTarget::default(),
            sketch_ses: SketchSession::default(),
            active_path: Vec::new(),
            sketch_hidden: std::collections::HashSet::new(),
            opts: FeatOptions { mirror_keep: true, ..Default::default() },
            deferred: DeferredUi::default(),
            sk_pat: SketchPattern { dx: 20.0, dy: 0.0, count: 3, dx2: 0.0, dy2: 20.0, count2: 1, angle: 360.0 },
            tool_prefs: SketchToolPrefs { poly_n: 6, poly_edge: 10.0, fillet: 3.0, offset: 3.0, text_h: 10.0, ..Default::default() },
            font_cache: None,
            gpu_ok: false,
            joint_anim: None,
        };
        // THE GUARD'S BASELINE COMES FROM THE DOCUMENT ITSELF, not from zero. With zero the very first
        // frame saw "the key does not match" and declared a fresh empty document changed outside an
        // operation.
        app.disk.edits.committed_key = doc_key(&app.project);
        app.disk.edits.saved_key = edit_key(&app.draw_ctx());
        app
    }
}



impl App {



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
        let src = embed_source(&mut self.project, path);
        self.cancel_all_tools(); // drop the other picking modes and commands, so clicks do not conflict
        self.viewing.mode_3d = true; // placement planes and faces are picked in 3D (like picking a sketch plane)
        self.chosen.sel = Sel::None;
        self.tools.pending_import.curves = Some((curves, src, file_name(path)));
        self.disk.dxf_path = Some(path.to_string());
        self.status = crate::i18n::tr1("g-import-place", "n", &n.to_string());
    }

    /// Place the waiting import on the chosen plane: build an editable sketch in the active context and
    /// enter its editing (visible and editable straight away — it can be drawn onto and dimensioned).
    fn place_pending_import(&mut self, plane: qymcad_core::feature::SketchPlane) {
        let Some((curves, source, name)) = self.tools.pending_import.curves.take() else { return };
        let plane = resolve_placement_plane(&mut self.tools.cmd, &mut self.project, &mut self.status, plane);
        let si = self.project.import_sketch(name, curves, source, plane);
        let ents = self.project.sketches[si].entities.len();
        invalidate(&mut self.regen);
        self.viewing.view.initialized = false;
        self.enter_sketch_edit(si);
        self.status = crate::i18n::tr1("g-sketch-imported", "n", &ents.to_string());
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
                self.disk.project_path = None;
                self.set.recent.retain(|p| p != path); // a template is not a "recent file"
                self.disk.edits.saved_key = edit_key(&self.draw_ctx()); // a blank slate: no edits have been made yet
                self.status = crate::i18n::tr1("tpl-new-from", "name", &std::path::Path::new(path).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default());
            }
            Err(e) => self.status = format!("{} {}", ph::WARNING, crate::i18n::tr1("tpl-open-failed", "error", &e.to_string())),
        }
    }


    pub(crate) fn new_project(&mut self) {
        self.project = Project::default();
        self.project.new_document();
        self.live.shapes.clear();
        self.chosen.sel = Sel::None;
        self.disk.project_path = None;
        self.disk.dxf_path = None;
        self.tools.pending_import.draw_pts = None;
        invalidate(&mut self.regen);
        self.viewing.view.initialized = false;
        self.viewing.cam.init = false;
        self.disk.edits.saved_key = edit_key(&self.draw_ctx()); // a clean start — there are no edits
        self.status = crate::i18n::tr("g-new-project");
    }

    /// A NEW ASSEMBLY DOCUMENT: the root without an empty part, with the root active.
    ///
    /// "Create a part" and "create an assembly" are different intents, and starting an assembly with an
    /// empty part that then has to be deleted means making a person clean up after the program.
    pub(crate) fn new_assembly_project(&mut self) {
        self.new_project();
        // `new_document` created a part under the root — an assembly document does not need it
        let root = self.project.root;
        let parts: Vec<Id> = self.project.components.iter().filter(|c| c.parent == Some(root)).map(|c| c.id).collect();
        for id in parts {
            self.project.delete_component(id); // ask_delete-exempt: this is not deleting anyone's work but cleaning up while creating the document
        }
        self.project.set_active_component(Some(root));
        self.disk.edits.saved_key = edit_key(&self.draw_ctx()); // a clean start — there are no edits
        self.status = crate::i18n::tr("g-new-assembly");
    }




    /// Request navigation that could lose edits: with unsaved work it asks first, otherwise it goes ahead.
    fn request_nav(&mut self, nav: Nav, ctx: &egui::Context) {
        if qymcad_ui_state::is_dirty(&mut self.rebuild_ctx()) {
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
                self.ask_open_file(rfd::AsyncFileDialog::new().add_filter("QymCAD", &["qcad", "ron"]), |app, p| {
                    crate::gui::io_jobs::spawn_project_load(&mut app.regen, p.to_string_lossy().into_owned());
                });
            }
            Nav::NewFromTemplate(path) => self.new_from_template(&path),
            Nav::OpenPath(path) => open_recent(&mut self.regen, &mut self.set, &mut self.status, path),
            Nav::Exit => {
                self.wait_bg(); // an unfinished background write must reach the disk before exiting
                self.disk.edits.allow_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }




    /// Enter the editing mode of sketch `si` (the usual "open the sketch").
    pub(crate) fn enter_sketch_edit(&mut self, si: usize) {
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
            self.viewing.nav_stash.push((self.viewing.cam, self.viewing.view, self.viewing.mode_3d));
            self.sketch_ses.editing = Some(s.id);
            self.workbench = Workbench::Sketch;
            self.viewing.mode_3d = false;
            self.chosen.sel = Sel::Sketch(si);
            self.tools.sel_sk.clear(); // the selection, and whatever was waiting on it
            self.tools.pending_import.draw_pts = None;
            self.tools.armed = qymcad_ui_state::Armed::None;
            self.viewing.view.initialized = false; // fit the view to the sketch
            self.status = crate::i18n::tr1("g-editing-sketch", "name", &s.name);
        }
    }

    /// Create a new empty sketch and enter its editing straight away.
    /// Pick a sketch drawing tool (entering a sketch first, if not in one yet).
    pub(crate) fn set_sk_tool(&mut self, t: u8) {
        if edit_si(&self.project, &self.sketch_ses).is_none() {
            self.create_new_sketch();
        }
        exit_draw_tools(&mut qymcad_ui_state::tools_of!(self)); // entering a tool means leaving all the others, in one move
        self.tools.tool.select(&mut self.tools.armed, t); // changing the tool clears whatever the previous one had collected
        self.viewing.mode_3d = false;
        self.status = match self.tools.armed.draw_kind() {
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

    fn create_new_sketch(&mut self) {
        self.create_sketch_on(qymcad_core::feature::SketchPlane::default());
    }


    /// Create a sketch on a given plane (World, Datum or Face) and enter its editing.
    fn create_sketch_on(&mut self, plane: qymcad_core::feature::SketchPlane) -> usize {
        let plane = resolve_placement_plane(&mut self.tools.cmd, &mut self.project, &mut self.status, plane);
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
        let plane = resolve_placement_plane(&mut self.tools.cmd, &mut self.project, &mut self.status, plane);
        let sid = self.project.sketches[si].id;
        self.project.sketches[si].plane = plane;
        self.project.mark_sketch_dirty(sid); // associativity: the bodies on this sketch will be rebuilt
        qymcad_ui_state::mark_dirty_for_rebuild(&mut self.rebuild_ctx()); // the document is marked; the scheduler does the computing
        self.tools.picking.clear();
        self.viewing.view.initialized = false;
        self.status = crate::i18n::tr("g-sketch-moved");
    }






    /// Enter a component (drill in): remember the camera, go one level down, fit the view.
    pub(crate) fn enter_component(&mut self, cid: Id) {
        if self.sketch_ses.editing.is_some() {
            self.finish_sketch_edit();
        }
        ensure_active_path(&mut self.active_path, &mut self.project);
        if current_ctx_id(&self.active_path, &self.project) == cid || !self.project.components.iter().any(|c| c.id == cid) {
            return;
        }
        self.cancel_all_tools(); // entering a component drops the active tool (nothing is carried in from the parent context)
        self.tree.search.clear(); // and the search query: it was about the PREVIOUS context, here it lies
        self.viewing.nav_stash.push((self.viewing.cam, self.viewing.view, self.viewing.mode_3d));
        self.active_path.push(cid);
        self.chosen.sel = Sel::None;
        self.viewing.view.initialized = false;
        self.viewing.cam.init = false;
        self.sync_workbench();
        let name = self.project.components.iter().find(|c| c.id == cid).map(|c| crate::i18n::name(&c.name)).unwrap_or_default();
        self.status = crate::i18n::tr1("g-context-is", "name", &name);
    }

    /// Go one level up (drill out): restore the camera.
    pub(crate) fn exit_context(&mut self) {
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
        if let Some((cam, view, mode_3d)) = self.viewing.nav_stash.pop() {
            self.viewing.cam = cam;
            self.viewing.cam.init = true;
            self.viewing.view = view;
            self.viewing.mode_3d = mode_3d;
        }
        self.chosen.sel = Sel::None;
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
        self.viewing.nav_stash.clear();
        self.chosen.sel = Sel::None;
        self.viewing.view.initialized = false;
        self.viewing.cam.init = false;
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




    /// READ THE SETTINGS FROM A FILE and adopt them.
    ///
    /// Missing fields fall back to the factory ones (`serde(default)` on the record): a profile taken
    /// from a different version of the program must still read, not be rejected wholesale — otherwise it
    /// cannot be shared. A broken file does NOT touch the current settings: the change arrives whole or
    /// not at all.
    pub(crate) fn import_settings_from(&mut self, path: &str, ctx: &egui::Context) -> Result<(), String> {
        let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let s: Settings = ron::from_str(&text).map_err(|e| e.to_string())?;
        adopt_settings(&mut self.regen, &mut self.scheme, &mut self.set, &mut self.status, s, ctx);
        Ok(())
    }










    /// After a DATUM is edited (its coordinates or its definition): datums are resolved unconditionally
    /// during regeneration, but their consumers (bodies on sketches that sit on a datum plane, axes
    /// through points) have to be rebuilt, otherwise they stay where they were.
    /// A FORCED regeneration of the whole document used to stand here, on the grounds that datums are
    /// rarely edited — on an assembly of a thousand bodies that is the same tens of seconds of freeze.
    /// Now only the nodes that depend on datums are marked dirty.
    fn regen_after_datum_change(&mut self) {
        crate::gui::io_jobs::ensure_brep(&mut self.rebuild_ctx()); // the kernel rebuilds the consumers' geometry, so a live B-rep is needed
        self.project.mark_datum_consumers_dirty();
        qymcad_ui_state::mark_dirty_for_rebuild(&mut self.rebuild_ctx()); // the document is marked; the scheduler does the computing
    }















    /// A rigid move of a body (mesh index `mi`) by the matrix `mat` (3x4): a B-rep gets a PARAMETRIC
    /// `Move` feature (which moves the shape), a raw imported mesh is transformed directly. This keeps the
    /// mesh panel from putting a B-rep body out of step with its shape.




























    /// Apply or update the pattern on Enter.
    fn confirm_pattern(&mut self) {
        begin_edit(&mut self.disk.edits, &self.project, crate::i18n::tr("g-sketch-array")); // THE OPERATION BOUNDARY
        let Sel::Sketch(si) = self.chosen.sel else { return };
        if let Some(pi) = self.tools.pat.edit {
            let src = self.project.sketches.get(si).and_then(|s| s.patterns.get(pi)).map(|p| p.source.clone()).unwrap_or_default();
            let kind = current_pattern_kind(&self.tools.armed, self.tools.pat, &self.project, self.sk_pat, si, &src);
            self.project.update_pattern(si, pi, kind);
            self.status = crate::i18n::tr("g-array-updated");
        } else {
            let eids: Vec<Id> = self.tools.sel_sk.items.iter().filter(|(k, _)| *k == 1).map(|(_, id)| *id).collect();
            if eids.is_empty() {
                self.status = crate::i18n::tr("g-select-for-array");
                return;
            }
            let kind = current_pattern_kind(&self.tools.armed, self.tools.pat, &self.project, self.sk_pat, si, &eids);
            self.project.add_pattern(si, &eids, kind);
            self.status = crate::i18n::tr("g-array-created");
        }
        self.tools.armed = qymcad_ui_state::Armed::None;
        self.tools.pat.edit = None;
        self.tools.pat.center = None;
        invalidate(&mut self.regen);
            qymcad_ui_state::commit_edit(&mut self.rebuild_ctx());
    }


    pub(crate) fn clipboard_copy(&mut self, cut: bool) {
        // while editing a sketch, the GEOMETRY is copied, whatever is selected in the tree
        if edit_si(&self.project, &self.sketch_ses).is_some() {
            let eids: Vec<Id> = self.tools.sel_sk.items.iter().filter(|(k, _)| *k == 1).map(|(_, id)| *id).collect();
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
            exit_draw_tools(&mut qymcad_ui_state::tools_of!(self));
            self.side.clip.geom_place = false;
            self.side.clip.geom_pending = Some((eids, cut));
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
        if edit_si(&self.project, &self.sketch_ses).is_some() && self.side.clip.geom.as_ref().is_some_and(|c| !c.is_empty()) {
            self.side.clip.geom_pending = None;
            self.side.clip.geom_place = true;
            self.status = crate::i18n::tr("g-insert-click");
            return;
        }
        self.tree_clipboard_paste(); // outside a sketch, paste into the tree (see `clipboard_copy`)
    }


    /// A bulk move or copy of components from the multiple-selection clipboard into a target assembly.
    /// Every node goes through the SINGLE core method `reparent_component` or `clone_component` (by
    /// design, the UI only delegates).
    fn paste_components_multi(&mut self, ids: &[Id], cut: bool) {
        use qymcad_core::feature::ComponentKind;
        let root = self.project.root;
        // the target is the selected subassembly; if a Part is selected, its parent assembly; otherwise the active context
        let target = match self.chosen.sel {
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
            self.side.clip.tree_multi = None;
            self.chosen.tree_sel.multi.clear();
            invalidate(&mut self.regen);
        } else {
            for &id in &roots {
                if self.project.clone_component(id, target).is_some() {
                    ok += 1;
                } else {
                    fail += 1;
                }
            }
            qymcad_ui_state::mark_dirty_for_rebuild(&mut self.rebuild_ctx()); // the document is marked; the scheduler does the computing
        }
        self.status = if fail == 0 {
            crate::i18n::tr2("g-paste-result", "what", &if cut { crate::i18n::tr("g-body-moved") } else { crate::i18n::tr("g-copied") }, "n", &ok.to_string())
        } else {
            crate::i18n::tr2("g-paste-partial", "ok", &ok.to_string(), "fail", &fail.to_string())
        };
    }




    /// A test facade for "enter a component" (a double click on a part in the tree).
    #[cfg(test)]
    pub(crate) fn enter_ctx_for_test(&mut self, id: Id) {
        self.active_path = vec![self.project.root, id];
        self.project.set_active_component(Some(id));
        self.sync_workbench();
    }

    /// Test facades for the "save?" question: a test must pull the same handles as the menu and the dialogue.
    #[cfg(test)]
    pub(crate) fn open_for_test(&mut self, path: String) {
        crate::gui::io_jobs::spawn_project_load(&mut self.regen, path);
        // READING A FILE GOES THROUGH THE MODAL QUEUE (`self.regen.busy`), not the background one (`self.regen.bg`).
        // Only `wait_bg` used to stand here, and it waited on an EMPTY queue: nobody took the result of
        // the read, the document stayed FACTORY EMPTY, and tests of the form "opened a file and..."
        // checked emptiness and therefore always passed.
        self.drain_busy_for_test();
        self.wait_bg();
        qymcad_ui_state::rebuild_if_dirty(&mut self.rebuild_ctx());
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




    /// The joints panel in a single call — the door for checks that look AT THE FRAME.
    #[cfg(test)]
    pub(crate) fn joints_panel_for_test(&mut self, ui: &mut egui::Ui) {
        let mut asks = Vec::new();
        crate::gui::panels_props::joints_panel(&mut self.props_ctx(&mut asks), ui);
        self.do_props_asks(asks);
    }

    /// THE BUILD TREE in a single call — the same door for checks that look at the frame.
    #[cfg(test)]
    pub(crate) fn build_tree_for_test(&mut self, ui: &mut egui::Ui) {
        let mut asks = Vec::new();
        crate::gui::panels_tree::build_tree(&mut self.tree_ctx(&mut asks), ui);
        self.do_tree_asks(asks);
    }







    #[cfg(test)]
    pub(crate) fn request_nav_for_test(&mut self, nav: Nav) {
        let ctx = egui::Context::default();
        self.request_nav(nav, &ctx);
    }

    /// Test facades for saving: a test must take the same path as Save does.
    #[cfg(test)]
    pub(crate) fn save_for_test(&mut self, path: String) {
        crate::gui::io_jobs::spawn_save(&mut self.disk.io, &mut self.live, &mut self.project, &mut self.regen, &mut self.status, path, false);
        self.wait_bg();
    }

    #[cfg(test)]
    pub(crate) fn autosave_for_test(&mut self, path: String) {
        crate::gui::io_jobs::spawn_save(&mut self.disk.io, &mut self.live, &mut self.project, &mut self.regen, &mut self.status, path, true);
        self.wait_bg();
    }










    #[cfg(test)]
    pub(crate) fn execute_deferred_delete_for_test(&mut self) {
        if let Some(sel) = self.deferred.delete.take() {
            self.execute_delete(sel);
        }
    }









    /// Answer Save in the unsaved-work dialogue — the same as pressing the button.
    #[cfg(test)]
    pub(crate) fn answer_save_for_test(&mut self) {
        self.save_project();
        if crate::gui::io_jobs::saving_now(&self.regen) || self.asking_for_a_file() {
            self.deferred.nav_after_save = true;
        }
    }

    /// Draw the dialogue or the card — the same thing a frame does.
    #[cfg(test)]
    pub(crate) fn nav_dialog_for_test(&mut self, ctx: &egui::Context) {
        { let mut asks = Vec::new(); crate::gui::panels_windows::nav_dialog(&mut self.win_ctx(&mut asks), ctx); self.do_win_asks(asks, ctx); }
    }






    /// How many bodies actually reach the viewport (the same list the CPU raster and the GPU pass draw).
    /// HOW MANY BODIES ARE ON SCREEN. The list itself cannot come back through a method: it borrows out of
    /// the document, and a context built on the stack dies with the call.
    #[cfg(test)]
    pub(crate) fn visible_mesh_count(&self) -> usize {
        let pn = self.painting();
        visible_mesh_items(&pn).len()
    }


    /// A facade for the parameters window: editing a global parameter by the same handle the window uses.
    #[cfg(test)]
    pub(crate) fn set_param_for_test(&mut self, name: &str, expr: &str) {
        use qymcad_core::model::Param;
        match self.project.parameters.iter_mut().find(|p| p.name == name) {
            Some(p) => p.expr = expr.to_string(),
            None => self.project.parameters.push(Param { name: name.to_string(), expr: expr.to_string(), value: 0.0 }),
        }
        { let mut asks = Vec::new(); crate::gui::panels_windows::apply_param_edit(&mut self.win_ctx(&mut asks)); self.do_win_asks(asks, &egui::Context::default()); () };
    }














    // --- Undo and redo (snapshots) ---











    /// OPEN AN OPERATION on the document. Everything that changes the document must go through it.
    /// A nested call joins the operation already open: one action, one undo step.
    pub(crate) fn edit(&mut self, name: impl Into<String>) -> Edit<'_> {
        qymcad_ui_state::edit_over(self.rebuild_ctx(), name)
    }

    pub(crate) fn undo(&mut self) {
        if let Some(step) = self.disk.edits.undo.pop() {
            let cur = std::mem::replace(&mut self.disk.edits.baseline, step.snap.clone());
            self.disk.edits.redo.push(Step { name: step.name.clone(), snap: cur });
            restore(&mut self.rebuild_ctx(), step.snap);
            self.disk.edits.committed_key = doc_key(&self.project);
            self.status = crate::i18n::tr1("g-undone", "what", &step.name);
        }
    }

    fn redo(&mut self) {
        if let Some(step) = self.disk.edits.redo.pop() {
            let cur = std::mem::replace(&mut self.disk.edits.baseline, step.snap.clone());
            self.disk.edits.undo.push(Step { name: step.name.clone(), snap: cur });
            restore(&mut self.rebuild_ctx(), step.snap);
            self.disk.edits.committed_key = doc_key(&self.project);
            self.status = crate::i18n::tr1("g-redone", "what", &step.name);
        }
    }







}

/// WHERE THE PANELS GO. The sizes and the framing that used to be written inside each panel are said here
/// instead - they describe the container, not what is drawn in it.
///
/// Built afresh each frame rather than kept in a field: it is a dozen pushes, and a stored layout is a
/// second copy of this list that would drift from it. What the person MOVED does live on, in the settings,
/// and is laid over the registration.
///
/// FREE, not a method: it reads the settings and nothing else, and `App` is large enough already.
pub(crate) fn shell(set: &Settings) -> qymcad_shell::Shell {
    use qymcad_shell::{Place, Slot};
    let mut s = qymcad_shell::Shell::default();
    s.put(Place::new("menubar", Slot::Menu));
    s.put(Place::new("toolbar", Slot::Top));
    s.put(Place::new("section_bar", Slot::Top).framed());
    s.put(Place::new("comp_array_bar", Slot::Top).framed());
    s.put(Place::new("feat_cmd_bar", Slot::Top).framed());
    s.put(Place::new("sk_tool_opts", Slot::Top).framed());
    s.put(Place::new("bool_tool_bar", Slot::Top).framed());
    s.put(Place::new("joint_tool_bar", Slot::Top).framed());
    s.put(Place::new("joint_edit_bar", Slot::Top).framed());
    s.put(Place::new("status", Slot::Bottom));
    s.put(Place::new("wbtools", Slot::Left).sized(108.0, false).bare());
    s.put(Place::new("tree", Slot::Left).sized(260.0, true));
    s.put(Place::new("props", Slot::Right).sized(290.0, true));
    s.put(Place::new("viewport", Slot::Centre));
    s.restore(set.layout.clone());
    s
}

impl qymcad_shell::Fills for App {
    /// IS THE PLACE WANTED THIS FRAME. Each of these used to be a `return` at the top of the panel - which
    /// worked only while the panel opened its own container. Now the container is opened first, and an empty
    /// one still eats a strip of the window, so the question is asked before opening rather than inside.
    fn live(&self, key: &'static str) -> bool {
        match key {
            "section_bar" => self.side.section.plane.is_some(),
            "comp_array_bar" => self.side.carr.mode != 0,
            "feat_cmd_bar" => self.tools.armed.commanding(),
            "sk_tool_opts" => edit_si(&self.project, &self.sketch_ses).is_some(),
            "bool_tool_bar" => self.params.boolean.pick.is_some(),
            // THE ASSEMBLY BARS: each is one of several by what is being picked, and the shell now owns
            // their places. The conditions used to live at the top of each bar as an early return - which
            // meant the place existed even when nothing was drawn in it.
            "joint_tool_bar" => {
                matches!(self.workbench, Workbench::Assembly)
                    && self.viewing.mode_3d
                    && (self.side.joint.pick_faces || self.side.joint.ground_pick || self.side.joint.group_pick.is_some() || self.side.joint.width_pick.is_some() || self.side.joint.tangent_pick.is_some() || self.side.joint.relation_pick.is_some() || self.side.joint.conn_pick)
            }
            "joint_edit_bar" => self.side.joint.edit.is_some() && matches!(self.workbench, Workbench::Assembly) && self.viewing.mode_3d,
            // while a sketch is being edited the tree on the left is not needed - only the tools
            "tree" => edit_si(&self.project, &self.sketch_ses).is_none(),
            _ => true,
        }
    }

    fn fill(&mut self, key: &'static str, ui: &mut egui::Ui) {
        // WHAT THE BARS ASKED FOR IS DONE AFTER THE DRAWING. A button that acted mid-frame would start a
        // command while the bar of that command was still being laid out.
        let mut bar_asks = Vec::new();
        match key {
            "menubar" => crate::gui::panels_bars::menu_bar(&mut self.bar_ctx(&mut bar_asks), ui),
            "toolbar" => crate::gui::panels_bars::toolbar(&mut self.bar_ctx(&mut bar_asks), ui),
            "section_bar" => crate::gui::panels_bars::section_bar(&mut self.regen, &mut self.side.section, ui),
            "comp_array_bar" => crate::gui::panels_bars::comp_array_bar(&mut self.part_ctx(), ui),
            "feat_cmd_bar" => self.feat_command_bar(ui),
            "sk_tool_opts" => crate::gui::panels_bars::tool_options_bar(&mut self.bar_ctx(&mut bar_asks), ui),
            "bool_tool_bar" => crate::gui::commands::bool_tool_bar(&mut self.part_ctx(), ui),
            "joint_tool_bar" => qymcad_assembly::joint_tool_bar(&mut self.joint_ctx(), ui),
            "joint_edit_bar" => qymcad_assembly::joint_edit_bar(&mut self.joint_ctx(), ui),
            "status" => crate::gui::panels_bars::status_bar(&self.cache, self.cursor, &self.project, &self.scheme, &self.sketch_ses, &self.status, ui),
            "wbtools" => crate::gui::panels_bars::wb_toolbar(&mut self.bar_ctx(&mut bar_asks), ui),
            "tree" => self.tree_panel(ui),
            "props" => self.properties_panel(ui),
            "viewport" => self.viewport(ui),
            _ => {}
        }
        if !bar_asks.is_empty() {
            let ctx = ui.ctx().clone();
            self.do_bar_asks(bar_asks, &ctx);
        }
    }

    fn bar_frame(&self) -> egui::Frame {
        qymcad_ui_state::tool_bar_frame(&self.scheme)
    }
}


impl eframe::App for App {
    /// Save the settings between sessions (the machine plus the view preferences).
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, "settings", &self.set);
        eframe::set_value(storage, "last_project", &self.disk.project_path); // the path of the current document
    }

    /// THE FRAME. Named `ui` rather than `update` since egui 0.36: panels now live inside a `Ui`, and
    /// the framework hands the root one in. The context is still wanted for windows, input and viewport
    /// commands, and it comes from the same place.
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.draw_frame(ui);
    }
}

impl App {
    /// THE WHOLE FRAME, in one method a test can call. `eframe::Frame` is not used by any of it and cannot be
    /// built outside a live window, so the body lives here rather than in the trait: without this the frame -
    /// the one place where the order of the phases is decided - could only be checked by reading it.
    pub(crate) fn draw_frame(&mut self, ui: &mut egui::Ui) {
        let ctx = &ui.ctx().clone();
        // The canvas of this frame, for a problem report: three atomic stores, so it costs nothing at
        // this rate. Taken before the prologue can return - a report is wanted most when the start-up
        // load is what went wrong.
        crate::diagnostics::note_viewport(ctx.viewport_rect().size(), ctx.pixels_per_point());
        self.keep_the_title_current(ctx);
        // THE FRAME PROLOGUE: until it says "carry on", there is nothing to draw (the start-up load is running).
        if !self.frame_prologue(ctx) {
            return;
        }
        // THE ANSWER FROM AN OPEN FILE CHOOSER, picked up at the top of the frame so the file is acted on in
        // the same frame it was chosen in. While one is open the frames are asked for by hand: the answer
        // comes from a thread, and egui, which sleeps between input events, hears nothing of it.
        let choosing = self.poll_file_ask();
        if choosing {
            ctx.request_repaint();
            crate::gui::file_ask::inert_while_choosing(ui); // the system chooser is modal: nothing here answers until it does
        }
        // Ctrl+S saves (silently into the current file, or a dialogue for a new one); Ctrl+Shift+S is "save as".
        if !choosing && !ctx.egui_wants_keyboard_input() && ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::S)) {
            if ctx.input(|i| i.modifiers.shift) {
                self.save_project_as();
            } else {
                self.save_project();
            }
        }
        // the indicator of a background rebuild sits over the ordinary interface, with dimming
        if let Some(label) = self.tools.dim.overlay.take() {
            let progress = self.tools.dim.overlay_progress.take();
            // A LIVE VIEWPORT during a rebuild: the rectangle is taken from the PREVIOUS frame — the
            // overlay is drawn before the viewport itself, and the window size does not jump between frames.
            let live = if self.regen.regen_running() { self.viewing.view_rect } else { egui::Rect::NOTHING };
            if crate::gui::render::draw_dim_overlay_with(&self.scheme, ctx, &label, progress, live) {
                cancel_regen(&self.regen, &mut self.status);
            }
        }
        // A SILENT REBUILD gets only a spinner on the canvas. No window, no dimming, no barrier: nobody is
        // kept from working, and nobody is left guessing whether what is on screen is still current.
        if std::mem::take(&mut self.tools.dim.spinner) {
            crate::gui::render::draw_quiet_spinner(&self.scheme, self.viewing.view_rect, ctx);
        }
        self.sync_workbench(); // the workbench and the active context are derived from `active_path` (drill in and out)
        self.chosen.hover.joint = None; // the hovered joint is rebuilt every frame (by the panel and by 3D below)
        self.keep_selection_on_edited_sketch(); // the selection follows the sketch being edited — in one phase
        // THE KEYBOARD IS THE CHOOSER'S while it is open: a barrier eats clicks, but these two read the
        // input directly and would go on obeying Delete, Escape and every tool letter behind it.
        if !choosing {
            self.handle_key_commands(ctx); // the frame's keyboard commands — in one phase
            self.handle_tool_hotkeys(ctx); // the tool shortcuts (L/R/C/A/P/G/D/S, E)
        }
        self.maybe_autosave(false); // a silent autosave every 3 minutes while there are unsaved edits
        self.help_window(ctx); // the help window
        { let mut asks = Vec::new(); crate::gui::panels_windows::save_template_dialog(&mut self.win_ctx(&mut asks), ctx); self.do_win_asks(asks, ctx); } // "save as a template" — the name and the confirmation
        { let mut asks = Vec::new(); crate::gui::panels_windows::confirm_delete_popup(&mut self.win_ctx(&mut asks), ctx); self.do_win_asks(asks, ctx); } // the popup confirming the deletion of a tree node
        let shell = crate::gui::shell(&self.set);
        shell.run_slot(qymcad_shell::Slot::Menu, ui, self);
        self.take_screenshot(ctx); // the picture of the window for a report comes back as an event
        crate::gui::panels_windows::crash_notice(&mut self.disk.crash_report, ctx); // "the last run ended in an error" - only after a crash
        self.report_window(ctx); // Help -> Report a problem
        crate::gui::panels_windows::about_dialog(&mut self.win, &self.scheme, ctx); // the About window
        { let mut asks = Vec::new(); crate::gui::panels_windows::doc_props_window(&mut self.win_ctx(&mut asks), ctx); self.do_win_asks(asks, ctx); } // the document properties
        self.start_screen(ctx); // where to begin — only on a blank slate
        { let mut asks = Vec::new(); crate::gui::panels_windows::nav_dialog(&mut self.win_ctx(&mut asks), ctx); self.do_win_asks(asks, ctx); } // the modal "save the changes?" over the menu
        { let mut asks = Vec::new(); crate::gui::panels_windows::stl_quality_dialog(&mut self.win_ctx(&mut asks), ctx); self.do_win_asks(asks, ctx); } // choosing the STL quality before exporting
        // "Finish" lives in one place: the button in the breadcrumbs (the toolbar). There is no separate banner.
        tick_view_anim(&mut self.viewing.cam, &mut self.viewing.view_anim, ctx); // the smooth turn of the view (the ViewCube)
        tick_joint_anim(&mut self.joint_anim, &mut self.project, ctx); // sweeping a joint's degree of freedom
        self.hotkeys_window(ctx); // Help -> Shortcuts
        self.command_search_window(ctx); // the command search (Space or Ctrl+K)
        shell.run_slot(qymcad_shell::Slot::Top, ui, self);
        shell.run_slot(qymcad_shell::Slot::Bottom, ui, self);
        shell.run_slot(qymcad_shell::Slot::Left, ui, self);
        shell.run_slot(qymcad_shell::Slot::Right, ui, self);
        { let mut asks = Vec::new(); crate::gui::panels_windows::parts_library_window(&mut self.win_ctx(&mut asks), ctx); self.do_win_asks(asks, ctx); }
        self.save_part_window(ctx);
        { let mut asks = Vec::new(); crate::gui::panels_windows::params_window(&mut self.win_ctx(&mut asks), ctx); self.do_win_asks(asks, ctx); }
        { let mut asks = Vec::new(); crate::gui::panels_windows::settings_window(&mut self.win_ctx(&mut asks), ctx); self.do_win_asks(asks, ctx); }
        shell.run_slot(qymcad_shell::Slot::Centre, ui, self); // last, and now for a stated reason
        // commit an undo step if the edit has finished
        maybe_commit(&mut self.disk.edits, &mut self.tools.place, &self.project, &self.set, ctx);
    }
}

impl App {







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
        let target = parts_insert_target(&mut self.project);
        match self.project.graft(&loaded.project, target) {
            Some(_) => {
                qymcad_ui_state::mark_dirty_for_rebuild(&mut self.rebuild_ctx()); // the document is marked; the scheduler does the computing
                invalidate(&mut self.regen);
                self.status = crate::i18n::tr1("lib-part-inserted", "name", &loaded.manifest.name);
            }
            None => {
                self.status = crate::i18n::tr("lib-no-assembly");
            }
        }
    }


    /// Open the "Save as a standard part" dialogue for component `cid`. The name comes from the component,
    /// the preview is rendered right away (the body in its own frame), and the list of existing categories
    /// is there for picking one quickly.
    fn open_save_part_dialog(&mut self, cid: qymcad_core::model::Id) {
        let name = self.project.components.iter().find(|c| c.id == cid).map(|c| crate::i18n::name(&c.name)).unwrap_or_default();
        let preview = crate::gui::render_scene::render_component_thumbnail(&self.draw_ctx(), cid);
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









    // ============ The splash screen and the progress of background work ============

    /// THE TITLE FOLLOWS THE DOCUMENT: which file is open, and whether it holds unsaved work.
    ///
    /// Sent only when it has changed. A viewport command every frame is a message to the window manager
    /// sixty times a second for a string that moves a few times an hour, and on some of them it makes the
    /// title flicker.
    fn keep_the_title_current(&mut self, ctx: &egui::Context) {
        // THE DIRTY FLAG FIRST: it is a `&mut self` call now (the rebuild moved behind a context), and
        // taking it inside the argument list would borrow `self` twice in one expression.
        let dirty = qymcad_ui_state::is_dirty(&mut self.rebuild_ctx());
        let want = crate::gui::window_title::window_title(self.disk.project_path.as_deref(), dirty);
        if want != self.disk.title_shown {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(want.clone()));
            self.disk.title_shown = want;
        }
    }



    /// Recompute the WHOLE timeline from scratch (the Edit -> Rebuild everything item). Every node is
    /// marked dirty and the work goes into a background regeneration — the screen does not collapse, an
    /// overlay spins instead.
    pub(crate) fn rebuild_everything(&mut self) {
        self.project.mark_all_dirty();
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
















    /// Turn the plane pick for a new sketch on or off (the Sketch button and the K shortcut). It cancels
    /// an active datum or command (`cancel_all_tools` clears the sketch-plane pick, so the target state is
    /// remembered BEFORE the reset), so that two tools are never held at once.
    fn toggle_sketch_pick(&mut self) {
        let turning_on = !self.tools.picking.is_sketch_plane();
        self.cancel_all_tools();
        if turning_on {
            self.tools.picking = Picking::SketchPlane(None);
        }
        if self.tools.picking.is_sketch_plane() {
            self.viewing.mode_3d = true; // planes and faces are picked in 3D
            self.chosen.sel = Sel::None;
            self.status = crate::i18n::tr("g-sketch-pick-plane");
        }
    }












    /// Which gizmo axis is under the cursor (0 = X, 1 = Y, 2 = Z), if it is close enough (within 8 px).
    fn gizmo_axis_hit(&self, comp: Id, rect: Rect, basis: &([f64; 3], [f64; 3], [f64; 3]), pp: Pos2) -> Option<u8> {
        let (o, l) = gizmo_geometry(self.viewing.cam, self.dragged.comp_giz, &self.project, comp);
        gizmo_axis_hit_at(&self.draw_ctx(), o, l, rect, basis, pp)
    }


    /// Dragging a component gizmo's AXIS: accumulate the world shift along the axis (projected onto the
    /// FIXED screen axis) into `comp_giz_drag.amt` and apply it through `apply_comp_giz`. Unified with the
    /// body gizmo.
    fn drag_component_axis(&mut self, _comp: Id, ax: u8, d: egui::Vec2, rect: Rect, basis: &([f64; 3], [f64; 3], [f64; 3])) {
        let Some((_, _, origin, _)) = self.dragged.comp_giz.drag else { return };
        let l = 60.0 / self.viewing.cam.scale as f64;
        let mut tip = origin;
        tip[ax as usize] += l;
        let scr = qymcad_ui_state::Screen { cam: &self.viewing.cam, set: &self.set, rect: rect, basis: basis };
        let s0 = scr.at(origin).0;
        let s1 = scr.at(tip).0;
        let pd = s1 - s0;
        let denom = (pd.x * pd.x + pd.y * pd.y) as f64;
        if denom < 1e-6 {
            return;
        }
        let inc = (d.x * pd.x + d.y * pd.y) as f64 * l / denom;
        if let Some(drag) = &mut self.dragged.comp_giz.drag {
            drag.3 += inc;
        }
        crate::gui::commands::apply_comp_giz(&mut self.part_ctx());
    }



    /// Which rotation ring of the gizmo is under the cursor (0 = X, 1 = Y, 2 = Z), if it is close enough
    /// (within 6 px). The ring of axis `ax` is a circle of radius L in the plane perpendicular to that
    /// axis, around the component's origin.
    fn gizmo_ring_hit(&self, comp: Id, rect: Rect, basis: &([f64; 3], [f64; 3], [f64; 3]), pp: Pos2) -> Option<u8> {
        let (o, l) = gizmo_geometry(self.viewing.cam, self.dragged.comp_giz, &self.project, comp);
        gizmo_ring_hit_at(&self.draw_ctx(), o, l, rect, basis, pp)
    }


    /// Dragging a component gizmo's RING: accumulate the angle (about the FIXED origin) into
    /// `comp_giz_drag.amt` and apply it through `apply_comp_giz` (the rotation is the accumulated one
    /// composed with the start, with snapping). Unified with the body gizmo.
    fn drag_component_ring(&mut self, _comp: Id, ax: u8, cursor: Pos2, d: egui::Vec2, rect: Rect, basis: &([f64; 3], [f64; 3], [f64; 3])) {
        let Some((_, _, origin, _)) = self.dragged.comp_giz.drag else { return };
        let center = qymcad_ui_state::Screen { cam: &self.viewing.cam, set: &self.set, rect: rect, basis: basis }.at(origin).0;
        let radial = cursor - center;
        let r2 = (radial.x * radial.x + radial.y * radial.y) as f64;
        if r2 < 4.0 {
            return;
        }
        let ccw = -(radial.x * d.y - radial.y * d.x) as f64 / r2;
        let sign = ring_drag_sign(basis.2[ax as usize]);
        if let Some(drag) = &mut self.dragged.comp_giz.drag {
            drag.3 += ccw.to_degrees() * sign;
        }
        crate::gui::commands::apply_comp_giz(&mut self.part_ctx());
    }

    // ===== The degree-of-freedom aware gizmo of a driven component. What gets dragged is the JOINT'S
    // FREEDOM (angle, offset, offset2) rather than a free 6-DOF transform, so the drag stays WITHIN the
    // joint and `solve_joints` works the rest out. =====



    // ===== The BODY gizmo in a Part: it reuses `gizmo_*_hit_at` and `draw_gizmo_at`; the movement itself
    // is a parametric Move feature. The drag state is `body_giz_axis`/`ring` plus the accumulated
    // `body_giz_drag`. =====











    /// A click on an axis candidate for a REVOLVE: a STRAIGHT edge of a body, a CYLINDRICAL face, or a
    /// datum axis, each turned into an associative datum. Reported behaviour: the "pick an axis (3D)"
    /// button would not take a datum axis and simply never worked.
    ///
    /// A method of its own, because this branch used to live in the 2D half of the viewport while the
    /// candidates are drawn and hit-tested ONLY in 3D. The button did switch the view to 3D and the axes
    /// did light up, but there was NOBODY there to catch the click: the handler stayed in the flat branch
    /// and was never called. Now the branch is in 3D and the logic sits here, checked by a test without egui.
    fn rev_axis_pick_click(&mut self, rect: Rect, pos: Pos2) -> bool {
        let picked = match crate::gui::pick::pick_axis_at(&self.painting(), rect, pos) {
            Some(AxisHit::Datum(id)) => Some((id, "g-axis-datum")),
            Some(AxisHit::Edge(i)) => axis_from_edge(&self.active_path, &self.edges, &self.live, &mut self.project, i).map(|id| (id, "g-axis-body-edge")),
            Some(AxisHit::Face(b, f)) => axis_from_face(&self.active_path, &self.edges, &self.live, &mut self.project, b, f).map(|id| (id, "g-axis-cyl-face")),
            None => None,
        };
        match picked {
            Some((id, what)) => {
                self.params.rev.axis_datum = id;
                self.params.rev.axis_line = 0; // the axis was set in 3D, so the sketch's axis line no longer applies
                self.params.rev.pick_axis = false;
                self.status = format!("{} {}", ph::CHECK, crate::i18n::tr1("g-rev-axis", "what", &crate::i18n::tr(what)));
                true
            }
            None => {
                self.status = crate::i18n::tr("g-rev-axis-miss");
                false
            }
        }
    }



    /// A CLICK hit (not a drag) on a body gizmo's arrow or ring, giving (mi, axis 0/1/2, is it a rotation?).
    /// None means the click missed the gizmo.
    fn body_gizmo_click_hit(&self, rect: Rect, pos: Pos2, basis: &([f64; 3], [f64; 3], [f64; 3])) -> Option<(usize, u8, bool)> {
        let (_, mi) = body_gizmo_target(qymcad_ui_state::body_view_of!(self), self.chosen.sel)?;
        let (o, l) = body_gizmo_geometry(&self.dragged.body_giz, self.viewing.cam, &self.project, &self.set, mi);
        if let Some(ax) = gizmo_axis_hit_at(&self.draw_ctx(), o, l, rect, basis, pos) {
            return Some((mi, ax, false));
        }
        if let Some(ax) = gizmo_ring_hit_at(&self.draw_ctx(), o, l, rect, basis, pos) {
            return Some((mi, ax, true));
        }
        None
    }






    /// Dragging the face arrow: the offset grows along the face's NORMAL, just as it does for a body gizmo's axis.
    pub(super) fn face_arrow_drag_to(&mut self, d: egui::Vec2, rect: Rect, basis: &([f64; 3], [f64; 3], [f64; 3])) {
        let Some((o, _, n)) = self.face_arrow_geometry() else { return };
        let l = 60.0 / self.viewing.cam.scale as f64;
        let scr = qymcad_ui_state::Screen { cam: &self.viewing.cam, set: &self.set, rect: rect, basis: basis };
        let s0 = scr.at(o).0;
        let s1 = scr.at([o[0] + n[0] * l, o[1] + n[1] * l, o[2] + n[2] * l]).0;
        let pd = s1 - s0;
        let denom = (pd.x * pd.x + pd.y * pd.y) as f64;
        if denom < 1e-6 {
            return;
        }
        let Some(key) = face_arrow_key(&self.tools.armed) else { return };
        let inc = (d.x * pd.x + d.y * pd.y) as f64 * l / denom;
        let cur = qymcad_ui_state::cmd_val(&self.tools.cmd, key) + inc;
        if let Some(p) = self.tools.cmd.params.iter_mut().find(|p| p.key == key) {
            p.val = cur;
            p.txt = format!("{:.2}", cur); // the field and the arrow are one value, not two independent ones
        }
        invalidate(&mut self.regen);
    }



    /// Finish a body gizmo drag: apply the accumulated transform as a PARAMETRIC Move feature.
    fn commit_body_gizmo(&mut self, snap: bool) {
        let accum = body_giz_accum(&self.dragged.body_giz, &self.set, snap);
        let drag = self.dragged.body_giz.drag;
        self.dragged.body_giz.drag = None;
        self.dragged.body_giz.axis = None;
        self.dragged.body_giz.ring = None;
        let (Some(accum), Some((mi, _, amt))) = (accum, drag) else { return };
        if amt.abs() < 1e-6 || qymcad_core::feature::is_identity12(&accum) {
            return; // a zero drag: nothing is committed
        }
        crate::gui::commands::apply_body_move(&mut self.part_ctx(), mi, accum);
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
        if self.tools.armed.commanding() {
            cancel_feat_cmd(&mut self.part_ctx());
        }
        exit_draw_tools(&mut qymcad_ui_state::tools_of!(self)); // every sketch mode, in one transition
        clear_feat_picks(qymcad_ui_state::feat_picks_of!(self)); // the aiming of the feature's subsystems
        self.params.boolean.pick = None;
        self.params.boolean.edit = None;
        // EVERY ASSEMBLY TOOL, THROUGH THE ONE DOOR THAT WALKS `AssemblyTool::ALL`.
        //
        // This used to be nine fields listed here by hand, and it had already fallen behind: the anchor
        // RE-PICK was missing. Taking any other assembly tool while it was armed left TWO in hand, the click
        // went to whichever handler stood higher, and the person was certain they were working with the last
        // one taken. Esc released it (that path always called `drop_assembly_tools`); changing tools did not.
        crate::gui::assembly_tools::drop_assembly_tools(&mut self.side.joint);
        self.side.section.pick = false; // the section pick (the section itself lives until it is switched off)
        self.side.section.drag = false;
        self.side.section.drag_anchor = None;
        self.params.mirror.part = None; // an unfinished pick of the part to mirror
        self.tools.pending_import.clear(); // the whole unfinished import (the curves plus the points)
        self.side.clip.geom_pending = None; // an unfinished copy or paste of geometry
        self.side.clip.geom_place = false;
        self.side.m3.clear(); // the 3D measuring tool is a tool too, exclusive with the rest
        self.side.carr = CompArrayCmd::default(); // an unfinished component pattern
    }


    /// The Part layout: K a new sketch, D a datum plane, E extrude, Q cut, R revolve, F fillet, C chamfer,
    /// H shell, O hole, M mirror, B box, Y cylinder.
    pub(super) fn part_hotkey(&mut self, key: egui::Key) {
        // WE MATCH ON THE ACTION, NOT ON THE KEY. While `Key::E` stood in the `match`, remapping was
        // inexpressible: the letter and the meaning were one and the same thing. Which key leads to which
        // action is decided by `hotkey_action` — one place for every workbench.
        let Some(action) = qymcad_ui_state::hotkey_action(&self.set, "part", key) else { return };
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
            "part.contour-reselect" if matches!(self.tools.armed.cmd_kind(), 1 | 3) && self.tools.cmd.sketch.is_some() && self.viewing.mode_3d => crate::gui::commands::enter_contour_reselect(&mut self.part_ctx()),
            _ => {}
        }
    }

    /// The Assembly layout: K a new skeleton sketch, D a datum plane, N a new part, U a subassembly,
    /// I insert a component (STEP or STL), J a rigid joint (picking faces).
    pub(super) fn assembly_hotkey(&mut self, key: egui::Key) {
        let Some(action) = qymcad_ui_state::hotkey_action(&self.set, "assembly", key) else { return };
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
                self.side.joint.new_kind = qymcad_core::feature::JointKind::Rigid;
                self.side.joint.pick_faces = true;
                self.side.joint.pick_first = None;
                self.status = crate::i18n::tr("g-rigid-joint-hint");
            }
            _ => {}
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
            crate::gui::commands::resync_after_topology_change(&mut self.part_ctx());
        }
        done
    }

    /// TURN THE SECTION VIEW ON OR OFF — one handle for everyone.
    ///
    /// The logic used to live inside the panel's button, and there was no way to reach it by hand: a test
    /// either poked at the fields directly (checking itself, that is) or checked nothing at all. The rule
    /// that a hand must use the same doors a person does applies here too.
    pub(crate) fn toggle_section(&mut self) {
        if self.side.section.plane.is_some() || self.side.section.pick {
            self.side.section.plane = None;
            self.side.section.pick = false;
            self.side.section.drag = false;
            self.side.section.drag_anchor = None;
            invalidate(&mut self.regen);
            self.status = crate::i18n::tr("tb-section-off");
        } else {
            self.cancel_all_tools();
            self.side.section.pick = true;
            self.status = crate::i18n::tr("tb-section-pick");
        }
    }







    /// Create (with target == 0) or re-target the face (target = plane_id) of an "offset from a face" datum plane.
    fn make_offset_plane_from_face(&mut self, target: Id, body: Id, key: qymcad_core::feature::FaceKey) {
        use qymcad_core::model::{PlaneDef, WorkPlane};
        if target == 0 {
            let wp = WorkPlane { name: crate::i18n::tr("plane-from-face"), def: PlaneDef::OffsetFace { body, face: key, dist: 10.0 }, ..Default::default() };
            let id = self.project.add_plane(wp);
            self.chosen.sel = self.project.planes.iter().position(|p| p.id == id).map(Sel::Plane).unwrap_or(Sel::None);
        } else if let Some(pi) = self.project.planes.iter().position(|p| p.id == target) {
            let dist = if let PlaneDef::OffsetFace { dist, .. } = self.project.planes[pi].def { dist } else { 10.0 };
            self.project.planes[pi].def = PlaneDef::OffsetFace { body, face: key, dist };
            self.chosen.sel = Sel::Plane(pi);
        }
        self.regen_after_datum_change();
        self.status = crate::i18n::tr("plane-from-face-hint");
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
        if !self.disk.edits.ready {
            self.disk.edits.baseline = snapshot(&self.project);
            self.disk.edits.committed_key = doc_key(&self.project);
            self.disk.edits.saved_key = edit_key(&self.draw_ctx()); // at start-up (an empty document, or one opened automatically) there are no edits
            self.disk.edits.ready = true;
        }
        // Debouncing a datum edit: while a coordinate or a direction is being dragged (the pointer is held
        // down) the consumers are NOT force-regenerated every frame; the datum's own glyph moves live,
        // because the renderer reads the fields directly. Release the pointer and one regeneration runs at
        // the final position. Done BEFORE the panels, so the new geometry shows in this same frame.
        if self.side.datum.regen_pending && !ctx.input(|i| i.pointer.any_down()) {
            self.side.datum.regen_pending = false;
            self.regen_after_datum_change();
        }
        // Intercepting the closing of the window (the cross, or Alt+F4): with unsaved work, cancel the close and ask.
        if ctx.input(|i| i.viewport().close_requested()) {
            // a background write must reach the disk even when the document is "clean" (Save was pressed and
            // the window closed straight away) — otherwise one is sure it saved while the file stayed as it
            // was (the write is atomic: an interruption does not spoil the old file, but neither does it
            // carry the new edits).
            self.wait_bg();
            if !self.disk.edits.allow_close && qymcad_ui_state::is_dirty(&mut self.rebuild_ctx()) {
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
        match edit_si(&self.project, &self.sketch_ses) {
            Some(si) => {
                self.chosen.sel = Sel::Sketch(si);
                self.workbench = Workbench::Sketch;
            }
            None => {
                if self.sketch_ses.editing.is_some() {
                    self.sketch_ses.editing = None; // the sketch is gone, so leave the mode
                }
            }
        }
        // remember the active sketch (for projecting the part's geometry into it)
        if let Sel::Sketch(si) = self.chosen.sel {
            if let Some(s) = self.project.sketches.get(si) {
                if self.sketch_ses.last != Some(s.id) {
                    self.tools.sel_sk.clear(); // the selection, and whatever was waiting on it: the sketch changed, so the element selection goes
                }
                self.sketch_ses.last = Some(s.id);
            }
        } else {
            self.tools.sel_sk.clear(); // the selection, and whatever was waiting on it
        }
    }










    pub(crate) fn viewport(&mut self, ui: &mut egui::Ui) {
        // The panel lives inside a `Ui` now; the context is still wanted for windows,
        // input and viewport commands, and it comes from the same place.
        let ctx = &ui.ctx().clone();
        let (resp, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        let rect = resp.rect;
        self.viewing.view_rect = rect; // where the canvas stands: the rebuild overlay needs it so as not to blank it
        // THE CANVAS BACKGROUND COMES FROM THE SCHEME. A `from_gray(26)` used to stand here, asking the
        // theme nothing at all: switch to the light one and the viewport and the sketcher stayed black.
        painter.rect_filled(rect, 0.0, self.scheme.pal.viewport_bg());
        let has_geom = !self.project.contours.is_empty() || !self.project.bodies.is_empty();
        let scroll = ctx.input(|i| i.smooth_scroll_delta.y);

        if self.viewing.mode_3d {
            self.viewport_3d(ctx, &resp, &painter, rect, has_geom, scroll);
        } else {
            self.viewport_2d(ctx, &resp, &painter, rect, has_geom, scroll);
        }
    }

































}

// ---------- helpers ----------

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

fn drag(ui: &mut egui::Ui, label: &str, v: &mut f64, speed: f64, range: std::ops::RangeInclusive<f64>) -> bool {
    ui.label(label);
    let changed = ui.add(egui::DragValue::new(v).speed(speed).range(range)).changed();
    ui.end_row();
    changed
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


/// "hh:mm" for the autosave line. UTC on purpose: it says "just now", it is not a wall clock.
fn clock_hh_mm() -> String {
    let t = time::OffsetDateTime::from_unix_timestamp(unix_secs() as i64).unwrap_or(time::OffsetDateTime::UNIX_EPOCH);
    format!("{:02}:{:02}", t.hour(), t.minute())
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





/// The GUI tests live in a file of their own (`gui/tests.rs`) rather than at the end of this one.
/// The file name out of a path (used as the name of a sketch or a source).
pub(crate) fn file_name(path: &str) -> String {
    std::path::Path::new(path).file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| path.to_string())
}

#[cfg(test)]
pub(crate) fn shade_tri_for_test(pal: &crate::palette::Palette, ghost_alpha: u8, hot: bool, ghost: bool, base: [u8; 3], n: [f64; 3], light: [f64; 3]) -> Color32 {
    qymcad_pick::shade_tri(pal, ghost_alpha, hot, ghost, base, n, light)
}

/// Draw a category node of the tree, recursively. `path` holds the indices from the root of the level
/// (mutated by push and pop).
pub(crate) fn parts_tree_node(
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
                    parts_tree_node(ui, c, tier, path, sel);
                    path.pop();
                }
            });
    }
}

/// The category node at a path of indices from the root of the level.
pub(crate) fn cat_at<'a>(root: &'a crate::parts_library::CatNode, path: &[usize]) -> Option<&'a crate::parts_library::CatNode> {
    let mut n = root;
    for &i in path {
        n = n.subcats.get(i)?;
    }
    Some(n)
}

/// Gather every part of the subtree whose name or tags contain `query` (in lower case).
pub(crate) fn collect_matching<'a>(
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
        collect_matching(c, query, out);
    }
}

/// A PNG out of a `ColorImage` (RGBA8) — for the `thumb.png` inside a `.qpart`. Uses `image` with its png feature.
pub(crate) fn color_image_to_png(img: &egui::ColorImage) -> Option<Vec<u8>> {
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

/// The icon of a feature in the tree (by its kind) — shared with the labels of `tree_feature_row` (which shows a custom name).
pub(crate) fn feat_icon(kind: &qymcad_core::feature::FeatureKind) -> &'static str {
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
pub(crate) fn feat_default_name(kind: &qymcad_core::feature::FeatureKind) -> String {
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

/// Embed the original of an imported file into the document and return its Id.
pub(crate) fn embed_source(project: &mut qymcad_core::model::Project, path: &str) -> Option<Id> {
    match std::fs::read(path) {
        Ok(bytes) => Some(project.add_source(file_name(path), bytes)),
        Err(_) => None,
    }
}

fn snapshot(project: &qymcad_core::model::Project) -> Snapshot {
    // THE BYTES OF THE EMBEDDED SOURCES DO NOT GO INTO A SNAPSHOT. They never change (they are the
    // original of a file imported once), and they weigh tens of megabytes: on a real assembly 89 MB
    // times 40 undo steps is gigabytes of memory and about 90 MB of memcpy for EVERY committed edit.
    // Restoring puts them back from the live document by id (see `restore`).
    let mut project = project.clone_without_source_data();
    project.regen_faces.clear(); // derived: the snapshot holds the faces itself (`faces`)
    project.regen_edges.clear();
    Snapshot { project }
}


/// The target assembly for an insertion: the active assembly; if a Part is active, its parent; otherwise the root.
pub(crate) fn parts_insert_target(project: &mut qymcad_core::model::Project) -> qymcad_core::model::Id {
    let root = project.ensure_root();
    let ctx = project.current_ctx();
    if ctx != 0 && !project.component_is_part(ctx) {
        return ctx;
    }
    project
        .components
        .iter()
        .find(|c| c.id == ctx)
        .and_then(|c| c.parent)
        .unwrap_or(root)
}

/// WHAT WILL GO WITH IT — by name, for the question asked before deleting.
///
/// Deletion in the timeline cascades: with a fillet goes everything built on it. This used to be
/// reported by one general line saying "together with its dependants" — true, but no answer to the
/// question "what am I about to lose". The list comes from the same `Project::dependents_of` as the
/// lineage in the properties card: two places must not answer one question differently.
pub(crate) fn delete_cascade_names(project: &qymcad_core::model::Project, sel: Sel) -> Vec<String> {
    let subject = match sel {
        Sel::Feature(ti) => project.timeline.get(ti).map(|n| n.id),
        Sel::Mesh(mi) => project.mesh_id(mi),
        Sel::Sketch(si) => project.sketches.get(si).map(|s| s.id),
        Sel::Plane(i) => project.planes.get(i).map(|p| p.id),
        Sel::DatumPoint(i) => project.datum_points.get(i).map(|d| d.id),
        Sel::DatumAxis(i) => project.datum_axes.get(i).map(|d| d.id),
        _ => None,
    };
    let Some(id) = subject else { return Vec::new() };
    project
        .dependents_of(id)
        .into_iter()
        .filter_map(|nid| project.timeline.iter().find(|n| n.id == nid))
        .map(|n| crate::i18n::name(&n.name))
        .collect()
}

pub(crate) fn sel_delete_label(project: &qymcad_core::model::Project, sel: Sel) -> String {
    match sel {
        Sel::Feature(_) => crate::i18n::tr("del-feature"),
        Sel::Mesh(mi) => crate::i18n::tr1("del-body-named", "name", &crate::i18n::name(&project.mesh_name(mi))),
        Sel::Sketch(si) => project.sketches.get(si).map(|s| crate::i18n::tr1("del-sketch-named", "name", &crate::i18n::name(&s.name))).unwrap_or_else(|| crate::i18n::tr("del-sketch")),
        Sel::Plane(i) => project.planes.get(i).map(|p| crate::i18n::tr1("del-plane-named", "name", &crate::i18n::name(&p.name))).unwrap_or_else(|| crate::i18n::tr("del-plane")),
        Sel::DatumPoint(i) => project.datum_points.get(i).map(|d| crate::i18n::tr1("del-point-named", "name", &crate::i18n::name(&d.name))).unwrap_or_else(|| crate::i18n::tr("del-point")),
        Sel::DatumAxis(i) => project.datum_axes.get(i).map(|d| crate::i18n::tr1("del-axis-named", "name", &crate::i18n::name(&d.name))).unwrap_or_else(|| crate::i18n::tr("del-axis")),
        Sel::Joint(_) => crate::i18n::tr("del-joint"),
        Sel::Component(ci) => project.components.get(ci).map(|c| crate::i18n::tr1("del-part-named", "name", &crate::i18n::name(&c.name))).unwrap_or_else(|| crate::i18n::tr("del-part")),
        _ => crate::i18n::tr("del-item"),
    }
}

pub(super) fn open_stl(regen: &mut Rebuilding, path: String) {
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
    regen.busy = Some(Busy { label: crate::i18n::tr("g-import-stl"), rx, kind: BgKind::ImportShapes, pulse: None, quiet: false });
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
pub(super) fn visibility_changed(regen: &mut Rebuilding) {
    regen.geom_rev = regen.geom_rev.wrapping_add(1);
}

/// WRITE THE SETTINGS TO A FILE — to carry them to another machine, to share them, to attach them to
/// a bug report.
///
/// The format is the same RON as in storage: a separate "export format" would have to be maintained
/// as a second one, and it would drift from the first at the very first new setting.
pub(crate) fn export_settings_to(set: &Settings, path: &str) -> Result<(), String> {
    let text = ron::ser::to_string_pretty(&set, ron::ser::PrettyConfig::default()).map_err(|e| e.to_string())?;
    std::fs::write(path, text).map_err(|e| e.to_string())
}

/// APPLY THE THEME TO `egui`. The one setting for which sitting in the record is not enough: the
/// library does not remember its palette between runs, so it has to be assigned explicitly — at
/// start-up and on every change. The theme used to be assigned ONLY on a click and stored nowhere:
/// pick the light one, restart, and it is dark again.
/// APPLY THE LANGUAGE. An empty setting means "never chosen", and then the system locale decides.
/// The resolution lives in `i18n`, and this only passes it on: two places deciding "which language"
/// would drift apart at the very first edit.
pub(crate) fn apply_language(set: &Settings) {
    let code = if set.language.is_empty() { crate::i18n::system_default() } else { set.language.clone() };
    crate::i18n::set_language(&code);
    // THE HELP LANGUAGE TRAVELS WITH THE INTERFACE LANGUAGE — otherwise "same as the interface" would
    // lie after a switch: the help would stay on the previous one until its own setting was touched.
    crate::help::set_lang(&set.help_lang);
}

/// Put a path at the top of the recent list: no duplicates, trimmed to length.
///
/// The freshest one goes first: the list is read from the top, and "the last thing worked on" must
/// be right under the cursor.
pub(crate) fn remember_recent(set: &mut Settings, path: String) {
    if path.trim().is_empty() {
        return;
    }
    set.recent.retain(|p| p != &path);
    set.recent.insert(0, path);
    let n = set.recent_limit.max(1);
    set.recent.truncate(n);
}

/// Drop a path from the recent list — the file would not open (renamed, moved away, the drive
/// unmounted).
///
/// A dead row must not be left there silently: it would be clicked again and again, while the list
/// exists so that one lands on the file at the first attempt.
pub(crate) fn forget_recent(set: &mut Settings, path: &str) {
    set.recent.retain(|p| p != path);
}

fn workbench_code(workbench: &Workbench) -> &'static str {
    workbench.code()
}

/// Test facades for the language setting: a test must take the same path as the settings window.
#[cfg(test)]
pub(crate) fn settings_language_is_empty(set: &Settings) -> bool {
    set.language.is_empty()
}

/// The look of the egui panels according to the current scheme. Kept apart from [`Self::apply_theme`],
/// because that one REBUILDS the scheme from the list — which, while a custom scheme is being edited
/// live, would wipe out the unsaved work.
pub(super) fn sync_visuals(scheme: &SchemeUi, ctx: &egui::Context) {
    ctx.set_visuals(crate::palette::visuals(&scheme.pal));
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
pub(crate) fn apply_ui_scale(set: &Settings, ctx: &egui::Context) {
    ctx.set_zoom_factor(set.ui_scale.clamp(0.5, 3.0));
}

/// The thumbnail texture of a part for the library grid (`thumb.png` is loaded lazily from the
/// `.qpart`, decoded, turned into a GPU texture, and cached by source). None means there is no preview
/// (an older part with no thumbnail), and the grid shows a placeholder icon. The cache is cleared when
/// the catalogue is rebuilt or something is saved.
pub(super) fn parts_thumb_texture(parts: &mut PartsLibrary, ctx: &egui::Context, src: &crate::parts_library::PartSource) -> Option<egui::TextureHandle> {
    use crate::parts_library::PartSource;
    let key = match src {
        PartSource::Embedded(s) => format!("e:{s}"),
        PartSource::User(p) => format!("u:{}", p.to_string_lossy()),
    };
    if let Some(cached) = parts.thumbs.get(&key) {
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
    parts.thumbs.insert(key, tex.clone());
    tex
}

pub(super) fn open_step(regen: &mut Rebuilding, path: String) {
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
    regen.busy = Some(Busy { label: crate::i18n::tr("io-step-importing"), rx, kind: BgKind::ImportShapes, pulse: None, quiet: false });
}

/// Load the logo lazily (a 256x256 PNG embedded in the binary) into a texture for the splash screen.
pub(super) fn ensure_logo(logo_tex: &mut Option<egui::TextureHandle>, ctx: &egui::Context) {
    if logo_tex.is_some() {
        return;
    }
    const LOGO: &[u8] = include_bytes!("../../../assets/icons/linux/256x256.png");
    if let Ok(img) = image::load_from_memory(LOGO) {
        let rgba = img.to_rgba8();
        let (w, h) = rgba.dimensions();
        let color = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], rgba.as_raw());
        *logo_tex = Some(ctx.load_texture("app-logo", color, egui::TextureOptions::LINEAR));
    }
}

/// The datum's visibility tick in the tree (a stable Id against `datum_hidden`). Shared by planes, points and axes.
pub(super) fn datum_vis_checkbox(datum: &mut DatumCommand, ui: &mut egui::Ui, id: Id) {
    let mut vis = !datum.hidden.contains(&id);
    if ui.add(egui::Checkbox::without_text(&mut vis)).on_hover_text(crate::i18n::tr("datum-vis-hint")).changed() {
        if vis {
            datum.hidden.remove(&id);
        } else {
            datum.hidden.insert(id);
        }
    }
}

/// The title of a tree node: the selected one is highlighted. A method rather than a free function:
/// the selection colour belongs to the scheme, and a function without `&self` cannot ask the scheme —
/// exactly the fault that kept the tool bar from being recoloured by the theme.
pub(super) fn sel_title(scheme: &SchemeUi, text: String, selected: bool) -> egui::RichText {
    let rt = egui::RichText::new(text);
    if selected {
        rt.color(scheme.pal.tree_selected()).strong()
    } else {
        rt
    }
}

/// The local position (in the body's frame) of the vertex at an end of the persistent edge `edge`
/// (`end`: false is the start, true is the end). An initial snapshot for an associative datum point;
/// regeneration refines it through the kernel.
pub(super) fn vertex_local_pos(live: &LiveGeom, body: Id, edge: u32, end: bool) -> Option<[f64; 3]> {
    let shape = live.shapes.get(&body)?;
    let (polys, ids) = shape.edges_with_ids();
    for (poly, id) in polys.iter().zip(ids) {
        if id == edge && poly.len() >= 2 {
            let v = if end { poly[poly.len() - 1] } else { poly[0] };
            return Some([v[0] as f64, v[1] as f64, v[2] as f64]);
        }
    }
    None
}

/// Did the document change outside an operation? (The boundary guard; pulled out so that a test can
/// check it rather than only a live run — that is precisely why the panic at start-up was not caught
/// by the tests.)
#[cfg(test)] // the guard is checked by a test; in production its role is played by the debt record itself
fn doc_changed_outside_edit(edits: &Edits, project: &Project) -> bool {
    edits.open.is_none() && doc_key(project) != edits.committed_key
}

/// A facade: the rebuild overlay WITH A LIVE CANVAS — by the same call the frame makes.
#[cfg(test)]
pub(crate) fn draw_regen_overlay_over(dc: &qymcad_ui_state::DrawCtx, ctx: &egui::Context, live: egui::Rect) -> bool {
    crate::gui::render::draw_dim_overlay_with(dc.scheme, ctx, &crate::i18n::tr("io-rebuilding"), Some((1, 2)), live)
}

// --- Exporting 3D (STEP, STL) and sketches (SVG, DXF) ------------------------------------
/// The VISIBLE bodies of the target: for the whole document, every live body; for a component, the
/// bodies of its subtree (itself plus its descendants). "Visible" means the body's own tick
/// (`mesh_visible[idx]`) AND the owner's hierarchical visibility (`component_chain_visible` up to the
/// root) — a hidden part or subassembly does not reach the export.
pub(crate) fn visible_export_bodies(dc: &qymcad_ui_state::DrawCtx, target: ExportTarget) -> Vec<Id> {
    let subtree: Option<std::collections::HashSet<Id>> = match target {
        ExportTarget::Project => None,
        ExportTarget::Component(cid) => {
            let mut s: std::collections::HashSet<Id> = dc.project.descendants(cid).into_iter().collect();
            s.insert(cid);
            Some(s)
        }
    };
    // CRITICAL: the CONSUMED bodies are excluded (the bases eaten by cuts, booleans and modifiers). In
    // 3D they are hidden by a separate filter, `consumed_bodies`, not by `mesh_visible`, so the export
    // used to pull them in TOGETHER with the final body — the bases covered the cuts, and the STEP or
    // STL came out as a solid blank with nothing cut away.
    let consumed = dc.project.consumed_bodies();
    dc.project
        .bodies
        .iter()
        .map(|b| b.id)
        .enumerate()
        .filter(|&(mi, _)| dc.project.bodies.get(mi).is_none_or(|b| b.visible))
        .filter(|&(_, b)| !consumed.contains(&b))
        .filter(|&(_, b)| dc.project.body_owner(b).is_none_or(|o| component_chain_visible(dc.project, o, None)))
        .filter(|&(_, b)| match &subtree {
            None => true,
            Some(s) => dc.project.body_owner(b).is_some_and(|o| s.contains(&o)),
        })
        .map(|(_, b)| b)
        .collect()
}

/// The axis of a gizmo with origin `o` and length `l` under the cursor — shared code for the COMPONENT
/// gizmo (Assembly) and the BODY gizmo (Part). The axes are the world X, Y and Z from `o`.
pub(crate) fn gizmo_axis_hit_at(dc: &qymcad_ui_state::DrawCtx, o: [f64; 3], l: f64, rect: Rect, basis: &([f64; 3], [f64; 3], [f64; 3]), pp: Pos2) -> Option<u8> {
    let scr = qymcad_ui_state::Screen { cam: dc.cam, set: dc.set, rect: rect, basis: basis };
    let s0 = scr.at(o).0;
    let mut best: Option<(f32, u8)> = None;
    for ax in 0..3u8 {
        let mut tip = o;
        tip[ax as usize] += l;
        let s1 = scr.at(tip).0;
        let d = screen_dist_seg(pp, s0, s1);
        if d <= 13.0 && best.is_none_or(|(bd, _)| d < bd) {
            best = Some((d, ax));
        }
    }
    best.map(|(_, a)| a)
}

/// The rotation ring of a gizmo with origin `o` and radius `l` under the cursor — shared code for the
/// Assembly and the Part.
pub(crate) fn gizmo_ring_hit_at(dc: &qymcad_ui_state::DrawCtx, o: [f64; 3], l: f64, rect: Rect, basis: &([f64; 3], [f64; 3], [f64; 3]), pp: Pos2) -> Option<u8> {
    let mut best: Option<(f32, u8)> = None;
    for ax in 0..3u8 {
        let (u, v) = ring_axes(ax);
        let mut prev: Option<Pos2> = None;
        let mut dmin = f32::MAX;
        for k in 0..=48 {
            let a = k as f64 / 48.0 * std::f64::consts::TAU;
            let p = [o[0] + l * (u[0] * a.cos() + v[0] * a.sin()), o[1] + l * (u[1] * a.cos() + v[1] * a.sin()), o[2] + l * (u[2] * a.cos() + v[2] * a.sin())];
            let s = qymcad_ui_state::Screen { cam: dc.cam, set: dc.set, rect: rect, basis: basis }.at(p).0;
            if let Some(pr) = prev {
                dmin = dmin.min(screen_dist_seg(pp, pr, s));
            }
            prev = Some(s);
        }
        if dmin <= 10.0 && best.is_none_or(|(bd, _)| dmin < bd) {
            best = Some((dmin, ax));
        }
    }
    best.map(|(_, a)| a)
}

pub(crate) fn fit3d(cam: &mut Cam3, project: &Project, rect: Rect) {
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
    for m in project.bodies.iter().map(|b| &b.mesh) {
        for v in &m.verts {
            acc([v.x, v.y, v.z]);
        }
    }
    for c in project.contours.iter() {
        for p in &c.points {
            acc([p.x, p.y, 0.0]);
        }
    }
    if !mn[0].is_finite() {
        return;
    }
    cam.target = [(mn[0] + mx[0]) / 2.0, (mn[1] + mx[1]) / 2.0, (mn[2] + mx[2]) / 2.0];
    let ext = (mx[0] - mn[0]).max(mx[1] - mn[1]).max(mx[2] - mn[2]).max(1.0) as f32;
    cam.scale = (rect.width().min(rect.height()) / ext) * 0.55;
    cam.init = true;
}

/// ONE STEP OF THE SWEEP: move the degree of freedom and re-solve the assembly.
///
/// It travels there and back rather than round in a circle: a travel or an angle with limits has no
/// circle to it, and jumping from one end to the other would read as a jerk rather than as a mechanism
/// moving.
pub(crate) fn step_joint_anim(joint_anim: &mut Option<JointAnim>, project: &mut Project, dt: f64) {
    let Some(a) = joint_anim.as_mut() else { return };
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
    project.set_joint_drive(joint, slot, Some(v));
    project.solve_joints();
}

/// Advance the turning animation. Called every frame; while it runs it asks for a repaint.

pub(crate) fn tick_view_anim(cam: &mut Cam3, view_anim: &mut Option<ViewTurn>, ctx: &egui::Context) {
    let Some(ViewTurn { from, to, since: t0 }) = view_anim else { return };
    const DUR: f32 = 0.22;
    let t = (t0.elapsed().as_secs_f32() / DUR).clamp(0.0, 1.0);
    // easing in and out: a linear turn looks mechanical
    let e = (t * t * (3.0 - 2.0 * t)) as f64;
    cam.yaw = from.0 + (to.0 - from.0) * e;
    cam.pitch = from.1 + (to.1 - from.1) * e;
    if t >= 1.0 {
        *view_anim = None;
        cam.init = false; // on arrival refit the scale and the target, as the instant jump used to
    } else {
        ctx.request_repaint();
    }
}

/// The faces of the ViewCube: (normal, four screen points, depth) in the corner of the viewport.
/// TURN THE VIEW SMOOTHLY to the given angles (about 220 ms). An instant jump reads as something having
/// broken: the eye has nothing to hold on to, and on a complex assembly the part has to be found again.
pub(crate) fn animate_view_to(cam: Cam3, mode_3d: &mut bool, view_anim: &mut Option<ViewTurn>, yaw: f64, pitch: f64) {
    *mode_3d = true;
    // THE SHORTEST WAY ROUND IN YAW: without normalisation a turn from -170 deg to +170 deg would go
    // all the way round — 340 deg instead of 20.
    let mut from_yaw = cam.yaw;
    while yaw - from_yaw > std::f64::consts::PI {
        from_yaw += std::f64::consts::TAU;
    }
    while yaw - from_yaw < -std::f64::consts::PI {
        from_yaw -= std::f64::consts::TAU;
    }
    *view_anim = Some(ViewTurn { from: (from_yaw, cam.pitch), to: (yaw, pitch), since: std::time::Instant::now() });
}

/// The properties of a datum POINT: its name, its coordinates and a delete button. Parametric
/// definitions live elsewhere.
pub(crate) fn datum_point_props(datum: &mut DatumCommand, deferred: &mut DeferredUi, project: &mut Project, ui: &mut egui::Ui, i: usize) {
    let lin = crate::gui::props_card::lineage_of(project, Some(project.datum_points[i].id));
    if let Some(n) = props_header(ui, ph::DOT, "datum-point-title", NameSlot::Editable(project.datum_points[i].name.clone()), &lin) {
        project.datum_points[i].name = n;
    }
    let mut changed = false;
    {
        let d = &mut project.datum_points[i];
        ui.label(crate::i18n::tr("datum-point-coords"));
        egui::Grid::new(("dpt", i)).num_columns(2).spacing([6.0, 3.0]).show(ui, |ui| {
            changed |= drag(ui, "X", &mut d.at[0], 1.0, -100000.0..=100000.0);
            changed |= drag(ui, "Y", &mut d.at[1], 1.0, -100000.0..=100000.0);
            changed |= drag(ui, "Z", &mut d.at[2], 1.0, -100000.0..=100000.0);
        });
    }
    if changed {
        datum.regen_pending = true; // the point moved, so an axis through two points and the bodies need rebuilding (debounced)
    }
    ui.separator();
    if ui.button(format!("{} {}", ph::TRASH, crate::i18n::tr("datum-point-delete"))).clicked() {
        ask_delete(deferred, Sel::DatumPoint(i));
    }
}

pub(crate) fn face_props(project: &mut Project, ui: &mut egui::Ui, mi: usize, fi: usize) {
    // INFORMATION ABOUT THE FACE, and nothing else. A face is only selected under the Shell or Hole
    // command (which is where this panel shows); the older buttons (sketch, plane, contour, projection,
    // Z0) are gone — a sketch on a face is made with the Sketch tool on the toolbar (a click on the
    // face), and a datum by the datum command.
    let Some(face) = project.bodies.get(mi).and_then(|b| b.faces.get(fi)) else { return };
    let n = face.normal;
    // THE LINEAGE IS TAKEN FROM THE BODY: a face has no Id of its own in the document, but "what made
    // the thing this face lies on" is exactly the question the face's properties are opened for.
    let lin = crate::gui::props_card::lineage_of(project, project.mesh_id(mi));
    props_header(ui, ph::SQUARE_HALF, "face-props-title", NameSlot::None, &lin);
    ui.label(crate::i18n::tr1("face-area", "v", &format!("{:.1}", face.area)));
    ui.label(crate::i18n::trn("face-normal", &[("n", &format!("[{}, {}, {}]", crate::i18n::num(n[0],2), crate::i18n::num(n[1],2), crate::i18n::num(n[2],2))), ("side", &crate::i18n::tr(normal_label(n)))]));
    ui.label(crate::i18n::tr1("face-center", "v", &format!("{:.1}, {:.1}, {:.1}", face.centroid.x, face.centroid.y, face.centroid.z)));
    ui.label(egui::RichText::new(crate::i18n::tr("face-picked-for-cmd")).weak().small());
}

/// The world (origin, dir) of the axis object caught by a click pick (an edge, a cylindrical face or a
/// datum axis) — WITHOUT creating anything. One resolver for both the preview and the creation (of a
/// pattern and of a datum axis).
pub(crate) fn axis_ref_world(active_path: &[Id], edges: &EdgeCache, live: &LiveGeom, project: &Project, hit: AxisHit) -> Option<([f64; 3], [f64; 3])> {
    use qymcad_core::feature::{apply12, is_identity12};
    let ctx = current_ctx_id(active_path, project);
    match hit {
        AxisHit::Datum(id) => project.datum_axes.iter().find(|d| d.id == id).map(|d| (d.origin(), d.dir())),
        AxisHit::Edge(i) => {
            let (body, _id, poly) = edges.axes.get(i)?;
            if poly.len() < 2 {
                return None;
            }
            let wt = project.body_display_transform(*body, ctx);
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
            let (lo, ld) = live.shapes.get(&body)?.face_axis(fid)?;
            let wt = project.body_display_transform(body, ctx);
            let o = if is_identity12(&wt) { lo } else { apply12(&wt, lo) };
            let z = if is_identity12(&wt) { [0.0; 3] } else { apply12(&wt, [0.0, 0.0, 0.0]) };
            let d = if is_identity12(&wt) { ld } else { [apply12(&wt, ld)[0] - z[0], apply12(&wt, ld)[1] - z[1], apply12(&wt, ld)[2] - z[2]] };
            let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
            (l > 1e-9).then(|| (o, [d[0] / l, d[1] / l, d[2] / l]))
        }
    }
}

pub(crate) fn mesh_props(deferred: &mut DeferredUi, project: &mut Project, regen: &mut Rebuilding, ui: &mut egui::Ui, i: usize) {
    // The name in the header is READABLE rather than a catalogue key, and only a name that was touched
    // gets written back; otherwise merely opening the properties would freeze the automatic name in the
    // current language (that is what `NameSlot::Editable` takes care of).
    let lin = crate::gui::props_card::lineage_of(project, project.mesh_id(i));
    if let Some(n) = props_header(ui, ph::CUBE, "mesh-props-title", NameSlot::Editable(project.mesh_name(i)), &lin) {
        project.set_mesh_name(i, n);
    }
    // the part's colour (so an assembly reads clearly)
    ui.horizontal(|ui| {
        ui.label(crate::i18n::tr("mesh-colour"));
        let mut rgb = project.mesh_color(i);
        if ui.color_edit_button_srgb(&mut rgb).changed() {
            project.set_mesh_color(i, rgb);
            regen.geom_rev = regen.geom_rev.wrapping_add(1); // redraw the 3D view
        }
        if ui.small_button(crate::i18n::tr("mesh-colour-reset")).on_hover_text(crate::i18n::tr("mesh-colour-reset-hint")).clicked() {
            project.reset_mesh_color(i); // drop the manual colour, back to the palette keyed by the lineage root
            regen.geom_rev = regen.geom_rev.wrapping_add(1);
        }
    });
    // INFORMATION, and only what is relevant. A body's size and position change PARAMETRICALLY, through
    // features in the timeline; moving and rotating go through the three-axis gizmo in 3D; booleans and
    // patterns are commands on the toolbar, and they are features too.
    ui.separator();
    if let Some(b) = project.bodies[i].mesh.bounds() {
        ui.label(crate::i18n::tr1("mesh-size", "v", &format!("{:.1} × {:.1} × {:.1}", b.max.x - b.min.x, b.max.y - b.min.y, b.max.z - b.min.z)));
        ui.label(egui::RichText::new(crate::i18n::tr1("mesh-tris-n", "n", &project.bodies[i].mesh.tris.len().to_string())).weak().small());
    } else {
        ui.label(egui::RichText::new(crate::i18n::tr("mesh-empty")).weak());
    }
    ui.separator();
    if ui.button(format!("{} {}", ph::TRASH, crate::i18n::tr("mesh-delete"))).clicked() {
        ask_delete(deferred, Sel::Mesh(i));
    }
}

/// Enter the joint EDITING mode (a double click on its glyph): the top parameter bar plus the popup of
/// anchors A and B. The competing picking and creating modes are cleared and the joint is selected, so
/// its degree-of-freedom gizmo is visible.
pub(crate) fn enter_joint_edit(joint: &mut JointCommand, sel: &mut Sel, status: &mut String, jid: Id) {
    joint.edit = Some(jid);
    joint.edit_repick = None;
    joint.pick_faces = false;
    joint.pick_first = None;
    joint.ground_pick = false;
    *sel = Sel::Joint(jid);
    *status = crate::i18n::tr("g-joint-edit-hint");
}

/// Write a `.qpart` out of the open dialogue — `subproject_of(component)` plus `save_part`. Returns the path.
pub(crate) fn commit_save_part(parts: &mut PartsLibrary, project: &mut Project, tex_graveyard: &mut Vec<egui::TextureHandle>) -> Result<String, String> {
    let d = parts.save.as_ref().ok_or(&crate::i18n::tr("lib-no-dialog"))?;
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
    let sub = project.subproject_of(d.component).ok_or(&crate::i18n::tr("lib-no-root"))?;
    let tags: Vec<String> = d.tags.split([',', ';', '\n']).flat_map(|s| s.split_whitespace()).map(str::trim).filter(|s| !s.is_empty()).map(String::from).collect();
    let png = d.preview.as_ref().and_then(color_image_to_png);
    let manifest = qymcad_core::part::PartManifest { schema_version: qymcad_core::part::PART_SCHEMA, name, description: d.description.trim().to_string(), tags, author: String::new() };
    qymcad_io::save_part(&sub, &manifest, &[], png.as_deref(), &path.to_string_lossy())?;
    // the catalogue is re-read, so the new part appears in the "My parts" tree at once
    parts.tree = Some(crate::parts_library::load_library_tree());
    // the old thumbnails go to the graveyard (freed later); textures must NOT be dropped in the current frame
    tex_graveyard.extend(parts.thumbs.drain().filter_map(|(_, v)| v));
    Ok(path.to_string_lossy().to_string())
}

/// Commit an undo step once the edit has settled (the pointer is released, nothing is being typed).
/// A drag is coalesced into one step (committed only when it ends).
pub(crate) fn maybe_commit(edits: &mut Edits, place: &mut Placing, project: &Project, set: &Settings, ctx: &egui::Context) {
    // A dimension flying after the cursor (`placing_dim`), and typing the dimensions of a rectangle,
    // polygon or ellipse, are edits NOT YET finished: the offset or the value changes every frame, and
    // committing is not allowed (otherwise undo would step through the intermediate states instead of
    // removing the dimension or the shape as a whole). These states are cleared by any new action of
    // the tool, so they do not get stuck.
    let placing = place.dim.is_some() || place.active();
    let busy = ctx.egui_is_using_pointer() || ctx.egui_wants_keyboard_input() || placing || edits.open.is_some();
    if busy {
        return;
    }
    let k = doc_key(project);
    if k == edits.committed_key {
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
        if edits.debt.insert(site.clone()) {
            eprintln!("[operation boundary] the document was changed outside App::edit: {site}");
        }
    }
    let snap = snapshot(project);
    let cur = std::mem::replace(&mut edits.baseline, snap);
    edits.undo.push(Step { name: crate::i18n::tr("undo-edit"), snap: cur });
    if edits.undo.len() > set.undo_cap.max(1) {
        edits.undo.remove(0);
    }
    edits.redo.clear();
    edits.committed_key = k;
}

/// Re-read the schemes from disk (at start-up and after a custom one is edited).
pub(crate) fn reload_schemes(scheme: &mut SchemeUi, status: &mut String) {
    let (all, errs) = crate::palette::all();
    scheme.all = all;
    if !errs.is_empty() {
        *status = crate::i18n::tr1("scheme-load-failed", "error", &errs.join("; "));
    }
}

/// THE DOCUMENT PATH HAS BECOME THE CURRENT ONE — AND WENT STRAIGHT INTO THE RECENT LIST.
///
/// One point for both events, deliberately: "opened" or "saved as" and "remember it" are one action,
/// and spread across the call sites, forgetting the second would only be a matter of time. A guard
/// (`recent_files.rs`) keeps a direct assignment to `project_path` out of the working code.
pub(crate) fn set_project_path(project_path: &mut Option<String>, set: &mut Settings, path: String) {
    *project_path = Some(path.clone());
    remember_recent(set, path);
    // A crash hook cannot reach the document, so it keeps the PATHS instead and the next start
    // offers them back. Told here, at the one point where the path can change.
    crate::crash::note_document(project_path.as_deref(), Some(&autosave_path(project_path)));
}

/// OPEN A RECENT FILE. If it is gone, say so and DROP it from the list.
///
/// A dead row must not be left there silently: the list exists so that one lands on the file at the
/// first attempt, and a row where nothing happens on a click reads as a broken program.
pub(crate) fn open_recent(regen: &mut Rebuilding, set: &mut Settings, status: &mut String, path: String) {
    if !std::path::Path::new(&path).exists() {
        *status = crate::i18n::tr1("recent-missing", "path", &path);
        forget_recent(set, &path);
        return;
    }
    crate::gui::io_jobs::spawn_project_load(regen, path);
}

/// Whether a multiple selection of components is active: the set holds more than one node AND the
/// current `sel` is a component from that set. Otherwise the set counts as stale and clears itself: a
/// click on a body or a sketch puts the multiple selection out without any explicit clean-up.
pub(crate) fn is_multi(project: &Project, sel: Sel, tree_sel: &TreeSelection) -> bool {
    tree_sel.multi.len() > 1
        && matches!(sel, Sel::Component(ci) if project.components.get(ci).map(|c| tree_sel.multi.contains(&c.id)).unwrap_or(false))
}

/// ASK THE REBUILD TO STOP.
///
/// To ask, precisely: the thread will reach the boundary of the next node and come back by itself.
/// There is nothing to kill it with in the middle of an OCCT boolean, and no reason to — a COPY is
/// what is being computed, and the document on screen is intact. `finish_regen_checked` sums it up
/// when the result arrives marked as cancelled.
pub(crate) fn cancel_regen(regen: &Rebuilding, status: &mut String) {
    if let Some(p) = regen.busy.as_ref().and_then(|b| b.pulse.as_ref()) {
        p.ask_stop();
        *status = crate::i18n::tr("io-rebuild-cancelling");
    }
}

/// Show or hide a component in 3D. Visibility is HIERARCHICAL: the flag is stored on the component
/// itself (`c.visible`), and a body's visibility is computed along the chain of its owners' ticks
/// (`component_chain_visible`). It does NOT cascade into `mesh_visible` — otherwise entering a hidden
/// subassembly would show everything hidden despite the ticks of the child parts being on
/// (`mesh_visible` means hiding an individual body by hand, and nothing else).
pub(crate) fn set_component_visible(project: &mut Project, regen: &mut Rebuilding, cid: Id, vis: bool) {
    project.set_component_visible(cid, vis);
    visibility_changed(regen);
}

/// Resolve the PLACEMENT plane (a shared step for a new sketch AND for an import).
/// A sketch on a face of ANOTHER component's body is a LIVE external reference (top-down): an
/// `ExternalRef` is registered and `Face(body, key)` is kept, so the sketch's frame resolves into the
/// consumer's local space on EVERY regeneration and travels with the neighbour's face (editing the
/// neighbour drives the part — exactly what in-context mode exists for). This used to take a ONE-OFF
/// snapshot of the face into a fixed datum — the part stayed free, but the top-down associativity was
/// lost silently. To break the link (freezing the geometry as a snapshot), use the part's properties
/// and its external references.
pub(crate) fn resolve_placement_plane(cmd: &mut FeatCommand, project: &mut Project, status: &mut String, plane: qymcad_core::feature::SketchPlane) -> qymcad_core::feature::SketchPlane {
    cmd.ref_body = None; // reset on every new placement
    if let qymcad_core::feature::SketchPlane::Face(body, key) = plane {
        let consumer = project.active_ctx();
        if project.body_owner(body).is_some_and(|bo| bo != consumer) {
            project.add_external_face_ref(consumer, body, key); // authorise the cross-reference (otherwise regeneration isolation blocks it)
            // the source is remembered for the session: to highlight its edges and snap to them
            cmd.ref_body = Some(body);
            let src = project.body_owner(body).and_then(|o| project.components.iter().find(|c| c.id == o)).map(|c| crate::i18n::name(&c.name)).unwrap_or_else(|| crate::i18n::tr("g-neighbour"));
            *status = crate::i18n::tr1("g-sketch-on-foreign-face", "name", &src);
        }
    }
    plane
}

/// Save the CURRENT document as a template.
pub(crate) fn save_as_template(project: &Project, status: &mut String, title: &str) {
    match crate::templates::save(project, title) {
        Ok(path) => *status = crate::i18n::tr1("tpl-saved", "path", &path),
        Err(e) => *status = format!("{} {}", ph::WARNING, crate::i18n::tr1("tpl-save-failed", "error", &e)),
    }
}

/// Add bodies (a mesh plus B-rep faces) to the document ADDITIVELY (each body becomes its own part
/// in the tree, which can be selected and hidden). The imported group is laid with its top at Z=0,
/// keeping the bodies' positions relative to each other.
pub(crate) fn add_bodies(cam: &mut Cam3, live: &mut LiveGeom, project: &mut Project, regen: &mut Rebuilding, sel: &mut Sel, view: &mut View2d, bodies: Vec<(qymcad_core::geom::Mesh, Vec<qymcad_core::geom::MeshFace>)>) {
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
        let bid = project.add_mesh(m);
        project.imported_bodies.insert(bid); // an import is a valid body with no node (prune leaves it alone)
        live.faces.insert(bid, fs.clone()); // the face cache keyed by body Id (for quick access)
        project.set_body_faces(bid, fs);
        if first.is_none() {
            first = Some(project.bodies.len() - 1);
        }
    }
    if let Some(i) = first {
        *sel = Sel::Mesh(i);
    }
    invalidate(regen);
    view.initialized = false;
    cam.init = false;
}


/// Advance the degree-of-freedom sweep. Called every frame; while it runs it asks for a repaint.
pub(crate) fn tick_joint_anim(joint_anim: &mut Option<JointAnim>, project: &mut Project, ctx: &egui::Context) {
    if joint_anim.is_none() {
        return;
    }
    // THE TIME COMES FROM THE FRAME rather than from counting frames: on a slow machine a frame count
    // would stretch the travel, and "two seconds" would turn into who knows how many.
    let dt = ctx.input(|i| i.stable_dt).clamp(0.0, 0.1) as f64;
    step_joint_anim(joint_anim, project, dt);
    ctx.request_repaint();
}

/// Dragging a body gizmo's ring: accumulate the turn, in degrees, into the gizmo's own drag record.
///
/// WHICH BODY AND WHICH RING are read from the gizmo, not passed in. They used to be two more arguments,
/// filled at the call site out of the very same two fields - a second path by which they could disagree.
pub(crate) fn body_gizmo_ring_drag(body_giz: &mut BodyGizmo, scr: &Screen, project: &Project, regen: &mut Rebuilding, cursor: Pos2, d: egui::Vec2) {
    let (Some((mi, _, _)), Some(ax)) = (body_giz.drag, body_giz.ring) else { return };
    let (o, _) = body_gizmo_geometry(body_giz, *scr.cam, project, scr.set, mi);
    let center = scr.at(o).0;
    let radial = cursor - center;
    let r2 = (radial.x * radial.x + radial.y * radial.y) as f64;
    if r2 < 4.0 {
        return;
    }
    let ccw = -(radial.x * d.y - radial.y * d.x) as f64 / r2;
    let sign = ring_drag_sign(scr.basis.2[ax as usize]);
    if let Some(drag) = &mut body_giz.drag {
        drag.2 += ccw.to_degrees() * sign;
    }
    invalidate(regen);
}

/// Dragging a body gizmo's axis: accumulate the world shift along the grabbed axis into the gizmo's own
/// drag record. Which body and which axis are read from the gizmo, as above.
pub(crate) fn body_gizmo_axis_drag(body_giz: &mut BodyGizmo, scr: &Screen, project: &Project, regen: &mut Rebuilding, d: egui::Vec2) {
    let (Some((mi, _, _)), Some(ax)) = (body_giz.drag, body_giz.axis) else { return };
    let (o, l) = body_gizmo_geometry(body_giz, *scr.cam, project, scr.set, mi);
    let mut tip = o;
    tip[ax as usize] += l;
    let s0 = scr.at(o).0;
    let s1 = scr.at(tip).0;
    let pd = s1 - s0;
    let denom = (pd.x * pd.x + pd.y * pd.y) as f64;
    if denom < 1e-6 {
        return;
    }
    let inc = (d.x * pd.x + d.y * pd.y) as f64 * l / denom;
    if let Some(drag) = &mut body_giz.drag {
        drag.2 += inc;
    }
    invalidate(regen);
}

/// Create a pattern's datum AXIS from the straight edge `axis_edges[i]` — ASSOCIATIVELY, so it travels with the edge.
pub(crate) fn axis_from_edge(active_path: &[Id], edges: &EdgeCache, live: &LiveGeom, project: &mut Project, i: usize) -> Option<Id> {
    let &(body, edge, _) = edges.axes.get(i)?;
    axis_ref_world(active_path, edges, live, project, AxisHit::Edge(i))?; // check the geometry
    Some(project.add_axis_from_edge(body, edge))
}

/// Create a pattern's datum AXIS from the axis of a cylindrical or conical face — ASSOCIATIVELY, so it travels with the face.
pub(crate) fn axis_from_face(active_path: &[Id], edges: &EdgeCache, live: &LiveGeom, project: &mut Project, body: Id, face_id: u32) -> Option<Id> {
    axis_ref_world(active_path, edges, live, project, AxisHit::Face(body, face_id))?; // check that there is an axis at all
    Some(project.add_axis_from_face(body, face_id))
}

/// ADOPT A WHOLE SET OF SETTINGS — the only path by which they take effect.
///
/// The record alone is not enough: the theme, the language and the interface scale live not only in
/// it but also in `egui`'s state, which is not remembered between runs. While this was done as a
/// list of calls at start-up, the second path (importing a profile) would have had to repeat that
/// same list — and would have drifted from it at the very first new setting, silently at that: the
/// record is right while the screen shows the old thing.
pub(crate) fn adopt_settings(regen: &mut Rebuilding, scheme: &mut SchemeUi, set: &mut Settings, status: &mut String, s: Settings, ctx: &egui::Context) {
    *set = s;
    // CUSTOM SCHEMES COME FROM DISK BEFORE THE THEME IS APPLIED: otherwise the chosen custom scheme
    // will not be found in the list and will silently fall back to the dark one.
    reload_schemes(scheme, status);
    apply_theme(scheme, set, ctx);
    apply_language(set);
    apply_ui_scale(set, ctx);
    invalidate(regen); // the colours and the scale are part of the picture caches' keys
}

/// Recompute the pairs of bodies that INTERPENETRATE (interference) — lazily, while idle. Bodies of
/// different components that are visible in the context are transformed into the context's frame ONCE,
/// and then intersected pairwise for a common volume. It is expensive, so it runs only while
/// `show_interference` is on, the scene is not being dragged, and the cache is stale.
/// TAKE THE "MIRROR A WHOLE PART" TOOL, in the order that works: release the others FIRST.
///
/// The button used to push the release and set the field straight after. The release is carried out
/// AFTER the frame, so it cleared the field again and the button did nothing - the same shape as the
/// ruler that switched the measuring tool on and off within one frame.
///
/// Beside the application rather than on it: nothing here needs to see the whole of it.
fn mirror_part_door(app: &mut App, comp: Id) {
    app.cancel_all_tools();
    app.params.mirror.part = Some(comp);
    app.status = crate::i18n::tr("tb-mirror-pick-plane");
}

pub(crate) fn refresh_interference(bv: qymcad_ui_state::BodyView, drag: qymcad_ui_state::SceneDrag, interference: &mut Interference, live: &LiveGeom, set: &Settings, workbench: Workbench) {
    if !set.show_interference || !matches!(workbench, Workbench::Assembly) {
        if !interference.pairs.is_empty() {
            interference.pairs.clear();
        }
        interference.rev = u64::MAX;
        return;
    }
    // the scene is moving (a gizmo drag), or the cache is fresh: nothing to recompute
    if qymcad_assembly::joint_drag_active(drag.joint, drag.part_pull) || drag.comp_giz.drag.is_some() || drag.body_giz.drag.is_some() {
        return;
    }
    if interference.rev == bv.regen.geom_rev {
        return;
    }
    interference.rev = bv.regen.geom_rev;
    let ctx = current_ctx_id(bv.active_path, bv.project);
    // the visible bodies that have an owning component become shapes in the context's frame (transformed once)
    let mut placed: Vec<(Id, Id, qymcad_kernel::Shape)> = Vec::new();
    for mi in 0..bv.project.bodies.len() {
        if !body_shown(bv, mi) {
            continue;
        }
        let Some(body) = bv.project.mesh_id(mi) else { continue };
        let Some(owner) = bv.project.body_owner(body) else { continue };
        let wt = bv.project.body_display_transform(body, ctx);
        if let Some(ws) = live.shapes.get(&body).and_then(|s| s.transformed(&wt)) {
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
    interference.pairs = pairs;
}

/// A kernel error turned into words. It came back from the dictionary crate: the kernel gives a CODE and
/// whoever has a window picks the words, so the mapping is the application's.

#[cfg(test)]
/// The checks that the INTERFACE goes through the dictionary. They stayed with the application when the
/// dictionary moved out: what they watch is the panels, not the catalogue.
#[cfg(test)]
mod i18n_use_tests;
#[cfg(test)]
mod scheme_use_tests;
mod audit;
mod behaviour_sweep;
mod described_picks;
mod expand_selection;
mod sketch_sweep;
mod input;
mod bg_rebuild;
mod focus_keys;
mod frame_cost;
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
mod the_cancel_button_cancels;
mod the_part_menu_can_delete;
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
mod ask_then_write;
mod draw_tools_are_mutually_exclusive;
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
mod catalog_flow;
mod comp_array_flow;
mod help_flow;
mod help_general;
mod help_images;
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
mod window_title;

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
pub(crate) mod sketch_source;
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
pub(crate) use qymcad_ui_state::{props_header, NameSlot};

/// The sketcher (geometry, dimensions, constraints) lives in `gui/sketching.rs`.
mod sketching;

/// Files and background jobs live in `gui/io_jobs.rs`.
mod io_jobs;

/// Asking for a file without stopping the frames lives in `gui/file_ask.rs`.
mod file_ask;
mod deaf_while_the_system_asks;
mod a_machine_with_no_graphics;
mod the_card_holds_its_words;

/// Is the cursor on the face arrow? The threshold matches the body gizmo's, in pixels along the segment.
pub(crate) fn face_arrow_hit(pn: &qymcad_ui_state::Painting, rect: Rect, pos: Pos2, basis: &([f64; 3], [f64; 3], [f64; 3])) -> bool {
    let Some((o, tip, _)) = face_arrow_geometry(pn) else { return false };
    let scr = qymcad_ui_state::Screen { cam: &pn.cam, set: pn.set, rect: rect, basis: basis };
    let (a, b) = (scr.at(o).0, scr.at(tip).0);
    screen_dist_seg(pos, a, b) <= 9.0
}
