//! THE STATE OF THE INTERFACE, in a crate of its own.
//!
//! Fifty-five records: the tools with parameters of their own, the selection, the view, the caches, the
//! settings. Every context of a workbench (`PartCtx`, `SketchCtx`, `JointCtx` and the rest) is built out
//! of these and out of nothing else.
//!
//! WHY A CRATE. A workbench cannot become a crate of its own while the records it edits are declared
//! beside the application: it would have to depend on the crate holding the god object, and that is the
//! very dependency the rebuild is undoing. Here they depend on the document, the kernel, the shell and
//! `egui` - which is what the manifest says and a guard below checks.
//!
//! THIS FILE NAMES `App` NOWHERE. That is the whole point, and it is guarded rather than promised.

pub mod grab;
pub mod platform_keys;
/// Clipping a triangle by a plane for the SECTION view: a pure module with no `self`, so it can be unit-tested.
pub mod smallvec_tris {
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

pub mod settings_sections;

use egui::{Pos2, Rect, Vec2};
use qymcad_core::measure::MeasureItem;
use qymcad_core::part::PartManifest;
use qymcad_core::refs::Query;
use qymcad_core::geom::{Contour, MeshFace, Point2};
use qymcad_core::model::{Id, Project};

#[derive(Clone, Copy)]
pub struct View2d {
    pub center: Vec2,
    pub scale: f32,
    pub initialized: bool,
    /// The scale the sheet was fitted at: the wheel zooms on from it (`zoom_limits`), as the 3D view does.
    pub fit: f32,
}

impl Default for View2d {
    fn default() -> Self {
        Self { center: Vec2::ZERO, scale: 4.0, initialized: false, fit: 4.0 }
    }
}

/// A workbench. It switches the left tool bar and the set of panels.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Workbench {
    Sketch,
    Part,
    Assembly,
}

impl Workbench {
    /// The workbench's code for linking to the help - not its label: the label is translated, and a link must not
    /// depend on the language.
    pub fn code(self) -> &'static str {
        match self {
            Workbench::Sketch => "sketch",
            Workbench::Part => "part",
            Workbench::Assembly => "assembly",
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum Sel {
    None,
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
    /// A feature of the build tree (a node index into project.timeline).
    Feature(usize),
    /// A component or part (an index into project.components).
    Component(usize),
    /// A mate (by the joint's Id) - selecting the list row and the 3D glyph together, both ways.
    Joint(Id),
    /// An edge of a body: (the body, the persistent id of the edge). Taken by a click on it with nothing in hand.
    Edge(Id, u32),
    /// A corner of a body, as the end of one of its edges: (the body, the edge's id, its far end).
    Vertex(Id, u32, bool),
}

/// What to export in 3D (STEP or STL): the selected component (with every body nested in its subtree) or the whole
/// project. In both cases ONLY the visible bodies are exported.
#[derive(Clone, Copy)]
pub enum ExportTarget {
    Component(Id),
    Project,
}

/// DERIVED STATE, NOT TRUTH: everything here can be thrown away, and the next frame rebuilds it.
///
/// Fourteen caches used to sit among the fields of `App` looking exactly like the document itself, and the
/// difference matters: losing one of these costs a frame, losing a field of the document costs the part.
///
/// Each is keyed by the revision it was computed at, so it answers "still good?" by comparing numbers rather
/// than by somebody remembering to clear it. THE EMPTY KEY IS `u64::MAX`, NOT ZERO: zero is a real revision,
/// and a cache that starts at it claims to hold the first build of the document before anything is built.
pub struct Caches {
    /// THE TEXTURE CACHE OF THE ViewCube'S LABELS (keyed by text and size). Rasterising the font every frame means
    /// thousands of glyphs a second for nothing; a texture is prepared once and lives until the language or the
    /// cube's size changes, and both of those change the key.
    pub label_tex: std::cell::RefCell<std::collections::HashMap<String, egui::TextureHandle>>,
    /// THE SCENE BUFFER'S BLOCKS BY BODY - one per body, see `SceneBlock`.
    ///
    /// Dragging a part moves ONE body, while the buffer was assembled whole: 463,878 vertices per frame, 30 to 48
    /// ms on a real assembly. A block depends on the body's FORM, its highlight and the shared display settings;
    /// the position lives separately and a move does not invalidate the block.
    pub scene_blocks: std::cell::RefCell<std::collections::HashMap<usize, SceneBlock>>,
    /// WHAT BECAME OF THE BLOCKS ON THE LAST SCENE PASS: [rebuilt, shifted, taken ready-made].
    ///
    /// Not decoration but the only honest answer to "why is this frame expensive": without it an optimisation is
    /// verified with a stopwatch, and a stopwatch is off by whole multiples on a debug build. The
    /// `a_moved_part_does_not_rebuild_its_block` check asks for exactly these numbers.
    pub scene_stats: std::cell::Cell<[u32; 3]>,
    /// The 3D render's cache: (the view key, the texture). It is redrawn only when the view changes.
    pub view: std::cell::RefCell<Option<(u64, egui::TextureHandle)>>,
    /// The GPU viewport is on (a switch; the CPU raster is the fallback).
    /// A perspective projection instead of an orthographic one. It is applied by one formula in `Screen::at`, so the
    /// overlays and both render paths agree. Orthographic by default, as is customary in CAD.
    /// Smooth (Gouraud) shading instead of flat. When true, the colour is computed at EVERY vertex from the
    /// smoothed normal (`Mesh::vertex_normals`) and interpolated across the triangle; sharp edges stay sharp (the
    /// mesh topology is split by face). False gives flat shading (the face normal).
    /// The cache of smoothed vertex normals, parallel to `project.meshes`, plus the `geom_rev` it was built at. It
    /// is recomputed only when the geometry changes, not every frame. The normals are local (before the world transform).
    pub norm: std::cell::RefCell<Cached<Vec<Normals>>>,
    /// The key of the scene uploaded into the GPU vertex buffer (re-uploaded only on a change). It does NOT depend on the camera.
    pub gpu_scene_key: std::cell::Cell<u64>,
    /// The cache of the visible scene's world bounding sphere (a centre and a radius) keyed by the scene - for
    /// tight near and far planes in perspective (the z buffer's precision). Recomputed only when the scene changes,
    /// not every frame.
    pub bounds: std::cell::Cell<Cached<Option<Sphere>>>,
    /// THE CACHE OF CONSUMED BODIES: `(the revision, the bodies)`. The set is computed by a pass over the whole
    /// timeline while being asked for on every body on every frame - recomputing it is out of the question.
    pub consumed: std::cell::RefCell<Cached<std::collections::HashSet<Id>>>,
    /// THE LIST OF VISIBLE BODIES: `(the revision, the context, [(the mesh index, the body id)])`.
    ///
    /// For each body `body_shown` finds its owner by a LINEAR pass over the whole timeline and walks the visibility
    /// tree. In the pick loop that happens for EVERY body on EVERY frame: at 120 bodies the measurement gave 67 ms
    /// per frame, which is an application that does not respond. The list is computed once per rebuild and per
    /// change of context.
    pub shown_bodies: std::cell::RefCell<Cached<ShownBodies>>,
    /// THE CACHE OF THE BODIES' WORLD EXTENTS: `(the revision, a body mapped to its 8 world corners)`.
    ///
    /// Rejecting by "the cursor is outside the extent" did not help by itself, because what was expensive was
    /// REACHING the body: finding the mesh by id and computing its display transform was done for every body on
    /// every frame. The measurement showed 0.56 ms per body - on an assembly that is seconds. The corners are
    /// computed once per rebuild.
    pub bbox_world: std::cell::RefCell<Cached<std::collections::HashMap<Id, [[f64; 3]; 8]>>>,
    /// THE CACHE OF THE BODIES' EDGES FOR PICKING: `(the geometry revision, a body mapped to (polylines, ids))`.
    ///
    /// Without it, picking an edge or a vertex anchor pulled the edges of EVERY body out of the kernel on EVERY
    /// mouse movement. On a couple of cubes that goes unnoticed; on an assembly of a thousand components the
    /// application stops responding. The key is the geometry revision: once the model is rebuilt, the cache throws
    /// itself away.
    pub pick_edges: std::cell::RefCell<Cached<std::collections::HashMap<Id, std::rc::Rc<EdgePolys>>>>,
    /// The cache of the section CAPS: (a key of plane plus geom_rev plus visibility, the caps in WORLD coordinates).
    pub section_caps: std::cell::RefCell<Cached<std::rc::Rc<Vec<qymcad_core::geom::Mesh>>>>,
    /// The meshes' world extents by index plus the `geom_rev` they were computed at. A cheap way of rejecting
    /// bodies for the section caps (a Common boolean is expensive, and the plane cuts a handful of bodies out of a
    /// thousand).
    pub mesh_bounds: std::cell::RefCell<Cached<std::collections::HashMap<usize, WorldBox>>>,
    /// The sketch diagnostics cache. The rank analysis builds a FULL Jacobian and runs Gaussian elimination
    /// (O(m * nv^2)) - without a cache that was computed EVERY FRAME several times over (the panel, the list, the
    /// overlay, the glyphs, the tree), and on large sketches the interface hung. It is recomputed only on an edit, and
    /// while points are dragged only on an edit of the sketch's shape (`sk_dragged`).
    /// A `RefCell`, because drawing goes through `&self` and needs that very cache.
    pub sk_status: std::cell::RefCell<Option<SketchStatus>>,
    /// THE SKETCH WHOSE POINTS ARE BEING DRAGGED, set by the sketch's drag handling after every frame of it. While it
    /// is set, the diagnostics of that sketch are those of its shape (`SketchStatus::shape`) and are not recomputed for
    /// the points moving: 3 000 lines took 3.7 s for their degrees of freedom on every frame. The release clears it,
    /// and the diagnostics are recomputed once where the points came to rest - a degenerate place shows there.
    pub sk_dragged: std::cell::Cell<Option<usize>>,
    /// THE BORDERS OF A BODY MADE OF A MESH THAT DID NOT CLOSE, by body index, each a loop of corners in the body's own
    /// coordinates; keyed by the meshes they were found on.
    pub open_borders: std::cell::RefCell<Cached<std::collections::HashMap<usize, Vec<OpenBorder>>>>,
}

impl Default for Caches {
    fn default() -> Self {
        Self {
            label_tex: std::cell::RefCell::new(std::collections::HashMap::new()),
            scene_blocks: Default::default(),
            scene_stats: Default::default(),
            view: std::cell::RefCell::new(None),
            norm: std::cell::RefCell::new(Cached { rev: u64::MAX, value: Vec::new() }),
            gpu_scene_key: std::cell::Cell::new(u64::MAX),
            bounds: std::cell::Cell::new(Cached { rev: u64::MAX, value: None }),
            consumed: std::cell::RefCell::new(Cached { rev: u64::MAX, value: std::collections::HashSet::new() }),
            shown_bodies: std::cell::RefCell::new(Cached { rev: u64::MAX, value: ShownBodies::default() }),
            bbox_world: std::cell::RefCell::new(Cached { rev: u64::MAX, value: std::collections::HashMap::new() }),
            pick_edges: std::cell::RefCell::new(Cached { rev: u64::MAX, value: std::collections::HashMap::new() }),
            sk_status: std::cell::RefCell::new(None),
            sk_dragged: std::cell::Cell::new(None),
            section_caps: std::cell::RefCell::new(Cached { rev: 0, value: std::rc::Rc::new(Vec::new()) }),
            mesh_bounds: std::cell::RefCell::new(Cached { rev: 0, value: std::collections::HashMap::new() }),
            open_borders: std::cell::RefCell::new(Cached { rev: u64::MAX, value: std::collections::HashMap::new() }),
        }
    }
}

/// WHAT IS OPEN ON SCREEN: the windows, the dialogues and the little state each of them keeps while open.
///
/// Two dozen `show_*` flags used to lie among the fields of the document, and a reader could not tell at a
/// glance which of them was part of the part and which was merely a window somebody had opened. They are all
/// the same kind of thing - "this is on screen right now" - and none of them belongs in a saved file.
/// ONE WINDOW THAT CAN BE OPEN. The set of these, not ten separate flags, is what "what is on screen" means.
///
/// Ten booleans could not be walked over, could not be closed at once, and could not be checked against the menu:
/// every one of those actions had to be written ten times, and a window added later was silently left out of all
/// three. As a set, "close everything" is one line and "is anything open" is one line.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum WinKind {
    /// "Save as a template": asks for a name.
    SaveTemplate,
    /// The start screen. It comes up at launch and from a menu item; any action dismisses it.
    Start,
    /// The document properties.
    DocProps,
    /// About (Help -> About).
    About,
    /// "Report a problem" (Help -> Report a problem).
    Report,
    /// The command search.
    CmdSearch,
    /// The hotkeys reference (under Help), built from ONE source - see hotkeys.rs.
    Hotkeys,
    /// The project parameters (the named dimensions and formulas).
    Params,
    /// The settings.
    Settings,
    /// The parts library catalogue.
    PartsLibrary,
    /// What the check for a newer version came to (Help -> Check for updates).
    Updates,
}

impl WinKind {
    /// EVERY KIND, so that a walk over the windows cannot silently miss one added later. A guard checks that the
    /// count here matches the number of variants declared above.
    pub const ALL: [WinKind; 11] = [
        WinKind::SaveTemplate,
        WinKind::Start,
        WinKind::DocProps,
        WinKind::About,
        WinKind::Report,
        WinKind::CmdSearch,
        WinKind::Hotkeys,
        WinKind::Params,
        WinKind::Settings,
        WinKind::PartsLibrary,
        WinKind::Updates,
    ];
}

#[derive(Default)]
pub struct Windows {
    /// WHICH WINDOWS ARE OPEN. Private on purpose: the only ways in are `is`, `set`, `open`, `close`, `toggle` and
    /// `close_all`, so no caller can invent an eleventh way of asking the same question.
    open: std::collections::BTreeSet<WinKind>,
    /// The help window, gathered into a record of its own: the four fields only ever move together.
    pub help: HelpWin,
    /// The name typed into the "save as a template" dialogue.
    pub tpl_name: String,
    /// THE START SCREEN WAS OPENED ON REQUEST - from the Windows menu rather than by itself.
    ///
    /// NOT A WINDOW, hence not in the set: it says HOW `WinKind::Start` came up. The rule "do not show it over
    /// someone's work" is written for what comes up BY ITSELF: an uninvited modal over somebody's part is what
    /// gets closed without a look. It has no right to cancel an explicit request: pressing the menu item gave
    /// NOTHING, not even a message.
    pub start_asked: bool,
    /// THE COMMAND SEARCH's own state: the query and the selected row.
    pub cmd_search_query: String,
    pub cmd_search_sel: usize,
    /// NOT A WINDOW: a one-shot request to put the caret in the search field on the next frame, taken by `mem::take`.
    pub cmd_search_focus: bool,
    /// NOT A WINDOW, a VIEW TOGGLE. "In context" (top-down): show the bodies of NEIGHBOURING parts as ghosts while
    /// working inside a part, so that their geometry can be referred to (a sketch on a neighbour's face becomes an
    /// automatic ExternalRef). Nothing opens or closes; the viewport draws differently.
    pub context: bool,
    /// NOT A WINDOW, a VIEW TOGGLE: whether to show the constraint glyphs in the viewport. The dimensions are
    /// always shown.
    pub constraints: bool,
    /// A file just read, waiting for its units and scale (see `ImportScale`).
    pub import_scale: Option<ImportScale>,
}

impl Windows {
    /// Is this window open?
    pub fn is(&self, k: WinKind) -> bool {
        self.open.contains(&k)
    }
    /// Open it or close it, whichever `on` says. The form the `egui` windows need: they hand back a bool.
    pub fn set(&mut self, k: WinKind, on: bool) {
        if on {
            self.open.insert(k);
        } else {
            self.open.remove(&k);
        }
    }
    /// Open it.
    pub fn open(&mut self, k: WinKind) {
        self.open.insert(k);
    }
    /// Close it.
    pub fn close(&mut self, k: WinKind) {
        self.open.remove(&k);
    }
    /// Flip it, and say what it became - the menu items need both.
    pub fn toggle(&mut self, k: WinKind) -> bool {
        let on = !self.is(k);
        self.set(k, on);
        on
    }
    /// CLOSE EVERYTHING. One line, and a window added later is closed by it without anybody remembering to.
    pub fn close_all(&mut self) {
        self.open.clear();
    }
    /// Raise or put out the start screen, telling ITS TWO STATES APART as the program does.
    ///
    /// `show_start` means "it comes up by itself on an empty document"; a request from the menu is a
    /// separate thing. Closing puts out both: closed is closed, whatever raised it.
    pub fn show_start(&mut self, on: bool) {
        self.set(WinKind::Start, on);
        if !on {
            self.start_asked = false;
        }
    }

    /// Is anything at all open? Escape and the modal checks ask this.
    pub fn any_open(&self) -> bool {
        !self.open.is_empty()
    }
    /// What is open, in a stable order - for a check, a report or a walk.
    pub fn open_kinds(&self) -> impl Iterator<Item = WinKind> + '_ {
        self.open.iter().copied()
    }
}

/// THE EDGES OF THE SELECTED BODY, ready for picking and for drawing.
///
/// Recomputed when the selection or the geometry changes, and keyed by the revision it was taken at, so a
/// stale set is never offered to a click.
#[derive(Default)]
pub struct EdgeCache {
    /// Whether the extrude gizmo's arrow is being dragged (the feature node's index).
    /// The body whose edges are currently shown for picking (chamfer or fillet), plus their polylines in world space.
    pub body: Option<Id>,
    pub polys: Vec<Vec<[f32; 3]>>,
    /// The edge's PERSISTENT id, parallel to `edge_polys` - for picking by an id that survives a rebuild.
    pub ids: Vec<u32>,
    /// The `geom_rev` at the moment the edge cache was built: when a body is rebuilt (the same Id, a new topology)
    /// the cache goes stale, so it is refreshed and any dangling picked edge ids are cleared.
    pub rev: u64,
    /// The straight edges of EVERY visible body (the body, the edge's persistent id, the polyline in its LOCAL
    /// frame) - for the axis click-pick (a pattern or a datum axis): an edge of ANY body may be picked, and the id
    /// is what makes the axis associative. Loaded on entry.
    pub axes: Vec<(Id, u32, Vec<[f32; 3]>)>,
}

/// TRIMMING A SURFACE: the stroke drawn by hand, what it has already cut, and which piece is being kept.
#[derive(Default)]
pub struct TrimTool {
    /// POWER TRIM: the on-screen trail of the drag and the spans already trimmed (an entity plus a span number).
    /// They live only within one drag - otherwise a second pass over the same line would cut nothing.
    pub path: Vec<Pos2>,
    pub done: std::collections::HashSet<(Id, u32)>,
    /// TRIM: what is being cut and which side is kept: (the sheet body, the click point) plus the tool body.
    ///
    /// The point arrives with the first click: a person points at THE part they are keeping, and that same movement
    /// chooses the sheet. Asking for the side as a separate step would split one gesture into two for the sake of
    /// data that has already been given.
    pub keep: Option<(Id, [f64; 3])>,
    pub tool: Option<Id>,
}

/// UNDO, REDO AND WHAT IS SAVED: the history of the document and how far it has been written to disk.
pub struct Edits {
    /// Undo and redo: snapshots of the editor's state.
    pub undo: Vec<Step>,
    pub redo: Vec<Step>,
    /// The currently committed state (the baseline) and its key.
    pub baseline: Snapshot,
    /// the open edit: (its name, the state BEFORE it). Empty means there is no edit and a change is going past the boundary.
    pub open: Option<(String, Snapshot)>,
    /// the nesting depth of the edits: an inner `commit_edit` must not close the OUTER edit
    pub depth: usize,
    pub committed_key: u64,
    /// The places that change the document past `App::edit` - they are gathered during real work and converted one
    /// at a time. A field rather than a global flag: the debt belongs to the session, not to the process.
    ///
    /// Only the debug build keeps it: the record is written by the debug-only watch below, and a field
    /// nobody reads in release is a `dead_code` refusal, which is denied in the manifest.
    #[cfg(debug_assertions)]
    pub debt: std::collections::HashSet<String>,
    pub ready: bool,
    /// The state key as of the last save, load or new project. Being dirty (having unsaved changes) means
    /// `edit_key() != saved_key`.
    pub saved_key: u64,
    /// Allow the window to close (the dialogue has been answered), so that intercepting `close_requested` does not loop.
    pub allow_close: bool,
    /// AUTOSAVE: the timer and the key of the last autosave (so the same thing is not written twice).
    pub last_autosave: std::time::Instant,
    pub autosave_key: u64,
}

impl Edits {
    /// A NEW DOCUMENT STARTS A CLEAN HISTORY: no step to take back, none to put again, and the baseline and the saved
    /// mark taken afresh on the next frame. Reported behaviour: File -> New project came with a step "Edit" nobody made
    /// and unsaved at once, so the next New project asked to save the one just made.
    pub fn fresh(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.open = None;
        self.depth = 0;
        self.ready = false;
    }
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
#[derive(Default)]
pub struct Rebuilding {
    /// The active background operation (a STEP or STL import or export) - a modal overlay with a spinner while it runs.
    pub busy: Option<Busy>,
    /// A rebuild was requested: it runs in the background on the next frame, with an indicator shown.
    pub wanted: bool,
    /// THE REBUILD WAS STOPPED BY A PERSON - it no longer starts by itself, for the document it was stopped on (its
    /// `rebuild_key`).
    ///
    /// The dirty marks on the nodes remain after a cancellation (the document really has not been rebuilt), and the
    /// scheduler looks at exactly those. Without this mark the next frame would start precisely the work that was
    /// just stopped. Any change of the document and an explicit "rebuild everything" clear it. Reported behaviour:
    /// held as a plain flag it outlived every later edit - a recognition applied after a cancelled one made its node
    /// at once and never computed it, nor anything after it.
    pub paused: Option<u64>,
    /// How many undo steps the document had when the rebuild now asked for was asked for, and when the last one that
    /// ran to its end was: a cancelled rebuild takes back the one edit it was computing, and only that.
    pub computing_depth: usize,
    pub computed_depth: usize,
    /// the application really is running frames (not a headless test) - only then does a rebuild go into the background.
    pub ui_running: bool,
    /// BACKGROUND (non-modal) work - it runs while the model is already being turned: topping up the imports'
    /// B-rep, saving the project. No overlay is shown, only the status line; the result comes over the same channel
    /// as `busy`.
    pub bg: Vec<Busy>,
    /// WHETHER THE IMPORTS' RESTORATION WAS CALLED - for the tests: "rebuild everything" must call it.
    pub import_asked: bool,
    /// The reference rebindings of the last rebuild - they are shown in the status line and mark the nodes in the
    /// tree. They live until the next rebuild and never reach the file.
    pub rebinds: Vec<qymcad_core::feature::Rebind>,
    /// The last line the rebuild itself wrote on the status line: its end replaces only that, not what an operation
    /// wrote since.
    pub line: String,
    /// What stood on the status line before a quiet rebuild wrote its progress over it - put back when it ends well.
    pub over: String,
    /// THE STATE AS OF THE PREVIOUS REBUILD: the document's key, the list of dirty nodes and the number of live
    /// B-reps. The scheduler recomputes only if AT LEAST ONE of those has changed.
    ///
    /// A node that failed to build deliberately stays dirty - the attempt must happen again once its input appears.
    /// But the scheduler read "dirty" as "compute now" and took it up on every frame: with a single red feature the
    /// rebuild window flickered without stopping.
    pub last: (u64, Vec<Id>, usize),
    /// A counter of geometry changes (for invalidating the 3D render cache).
    pub geom_rev: u64,
    /// THE LAYOUT'S REVISION - where the parts stand. It moves on a drag and on any component move.
    ///
    /// It is kept separate from `geom_rev` for exactly the reason every other such pair was separated: A LAYOUT IS
    /// NOT GEOMETRY. Bodies do not change when they move, and everything that depends only on their form (the
    /// edges, the smoothed normals, the scene buffer's blocks) has no reason to be recomputed.
    pub place_rev: u64,
    /// a rebuild was requested inside an edit - it runs ONCE when that edit closes
    pub pending: bool,
}

impl Rebuilding {
    /// Is a REBUILD running in the background, as opposed to some other slow job?
    ///
    /// The `busy` slot holds one job of any kind - an import, an export, a rebuild - so "is it busy"
    /// and "is it rebuilding" are different questions. The second was spelled out three times in three
    /// places, each repeating which kind counts.
    pub fn regen_running(&self) -> bool {
        matches!(&self.busy, Some(b) if b.kind == BgKind::Regen)
    }
}

/// THE LIVE GEOMETRY behind the meshes: the kernel's own solids and the faces they were tessellated into.
///
/// A bundle holds meshes and faces, not solids, so the live B-rep is fetched on demand rather than rebuilt
/// when a file is opened.
#[derive(Default)]
pub struct LiveGeom {
    /// THE BLOBS OF THE LIVE BODIES FOR WRITING, one per body. Serialising every body of a real file costs about
    /// 0.6 s, and without a cache EVERY autosave (once every three minutes) would pay it straight on the UI thread.
    /// An entry here is dropped for exactly those bodies that were rebuilt.
    pub blobs: std::collections::HashMap<Id, Vec<u8>>,
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
    pub tried_rev: Option<u64>,
    pub wait: Option<bool>,
    /// Does the cache of live `Shape`s match the timeline? After opening from a bundle the geometry is shown from
    /// the file while the B-rep is not yet built - the first operation that needs it brings it up through
    /// `ensure_brep`.
    pub ready: bool,
    /// The faces keyed by BODY Id (unlike the index-parallel `faces`, this survives deletions and reorderings of
    /// the meshes). The source for `rebuild_faces_from_cache` after a change of topology.
    pub faces: std::collections::HashMap<Id, Vec<MeshFace>>,
    /// A runtime cache of the live B-rep shapes keyed by body Id (for the shared booleans). It is not serialised
    /// and is filled in by extrude, revolve, STEP and the booleans.
    pub shapes: std::collections::HashMap<Id, qymcad_kernel::Shape>,
    /// THE LIVE SHAPES OF IMPORTS AN UNDO TOOK OUT of the document, kept for the redo that brings them back. An import
    /// has no recipe: its shape comes only from parsing the file again, and without it the body came back from redo
    /// as the snapshot's mesh, faces unnamed and no edges to pick.
    pub shelved: std::collections::HashMap<Id, qymcad_kernel::Shape>,
    /// THE BYTES OF THE SOURCES AN UNDO TOOK OUT, for the same redo: a snapshot carries a source without its bytes,
    /// and a drawing brought back without them had no geometry to bring in again.
    pub shelved_sources: std::collections::HashMap<Id, Vec<u8>>,
}

/// THE COLOUR SCHEME AND THE SETTINGS WINDOW: the palette in force, the ones to choose from, and what is
/// being searched for or edited.
pub struct SchemeUi {
    /// THE CURRENT PALETTE, derived from the `scheme` setting. It is kept resolved rather than looked up by name
    /// for every colour: a colour is asked for thousands of times per frame.
    pub pal: qymcad_scheme::Palette,
    /// EVERY KNOWN SCHEME: the built-in ones plus the user's from disk. Kept as a list rather than reading the
    /// directory every frame - the files are re-read only when the editor has changed them.
    pub all: Vec<qymcad_scheme::Palette>,
    /// the state of the custom-scheme screen: which one is being edited and what to say about saving
    pub edit: SchemeEdit,
    /// THE OPEN SECTION of the settings window. INTERFACE state rather than a setting: `Settings` holds only the
    /// values that are saved whole, and "where I am in the window right now" is not one of them.
    pub section: settings_sections::SettingsSection,
    /// The settings search's query.
    pub search: String,
    /// What to say after a section is reset or after an attempt to open the config folder.
    pub note: String,
}

impl Default for SchemeUi {
    fn default() -> Self {
        Self {
            pal: qymcad_scheme::dark(),
            all: qymcad_scheme::builtin(),
            edit: SchemeEdit::default(),
            section: settings_sections::SettingsSection::General,
            search: String::new(),
            note: String::new(),
        }
    }
}

/// WHAT EXACTLY THE COMMAND BUILDS: the operation on the body and the direction. These used to be three
/// independent fields of `App`, and the ORDER in which they were set mattered more than their values: the direction
/// was computed when the command opened while the operation was chosen later - so a cut went outwards and removed
/// nothing. Gathered into one record they cannot drift apart: what is derived (`flip`) is deduced from `op` when
/// it is needed.
#[derive(Clone, Copy, Default, PartialEq, Debug)]
pub struct FeatTarget {
    /// 0 add (a new body), 1 a boss, 2 a cut, 3 an intersection
    pub op: u8,
    /// the direction: true means against the sketch's normal
    pub flip: bool,
    /// the direction is still automatic (nobody has touched it), so it is recomputed on apply
    pub flip_auto: bool,
}

impl FeatTarget {
    /// Open the command: the direction is automatic for now.
    pub fn opened(op: u8) -> Self {
        Self { op, flip: false, flip_auto: true }
    }

    /// The direction was set by hand - it is no longer recomputed.
    pub fn set_flip(&mut self, flip: bool) {
        self.flip = flip;
        self.flip_auto = false;
    }
}

/// THE SWEEP'S PARAMETERS: the profile and the path, each with its own sketch and chosen contour.
/// Five independent fields of `App` turned "which sketch is being picked right now" into implicit state:
/// `pick_path` decided where the next click would land and drifted apart from what had already been chosen.
#[derive(Clone, Copy, Default, PartialEq)]
pub struct SweepParams {
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
pub struct LoftParams {
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
pub struct RevolveParams {
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
#[derive(Clone, Copy, Default, PartialEq, Debug)]
pub struct ThreadParams {
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

/// The unfinished drawing as a whole: the shape plus the input of its dimensions.
#[derive(Clone, Default, PartialEq)]
pub struct Placing {
    pub shape: PlacingShape,
    /// the dimension is following the cursor and has not been placed yet (the constraint's index)
    pub dim: Option<usize>,
    /// the dimension input fields and the focus within them
    pub buf: [String; 2],
    pub focus: bool,
    /// a value was typed into the fields: closing them with Enter then lays the dimensions typed
    pub typed: bool,
}

impl Placing {
    /// Are the unfinished shape's dimensions being typed in?
    pub fn active(&self) -> bool {
        self.shape != PlacingShape::None
    }

    /// Abandon the unfinished drawing.
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn rect(&self) -> Option<(Point2, Point2, Vec<Id>)> {
        match &self.shape {
            PlacingShape::Rect { a, b, ids } => Some((*a, *b, ids.clone())),
            _ => None,
        }
    }

    /// A rectangle reopened for its width and height: the indices of those dimensions.
    pub fn rect_dims(&self) -> Option<qymcad_core::model::RectDims> {
        match &self.shape {
            PlacingShape::RectDims(d) => Some(*d),
            _ => None,
        }
    }

    /// A rectangle drawn from its centre: the centre plus the corner under the pointer.
    pub fn rect_center(&self) -> Option<(Point2, Point2, Vec<Id>)> {
        match &self.shape {
            PlacingShape::RectCenter { center, corner, ids } => Some((*center, *corner, ids.clone())),
            _ => None,
        }
    }

    pub fn poly(&self) -> Option<Id> {
        match self.shape {
            PlacingShape::Poly(id) => Some(id),
            _ => None,
        }
    }

    pub fn ellipse(&self) -> Option<(Id, Point2)> {
        match self.shape {
            PlacingShape::Ellipse(id, c) => Some((id, c)),
            _ => None,
        }
    }

    /// Start or update the shape. The previous one disappears - it is no longer being drawn.
    pub fn set(&mut self, shape: PlacingShape) {
        self.shape = shape;
    }
}

/// THE DRAWING TOOLS' PREFERENCES: each has a mode and numbers of its own. This is NOT the state of the current
/// action (that lives in `Placing`) but preferences proper: they survive a change of tool, as in any grown-up CAD -
/// come back to the polygon and the number of sides is the one left there.
#[derive(Clone, Default, PartialEq)]
pub struct SketchToolPrefs {
    /// the polygon: the number of sides and how its two clicks are read (see `polygon_from_clicks`)
    pub poly_n: u32,
    pub poly_mode: u8,
    /// the construction modes: an arc (by three points or by a centre), a rectangle (corners or centre), a circle
    pub arc_mode: u8,
    pub rect_mode: u8,
    pub circ_mode: u8,
    /// EXTEND TAKES BOTH ENDS of its line, each to the nearest geometry on its side ("Both sides" in its bar)
    pub extend_both: bool,
    /// the corner fillet's radius - the first leg of a chamfer - and the offset's distance
    pub fillet: f64,
    pub offset: f64,
    /// HOW A CHAMFER IS GIVEN (as the chamfer of a part): equal legs, two legs, or a leg and an angle; and its second
    /// value - the second leg, or the angle from the first line in degrees
    pub chamfer_mode: qymcad_core::feature::ChamferMode,
    pub chamfer_second: f64,
    /// HOW A FILLET IS GIVEN: by its radius, its chord or the length of its arc; `fillet` holds the value in that way
    pub fillet_by: qymcad_core::model::FilletBy,
    /// a text in a sketch: its contents, its height, and whether it is an annotation rather than geometry
    pub text: String,
    pub text_h: f64,
    pub text_note: bool,
    /// WHAT THE TEXT TOOL WRITES WITH, beside what it writes and how tall.
    ///
    /// A setting of the tool, not a thing the application owns: the top bar shows the family here beside the
    /// height, and the label made by the next click keeps it. Empty until a font is chosen or found.
    pub font: qymcad_core::model::FontRef,
}

/// THE SKETCH PATTERN'S PARAMETERS: two directions with a step and a count each, plus the circular variant.
#[derive(Clone, Copy, Default, PartialEq)]
pub struct SketchPattern {
    pub dx: f64,
    pub dy: f64,
    pub count: u32,
    pub dx2: f64,
    pub dy2: f64,
    pub count2: u32,
    pub angle: f64,
}

/// WHETHER WHAT A DRAWING TOOL PUTS DOWN IS TIED TO WHAT IT LANDS ON, construction or not. Reported behaviour:
/// "construction geometry is built without ties" - drawing construction switched the ties off, and nothing switched
/// them off for ordinary geometry.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Ties {
    /// the pointer snaps to points and lines, and what is drawn is tied there and laid level or upright where it
    /// stands so, as the auto constraints of the sketch say
    #[default]
    Laid,
    /// "No ties": put down exactly where it is clicked - nothing snaps, nothing is tied, the preview snaps to nothing
    Free,
}

/// WHETHER A DRAWING TOOL TIES WHAT IT PUTS DOWN: "No ties" not ticked on its bar and the auto constraints of the
/// sketch on - construction geometry as any other. One rule for what is laid and for what the preview shows: the
/// preview of a line showed the badge of a level tie with "No ties" ticked, a tie the click was not going to lay.
pub fn ties_laid(tool: &SketchTool, set: &Settings) -> bool {
    tool.ties == Ties::Laid && set.auto_constrain
}

/// THE ACTIVE SKETCH TOOL: what is chosen on the panel and what has already been clicked with it.
/// "Which tool" and "how many points it has gathered" used to be different fields of `App`, and changing the tool
/// was not obliged to clear what had been gathered - yet it must: a line's points mean nothing to an arc.
#[derive(Clone, Default, PartialEq)]
pub struct SketchTool {
    /// the points clicked with THE CURRENT tool (an arc has three, a line two, and so on)
    pub pts: Vec<Point2>,
    /// construction geometry is being drawn
    pub construction: bool,
    /// whether what is drawn is tied to what it lands on - "No ties" on the bar of a drawing tool
    pub ties: Ties,
    /// PROJECTION: take not one edge but THE WHOLE outline of the face the sketch stands on. By a switch of its
    /// own rather than a guess from the click: "clicked inside the face" is indistinguishable from a miss past an
    /// edge on outlines with complicated cut-outs.
    pub proj_face: bool,
    pub move_base: Option<Point2>,
    /// THE LINE EXTEND HOLDS: taken with the tool from the selection made before it, or picked by the first click; its
    /// extension is previewed up to what the pointer is over, and Enter or a click applies it
    pub extend: Option<Id>,
    /// the tangent edge given for a circle (picked with THIS tool rather than globally)
    pub circ_tan: Option<EdgeRef>,
    /// THE STRING OF THE TEXT TOOL AS OUTLINES at the origin, for the preview at the pointer: baked once when the
    /// string, its height or its font change (the fonts are not at hand where the canvas is drawn) - (the string,
    /// the height, the font, the outlines)
    pub text_ghost: Option<(String, f64, qymcad_core::model::FontRef, Vec<Vec<Point2>>)>,
    /// letters of the string were taken out because the font cannot write them: a click on the empty string names
    /// the font rather than blaming the string
    pub text_refused: bool,
}

impl SketchTool {
    /// Choose a tool: whatever was clicked with the previous one MUST disappear - it has nothing to do
    /// with the new one - and WHAT IS IN HAND changes with it. Clicking the tool already in hand puts it
    /// down, which is why the old value is read before the new one is written.
    pub fn select(&mut self, armed: &mut Armed, kind: u8) {
        *armed = if armed.draw_kind() == kind { Armed::None } else { Armed::Draw(kind) };
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
pub struct FeatCommand {
    /// the sketch the command works from
    pub sketch: Option<usize>,
    /// the parameters on the canvas (a height, a radius, a thickness) - their values and the expressions typed in
    pub params: Vec<CmdParam>,
    /// the extent (one side, symmetric, two sides, through all) and the second side
    pub extent: ExtentMode,
    pub down: f64,
    /// an EXISTING feature is being edited (its Id) rather than a new one created
    pub edit: Option<Id>,
    /// the arrow being dragged: the key of the field it drives ("height", or "down" for the second side)
    pub drag: Option<&'static str>,
    /// the view was 3D before the command opened - restore it on Esc or on apply
    pub prev_3d: bool,
    /// the parameter field takes the focus on the first frame (so Enter works straight away)
    pub focus: bool,
    /// the body on whose face the command's sketch sits (an associative placement)
    pub ref_body: Option<Id>,
}

impl FeatCommand {
    /// Open command `kind`: the previous state MUST disappear entirely, and WHAT IS IN HAND changes
    /// with it. The two travel together because they are one act - taking this command means letting
    /// whatever was in hand go, and `Armed` is a single field, so it cannot say both.
    pub fn open(&mut self, armed: &mut Armed, kind: u8, prev_3d: bool) {
        *self = Self { prev_3d, focus: true, ..Default::default() };
        // KIND ZERO IS NOT A COMMAND. While the kind lived in a field, "open command 0" was how the
        // apply path SAID "close", and the two readings of one number went unnoticed. The enum has a
        // name for "nothing in hand", and a command numbered zero is not it.
        *armed = if kind == 0 { Armed::None } else { Armed::Command(kind) };
    }

    /// Close the command (applied or cancelled).
    pub fn close(&mut self, armed: &mut Armed) {
        *self = Self::default();
        *armed = Armed::None;
    }
}

/// TYPING A FILLET RADIUS OR A CHAMFER LEG IN A SKETCH: the value is being written, and the corners it will be
/// taken off are the set beside it.
///
/// **THE VALUE BELONGS TO THE SET, NOT TO ONE CORNER.** The lines chosen in the sketch make the corners, the
/// corners are remembered as they were made, and one answer cuts all of them: a person naming four corners of a
/// shape meant one act, and undo taking back one corner of it would leave the drawing in a state nobody asked for.
#[derive(Clone, Default, PartialEq)]
pub struct CornerInput {
    /// WHICH CORNER TOOL IS IN HAND: the sketch, the point the field belongs to, and whether this is a chamfer
    /// rather than a fillet.
    pub at: Option<(usize, Id, bool)>,
    /// THE CORNERS THE CHOSEN LINES HAVE MADE, the ones put away at their point, and the ones named at a point.
    pub set: CornerSet,
    /// where on the canvas the input field stands
    pub pos: Option<Pos2>,
    pub buf: String,
    /// the second value of a chamfer of two legs or of a leg and an angle, typed beside the first
    pub buf2: String,
    pub focus: bool,
    /// where the corner was clicked, on the sheet: the line it stands nearer to is the first of a chamfer
    pub near: Option<qymcad_core::geom::Point2>,
    /// restrict the corners to this set (rounding THE SELECTED corners rather than all of them)
    pub only: Option<std::collections::HashSet<Id>>,
    /// why the value in the field was refused, said beside it
    pub why: Option<String>,
    /// HOW NEAR THE POINT THE CURSOR COUNTS, in pixels, remembered when the popup was opened: `Grab::Corner` read
    /// with the pick precision of the person. Farther than that the cursor is over the rest of the sheet and says
    /// nothing about which of the corners at the point is meant, so the corner does not follow the pointer across
    /// the drawing while the radius is being typed.
    pub track_px: f32,
    /// WHETHER THE SET IN HAND IS BEING BUILT WITH SHIFT. Shift is the only way more than one line is chosen, so
    /// every left click without it leaves the set and the tool goes back to choosing one line at a time.
    pub shifted: bool,
    /// WHETHER A CLICK CUT TWO LINES WHERE THEY CROSS to name a corner there - a step of undo of its own, which the
    /// corner cut joins and Esc takes back
    pub crossing: CrossingCut,
    /// whether the corner follows the pointer or is fixed where it was clicked
    pub aim: Aim,
}

/// WHETHER THE CORNER OF THE SET FOLLOWS THE POINTER OR STANDS WHERE IT WAS CLICKED. A point with more than one corner
/// to give (two crossing lines chosen with Shift, a click at a crossing, a point the corners of several shapes meet at)
/// is named, not a side of it: the corner follows the pointer into the sector it is in, drawn as a corner not named
/// yet, and a click fixes it there. Its preview turns the colour of the set, and the box of its size opens beside it
/// and stays, so the pointer can go to it. Reported behaviour: "the box runs away from the cursor; the corner must be
/// fixed by a click"; "a click at the crossing puts a corner at random - the same choice by a click as for two lines,
/// the standard for the fillet and the chamfer wherever more than one corner can be chosen".
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Aim {
    /// fixed by a click on its side, or the only corner of its point: it stays
    #[default]
    Fixed,
    /// named at a point of more corners than one, not yet by a side: it follows the pointer, no box yet
    Following,
}

/// HOW MANY CORNERS THE POINT OF THE SET HAS TO GIVE (`CornerSet::choice`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Choice {
    One,
    Many,
}

/// WHETHER THE POINTER STEERS THE CORNER OF A SET (`CornerSet::steer`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Steered {
    Yes,
    No,
}

/// WHETHER THE CORNER TOOL CUT LINES AT A CROSSING to name its corner.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum CrossingCut {
    #[default]
    None,
    /// the lines were cut, and the cut is the last step of undo: the corner joins it, Esc takes it back
    Cut,
    /// the tool was put down - Esc, another tool - with the lines cut and no corner cut: the cut is taken back on the
    /// next frame (`qymcad_sketch::sketch_tool_keys`), where the document is at hand
    Left,
}

impl CornerInput {
    /// THE TOOL PUT DOWN: all of the input goes, and lines it cut at a crossing for a corner never cut are left to be
    /// made whole again.
    pub fn put_down(&mut self) {
        let left = self.crossing == CrossingCut::Cut;
        self.clear();
        if left {
            self.crossing = CrossingCut::Left;
        }
    }
}

/// THE POINT THE FIELD OF A WHOLE SET NAMES, and it is not a point of the drawing.
///
/// **ZERO BELONGS TO "ROUND EVERY CORNER"**, whose field has stood for a whole shape since before a set of corners
/// existed, and whose button is lit by that zero. A set of corners is a whole set too, and its field named zero as
/// well: taking the fillet or the chamfer therefore pressed the button of the round-every-corner beside them. Its
/// field names this instead, so the two are told apart by the number itself rather than by a rule each door reads
/// for itself.
pub const CORNER_SET: Id = 1;

impl CornerInput {
    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

/// THE BODY-TO-BODY BOOLEAN: what is chosen as an operand, and whether an existing node is being edited.
#[derive(Clone, Copy, Default, PartialEq)]
pub struct BoolCommand {
    /// the second operand of a 2D boolean (a contour index)
    pub other2d: usize,
    /// the chosen body and its role (0 the base, 1 the tool)
    pub pick: Option<(Id, u8)>,
    /// body B, clicked and waiting for Enter; a second click on it lets it go
    pub b: Option<Id>,
    /// an existing boolean node is being edited (its index in the timeline)
    pub edit: Option<usize>,
}

/// What a bar takes up once every other tool is put down.
#[derive(Clone, Copy, Debug)]
pub enum Then {
    Nothing,
    /// The body boolean, with body A when one was selected before the button.
    Boolean(Option<Id>),
}

impl Then {
    pub fn take(self, boolean: &mut BoolCommand, status: &mut String) {
        if let Then::Boolean(a) = self {
            boolean.pick = a.map(|a| (a, 0));
            boolean.b = None;
            *status = qymcad_i18n::tr(if a.is_some() { "tb-bool-pick-b" } else { "tb-pick-body-a-first" });
        }
    }
}

/// THE MIRROR'S PARAMETERS: a given plane OR a given part - the mirroring goes either by a plane or over a whole
/// part, and as separate fields both could end up given at once.
#[derive(Clone, Default)]
pub struct MirrorParams {
    pub plane: Option<qymcad_core::feature::SketchPlane>,
    pub part: Option<Id>,
    /// the plane the mirrored copy of `part` is made about, clicked and waiting for Enter: (origin, normal) in the
    /// frame of the context it was clicked in
    pub at: Option<([f64; 3], [f64; 3])>,
    /// the tool is in hand with no part yet: a click on a body takes its part
    pub waiting: bool,
}

impl MirrorParams {
    /// Put the mirrored copy of a part down: the part and the plane picked for it go together.
    pub fn drop_part(&mut self) {
        (self.part, self.at, self.waiting) = (None, None, false);
    }

    /// The mirrored copy of a part is in hand: with its part, or waiting for one to be clicked.
    pub fn in_hand(&self) -> bool {
        self.part.is_some() || self.waiting
    }
}

/// WHAT OCCUPIES THE HAND among the things with no command number - by the key of its hint.
///
/// The conditions here are THE SAME ones by which a button in the bar is shown as pressed (`icon_tool(..., active)`).
/// Otherwise F1 and the highlight of the button would diverge: the button glows and the help is about something else.
///
/// ACTION buttons (create a part, insert a component) are not here and cannot be: they leave no state, and there is
/// nothing to ask about during them. Their rows in the table hold a different promise - that the article is written and
/// will be found through the contents.
pub fn armed_toolbar_hint(pn: &Painting) -> Option<&'static str> {
    if pn.boolean.pick.is_some() {
        return Some("tb-bool-bodies-hint");
    }
    if pn.mirror.in_hand() {
        return Some("tb-mirror-part-hint");
    }
    if pn.picking.fillet_all() {
        return Some("tb-fillet-all-hint");
    }
    // Edit -> Copy, Cut, Insert in a sketch are the copy's article: the base point and the place
    if pn.clip.geom_pending.is_some() || pn.clip.geom_place {
        return Some("tb-copy-hint");
    }
    match pn.armed.move_op() {
        1 => return Some("tb-move-hint"),
        2 => return Some("tb-copy-hint"),
        3 => return Some("tb-rotate-hint"),
        _ => {}
    }
    match pn.armed.pat_op() {
        1 => return Some("tb-lin-array-hint"),
        2 => return Some("tb-circ-array-hint"),
        _ => {}
    }
    if pn.m3.on {
        return Some("tb-measure3d-hint");
    }
    if pn.armed.measuring() {
        return Some("tb-measure-hint");
    }
    if pn.section.pick || pn.section.plane.is_some() {
        return Some("tb-section-hint");
    }
    // IN A SKETCH THE HAND IS NEVER EMPTY: with no tool taken it holds the arrow, and F1 answers about selecting
    if pn.sketch_ses.editing.is_some() && matches!(pn.armed, Armed::None) && pn.sel_sk.constraint.is_none() && pn.sel_sk.modify.is_none() {
        return Some("tb-select-hint");
    }
    None
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
/// HOW THE CAMERA PROJECTS. Two states with names, not a boolean: the settings window draws it as a pair of
/// buttons, and `selectable_value(&mut flag, false, "Orthographic")` made the reader work out which of the two
/// `false` meant.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum Projection {
    /// Parallel: the size on screen does not depend on the distance. What engineering drawing means by a view.
    Ortho,
    /// A vanishing point: nearer is larger. Easier to read a shape by, harder to measure by eye.
    Perspective,
}

/// HOW A FACE IS LIT. Same reason as `Projection`: a pair of named buttons rather than a boolean.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum Shading {
    /// From the smoothed vertex normals (Gouraud): a curved surface looks curved.
    Smooth,
    /// From the face's own normal: every facet is visible, which is what one wants when checking a mesh.
    Flat,
}

/// A NAVIGATION GESTURE: which buttons are held and which modifiers, to move the view.
///
/// Written as data rather than as a branch per style. With eleven layouts a condition written out at each
/// one is eleven places to get wrong, and the differences between them are exactly this: which buttons,
/// which modifiers.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Gesture {
    /// Every one of these must be held. EMPTY means no button at all - a touchpad layout moves the view on
    /// a modifier plus a movement.
    pub buttons: &'static [egui::PointerButton],
    /// WHICHEVER BUTTON IS DRAGGING will do. Only ours works this way, and it is why the right button has
    /// always turned the camera here: a drag is a drag. Naming the left button instead would have taken
    /// that away silently.
    pub any_button: bool,
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
}

impl Gesture {
    const fn of(buttons: &'static [egui::PointerButton]) -> Self {
        Gesture { buttons, any_button: false, shift: false, ctrl: false, alt: false }
    }
    /// Whichever button is dragging.
    const fn any() -> Self {
        Gesture { buttons: &[], any_button: true, shift: false, ctrl: false, alt: false }
    }
    /// MAY THIS GESTURE MOVE THE SHEET OF A SKETCH? The left button is the sketch's own there - it draws, grabs and, with
    /// Ctrl or Shift, adds to the selection - so a gesture holding it moves the sheet only as a chord of two buttons, the
    /// left with the right or the middle one (decided 28.09).
    pub fn sheet_may_take(&self) -> bool {
        !self.takes_a_bare_left_drag() && (!self.buttons.contains(&egui::PointerButton::Primary) || self.buttons.len() >= 2)
    }

    /// Is the left button held together with the right or the middle one - a chord of the layout's movements rather than
    /// the left button's own drag?
    pub fn chord_held(ctx: &egui::Context) -> bool {
        ctx.input(|i| i.pointer.primary_down() && (i.pointer.secondary_down() || i.pointer.middle_down()))
    }

    /// Does a plain left drag, with no modifier, make this gesture?
    pub fn takes_a_bare_left_drag(&self) -> bool {
        !self.shift && !self.ctrl && !self.alt && (self.any_button || self.buttons == [egui::PointerButton::Primary])
    }
    const fn shift(mut self) -> Self {
        self.shift = true;
        self
    }
    const fn ctrl(mut self) -> Self {
        self.ctrl = true;
        self
    }
    const fn alt(mut self) -> Self {
        self.alt = true;
        self
    }
    /// Nothing at all: the layout does not offer this movement.
    pub const NONE: Gesture = Gesture { buttons: &[], any_button: false, shift: false, ctrl: false, alt: false };

    /// Is this gesture being made right now over `resp`?
    ///
    /// THE MODIFIERS ARE MATCHED EXACTLY, not merely "held". Otherwise a layout whose pan is Shift and
    /// whose rotate is bare would pan and rotate at once the moment Shift went down, and the two layouts
    /// that differ only by a modifier would be the same layout.
    pub fn active(&self, ctx: &egui::Context, resp: &egui::Response) -> bool {
        if *self == Gesture::NONE {
            return false;
        }
        let m = ctx.input(|i| i.modifiers);
        if m.shift != self.shift || (m.ctrl || m.command) != self.ctrl || m.alt != self.alt {
            return false;
        }
        if self.any_button {
            return resp.dragged();
        }
        if self.buttons.is_empty() {
            // no button: the pointer must simply be over the canvas and moving
            return resp.hovered() && ctx.input(|i| i.pointer.delta() != egui::Vec2::ZERO);
        }
        // AT LEAST ONE OF THEM MUST BE A DRAG ON THIS CANVAS. `button_down` alone says only that a button is
        // held SOMEWHERE - dragging a window by its title bar holds one too, and the camera turned along
        // with the window. The drag anchors the gesture to the viewport; the rest of the buttons are then
        // merely required to be down, because egui reports the drag for one of them only.
        //
        // AND NO OTHER BUTTON IS HELD: the buttons are matched exactly, as the modifiers are. A pan on the middle
        // button and a rotate on middle + left otherwise both answered the chord, the pan was asked first, and the
        // chord moved the view by the drag without ever turning it.
        let extra = [LEFT, RIGHT, MIDDLE].into_iter().filter(|b| !self.buttons.contains(b)).any(|b| ctx.input(|i| i.pointer.button_down(b)));
        !extra && self.buttons.iter().any(|b| resp.dragged_by(*b)) && self.buttons.iter().all(|b| resp.dragged_by(*b) || ctx.input(|i| i.pointer.button_down(*b)))
    }
}

const LEFT: egui::PointerButton = egui::PointerButton::Primary;
const RIGHT: egui::PointerButton = egui::PointerButton::Secondary;
const MIDDLE: egui::PointerButton = egui::PointerButton::Middle;

/// HOW THE MOUSE MOVES THE VIEW - a layout picked from a list, as other CADs offer.
///
/// Reported behaviour: "add a way to choose how the mouse behaves; other CADs have a dropdown of mouse
/// layouts for the popular ones. Make the same list, with our current layout in it as QymCAD, and let it
/// be the default."
///
/// The ten borrowed layouts are transcribed from the documentation of the CAD they are named after rather
/// than invented here: somebody choosing "Blender" wants Blender's bindings, and a layout that only
/// resembles them is worse than none - it is the habit they came with, failing in a way they cannot name.
///
/// IN A SKETCH the layouts agree wherever they can: a flat sheet has nothing to rotate, so what is left is
/// pan and zoom.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum MouseNav {
    /// OURS, and the factory choice: a drag turns the model, Shift and a drag moves it.
    QymCad,
    Cad,
    Blender,
    Gesture,
    MayaGesture,
    OpenCascade,
    OpenInventor,
    OpenScad,
    Revit,
    TinkerCad,
    Touchpad,
    /// A PEN's: the left button turns the model, the middle one moves the view, the two together zoom - right or up
    /// nearer, left or down farther. Asked for by a person working with a pen; named as they asked.
    Den,
}

impl MouseNav {
    /// EVERY LAYOUT. The one list the settings window, the checks and the catalogue walk over.
    pub const ALL: [MouseNav; 12] = [
        MouseNav::QymCad,
        MouseNav::Cad,
        MouseNav::Blender,
        MouseNav::Gesture,
        MouseNav::MayaGesture,
        MouseNav::OpenCascade,
        MouseNav::OpenInventor,
        MouseNav::OpenScad,
        MouseNav::Revit,
        MouseNav::TinkerCad,
        MouseNav::Touchpad,
        MouseNav::Den,
    ];

    /// The stable name used in the settings file and in the catalogue key.
    pub fn code(self) -> &'static str {
        match self {
            MouseNav::QymCad => "qymcad",
            MouseNav::Cad => "cad",
            MouseNav::Blender => "blender",
            MouseNav::Gesture => "gesture",
            MouseNav::MayaGesture => "maya",
            MouseNav::OpenCascade => "opencascade",
            MouseNav::OpenInventor => "openinventor",
            MouseNav::OpenScad => "openscad",
            MouseNav::Revit => "revit",
            MouseNav::TinkerCad => "tinkercad",
            MouseNav::Touchpad => "touchpad",
            MouseNav::Den => "den",
        }
    }

    /// TURNING THE MODEL: every gesture of the layout that does it, as the program the layout is named after has them,
    /// read from that program's handling of the events rather than from its help lines, which lag behind it.
    pub fn rotates(self) -> &'static [Gesture] {
        match self {
            MouseNav::QymCad => const { &[Gesture::any()] },
            MouseNav::Cad => const { &[Gesture::of(&[MIDDLE, LEFT]), Gesture::of(&[MIDDLE, RIGHT]), Gesture::of(&[RIGHT]).shift()] },
            MouseNav::Blender => const { &[Gesture::of(&[MIDDLE])] },
            MouseNav::Gesture | MouseNav::OpenScad | MouseNav::OpenInventor | MouseNav::Den => const { &[Gesture::of(&[LEFT])] },
            MouseNav::MayaGesture => const { &[Gesture::of(&[LEFT]).alt()] },
            MouseNav::OpenCascade => const { &[Gesture::of(&[RIGHT]).ctrl()] },
            MouseNav::Revit => const { &[Gesture::of(&[MIDDLE]).shift()] },
            MouseNav::TinkerCad => const { &[Gesture::of(&[RIGHT])] },
            MouseNav::Touchpad => const { &[Gesture::of(&[]).alt()] },
        }
    }

    /// MOVING THE VIEW SIDEWAYS: every gesture of the layout that does it.
    pub fn pans(self) -> &'static [Gesture] {
        match self {
            MouseNav::QymCad => const { &[Gesture::any().shift()] },
            MouseNav::Cad => const { &[Gesture::of(&[MIDDLE]), Gesture::of(&[RIGHT]).ctrl()] },
            MouseNav::Blender => const { &[Gesture::of(&[MIDDLE]).shift(), Gesture::of(&[LEFT, RIGHT])] },
            MouseNav::Revit => const { &[Gesture::of(&[MIDDLE]), Gesture::of(&[LEFT, RIGHT])] },
            MouseNav::OpenCascade => const { &[Gesture::of(&[MIDDLE]), Gesture::of(&[MIDDLE]).ctrl()] },
            MouseNav::OpenInventor => const { &[Gesture::of(&[MIDDLE]), Gesture::of(&[LEFT]).ctrl().shift(), Gesture::of(&[]).ctrl().shift()] },
            MouseNav::TinkerCad | MouseNav::Den => const { &[Gesture::of(&[MIDDLE])] },
            MouseNav::Gesture | MouseNav::OpenScad => const { &[Gesture::of(&[RIGHT])] },
            MouseNav::MayaGesture => const { &[Gesture::of(&[MIDDLE]).alt()] },
            MouseNav::Touchpad => const { &[Gesture::of(&[]).shift()] },
        }
    }

    /// ZOOMING BY A MOVEMENT, up and down, besides the wheel: every gesture of the layout that does it.
    pub fn zooms(self) -> &'static [Gesture] {
        match self {
            MouseNav::QymCad | MouseNav::Gesture | MouseNav::TinkerCad => &[],
            MouseNav::Cad => const { &[Gesture::of(&[RIGHT]).ctrl().shift()] },
            MouseNav::Blender | MouseNav::Revit => const { &[Gesture::of(&[MIDDLE]).ctrl(), Gesture::of(&[RIGHT]).ctrl().shift()] },
            MouseNav::OpenInventor => const { &[Gesture::of(&[LEFT, MIDDLE]), Gesture::of(&[MIDDLE]).ctrl(), Gesture::of(&[RIGHT]).ctrl().shift()] },
            MouseNav::OpenScad => const { &[Gesture::of(&[MIDDLE]), Gesture::of(&[RIGHT]).shift(), Gesture::of(&[MIDDLE]).shift()] },
            MouseNav::MayaGesture => const { &[Gesture::of(&[RIGHT]).alt()] },
            MouseNav::OpenCascade => const { &[Gesture::of(&[LEFT]).ctrl()] },
            MouseNav::Touchpad => const { &[Gesture::of(&[]).ctrl().shift()] },
            MouseNav::Den => const { &[Gesture::of(&[MIDDLE, LEFT])] },
        }
    }

    /// DOES A MOVEMENT SIDEWAYS ZOOM TOO? Under the pen's layout right brings the view nearer and left takes it away, as
    /// up and down do; every other layout zooms by up and down alone, as its program does.
    pub fn zooms_sideways(self) -> bool {
        self == MouseNav::Den
    }

    /// TILTING THE VIEW about the line of sight: the left and right buttons together under Gesture, as its program has
    /// it; no other layout tilts.
    pub fn tilts(self) -> &'static [Gesture] {
        match self {
            MouseNav::Gesture => const { &[Gesture::of(&[LEFT, RIGHT])] },
            _ => &[],
        }
    }

    /// The first way of turning the model - the one a layout is known by.
    pub fn rotate(self) -> Gesture {
        self.rotates()[0]
    }

    /// The first way of moving the view - the one a layout is known by.
    pub fn pan(self) -> Gesture {
        self.pans()[0]
    }

    /// TAKING A PART WITHOUT ITS GIZMO: ours alone - Shift and the left button, the drag begun on the part; begun on
    /// empty space it is still ours to move the view. Every other layout is the one of its own program and does what
    /// that program does, which has no such gesture. Reported behaviour: the middle and the right button begun on a
    /// part carried the part instead of the view. Decided 28.09: the view keeps every button, and a part is taken
    /// with Shift and the left one.
    pub fn take_a_part(self) -> Gesture {
        match self {
            MouseNav::QymCad => Gesture::of(&[LEFT]).shift(),
            _ => Gesture::NONE,
        }
    }

    /// Whether the wheel zooms under this layout.
    pub fn wheel_zooms(self) -> bool {
        self != MouseNav::Touchpad
    }

    /// THE FRAME OF SELECTION: the gesture that draws it, and whether it may start on the model (`true`) or from empty
    /// space only. The layouts of other programs draw it as their programs do - a bare left drag from anywhere, or Shift
    /// and the left one where the bare left turns the model; two draw none, their left drag turns the model wherever it
    /// starts. Ours draws it with the bare left from empty space: begun on the model, the left
    /// drag turns it.
    pub fn frames(self) -> Option<(Gesture, bool)> {
        match self {
            MouseNav::QymCad => Some((Gesture::of(&[LEFT]), false)),
            MouseNav::Cad | MouseNav::Blender | MouseNav::Revit | MouseNav::TinkerCad | MouseNav::Touchpad | MouseNav::OpenCascade => Some((Gesture::of(&[LEFT]), true)),
            MouseNav::Gesture | MouseNav::MayaGesture | MouseNav::Den => Some((Gesture::of(&[LEFT]).shift(), true)),
            MouseNav::OpenInventor | MouseNav::OpenScad => None,
        }
    }

    /// A SHORT CLICK OF THE MIDDLE BUTTON LOOKS AT THE POINT UNDER IT - the view turns about it from then on, and it
    /// comes to the middle of the view - under the layouts whose programs do so; ours, OpenCascade, OpenSCAD, TinkerCAD
    /// and the touchpad give the middle click nothing.
    pub fn middle_click_looks(self) -> bool {
        matches!(self, MouseNav::Cad | MouseNav::Blender | MouseNav::Revit | MouseNav::OpenInventor | MouseNav::MayaGesture | MouseNav::Gesture)
    }

    /// DOES A CLICK IN THE 3D VIEW TAKE WHAT IS UNDER IT, with the modifiers held now? One layout turns the model on
    /// the bare left button, and its program selects with Ctrl or Shift and the left button; a bare click there
    /// takes nothing. In a sketch the click is the sketch's own, as it is in that program's edit mode.
    pub fn click_takes(self, ctx: &egui::Context) -> bool {
        self != MouseNav::OpenInventor || ctx.input(|i| i.modifiers.ctrl || i.modifiers.command || i.modifiers.shift)
    }

    /// The catalogue key holding this layout's name.
    pub fn key(self) -> String {
        format!("settings-mouse-{}", self.code())
    }

    /// The catalogue key of the line describing what the layout does.
    pub fn hint_key(self) -> String {
        format!("settings-mouse-{}-hint", self.code())
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Settings {
    /// WHERE THE PANELS STAND, but only what the person MOVED. The defaults come from the code: writing them
    /// into the file would freeze them, and a panel added later would never appear for anyone who had ever
    /// saved. An empty list therefore means "as the workbenches registered them", not "no panels".
    pub layout: Vec<(String, qymcad_shell::Slot)>,
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
    /// HOW MANY CORES THE KERNEL MAY TAKE for a rebuild. Zero means all but one; ONE means single-threaded.
    ///
    /// A number rather than a number plus a tick: "one core" is the off switch, and a separate flag beside it
    /// would allow the pair "off, eight cores", which means nothing. Measured on the reference documents: on all
    /// cores the scenario document rebuilds in 17.2 s against 20.3 s
    /// on one. All but one by default, so the machine stays usable while a heavy rebuild runs - and so that a
    /// person whose computer is busy with something else can give the kernel less.
    pub kernel_threads: i32,
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
    /// the GPU viewport (otherwise the CPU raster)
    pub gpu_viewport: bool,
    /// The camera's projection.
    pub projection: Projection,
    /// How a face is lit.
    pub shading: Shading,
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
    /// THE NUMBERS OF THE POINTS of a sketch beside them: off by default - to a person drawing they mean nothing and
    /// clutter the sheet; on for looking into a sketch (`draw_sketch_points`)
    pub show_point_numbers: bool,
    /// WHAT THE LABEL OF A SKETCH DIMENSION SAYS beside its value: the name of a driver (`w = 110`) and the formula it
    /// is set by (`2*w+10 = 110`); both on reads `w = 2*w+10 = 110`
    pub dim_show_name: bool,
    pub dim_show_formula: bool,
    /// the size of the labels of sketch dimensions, px
    pub dim_font: f32,
    /// how the text of a linear dimension is turned: level whatever the line, or along its dimension line
    pub dim_text: DimTextTurn,
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

    // --- WHAT THE PROGRAM OPENS WITH. Two independent answers, not one setting with three states. ---
    /// REOPEN THE PROJECT OF THE PREVIOUS SESSION.
    ///
    /// On by default, decided 25.09 on a report: the project open when the program closed is open when it starts.
    /// Somebody who opens the CAD to try something turns it off in the settings; with no previous project the start
    /// screen comes up.
    #[serde(default = "default_true")]
    pub open_last: bool,
    /// LET THE START SCREEN COME UP BY ITSELF on an empty document.
    ///
    /// Separate from `open_last` because the two answer different questions: one is about which document,
    /// the other about whether to be greeted. With it off an empty document simply opens, and the screen
    /// stays reachable from the menu.
    #[serde(default = "default_true")]
    pub show_start_screen: bool,
    /// WHICH BUTTON MOVES THE VIEW. See `MouseNav`.
    #[serde(default = "default_mouse_nav")]
    pub mouse_nav: MouseNav,
    /// Where the view zooms from: the cursor, or the middle of the viewport.
    #[serde(default = "default_zoom_at")]
    pub zoom_at: ZoomAt,
    /// The exception to that while a command is open.
    #[serde(default = "default_zoom_editing")]
    pub zoom_editing: ZoomWhileEditing,
    /// What the view turns about: the middle of the view, or the point under the pointer (see `OrbitAbout`).
    #[serde(default = "default_orbit_about")]
    pub orbit_about: OrbitAbout,
    /// How often to ask whether a newer version exists (see `UpdateCheck`).
    #[serde(default = "default_update_check")]
    pub update_check: UpdateCheck,
    /// When it was last asked, in seconds since the epoch. `0` means never.
    ///
    /// Kept beside the setting rather than worked out from anything: without it "once a day" would mean
    /// "at every start", since nothing else in the program remembers that a start happened.
    #[serde(default)]
    pub update_last_checked: u64,
    /// ASK FOR THE UNITS AND THE SCALE ON EVERY IMPORT. Off, the window comes up only for a file without units and
    /// for a model under 1 mm or over 10 m.
    #[serde(default)]
    pub import_ask_always: bool,
    /// THE UNIT LAST CHOSEN FOR A FORMAT WITHOUT UNITS, by the format's name: the next file of it comes in the same.
    #[serde(default)]
    pub import_units: std::collections::BTreeMap<String, String>,
}

fn default_orbit_about() -> OrbitAbout {
    OrbitAbout::ViewCentre // the program's own way: the view turns as it always has
}

/// The factory layout: ours.
fn default_zoom_at() -> ZoomAt {
    ZoomAt::Cursor // what every CAD a person comes from does
}

fn default_update_check() -> UpdateCheck {
    UpdateCheck::Daily
}

fn default_zoom_editing() -> ZoomWhileEditing {
    ZoomWhileEditing::PartCentre
}

fn default_mouse_nav() -> MouseNav {
    MouseNav::QymCad
}

/// A `serde(default)` for a flag whose factory value is "on".
fn default_true() -> bool {
    true
}

/// WHAT THE PROGRAM OPENS WITH, decided in one place out of the settings.
///
/// A named answer rather than two ifs inside the launch: the launch cannot be run by a check, so a decision
/// living there is a decision nobody measures. Four combinations of two flags give three outcomes, and each
/// one is somebody's first minute.
#[derive(Debug, PartialEq, Clone)]
pub enum Opening {
    /// Load this file - the project of the previous session.
    LastProject(String),
    /// An empty document. `start_screen` says whether the start screen comes up by itself.
    Empty { start_screen: bool },
}

/// The decision itself. `last` is the path remembered from the previous session, if there is one.
///
/// A remembered path that no longer names a file is NOT a reason to show an empty window in silence: the
/// answer falls back to an empty document, and the start screen comes up if it is allowed to.
pub fn opening(set: &Settings, last: Option<&str>) -> Opening {
    match last.filter(|p| set.open_last && std::path::Path::new(p).is_file()) {
        Some(p) => Opening::LastProject(p.to_string()),
        None => Opening::Empty { start_screen: set.show_start_screen },
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            layout: Vec::new(),
            language: String::new(),
            scheme: "dark".into(),
            viewcube_size: 1,
            msaa: 4,
            autosave_secs: 180,
            undo_cap: 40,
            ghost_alpha: 115,
            kernel_threads: 0,   // all the cores but one
            persp_fov_deg: 35.5, // equals the former PERSP_FOV_HALF_TAN of 0.32: 2*atan(0.32)
            hotkeys: Default::default(),
            help_lang: String::new(),
            help_external: false,
            gpu_viewport: true,
            projection: Projection::Ortho,
            shading: Shading::Smooth,
            show_contours: true,
            show_joints: true,
            show_interference: false,
            snap: Snapping::default(),
            auto_constrain: true,
            show_point_numbers: false,
            dim_show_name: false,
            dim_show_formula: false,
            dim_font: DIM_FONT_DEFAULT,
            dim_text: DimTextTurn::default(),
            defaults: Defaults::default(),
            ui_scale: default_ui_scale(),
            recent: Vec::new(),
            recent_limit: default_recent_limit(),
            pick_precision: default_pick_precision(),
            open_last: true,
            show_start_screen: true,
            mouse_nav: default_mouse_nav(),
            zoom_at: default_zoom_at(),
            update_check: default_update_check(),
            update_last_checked: 0,
            zoom_editing: default_zoom_editing(),
            orbit_about: default_orbit_about(),
            import_ask_always: false,
            import_units: Default::default(),
        }
    }
}

/// THE COMPONENT PATTERN COMMAND (in an assembly): what is being replicated and how.
///
/// Separate from the BODY pattern (`ArrayParams`): they have different sources (a component against a body),
/// different results (instance components against one merged body) and different edits. Shared state would mean
/// that editing one silently changes the other.
#[derive(Clone, Debug)]
pub struct CompArrayCmd {
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
pub struct SplitParams {
    pub plane: Option<qymcad_core::feature::SketchPlane>,
}

/// THE OPTIONS OF THE INDIVIDUAL Part tools that have a switch or two each.
#[derive(Clone, Copy, Default, PartialEq)]
pub struct FeatOptions {
    /// the shell: which side of the opened face the wall goes to
    pub shell_side: qymcad_core::feature::ShellSide,
    /// the patch: meet the edges smoothly (tangent to the neighbouring faces) rather than by position alone
    pub patch_tangent: bool,
    /// the mirror: keep the original
    pub mirror_keep: bool,
    /// the rotation angle used when inserting
    pub rot_deg: f64,
}

/// THE KIND OF A TEXT in the sketch selection (`SketchSelection::items`), beside a point (0), an entity (1) and an axis
/// (3): a text kept by its id, so the move tools take it, alone or with lines. Reported behaviour (issue #32): a text was
/// picked apart from the selection, and Move and Rotate never saw it.
pub const SEL_TEXT: u8 = 4;

/// DOES THE TEXT TOOL SHOW ITS LABEL AT THE POINTER - where a click would put a new one. Not while a label is being
/// edited: the tool is taken up for the edit, and its ghost followed the pointer over the label being edited. Reported
/// behaviour (found checking issue #32): a double click on a text, and a second copy of it slid about under the cursor.
pub fn text_ghost_shown(armed: &Armed, inline: &InlineEdit) -> bool {
    armed.draw_kind() == 11 && inline.text().is_none()
}

/// A RECTANGLE IS TAKEN WHOLE by the tools that carry geometry - move, copy, rotate, the patterns: a click on one side of a
/// rectangle picked that side alone, and a turn of it was a turn of one line out of a shape held square. Every side of
/// a rectangle with one side picked joins the selection.
pub fn take_whole_rects(project: &Project, si: usize, sel_sk: &mut SketchSelection) {
    let Some(s) = project.sketches.get(si) else { return };
    let picked: Vec<Id> = sel_sk.items.iter().filter(|(k, _)| *k == 1).map(|(_, id)| *id).collect();
    for r in s.rects.iter().filter(|r| r.sides.iter().any(|e| picked.contains(e))) {
        for side in r.sides {
            if !picked.contains(&side) {
                sel_sk.items.push((1, side));
            }
        }
    }
}

/// The indices of the texts selected in sketch `si`, in the order they were picked.
pub fn sel_text_indices(project: &Project, sel_sk: &SketchSelection, si: usize) -> Vec<usize> {
    let Some(s) = project.sketches.get(si) else { return Vec::new() };
    sel_sk.items.iter().filter(|(k, _)| *k == SEL_TEXT).filter_map(|(_, id)| s.texts.iter().position(|t| t.id == *id)).collect()
}

/// AN EDITING TOOL OF THE SKETCH: a button that works on the selected entities, or waits in hand for them.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EditTool {
    /// Delete the selected entities.
    Delete,
    /// Mirror them about an axis or a line, pointed at after they are picked.
    Mirror,
    /// Copy them in a row.
    LinearPattern,
    /// Copy them round a centre.
    CircularPattern,
    /// Round the corner of two picked lines.
    Fillet,
    /// Cut the corner of two picked lines.
    Chamfer,
    /// Lay a copy at a distance beside them.
    Offset,
}

impl EditTool {
    /// Every editing tool, in the order of the button numbers.
    pub const ALL: [EditTool; 7] = [EditTool::Delete, EditTool::Mirror, EditTool::LinearPattern, EditTool::CircularPattern, EditTool::Fillet, EditTool::Chamfer, EditTool::Offset];

    /// THE BUTTON'S NUMBER: the help table (`qymcad_help::map::SKETCH`, under "mod") knows the tools by it, and
    /// that crate knows nothing of this one.
    pub fn code(self) -> u8 {
        match self {
            EditTool::Delete => 0,
            EditTool::Mirror => 1,
            EditTool::LinearPattern => 2,
            EditTool::CircularPattern => 3,
            EditTool::Fillet => 4,
            EditTool::Chamfer => 5,
            EditTool::Offset => 6,
        }
    }
}

/// THE SKETCH SELECTION AND THE DEFERRED ACTION ON IT.
///
/// "What is selected" and "what to do once enough has been gathered" are one state: a constraint or an editing
/// tool is pressed and then geometry is clicked. As separate fields this was expressed so that the deferred
/// action could outlive a cleared selection and fire on THE NEXT, unrelated set.
#[derive(Clone, Default, PartialEq)]
pub struct SketchSelection {
    /// what is selected: (the kind - a point, an entity and so on - plus its Id)
    pub items: Vec<(u8, Id)>,
    /// awaiting a set for a constraint (the constraint's code)
    pub constraint: Option<u8>,
    /// awaiting a set for an editing tool
    pub modify: Option<EditTool>,
    /// WHAT THE MIRROR IS ABOUT TO REFLECT, while the axis is being pointed at.
    ///
    /// Non-empty means the tool is on its SECOND step: it has been told what, and waits to be told about
    /// what. It used to have no second step at all - the axis was whichever line happened to be in the
    /// selection (and got mirrored too, being in the same set), or silently Y.
    pub mirror_of: Vec<Id>,
    /// THE LINES OF `items` CLICKED AT THEIR MIDDLE, where the triangle of the midpoint showed. Picked as lines; a
    /// constraint that takes points reads them as their midpoints (`qymcad_sketch::middles_for`).
    pub at_middle: Vec<Id>,
}

impl SketchSelection {
    /// Clear the selection, together with whatever was waiting for it. A deferred action without a selection means nothing.
    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

/// WHAT THE TRIAL BUILD SAYS of the command in hand (written by the part workbench, read by the drawing).
#[derive(Clone, Debug, PartialEq)]
pub enum Trial {
    /// It builds, or it is not one the geometry bounds, or it is not ready to be tried.
    Clear,
    /// Still being built on the worker: the command waits for the answer before it applies.
    Checking,
    /// It would not build: the key of the field the refusal is said beside, and the words.
    Refused(String, String),
}

impl Trial {
    /// The refusal, if there is one.
    pub fn refusal(&self) -> Option<(String, String)> {
        match self {
            Trial::Refused(k, w) => Some((k.clone(), w.clone())),
            _ => None,
        }
    }
}

/// The triangles of the faces a trial build adds, in the frame of the context: what the preview shows.
pub type TrialFaces = std::sync::Arc<Vec<[[f64; 3]; 3]>>;

/// What a trial came to: the verdict, and the faces the build adds.
#[derive(Clone)]
pub struct TrialAnswer {
    pub verdict: Trial,
    pub faces: TrialFaces,
}

/// A trial in flight or done, for the command's state key: the worker writes its answer when it has one.
#[derive(Clone)]
pub struct TrialSlot {
    pub key: u64,
    pub answer: std::sync::Arc<std::sync::Mutex<Option<TrialAnswer>>>,
}

/// Where the trial of the command in hand is kept in the frame's memory.
pub fn trial_slot_id() -> egui::Id {
    egui::Id::new("feat_cmd_trial")
}

/// Where the frame the trial last answered for the command in hand is kept.
fn trial_seen_id() -> egui::Id {
    egui::Id::new("feat_cmd_trial_seen")
}

/// THE TRIAL IN THE SLOT IS THE ONE OF THE COMMAND IN HAND, as of this frame: said by whoever asked for it with the
/// state of the command.
pub fn trial_is_current(ctx: &egui::Context) {
    let frame = ctx.cumulative_frame_nr();
    ctx.data_mut(|d| d.insert_temp(trial_seen_id(), frame));
}

/// THE FACES THE COMMAND IN HAND WILL ADD, as its trial built them - once the trial has answered that it builds.
///
/// Only a trial asked for in this frame or the one before is drawn: the slot lives in the memory of the window, not of
/// the document, and one left by a command no longer asked about is of another command, or of another document.
/// Reported behaviour: a project closed, a new one made, Extrude taken on a sketch - and the walls a trial of a hole
/// had built in the closed project stood green over the new sketch.
pub fn trial_faces(ctx: &egui::Context) -> Option<TrialFaces> {
    let seen = ctx.data(|d| d.get_temp::<u64>(trial_seen_id()))?;
    if seen + 1 < ctx.cumulative_frame_nr() {
        return None;
    }
    let slot = ctx.data(|d| d.get_temp::<TrialSlot>(trial_slot_id()))?;
    let got = slot.answer.lock().ok()?.clone()?;
    (got.verdict == Trial::Clear && !got.faces.is_empty()).then_some(got.faces)
}

/// A POINT AN AXIS IS PICKED THROUGH: a datum point by its id, or a vertex of a body by its edge and end - both followed
/// by the axis as they move - with where it stood when picked, in the frame of the context.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AxisPoint {
    Datum(Id, [f64; 3]),
    Vertex { body: Id, edge: u32, end: bool, at: [f64; 3] },
}

impl AxisPoint {
    pub fn at(&self) -> [f64; 3] {
        match *self {
            AxisPoint::Datum(_, at) | AxisPoint::Vertex { at, .. } => at,
        }
    }
}

/// THE PART PATTERN'S PARAMETERS: up to three linear directions, or a circular one about an axis.
/// Eleven fields described ONE intention, and "two directions" could end up switched on with a count of 1, or an
/// axis chosen for a linear pattern.
#[derive(Clone, Copy, Default, PartialEq)]
pub struct ArrayParams {
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
    /// the first of two points an axis is being picked through, until the second comes
    pub axis_first: Option<AxisPoint>,
}

/// BUILDING A DATUM: which mode it is in and what references have been gathered.
#[derive(Clone, Default)]
pub struct DatumCommand {
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
#[derive(Clone, Default)]
pub struct DimTool {
    /// the first reference pointed at, and what has been clicked
    pub first: Option<DimRef>,
    pub pick: Vec<Id>,
    /// typing the value: buffer, focus, editing an existing dimension
    pub buf: String,
    pub focus: bool,
    /// THE FIELD OF THE SIZE OF A CIRCLE OR AN ARC WAS TYPED IN since it opened: only a value typed is laid as a
    /// dimension, as the width and the height of a rectangle are (`Placing::typed`)
    pub typed: bool,
    pub edit: Option<(Id, String, String)>,
    /// THE FIELD OF A DIMENSION JUST MADE: (its constraint, the length of the undo list when it was made). Its value
    /// joins the step that made it - making a dimension and typing its value is one act, undone by one Ctrl+Z - as
    /// long as no other step came between.
    pub fresh: Option<(usize, usize)>,
    /// THE SKETCH AS IT WAS before a provisional length was put on a picked line: Esc puts it back whole, the
    /// points the length brought with it (an axis of the sketch made on demand) as well.
    pub before: Option<Box<qymcad_core::model::Sketch>>,
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
pub struct JointCommand {
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
    /// the joint in `edit` is the one just made by the second pick, still inside its open operation: Apply or Enter
    /// keeps it as one step, Esc, Cancel or another tool takes it away as if it had never been
    pub creating: bool,
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

impl JointCommand {
    /// Take the rigid mate and wait for the two faces.
    pub fn start_rigid_pick(&mut self) {
        self.new_kind = qymcad_core::feature::JointKind::Rigid;
        self.pick_faces = true;
    }
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
            creating: false,
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

/// THE SECTION TOOL: the cutting plane and its gizmo editing.
#[derive(Clone, Default)]
pub struct SectionTool {
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

impl SectionTool {
    /// Turn the section off: no plane, no pick, no drag.
    pub fn put_away(&mut self) {
        (self.plane, self.pick, self.drag, self.drag_anchor) = (None, false, false, None);
    }
}

/// THE HOLE COMMAND: the kind, how it is placed, and the sketch holding the points.
#[derive(Clone, Copy, Default, Debug)]
pub struct HoleCommand {
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
pub struct ChamferParams {
    pub mode: qymcad_core::feature::ChamferMode,
    pub flip: bool,
    pub ref_face: u32,
    pub pick_ref: bool,
}

/// DRAFT PARAMETERS: the neutral face (the angle is measured from it) and the direction.
#[derive(Clone, Copy, Default, Debug)]
pub struct DraftParams {
    pub neutral: u32,
    pub pick_neutral: bool,
    pub flip: bool,
}

/// PRIMITIVE PARAMETERS: how many sides (for a prism) and where it goes.
#[derive(Clone, Copy, Default, Debug)]
pub struct PrimParams {
    pub n: u32,
    pub place: Option<[f64; 3]>,
    pub frame: Option<[f64; 12]>,
}

/// SKETCH PATTERN PARAMETERS (a tool, not a feature): the kind, editing an existing one, the centre.
#[derive(Clone, Copy, Default)]
pub struct PatternTool {
    pub edit: Option<usize>,
    pub center: Option<Point2>,
}

/// THE COMPONENT GIZMO IN AN ASSEMBLY: what is being dragged, along which axis or ring.
#[derive(Clone, Copy, Default)]
pub struct CompGizmo {
    pub drag: Option<(Id, [f64; 12], [f64; 3], f64)>,
    pub snap: bool,
    pub axis: Option<u8>,
    pub ring: Option<u8>,
}

/// TYPING A ROTATION ANGLE: the value, the buffer and the focus — one unfinished input.
#[derive(Clone, Default)]
pub struct RotInput {
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
pub enum Dragging {
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
    pub fn active(&self) -> bool {
        *self != Dragging::None
    }

    pub fn clear(&mut self) {
        *self = Dragging::None;
    }

    pub fn pt(&self) -> Option<(usize, usize)> {
        match *self {
            Dragging::Point(a, b) => Some((a, b)),
            _ => None,
        }
    }

    pub fn handle(&self) -> Option<(usize, usize, usize)> {
        match *self {
            Dragging::Handle(a, b, c) => Some((a, b, c)),
            _ => None,
        }
    }

    pub fn mov(&self) -> Option<(usize, Vec<Id>)> {
        match self {
            Dragging::Move(a, ids) => Some((*a, ids.clone())),
            _ => None,
        }
    }

    pub fn dim(&self) -> Option<usize> {
        match *self {
            Dragging::Dim(i) => Some(i),
            _ => None,
        }
    }

    pub fn note(&self) -> Option<usize> {
        match *self {
            Dragging::Note(i) => Some(i),
            _ => None,
        }
    }

    pub fn text(&self) -> Option<usize> {
        match *self {
            Dragging::Text(i) => Some(i),
            _ => None,
        }
    }
}

/// WHAT A GIZMO STANDS ON, for a number typed at it: a body in a part (by its mesh), or a part in an assembly.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum GizmoOf {
    Body(usize),
    Part(Id),
}

/// THE BODY GIZMO: moving or rotating a body with the mouse, plus typing the same value as a number.
/// The same pairing as `CompGizmo` has for a component: "what is being dragged" and "what is being
/// typed" are one intent, and as separate fields the input could outlive the release of the gizmo.
#[derive(Clone, Default)]
pub struct BodyGizmo {
    pub axis: Option<u8>,
    pub ring: Option<u8>,
    pub drag: Option<(usize, [f64; 3], f64)>,
    pub snap: bool,
    /// the gizmo itself is being dragged (an arrow or a ring has been grabbed)
    pub dragging: bool,
    /// typed input: (what the gizmo stands on, axis, is it a rotation?) plus its buffer and focus
    pub num: Option<(GizmoOf, u8, bool)>,
    pub num_buf: String,
    pub num_focus: bool,
}

/// IN-PLACE EDITING OF A SKETCH ELEMENT: what exactly is being edited right on the canvas. Exactly one
/// thing is edited at a time — hence an enum rather than four `Option`s.
#[derive(Clone, Copy, Default, PartialEq)]
pub enum InlineEdit {
    #[default]
    None,
    Note(usize),
    Text(usize),
    Dim(usize),
    Circle(Id),
}

impl InlineEdit {
    pub fn note(&self) -> Option<usize> {
        match *self {
            InlineEdit::Note(i) => Some(i),
            _ => None,
        }
    }
    pub fn text(&self) -> Option<usize> {
        match *self {
            InlineEdit::Text(i) => Some(i),
            _ => None,
        }
    }
    pub fn dim(&self) -> Option<usize> {
        match *self {
            InlineEdit::Dim(i) => Some(i),
            _ => None,
        }
    }
    pub fn circle(&self) -> Option<Id> {
        match *self {
            InlineEdit::Circle(i) => Some(i),
            _ => None,
        }
    }
    pub fn clear(&mut self) {
        *self = InlineEdit::None;
    }
}

/// WHAT THE COMMAND IS POINTING AT BY A CLICK RIGHT NOW. The modes are MUTUALLY EXCLUSIVE — in the
/// code that was held by hand, with comments next to it saying "exclusive: reset the others". An enum
/// makes it a property of the type: a second mode simply cannot be turned on without turning the first
/// one off.
#[derive(Clone, Copy, Default, PartialEq)]
pub enum Picking {
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
    /// A TOOL WAITING FOR THE SKETCH IT NEEDS, carrying the kind of command that was pressed.
    ///
    /// Reported behaviour: a tool that needs geometry from a sketch used to write a line into the status
    /// bar and not start. Nobody reads the status bar at the moment of pressing, so what a person saw was
    /// a button that did nothing. The kind travels with the wait because the answer must open the command
    /// that was ASKED FOR - a wait that opens the wrong one is worse than not opening any.
    SketchFor(u8),
}

impl Picking {
    pub fn clear(&mut self) {
        *self = Picking::None;
    }
    pub fn is_sketch_plane(&self) -> bool {
        matches!(self, Picking::SketchPlane(_))
    }
    pub fn plane_face(&self) -> Option<Id> {
        match *self {
            Picking::SketchPlane(f) => f,
            _ => None,
        }
    }
    pub fn set_plane_face(&mut self, f: Option<Id>) {
        if self.is_sketch_plane() {
            *self = Picking::SketchPlane(f);
        }
    }
    pub fn replace_sketch(&self) -> Option<usize> {
        match *self {
            Picking::ReplaceSketch(i) => Some(i),
            _ => None,
        }
    }
    pub fn fillet_all(&self) -> bool {
        *self == Picking::FilletAll
    }
    pub fn contour(&self) -> Option<ContourSlot> {
        match *self {
            Picking::Contour(s) => Some(s),
            _ => None,
        }
    }
    /// Which command is waiting for a sketch, if any.
    pub fn sketch_for(&self) -> Option<u8> {
        match *self {
            Picking::SketchFor(k) => Some(k),
            _ => None,
        }
    }

    /// THE WORDS FOR "THIS PICK IS CANCELLED", for the modes that Escape simply puts down.
    ///
    /// The Escape ladder used to carry a rung per mode, three of them in a row, each three lines long and
    /// each saying the same two things: clear the pick, name it. That is a hand-written list of the variants
    /// of this very enum living somewhere else - it falls behind the moment a mode is added, silently,
    /// because a mode nobody put in the ladder is simply a pick Escape does not release.
    ///
    /// `None` for the modes that need MORE than clearing: a contour pick returns the borrowed view, and
    /// "fillet all" belongs to the command that armed it.
    pub fn cancel_key(&self) -> Option<&'static str> {
        match *self {
            Picking::ReplaceSketch(_) => Some("in-sketch-move-cancelled"),
            Picking::SketchPlane(_) => Some("in-sketch-plane-cancelled"),
            Picking::SketchFor(_) => Some("in-sketch-wait-cancelled"),
            Picking::FilletAll => Some("in-fillet-all-cancelled"),
            Picking::None | Picking::Contour(_) => None,
        }
    }
}

/// EDITING AN ANNOTATION (a note or a text) in a sketch: what is being edited and what has been typed.
#[derive(Clone, Default)]
pub struct AnnotEdit {
    pub note: Option<usize>,
    pub note_buf: String,
    pub text: Option<usize>,
    pub text_buf: String,
    pub text_h: f64,
    /// The text field of the popup has just been opened and asks for the caret.
    ///
    /// Without it the popup opened with the focus nowhere: a person had to click into the field before
    /// typing, and Enter - which is what applies the change - did nothing at all until they did.
    pub text_focus: bool,
}

/// THE MEASURING TOOL: whether it is on, together with the points collected — one state, not a flag
/// apart from the points (otherwise the points outlive the switch-off and pop up the next time it is
/// turned on).
#[derive(Clone, Default)]
pub struct Measuring {
    pub pts: Vec<Point2>,
}

impl Measuring {
    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

/// THE SELECTION IN THE TREE: the multiple selection, the range anchor and the rubber-band box.
#[derive(Clone, Default)]
pub struct TreeSelection {
    pub multi: Vec<Id>,
    pub anchor: Option<Id>,
    pub box_start: Option<Pos2>,
}

/// AN IMPORT WAITING TO BE PLACED: the curves have been read, but where they go has not been said yet.
/// Together with the drawing points that came from the same import, this is one unfinished action.
#[derive(Clone, Default)]
pub struct PendingImport {
    /// the curves read, their source and the file name
    pub curves: Option<(Vec<qymcad_core::geom::ProfEdge>, Option<Id>, String)>,
    /// the points to be drawn once it has been placed
    pub draw_pts: Option<Vec<Point2>>,
}

impl PendingImport {
    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

/// A FILE READ AND LAID IN, WAITING FOR ITS UNITS AND SCALE. The bodies are already in the document inside an open
/// edit, so the answer shows on screen as it is given; Enter closes the edit, Esc rolls it back.
pub struct ImportScale {
    /// The file's name, for the window's line.
    pub file: String,
    /// The format's name: the key the unit last chosen for it is remembered under.
    pub format: String,
    /// The file carries no unit (STL, OBJ, PLY): the window offers units, not only a factor.
    pub unitless: bool,
    /// The factor asked for, from the file's own numbers.
    pub factor: f64,
    /// The factor the bodies in the document are at now.
    pub applied: f64,
    /// The sides of the box of everything that came in, as the file has it.
    pub span: [f64; 3],
    /// Every mesh that came in: its body, and the mesh and faces as read.
    pub meshes: Vec<(Id, qymcad_core::geom::Mesh, Vec<qymcad_core::geom::MeshFace>)>,
    /// Every solid that came in: its body, and the shape as read.
    pub solids: Vec<(Id, qymcad_kernel::Shape)>,
    /// Every component that came in with the file's tree: its id, and where the file places it.
    pub places: Vec<(Id, [f64; 12])>,
    /// The factor the import stood at when it was asked about again from its node; `None` for a file just read. The
    /// window then applies a new factor rather than bringing a file in, and Esc puts this one back.
    pub again: Option<f64>,
}

/// THE SKETCH WORKING SESSION: which one is open for editing and which one was the last. Kept apart,
/// "open" could be cleared while "last" still held a different sketch — and the next command took hold
/// of the wrong one.
#[derive(Clone, Copy, Default)]
pub struct SketchSession {
    /// the sketch open for editing (None means not in sketch mode)
    pub editing: Option<Id>,
    /// the last one worked on (for commands that act "on the last sketch")
    pub last: Option<Id>,
}

/// THE GEOMETRY SELECTED FOR COMMANDS: profiles, faces, edges. "Faces of which body" used to live apart
/// from the faces themselves, and the set could outlive a change of body.
#[derive(Clone, Default)]
pub struct GeomSelection {
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
        self.described = Some(match self.described.take() {
            Some(Query::Adjacent(inner)) => Query::Adjacent(Box::new(Query::Union(inner, Box::new(Query::Id(fid))))),
            _ => Query::Adjacent(Box::new(Query::Id(fid))),
        });
    }

    /// THE DESCRIPTION AS A FEATURE RECORDS IT, when one was given: it survives an edit that adds elements.
    pub fn described_ref(&self) -> Option<qymcad_core::refs::Ref> {
        self.described.clone().map(qymcad_core::refs::Ref::many)
    }

    /// THE SELECTION AS A FEATURE RECORDS IT: the description when one was given, otherwise the list `picks` of what
    /// was clicked. One rule for every command that records a selection, on creation and on an edit alike.
    pub fn recorded(&self, picks: &[u32]) -> qymcad_core::refs::Ref {
        self.described_ref().unwrap_or_else(|| qymcad_core::refs::Ref::picks(picks))
    }
}

/// THE CLIPBOARD: sketch geometry and tree nodes are different things, but they share one state of "what has been copied".
#[derive(Clone, Default)]
pub struct Clipboard {
    pub geom: Option<qymcad_core::model::GeomClip>,
    pub geom_pending: Option<(Vec<Id>, bool)>,
    /// THE PLACE OF A PASTE IS AWAITED: a ghost follows the pointer, a click places it. A copy or a cut waits for its
    /// base point alone and fills the clipboard (`geom_pending`); nothing follows it until a paste.
    pub geom_place: bool,
    pub tree: Option<TreeClip>,
    pub tree_multi: Option<(Vec<Id>, bool)>,
    pub os_ping: bool,
}

impl Clipboard {
    /// A COPY (or with `cut`, a cut) OF THE GEOMETRY `eids` taken: its base point is awaited; a paste waiting before it
    /// goes.
    pub fn arm_copy(&mut self, eids: Vec<Id>, cut: bool) {
        (self.geom_pending, self.geom_place) = (Some((eids, cut)), false);
    }

    /// An unfinished copy or paste of geometry put down.
    pub fn drop_geom(&mut self) {
        (self.geom_pending, self.geom_place) = (None, false);
    }
}

/// The orbit camera for the software 3D view (the projection comes from `Settings::projection`).
#[derive(Clone, Copy)]
pub struct Cam3 {
    pub yaw: f64,
    pub pitch: f64,
    pub scale: f32,
    pub target: [f64; 3],
    pub init: bool,
    /// The scale the view was last framed at. The wheel's limits are taken from it, so a model of 38 m and one of
    /// 0.1 mm both zoom on from where they were framed.
    pub fit: f32,
    /// THE TILT about the line of sight, in radians: turned by the tilt of the Gesture layout, back to nothing with any
    /// standard view of the cube.
    pub roll: f64,
}

impl Default for Cam3 {
    fn default() -> Self {
        // Front isometric: the camera sits at +X -Y +Z (front = -Y towards the viewer), the usual choice.
        // +Y runs up and away, so the "top" of a sketch on the XY plane matches the "top" in 3D.
        Self { yaw: -0.7, pitch: 0.6, scale: 4.0, target: [0.0; 3], init: false, fit: 4.0, roll: 0.0 }
    }
}

impl Cam3 {
    pub fn basis(&self) -> ([f64; 3], [f64; 3], [f64; 3]) {
        let dir = [self.pitch.cos() * self.yaw.cos(), self.pitch.cos() * self.yaw.sin(), self.pitch.sin()];
        let fwd = [-dir[0], -dir[1], -dir[2]];
        // TOP AND BOTTOM VIEWS (fwd parallel to Z): world Z as the "up" gives a zero cross product, so the
        // basis degenerates and the picture collapses. In that case world Y is taken as the reference
        // "up" (the ViewCube's top no longer breaks the view).
        let ref_up = if fwd[2].abs() > 0.999 { [0.0, 1.0, 0.0] } else { [0.0, 0.0, 1.0] };
        let right = v_norm(v_cross(fwd, ref_up));
        let up = v_norm(v_cross(right, fwd));
        if self.roll == 0.0 {
            return (right, up, fwd);
        }
        let (c, sn) = (self.roll.cos(), self.roll.sin());
        let turn = |a: [f64; 3], b: [f64; 3], ka: f64, kb: f64| [a[0] * ka + b[0] * kb, a[1] * ka + b[1] * kb, a[2] * ka + b[2] * kb];
        (turn(right, up, c, sn), turn(up, right, c, -sn), fwd)
    }
}

/// THE STATE OF THE HELP WINDOW, gathered out of the pile of window flags.
///
/// Four fields that only ever move together - open, which article, where Back leads, what is typed into
/// the search. In the pile they were four unrelated names among fifteen, and nothing said they belonged
/// to one another.
#[derive(Default)]
pub struct HelpWin {
    pub open: bool,
    pub article: String,
    pub back: Vec<String>,
    pub query: String,
}

impl HelpWin {
    /// Step back to the article visited before this one, if there is one.
    pub fn go_back(&mut self) {
        if let Some(prev) = self.back.pop() {
            self.article = prev;
        }
    }

    /// Whether there is somewhere to go back to.
    pub fn can_go_back(&self) -> bool {
        !self.back.is_empty()
    }
}

/// One editable dimension of the active Part command: the label + the key (for `feat_dims`) + the current value +
/// the input buffer (a number OR an expression like `w/2+3`). An on-screen field like the sketch dimensions have.
#[derive(Clone)]
pub struct CmdParam {
    /// A CATALOGUE KEY rather than a finished phrase: the field's label must follow a change of language by itself,
    /// without the command being recreated. The key is stable; the translation is taken at the moment of display.
    pub label: LabelKey,
    /// The dimension's key. A STRING rather than a `&'static str`: a variable fillet has as many fields as there
    /// are vertices in the table, and each has a key of its own of the form `at{vertex descriptor}`. That cannot be
    /// expressed as a list of constants, and a second field mechanism for one tool would fork into copies what
    /// already works: the popup, the expressions, the parametrics, Enter and Esc.
    pub key: String,
    pub val: f64,
    pub txt: String,
    pub lo: f64,
    pub hi: f64,
    /// ZERO IS NO VALUE for this field, though both signs are: a face pushed by 0 is the body unchanged.
    pub nonzero: bool,
    /// WHERE THIS FIELD LIVES IN SPACE. `None` means the command's shared popup; a point means a small window of
    /// its own at that place (a radius at a vertex is shown AT THE VERTEX, otherwise six identical fields in a
    /// column cannot be told apart).
    pub at: Option<[f64; 3]>,
}

impl CmdParam {
    /// The KEY of the field's label. The words are chosen by whoever draws it - a record of the
    /// interface holds keys and never reaches for the dictionary.
    pub fn label_key(&self) -> &'static str {
        self.label.0
    }

    /// Change the label on the fly (on a chamfer the second leg becomes an angle when the mode changes).
    pub fn set_label(&mut self, key: &'static str) {
        self.label = LabelKey(key);
    }

    pub fn new(label: &'static str, key: &str, val: f64, lo: f64, hi: f64) -> Self {
        Self { label: LabelKey(label), key: key.to_string(), val, txt: format!("{val:.2}"), lo, hi, at: None, nonzero: false }
    }

    /// A field AT THE GEOMETRY: the same thing, but with a place of its own in space.
    pub fn at(mut self, p: [f64; 3]) -> Self {
        self.at = Some(p);
        self
    }

    /// A field for which zero is no value.
    pub fn nonzero(mut self) -> Self {
        self.nonzero = true;
        self
    }
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
pub struct SceneBlock {
    /// The key of the FORM and appearance - everything except the position.
    pub shape: u64,
    /// The position these vertices already stand at.
    pub at: [f64; 12],
    /// The number these vertices name: the body's row in the look table the last time the block was used.
    pub body: u32,
    /// SHARED, NOT OWNED OUTRIGHT: the frame takes the block as it is, without copying a single vertex.
    ///
    /// Every rebuild of the scene used to glue all the blocks into one vector purely to hand it to the
    /// upload - a second full copy of the scene, 739 MB of it on the reference engine, at the worst possible
    /// moment. Now the pieces are uploaded one after another, each straight from its block.
    ///
    /// A move still shifts the vertices in place: after the frame has been drawn the block is the only owner,
    /// so `Arc::make_mut` copies nothing.
    /// ALL the geometry of the body. It used to be split into opaque and translucent here, which tied the
    /// DATA to the look: a body turning into a ghost had to be rebuilt to move its vertices into the other
    /// half. The pass is chosen when drawing, from the look table.
    ///
    /// Several parts only when the body is too big for one buffer of the card (see `BLOCK_VERTEX_CAP`);
    /// ordinarily there is exactly one.
    pub parts: Vec<BlockPart>,
}

/// ONE PIECE OF A BODY as the card takes it: the vertices, and the triangles as indices into them.
///
/// A VERTEX OF THE BLOCK IS A VERTEX OF THE MESH, one for one. Each used to be written out three times over -
/// once per triangle that touches it - because the colour and the normal made neighbours differ; now the
/// vertex carries only a position and the number of its body, so the triangles simply point at it.
/// Measured on a closed mesh: about three times fewer vertices, and 4 bytes of index against 20 of vertex.
///
/// The exception is the vertices BORN OF A SECTION: clipping a triangle by the plane makes points that are in
/// no mesh. They are appended at the end of the block and indexed in order.
#[derive(Clone)]
pub struct BlockPart {
    pub verts: std::sync::Arc<Vec<GpuVert>>,
    pub idx: std::sync::Arc<Vec<u32>>,
}

/// HOW BIG ONE PART OF A BLOCK MAY GROW, in vertices and in indices.
///
/// An indexed draw reads from ONE buffer, so a part must fit inside a single piece of the scene, and a piece
/// may not exceed the device's limit on a buffer. The smallest such limit met in practice is 256 MB: 4.19
/// million vertices at 20 bytes is 84 MB, and 16.7 million indices at 4 bytes is 67 MB, so both stay well
/// inside it on any device. A body bigger than that is cut into parts when the block is built.
pub const BLOCK_VERTEX_CAP: usize = 1 << 22;
/// The companion of `BLOCK_VERTEX_CAP` for the triangles: a mesh has about twice as many triangles as
/// vertices, so the indices are the number that runs out first.
pub const BLOCK_INDEX_CAP: usize = 1 << 24;

/// The extent of an extrude or a cut (in place of the magic numbers 0 to 3). The discriminants are kept as the
/// former u8 in case of outside places, but the comparisons go by variant.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum ExtentMode {
    #[default]
    Length, // to a length (one side)
    Symmetric,
    TwoSided, // two sides (the "down" field is active)
    Through,  // through all (operations on a body only)
}

impl ExtentMode {
    pub fn symmetric(self) -> bool {
        matches!(self, ExtentMode::Symmetric)
    }
    pub fn two_sided(self) -> bool {
        matches!(self, ExtentMode::TwoSided)
    }
    pub fn through(self) -> bool {
        matches!(self, ExtentMode::Through)
    }
    /// Restore the mode from a saved feature's flags (through takes priority, then symmetry, then two-sided).
    pub fn from_extent(reach: qymcad_core::feature::Reach, down: f64, through: bool) -> Self {
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

/// The TREE clipboard: what was copied or cut (by stable Id) and which of the two it was.
#[derive(Clone, Copy, PartialEq)]
pub enum TreeClip {
    /// A sketch node (by the sketch's Id).
    Sketch { sid: Id, cut: bool },
    /// A component - a Part or a subassembly (by the component's Id).
    Component { id: Id, cut: bool },
    /// A node of a part's timeline (by the node's Id): pasting opens its tool with its values, to be placed anew.
    Feature { nid: Id },
}

/// A reference for the dimension tool: a point or a straight line (a line or an axis). A dimension between two
/// references: point to point, point to line (a perpendicular), line to a parallel line, or to an axis.
#[derive(Clone, Copy, PartialEq)]
pub enum DimRef {
    Point(Id),
    Line(Id, Id),
}

/// A base edge for building tangents (a tangent arc or a tangent circle): either a straight line (with ends a
/// and b) or a circle or arc (a centre plus a radius).
#[derive(Clone, Copy, PartialEq)]
pub enum EdgeRef {
    Line { a: Id, b: Id },
    Circle { center: Id, r: f64 },
}

pub struct Busy {
    /// When the job began: a long import shows how long it has gone, so that a wait does not look like a hang.
    pub started: std::time::Instant,
    pub label: String,
    /// The rebuild is quiet: the work is small, no window is shown - only the status line.
    pub quiet: bool,
    pub rx: std::sync::mpsc::Receiver<JobResult>,
    /// Only a rebuild has this: there is nothing to interrupt the other tasks with (a file write stopped halfway
    /// is a broken file, and parsing a STEP is all or nothing).
    pub pulse: Option<std::sync::Arc<RegenPulse>>,
    /// The kind of background task - it shows whether another one of the same kind may be queued (two writes of
    /// one file at once are a race between the temporary file and the rename) and what exactly to wait for on exit.
    pub kind: BgKind,
}

/// The sketch's diagnostics in one piece: the degrees of freedom and the redundancy, the free points, THE ARGUING
/// SET of constraints and the redundant ones. Computed in a single pass in `sketch_diag`.
/// The diagnostics of one sketch and what they were computed for.
pub struct SketchStatus {
    pub si: usize,
    /// `sketch_shape_key` at the time
    pub shape: u64,
    /// `sketch_place_key` at the time
    pub place: u64,
    pub diag: SketchDiag,
}

#[derive(Clone, Default)]
pub struct SketchDiag {
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
pub struct Snapshot {
    pub project: Project,
}

/// AN UNFINISHED DRAWING: which shape is being finished right now.
///
/// AN ENUMERATION RATHER THAN THREE `Option`s: exactly ONE THING is ever being drawn, and the type must say so.
/// While these were three independent fields, "a rectangle and an ellipse at once" remained a possible state -
/// held off by discipline in the code rather than by the type system.
#[derive(Clone, Default, PartialEq)]
pub enum PlacingShape {
    #[default]
    None,
    /// a rectangle: two corners and the entities it created
    Rect { a: Point2, b: Point2, ids: Vec<Id> },
    /// a rectangle drawn from the centre: the centre stays put while the width and height are typed,
    /// the corner under the pointer follows them
    RectCenter { center: Point2, corner: Point2, ids: Vec<Id> },
    /// a polygon by its inscribed or circumscribed circle
    Poly(Id),
    /// an ellipse: the entity and its centre
    Ellipse(Id, Point2),
    /// a rectangle reopened by a double click on a side: its width and height dimensions, edited in place
    RectDims(qymcad_core::model::RectDims),
}

/// THE CUSTOM SCHEME SCREEN: what is being edited and what to say about saving.
///
/// The edits go in LIVE - a colour is seen at once, with no Apply. Otherwise choosing a shade would be blind:
/// close the window, look, come back. So only the name of the scheme being edited and a note about the last save
/// live here; the colours themselves live in the scheme, which is already gathered in the list.
#[derive(Clone, Default)]
pub struct SchemeEdit {
    /// whether the colour editor is open
    pub open: bool,
    /// the name in the rename field (empty means no rename is in progress)
    pub rename: String,
    /// what to say about the last save or error
    pub note: String,
}

/// A RELATION BEING PICKED: its kind, the joints already pointed at with their slots, and the number.
///
/// The slot is not picked separately by hand: the kind itself says what is what
/// (`qymcad_core::feature::RelationKind::slots_are_rotations`), and the matching degree of freedom is taken from the joint
/// pointed at. A separate dialogue for choosing the degree is only warranted where a joint has several
/// degrees of the matching sort.
#[derive(Clone)]
pub struct RelationPick {
    pub kind: qymcad_core::feature::RelationKind,
    /// already pointed at, as (joint, slot), in order
    pub picks: Vec<(Id, usize)>,
    /// the relation's number: a gear ratio or a travel per turn, depending on the kind
    pub value: f64,
    pub reversed: bool,
}

impl RelationPick {
    /// Set the kind and the number of the relation being built.
    ///
    /// CHANGING THE KIND CLEARS THE PICKS: the degrees already pointed at were of the right sort for the
    /// PREVIOUS kind, and keeping them would build the relation on the wrong ones. That rule was written
    /// out twice - in the popup and beside it - and two copies of a rule drift.
    pub fn set(&mut self, kind: qymcad_core::feature::RelationKind, value: f64) {
        if self.kind != kind {
            self.picks.clear();
        }
        self.kind = kind;
        self.value = value;
    }
}

impl Default for RelationPick {
    fn default() -> Self {
        Self { kind: qymcad_core::feature::RelationKind::Gear, picks: Vec::new(), value: 1.0, reversed: false }
    }
}

/// DEFAULT VALUES — not the state of a command but SETTINGS: a command opens with them, and they
/// outlive its closing. They used to sit among the command's own fields and so looked like state.
#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Defaults {
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

/// SNAPPING WHILE DRAWING — SETTINGS ONLY.
///
/// The hint saying "where it snapped just now" used to live here too, and it is derived from the
/// cursor: it is recomputed every frame and has no place among settings. While it sat inside, this
/// record could not be a setting as a whole — and being a setting as a whole is exactly what makes
/// saving automatic (see `Settings`). The hint moved to `App::snap_hint`.
#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Snapping {
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

/// AN UNDO STEP IS AN OPERATION. Not "the document changed somehow" but "an Extrude was carried out":
/// a step has a name, and that name is shown. The stack used to hold nameless snapshots, because the
/// boundary of a step was decided by THE FRAME rather than by the command.
pub struct Step {
    pub name: String,
    pub snap: Snapshot,
}

/// The state of the "Save as a standard part" dialogue (right-click on a component in the tree).
/// The manifest metadata, plus the chosen category (a path relative to the user's directory), plus a
/// pre-rendered PNG preview of the body. Writing it out goes through `subproject_of` and `save_part`.
pub struct SavePartDialog {
    /// The source component (a Part or a subassembly) the standard part is taken from.
    pub component: qymcad_core::model::Id,
    pub name: String,
    pub description: String,
    /// Tags separated by commas or spaces (parsed when written out).
    pub tags: String,
    /// The category path relative to `<data>/library/parts` (e.g. "Profiles/Aluminium"). Empty = the root.
    pub category: String,
    /// The user's existing categories (folders) — for picking one quickly from chips.
    pub known_cats: Vec<String>,
    /// The raw preview of the body (256^2, rendered at the moment it opened). Encoded to PNG when written out.
    pub preview: Option<egui::ColorImage>,
    /// The GPU texture of the preview (loaded lazily from `preview` to show it in the window).
    pub tex: Option<egui::TextureHandle>,
}

/// The contour slot of a sweep or a loft for which a contour is being picked in the half-sketcher (as in
/// Extrude: the sketch lies flat, a contour is clicked). The sweep profile, the sweep path, or the i-th
/// section of a loft.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ContourSlot {
    SweepProfile,
    SweepPath,
    LoftSection(usize),
}

/// The active drag of a joint gizmo's handle. The frame (`o` and `dir`) is fixed when the drag starts.
#[derive(Clone, Copy)]
pub struct JointGizDrag {
    pub jid: Id,
    pub slot: u8,   // 0 = angle, 1 = offset, 2 = offset2
    pub ring: bool, // true means a rotation about `dir`, false a translation along it
    pub start: f64, // the parameter's value when the drag started
    pub amt: f64,   // accumulated so far (degrees or millimetres)
    pub o: [f64; 3],
    pub dir: [f64; 3],
}

/// What the click pick of a circular pattern's axis caught: a datum AXIS, a STRAIGHT edge (an index into
/// `edge_polys`), or the axis of a CYLINDRICAL or conical face (the body plus the face's persistent id).
#[derive(Clone, Copy, PartialEq)]
pub enum AxisHit {
    Datum(Id),
    Edge(usize),
    Face(Id, u32),
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
pub struct LabelKey(&'static str);

/// A piece of a mesh file on its way into the document: a piece as the file names it (see `qymcad_io::NamedMesh`) with
/// the faces found on its mesh.
pub struct MeshPiece {
    pub name: String,
    pub mesh: qymcad_core::geom::Mesh,
    pub faces: Vec<qymcad_core::geom::MeshFace>,
    pub color: Option<[u8; 3]>,
    pub place: [f64; 12],
    /// the colour of every triangle, where the file gives one
    pub tri_colors: Vec<[u8; 3]>,
    /// the groups of the file the piece stands in, from the top down
    pub within: Vec<qymcad_core::model::FileGroup>,
}

/// The result of a background (worker thread) import or export, arriving at the interface over a channel.
pub enum JobResult {
    /// An exact file (STEP, IGES) was imported: the bodies (mesh plus B-rep faces) + the solids' live shapes +
    /// the file's path and format.
    ExactImported { path: String, format: qymcad_kernel::ExactFormat, bodies: Vec<qymcad_core::geom::Built>, shapes: Vec<qymcad_kernel::Shape>, nodes: Vec<qymcad_kernel::ImportNode> },
    /// An IGES with no surfaces, read as a drawing: the curves go into a sketch the way DXF does, and `note` is
    /// what the status adds (a library's cells are shown, entities were not drawn).
    DrawingRead { path: String, curves: Vec<qymcad_core::geom::ProfEdge>, note: String },
    /// Meshes were imported (STL, OBJ): each piece is a body with its detected faces (the detection runs in the
    /// worker too - it is heavy on large meshes).
    MeshImported { path: String, format: MeshFormat, pieces: Vec<MeshPiece> },
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
pub struct RegenPulse {
    /// The node being computed right now, and the total number of nodes.
    pub done: std::sync::atomic::AtomicUsize,
    pub total: std::sync::atomic::AtomicUsize,
    /// Shared with the kernel doing the work, which looks at it inside a long node too.
    pub stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// The fingerprint (`Project::rebuild_key`) of the document the rebuild computes: once the live one differs, the
    /// result is stale before it arrives.
    pub stamp: u64,
}

impl RegenPulse {
    pub fn progress(&self) -> (usize, usize) {
        use std::sync::atomic::Ordering::Relaxed;
        (self.done.load(Relaxed), self.total.load(Relaxed))
    }
    pub fn ask_stop(&self) {
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
    fn stopped(&self) -> bool {
        self.stop_asked()
    }
}

/// The kinds of background work. There used to be a SINGLE slot, and Save during the restoration of the imports'
/// B-rep overwrote its receiver: the thread finished computing and sent the shapes into a closed channel, so the
/// B-rep did not appear until a restart.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BgKind {
    /// Writing the project to disk (by hand or by autosave).
    Save,
    /// Restoring the imported solids' B-rep from the embedded STEP.
    ImportShapes,
    /// A rebuild of the timeline by the kernel - modal (edits are forbidden while it runs).
    Regen,
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

pub fn v_norm(a: [f64; 3]) -> [f64; 3] {
    let l = v_dot(a, a).sqrt().max(1e-9);
    [a[0] / l, a[1] / l, a[2] / l]
}

pub fn v_cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

pub fn v_dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// THE OUTLINE OF A HOLE BEFORE ENTER, in the frame the rebuild drills it in (`pl`: its Z the outward normal of the
/// face, the hole going down it): the rims of the bore at the face and at the bottom with four lines between, and the
/// recess over it - a counterbore's step (kind 1) or a countersink's cone (kind 2) from `dia2` down `depth2`. It follows
/// the fields: a diameter of 6 draws rims 6 across, 12 draws them 12 across.
pub fn hole_outline(pl: &[f64; 12], tool: qymcad_core::model::HoleTool) -> Vec<Vec<[f64; 3]>> {
    let at = |r: f64, a: f64, z: f64| qymcad_core::feature::apply12(pl, [r * a.cos(), r * a.sin(), z]);
    let rim = |r: f64, z: f64| (0..=32).map(|i| at(r, std::f64::consts::TAU * i as f64 / 32.0, z)).collect::<Vec<_>>();
    let quarters = |r0: f64, z0: f64, r1: f64, z1: f64| (0..4).map(move |i| std::f64::consts::FRAC_PI_2 * i as f64).map(move |a| vec![at(r0, a, z0), at(r1, a, z1)]);
    let (r, depth) = (tool.diameter / 2.0, tool.depth.abs());
    let mut out = vec![rim(r, 0.0), rim(r, -depth)];
    out.extend(quarters(r, 0.0, r, -depth));
    if tool.kind != 0 && tool.dia2 > tool.diameter && tool.depth2 > 0.0 {
        let r2 = tool.dia2 / 2.0;
        out.push(rim(r2, 0.0));
        if tool.kind == 1 {
            out.push(rim(r2, -tool.depth2));
            out.extend(quarters(r2, 0.0, r2, -tool.depth2));
        } else {
            out.extend(quarters(r2, 0.0, r, -tool.depth2));
        }
    }
    out
}

/// THE DRAFTED FACE BEFORE ENTER: the border of the face as it will lean, each point moved across the pull by its
/// height over the neutral plane times tan(angle) - out below the neutral face, in above it, as the rebuild tilts it
/// (a side 10 high drafted 3 deg leans out 10 * tan 3 deg = 0.52 at the bottom). The border is the sides of the face's
/// triangles that no other triangle of the face shares. A face square to the pull (a cap) has nothing to lean and
/// draws nothing.
pub fn draft_outline(mesh: &qymcad_core::geom::Mesh, face: &qymcad_core::geom::MeshFace, origin: [f64; 3], pull: [f64; 3], angle_deg: f64) -> Vec<Vec<[f64; 3]>> {
    let pull = v_norm(pull);
    let across = v_sub(face.normal, [pull[0] * v_dot(face.normal, pull), pull[1] * v_dot(face.normal, pull), pull[2] * v_dot(face.normal, pull)]);
    if v_dot(across, across).sqrt() < 1e-6 {
        return Vec::new();
    }
    let across = v_norm(across);
    let tan = angle_deg.to_radians().tan();
    let mut sides: std::collections::HashMap<(u32, u32), u32> = std::collections::HashMap::new();
    for &t in &face.triangles {
        let tri = mesh.tris[t as usize];
        for k in 0..3 {
            let (a, b) = (tri[k], tri[(k + 1) % 3]);
            *sides.entry((a.min(b), a.max(b))).or_default() += 1;
        }
    }
    let lean = |i: u32| {
        let v = mesh.verts[i as usize];
        let p = [v.x, v.y, v.z];
        let h = v_dot(v_sub(p, origin), pull);
        let k = -h * tan;
        [p[0] + across[0] * k, p[1] + across[1] * k, p[2] + across[2] * k]
    };
    let mut out: Vec<Vec<[f64; 3]>> = sides.into_iter().filter(|(_, n)| *n == 1).map(|((a, b), _)| vec![lean(a), lean(b)]).collect();
    out.sort_by(|x, y| x[0].partial_cmp(&y[0]).unwrap_or(std::cmp::Ordering::Equal));
    out
}

/// A REGULAR POLYGON FROM ITS TWO CLICKS: (centre, a vertex), by the mode of the bar. The second click lies on the
/// polygon in every mode, as it does in the professional systems:
/// - 0 inscribed - the centre and a vertex;
/// - 1 circumscribed - the centre and the middle of an edge: the vertices stand r / cos(pi/n) out, turned by pi/n;
/// - 2 by edge - the two ends of an edge, the polygon to the left of the first -> second.
pub fn polygon_from_clicks(a: Point2, b: Point2, n: u32, mode: u8) -> (Point2, Point2) {
    let half = std::f64::consts::PI / n.max(3) as f64;
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len = (dx * dx + dy * dy).sqrt().max(1e-6);
    match mode {
        1 => {
            let (r, ang) = (len / half.cos(), dy.atan2(dx) + half);
            (a, Point2::new(a.x + r * ang.cos(), a.y + r * ang.sin()))
        }
        2 => {
            let apothem = len / (2.0 * half.tan());
            let c = Point2::new((a.x + b.x) / 2.0 - dy / len * apothem, (a.y + b.y) / 2.0 + dx / len * apothem);
            (c, b)
        }
        _ => (a, b),
    }
}

/// What is made along an edge: a fillet of a radius, or a chamfer of a leg on either face.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Blend {
    Round(f64),
    /// A chamfer: the first leg, the second, and the normal of the reference face the first leg lies on, when one is
    /// named - the kernel lays the first leg there; with none named the legs take the faces in the order met.
    Cut(f64, f64, Option<[f64; 3]>),
}

/// The point of triangle `t` nearest to `p` (by the regions of the triangle's plane).
pub fn closest_on_triangle(p: [f64; 3], t: &[[f64; 3]; 3]) -> [f64; 3] {
    let [a, b, c] = *t;
    let (ab, ac, ap) = (v_sub(b, a), v_sub(c, a), v_sub(p, a));
    let at = |u: f64, v: f64| [a[0] + ab[0] * u + ac[0] * v, a[1] + ab[1] * u + ac[1] * v, a[2] + ab[2] * u + ac[2] * v];
    let (d1, d2) = (v_dot(ab, ap), v_dot(ac, ap));
    if d1 <= 0.0 && d2 <= 0.0 {
        return a;
    }
    let bp = v_sub(p, b);
    let (d3, d4) = (v_dot(ab, bp), v_dot(ac, bp));
    if d3 >= 0.0 && d4 <= d3 {
        return b;
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        return at(d1 / (d1 - d3), 0.0);
    }
    let cp = v_sub(p, c);
    let (d5, d6) = (v_dot(ab, cp), v_dot(ac, cp));
    if d6 >= 0.0 && d5 <= d6 {
        return c;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        return at(0.0, d2 / (d2 - d6));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        return [b[0] + (c[0] - b[0]) * w, b[1] + (c[1] - b[1]) * w, b[2] + (c[2] - b[2]) * w];
    }
    let denom = 1.0 / (va + vb + vc);
    at(vb * denom, vc * denom)
}

/// A side of a mesh triangle, from `a` to `b`, with the triangle's normal and its third corner `far`.
struct TriSide {
    a: [f64; 3],
    b: [f64; 3],
    normal: [f64; 3],
    far: [f64; 3],
}

/// A face met along a piece of an edge: its normal, and the unit direction it runs on in, away from the edge.
#[derive(Clone, Copy)]
struct FaceRun {
    normal: [f64; 3],
    away: [f64; 3],
}

/// The two faces either side of a piece of an edge.
#[derive(Clone, Copy)]
struct FacePair {
    one: FaceRun,
    other: FaceRun,
}

/// At a vertex of the edge: where the blend meets each face, and the centre of a fillet's arc.
#[derive(Clone, Copy)]
struct Rail {
    q1: [f64; 3],
    q2: [f64; 3],
    centre: [f64; 3],
}

/// THE OUTLINE OF A BLEND ALONG AN EDGE, before Enter: the two lines where a fillet (or a chamfer) meets the faces
/// either side of the edge, then its section at both ends - an arc for a fillet, a straight cut for a chamfer. It
/// follows the value typed: a radius of 2 on a square edge sets the lines 2 back from it, a radius of 4 sets them 4.
///
/// The faces are told by the triangles of the body's mesh with a side along a piece of the edge: the edge's polyline
/// is cut finer than the mesh (24 pieces against one side of a block's face), so the pieces are matched to the sides
/// by where they lie, not by their vertices. A piece with only one
/// face found, or two faces within 2.5 deg of each other, has nothing to set back from and draws nothing. The
/// setback of a fillet is r * tan(bend / 2): r on a square edge, less on an obtuse one.
pub fn edge_blend_outline(mesh: &qymcad_core::geom::Mesh, poly: &[[f32; 3]], blend: Blend) -> Vec<Vec<[f64; 3]>> {
    let n = poly.len();
    if n < 2 {
        return Vec::new();
    }
    let pt = |p: [f32; 3]| [p[0] as f64, p[1] as f64, p[2] as f64];
    let add = |p: [f64; 3], d: [f64; 3], k: f64| [p[0] + d[0] * k, p[1] + d[1] * k, p[2] + d[2] * k];
    let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
    for p in poly {
        for i in 0..3 {
            lo[i] = lo[i].min(p[i] as f64 - 0.01);
            hi[i] = hi[i].max(p[i] as f64 + 0.01);
        }
    }
    // the triangles within the reach of the blend around the edge: a point set back along a curved face is laid back on
    // it - along the tangent it leaves a cylinder of R5 by 2.1 mm at a setback of 8.6
    let reach = match blend {
        Blend::Round(r) => r * 2.0,
        Blend::Cut(a, b, _) => a.max(b) * 2.0,
    } + 0.1;
    let near: Vec<[[f64; 3]; 3]> = (0..mesh.tris.len())
        .map(|t| mesh.triangle(t))
        .map(|[a, b, c]| [[a.x, a.y, a.z], [b.x, b.y, b.z], [c.x, c.y, c.z]])
        .filter(|v| !(0..3).any(|i| v.iter().all(|p| p[i] < lo[i] - reach) || v.iter().all(|p| p[i] > hi[i] + reach)))
        .collect();
    // only a curved face lays the point back: where the nearest triangle turns away from the face at the edge (by more
    // than 0.5 deg); a flat face keeps the point on its plane, even past the triangles a mesh happens to have there
    let on_surface = |q: [f64; 3], n: [f64; 3]| -> [f64; 3] {
        let best = near.iter().map(|t| (closest_on_triangle(q, t), t)).min_by(|a, b| v_dot(v_sub(a.0, q), v_sub(a.0, q)).total_cmp(&v_dot(v_sub(b.0, q), v_sub(b.0, q))));
        match best {
            Some((c, t)) if v_dot(v_norm(v_cross(v_sub(t[1], t[0]), v_sub(t[2], t[0]))), n).abs() < 0.99996 => c,
            _ => q,
        }
    };
    // the sides of the triangles that touch the edge's box, with the triangle's normal and its third corner: few,
    // whatever the size of the mesh, and each piece of the edge is looked up among them only. The third corner says
    // which way the face runs on from the edge - the one thing the two normals cannot tell an inner corner from an
    // outer one by
    let mut sides: Vec<TriSide> = Vec::new();
    for t in 0..mesh.tris.len() {
        let [a, b, c] = mesh.triangle(t);
        let v = [[a.x, a.y, a.z], [b.x, b.y, b.z], [c.x, c.y, c.z]];
        if (0..3).any(|i| v.iter().all(|p| p[i] < lo[i]) || v.iter().all(|p| p[i] > hi[i])) {
            continue;
        }
        let nrm = v_norm(v_cross(v_sub(v[1], v[0]), v_sub(v[2], v[0])));
        for k in 0..3 {
            sides.push(TriSide { a: v[k], b: v[(k + 1) % 3], normal: nrm, far: v[(k + 2) % 3] });
        }
    }
    // per piece of the edge: the faces met, each as its normal and the way it runs on from the edge, across it
    let mut normals: Vec<Vec<FaceRun>> = vec![Vec::new(); n - 1];
    for (seg, ns) in normals.iter_mut().enumerate() {
        let (p, q) = (pt(poly[seg]), pt(poly[seg + 1]));
        let (m, d) = ([(p[0] + q[0]) / 2.0, (p[1] + q[1]) / 2.0, (p[2] + q[2]) / 2.0], v_norm(v_sub(q, p)));
        for TriSide { a, b, normal: nrm, far } in &sides {
            let ab = v_sub(*b, *a);
            let len = v_dot(ab, ab).sqrt();
            if len < 1e-9 || v_dot(ab, d).abs() < 0.98 * len {
                continue;
            }
            // the middle of the piece lies on the side, to 0.001 mm plus 2 % of the side for a curved edge's chord
            let t = (v_dot(v_sub(m, *a), ab) / (len * len)).clamp(0.0, 1.0);
            let off = v_sub(m, [a[0] + ab[0] * t, a[1] + ab[1] * t, a[2] + ab[2] * t]);
            if v_dot(off, off).sqrt() < 1e-3 + 0.02 * len {
                let to = v_sub(*far, m);
                ns.push(FaceRun { normal: *nrm, away: v_norm(v_sub(to, add([0.0; 3], d, v_dot(to, d)))) });
            }
        }
    }
    // the two faces of each piece, kept in step along the edge so that a line does not jump from one face to the other
    let mut faces: Vec<Option<FacePair>> = Vec::with_capacity(n - 1);
    let mut prev: Option<FacePair> = None;
    for ns in &normals {
        let pair = ns.first().and_then(|n1| ns.iter().find(|n2| v_dot(n1.normal, n2.normal) < 0.999).map(|n2| FacePair { one: *n1, other: *n2 }));
        let pair = match (pair, prev) {
            (Some(p), Some(q)) if v_dot(p.one.normal, q.one.normal) < v_dot(p.one.normal, q.other.normal) => Some(FacePair { one: p.other, other: p.one }),
            (p, _) => p,
        };
        if pair.is_some() {
            prev = pair;
        }
        faces.push(pair);
    }
    // per vertex of the edge: the two points on the faces and, for a fillet, the centre of its arc
    let mut rails: Vec<Rail> = Vec::with_capacity(n);
    for (k, p) in poly.iter().enumerate() {
        let Some(FacePair { one: FaceRun { normal: n1, away: t1 }, other: FaceRun { normal: n2, away: t2 } }) = faces[k.min(n - 2)].or_else(|| faces[k.saturating_sub(1)]) else { continue };
        let p = pt(*p);
        // t1, t2: in each face, away from the edge. An outer edge has the faces run on away from each other's normal,
        // an inner one towards it; the fillet's centre is inside the part for the first and in the air for the second
        let inner = v_dot(t1, n2) > 0.0;
        let (q1, q2, c) = match blend {
            Blend::Round(r) => {
                let bend = v_dot(n1, n2).clamp(-1.0, 1.0).acos();
                let back = r * (bend / 2.0).tan();
                let q1 = add(p, t1, back);
                (q1, add(p, t2, back), add(q1, n1, if inner { r } else { -r }))
            }
            Blend::Cut(a, b, reference) => {
                // the two faces of an edge are never parallel, so the reference is the one its normal lies along
                let swap = reference.is_some_and(|r| v_dot(n2, r).abs() > v_dot(n1, r).abs());
                let (a, b) = if swap { (b, a) } else { (a, b) };
                (add(p, t1, a), add(p, t2, b), p)
            }
        };
        let (q1, q2) = (on_surface(q1, n1), on_surface(q2, n2));
        rails.push(Rail { q1, q2, centre: c });
    }
    if rails.len() < 2 {
        return Vec::new();
    }
    let section = |Rail { q1, q2, centre: c }: Rail| -> Vec<[f64; 3]> {
        match blend {
            Blend::Round(r) => (0..=8)
                .map(|i| {
                    let t = i as f64 / 8.0;
                    let (u, w) = (v_sub(q1, c), v_sub(q2, c));
                    add(c, v_norm([u[0] + (w[0] - u[0]) * t, u[1] + (w[1] - u[1]) * t, u[2] + (w[2] - u[2]) * t]), r)
                })
                .collect(),
            Blend::Cut(..) => vec![q1, q2],
        }
    };
    vec![rails.iter().map(|r| r.q1).collect(), rails.iter().map(|r| r.q2).collect(), section(rails[0]), section(rails[rails.len() - 1])]
}

/// A vertex of a body for the GPU. The colour and the normal do NOT depend on the camera (the light is
/// of the world), so the buffer is re-uploaded only when the scene changes and not when it is rotated.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuVert {
    /// The position in world coordinates (with the transform of the owning component already applied).
    pub pos: [f32; 3],
    /// WHOSE VERTEX THIS IS - the number of the body in the scene's own list, not its colour.
    ///
    /// The colour used to be baked in here, and with it the highlight and the ghosting. That made the
    /// appearance part of the geometry: hovering over a part, or stepping into a subassembly, changed the
    /// vertices themselves - all 23 million of them on the reference engine - and the whole scene went to the
    /// card again. Reported as "cannot enter the assembly, it hangs".
    ///
    /// Now the vertex says only WHOSE it is, and what that body looks like right now lives in a small table
    /// beside it (`BodyLook`), which is rewritten in full on every frame and costs bytes rather than megabytes.
    pub body: u32,
    /// THE NORMAL, PACKED INTO FOUR BYTES - one signed byte per axis, which is 1/127 of accuracy and more
    /// than shading needs.
    ///
    /// It is here for SMOOTH shading only: there the normal belongs to the vertex, not to the triangle, and
    /// no derivative can recover it. Flat shading leaves it zero and the fragment takes the normal from the
    /// derivatives of the world position, as before.
    ///
    /// The size is an admitted trade: 20 bytes against the 16 a flat-only vertex would need. Splitting the
    /// two into separate formats and pipelines would bring those four bytes back; it is written down in the
    /// plan rather than done here, because the freeze on entering an assembly is worth more than 20 % of the
    /// scene's size.
    pub nrm: u32,
}

/// WHERE THE SCENE IS LIT FROM. One direction, named once: the raster and the card must agree, or the same
/// part comes out shaded differently in a picture of the viewport and on screen.
pub fn scene_light() -> [f64; 3] {
    let v: [f64; 3] = [0.35, 0.5, 0.78];
    let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    [v[0] / n, v[1] / n, v[2] / n]
}

/// ONE PIECE OF THE SCENE handed to the card: the vertices of one body (or of the section caps) and the row
/// of the look table they belong to.
pub struct ScenePiece {
    pub verts: std::sync::Arc<Vec<GpuVert>>,
    /// The triangles as indices into `verts`.
    pub idx: std::sync::Arc<Vec<u32>>,
    pub body: u32,
    /// How many rows of the look table from `body` on the vertices name: the body's own, and one per colour of its faces.
    pub rows: u32,
}

/// THE WHOLE SCENE as it goes to the card: the pieces in a stable order, and the table saying what each body
/// looks like at this moment.
///
/// The two travel apart on purpose. The pieces change when a body changes SHAPE or place; the table changes
/// when a body is highlighted, ghosted or recoloured - which is every time the pointer moves over the model
/// or a person steps into a subassembly. Keeping them together meant re-uploading the geometry for a change
/// of colour: 739 MB on the reference engine, and a frozen window.
pub struct GpuScene {
    pub pieces: Vec<ScenePiece>,
    pub looks: Vec<BodyLook>,
}

/// HOW A BODY LOOKS RIGHT NOW: its own colour, already brightened, and the state it is in.
///
/// One record per body of the scene, handed to the card as a table. Everything that depends on the light -
/// and that is the whole of the shading - is computed in the fragment, because the light depends on the
/// normal and the normal on the triangle.
#[repr(C)]
#[derive(Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct BodyLook {
    /// The body's own colour after `brighten`, as rgb in the low three bytes.
    pub tint: u32,
    /// THE FLAGS OF THE LOOK, one bit each: `LOOK_HOT`, `LOOK_GHOST`, `LOOK_CAP`.
    ///
    /// Bits rather than a number of three, and that is not taste. Selection and ghostliness are two
    /// different answers: the first decides the COLOUR, the second decides WHICH PASS the triangle is drawn
    /// in - the blended one, with no z-write. A selected neighbour's body is both at once: painted as
    /// selected, and still drawn in the blended pass. Written as one number of three, that case collapsed
    /// into "selected" and a ghost of a neighbouring part jumped into the opaque pass the moment it was
    /// picked.
    pub state: u32,
}

/// The body is selected: it is painted as selected, whatever else it is.
pub const LOOK_HOT: u32 = 1;
/// The body belongs to a neighbouring context: it is drawn in the blended pass, with no z-write.
pub const LOOK_GHOST: u32 = 2;
/// Not a surface of a part but the cap of a section: a fill, and it is not shaded.
pub const LOOK_CAP: u32 = 4;

// THE NORMAL IS NOT CARRIED ANY MORE - it is computed in the fragment from the derivatives of the world
// position. It was needed for one thing only: culling faces turned away from the eye (the light is already
// baked into the colour), and a flat normal of a triangle is exactly what the derivatives give.
//
// Measured on the reference engine: 23 104 941 vertices at 32 bytes made 739 MB of scene; without the normal
// the vertex is 16 bytes and the scene is 370 MB.

/// A navigation that could throw away unsaved changes: it happens at once (when there are no edits) or after the
/// "save the changes?" dialogue has been answered.
#[derive(Clone)]
pub enum Nav {
    /// A new PART document: a root assembly plus one empty part, with the part active.
    New,
    /// A new ASSEMBLY document: the root only, and it is active. An empty part in an assembly document is litter
    /// that would have to be deleted by hand.
    NewAssembly,
    /// A new document FROM A TEMPLATE. It goes through the same guard against unsaved edits that Open does.
    /// Pick a template in the chooser, opened in the folder of templates, and start a new document from it.
    NewFromTemplate,
    OpenDialog,
    /// Open A SPECIFIC path (the recent files, the start screen). It goes through the same guard against unsaved
    /// edits that Open does: otherwise a click on a recent file would silently lose the work.
    OpenPath(String),
    Exit,
}

/// THE MODEL TREE AS DRAWN: what is being searched for, what is being dragged, and where the rows landed.
#[derive(Default)]
pub struct TreeUi {
    /// The last frame's tree row rectangles - the tests move the mouse over them.
    pub row_rects: Vec<(Id, egui::Rect)>,
    /// The rectangles of the path labels in the parameters window: (the driver's number, where its path was drawn).
    /// Also for the tests - aiming at a label "roughly there" does not work, as a miss confirmed.
    pub drv_path_rects: Vec<(usize, egui::Rect)>,
    /// The build tree search's query. Interface state, not a setting.
    pub search: String,
    /// The tree row grabbed for a move. `None` means nothing is being dragged.
    pub drag: Option<Id>,
}

/// WHAT THE PERSON IS BEING MADE TO WAIT FOR, and since when - so that a wait shorter than an eye blink
/// never puts a spinner on screen.
pub struct Waiting {
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
    pub splash_until: Option<std::time::Instant>,
    /// WHEN THE WRITE BEING WAITED FOR STARTED, and when the waiting card was first drawn.
    ///
    /// The two together keep the card from blinking: nothing is shown before `SAVE_WAIT_GRACE`, and what has
    /// been shown stays at least `SAVE_WAIT_MIN`.
    pub save_since: Option<std::time::Instant>,
    pub save_shown: Option<std::time::Instant>,
}

impl Waiting {
    /// Move the saving clocks BACK by `by`, so that a check can reach a later moment without sleeping.
    ///
    /// Used by the checks: the program never turns its clocks back. It lives here rather than on the
    /// application because the two fields it moves are here, and a check has no business reaching past
    /// the record that owns them.
    pub fn age(&mut self, by: std::time::Duration) {
        if let Some(t) = self.save_since {
            self.save_since = Some(t - by);
        }
        if let Some(t) = self.save_shown {
            self.save_shown = Some(t - by);
        }
    }
}

impl Default for Waiting {
    fn default() -> Self {
        Self { splash_until: Some(std::time::Instant::now() + SPLASH_MIN), save_since: None, save_shown: None }
    }
}

/// WHICH BODIES OCCUPY THE SAME SPACE, and at which revision that was last worked out.
pub struct Interference {
    /// Interference detection (bodies of different parts penetrating one another) in an assembly - a toggle.
    /// It is expensive (a pairwise OCCT boolean), so it is off by default and computed lazily while idle, never
    /// during a drag. The cache holds the pairs of intersecting bodies (Id, Id) plus the `geom_rev` it was computed
    /// at (it is recomputed when the scene changes).
    pub pairs: Vec<(Id, Id)>,
    pub rev: u64,
}

impl Default for Interference {
    fn default() -> Self {
        Self { pairs: Vec::new(), rev: u64::MAX }
    }
}

/// THE INTERFACE'S DEFERRED INTENTIONS: an action is decided within a frame but carried out at a safe point, not
/// in the middle of drawing. Keeping them as separate fields would mean that "delete" and "navigate" could drift
/// apart from whatever produced them.
#[derive(Clone, Default)]
pub struct DeferredUi {
    /// delete what is selected
    pub delete: Option<Sel>,
    /// the confirmation's tick: delete what stands on the node too, rather than leave it red
    pub delete_dependents: bool,
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

/// THE RENAME INPUT: what is being renamed and what has been typed.
#[derive(Clone, Default)]
pub struct RenameInput {
    /// a timeline node / a tree node / a sketch — three different targets of one input
    pub target: Option<Id>,
    pub node: Option<RenameNode>,
    pub sketch: Option<Id>,
    pub buf: String,
    pub focus: bool,
}

/// SKETCH PATTERN PARAMETERS (the linear step and the count).
#[derive(Clone, Copy, Default)]
pub struct ArrayTool {
    pub n: u32,
    pub dx: f64,
    pub dy: f64,
}

/// DRAGGING THE TIMELINE ROLLBACK BAR: the accumulated shift and whether a rollback is still pending.
#[derive(Clone, Copy, Default)]
pub struct RollbackDrag {
    pub accum: f32,
    pub pending: bool,
}

/// WHAT IS UNDER THE CURSOR. Three independent fields could stay filled at once: move the cursor off
/// the sketch onto a joint, and the constraint highlight did not go out, because a different handler
/// was the one that cleared it.
#[derive(Clone, Copy, Default)]
pub struct Hover {
    /// sketch geometry, as (kind, Id)
    pub sketch: Option<(u8, Id)>,
    /// a constraint in the sketch list or glyph
    pub constraint: Option<usize>,
    /// an assembly joint
    pub joint: Option<Id>,
    /// THE SKETCH UNDER THE CURSOR IN THE 3D VIEW, by its index, while a tool waits for one to be named.
    ///
    /// Separate from `sketch`, which holds the geometry hovered INSIDE the flat sketcher: one is an entity
    /// of the sketch being edited, the other is a whole sketch being pointed at from outside it.
    pub sketch_3d: Option<usize>,
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
pub enum TreeDrop {
    /// Go BEFORE the target among its siblings.
    Before,
    /// Go AFTER the target among its siblings.
    After,
    /// Gather the selection and the target into a new subassembly.
    Onto,
}

/// The target of an inline rename of any tree node (sharing the `rename_buf` buffer).
/// The names live in different places: a component's in `components[].name`; a datum's in `planes`,
/// `datum_points` or `datum_axes[].name`; a body's through `set_mesh_name(mi)` (by mesh index). Sketches and
/// features have fields of their own (rename_sketch and rename_target).
#[derive(Clone, Copy, PartialEq)]
pub enum RenameNode {
    Component(Id),
    Plane(Id),
    DatumPoint(Id),
    DatumAxis(Id),
    Body(usize),
}

/// HOW LONG THE SPLASH SCREEN IS HELD AT LEAST. Five seconds, even if it opened instantly; if loading
/// takes longer, it stays longer. The splash here is a greeting rather than an indicator: one that
/// flashes for a hundred milliseconds says nothing at all.
pub const SPLASH_MIN: std::time::Duration = std::time::Duration::from_secs(5);

/// Where a part comes from: built in (read-only) or a file of the user's own on disk.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PartSource {
    /// A path inside the built-in catalogue (relative to the root `library/parts`).
    Embedded(String),
    /// The absolute path of a `.qpart` on disk.
    User(std::path::PathBuf),
}

/// WHAT THE REBUILD TOUCHES, and nothing besides.
///
/// Measured rather than guessed: the whole chain - "the document changed" -> "rebuild" -> "wait for the
/// B-rep" -> "commit an undo step" - is twelve methods, 291 lines, and reaches for exactly these ten fields.
///
/// THIS IS THE KEYSTONE OF THE WORKBENCH CRATES. Every workbench calls `mark_dirty_for_rebuild` and
/// `commit_edit` constantly - the assembly alone twenty times - and while those were methods of `App`, every
/// caller had to be a method of `App` too, whatever else it touched. A panel cannot leave for a crate of its
/// own while the commonest thing it does drags the whole application along.
pub struct RebuildCtx<'a> {
    pub project: &'a mut qymcad_core::model::Project,
    /// The flat view, so that an operation that moves a body can ask it to refit.
    pub view: &'a mut View2d,
    pub edits: &'a mut Edits,
    pub regen: &'a mut Rebuilding,
    pub live: &'a mut LiveGeom,
    pub params_seen: &'a mut std::collections::HashMap<String, f64>,
    pub status: &'a mut String,
    pub sel: &'a mut Sel,
    pub active_path: &'a Vec<Id>,
    pub cam: &'a Cam3,
    pub scheme: &'a SchemeUi,
    pub set: &'a Settings,
}

/// WHAT THE PART WORKBENCH TOUCHES.
///
/// Large, and honestly so: fifteen of these fields are the PARAMETER RECORDS OF ITS OWN TOOLS - the
/// chamfer, the shell, the array, the thread, the hole, the sweep and the rest. A workbench holds its own
/// tools; that is what makes it a workbench rather than a folder. The rest is what any tool needs: the
/// document, the command being run, the status line, the selection, the view.
pub struct PartCtx<'a> {
    pub armed: &'a mut Armed,
    pub status: &'a mut String,
    pub cmd: &'a mut FeatCommand,
    pub project: &'a mut qymcad_core::model::Project,
    pub gsel: &'a mut GeomSelection,
    pub sel: &'a mut Sel,
    pub split: &'a mut SplitParams,
    pub opts: &'a mut FeatOptions,
    pub feat: &'a mut FeatTarget,
    pub mirror: &'a mut MirrorParams,
    pub loft: &'a mut LoftParams,
    pub chamfer: &'a mut ChamferParams,
    pub arr: &'a mut ArrayParams,
    pub datum: &'a mut DatumCommand,
    pub stitch_parts: &'a mut Vec<Id>,
    /// the recognition tool in hand: its mesh and what a count of it found
    pub recognise: &'a mut RecogniseTool,
    pub trim: &'a mut TrimTool,
    pub prim: &'a mut PrimParams,
    pub draft: &'a mut DraftParams,
    pub hole: &'a mut HoleCommand,
    pub sweep: &'a mut SweepParams,
    pub repl_surface: &'a mut Option<Id>,
    pub thread: &'a mut ThreadParams,
    pub edges: &'a mut EdgeCache,
    pub edits: &'a mut Edits,
    pub live: &'a mut LiveGeom,
    pub regen: &'a mut Rebuilding,
    pub params_seen: &'a mut std::collections::HashMap<String, f64>,
    pub boolean: &'a mut BoolCommand,
    pub bar_exprs: &'a mut std::collections::HashMap<&'static str, String>,
    pub rev: &'a mut RevolveParams,
    pub cmd_failed: &'a mut bool,
    pub carr: &'a mut CompArrayCmd,
    pub view: &'a mut View2d,
    pub view_restore: &'a mut Option<(bool, Cam3, View2d)>,
    pub cam: &'a mut Cam3,
    pub picking: &'a mut Picking,
    pub set: &'a Settings,
    pub active_path: &'a Vec<Id>,
    pub scheme: &'a SchemeUi,
    pub comp_giz: &'a CompGizmo,
    /// BORROWED, NOT COPIED. Three tools switch the viewport to 3D by assigning it, and a copy would have
    /// swallowed the assignment silently - the build stays green and the tool simply stops working.
    pub mode_3d: &'a mut bool,
    /// The gizmo of a body: reopening a Move command puts its handle back where the body is.
    pub body_giz: &'a mut BodyGizmo,
    /// TWO THINGS FROM OUTSIDE THE WORKBENCH, AND BOTH READ-ONLY. Whether the live B-rep has to be
    /// prepared is a question that spans workbenches: an assembly tool needs it as much as a chamfer
    /// does. `needs_live_brep` is asked before refreshing the edge candidates, and it wants the assembly
    /// command and the active workbench. They are shared, never assigned - a Part tool has no business
    /// changing either.
    pub joint: &'a JointCommand,
    pub workbench: Workbench,
    /// What is SHOWN: the axis candidates are edges of the visible bodies only, and visibility is decided
    /// by the caches, the sketch session and the windows together.
    pub cache: &'a mut Caches,
    pub sketch_ses: &'a mut SketchSession,
    pub win: &'a mut Windows,
}

impl PartCtx<'_> {
    /// The rebuild, through this workbench's own fields - as in the assembly. Composed rather than carried
    /// alongside: two contexts in one signature would let them drift apart.
    pub fn rebuild(&mut self) -> RebuildCtx<'_> {
        RebuildCtx {
            project: self.project,
            view: self.view,
            edits: self.edits,
            regen: self.regen,
            live: self.live,
            params_seen: self.params_seen,
            status: self.status,
            sel: self.sel,
            active_path: self.active_path,
            cam: self.cam,
            scheme: self.scheme,
            set: self.set,
        }
    }
}

/// A WORKBENCH STANDING ON ITS OWN, for the checks. One bench for all three.
///
/// The workbench crates take a CONTEXT, and a context is a bundle of borrows. Until now only the
/// application could tie that bundle: it owns the records the borrows point at. That made every check of
/// a workbench a check of the application, running whole frames to reach three lines of arithmetic - and
/// it meant a crate could not be verified on its own at all.
///
/// This owns the same records and hands out the same contexts. It is not a mock: the records are the real
/// ones, and so is the document.
///
/// ONE BENCH RATHER THAN THREE, and the timing of that is a rule rather than taste: the shared thing is
/// made at the SECOND caller, because a common thing designed from one example is designed wrong. It was
/// born serving the Part alone. With the sketcher asking too, the overlap is a measurement instead of a
/// guess: of the sketcher's 45 fields, 26 are ones the Part already needed, and the assembly adds three of
/// its own to what stands here. Three near-identical fifty-field records would have drifted apart on the
/// first field added to one of them.
///
/// IT ALSO CHECKS SOMETHING BY EXISTING. If a context stops being tie-able without `App`, this stops
/// compiling - which is the loudest possible way to say the context is no longer narrow.
pub struct Bench {
    pub armed: Armed,
    pub status: String,
    pub cmd: FeatCommand,
    pub project: qymcad_core::model::Project,
    pub gsel: GeomSelection,
    pub sel: Sel,
    pub split: SplitParams,
    pub opts: FeatOptions,
    pub feat: FeatTarget,
    pub mirror: MirrorParams,
    pub loft: LoftParams,
    pub chamfer: ChamferParams,
    pub arr: ArrayParams,
    pub datum: DatumCommand,
    pub stitch_parts: Vec<Id>,
    /// the recognition tool in hand: its mesh and what a count of it found
    pub recognise: RecogniseTool,
    pub trim: TrimTool,
    pub prim: PrimParams,
    pub draft: DraftParams,
    pub hole: HoleCommand,
    pub sweep: SweepParams,
    pub repl_surface: Option<Id>,
    pub thread: ThreadParams,
    pub edges: EdgeCache,
    pub edits: Edits,
    pub live: LiveGeom,
    pub regen: Rebuilding,
    pub params_seen: std::collections::HashMap<String, f64>,
    pub boolean: BoolCommand,
    pub bar_exprs: std::collections::HashMap<&'static str, String>,
    pub rev: RevolveParams,
    pub cmd_failed: bool,
    pub carr: CompArrayCmd,
    pub view: View2d,
    pub view_restore: Option<(bool, Cam3, View2d)>,
    pub cam: Cam3,
    pub picking: Picking,
    pub set: Settings,
    pub active_path: Vec<Id>,
    pub scheme: SchemeUi,
    pub comp_giz: CompGizmo,
    pub workbench: Workbench,
    pub sketch_ses: SketchSession,
    pub win: Windows,
    pub mode_3d: bool,
    pub body_giz: BodyGizmo,
    pub joint: JointCommand,
    pub cache: Caches,

    /// What only the sketcher reaches for. Nineteen records against the Part's, and every one of them a
    /// tool or a pick that has no meaning outside a sketch.
    pub place: Placing,
    pub tool: SketchTool,
    pub sel_sk: SketchSelection,
    pub drag: Dragging,
    pub dim: DimTool,
    pub pending_import: PendingImport,
    pub measure: Measuring,
    pub inline: InlineEdit,
    pub pat: PatternTool,
    pub corner: CornerInput,
    pub annot: AnnotEdit,
    pub cursor: Option<qymcad_core::geom::Point2>,
    pub sk_pat: SketchPattern,
    pub tool_prefs: SketchToolPrefs,
    pub font_cache: FontCache,
    pub snap_hint: Option<(qymcad_core::geom::Point2, u8)>,
    pub tree_sel: TreeSelection,
    pub clip: Clipboard,
    pub rot: RotInput,

    /// What only the assembly adds: the running mechanism, the picked connector and the part being pulled.
    pub joint_anim: Option<JointAnim>,
    pub sel_conn: Option<Id>,
    pub part_pull: Option<(Id, [f64; 3], [f64; 3])>,
}

impl Default for Bench {
    /// `Sel` and `Workbench` have no derived default ON PURPOSE - "nothing is selected" and "which
    /// workbench" are named choices, not zeroes - so the bench names them here: an empty selection and
    /// the Part bench, which is what these checks are about.
    fn default() -> Self {
        Bench {
            sel: Sel::None,
            workbench: Workbench::Part,
            armed: Armed::default(),
            status: String::new(),
            cmd: FeatCommand::default(),
            project: qymcad_core::model::Project::default(),
            gsel: GeomSelection::default(),
            split: SplitParams::default(),
            opts: FeatOptions::default(),
            feat: FeatTarget::default(),
            mirror: MirrorParams::default(),
            loft: LoftParams::default(),
            chamfer: ChamferParams::default(),
            arr: ArrayParams::default(),
            datum: DatumCommand::default(),
            stitch_parts: Vec::new(),
            recognise: RecogniseTool::default(),
            trim: TrimTool::default(),
            prim: PrimParams::default(),
            draft: DraftParams::default(),
            hole: HoleCommand::default(),
            sweep: SweepParams::default(),
            repl_surface: None,
            thread: ThreadParams::default(),
            edges: EdgeCache::default(),
            edits: Edits::default(),
            live: LiveGeom::default(),
            regen: Rebuilding::default(),
            params_seen: std::collections::HashMap::new(),
            boolean: BoolCommand::default(),
            bar_exprs: std::collections::HashMap::new(),
            rev: RevolveParams::default(),
            cmd_failed: false,
            carr: CompArrayCmd::default(),
            view: View2d::default(),
            view_restore: None,
            cam: Cam3::default(),
            picking: Picking::default(),
            set: Settings::default(),
            active_path: Vec::new(),
            scheme: SchemeUi::default(),
            comp_giz: CompGizmo::default(),
            sketch_ses: SketchSession::default(),
            win: Windows::default(),
            mode_3d: false,
            body_giz: BodyGizmo::default(),
            joint: JointCommand::default(),
            cache: Caches::default(),
            place: Placing::default(),
            tool: SketchTool::default(),
            sel_sk: SketchSelection::default(),
            drag: Dragging::default(),
            dim: DimTool::default(),
            pending_import: PendingImport::default(),
            measure: Measuring::default(),
            inline: InlineEdit::default(),
            pat: PatternTool::default(),
            corner: CornerInput::default(),
            annot: AnnotEdit::default(),
            cursor: None,
            sk_pat: SketchPattern::default(),
            tool_prefs: SketchToolPrefs::default(),
            font_cache: FontCache::default(),
            snap_hint: None,
            tree_sel: TreeSelection::default(),
            clip: Clipboard::default(),
            rot: RotInput::default(),
            joint_anim: None,
            sel_conn: None,
            part_pull: None,
        }
    }
}

impl Bench {
    /// The Part's context, tied out of the records above.
    pub fn part_ctx(&mut self) -> PartCtx<'_> {
        PartCtx {
            armed: &mut self.armed,
            status: &mut self.status,
            cmd: &mut self.cmd,
            project: &mut self.project,
            gsel: &mut self.gsel,
            sel: &mut self.sel,
            split: &mut self.split,
            opts: &mut self.opts,
            feat: &mut self.feat,
            mirror: &mut self.mirror,
            loft: &mut self.loft,
            chamfer: &mut self.chamfer,
            arr: &mut self.arr,
            datum: &mut self.datum,
            stitch_parts: &mut self.stitch_parts,
            recognise: &mut self.recognise,
            trim: &mut self.trim,
            prim: &mut self.prim,
            draft: &mut self.draft,
            hole: &mut self.hole,
            sweep: &mut self.sweep,
            repl_surface: &mut self.repl_surface,
            thread: &mut self.thread,
            edges: &mut self.edges,
            edits: &mut self.edits,
            live: &mut self.live,
            regen: &mut self.regen,
            params_seen: &mut self.params_seen,
            boolean: &mut self.boolean,
            bar_exprs: &mut self.bar_exprs,
            rev: &mut self.rev,
            cmd_failed: &mut self.cmd_failed,
            carr: &mut self.carr,
            view: &mut self.view,
            view_restore: &mut self.view_restore,
            cam: &mut self.cam,
            picking: &mut self.picking,
            set: &self.set,
            active_path: &self.active_path,
            scheme: &self.scheme,
            comp_giz: &self.comp_giz,
            workbench: self.workbench,
            sketch_ses: &mut self.sketch_ses,
            win: &mut self.win,
            mode_3d: &mut self.mode_3d,
            body_giz: &mut self.body_giz,
            joint: &self.joint,
            cache: &mut self.cache,
        }
    }

    /// The sketcher's context, out of the same records.
    ///
    /// Twenty-six of these fields are the Part's too - `project`, `armed`, `sel`, `edits`, `regen`, the
    /// view and the camera among them. That overlap is why there is one bench: the sketcher and the Part
    /// are two views of one document, and two benches would have meant two documents that agree by
    /// accident.
    pub fn sketch_ctx(&mut self) -> SketchCtx<'_> {
        SketchCtx {
            armed: &mut self.armed,
            project: &mut self.project,
            regen: &mut self.regen,
            status: &mut self.status,
            place: &mut self.place,
            cmd: &mut self.cmd,
            tool: &mut self.tool,
            sel: &mut self.sel,
            sel_sk: &mut self.sel_sk,
            drag: &mut self.drag,
            edits: &mut self.edits,
            dim: &mut self.dim,
            pending_import: &mut self.pending_import,
            gsel: &mut self.gsel,
            measure: &mut self.measure,
            inline: &mut self.inline,
            pat: &mut self.pat,
            picking: &mut self.picking,
            corner: &mut self.corner,
            annot: &mut self.annot,
            live: &mut self.live,
            params_seen: &mut self.params_seen,
            view: &mut self.view,
            cursor: &mut self.cursor,
            cache: &self.cache,
            cam: &self.cam,
            set: &self.set,
            scheme: &self.scheme,
            active_path: &self.active_path,
            sk_pat: &mut self.sk_pat,
            tool_prefs: &mut self.tool_prefs,
            sketch_ses: &mut self.sketch_ses,
            font_cache: &mut self.font_cache,
            mode_3d: &mut self.mode_3d,
            snap_hint: &mut self.snap_hint,
            workbench: &mut self.workbench,
            body_giz: &mut self.body_giz,
            tree_sel: &mut self.tree_sel,
            trim: &mut self.trim,
            clip: &mut self.clip,
            loft: &mut self.loft,
            rev: &mut self.rev,
            rot: &mut self.rot,
            sweep: &mut self.sweep,
        }
    }

    /// The assembly's context, out of the same records.
    pub fn joint_ctx(&mut self) -> JointCtx<'_> {
        JointCtx {
            view: &mut self.view,
            sel: &mut self.sel,
            joint: &mut self.joint,
            joint_anim: &mut self.joint_anim,
            comp_giz: &self.comp_giz,
            project: &mut self.project,
            status: &mut self.status,
            edits: &mut self.edits,
            sel_conn: &mut self.sel_conn,
            live: &mut self.live,
            part_pull: &mut self.part_pull,
            regen: &mut self.regen,
            params_seen: &mut self.params_seen,
            active_path: &self.active_path,
            scheme: &self.scheme,
            cam: &self.cam,
            set: &self.set,
            workbench: self.workbench,
            mode_3d: self.mode_3d,
        }
    }
}

/// WHAT THE SKETCH WORKBENCH TOUCHES.
///
/// The sketcher is the most entangled of the three - eighteen per cent of its lines reach for the Part's
/// fields - so the record is measured over the methods that call nothing outside, and grows as the rest
/// follow. The drawing tools, the snapping, the selection and the dimension input are its own.
pub struct SketchCtx<'a> {
    pub armed: &'a mut Armed,
    pub project: &'a mut qymcad_core::model::Project,
    pub regen: &'a mut Rebuilding,
    pub status: &'a mut String,
    pub place: &'a mut Placing,
    pub cmd: &'a mut FeatCommand,
    pub tool: &'a mut SketchTool,
    pub sel: &'a mut Sel,
    pub sel_sk: &'a mut SketchSelection,
    pub drag: &'a mut Dragging,
    pub edits: &'a mut Edits,
    pub dim: &'a mut DimTool,
    pub pending_import: &'a mut PendingImport,
    pub gsel: &'a mut GeomSelection,
    pub measure: &'a mut Measuring,
    pub inline: &'a mut InlineEdit,
    pub pat: &'a mut PatternTool,
    pub picking: &'a mut Picking,
    pub corner: &'a mut CornerInput,
    pub annot: &'a mut AnnotEdit,
    pub live: &'a mut LiveGeom,
    pub params_seen: &'a mut std::collections::HashMap<String, f64>,
    pub view: &'a mut View2d,
    pub cursor: &'a mut Option<Point2>,
    pub cache: &'a Caches,
    pub cam: &'a Cam3,
    pub set: &'a Settings,
    pub scheme: &'a SchemeUi,
    pub active_path: &'a Vec<Id>,
    /// The rest of what the sketcher itself touches, gathered as its own methods came over: the tools with
    /// records of their own (trim, pattern, preferences), the session, the clipboard, the hover and the
    /// snap hint, and the four Part commands whose contours are picked in the flat half-sketcher.
    pub sk_pat: &'a mut SketchPattern,
    pub tool_prefs: &'a mut SketchToolPrefs,
    pub sketch_ses: &'a mut SketchSession,
    pub font_cache: &'a mut FontCache,
    pub mode_3d: &'a mut bool,
    pub snap_hint: &'a mut Option<(Point2, u8)>,
    pub workbench: &'a mut Workbench,
    pub body_giz: &'a mut BodyGizmo,
    pub tree_sel: &'a mut TreeSelection,
    pub trim: &'a mut TrimTool,
    pub clip: &'a mut Clipboard,
    pub loft: &'a mut LoftParams,
    pub rev: &'a mut RevolveParams,
    pub rot: &'a mut RotInput,
    pub sweep: &'a mut SweepParams,
}

impl SketchCtx<'_> {
    /// Picking, through this workbench's own fields.
    pub fn pick(&self) -> PickCtx<'_> {
        PickCtx { project: self.project, set: self.set, view: self.view }
    }

    /// Drawing, through this workbench's own fields.
    pub fn draw(&self) -> DrawCtx<'_> {
        DrawCtx { cam: self.cam, set: self.set, scheme: self.scheme, project: self.project, active_path: self.active_path }
    }

    /// The rebuild, through this workbench's own fields - as in the assembly and the part.
    pub fn rebuild(&mut self) -> RebuildCtx<'_> {
        RebuildCtx {
            project: self.project,
            view: self.view,
            edits: self.edits,
            regen: self.regen,
            live: self.live,
            params_seen: self.params_seen,
            status: self.status,
            sel: self.sel,
            active_path: self.active_path,
            cam: self.cam,
            scheme: self.scheme,
            set: self.set,
        }
    }
}

/// WHAT THE FEATURE TREE NEEDS, AND NOTHING ELSE. Nineteen fields out of a hundred: the document, the
/// selection, the row state (renaming, expansion, drag), and the read-only view settings. The panel used
/// to be twelve methods on `App`, so it belonged to the crate declaring `App` no matter how it was
/// written; over this record it belongs to the file it lives in.
pub struct TreeCtx<'a> {
    pub project: &'a mut qymcad_core::model::Project,
    pub view: &'a mut View2d,
    pub sel: &'a mut Sel,
    pub tree_sel: &'a mut TreeSelection,
    pub tree: &'a mut TreeUi,
    pub rename: &'a mut RenameInput,
    pub clip: &'a mut Clipboard,
    pub status: &'a mut String,
    pub edits: &'a mut Edits,
    pub regen: &'a mut Rebuilding,
    pub live: &'a mut LiveGeom,
    pub params_seen: &'a mut std::collections::HashMap<String, f64>,
    pub datum: &'a mut DatumCommand,
    pub deferred: &'a mut DeferredUi,
    pub interference: &'a mut Interference,
    pub sketch_hidden: &'a mut std::collections::HashSet<Id>,
    pub sketch_ses: &'a mut SketchSession,
    pub mesh_export: &'a mut Option<(MeshFormat, ExportTarget)>,
    pub rollback: &'a mut RollbackDrag,
    pub workbench: Workbench,
    /// The tree carries three of the view switches (contours, joints, interference), so the settings come
    /// as `&mut` - the rebuild below takes them back as shared.
    pub set: &'a mut Settings,
    pub active_path: &'a Vec<Id>,
    pub cam: &'a Cam3,
    pub scheme: &'a SchemeUi,
    /// WHAT THE TREE ASKS THE APPLICATION TO DO. A row can open a command, enter a sketch, export a body
    /// or move a feature - work that belongs to the whole application. The panel names the request here
    /// and the frame carries it out AFTER the drawing: acting mid-frame would rebuild the very tree being
    /// walked, and reaching for `&mut App` would nail the panel to the crate declaring it.
    pub ask: &'a mut Vec<TreeAsk>,
}

/// The named things the feature tree can ask for.
#[derive(Clone)]
pub enum TreeAsk {
    /// Reopen the command of a feature (a double click on its row, or a datum row's pencil).
    EditFeature(Id),
    /// Enter the sketch for editing.
    EnterSketch(usize),
    /// The row of a sketch already selected was clicked again: a tool that took it lets it go.
    SketchAgain(usize),
    /// Copy (or cut) the selection.
    Clipboard { cut: bool },
    /// Paste what the clipboard holds.
    Paste,
    /// Export a sketch: as a drawing, or flattened.
    ExportSketch { si: usize, flat: bool },
    /// Arm the picking of a new plane for the sketch.
    ReplaceSketchPlane(usize),
    /// A row's context-menu action on a timeline node.
    Action { act: u8, ti: usize, nid: Id, prev_feat: Option<usize>, next_feat: Option<usize> },
    /// Ask again about the units and scale of an import already in the document (editing its node).
    RescaleImport(Id),
    /// Start a sketch on one of the base planes of the component.
    SketchOnBasePlane(qymcad_core::feature::BasePlane),
    /// Step inside the component.
    EnterComponent(Id),
    /// Reopen the pattern command of the component.
    EditCompArray(Id),
    /// Bring a file in through the door File -> Import takes: a kept source written out again
    ImportFile(std::path::PathBuf),
    /// Export a component into an exact file.
    ExportExact(qymcad_kernel::ExactFormat, Id),
    /// Open the "save as a part" dialog for the component.
    SavePart(Id),
    /// Drag and drop inside the tree.
    Drop { dragged: Id, target: Id, how: TreeDrop },
    /// Re-hang the derived caches after the rollback line has moved.
    Resync,
}

impl TreeCtx<'_> {
    /// The rebuild, through the tree's own fields - as in the workbenches and the properties panel.
    pub fn rebuild(&mut self) -> RebuildCtx<'_> {
        RebuildCtx {
            project: self.project,
            view: self.view,
            edits: self.edits,
            regen: self.regen,
            live: self.live,
            params_seen: self.params_seen,
            status: self.status,
            sel: self.sel,
            active_path: self.active_path,
            cam: self.cam,
            scheme: self.scheme,
            set: &*self.set,
        }
    }
}

/// WHAT THE BARS TOUCH: the menu, the tool bar, the workbench bar and the options bar. Forty-two fields -
/// a lot, and honestly so: a bar of buttons is a list of EVERY tool, and every tool has a record of its
/// own. What the bars do NOT do is the work behind the button; that they name (`BarAsk`).
///
/// Every field is `&mut` on purpose, even the ones only read today. A bar that starts assigning to a
/// field taken by value would write into a local copy: the build stays green, no warning appears, and the
/// button silently stops working. That mistake has happened once and is not worth a second time.
pub struct BarCtx<'a> {
    pub armed: &'a mut Armed,
    pub active_path: &'a mut Vec<Id>,
    pub annot: &'a mut AnnotEdit,
    pub bar_exprs: &'a mut std::collections::HashMap<&'static str, String>,
    pub boolean: &'a mut BoolCommand,
    pub cam: &'a mut Cam3,
    pub carr: &'a mut CompArrayCmd,
    pub clip: &'a mut Clipboard,
    pub cmd: &'a mut FeatCommand,
    pub corner: &'a mut CornerInput,
    pub dim: &'a mut DimTool,
    pub drag: &'a mut Dragging,
    pub dxf_path: &'a mut Option<String>,
    pub edits: &'a mut Edits,
    pub feat: &'a mut FeatTarget,
    pub gsel: &'a mut GeomSelection,
    pub inline: &'a mut InlineEdit,
    pub joint: &'a mut JointCommand,
    pub m3: &'a mut Measure3,
    pub measure: &'a mut Measuring,
    pub mirror: &'a mut MirrorParams,
    pub mode_3d: &'a mut bool,
    pub parts: &'a mut PartsLibrary,
    pub pat: &'a mut PatternTool,
    pub pending_import: &'a mut PendingImport,
    pub picking: &'a mut Picking,
    pub place: &'a mut Placing,
    pub project: &'a mut qymcad_core::model::Project,
    pub regen: &'a mut Rebuilding,
    pub scheme: &'a mut SchemeUi,
    pub section: &'a mut SectionTool,
    pub sel: &'a mut Sel,
    pub sel_sk: &'a mut SketchSelection,
    pub set: &'a mut Settings,
    pub sk_pat: &'a mut SketchPattern,
    pub sketch_ses: &'a mut SketchSession,
    pub status: &'a mut String,
    pub mesh_export: &'a mut Option<(MeshFormat, ExportTarget)>,
    pub tool: &'a mut SketchTool,
    pub tool_prefs: &'a mut SketchToolPrefs,
    pub view: &'a mut View2d,
    pub win: &'a mut Windows,
    pub workbench: &'a mut Workbench,
    /// WHAT THE BAR ASKS THE APPLICATION TO DO. A button does not open a file, start a command or undo a
    /// step by itself - it names the request, and the frame carries it out after the drawing.
    pub ask: &'a mut Vec<BarAsk>,
}

/// WHAT THE STATUS LINE READS, in one record rather than nine borrows.
///
/// Nine arguments is what it had grown to, and the guard on wide signatures said so. A list that long is
/// read by nobody: at the call site it is nine expressions in a row, and getting two of them the wrong
/// way round is a compile error only while their types differ.
pub struct StatusCtx<'a> {
    pub cache: &'a Caches,
    pub cursor: Option<qymcad_core::geom::Point2>,
    pub project: &'a qymcad_core::model::Project,
    pub scheme: &'a SchemeUi,
    /// Needed because the check for a newer version is STARTED here: nothing else in the program knows
    /// that a start happened, and the setting says how often to ask.
    pub set: &'a mut Settings,
    pub sketch_ses: &'a SketchSession,
    pub status: &'a str,
    /// The badge leads to the window, so the line must be able to open it.
    pub win: &'a mut Windows,
}

/// THE MESH FORMATS a body can be written as. Their names and extensions live in the table of formats
/// (`qymcad_io::Format`); this says only which one was asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MeshFormat {
    Stl,
    Obj,
    Ply,
    Glb,
    ThreeMf,
    Amf,
}

/// WHAT A FILE BROUGHT IN MAY BECOME, at the place the door was opened from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Want {
    /// From the File menu: a solid, a mesh or a drawing, whatever the file is.
    Anything,
    /// Into an assembly: only what can be a part - a flat drawing has no body to be one with.
    Part,
}

/// The named things a bar can ask for.
#[derive(Clone)]
pub enum BarAsk {
    /// Leave the document: new, open, a template, exit.
    Nav(Nav),
    /// Save, or save under a new name.
    Save,
    SaveAs,
    /// Bring a file in through the one door; what it may become depends on where the door was opened.
    Import(Want),
    /// A font of one's own.
    PickFont,
    /// A font from a file of one's own, asked for inside the list of installed ones. Separate from
    /// `PickFont`, which opens that list: one request cannot mean both, or the button would loop.
    PickFontFile,
    /// Write the whole document out as a solid.
    ExportExact(qymcad_kernel::ExactFormat, ExportTarget),
    /// The undo history.
    Undo,
    Redo,
    /// The clipboard.
    Clipboard {
        cut: bool,
    },
    Paste,
    /// Rebuild everything from the timeline.
    RebuildEverything,
    /// Open a help article.
    Help(String),
    /// Walk the context path, or step out of it.
    GotoPath(usize),
    ExitContext,
    /// Take up a sketch tool.
    SketchTool(u8),
    /// Start a feature command, or a primitive.
    FeatCmd(u8),
    PrimCmd(u8),
    /// The view switches that need more than a flag.
    ToggleSection,
    /// Start mirroring the whole part or subassembly named here: release every other tool, then wait for
    /// the plane. A request rather than a write, because the release lives on the application side.
    MirrorPart(Option<Id>),
    ToggleMeasure3d,
    /// Step into a component.
    EnterComponent(Id),
    /// Start an array of components.
    CompArray(u8),
    /// Put every tool down, then take up the one named (if any). A tool taken straight from the bar goes through
    /// here: set before the put-down runs, it would be put down with the rest, and set without one it stood beside
    /// the tool already in hand - two bars at once.
    CancelAllTools(Then),
    /// Show or hide the parts library (the catalogue is read from disk, so the application does it).
    ToggleLibrary,
    /// Turn the pick of a plane for a new sketch on or off.
    ToggleSketchPick,
    /// Put the sketch tools down and go back to picking with the arrow.
    SketchSelectMode,
    /// Apply a sketch constraint by its code.
    Constraint(u8),
    /// The assembly picks.
    JointPick,
    GroundPick,
    GroupPick,
    WidthPick,
    TangentPick,
    RelationPick,
}

/// WHAT THE WINDOWS TOUCH: the parts library, the parameters, the settings, the document properties, the
/// navigation question and the two small dialogs. Twenty-five fields - the last five are carried only so
/// that the rebuild context can be composed out of this one.
pub struct WinCtx<'a> {
    pub cache: &'a mut Caches,
    pub deferred: &'a mut DeferredUi,
    pub edits: &'a mut Edits,
    pub gpu_ok: &'a mut bool,
    pub logo_tex: &'a mut Option<egui::TextureHandle>,
    pub par_search: &'a mut String,
    pub parts: &'a mut PartsLibrary,
    pub pending_nav: &'a mut Option<Nav>,
    pub project: &'a mut qymcad_core::model::Project,
    pub project_path: &'a mut Option<String>,
    /// The last file read or written: the next file chooser opens beside it.
    pub dxf_path: &'a mut Option<String>,
    pub regen: &'a mut Rebuilding,
    pub scheme: &'a mut SchemeUi,
    pub sel: &'a mut Sel,
    pub set: &'a mut Settings,
    pub status: &'a mut String,
    pub mesh_export: &'a mut Option<(MeshFormat, ExportTarget)>,
    pub tex_graveyard: &'a mut Vec<egui::TextureHandle>,
    pub tree: &'a mut TreeUi,
    pub waiting: &'a mut Waiting,
    pub win: &'a mut Windows,
    pub live: &'a mut LiveGeom,
    pub params_seen: &'a mut std::collections::HashMap<String, f64>,
    pub view: &'a mut View2d,
    pub cam: &'a mut Cam3,
    pub active_path: &'a mut Vec<Id>,
    /// Whether a file chooser is open right now. The window only ASKS about it: the chooser itself lives
    /// in the application, and the navigation question must not act while one is up.
    pub file_ask_open: bool,
    /// The hotkey window's own state while it waits for a press.
    pub hotkeys: &'a mut HotkeyCapture,
    /// WHICH WORKBENCH IS CURRENT. Read only: the command search puts its own commands first and marks the
    /// rest. Changing the workbench is the application's business and goes out as a request.
    pub workbench: Workbench,
    /// WHAT THE WINDOW ASKS THE APPLICATION TO DO.
    pub ask: &'a mut Vec<WinAsk>,
}

/// The named things a window can ask for.
#[derive(Clone)]
pub enum WinAsk {
    /// Leave the document (the navigation question answers with this).
    Nav(Nav),
    /// Save the document.
    Save,
    /// Jump to a sketch or a feature: step into its component (if any) and select it. ONE request,
    /// because stepping in clears the selection.
    GoTo {
        owner: Option<Id>,
        sel: Sel,
    },
    /// Bring a part of the library into the document.
    InsertPart(PartSource),
    /// Write the settings out, or read them in.
    ExportSettings,
    ImportSettings,
    /// Write a mesh out in a format, with the chosen deflection.
    ExportMesh(MeshFormat, ExportTarget, f64),
    /// Carry out a deletion the confirmation asked about.
    /// delete the selection; true takes what stands on it too
    Delete(Sel, bool),
    /// Rebuild everything from the timeline (a parameter changed).
    RegenerateAll,
    /// Launch a command by its catalogue code - through the application's one door, the same one a button uses.
    RunCommand(&'static str),
    /// Go somewhere, BUT ASK ABOUT UNSAVED WORK FIRST. `Nav` above goes straight there and is what the
    /// unsaved-work question itself uses once it has been answered - asking again from there would loop.
    NavGuarded(Nav),
}

impl WinCtx<'_> {
    /// The rebuild, through the windows' own fields.
    pub fn rebuild(&mut self) -> RebuildCtx<'_> {
        RebuildCtx {
            project: self.project,
            view: self.view,
            edits: self.edits,
            regen: self.regen,
            live: self.live,
            params_seen: self.params_seen,
            status: self.status,
            sel: self.sel,
            active_path: &*self.active_path,
            cam: &*self.cam,
            scheme: &*self.scheme,
            set: &*self.set,
        }
    }
}

/// WHAT THE PROPERTIES PANEL TOUCHES: twelve fields, measured over its three methods.
pub struct PropsCtx<'a> {
    pub project: &'a mut qymcad_core::model::Project,
    pub datum: &'a mut DatumCommand,
    pub deferred: &'a mut DeferredUi,
    pub hover: &'a mut Hover,
    pub joint: &'a mut JointCommand,
    pub picking: &'a mut Picking,
    pub sel_conn: &'a mut Option<Id>,
    pub status: &'a mut String,
    pub edits: &'a mut Edits,
    pub regen: &'a mut Rebuilding,
    pub live: &'a mut LiveGeom,
    pub params_seen: &'a mut std::collections::HashMap<String, f64>,
    pub sel: &'a mut Sel,
    /// The rest of the properties COLUMN, gathered as its panels came over: the tools a row can arm
    /// (array, boolean), the feature options, the caches a sketch row reads, the windows, the 2D view and
    /// the geometry selection.
    pub array: &'a mut ArrayTool,
    pub boolean: &'a mut BoolCommand,
    pub opts: &'a mut FeatOptions,
    pub cache: &'a mut Caches,
    pub win: &'a mut Windows,
    pub view: &'a mut View2d,
    pub gsel: &'a mut GeomSelection,
    pub sketch_ses: &'a mut SketchSession,
    /// The contour row edits a view switch, so the settings come as `&mut`; the rebuild takes them back
    /// as shared.
    pub set: &'a mut Settings,
    pub active_path: &'a Vec<Id>,
    pub cam: &'a Cam3,
    pub scheme: &'a SchemeUi,
    pub workbench: Workbench,
    /// BY REFERENCE, NOT BY VALUE. A properties row does switch to 3D - "pick the face again" sends one to the
    /// viewport. Held by value it was assigned to a copy that died with the call, and the switch never happened:
    /// green build, no warning, no dead code. The one class of transfer error the compiler says nothing about.
    pub mode_3d: &'a mut bool,
    /// WHAT THE PANEL ASKS THE APPLICATION TO DO. A properties row can start a command or create a sketch -
    /// actions that belong to the whole application, not to a panel. It puts a named request here and the
    /// frame carries it out AFTER the drawing: a panel that reached for `&mut App` could not live in a crate
    /// of its own, and a panel that acted mid-frame would redraw a tree it had just changed.
    pub ask: &'a mut Vec<PropsAsk>,
}

/// The named things the properties panel can ask for.
#[derive(Clone, Copy)]
pub enum PropsAsk {
    /// Ask again about the units and scale of an import already in the document (its Edit button).
    RescaleImport(Id),
    /// Reopen the command of an existing feature (a double click on its row).
    EditFeature(Id),
    /// Open an existing joint for editing (a double click on its row in the list of mates).
    EditJoint(Id),
    /// Start a sketch on a datum plane.
    SketchOnDatum(Id),
    /// Arm the joint tool.
    JointPick,
    /// Arm the standalone connector tool.
    ConnPick,
    /// Delete a standalone connector (the assembly owns the operation, the panel only asks).
    DeleteConnector(Id),
    /// Arm the picking of the second part for a relation.
    RelationPick(Id),
    /// Start a sketch on one of the base planes of the document.
    SketchOnBasePlane(qymcad_core::feature::BasePlane),
    /// Enter the sketch for editing.
    EnterSketch(usize),
    /// Step out of the current component context.
    ExitContext,
    /// Step into the component context.
    SetContext(Id),
}

impl PropsCtx<'_> {
    /// The rebuild, through this panel's own fields - as in the workbenches.
    pub fn rebuild(&mut self) -> RebuildCtx<'_> {
        RebuildCtx {
            project: self.project,
            view: self.view,
            edits: self.edits,
            regen: self.regen,
            live: self.live,
            params_seen: self.params_seen,
            status: self.status,
            sel: self.sel,
            active_path: self.active_path,
            cam: self.cam,
            scheme: self.scheme,
            set: &*self.set,
        }
    }
}

/// What the measuring tool has clicked: the element itself plus a human-readable name (shown in the
/// hint — otherwise it is not clear what exactly the click caught).
#[derive(Clone, Debug)]
pub struct MeasurePick {
    pub item: MeasureItem,
    pub what: String,
    /// The point at which to draw the label and run the leader.
    pub at: [f64; 3],
}

/// THE STATE OF THE 3D MEASURING TOOL: up to two elements. A third click starts a new measurement,
/// the same as in the sketch measuring tool, and that is the one behaviour nobody confuses.
#[derive(Clone, Debug, Default)]
pub struct Measure3 {
    pub on: bool,
    pub picks: Vec<MeasurePick>,
}

impl Measure3 {
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// TAKE AN ELEMENT CLICKED: the same one clicked again is let go, as a second click on any pick; a third one begins
    /// a new measurement.
    pub fn take(&mut self, p: MeasurePick) {
        let same = |q: &MeasurePick| q.what == p.what && (0..3).all(|i| (q.at[i] - p.at[i]).abs() < 1e-6);
        if let Some(k) = self.picks.iter().position(same) {
            self.picks.remove(k);
            return;
        }
        if self.picks.len() >= 2 {
            self.picks.clear();
        }
        self.picks.push(p);
    }
}

/// A leaf part in the tree of the catalogue. The manifest is loaded lazily (only `part.ron`, without
/// `document.ron`).
#[derive(Clone, Debug)]
pub struct PartEntry {
    /// The name from the manifest (or the file name as a stand-in, if the manifest does not read).
    pub name: String,
    /// The file name without the `.qpart` extension (the key for creating, reading and deleting).
    pub file_stem: String,
    pub source: PartSource,
    pub manifest: Option<PartManifest>,
}

/// A category node is a folder. It holds subcategories and parts.
#[derive(Clone, Debug, Default)]
pub struct CatNode {
    /// The name shown (`category.ron.title` or the name of the folder; for the roots, the built-in and
    /// the user's own captions).
    pub title: String,
    /// The order among siblings (from `category.ron`); equal ones go alphabetically by `title`.
    pub order: i32,
    /// The `egui_phosphor::regular::*` icon of the node (from `category.ron`), without the prefix.
    pub icon: Option<String>,
    pub subcats: Vec<CatNode>,
    pub parts: Vec<PartEntry>,
}

impl CatNode {
    /// The total number of parts in the subtree (for badges and empty states).
    pub fn total_parts(&self) -> usize {
        self.parts.len() + self.subcats.iter().map(CatNode::total_parts).sum::<usize>()
    }

    pub fn sort_recursive(&mut self) {
        self.subcats.sort_by(|a, b| a.order.cmp(&b.order).then_with(|| a.title.cmp(&b.title)));
        self.parts.sort_by(|a, b| a.name.cmp(&b.name));
        for c in &mut self.subcats {
            c.sort_recursive();
        }
    }
}

/// The merged tree of the catalogue: two roots (the built-in one and the user's own). Clashes of names
/// between the tiers are NOT merged — each stays under its own root.
#[derive(Clone, Debug, Default)]
pub struct LibraryTree {
    pub embedded: CatNode,
    pub user: CatNode,
}

/// THE PARTS LIBRARY: its tree, what is picked in it, and the thumbnails already drawn.
#[derive(Default)]
pub struct PartsLibrary {
    /// The cache of the parts catalogue's tree (the built-in one plus the user's). None means it is built on opening or refreshing.
    pub tree: Option<LibraryTree>,
    /// The category selected in the library's tree: (is it built in?, the path of indexes from the level's root).
    pub sel: Option<(bool, Vec<usize>)>,
    /// The search string over names and tags in the library window.
    pub search: String,
    /// The open "save as a standard part" dialogue (the component plus its metadata and preview).
    pub save: Option<SavePartDialog>,
    /// The cache of the parts' thumbnail textures for the library grid (keyed by source; None means there is no preview or it would not decode).
    pub thumbs: std::collections::HashMap<String, Option<egui::TextureHandle>>,
}

/// THE DOCUMENT'S FINGERPRINT — the CONTENT alone, with nothing derived.
///
/// One key for two questions: "did anything change outside an operation boundary" (the guard,
/// `committed_key`) and "is there anything unsaved" ([`edit_key`], `saved_key`). There used to be two
/// of them, and the second added `geom_rev` — the revision of the DRAWING cache. Any rebuild moves
/// that, and so does a change of visibility, so "is there anything unsaved" answered "yes" after
/// simply opening a file. A document key must describe the document; feature parameters now go into
/// `Project::state_key`, and there is nothing left to prop it up with a picture revision.
pub fn doc_key(project: &qymcad_core::model::Project) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    // The model's structure goes through the core's cheap hand-written key (not a single
    // allocation), rather than a RON serialisation of half the document EVERY FRAME (the key is
    // computed in `maybe_commit` on every frame). It also finally SEES the structure: creating an
    // empty part, renaming, moving a component, joints and external references used to leave the
    // document "clean" — closing the window asked nothing about unsaved work, and it was lost
    // silently.
    project.state_key().hash(&mut h);
    h.finish()
}

/// WHAT FINDING THE NEAREST THING NEEDS, and nothing besides.
///
/// MEASURED: every `nearest_*` of this file reads the document, the picking precision out of the settings,
/// and where the flat view stands. Nothing is written - picking asks a question, it does not change
/// anything, and taking the three by shared borrow says so in the signature.
pub struct PickCtx<'a> {
    pub project: &'a qymcad_core::model::Project,
    pub set: &'a Settings,
    pub view: &'a View2d,
}

/// WHAT DRAWING IN THREE DIMENSIONS NEEDS, and nothing besides.
///
/// MEASURED, not guessed: every `draw_*` of this file reads the same five things - where the camera
/// stands, the projection settings, the palette, the document, and which component is being worked in.
/// They were reachable through `self` because the drawing was a method; named here, a drawing function
/// says what it depends on and can be called without an application at all.
pub struct DrawCtx<'a> {
    pub cam: &'a Cam3,
    pub set: &'a Settings,
    pub scheme: &'a SchemeUi,
    pub project: &'a qymcad_core::model::Project,
    /// The path down to the component being worked in: a mate is drawn where its part stands NOW.
    pub active_path: &'a [qymcad_core::model::Id],
}

/// The TextEdit used by dimension popups: it SELECTS all the text when it gains focus (automatically,
/// or by a click or Tab) — a new value overwrites the old one without Ctrl+A. `autofocus` is one-shot:
/// it asks for focus on this frame. Returns the Response (for lost_focus and Enter). The same one is
/// used in sketches, parts and assemblies.
/// THE FIELD OF A NAME BEING CHANGED IN PLACE: when it opens (`opening`) it takes the focus with the whole name
/// selected, so what is typed stands INSTEAD of the old name, as in any tree. Reported behaviour: after F2 and
/// "Contour" the sketch was called "Sketch 1Contour".
pub fn rename_field(ui: &mut egui::Ui, text: &mut String, width: f32, opening: bool) -> egui::Response {
    let mut out = egui::TextEdit::singleline(text).desired_width(width).show(ui);
    if opening {
        out.response.request_focus();
        let range = egui::text::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(text.chars().count()));
        out.state.cursor.set_char_range(Some(range));
        out.state.store(ui.ctx(), out.response.id);
    }
    out.response.response
}

pub fn focus_edit(ui: &mut egui::Ui, text: &mut String, width: f32, hint: &str, autofocus: bool) -> egui::Response {
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
    out.response.response
}

/// Keep the ANCHOR of a dimension input popup inside the viewport `rect`, so that with large values
/// (where the geometry runs off the edge) the little window does not fly out of the frame. The popup
/// is about 200x60 px, so room is left for it.
pub fn clamp_popup(p: Pos2, rect: Rect) -> Pos2 {
    let r = rect.shrink(4.0);
    Pos2::new(p.x.clamp(r.left(), (r.right() - 200.0).max(r.left())), p.y.clamp(r.top() + 32.0, (r.bottom() - 50.0).max(r.top() + 32.0)))
}

/// THE PLACEMENT CHANGED — THE SHAPE OF THE BODIES DID NOT.
///
/// The door for driving a part and for moving a component. `invalidate` declares the GEOMETRY stale,
/// and on a real assembly that cost 30-48 ms per frame: all 463,878 vertices of the scene buffer were
/// assembled again although one body had moved. Reported behaviour: the part drags along as if on a
/// rubber band.
pub fn invalidate_placement(regen: &mut Rebuilding) {
    regen.place_rev = regen.place_rev.wrapping_add(1);
}

pub fn invalidate(regen: &mut Rebuilding) {
    regen.geom_rev = regen.geom_rev.wrapping_add(1); // invalidate the 3D render cache
}

pub fn to_world(view: &View2d, rect: Rect, s: Pos2) -> Point2 {
    let c = rect.center();
    Point2::new((view.center.x + (s.x - c.x) / view.scale) as f64, (view.center.y - (s.y - c.y) / view.scale) as f64)
}

/// The index of the note under a screen point (a rough bounding box of the text).
pub fn note_at(project: &Project, view: &View2d, rect: Rect, pos: Pos2, si: usize) -> Option<usize> {
    let s = project.sketches.get(si)?;
    for (i, n) in s.notes.iter().enumerate() {
        let sp = (Sheet { view: *view, rect }).at(Point2::new(n.x, n.y));
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
pub fn polygon_under(project: &Project, view: &View2d, rect: Rect, pos: Pos2, si: usize) -> Option<Id> {
    let sh = Sheet { view: *view, rect };
    use qymcad_core::model::{Constraint, EntityKind};
    let s = project.sketches.get(si)?;
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
            let sc = sh.at(Point2::new(cx, cy));
            let rp = (sh.at(Point2::new(cx + r, cy)).x - sc.x).abs();
            if (sc.distance(pos) - rp).abs() <= 8.0 {
                return Some(center);
            }
        }
    }
    None
}

/// A dependable unit SCREEN direction of the line a -> b. When the screen segment degenerates (a
/// unit-long coordinate axis seen from far away puts both ends in one pixel), the direction is taken
/// from THE MODEL (by projecting a far point of the line); otherwise a dimension to an axis comes
/// out mangled, because its perpendicular degenerates.
pub fn line_screen_dir(project: &Project, view: &View2d, si: usize, a: Id, b: Id, rect: Rect) -> Option<egui::Vec2> {
    let sh = Sheet { view: *view, rect };
    let (pa, pb) = (sketch_pt(project, si, a)?, sketch_pt(project, si, b)?);
    let sa = sh.at(pa);
    let v = sh.at(pb) - sa;
    if v.length() > 0.5 {
        return Some(v.normalized());
    }
    let far = Point2::new(pa.x + (pb.x - pa.x) * 1e4, pa.y + (pb.y - pa.y) * 1e4);
    let d = sh.at(far) - sa;
    (d.length() > 1e-4).then(|| d.normalized())
}

pub fn sel_circle_centers(project: &Project, sel_sk: &SketchSelection, si: usize) -> Vec<Id> {
    use qymcad_core::model::EntityKind;
    let Some(s) = project.sketches.get(si) else { return Vec::new() };
    sel_sk
        .items
        .iter()
        .filter(|(k, _)| *k == 1)
        .filter_map(|(_, eid)| {
            s.entities.iter().find(|e| e.id == *eid).and_then(|e| match e.kind {
                EntityKind::Circle { center, .. } | EntityKind::Arc { center, .. } => Some(center),
                _ => None,
            })
        })
        .collect()
}

/// The text object under a screen point (by the bounding box of its glyphs). For selecting, moving and editing.
pub fn text_at(project: &Project, view: &View2d, rect: Rect, pos: Pos2, si: usize) -> Option<usize> {
    let sh = Sheet { view: *view, rect };
    let s = project.sketches.get(si)?;
    for i in 0..s.texts.len() {
        if let Some((minx, miny, maxx, maxy)) = project.sketch_text_bbox(si, i) {
            let p0 = sh.at(Point2::new(minx, miny));
            let p1 = sh.at(Point2::new(maxx, maxy));
            if Rect::from_two_pos(p0, p1).expand(5.0).contains(pos) {
                return Some(i);
            }
        }
    }
    None
}

/// The index of the active sketch (while in editing mode).
pub fn edit_si(project: &Project, sketch_ses: &SketchSession) -> Option<usize> {
    sketch_ses.editing.and_then(|id| project.sketch_index(id))
}

/// The radius of a curve, found by its CENTRE: a circle (r) or an arc (|centre to end|). The single
/// source for radius and diameter dimensions — they work the same for a circle and for an arc (after
/// trimming).
pub fn center_radius(dc: &DrawCtx, si: usize, c: Id) -> Option<f64> {
    radius_of(dc.project, si, c)
}

/// The radius of the circle or arc of sketch `si` centred at point `c`.
pub fn radius_of(project: &Project, si: usize, c: Id) -> Option<f64> {
    use qymcad_core::model::EntityKind;
    let s = project.sketches.get(si)?;
    s.entities.iter().find_map(|e| match e.kind {
        EntityKind::Circle { center, r } if center == c => Some(r),
        EntityKind::Arc { center, a, .. } if center == c => sketch_pt(project, si, a).zip(sketch_pt(project, si, center)).map(|(pa, pc)| ((pa.x - pc.x).powi(2) + (pa.y - pc.y).powi(2)).sqrt()),
        _ => None,
    })
}

/// TAKE DRAWING TOOL `kind` IN HAND: every other tool put down in one move (`exit_draw_tools`), whatever the one before
/// had collected cleared (`SketchTool::select`), and the selection of the geometry dropped - a drawing tool works on
/// nothing selected before it. Reported behaviour: a line selected, the circle taken and drawn, and the line stood
/// selected the whole time and after. The arrow of selection (kind 0) keeps the selection.
pub fn take_drawing_tool(t: &mut Tools, kind: u8) {
    exit_draw_tools(t);
    t.tool.select(t.armed, kind);
    if t.armed.draw_kind() != 0 {
        t.sel_sk.clear();
    }
}

/// LEAVING ALL THE TOOLS — the single transition from a mode back to selection.
///
/// The sketch modes are mutually exclusive: exactly one is active. This exit used to be written out
/// by hand in six places, and the sets of fields in the copies DIFFERED — one reset forgot an
/// unfinished import, another the pattern, a third the modify mode. So the exit exists in a single
/// copy, and entering any tool begins with it. The selection (`sel_sk.items`) survives: what is
/// cleared is the modes, not the work already done.
pub fn exit_draw_tools(t: &mut Tools) {
    let Tools { armed, annot, cmd, corner, dim, drag, gsel, inline, measure, pat, pending_import, picking, place, sel_sk, tool } = t;
    // THE CORNER TOOL'S PICKS ARE ITS OWN STATE: the lines it was offered are lit to show a person WHICH corner is
    // being rounded, and with the mode gone there is nothing to show - they stood lit with nothing in hand, and the
    // next mode read a selection that was not its own. (The button that offers the corner of two lines already
    // chosen reads them while no mode is in hand, so this costs that nothing.)
    if matches!(armed.click_op(), 4 | 5) {
        sel_sk.items.clear();
    }
    **armed = Armed::None; // ONE FIELD: letting go is saying that nothing is in hand
    tool.pts.clear();
    tool.circ_tan = None;
    sel_sk.constraint = None;
    sel_sk.modify = None;
    dim.pick.clear();
    dim.first = None;
    place.dim = None;
    pending_import.draw_pts = None;
    drag.clear();
    inline.clear();
    picking.clear(); // exclusivity: the shape pick of "Fillet all" does not survive a change of mode
    corner.put_down();
    measure.clear(); // both the FLAG and the points collected: otherwise they outlived the exit
    tool.move_base = None;
    tool.extend = None; // the line Extend held
    pat.edit = None;
    pat.center = None;
    cmd.close(armed); // the command is closed as a whole, not just a field zeroed out
    cmd.sketch = None;
    gsel.profiles.clear();
    // clear the viewport selection of a dimension, a note or a text (otherwise it stays highlighted while drawing)
    gsel.constraint = None;
    annot.note = None;
    annot.text = None;
}

/// Attach a tangency constraint between a new circle or arc (centre `cen`, radius `r`, at (cx, cy)) and the base edge.
pub fn add_tangent_to_edge(project: &mut Project, si: usize, eref: EdgeRef, cen: Id, cx: f64, cy: f64, r: f64) {
    use qymcad_core::model::Constraint;
    match eref {
        EdgeRef::Line { a, b } => {
            project.add_constraint_if_independent(si, Constraint::Tangent { a, b, c: cen, r });
        }
        EdgeRef::Circle { center, r: rr } => {
            if let Some(pc) = sketch_pt(project, si, center) {
                let dist = ((cx - pc.x).powi(2) + (cy - pc.y).powi(2)).sqrt();
                let external = (dist - (rr + r)).abs() <= (dist - (rr - r).abs()).abs();
                project.add_constraint_if_independent(si, Constraint::CircleTangent { c1: center, c2: cen, external });
            }
        }
    }
}

pub fn sketch_pt(project: &qymcad_core::model::Project, si: usize, id: Id) -> Option<Point2> {
    project.sketches.get(si)?.points.iter().find(|p| p.id == id).map(|p| Point2::new(p.x, p.y))
}

/// WHAT DRAWING READS. Fifty-five fields, every one of them SHARED: the whole of `render.rs` is `&self`,
/// so there is no mutability to reason about here and nothing to lose by assignment. That is why the file
/// went over whole rather than in batches - the traps that make a conversion slow are all about `&mut`.
///
/// Copy types travel by value so that matching and comparison keep working; the rest by reference.
pub struct Painting<'a> {
    pub armed: &'a Armed,
    pub boolean: &'a BoolCommand,
    pub active_path: &'a Vec<Id>,
    pub arr: ArrayParams,
    pub body_giz: &'a BodyGizmo,
    pub cache: &'a Caches,
    pub cam: Cam3,
    pub carr: &'a CompArrayCmd,
    pub clip: &'a Clipboard,
    pub cmd: &'a FeatCommand,
    pub comp_giz: CompGizmo,
    pub cursor: Option<Point2>,
    pub datum: &'a DatumCommand,
    pub draft: DraftParams,
    /// the chamfer in hand: its mode and its reference face, which the preview lays the legs by
    pub chamfer: ChamferParams,
    pub edges: &'a EdgeCache,
    pub face_arrow_drag: Option<f64>,
    pub feat: FeatTarget,
    pub gpu_ok: bool,
    pub gsel: &'a GeomSelection,
    pub hole: HoleCommand,
    pub hover: Hover,
    pub inline: InlineEdit,
    pub interference: &'a Interference,
    pub joint: &'a JointCommand,
    pub live: &'a LiveGeom,
    pub loft: &'a LoftParams,
    pub m3: &'a Measure3,
    pub mirror: &'a MirrorParams,
    pub mode_3d: bool,
    pub pat: PatternTool,
    pub pending_import: &'a PendingImport,
    pub picking: Picking,
    pub prim: PrimParams,
    pub project: &'a Project,
    pub regen: &'a Rebuilding,
    pub repl_surface: Option<Id>,
    pub rev: RevolveParams,
    pub rot: &'a RotInput,
    pub scheme: &'a SchemeUi,
    pub section: &'a SectionTool,
    pub snap_hint: Option<(Point2, u8)>,
    pub sel: Sel,
    pub sel_sk: &'a SketchSelection,
    pub set: &'a Settings,
    pub sk_pat: SketchPattern,
    pub sketch_hidden: &'a std::collections::HashSet<Id>,
    pub sketch_ses: SketchSession,
    pub split: &'a SplitParams,
    pub stitch_parts: &'a Vec<Id>,
    pub recognise: &'a RecogniseTool,
    pub sweep: SweepParams,
    pub thread: ThreadParams,
    pub tool: &'a SketchTool,
    pub tool_prefs: &'a SketchToolPrefs,
    pub trim: &'a TrimTool,
    pub view: View2d,
    pub view_dragging: bool,
    pub win: &'a Windows,
    pub workbench: Workbench,
}

/// The bodies consumed by modifier features (hidden, so that only the result is seen). A delegate to
/// the core (`Project::consumed_bodies`) — one source for the core and for the UI.
pub fn consumed_bodies(project: &qymcad_core::model::Project) -> std::collections::HashSet<Id> {
    project.consumed_bodies()
}

/// Is the cylindrical face an INTERNAL one (a hole)? The core decides, from the face's triangles; here
/// we only fetch the mesh and the face.
pub fn cyl_face_is_internal(project: &qymcad_core::model::Project, body: Id, fid: u32, center: [f64; 3], axis: [f64; 3]) -> bool {
    let Some(mi) = project.mesh_index(body) else { return false };
    let Some(f) = project.bodies.get(mi).and_then(|b| b.faces.iter().find(|f| f.id == fid)) else { return false };
    qymcad_core::geom::cyl_face_is_internal(&project.bodies[mi].mesh, &f.triangles, center, axis)
}

/// THE VIEW REVISION: what is on screen depends both on the shape of the bodies and on where they
/// stand.
///
/// Caches that depend on the placement are keyed by it (world bounding boxes, section caps, the scene
/// buffer). Caches that depend ONLY on the shape (a body's edges) still look at `geom_rev`.
pub fn view_rev(regen: &Rebuilding) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (regen.geom_rev, regen.place_rev).hash(&mut h);
    h.finish()
}

/// The world snapping point under the cursor for PLACING a primitive: a vertex > a datum point > the
/// centre of a face.
/// The cursor's ray in WORLD space. The camera is orthographic (`Screen::at` gives sx = rel . right,
/// sy = rel . up, depth = rel . fwd), so every ray is parallel to `fwd`; the ray's origin is taken in
/// the plane of `target`, leaving the depth free.
pub fn screen_ray(cam: &Cam3, rect: Rect, screen: Pos2) -> ([f64; 3], [f64; 3]) {
    let (right, up, fwd) = cam.basis();
    let c = rect.center();
    let sx = ((screen.x - c.x) / cam.scale) as f64;
    let sy = (-(screen.y - c.y) / cam.scale) as f64; // the screen Y is inverted (see `Screen::at`)
    let t = cam.target;
    let o = [t[0] + right[0] * sx + up[0] * sy, t[1] + right[1] * sx + up[1] * sy, t[2] + right[2] * sx + up[2] * sy];
    (o, fwd)
}

/// The current context (the last one on the path) — the component being edited.
pub fn current_ctx_id(active_path: &[Id], project: &Project) -> Id {
    active_path.last().copied().unwrap_or(project.root)
}

/// The SOURCE body consumed by the feature being edited: while editing it is shown, so its face and edges are visible.
pub fn edit_src_body(cmd: &FeatCommand, project: &Project) -> Option<Id> {
    let fid = cmd.edit?;
    project.timeline.iter().find(|n| n.id == fid).and_then(|n| n.kind.consumed_body())
}

/// The bodies HIDDEN while a modifier feature is being edited: the feature itself plus its whole
/// DESCENDANT chain (whatever consumes it, transitively). The source is shown, with its faces and
/// edges visible and selectable — a temporary rollback to the feature being edited. It works IN THE
/// MIDDLE of a chain too (mid-chain used to show the final model, with the face sitting on a hidden
/// body). Empty when a base feature is being edited (it has no source), so its body is not hidden.
pub fn edit_hidden_bodies(cmd: &FeatCommand, project: &Project) -> std::collections::HashSet<Id> {
    let mut hide = std::collections::HashSet::new();
    let Some(fid) = cmd.edit else { return hide };
    let Some(node) = project.timeline.iter().find(|n| n.id == fid) else { return hide };
    if node.kind.consumed_body().is_none() {
        return hide; // a base feature: its body stays visible
    }
    let Some(b0) = node.kind.body() else { return hide };
    hide.insert(b0);
    // a forward closure over consumed(): everything that sits on b0, transitively
    let mut changed = true;
    while changed {
        changed = false;
        for n in &project.timeline {
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

/// IS THE SCENE BEING MOVED BY HAND RIGHT NOW.
///
/// Four different things can have hold of it at once: a body's gizmo, a component's gizmo, a joint being
/// dragged, and a part pulled along its joint. Everything expensive is skipped while any of them has
/// hold, and asking about one and forgetting the rest is how a heavy recompute lands in the middle of a
/// drag and the picture jerks.
#[derive(Clone, Copy)]
pub struct SceneDrag<'a> {
    pub body_giz: &'a BodyGizmo,
    pub comp_giz: CompGizmo,
    pub joint: &'a JointCommand,
    pub part_pull: &'a Option<(Id, [f64; 3], [f64; 3])>,
}

/// The four, taken from the application, where they live in two records.
#[macro_export]
macro_rules! scene_drag_of {
    ($x:expr) => {
        $crate::SceneDrag { body_giz: &$x.dragged.body_giz, comp_giz: $x.dragged.comp_giz, joint: &$x.side.joint, part_pull: &$x.dragged.part_pull }
    };
}

/// THE FLAT FRAME A 3D SCENE IS DRAWN INTO: where the camera stands, how the world is turned to face
/// the screen, the rectangle it lands in, and the settings that decide perspective against orthographic.
///
/// These four travel together everywhere a world point has to become a screen point. `basis` is the
/// camera's own three directions (`Cam3::basis`), computed once per drawing pass and carried along
/// rather than recomputed per point.
#[derive(Clone, Copy)]
pub struct Screen<'a> {
    pub cam: &'a Cam3,
    pub set: &'a Settings,
    pub rect: Rect,
    pub basis: &'a ([f64; 3], [f64; 3], [f64; 3]),
}

impl Screen<'_> {
    /// Where a world point lands, and how deep it is.
    pub fn at(&self, p: [f64; 3]) -> (Pos2, f64) {
        let (cam, set, rect, basis) = (self.cam, self.set, self.rect, self.basis);
        let rel = v_sub(p, cam.target);
        let sx = v_dot(rel, basis.0);
        let sy = v_dot(rel, basis.1);
        let depth = v_dot(rel, basis.2);
        // Perspective: the eye sits a finite `d_eye` behind the target plane along -fwd, and the screen
        // offset is divided by (1 + depth/d_eye). In orthographic mode `inv_d_eye` is 0, so f = 1 and the
        // projection matches the previous one exactly. The denominator is clamped from below (a point in
        // front of the eye) — unreachable in a CAD orbit, but it guards against degeneracy.
        let inv = persp_inv_d_eye(cam, set, rect.height() * 0.5);
        let f = (1.0 / (1.0 + depth * inv).max(0.05)) as f32;
        let c = rect.center();
        (Pos2::new(c.x + sx as f32 * f * cam.scale, c.y - sy as f32 * f * cam.scale), depth)
    }
}

/// The body is shown only as CONTEXT (a neighbouring part while `show_context` is on) rather than as
/// the active part, so it is drawn as a ghost and not picked as one's own (edit-in-context).
pub fn body_is_ghost(dc: &DrawCtx, mi: usize) -> bool {
    let ctx = current_ctx_id(dc.active_path, dc.project);
    if ctx == dc.project.root {
        return false;
    }
    match dc.project.mesh_id(mi).and_then(|b| dc.project.body_owner(b)) {
        Some(owner) => !dc.project.component_is_within(owner, ctx),
        None => false,
    }
}

/// IS A HIT UNDER THE CURSOR NEARER THAN THE BEST ONE SO FAR? By depth - except that a copy of a face lies exactly on
/// the face it was taken from, at the same depth, and which of the two a click got was down to which body came first:
/// always the part. Where a tool wants a surface - a stitch; a replace-face once the faces to replace are taken; the
/// first click of a trim - a sheet at the same depth as a solid wins; for every other tool, and for the steps that want
/// the body (the faces to replace, the body to trim with), the depth alone decides, as before.
pub fn nearer_hit(depth: f64, sheet: bool, best: Option<(f64, bool)>, prefer_sheets: bool) -> bool {
    match best {
        None => true,
        Some((best_depth, best_sheet)) => {
            // the same triangles copied: the depths agree to the rounding of the projection
            let tie = (depth - best_depth).abs() <= 1e-6 * best_depth.abs().max(1.0);
            if prefer_sheets && tie {
                sheet && !best_sheet
            } else {
                depth < best_depth
            }
        }
    }
}

/// The triangle is hidden by the section (by its centroid — for PICKING; the renderer cuts it properly by clipping).
pub fn section_tri_hidden(section: &SectionTool, a: [f64; 3], b: [f64; 3], c: [f64; 3]) -> bool {
    section.plane.is_some() && section_hidden(section, [(a[0] + b[0] + c[0]) / 3.0, (a[1] + b[1] + c[1]) / 3.0, (a[2] + b[2] + c[2]) / 3.0])
}

/// Whether to show a body (by mesh index) in the current context: the visibility tick AND the part's
/// isolation — inside a Part ONLY its own bodies are visible (each part is its own space and frame);
/// in the root assembly all of them are.
pub fn body_shown(bv: BodyView, mi: usize) -> bool {
    // THE BODY OF A RED NODE IS NOT SHOWN. A node that failed to build still has a body: it is created
    // together with the node and, until the first successful build, carries the source's mesh. Showing
    // it means drawing the part's ghost right next to it — two bodies on screen although the operation
    // failed. Caught by the fuzzer: a fillet applied to a surface goes red, and the part ends up with
    // two visible bodies.
    //
    // An empty body is hidden for the same reason: there is nothing to draw in it, yet it counts in
    // the lists of "what we show".
    // THE SOURCE OF A RED NODE, which stays on screen in place of the result that failed to build
    let mut red_source = false;
    if let Some(b) = bv.project.bodies.get(mi) {
        if b.mesh.tris.is_empty() {
            return false;
        }
        // THE BODY OF A RED NODE IS A GHOST. Only that body ITSELF is hidden: the source must stay on
        // screen, otherwise a failed operation wipes out the whole part (its body having been consumed
        // by the node that failed to build). That is the rule in full: on failure we show what was
        // there BEFORE, not emptiness and not two bodies at once.
        // NOTHING IS RED IN THE ORDINARY CASE, and then there is nothing to look for. The scan walks the whole
        // timeline for every body; on a document of 1296 bodies that is 1.7 million comparisons per pass, and
        // the pass happens on every frame.
        let red = |id: Id| !bv.project.regen_errors.is_empty() && bv.project.timeline.iter().any(|n| n.kind.owns_body(id) && bv.project.regen_errors.contains_key(&n.id));
        if red(b.id) {
            return false;
        }
        // The consuming node is red, so the source stays visible - AS THE PART'S BODY, under every rule below: its
        // own tick, its component's, the context. Reported behaviour: a part with a broken feature was seen as a
        // ghost from every other component, with In context off and with its tick off in the tree, because this
        // answered "shown" before any of them was asked.
        red_source = body_is_consumed(bv.cache, bv.project, bv.regen, b.id) && bv.project.timeline.iter().any(|n| n.kind.consumed().contains(&b.id) && bv.project.regen_errors.contains_key(&n.id));
    }
    let ctx = current_ctx_id(bv.active_path, bv.project);
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
    if !red_source && bv.project.mesh_id(mi).is_some_and(|b| body_is_consumed(bv.cache, bv.project, bv.regen, b) && Some(b) != edit_src_body(bv.cmd, bv.project)) {
        return false;
    }
    let owner = bv.project.mesh_id(mi).and_then(|b| bv.project.body_owner(b));
    // Hiding an INDIVIDUAL body by hand (the body's tick in the tree) always applies, whatever the context.
    if !bv.project.bodies.get(mi).is_none_or(|b| b.visible) {
        return false;
    }
    let Some(owner) = owner else {
        return ctx == bv.project.root; // a body with no owner is visible only in the root assembly
    };
    // The root assembly: visibility follows the chain of component ticks all the way to the root.
    if ctx == bv.project.root {
        return component_chain_visible(bv.project, owner, None);
    }
    // Inside a part, its own bodies; inside a subassembly, the bodies of all its parts (the subtree).
    // The ticks of the child parts hide their bodies, while the context component ITSELF (`stop`) is
    // excluded — on entering a hidden subassembly we see its contents according to its descendants' ticks.
    if bv.project.component_is_within(owner, ctx) {
        return component_chain_visible(bv.project, owner, Some(ctx));
    }
    // IN THE SKETCHER (while editing a sketch) the 3D bodies of in-context neighbours are NOT drawn at
    // all: they get in the way of building the part. The face being built on is shown by highlighting
    // its edges (`draw_sketch_face_edges`), not as a body.
    if bv.sketch_ses.editing.is_some() {
        return false;
    }
    // In-context mode: the bodies of NEIGHBOURING parts (in the parent assembly) are shown as ghosts,
    // so that their geometry can be referred to (top-down). Otherwise isolation hides them.
    if bv.win.context {
        if let Some(parent) = bv.project.components.iter().find(|c| c.id == ctx).and_then(|c| c.parent) {
            if bv.project.component_is_within(owner, parent) {
                return component_chain_visible(bv.project, owner, Some(parent));
            }
        }
    }
    false
}

/// Reported behaviour: the base planes XY/XZ/YZ are impossible to hit — they flash for a moment and
/// vanish as soon as the cursor moves by a pixel. A fixed half-size of 60 mm makes a clickable spot the
/// size of a pinhead at the scale of a typical assembly (hundreds of millimetres across, with the camera
/// far back) — the square is barely visible and cannot be hovered. So it is scaled by the actual
/// bounding box of the visible scene, as the section gizmo is; an empty scene keeps the old default.
pub fn plane_pick_half_size(pn: &Painting) -> f64 {
    let mut lo = [f64::MAX; 3];
    let mut hi = [f64::MIN; 3];
    for SceneMesh { mesh, world: wt, .. } in visible_mesh_items(pn) {
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

/// The display transform of a datum (a point, an axis or a plane, by its Id) in the active context's
/// frame — so that a part's datums travel with it in an assembly, just as its bodies do. None means
/// another component's datum (isolation: we do not draw it).
/// A PLANE PICKED IN THE VIEW as an origin and a normal in the frame of the context: a base plane as it is, a datum
/// carried from its owner's frame, a face from its body's. One answer for every tool that takes a plane by a click - the
/// mirrored copy and the section had a copy of it each.
pub fn plane_in_context(pn: &Painting, plane: qymcad_core::feature::SketchPlane) -> Option<([f64; 3], [f64; 3])> {
    use qymcad_core::feature::{apply12, apply12_dir, SketchPlane, PLACE_IDENTITY};
    match plane {
        SketchPlane::World(bp) => {
            let f = bp.frame();
            Some((f.origin, f.normal()))
        }
        SketchPlane::Datum(id) => pn.project.planes.iter().find(|p| p.id == id).map(|p| {
            // a datum is stored in its owner's LOCAL frame - its transform carries it into the context
            let wt = datum_render_transform(pn, id).unwrap_or(PLACE_IDENTITY);
            (apply12(&wt, p.origin), apply12_dir(&wt, p.normal))
        }),
        SketchPlane::Face(body, key) => {
            let wt = pn.project.body_display_transform(body, current_ctx_id(pn.active_path, pn.project));
            Some((apply12(&wt, key.centroid), apply12_dir(&wt, key.normal)))
        }
    }
}

pub fn datum_render_transform(pn: &Painting, datum_id: Id) -> Option<[f64; 12]> {
    use qymcad_core::feature::FeatureKind as FK;
    // the visibility tick in the tree (by a stable Id): a hidden datum is drawn nowhere
    if pn.datum.hidden.contains(&datum_id) {
        return None;
    }
    let owner = pn
        .project
        .timeline
        .iter()
        .find(|n| {
            matches!(n.kind, FK::DatumPoint { point } if point == datum_id)
                || matches!(n.kind, FK::DatumAxis { axis } if axis == datum_id)
                || matches!(n.kind, FK::Plane { plane } if plane == datum_id)
        })
        .and_then(|n| n.parent)?;
    // A datum is visible ONLY when the visibility context is the owner itself or one of its
    // descendants (we are "inside" the owner). So a Part's datums are NOT shown in the assembly or at
    // the root (owner is the Part, viz is the Assembly, and viz is not inside owner), but they appear
    // as soon as one enters the part. An ancestor assembly's datums, meanwhile, stay visible from a
    // nested part (ancestor reference geometry). `viz_ctx_id` accounts for sketch editing — the datums
    // of the sketch's owner are visible even when it was entered by a double click from the root.
    let viz = viz_ctx_id(pn.active_path, pn.project, pn.sketch_ses);
    if !pn.project.component_is_within(viz, owner) {
        return None; // another component's datum, or a part's datum seen from the assembly: not shown
    }
    // An ANCESTOR's datum (the owner being a strict ancestor of the context: a plane or an axis of the
    // ASSEMBLY, seen from a nested subassembly or part) is shown ONLY with in-context mode on. The
    // context's own datums are always visible. This keeps the parent's reference geometry from
    // cluttering the part by default.
    if owner != viz && !pn.win.context {
        return None;
    }
    // the placement is RELATIVE to the active context, just as bodies are, so that the datum travels with the part
    Some(pn.project.relative_transform(owner, current_ctx_id(pn.active_path, pn.project)))
}

/// Whether the chain of components from `owner` up through its parents is visible (every `visible`
/// tick is on). The walk stops BEFORE `stop` (`stop` itself and its ancestors are not checked) — so
/// on entering a hidden subassembly or part (the context being `stop`), its contents are shown
/// according to the ticks of its own descendants.
/// IS SKETCH `sketch` SHOWN BY THE TICKS OF ITS COMPONENTS, seen from context `ctx`: the same chain its part's bodies go
/// by - a sketch of a part whose tick is off is hidden with the part. Reported behaviour: in an assembly the sketches of
/// a hidden part stayed on screen with "Sketch outlines" on, over the place the part was hidden from.
pub fn sketch_shown_by_components(project: &qymcad_core::model::Project, sketch: Id, ctx: Id) -> bool {
    let Some(owner) = project.node_component(sketch) else { return ctx == project.root };
    if ctx == project.root {
        return component_chain_visible(project, owner, None);
    }
    !project.component_is_within(owner, ctx) || component_chain_visible(project, owner, Some(ctx))
}

pub fn component_chain_visible(project: &qymcad_core::model::Project, owner: Id, stop: Option<Id>) -> bool {
    let mut cur = Some(owner);
    while let Some(id) = cur {
        if Some(id) == stop {
            break;
        }
        let Some(c) = project.components.iter().find(|c| c.id == id) else {
            break;
        };
        if !c.visible {
            return false;
        }
        cur = c.parent;
    }
    true
}

/// `inv_d_eye = 1/d_eye` for perspective (0 in orthographic mode). `d_eye = world_half_h / tan(fov/2)`,
/// where `world_half_h = half_h_points / scale`. The single source of the formula for `Screen::at` and for
/// the GPU shader.
pub fn persp_inv_d_eye(cam: &Cam3, set: &Settings, half_h_points: f32) -> f64 {
    if set.projection == Projection::Ortho {
        return 0.0;
    }
    // THE HALF-TANGENT FROM THE ANGLE: the setting holds an angle in degrees (which is what one
    // pictures), while the projection needs the half-tangent. The conversion lives here alone, at the
    // point of use.
    (set.persp_fov_deg.to_radians() / 2.0).tan() * cam.scale as f64 / (half_h_points.max(1.0) as f64)
}

/// Whether a body is consumed — cached, because it is asked for every body on every frame.
pub fn body_is_consumed(cache: &Caches, project: &Project, regen: &Rebuilding, body: Id) -> bool {
    let mut c = cache.consumed.borrow_mut();
    if c.rev != view_rev(regen) {
        c.rev = view_rev(regen);
        c.value = project.consumed_bodies();
    }
    c.value.contains(&body)
}

/// Is the point HIDDEN by the section? (the side along the normal is hidden; "Flip" changes the normal's sign)
pub fn section_hidden(section: &SectionTool, p: [f64; 3]) -> bool {
    match section_eff(section) {
        Some((o, n)) => (p[0] - o[0]) * n[0] + (p[1] - o[1]) * n[1] + (p[2] - o[2]) * n[2] > 0.0,
        None => false,
    }
}

/// The VISIBILITY context: while a sketch is being edited it is the sketch's owner component (so
/// isolation works however the sketch was entered, even by a double click from the root); otherwise
/// it is the active context. `active_path` is NOT touched — that keeps the drill-in/drill-out stack
/// invariant (every push of the path is paired with one on `nav_stash`).
pub fn viz_ctx_id(active_path: &[Id], project: &Project, sketch_ses: SketchSession) -> Id {
    if let Some(sid) = sketch_ses.editing {
        if let Some(owner) = project.sketch_owner(sid) {
            return owner;
        }
    }
    current_ctx_id(active_path, project)
}

pub fn v_sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// The EFFECTIVE section plane: the base one, plus the tilts (deg about U and V), plus the shift along the normal.
pub fn section_eff(section: &SectionTool) -> Option<([f64; 3], [f64; 3])> {
    let (o, n0) = section.plane?;
    let nn = v_norm(n0);
    let a = if nn[0].abs() < 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
    let u = v_norm(v_cross(a, nn));
    let v = v_cross(nn, u);
    let rot = |vec: [f64; 3], axis: [f64; 3], deg: f64| -> [f64; 3] {
        let (s, c) = deg.to_radians().sin_cos();
        let cr = v_cross(axis, vec);
        let d = v_dot(axis, vec);
        [vec[0] * c + cr[0] * s + axis[0] * d * (1.0 - c), vec[1] * c + cr[1] * s + axis[1] * d * (1.0 - c), vec[2] * c + cr[2] * s + axis[2] * d * (1.0 - c)]
    };
    let n = v_norm(rot(rot(nn, u, section.rot[0]), v, section.rot[1]));
    let o = [o[0] + n[0] * section.offset, o[1] + n[1] * section.offset, o[2] + n[2] * section.offset];
    Some((o, n))
}

/// The visible bodies to draw: (highlighted, ghosted, base colour, mesh, world transform).
/// The single source for the CPU raster and for the GPU pass (it accounts for consumed bodies, feature
/// editing, the assembly context and the body gizmo's preview).
/// The first element of the tuple is the mesh's index in `project.meshes` (for the normals cache).
/// ONE BODY AS THE SCENE SEES IT: where it stands, what colour it is, and how it is being shown.
///
/// It was `(usize, bool, bool, [u8; 3], &Mesh, [f64; 12])`, and the catalogue of debt names it as ITS OWN
/// example: to learn what the second `bool` meant one had to read the body of the function. Every caller
/// read it as `for (_, _, _, _, mesh, wt)` - four holes in a row, and getting one wrong would have drawn
/// the ghosts as solid.
pub struct SceneMesh<'a> {
    /// The mesh's index in `project.bodies`.
    pub index: usize,
    /// HIGHLIGHTED BY THE SELECTION. A body lights itself; a component lights its whole subtree.
    pub hot: bool,
    /// DRAWN AS A GHOST: a part outside the active context, or an operation's preview.
    pub ghost: bool,
    /// Its colour.
    pub tint: [u8; 3],
    /// The mesh itself.
    pub mesh: &'a qymcad_core::geom::Mesh,
    /// The WORLD transform of the owning component, with the gizmo's drag laid over it.
    pub world: [f64; 12],
}
/// THE COLOURS OF A BODY'S FACES, where a file coloured single faces of it: the distinct colours in the order the faces
/// meet them, and for every one of the mesh's `n_tris` triangles the place of its colour in that list - `None` where the
/// triangle takes the body's own. Both empty for a body with no face of a colour of its own, which is nearly every body;
/// a document with none at all answers without looking at a single face. Both painters draw from this one list, the
/// software raster and the device's scene, so they cannot tell a face's colour differently.
pub fn face_palette(project: &Project, mi: usize, n_tris: usize) -> (Vec<[u8; 3]>, Vec<Option<u8>>) {
    let none = (Vec::new(), Vec::new());
    if project.face_colors.is_empty() && project.tri_colors.is_empty() {
        return none;
    }
    let Some(body) = project.mesh_id(mi) else { return none };
    // a piece of a mesh whose file coloured it triangle by triangle. Whether it has a palette is the mesh's own answer,
    // whatever `n_tris` a caller passes, so the rows of the look table agree between the frame's table and the scene.
    if let Some((palette, places)) = project.tri_colors.get(&project.lineage_root(body)) {
        if places.len() == project.bodies[mi].mesh.tris.len() {
            let per = if n_tris == places.len() { places.iter().map(|&k| Some(k)).collect() } else { Vec::new() };
            return (palette.clone(), per);
        }
    }
    let Some(coloured) = project.face_colors.get(&project.lineage_root(body)).filter(|f| !f.is_empty()) else { return none };
    let faces = project.regen_faces.get(&body).map(|f| f.as_slice()).unwrap_or(project.bodies[mi].faces.as_slice());
    let (mut palette, mut per) = (Vec::new(), vec![None; n_tris]);
    for f in faces {
        let Some(c) = coloured.iter().find(|(id, _)| *id == f.id && f.id != 0).map(|(_, c)| *c) else { continue };
        let k = match palette.iter().position(|p| *p == c) {
            Some(k) => k,
            None if palette.len() < 255 => {
                palette.push(c);
                palette.len() - 1
            }
            None => continue, // past 255 colours a face takes the body's own
        };
        for &t in &f.triangles {
            if let Some(slot) = per.get_mut(t as usize) {
                *slot = Some(k as u8);
            }
        }
    }
    if palette.is_empty() {
        per.clear();
    }
    (palette, per)
}

/// THE TALLEST BODY ON SCREEN, along Z. Says whether the shapes came back after a reload.
pub fn tallest_body(pn: &Painting) -> f64 {
    visible_mesh_items(pn).iter().filter_map(|m| m.mesh.bounds()).map(|bb| bb.max.z - bb.min.z).fold(0.0, f64::max)
}

/// WHAT `body_shown` ACTUALLY DEPENDS ON, as one number - the key of the cache below.
///
/// The geometry revision alone is not enough: the answer also turns on the context, on the "show neighbours"
/// toggle, on a sketch being edited, on the feature being edited, and on every tick box in the tree. Keyed by
/// the revision alone, the list stayed stale until the next rebuild - switching "in context" changed nothing
/// on screen. Hashing two fields per body and per component costs microseconds on 1296 bodies.
fn shown_bodies_key(pn: &Painting) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    view_rev(pn.regen).hash(&mut h);
    current_ctx_id(pn.active_path, pn.project).hash(&mut h);
    pn.win.context.hash(&mut h);
    pn.sketch_ses.editing.is_some().hash(&mut h);
    pn.cmd.edit.hash(&mut h);
    pn.project.regen_errors.len().hash(&mut h);
    for b in &pn.project.bodies {
        (b.visible, b.mesh.tris.is_empty()).hash(&mut h);
    }
    for c in &pn.project.components {
        (c.visible, c.parent).hash(&mut h);
    }
    h.finish()
}

/// THE VISIBLE BODIES - as a list from a cache rather than recomputed for every body every frame.
///
/// Deciding whether a body is visible means finding its owner (a walk over the timeline) and following the
/// chain of tick boxes up the tree. That is not expensive in itself, but it repeats for EVERY body: in the
/// picking loop it hung the application while an edge anchor was being chosen, and in the scene it cost 175 ms
/// per frame on a document of 1296 bodies - felt as the whole program going sluggish, settings windows
/// included.
pub fn shown_bodies(pn: &Painting) -> Vec<(usize, Id)> {
    let key = shown_bodies_key(pn);
    let ctx = current_ctx_id(pn.active_path, pn.project);
    {
        let c = pn.cache.shown_bodies.borrow();
        if c.rev == key && c.value.ctx == ctx {
            return c.value.list.clone();
        }
    }
    let list: Vec<(usize, Id)> = (0..pn.project.bodies.len()).filter(|&mi| body_shown(pn.body_view(), mi)).filter_map(|mi| pn.project.mesh_id(mi).map(|b| (mi, b))).collect();
    pn.cache.shown_bodies.borrow_mut().put(key, ShownBodies { ctx, list: list.clone() });
    list
}

/// THE PATH OF CONTEXTS AFTER STEPPING INTO `cid` FROM `path`: one level deeper - or, for a clone, the way from the
/// root to its original. A clone has no timeline of its own; stepping into it is editing the part it repeats, and
/// that part may stand in another subassembly.
pub fn path_into(path: &[Id], project: &Project, cid: Id) -> Vec<Id> {
    let origin = project.instance_origin(cid);
    if origin == cid {
        let mut deeper = path.to_vec();
        deeper.push(cid);
        return deeper;
    }
    let mut chain = vec![origin];
    let mut cur = origin;
    // the parents up to the root; the bound guards a loop in a damaged document, no tree is that deep
    while let Some(parent) = project.components.iter().find(|c| c.id == cur).and_then(|c| c.parent) {
        if chain.len() > 256 {
            break;
        }
        chain.push(parent);
        cur = parent;
    }
    chain.reverse();
    chain
}

pub fn visible_mesh_items<'a>(pn: &'a Painting) -> Vec<SceneMesh<'a>> {
    // the selection highlight: a body highlights itself; a component highlights its whole subtree (the part or subassembly entire)
    let hl = highlight_mesh_set(pn.project, &pn.sel);
    // every mesh carries the WORLD transform of its owning component: a body is built in the part's
    // local frame, and `world_transform` (composed up the assembly tree) places it into the world.
    // While a modifier feature (shell, hole, fillet, chamfer and so on) is being edited, the state
    // BEFORE it is shown: its RESULT and its whole descendant chain are hidden. The consumed SOURCE
    // is shown by `body_shown`, which is also what throws consumed bodies away — keeping a SECOND
    // check of the same thing here was a mistake: the exception for the source ended up behind an
    // earlier refusal and never worked.
    let edit_hide = edit_hidden_bodies(pn.cmd, pn.project);
    let ctx = current_ctx_id(pn.active_path, pn.project); // the placement is RELATIVE to the active context (a part sits at the origin, an assembly in place)
                                                          // THE PLACEMENT IS ASKED ONCE PER OWNER, not once per body. `body_display_transform` inverts and
                                                          // multiplies its way up the component tree, looking every level up by a linear scan; on a document of
                                                          // 1296 bodies that pass is what the frame was being spent on. Bodies of one part share one answer.
    let mut placed: std::collections::HashMap<Id, [f64; 12]> = std::collections::HashMap::new();
    // WHOSE EACH BODY IS, in ONE pass over the timeline instead of one pass per body. Asked body by body
    // it is 1.7 million comparisons on a document of 1296 bodies, every frame.
    // THE FIRST NODE THAT OWNS THE BODY, exactly as `Project::body_owner` answers: a body travels through
    // the nodes that consume and remake it, and the later ones may sit in another component. Taking the
    // last one instead turned parts into ghosts of a foreign context and placed them by the wrong frame.
    let mut owner_of: std::collections::HashMap<Id, Option<Id>> = std::collections::HashMap::new();
    for n in &pn.project.timeline {
        for b in n.kind.bodies() {
            owner_of.entry(b).or_insert(n.parent);
        }
    }
    // A GHOST IS A PROPERTY OF THE OWNER, not of the body: every body of one part answers alike.
    let root = pn.project.root;
    let mut ghostly: std::collections::HashMap<Id, bool> = std::collections::HashMap::new();
    shown_bodies(pn)
            .into_iter()
            .filter(|(_, b)| !edit_hide.contains(b)) // the feature being edited plus its descendant chain
            .filter_map(|(mi, b)| pn.project.bodies.get(mi).map(|body| (mi, b, &body.mesh)))
            .map(|(mi, id, m)| {
                let owner = owner_of.get(&id).copied().flatten();
                let ghost = match owner {
                    Some(o) if ctx != root => *ghostly.entry(o).or_insert_with(|| !pn.project.component_is_within(o, ctx)),
                    _ => false,
                };
                let mut wt = match owner {
                    Some(o) => *placed.entry(o).or_insert_with(|| pn.project.relative_transform(o, ctx)),
                    None => qymcad_core::feature::PLACE_IDENTITY,
                };
                // THE BODY GIZMO'S PREVIEW: while dragging, the accumulated transform is laid over this body
                // (with no B-rep rebuild until release). Ctrl snaps.
                if let Some((dmi, _, _)) = pn.body_giz.drag {
                    if dmi == mi {
                        if let Some(accum) = body_giz_accum(pn.body_giz, pn.set, pn.body_giz.snap) {
                            wt = compose12(&accum, &wt);
                        }
                    }
                }
                SceneMesh {
                    index: mi,
                    hot: hl.contains(&mi),
                    ghost,
                    tint: pn.project.mesh_color(mi),
                    mesh: m,
                    world: wt,
                }
            })
            .collect()
}

/// The composition of two 3x4 affine transforms (row-major): the result is `a` after `b` (apply b, then a). Used by the body gizmo.
pub fn compose12(a: &[f64; 12], b: &[f64; 12]) -> [f64; 12] {
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

/// The indices of the bodies highlighted as "selected" in 3D. Select a body and that body lights up;
/// select a component (a part or a subassembly) in the tree or by a click in the assembly and its
/// WHOLE subtree lights up, so that it is plain that this part or subassembly was selected entire.
pub fn highlight_mesh_set(project: &Project, sel: &Sel) -> std::collections::HashSet<usize> {
    let mut hl = std::collections::HashSet::new();
    match *sel {
        Sel::Mesh(i) | Sel::Face(i, _) => {
            hl.insert(i);
        }
        Sel::Component(ci) => {
            if let Some(cid) = project.components.get(ci).map(|c| c.id) {
                let mut sub: std::collections::HashSet<Id> = project.descendants(cid).into_iter().collect();
                sub.insert(cid);
                for (mi, b) in project.bodies.iter().map(|b| b.id).enumerate() {
                    if project.body_owner(b).is_some_and(|o| sub.contains(&o)) {
                        hl.insert(mi);
                    }
                }
            }
        }
        _ => {}
    }
    hl
}

/// The transform accumulated by a body gizmo over the current drag (in the Part's world frame): a shift
/// along an axis OR a rotation about a ring through the fixed origin. `snap` rounds to the step (the
/// grid, or the angle). None means there is no drag.
pub fn body_giz_accum(body_giz: &BodyGizmo, set: &Settings, snap: bool) -> Option<[f64; 12]> {
    let (_, o, amt) = body_giz.drag?;
    if let Some(ax) = body_giz.axis {
        let step = body_snap_mm(set);
        let d = if snap { (amt / step).round() * step } else { amt };
        let mut t = qymcad_core::feature::PLACE_IDENTITY;
        t[[3, 7, 11][ax as usize]] = d; // a shift along the world axis `ax`
        Some(t)
    } else if let Some(ax) = body_giz.ring {
        let deg = if snap { (amt / body_snap_deg(set)).round() * body_snap_deg(set) } else { amt };
        Some(rot_about_point(ax, deg, o))
    } else {
        None
    }
}

/// A rotation by `deg` about the WORLD axis `ax` (0 = X, 1 = Y, 2 = Z) through the point `o`, as a 3x4
/// affine transform. Used by the body gizmo.
pub fn rot_about_point(ax: u8, deg: f64, o: [f64; 3]) -> [f64; 12] {
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

/// The snapping step for moving a body (mm): the grid step from the snapping panel.
pub fn body_snap_mm(set: &Settings) -> f64 {
    set.snap.grid.max(0.01)
}

/// The snapping step for rotating a body (deg): the rotation field from the snapping panel.
pub fn body_snap_deg(set: &Settings) -> f64 {
    set.snap.rot_deg.max(0.1)
}

/// The current value of a command parameter, by key.
pub fn cmd_val(cmd: &FeatCommand, key: &str) -> f64 {
    cmd.params.iter().find(|p| p.key == key).map(|p| p.val).unwrap_or(0.0)
}

/// The command's fields for the current mode. A thread is specified the professional way — by standard
/// and size; the depth, the diameters and the profile are computed by the model's core from the
/// standard's formulas (the angle and the thread depth used to be typed by hand, and the result was
/// whatever came out). An auger has a flight of its own: the outer diameter, the pitch, the thickness.
pub fn set_thread_params(cmd: &mut FeatCommand, thread: ThreadParams) {
    let d = if thread.radius > 1e-6 { thread.radius * 2.0 } else { 10.0 };
    cmd.params = if thread.auger {
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
    if !thread.auger && thread_standard(thread.form) == qymcad_core::thread::ThreadStandard::Custom {
        cmd.params.push(CmdParam::new("th-profile-angle", "angle", 60.0, 5.0, 170.0));
        // THE DEPTH OPENS ON THE ONE THAT WILL BE CUT. Zero in this field does not mean a groove of no
        // depth: the core reads it as "nothing was typed" and cuts 0.6 of the pitch. And the pitch itself
        // is zero by default, meaning "the standard coarse pitch for this size" - so seeding the field from
        // the pitch typed in gave 0.00 over a groove 1.50 deep on a Ø20, a number describing nothing.
        cmd.params.push(CmdParam::new("th-depth", "depth", custom_depth_to_start_from(cmd, thread), 0.0, 1000.0));
    }
}

/// Start renaming a component, a datum or a body in place: the current name goes into the field, with
/// auto-focus. The other two targets are cleared - one field serves all three, and two of them set at once
/// would rename twice.
pub fn start_rename_node(rename: &mut RenameInput, node: RenameNode, cur: String) {
    *rename = RenameInput { node: Some(node), buf: cur, focus: true, ..Default::default() };
}

/// Start renaming timeline node `id` in place.
pub fn start_rename(project: &qymcad_core::model::Project, rename: &mut RenameInput, id: Id) {
    let cur = project.timeline.iter().find(|n| n.id == id).map(|n| qymcad_i18n::name(&n.name)).unwrap_or_default();
    *rename = RenameInput { target: Some(id), buf: cur, focus: true, ..Default::default() };
}

/// Start renaming sketch `sid` in place.
pub fn start_rename_sketch(rename: &mut RenameInput, sid: Id, cur: String) {
    *rename = RenameInput { sketch: Some(sid), buf: cur, focus: true, ..Default::default() };
}

/// START RENAMING WHATEVER IS SELECTED. Says whether there was anything to rename.
///
/// Reported behaviour: "renaming parts/subassemblies/sketches/features not only by right-click -> Rename
/// but also by pressing F2."
///
/// ONE DOOR, because the tree renames five kinds of node through three different fields - a feature by
/// `target`, a sketch by `sketch`, a component, a body and a datum by `node` - and each menu item started
/// its own. A key that had to repeat that fan-out would be a sixth copy, drifting from the other five at
/// the first kind of node added.
pub fn rename_selected(project: &qymcad_core::model::Project, rename: &mut RenameInput, sel: Sel) -> bool {
    // ALREADY RENAMING THIS VERY NODE: leave what has been typed alone. F2 pressed a second time, or the
    // menu item used while the field is open, must not throw a half-typed name away and start over.
    if renaming_now(project, rename, sel) {
        return true;
    }
    match sel {
        Sel::Feature(fi) => match project.timeline.get(fi) {
            Some(n) => {
                start_rename(project, rename, n.id);
                true
            }
            None => false,
        },
        Sel::Sketch(si) => match project.sketches.get(si) {
            Some(s) => {
                start_rename_sketch(rename, s.id, qymcad_i18n::name(&s.name));
                true
            }
            None => false,
        },
        Sel::Component(ci) => match project.components.get(ci) {
            Some(c) => {
                start_rename_node(rename, RenameNode::Component(c.id), qymcad_i18n::name(&c.name));
                true
            }
            None => false,
        },
        Sel::Mesh(mi) => match project.bodies.get(mi) {
            Some(_) => {
                start_rename_node(rename, RenameNode::Body(mi), qymcad_i18n::name(&project.mesh_name(mi)));
                true
            }
            None => false,
        },
        Sel::Plane(pi) => match project.planes.get(pi) {
            Some(p) => {
                start_rename_node(rename, RenameNode::Plane(p.id), qymcad_i18n::name(&p.name));
                true
            }
            None => false,
        },
        Sel::DatumPoint(i) => match project.datum_points.get(i) {
            Some(d) => {
                start_rename_node(rename, RenameNode::DatumPoint(d.id), qymcad_i18n::name(&d.name));
                true
            }
            None => false,
        },
        Sel::DatumAxis(i) => match project.datum_axes.get(i) {
            Some(d) => {
                start_rename_node(rename, RenameNode::DatumAxis(d.id), qymcad_i18n::name(&d.name));
                true
            }
            None => false,
        },
        // A face, a contour, a mate or nothing at all: none of these carries a name of its own.
        Sel::None | Sel::Face(..) | Sel::Contour(_) | Sel::Joint(_) | Sel::Edge(..) | Sel::Vertex(..) => false,
    }
}

/// WHERE A SEGMENT CROSSES ONE OF THE SKETCH AXES: 0 is the line x = 0, 1 is the line y = 0.
///
/// The axes are infinite, so this is not `seg_seg_intersect` against some long enough stand-in segment -
/// "long enough" is a number pulled out of the air, and at a far enough zoom it stops being long enough.
/// THE MIDDLE OF WHAT IS SELECTED, in sketch coordinates - to tell "it moved" from "it was held".
///
/// The centre of the bounding box rather than an average of points: an average shifts with how many
/// points a shape happens to be built from, and a circle carries one while a rectangle carries four.
pub fn entities_centroid(project: &Project, si: usize, eids: &[Id]) -> Option<Point2> {
    let s = project.sketches.get(si)?;
    let points_by_id: std::collections::HashMap<Id, &qymcad_core::model::SketchPoint> = s.points.iter().map(|p| (p.id, p)).collect(); // a table: these are looked up for every entity or constraint
    let pt = |id: Id| points_by_id.get(&id).copied().map(|p| Point2::new(p.x, p.y));
    let (mut lo, mut hi) = ((f64::MAX, f64::MAX), (f64::MIN, f64::MIN));
    let mut seen = false;
    for e in s.entities.iter().filter(|e| eids.contains(&e.id)) {
        for id in qymcad_core::model::entity_points(e) {
            if let Some(p) = pt(id) {
                lo = (lo.0.min(p.x), lo.1.min(p.y));
                hi = (hi.0.max(p.x), hi.1.max(p.y));
                seen = true;
            }
        }
    }
    seen.then(|| Point2::new((lo.0 + hi.0) / 2.0, (lo.1 + hi.1) / 2.0))
}

pub fn seg_axis_intersect(a: Point2, b: Point2, axis: u8) -> Option<Point2> {
    let (pa, pb) = if axis == 0 { (a.x, b.x) } else { (a.y, b.y) };
    let d = pb - pa;
    if d.abs() < 1e-12 {
        return None; // parallel to the axis: either no crossing or the whole segment lies on it
    }
    let t = -pa / d;
    (0.0..=1.0).contains(&t).then(|| Point2::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t))
}

/// Where a circle crosses one of the sketch axes - none, or both points.
pub fn circle_axis_intersect(c: Point2, r: f64, axis: u8) -> Vec<Point2> {
    let off = if axis == 0 { c.x } else { c.y };
    if off.abs() > r {
        return Vec::new();
    }
    let h = (r * r - off * off).max(0.0).sqrt();
    if axis == 0 {
        vec![Point2::new(0.0, c.y - h), Point2::new(0.0, c.y + h)]
    } else {
        vec![Point2::new(c.x - h, 0.0), Point2::new(c.x + h, 0.0)]
    }
}

/// Is the rename field already open on exactly this selection?
fn renaming_now(project: &qymcad_core::model::Project, rename: &RenameInput, sel: Sel) -> bool {
    match sel {
        Sel::Feature(fi) => project.timeline.get(fi).is_some_and(|n| rename.target == Some(n.id)),
        Sel::Sketch(si) => project.sketches.get(si).is_some_and(|s| rename.sketch == Some(s.id)),
        Sel::Component(ci) => project.components.get(ci).is_some_and(|c| rename.node == Some(RenameNode::Component(c.id))),
        Sel::Mesh(mi) => rename.node == Some(RenameNode::Body(mi)),
        Sel::Plane(pi) => project.planes.get(pi).is_some_and(|p| rename.node == Some(RenameNode::Plane(p.id))),
        Sel::DatumPoint(i) => project.datum_points.get(i).is_some_and(|d| rename.node == Some(RenameNode::DatumPoint(d.id))),
        Sel::DatumAxis(i) => project.datum_axes.get(i).is_some_and(|d| rename.node == Some(RenameNode::DatumAxis(d.id))),
        Sel::None | Sel::Face(..) | Sel::Contour(_) | Sel::Joint(_) | Sel::Edge(..) | Sel::Vertex(..) => false,
    }
}

/// WHERE THE VIEW ZOOMS FROM.
///
/// Reported behaviour: "zooming pans from the origin rather than from the cursor's coordinates - both in
/// sketches and in the 3D viewport. Two variants are needed, in the settings."
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum ZoomAt {
    /// The point under the cursor stays put - the point one is looking at is the point one aims at.
    Cursor,
    /// The middle of the viewport stays put, whatever the cursor is over.
    ViewCentre,
}

impl ZoomAt {
    pub const ALL: [ZoomAt; 2] = [ZoomAt::Cursor, ZoomAt::ViewCentre];

    pub fn key(self) -> &'static str {
        match self {
            ZoomAt::Cursor => "settings-zoom-at-cursor",
            ZoomAt::ViewCentre => "settings-zoom-at-centre",
        }
    }
}

/// WHAT THE VIEW TURNS ABOUT.
///
/// The middle of the view is the program's own way and stays the default. Turning about the point of the model under
/// the pointer is the other habit, asked for in a pull request: the place one is looking at stays where it is while
/// the rest turns around it. A list rather than a pair of buttons, so another centre - the model's, a chosen element -
/// is one more variant.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum OrbitAbout {
    /// The middle of the view, whatever the pointer is over.
    ViewCentre,
    /// The point under the pointer where the turn begins: the face hit there, else the plane through the middle of the
    /// view square to it. Held for as long as the turn lasts.
    Pointer,
}

impl OrbitAbout {
    pub const ALL: [OrbitAbout; 2] = [OrbitAbout::ViewCentre, OrbitAbout::Pointer];

    pub fn key(self) -> &'static str {
        match self {
            OrbitAbout::ViewCentre => "settings-orbit-about-centre",
            OrbitAbout::Pointer => "settings-orbit-about-pointer",
        }
    }

    /// What the variant does, said under the list.
    pub fn hint_key(self) -> &'static str {
        match self {
            OrbitAbout::ViewCentre => "settings-orbit-about-centre-hint",
            OrbitAbout::Pointer => "settings-orbit-about-pointer-hint",
        }
    }
}

/// HOW OFTEN THE PROGRAM ASKS WHETHER A NEWER VERSION EXISTS.
///
/// A program that never says so leaves people on the version they installed. Measured in September 2026:
/// 470 downloads against 10-20 people in the chat - so almost everybody running it is reachable by
/// nothing, and every fix made since their download is invisible to them.
///
/// The other side is a program that goes to the network without being asked, and that is why this is a
/// setting with `Never` in it rather than a fact of life.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum UpdateCheck {
    /// At every start.
    AtStart,
    /// At a start, but no oftener than once a day. The default: a release may appear any day, and one
    /// request a day is nothing to anybody.
    Daily,
    /// At a start, no oftener than once a week.
    Weekly,
    /// At a start, no oftener than once a month.
    Monthly,
    /// Never on its own. The menu item still works: a person who presses it is asking.
    Never,
}

impl UpdateCheck {
    pub const ALL: [UpdateCheck; 5] = [UpdateCheck::AtStart, UpdateCheck::Daily, UpdateCheck::Weekly, UpdateCheck::Monthly, UpdateCheck::Never];

    pub fn key(self) -> &'static str {
        match self {
            UpdateCheck::AtStart => "settings-updates-at-start",
            UpdateCheck::Daily => "settings-updates-daily",
            UpdateCheck::Weekly => "settings-updates-weekly",
            UpdateCheck::Monthly => "settings-updates-monthly",
            UpdateCheck::Never => "settings-updates-never",
        }
    }

    /// How long to wait between checks, in seconds. `None` means never on its own.
    fn every(self) -> Option<u64> {
        match self {
            UpdateCheck::AtStart => Some(0),
            UpdateCheck::Daily => Some(24 * 60 * 60),
            UpdateCheck::Weekly => Some(7 * 24 * 60 * 60),
            UpdateCheck::Monthly => Some(30 * 24 * 60 * 60),
            UpdateCheck::Never => None,
        }
    }
}

/// IS IT TIME TO ASK, given when it was last asked. Both times are seconds since the epoch.
///
/// A CLOCK THAT WENT BACKWARDS ASKS AGAIN. The last time is written by the program itself, and a machine
/// whose clock was wrong, or that came back from suspend across a change of timezone, can hold a "last
/// checked" in the future. Waiting for it to come round would mean a copy that never checks again and
/// never says why, which is the worst of the outcomes here - so an impossible answer asks now.
pub fn update_check_due(how_often: UpdateCheck, last_checked: u64, now: u64) -> bool {
    let Some(every) = how_often.every() else { return false };
    if last_checked == 0 || last_checked > now {
        return true; // never asked, or a clock that cannot be believed
    }
    now - last_checked >= every
}

/// WHERE THE VIEW ZOOMS FROM WHILE A COMMAND IS OPEN.
///
/// Reported behaviour: "and with a tool's popup open, panning should go from the centre of the part (put
/// that in the settings too)."
///
/// A command's fields stand AT THE GEOMETRY, and zooming to the cursor drags that geometry - and the
/// fields with it - out from under the hand that is typing into them. Holding the part still keeps the
/// popup where it was put.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum ZoomWhileEditing {
    /// The middle of the part being edited stays put.
    PartCentre,
    /// No exception: the ordinary rule applies while a command is open too.
    AsUsual,
}

impl ZoomWhileEditing {
    pub const ALL: [ZoomWhileEditing; 2] = [ZoomWhileEditing::PartCentre, ZoomWhileEditing::AsUsual];

    pub fn key(self) -> &'static str {
        match self {
            ZoomWhileEditing::PartCentre => "settings-zoom-editing-part",
            ZoomWhileEditing::AsUsual => "settings-zoom-editing-usual",
        }
    }
}

/// ZOOM THE FLAT VIEW, KEEPING `anchor` WHERE IT IS ON SCREEN.
///
/// The scale alone was changed, which keeps the MIDDLE of the viewport still: whatever a person is looking
/// at slides away from the cursor as the view grows, and to reach a corner one zooms and then pans it back.
/// Keeping the anchor still is one arithmetic step: the world point under it must read the same after the
/// change as before, and `to_world` says what that costs.
pub fn zoom_view_2d(view: &mut View2d, rect: Rect, anchor: Pos2, factor: f32, lo: f32, hi: f32) {
    let before = view.scale;
    view.scale = (view.scale * factor).clamp(lo, hi);
    if (view.scale - before).abs() < f32::EPSILON {
        return; // at the limit: nothing moved, so nothing has to be compensated
    }
    let c = rect.center();
    let k = 1.0 / before - 1.0 / view.scale;
    view.center.x += (anchor.x - c.x) * k;
    view.center.y -= (anchor.y - c.y) * k;
}

/// ZOOM THE 3D VIEW, KEEPING `anchor` WHERE IT IS ON SCREEN.
///
/// The world point held still is the one lying in the plane through the camera's target, which is the plane
/// a CAD orbit turns about; under perspective a point off that plane drifts by the difference in
/// foreshortening, and that is invisible next to the zoom itself.
pub fn zoom_cam_3d(cam: &mut Cam3, rect: Rect, anchor: Pos2, factor: f32, lo: f32, hi: f32) {
    let before = cam.scale;
    cam.scale = (cam.scale * factor).clamp(lo, hi);
    if (cam.scale - before).abs() < f32::EPSILON {
        return;
    }
    let c = rect.center();
    let (bx, by, _) = cam.basis();
    let k = (1.0 / before - 1.0 / cam.scale) as f64;
    let (du, dv) = (((anchor.x - c.x) as f64) * k, (-(anchor.y - c.y) as f64) * k);
    for a in 0..3 {
        cam.target[a] += bx[a] * du + by[a] * dv;
    }
}

/// WHERE THE VIEW MUST HOLD STILL WHILE IT IS ZOOMED: the cursor, the middle of the part being edited, or
/// the middle of the viewport.
///
/// One place decides it for both viewports - the flat one and the 3D one - because it is one rule and a
/// second copy of it would drift on the first change.
pub fn zoom_anchor(set: &Settings, rect: Rect, cursor: Option<Pos2>, part_centre: Option<Pos2>) -> Pos2 {
    if let (ZoomWhileEditing::PartCentre, Some(p)) = (set.zoom_editing, part_centre) {
        return p; // a command is open and its fields stand at the geometry: hold the geometry still
    }
    match set.zoom_at {
        ZoomAt::Cursor => cursor.unwrap_or_else(|| rect.center()),
        ZoomAt::ViewCentre => rect.center(),
    }
}

/// THE MOVEMENT OF A GESTURE THIS FRAME. The raw pointer delta is for the buttonless gesture alone: a touchpad layout
/// moves the view with no button held, so egui reports no drag and there is nothing else to ask. Reaching for it
/// whenever `drag_delta` came back zero was wrong: dragging an open window by its title bar moved the pointer, and the
/// camera turned along with the window.
pub fn nav_delta(ctx: &egui::Context, resp: &egui::Response, g: &Gesture) -> egui::Vec2 {
    if g.buttons.is_empty() && !g.any_button {
        ctx.input(|i| i.pointer.delta())
    } else {
        resp.drag_delta()
    }
}

/// THE GESTURE OF THE LAYOUT MOVING THE VIEW SIDEWAYS THIS FRAME, if one is made: one of its own, or the left button
/// held still and then led under Gesture (its program pans so after a long press) - and none while the middle button's
/// zoom is latched under CAD.
pub fn pan_now(nav: MouseNav, ctx: &egui::Context, resp: &egui::Response) -> Option<&'static Gesture> {
    if zoom_latched(ctx) {
        return None;
    }
    if nav == MouseNav::Gesture && hold_latched(ctx) && ctx.input(|i| i.pointer.primary_down()) && resp.dragged() {
        return Some(const { &Gesture::of(&[LEFT]) });
    }
    nav.pans().iter().find(|g| g.active(ctx, resp))
}

/// TURN THE VIEW by the layout's gestures this frame: a tilt about the line of sight first, where the layout has one,
/// then a turn - about the middle of the view, or about the world point `about` when one is given.
///
/// ABOUT A POINT THE CENTRE OF THE VIEW TURNS WITH THE CAMERA. The camera turns about its centre as ever, and then the
/// centre is carried by the same turn about the point: the point keeps its place in the camera's own frame, so it
/// stays on its spot of the screen at its depth, in perspective as well, and turning back returns the view to where it
/// was. Sliding the centre in the plane of the screen until the point came back to its spot - the first way this was
/// tried - kept the spot but not the depth, and the centre wandered off a little with every frame: after a few turns
/// at a corner of a long block the model swung round a point hundreds of millimetres away.
pub fn turn_view(about: Option<[f64; 3]>, cam: &mut Cam3, nav: MouseNav, ctx: &egui::Context, resp: &egui::Response) {
    let (before, target) = (cam.basis(), cam.target);
    if let Some(tilt) = nav.tilts().iter().find(|g| g.active(ctx, resp)) {
        cam.roll += nav_delta(ctx, resp, tilt).x as f64 * 0.01;
    } else if let Some(turn) = nav.rotates().iter().find(|g| g.active(ctx, resp)) {
        let d = nav_delta(ctx, resp, turn);
        cam.yaw -= d.x as f64 * 0.01;
        cam.pitch = (cam.pitch + d.y as f64 * 0.01).clamp(-1.5, 1.5);
    }
    if let Some(p) = about {
        cam.target = carried_about(p, target, before, cam.basis());
    }
}

/// THE CENTRE `target` CARRIED BY A TURN OF THE CAMERA ABOUT `p`: the point keeps its coordinates in the camera's frame
/// as the frame goes from `before` to `after`.
pub fn carried_about(p: [f64; 3], target: [f64; 3], before: ([f64; 3], [f64; 3], [f64; 3]), after: ([f64; 3], [f64; 3], [f64; 3])) -> [f64; 3] {
    let rel = v_sub(p, target);
    let (c0, c1, c2) = (v_dot(rel, before.0), v_dot(rel, before.1), v_dot(rel, before.2));
    std::array::from_fn(|a| p[a] - (c0 * after.0[a] + c1 * after.1[a] + c2 * after.2[a]))
}

/// Where the long press of the Gesture layout is kept in the frame's memory: when and where the left button went down,
/// and whether the press has become a hold.
fn hold_latch_id() -> egui::Id {
    egui::Id::new("nav_hold_latch")
}

/// THE LONG PRESS OF THE GESTURE LAYOUT: the left button held still (6 px) for 0.63 s - its program's hold, 0.9 of the
/// system's 0.7 s - and led from then on moves the view instead of turning it, until the button goes up. Called once a
/// frame; answers whether the press is a hold.
pub fn latch_hold(ctx: &egui::Context, nav: MouseNav) -> bool {
    if nav != MouseNav::Gesture {
        return false;
    }
    let id = hold_latch_id();
    let (mut since, mut on): (Option<(f64, egui::Pos2)>, bool) = ctx.data(|d| d.get_temp(id)).unwrap_or((None, false));
    let (down, now, at, pressed) = ctx.input(|i| {
        let pressed = i.events.iter().find_map(|e| match e {
            egui::Event::PointerButton { button: egui::PointerButton::Primary, pressed: true, pos, .. } => Some(*pos),
            _ => None,
        });
        (i.pointer.primary_down(), i.time, i.pointer.latest_pos(), pressed)
    });
    if let Some(p) = pressed {
        since = Some((now, p));
        on = false;
    }
    if !down {
        since = None;
        on = false;
    } else if let (Some((t, p)), false) = (since, on) {
        if at.is_some_and(|a| a.distance(p) > 6.0) {
            since = None; // led before it was held: a turn, as ever
        } else if now - t >= 0.63 {
            on = true;
        }
    }
    ctx.data_mut(|d| d.insert_temp(id, (since, on)));
    on
}

/// Has the left button of the Gesture layout been held long enough to move the view?
pub fn hold_latched(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<(Option<(f64, egui::Pos2)>, bool)>(hold_latch_id())).is_some_and(|(_, on)| on)
}

/// HOW MUCH THE VIEW IS ASKED TO SCALE THIS FRAME, in the wheel's units: the wheel where the layout zooms with it, and
/// the layout's own zoom gesture where it has one - a movement up counts as the wheel turned forward. The touchpad has
/// no wheel and puts the zoom on Ctrl + Shift and a movement; nothing asked for that gesture, and the scale stood at
/// 323.44 before and after. One wheel unit a pixel: 120 px of movement scale the view 1.27 times. The zoom holds the
/// cursor still, and the cursor travels with this gesture: at two units a pixel a framed block's near corner went off
/// the bottom of the canvas (616 -> 780 of 770) on the same 120 px.
/// On the sheet of a sketch (`sheet`) a gesture holding the left button zooms only as a chord of two buttons: the left
/// alone, with Ctrl or Shift, is the sketch's own - it draws, grabs and adds to the selection.
pub fn view_scroll(ctx: &egui::Context, resp: &egui::Response, nav: MouseNav, sheet: bool) -> f32 {
    let wheel = if nav.wheel_zooms() { ctx.input(|i| i.smooth_scroll_delta.y) } else { 0.0 };
    // over the canvas rather than hovered: the click of the latch takes the hover away while the middle button is held
    let _ = latch_hold(ctx, nav); // the long press, looked at once a frame as the latch of the zoom is
    let latched = latch_zoom(ctx, nav) && resp.contains_pointer();
    let usable = |g: &&Gesture| !sheet || g.sheet_may_take();
    let moved = ctx.input(|i| i.pointer.delta());
    let by = -moved.y + if nav.zooms_sideways() { moved.x } else { 0.0 };
    let gesture = if latched || nav.zooms().iter().filter(usable).any(|g| g.active(ctx, resp)) { by } else { 0.0 };
    wheel + gesture
}

/// Where the zoom latched by the middle button is kept in the frame's memory: the time a left or right press began
/// while the middle button was held, and whether the latch is on.
fn zoom_latch_id() -> egui::Id {
    egui::Id::new("nav_zoom_latch")
}

/// THE ZOOM LATCHED BY THE MIDDLE BUTTON, as the CAD layout's program has it: the middle button held (it moves the
/// view), a short click of the left or right button - quicker than a double click - and from then on, until the middle
/// button is let go, moving zooms. Held longer, the left or right button with the middle one turns the model instead.
/// Answers whether the latch is on this frame; called once a frame.
pub fn latch_zoom(ctx: &egui::Context, nav: MouseNav) -> bool {
    if nav != MouseNav::Cad {
        return false;
    }
    let id = zoom_latch_id();
    let (mut pressed_at, mut on): (Option<f64>, bool) = ctx.data(|d| d.get_temp(id)).unwrap_or((None, false));
    let (middle, now, events) = ctx.input(|i| (i.pointer.middle_down(), i.time, i.events.clone()));
    if !middle {
        pressed_at = None;
        on = false;
    } else {
        for e in events {
            if let egui::Event::PointerButton { button: egui::PointerButton::Primary | egui::PointerButton::Secondary, pressed, .. } = e {
                match (pressed, pressed_at) {
                    (true, _) => pressed_at = Some(now),
                    (false, Some(t)) if now - t < 0.5 => {
                        on = true;
                        pressed_at = None;
                    }
                    _ => pressed_at = None,
                }
            }
        }
    }
    ctx.data_mut(|d| d.insert_temp(id, (pressed_at, on)));
    on
}

/// Is the zoom latched by the middle button on now - so that the middle button, which moves the view, zooms instead?
pub fn zoom_latched(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<(Option<f64>, bool)>(zoom_latch_id())).is_some_and(|(_, on)| on)
}

/// THE WHEEL OVER THE 3D VIEWPORT: zoom towards whatever the rule says to hold still.
///
/// `part` is where the open command's fields stand, when one is open - the workbench knows that and this
/// module does not, so it is handed in.
pub fn wheel_zoom_3d(cam: &mut Cam3, set: &Settings, rect: Rect, cursor: Option<Pos2>, part: Option<Pos2>, scroll: f32) {
    let (lo, hi) = zoom_limits(cam.fit);
    zoom_cam_3d(cam, rect, zoom_anchor(set, rect, cursor, part), (scroll * 0.002).exp(), lo, hi);
}

/// HOW FAR THE WHEEL GOES: two hundred times either way from where the view was last framed, and never tighter than
/// 0.05 - 400 px/mm. Held at 0.05 - 400 it kept a 38 m model (framed at 0.0087 px/mm) and a 0.1 mm one out of
/// reach: the first notch threw the one out of the frame, and the other stopped at 48 px across.
pub fn zoom_limits(fit: f32) -> (f32, f32) {
    ((fit / 200.0).min(0.05), (fit * 200.0).max(400.0))
}

/// The same over the flat sheet of a sketch.
pub fn wheel_zoom_2d(view: &mut View2d, set: &Settings, rect: Rect, cursor: Option<Pos2>, scroll: f32) {
    // from the scale the sheet was fitted at, as the 3D view zooms. Measured with fixed 0.02 - 800 px/mm: a 40 m sketch
    // is framed at 0.017 px/mm, under that floor, and the wheel could take it out no further than 0.02
    let (lo, hi) = zoom_limits(view.fit);
    zoom_view_2d(view, rect, zoom_anchor(set, rect, cursor, None), (scroll * 0.002).exp(), lo, hi);
}

/// PANNING THE SHEET: with the layout's own gestures of moving the view - the middle button of CAD, the right one of
/// Gesture and OpenSCAD, Shift and the middle one of Blender, the left and right together of Blender and Revit, Shift
/// and a movement on a touchpad - and, in ours, with the middle button as well. A flat sheet has nothing to turn, so
/// the gestures that turn the model do nothing here. The left button alone is the sketch's own (`sheet_may_take`). Reported behaviour: those layouts moved the space and not the sheet; and every layout panned on the
/// middle button, which in Blender turns the model and pans only with Shift.
pub fn pan_sheet_2d(view: &mut View2d, ctx: &egui::Context, resp: &egui::Response, nav: MouseNav) {
    if zoom_latched(ctx) {
        return; // the middle button zooms while the latch is on
    }
    // THE LEFT BUTTON ALONE IS THE SKETCH'S OWN, whatever the gesture names: a gesture of any button with Shift (ours)
    // names no left button of its own, and passed `sheet_may_take`, so a left drag with Shift drew the selection box and
    // moved the sheet under it at once. Held with the right or the middle one it is a chord, and moves the sheet.
    let left_alone = ctx.input(|i| i.pointer.primary_down() && !i.pointer.secondary_down() && !i.pointer.middle_down());
    let by_layout = !left_alone && nav.pans().iter().any(|g| g.sheet_may_take() && g.active(ctx, resp));
    let ours = nav == MouseNav::QymCad && ctx.input(|i| i.pointer.middle_down());
    if !ours && !by_layout {
        return;
    }
    let d = ctx.input(|i| i.pointer.delta());
    view.center.x -= d.x / view.scale;
    view.center.y += d.y / view.scale;
}

/// WHAT A SKETCH'S OWN AXIS IS CALLED IN THE WORLD.
///
/// Reported behaviour: "in Part -> Revolve there are two axes, X and Y, but there is no Z axis to revolve
/// the sketch about."
///
/// A flat profile can only be turned about an axis lying IN its plane, and a plane has two of them - the
/// sketch's own X and Y. Its local Z is the normal, and a flat profile turned about its own normal sweeps
/// nothing. So there is no third axis to add. What was wrong is the NAME: the two buttons wore the world's
/// letters over the sketch's local axes, and on the front plane (`BasePlane::XZ`, whose y is [0,0,1]) the
/// button labelled Y revolved about the world Z. The axis a person was looking for was there, under someone
/// else's letter.
///
/// Named by the world direction the axis points along whenever it points along one; a sketch on an
/// arbitrary face points along nothing in particular and keeps its own letter.
pub fn sketch_axis_name(project: &qymcad_core::model::Project, si: Option<usize>, axis: u8) -> &'static str {
    let own = if axis == 0 { "X" } else { "Y" };
    let Some(f) = si.and_then(|si| project.sketch_frame(si)) else { return own };
    let d = if axis == 0 { f.x } else { f.y };
    let n = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    if n < 1e-9 {
        return own;
    }
    // Along a world axis to within a millionth: anything else is a direction of its own and gets no letter.
    ["X", "Y", "Z"].into_iter().enumerate().find(|(i, _)| d[*i].abs() / n > 1.0 - 1e-6).map(|(_, w)| w).unwrap_or(own)
}

/// The depth a profile of one's own starts at: the one the core would cut with nothing typed.
fn custom_depth_to_start_from(cmd: &FeatCommand, thread: ThreadParams) -> f64 {
    qymcad_core::thread::ThreadSpec {
        standard: qymcad_core::thread::ThreadStandard::Custom,
        nominal_d: if cmd_val(cmd, "nominal") > 0.0 { cmd_val(cmd, "nominal") } else { thread.radius * 2.0 },
        pitch: cmd_val(cmd, "pitch"),
        ..Default::default()
    }
    .geometry()
    .depth
}

/// The candidate planes for a sketch: the world XY, XZ and YZ plus the datum planes, with their frames.
pub fn sketch_plane_candidates(pn: &Painting) -> Vec<(qymcad_core::feature::SketchPlane, qymcad_core::feature::PlaneFrame)> {
    use qymcad_core::feature::{BasePlane, PlaneFrame, SketchPlane};
    let mut v = vec![
        (SketchPlane::World(BasePlane::XY), BasePlane::XY.frame()),
        (SketchPlane::World(BasePlane::XZ), BasePlane::XZ.frame()),
        (SketchPlane::World(BasePlane::YZ), BasePlane::YZ.frame()),
    ];
    for p in &pn.project.planes {
        // only the datums visible in the current context (not hidden by a tick box, not belonging to
        // another component) - otherwise an invisible datum of a part could be picked as the plane of a
        // sketch from inside an assembly. The same holds for picking a mirror or a section plane: what
        // gets drawn are the datums of THE assembly currently open, with no exceptions for direct
        // children - only the datums OF the current context (plus those of its ancestors while working in
        // context, as usual). The frame is carried by the transform of the owner: a datum travels with
        // its part.
        if let Some(wt) = datum_render_transform(pn, p.id) {
            let fr = PlaneFrame::from_origin_normal(p.origin, p.normal, p.rot_deg);
            let fr = if qymcad_core::feature::is_identity12(&wt) { fr } else { fr.transformed(&wt) };
            v.push((SketchPlane::Datum(p.id), fr));
        }
    }
    v
}

/// The key of an action WITH REBINDING TAKEN INTO ACCOUNT: the one set by a person if there is
/// one, otherwise the factory key.
/// A BODY'S EDGES, ready for picking: the polylines and the persistent id of each.
///
/// Two parallel lists that are only ever used together, and were a bare pair inside an `Rc`.
#[derive(Default)]
pub struct EdgePolys {
    /// Each edge as a polyline in world space.
    pub polys: Vec<Vec<[f32; 3]>>,
    /// The persistent id of each, parallel to `polys`.
    pub ids: Vec<u32>,
    /// Whether each is smooth - a seam with the same face on both sides, or the tangent edge of a fillet - parallel
    /// to `polys`. Its ends are no corners a person sees.
    pub smooth: Vec<bool>,
}

/// A VALUE KEPT UNTIL THE THING IT WAS COMPUTED FROM CHANGES.
///
/// Nine cache fields were `(u64, T)`, and every one of them carried a doc comment explaining that the first
/// place is the revision - `(the revision, the bodies)`, `(the geometry revision, a body mapped to ...)`.
/// A comment saying what a type should say is a comment nothing checks. Two named fields say it once.
#[derive(Clone, Copy)]
pub struct Cached<T> {
    /// The revision the value was computed at. When it differs from the current one the value is stale.
    pub rev: u64,
    /// What was computed.
    pub value: T,
}

impl<T: Default> Default for Cached<T> {
    fn default() -> Self {
        Cached { rev: 0, value: T::default() }
    }
}

impl<T> Cached<T> {
    /// The value if it was computed at THIS revision; `None` means it has to be recomputed.
    pub fn get(&self, rev: u64) -> Option<&T> {
        (self.rev == rev).then_some(&self.value)
    }
    /// Keep a freshly computed value under the revision it belongs to.
    pub fn put(&mut self, rev: u64, value: T) {
        self.rev = rev;
        self.value = value;
    }
}

/// THE DOCUMENT UNDER THE HAND: the five things every editing gesture reads and writes.
///
/// The document itself; what has gone out of date in it; what is selected; the line the user is told;
/// the flat view. Eight of the thirteen workbench contexts carry exactly these five and nothing said
/// they belonged together, so eight functions took them one by one - `place_input_popup` reached eleven
/// arguments that way.
pub struct Editing<'a> {
    pub project: &'a mut qymcad_core::model::Project,
    /// The undo journal: an edit of the sketch is one step of it, named after its tool.
    pub edits: &'a mut Edits,
    pub regen: &'a mut Rebuilding,
    pub sel: &'a mut Sel,
    pub status: &'a mut String,
    pub view: &'a mut View2d,
}

/// TURN THE SELECTED LINES, ARCS AND CIRCLES INTO CONSTRUCTION GEOMETRY, or back: one named step of undo, the words said.
/// The button and the X key both come here. False when nothing of the sketch being edited is selected - the button then
/// switches what is drawn next instead.
pub fn construction_selected(ed: Editing, sel_sk: &SketchSelection, sketch_ses: &SketchSession) -> bool {
    let Sel::Sketch(si) = *ed.sel else { return false };
    if edit_si(ed.project, sketch_ses) != Some(si) {
        return false;
    }
    let eids: Vec<Id> = sel_sk.items.iter().filter(|(k, _)| *k == 1).map(|(_, id)| *id).collect();
    if eids.is_empty() {
        return false;
    }
    begin_edit(ed.edits, ed.project, qymcad_i18n::tr("tool-construction"));
    let now = ed.project.toggle_construction(si, &eids);
    ed.project.solve_sketch(si);
    invalidate(ed.regen);
    *ed.status = qymcad_i18n::tr(if now { "in-made-construction" } else { "in-made-normal" });
    close_edit(ed.edits, ed.project);
    true
}

/// THE CONSTRUCTION TOGGLE, by the button of the bar or by its key X - one rule for both: what is selected is turned
/// into construction geometry or back, and with nothing selected the kind of what is drawn next is switched. The key
/// switched the drawing mode whatever was selected: a line selected stayed as it was, and the next line came out
/// construction.
pub fn construction_toggle(ed: Editing, sel_sk: &SketchSelection, sketch_ses: &SketchSession, drawing_construction: &mut bool) {
    if !construction_selected(ed, sel_sk, sketch_ses) {
        *drawing_construction = !*drawing_construction;
    }
}

impl Editing<'_> {
    /// A shorter borrow of the same five, so one gesture can hand them on to another.
    pub fn reborrow(&mut self) -> Editing<'_> {
        Editing { project: self.project, edits: self.edits, regen: self.regen, sel: self.sel, status: self.status, view: self.view }
    }
}

/// The five, taken from the application, where they live in four different records.
/// THE PIECES THE POPUPS OF A TEXT WORK ON, gathered from the application - see `TextCtx`.
#[macro_export]
macro_rules! text_ctx_of {
    ($x:expr) => {
        $crate::TextCtx {
            annot: &mut $x.tools.annot,
            inline: &mut $x.tools.inline,
            armed: &mut $x.tools.armed,
            tool: &mut $x.tools.tool,
            font: &mut $x.tool_prefs.font,
            text: &mut $x.tool_prefs.text,
            tool_text_height: $x.tool_prefs.text_h,
            writes_note: $x.tool_prefs.text_note,
        }
    };
}

/// THE FIVE PIECES OF AN EDIT, gathered from the application.
#[macro_export]
macro_rules! editing_of {
    ($x:expr) => {
        $crate::Editing { project: &mut $x.project, edits: &mut $x.disk.edits, regen: &mut $x.regen, sel: &mut $x.chosen.sel, status: &mut $x.status, view: &mut $x.viewing.view }
    };
}

/// The same, from a CONTEXT, where the fields are already borrows and are reborrowed.
#[macro_export]
macro_rules! editing_in {
    ($x:expr) => {
        $crate::Editing { project: &mut *$x.project, edits: &mut *$x.edits, regen: &mut *$x.regen, sel: &mut *$x.sel, status: &mut *$x.status, view: &mut *$x.view }
    };
}

/// WHAT IS IN HAND. Exactly one thing, because it is exactly one field.
///
/// Eight tools used to say "I am the one in hand" through eight separate fields in five records - a zero in
/// `tool.kind`, a zero in `dim.kind`, a `false` in `measure.on`, and so on. That two of them could be
/// non-zero at once was a rule held by checks, and the rule was broken three times: an anchor re-pick no
/// door released, a ruler that armed and disarmed itself in one frame, and an array re-opened by double
/// click over whatever was already in hand. Each time the click went to whichever handler stood higher in
/// the code, and the person was certain they were working with the last tool taken.
///
/// One field cannot hold two values. The payload of the tool - which shape is being drawn, which points
/// have been picked - stays in its own record; what moved here is only the answer to "which tool".
///
/// FIVE OF THE THIRTEEN STAYED OUTSIDE, and each for a named reason. Four are sub-modes rather than
/// tools: `DrawTool::has_door` says so on the type, and the corner fillet is reached WITH the click
/// tool in hand, not instead of it. The fifth is `Placing`, whose "in hand" would have had to travel
/// into `PlaceCtx` and push `place_input_popup` past seven arguments - one debt traded for another.
#[derive(Clone, Copy, Default, PartialEq, Debug)]
pub enum Armed {
    /// Nothing: clicks go to selection.
    #[default]
    None,
    /// A sketch drawing tool, by its own code: line, circle, arc, rectangle.
    Draw(u8),
    /// Trim, extend, break, corner fillet - the ones that act on a click.
    ClickOp(u8),
    /// Mirror, offset and the rest of the editing buttons.
    Modify(EditTool),
    /// Move, copy, rotate.
    Move(u8),
    /// The sketch pattern: 1 linear, 2 circular.
    Pattern(u8),
    /// The dimension tool, by kind.
    Dimension(u8),
    /// A feature command of the Part (extrude, hole, fillet...), by kind.
    Command(u8),
    /// The measuring tool.
    Measure,
}

impl Armed {
    /// The code of the drawing tool in hand, or zero. Named as the field it replaces, so that a reading
    /// site says the same thing it said before.
    pub fn draw_kind(&self) -> u8 {
        if let Armed::Draw(k) = self {
            *k
        } else {
            0
        }
    }

    /// The code of the click-acting tool, or zero.
    pub fn click_op(&self) -> u8 {
        if let Armed::ClickOp(k) = self {
            *k
        } else {
            0
        }
    }

    /// The editing tool in hand, if one is.
    pub fn modify(&self) -> Option<EditTool> {
        if let Armed::Modify(k) = self {
            Some(*k)
        } else {
            None
        }
    }

    /// The code of the move tool, or zero.
    pub fn move_op(&self) -> u8 {
        if let Armed::Move(k) = self {
            *k
        } else {
            0
        }
    }

    /// The code of the sketch pattern, or zero.
    pub fn pat_op(&self) -> u8 {
        if let Armed::Pattern(k) = self {
            *k
        } else {
            0
        }
    }

    /// The kind of dimension being placed, or zero.
    pub fn dim_kind(&self) -> u8 {
        if let Armed::Dimension(k) = self {
            *k
        } else {
            0
        }
    }

    /// The kind of feature command being run, or zero.
    pub fn cmd_kind(&self) -> u8 {
        if let Armed::Command(k) = self {
            *k
        } else {
            0
        }
    }

    /// Whether a feature command is open.
    pub fn commanding(&self) -> bool {
        matches!(self, Armed::Command(_))
    }

    /// Whether the measuring tool is in hand.
    pub fn measuring(&self) -> bool {
        matches!(self, Armed::Measure)
    }

    /// Whether anything at all is in hand.
    pub fn any(&self) -> bool {
        !matches!(self, Armed::None)
    }

    /// WHICH TOOL, as the checks name them. `None` means the hand is empty.
    ///
    /// At most one, and not by a rule: there is one field, and a field holds one value.
    pub fn tool(&self) -> Option<DrawTool> {
        match self {
            Armed::None => None,
            Armed::Draw(_) => Some(DrawTool::Draw),
            Armed::ClickOp(_) => Some(DrawTool::ClickOp),
            Armed::Modify(_) => Some(DrawTool::Modify),
            Armed::Move(_) => Some(DrawTool::Move),
            Armed::Pattern(_) => Some(DrawTool::Pattern),
            Armed::Dimension(_) => Some(DrawTool::Dimension),
            Armed::Command(_) => Some(DrawTool::Command),
            Armed::Measure => Some(DrawTool::Measure),
        }
    }
}

/// WHICH BODIES ARE ON SCREEN, and in which context they were counted.
///
/// The context is part of the key, not of the answer: stepping into a component changes what is shown
/// without changing the geometry, so the revision alone would hand back the wrong list.
#[derive(Default)]
pub struct ShownBodies {
    /// The assembly whose contents were walked.
    pub ctx: Id,
    /// The mesh index and body id of everything visible.
    pub list: Vec<(usize, Id)>,
}

/// THE PARAMETERS OF THE FEATURE BEING BUILT, whichever it is.
///
/// Fourteen records - the numbers a revolve, a chamfer, a sweep, a loft, a draft, a hole, a primitive, a
/// mirror, a split, an array, a boolean and a thread are waiting for. At most one feature is being built
/// at a time, so at most one of these is meaningful; the rest hold whatever was typed into them last.
///
/// They lived as fourteen fields of `App`, and a reader had to know which of the hundred were the
/// command's and which the document's. `FeatParams` is that answer written down.
/// THE RECOGNITION TOOL IN HAND: the mesh it is aimed at, and what a count of that mesh found - made in the
/// background, for the mesh, the tolerance and the sharp angle it was asked for, so a person sees what Enter will
/// build before building it.
#[derive(Default)]
pub struct RecogniseTool {
    pub src: Option<Id>,
    /// the mesh, the tolerance and the sharp angle (as bits) last sent to be counted
    pub asked: Option<(Id, u64, u64)>,
    pub found: std::sync::Arc<std::sync::Mutex<Option<RecogniseFound>>>,
}

/// What a count of a mesh found: the kind of surface under each triangle (0 plane, 1 cylinder, 2 cone, 3 sphere,
/// 4 torus, 5 none, 6 a free form, 7 a helix) and how many regions of each kind.
pub struct RecogniseFound {
    pub key: (Id, u64, u64),
    pub kinds: Vec<u8>,
    pub counts: [usize; 8],
}

impl RecogniseTool {
    /// What was found for the mesh in hand as it was last asked, once the count is done.
    pub fn ready<T>(&self, read: impl FnOnce(&RecogniseFound) -> T) -> Option<T> {
        let key = self.asked?;
        let found = self.found.lock().ok()?;
        found.as_ref().filter(|f| f.key == key && Some(key.0) == self.src).map(read)
    }
}

pub struct FeatParams {
    pub stitch_parts: Vec<Id>,
    /// the recognition tool in hand: its mesh and what a count of it found
    pub recognise: RecogniseTool,
    pub repl_surface: Option<Id>,
    pub rev: RevolveParams,
    pub chamfer: ChamferParams,
    pub sweep: SweepParams,
    pub loft: LoftParams,
    pub draft: DraftParams,
    pub hole: HoleCommand,
    pub prim: PrimParams,
    pub mirror: MirrorParams,
    pub split: SplitParams,
    pub arr: ArrayParams,
    pub boolean: BoolCommand,
    pub thread: ThreadParams,
}

impl Default for FeatParams {
    /// THE VALUES A COMMAND OPENS WITH, and four of them are not zero.
    ///
    /// They used to sit in `App::default()`, one line per field among a hundred. Moving the fields here and
    /// leaving the values behind cost a run: the array opened with a count of zero, and the ghosts of the
    /// pattern stopped appearing - `#[derive(Default)]` compiled perfectly and meant something else.
    fn default() -> Self {
        FeatParams {
            stitch_parts: Vec::new(),
            recognise: RecogniseTool::default(),
            repl_surface: None,
            // A full turn, so a revolve without touching anything makes a solid of revolution.
            rev: RevolveParams { angle: 360.0, ..Default::default() },
            chamfer: ChamferParams::default(),
            sweep: SweepParams::default(),
            loft: LoftParams::default(),
            draft: DraftParams::default(),
            hole: HoleCommand::default(),
            // A hexagon: the polygon everyone draws first.
            prim: PrimParams { n: 6, ..Default::default() },
            mirror: MirrorParams::default(),
            split: SplitParams::default(),
            // Three copies along the first direction, two along each of the others.
            arr: ArrayParams { count: 3, count2: 2, count3: 2, dir2: 1, dir3: 2, full: true, ..Default::default() },
            boolean: BoolCommand::default(),
            // A single-start thread.
            thread: ThreadParams { starts: 1, ..Default::default() },
        }
    }
}

/// THE VIEW TURNING TO A NEW ANGLE, while it turns.
///
/// It was `Option<((f64, f64), (f64, f64), Instant)>` - two nameless pairs and a moment. Which pair is
/// where the view came FROM and which where it is going TO could only be told from the body of the tick.
#[derive(Clone, Copy)]
pub struct ViewTurn {
    /// Yaw and pitch it started at.
    pub from: (f64, f64),
    /// The tilt it starts at and goes to: a standard view has none, a look at a point keeps what there is.
    pub roll: (f64, f64),
    /// The centre it moves from and to, when it moves one - a look at a point; the view keeps its scale then, and is
    /// not framed again on arrival.
    pub target: Option<([f64; 3], [f64; 3])>,
    /// Yaw and pitch it is going to.
    pub to: (f64, f64),
    /// When it started; the whole turn lasts a fixed time from here.
    pub since: std::time::Instant,
}

/// THE TOOLS THAT ARE NOT DRAWING TOOLS: what the Part and the Assembly hold while one of them is in hand.
///
/// `DrawState` holds the fourteen a sketch draws with. These ten are the rest - trimming, datums, mates,
/// measuring in 3D, the section, the component array, the clipboard, the sketch array, a rotation being
/// typed and a name being typed. At most one is live, same as there; naming the set is what makes that a
/// thing the code can be asked about rather than a rule people keep.
pub struct SideTools {
    /// Trimming a sketch entity by a click.
    pub trim: TrimTool,
    /// A datum being placed: a plane, an axis or a point.
    pub datum: DatumCommand,
    /// The mate being assembled, and every pick of the Assembly.
    pub joint: JointCommand,
    /// The 3D measuring tool.
    pub m3: Measure3,
    /// The section plane and its handle.
    pub section: SectionTool,
    /// A pattern of components being built.
    pub carr: CompArrayCmd,
    /// What was copied, geometry or a component.
    pub clip: Clipboard,
    /// The sketch array's numbers.
    pub array: ArrayTool,
    /// A rotation being typed into.
    pub rot: RotInput,
    /// A name being typed into (a rename in the tree).
    pub rename: RenameInput,
}

impl Default for SideTools {
    /// THE SKETCH ARRAY OPENS WITH THREE COPIES 20 mm APART, and that is written here rather than derived:
    /// zeroed, the tool would offer an array of nothing at no distance, which is not a starting point.
    fn default() -> Self {
        SideTools {
            trim: TrimTool::default(),
            datum: DatumCommand::default(),
            joint: JointCommand::default(),
            m3: Measure3::default(),
            section: SectionTool::default(),
            carr: CompArrayCmd::default(),
            clip: Clipboard::default(),
            array: ArrayTool { n: 3, dx: 20.0, dy: 0.0 },
            rot: RotInput::default(),
            rename: RenameInput::default(),
        }
    }
}

/// WHAT IS CHOSEN AND WHAT IS UNDER THE CURSOR.
///
/// Four fields of `App` that answer one question, and two of them are easy to confuse: `sel` is what the
/// document considers selected, `tree_sel` is what is highlighted in the tree, and the two are not the
/// same thing - a multiple selection in the tree does not make four bodies "the selection".
pub struct Chosen {
    /// THE SELECTION of the document: one body, face, sketch, plane, component or feature.
    pub sel: Sel,
    /// What is picked in the TREE - several rows may be, and that is not the same as `sel`.
    pub tree_sel: TreeSelection,
    /// THE ANCHOR chosen in the connector list: its handles are the ones being edited.
    pub sel_conn: Option<Id>,
    /// What is under the cursor right now.
    pub hover: Hover,
}

impl Default for Chosen {
    /// `Sel` has no derived default on purpose: "nothing is selected" is a named variant, not a zero.
    fn default() -> Self {
        Chosen { sel: Sel::None, tree_sel: TreeSelection::default(), sel_conn: None, hover: Hover::default() }
    }
}

/// WHAT IS BEING DRAGGED IN THE SCENE, and by which handle.
///
/// The gizmos and the two pulls are one subject: at most one of them is live, and each holds what it needs
/// only while the mouse is down.
#[derive(Default)]
pub struct Dragged {
    /// The component gizmo (moving a part in an assembly).
    pub comp_giz: CompGizmo,
    /// The body gizmo (moving a body inside a part).
    pub body_giz: BodyGizmo,
    /// Dragging the rollback bar along the timeline.
    pub rollback: RollbackDrag,
    /// The arrow on a face being dragged: how far it has travelled.
    pub face_arrow_drag: Option<f64>,
    /// Pulling a part by a degree of freedom: which one, from where, in which direction.
    pub part_pull: Option<(Id, [f64; 3], [f64; 3])>,
}

/// HOW THE SCENE IS BEING LOOKED AT: the camera, the flat view, and everything a turn or a drag of it
/// needs while it lasts.
///
/// Eight fields of `App` that answer one question. `mode_3d` in particular read as a loose flag among a
/// hundred others; it belongs here, beside the two views it chooses between.
pub struct Viewing {
    /// The orbit camera of the 3D view.
    pub cam: Cam3,
    /// The pan and zoom of the flat view.
    pub view: View2d,
    /// WHICH OF THE TWO is on screen.
    pub mode_3d: bool,
    /// The canvas rectangle of the previous frame - the window in the rebuild barrier.
    pub view_rect: egui::Rect,
    /// A turn to a new angle, while it lasts (the ViewCube).
    pub view_anim: Option<ViewTurn>,
    /// What to come back to after a temporary look from somewhere else: the mode, the camera, the flat view.
    pub view_restore: Option<(bool, Cam3, View2d)>,
    /// The camera is being dragged right now: the raster cache is not worth rebuilding at every step.
    pub view_dragging: bool,
    /// Where the view stood before stepping into a component, one entry per step.
    pub nav_stash: Vec<(Cam3, View2d, bool)>,
}

impl Viewing {
    /// End a running turn of the view at once, putting the camera where it was heading.
    pub fn finish_view_anim(&mut self) {
        if let Some(ViewTurn { to, roll, target, .. }) = self.view_anim.take() {
            self.cam.yaw = to.0;
            self.cam.pitch = to.1;
            self.cam.roll = roll.1;
            if let Some((_, t)) = target {
                self.cam.target = t;
            }
        }
    }
}

impl Default for Viewing {
    /// `Rect::NOTHING` rather than a zero rectangle: an empty rect at the origin would read as a canvas
    /// one pixel wide at the corner, and the barrier that uses it would let a frame through.
    fn default() -> Self {
        Viewing { cam: Cam3::default(), view: View2d::default(), mode_3d: false, view_rect: egui::Rect::NOTHING, view_anim: None, view_restore: None, view_dragging: false, nav_stash: Vec::new() }
    }
}

/// THE DRAWING TOOLS AS THE APPLICATION KEEPS THEM: fourteen records that only ever move together.
///
/// They used to be fourteen fields of `App` among a hundred, so "the tools in hand" was a thing one had to
/// know rather than read. `Tools` already named the SET for the functions that take it; this is the same
/// set OWNED, so the application carries one field where it carried fourteen.
///
/// Which of them is live at a time is still a rule rather than a type - see the debt catalogue, D11. The
/// naming is the step that makes the type possible later.
#[derive(Default)]
pub struct DrawState {
    /// WHAT IS IN HAND. One field, so two tools at once cannot be written down.
    pub armed: Armed,
    pub annot: AnnotEdit,
    pub cmd: FeatCommand,
    pub corner: CornerInput,
    pub dim: DimTool,
    pub drag: Dragging,
    pub gsel: GeomSelection,
    pub inline: InlineEdit,
    pub measure: Measuring,
    pub pat: PatternTool,
    pub pending_import: PendingImport,
    pub picking: Picking,
    pub place: Placing,
    pub sel_sk: SketchSelection,
    pub tool: SketchTool,
}

/// THE TOOLS IN HAND: every record a drawing tool keeps while it is being used.
///
/// FOURTEEN ARGUMENTS THAT ALWAYS TRAVELLED TOGETHER. Six functions took exactly this set and nothing
/// else in common - `exit_draw_tools` took the fourteen and no other argument at all, which is as plain a
/// statement as code makes that they are one thing. The widest signature ran to twenty-one places, and
/// telling `pat` from `place` there meant counting commas.
///
/// WHY A RECORD OF BORROWS rather than an owned one: they are fields of the application, and a tool must
/// change them where they live. This is the same shape as the workbench contexts, and it is built the same
/// way - once, at a doorway.
///
/// Only ONE of these is ever live at a time; that rule is held by a check rather than by the type. Naming
/// the set is the first step towards holding it by the type instead.
pub struct Tools<'a> {
    pub armed: &'a mut Armed,
    /// A note or a dimension being typed into.
    pub annot: &'a mut AnnotEdit,
    /// The feature command in progress (extrude, hole, fillet...).
    pub cmd: &'a mut FeatCommand,
    /// The corner tool: fillet or chamfer on a sketch vertex.
    pub corner: &'a mut CornerInput,
    /// The dimension tool.
    pub dim: &'a mut DimTool,
    /// What is being dragged right now.
    pub drag: &'a mut Dragging,
    /// The picked faces, edges and vertices.
    pub gsel: &'a mut GeomSelection,
    /// The value popup at the geometry.
    pub inline: &'a mut InlineEdit,
    /// The measuring tool.
    pub measure: &'a mut Measuring,
    /// The sketch pattern tool.
    pub pat: &'a mut PatternTool,
    /// A part being brought in from the library, waiting for its place.
    pub pending_import: &'a mut PendingImport,
    /// The click-pick mode: what is being pointed at and why.
    pub picking: &'a mut Picking,
    /// Placing a component.
    pub place: &'a mut Placing,
    /// The sketch selection.
    pub sel_sk: &'a mut SketchSelection,
    /// Which drawing tool is chosen.
    pub tool: &'a mut SketchTool,
}

/// ONE DRAWING TOOL THAT CAN BE IN HAND.
///
/// The rule "taking one tool releases the previous" was guarded for the ASSEMBLY tools only
/// (`AssemblyTool`), and the guard there found a real defect the moment it was made exhaustive. The
/// drawing tools - fourteen records in `Tools`, shared by the Sketch and the Part - had no such guard at
/// all. This names them so they can be walked.
///
/// Two tools at once means ambiguity under the cursor: the click goes to whichever handler stands higher
/// in the code, while the person is certain they are working with the one picked last.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DrawTool {
    /// A sketch drawing tool: line, circle, arc, rectangle.
    Draw,
    /// Trim, extend, break, corner fillet - the ones that act on a click.
    ClickOp,
    /// Mirror, offset and the rest of the editing buttons.
    Modify,
    /// Move, copy, rotate.
    Move,
    /// The sketch pattern: linear or circular.
    Pattern,
    /// The dimension tool.
    Dimension,
    /// A feature command of the Part (extrude, hole, fillet...).
    Command,
    /// The measuring tool.
    Measure,
    /// Pointing at something for a command: a plane for a sketch, every edge for a fillet, a contour.
    Pick,
    /// Placing a primitive shape: a rectangle, a polygon.
    Place,
    /// An import waiting for its place.
    Import,
    /// A fillet or chamfer on a sketch vertex.
    Corner,
    /// Typing into a note or a text.
    Annot,
}

impl DrawTool {
    /// EVERY KIND. Walked by the checks; a guard below holds it against the declaration.
    pub const ALL: [DrawTool; 13] = [
        DrawTool::Draw,
        DrawTool::ClickOp,
        DrawTool::Modify,
        DrawTool::Move,
        DrawTool::Pattern,
        DrawTool::Dimension,
        DrawTool::Command,
        DrawTool::Measure,
        DrawTool::Pick,
        DrawTool::Place,
        DrawTool::Import,
        DrawTool::Corner,
        DrawTool::Annot,
    ];

    /// IS IT TAKEN BY A DOOR OF ITS OWN - a button or a key a person presses.
    ///
    /// The rest are SUB-MODES: they are switched on from inside another tool and cannot be taken on their
    /// own, so "taking this one releases the previous" is not a sentence about them. Named here rather
    /// than left out of a list, so that the reason travels with the type.
    pub fn has_door(self) -> bool {
        !matches!(self, DrawTool::Pick | DrawTool::Import | DrawTool::Corner | DrawTool::Annot)
    }
}

/// WHAT IS IN HAND RIGHT NOW. The same question `armed_assembly_tools` answers for the Assembly.
pub fn armed_draw_tools(t: &Tools) -> Vec<DrawTool> {
    // THE TOOL IN HAND IS ONE FIELD, so this half can return at most one thing and no check has to say so.
    let mut out: Vec<DrawTool> = t.armed.tool().into_iter().collect();
    // THE REST ARE NOT TOOLS ONE TAKES. Four are sub-modes fallen into WITH a tool in hand
    // (`DrawTool::has_door` says which), and placing is the size of a shape being typed while it is
    // drawn. They keep their own records, and more than one of them can be true at once.
    if *t.picking != Picking::None {
        out.push(DrawTool::Pick);
    }
    if t.place.shape != PlacingShape::None {
        out.push(DrawTool::Place);
    }
    if t.pending_import.curves.is_some() || t.pending_import.draw_pts.is_some() {
        out.push(DrawTool::Import);
    }
    if t.corner.at.is_some() {
        out.push(DrawTool::Corner);
    }
    if t.annot.note.is_some() || t.annot.text.is_some() {
        out.push(DrawTool::Annot);
    }
    out
}

/// TAKE THE MEASURING TOOL, or put it down.
///
/// LIKE EVERY OTHER TOOL: the others are released FIRST, and only then is this one taken. The reverse
/// order is not a style question - it is what the ruler button did, and it meant the button did nothing at
/// all. It set `measure.on` and pushed `BarAsk::SketchSelectMode`; the frame performs a request AFTER the
/// panel has drawn, that request calls `exit_draw_tools`, and `exit_draw_tools` clears the measuring tool
/// along with the rest. On, then off, in the same frame.
///
/// Reported behaviour: none - nobody had reported it, because a button that does nothing looks like a
/// button one has misunderstood. It was found by sweeping every pair of drawing tools.
pub fn set_measure(t: &mut Tools, on: bool) {
    exit_draw_tools(&mut t.reborrow());
    *t.armed = if on { Armed::Measure } else { Armed::None };
    t.measure.pts.clear();
}

/// THE TOOLS IN HAND, from an OWNER - the application, where these fields are held by value.
///
/// A MACRO AND NOT A METHOD, and the reason is the borrow checker rather than taste. A method takes the
/// WHOLE receiver, so `set_dim_tool(&mut self.tools(), &mut self.mode_3d, ...)` borrows the application
/// twice and does not compile. The macro expands to a struct literal of FOURTEEN SEPARATE FIELD BORROWS,
/// which are disjoint, so the neighbouring arguments can still be taken from the same owner.
#[macro_export]
macro_rules! tools_of {
    ($x:expr) => {
        $crate::Tools {
            armed: &mut $x.tools.armed,
            annot: &mut $x.tools.annot,
            cmd: &mut $x.tools.cmd,
            corner: &mut $x.tools.corner,
            dim: &mut $x.tools.dim,
            drag: &mut $x.tools.drag,
            gsel: &mut $x.tools.gsel,
            inline: &mut $x.tools.inline,
            measure: &mut $x.tools.measure,
            pat: &mut $x.tools.pat,
            pending_import: &mut $x.tools.pending_import,
            picking: &mut $x.tools.picking,
            place: &mut $x.tools.place,
            sel_sk: &mut $x.tools.sel_sk,
            tool: &mut $x.tools.tool,
        }
    };
}

/// THE TOOLS THE SIZE POPUPS TAKE, out of the application that holds them - one spelling, so that every caller
/// gathers the same four and the list is not written out again where a panel is.
#[macro_export]
macro_rules! popup_tools {
    ($x:expr) => {
        $crate::PopupTools { corner: &mut $x.tools.corner, place: &mut $x.tools.place, sel_sk: &mut $x.tools.sel_sk, armed: &$x.tools.armed }
    };
}

/// THE VALUES AND THE SCHEME THE SIZE POPUPS TAKE, out of the application that holds them: what the tools
/// remember between two corners, and the colours everything over the sheet is read from.
#[macro_export]
macro_rules! popup_looks {
    ($x:expr) => {
        $crate::PopupLooks { tool_prefs: &mut $x.tool_prefs, scheme: &$x.scheme, set: &$x.set }
    };
}

/// The same, from a CONTEXT, where the fields are already borrows and are reborrowed.
#[macro_export]
macro_rules! tools_in {
    ($x:expr) => {
        $crate::Tools {
            armed: $x.armed,
            annot: $x.annot,
            cmd: $x.cmd,
            corner: $x.corner,
            dim: $x.dim,
            drag: $x.drag,
            gsel: $x.gsel,
            inline: $x.inline,
            measure: $x.measure,
            pat: $x.pat,
            pending_import: $x.pending_import,
            picking: $x.picking,
            place: $x.place,
            sel_sk: $x.sel_sk,
            tool: $x.tool,
        }
    };
}

/// WHICH BODIES ARE ON SCREEN: the seven things that together decide whether a body is drawn at all.
///
/// Visibility is not a flag on the body. It follows the chain of ticks up the assembly, the active
/// context, the sketch session and the feature being edited, and the caches hold the answer. Every
/// caller needs the same seven, so they travel as one record: `body_shown` alone is asked from
/// twenty-three places, each of which used to spell all seven out.
#[derive(Clone, Copy)]
pub struct BodyView<'a> {
    pub armed: &'a Armed,
    pub active_path: &'a [Id],
    pub cache: &'a Caches,
    pub cmd: &'a FeatCommand,
    pub project: &'a Project,
    pub regen: &'a Rebuilding,
    pub sketch_ses: SketchSession,
    pub win: &'a Windows,
}

/// The visibility seven, taken from the application, where they live in four different records.
#[macro_export]
macro_rules! body_view_of {
    ($x:expr) => {
        $crate::BodyView { armed: &$x.tools.armed, active_path: &$x.active_path, cache: &$x.cache, cmd: &$x.tools.cmd, project: &$x.project, regen: &$x.regen, sketch_ses: $x.sketch_ses, win: &$x.win }
    };
}

/// The same, from a CONTEXT, where the fields are already borrows.
#[macro_export]
macro_rules! body_view_in {
    ($x:expr) => {
        $crate::BodyView { armed: $x.armed, active_path: $x.active_path, cache: &*$x.cache, cmd: &*$x.cmd, project: &*$x.project, regen: &*$x.regen, sketch_ses: *$x.sketch_ses, win: &*$x.win }
    };
}

impl<'a> Painting<'a> {
    /// What the interface is in the middle of, out of a whole frame's drawing state.
    pub fn doing(&self) -> Doing<'a> {
        Doing { armed: self.armed, cmd: self.cmd, gsel: self.gsel, joint: self.joint, picking: self.picking, sketch_ses: self.sketch_ses, workbench: self.workbench }
    }
}

impl<'a> Painting<'a> {
    /// The visibility seven out of a whole frame's drawing state.
    pub fn body_view(&self) -> BodyView<'a> {
        BodyView { armed: self.armed, active_path: self.active_path, cache: self.cache, cmd: self.cmd, project: self.project, regen: self.regen, sketch_ses: self.sketch_ses, win: self.win }
    }
}

/// THE AIMING OF A FEATURE COMMAND: the ten records a command points at geometry with.
///
/// They are cleared together - a command starting or ending lets go of every pick at once, and there is
/// no case where one of them keeps a pick while the rest let go. Held as one record so that a new
/// pickable tool becomes a field here instead of an eleventh argument at four call sites.
pub struct FeatPicks<'a> {
    pub arr: &'a mut ArrayParams,
    pub datum: &'a mut DatumCommand,
    pub draft: &'a mut DraftParams,
    pub loft: &'a mut LoftParams,
    pub mirror: &'a mut MirrorParams,
    pub picking: &'a mut Picking,
    pub recognise: &'a mut RecogniseTool,
    pub stitch_parts: &'a mut Vec<Id>,
    pub sweep: &'a mut SweepParams,
    pub thread: &'a mut ThreadParams,
    pub trim: &'a mut TrimTool,
}

/// The aiming, taken from the application, where the ten live in three different records.
#[macro_export]
macro_rules! feat_picks_of {
    ($x:expr) => {
        $crate::FeatPicks {
            arr: &mut $x.params.arr,
            datum: &mut $x.side.datum,
            draft: &mut $x.params.draft,
            loft: &mut $x.params.loft,
            mirror: &mut $x.params.mirror,
            picking: &mut $x.tools.picking,
            recognise: &mut $x.params.recognise,
            stitch_parts: &mut $x.params.stitch_parts,
            sweep: &mut $x.params.sweep,
            thread: &mut $x.params.thread,
            trim: &mut $x.side.trim,
        }
    };
}

/// The same, from a CONTEXT, where the fields are already borrows and are reborrowed.
#[macro_export]
macro_rules! feat_picks_in {
    ($x:expr) => {
        $crate::FeatPicks {
            arr: $x.arr,
            datum: $x.datum,
            draft: $x.draft,
            loft: $x.loft,
            mirror: $x.mirror,
            picking: $x.picking,
            recognise: $x.recognise,
            stitch_parts: $x.stitch_parts,
            sweep: $x.sweep,
            thread: $x.thread,
            trim: $x.trim,
        }
    };
}

impl Tools<'_> {
    /// LEND THE TOOLS ONWARD. A record of borrows cannot be copied, and a caller that has already unpacked
    /// it still needs to hand the whole set to a neighbour; this borrows every field afresh.
    pub fn reborrow(&mut self) -> Tools<'_> {
        Tools {
            armed: self.armed,
            annot: self.annot,
            cmd: self.cmd,
            corner: self.corner,
            dim: self.dim,
            drag: self.drag,
            gsel: self.gsel,
            inline: self.inline,
            measure: self.measure,
            pat: self.pat,
            pending_import: self.pending_import,
            picking: self.picking,
            place: self.place,
            sel_sk: self.sel_sk,
            tool: self.tool,
        }
    }
}

/// WHILE A KEY IS BEING REASSIGNED: what is being waited for, and why the last press was refused.
///
/// Two fields on the application that only ever moved together and only ever meant something while one
/// window was open.
#[derive(Default)]
pub struct HotkeyCapture {
    /// The action a key is being assigned to right now - the window is waiting for a press.
    pub action: Option<String>,
    /// Why the last press was refused. Shown in that same window; empty means nothing was refused.
    pub note: String,
    /// A PRESS THAT HIT A TAKEN KEY, held until the person decides: swap the two, take the key away from
    /// the other action, or keep things as they were. Refusing outright sent people hunting for the
    /// other row to free it first - two edits for one intention.
    pub clash: Option<HotkeyClash>,
    /// The filter typed above the table: a word of the description or a key.
    pub filter: String,
}

/// A key asked for by one action while another already holds it in the same area.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HotkeyClash {
    /// The action being assigned.
    pub action: String,
    /// The chord pressed for it, in its stored spelling.
    pub chord: String,
    /// The action that holds that chord now.
    pub holder: &'static str,
}

/// A KEY WITH ITS MODIFIERS: `W`, `Shift+W`, `Ctrl+J`, `Ctrl+Shift+F5`, `Control+J`.
///
/// Ctrl stands for the platform's command key (Cmd on a Mac), as everywhere else in the program: the factory
/// layout and a profile mean the same keys on every system. Control is the Mac's own Control key, a modifier of
/// its own there; what each system refuses is in `platform_keys`. ALT IS NOT A MODIFIER OF A BINDING: it is the
/// way to reach a bare binding from inside a text field (Alt+U instead of U), and a chord that needed Alt would be
/// unreachable exactly there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Chord {
    /// The Mac's Control key (never set elsewhere: off a Mac the Ctrl key is `ctrl`).
    pub control: bool,
    pub ctrl: bool,
    pub shift: bool,
    pub key: egui::Key,
}

impl From<egui::Key> for Chord {
    fn from(key: egui::Key) -> Self {
        Chord { control: false, ctrl: false, shift: false, key }
    }
}

impl Chord {
    /// THE CHORD OF A PRESS, Alt aside. egui reports the command key as `command` everywhere and the physical
    /// Ctrl key as `ctrl`; off a Mac they are one key, on a Mac `command` is Cmd (`mac_cmd`) and `ctrl` is Control.
    /// So Control is `ctrl` that is not the command key, or `ctrl` held beside Cmd.
    pub fn of_press(mods: egui::Modifiers, key: egui::Key) -> Chord {
        Chord { control: mods.ctrl && (mods.mac_cmd || !mods.command), ctrl: mods.command, shift: mods.shift, key }
    }

    /// Reads the stored spelling. `None` for anything that is not a chord: an empty record (an unbound
    /// action), a description like `Ctrl+Z / Ctrl+Y`, a key egui does not know.
    pub fn parse(s: &str) -> Option<Chord> {
        let mut c = Chord::from(egui::Key::Escape);
        let mut parts = s.split('+').map(str::trim).peekable();
        while let Some(p) = parts.next() {
            if parts.peek().is_none() {
                c.key = egui::Key::from_name(p)?;
                return Some(c);
            }
            match p {
                "Control" => c.control = true,
                "Ctrl" => c.ctrl = true,
                "Shift" => c.shift = true,
                _ => return None,
            }
        }
        None
    }

    /// The stored and the shown spelling at once - one form, so a record reads the way the window shows it.
    pub fn name(&self) -> String {
        let mut s = String::new();
        if self.control {
            s.push_str("Control+");
        }
        if self.ctrl {
            s.push_str("Ctrl+");
        }
        if self.shift {
            s.push_str("Shift+");
        }
        s.push_str(self.key.name());
        s
    }

    /// WHETHER THE KEY ITSELF CAN CARRY A BINDING: letters, digits and F3-F12.
    ///
    /// Everything else already means something that must not move. Space opens the search, F1 is help, F2
    /// renames, the arrows and Home/End walk a caret through a field, Tab walks the focus, Enter, Esc and
    /// Delete are the ladder of every dialogue. Bound to a tool, any of them would quietly stop doing the
    /// thing every program does with it.
    pub fn bindable_key(&self) -> bool {
        use egui::Key as K;
        let n = self.key.name();
        let letter_or_digit = n.len() == 1 && n.chars().all(|c| c.is_ascii_alphanumeric());
        letter_or_digit || matches!(self.key, K::F3 | K::F4 | K::F5 | K::F6 | K::F7 | K::F8 | K::F9 | K::F10 | K::F11 | K::F12)
    }
}

// HOW A KEY IS WRITTEN ON SCREEN lives in the dictionary crate, which writes every caption: a key typed into a
// sentence ("Copy (Ctrl+C)") has to follow the system just like a key in the table.
pub use qymcad_i18n::keys::{key_label, key_label_in, key_style, set_key_style, KeyStyle};

/// WHY A CHORD CANNOT GO TO THIS ACTION on this system - a catalogue key, or `None` when it can.
pub fn hotkey_refusal(action: &str, chord: &Chord) -> Option<&'static str> {
    hotkey_refusal_on(platform_keys::Os::current(), action, chord)
}

/// The same for a given system, so the rules of each can be checked on any of them.
///
/// An action's own factory key is never refused: bare X is the factory key of the sketch's construction
/// toggle, and refusing it would make "put it back as it was" impossible by hand.
pub fn hotkey_refusal_on(os: platform_keys::Os, action: &str, chord: &Chord) -> Option<&'static str> {
    if !chord.bindable_key() {
        return Some("hotkeys-reserved");
    }
    let factory = HOTKEYS.iter().find(|r| r.action == action).is_some_and(|r| Chord::parse(r.key) == Some(*chord));
    // everything else is the system's table (see `platform_keys`)
    platform_keys::platform_keys(os).refusal(chord).filter(|_| !factory)
}

/// WHETHER THE KEY in this area is taken by somebody else - the name of the neighbouring action.
///
/// Two commands on one key is not "the last one wins" but a silently lost tool: the habitual key is
/// pressed, something else arrives, and it is not clear what broke. So rebinding asks here first.
/// Compared as CHORDS, not as text: `Shift+Ctrl+W` written by hand and `Ctrl+Shift+W` are one key.
pub fn hotkey_taken_by(set: &Settings, area: &str, key: &str, except: &str) -> Option<&'static str> {
    let want = Chord::parse(key)?;
    HOTKEYS.iter().filter(|r| r.area == area && r.action != except).find(|r| Chord::parse(&hotkey_key(set, r.action)) == Some(want)).map(|r| r.action)
}

/// The key bound to an action now: the person's record, else the factory one. EMPTY means the action
/// was deliberately left without a key.
pub fn hotkey_key(set: &Settings, action: &str) -> String {
    if let Some(k) = set.hotkeys.get(action) {
        return k.clone();
    }
    HOTKEYS.iter().find(|r| r.action == action).map(|r| r.key.to_string()).unwrap_or_default()
}

/// RECORDS A BINDING keeping the record clean: the factory key removes the entry rather than repeating it.
pub fn set_hotkey(set: &mut Settings, action: &str, key: &str) {
    if HOTKEYS.iter().any(|r| r.action == action && r.key == key) {
        set.hotkeys.remove(action);
    } else {
        set.hotkeys.insert(action.to_string(), key.to_string());
    }
}

/// HOW A CLASH IS SETTLED, as chosen in the window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClashChoice {
    /// The holder gets the key the asking action had: nothing ends up without a key.
    Swap,
    /// The asking action takes the key, the holder is left with none.
    Unbind,
}

/// SETTLES A CLASH: the asking action always gets its chord, the holder gets what the choice says.
pub fn resolve_hotkey_clash(set: &mut Settings, clash: &HotkeyClash, choice: ClashChoice) {
    let old = hotkey_key(set, &clash.action);
    let for_holder = match choice {
        ClashChoice::Swap => old,
        ClashChoice::Unbind => String::new(),
    };
    set_hotkey(set, &clash.action, &clash.chord);
    set_hotkey(set, clash.holder, &for_holder);
}

/// RESETS A BINDING TO THE FACTORY KEY unless another action of the area holds that key: then nothing changes and
/// the clash is handed back, to be asked like the press of that key. Applied regardless, two commands would share it.
pub fn reset_hotkey(set: &mut Settings, action: &str) -> Option<HotkeyClash> {
    let row = HOTKEYS.iter().find(|r| r.action == action)?;
    if let Some(holder) = hotkey_taken_by(set, row.area, row.key, action) {
        return Some(HotkeyClash { action: action.to_string(), chord: row.key.to_string(), holder });
    }
    set.hotkeys.remove(action);
    None
}

/// The thread standard for the index of the command bar's switch.
pub fn thread_standard(idx: u8) -> qymcad_core::thread::ThreadStandard {
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

/// One row of the reference. THERE IS NO TEXT HERE — only keys of the language catalogue: the reference
/// is read by a person, so it must speak their language like the rest of the interface.
pub struct HotkeyRow {
    /// Where it acts: `general`, `part`, `sketch`, `assembly`. A code and not a caption: the check
    /// against the handlers goes by it, and it cannot depend on the language.
    pub area: &'static str,
    /// WHAT IT DOES — a stable code of the action, and it is this that comes first.
    ///
    /// A key is rebindable, an action is not. While the handler matched THE KEY, rebinding was
    /// inexpressible: "E" meant "extrude" right inside the `match`. Now the key leads to an action and
    /// the action to a branch of the handler, and the first can be moved without touching the second.
    ///
    /// It is also the key the rebinding is stored under in the settings. So it must NOT contain the
    /// letter of the key: `part.extrude` and not `part.e`, otherwise the record "part.e = X" would lie
    /// to itself.
    ///
    /// WITH A DOT AND NOT A HYPHEN, and that is not taste: the keys of the language catalogue are
    /// written with hyphens, and the guard against a key reaching the screen untranslated catches ANY
    /// literal of that shape. The code of an action is not text for a person and must not be translated;
    /// the dot tells one from the other by eye and in the guards.
    pub action: &'static str,
    /// The DEFAULT key, in the spelling of `Chord::name`. What is actually pressed — see `hotkey_key`.
    pub key: &'static str,
    /// A catalogue key (`hotkey-<area>-<key>`), not a phrase.
    pub what: &'static str,
}

/// EVERY HOTKEY. `key` is exactly the name of the `egui::Key` variant (the test checks against the code
/// by it), except for the rows of the general area, where the keys are handled apart from the `match`
/// table.
pub const HOTKEYS: &[HotkeyRow] = &[
    // --- General ---
    HotkeyRow { area: "general", action: "general.esc", key: "Esc", what: "hotkey-general-esc" },
    HotkeyRow { area: "general", action: "general.enter", key: "Enter", what: "hotkey-general-enter" },
    HotkeyRow { area: "general", action: "general.delete", key: "Delete", what: "hotkey-general-delete" },
    HotkeyRow { area: "general", action: "general.rename", key: "F2", what: "hotkey-general-f2" },
    HotkeyRow { area: "general", action: "general.exit-context", key: "Ctrl+Enter", what: "hotkey-general-ctrl-enter" },
    HotkeyRow { area: "general", action: "general.undo-redo", key: "Ctrl+Z / Ctrl+Y", what: "hotkey-general-ctrl-z-ctrl-y" },
    HotkeyRow { area: "general", action: "general.save", key: "Ctrl+S", what: "hotkey-general-ctrl-s" },
    // --- Part ---
    HotkeyRow { area: "part", action: "part.sketch-pick", key: "K", what: "hotkey-part-k" },
    HotkeyRow { area: "part", action: "part.datum-plane", key: "D", what: "hotkey-part-d" },
    HotkeyRow { area: "part", action: "part.extrude", key: "E", what: "hotkey-part-e" },
    HotkeyRow { area: "part", action: "part.cut", key: "Q", what: "hotkey-part-q" },
    HotkeyRow { area: "part", action: "part.revolve", key: "R", what: "hotkey-part-r" },
    HotkeyRow { area: "part", action: "part.fillet", key: "F", what: "hotkey-part-f" },
    HotkeyRow { area: "part", action: "part.chamfer", key: "C", what: "hotkey-part-c" },
    HotkeyRow { area: "part", action: "part.shell", key: "H", what: "hotkey-part-h" },
    HotkeyRow { area: "part", action: "part.hole", key: "O", what: "hotkey-part-o" },
    HotkeyRow { area: "part", action: "part.mirror", key: "M", what: "hotkey-part-m" },
    HotkeyRow { area: "part", action: "part.box", key: "B", what: "hotkey-part-b" },
    HotkeyRow { area: "part", action: "part.cylinder", key: "Y", what: "hotkey-part-y" },
    HotkeyRow { area: "part", action: "part.measure", key: "I", what: "hotkey-part-i" },
    HotkeyRow { area: "part", action: "part.contour-reselect", key: "U", what: "hotkey-part-u" },
    // --- Sketch ---
    HotkeyRow { area: "sketch", action: "sketch.select", key: "S", what: "hotkey-sketch-s" },
    HotkeyRow { area: "sketch", action: "sketch.line", key: "L", what: "hotkey-sketch-l" },
    HotkeyRow { area: "sketch", action: "sketch.rect", key: "R", what: "hotkey-sketch-r" },
    HotkeyRow { area: "sketch", action: "sketch.circle", key: "C", what: "hotkey-sketch-c" },
    HotkeyRow { area: "sketch", action: "sketch.arc", key: "A", what: "hotkey-sketch-a" },
    HotkeyRow { area: "sketch", action: "sketch.point", key: "P", what: "hotkey-sketch-p" },
    HotkeyRow { area: "sketch", action: "sketch.polygon", key: "G", what: "hotkey-sketch-g" },
    HotkeyRow { area: "sketch", action: "sketch.slot", key: "O", what: "hotkey-sketch-o" },
    HotkeyRow { area: "sketch", action: "sketch.ellipse", key: "E", what: "hotkey-sketch-e" },
    HotkeyRow { area: "sketch", action: "sketch.spline", key: "N", what: "hotkey-sketch-n" },
    HotkeyRow { area: "sketch", action: "sketch.text", key: "T", what: "hotkey-sketch-t" },
    HotkeyRow { area: "sketch", action: "sketch.dim", key: "D", what: "hotkey-sketch-d" },
    HotkeyRow { area: "sketch", action: "sketch.corner-fillet", key: "F", what: "hotkey-sketch-f" },
    HotkeyRow { area: "sketch", action: "sketch.trim", key: "K", what: "hotkey-sketch-k" },
    HotkeyRow { area: "sketch", action: "sketch.mirror", key: "M", what: "hotkey-sketch-m" },
    HotkeyRow { area: "sketch", action: "sketch.construction", key: "X", what: "hotkey-sketch-x" },
    // --- Assembly ---
    HotkeyRow { area: "assembly", action: "assembly.datum-plane", key: "D", what: "hotkey-assembly-d" },
    HotkeyRow { area: "assembly", action: "assembly.new-part", key: "N", what: "hotkey-assembly-n" },
    HotkeyRow { area: "assembly", action: "assembly.new-subassembly", key: "U", what: "hotkey-assembly-u" },
    HotkeyRow { area: "assembly", action: "assembly.insert", key: "I", what: "hotkey-assembly-i" },
    HotkeyRow { area: "assembly", action: "assembly.rigid-joint", key: "J", what: "hotkey-assembly-j" },
];

/// The intersection of a ray (o + t*d) with a plane (point p0, normal n). None means the ray is nearly
/// parallel to the plane.
pub fn ray_plane(o: [f64; 3], d: [f64; 3], p0: [f64; 3], n: [f64; 3]) -> Option<[f64; 3]> {
    let dn = v_dot(d, n);
    if dn.abs() < 1e-9 {
        return None;
    }
    let t = v_dot(v_sub(p0, o), n) / dn;
    Some([o[0] + d[0] * t, o[1] + d[1] * t, o[2] + d[2] * t])
}

/// The distance from a point to a segment (in screen coordinates).
pub fn screen_dist_seg(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let (vx, vy) = (b.x - a.x, b.y - a.y);
    let (wx, wy) = (p.x - a.x, p.y - a.y);
    let len2 = vx * vx + vy * vy;
    let t = if len2 > 1e-6 { ((wx * vx + wy * vy) / len2).clamp(0.0, 1.0) } else { 0.0 };
    let (cx, cy) = (a.x + t * vx, a.y + t * vy);
    ((p.x - cx).powi(2) + (p.y - cy).powi(2)).sqrt()
}

/// The axis segment (origin plus or minus len * dir), for drawing and picking a datum axis.
pub fn axis_segment(origin: [f64; 3], dir: [f64; 3], len: f64) -> ([f64; 3], [f64; 3]) {
    let n = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt();
    let u = if n > 1e-9 { [dir[0] / n, dir[1] / n, dir[2] / n] } else { [0.0, 0.0, 1.0] };
    ([origin[0] - u[0] * len, origin[1] - u[1] * len, origin[2] - u[2] * len], [origin[0] + u[0] * len, origin[1] + u[1] * len, origin[2] + u[2] * len])
}

pub fn dist_to_contour(c: &Contour, p: Point2) -> f64 {
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

pub fn point_in_tri(p: Pos2, a: Pos2, b: Pos2, c: Pos2) -> bool {
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
pub fn tri_depth_at(p: Pos2, a: Pos2, da: f64, b: Pos2, db: f64, c: Pos2, dc: f64) -> f64 {
    let (wa, wb, wc) = tri_weights(p, a, b, c);
    da * wa + db * wb + dc * wc
}

/// A CORNER OF A TRIANGLE AS THE VIEW DRAWS IT: where it lies on the screen, where it is in space, and the perspective
/// factor it is drawn with - `Screen::at`'s `1 / (1 + depth / d_eye)`, 1 in an orthographic view.
#[derive(Clone, Copy, Debug)]
pub struct TriCorner {
    pub screen: Pos2,
    pub world: [f64; 3],
    pub persp: f64,
}

/// THE POINT OF THE TRIANGLE `corners` UNDER SCREEN `p`. The centre of the triangle stood here before, and it is
/// not the point pointed at: a click near the corner of a large facet looked at, measured from and trimmed at the
/// middle of the facet - measured, 50 points away on screen on a 60 x 40 block.
///
/// Weights taken on the screen are not weights in space once the view has perspective: the nearer corner is drawn
/// larger, and a point interpolated by the screen weights alone lay 12 points off the pointer on the top of a block
/// seen at three quarters. Each corner's weight is scaled by its perspective factor, as a perspective interpolation is.
pub fn tri_world_at(p: Pos2, corners: [TriCorner; 3]) -> [f64; 3] {
    let [a, b, c] = corners;
    let (la, lb, lc) = tri_weights(p, a.screen, b.screen, c.screen);
    let (ka, kb, kc) = (la * a.persp, lb * b.persp, lc * c.persp);
    let sum = ka + kb + kc;
    let (ka, kb, kc) = if sum.abs() > 1e-12 { (ka / sum, kb / sum, kc / sum) } else { (la, lb, lc) };
    std::array::from_fn(|i| a.world[i] * ka + b.world[i] * kb + c.world[i] * kc)
}

/// The weights of the corners of the screen triangle `a, b, c` at `p`; a third each when it is seen edge-on.
fn tri_weights(p: Pos2, a: Pos2, b: Pos2, c: Pos2) -> (f64, f64, f64) {
    let (v0x, v0y) = ((b.x - a.x) as f64, (b.y - a.y) as f64);
    let (v1x, v1y) = ((c.x - a.x) as f64, (c.y - a.y) as f64);
    let (v2x, v2y) = ((p.x - a.x) as f64, (p.y - a.y) as f64);
    let den = v0x * v1y - v1x * v0y;
    if den.abs() < 1e-12 {
        return (1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0); // degenerate (edge-on to the camera): fall back to the average
    }
    let u = (v2x * v1y - v1x * v2y) / den;
    let v = (v0x * v2y - v2x * v0y) / den;
    (1.0 - u - v, u, v)
}

pub fn dist_point_seg(p: Point2, a: Point2, b: Point2) -> f64 {
    let ab = b - a;
    let l2 = ab.x * ab.x + ab.y * ab.y;
    if l2 < 1e-12 {
        return p.dist(a);
    }
    let t = ((p.x - a.x) * ab.x + (p.y - a.y) * ab.y) / l2;
    let t = t.clamp(0.0, 1.0);
    p.dist(Point2::new(a.x + ab.x * t, a.y + ab.y * t))
}

/// WHERE THE BREADCRUMB GOES. Opening an edit records what a person just did, so that a report about a
/// crash carries the last few steps. The record crate must not know what a crash report is, nor where it
/// is written - the application plugs a sink in at startup, and until it does, the breadcrumb goes
/// nowhere.
static STEP_SINK: std::sync::Mutex<Option<fn(&str)>> = std::sync::Mutex::new(None);

/// Install the sink for the breadcrumbs. Called once, at startup.
pub fn set_step_recorder(f: fn(&str)) {
    if let Ok(mut s) = STEP_SINK.lock() {
        *s = Some(f);
    }
}

/// Record what is being done, if anyone is listening.
pub fn note_step(name: &str) {
    let f = STEP_SINK.lock().ok().and_then(|s| *s);
    if let Some(f) = f {
        f(name);
    }
}

/// "IS THERE ANYTHING UNSAVED" — the same document fingerprint the operation-boundary guard uses.
///
/// The name is kept because the question is a different one (it is compared against `saved_key`, not
/// `committed_key`), but the COMPUTATION must be single: two identical keys would drift apart on an
/// edit to one of them, and would drift silently. Drifting here is expensive both ways — a needless
/// question on closing, or lost work.
pub fn edit_key(dc: &DrawCtx) -> u64 {
    doc_key(dc.project)
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
pub fn mark_changed_params_dirty(params_seen: &std::collections::HashMap<String, f64>, project: &mut Project) {
    let vars = project.param_map();
    let mut changed: Vec<String> = vars.iter().filter(|(k, v)| params_seen.get(*k).is_none_or(|old| (*old - **v).abs() > 1e-12)).map(|(k, _)| k.clone()).collect();
    // a name that was seen and is gone - a parameter deleted - changes whatever was counted from it
    changed.extend(params_seen.keys().filter(|k| !vars.contains_key(*k)).cloned());
    for name in &changed {
        project.mark_param_dependents_dirty_for(name);
    }
}

/// AN ANGULAR DIMENSION AS IT STANDS ON THE SCREEN - one geometry for drawing it and for taking its label, so the label
/// is grabbed where the eye sees it.
pub struct AngleDim {
    /// Where the sides meet: the vertex, or where the two lines cross.
    pub center: Pos2,
    /// The screen direction of the first side and the signed turn to the second (|sweep| < pi).
    pub a0: f32,
    pub sweep: f32,
    /// The radius of the arc, px.
    pub r: f32,
    /// The arc drawn, as screen angles: the angle between the sides, run on to the label when it stands past one.
    pub arc: (f32, f32),
    /// The label, outside the arc so the arc does not run through the text.
    pub label: Pos2,
    /// Extension lines carrying a side out to the arc (or in to it) where the arc does not reach the side itself.
    pub ext: Vec<[Pos2; 2]>,
}

/// HOW THE TEXT OF A LINEAR DIMENSION IS TURNED. Along its line is the default: the text of a vertical dimension laid
/// level across a tall narrow line reads badly and takes room the sheet does not have.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, serde::Serialize, serde::Deserialize)]
pub enum DimTextTurn {
    /// level, whatever way the line runs
    Horizontal,
    /// along the dimension line, read from left to right or from the bottom up
    #[default]
    AlongLine,
}

/// A TURN THAT KEEPS A TEXT READABLE: the direction `angle` (radians, screen) folded into [-pi/2, pi/2), so a text laid
/// along a line reads from left to right, and along an upright line from the bottom up.
pub fn readable_angle(angle: f32) -> f32 {
    let pi = std::f32::consts::PI;
    let mut a = angle;
    while a >= pi / 2.0 {
        a -= pi;
    }
    while a < -pi / 2.0 {
        a += pi;
    }
    a
}

/// The size of the labels of sketch dimensions, px, and the range the settings keep it in.
pub const DIM_FONT_DEFAULT: f32 = 13.0;
pub const DIM_FONT_RANGE: std::ops::RangeInclusive<f32> = 8.0..=32.0;

/// WHAT THE LABEL OF A SKETCH DIMENSION SAYS: its value as the sheet shows it (`50.0`, `R5.0`, `45°`), led by the
/// name of a driver and the formula it is set by when the settings ask for them - `w = 2*w+10 = 50.0`. A reference
/// dimension is in brackets. `None` for a constraint that is not a dimension.
pub fn dim_caption(project: &Project, si: usize, c: &qymcad_core::model::Constraint, set: &Settings) -> Option<String> {
    use qymcad_core::model::Constraint;
    let value = match *c {
        // the number alone for every length: which way it runs is plain from its line
        Constraint::Distance { d, .. } => format!("{d:.1}"),
        Constraint::EdgeDistance { d, .. } => format!("T {d:.1}"),
        Constraint::DistancePL { d, .. } => format!("{:.1}", d.abs()), // d is signed (it carries the side)
        Constraint::Diameter { d, diam, .. } => format!("{}{d:.1}", if diam { "Ø" } else { "R" }),
        // the number alone: the arc mark over it is drawn with the dimension, a letter said less
        Constraint::ArcLength { len, .. } => format!("{len:.1}"),
        Constraint::Angle { deg, .. } | Constraint::AngleLines { deg, .. } => format!("{deg:.0}°"),
        _ => return None,
    };
    let mut parts: Vec<String> = Vec::new();
    if set.dim_show_name {
        let sketch = project.sketches.get(si)?.id;
        let refs = Project::dim_refs(c).map(|r| Project::dim_key_pub(&r)).unwrap_or_default();
        let name = project.name_of_target(&qymcad_core::model::DimTarget::Sketch { sketch, refs });
        if !name.is_empty() {
            parts.push(name);
        }
    }
    let expr = c.dim_expr().map(str::trim).unwrap_or("");
    // a formula is shown when there is one: a bare number typed into the field says nothing the value does not
    if set.dim_show_formula && !expr.is_empty() && expr.parse::<f64>().is_err() {
        parts.push(expr.to_string());
    }
    parts.push(value);
    let text = parts.join(" = ");
    Some(if c.is_driven() { format!("({text})") } else { text })
}

/// HOW MUCH ROOM A LABEL TAKES, px, at the size `px` - an estimate from the count of letters, the same for drawing the
/// label and for taking it with the mouse, so the two agree whatever the font makes of it.
pub fn dim_text_size(text: &str, px: f32) -> egui::Vec2 {
    egui::vec2(text.chars().count() as f32 * px * 0.56, px * 1.2)
}

/// A VALUE WITH A MINUS AND A PLUS beside it: each press moves it by `step` within `range`, and the value stands between
/// the buttons as a number. Answers the value as it now is.
pub fn step_buttons(ui: &mut egui::Ui, v: f32, step: f32, range: std::ops::RangeInclusive<f32>, less: &str, more: &str) -> f32 {
    let mut v = v.clamp(*range.start(), *range.end());
    if ui.add_enabled(v > *range.start(), egui::Button::new(egui_phosphor::regular::MINUS)).on_hover_text(less).clicked() {
        v = (v - step).max(*range.start());
    }
    ui.label(qymcad_i18n::num(v as f64, 0));
    if ui.add_enabled(v < *range.end(), egui::Button::new(egui_phosphor::regular::PLUS)).on_hover_text(more).clicked() {
        v = (v + step).min(*range.end());
    }
    v
}

/// HOW FAR POINT `pos` IS FROM A LABEL whose text of `size` stands centred at `center`: zero anywhere on the text,
/// otherwise the distance to its middle - so a label is taken by any of its letters, however large the settings make it.
pub fn label_reach(center: Pos2, size: egui::Vec2, pos: Pos2) -> f32 {
    if egui::Rect::from_center_size(center, size + egui::vec2(4.0, 4.0)).contains(pos) {
        0.0
    } else {
        center.distance(pos)
    }
}

/// Where the text of a linear dimension stands, and the shelf it stands on when it is carried out.
pub struct DimTextPlace {
    pub center: Pos2,
    /// the turn of the text, radians: 0 level, or the readable direction of its line
    pub angle: f32,
    /// the shelf past the arrow at `lb`, drawn along the dimension line, when the text does not fit between the arrows
    pub shelf: Option<[Pos2; 2]>,
}

/// THE GAP BETWEEN A LABEL AND THE LINE IT NAMES, px.
pub const DIM_TEXT_GAP: f32 = 3.0;

/// WHERE THE TEXT OF A LINEAR DIMENSION STANDS: beside the dimension line `la`-`lb` on the side `perp`, its box clear of
/// the line by a gap whatever the line's direction - the middle of the text used to stand 8 px off the line, and a
/// vertical dimension's text, 40 px wide, lay across it. Text that does not fit between the arrows (6 px each) is
/// carried past the arrow at `lb` onto a shelf along the line.
pub fn dim_text_place(la: Pos2, lb: Pos2, perp: egui::Vec2, size: egui::Vec2, at: Option<f64>, along_line: bool) -> DimTextPlace {
    let len = (lb - la).length();
    let dir = if len > 1e-3 { (lb - la) / len } else { egui::vec2(1.0, 0.0) };
    let perp = if perp.length() > 1e-6 { perp.normalized() } else { egui::vec2(-dir.y, dir.x) };
    // LAID ALONG THE LINE the text runs its width along it and its height across; level, its box meets the line askew
    let angle = if along_line { readable_angle(dir.y.atan2(dir.x)) } else { 0.0 };
    let (along, across) = if along_line {
        (size.x, size.y / 2.0 + DIM_TEXT_GAP)
    } else {
        (dir.x.abs() * size.x + dir.y.abs() * size.y, (perp.x.abs() * size.x + perp.y.abs() * size.y) / 2.0 + DIM_TEXT_GAP)
    };
    // LED ALONG BY HAND: the text stands where it was put, and past an arrow the line runs on under it as a shelf
    if let Some(t) = at {
        let s = t as f32 * len;
        let center = la + dir * s + perp * across;
        let shelf = if s > len {
            Some([lb, la + dir * (s + along / 2.0 + DIM_TEXT_GAP)])
        } else if s < 0.0 {
            Some([la, la + dir * (s - along / 2.0 - DIM_TEXT_GAP)])
        } else {
            None
        };
        return DimTextPlace { center, angle, shelf };
    }
    if along + 2.0 * 6.0 + 4.0 <= len {
        DimTextPlace { center: ((la.to_vec2() + lb.to_vec2()) / 2.0).to_pos2() + perp * across, angle, shelf: None }
    } else {
        let end = lb + dir * (along + 2.0 * DIM_TEXT_GAP + 4.0);
        DimTextPlace { center: lb + dir * (4.0 + DIM_TEXT_GAP + along / 2.0) + perp * across, angle, shelf: Some([lb, end]) }
    }
}

/// THE DIMENSION LINE OF A LINEAR DIMENSION on the screen - its two ends and the side its text stands on - for a length,
/// a point-to-line distance and a gap between round edges: one geometry for drawing the dimension, placing its text and
/// taking it with the mouse. `None` for any other constraint.
pub fn linear_dim_line(project: &Project, si: usize, c: &qymcad_core::model::Constraint, sh: &Sheet) -> Option<(Pos2, Pos2, egui::Vec2)> {
    use qymcad_core::model::Constraint;
    let sc = sh.view.scale;
    let p = |id| sketch_pt(project, si, id);
    match *c {
        Constraint::Distance { a, b, off, axis, .. } => {
            let (sa, sb) = (sh.at(p(a)?), sh.at(p(b)?));
            // THE TEXT STANDS ON THE FAR SIDE OF ITS LINE FROM THE GEOMETRY: a dimension above the geometry has its
            // text above its line, one below below it. A line lying on the geometry puts it above, or to the right.
            let away = |shift: f32, dflt: f32| if shift.abs() > 1e-3 { shift.signum() } else { dflt };
            Some(match axis {
                1 => {
                    let y = (sa.y + sb.y) / 2.0 + off as f32 * sc;
                    (Pos2::new(sa.x, y), Pos2::new(sb.x, y), egui::vec2(0.0, away(off as f32, -1.0)))
                }
                2 => {
                    let x = (sa.x + sb.x) / 2.0 + off as f32 * sc;
                    (Pos2::new(x, sa.y), Pos2::new(x, sb.y), egui::vec2(away(off as f32, 1.0), 0.0))
                }
                _ => {
                    let dir = (sb - sa).normalized();
                    let perp = egui::vec2(-dir.y, dir.x);
                    let shift = 16.0 + off as f32 * sc;
                    (sa + perp * shift, sb + perp * shift, perp * away(shift, 1.0))
                }
            })
        }
        Constraint::DistancePL { p: pt, a, b, off, .. } => {
            let (sp, sa) = (sh.at(p(pt)?), sh.at(p(a)?));
            let ab = line_screen_dir(project, &sh.view, si, a, b, sh.rect)?;
            let foot = sa + ab * (sp - sa).dot(ab);
            let o = ab * (off as f32 * sc); // the leader runs along the line measured from
                                            // its text beside it, on the side it was led to along that line - not along the dimension line itself
            Some((sp + o, foot + o, ab * if off < 0.0 { -1.0 } else { 1.0 }))
        }
        Constraint::EdgeDistance { c1, c2, m1, m2, off, .. } => {
            let (p1, p2) = (p(c1)?, p(c2)?);
            let (r1, r2) = (radius_of(project, si, c1).unwrap_or(0.0), radius_of(project, si, c2).unwrap_or(0.0));
            let len = ((p2.x - p1.x).powi(2) + (p2.y - p1.y).powi(2)).sqrt().max(1e-9);
            let (ux, uy) = ((p2.x - p1.x) / len, (p2.y - p1.y) / len);
            let e1 = Point2::new(p1.x - m1 as f64 * r1 * ux, p1.y - m1 as f64 * r1 * uy);
            let e2 = Point2::new(p2.x + m2 as f64 * r2 * ux, p2.y + m2 as f64 * r2 * uy);
            let (sa, sb) = (sh.at(e1), sh.at(e2));
            let dir = (sb - sa).normalized();
            let perp = egui::vec2(-dir.y, dir.x);
            let shift = 16.0 + off as f32 * sc;
            Some((sa + perp * shift, sb + perp * shift, if shift < 0.0 { -perp } else { perp }))
        }
        _ => None,
    }
}

/// Where the text of linear dimension `ci` stands, as drawn and as taken.
pub fn linear_text_of(project: &Project, si: usize, ci: usize, sh: &Sheet, set: &Settings) -> Option<(DimTextPlace, egui::Vec2)> {
    use qymcad_core::model::Constraint;
    let c = project.sketches.get(si)?.constraints.get(ci)?;
    let (la, lb, perp) = linear_dim_line(project, si, c, sh)?;
    let size = dim_text_size(&dim_caption(project, si, c, set)?, set.dim_font);
    let at = match *c {
        Constraint::Distance { at, .. } | Constraint::DistancePL { at, .. } | Constraint::EdgeDistance { at, .. } => at,
        _ => None,
    };
    Some((dim_text_place(la, lb, perp, size, at, set.dim_text == DimTextTurn::AlongLine), size))
}

/// THE TEXT OF A LINEAR DIMENSION LED ALONG ITS LINE by the pointer moving `delta` px: the share of the line it stands at
/// after the move, counted from where it stands now (the middle, or its shelf, when it was never placed).
pub fn linear_text_led(project: &Project, si: usize, ci: usize, sh: &Sheet, set: &Settings, delta: egui::Vec2) -> Option<f64> {
    let c = project.sketches.get(si)?.constraints.get(ci)?;
    let (la, lb, _) = linear_dim_line(project, si, c, sh)?;
    let len = (lb - la).length();
    if len < 1.0 {
        return None;
    }
    let dir = (lb - la) / len;
    let (place, _) = linear_text_of(project, si, ci, sh, set)?;
    let now = (place.center - la).dot(dir) / len;
    Some((now + delta.dot(dir) / len) as f64)
}

/// A RADIUS OR A DIAMETER ON THE SCREEN: its dimension line from `start` to the rim at `edge`, and its text - on a shelf
/// past the knee of the leader, or laid along the dimension line itself, turned by `angle`. One geometry for drawing the
/// dimension and for taking its text.
pub struct RadialDim {
    pub start: Pos2,
    pub edge: Pos2,
    pub knee: Pos2,
    pub text: Pos2,
    pub size: egui::Vec2,
    /// the turn of the text, radians: 0 on the shelf, the line's own direction (kept readable) on the line
    pub angle: f32,
    pub shelf: Option<[Pos2; 2]>,
}

/// AN ARC LENGTH ON THE SCREEN, drawn as the drawing standards draw it: a dimension arc about the same centre, out
/// past the measured arc by `ARC_DIM_GAP` px plus the dimension's own offset (`off`, mm along the radius), extension
/// lines from the ends of the arc out to it, arrows at both ends along it, and the number over its middle with a small
/// arc drawn over the number - the mark of an arc length. It was a caption beside the middle of the arc with a letter
/// L, no line and nothing to say what it measured. One geometry for drawing the dimension and for taking its text.
pub struct ArcLengthDim {
    /// the dimension arc, as screen points from its first end to its second
    pub arc: Vec<Pos2>,
    /// the two extension lines, from an end of the arc out past the dimension arc
    pub ext: [[Pos2; 2]; 2],
    /// the two arrows: the tip and the direction it points along the dimension arc
    pub arrows: [(Pos2, egui::Vec2); 2],
    /// the centre of the number
    pub text: Pos2,
    pub size: egui::Vec2,
    /// the arc mark over the number, as screen points
    pub mark: Vec<Pos2>,
}

/// How far the dimension arc of an arc length stands out past the arc it measures, in pixels, before its own offset.
const ARC_DIM_GAP: f32 = 14.0;

/// The screen geometry of arc length dimension `ci`; `None` for any other constraint.
pub fn arc_length_dim_geom(project: &Project, si: usize, ci: usize, sh: &Sheet, set: &Settings) -> Option<ArcLengthDim> {
    let s = project.sketches.get(si)?;
    let c = s.constraints.get(ci)?;
    let qymcad_core::model::Constraint::ArcLength { c: centre, a, b, ccw, off, .. } = *c else { return None };
    let (cp, pa, pb) = (sketch_pt(project, si, centre)?, sketch_pt(project, si, a)?, sketch_pt(project, si, b)?);
    let r = (pa.x - cp.x).hypot(pa.y - cp.y);
    let scale = sh.view.scale as f64;
    let rd = (r + ARC_DIM_GAP as f64 / scale + off).max(r * 0.2); // never through the centre
    let (a0, a1) = ((pa.y - cp.y).atan2(pa.x - cp.x), (pb.y - cp.y).atan2(pb.x - cp.x));
    let tau = std::f64::consts::TAU;
    let sweep = if ccw { (a1 - a0).rem_euclid(tau) } else { -(a0 - a1).rem_euclid(tau) };
    let at = |ang: f64, rad: f64| sh.at(Point2::new(cp.x + rad * ang.cos(), cp.y + rad * ang.sin()));
    let steps = ((sweep.abs() * rd * scale / 4.0).ceil() as usize).clamp(4, 96);
    let arc: Vec<Pos2> = (0..=steps).map(|k| at(a0 + sweep * k as f64 / steps as f64, rd)).collect();
    let past = rd + 3.0 / scale; // the extension lines run a little past the dimension arc
    let ext = [[at(a0, r), at(a0, past)], [at(a0 + sweep, r), at(a0 + sweep, past)]];
    let along = |i: usize, j: usize| (arc[j] - arc[i]).normalized();
    let n = arc.len() - 1;
    let arrows = [(arc[0], along(1, 0)), (arc[n], along(n - 1, n))];
    let mid = a0 + sweep / 2.0;
    let size = dim_text_size(&dim_caption(project, si, c, set)?, set.dim_font);
    let out = egui::vec2(mid.cos() as f32, -(mid.sin() as f32)); // the screen's y runs down
    let text = at(mid, rd) + out * (size.y * 0.5 + 9.0);
    // the arc mark: a small arc over the number, bulging away from the dimension arc
    let half = size.x.min(14.0) * 0.5;
    let side = egui::vec2(out.y, -out.x);
    let crown = text + out * (size.y * 0.5 + 4.0);
    let mark: Vec<Pos2> = (0..=8)
        .map(|k| {
            let t = k as f32 / 8.0 * 2.0 - 1.0;
            crown + side * (t * half) + out * ((1.0 - t * t) * 3.0)
        })
        .collect();
    Some(ArcLengthDim { arc, ext, arrows, text, size, mark })
}

/// The screen geometry of radius or diameter dimension `ci`; `None` for any other constraint.
pub fn radial_dim_geom(project: &Project, si: usize, ci: usize, sh: &Sheet, set: &Settings) -> Option<RadialDim> {
    use qymcad_core::model::Constraint;
    let c = project.sketches.get(si)?.constraints.get(ci)?;
    let Constraint::Diameter { c: centre, off, diam, at, .. } = *c else { return None };
    let cp = sketch_pt(project, si, centre)?;
    let r = radius_of(project, si, centre)?;
    let sc = sh.at(cp);
    let r_px = (sh.at(Point2::new(cp.x + r, cp.y)) - sc).length();
    let dir = egui::vec2((off as f32).cos(), (off as f32).sin());
    let (start, edge, knee) = (if diam { sc - dir * r_px } else { sc }, sc + dir * r_px, sc + dir * (r_px + 14.0));
    let size = dim_text_size(&dim_caption(project, si, c, set)?, set.dim_font);
    Some(match at {
        None => {
            let (text, shelf) = radial_text_place(knee, dir, size);
            RadialDim { start, edge, knee, text, size, angle: 0.0, shelf: Some(shelf) }
        }
        Some(t) => {
            // ON THE LINE: turned along it, kept reading left to right, and standing above it by a gap
            let angle = readable_angle(dir.y.atan2(dir.x));
            let up = egui::vec2(angle.sin(), -angle.cos());
            let on = start + (edge - start) * t as f32;
            RadialDim { start, edge, knee, text: on + up * (size.y / 2.0 + DIM_TEXT_GAP), size, angle, shelf: None }
        }
    })
}

/// THE TEXT OF A RADIUS OR A DIAMETER FOLLOWS THE POINTER: the dimension line turns to it, and the text lies on the line
/// while the pointer is within the circle (a little past its rim), on the shelf once it is led farther out. Answers
/// `(off, at)`.
pub fn radial_text_follow(project: &Project, si: usize, ci: usize, sh: &Sheet, pp: Pos2) -> Option<(f64, Option<f64>)> {
    use qymcad_core::model::Constraint;
    let c = project.sketches.get(si)?.constraints.get(ci)?;
    let Constraint::Diameter { c: centre, off, diam, .. } = *c else { return None };
    let cp = sketch_pt(project, si, centre)?;
    let r = radius_of(project, si, centre)?;
    let sc = sh.at(cp);
    let r_px = (sh.at(Point2::new(cp.x + r, cp.y)) - sc).length().max(1.0);
    let w = pp - sc;
    let rho = w.length();
    let ang = if rho > 4.0 { w.y.atan2(w.x) as f64 } else { off };
    if rho > r_px + 8.0 {
        return Some((ang, None));
    }
    let t = if diam { (rho + r_px) / (2.0 * r_px) } else { rho / r_px };
    Some((ang, Some(t.clamp(0.05, 0.95) as f64)))
}

/// How far `pos` is from a text of `size` centred at `center` and turned by `angle`: zero anywhere on the text.
pub fn label_reach_turned(center: Pos2, size: egui::Vec2, angle: f32, pos: Pos2) -> f32 {
    let d = pos - center;
    let (c, s) = (angle.cos(), angle.sin());
    let local = egui::vec2(d.x * c + d.y * s, -d.x * s + d.y * c); // the pointer in the frame of the text
    label_reach(Pos2::ZERO, size, local.to_pos2())
}

/// WHERE THE TEXT OF A RADIUS OR A DIAMETER STANDS: the leader breaks at `knee` into a shelf running the way the leader
/// points, and the text stands on the shelf rather than on the leader. Answers the middle of the text and the shelf.
pub fn radial_text_place(knee: Pos2, toward: egui::Vec2, size: egui::Vec2) -> (Pos2, [Pos2; 2]) {
    let sign = if toward.x < 0.0 { -1.0 } else { 1.0 };
    let end = knee + egui::vec2(sign * (size.x + 2.0 * DIM_TEXT_GAP), 0.0);
    (knee + egui::vec2(sign * (size.x / 2.0 + DIM_TEXT_GAP), -(size.y / 2.0 + DIM_TEXT_GAP)), [knee, end])
}

/// The default radius of the arc of an angle, px, before one is placed.
pub const ANGLE_ARC_PX: f32 = 24.0;

/// Where the sides of an angular dimension meet and run, on the screen, and where its arc is placed.
struct AngleSides {
    /// where the sides meet
    center: Pos2,
    /// each side by its two ends, the far end first, which gives the side its direction
    s1: [Pos2; 2],
    s2: [Pos2; 2],
    /// the radius of the arc, in sketch units; 0 is the default distance on screen
    off: f64,
    /// where the label stands along the arc, as a share of the angle from the first side; none on the bisector
    at: Option<f64>,
}

/// Where the sides of angular dimension `ci` of sketch `si` meet and run, on the screen.
fn angle_sides(project: &Project, si: usize, c: &qymcad_core::model::Constraint, sh: &Sheet) -> Option<AngleSides> {
    use qymcad_core::model::Constraint;
    let p = |id| sketch_pt(project, si, id).map(|w| sh.at(w));
    match *c {
        Constraint::Angle { a, b, c, off, at, .. } => {
            let (sa, sb, sc) = (p(a)?, p(b)?, p(c)?);
            Some(AngleSides { center: sb, s1: [sa, sb], s2: [sc, sb], off, at })
        }
        Constraint::AngleLines { a, b, c, d, off, at, .. } => {
            let (sa, sb, sc, sd) = (p(a)?, p(b)?, p(c)?, p(d)?);
            let ix = lines_intersect(sa, sb, sc, sd)?;
            // A SIDE THAT ENDS WHERE THE SIDES MEET runs on past its end the way its line runs, first end to second:
            // the angle of a chamfer is measured at the end of the cut, and the side taken from the meeting to that
            // same end had no length and so no direction - the arc of the angle turned with the rounding noise.
            let far = |first: Pos2, second: Pos2| if (second - ix).length() < 1.0 { ix + (second - first).normalized() } else { second };
            Some(AngleSides { center: ix, s1: [far(sa, sb), sa], s2: [far(sc, sd), sc], off, at })
        }
        _ => None,
    }
}

/// The screen geometry of angular dimension `ci` of sketch `si`; `None` for any other constraint.
pub fn angle_dim_geom(project: &Project, si: usize, ci: usize, sh: &Sheet, set: &Settings) -> Option<AngleDim> {
    let c = project.sketches.get(si)?.constraints.get(ci)?;
    let size = dim_text_size(&dim_caption(project, si, c, set)?, set.dim_font);
    let AngleSides { center, s1, s2, off, at } = angle_sides(project, si, c, sh)?;
    let (u, v) = ((s1[0] - center).normalized(), (s2[0] - center).normalized());
    let pi = std::f32::consts::PI;
    let a0 = u.y.atan2(u.x);
    let mut sweep = v.y.atan2(v.x) - a0;
    while sweep > pi {
        sweep -= 2.0 * pi;
    }
    while sweep < -pi {
        sweep += 2.0 * pi;
    }
    let r = if off > 0.0 { (sh.at(Point2::new(off, 0.0)) - sh.at(Point2::new(0.0, 0.0))).length() } else { ANGLE_ARC_PX };
    let t = at.unwrap_or(0.5) as f32;
    let (lo, hi) = (t.min(0.0), t.max(1.0));
    let la = a0 + sweep * t;
    let ld = egui::vec2(la.cos(), la.sin());
    // the whole box of the text stands outside the arc: its half extent along the direction plus a gap
    let label = center + ld * (r + angle_label_clear(ld, size));
    // A SIDE SHORTER THAN THE ARC, or one that starts past it, is carried to the arc by a thin extension line with a
    // 4 px overshoot, as on a drawing
    let mut ext = Vec::new();
    for (side, dir) in [(s1, u), (s2, v)] {
        let along = |q: Pos2| (q - center).dot(dir);
        let (near, far) = (along(side[0]).min(along(side[1])).max(0.0), along(side[0]).max(along(side[1])));
        if r > far + 1.0 {
            ext.push([center + dir * far, center + dir * (r + 4.0)]);
        } else if r < near - 1.0 {
            ext.push([center + dir * (r - 4.0), center + dir * near]);
        }
    }
    Some(AngleDim { center, a0, sweep, r, arc: (a0 + sweep * lo, a0 + sweep * hi), label, ext })
}

/// How far past the arc the middle of an angle's label stands in direction `ld`: its half extent that way and a gap.
fn angle_label_clear(ld: egui::Vec2, size: egui::Vec2) -> f32 {
    (ld.x.abs() * size.x + ld.y.abs() * size.y) / 2.0 + DIM_TEXT_GAP + 2.0
}

/// THE LABEL OF AN ANGLE FOLLOWS THE POINTER: the radius of the arc is the pointer's distance from where the sides
/// meet (sketch units), and the place along it the share of the angle the pointer stands at - past a side the arc
/// runs on to it. Answers `(off, at)`, or `None` for any other constraint.
pub fn angle_dim_follow(project: &Project, si: usize, ci: usize, sh: &Sheet, set: &Settings, cur: Pos2) -> Option<(f64, Option<f64>)> {
    let g = angle_dim_geom(project, si, ci, sh, set)?;
    let size = dim_text_size(&dim_caption(project, si, project.sketches.get(si)?.constraints.get(ci)?, set)?, set.dim_font);
    let w = cur - g.center;
    let r_px = w.length();
    if r_px < 1.0 || g.sweep.abs() < 1e-4 {
        return None;
    }
    let u = egui::vec2(g.a0.cos(), g.a0.sin());
    let phi = (u.x * w.y - u.y * w.x).atan2(u.dot(w)); // signed turn from the first side to the pointer
    let t = (phi / g.sweep).clamp(-1.0, 2.0);
    let per_px = (sh.at(Point2::new(1.0, 0.0)) - sh.at(Point2::new(0.0, 0.0))).length().max(1e-9);
    Some((((r_px - angle_label_clear(w / r_px, size)).max(4.0) / per_px) as f64, Some(t as f64)))
}

pub fn lines_intersect(a: Pos2, b: Pos2, c: Pos2, d: Pos2) -> Option<Pos2> {
    let (rx, ry) = (b.x - a.x, b.y - a.y);
    let (sx, sy) = (d.x - c.x, d.y - c.y);
    let denom = rx * sy - ry * sx;
    if denom.abs() < 1e-6 {
        return None;
    }
    let t = ((c.x - a.x) * sy - (c.y - a.y) * sx) / denom;
    Some(Pos2::new(a.x + t * rx, a.y + t * ry))
}

/// An ellipse from its three clicks: the centre `c`, the end `a` of the major semi-axis, and a point `e` whose distance
/// from the major axis is the minor semi-axis. Gives (major semi-axis, rotation in radians, minor semi-axis).
pub fn ellipse_from_clicks(c: Point2, a: Point2, e: Point2) -> (f64, f64, f64) {
    let (dx, dy) = (a.x - c.x, a.y - c.y);
    let major = dx.hypot(dy);
    if major < 1e-9 {
        return (0.0, 0.0, 0.0);
    }
    let minor = ((e.x - c.x) * (-dy) + (e.y - c.y) * dx).abs() / major;
    (major, dy.atan2(dx), minor)
}

/// A constraint's label for the list.
/// The circle through three points, giving (centre x, centre y, radius). None means they are collinear.
pub fn circumcircle(a: Point2, b: Point2, c: Point2) -> Option<(f64, f64, f64)> {
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
pub fn tangent_arc(s: Point2, t: (f64, f64), e: Point2) -> Option<(f64, f64, f64, bool)> {
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

/// The two unit world axes that define the plane of a gizmo's rotation ring for axis `ax`.
pub fn ring_axes(ax: u8) -> ([f64; 3], [f64; 3]) {
    match ax {
        0 => ([0.0, 1.0, 0.0], [0.0, 0.0, 1.0]), // the ring about X lies in the YZ plane
        1 => ([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]), // about Y, in XZ
        _ => ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]), // about Z, in XY
    }
}

/// An orthonormal pair (u, v) spanning the plane perpendicular to an arbitrary axis `n` — for the ring of a
/// degree-of-freedom gizmo about a joint's axis, which, unlike `ring_axes`, does not line up with the world
/// X, Y and Z.
pub fn perp_basis(n: [f64; 3]) -> ([f64; 3], [f64; 3]) {
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

/// Rotate the point `p` about an axis (origin `o`, direction `dir`) by `ang` radians (Rodrigues) — for the
/// preview of a circular pattern. It matches the core's `rot_about_axis` (a right-handed frame).
pub fn rotate_pt_about_axis(o: [f64; 3], dir: [f64; 3], ang: f64, p: [f64; 3]) -> [f64; 3] {
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

/// The signed area (the edge function) used for barycentric coordinates and rasterisation.
pub fn edge(ax: f32, ay: f32, bx: f32, by: f32, px: f32, py: f32) -> f32 {
    (bx - ax) * (py - ay) - (by - ay) * (px - ax)
}

/// Barycentric interpolation of the vertex colours (Gouraud). `b0` to `b2` are normalised weights summing
/// to about 1, paired with `cols[0..2]`. It works for premultiplied colour too, being linear.
#[inline]
/// WHICH BODY AN OPERATION WORKS ON: the one selected, failing that the one the context stands in.
///
/// Three lines, and twenty-seven methods of the Part workbench used to hang off it - the commonest question
/// a feature asks. Free, it stops holding them all inside the crate that declares `App`.
pub fn op_target_body(dc: &DrawCtx, sel: Sel) -> Option<Id> {
    selected_body(dc.project, &sel).or_else(|| current_body(dc))
}

/// The step vector for direction `dir` (0 = X, 1 = Y, 2 = Z) of magnitude `step`.
pub fn arr_vec(dir: u8, step: f64) -> (f64, f64, f64) {
    match dir {
        1 => (0.0, step, 0.0),
        2 => (0.0, 0.0, step),
        _ => (step, 0.0, 0.0),
    }
}

/// The screen-linear depth `ndc_z` for a world `depth = rel . fwd`. In perspective this is
/// `clip_z/clip_w` (PERSPECTIVE-CORRECT, planar on the screen), so the linear interpolation in
/// `raster_band` gives the right visibility; in orthographic mode it is linear in world space. The
/// formula is THE SAME as in the GPU shader (`vs_mesh`), so both paths agree. `near` maps to 0 and
/// `far` to 1, smaller is nearer (the z comparison is `<`). The parameters come from `proj_params`,
/// computed once per frame rather than per vertex.
pub fn depth_ndc(world_depth: f64, inv_d: f64, z_near: f64, z_far: f64, depth_half: f64) -> f32 {
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

/// Rotate a normal by the linear (3x3) part of a body's world transform, ignoring the translation. The
/// components' transforms are rigid and uniform, so R * n is enough without an inverse transpose; the
/// result is normalised.
pub fn rotate_normal(wt: &[f64; 12], n: [f64; 3]) -> [f64; 3] {
    v_norm([wt[0] * n[0] + wt[1] * n[1] + wt[2] * n[2], wt[4] * n[0] + wt[5] * n[1] + wt[6] * n[2], wt[8] * n[0] + wt[9] * n[1] + wt[10] * n[2]])
}

/// The outline polyline of an ellipse entity in WORLD coordinates (for hit-testing and drawing).
pub fn ellipse_outline_world(project: &qymcad_core::model::Project, si: usize, c: Id, ma: Id, mi: Id) -> Vec<Point2> {
    let Some(s) = project.sketches.get(si) else { return Vec::new() };
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

/// THE LINE OF SIGHT AT POINT `p` — the ray FROM THE EYE, not the camera's overall direction.
///
/// In orthographic mode there is one ray for the whole frame, and it is `fwd`. In perspective every
/// point has its own, diverging from `fwd` towards the edges of the frame the more, the wider the field
/// of view. Back-face culling computed from `fwd` therefore lied both ways: visible faces were thrown
/// away (gaps in the body) and invisible ones stayed (a ring fell apart into ribbons). One formula for
/// the raster and for the shader.
pub fn view_dir_at(cam: &Cam3, p: [f64; 3], fwd: [f64; 3], inv_d: f64) -> [f64; 3] {
    if inv_d <= 0.0 {
        return fwd;
    }
    let d_eye = 1.0 / inv_d;
    let eye = [cam.target[0] - fwd[0] * d_eye, cam.target[1] - fwd[1] * d_eye, cam.target[2] - fwd[2] * d_eye];
    v_norm(v_sub(p, eye))
}

/// Whether a body takes part in an interference (for the red highlight in `draw_mesh`).
pub fn body_interferes(interference: &Interference, body: Id) -> bool {
    interference.pairs.iter().any(|(a, b)| *a == body || *b == body)
}

/// The world contours of the loft's sections (in `loft_sids` order), each a closed loop of world points.
pub fn loft_preview(loft: &LoftParams, project: &Project) -> Vec<Vec<[f64; 3]>> {
    let mut out = Vec::with_capacity(loft.sids.len());
    for (i, &sid) in loft.sids.iter().enumerate() {
        let cid = project.loft_section_contour(sid, loft.cids.get(i).copied().unwrap_or(0));
        let (Some(cid), Some(pf)) = (cid, project.sketch_frame_by_id(sid)) else { continue };
        let Some(idx) = project.contour_index(cid) else { continue };
        let c = &project.contours[idx];
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

pub fn selected_body(project: &Project, sel: &Sel) -> Option<Id> {
    match *sel {
        Sel::Mesh(mi) => project.mesh_id(mi),
        Sel::Feature(ti) => project.timeline.get(ti).and_then(|n| n.kind.body()),
        _ => None,
    }
}

/// The readout of a component's gizmo (mm or degrees), taking snapping into account.
pub fn comp_giz_readout(comp_giz: &CompGizmo, set: &Settings, snap: bool) -> Option<String> {
    let (_, _, _, amt) = comp_giz.drag?;
    if comp_giz.axis.is_some() {
        let step = set.snap.grid.max(0.01);
        let d = if snap { (amt / step).round() * step } else { amt };
        Some(qymcad_i18n::tr1("giz-shift-mm", "v", &format!("{d:+.2}")))
    } else if comp_giz.ring.is_some() {
        let step = set.snap.rot_deg.max(0.1);
        let g = if snap { (amt / step).round() * step } else { amt };
        Some(format!("{g:+.1}{}", qymcad_i18n::tr("unit-deg-suffix")))
    } else {
        None
    }
}

/// The current number for a body gizmo's readout (mm for an axis, degrees for a ring), with snapping applied.
pub fn body_giz_readout(body_giz: &BodyGizmo, set: &Settings, snap: bool) -> Option<String> {
    let (_, _, amt) = body_giz.drag?;
    if body_giz.axis.is_some() {
        let step = body_snap_mm(set);
        let d = if snap { (amt / step).round() * step } else { amt };
        Some(qymcad_i18n::tr1("giz-shift-mm", "v", &format!("{d:+.2}")))
    } else if body_giz.ring.is_some() {
        let g = if snap { (amt / body_snap_deg(set)).round() * body_snap_deg(set) } else { amt };
        Some(format!("{g:+.1}{}", qymcad_i18n::tr("unit-deg-suffix")))
    } else {
        None
    }
}

/// The radius of a circle centred at `p` that touches the base edge (the perpendicular distance to a
/// line, or |dist - R| to a circle).
pub fn tangent_radius_to_edge(dc: &DrawCtx, si: usize, eref: EdgeRef, p: Point2) -> f64 {
    match eref {
        EdgeRef::Line { a, b } => match (sketch_pt(dc.project, si, a), sketch_pt(dc.project, si, b)) {
            (Some(pa), Some(pb)) => {
                let (dx, dy) = (pb.x - pa.x, pb.y - pa.y);
                let len = (dx * dx + dy * dy).sqrt().max(1e-9);
                ((dx * (p.y - pa.y) - dy * (p.x - pa.x)) / len).abs()
            }
            _ => 0.0,
        },
        EdgeRef::Circle { center, r } => match sketch_pt(dc.project, si, center) {
            Some(pc) => (((p.x - pc.x).powi(2) + (p.y - pc.y).powi(2)).sqrt() - r).abs(),
            None => 0.0,
        },
    }
}

/// The world (origin, normal) of the plane the command has picked — for the PREVIEW, without creating a
/// datum (unlike `resolve_mirror_plane`). For a face, the local centroid and normal are taken into
/// world space through the body's display transform.
pub fn mirror_plane_world(dc: &DrawCtx, sp: &qymcad_core::feature::SketchPlane) -> Option<([f64; 3], [f64; 3])> {
    use qymcad_core::feature::{apply12, BasePlane, SketchPlane};
    match sp {
        SketchPlane::World(BasePlane::XY) => Some(([0.0, 0.0, 0.0], [0.0, 0.0, 1.0])),
        SketchPlane::World(BasePlane::XZ) => Some(([0.0, 0.0, 0.0], [0.0, 1.0, 0.0])),
        SketchPlane::World(BasePlane::YZ) => Some(([0.0, 0.0, 0.0], [1.0, 0.0, 0.0])),
        SketchPlane::Datum(id) => dc.project.planes.iter().find(|p| p.id == *id).map(|p| (p.origin, p.normal)),
        SketchPlane::Face(body, key) => {
            let ctx = current_ctx_id(dc.active_path, dc.project);
            let wt = dc.project.body_display_transform(*body, ctx);
            let o = apply12(&wt, key.centroid);
            let z = apply12(&wt, [0.0, 0.0, 0.0]);
            let n = [apply12(&wt, key.normal)[0] - z[0], apply12(&wt, key.normal)[1] - z[1], apply12(&wt, key.normal)[2] - z[2]];
            Some((o, n))
        }
    }
}

/// The WORLD frame of a candidate sketch plane, built the same way `Project::sketch_frame` builds it
/// (World gives `b.frame()`, Datum and Face give `world_aligned`), so that the (u, v) snapping is
/// computed in THE SAME axes the origin is later lifted in. For a face it is the local base frame
/// carried into world space by `body_display_transform`.
pub fn world_frame_of_plane(dc: &DrawCtx, sp: &qymcad_core::feature::SketchPlane) -> Option<qymcad_core::feature::PlaneFrame> {
    use qymcad_core::feature::{PlaneFrame, SketchPlane};
    match sp {
        SketchPlane::World(b) => Some(b.frame()),
        SketchPlane::Datum(id) => dc.project.planes.iter().find(|p| p.id == *id).map(|p| PlaneFrame::world_aligned(p.origin, p.normal, p.rot_deg)),
        SketchPlane::Face(body, key) => {
            let (c, n) = dc.project.resolve_face(*body, key);
            let wt = dc.project.body_display_transform(*body, current_ctx_id(dc.active_path, dc.project));
            Some(PlaneFrame::world_aligned(c, n, 0.0).transformed(&wt))
        }
    }
}

/// The contour under the cursor (for the hover highlight), within about 8 px.
pub fn hovered_contour(cursor: Option<Point2>, project: &Project, view: View2d) -> Option<usize> {
    let cur = cursor?;
    let thresh = 8.0 / view.scale as f64;
    let mut best: Option<(usize, f64)> = None;
    for (i, c) in project.contours.iter().enumerate() {
        let d = dist_to_contour(c, cur);
        if best.is_none_or(|(_, bd)| d < bd) {
            best = Some((i, d));
        }
    }
    best.filter(|(_, d)| *d <= thresh).map(|(i, _)| i)
}

/// THE NORMALS A BODY IS LIT BY in smooth shading. A body from the kernel is tessellated face by face, so a vertex is
/// shared only within a face and its averaged normal is right (`Mesh::vertex_normals`). A piece of a mesh from a file
/// shares its vertices across its sharp edges too, and is lit at the corners of its triangles, smoothed only where the
/// surface turns by less than `CREASE_DEG` (`Mesh::corner_normals`).
pub enum Normals {
    AtVertex(Vec<[f64; 3]>),
    AtCorner(Vec<[f64; 3]>),
}

impl Normals {
    /// The normal at corner `k` of triangle `t`, whose vertex is `v`.
    pub fn at(&self, t: usize, k: usize, v: u32) -> [f64; 3] {
        match self {
            Normals::AtVertex(n) => n[v as usize],
            Normals::AtCorner(n) => n[3 * t + k],
        }
    }
}

/// The turn between two triangles of a mesh piece across which its light is still smoothed.
pub const CREASE_DEG: f64 = 30.0;

/// Refresh the cache of smoothed normals (`Normals`) for the current `geom_rev` (lazily, indexed as
/// `project.meshes` is). The heavy pass over the triangles is done ONCE per change of geometry rather
/// than every frame.
pub fn ensure_vertex_normals(cache: &Caches, project: &Project, regen: &Rebuilding) {
    let mut c = cache.norm.borrow_mut();
    if c.rev == regen.geom_rev && c.value.len() == project.bodies.len() {
        return;
    }
    let pieces: std::collections::HashSet<qymcad_core::model::Id> = project
        .timeline
        .iter()
        .filter_map(|n| match n.kind {
            qymcad_core::feature::FeatureKind::MeshPiece { body, .. } => Some(body),
            _ => None,
        })
        .collect();
    c.value = project.bodies.iter().map(|b| if pieces.contains(&b.id) { Normals::AtCorner(b.mesh.corner_normals(CREASE_DEG)) } else { Normals::AtVertex(b.mesh.vertex_normals()) }).collect();
    c.rev = regen.geom_rev;
}

/// The scene key for the GPU buffer: the same as in `view_key`, BUT without the camera, the size or the
/// dragging flag (the vertices and their colours do not depend on the camera). When it changes, the
/// vertex buffer is uploaded anew.
pub fn gpu_scene_key(pn: &Painting) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    view_rev(pn.regen).hash(&mut h);
    (pn.set.shading == Shading::Smooth).hash(&mut h); // smooth carries a vertex normal, flat carries none - a different buffer
                                                      // THE COLOURS ARE NOT HERE ANY MORE, and neither is the highlight. Both live in the look table beside the
                                                      // vertices: changing a scheme or moving the pointer over the model rewrites two numbers per body instead
                                                      // of the whole scene. Measured on the reference engine: 739 MB re-uploaded for a change of highlight.
    if let Some((o, n)) = section_eff(pn.section) {
        for v in o.iter().chain(n.iter()) {
            v.to_bits().hash(&mut h); // the section moved or turned, so the buffer is rebuilt
        }
    }
    // WHICH BODIES ARE DRAWN AND WHERE EACH ONE STANDS - the two things the vertices are made of.
    //
    // The context used to be hashed as itself, together with the component transforms and the visibility
    // ticks. Stepping into a subassembly changed the key even when not a single vertex moved, and the whole
    // scene went to the card again: 739 MB on the reference engine, which is what "cannot enter the
    // assembly, it hangs" was. Membership and placement are asked of the same function that builds the
    // scene, so the key cannot drift away from what is in the buffer; it costs one composed transform per
    // visible body (139 of them on the reference assembly, 2000 on the engine).
    for m in visible_mesh_items(pn) {
        (m.index as u32).hash(&mut h);
        for v in m.world {
            v.to_bits().hash(&mut h); // the gizmo's drag preview is already inside this transform
        }
    }
    h.finish()
}

/// The key of the 3D render cache: the view (camera, size, pixels per point), the geometry revision and
/// the visibility.
pub fn view_key(pn: &Painting, rect: Rect, ppp: f32) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    pn.cam.yaw.to_bits().hash(&mut h);
    pn.cam.pitch.to_bits().hash(&mut h);
    (pn.cam.scale as f64).to_bits().hash(&mut h);
    for t in pn.cam.target {
        t.to_bits().hash(&mut h);
    }
    ((rect.width() * ppp) as u32).hash(&mut h);
    ((rect.height() * ppp) as u32).hash(&mut h);
    pn.view_dragging.hash(&mut h); // the low-resolution and full-resolution pictures are separate cache entries
    pn.regen.geom_rev.hash(&mut h);
    // the components' placements: moving a part in an assembly changes the picture, and the raster cache must see it
    for c in &pn.project.components {
        for v in c.transform {
            v.to_bits().hash(&mut h);
        }
        c.visible.hash(&mut h); // visibility is hierarchical (`body_shown` follows the chain of ticks), so it goes into the key
    }
    (pn.set.projection == Projection::Perspective).hash(&mut h); // perspective against orthographic changes the projection, so it goes into the raster cache key
    (pn.set.shading == Shading::Smooth).hash(&mut h); // smooth against flat shading changes the vertex colours, so it goes in too
    pn.scheme.pal.fingerprint().hash(&mut h); // the raster is already coloured by the scheme: change the scheme and it is drawn anew
                                              // The selection highlight is painted into the texture, so the WHOLE set of highlighted bodies goes into the
                                              // key - the one both drawing paths colour by (a body lights itself, a component its whole subtree), in a
                                              // fixed order, the set being a hash set. Without it a click on a part in the tree lit nothing on the
                                              // software picture until the camera moved.
    let mut lit: Vec<usize> = highlight_mesh_set(pn.project, &pn.sel).into_iter().collect();
    lit.sort_unstable();
    lit.hash(&mut h);
    // a body switched off by its own tick is not drawn
    for (i, v) in pn.project.bodies.iter().map(|b| b.visible).enumerate() {
        if v {
            (i as u32).hash(&mut h);
        }
    }
    // in-context mode and the active context change which bodies are visible and which are ghosted, so the raster cache must see them
    pn.win.context.hash(&mut h);
    current_ctx_id(pn.active_path, pn.project).hash(&mut h);
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
    pn.cmd.edit.hash(&mut h);
    h.finish()
}

/// The slot's current contour (for highlighting in the half-sketcher). 0 means the first suitable
/// contour of the sketch, chosen automatically.
pub fn slot_current_cid(loft: &LoftParams, project: &Project, sweep: SweepParams, slot: ContourSlot) -> Id {
    let resolve = |sid: Id, cid: Id, path: bool| -> Id {
        if cid != 0 {
            return cid;
        }
        let cands = if path { project.sweep_path_contours(sid) } else { project.sweep_profile_contours(sid) };
        cands.first().copied().unwrap_or(0)
    };
    match slot {
        ContourSlot::SweepProfile => resolve(sweep.prof_sid, sweep.prof_cid, false),
        ContourSlot::SweepPath => resolve(sweep.path_sid, sweep.path_cid, true),
        ContourSlot::LoftSection(i) => {
            let sid = loft.sids.get(i).copied().unwrap_or(0);
            resolve(sid, loft.cids.get(i).copied().unwrap_or(0), false)
        }
    }
}

/// The origin of a body's gizmo (the centre of the mesh's bounding box, in the Part's display frame,
/// which is the local one) and the length of an axis (about 60 px). During a drag the origin is fixed
/// (`body_giz_drag.1`); otherwise it is the current centre of the box.
pub fn body_gizmo_geometry(body_giz: &BodyGizmo, cam: Cam3, project: &Project, set: &Settings, mi: usize) -> ([f64; 3], f64) {
    let o = if let Some((dmi, org, _)) = body_giz.drag {
        if dmi == mi {
            // THE GIZMO FOLLOWS the body during a drag (the preview moves the body by the same
            // accumulator). For a move the origin travels along; for a rotation
            // apply12(rot_about_org, org) = org, so the centre stays put. Otherwise the gizmo would
            // stand still while the body moved, and would jump to the body on release.
            match body_giz_accum(body_giz, set, body_giz.snap) {
                Some(acc) => qymcad_core::feature::apply12(&acc, org),
                None => org,
            }
        } else {
            mesh_center(project, mi)
        }
    } else {
        mesh_center(project, mi)
    };
    (o, 60.0 / cam.scale as f64)
}

/// The gizmo's origin (in the display frame, that is, the component's transform) and the length of an
/// axis (about 60 px). During a drag the origin is FIXED, as it is for a body, so the axis projection
/// does not float along with the component.
pub fn gizmo_geometry(cam: Cam3, comp_giz: CompGizmo, project: &Project, comp: Id) -> ([f64; 3], f64) {
    let o = match comp_giz.drag {
        Some((dc, _, origin, _)) if dc == comp => origin,
        _ => {
            let t = project.component_transform(comp);
            [t[3], t[7], t[11]]
        }
    };
    (o, 60.0 / cam.scale as f64)
}

/// The component the placement gizmo applies to: a component selected in the tree whose parent is the
/// current context (so its transform is the display frame), in the Assembly workbench, and not the root.
pub fn gizmo_component(active_path: &[Id], project: &Project, sel: Sel, workbench: Workbench) -> Option<Id> {
    if !matches!(workbench, Workbench::Assembly) {
        return None;
    }
    if let Sel::Component(ci) = sel {
        let c = project.components.get(ci)?;
        if c.id != project.root && c.parent == Some(current_ctx_id(active_path, project)) {
            return Some(c.id);
        }
    }
    None
}

/// A `PatternKind` built from the parameter fields (for a circular one the centre is the point picked,
/// or the centroid of the selection).
pub fn current_pattern_kind(armed: &Armed, pat: PatternTool, project: &Project, sk_pat: SketchPattern, si: usize, eids: &[Id]) -> qymcad_core::model::PatternKind {
    use qymcad_core::model::PatternKind;
    if armed.pat_op() == 2 {
        // the centre in order of priority: the one explicitly clicked, then the stored one (while editing), then the selection's centroid
        let (cx, cy) = if let Some(c) = pat.center {
            (c.x, c.y)
        } else if let Some(PatternKind::Circular { cx, cy, .. }) = pat.edit.and_then(|pi| project.sketches.get(si).and_then(|s| s.patterns.get(pi)).map(|p| p.kind)) {
            (cx, cy)
        } else {
            project.entities_centroid(si, eids)
        };
        PatternKind::Circular { cx, cy, count: sk_pat.count, total_deg: sk_pat.angle }
    } else {
        PatternKind::Linear { dx: sk_pat.dx, dy: sk_pat.dy, count: sk_pat.count, dx2: sk_pat.dx2, dy2: sk_pat.dy2, count2: sk_pat.count2 }
    }
}

/// A PROPER clip of a triangle by the section plane (Sutherland-Hodgman): the visible side (dist <= 0)
/// is cut EXACTLY along the plane, giving 0 to 2 triangles. With no section, the original triangle
/// comes back. A resulting vertex is (position, source weights [w0, w1, w2]) — for interpolating the
/// colour on the GPU.
pub fn section_clip_tri(section: &SectionTool, v: [[f64; 3]; 3]) -> smallvec_tris::ClipTris {
    use smallvec_tris::*;
    let Some((o, n)) = section_eff(section) else {
        return ClipTris::whole();
    };
    let d = [
        (v[0][0] - o[0]) * n[0] + (v[0][1] - o[1]) * n[1] + (v[0][2] - o[2]) * n[2],
        (v[1][0] - o[0]) * n[0] + (v[1][1] - o[1]) * n[1] + (v[1][2] - o[2]) * n[2],
        (v[2][0] - o[0]) * n[0] + (v[2][1] - o[1]) * n[1] + (v[2][2] - o[2]) * n[2],
    ];
    clip_by_dists(v, d)
}

/// The axis of the active sketch-based command (extrude, cut and so on): the centroid of the
/// profiles in world space, the normal, and the length.
pub fn feat_cmd_axis(cmd: &FeatCommand, gsel: &GeomSelection, project: &Project) -> Option<([f64; 3], [f64; 3], f64)> {
    let si = cmd.sketch?;
    let f = project.sketch_frame(si)?;
    let mut polys: Vec<Vec<f64>> = Vec::new();
    for cid in &gsel.profiles {
        if let Some(xy) = project.contour_profile_xy(*cid) {
            polys.push(xy);
        }
    }
    if polys.is_empty() {
        if let Some((_, xy)) = project.sketch_profile_xy(si) {
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
    Some(([base.x, base.y, base.z], f.normal(), cmd_val(cmd, "height")))
}

/// ONE ARROW OF A COMMAND AT ITS PROFILE, as it is drawn and as it is grabbed: one geometry for both, so the handle is
/// where the eye sees it.
pub struct CmdArrow {
    pub base: [f64; 3],
    /// The unit direction the arrow points in - the one the body grows in.
    pub dir: [f64; 3],
    pub len: f64,
    /// The field of the command it drives.
    pub key: &'static str,
}

impl CmdArrow {
    pub fn tip(&self) -> [f64; 3] {
        [self.base[0] + self.dir[0] * self.len, self.base[1] + self.dir[1] * self.len, self.base[2] + self.dir[2] * self.len]
    }
}

/// THE ARROWS OF THE COMMAND IN HAND: the first side along the direction in force (a flip turns it against the sketch's
/// normal), and with two sides a second arrow the other way, driving the second value. Reported behaviour: after a
/// flip the arrow could not be grabbed - it was drawn turned and grabbed where it stood before the flip - and two
/// sides showed no second arrow.
pub fn feat_cmd_arrows(cmd: &FeatCommand, gsel: &GeomSelection, project: &Project, flip: bool) -> Vec<CmdArrow> {
    let Some((base, n, h)) = feat_cmd_axis(cmd, gsel, project) else { return Vec::new() };
    let dir = if flip { [-n[0], -n[1], -n[2]] } else { n };
    let mut out = vec![CmdArrow { base, dir, len: h, key: "height" }];
    if cmd.extent.two_sided() {
        out.push(CmdArrow { base, dir: [-dir[0], -dir[1], -dir[2]], len: cmd_val(cmd, "down"), key: "down" });
    }
    out
}

/// A PRESS ON AN ARROW OF THE COMMAND takes it: the one whose tip is within 14 px of the pointer, the nearest first.
/// The field of a value lets go of the keyboard, so what was typed in it is taken as it stands and the arrow drives
/// the value from there: a drag is not a click, and the field kept the caret and wrote its own text back over the
/// arrow on every frame. Answers whether an arrow was taken.
pub fn grab_cmd_arrow(ctx: &egui::Context, cmd: &mut FeatCommand, gsel: &GeomSelection, project: &Project, flip: bool, scr: &Screen, at: Pos2) -> bool {
    let arrows = feat_cmd_arrows(cmd, gsel, project, flip);
    let hit = arrows.iter().map(|a| (a.key, scr.at(a.tip()).0.distance(at))).filter(|(_, d)| *d <= 14.0).min_by(|a, b| a.1.total_cmp(&b.1));
    cmd.drag = hit.map(|(k, _)| k);
    if cmd.drag.is_some() {
        ctx.memory_mut(|m| m.stop_text_input());
    }
    cmd.drag.is_some()
}

/// A PRESS ON THE ARROW AT A FACE takes the value of field `key` as it stands, and the field lets go of the keyboard -
/// for the same reason as with [`grab_cmd_arrow`]: what was typed there wrote itself back over the arrow every frame.
pub fn take_arrow_value(ctx: &egui::Context, cmd: &FeatCommand, key: &str) -> f64 {
    ctx.memory_mut(|m| m.stop_text_input());
    cmd_val(cmd, key)
}

/// THE ARROW TAKEN FOLLOWS THE POINTER: the pointer is projected onto the arrow's line on the screen. The first side
/// pulls both ways - back past the profile it turns the direction (flip) and the value is the distance; the second
/// side only lengthens and shortens.
pub fn drag_cmd_arrow(cmd: &mut FeatCommand, gsel: &GeomSelection, project: &Project, feat: &mut FeatTarget, scr: &Screen, cur: Pos2) {
    let Some(key) = cmd.drag else { return };
    let Some((base, n, _)) = feat_cmd_axis(cmd, gsel, project) else { return };
    let dir = if key == "down" {
        if feat.flip {
            n
        } else {
            [-n[0], -n[1], -n[2]]
        }
    } else {
        n
    };
    let (s0, s1) = (scr.at(base).0, scr.at([base[0] + dir[0], base[1] + dir[1], base[2] + dir[2]]).0);
    let pd = s1 - s0;
    let denom = (pd.x * pd.x + pd.y * pd.y) as f64;
    if denom <= 1e-6 {
        return;
    }
    let t = ((cur.x - s0.x) * pd.x + (cur.y - s0.y) * pd.y) as f64 / denom;
    if key == "height" {
        feat.set_flip(t < 0.0);
    }
    if let Some(p) = cmd.params.iter_mut().find(|p| p.key == key) {
        p.val = if key == "height" { t.abs() } else { t }.max(0.1);
        p.txt = format!("{:.2}", p.val);
    }
}

/// The index of the sketch that defines the viewport's 2D projection: while editing it is the open
/// sketch, otherwise (in the half-sketcher of a Part command) it is the command's profile sketch, so
/// that the body is projected IN THE PLANE of the drawing rather than seen from above.
pub fn active_2d_sketch(armed: &Armed, cmd: &FeatCommand, project: &Project, sel: Sel, sketch_ses: SketchSession) -> Option<usize> {
    if let Some(si) = edit_si(project, &sketch_ses) {
        return Some(si);
    }
    if armed.commanding() {
        if let Some(si) = cmd.sketch {
            return Some(si);
        }
        if let Sel::Sketch(si) = sel {
            return Some(si);
        }
    }
    None
}

/// The body under the gizmo in a Part: the selected body of the active part (`Sel::Mesh`, or
/// `Sel::Feature` carrying a body). Returns (the body's Id, the mesh index). Only in the Part workbench.
pub fn body_gizmo_target(bv: BodyView, sel: Sel) -> Option<(Id, usize)> {
    // A PART IS ONE BODY, so the Part workbench does NOT offer a gizmo for an INDIVIDUAL body. Dragging
    // a body would bake in a Move feature and tear the body away from its sketch. A part's body is moved
    // in an assembly by the COMPONENT gizmo (its placement). The body gizmo is kept ONLY for a "free"
    // imported body (an STL with no owning component) — that one cannot be moved by a component gizmo.
    if bv.armed.commanding() {
        return None; // during a command (extrude, cut and so on) the body gizmo stays out of the way
    }
    let body = visible_lineage_body(bv.project, selected_body(bv.project, &sel)?);
    // a body that BELONGS to a part (it has an owner) gets NO body gizmo, neither in the part nor in the
    // assembly: a part is a body and is moved by its component. The body gizmo is only for an imported
    // mesh with no owner.
    if bv.project.body_owner(body).is_some() {
        return None;
    }
    let mi = bv.project.mesh_index(body)?;
    body_shown(bv, mi).then_some((body, mi))
}

/// The frame's projection parameters: `(inv_d_eye, z_near, z_far, depth_half)`. Perspective takes TIGHT
/// near and far planes from the scene's bounding box (the most z-buffer precision, so thin features do
/// not z-fight); orthographic takes a linear depth from `depth_half`. `inv_d_eye = 0` means
/// orthographic. The single source for the CPU (`depth_ndc`) and for the GPU (`CamRaw`).
pub fn proj_params(pn: &Painting, rect: Rect, scene_key: u64) -> (f64, f64, f64, f64) {
    let half_w = (rect.width() * 0.5) as f64;
    let half_h = (rect.height() * 0.5) as f64;
    let scale = (pn.cam.scale as f64).max(1e-4);
    let depth_half = (half_w.max(half_h) / scale) * 50.0 + 1000.0; // the orthographic range (as in `CamRaw::new`)
    let inv_d = persp_inv_d_eye(&pn.cam, pn.set, rect.height() * 0.5);
    if inv_d <= 0.0 {
        return (0.0, 0.0, 0.0, depth_half);
    }
    let d_eye = 1.0 / inv_d;
    let (mut z_near, mut z_far) = (d_eye * 0.05, d_eye * 4.0); // the fallback when the scene is empty
    if let Some(Sphere { centre: c, radius: r }) = scene_sphere_cached(pn, scene_key) {
        let (_, _, fwd) = pn.cam.basis();
        let dc = v_dot(v_sub(c, pn.cam.target), fwd); // the world depth of the sphere's centre along the line of sight
        let margin = (r * 0.05).max(1e-3);
        z_near = (dc - r + d_eye - margin).max(d_eye * 0.02); // clamped above 0 (a point in front of the eye)
        z_far = (dc + r + d_eye + margin).max(z_near + d_eye * 0.01);
    }
    (inv_d, z_near, z_far, depth_half)
}

pub fn face_arrow_geometry(pn: &Painting) -> Option<([f64; 3], [f64; 3], [f64; 3])> {
    let key = face_arrow_key(pn.armed)?;
    let (o, n) = cmd_arrow_ref(pn)?;
    let d = cmd_val(pn.cmd, key);
    // AT ZERO THE LENGTH COMES FROM THE SIZE OF THE PART. Otherwise a zero value leaves the handle
    // invisible, and there is nothing to drag at exactly the moment one needs to.
    let span = op_target_body(&DrawCtx { cam: &pn.cam, set: pn.set, scheme: pn.scheme, project: pn.project, active_path: pn.active_path }, pn.sel)
        .or(pn.gsel.faces_body)
        .and_then(|b| pn.project.mesh_index(b))
        .and_then(|mi| pn.project.bodies[mi].mesh.bounds())
        .map(|b| (b.max.x - b.min.x).abs().max(1.0) * 0.25)
        .unwrap_or(5.0);
    let l = if d.abs() > 1e-6 { d } else { span };
    Some((o, [o[0] + n[0] * l, o[1] + n[1] * l, o[2] + n[2] * l], n))
}

/// The geometry of the section GIZMO: the centre of the quad on the plane, u, v, the half-size, and the arrow's tip.
/// THE SECTION'S HANDLE, as it is drawn: a square lying in the cutting plane with an arrow out of it.
///
/// It was `Option<([f64; 3], [f64; 3], [f64; 3], f64, [f64; 3])>` - three unnamed triples, a number and a
/// fourth triple. One caller reads it as `(_, _, _, _, tip)`.
pub struct SectionGizmo {
    /// The scene's centre projected onto the cutting plane - where the handle sits.
    pub centre: [f64; 3],
    /// The two axes of the square, in the plane.
    pub u: [f64; 3],
    pub v: [f64; 3],
    /// Half the square's side, from the scene's diagonal.
    pub half: f64,
    /// The end of the arrow along the normal - what is grabbed to move the plane.
    pub tip: [f64; 3],
}

pub fn section_gizmo_geom(pn: &Painting) -> Option<SectionGizmo> {
    let (o, n) = section_eff(pn.section)?;
    // the bounding box of the visible scene, in world space
    let mut lo = [f64::MAX; 3];
    let mut hi = [f64::MIN; 3];
    for SceneMesh { mesh, world: wt, .. } in visible_mesh_items(pn) {
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
    Some(SectionGizmo { centre: cp, u, v, half, tip })
}

/// The live body of a lineage — a delegate to the core (`Project::live_body`). The FORWARD walk lives
/// there in a single definition: a copy in the interface layer would drift from the core silently.
pub fn visible_lineage_body(project: &qymcad_core::model::Project, body: Id) -> Id {
    project.live_body(body)
}

/// The centre of mesh `mi`'s bounding box (world, or the Part's local frame). [0, 0, 0] when the mesh is empty.
pub fn mesh_center(project: &qymcad_core::model::Project, mi: usize) -> [f64; 3] {
    project.bodies.get(mi).and_then(|b| b.mesh.bounds()).map(|b| [(b.min.x + b.max.x) / 2.0, (b.min.y + b.max.y) / 2.0, (b.min.z + b.max.z) / 2.0]).unwrap_or([0.0, 0.0, 0.0])
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
pub fn face_arrow_key(armed: &Armed) -> Option<&'static str> {
    match armed.cmd_kind() {
        25 => Some("dist"),
        6 | 28 => Some("thickness"),
        27 => Some("offset"),
        _ => None,
    }
}

pub fn current_body(dc: &DrawCtx) -> Option<Id> {
    // THE CURRENT part: the last unconsumed body of THIS context (not the globally last one — otherwise
    // a cut in one part would catch hold of another part's body, and the second part appeared to
    // vanish). The logic lives in the core, under test.
    // A FALLBACK ON THE DOCUMENT'S ACTIVE COMPONENT: the GUI navigation (`active_path`) may stand at the
    // root when the part was never "entered" by a double click — the commands must still see the active
    // part's body, otherwise the part looks empty and new bodies start multiplying.
    dc.project.active_body(current_ctx_id(dc.active_path, dc.project)).or_else(|| dc.project.active_body(dc.project.current_ctx()))
}

/// A loop of corners round a hole of a body made of a mesh that did not close, in the body's own coordinates.
#[derive(Clone, Debug, PartialEq)]
pub struct OpenBorder {
    pub corners: Vec<[f64; 3]>,
}

/// A sphere in world coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sphere {
    pub centre: [f64; 3],
    pub radius: f64,
}

/// A box in world coordinates, along the axes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldBox {
    pub min: [f64; 3],
    pub max: [f64; 3],
}

/// The world bounding sphere of the visible scene, taken from `visible_mesh_items`
/// (exactly what gets drawn). Cached by the scene key: recomputed only when the scene changes, not
/// every frame.
pub fn scene_sphere_cached(pn: &Painting, scene_key: u64) -> Option<Sphere> {
    let cached = pn.cache.bounds.get();
    if cached.rev == scene_key {
        return cached.value;
    }
    let (mut mn, mut mx) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
    let mut any = false;
    for SceneMesh { mesh, world: wt, .. } in visible_mesh_items(pn) {
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
        Sphere { centre: c, radius: r }
    });
    pn.cache.bounds.set(Cached { rev: scene_key, value: sphere });
    sphere
}

/// WHERE THE HANDLE STARTS AND WHERE IT POINTS: the point it acts on and a unit direction.
///
/// For face commands that is the centroid of the selected face and its normal; for splits it is the
/// origin and normal of the chosen plane. Everything after that is shared: the drawing, the grabbing
/// and the dragging.
pub fn cmd_arrow_ref(pn: &Painting) -> Option<([f64; 3], [f64; 3])> {
    match pn.armed.cmd_kind() {
        6 | 25 | 28 => {
            let mi = pn.project.mesh_index(pn.gsel.faces_body?)?;
            let f = pn.project.bodies.get(mi)?.faces.iter().find(|f| pn.gsel.faces.contains(&f.id))?;
            Some(([f.centroid.x, f.centroid.y, f.centroid.z], f.normal))
        }
        27 => pn.split.plane.as_ref().and_then(|sp| mirror_plane_world(&DrawCtx { cam: &pn.cam, set: pn.set, scheme: pn.scheme, project: pn.project, active_path: pn.active_path }, sp)),
        _ => None,
    }
}

/// The DOF gizmo handles of a joint in the frame of the context: (the origin, [(slot, whether it is a
/// ring, the direction axis)]). Parameters driven by an expression (`feat_dim`) are NOT free to drag and
/// are left out.
/// THE HANDLES OF A MATE'S GIZMO: where it stands and what can be grabbed on it.
///
/// It was `Option<([f64; 3], Vec<(u8, bool, [f64; 3])>)>`, and inside the inner triple neither the code
/// of the handle nor the flag had a name at the point of use.
pub struct JointGizmo {
    /// The mate's origin in world space.
    pub origin: [f64; 3],
    /// The handles: what each one drives, whether it is a ring or an arrow, and which way it goes.
    pub handles: Vec<JointHandle>,
}

/// One grabbable handle of a mate's gizmo.
pub struct JointHandle {
    /// Which degree it drives - the kernel's own code.
    pub slot: u8,
    /// A RING (a rotation) rather than an arrow (a translation). The two are drawn and grabbed
    /// differently, and the callers filter on exactly this.
    pub ring: bool,
    /// The direction of the degree, asked of the core rather than derived from the joint frame.
    pub dir: [f64; 3],
}

/// A SWEEP AS IT IS PREVIEWED: the path it runs along and the profile placed at each step.
///
/// It was `Option<(Vec<[f64; 3]>, Vec<Vec<[f64; 3]>>)>` - a list of points and a list of lists of points,
/// told apart only by their nesting.
pub struct SweepPreview {
    /// The path, as points in world space.
    pub path: Vec<[f64; 3]>,
    /// The profile at each step along it.
    pub sections: Vec<Vec<[f64; 3]>>,
}

pub fn joint_giz_handles(dc: &DrawCtx, jid: Id) -> Option<JointGizmo> {
    let kind = dc.project.joints.iter().find(|x| x.id == jid)?.kind;
    let m = dc.project.joint_frame(jid, current_ctx_id(dc.active_path, dc.project))?;
    let o = [m[3], m[7], m[11]];
    let key = |slot: u8| match slot {
        0 => "angle",
        1 => "offset",
        _ => "offset2",
    };
    let mut hs = Vec::new();
    for (slot, ring, _ax) in joint_slot_geom(kind) {
        if dc.project.feat_dim(jid, key(slot)).is_some_and(|e| !e.trim().is_empty()) {
            continue; // an expression drives this parameter - dragging must not touch it
        }
        // THE DIRECTION OF A DEGREE IS ASKED OF THE CORE rather than derived from the joint frame. The
        // frame is built on the FIRST anchor, while for a pin-slot the travel belongs to the SECOND: the
        // arrow pointed one way and the part moved another. There must be one source of direction - the
        // same one the solver computes by.
        let Some(dir) = dc.project.joint_slot_axis(jid, slot as usize, current_ctx_id(dc.active_path, dc.project)) else { continue };
        hs.push((slot, ring, dir));
    }
    Some(JointGizmo { origin: o, handles: hs.into_iter().map(|(slot, ring, dir)| JointHandle { slot, ring, dir }).collect() })
}

/// The screen points of connectors A and B of a joint in the active context (world coordinates through
/// `world_transform`).
pub fn joint_endpoints(dc: &DrawCtx, j: &qymcad_core::feature::Joint, rect: Rect, basis: &([f64; 3], [f64; 3], [f64; 3])) -> Option<(Pos2, Pos2)> {
    let ctx = current_ctx_id(dc.active_path, dc.project);
    let pt = |cid: Id| -> Option<Pos2> {
        let conn = dc.project.connector(cid)?;
        let fr = dc.project.connector_frame(conn)?;
        let w = qymcad_core::feature::apply12(&dc.project.relative_transform(conn.owner, ctx), fr.origin);
        Some(Screen { cam: dc.cam, set: dc.set, rect, basis }.at(w).0)
    };
    Some((pt(j.a)?, pt(j.b)?))
}

/// Whether a joint is visible in the current view: the joints tick box is on, the workbench is Assembly,
/// and the joint's home is the active context (the joints of nested subassemblies are not shown in the
/// parent - they get in the way).
pub fn joint_visible(active_path: &[Id], project: &Project, set: &Settings, workbench: Workbench, j: &qymcad_core::feature::Joint) -> bool {
    set.show_joints && matches!(workbench, Workbench::Assembly) && project.joint_in_context(j, current_ctx_id(active_path, project))
}

/// The geometry of the DOF gizmo handles for a kind of joint: (slot [0=angle, 1=offset, 2=offset2],
/// whether it is a ring, the frame axis 0/1/2). It matches `JointKind::motion`: the angle turns about Z;
/// translations and extra angles follow the axes, as in `motion`.
pub fn joint_slot_geom(kind: qymcad_core::feature::JointKind) -> Vec<(u8, bool, u8)> {
    use qymcad_core::feature::JointKind::*;
    match kind {
        Rigid => vec![],
        Revolute => vec![(0, true, 2)],                             // an angle about Z
        Slider => vec![(1, false, 2)],                              // a shift along Z
        Cylindrical => vec![(0, true, 2), (1, false, 2)],           // an angle and a shift along Z
        PinSlot => vec![(0, true, 2), (1, false, 0)],               // an angle about Z plus a shift along X
        Planar => vec![(0, true, 2), (1, false, 0), (2, false, 1)], // an angle about Z plus shifts along X and Y
        Ball => vec![(0, true, 2), (1, true, 0), (2, true, 1)],     // angles about Z, X and Y
        // parallelism has no handles: it holds a direction, not a value - there is nothing to drag
        Parallel => vec![],
    }
}

/// ALL of the sketch diagnostics in one pass and THROUGH A CACHE (recomputed only when the imprint
/// changes).
///
/// Every piece of it is a full Jacobian plus a Gaussian elimination (O(m*nv^2)). Only the degrees of
/// freedom and the free points used to be cached, while the conflicting set and the redundant
/// constraints were computed DIRECTLY and several times per frame: the panel, the list of constraints,
/// the dimension overlay, the glyphs and the model tree - five independent runs over the same sketch.
/// Now there is one source.
pub fn sketch_diag(cache: &Caches, project: &Project, si: usize) -> SketchDiag {
    let (shape, place) = (sketch_shape_key(project, si), sketch_place_key(project, si));
    if let Some(st) = &*cache.sk_status.borrow() {
        let dragged = cache.sk_dragged.get() == Some(si);
        if st.si == si && st.shape == shape && (dragged || st.place == place) {
            return st.diag.clone();
        }
    }
    // the degrees of freedom, the free points and the redundant from one elimination a part (`sketch_checks`)
    let checks = project.sketch_checks(si);
    let d = SketchDiag { dof: checks.dof, free: checks.free, conflicts: project.sketch_conflicts(si).into_iter().collect(), redundant: checks.redundant.into_iter().collect() };
    *cache.sk_status.borrow_mut() = Some(SketchStatus { si, shape, place, diag: d.clone() });
    d
}

/// WHICH REDUNDANT CONSTRAINTS TO MARK - one place for the list of constraints AND for the glyphs on the
/// canvas.
///
/// The rule used to be written down only in the list, while the glyphs on the canvas were painted from a
/// raw `diag.redundant`. The divergence looked like this: the slot is clean in the list, while orange
/// "redundant constraint" glyphs burn on its tangencies in the sketch. It was caught on a picture for the
/// help - on screen it had been visible for years to anyone who drew a slot.
///
/// WHAT IS EXCLUDED AND WHY:
/// * **dimensions** - consistent redundancy among dimensions is harmless (a reference dimension is not an
///   error);
/// * **tangencies** (`Tangent`/`CircleTangent`) - their Jacobian at the point of contact is PARALLEL to
///   the intrinsic of the arc, both measure the radius in one direction, and the rank analysis marks them
///   falsely. A tangency is the structural constraint of a fillet, a slot, a rounded rectangle;
///   professional CAD does not mark them red;
/// * **a virtual corner** - a `PointOnLine` on a point that belongs to no entity (the sharp corner under
///   a fillet, kept for the dimensions): the same lie of the rank;
/// * **everything geometric while the sketch contains tangencies** - the rank lies not only about the
///   tangencies themselves but about the constraints entangled with them (the horizontals and verticals
///   of a rectangle).
///
/// This hides no real contradictions: those are caught by `sketch_conflicts`, which counts by the
/// geometry.
pub fn flagged_redundant(cache: &Caches, project: &qymcad_core::model::Project, si: usize) -> std::collections::HashSet<usize> {
    use qymcad_core::model::Constraint;
    let diag = sketch_diag(cache, project, si);
    let Some(s) = project.sketches.get(si) else { return Default::default() };
    let tangency = |c: &Constraint| matches!(c, Constraint::Tangent { .. } | Constraint::CircleTangent { .. });
    // nothing redundant, nothing to mark - before the points of every entity are gathered: 7 ms a call on 70 000
    // segments, called by the list and by the canvas every frame
    if diag.redundant.is_empty() || s.constraints.iter().any(tangency) {
        return Default::default();
    }
    let entity_pts: std::collections::HashSet<Id> = s
        .entities
        .iter()
        .flat_map(|e| match e.kind {
            qymcad_core::model::EntityKind::Line { a, b } => vec![a, b],
            qymcad_core::model::EntityKind::Arc { center, a, b, .. } => vec![center, a, b],
            qymcad_core::model::EntityKind::Circle { center, .. } => vec![center],
            qymcad_core::model::EntityKind::Ellipse { c, ma, mi } => vec![c, ma, mi],
        })
        .collect();
    diag.redundant
        .iter()
        .copied()
        .filter(|ci| {
            let Some(c) = s.constraints.get(*ci) else { return false };
            let is_dim = matches!(
                c,
                Constraint::Distance { .. }
                    | Constraint::Angle { .. }
                    | Constraint::DistancePL { .. }
                    | Constraint::AngleLines { .. }
                    | Constraint::ArcLength { .. }
                    | Constraint::Diameter { .. }
                    | Constraint::EdgeDistance { .. }
            );
            let virtual_corner = matches!(c, Constraint::PointOnLine { p, .. } if !entity_pts.contains(p));
            !is_dim && !tangency(c) && !virtual_corner
        })
        .collect()
}

/// A QUICK HASH FOR THE KEYS OF THE STATUS CACHE: each word turned and multiplied in. The keys are taken several times
/// a frame (the panel, the points, the glyphs, the pick, the dimensions), over every point, entity and constraint of
/// the sketch; the hasher of a map, built to stand against an attacker, took most of a frame on 70 000 segments. A
/// key of a cache needs no such standing.
#[derive(Default)]
struct Mix(u64);

impl std::hash::Hasher for Mix {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write_u64(&mut self, x: u64) {
        self.0 = (self.0.rotate_left(5) ^ x).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    }

    fn write(&mut self, bytes: &[u8]) {
        for chunk in bytes.chunks(8) {
            let mut word = [0u8; 8];
            word[..chunk.len()].copy_from_slice(chunk);
            self.write_u64(u64::from_le_bytes(word));
        }
    }
}

/// THE SHAPE OF A SKETCH, for the status cache: its points, entities, splines and constraints by what they are and
/// what they tie, and the values of its dimensions - everything the diagnostics depend on but where the points stand.
/// O(n) per frame is pennies against the Jacobian.
pub fn sketch_shape_key(project: &qymcad_core::model::Project, si: usize) -> u64 {
    use qymcad_core::model::EntityKind;
    use std::hash::{Hash, Hasher};
    let mut h = Mix::default();
    if let Some(s) = project.sketches.get(si) {
        for p in &s.points {
            p.id.hash(&mut h);
        }
        for e in &s.entities {
            e.id.hash(&mut h);
            e.construction.hash(&mut h);
            match e.kind {
                EntityKind::Line { a, b } => [0, a, b, 0].hash(&mut h),
                EntityKind::Arc { center, a, b, ccw } => [1, center, a, b, ccw as u64].hash(&mut h),
                EntityKind::Circle { center, .. } => [2, center].hash(&mut h),
                EntityKind::Ellipse { c, ma, mi } => [3, c, ma, mi].hash(&mut h),
            }
        }
        s.splines.len().hash(&mut h);
        for c in &s.constraints {
            std::mem::discriminant(c).hash(&mut h);
            c.points().hash(&mut h);
            c.is_driven().hash(&mut h);
            // the values of the dimensions (a number edited without the points moving - before solving)
            if let Some(d) = c.dim_value() {
                d.to_bits().hash(&mut h);
            }
        }
    }
    h.finish()
}

/// WHERE THE POINTS OF A SKETCH STAND AND HOW BIG ITS CIRCLES ARE, for the status cache: any move of a point or a
/// rim changes it.
pub fn sketch_place_key(project: &qymcad_core::model::Project, si: usize) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = Mix::default();
    if let Some(s) = project.sketches.get(si) {
        for p in &s.points {
            p.x.to_bits().hash(&mut h);
            p.y.to_bits().hash(&mut h);
        }
        for e in &s.entities {
            if let qymcad_core::model::EntityKind::Circle { r, .. } = e.kind {
                r.to_bits().hash(&mut h);
            }
        }
    }
    h.finish()
}

/// Whether a sketch (by id) is shown in the visibility context.
pub fn sketch_in_ctx(active_path: &[Id], project: &qymcad_core::model::Project, sketch_ses: SketchSession, sketch_id: Id) -> bool {
    // NESTED ONES BELONG. The rule used to be strict equality, and in the root of an assembly EVERY
    // sketch counted as someone else's: they belong to the parts, not to the root. Because of that the
    // contours tick box in an assembly showed nothing at all - reported as there being no contours in 3D
    // however much they were clicked.
    //
    // Bodies count differently (`component_is_within`, see `body_shown`), which is why the bodies of
    // nested parts are visible in an assembly. Sketches must follow the same rule: one's own means the
    // context AND its descendants. Neighbours (another branch of the tree) stay foreign, and the
    // isolation does not suffer from this.
    let ctx = viz_ctx_id(active_path, project, sketch_ses);
    match project.sketch_owner(sketch_id) {
        Some(owner) => owner == ctx || project.component_is_within(owner, ctx),
        None => false,
    }
}

/// Font-independent glyphs for constraints, dimensions and tools (drawn as lines).
#[derive(Clone, Copy, PartialEq)]
pub enum Gly {
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

/// The gizmo mode of the component selected in an assembly.
pub enum CompGizmoMode {
    None,      // grounded, or fully pinned by a joint: there is no gizmo
    Free,      // free of joints: the plain 6-DOF gizmo
    Joint(Id), // driven by a joint: the degree-of-freedom gizmo, offering only that joint's freedoms
}

/// The value of the parameter with snapping applied (the degree or millimetre step from the panel).
pub fn joint_giz_value(joint: &JointCommand, set: &Settings, snap: bool) -> Option<(f64, bool)> {
    let dg = joint.giz_drag?;
    let step = if dg.ring { set.snap.rot_deg.max(0.1) } else { set.snap.grid.max(0.01) };
    let amt = if snap { (dg.amt / step).round() * step } else { dg.amt };
    Some((dg.start + amt, dg.ring))
}

/// The live wireframe preview of the active command (the prism of the profile along its extent) plus the
/// arrow gizmo of the length.
/// The geometry of the LIVE SWEEP PREVIEW: the contour of the profile carried along the path with a
/// parallel-transported frame (the same idea as the corrected Frenet frame in the core: the minimal
/// rotation between tangents, with no false twist). Returns the polyline of the path in world
/// coordinates and the section loops at the stations along it. It agrees with the core at the start and
/// the finish (the same frame perpendicular to the tangent); the middle is a stable approximation.
pub fn sweep_preview(project: &Project, sweep: SweepParams) -> Option<SweepPreview> {
    let (prof_sid, path_sid) = (sweep.prof_sid, sweep.path_sid);
    if prof_sid == 0 || path_sid == 0 {
        return None;
    }
    let prof_cid = if sweep.prof_cid != 0 { sweep.prof_cid } else { *project.sweep_profile_contours(prof_sid).first()? };
    let path_cid = if sweep.path_cid != 0 { sweep.path_cid } else { *project.sweep_path_contours(path_sid).first()? };
    let pxy = project.contour_profile_xy(prof_cid)?; // the flat contour of the profile, closed
    let np = pxy.len() / 2;
    if np < 3 {
        return None;
    }
    let pf = project.sketch_frame_by_id(path_sid)?;
    let pidx = project.contour_index(path_cid)?;
    let pts2 = &project.contours[pidx].points;
    if pts2.len() < 2 {
        return None;
    }
    let path: Vec<[f64; 3]> = pts2
        .iter()
        .map(|p| {
            let w = pf.lift(*p);
            [w.x, w.y, w.z]
        })
        .collect();
    let n = path.len();
    let sub = |a: [f64; 3], b: [f64; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    let add = |a: [f64; 3], b: [f64; 3]| [a[0] + b[0], a[1] + b[1], a[2] + b[2]];
    let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let cross = |a: [f64; 3], b: [f64; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let norm = |v: [f64; 3]| {
        let l = dot(v, v).sqrt();
        if l < 1e-9 {
            [0.0, 0.0, 1.0]
        } else {
            [v[0] / l, v[1] / l, v[2] / l]
        }
    };
    let tangent = |i: usize| -> [f64; 3] {
        if i == 0 {
            norm(sub(path[1], path[0]))
        } else if i == n - 1 {
            norm(sub(path[n - 1], path[n - 2]))
        } else {
            norm(add(norm(sub(path[i], path[i - 1])), norm(sub(path[i + 1], path[i]))))
        }
    };
    // the starting frame (as in the C++ core): refX perpendicular to Z, Y = Z x refX, X = Y x Z
    let z0 = tangent(0);
    let refx = if dot(z0, [1.0, 0.0, 0.0]).abs() > 0.9 { [0.0, 1.0, 0.0] } else { [1.0, 0.0, 0.0] };
    let mut y = norm(cross(z0, refx));
    let mut x = norm(cross(y, z0));
    let mut zprev = z0;
    // rotating vector v about the unit axis a by an angle given as (sin, cos) - Rodrigues
    let rot = |v: [f64; 3], a: [f64; 3], s: f64, c: f64| {
        let av = cross(a, v);
        let ad = dot(a, v) * (1.0 - c);
        [v[0] * c + av[0] * s + a[0] * ad, v[1] * c + av[1] * s + a[1] * ad, v[2] * c + av[2] * s + a[2] * ad]
    };
    let mut sections = Vec::with_capacity(n);
    for (i, org) in path.iter().enumerate() {
        let zi = tangent(i);
        // parallel transport of the frame from zprev to zi (the minimal rotation)
        let axc = cross(zprev, zi);
        let sn = dot(axc, axc).sqrt();
        if sn > 1e-9 {
            let a = [axc[0] / sn, axc[1] / sn, axc[2] / sn];
            let cs = dot(zprev, zi).clamp(-1.0, 1.0);
            x = norm(rot(x, a, sn, cs));
            y = norm(rot(y, a, sn, cs));
        }
        let sec: Vec<[f64; 3]> = (0..np)
            .map(|k| {
                let (px, py) = (pxy[2 * k], pxy[2 * k + 1]);
                [org[0] + px * x[0] + py * y[0], org[1] + px * x[1] + py * y[1], org[2] + px * x[2] + py * y[2]]
            })
            .collect();
        sections.push(sec);
        zprev = zi;
    }
    Some(SweepPreview { path, sections })
}

/// The layout of the command turned into a description of the array.
pub fn comp_array_kind(arr: ArrayParams, carr: &CompArrayCmd, cmd: &FeatCommand) -> qymcad_core::model::CompPatternKind {
    use qymcad_core::model::CompPatternKind;
    let unit = |a: u8| match a {
        1 => [0.0, 1.0, 0.0],
        2 => [0.0, 0.0, 1.0],
        _ => [1.0, 0.0, 0.0],
    };
    let count = arr.count.max(1);
    if carr.mode == 2 {
        let angle = if arr.full { 360.0 } else { cmd_val(cmd, "cangle") };
        // THE AXIS PASSES THROUGH THE ORIGIN OF THE ASSEMBLY: the array has no axis of its own yet, and
        // that is stated honestly - for bolts around a flange the part is placed relative to the origin
        // of the assembly, and the centre sits there too.
        // an axis picked in the view (a datum axis, an edge, a cylindrical face) wins over the axis of the assembly
        CompPatternKind::Circular { origin: [0.0; 3], dir: unit(carr.axis), angle, count, axis: arr.axis }
    } else {
        // the second and third directions as a pattern of bodies has them: each its own count, axis and step
        let second = if arr.two { (unit(arr.dir2), cmd_val(cmd, "cstep2"), arr.count2.max(1)) } else { (unit(1), 0.0, 1) };
        let third = if arr.two && arr.three { (unit(arr.dir3), cmd_val(cmd, "cstep3"), arr.count3.max(1)) } else { (unit(2), 0.0, 1) };
        CompPatternKind::Linear { dir: unit(carr.dir), step: cmd_val(cmd, "cstep"), count, more: [second, third] }
    }
}

/// WHICH WAY THE TOOL GROWS, in the word the model uses.
///
/// The window holds this as two separate switches - the extent mode of the top bar and the flip of the
/// gizmo - while the model holds one word of three. The translation lives here alone, so the preview and
/// the rebuild cannot come to different answers about the same two switches.
pub fn cmd_reach(cmd: &FeatCommand, feat: FeatTarget) -> qymcad_core::feature::Reach {
    use qymcad_core::feature::Reach;
    if cmd.extent.symmetric() {
        Reach::BothWays
    } else if feat.flip {
        Reach::Backward
    } else {
        Reach::Forward
    }
}

/// THE RESULT TEXT — the only place where the numbers turn into a string (the status line and the
/// plate at the geometry say the same thing: two wordings of one measurement drift apart
/// silently).
pub fn measure_text(pn: &Painting) -> String {
    let Some(r) = measure_result(pn.m3) else { return qymcad_i18n::tr("m3-hint-short") };
    let names: Vec<&str> = pn.m3.picks.iter().map(|p| p.what.as_str()).collect();
    let mut parts: Vec<String> = Vec::new();
    if let Some((label, v)) = r.value {
        // the label is a code of the catalogue (`m3-length`) or a sign (`Ø`): translated where it is a code
        parts.push(qymcad_i18n::tr2("m3-value-mm", "label", &qymcad_i18n::name(label), "v", &qymcad_i18n::num(v, 3)));
    }
    if let Some(d) = r.distance {
        parts.push(qymcad_i18n::tr1("m3-distance", "v", &qymcad_i18n::num(d, 3)));
    }
    if let Some(a) = r.angle_deg {
        parts.push(qymcad_i18n::tr1("m3-angle", "v", &qymcad_i18n::num(a, 3)));
    }
    if let Some(d) = r.delta {
        parts.push(format!("Δ {:.3} / {:.3} / {:.3}", d[0], d[1], d[2]));
    }
    if parts.is_empty() {
        // HONESTLY: there is no meaningful number for this pair (converging faces, for one — the
        // distance between them depends on where it is measured). Silence is worse: a person will
        // decide the tool is broken.
        return qymcad_i18n::tr1("m3-not-parallel", "what", &names.join(" - "));
    }
    format!("{}: {}", names.join(" - "), parts.join(" · "))
}

/// IS THE WORKBENCH WAITING FOR GEOMETRY — the question the highlight and the click pass ask.
pub fn assembly_wants_geometry(pn: &Painting) -> bool {
    armed_assembly_tools(pn).into_iter().any(AssemblyTool::wants_geometry)
}

/// The result for what was clicked: one element gives its own size, two give a pair.
pub fn measure_result(m3: &Measure3) -> Option<qymcad_core::measure::MeasureResult> {
    match m3.picks.len() {
        1 => Some(qymcad_core::measure::measure_one(&m3.picks[0].item)),
        2 => Some(qymcad_core::measure::measure_pair(&m3.picks[0].item, &m3.picks[1].item)),
        _ => None,
    }
}

/// WHICH ASSEMBLY TOOLS ARE TAKEN UP RIGHT NOW.
pub fn armed_assembly_tools(pn: &Painting) -> Vec<AssemblyTool> {
    AssemblyTool::ALL.into_iter().filter(|&t| assembly_tool_armed(pn.joint, t)).collect()
}

/// IS THIS PARTICULAR TOOL TAKEN UP. One answer for everyone who asks.
pub fn assembly_tool_armed(joint: &JointCommand, t: AssemblyTool) -> bool {
    match t {
        AssemblyTool::Mate => joint.pick_faces,
        AssemblyTool::Anchor => joint.conn_pick,
        AssemblyTool::Group => joint.group_pick.is_some(),
        AssemblyTool::Width => joint.width_pick.is_some(),
        AssemblyTool::Tangent => joint.tangent_pick.is_some(),
        AssemblyTool::Relation => joint.relation_pick.is_some(),
        AssemblyTool::Ground => joint.ground_pick,
        AssemblyTool::Axis => joint.axis_pick.is_some(),
        AssemblyTool::Repick => joint.edit_repick.is_some(),
    }
}

/// THE TOOLS OF THE ASSEMBLY WORKBENCH — everything that can be "taken up".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssemblyTool {
    /// Assembling a mate: two anchors pointed at.
    Mate,
    /// A standalone connector: one pointing, no joint created.
    Anchor,
    /// A group: clicks on the parts, then Enter.
    Group,
    /// A width: two walls and a tab between them.
    Width,
    /// A tangent: two surfaces, no connectors.
    Tangent,
    /// A relation between degrees: clicks on the JOINTS rather than on geometry.
    Relation,
    /// Ground: a click on a part fixes it.
    Ground,
    /// Pointing at the secondary axis of a connector (the second pick).
    Axis,
    /// Re-picking an anchor of a finished joint.
    Repick,
}

impl AssemblyTool {
    /// EVERY KIND. The single list the checks walk over.
    pub const ALL: [AssemblyTool; 9] = [
        AssemblyTool::Mate,
        AssemblyTool::Anchor,
        AssemblyTool::Group,
        AssemblyTool::Width,
        AssemblyTool::Tangent,
        AssemblyTool::Relation,
        AssemblyTool::Ground,
        AssemblyTool::Axis,
        AssemblyTool::Repick,
    ];

    /// DOES IT ASK FOR GEOMETRY TO BE POINTED AT IN THE FRAME.
    ///
    /// A relation is the only one that does not: the degrees are pointed at by clicking joints in the
    /// list, and they cannot be collected in the viewport.
    pub fn wants_geometry(self) -> bool {
        !matches!(self, AssemblyTool::Relation)
    }

    /// THE MODE FOR THE HELP: F1 finds the article by it (`help_map::ASSEMBLY`).
    ///
    /// With a DOT rather than a hyphen: the keys of the language catalogue are written with hyphens,
    /// and the guard against an internal name reaching the screen catches any literal of that shape.
    pub fn help_mode(self) -> &'static str {
        match self {
            AssemblyTool::Mate => "asm.joint",
            AssemblyTool::Anchor => "asm.anchor",
            AssemblyTool::Group => "asm.group",
            AssemblyTool::Width => "asm.width",
            AssemblyTool::Tangent => "asm.tangent",
            AssemblyTool::Relation => "asm.relation",
            AssemblyTool::Ground => "asm.ground",
            AssemblyTool::Axis => "asm.axis",
            AssemblyTool::Repick => "asm.repick",
        }
    }
}

/// THE SECTION CAPS: an exact planar cut of every visible body, in world coordinates, cached by (plane,
/// geom_rev, visible bodies).
pub fn section_caps_for_frame(pn: &Painting) -> std::rc::Rc<Vec<qymcad_core::geom::Mesh>> {
    let Some((o, n)) = section_eff(pn.section) else {
        return std::rc::Rc::new(Vec::new());
    };
    let key = {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        for v in o.iter().chain(n.iter()) {
            v.to_bits().hash(&mut h);
        }
        view_rev(pn.regen).hash(&mut h); // the section cuts IN THE WORLD, so it depends on the layout too
        for SceneMesh { index: mi, ghost, .. } in visible_mesh_items(pn) {
            (mi as u32, ghost).hash(&mut h);
        }
        h.finish()
    };
    {
        let c = pn.cache.section_caps.borrow();
        if c.rev == key {
            return c.value.clone();
        }
    }
    // THE CAP IS COMPUTED FROM THE MESH rather than by a kernel boolean. Reported behaviour: no amber cap
    // and hollow bodies inside - no CAD cuts that way. A Common boolean needed a live B-rep, which is not
    // there right after opening from a bundle, will never be there for an imported STL, and cost minutes on
    // a thousand bodies. A mesh is ALWAYS there and cuts orders of magnitude faster, so the cap is now drawn
    // during a gizmo drag as well - the section looks like a closed body at any moment.
    let mut caps: Vec<qymcad_core::geom::Mesh> = Vec::new();
    for SceneMesh { index: mi, ghost, world: wt, .. } in visible_mesh_items(pn) {
        if ghost {
            continue; // ghosts get no cap
        }
        // A CHEAP REJECT by extent - the plane cuts a handful of bodies out of a thousand
        if !mesh_crosses_plane(pn, mi, o, n) {
            continue;
        }
        let Some(mesh) = pn.project.bodies.get(mi).map(|b| &b.mesh) else { continue };
        // the body's mesh is in its local frame, so the plane is carried there too (wt is rigid: p_l = R^T (p - t))
        let r = |v: [f64; 3]| [wt[0] * v[0] + wt[4] * v[1] + wt[8] * v[2], wt[1] * v[0] + wt[5] * v[1] + wt[9] * v[2], wt[2] * v[0] + wt[6] * v[1] + wt[10] * v[2]];
        let po = [o[0] - wt[3], o[1] - wt[7], o[2] - wt[11]];
        let (ol, nl) = (r(po), r(n));
        let tris = qymcad_core::geom::mesh_section_cap(mesh, ol, nl);
        if tris.is_empty() {
            continue;
        }
        let mut cap = qymcad_core::geom::Mesh::default();
        for t in tris {
            let base = cap.verts.len() as u32;
            cap.verts.extend(t);
            cap.tris.push([base, base + 1, base + 2]);
        }
        if !qymcad_core::feature::is_identity12(&wt) {
            cap.transform(&wt); // the cap into world coordinates
        }
        caps.push(cap);
    }
    let rc = std::rc::Rc::new(caps);
    pn.cache.section_caps.borrow_mut().put(key, rc.clone());
    rc
}

/// Whether the PLANE crosses the world bounding box of mesh `mi`. A cheap rejection before the
/// expensive boolean for the section cap: a body lying entirely on one side has no cap by definition.
pub fn mesh_crosses_plane(pn: &Painting, mi: usize, o: [f64; 3], n: [f64; 3]) -> bool {
    let Some(b) = mesh_world_bounds(pn, mi) else { return true }; // the box is unknown, so assume it cuts
                                                                  // the box's support point along the normal: if the WHOLE box is on one side, there is no intersection
    let (mut lo, mut hi) = (0.0, 0.0);
    for k in 0..3 {
        let (a, b2) = ((b.min[k] - o[k]) * n[k], (b.max[k] - o[k]) * n[k]);
        lo += a.min(b2);
        hi += a.max(b2);
    }
    lo <= 0.0 && hi >= 0.0
}

/// The world bounding box of mesh `mi` (cached by `geom_rev` — computed once per change of the scene).
pub fn mesh_world_bounds(pn: &Painting, mi: usize) -> Option<WorldBox> {
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
        view_rev(pn.regen).hash(&mut h);
        for b in &pn.project.bodies {
            b.visible.hash(&mut h);
        }
        h.finish()
    };
    {
        let c = pn.cache.mesh_bounds.borrow();
        if c.rev == key {
            return c.value.get(&mi).copied();
        }
    }
    let mut map: std::collections::HashMap<usize, WorldBox> = std::collections::HashMap::new();
    for SceneMesh { index: i, world: wt, .. } in visible_mesh_items(pn) {
        let Some(m) = pn.project.bodies.get(i).map(|b| &b.mesh) else { continue };
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
        map.insert(i, WorldBox { min: mn, max: mx });
    }
    let out = map.get(&mi).copied();
    pn.cache.mesh_bounds.borrow_mut().put(key, map);
    out
}

/// THE LIVE BODIES A REBUILD LEAVES: those the document holds a mesh for. A body whose node stopped building keeps
/// both its mesh and its live body - the last good state, for the author to repair and to measure.
///
/// IMPORTED bodies are kept even without a mesh. Their B-rep cannot be rebuilt from a recipe - only by parsing the
/// embedded STEP again (tens of seconds), so their shape is kept even while the body does not build (a rollback or a
/// suppression): move the rollback bar back, and the import is on screen at once.
pub fn keep_live_shapes(live: &mut LiveGeom, project: &Project) {
    let imports: std::collections::HashSet<Id> = project
        .timeline
        .iter()
        .filter_map(|n| match n.kind {
            qymcad_core::feature::FeatureKind::Import { body, .. } => Some(body),
            _ => None,
        })
        .collect();
    live.shapes.retain(|body, _| imports.contains(body) || project.mesh_index(*body).is_some());
}

pub fn regenerate_now(rc: &mut RebuildCtx) {
    prune_dangling_features(rc.live, rc.project); // anti-ghost: on EVERY regen, orphan meshes and dangling features go
                                                  // THE REBUILD GRAPH: a parameter may have changed (including a named sketch dimension - those are in
                                                  // `param_map`) -> ONLY the features that refer to it are rebuilt. This used to mark ALL features
                                                  // carrying expressions, and it fired on EVERY rebuild: any trifle dragged the whole parametrics of
                                                  // the project through a recount.
    mark_changed_params_dirty(rc.params_seen, rc.project);
    let _gate = qymcad_kernel::kernel_gate();
    // HOW MANY CORES THE KERNEL MAY TAKE, said where a rebuild starts rather than remembered somewhere.
    //
    // One atomic store, and it cannot fall out of step with the setting. One core means single-threaded, which
    // is the switch a person reaches for when a parallel pass is suspected of lying.
    qymcad_kernel::set_parallel(rc.set.kernel_threads != 1, rc.set.kernel_threads);
    let kernel = qymcad_kernel::OcctKernel { shapes: std::cell::RefCell::new(std::mem::take(&mut rc.live.shapes)), quality_k: rc.project.geom_quality.deflection_k(), ..Default::default() };
    // HOW OFTEN THE GEOMETRIC FALLBACK FIRED is counted AROUND the rebuild. This is an event of a
    // different kind from rebinding a reference: there a name was found and moved, here no name was
    // found at all and the element was identified BY PLACE, by resemblance. Staying silent about it is
    // not allowed - that is exactly how a reference lands on a neighbouring face, and it is discovered
    // three operations later.
    let snaps_before = rc.project.snap_rebinds.load(std::sync::atomic::Ordering::Relaxed);
    let report = rc.project.regenerate(&kernel);
    let snaps = rc.project.snap_rebinds.load(std::sync::atomic::Ordering::Relaxed).saturating_sub(snaps_before);
    rc.live.shapes = kernel.shapes.into_inner();
    settle_params_seen(rc.params_seen, rc.project); // the rebuild HAPPENED - only now are the values "seen"
                                                    // the cache of live B-rep must match what the project actually holds. A regen REMOVES the mesh of a
                                                    // body that stopped building (a rollback, a suppression, a cascade of an error) - while its former
                                                    // shape stayed in the cache as a ghost: a foreign volume in the counts, wasted memory, and the risk
                                                    // of handing dead geometry outside.
    keep_live_shapes(rc.live, rc.project);
    for (body, faces) in report.built {
        set_body_faces(rc.live, rc.project, body, faces); // both into the index-parallel `faces` and into the cache by body Id
    }
    // REBINDS OF GEOMETRY REFERENCES ARE VISIBLE. A lost reference used to latch onto a "similar" face
    // again on every rebuild, silently - now it is an event that gets announced.
    rc.regen.rebinds = report.rebinds.clone();
    if let Some((_, e)) = report.errors.first() {
        // the error text is in the person's language: the core returned a CODE, the words are found here
        *rc.status = format!("{} {}", qymcad_i18n::tr("status-rebuild"), qymcad_i18n::error_words::error_text(e));
    } else if !report.rebinds.is_empty() {
        let first = &report.rebinds[0];
        *rc.status = if report.rebinds.len() == 1 {
            format!("{} {}", egui_phosphor::regular::LINK_BREAK, qymcad_i18n::tr1("io-rebound-one", "what", &first.what))
        } else {
            format!("{} {}", egui_phosphor::regular::LINK_BREAK, qymcad_i18n::tr2("io-rebound-many", "n", &report.rebinds.len().to_string(), "first", &first.what))
        };
    } else if snaps > 0 {
        *rc.status = format!("{} {}", egui_phosphor::regular::LINK_BREAK, qymcad_i18n::tr1("io-rebound-by-place", "n", &snaps.to_string()));
    }
    invalidate(rc.regen);
}

pub fn ensure_brep(rc: &mut RebuildCtx) {
    if rc.live.ready || rc.live.wait.is_some() {
        return; // done, or an attempt is ALREADY running - a second launch would make the overlay blink
    }
    // THE CACHE OF LIVE SHAPES HAS GONE TO THE THREAD - THERE IS NOTHING TO JUDGE BY.
    //
    // A rebuild TAKES the cache for the duration of the computation (`mem::take` in `spawn_regen`):
    // otherwise the thread has nothing to work with. All that time the application has ZERO live shapes
    // - not because there are none, but because the thread holds them. The preparation, asked at that
    // moment, honestly answered that not a single body was up and ordered a rebuild on top of the
    // running one; the answer depended on whether a computation was in flight, and the "we have already
    // tried this" guard missed every other time.
    //
    // A MEASUREMENT IN A LIVE WINDOW, a document with 138 imports: "136 live shapes" and "0 live shapes"
    // alternated endlessly and the rebuild window blinked without stopping. Judging by what you do not
    // hold is not allowed - while the computation runs, the preparation stays silent.
    if rc.regen.regen_running() {
        return;
    }
    // building the B-rep is DERIVED work: the model does not change from it, the caches do. If the
    // project was clean (just opened) it must stay clean, otherwise closing asks whether to save after
    // nothing at all has been touched.
    let was_clean = !is_dirty(rc);
    // retrying on EVERY call is pointless - if nothing changed since the last attempt, the result will
    // be the same. But declaring the cache ready is not allowed either: that would be a lie, and because
    // of it operations on imports silently failed with "body not built" right up to a restart.
    if rc.live.tried_rev == Some(brep_input_key(rc.live, rc.project)) {
        return;
    }
    rc.live.tried_rev = Some(brep_input_key(rc.live, rc.project)); // guards against re-entering from inside a rebuild
                                                                   // ONLY the nodes whose bodies have no live B-rep are rebuilt (their sources - also without shapes -
                                                                   // land on the same list, so the chain comes whole). A forced regen of the entire project here would
                                                                   // cost a full re-tessellation of 1170 imports for nothing.
                                                                   // only what waits for a B-rep: a mesh piece never has one (see `FeatureKind::waits_for_brep`)
    let missing: Vec<Id> =
        rc.project.timeline.iter().filter(|n| n.kind.waits_for_brep()).filter_map(|n| n.kind.body().map(|b| (n.id, b))).filter(|(_, b)| !rc.live.shapes.contains_key(b)).map(|(id, _)| id).collect();
    if missing.is_empty() {
        rc.live.ready = true;
        fill_model_edges_for_anchors(rc); // see above: otherwise edge anchors are dead after opening
        if was_clean {
            rc.edits.saved_key = edit_key(&DrawCtx { cam: rc.cam, set: rc.set, scheme: rc.scheme, project: rc.project, active_path: rc.active_path });
        }
        return;
    }
    *rc.status = qymcad_i18n::tr1("io-brep-prepare", "n", &missing.len().to_string());
    for n in &mut rc.project.timeline {
        if missing.contains(&n.id) {
            n.dirty = true;
        }
    }
    let deferred = rc.regen.ui_running; // in a live window the rebuild goes to a thread and comes back later
    mark_dirty_for_rebuild(rc); // the document is marked; the planner does the counting
    if deferred {
        // Reported: pressing "create sketch" made the rebuild window blink back and forth. The rebuild
        // HAS NOT RUN YET - it is queued. Marking "tried at revision N" now is not allowed: the rebuild,
        // once it arrives, moves the revision, the mark stops matching, and the next frame (and picking
        // a sketch plane calls us on EVERY frame) starts it all over - an endless loop. The outcome is
        // drawn by `settle_brep_wait` when the result gets here.
        rc.live.wait = Some(was_clean);
        return;
    }
    settle_brep_wait(rc, was_clean);
}

/// Sum up the B-rep preparation ON THE FACT of a rebuild having happened (synchronous or arrived
/// from a thread). `was_clean` says whether the document was clean before the preparation.
pub fn settle_brep_wait(rc: &mut RebuildCtx, was_clean: bool) {
    rc.live.tried_rev = Some(brep_input_key(rc.live, rc.project));
    // the flag follows THE FACT. If a body is left without a B-rep (an import waiting to be restored
    // from the embedded STEP), the cache is NOT ready, and the next attempt will happen once new data
    // appears.
    rc.live.ready = rc.project.timeline.iter().filter(|n| n.kind.waits_for_brep()).filter_map(|n| n.kind.body()).all(|b| rc.live.shapes.contains_key(&b));
    fill_model_edges_for_anchors(rc); // anchors on edges are dead after opening otherwise (see io_jobs)
    if was_clean {
        rc.edits.saved_key = edit_key(&DrawCtx { cam: rc.cam, set: rc.set, scheme: rc.scheme, project: rc.project, active_path: rc.active_path });
        // rebuilding the cache is not an edit made by hand
    }
}

/// BRING UP the cache of live B-rep when the project was opened from a bundle WITHOUT a rebuild (the
/// geometry is shown at once, but the `Shape`s of feature bodies do not exist yet). The first operation
/// that needs a real B-rep pays for the rebuild once - the same idea as resolving a lightweight
/// document. Until a project has been opened the flag is set and this is a pure no-op.
///
/// EDGES FOR ANCHORS GO INTO THE MODEL as soon as the live B-rep is up.
///
/// The first fix here was incomplete: joints on edges worked in the session where they were made and
/// died on the next OPENING - "anchor lost", travel 0.000 mm. The two sources of edges were reconciled
/// only at the moment an anchor is PLACED (`ensure_model_edges` while picking) and not reconciled when
/// the document is opened: `regen_edges` is filled by the post-pass of a rebuild, and imported bodies
/// are not rebuilt at all - they have no timeline nodes.
///
/// The core is asked only about the bodies that EDGE anchors and VERTEX anchors refer to: there are a
/// handful of those, and going through all hundred-odd would be pointless.
pub fn fill_model_edges_for_anchors(rc: &mut RebuildCtx) {
    use qymcad_core::feature::AnchorRef;
    let want: Vec<Id> = rc
        .project
        .connectors
        .iter()
        .filter_map(|c| match c.anchor {
            AnchorRef::EdgeMid(b, _) | AnchorRef::Vertex(b, _, _) => Some(b),
            _ => None,
        })
        .filter(|b| !rc.project.regen_edges.contains_key(b) && rc.live.shapes.contains_key(b))
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    for b in want {
        ensure_model_edges(rc, b);
    }
}

/// Abort a lasting operation: the document comes back as it was, and no step is left behind.
pub fn abort_edit(rc: &mut RebuildCtx) {
    rc.edits.depth = rc.edits.depth.saturating_sub(1);
    if rc.edits.depth > 0 {
        return;
    }
    let Some((_, before)) = rc.edits.open.take() else { return };
    rc.regen.pending = false; // the operation was rolled back: there is nothing to rebuild
    let _ = restore(rc, before); // an aborted operation leaves the window where it is
    rc.edits.committed_key = doc_key(rc.project);
}

/// Close an operation that is whole when it returns, leaving a step only if it changed the document: the first
/// click of a circle only holds its centre in the tool, and a step for it made one circle take two presses of
/// Ctrl+Z, the first of them undoing nothing. Not for an operation whose result lands later (a build in the
/// background) - there the change is not in the document yet when it closes.
pub fn commit_edit_if_changed(rc: &mut RebuildCtx) {
    let unchanged = rc.edits.depth == 1 && rc.edits.open.as_ref().is_some_and(|(_, before)| doc_key(&before.project) == doc_key(rc.project));
    if unchanged {
        rc.edits.depth = 0;
        rc.edits.open = None;
    } else {
        commit_edit(rc);
    }
}

/// FOLD WHAT CHANGED SINCE INTO THE LAST STEP: undoing that step takes this change with it, and no step of its own
/// is made. Only between operations; with one open, it is the open one that takes the change.
/// THE LAST STEP TAKEN BACK AND FORGOTTEN: the document as it was before it, and no step to redo - for a step a tool laid
/// on its way and is put down without finishing (the lines a corner tool cut at a crossing, the corner never cut).
pub fn take_back_last_step(edits: &mut Edits, project: &mut Project, regen: &mut Rebuilding) {
    if edits.open.is_some() {
        return;
    }
    let Some(step) = edits.undo.pop() else { return };
    *project = step.snap.project.clone();
    edits.baseline = step.snap;
    edits.committed_key = doc_key(project);
    invalidate(regen);
}

pub fn fold_into_last_step(edits: &mut Edits, project: &Project) {
    if edits.open.is_some() || edits.undo.is_empty() {
        return;
    }
    edits.baseline = Snapshot { project: project.clone() };
    edits.committed_key = doc_key(project);
    edits.redo.clear();
}

/// LAY WHAT A PANEL CHANGED AS ONE NAMED STEP, measured from the last committed state: for a panel whose widgets write
/// straight into the document (the values of a joint), where the state before the change is no longer at hand once it
/// is known that something changed. `continuing` - the same drag as the step before - folds the change into that step,
/// so a drag across forty frames is one step, not forty.
pub fn settle_step(edits: &mut Edits, project: &Project, name: impl Into<String>, continuing: bool) {
    if edits.open.is_some() {
        return; // an open operation takes the change itself
    }
    let k = doc_key(project);
    if k == edits.committed_key {
        return;
    }
    if continuing && !edits.undo.is_empty() {
        fold_into_last_step(edits, project);
        edits.committed_key = k;
        return;
    }
    let name = name.into();
    note_step(&name);
    let before = std::mem::replace(&mut edits.baseline, Snapshot { project: project.clone() });
    edits.undo.push(Step { name, snap: before });
    edits.redo.clear();
    edits.committed_key = k;
}

/// CLOSE AN EDIT OF THE SKETCH where only the journal and the document are at hand (an `Editing`): the named step goes
/// onto the undo stack, or nothing does when the document did not change. The rebuild is the frame's - the edit has
/// already marked it. Without a boundary of its own an edit of the sketch was laid by the frame's safety net as a step
/// called "Edit", in every language.
pub fn close_edit(edits: &mut Edits, project: &Project) {
    edits.depth = edits.depth.saturating_sub(1);
    if edits.depth > 0 {
        return; // a nested operation: the outer one will sum it up
    }
    let Some((name, before)) = edits.open.take() else { return };
    if doc_key(&before.project) == doc_key(project) {
        return;
    }
    edits.undo.push(Step { name, snap: before });
    edits.redo.clear();
    edits.baseline = Snapshot { project: project.clone() };
    edits.committed_key = doc_key(project);
}

/// A REBUILD FROM THE THREAD HAS LANDED ON A DOCUMENT NOBODY TOUCHED SINCE: the state the next undo comes back to is
/// the one built. The state was laid down when the step closed, before the thread finished, and brought back from
/// there it carried the faces as they were before the rebuild named them - a seam face undone and done again came
/// back under the kernel's raw number 11 instead of its name.
pub fn settle_baseline(edits: &mut Edits, project: &Project) {
    if edits.open.is_none() && edits.committed_key == doc_key(project) {
        edits.baseline = Snapshot { project: project.clone() };
    }
}

/// Close a lasting operation: a named step goes onto the undo stack.
pub fn commit_edit(rc: &mut RebuildCtx) {
    rc.edits.depth = rc.edits.depth.saturating_sub(1);
    if rc.edits.depth > 0 {
        return; // a nested operation: the outer one will sum it up
    }
    let Some((name, before)) = rc.edits.open.take() else { return };
    rc.edits.undo.push(Step { name, snap: before });
    rc.regen.pending = false;
    rebuild_if_dirty(rc); // ONE point: the scheduler decides for itself whether to compute
    if rc.edits.undo.len() > rc.set.undo_cap.max(1) {
        rc.edits.undo.remove(0);
    }
    rc.edits.redo.clear();
    rc.edits.baseline = Snapshot { project: rc.project.clone() };
    rc.edits.committed_key = doc_key(rc.project);
}

/// THE PATH OF CONTEXTS DOWN TO COMPONENT `cid`: the root, the chain of its ancestors, the component itself - what
/// `active_path` holds while working inside it. Built from the parents, at most 256 deep.
pub fn context_path_to(project: &Project, cid: Id) -> Vec<Id> {
    let root = project.root;
    let (mut chain, mut cur) = (vec![cid], cid);
    for _ in 0..256 {
        if cur == root {
            break;
        }
        match project.components.iter().find(|c| c.id == cur).and_then(|c| c.parent) {
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
    chain
}

/// UNDO AND REDO BY KEY: Ctrl+Z, Ctrl+Shift+Z or Ctrl+Y - (undo, redo). A text field keeps them for its own text,
/// except the field a new shape opens for its size: the shape is already in the document, and Ctrl+Z right after
/// drawing a circle is meant for the circle. Such a field goes with the undo - its shape is gone.
pub fn undo_keys(ctx: &egui::Context, place: &mut Placing, inline: &mut InlineEdit, in_sketch: bool) -> (bool, bool) {
    // the field of a new shape stands in the sketch being edited: out of it, a field in focus is some other one (the
    // search of the tree) and keeps its keys - a shape left sizing when the sketch was finished took them from it
    let sizing = in_sketch && (place.active() || place.dim.is_some() || inline.dim().is_some() || inline.circle().is_some());
    if ctx.egui_wants_keyboard_input() && !sizing {
        return (false, false);
    }
    let (undo, redo) = ctx.input(|i| {
        let (cmd, z, y) = (i.modifiers.command, i.key_pressed(egui::Key::Z), i.key_pressed(egui::Key::Y));
        (cmd && z && !i.modifiers.shift, cmd && ((z && i.modifiers.shift) || y))
    });
    if undo && sizing {
        place.clear();
        inline.clear();
    }
    (undo, redo)
}

pub fn begin_edit(edits: &mut Edits, project: &Project, name: impl Into<String>) {
    if edits.open.is_none() {
        let name = name.into();
        // THE TRAIL FOR A CRASH REPORT IS WRITTEN HERE, NOT AT THE COMMIT. A crash happens in the
        // middle of an operation, so a trail of committed steps is missing exactly the one that
        // killed the program - the only one worth having.
        note_step(&name);
        edits.open = Some((name, Snapshot { project: project.clone() }));
    }
    edits.depth += 1;
}

/// OPEN A LASTING OPERATION (one that lives across frames: a sketch editing session, a drag).
/// The `Edit` guard holds `&mut App` and suits only a short operation inside a single call; a session
/// is closed explicitly, by `commit_edit` or `abort_edit`.
/// AN UNDO STEP OVER A CONTEXT, not over the whole application. `App::edit` exists for the frame; a panel
/// that already holds a rebuild context has no application to ask.
pub fn edit_over<'a>(rc: RebuildCtx<'a>, name: impl Into<String>) -> Edit<'a> {
    begin_edit(rc.edits, rc.project, name);
    Edit { rc, done: false }
}

/// Answers the context path the window stands at afterwards: the component the restored document was worked in, when
/// it is still there - a step is taken back and put again where it was made.
#[must_use]
pub fn restore(rc: &mut RebuildCtx, snap: Snapshot) -> Vec<Id> {
    // A snapshot carries THE MESHES but not the live B-rep (`Shape` is not cloneable). `shapes` used
    // to be left over from the undone state: old geometry on screen, new geometry in the kernel, and
    // the NEXT operation built on the undone shape. Silently, because nodes are not dirty after an
    // undo. What gets rebuilt is exactly the bodies whose RECIPE differs between the states (not a
    // forced pass over the whole document — on an assembly of a thousand imports that is tens of
    // seconds).
    let mut changed = rc.project.changed_bodies_vs(&snap.project);
    // A PIECE OF A MESH IS ITS OWN GEOMETRY: the snapshot brings its mesh and faces back as they were, and there is
    // nothing to rebuild it from. Counted as changed, its node is marked and a rebuild of nothing is started.
    let pieces: Vec<Id> = changed.iter().copied().filter(|&b| snap.project.timeline.iter().any(|n| n.kind.owns_body(b) && !n.kind.waits_for_brep())).collect();
    changed.retain(|b| !pieces.contains(b));
    let mut restored = snap.project;
    // THE PARAMETERS THAT DIFFER between the two states change whatever reads them, though no recipe changed: the
    // node still says "k". A snapshot taken when an edit closed holds the new expression beside the old value and the
    // old geometry - both are counted by the rebuild, which comes after - and restored as it was, a chamfer driven by
    // k kept the size of the k before. Measured by the check of undo and redo after every step: k written as 3 came
    // back holding 1.5, the chamfer with it. Asked by the expression as well as the value for that reason.
    // the values counted again from the expressions first: the snapshot may hold an expression its value never caught up
    // with, the value being counted when the table's edit is applied
    let _ = restored.eval_parameters();
    let said = |p: &Project| p.parameters.iter().map(|q| (q.name.clone(), (q.expr.clone(), q.value.to_bits()))).collect::<std::collections::HashMap<_, _>>();
    let (was, now) = (said(rc.project), said(&restored));
    let moved: Vec<String> = was.keys().chain(now.keys()).filter(|k| was.get(*k) != now.get(*k)).cloned().collect();
    shelve_source_data(rc.live, rc.project, &mut restored);
    restored.take_source_data_from(rc.project); // the source bytes come from the live document
    restored.keep_ids_past(rc.project); // an id handed out once is never handed out again
                                        // the derived topology caches never went into the snapshot — bring them back from the live state
                                        // for the bodies the edit did not touch (the changed ones are rebuilt below anyway)
    let (rf, re) = (std::mem::take(&mut rc.project.regen_faces), std::mem::take(&mut rc.project.regen_edges));
    shelve_imports(rc.live, rc.project, &restored);
    *rc.project = restored;
    rc.project.regen_faces = rf;
    rc.project.regen_edges = re;
    // THE LIVE BODIES OF THE CHANGED ONES STAY until the rebuild below replaces them: every node of theirs is marked
    // dirty and built again, and one that no longer builds keeps its last good body, as a partial rebuild keeps it.
    // Reported behaviour: a sketch deleted under its extrusion left the body with 12 edges to pick; the same step
    // undone and done again had none - the body thrown away here, and the red node built nothing to put back.
    // a piece's face cache comes back with it: a stale one is laid over its faces at the next change of topology
    for &b in &pieces {
        if let Some(i) = rc.project.mesh_index(b) {
            rc.live.faces.insert(b, rc.project.bodies[i].faces.clone());
        }
    }
    // every body of the node counts: a pattern of parts and a split body have several and no single `body()`, and a
    // pattern brought back by redo kept its copies without a live shape - no edges to pick on any of them
    let dirty: Vec<Id> = rc.project.timeline.iter().filter(|n| n.kind.bodies().iter().any(|b| changed.contains(b))).map(|n| n.id).collect();
    for n in &mut rc.project.timeline {
        if dirty.contains(&n.id) {
            n.dirty = true;
        }
    }
    for name in &moved {
        rc.project.mark_param_dependents_dirty_for(name);
    }
    *rc.sel = Sel::None;
    rc.regen.geom_rev = rc.regen.geom_rev.wrapping_add(1);
    // THE VIEW IS NOT TOUCHED. Restoring a snapshot is an operation on the DOCUMENT, and the point of
    // view belongs to the person: undoing an edit must not throw away the corner they had zoomed in on.
    //
    // Reported behaviour: in the sketcher, picking the measuring tools and the constraints resets the
    // camera. And so it did, by this line: a constraint button that does not fit the current selection
    // is the ordinary way of working - press the button, then pick - and it ends in `abort_edit`, that
    // is here. `view.initialized = false` made the next frame re-fit the whole sketch.
    if !changed.is_empty() {
        regenerate_all(rc); // bring the B-rep of the changed bodies up to the restored state
    }
    let worked_in = rc.project.active_component.filter(|&c| rc.project.components.iter().any(|x| x.id == c));
    worked_in.map_or_else(|| rc.active_path.clone(), |c| context_path_to(rc.project, c))
}

/// The imported bodies of `p`.
fn import_bodies(p: &Project) -> std::collections::HashSet<Id> {
    p.timeline
        .iter()
        .filter_map(|n| match n.kind {
            qymcad_core::feature::FeatureKind::Import { body, .. } => Some(body),
            _ => None,
        })
        .collect()
}

/// Between two states of the document: the live shapes of the imports leaving it go to the shelf, those of the
/// imports coming back are taken off it, so the rebuild after the restore tessellates them with named faces.
fn shelve_imports(live: &mut LiveGeom, was: &Project, now: &Project) {
    let (before, after) = (import_bodies(was), import_bodies(now));
    for b in before.difference(&after) {
        if let Some(s) = live.shapes.remove(b) {
            live.shelved.insert(*b, s);
        }
    }
    for b in &after {
        if !live.shapes.contains_key(b) {
            if let Some(s) = live.shelved.remove(b) {
                live.shapes.insert(*b, s);
            }
        }
    }
}

/// Between two states of the document: the bytes of the sources leaving it go to the shelf, those of the sources
/// coming back without bytes are taken off it.
fn shelve_source_data(live: &mut LiveGeom, was: &mut Project, now: &mut Project) {
    for src in &mut was.sources {
        if !src.data.is_empty() && !now.sources.iter().any(|n| n.id == src.id) {
            live.shelved_sources.insert(src.id, std::mem::take(&mut src.data));
        }
    }
    for src in &mut now.sources {
        if src.data.is_empty() {
            if let Some(d) = live.shelved_sources.remove(&src.id) {
                src.data = d;
            }
        }
    }
}

/// After a component's placement changes: with external references present, rebuild the consumers
/// (top-down associativity — the source face travelled with the part); otherwise simply invalidate the cache.
pub fn after_placement_change(rc: &mut RebuildCtx) {
    // a mirrored part is placed FREELY by the gizmo, so moving either the source or the mirror needs no
    // regeneration of the shape (the plane is fixed in the source's local frame)
    if !rc.project.external_refs.is_empty() {
        rc.project.mark_external_consumers_dirty(); // the consumers' `sketch_frame` has moved, so rebuild
        mark_dirty_for_rebuild(rc); // the document is marked; the scheduler does the computing
    } else {
        invalidate(rc.regen);
    }
}

pub fn regenerate_all(rc: &mut RebuildCtx) {
    // ONE REBUILD PER ACTION. Inside an operation it is asked for several times (the command touches
    // the sketch, then the timeline, then the caches) — but the action is one, so the rebuild is one
    // too: the request is accumulated and carried out when the operation closes. Every request used
    // to recompute the model from scratch.
    if rc.edits.open.is_some() {
        rc.regen.pending = true;
        return;
    }
    rc.regen.computing_depth = rc.edits.undo.len();
    if rc.regen.ui_running {
        rc.regen.wanted = true;
        return;
    }
    regenerate_now(rc);
    rc.regen.computed_depth = rc.regen.computing_depth;
}

/// ONE STEP BACK (`redo` false) OR FORWARD through the history; answers the context path to stand at, or None when
/// there is no step. A command open at that moment keeps going from the state brought back: its snapshot was taken
/// before the step, and restoring it when the command was put down brought the undone step back. Reported behaviour
/// found by the check: with a mate tool holding its picks, Ctrl+Z took "Extrusion" off the list and the extrusion
/// stayed in the document.
pub fn step_through_history(rc: &mut RebuildCtx, redo: bool) -> Option<Vec<Id>> {
    let step = if redo { rc.edits.redo.pop() } else { rc.edits.undo.pop() }?;
    // an operation still open - a command, a sketch session - now starts from the state brought back: put down, it
    // returns there. Letting it go instead broke the sketch session that owned it, and rebuilding everything after an
    // undo in a sketch never came to rest.
    if let Some((_, before)) = rc.edits.open.as_mut() {
        *before = step.snap.clone();
    }
    let cur = Step { name: step.name.clone(), snap: std::mem::replace(&mut rc.edits.baseline, step.snap.clone()) };
    if redo {
        rc.edits.undo.push(cur)
    } else {
        rc.edits.redo.push(cur)
    }
    let path = restore(rc, step.snap);
    rc.edits.committed_key = doc_key(rc.project);
    *rc.status = qymcad_i18n::tr1(if redo { "g-redone" } else { "g-undone" }, "what", &step.name);
    Some(path)
}

/// A REBUILD STOPPED BY A PERSON TAKES BACK THE EDIT IT WAS COMPUTING: a recognition cancelled leaves no recognised body
/// in the tree, as a cancelled command in a professional CAD leaves nothing. Only the one edit on top, and only where the
/// state under it had been computed - a cancelled "rebuild everything" takes back nothing. The edit goes to redo, so
/// it can be brought back. Otherwise the document stays as it is, and the rebuild does not start again by itself
/// until the document changes. The way to go on, where the edit was taken back.
pub fn rebuild_cancelled(rc: &mut RebuildCtx) -> Option<Vec<Id>> {
    let depth = rc.edits.undo.len();
    if depth > 0 && depth == rc.regen.computing_depth && rc.regen.computed_depth + 1 == depth {
        let step = rc.edits.undo.pop()?;
        let now = std::mem::replace(&mut rc.edits.baseline, step.snap.clone());
        rc.edits.redo.push(Step { name: step.name.clone(), snap: now });
        let path = restore(rc, step.snap);
        rc.edits.committed_key = doc_key(rc.project);
        rc.regen.computed_depth = rc.edits.undo.len();
        rc.regen.paused = None;
        *rc.status = format!("{} {}", egui_phosphor::regular::WARNING, qymcad_i18n::tr1("io-rebuild-cancelled-undone", "what", &step.name));
        return Some(path);
    }
    rc.regen.paused = Some(rc.project.rebuild_key());
    *rc.status = format!("{} {}", egui_phosphor::regular::WARNING, qymcad_i18n::tr("io-rebuild-cancelled"));
    None
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
pub fn rebuild_if_dirty(rc: &mut RebuildCtx) {
    if rc.edits.open.is_some() {
        return; // an operation is under way — its closing will sum it up (one rebuild per action)
    }
    mark_changed_params_dirty(rc.params_seen, rc.project);
    // THE REBUILD RUNNING NOW IS FOR A DOCUMENT THAT IS NO MORE: its result is thrown away on arrival, so it is asked
    // to stop rather than left to finish. Asked before anything below may return: deleting a part leaves no node
    // dirty. Reported behaviour: a part deleted while its mesh was being recognised, and the spinner went on turning
    // for as long as the recognition took (14 s on a 160k-triangle ball in a debug build), over nothing.
    if let Some(pulse) = rc.regen.busy.as_ref().and_then(|b| b.pulse.as_ref()).filter(|p| p.stamp != rc.project.rebuild_key()) {
        pulse.ask_stop();
    }
    let asked = std::mem::take(&mut rc.regen.pending);
    // THE REBUILD WAS STOPPED BY HAND — it does not start again on its own. An explicit request
    // (`asked`) clears the mark: "Rebuild everything" and any edit of the document both mean
    // "compute again".
    if rc.regen.paused == Some(rc.project.rebuild_key()) && !asked {
        return;
    }
    rc.regen.paused = None;
    if !asked && !rc.project.timeline.iter().any(|n| n.dirty) {
        rc.edits.committed_key = doc_key(rc.project); // the "dirty" marks set above are derived too
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
    let now = (rc.project.rebuild_key(), rc.project.timeline.iter().filter(|n| n.dirty).map(|n| n.id).collect::<Vec<_>>(), rc.live.shapes.len());
    if !asked && now == rc.regen.last {
        return;
    }
    regenerate_all(rc);
    rc.regen.last = (rc.project.rebuild_key(), rc.project.timeline.iter().filter(|n| n.dirty).map(|n| n.id).collect(), rc.live.shapes.len());
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
    if !rc.live.ready && rc.project.regen_errors.values().any(|e| e.retryable()) {
        ensure_brep(rc);
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
    rc.edits.committed_key = doc_key(rc.project);
}

/// ASK for a rebuild. In a running window it goes into a worker thread and runs behind an indicator:
/// a boolean on a thread takes seconds, and the whole interface used to freeze for that time — the
/// system showed "the application is not responding" and offered to kill it. In headless tests
/// (there is no window) the rebuild runs straight away, so the result is available on the next line.
/// Mark the model as needing a rebuild. It does NOT compute — that is done by the scheduler
/// (`rebuild_if_dirty`) when an operation closes or once a frame. Needed where an edit did not set
/// `dirty` itself (a change of context, the caches); such places should get fewer, not more.
pub fn mark_dirty_for_rebuild(rc: &mut RebuildCtx) {
    rc.regen.pending = true;
    if rc.edits.open.is_none() {
        rebuild_if_dirty(rc);
    }
}

/// Whether there are unsaved changes (the state key has drifted from the moment of saving).
pub fn is_dirty(rc: &mut RebuildCtx) -> bool {
    edit_key(&DrawCtx { cam: rc.cam, set: rc.set, scheme: rc.scheme, project: rc.project, active_path: rc.active_path }) != rc.edits.saved_key
}

/// LEND THE KERNEL a live B-rep cache. The `shapes` cache is the only owner of the `Shape`s, so
/// operations that need geometry (projecting an edge into a sketch) take it from here rather than
/// building a second cache alongside: copies drift apart silently, and the projection then comes from
/// the wrong part.
///
/// THE CLOSURE IS HANDED THE DOCUMENT, not the whole application. It used to get `&mut Self`, and that
/// alone kept the lending - and everything that calls it - inside the crate that declares `App`. Both
/// callers were checked: one ignores the argument entirely, the other reaches for `project` and nothing
/// else. So the wider loan was never used, only paid for.
pub fn with_kernel<R>(rc: &mut RebuildCtx, f: impl FnOnce(&mut qymcad_core::model::Project, &dyn qymcad_core::feature::Kernel) -> R) -> R {
    let _gate = qymcad_kernel::kernel_gate();
    // HOW MANY CORES THE KERNEL MAY TAKE, said where a rebuild starts rather than remembered somewhere.
    //
    // One atomic store, and it cannot fall out of step with the setting. One core means single-threaded, which
    // is the switch a person reaches for when a parallel pass is suspected of lying.
    qymcad_kernel::set_parallel(rc.set.kernel_threads != 1, rc.set.kernel_threads);
    let kernel = qymcad_kernel::OcctKernel { shapes: std::cell::RefCell::new(std::mem::take(&mut rc.live.shapes)), quality_k: rc.project.geom_quality.deflection_k(), ..Default::default() };
    let out = f(rc.project, &kernel);
    rc.live.shapes = kernel.shapes.into_inner();
    out
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
pub fn brep_input_key(live: &LiveGeom, project: &Project) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    let mut missing: Vec<Id> = project.timeline.iter().filter(|n| n.kind.waits_for_brep()).filter_map(|n| n.kind.body()).filter(|b| !live.shapes.contains_key(b)).collect();
    missing.sort_unstable();
    missing.hash(&mut h);
    h.finish()
}

/// Remove the ghosts (orphan meshes and dangling features) — a delegate to the tested
/// `Project::prune_dangling`; here we only clear the application's shape cache by the list of removed bodies.
pub fn prune_dangling_features(live: &mut LiveGeom, project: &mut Project) {
    for db in project.prune_dangling() {
        live.shapes.remove(&db);
    }
}

/// REMEMBER THE PARAMETER VALUES ON THE FACT OF A FINISHED REBUILD.
///
/// The snapshot is taken AFTER it, not before: named dimensions are derived from the sketch
/// geometry, and the sketches are solved by the rebuild itself (`settle_sketches`). A "before"
/// snapshot would drift from the result for no reason at all, and the next frame would declare the
/// parameter changed again.
pub fn settle_params_seen(params_seen: &mut std::collections::HashMap<String, f64>, project: &mut Project) {
    *params_seen = project.param_map();
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
pub struct Edit<'a> {
    /// THE REBUILD CONTEXT, NOT THE APPLICATION. It used to hold `&mut App`, and that single reference kept
    /// every panel opening an operation inside the crate that declares the type. It was needed for exactly
    /// one call - `apply_feat_cmd_inner` - which is now free; the rest only ever reached for the document.
    ///
    /// The nesting is counted by `commit_edit` itself (`edits.depth`), so the guard no longer carries an
    /// `outer` flag: it had already become dead (`let _ = self.outer;`) and said nothing.
    rc: RebuildCtx<'a>,
    done: bool,
}

impl<'a> Edit<'a> {
    /// THE DOCUMENT, and only it. Every user of the guard reaches for this and nothing else - checked by
    /// counting: twelve places in two files, all `ed.project()`.
    pub fn project(&mut self) -> &mut qymcad_core::model::Project {
        self.rc.project
    }

    fn finish(&mut self, ok: bool) {
        if self.done {
            return;
        }
        self.done = true;
        if ok {
            commit_edit(&mut self.rc);
        } else {
            // ABORT: a failed operation leaves no trace, neither in the document nor on the undo stack
            abort_edit(&mut self.rc);
        }
    }
}

impl Drop for Edit<'_> {
    fn drop(&mut self) {
        self.finish(true);
    }
}

/// Clicking an anchor (a face or an edge) for a joint: the first pick becomes A, the second one (on
/// ANOTHER part) becomes B, and a joint of the chosen kind is created. Face to face flips B so the
/// normals meet; the axis of an edge takes no flip. A face anchor is persistent (a `FaceKey`), so it
/// travels with the face through a rebuild.
///
/// THE EDGES OF A BODY GO INTO THE MODEL once the live B-rep is up.
///
/// An anchor on an edge or on a vertex is resolved by the core through `Project::regen_edges`, and that
/// map is filled by THE POST-PASS of a rebuild. Opening a file does not rebuild: the bundle holds
/// meshes and faces but no edges. A click still HITS an edge - the pick takes them from the live B-rep
/// - and the joint was born dead: "anchor lost", travel 0.000 mm, no axis of travel. Measured on a real
///   document: 138 bodies, faces on all 138, EDGES ON TWO, live B-rep on all 138.
///
/// The two sources of edges are reconciled here, at the point where the anchor is created: the core is
/// asked through the same call a rebuild uses to fill them.
pub fn ensure_model_edges(rc: &mut RebuildCtx, body: Id) {
    if rc.project.regen_edges.contains_key(&body) || !rc.live.shapes.contains_key(&body) {
        return;
    }
    let edges = with_kernel(rc, |_, k| k.edges(body));
    if !edges.is_empty() {
        rc.project.regen_edges.insert(body, edges);
    }
}

pub fn set_body_faces(live: &mut LiveGeom, project: &mut Project, body: Id, faces: Vec<MeshFace>) {
    // THE BODY WAS REBUILT — THE STORED BLOB IS STALE. It is dropped here rather than in one of the
    // rebuild branches: the synchronous and the background one meet exactly at this point, and they must
    // not drift apart — a stale blob would go into the file and open as a body FROM THE PAST.
    live.blobs.remove(&body);
    if let Some(idx) = project.mesh_index(body) {
        if idx < project.bodies.len() {
            project.bodies[idx].faces = faces.clone();
        }
    }
    live.faces.insert(body, faces);
}

/// ONE style for the active tool's top bar across ALL the workbenches (the sketcher is the reference):
/// the same background and padding, so the Part, Assembly and Sketch bars look alike. The tool's name
/// is `strong()` in a neutral colour, the icon tells the tools apart, and the parameters follow a
/// `separator()`.
/// THE ACTIVE TOOL'S BAR. This used to be a function WITHOUT `&self` — it simply could not look at the
/// theme, even had it wanted to. Not "somebody forgot to colour it", but structurally impossible.
pub fn tool_bar_frame(scheme: &SchemeUi) -> egui::Frame {
    egui::Frame::NONE.fill(scheme.pal.toolbar_bg()).inner_margin(egui::Margin::symmetric(8, 3))
}

/// A human-readable description of a connector's anchor: the kind (face, edge, vertex and so on) and the part.
pub fn anchor_desc(dc: &DrawCtx, anchor: &qymcad_core::feature::AnchorRef) -> String {
    use qymcad_core::feature::AnchorRef;
    match anchor {
        AnchorRef::Origin => qymcad_i18n::tr("anchor-origin"),
        AnchorRef::BasePlane(_) => qymcad_i18n::tr("anchor-base-plane"),
        AnchorRef::FaceCenter(body, _) => qymcad_i18n::tr1("anchor-face", "what", &body_comp_name(dc.project, *body)),
        AnchorRef::EdgeMid(body, _) => qymcad_i18n::tr1("anchor-edge", "what", &body_comp_name(dc.project, *body)),
        AnchorRef::Vertex(body, _, _) => qymcad_i18n::tr1("anchor-vertex", "what", &body_comp_name(dc.project, *body)),
    }
}

/// START SWEEPING A DEGREE OF FREEDOM: `false` means there is nothing to sweep, and the caller must say why.
pub fn start_joint_anim(joint_anim: &mut Option<JointAnim>, project: &mut Project, joint: Id, slot: usize) -> bool {
    let Some((from, to)) = project.joint_anim_range(joint, slot) else { return false };
    let placed = project.components.iter().map(|c| (c.id, c.transform)).collect();
    *joint_anim = Some(JointAnim { joint, slot, from, to, t: 0.0, forward: true, placed, mates: project.joints.clone() });
    true
}

/// Change the KIND of an existing joint on the fly, KEEPING its anchors (without recreating the
/// joint). Returns true when the kind changed (the caller then regenerates).
pub fn change_joint_kind(project: &mut Project, status: &mut String, jid: Id, newk: qymcad_core::feature::JointKind) -> bool {
    // CHANGING THE KIND IS THE CORE'S BUSINESS: the mating side follows it (an anchor's frame is built
    // FOR A KIND), and so does the declared "as it stands". The interface here only wrote the `kind`
    // field, and after a change of kind the joint kept the side chosen for the previous kind's frame.
    if !project.set_joint_kind(jid, newk) {
        return false;
    }
    // ANY PAIR OF ANCHORS SUITS ANY KIND OF JOINT. A "compatibility" check used to stand here, with a
    // message saying the anchor kinds did not suit, but it always answered "they do": the rule existed
    // only in the message. It is not needed either — an anchor is a full coordinate system, and the
    // kind of joint only says which degrees of freedom stay free.
    *status = qymcad_i18n::tr1("g-joint-kind-set", "kind", &qymcad_i18n::tr(newk.label()));
    true
}

/// Leave the joint editing mode (Esc or Done).
pub fn exit_joint_edit(joint: &mut JointCommand, status: &mut String) {
    joint.edit = None;
    joint.edit_repick = None;
    *status = qymcad_i18n::tr("g-joint-edit-done");
}

/// The name of the PART that owns body `body` (for the anchor's label in the editing popup).
pub fn body_comp_name(project: &qymcad_core::model::Project, body: Id) -> String {
    project.body_owner(body).and_then(|o| project.components.iter().find(|c| c.id == o)).map(|c| qymcad_i18n::name(&c.name)).unwrap_or_else(|| "?".into())
}

/// SWEEPING A DEGREE OF FREEDOM (animating a joint).
///
/// A mechanism is checked by eye: assemble it, then sweep it and watch how it travels. Numbers do not
/// show that, and dragging a part by hand to convince yourself that it reaches the end and does not
/// pass through its neighbour is guesswork, not a check.
#[derive(Clone)]
pub struct JointAnim {
    pub joint: Id,
    pub slot: usize,
    /// the bounds of the sweep, taken from the joint's limits (`Project::joint_anim_range`)
    pub from: f64,
    pub to: f64,
    /// how far along from `from` to `to`, 0..1, and which way it travels
    pub t: f64,
    pub forward: bool,
    /// WHERE EVERY COMPONENT STOOD AND WHAT EVERY MATE READ when the sweep began, to put back when it is stopped. A
    /// sweep is a PREVIEW, not an edit: leaving the part wherever the stop caught it would silently change the document
    /// by pressing a "have a look" button - and putting the drive back alone did just that, the solve after it keeping
    /// the part where the stop caught it
    pub placed: Vec<(Id, [f64; 12])>,
    pub mates: Vec<qymcad_core::feature::Joint>,
}

/// WHAT THE ASSEMBLY WORKBENCH TOUCHES.
///
/// Measured, not guessed: fifty-two of the fifty-three methods of this file reach for fifteen fields, and
/// the fifty-third - `run_command` - clears the tools of EVERY workbench, so it is the application's
/// dispatcher standing in the wrong file.
///
/// The record starts at the fields the converted half needs and grows as the rest follows: a field nobody
/// reads yet is a warning, and the manifest keeps warnings at zero.
pub struct JointCtx<'a> {
    pub view: &'a mut View2d,
    pub sel: &'a mut Sel,
    pub joint: &'a mut JointCommand,
    pub joint_anim: &'a mut Option<JointAnim>,
    pub comp_giz: &'a CompGizmo,
    pub project: &'a mut qymcad_core::model::Project,
    pub status: &'a mut String,
    pub edits: &'a mut Edits,
    pub sel_conn: &'a mut Option<Id>,
    pub live: &'a mut LiveGeom,
    pub part_pull: &'a mut Option<(Id, [f64; 3], [f64; 3])>,
    pub regen: &'a mut Rebuilding,
    pub params_seen: &'a mut std::collections::HashMap<String, f64>,
    pub active_path: &'a Vec<Id>,
    pub scheme: &'a SchemeUi,
    pub cam: &'a Cam3,
    pub set: &'a Settings,
    pub workbench: Workbench,
    pub mode_3d: bool,
}

impl JointCtx<'_> {
    /// THE REBUILD, THROUGH THE ASSEMBLY'S OWN FIELDS. `mark_dirty_for_rebuild` and `commit_edit` are the
    /// commonest things this workbench does - twenty calls between them - and both take the rebuild
    /// context, which is built out of ten fields this record already holds. Composed rather than carried
    /// alongside: two contexts in one signature would let them drift apart.
    pub fn rebuild(&mut self) -> RebuildCtx<'_> {
        RebuildCtx {
            project: self.project,
            view: self.view,
            edits: self.edits,
            regen: self.regen,
            live: self.live,
            params_seen: self.params_seen,
            status: self.status,
            sel: self.sel,
            active_path: self.active_path,
            cam: self.cam,
            scheme: self.scheme,
            set: self.set,
        }
    }
}

pub fn dim_expr_field_in(rc: &mut RebuildCtx, ui: &mut egui::Ui, id: Id, key: &str, place: &str) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("ƒ=").weak());
        // THE SAME FIELD AS ON A SKETCH DIMENSION AND IN THE PARAMETER TABLE. It was asked for plainly:
        // features must have all of this too. Different behaviour in two similar places is a defect, even
        // when both of them work.
        let model = rc.project.feat_dim(id, key).unwrap_or("").to_string();
        // THE WIDGET NAME COMES FROM THE PLACE IT IS DRAWN IN. An absolute key ("featdim", the joint, the
        // slot) coincided between the joint popup and the row of the right-hand panel, and the two can be
        // on screen AT THE SAME TIME: egui honestly complained in red right in the frame - "First use of
        // widget ID... Second use of widget ID D898..." - and one of the two fields stopped responding.
        // The name of the parent is appended to the key, and the two places stop arguing.
        let fid = if place.is_empty() { egui::Id::new(("featdim", id, key)) } else { egui::Id::new(("featdim", id, key, place)) };
        let o = expr_field(ui, rc.project, fid, &model, 120.0, &qymcad_i18n::tr("sk-expr-placeholder"));
        if o.committed && o.text != model {
            // ONE EDIT, ONE UNDO STEP. The edit used to go into the model on losing focus, past
            // `App::edit`, and Ctrl+Z never saw it.
            begin_edit(rc.edits, rc.project, qymcad_i18n::tr("par-edit-step"));
            rc.project.set_feat_dim(id, key, o.text.clone());
            commit_edit(rc);
            mark_dirty_for_rebuild(rc); // the document is marked; the planner does the counting
        }
        // WHAT CAME OUT OF WHAT WAS TYPED, IN WORDS, RIGHT HERE.
        //
        // The field can REFUSE: a bad expression does not reach the model, the letters survive and the
        // caret stays where it was (`ExprOut::refused`). But there was nobody to say WHAT was wrong, and
        // a refusal looked like the program not listening: Enter is pressed, nothing happens, no
        // explanation follows. Exactly the silent refusal that counts as the worst answer in this tree.
        //
        // The speaker (`expr_value_label`) had been written and NEVER ONCE CALLED - a compiler warning
        // said the method was never used, and it had drowned among 175 others. This is where it gets
        // wired up: for feature dimensions and joint values it is the only place an answer is visible.
        expr_value_label(&DrawCtx { cam: rc.cam, set: rc.set, scheme: rc.scheme, project: rc.project, active_path: rc.active_path }, ui, &o.text);
        // A DRIVER NAME BELONGS TO A FEATURE PARAMETER TOO. It was asked for plainly: features must have
        // all of this, not sketches alone. The field is the same, in the same place, with the same
        // behaviour: a buffer, a refusal that loses no letters, and the owner named.
        let target = qymcad_core::model::DimTarget::Feature { node: id, key: key.to_string() };
        let cur_name = rc.project.name_of_target(&target);
        // THE NAME CHECK IS COMPUTED BEFORE THE EDIT. The closure holds `rc.project` while applying
        // asks for `&mut self`: both cannot be borrowed at once, so the verdict is computed up front and
        // the ready answer is what gets used further on.
        ui.label(qymcad_i18n::tr("sk-driver-label")).on_hover_text(qymcad_i18n::tr("sk-driver-name-hint"));
        let nid = if place.is_empty() { egui::Id::new(("featdrv", id, key)) } else { egui::Id::new(("featdrv", id, key, place)) };
        let rn = {
            let ok = |nm: &str| nm.is_empty() || (qymcad_core::drivers::check_ident(nm).is_ok() && !rc.project.driver_name_taken_by(nm, &target));
            name_field(ui, rc.project, nid, &cur_name, 100.0, &qymcad_i18n::tr("sk-name-placeholder"), &ok)
        };
        let nm = rn.text.trim().to_string();
        let name_ok = nm.is_empty() || (qymcad_core::drivers::check_ident(&nm).is_ok() && !rc.project.driver_name_taken_by(&nm, &target));
        if rn.committed && nm != cur_name.trim() {
            begin_edit(rc.edits, rc.project, qymcad_i18n::tr("sk-driver-step"));
            let old = cur_name.trim().to_string();
            if !old.is_empty() && !nm.is_empty() {
                let _ = rc.project.rename_driver(&old, &nm);
            } else {
                rc.project.name_dim(nm.clone(), target.clone());
            }
            commit_edit(rc);
            if !nm.is_empty() {
                rc.project.mark_param_dependents_dirty_for(&nm);
            }
            mark_dirty_for_rebuild(rc);
        }
        if !nm.is_empty() && !name_ok {
            let who = rc.project.name_owner(&nm).map(|o| if o.path.is_empty() { qymcad_i18n::tr("par-owner-project") } else { o.path });
            let msg = match who {
                Some(w) => qymcad_i18n::tr2("par-name-taken", "name", &nm, "where", &w),
                None => qymcad_i18n::tr("sk-driver-name-bad"),
            };
            ui.label(egui::RichText::new(&msg).color(rc.scheme.pal.warning()).small()).on_hover_text(qymcad_i18n::tr("sk-driver-name-taken-hint"));
        }
        let shown = o.text;
        if !shown.trim().is_empty() {
            match qymcad_core::expr::eval(&shown, &rc.project.param_map()) {
                Ok(v) => {
                    ui.label(egui::RichText::new(format!("= {v:.3}")).weak().small());
                }
                Err(_) => {
                    ui.colored_label(rc.scheme.pal.error_mild(), egui_phosphor::regular::X);
                }
            }
        }
    });
}

/// The caption "= 42.500" or the reason for the error in words. One for every field: it is computed
/// and shown the same way wherever the field stands.
pub fn expr_value_label(dc: &DrawCtx, ui: &mut egui::Ui, text: &str) {
    if text.trim().is_empty() {
        return;
    }
    match dc.project.eval_expr(text) {
        Ok(v) => {
            ui.label(egui::RichText::new(format!("= {v:.3}")).weak().small());
        }
        Err(e) => {
            // THE REASON IN WORDS AND NOT AN ICON: what exactly did not add up is what needs
            // knowing.
            let msg = qymcad_i18n::error_words::expr_error_text(&e);
            ui.label(egui::RichText::new(&msg).color(dc.scheme.pal.error_mild()).small()).on_hover_text(&msg);
        }
    }
}

/// THE EXPRESSION FIELD. `model` is what is written in the document right now; the field shows it
/// until an edit is begun.
pub fn expr_field(ui: &mut egui::Ui, project: &Project, id: egui::Id, model: &str, w: f32, hint: &str) -> ExprOut {
    field(ui, project, id, model, w, hint, FieldRules { valid: &|_| true, list: NameList::Offered, focus: Focus::Waits })
}

/// THE NAME FIELD. The same thing, but WITHOUT the list of drivers.
///
/// A name is not a formula: there is nothing to substitute other names into it, and the list would also
/// take Enter for itself — the very key a name is confirmed with. In grown-up CAD autocompletion lives
/// in the expression field only, and that is no trifle: a measurement showed that with the list open on
/// the name "h" Enter went to the list and the edit was not applied at all.
pub fn name_field(ui: &mut egui::Ui, project: &Project, id: egui::Id, model: &str, w: f32, hint: &str, valid: &dyn Fn(&str) -> bool) -> ExprOut {
    field(ui, project, id, model, w, hint, FieldRules { valid, list: NameList::Not, focus: Focus::Waits })
}

/// Whether a field offers the names of the parameters while a name is typed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NameList {
    Offered,
    Not,
}

/// Whether a field takes the keyboard as soon as it is drawn, selecting what it holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Focus {
    Takes,
    Waits,
}

/// How a field treats what is typed into it: what it accepts, whether it offers names, whether it takes the keyboard.
pub struct FieldRules<'a> {
    pub valid: &'a dyn Fn(&str) -> bool,
    pub list: NameList,
    pub focus: Focus,
}

/// THE SAME FIELD, BUT WITH A CHECK BEFORE THE COMMIT.
///
/// `valid` decides whether what was typed can be accepted. A refusal DOES NOT ERASE the text and does
/// not release the focus — otherwise out comes exactly the reported trouble: typing `len` and having
/// the `n` deleted automatically. A person finishes the name or presses Escape, and meanwhile the
/// program says what is wrong.
pub fn field(ui: &mut egui::Ui, project: &Project, id: egui::Id, model: &str, w: f32, hint: &str, rules: FieldRules) -> ExprOut {
    let FieldRules { valid, list, focus } = rules;
    let (with_list, autofocus) = (list == NameList::Offered, focus == Focus::Takes);
    let mut st: FieldState = ui.data_mut(|d| d.get_temp(id)).unwrap_or_default();
    // A VALUE SET FROM ELSEWHERE REACHES A FIELD NOBODY HAS TYPED INTO: the field that took the focus when the tool
    // opened keeps its text as a buffer, and a pick that sizes the tool (a thread sized to the cylinder picked) was
    // written back over by that stale buffer. What a person typed is never replaced.
    if st.buf.is_some() && st.buf == st.base && st.base.as_deref() != Some(model) {
        (st.buf, st.base) = (None, None);
    }
    let mut buf = st.buf.clone().unwrap_or_else(|| model.to_string());

    // THE KEYS ARE TAKEN BEFORE THE FIELD IS DRAWN. While the list is open the arrows belong to IT;
    // otherwise they simply move the caret in the text and a row cannot be chosen from the keyboard —
    // which is how it used to be.
    let focused = ui.memory(|m| m.has_focus(id));
    let (mut go_down, mut go_up, mut take, mut esc, mut open_list) = (false, false, false, false, false);
    // A NAME TYPED IN FULL HAS NOTHING LEFT TO INSERT: with the first row - untouched by the arrows - already the word
    // under the caret, Enter is left to the field and applies the value; a second Enter for "pa" typed whole was a key
    // pressed for nothing. A row reached with the arrows is a choice, and Enter inserts it.
    let whole = st.open && st.sel == 0 && {
        let caret = caret_byte(ui.ctx(), id, &buf);
        let word = current_token(&buf, caret).2;
        project.drivers_matching(word).get(st.sel).is_some_and(|d| d.name == word)
    };
    if focused {
        ui.input_mut(|i| {
            open_list = i.consume_key(egui::Modifiers::COMMAND, egui::Key::Space);
            esc = i.consume_key(egui::Modifiers::NONE, egui::Key::Escape);
            if st.open {
                go_down = i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown);
                go_up = i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp);
                // with the list open, Enter and Tab INSERT what is chosen rather than closing the
                // popup of the tool: the formula is still being written.
                take = (!whole && i.consume_key(egui::Modifiers::NONE, egui::Key::Enter)) || i.consume_key(egui::Modifiers::NONE, egui::Key::Tab);
            }
        });
    }
    // ESCAPE IS DECIDED BY THE STATE FROM THE PREVIOUS FRAME: if the list is open it gets closed,
    // otherwise the edit is cancelled. Grown-up CAD does the same: the first Escape removes the list and
    // the popup of the tool stays.
    let close_list = esc && st.open;
    let escaped = esc && !st.open;

    let h = ui.spacing().interact_size.y;
    // `add_sized` AND NOT `desired_width`: inside an `egui::Grid` a request for width is ignored and the
    // field collapses to 32 points — measured, and that is exactly why the parameters window stayed
    // narrow.
    let resp = ui.add_sized(egui::vec2(w, h), egui::TextEdit::singleline(&mut buf).id(id).hint_text(hint));
    // IT STEPS INTO THE FIELD ITSELF AND SELECTS THE FORMER VALUE — the way the dimension popup opened
    // before it moved onto the shared field. The selection is set through the state of the `TextEdit`:
    // `add_sized` returns only the response, yet the width has to be given to the field with it (inside
    // a grid `desired_width` is ignored).
    if autofocus {
        resp.request_focus();
        if let Some(mut ts) = egui::TextEdit::load_state(ui.ctx(), id) {
            let end = buf.chars().count();
            ts.cursor.set_char_range(Some(egui::text::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(end))));
            ts.store(ui.ctx(), id);
        }
    }

    // WHERE THE CARET STANDS — asked of the field itself, right after it has been drawn. The list and the
    // insertion both follow the word under the caret, and a formula is edited in its middle as often as at
    // its end.
    let caret = caret_byte(ui.ctx(), id, &buf);

    if resp.gained_focus() || resp.changed() {
        if st.buf.is_none() {
            st.base = Some(model.to_string());
        }
        st.buf = Some(buf.clone());
    }
    if resp.changed() {
        st.sel = 0;
        st.open = false;
        // THE LIST OPENS ON TYPING AND NOT ON FOCUS. The former one climbed onto the screen at a single
        // click in the field and covered the geometry, though nothing had been asked yet.
        st.open = with_list && !current_token(&buf, caret).2.is_empty();
    }
    if open_list && with_list {
        st.open = true;
    }
    if close_list {
        st.open = false;
    }

    let hits = if st.open { project.drivers_matching(current_token(&buf, caret).2) } else { Vec::new() };
    if hits.is_empty() {
        st.open = false;
    }
    if st.open {
        note_list_open(ui.ctx());
    }
    if !hits.is_empty() {
        st.sel = st.sel.min(hits.len().min(LIST_MAX_ROWS) - 1);
        if go_down {
            st.sel = (st.sel + 1).min(hits.len().min(LIST_MAX_ROWS) - 1);
        }
        if go_up {
            st.sel = st.sel.saturating_sub(1);
        }
    }

    // THE LIST GOES IN A LAYER ABOVE THE POPUPS OF THE TOOLS. Dimension popups live in
    // `Order::Foreground`, and the former list, drawn there too, went BEHIND them: within one layer the
    // area being interacted with wins. `Order::Tooltip` is higher, so the list is always visible.
    let mut picked: Option<String> = None;
    if !hits.is_empty() {
        let area = egui::Area::new(id.with("drv")).order(egui::Order::Tooltip).fixed_pos(resp.rect.left_bottom() + egui::vec2(0.0, 2.0)).constrain(true);
        area.show(ui.ctx(), |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_min_width(resp.rect.width().max(240.0));
                egui::ScrollArea::vertical().max_height(LIST_MAX_H).show(ui, |ui| {
                    for (k, d) in hits.iter().take(LIST_MAX_ROWS).enumerate() {
                        let row = ui
                            .horizontal(|ui| {
                                // THE ONE ENTER WILL TAKE IS HIGHLIGHTED. Without this the arrows
                                // move something invisible and the key is pressed blind.
                                let hit = ui.selectable_label(k == st.sel, egui::RichText::new(&d.name).strong()).clicked();
                                if !d.path.is_empty() {
                                    // THE PATH ANSWERS "WHICH OF THE NAMESAKES". On an ambiguous one
                                    // it is highlighted too: a bare name in a formula will take who
                                    // knows which.
                                    let mut t = egui::RichText::new(&d.path).small();
                                    t = if d.ambiguous { t.color(ui.visuals().warn_fg_color) } else { t.weak() };
                                    ui.label(t);
                                }
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    let val = d.value.map(|v| format!("{v:.3}")).unwrap_or_else(|| qymcad_i18n::tr("par-no-value"));
                                    ui.label(egui::RichText::new(val).weak().small());
                                });
                                hit
                            })
                            .inner;
                        if row {
                            picked = Some(d.name.clone());
                        }
                    }
                    if hits.len() > LIST_MAX_ROWS {
                        ui.label(egui::RichText::new(qymcad_i18n::tr1("par-more", "n", &(hits.len() - LIST_MAX_ROWS).to_string())).weak().small());
                    }
                });
            });
        });
        if take {
            picked = hits.get(st.sel).map(|d| d.name.clone());
        }
    }

    let took = picked.is_some();
    if let Some(name) = picked {
        let (next, at) = insert_driver(&buf, &name, caret);
        buf = next;
        st.buf = Some(buf.clone());
        st.open = false;
        // THE CARET GOES BEHIND THE INSERTED NAME so that typing carries on from there and not from the
        // end of the line.
        if let Some(mut ts) = egui::TextEdit::load_state(ui.ctx(), id) {
            let ch = egui::text::CCursor::new(buf[..at].chars().count());
            ts.cursor.set_char_range(Some(egui::text::CCursorRange::two(ch, ch)));
            ts.store(ui.ctx(), id);
        }
        // THE FOCUS IS RETURNED TO THE FIELD. A click on a row of the list takes it away while the
        // formula is still being written — and without the return the next letter would fly off
        // nowhere.
        resp.request_focus();
    }

    // THE KEYS ARE LEFT WITH THE FIELD AND NOT WITH THE FOCUS SYSTEM.
    //
    // A measurement (a probe inside a frame): on the frame carrying Escape the field is already NOT
    // focused, though nobody left it — `focused=false, esc=true, lost=true`. The cause is egui itself:
    // it extinguishes focus on Escape at the start of the frame, before any widget. While that holds,
    // the field can neither close its list nor tell a cancellation from a commit: Escape would look like
    // a loss of focus, that is, like AGREEMENT.
    //
    // The regular remedy is the filter lock: `escape` stays with the field always, `tab` while the list
    // is open (there Tab inserts what is chosen); with the list closed Tab walks the fields again as it
    // ought to.
    if ui.memory(|m| m.has_focus(id)) {
        let filter = egui::EventFilter { tab: st.open, horizontal_arrows: true, vertical_arrows: true, escape: true };
        ui.memory_mut(|m| m.set_focus_lock_filter(id, filter));
    }

    // COMMIT AND CANCELLATION. With the list CLOSED, Escape cancels the whole edit; with it open Escape
    // closes the list (above) and the popup of the tool stays where it is.
    // THE COMMIT IS AN EDGE, NOT A STATE. It used to hang on `lost_focus()` alone, and that worked while
    // the flag lasted exactly one frame. Since egui 0.35 it stays raised longer, and the edit went into
    // the model TWICE - the guard beside this file caught it on the very first run after the upgrade.
    //
    // What is asked instead is the transition: the field HAD the focus and no longer has it. That is the
    // moment a person finished typing, and it happens once however long any flag lingers.
    let has_focus = ui.memory(|m| m.has_focus(id));
    let had_focus = ui.data_mut(|d| {
        let key = id.with("was-focused");
        let was = d.get_temp::<bool>(key).unwrap_or(false);
        d.insert_temp(key, has_focus);
        was
    });
    let mut committed = had_focus && !has_focus && !took && !escaped;
    let cancelled = escaped;
    if escaped {
        committed = false;
        resp.surrender_focus();
    }
    // A REFUSAL LEAVES A PERSON WHERE THEY WERE: the text is intact, the caret is in the field. Losing
    // what was typed is the worst thing a field can do, because it is not clear who did it.
    let refused = committed && !valid(buf.trim());
    if refused {
        committed = false;
        resp.request_focus();
    }
    if committed || cancelled {
        st.buf = None;
        st.base = None;
        st.open = false;
        st.sel = 0;
    }
    let text = if cancelled { model.to_string() } else { buf };
    ui.data_mut(|d| d.insert_temp(id, st));
    // THE REFUSAL DOES ITS WORK RIGHT HERE: the letters are intact, the focus is in the field. There is
    // no point handing it outwards — the reason is told by `expr_value_label` next to the field, not by
    // the caller acting on a flag.
    ExprOut { resp, text, committed, cancelled }
}

/// What the field told the frame.
pub struct ExprOut {
    pub resp: egui::Response,
    /// The text that is in the field right now.
    pub text: String,
    /// COMMITTED. This flag alone is what makes the caller touch the model — and only inside
    /// `App::edit(...)`, so that the edit becomes one step of undo.
    pub committed: bool,
    /// Cancelled by Escape: the buffer was thrown away, the model was not touched.
    ///
    /// ONLY THE BEHAVIOUR CHECK READS IT — and that is the right consumer: the contract that Escape
    /// cancels the edit rather than closing the list is held by that check and not by eye. The ban is
    /// lifted narrowly so that the field does not have to be invented anew when the contract is needed
    /// in code.
    #[allow(dead_code)]
    pub cancelled: bool,
}

/// The field reports its open list, once per frame it is drawn with one.
pub fn note_list_open(ctx: &egui::Context) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(LIST_OPEN), true));
}

/// THE CARET'S POSITION IN THE BUFFER, IN BYTES.
///
/// egui counts the caret in CHARACTERS while the text functions work in bytes; names may be written in any
/// alphabet, so confusing the two cuts a string in the middle of a letter. With no state yet (the first
/// frame) the caret is taken to be at the end — that is where a person who has just typed something is.
pub fn caret_byte(ctx: &egui::Context, id: egui::Id, buf: &str) -> usize {
    let chars = egui::TextEdit::load_state(ctx, id).and_then(|s| s.cursor.char_range()).map(|r| r.primary.index.0).unwrap_or_else(|| buf.chars().count());
    buf.char_indices().nth(chars).map(|(i, _)| i).unwrap_or(buf.len())
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
pub fn current_token(text: &str, caret: usize) -> (usize, usize, &str) {
    let caret = caret.min(text.len());
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    let start = text[..caret].rfind(|c: char| !is_word(c)).map(|i| i + c_len(text, i)).unwrap_or(0);
    let end = caret + text[caret..].find(|c: char| !is_word(c)).unwrap_or(text.len() - caret);
    (start, end, &text[start..end])
}

/// Put a driver's name in place of the word under the caret. Returns the new text and the byte offset
/// the caret lands on — right behind the name, so that typing carries on from there.
///
/// What comes after the word is KEPT. It used to be dropped: the result was assembled as head plus name,
/// so choosing `len` in `10+le*2` gave `10+len` and the `*2` was gone.
pub fn insert_driver(text: &str, name: &str, caret: usize) -> (String, usize) {
    let (start, end, _) = current_token(text, caret);
    (format!("{}{}{}", &text[..start], name, &text[end..]), start + name.len())
}

/// The length of a character in bytes, from its first byte — so that a string is never cut in the middle
/// of a letter (names may be written in any alphabet, and `i + 1` panics there).
pub fn c_len(text: &str, i: usize) -> usize {
    text[i..].chars().next().map(|c| c.len_utf8()).unwrap_or(1)
}

/// The state of one field between frames. It sits in the temporary memory of egui under the `id` of the
/// field: that way the widget needs no access to `App`, so it can be called even from where `App` is
/// already borrowed whole.
#[derive(Clone, Default)]
pub struct FieldState {
    /// The text being typed. `None` means the field is not being edited and what is in the model is
    /// shown.
    pub buf: Option<String>,
    /// The list is open.
    pub open: bool,
    /// The chosen row of the list (for the arrows).
    pub sel: usize,
    /// What the model held when the buffer was taken: a buffer nobody has typed into since yields to a model
    /// changed from elsewhere.
    pub base: Option<String>,
}

/// The height of the drop-down list; past that it scrolls.
pub const LIST_MAX_H: f32 = 220.0;

/// How many rows are shown before saying "N more".
pub const LIST_MAX_ROWS: usize = 8;

/// THE KEY UNDER WHICH A FIELD SAYS "MY LIST IS OPEN RIGHT NOW".
pub const LIST_OPEN: &str = "qym_expr_list_open";

/// The sign of a gizmo ring's rotation. `axis_depth` is the component of the rotation axis along the
/// INTO-THE-SCREEN direction (`basis.2`, where depth grows AWAY from the camera). With the axis pointing
/// TOWARDS the viewer (depth < 0), a visually counter-clockwise drag is a POSITIVE right-handed angle. The
/// sign used to be inverted — dragging to the right gave negative degrees and turned the part the other way.
pub fn ring_drag_sign(axis_depth: f64) -> f64 {
    if axis_depth >= 0.0 {
        -1.0
    } else {
        1.0
    }
}

/// THE CHORD PRESSED THIS FRAME that a tool key may answer to, or `None`.
///
/// FOCUS IN A FIELD MUST NOT KILL EVERY KEY - and must not let a letter through either. A bare letter in a
/// field types itself (expressions hold both `w` and `len`); Alt plus a letter types nothing, so it goes to the
/// command. The rule is one: with no focus, the bare key; with focus, Alt.
///
/// CHORDS: a Ctrl or Control chord that can be bound types nothing in a field - the letters a field edits with
/// under either are refused at binding (`platform_keys`) - so it is heard as it is, focus or not. Without them the
/// rule above holds, Shift included: Shift+W in a field is a capital letter.
///
/// "CTRL" IS THE COMMAND KEY: Cmd on a Mac. The Mac's own Control key is a modifier of its own (`Chord::of_press`).
///
/// ANY BINDABLE KEY, NOT A FIXED LIST. Twenty-three letters were listed by hand in the handler, and a tool
/// rebound to W or Z was saved, shown in the window, and never heard: the press did not reach the table.
pub fn pressed_chord(ctx: &egui::Context) -> Option<Chord> {
    // asked BEFORE `ctx.input`: inside it `wants_keyboard_input` deadlocks on the input lock
    let typing = ctx.egui_wants_keyboard_input();
    ctx.input(|i| {
        let held = Chord::of_press(i.modifiers, egui::Key::Escape);
        let chord_key = held.ctrl || held.control;
        // Alt and Ctrl together reach nothing: Alt is the way out of a field, Ctrl and Control do not need one
        let ok = if typing { i.modifiers.alt != chord_key } else { !i.modifiers.alt };
        if !ok {
            return None;
        }
        i.events.iter().find_map(|e| match e {
            egui::Event::Key { key, pressed: true, repeat: false, .. } => Some(Chord { key: *key, ..held }),
            _ => None,
        })
    })
    .filter(Chord::bindable_key)
}

/// Which action a key press means in this area. It reads the settings and nothing else - the whole
/// application was never needed for a table lookup.
pub fn hotkey_action(set: &Settings, area: &str, key: impl Into<Chord>) -> Option<&'static str> {
    let pressed = key.into();
    // A BINDING THIS SYSTEM REFUSES IS NOT RUN. The settings travel with the profile: Cmd+W bound on a Mac arrives
    // on Linux as Ctrl+W, which a field erases a word with - running the tool there would do both.
    HOTKEYS.iter().filter(|r| r.area == area).find(|r| Chord::parse(&hotkey_key(set, r.action)) == Some(pressed)).map(|r| r.action).filter(|a| hotkey_refusal(a, &pressed).is_none())
}

pub fn sel_point_ids(sel_sk: &SketchSelection) -> Vec<Id> {
    sel_sk.items.iter().filter(|(k, _)| *k == 0).map(|(_, id)| *id).collect()
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
pub fn ask_delete(deferred: &mut DeferredUi, sel: Sel) {
    deferred.delete = Some(sel);
}

/// The centre of the selected object (for the gizmo), in the part's XY coordinates.
pub fn selected_centroid(project: &Project, sel: &Sel) -> Option<Point2> {
    match *sel {
        Sel::Contour(i) => project.contours.get(i).map(|c| c.centroid()),
        Sel::Mesh(i) => project.bodies.get(i).map(|b| &b.mesh).and_then(|m| m.bounds()).map(|b| Point2::new((b.min.x + b.max.x) / 2.0, (b.min.y + b.max.y) / 2.0)),
        _ => None,
    }
}

pub fn sel_line_pts(project: &Project, sel_sk: &SketchSelection, si: usize) -> Vec<(Id, Id)> {
    use qymcad_core::model::EntityKind;
    let Some(s) = project.sketches.get(si) else { return Vec::new() };
    sel_sk
        .items
        .iter()
        .filter(|(k, _)| *k == 1)
        .filter_map(|(_, eid)| {
            s.entities.iter().find(|e| e.id == *eid).and_then(|e| match e.kind {
                EntityKind::Line { a, b } => Some((a, b)),
                _ => None,
            })
        })
        .collect()
}

/// The (centre, radius) of the first selected circle entity — for a tangency.
pub fn sel_circle_cr(project: &Project, sel_sk: &SketchSelection, si: usize) -> Option<(Id, f64)> {
    use qymcad_core::model::EntityKind;
    let s = project.sketches.get(si)?;
    let pos = |id: Id| s.points.iter().find(|q| q.id == id).map(|q| (q.x, q.y));
    sel_sk.items.iter().filter(|(k, _)| *k == 1).find_map(|(_, eid)| {
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

/// The ends of an existing LINE on which the point `p` lies (somewhere along it, not at an end) — for
/// the automatic point-on-edge. Excludes the line (ea, eb) and any line where `p` is an end.
pub fn line_under_point(project: &Project, view: &View2d, si: usize, p: Point2, ea: Id, eb: Id) -> Option<(Id, Id)> {
    use qymcad_core::model::EntityKind;
    let s = project.sketches.get(si)?;
    let points_by_id: std::collections::HashMap<Id, &qymcad_core::model::SketchPoint> = s.points.iter().map(|p| (p.id, p)).collect(); // a table: these are looked up for every entity or constraint
    let pt = |id: Id| points_by_id.get(&id).copied().map(|q| Point2::new(q.x, q.y));
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
                if dist_world * view.scale as f64 <= 2.0 {
                    return Some((a, b));
                }
            }
        }
    }
    None
}

/// Whether the segments a1 -> b1 and a2 -> b2 are parallel (their normalised cross product is under 5%).
pub fn lines_parallel(dc: &DrawCtx, si: usize, a1: Id, b1: Id, a2: Id, b2: Id) -> bool {
    match (sketch_pt(dc.project, si, a1), sketch_pt(dc.project, si, b1), sketch_pt(dc.project, si, a2), sketch_pt(dc.project, si, b2)) {
        (Some(p1), Some(p2), Some(p3), Some(p4)) => {
            let (u, v) = ((p2.x - p1.x, p2.y - p1.y), (p4.x - p3.x, p4.y - p3.y));
            let cross = u.0 * v.1 - u.1 * v.0;
            let (lu, lv) = ((u.0 * u.0 + u.1 * u.1).sqrt(), (v.0 * v.0 + v.1 * v.1).sqrt());
            cross.abs() / (lu * lv).max(1e-9) < 0.05
        }
        _ => false,
    }
}

pub fn fit(project: &Project, view: &mut View2d, rect: Rect) {
    let mut b = bounds(&project.contours);
    for body in &project.bodies {
        if let Some(mb) = body.mesh.bounds() {
            let lo = Point2::new(mb.min.x, mb.min.y);
            let hi = Point2::new(mb.max.x, mb.max.y);
            b = Some(match b {
                None => (lo, hi),
                Some((bl, bh)) => (Point2::new(bl.x.min(lo.x), bl.y.min(lo.y)), Point2::new(bh.x.max(hi.x), bh.y.max(hi.y))),
            });
        }
    }
    let Some(b) = b else { return };
    // a sketch under 1 mm is framed at its own size, as the 3D view frames a model; only one with no extent at all, a
    // single point, takes 1 mm, and a side of no length (a lone line) divides nothing
    let raw = (b.1.x - b.0.x).max(b.1.y - b.0.y);
    let (w, h) = if raw > 1e-9 { ((b.1.x - b.0.x).max(raw * 1e-9) as f32, (b.1.y - b.0.y).max(raw * 1e-9) as f32) } else { (1.0, 1.0) };
    view.scale = (rect.width() / w).min(rect.height() / h) * 0.85;
    view.fit = view.scale;
    view.center = Vec2::new(((b.0.x + b.1.x) / 2.0) as f32, ((b.0.y + b.1.y) / 2.0) as f32);
    view.initialized = true;
}

/// The bytes of the default system font (used for text); cached.
pub fn any_font_at_all(font_cache: &mut FontCache) -> bool {
    let mut probe = qymcad_core::model::FontRef::default();
    font_cache.for_tool(&mut probe).is_some()
}

/// THE FONTS THE APPLICATION HAS IN HAND.
///
/// `chosen` is what the text tool writes with - the file the person picked last, or the system font it fell
/// back to. `files` holds the bytes by path, so one file is read once.
///
/// Why this is not simply a pile of bytes any more: a text that is re-baked has to be re-baked IN ITS OWN
/// font, and the only way to find that font again is the name and the path recorded with the text. Measured
/// on a label written in Liberation Sans and edited after a reopen: it came back in Cantarell.
#[derive(Default)]
pub struct FontCache {
    /// The list of installed faces and the state of the window that shows it.
    pub picker: FontPicker,
    files: std::collections::HashMap<String, Vec<u8>>,
}

/// THE WINDOW THAT SHOWS THE INSTALLED FONTS: whether it is open, what is being searched for, and the faces
/// found.
///
/// It lives beside the fonts rather than in the application object, which takes nothing new by rule; and the
/// list belongs here anyway, because walking the font tree costs a noticeable moment and is done once.
#[derive(Default)]
pub struct FontPicker {
    pub open: bool,
    /// Pictures of the names, one per face and size - see `text_texture`.
    textures: std::collections::HashMap<String, egui::TextureHandle>,
    /// The choice is being made FOR THE LABEL BEING EDITED, not for the tool.
    ///
    /// The same list serves both, and what is chosen has to land where it was asked for: a font picked while
    /// editing a label changes THAT label, not what the next click will write.
    pub for_label: bool,
    pub search: String,
    /// Every face found on this machine. Empty until the walk that fills it comes back.
    pub faces: Vec<FontFace>,
    /// The walk, while it is still going.
    coming: Option<std::sync::mpsc::Receiver<Vec<FontFace>>>,
}

impl FontPicker {
    /// START THE WALK OVER THE FONT FOLDERS, in a thread of its own, and return at once.
    ///
    /// Measured on a real machine: the walk takes 536 ms for 232 faces - reading the names out of every file
    /// under half a dozen folders. Done inside a frame that is half a second of a frozen window every time
    /// the list is opened for the first time, and a font tree of several thousand files makes it worse.
    pub fn start_scan(&mut self) {
        if !self.faces.is_empty() || self.coming.is_some() {
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(installed_fonts());
        });
        self.coming = Some(rx);
    }

    /// Take the result if it has arrived. Answers whether the list is ready to be shown.
    pub fn poll_scan(&mut self) -> bool {
        if let Some(rx) = &self.coming {
            match rx.try_recv() {
                Ok(faces) => {
                    self.faces = faces;
                    self.coming = None;
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => self.coming = None, // the thread died; nothing to wait for
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
            }
        }
        !self.faces.is_empty()
    }

    /// Is the walk still going.
    pub fn scanning(&self) -> bool {
        self.coming.is_some()
    }
}

impl FontCache {
    /// The bytes of a font that was recorded somewhere - with a text, or as the current choice. `None` when
    /// nothing is recorded or the file is no longer there.
    pub fn bytes(&mut self, font: &qymcad_core::model::FontRef) -> Option<Vec<u8>> {
        if font.path.is_empty() {
            return None;
        }
        if let Some(b) = self.files.get(&font.path) {
            return Some(b.clone());
        }
        let b = std::fs::read(&font.path).ok()?;
        self.files.insert(font.path.clone(), b.clone());
        Some(b)
    }

    /// Read a font file and say what family it holds, keeping the bytes. `None` if the file cannot be read or
    /// is not a font.
    pub fn read(&mut self, path: &str, index: u32) -> Option<qymcad_core::model::FontRef> {
        let bytes = std::fs::read(path).ok()?;
        let family = qymcad_core::text::family_name(&bytes, index)?;
        self.files.insert(path.to_string(), bytes);
        Some(qymcad_core::model::FontRef { family, path: path.to_string(), index })
    }

    /// THE FONT A TOOL WILL WRITE WITH: what it already carries, or the first font of the system that can be
    /// read - which is then written into the tool, so that what is used is what is shown in the bar.
    pub fn for_tool(&mut self, tool_font: &mut qymcad_core::model::FontRef) -> Option<(qymcad_core::model::FontRef, Vec<u8>)> {
        if !tool_font.path.is_empty() {
            if let Some(b) = self.bytes(&tool_font.clone()) {
                return Some((tool_font.clone(), b));
            }
        }
        for path in system_font_paths() {
            if let Some(f) = self.read(&path, 0) {
                let b = self.bytes(&f)?;
                *tool_font = f.clone();
                return Some((f, b));
            }
        }
        None
    }
}

/// THE FACES THAT MATCH WHAT IS BEING SEARCHED FOR - by family or by style, ignoring case.
///
/// A function of its own so it can be measured without a window: what a person types is the only thing
/// standing between a list of six hundred faces and the one they want.
pub fn fonts_matching(faces: &[FontFace], query: &str) -> Vec<FontFace> {
    let q = query.trim().to_lowercase();
    faces.iter().filter(|f| q.is_empty() || f.family.to_lowercase().contains(&q) || f.style.to_lowercase().contains(&q)).cloned().collect()
}

/// EVERY POPUP A TEXT NEEDS, drawn in one call: the editor of a label and the list of fonts.
///
/// One door on purpose - the application object takes no logic by rule, and two calls plus the answer to
/// "the person asked for a file" would be five lines of it. What the list asks for goes into the ordinary
/// queue of requests, the same way every other button of the interface asks the application for a file.
pub fn text_popups(mut ed: Editing, font_cache: &mut FontCache, tc: &mut TextCtx, ctx: &egui::Context, rect: Rect) -> Vec<BarAsk> {
    let editing = tc.inline.text();
    // the preview of the text tool follows the string as it is typed: re-baked only when the string, the height or
    // the font changed, not every frame
    // A NOTE IS DRAWN IN THE INTERFACE'S FONT, so its letters are asked of that font and not of the tool's: a letter
    // the interface cannot draw is a box on the sheet, whatever the tool's font holds
    if tc.armed.draw_kind() == 11 && tc.writes_note {
        let font = egui::FontId::proportional(14.0);
        let kept: String = tc.text.chars().filter(|c| c.is_whitespace() || ctx.fonts_mut(|f| f.has_glyph(&font, *c))).collect();
        if kept != *tc.text {
            *tc.text = kept;
            *ed.status = qymcad_i18n::tr("sk-note-no-letters");
        }
    } else if tc.armed.draw_kind() == 11 {
        let stale = tc.tool.text_ghost.as_ref().is_none_or(|(t, h, f, _)| t != &*tc.text || *h != tc.tool_text_height || f != &*tc.font);
        if stale {
            // a copy: finding the face a font stands for fills the name in, and the bar would then name a font the
            // person never chose - "Font..." must stay until one is
            let mut asked = tc.font.clone();
            let font = font_cache.for_tool(&mut asked).map(|(f, _)| f).unwrap_or_default();
            if let Some(bytes) = font_cache.bytes(&font) {
                // A LETTER THE FONT DOES NOT HAVE IS NOT TAKEN: it would be a box in the field and nothing on the
                // sheet. It is taken out, and the status line names the font that lacks it.
                let kept: String = tc.text.chars().filter(|c| c.is_whitespace() || qymcad_core::text::can_write(&bytes, font.index, &c.to_string())).collect();
                tc.tool.text_refused = kept != *tc.text || (tc.tool.text_refused && tc.text.trim().is_empty());
                if kept != *tc.text {
                    *tc.text = kept;
                    *ed.status = qymcad_i18n::tr1("sk-text-no-letters", "name", &font_label(&font, "opt-font"));
                }
                // what the font does write, the field shows in it: the tool's font stands behind the interface's
                // own, so a script the interface has no letters for is drawn rather than boxed (egui adds a font of
                // a name once)
                let mut data = egui::FontData::from_owned(bytes);
                data.index = font.index;
                ctx.add_font(egui::epaint::text::FontInsert::new(
                    &format!("tool-font:{}#{}", font.path, font.index),
                    data,
                    vec![egui::epaint::text::InsertFontFamily { family: egui::FontFamily::Proportional, priority: egui::epaint::text::FontPriority::Lowest }],
                ));
            }
            let glyphs = bake_text_glyphs(font_cache, &font, GlyphPlace { at: Point2::new(0.0, 0.0), height: tc.tool_text_height, angle: 0.0 }, tc.text);
            tc.tool.text_ghost = Some((tc.text.clone(), tc.tool_text_height, tc.font.clone(), glyphs));
        }
    }
    text_obj_editor(ed.reborrow(), font_cache, tc, ctx, rect);
    let mut want_file = false;
    if let Some(f) = font_picker_window(font_cache, ctx, &mut want_file) {
        *ed.status = qymcad_i18n::tr1("pk-font-is", "name", &f.family);
        if font_cache.picker.for_label {
            // THE LABEL BEING EDITED CHANGES ITS FONT, at once and visibly: the glyphs are re-baked in the
            // face just chosen, with the string and the height as they stand in the popup.
            font_cache.picker.for_label = false;
            if let (Sel::Sketch(si), Some(ti)) = (*ed.sel, editing) {
                let at = ed.project.sketches[si].texts.get(ti).map(|t| (t.x, t.y, t.angle));
                if let Some((x, y, angle)) = at {
                    let (txt, h) = (tc.annot.text_buf.clone(), tc.annot.text_h);
                    let glyphs = bake_text_glyphs(font_cache, &f, GlyphPlace { at: Point2::new(x, y), height: h, angle }, &txt);
                    ed.project.set_sketch_text(si, ti, qymcad_core::model::TextSpec { at: Point2::new(x, y), height: h, angle, text: txt, glyphs, font: f });
                    invalidate(ed.regen);
                }
            }
        } else {
            *tc.font = f;
        }
    }
    if want_file {
        vec![BarAsk::PickFontFile]
    } else {
        Vec::new()
    }
}

/// ONE ROW OF THE LIST: the family and the style, written in that very face.
///
/// DRAWN AS OUTLINES, NOT AS A FILLED SHAPE. A glyph is not convex - `o` and `e` have holes, `s` bends back
/// on itself - and asking a painter for a convex polygon per loop gives torn letters with their holes filled
/// in. Measured on the list: every second face came out ragged. The sketch itself draws its text as outlines
/// too, so the row shows exactly what the canvas will.
///
/// A face that cannot write its own name is shown in the interface font and marked instead: an icon font
/// holds thousands of glyphs and not one letter, and chosen by mistake it puts nothing into the sketch at
/// all. Such a row does not answer to a click - `Sense::hover` rather than `click`.
pub fn font_row(ui: &mut egui::Ui, cache: &mut FontCache, f: &FontFace, row: f32) -> egui::Response {
    let label = font_row_text(f);
    let font = qymcad_core::model::FontRef { family: f.family.clone(), path: f.path.clone(), index: f.index };
    let bytes = cache.bytes(&font);
    let writes = bytes.as_deref().is_some_and(|b| qymcad_core::text::can_write(b, f.index, &label));
    let sense = if writes { egui::Sense::click() } else { egui::Sense::hover() };
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), row), sense);
    // THE FILE, ON HOVER. The row shows the name; where the name is shared by two editions the file tells
    // them apart, and a long file name runs off the edge of the row. The whole path is one hover away.
    let resp = resp.on_hover_text(&f.path);
    if resp.hovered() && writes {
        ui.painter().rect_filled(rect, 2.0, ui.visuals().widgets.hovered.bg_fill);
    }
    match (&bytes, writes) {
        (Some(b), true) => {
            // THE NAME IS RASTERISED IN ITS OWN FACE, not drawn as outlines.
            //
            // Reported behaviour, with a screenshot: "the fonts are still drawn wrong". Outlines were the
            // first answer and they are right on the canvas of a sketch, where a line is a line; in a list
            // row they are not. A row is 26 px tall, a window is one pixel to the point, and a letter drawn
            // as a one-pixel outline of its inner and outer edge at that size comes out as a smear. What a
            // person needs here is to see the face, which is what a rasteriser gives.
            let px = (row - 10.0).max(8.0);
            let key = format!("{}#{}@{:.0}", f.path, f.index, px);
            let ink = ui.visuals().strong_text_color();
            match text_texture(ui.ctx(), &mut cache.picker.textures, &key, b, px, &label) {
                Some(tex) => {
                    let size = tex.size_vec2();
                    let at = egui::Rect::from_min_size(rect.left_top() + egui::vec2(8.0, (row - size.y) / 2.0), size);
                    let mut mesh = egui::Mesh::with_texture(tex.id());
                    mesh.add_rect_with_uv(at, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), ink);
                    ui.painter().add(egui::Shape::mesh(mesh));
                }
                None => {
                    ui.painter().text(rect.left_center() + egui::vec2(8.0, 0.0), egui::Align2::LEFT_CENTER, &label, egui::TextStyle::Body.resolve(ui.style()), ink);
                }
            }
        }
        _ => {
            ui.painter().text(
                rect.left_center() + egui::vec2(8.0, 0.0),
                egui::Align2::LEFT_CENTER,
                format!("{} {} - {}", egui_phosphor::regular::WARNING, label, qymcad_i18n::tr("font-no-letters")),
                egui::TextStyle::Body.resolve(ui.style()),
                ui.visuals().weak_text_color(),
            );
        }
    }
    resp
}

/// THE WINDOW OF INSTALLED FONTS: a search field, the faces found, each drawn in its own outlines, and the
/// button that still opens a file of one's own.
///
/// Returns the face a person chose, or `None` while they have not. Sets `want_file` when they ask for the
/// file dialog instead - a Snap sees nothing under `~/.local/share/fonts`, so the button is the only way to
/// that font there, and the portal hands the file over with no permission at all.
///
/// The preview is drawn with OUR OWN outlines, the same ones the text tool will cut into the part: a font
/// shown in the interface font would say nothing about what comes out in the drawing. Only the visible rows
/// are drawn, because parsing a face costs milliseconds and a system holds hundreds of them.
pub fn font_picker_window(cache: &mut FontCache, ctx: &egui::Context, want_file: &mut bool) -> Option<qymcad_core::model::FontRef> {
    if !cache.picker.open {
        return None;
    }
    cache.picker.start_scan();
    let ready = cache.picker.poll_scan();
    let shown = fonts_matching(&cache.picker.faces, &cache.picker.search);

    let mut chosen: Option<qymcad_core::model::FontRef> = None;
    let mut open = true;
    egui::Window::new(qymcad_i18n::tr("font-window")).id(egui::Id::new("win_font")).open(&mut open).default_pos(egui::pos2(40.0, 40.0)).default_width(460.0).resizable(true).show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut cache.picker.search).desired_width(200.0).hint_text(qymcad_i18n::tr("font-search")));
            if ui.button(qymcad_i18n::tr("font-from-file")).on_hover_text(qymcad_i18n::tr("opt-pick-font")).clicked() {
                *want_file = true;
            }
        });
        ui.label(if ready { qymcad_i18n::tr1("font-found", "n", &shown.len().to_string()) } else { qymcad_i18n::tr("font-searching") });
        ui.separator();
        let row = 26.0;
        egui::ScrollArea::vertical().max_height(420.0).show_rows(ui, row, shown.len(), |ui, range| {
            for i in range {
                let f = shown[i].clone();
                if font_row(ui, cache, &f, row).clicked() {
                    chosen = Some(qymcad_core::model::FontRef { family: f.family.clone(), path: f.path.clone(), index: f.index });
                }
            }
        });
    });
    if !open || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        cache.picker.open = false;
    }
    if chosen.is_some() {
        cache.picker.open = false;
    }
    chosen
}

/// CAN THIS FONT WRITE THIS STRING - asked of the font a label is about to be written in.
///
/// A font that cannot be read at all answers no: there is nothing to write with either way.
pub fn font_can_write(cache: &mut FontCache, font: &qymcad_core::model::FontRef, text: &str) -> bool {
    cache.bytes(font).is_some_and(|b| qymcad_core::text::can_write(&b, font.index, text))
}

/// A STRING BAKED INTO A TEXTURE, in the font it is given, and kept in a cache.
///
/// TWO CALLERS WANTED THIS, which is why it lives here: the caption of a face of the view cube (stretched
/// over the face, so it is distorted exactly as the face is), and a row of the list of fonts (drawn in the
/// very face it names). Rasterising a font every frame is thousands of glyphs a second for nothing, so the
/// picture is made once per string, size and font.
///
/// ONLY THE ALPHA IS BAKED: the colour comes from the vertices it is drawn with, so one texture serves a
/// dark caption on a light ground and the other way round alike.
pub fn text_texture(ctx: &egui::Context, cache: &mut std::collections::HashMap<String, egui::TextureHandle>, key: &str, font: &[u8], px: f32, text: &str) -> Option<egui::TextureHandle> {
    if let Some(t) = cache.get(key) {
        return Some(t.clone());
    }
    use ab_glyph::{Font, ScaleFont};
    let parsed = ab_glyph::FontRef::try_from_slice(font).ok()?;
    let scaled = parsed.as_scaled(px);
    // THE WIDTH COMES FROM THE GLYPHS THEMSELVES rather than from the number of letters: strings differ in
    // length, and a texture of fixed width would stretch one and squeeze another.
    let glyphs: Vec<_> = text.chars().map(|c| parsed.glyph_id(c)).collect();
    let advance: f32 = glyphs.iter().map(|g| scaled.h_advance(*g)).sum();
    let pad = px * 0.25;
    let w = (advance + pad * 2.0).ceil().max(1.0) as usize;
    let h = (scaled.height() + pad).ceil().max(1.0) as usize;
    let mut alpha = vec![0u8; w * h];
    let mut pen = pad;
    let baseline = pad * 0.5 + scaled.ascent();
    for g in &glyphs {
        let q = g.with_scale_and_position(px, ab_glyph::point(pen, baseline));
        if let Some(outline) = parsed.outline_glyph(q) {
            let bb = outline.px_bounds();
            outline.draw(|gx, gy, c| {
                let (x, y) = (bb.min.x as i32 + gx as i32, bb.min.y as i32 + gy as i32);
                if x >= 0 && y >= 0 && (x as usize) < w && (y as usize) < h {
                    let i = y as usize * w + x as usize;
                    // THE MAXIMUM is taken rather than the sum: neighbouring glyphs overlap, and adding
                    // gives dirty dark patches at the joins.
                    alpha[i] = alpha[i].max((c * 255.0) as u8);
                }
            });
        }
        pen += scaled.h_advance(*g);
    }
    let pixels: Vec<egui::Color32> = alpha.iter().map(|a| egui::Color32::from_white_alpha(*a)).collect();
    let img = egui::ColorImage { size: [w, h], source_size: egui::Vec2::new(w as f32, h as f32), pixels };
    let tex = ctx.load_texture(key, img, egui::TextureOptions::LINEAR);
    cache.insert(key.to_string(), tex.clone());
    Some(tex)
}

/// HOW A FONT IS NAMED IN THE INTERFACE: its family, or the given word when nothing is recorded.
///
/// One function for the two places that name it - the button of the top bar, which says what the text tool
/// will write with, and the editor of a label, which says what THAT label is written in. Spelled out twice
/// they drift, and both are the answer to the same question a person asks before pressing anything: in which
/// font will this be.
pub fn font_label(font: &qymcad_core::model::FontRef, when_unknown: &str) -> String {
    if font.family.trim().is_empty() {
        qymcad_i18n::tr(when_unknown)
    } else {
        font.family.clone()
    }
}

/// TAKE THE FILE A PERSON CHOSE and say what to put in the status line.
///
/// The name comes out of the FILE, not out of the file name: the label keeps the family it was written in,
/// and that is what has to be shown in the bar and looked for later on another machine.
pub fn choose_font(cache: &mut FontCache, tool_font: &mut qymcad_core::model::FontRef, path: &str) -> String {
    match cache.read(path, 0) {
        Some(f) => {
            let said = qymcad_i18n::tr1("pk-font-is", "name", &f.family);
            *tool_font = f;
            said
        }
        None => qymcad_i18n::tr1("pk-font-error", "error", path),
    }
}

/// A FACE A PERSON CAN WRITE WITH: one entry of the list of installed fonts.
///
/// A file is not a face: a `.ttc` collection holds several, and each of them is its own family and style, so
/// the index travels with the path everywhere - into the list, into the record kept with a label, and into
/// the parsing of the outlines.
#[derive(Clone, Debug, PartialEq)]
pub struct FontFace {
    pub family: String,
    pub style: String,
    pub path: String,
    pub index: u32,
    /// WHAT TELLS THIS FACE FROM ITS NAMESAKE, empty when there is none.
    ///
    /// Two different files calling themselves the same thing are ordinary: measured on a real machine,
    /// `CaskaydiaCove Nerd Font Regular` lies there twice, once as a `.ttf` and once as an `.otf` "Complete"
    /// edition - different formats, different sizes, possibly different coverage. Hiding one of them would be
    /// a lie, and two identical lines tell a person nothing about which they are taking.
    pub note: String,
}

/// The line shown for a face: its name, and what tells it from its namesake when there is one.
pub fn font_row_text(f: &FontFace) -> String {
    if f.note.is_empty() {
        format!("{} {}", f.family, f.style)
    } else {
        format!("{} {} - {}", f.family, f.style, f.note)
    }
}

/// WHERE FONTS LIE, per system and per sandbox.
///
/// The sandboxes matter as much as the systems here. Inside Flatpak the host's fonts are mounted read-only
/// under `/run/host` while `/usr/share/fonts` holds only the runtime's own few, and inside Snap the host's
/// tree appears under `/var/lib/snapd/hostfs`. Neither needs a permission of any kind; what they need is to
/// be looked at, or a person is shown three fonts and none of their own.
///
/// What is NOT here, and cannot be: a Snap sees nothing under `~/.local/share/fonts`, because its `home`
/// permission excludes hidden directories on purpose. That is why the "choose a file" button stays beside
/// the list - the portal hands over whatever file a person picks, permission or not.
pub fn font_directories() -> Vec<std::path::PathBuf> {
    let home = std::env::var("HOME").ok();
    let mut dirs: Vec<String> = Vec::new();
    if cfg!(target_os = "windows") {
        dirs.push("C:/Windows/Fonts".into());
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            dirs.push(format!("{local}/Microsoft/Windows/Fonts"));
        }
    } else if cfg!(target_os = "macos") {
        dirs.push("/System/Library/Fonts".into());
        dirs.push("/Library/Fonts".into());
        if let Some(h) = &home {
            dirs.push(format!("{h}/Library/Fonts"));
        }
    } else {
        dirs.push("/usr/share/fonts".into());
        dirs.push("/usr/local/share/fonts".into());
        if let Some(h) = &home {
            dirs.push(format!("{h}/.local/share/fonts"));
            dirs.push(format!("{h}/.fonts"));
        }
        // the host, as the sandboxes show it
        dirs.push("/run/host/fonts".into());
        dirs.push("/run/host/local-fonts".into());
        dirs.push("/run/host/user-fonts".into());
        dirs.push("/var/lib/snapd/hostfs/usr/share/fonts".into());
    }
    dirs.into_iter().map(std::path::PathBuf::from).collect()
}

/// EVERY FACE FOUND UNDER `dirs`, sorted by family and style.
///
/// The walk reads the names out of each file, because a file name says nothing reliable: `DejaVuSans.ttf`
/// and `n019003l.pfb` are the same kind of thing to a directory listing. Files that are not fonts are
/// skipped in silence - a font directory holds licences, caches and READMEs, and none of that is an error.
pub fn installed_fonts_in(dirs: &[std::path::PathBuf]) -> Vec<FontFace> {
    let mut out: Vec<FontFace> = Vec::new();
    let mut stack: Vec<std::path::PathBuf> = dirs.to_vec();
    let mut seen_dirs: std::collections::HashSet<std::path::PathBuf> = std::collections::HashSet::new();
    let mut same: std::collections::HashSet<(String, String, u32, usize)> = std::collections::HashSet::new();
    while let Some(dir) = stack.pop() {
        // A font tree is full of symlinks; the same directory reached twice would list the same faces twice.
        let key = std::fs::canonicalize(&dir).unwrap_or(dir.clone());
        if !seen_dirs.insert(key) {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for e in entries.flatten() {
            let path = e.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("").to_ascii_lowercase();
            if !matches!(ext.as_str(), "ttf" | "otf" | "ttc" | "otc") {
                continue;
            }
            let Ok(bytes) = std::fs::read(&path) else { continue };
            for index in 0..qymcad_core::text::faces_in(&bytes) {
                if let Some((family, style)) = qymcad_core::text::face_name(&bytes, index) {
                    // THE SAME FONT IN TWO FOLDERS IS ONE ROW. Measured on a real machine: 254 faces held
                    // thirteen pairs saying exactly the same thing - one `.ttc` lying in three font folders
                    // at once, and a family copied into a folder inside its own. Real copies, not symlinks,
                    // so canonicalising the directories does not catch them; the names plus the size of the
                    // file do. A person choosing a font must not be shown three identical lines with no way
                    // to tell them apart, because there is nothing to tell.
                    if same.insert((family.clone(), style.clone(), index, bytes.len())) {
                        out.push(FontFace { family, style, path: path.to_string_lossy().into_owned(), index, note: String::new() });
                    }
                }
            }
        }
    }
    out.sort_by_key(|a| (a.family.to_lowercase(), a.style.to_lowercase()));
    // NAMESAKES ARE GIVEN THEIR FILE NAME. Only namesakes: putting the file beside every row would drown the
    // name a person is actually looking for.
    let mut namesakes: std::collections::HashMap<(String, String), usize> = std::collections::HashMap::new();
    for f in &out {
        *namesakes.entry((f.family.clone(), f.style.clone())).or_default() += 1;
    }
    for f in &mut out {
        if namesakes.get(&(f.family.clone(), f.style.clone())).copied().unwrap_or(0) > 1 {
            f.note = std::path::Path::new(&f.path).file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        }
    }
    out
}

/// Every face installed on this machine, sandboxes included.
pub fn installed_fonts() -> Vec<FontFace> {
    installed_fonts_in(&font_directories())
}

/// WHERE A FONT MIGHT BE FOUND when the person has chosen none - one file per system, not a catalogue. The
/// list of installed fonts is a separate matter; this is only the fallback for writing something at all.
fn system_font_paths() -> Vec<String> {
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
    paths
}

pub fn note_editor(annot: &mut AnnotEdit, inline: &mut InlineEdit, project: &mut Project, sel: Sel, view: View2d, ctx: &egui::Context, rect: Rect) {
    let Sel::Sketch(si) = sel else {
        inline.clear();
        return;
    };
    let Some(ni) = inline.note() else { return };
    let Some(at) = project.sketches.get(si).and_then(|s| s.notes.get(ni)).map(|n| (Sheet { view, rect }).at(Point2::new(n.x, n.y))) else {
        inline.clear();
        return;
    };
    let (mut apply, mut close) = (false, false);
    egui::Area::new(egui::Id::new(("noteedit", si, ni))).fixed_pos(clamp_popup(at, rect) + egui::vec2(0.0, -28.0)).order(egui::Order::Foreground).show(ctx, |ui| {
        egui::Frame::popup(ui.style()).show(ui, |ui| {
            ui.horizontal(|ui| {
                let r = ui.add(egui::TextEdit::singleline(&mut annot.note_buf).desired_width(160.0));
                if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    apply = true;
                }
                if ui.button(egui_phosphor::regular::CHECK).clicked() {
                    apply = true;
                }
            });
        });
    });
    if apply {
        project.set_note_text(si, ni, annot.note_buf.clone());
        close = true;
    }
    if close || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        inline.clear(); // a note is not a label: no tool is in hand for it
    }
}

/// CLOSE THE EDITING OF A LABEL and put the text tool down.
///
/// Reported behaviour: "after the tick or Enter the tool stays in hand with the parameters of the edit, and
/// clicking the sketch spams more of them". Taking the tool up when the editor opens is right - the bar is
/// where the string, the height and the font are named - and keeping it after the edit is applied is not:
/// the work is finished and nothing is waiting to be placed.
pub fn end_text_edit(tc: &mut TextCtx) {
    // the string of the label edited is not offered to the next label (found checking issue #32)
    tc.text.clear();
    tc.inline.clear();
    tc.annot.text = None;
    tc.annot.text_focus = false;
    *tc.armed = Armed::None;
    tc.tool.pts.clear();
}

/// Select all the geometry of a sketch: entities (lines, arcs, circles), primitives and free points
/// (apart from the system ones — the origin and the axes). Used by Ctrl+A.
pub fn select_all_sketch(annot: &mut AnnotEdit, gsel: &mut GeomSelection, project: &Project, sel_sk: &mut SketchSelection, status: &mut String, si: usize) {
    let Some(s) = project.sketches.get(si) else { return };
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
    sel_sk.items = sel;
    gsel.constraint = None;
    annot.note = None;
    *status = qymcad_i18n::tr1("g-selected-n", "n", &n.to_string());
}

/// Delete the current sketch selection (entities, primitives and points, apart from the system ones).
pub fn delete_sketch_sel(project: &mut Project, regen: &mut Rebuilding, sel_sk: &mut SketchSelection, status: &mut String, si: usize) {
    let eids: Vec<Id> = sel_sk.items.iter().filter(|(k, _)| *k == 1).map(|(_, id)| *id).collect();
    let eids: Vec<Id> = {
        let s = &project.sketches[si];
        eids.into_iter().filter(|e| s.entities.iter().any(|x| x.id == *e)).collect()
    };
    let sys: std::collections::HashSet<Id> = {
        let s = &project.sketches[si];
        s.immovable_points().into_iter().collect() // system and driven points: they are not deleted one by one
    };
    let pids: Vec<Id> = sel_sk.items.iter().filter(|(k, id)| *k == 0 && !sys.contains(id)).map(|(_, id)| *id).collect();
    // A PIECE OF A PROJECTION DELETED cuts it: the rest is ordinary geometry held by what it plainly is
    // (`delete_sketch_curves`)
    if !eids.is_empty() {
        project.delete_sketch_curves(si, &eids);
    }
    if !pids.is_empty() {
        project.delete_points(si, &pids);
    }
    project.solve_sketch(si);
    sel_sk.clear(); // the selection, and whatever was waiting on it
    invalidate(regen);
    *status = qymcad_i18n::tr("g-deleted");
}

/// Turn a reference object into a REAL point (creating the midpoint or the origin if needed) -> its Id.
pub fn materialize_ref(project: &mut Project, si: usize, r: SketchRef) -> Id {
    use qymcad_core::model::{Constraint, SketchPoint};
    match r {
        SketchRef::Point(id) => id,
        SketchRef::Origin => project.ensure_origin(si),
        SketchRef::Midpoint(a, b) => {
            // reuse an existing midpoint if a Midpoint constraint is already there
            if let Some(p) = project.sketches[si].constraints.iter().find_map(|c| match c {
                Constraint::Midpoint { p, a: ca, b: cb } if (*ca == a && *cb == b) || (*ca == b && *cb == a) => Some(*p),
                _ => None,
            }) {
                return p;
            }
            let (pa, pb) = (sketch_pt(project, si, a), sketch_pt(project, si, b));
            let (mx, my) = match (pa, pb) {
                (Some(pa), Some(pb)) => ((pa.x + pb.x) * 0.5, (pa.y + pb.y) * 0.5),
                _ => (0.0, 0.0),
            };
            let id = project.alloc_id();
            project.sketches[si].points.push(SketchPoint { id, x: mx, y: my });
            project.sketches[si].constraints.push(Constraint::Midpoint { p: id, a, b });
            id
        }
    }
}

pub fn move_body_at(rc: &mut RebuildCtx, mi: usize, mat: [f64; 12]) {
    begin_edit(rc.edits, rc.project, qymcad_i18n::tr("status-move-body")); // THE OPERATION BOUNDARY
    if let Some(id) = brep_at(rc.live, rc.project, mi) {
        let nb = rc.project.add_move(id, mat);
        mark_dirty_for_rebuild(rc); // the document is marked; the scheduler does the computing
        select_body(rc.project, rc.sel, rc.view, nb);
    } else {
        rc.project.bodies[mi].mesh.transform(&mat);
        rc.project.bodies[mi].faces = rc.project.bodies[mi].mesh.detect_faces(8.0);
        invalidate(rc.regen);
    }
    commit_edit(rc);
}

/// In-place editing of a note's text (a double click).
/// Bake the glyph polylines of a text through the active font (world coordinates, baseline point x, y).
/// WHERE A TEXT'S GLYPHS ARE LAID: the point the line of text starts at, the height of its letters, and the angle it is
/// turned by, in degrees.
#[derive(Clone, Copy, Debug)]
pub struct GlyphPlace {
    pub at: Point2,
    pub height: f64,
    pub angle: f64,
}

/// The glyphs of `text` in `font`, laid at `place`. TURNED BY ITS ANGLE: baked level, a turned text edited went back to
/// standing level (found with issue #32).
pub fn bake_text_glyphs(font_cache: &mut FontCache, font: &qymcad_core::model::FontRef, place: GlyphPlace, text: &str) -> Vec<Vec<Point2>> {
    let Some(bytes) = font_cache.bytes(font) else { return Vec::new() };
    let GlyphPlace { at, height, angle } = place;
    let (sn, cs) = angle.to_radians().sin_cos();
    qymcad_core::text::text_outline_contours(&bytes, font.index, text, height, at.x, at.y)
        .into_iter()
        .map(|c| c.points.into_iter().map(|p| Point2::new(at.x + (p.x - at.x) * cs - (p.y - at.y) * sn, at.y + (p.x - at.x) * sn + (p.y - at.y) * cs)).collect())
        .collect()
}

/// The popup for editing a text object: the string plus the height. On apply the glyphs are re-baked and updated.
/// WHAT THE POPUPS OF A TEXT WORK ON: the buffers being typed into, the tool in hand, and the font the tool
/// writes with.
///
/// A record rather than five arguments: the editor of a label has to PUT THE TOOL DOWN when the edit is
/// applied, and that is two more things to reach - past the limit a signature is allowed.
pub struct TextCtx<'a> {
    pub annot: &'a mut AnnotEdit,
    pub inline: &'a mut InlineEdit,
    pub armed: &'a mut Armed,
    pub tool: &'a mut SketchTool,
    /// The font the TEXT TOOL writes with (not the one a label is written in).
    pub font: &'a mut qymcad_core::model::FontRef,
    /// the string and the height the text tool writes, for its preview; a letter its font cannot write is taken out
    pub text: &'a mut String,
    pub tool_text_height: f64,
    /// the text tool writes a note - not geometry, drawn in the interface's own font
    pub writes_note: bool,
}

/// OPEN THE EDITING OF A TEXT OBJECT: what a double click on it does.
///
/// One door for the two callers - the double click in the sketch and the hand of a test. Spelled out in both
/// places it drifts: the buffers are what the popup edits, and a caller that forgets one of them opens the
/// popup over the previous text.
pub fn begin_text_edit(project: &Project, tools: &mut Tools, prefs: &mut SketchToolPrefs, si: usize, ti: usize) {
    let Some(t) = project.sketches.get(si).and_then(|s| s.texts.get(ti)) else { return };
    tools.annot.text_buf = t.text.clone();
    tools.annot.text_h = t.height;
    tools.annot.text = Some(ti);
    tools.annot.text_focus = true; // the caret lands in the field, so typing and Enter work straight away
    *tools.inline = InlineEdit::Text(ti);
    // THE TEXT TOOL IS TAKEN UP, and its options in the top bar are set to what is being edited.
    //
    // Reported behaviour, with a screenshot: "while editing, the tool stays Select". Editing a label IS
    // working with text, and the bar is where its string, height and font are named: a person editing a
    // label looked at the options of the arrow. What follows from taking the tool is the ordinary rule of
    // every tool of the sketch - the next click on the canvas places another label.
    prefs.text = t.text.clone();
    prefs.text_h = t.height;
    prefs.font = t.font.clone();
    // Not `select`, which TOGGLES: pressing the text tool twice puts it down, and opening an editor must
    // always end with the tool in hand.
    *tools.armed = Armed::Draw(11);
    tools.tool.pts.clear();
}

pub fn text_obj_editor(ed: Editing, font_cache: &mut FontCache, tc: &mut TextCtx, ctx: &egui::Context, rect: Rect) {
    let Sel::Sketch(si) = *ed.sel else {
        end_text_edit(tc);
        return;
    };
    let Some(ti) = tc.inline.text() else { return };
    let Some((bx, _by, _, maxy)) = ed.project.sketch_text_bbox(si, ti) else {
        end_text_edit(tc);
        return;
    };
    let at = (Sheet { view: *ed.view, rect }).at(Point2::new(bx, maxy));
    let (mut apply, mut close) = (false, false);
    egui::Area::new(egui::Id::new(("textedit", si, ti))).fixed_pos(clamp_popup(at, rect) + egui::vec2(0.0, -34.0)).order(egui::Order::Foreground).show(ctx, |ui| {
        egui::Frame::popup(ui.style()).show(ui, |ui| {
            ui.horizontal(|ui| {
                let want_focus = tc.annot.text_focus;
                let r = focus_edit(ui, &mut tc.annot.text_buf, 160.0, "", want_focus);
                if r.has_focus() {
                    tc.annot.text_focus = false;
                }
                ui.label(qymcad_i18n::tr("g-height-short"));
                ui.add(egui::DragValue::new(&mut tc.annot.text_h).speed(0.2).range(1.0..=1000.0).suffix(qymcad_i18n::tr("unit-mm-suffix")));
                // WHICH FONT THIS LABEL IS IN. It is re-baked in that one, so it is the one to show; a label
                // made before fonts were recorded says so instead, because its edit will be refused.
                let of_label = ed.project.sketches[si].texts.get(ti).map(|t| t.font.clone()).unwrap_or_default();
                if ui.button(font_label(&of_label, "sk-text-font-unknown-short")).on_hover_text(qymcad_i18n::tr("opt-pick-font")).clicked() {
                    font_cache.picker.open = true;
                    font_cache.picker.for_label = true; // what is chosen changes THIS label
                }
                if (r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))) || ui.button(egui_phosphor::regular::CHECK).clicked() {
                    apply = true;
                }
            });
        });
    });
    if apply {
        // THE LABEL IS RE-BAKED IN ITS OWN FONT, not in whatever the application has in hand. A text that was
        // written in one typeface and edited in another is a change nobody asked for and nobody is told
        // about: measured, a label written in Liberation Sans came back in Cantarell after a reopen.
        let (x, y, angle, font) = {
            let t = &ed.project.sketches[si].texts[ti];
            (t.x, t.y, t.angle, t.font.clone())
        };
        let (txt, h) = (tc.annot.text_buf.clone(), tc.annot.text_h);
        // A label made before the font was recorded, or one whose font file is gone: the drawing stays as it
        // is and the person is told why, rather than being handed a different typeface silently.
        match font_cache.bytes(&font) {
            Some(_) => {
                let glyphs = bake_text_glyphs(font_cache, &font, GlyphPlace { at: Point2::new(x, y), height: h, angle }, &txt);
                ed.project.set_sketch_text(si, ti, qymcad_core::model::TextSpec { at: qymcad_core::geom::Point2::new(x, y), height: h, angle, text: txt, glyphs, font });
                invalidate(ed.regen);
            }
            None => {
                *ed.status = if font.family.is_empty() { qymcad_i18n::tr("sk-text-font-unknown") } else { qymcad_i18n::tr1("sk-text-font-gone", "name", &font.family) };
            }
        }
        close = true;
    }
    if close || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        end_text_edit(tc);
    }
}

/// Draw the header. Returns the new name if a person touched it.
pub fn props_header(ui: &mut egui::Ui, icon: &str, kind_key: &str, name: NameSlot, lin: &Lineage) -> Option<String> {
    ui.heading(format!("{icon} {}", qymcad_i18n::tr(kind_key)));
    let mut renamed = None;
    match name {
        NameSlot::Editable(mut stored) => {
            ui.horizontal(|ui| {
                ui.label(qymcad_i18n::tr("pp-name"));
                if name_edit(ui, &mut stored).changed() {
                    renamed = Some(stored);
                }
            });
        }
        NameSlot::Fixed(s) if !s.is_empty() => {
            ui.label(egui::RichText::new(qymcad_i18n::name(&s)).weak().small());
        }
        _ => {}
    }
    let mut chain = |key: &str, items: &[String]| {
        if items.is_empty() {
            return;
        }
        ui.label(egui::RichText::new(qymcad_i18n::tr(key)).small().weak());
        for it in items {
            ui.label(format!("  · {it}"));
        }
    };
    chain("fp-built-on", &lin.built_on);
    chain("fp-dependents", &lin.dependents);
    ui.separator();
    renamed
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
pub fn name_edit(ui: &mut egui::Ui, stored: &mut String) -> egui::Response {
    let mut shown = qymcad_i18n::name(stored);
    let r = ui.text_edit_singleline(&mut shown);
    if r.changed() {
        *stored = shown;
    }
    r
}

/// The projection of point `p` onto the SEGMENT a -> b (clamped to its ends). None means a degenerate segment.
pub fn project_on_seg(p: Point2, a: Point2, b: Point2) -> Option<Point2> {
    let (vx, vy) = (b.x - a.x, b.y - a.y);
    let len2 = vx * vx + vy * vy;
    if len2 < 1e-12 {
        return None;
    }
    let t = (((p.x - a.x) * vx + (p.y - a.y) * vy) / len2).clamp(0.0, 1.0);
    Some(Point2::new(a.x + t * vx, a.y + t * vy))
}

pub fn bounds(contours: &[Contour]) -> Option<(Point2, Point2)> {
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

/// The Id of a body by its mesh index, IF it is a B-rep body (a live shape exists) — otherwise None (a raw import).
pub fn brep_at(live: &LiveGeom, project: &Project, mi: usize) -> Option<Id> {
    project.mesh_id(mi).filter(|id| live.shapes.contains_key(id))
}

/// Select the feature that produced the body (so its parameters show up at once) and fit the view.
pub fn select_body(project: &mut Project, sel: &mut Sel, view: &mut View2d, body: Id) {
    if let Some(ti) = project.timeline.iter().position(|n| n.kind.body() == Some(body)) {
        *sel = Sel::Feature(ti);
    } else if let Some(mi) = project.mesh_index(body) {
        *sel = Sel::Mesh(mi);
    }
    view.initialized = false;
}

/// The sketch anchor under the cursor - for dimensions and constraints between ANY geometry.
#[derive(Clone, Copy)]
pub enum SketchRef {
    /// An existing point (a circle's or an arc's centre included).
    Point(Id),
    /// The midpoint of segment (a,b) - materialised as a point with a Midpoint constraint.
    Midpoint(Id, Id),
    /// The origin (0,0) - materialised as a Fixed reference point.
    Origin,
}

/// THE NAME OF THE OBJECT in the header: editable, shown only, or absent.
///
/// The edit is returned outwards rather than written here: for a body the name is set by
/// `set_mesh_name`, for a sketch by a field of the structure, and a feature has no name in that sense
/// at all. The write paths differ in substance, and folding them into one would be a lie for the sake
/// of symmetry.
pub enum NameSlot {
    /// The stored value (may be an auto-name key — shown translated, written only if touched).
    Editable(String),
    Fixed(String),
    None,
}

/// THE LINEAGE OF AN OBJECT, ready to be shown: what created it and what depends on it.
#[derive(Default)]
pub struct Lineage {
    pub built_on: Vec<String>,
    pub dependents: Vec<String>,
}

/// The point on the segment a -> b nearest to `p` that also lies on a GRID LINE (x = n*g or y = n*g) — the
/// intersection of an edge with the grid. For a vertical edge it ties Y to the grid (the edge holds X), for
/// a horizontal one it ties X, and for a slanted one it takes the nearest crossing with the grid. None when
/// there is no crossing within the segment.
pub fn grid_cross_on_seg(p: Point2, a: Point2, b: Point2, g: f64) -> Option<Point2> {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    if dx * dx + dy * dy < 1e-12 || g <= 0.0 {
        return None;
    }
    let mut best: Option<(f64, Point2)> = None;
    let mut consider = |q: Point2| {
        let dd = (q.x - p.x).powi(2) + (q.y - p.y).powi(2);
        if best.is_none_or(|(bd, _)| dd < bd) {
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
pub fn seg_seg_intersect(a1: Point2, a2: Point2, b1: Point2, b2: Point2) -> Option<Point2> {
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

/// The intersection of INFINITE lines (through the segments ab and cd) in the sketch's world coordinates.
/// None means they are parallel. Used to orient an angular dimension's directions outwards from the
/// intersection, so the angle shown is the visible opening rather than its supplement.
pub fn line_line_ix(a: Point2, b: Point2, c: Point2, d: Point2) -> Option<Point2> {
    let (rx, ry) = (b.x - a.x, b.y - a.y);
    let (sx, sy) = (d.x - c.x, d.y - c.y);
    let denom = rx * sy - ry * sx;
    if denom.abs() < 1e-9 {
        return None;
    }
    let t = ((c.x - a.x) * sy - (c.y - a.y) * sx) / denom;
    Some(Point2::new(a.x + t * rx, a.y + t * ry))
}

/// The points where the SEGMENT a -> b crosses a circle (centre c, radius r), within the segment.
pub fn seg_circle_intersect(a: Point2, b: Point2, c: Point2, r: f64) -> Vec<Point2> {
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

pub fn constraint_label(c: &qymcad_core::model::Constraint) -> String {
    use qymcad_core::model::Constraint as C;
    // A DIMENSION'S VALUE: either a number, or "expression = number". The expression is typed by hand and is kept as it is.
    let value = |num: String, expr: &str| if expr.trim().is_empty() { num } else { format!("{expr} = {num}") };
    // "Dimension = 5.0" or "Dimension (5.0), driven" — one pair of messages for every kind of dimension.
    let dim = |key: &str, val: String, driven: bool| {
        if driven {
            qymcad_i18n::tr2("con-dim-driven", "what", &qymcad_i18n::tr(key), "value", &val)
        } else {
            qymcad_i18n::tr2("con-dim", "what", &qymcad_i18n::tr(key), "value", &val)
        }
    };
    match c {
        C::Fixed { .. } => qymcad_i18n::tr("con-fixed"),
        C::Horizontal { .. } => qymcad_i18n::tr("con-horizontal"),
        C::Vertical { .. } => qymcad_i18n::tr("con-vertical"),
        // never listed, but named where a conflict has to name what holds the rectangle
        C::Orientation { .. } => qymcad_i18n::tr("con-rect-turn"),
        C::Coincident { .. } => qymcad_i18n::tr("con-coincident"),
        C::Distance { d, driven, expr, .. } => dim("con-name-distance", value(qymcad_i18n::num(*d, 1), expr), *driven),
        C::Parallel { .. } => qymcad_i18n::tr("con-parallel"),
        C::Perpendicular { .. } => qymcad_i18n::tr("con-perpendicular"),
        C::Equal { .. } => qymcad_i18n::tr("con-equal-length"),
        C::EqualRadius { .. } => qymcad_i18n::tr("con-equal-radius"),
        C::CircleTangent { external, .. } => qymcad_i18n::tr(if *external { "con-circle-tangent-out" } else { "con-circle-tangent-in" }),
        C::EdgeDistance { d, driven, expr, .. } => dim("con-name-edge-distance", value(qymcad_i18n::num(*d, 1), expr), *driven),
        C::PointOnCircle { .. } => qymcad_i18n::tr("con-point-on-circle"),
        C::Concentric { .. } => qymcad_i18n::tr("con-concentric"),
        C::Angle { deg, driven, .. } => dim("con-name-angle", format!("{}°", qymcad_i18n::num(*deg, 0)), *driven),
        C::AngleLines { deg, driven, expr, .. } => dim("con-name-angle", value(format!("{}°", qymcad_i18n::num(*deg, 0)), expr), *driven),
        C::ArcLength { len, driven, expr, .. } => dim("con-name-arc-length", value(qymcad_i18n::num(*len, 1), expr), *driven),
        C::Collinear { .. } => qymcad_i18n::tr("con-collinear"),
        C::Midpoint { .. } => qymcad_i18n::tr("con-midpoint"),
        C::Tangent { .. } => qymcad_i18n::tr("con-tangent"),
        C::Symmetric { .. } => qymcad_i18n::tr("con-symmetric"),
        C::PointOnLine { .. } => qymcad_i18n::tr("con-point-on-line"),
        // the diameter and radius signs are symbols rather than words: the same in every language
        C::Diameter { d, driven, diam, expr, .. } => {
            let val = value(qymcad_i18n::num(*d, 1), expr);
            let pfx = if *diam { "Ø" } else { "R" };
            if *driven {
                qymcad_i18n::tr2("con-dim-driven", "what", pfx, "value", &val)
            } else {
                qymcad_i18n::tr2("con-dim", "what", pfx, "value", &val)
            }
        }
        C::DistancePL { d, driven, expr, .. } => dim("con-name-distance-pl", value(qymcad_i18n::num(*d, 1), expr), *driven),
    }
}

/// A 3x4 row-major translation matrix (for the Move feature and for a body's transform).
pub fn mat_translate(dx: f64, dy: f64, dz: f64) -> [f64; 12] {
    [1.0, 0.0, 0.0, dx, 0.0, 1.0, 0.0, dy, 0.0, 0.0, 1.0, dz]
}

/// Is the line (a, b) a coordinate axis of the sketch (one of its axis reference points)?
pub fn is_axis_line(project: &qymcad_core::model::Project, si: usize, a: Id, b: Id) -> bool {
    project.sketches.get(si).is_some_and(|s| s.axis_pts.contains(&a) || s.axis_pts.contains(&b))
}

/// The LINES of the profile sketch that are candidates for the axis of revolution (a centreline). They
/// are chosen by a button or a combo box on the bar, NOT by a click in 3D: sketch geometry is not
/// hit-tested in the command's 3D view.
pub fn profile_axis_lines(project: &qymcad_core::model::Project, si: usize) -> Vec<Id> {
    // ANY straight line of the sketch can be the axis, not only a construction one — the usual CAD
    // behaviour. The core never minded; the restriction sat right here, in the list of candidates, and a
    // plain line drawn to serve as an axis simply did not appear on the command's bar. Reported
    // behaviour: only X or Y could be chosen, an axis of one's own could not.
    // Construction lines come FIRST: they are drawn precisely to serve as an axis, and that is the most
    // frequent choice.
    let Some(s) = project.sketches.get(si) else { return Vec::new() };
    let lines = |constr: bool| s.entities.iter().filter(move |e| e.construction == constr && matches!(e.kind, qymcad_core::model::EntityKind::Line { .. })).map(|e| e.id);
    lines(true).chain(lines(false)).collect()
}

/// The closed contour of sketch `si` under a screen point (point-in-polygon; the smaller area wins).
pub fn contour_under_2d(project: &Project, view: &View2d, rect: Rect, screen: Pos2, si: usize) -> Option<Id> {
    let mut best: Option<(f64, Id)> = None;
    for cid in sketch_closed_contours(project, si) {
        let ci = project.contour_index(cid)?;
        let pts: Vec<Pos2> = project.contours[ci].points.iter().map(|p| (Sheet { view: *view, rect }).at(*p)).collect();
        if pts.len() >= 3 && point_in_poly(screen, &pts) {
            let area = poly_area(&pts);
            if best.is_none_or(|(ba, _)| area < ba) {
                best = Some((area, cid));
            }
        }
    }
    best.map(|(_, id)| id)
}

/// The closed contour of sketch `si` under a screen point of the 3D view: each contour lifted into the world by the
/// sketch's frame - as the command's arrow is - and laid onto the screen; the smaller area wins, as on the sheet.
pub fn contour_under_3d(project: &Project, scr: &Screen, screen: Pos2, si: usize) -> Option<Id> {
    let frame = project.sketch_frame(si)?;
    let mut best: Option<(f64, Id)> = None;
    for cid in sketch_closed_contours(project, si) {
        let Some(ci) = project.contour_index(cid) else { continue };
        let pts: Vec<Pos2> = project.contours[ci]
            .points
            .iter()
            .map(|p| {
                let w = frame.lift(*p);
                scr.at([w.x, w.y, w.z]).0
            })
            .collect();
        if pts.len() >= 3 && point_in_poly(screen, &pts) {
            let area = poly_area(&pts);
            if best.is_none_or(|(ba, _)| area < ba) {
                best = Some((area, cid));
            }
        }
    }
    best.map(|(_, id)| id)
}

/// The geometry of the active sketch, for snapping: segments and circles (arcs count as circles).
/// Returns (lines as [(A, B)], circles as [(centre, radius)]).
/// THE EDGES OF A SKETCH READY FOR SNAPPING AND PICKING, told apart by their shape.
///
/// It was `(Vec<(Point2, Point2)>, Vec<(Point2, f64)>)` - which of the two is a segment and which a circle
/// could only be worked out from the body of the function.
pub struct ActiveEdges {
    /// The straight ones, as their two endpoints.
    pub lines: Vec<(Point2, Point2)>,
    /// The round ones: circles whole, and arcs with the part of their circle they run over.
    pub circles: Vec<Rim>,
}

/// THE RIM OF A CIRCLE OR AN ARC, as the cursor snaps to it.
pub struct Rim {
    pub centre: Point2,
    pub radius: f64,
    /// For an arc, the part of the circle it runs over; `None` for a whole circle.
    pub arc: Option<RimArc>,
}

/// THE PART OF ITS CIRCLE AN ARC RUNS OVER: from its start to its end, counter-clockwise or not.
pub struct RimArc {
    pub from: Point2,
    pub to: Point2,
    pub ccw: bool,
}

impl Rim {
    /// WHETHER A POINT OF THE CIRCLE LIES ON THE RIM ITSELF: anywhere for a circle, within its sweep for an arc. A snap
    /// on the rest of an arc's circle stood where nothing is drawn - beside a fillet R20, on the far side of its circle.
    pub fn holds(&self, p: Point2) -> bool {
        let Some(arc) = &self.arc else { return true };
        let ang = |q: Point2| (q.y - self.centre.y).atan2(q.x - self.centre.x);
        let (a0, a1, at) = (ang(arc.from), ang(arc.to), ang(p));
        let tau = std::f64::consts::TAU;
        let (start, sweep) = if arc.ccw { (a0, (a1 - a0).rem_euclid(tau)) } else { (a1, (a0 - a1).rem_euclid(tau)) };
        (at - start).rem_euclid(tau) <= sweep + 1e-9
    }
}

pub fn active_edges(dc: &DrawCtx, si: usize) -> ActiveEdges {
    use qymcad_core::model::EntityKind;
    let mut lines = Vec::new();
    let mut circs = Vec::new();
    if let Some(s) = dc.project.sketches.get(si) {
        let points_by_id: std::collections::HashMap<Id, &qymcad_core::model::SketchPoint> = s.points.iter().map(|p| (p.id, p)).collect(); // a table: these are looked up for every entity or constraint
        let pt = |id: Id| points_by_id.get(&id).copied().map(|p| Point2::new(p.x, p.y));
        for e in &s.entities {
            match e.kind {
                EntityKind::Line { a, b } => {
                    if let (Some(pa), Some(pb)) = (pt(a), pt(b)) {
                        lines.push((pa, pb));
                    }
                }
                EntityKind::Circle { center, r } => {
                    if let Some(c) = pt(center) {
                        circs.push(Rim { centre: c, radius: r, arc: None });
                    }
                }
                EntityKind::Arc { center, a, b, ccw } => {
                    if let (Some(c), Some(pa), Some(pb)) = (pt(center), pt(a), pt(b)) {
                        circs.push(Rim { centre: c, radius: ((pa.x - c.x).powi(2) + (pa.y - c.y).powi(2)).sqrt(), arc: Some(RimArc { from: pa, to: pb, ccw }) });
                    }
                }
                EntityKind::Ellipse { .. } => {} // drawn by its own outline (as a profile)
            }
        }
    }
    ActiveEdges { lines, circles: circs }
}

/// The entity nearest to a screen point (within 12 px): for a line the distance to the segment, for
/// a circle or an arc the distance to the rim.
pub fn entity_near(pick: &PickCtx, rect: Rect, pos: Pos2, si: usize) -> Option<Id> {
    let s = pick.project.sketches.get(si)?;
    let points_by_id: std::collections::HashMap<Id, &qymcad_core::model::SketchPoint> = s.points.iter().map(|p| (p.id, p)).collect(); // a table: these are looked up for every entity or constraint
    let scr = |id: Id| points_by_id.get(&id).copied().map(|q| (Sheet { view: *pick.view, rect }).at(Point2::new(q.x, q.y)));
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
                Some(c) => (pos.distance(c) - (r * pick.view.scale as f64) as f32).abs(),
                _ => continue,
            },
            EntityKind::Arc { center, a, .. } => match (scr(center), scr(a)) {
                (Some(c), Some(pa)) => (pos.distance(c) - c.distance(pa)).abs(),
                _ => continue,
            },
            _ => continue,
        };
        if d < 12.0 && best.is_none_or(|(bd, _)| d < bd) {
            best = Some((d, e.id));
        }
    }
    best.map(|(_, id)| id)
}

/// Store the picked contour `cid` into the sweep or loft slot.
pub fn set_contour_slot(loft: &mut LoftParams, sweep: &mut SweepParams, slot: ContourSlot, cid: Id) {
    match slot {
        ContourSlot::SweepProfile => sweep.prof_cid = cid,
        ContourSlot::SweepPath => sweep.path_cid = cid,
        ContourSlot::LoftSection(i) => {
            if let Some(c) = loft.cids.get_mut(i) {
                *c = cid;
            }
        }
    }
}

/// The slot's candidate contours (for hit-testing in the half-sketcher). A path allows open contours.
pub fn slot_candidates(loft: &LoftParams, project: &Project, sweep: SweepParams, slot: ContourSlot) -> Vec<Id> {
    match slot {
        ContourSlot::SweepProfile => project.sweep_profile_contours(sweep.prof_sid),
        ContourSlot::SweepPath => project.sweep_path_contours(sweep.path_sid),
        ContourSlot::LoftSection(i) => project.sweep_profile_contours(loft.sids.get(i).copied().unwrap_or(0)),
    }
}

/// A rubber-band selection: while a sketch is being edited the band selects its entities.
pub fn box_select(ed: Editing, sel_sk: &mut SketchSelection, sketch_ses: SketchSession, rect: Rect, a: Pos2, b: Pos2) {
    if let Sel::Sketch(si) = *ed.sel {
        if sketch_ses.editing.is_some() {
            box_select_sketch(ed, sel_sk, rect, a, b, si);
        }
    }
}

/// The label of a candidate axis line for the command bar — construction lines are marked as such.
pub fn axis_line_label(project: &qymcad_core::model::Project, si: usize, line: Id, n: usize) -> String {
    let constr = project.sketches.get(si).and_then(|s| s.entities.iter().find(|e| e.id == line)).map(|e| e.construction).unwrap_or(false);
    if constr {
        qymcad_i18n::tr1("sk-axis-line-n", "n", &n.to_string())
    } else {
        qymcad_i18n::tr1("sk-line-n", "n", &n.to_string())
    }
}

/// Finish adding dimension `ci`: solve and CLASSIFY it. Redundant-but-CONSISTENT becomes a driven
/// (reference) dimension. A CONFLICT with other constraints is NOT muffled - such a dimension is NOT
/// made driven (otherwise it would "measure" averaged geometry and hide the conflict; making it driven
/// is available by hand, with the button in the list of constraints). Returns (redundant, conflicting).
///
/// The rank analysis is called DIRECTLY here, past the diagnostics cache: this is a one-off action
/// right after an edit, and the answer is needed for the fresh state rather than for an imprint of the
/// previous frame.
pub fn finish_dim(project: &mut Project, regen: &mut Rebuilding, si: usize, ci: usize) -> (bool, bool) {
    project.solve_sketch(si);
    let redundant = project.dim_redundant(si, ci);
    let conflict = project.sketch_conflicts(si).contains(&ci);
    if redundant && !conflict {
        project.auto_driven(si, ci);
        project.solve_sketch(si);
    }
    invalidate(regen);
    (redundant, conflict)
}

/// The lineage of object `id` BY NAMES. `None` means the object has no Id of its own (a face)
/// and has no lineage.
pub fn lineage_of(project: &qymcad_core::model::Project, id: Option<Id>) -> Lineage {
    let Some(id) = id else { return Lineage::default() };
    let name_of = |nid: Id| -> String { project.timeline.iter().find(|n| n.id == nid).map(|n| qymcad_i18n::name(&n.name)).unwrap_or_else(|| format!("#{nid}")) };
    // "WHAT CREATED IT" for a timeline node is the node itself, and there is no point repeating
    // it under its own heading: what is selected is already written there. What is shown is the
    // input it stands on.
    let built_on: Vec<String> = match project.timeline.iter().find(|n| n.id == id) {
        Some(node) => node.kind.inputs().iter().map(|i| project.creator_of(*i).map(name_of).unwrap_or_else(|| format!("#{i}"))).collect(),
        None => project.creator_of(id).map(name_of).into_iter().collect(),
    };
    Lineage { built_on, dependents: project.dependents_of(id).into_iter().map(name_of).collect() }
}

/// Whether a point lies inside a polygon (by ray casting). The contour is closed implicitly.
pub fn point_in_poly(p: Pos2, poly: &[Pos2]) -> bool {
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

pub fn poly_area(poly: &[Pos2]) -> f64 {
    let n = poly.len();
    let mut s = 0.0;
    for i in 0..n {
        let a = poly[i];
        let b = poly[(i + 1) % n];
        s += (a.x as f64) * (b.y as f64) - (b.x as f64) * (a.y as f64);
    }
    (s * 0.5).abs()
}

/// THE OUTLINE OF A SKETCH ENTITY as a polyline in sketch coordinates: a line its two ends, a circle, an arc and an
/// ellipse 48 pieces of themselves - enough for a box to tell touched from missed at any zoom a box is drawn at.
pub fn entity_outline(s: &qymcad_core::model::Sketch, kind: &qymcad_core::model::EntityKind) -> Vec<Point2> {
    use qymcad_core::model::EntityKind;
    let pt = |id: Id| s.points.iter().find(|q| q.id == id).map(|q| Point2::new(q.x, q.y));
    let ring = |c: Point2, u: (f64, f64), v: (f64, f64), a0: f64, sweep: f64| -> Vec<Point2> {
        (0..=48)
            .map(|i| {
                let t = a0 + sweep * i as f64 / 48.0;
                Point2::new(c.x + u.0 * t.cos() + v.0 * t.sin(), c.y + u.1 * t.cos() + v.1 * t.sin())
            })
            .collect()
    };
    match *kind {
        EntityKind::Line { a, b } => [pt(a), pt(b)].into_iter().flatten().collect(),
        EntityKind::Circle { center, r } => pt(center).map(|c| ring(c, (r, 0.0), (0.0, r), 0.0, std::f64::consts::TAU)).unwrap_or_default(),
        EntityKind::Arc { center, a, b, ccw } => match (pt(center), pt(a), pt(b)) {
            (Some(c), Some(pa), Some(pb)) => {
                let r = ((pa.x - c.x).powi(2) + (pa.y - c.y).powi(2)).sqrt();
                let (a0, a1) = ((pa.y - c.y).atan2(pa.x - c.x), (pb.y - c.y).atan2(pb.x - c.x));
                let mut sweep = (a1 - a0).rem_euclid(std::f64::consts::TAU);
                if !ccw {
                    sweep -= std::f64::consts::TAU;
                }
                ring(c, (r, 0.0), (0.0, r), a0, sweep)
            }
            _ => Vec::new(),
        },
        EntityKind::Ellipse { c, ma, mi } => match (pt(c), pt(ma), pt(mi)) {
            (Some(c), Some(pa), Some(pi)) => ring(c, (pa.x - c.x, pa.y - c.y), (pi.x - c.x, pi.y - c.y), 0.0, std::f64::consts::TAU),
            _ => Vec::new(),
        },
    }
}

/// Does the segment p-q cross the box [x0, x1] x [y0, y1] (screen)? Tested against the four sides; an end inside is
/// the caller's own test.
fn segment_crosses_box(p: Pos2, q: Pos2, x0: f32, x1: f32, y0: f32, y1: f32) -> bool {
    let cross = |a: Pos2, b: Pos2, c: Pos2, d: Pos2| {
        let o = |p: Pos2, q: Pos2, r: Pos2| (q.x - p.x) * (r.y - p.y) - (q.y - p.y) * (r.x - p.x);
        let (d1, d2, d3, d4) = (o(c, d, a), o(c, d, b), o(a, b, c), o(a, b, d));
        (d1 > 0.0) != (d2 > 0.0) && (d3 > 0.0) != (d4 > 0.0)
    };
    let (a, b, c, d) = (egui::pos2(x0, y0), egui::pos2(x1, y0), egui::pos2(x1, y1), egui::pos2(x0, y1));
    cross(p, q, a, b) || cross(p, q, b, c) || cross(p, q, c, d) || cross(p, q, d, a)
}

/// A rubber-band selection of sketch entities: left to right means enclosure (wholly inside), right to
/// left means crossing (merely touched) — the usual CAD convention.
pub fn box_select_sketch(ed: Editing, sel_sk: &mut SketchSelection, rect: Rect, a: Pos2, b: Pos2, si: usize) {
    let sh = Sheet { view: *ed.view, rect };
    let crossing = b.x < a.x;
    let (x0, x1) = (a.x.min(b.x), a.x.max(b.x));
    let (y0, y1) = (a.y.min(b.y), a.y.max(b.y));
    let inside = |p: Pos2| p.x >= x0 && p.x <= x1 && p.y >= y0 && p.y <= y1;
    // gather the hits first (`s` holds a borrow of self, so they are pushed afterwards)
    let (mut add_ent, mut add_pt): (Vec<Id>, Vec<Id>) = (Vec::new(), Vec::new());
    {
        let Some(s) = ed.project.sketches.get(si) else { return };
        // THE SHAPE ITSELF IS TESTED, not its ends: a line that crosses the box with both ends outside is touched, and
        // a circle is inside only when all of it is, not its centre alone
        let pts_of = |e: &qymcad_core::model::SketchEntity| -> Vec<Pos2> { entity_outline(s, &e.kind).into_iter().map(|p| sh.at(p)).collect() };
        for e in &s.entities {
            let pts = pts_of(e);
            if pts.is_empty() {
                continue;
            }
            let hit = if crossing {
                pts.iter().any(|p| inside(*p)) || pts.windows(2).any(|w| segment_crosses_box(w[0], w[1], x0, x1, y0, y1))
            } else {
                pts.iter().all(|p| inside(*p))
            };
            if hit {
                add_ent.push(e.id);
            }
        }
        for p in &s.points {
            if inside(sh.at(Point2::new(p.x, p.y))) {
                add_pt.push(p.id);
            }
        }
    }
    for id in add_ent {
        if !sel_sk.items.contains(&(1, id)) {
            sel_sk.items.push((1, id));
        }
    }
    for id in add_pt {
        if !sel_sk.items.contains(&(0, id)) {
            sel_sk.items.push((0, id));
        }
    }
    *ed.status = qymcad_i18n::tr1("g-selected-n", "n", &sel_sk.items.len().to_string());
}

/// The closed contours of sketch `si` (by Id) that will do as the profile of a feature.
pub fn sketch_closed_contours(project: &qymcad_core::model::Project, si: usize) -> Vec<Id> {
    project.sketches.get(si).map(|s| s.contour_ids.iter().copied().filter(|cid| project.contour_profile_xy(*cid).is_some()).collect()).unwrap_or_default()
}

/// The contour under the cursor among the slot's CANDIDATES (for the half-sketcher of a sweep or a
/// loft). A closed one with the cursor inside wins (the smaller area is the nearer); otherwise the
/// NEAREST polyline within the threshold, which is how an OPEN path gets caught, since point-in-polygon
/// will not take it. `cands` holds the slot's candidate contours.
pub fn slot_contour_under_2d(pick: &PickCtx, rect: Rect, screen: Pos2, cands: &[Id]) -> Option<Id> {
    let seg_d = |p: Pos2, a: Pos2, b: Pos2| -> f32 {
        let ab = b - a;
        let l2 = ab.length_sq();
        let t = if l2 <= 1e-6 { 0.0 } else { ((p - a).dot(ab) / l2).clamp(0.0, 1.0) };
        (p - (a + ab * t)).length()
    };
    let mut inside: Option<(f64, Id)> = None; // closed, cursor inside: ranked by area
    let mut near: Option<(f32, Id)> = None; // ranked by distance to the polyline
    for &cid in cands {
        let Some(ci) = pick.project.contour_index(cid) else { continue };
        let c = &pick.project.contours[ci];
        if c.points.len() < 2 {
            continue;
        }
        let pts: Vec<Pos2> = c.points.iter().map(|p| (Sheet { view: *pick.view, rect }).at(*p)).collect();
        if c.closed && pts.len() >= 3 && point_in_poly(screen, &pts) {
            let area = poly_area(&pts);
            if inside.is_none_or(|(ba, _)| area < ba) {
                inside = Some((area, cid));
            }
        }
        let n = pts.len();
        let segs = if c.closed { n } else { n - 1 };
        let mut d = f32::INFINITY;
        for k in 0..segs {
            d = d.min(seg_d(screen, pts[k], pts[(k + 1) % n]));
        }
        if d < 8.0 && near.is_none_or(|(bd, _)| d < bd) {
            near = Some((d, cid));
        }
    }
    inside.map(|(_, id)| id).or(near.map(|(_, id)| id))
}

/// Apply an edit operation to the selection. Returns true when it was applied.
pub fn try_modify(mut ed: Editing, sel_sk: &mut SketchSelection, sk_pat: SketchPattern, tool_prefs: &SketchToolPrefs, op: EditTool) -> bool {
    // THE BOUNDARY OF AN OPERATION: one step of undo, named after the tool
    let name = match op {
        EditTool::Delete => "sk-delete",
        EditTool::Mirror => "tool-mirror",
        EditTool::LinearPattern => "tool-lin-array",
        EditTool::CircularPattern => "tool-circ-array",
        EditTool::Fillet => "tool-fillet",
        EditTool::Chamfer => "tool-chamfer",
        EditTool::Offset => "tool-offset",
    };
    begin_edit(ed.edits, &*ed.project, qymcad_i18n::tr(name));
    let ok = try_modify_in(ed.reborrow(), sel_sk, sk_pat, tool_prefs, op);
    close_edit(ed.edits, ed.project);
    ok
}

fn try_modify_in(ed: Editing, sel_sk: &mut SketchSelection, sk_pat: SketchPattern, tool_prefs: &SketchToolPrefs, op: EditTool) -> bool {
    let Sel::Sketch(si) = *ed.sel else { return false };
    let eids: Vec<Id> = sel_sk.items.iter().filter(|(k, _)| *k == 1).map(|(_, id)| *id).collect();
    if eids.is_empty() {
        return false;
    }
    let ok = match op {
        EditTool::Delete => {
            ed.project.delete_entities(si, &eids);
            sel_sk.clear(); // the selection and whatever was waiting for it
            true
        }
        // THE MIRROR IS NOT APPLIED FROM A CLICK. It has two steps of its own (`modify_button` takes the
        // selection, `mirror_about_axis` or `mirror_about_line` finishes it), and applying here would mean
        // mirroring on the FIRST entity clicked - which is how several things could never be mirrored
        // together, and how a misclick became an edit.
        EditTool::Mirror => return false,
        EditTool::LinearPattern => {
            ed.project.array_linear(si, &eids, sk_pat.dx, sk_pat.dy, sk_pat.count);
            true
        }
        EditTool::CircularPattern => {
            let (cx, cy) = if let Some(p) = sel_point_ids(sel_sk).first().and_then(|id| sketch_pt(ed.project, si, *id)) { (p.x, p.y) } else { ed.project.entities_centroid(si, &eids) };
            ed.project.array_circular(si, &eids, cx, cy, sk_pat.count, sk_pat.angle);
            true
        }
        EditTool::Fillet => {
            eids.len() >= 2 && ed.project.fillet_lines_by(si, eids[0], eids[1], fillet_size(tool_prefs)) && {
                sel_sk.clear(); // the selection and whatever was waiting for it
                true
            }
        }
        EditTool::Chamfer => {
            eids.len() >= 2 && ed.project.chamfer_lines(si, eids[0], eids[1], chamfer_legs(tool_prefs)) && {
                sel_sk.clear(); // the selection and whatever was waiting for it
                true
            }
        }
        // the distance field refusing its value refuses the offset too: the last good distance is not what was typed
        EditTool::Offset if bar_field_bad("sk_offset") => {
            *ed.status = qymcad_i18n::tr("sk-offset-field-bad");
            false
        }
        EditTool::Offset => ed.project.offset_entities(si, &eids, tool_prefs.offset) > 0,
    };
    if ok {
        invalidate(ed.regen);
    }
    ok
}

/// What the corner popup touches: its own input, the document, the sketch selection it clears, the rebuild
/// flag, the status line and the sticky radius offered next time. Six fields, and no more - which is the
/// point of writing them down.
pub struct CornerCtx<'a> {
    pub corner: &'a mut CornerInput,
    pub project: &'a mut qymcad_core::model::Project,
    /// The undo journal: a fillet or a chamfer of a corner is one step of it, named after its tool.
    pub edits: &'a mut Edits,
    pub sel_sk: &'a mut SketchSelection,
    /// The sheet the corner stands on: where the field was opened says which side of the point the corner is.
    pub view: &'a View2d,
    pub regen: &'a mut Rebuilding,
    pub status: &'a mut String,
    pub tool_prefs: &'a mut SketchToolPrefs,
    /// THE SCHEME IN HAND, and where every colour of the preview is read from. See `PopupLooks` for why a colour
    /// is not kept in the state of the tool.
    pub scheme: &'a SchemeUi,
    /// THE SETTINGS OF THE SHEET, and where the width of the zone of a point is read from: the zone is three times
    /// the reach of a pick of a point, and that reach is a setting of the person.
    pub set: &'a Settings,
    /// THE SKETCH OPEN FOR EDITING, or what is selected where there is none: the preview on the sheet is drawn
    /// whether or not the field is up, and before the first click there is no field and no corner to say which
    /// sketch - so the sheet the preview belongs to has to be reachable without the field.
    pub sel: &'a Sel,
    /// THE TOOL IN HAND, and so whether the corner under the cursor would be rounded or cut straight.
    pub armed: &'a Armed,
}

impl<'a> CornerCtx<'a> {
    /// WHAT THE CURSOR CATCHES, read the way the canvas reads it - the zone of a point that can name a corner is
    /// three times the reach of a pick of one, which is wider than any pick the canvas itself does.
    pub fn pick(&self) -> PickCtx<'_> {
        PickCtx { project: self.project, view: self.view, set: self.set }
    }
}

/// WHAT THE SIZE POPUPS OF THE SKETCH TOOLS TAKE FROM THE APPLICATION besides the drawing itself: the values the
/// tools remember between two corners, and the colours of the scheme in hand.
///
/// **A COLOUR IS NOT STATE OF A TOOL, AND THAT WAS THE FAULT.** The two colours of a corner preview were kept in
/// `CornerInput` and written by the one window that had a scheme to hand — so every other way of opening a corner
/// field left them at the transparent default of a `Color32` and drew nothing at all, and `CornerInput::clear()`
/// (which every applied corner and every mode change calls) took them away again. Reported: with the corner tool,
/// the lines of the preview were not drawn at all. The scheme is read where the drawing is done instead, on every
/// frame, so that every way of opening a field draws alike and a person who changes the scheme sees the change.
pub struct PopupLooks<'a> {
    /// THE VALUES THE TOOLS REMEMBER: the radius or the leg that was answered last, and the fillets of the bar.
    pub tool_prefs: &'a mut SketchToolPrefs,
    /// THE SCHEME IN HAND — the source of every colour drawn over the sheet.
    pub scheme: &'a SchemeUi,
    /// THE SETTINGS OF THE SHEET, and where the width of the zone of a point that names a corner is read from.
    pub set: &'a Settings,
}

/// THE STATES OF THE SKETCH THE POPUPS AT THE GEOMETRY WRITE: the corner being cut, the shape being placed, what
/// is picked in the drawing, and the tool in hand.
///
/// Four separate arguments that always travel together - the bar that carries a corner set and the box that
/// carries a shape are drawn in one go, from the same frame of the same panel.
pub struct PopupTools<'a> {
    /// The corner set and the value being typed for it.
    pub corner: &'a mut CornerInput,
    /// The shape being placed, and the size being typed for it.
    pub place: &'a mut Placing,
    /// What is picked in the drawing.
    pub sel_sk: &'a mut SketchSelection,
    /// The tool in hand: which of the corner tools the preview on the sheet belongs to.
    pub armed: &'a Armed,
}

/// What the three shape popups touch. All three edit the same thing - the shape being placed - through
/// named document operations, so they share one context rather than three alike.
pub struct PlaceCtx<'a> {
    pub place: &'a mut Placing,
    pub project: &'a mut qymcad_core::model::Project,
    pub view: &'a View2d,
    pub regen: &'a mut Rebuilding,
}

/// THE SAME FIELD, BUT IT TAKES FOCUS ITSELF AND SELECTS THE FORMER VALUE.
///
/// That is how the dimension popup opens: a dimension is clicked and a new number is typed straight
/// away, with no aiming of the mouse at the field and no erasing of the old one. This behaviour must
/// not be lost when moving onto the shared field.
pub fn expr_field_autofocus(ui: &mut egui::Ui, project: &Project, id: egui::Id, model: &str, w: f32, hint: &str, want_focus: bool) -> ExprOut {
    field(ui, project, id, model, w, hint, FieldRules { valid: &|_| true, list: NameList::Offered, focus: if want_focus { Focus::Takes } else { Focus::Waits } })
}

/// The editing button: with a ready selection it applies at once, otherwise it waits for one (Esc cancels).
pub fn modify_button(mut ed: Editing, t: &mut Tools, sk_pat: SketchPattern, tool_prefs: &SketchToolPrefs, op: EditTool) {
    // PRESSED AGAIN WITH THE TOOL IN HAND it is put down, as every tool button does - except the mirror with
    // geometry selected, where the press is the way on to the axis (see below)
    let held = t.sel_sk.modify == Some(op);
    let mirror_forward = op == EditTool::Mirror && t.sel_sk.items.iter().any(|(k, _)| *k == 1);
    exit_draw_tools(&mut t.reborrow());
    if held && !mirror_forward {
        return;
    }
    let Tools { armed, annot: _, cmd: _, corner: _, dim: _, drag: _, gsel: _, inline: _, measure: _, pat: _, pending_import: _, picking: _, place: _, sel_sk, tool: _ } = t;
    sel_sk.constraint = None;
    **armed = Armed::Modify(op);
    // THE MIRROR HAS TWO STEPS OF ITS OWN: what, then about what. Pressing the button with geometry
    // selected answers the first and asks the second; pressing it with a selection made while the tool was
    // already in hand does the same, which is why the button is the way forward rather than a stray click.
    if op == EditTool::Mirror {
        let eids: Vec<Id> = sel_sk.items.iter().filter(|(k, _)| *k == 1).map(|(_, id)| *id).collect();
        if !eids.is_empty() {
            // THE SELECTION STAYS ON SCREEN. It used to be moved into the tool and cleared, so the person
            // saw nothing selected, could not tell what was about to be reflected, and read the next click
            // as "it mirrored everything". Reported behaviour: "the geometry simply loses its selection -
            // it has to stay highlighted so one can see it is still chosen."
            sel_sk.mirror_of = eids;
            *ed.status = qymcad_i18n::tr("sk-mirror-pick-axis");
        } else {
            sel_sk.clear();
            sel_sk.modify = Some(op);
            *ed.status = qymcad_i18n::tr("sk-mirror-pick-what");
        }
        return;
    }
    // THE OFFSET IS NOT MADE BY THE PRESS: what is chosen stays chosen, the copy is previewed on the side of the pointer,
    // and Enter makes it there (`qymcad_sketch::sketch_tool_keys`). Made at once, it had no side but the sign of the
    // distance, and a click on a line - one side of a polyline - offset that line alone before the rest was chosen.
    if op == EditTool::Offset {
        sel_sk.modify = Some(op);
        let curves = sel_sk.items.iter().any(|(k, _)| *k == 1);
        // points chosen and no curve: said what the tool takes, not "pick the entities" as though nothing were chosen
        *ed.status = qymcad_i18n::tr(match (curves, sel_sk.items.is_empty()) {
            (true, _) => "sk-offset-side",
            (false, false) => "sk-offset-takes",
            (false, true) => "g-pick-for-op",
        });
        return;
    }
    let chosen = !sel_sk.items.is_empty();
    if try_modify(ed.reborrow(), sel_sk, sk_pat, tool_prefs, op) {
        sel_sk.modify = None;
    } else {
        sel_sk.clear(); // the selection, and whatever was waiting on it: pick anew, with nothing stuck from before
        sel_sk.modify = Some(op);
        // SOMETHING WAS CHOSEN AND NOTHING OF IT COULD BE OFFSET (points, construction): said so, and what the tool
        // takes. "Pick the entities" read as though nothing had been chosen, and the tool looked broken.
        *ed.status = qymcad_i18n::tr(if chosen && op == EditTool::Offset { "sk-offset-takes" } else { "g-pick-for-op" });
    }
}

/// MIRROR WHAT THE TOOL HOLDS ABOUT ONE OF THE SKETCH AXES (0 = X, 1 = Y).
///
/// The axes are drawn in every sketch and were, until now, the only drawn thing a person could not point
/// at: mirroring about Y happened silently when no line was selected, and X could not be asked for at all.
pub fn mirror_about_axis(ed: Editing, sel_sk: &mut SketchSelection, which: usize) {
    let (bx, by) = if which == 0 { (1.0, 0.0) } else { (0.0, 1.0) };
    mirror_about(ed, sel_sk, 0.0, 0.0, bx, by);
}

/// MIRROR WHAT THE TOOL HOLDS ABOUT A LINE OF THE SKETCH, given by its two points.
///
/// Construction geometry serves as well as ordinary geometry: what matters is that two points define a
/// line, and a construction line is exactly what people draw to mirror about.
pub fn mirror_about_line(ed: Editing, sel_sk: &mut SketchSelection, a: Id, b: Id) {
    let Sel::Sketch(si) = *ed.sel else { return };
    let (Some(pa), Some(pb)) = (sketch_pt(ed.project, si, a), sketch_pt(ed.project, si, b)) else { return };
    mirror_about(ed, sel_sk, pa.x, pa.y, pb.x, pb.y);
}

/// The two doors above meet here: reflect what is held about the line through (ax, ay) and (bx, by) - one step of undo,
/// named after the tool.
fn mirror_about(mut ed: Editing, sel_sk: &mut SketchSelection, ax: f64, ay: f64, bx: f64, by: f64) {
    begin_edit(ed.edits, &*ed.project, qymcad_i18n::tr("tool-mirror"));
    mirror_about_in(ed.reborrow(), sel_sk, ax, ay, bx, by);
    close_edit(ed.edits, ed.project);
}

fn mirror_about_in(ed: Editing, sel_sk: &mut SketchSelection, ax: f64, ay: f64, bx: f64, by: f64) {
    let Sel::Sketch(si) = *ed.sel else { return };
    // THE AXIS IS NOT MIRRORED WITH THE REST. It used to be: the line serving as the axis sat in the same
    // selection as everything else, so the tool reflected it onto itself along with the geometry.
    let eids: Vec<Id> = std::mem::take(&mut sel_sk.mirror_of);
    if eids.is_empty() {
        return;
    }
    ed.project.mirror_entities(si, &eids, ax, ay, bx, by);
    sel_sk.clear(); // the tool is done and lets go: nothing is left in hand to catch the next click
    *ed.status = qymcad_i18n::tr("sk-mirror-done");
}

/// ESC FROM AN EDITING BUTTON THAT WORKS ON A CLICK, whatever it is: the tool goes down, and what it was working on
/// goes with it.
///
/// **THE CORNER TOOLS TAKE THEIR WHOLE SET WITH THEM.** The fillet and the chamfer work on a set of lines chosen with
/// Shift, and Esc puts that set down with the tool: what was lit goes dark, the corners drawn on it come off, and the
/// field stands no longer. It used to take the tool down alone - the lines stayed lit with their corners drawn on
/// them, a set that was on the screen and in no tool, and the next click landed in it.
///
/// The other editing buttons act on one click and keep nothing of their own, so for them Esc takes the button alone.
///
/// `None` means no editing button is in hand and the ladder should carry on to its next rung.
pub fn leave_editing_tool(t: &mut Tools, status: &mut String) -> Option<()> {
    let corner = match t.armed.click_op() {
        0 => return None,
        4 | 5 => true,
        _ => false,
    };
    // EXTEND HOLDING A LINE lets it go first and stays in hand, as a drawing tool drops its points before itself
    if t.armed.click_op() == 2 && t.tool.extend.take().is_some() {
        *status = qymcad_i18n::tr("opt-extend-hint");
        return Some(());
    }
    *t.armed = Armed::None;
    if corner {
        t.corner.put_down();
        t.sel_sk.clear();
        *status = qymcad_i18n::tr("in-selection-cleared");
    }
    Some(())
}

impl Tools<'_> {
    /// ESC FROM AN EDITING BUTTON THAT WORKS ON A CLICK, said as a question: was one in hand, and has it been put
    /// down with what it was working on. `leave_editing_tool` says what each of them takes with it; this is the same
    /// answer in the shape the ladder of cancellations reads.
    pub fn leave_click_tool(&mut self, status: &mut String) -> bool {
        leave_editing_tool(self, status).is_some()
    }
}

/// PUT DOWN THE SKETCH TOOL THAT IS IN HAND, and say what to tell the person.
///
/// Reported behaviour: "Esc does not reset the Mirror tool to the default Select - the selection is lost
/// and the tool stays active with its bar at the top." Switched off yet looking switched on is the worst
/// kind of cancellation.
///
/// THE LADDER CARRIED A RUNG PER FAMILY and had none for the editing tools, so mirror, offset, fillet,
/// chamfer and the arrays were all unreleasable - the mirror was simply the one somebody pressed. A rung
/// per variant written out in the ladder is a list of THIS enum living somewhere else, and it falls behind
/// silently: a family nobody added is a tool Escape does not put down (D19).
///
/// `None` means nothing was in hand, and the ladder should carry on to its next rung.
pub fn release_armed_sketch_tool(t: &mut Tools) -> Option<&'static str> {
    let Tools { armed, dim, measure, sel_sk, tool, .. } = t.reborrow();
    let msg = match *armed {
        Armed::Dimension(_) => {
            dim.pick.clear();
            None
        }
        Armed::Draw(_) => None,
        Armed::Measure => {
            measure.clear();
            Some("in-measure-cancelled")
        }
        Armed::Modify(_) => {
            // the editing family: what it was waiting for goes with it, or the next click feeds a tool
            // that is no longer in hand
            sel_sk.modify = None;
            sel_sk.mirror_of.clear();
            Some("in-modify-cancelled")
        }
        _ => return None,
    };
    tool.pts.clear();
    *armed = Armed::None;
    Some(msg.unwrap_or("in-tool-released"))
}

/// OPEN THE LITTLE BOX OF THE CORNER: at a vertex, with the tool's value already in it.
///
/// The value at the corner is the one the bar carries, and it is carried on afterwards: the next corner offers the
/// same number. One place writes the box down, so the corner found by clicking the point and the corner found by
pub fn corner_reach(set: &Settings) -> f32 {
    grab::grab(set, grab::Grab::Corner)
}

/// THE CORNER AS A POINT IN THE DRAWING: where the field was opened, read on the sheet. It is the side of the point
/// the person pressed on, which is what says which corner of several is meant where more than two lines meet there.
pub fn corner_where(cc: &CornerCtx, rect: Rect) -> Option<(f64, f64)> {
    let at = cc.corner.pos?;
    let w = to_world(cc.view, rect, at);
    Some((w.x, w.y))
}
/// THE TWO EDGES THE CORNER IN THE FIELD IS, right now.
///
/// **A PAIR THE TWO LINES NAMED IS THE CORNER, and the cursor does not move it.** Two edges that share a point are
/// one corner of the drawing and the person named it by picking both of them: at a point where four lines meet,
/// letting the cursor choose a different pair of them put the arc where nobody had asked for it - it followed the
/// pointer to the far corner of the point and was cut there on Enter. That is what `pinned` says: the pair was picked,
/// not read off the cursor.
///
/// Where the pair was READ OFF THE CURSOR it is the cursor's to move while it is over the point - and the last one it
/// named stands while it is away, which is the whole of a field: a person typing the radius has the pointer in the
/// field, and the preview under the value must not jump back to another corner when the pointer leaves the point.
///
/// `through` narrows the question where the corner was named by an edge and a point together: then only the corners
/// that edge takes part in are among the answers, and the cursor says which of them.
pub fn corner_pair_now(project: &Project, si: usize, pid: Id, named: Option<(Id, Id)>, through: Option<Id>, pinned: bool, cursor: Option<(f64, f64)>) -> Option<(Id, Id)> {
    // THE PAIR IN FORCE STANDS WHILE THE CURSOR IS AWAY: it is the corner the field was opened for, and the value in
    // the field belongs to it. `through` has to be in the pair, or the corner would change its own subject.
    let stands = |pair: Option<(Id, Id)>| pair.filter(|p| project.corner_of_pair(si, p.0, p.1) == Some(pid) && through.is_none_or(|line| p.0 == line || p.1 == line));
    if let Some(line) = through {
        // THE CORNER IS ONE OF THIS LINE'S, so the pair is read off the side the cursor stands on
        return cursor.and_then(|c| project.vertex_pair_through(si, pid, line, Some(c))).or_else(|| stands(named)).or_else(|| project.vertex_pair_through(si, pid, line, None));
    }
    if pinned {
        return stands(named); // the two lines that were picked: the cursor has no word in it
    }
    if project.vertex_edges(si, pid).len() > 2 {
        if let Some(pair) = cursor.and_then(|c| project.vertex_pair(si, pid, Some(c))) {
            return Some(pair);
        }
    }
    stands(named).or_else(|| project.vertex_pair(si, pid, None))
}

/// THE CURSOR AS A SAY IN THE CORNER, and only while it stands near the point.
///
/// The cursor answers "which of the corners at this point" by the side of the point it is on, which is only worth
/// asking while it is ON the point: ten units away it is over some other part of the drawing, and taking its word for
/// it made the corner change as the pointer crossed the sheet, so the radius being typed was cut at a corner nobody
/// was looking at. `track_px` is the reach of the aim for the point itself, `Grab::Corner` - a shade wider than the
/// point is caught from, because a person who aimed there has the corner open and cannot move it.
///
/// `None` - no cursor over the sheet, or one out of reach - says nothing, and the pair stands as it was named.
pub fn corner_cursor(project: &Project, si: usize, pid: Id, cursor: Option<(f64, f64)>, track_px: f32, scale: f32) -> Option<(f64, f64)> {
    let c = cursor?;
    let s = project.sketches.get(si)?;
    let (px, py) = s.points.iter().find(|q| q.id == pid).map(|q| (q.x, q.y))?;
    (((c.0 - px).hypot(c.1 - py) * scale as f64) <= track_px as f64).then_some(c)
}

/// A CORNER THE SELECTED LINES HAVE MADE, and the point it stands at.
///
/// The two lines ARE the corner; the point is where it is. It is remembered rather than read off the selection
/// afresh every time: a line that leaves the selection takes its corner with it, and the line that stays does not
/// pair again with a neighbour of the one that left.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineCorner {
    pub point: Id,
    pub pair: (Id, Id),
}

/// A CORNER NAMED AT A POINT, where no chosen line arrives. One at a point at most, and it dies the moment a line
/// corner stands there in its place.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointCorner {
    pub point: Id,
    pub pair: (Id, Id),
}

/// WHAT A CLICK AT A POINT DID, so that the words to say are chosen at the place the click was read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointAct {
    /// NOTHING STANDS THERE: the click named no corner, and the selection of lines does not move either.
    Nothing,
    /// A CORNER WAS NAMED AT THE POINT, and is in the set.
    PointAdded,
    /// THE CORNER NAMED AT THE POINT WAS TAKEN AWAY, and the point is free for it again.
    PointRemoved,
    /// A CORNER OF THE LINES WAS PUT AWAY, not deleted: the same click brings it back.
    LineHidden,
    /// THE CORNER OF THE LINES THAT WAS PUT AWAY STANDS AGAIN.
    LineShown,
    /// THE POINT IS NOT FREE FOR A CORNER: a corner of the lines is standing there.
    TakenByLines,
}

/// WHAT THE SELECTED LINES HAVE MADE OF THE DRAWING: the corners that stand, the ones put away by a click at
/// their point, and the ones named at a point.
///
/// ONE RECORD, because the three are one thing: a change of the selection moves all of them at once, and a reader
/// holding two of the three is reading half a state.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CornerSet {
    /// THE CORNERS THAT STAND, each with its two lines and the point between them.
    pub made: Vec<LineCorner>,
    /// THE CORNERS PUT AWAY BY A CLICK AT THEIR POINT, remembered as the pair of lines and NOT deleted: the same
    /// click at the same point brings that very corner back. A corner that is put away is not shown and not cut,
    /// and it stays that way as long as both its lines stand in the selection.
    pub hidden: Vec<(Id, Id)>,
    /// THE CORNERS NAMED AT A POINT, one at a point at most.
    pub points: Vec<PointCorner>,
}

impl CornerSet {
    /// NOTHING OF THE SET, which is what an applied answer and a refused one both leave behind.
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// THE ONE CORNER NAMED AT A POINT FOLLOWS THE POINTER, where the point has more than one corner to give - a
    /// crossing cut into four, a T, a point several lines meet at: the corner is the one the sector of the pointer
    /// is, so the preview is drawn and the cut made on the side the pointer is on. A set of more corners than one is
    /// left as it was named: one pointer does not say a side for every point of a set. Answers whether the pointer
    /// steers the corner.
    pub fn steer(&mut self, project: &Project, si: usize, cursor: (f64, f64)) -> Steered {
        if self.choice(project, si) == Choice::One {
            return Steered::No;
        }
        let [one] = self.points.as_mut_slice() else { return Steered::No };
        if let Some(pair) = project.vertex_pair(si, one.point, Some(cursor)) {
            one.pair = pair;
        }
        Steered::Yes
    }

    /// WHETHER THE SET IS ONE CORNER NAMED AT A POINT THAT HAS MORE THAN ONE TO GIVE: a crossing cut into four, a T, a
    /// point the corners of several shapes meet at. Such a corner is chosen by a click on its side, not by the click that
    /// named the point.
    pub fn choice(&self, project: &Project, si: usize) -> Choice {
        let [one] = self.points.as_slice() else { return Choice::One };
        if !self.made.is_empty() {
            return Choice::One;
        }
        let corners = project.vertex_pairs(si, one.point).into_iter().filter(|&(a, b)| project.corner_of_pair(si, a, b).is_some()).count();
        if corners < 2 {
            Choice::One
        } else {
            Choice::Many
        }
    }

    /// THE CORNERS ONE ANSWER CUTS, each with the point it stands at: what stands, in the order it was made.
    pub fn standing(&self) -> Vec<(Id, (Id, Id))> {
        let mut out: Vec<(Id, (Id, Id))> = self.made.iter().map(|c| (c.point, c.pair)).collect();
        out.extend(self.points.iter().map(|c| (c.point, c.pair)));
        out
    }

    /// THE LINES THE SET IS CUT FROM, which is what the lines between the corners have to spare.
    pub fn lines(&self) -> std::collections::HashSet<Id> {
        self.standing().into_iter().flat_map(|(_, (a, b))| [a, b]).collect()
    }

    /// THE LINES THAT STAND IN A CORNER OF THE SET, put away or not: they are spoken for, and a line already in a
    /// corner makes no new one by arriving beside another at the same point.
    pub fn spoken_at_points(&self) -> Vec<Id> {
        self.made.iter().flat_map(|c| [c.pair.0, c.pair.1]).chain(self.points.iter().flat_map(|c| [c.pair.0, c.pair.1])).collect()
    }

    /// WHETHER ANY CORNER STANDS AT A POINT, put away or not. A point that has a corner shows no yellow preview
    /// and takes no second one: it is one place, and a second corner there is a second reading of it.
    pub fn occupied(&self, project: &Project, si: usize, pid: Id) -> bool {
        self.made.iter().any(|c| c.point == pid) || self.points.iter().any(|c| c.point == pid) || self.hidden.iter().any(|&(a, b)| project.corner_of_pair(si, a, b) == Some(pid))
    }

    /// THE CORNERS OF THE SELECTION, brought up to date.
    ///
    /// **A CORNER IS REMEMBERED, NOT READ OFF THE SELECTION EVERY TIME.** Two lines that met are a corner, and
    /// they stay one until one of them leaves: the line that stays does not pair again with a neighbour of the one
    /// that left. Reported, where it was read afresh: four lines through one point were chosen, then one of them
    /// was let go, and the two lines beside it made a corner nobody had asked for.
    ///
    /// **THE LINES WAITING AT A POINT ARE PAIRED TWO AT A TIME, IN THE ORDER THEY WERE CHOSEN**, and a line that is
    /// already in a corner there is spoken for - it makes one corner at that point and no other. So of four lines
    /// through one point the first two make the first corner, the third waits and the fourth makes the second with
    /// it; letting the first go leaves the second corner alone and the first line waiting, for a fourth line that
    /// has not been chosen.
    ///
    /// A pair that is put away stays put while both its lines are chosen: it is not made again while it is
    /// remembered, and it is not shown. It goes with a line of it that is let go, as a corner named at a point does,
    /// and choosing the line again makes the corner afresh - this time standing.
    /// A corner named at a point dies as soon as one of its lines leaves the selection or a corner of the lines
    /// stands there in its place.
    pub fn follow(&mut self, project: &Project, si: usize, selected: &[Id]) {
        self.follow_from(project, si, selected, selected);
    }

    /// THE SAME, WITH THE SELECTION IT WAS LAST READ FROM, which is how a corner named at a point knows that a
    /// line of it was let go: a line that has left the selection is one a person has said they do not want.
    pub fn follow_from(&mut self, project: &Project, si: usize, before: &[Id], selected: &[Id]) {
        let lost: Vec<Id> = before.iter().copied().filter(|l| !selected.contains(l)).collect();
        self.made.retain(|c| selected.contains(&c.pair.0) && selected.contains(&c.pair.1) && project.corner_of_pair(si, c.pair.0, c.pair.1) == Some(c.point));
        // A CORNER PUT AWAY LIVES WHILE BOTH ITS LINES ARE CHOSEN, and it goes with them. It is remembered as a pair
        // of lines and not as a mark on the drawing, so a line that is let go takes the corner at its end with it -
        // put away or not: what is put away is a corner of chosen lines, and a line a person has let go of is one
        // they do not want. Choosing the line again makes the corner again, and it stands.
        self.hidden.retain(|&(a, b)| selected.contains(&a) && selected.contains(&b) && project.corner_of_pair(si, a, b).is_some());
        // EVERY POINT WHERE CHOSEN LINES STAND, and the lines standing there in the order they were chosen.
        let mut at: Vec<(Id, Vec<Id>)> = Vec::new();
        // a construction line chosen before the tool was taken makes no corner either
        for &line in selected.iter().filter(|&&l| !line_is_construction(project, si, l)) {
            let Some((x, y)) = project.edge_ends(si, line) else { continue };
            for p in [x, y] {
                match at.iter_mut().find(|(q, _)| *q == p) {
                    Some((_, ls)) => ls.push(line),
                    None => at.push((p, vec![line])),
                }
            }
        }
        for (p, lines) in at {
            // THE LINES ALREADY IN A CORNER AT THIS POINT, standing or put away: a corner that is put away is
            // still a corner, and its lines are still spoken for.
            let spoken: Vec<Id> = self
                .made
                .iter()
                .filter(|c| c.point == p)
                .flat_map(|c| [c.pair.0, c.pair.1])
                .chain(self.hidden.iter().copied().filter(|(a, b)| project.corner_of_pair(si, *a, *b) == Some(p)).flat_map(|(a, b)| [a, b]))
                .collect();
            let waiting: Vec<Id> = lines.into_iter().filter(|l| !spoken.contains(l)).collect();
            for two in waiting.chunks(2) {
                let [a, b] = two else { continue };
                let pair = (*a, *b);
                // TWO LINES ALONG ONE STRAIGHT LINE THROUGH A POINT MAKE NO ANGLE: there is nothing to cut
                // there, and naming it as a corner would offer a cut where there is none.
                if project.corner_of_pair(si, *a, *b) != Some(p) || self.hidden.contains(&pair) || self.made.iter().any(|c| c.pair == pair) {
                    continue;
                }
                self.made.push(LineCorner { point: p, pair });
            }
        }
        // A CORNER NAMED AT A POINT LIVES WHILE BOTH ITS LINES ARE CHOSEN, and only while no corner of the lines
        // stands in its place: two corners at one point is one place read twice.
        let taken: Vec<Id> = self.made.iter().map(|c| c.point).collect();
        self.points.retain(|c| project.corner_of_pair(si, c.pair.0, c.pair.1) == Some(c.point) && !taken.contains(&c.point) && !lost.iter().any(|l| project.edge_stands_at(si, *l, c.point)));
    }

    /// THE MOST ONE ANSWER TAKES OFF EVERY CORNER OF THE SET AT ONCE, and what the lines between them can spare.
    ///
    /// One value cuts them all, so it is held by the tightest of them; and a line between two corners spends the
    /// value on itself at both ends, which is why rounding every corner of a rectangle takes half its short side.
    /// `None` when the set holds no corner, and a value then is held by nothing.
    ///
    /// A FILLET GIVEN BY ITS CHORD OR ITS ARC LENGTH is held in that way: the radius bound is the same for every corner,
    /// and each corner turns it into the chord or the arc of its own sweep - the set takes the least of them. Held by
    /// the radius alone, a chord of 18 passed on a corner of 135 deg where it is a radius of 23.5 against 20.5.
    pub fn limit(&self, project: &Project, si: usize, tool: qymcad_core::model::CornerTool, by: qymcad_core::model::FilletBy) -> Option<f64> {
        let least = |m: Option<f64>, v: f64| Some(m.map_or(v, |m: f64| m.min(v)));
        let standing = self.standing();
        let of_corners = standing.iter().filter_map(|&(pid, pair)| project.corner_limit_of_pair(si, pid, pair, tool)).fold(None, least);
        let lines = self.lines();
        let radius = match (of_corners, project.all_corners_limit(si, Some(&lines), tool)) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }?;
        if tool != qymcad_core::model::CornerTool::Fillet || by == qymcad_core::model::FilletBy::Radius {
            return Some(radius);
        }
        standing.iter().filter_map(|&(pid, pair)| project.corner_sweep_of_pair(si, pid, pair)).map(|sweep| qymcad_core::model::FilletSize::of_radius(by, radius, sweep)).fold(None, least)
    }

    /// WHAT A CLICK AT A POINT DOES TO THE SET, and nothing else: the selection of lines does not move, and a
    /// point is not part of it.
    ///
    /// * A corner named at that point is **taken away**, with Shift or without, and the point is free again.
    /// * A corner of the lines standing there is **put away**, and the same click brings it back.
    /// * A point where nothing stands takes the corner the click read - the two lines at it, or the two the
    ///   cursor stands between where more than two are there.
    pub fn click_at_point(&mut self, project: &Project, si: usize, pid: Id, read: Option<(Id, Id)>) -> PointAct {
        if let Some(k) = self.points.iter().position(|c| c.point == pid) {
            self.points.remove(k);
            return PointAct::PointRemoved;
        }
        let made_here: Vec<(Id, Id)> = self.made.iter().filter(|c| c.point == pid).map(|c| c.pair).collect();
        let hidden_here: Vec<(Id, Id)> = self.hidden.iter().copied().filter(|(a, b)| project.corner_of_pair(si, *a, *b) == Some(pid)).collect();
        if !made_here.is_empty() {
            // A CORNER OF THE LINES IS PUT AWAY, NOT DELETED: the lines keep the ends they have, and the same
            // click at the same point brings that very corner back.
            self.hidden.extend(made_here.iter().copied());
            self.made.retain(|c| c.point != pid);
            return PointAct::LineHidden;
        }
        if !hidden_here.is_empty() {
            // EVERY CORNER THAT WAS PUT AWAY AT THIS POINT STANDS AGAIN, all of them at once: one click hides them
            // all, and the same click brings them all back.
            self.hidden.retain(|&(a, b)| !hidden_here.contains(&(a, b)));
            self.made.extend(hidden_here.into_iter().map(|pair| LineCorner { point: pid, pair }));
            return PointAct::LineShown;
        }
        match read {
            Some(pair) if project.corner_of_pair(si, pair.0, pair.1) == Some(pid) => {
                self.points.push(PointCorner { point: pid, pair });
                PointAct::PointAdded
            }
            _ => PointAct::Nothing,
        }
    }

    /// THE CORNER A CLICK AT A POINT WOULD NAME, which is what the yellow preview shows: the two lines at the
    /// point, or - where more than two stand there - the two the cursor is between. `None` where the point is not
    /// free for a corner, or where there is nothing to read.
    ///
    /// **THE CURSOR IS THE ONLY WORD, AND IT SAYS WHICH CORNER IS MEANT.** A point of three lines or more is a
    /// corner of every PAIR of them, and the drawing cannot say which pair a person means: the sector the cursor
    /// stands in is the corner being pointed at. `vertex_pair` answers exactly that, and answers it for a point of
    /// two lines as well - there is only one pair there and no word is needed.
    pub fn read_at_point(&self, project: &Project, si: usize, pid: Id, cursor: Option<(f64, f64)>) -> Option<(Id, Id)> {
        if self.occupied(project, si, pid) {
            return None;
        }
        project.vertex_pair(si, pid, cursor)
    }
}

/// HOW WIDE THE ZONE OF A POINT THAT CAN NAME A CORNER IS, in pixels: the reach of a pick of a point, THREE TIMES
/// OVER.
///
/// Three, and not one, because a corner at a point is named by a click and a person aims at a point rather than at
/// a spot beside it: with the reach of the pick itself the zone was a disc a shade wider than the dot, and a click
/// that a person meant for the point landed on the rest of the sheet instead. The zone is generous on purpose -
/// what a wide zone costs is a click on a point that was meant for a line, and that line is one Shift+click away,
/// while a zone too narrow costs the corner the person was sure they had named.
pub fn corner_zone_px(set: &Settings) -> f32 {
    grab::grab(set, grab::Grab::Point) * 3.0
}

/// WHAT THE CURSOR IS OVER, said by the sheet and not by the drawing.
///
/// **A POINT OF THE ZONE OUTRANKS A LINE UNDER THE CURSOR.** A point is the end of the lines that meet there, so
/// a click near a point is near both, and a rule that asked the line first would answer about a line whenever the
/// cursor was near its own end - which is where a person aims when they mean the corner.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CornerOver {
    /// NOTHING OF THE DRAWING, or something the corner tools have no word for.
    Nothing,
    /// A POINT WITHIN THE ZONE, and where the cursor stands in the drawing's own coordinates: it is what says
    /// which of several corners at that point is meant.
    Point { id: Id, at: (f64, f64) },
    /// A LINE, with no point of the zone near it.
    Line(Id),
}

/// WHAT A CLICK DID TO THE CORNER TOOL, so that the words to say are chosen where the click was read and not in
/// the branch that happened to run first.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CornerClick {
    /// THE FIELD WAS NOT OPEN, and is open now for the whole set: the corners are made by the lines as they are
    /// chosen, so there is no single corner to wait to be told about.
    Opened,
    /// THE LINE IS CHOSEN NOW.
    LineChosen(Id),
    /// THE LINE IS LET GO OF, and the corner it made goes with it.
    LineLetGo(Id),
    /// THE LINE IS NOT CHOSEN: it would be a THIRD at a straight joint, and this is the point it would have stood
    /// at. Said rather than cut, because there is nothing to cut there.
    StraightJoint(Id),
    /// WHAT THE CLICK AT THE POINT DID TO THE CORNER THERE.
    AtPoint(PointAct),
    /// A LEFT CLICK WITHOUT SHIFT, ON NOTHING: THE MODE IS SINGLE FROM HERE, and the lines chosen stay chosen.
    SingleMode,
}

/// A CLICK WITH THE CORNER TOOL IN HAND: what is chosen of the drawing, and what that does to the set.
///
/// **THE CLICK THAT OPENS THE FIELD IS THE ONE THAT DOES NOT RESET ANYTHING.** Taking the tool leaves the field
/// closed, so there is no set in hand to single out and the lines that stood chosen are the beginning of one - a
/// person who chose a contour and then took the fillet meant that contour, and the first click adds to it rather
/// than throwing it away. Every click after that is an ordinary one.
///
/// **EVERY LEFT CLICK WITHOUT SHIFT LEAVES THE MULTI-SELECTION, WHATEVER IT LANDED ON - a line, a point, or
/// nothing at all.** Shift is the only way more than one line is chosen, so the set is begun and left with Shift
/// held: a multi-selection that outlives the key is a mode a person cannot see and cannot get out of, and the
/// report was that a click meant for one corner cut the whole set instead. What the click then chooses is a
/// matter of what it landed on, and only that.
///
/// **A CLICK ON A POINT WITHOUT SHIFT IS A SINGLE SELECTION OF A POINT, THE SAME AS A CLICK ON A LINE IS OF A
/// LINE.** It was the one exception, and the exception is what made the tool read as two: a click that landed on
/// a line left the set behind, a click that landed on a point beside one kept it, so what a click did depended on
/// a few pixels of aim. The lines chosen are let go of, the corners they made go with them, the corners named at
/// the other points - and this point is all that is left of the set.
///
/// **THE POINT CLICKED WITHOUT SHIFT IS ITSELF THE ONE EXCEPTION TO THAT**: a point already in the set clicked
/// again without Shift is the one thing to let go of, and the whole set goes with it rather than leaving a set of
/// nothing standing as though it were an answer. With Shift the click is about that point alone and the rest stays.
pub fn corner_click(project: &Project, si: usize, corner: &mut CornerInput, sel_sk: &mut SketchSelection, over: CornerOver, shift: bool) -> CornerClick {
    // THE FIELD OPENS FOR THE WHOLE SET, with the lines that are chosen already chosen: a person who drew a contour,
    // chose it and took the tool meant that contour, and asking again would be asking twice. **THE CLICK THAT OPENS
    // IT IS STILL A CLICK**, and it is acted upon as one - the tool is taken and a corner is pointed at in the same
    // gesture, and making the click only open the field would leave "click a corner, type the value, press Enter" -
    // the whole of the tool - naming nothing at all.
    let was_open = corner.at.is_some();
    if !was_open {
        corner.at = Some((si, CORNER_SET, false));
    }
    let before: Vec<Id> = sel_sk.items.iter().filter(|(k, _)| *k == 1).map(|(_, id)| *id).collect();
    // THE CLICK THAT OPENS THE FIELD GROWS THE SET RATHER THAN SINGLING IT OUT: there was no set in hand to single
    // out, and the lines standing chosen are the start of one.
    let grow = shift || !was_open;
    // A CONSTRUCTION LINE IS NO SIDE OF A CORNER - the diagonal of a rectangle ends at its corners, and a cut between it
    // and a side cuts the drawing against a line that is not part of it - so a click on one is a click on nothing.
    let over = match over {
        CornerOver::Line(eid) if line_is_construction(project, si, eid) => CornerOver::Nothing,
        other => other,
    };
    let act = match over {
        // A CLICK AT A POINT. WITH SHIFT it is about that point alone, as it always was. WITHOUT SHIFT it is a
        // single selection, the same as a click on a line, so everything chosen before is let go of first - and
        // what it leaves behind is either this point alone or, when this point was in the set already, nothing.
        CornerOver::Point { id, at } => {
            corner.shifted = grow;
            if !grow {
                let was_there = corner.set.occupied(project, si, id);
                // THE WHOLE OF THE SET GOES: the lines chosen, the corners they made, the corners named at the
                // other points. What is put back is this point and this point alone.
                sel_sk.items.retain(|&(k, _)| k != 1);
                corner.set.clear();
                let act = if was_there {
                    PointAct::PointRemoved
                } else {
                    let read = corner.set.read_at_point(project, si, id, Some(at));
                    corner.set.click_at_point(project, si, id, read)
                };
                let none: Vec<Id> = Vec::new();
                corner.set.follow_from(project, si, &none, &none);
                return CornerClick::AtPoint(act);
            }
            let read = corner.set.read_at_point(project, si, id, Some(at));
            CornerClick::AtPoint(corner.set.click_at_point(project, si, id, read))
        }
        CornerOver::Line(eid) => {
            corner.shifted = grow;
            let chosen = before.contains(&eid);
            if chosen {
                // A LINE THAT IS CHOSEN AND IS CLICKED AGAIN IS LET GO OF: the corner it made goes with it, and the
                // line that stays does not pair again with a neighbour of the one that left.
                sel_sk.items.retain(|&(k, id)| !(k == 1 && id == eid));
                CornerClick::LineLetGo(eid)
            } else if grow {
                // A THIRD LINE AT A STRAIGHT JOINT IS NOT TAKEN, and the point it would have stood at is named:
                // the corners at a point are taken two at a time, and this one would have to be cut against a
                // straight joint, where there is no corner to cut.
                if let Some(pid) = straight_joint_in_the_way(project, si, eid, &before, &corner.set.spoken_at_points()) {
                    return CornerClick::StraightJoint(pid);
                }
                sel_sk.items.push((1, eid));
                CornerClick::LineChosen(eid)
            } else {
                // A CLICK WITHOUT SHIFT IS A SINGLE SELECTION: only this line is chosen, and the corners that
                // follow from it are the ones one answer cuts.
                sel_sk.items.retain(|&(k, _)| k != 1);
                sel_sk.items.push((1, eid));
                CornerClick::LineChosen(eid)
            }
        }
        // A CLICK ON NOTHING AT ALL IS STILL A CLICK, and it is still one without Shift: the mode is single from
        // here and the lines chosen stay chosen. Said rather than nothing, so that a click which changed no
        // geometry is still a rule and not a corner that went missing.
        CornerOver::Nothing => {
            corner.shifted = shift;
            return if was_open { CornerClick::SingleMode } else { CornerClick::Opened };
        }
    };
    let after: Vec<Id> = sel_sk.items.iter().filter(|(k, _)| *k == 1).map(|(_, id)| *id).collect();
    corner.set.follow_from(project, si, &before, &after);
    act
}

/// WHETHER `eid` IS A CONSTRUCTION LINE of the sketch: no side of a corner the fillet or the chamfer cuts.
fn line_is_construction(project: &Project, si: usize, eid: Id) -> bool {
    project.sketches.get(si).is_some_and(|s| s.entities.iter().any(|e| e.id == eid && e.construction))
}

/// THE POINT AT WHICH A LINE MUST NOT BE ADDED TO THE SELECTION, which is the point where two chosen lines stand
/// along ONE straight line and this one would be a third.
///
/// Two lines that meet at a straight angle are no conflict at all: there is nothing to cut there, and their other
/// ends may carry corners of their own. A third line arriving is a conflict, because the corners at that point are
/// taken two at a time and this one would have to be cut against a straight joint - so the line is not taken, and
/// the reason is said.
///
/// **ONLY THE LINES STILL WAITING AT THE POINT COUNT, NOT THE WHOLE SELECTION.** The lines that already stand in a
/// corner there are spoken for, and a straight joint among them says nothing about the line arriving now: at a
/// point of four lines, two of which lie along one straight line, the third and the fourth are a corner of their
/// own and the joint between the first two is none of their business. Refused there, the fourth line of a cross
/// could not be taken at all, and a cross is the one shape where four corners meet.
pub fn straight_joint_in_the_way(project: &Project, si: usize, line: Id, selected: &[Id], spoken: &[Id]) -> Option<Id> {
    let (x, y) = project.edge_ends(si, line)?;
    [x, y].into_iter().find(|&p| {
        let there: Vec<Id> = selected.iter().copied().filter(|&l| l != line && !spoken.contains(&l) && project.edge_stands_at(si, l, p)).collect();
        there.iter().enumerate().any(|(i, &a)| there[i + 1..].iter().any(|&b| project.shared_vertex(si, a, b) == Some(p) && project.corner_of_pair(si, a, b).is_none()))
    })
}

/// WHETHER THE PICK IS STILL THERE once the corner has been taken off.
///
/// The point of a corner goes with the corner, so a pick that was standing on it cannot stand lit afterwards: the
/// selection would show a point the drawing has not got.
pub fn pick_still_stands(project: &Project, si: usize, id: Id) -> bool {
    project.sketches.get(si).is_some_and(|s| s.points.iter().any(|q| q.id == id) || s.entities.iter().any(|e| e.id == id))
}

/// THE CORNER TWO PICKS NAME: the point it is at, and how the picks say which of the corners there is meant.
///
/// Which of several corners at one point is meant cannot be read off the point alone, so the picks carry it: two
/// edges that share a point name the pair outright, while an edge named together with the point says only that the
/// corner is one of those this edge takes part in - `through` - and the cursor says which.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CornerChoice {
    pub pid: Id,
    /// THE TWO EDGES THAT NAME THE CORNER, where two edges do. `None` where a point was picked, or a point and an
    /// edge: the pair is read off the cursor instead.
    pub edges: Option<(Id, Id)>,
    /// THE EDGE THE CORNER MUST BE TAKEN FROM, where an edge and a point name it together.
    pub through: Option<Id>,
}

/// TAKE THE FILLET OR THE CHAMFER INTO HAND (4 = fillet, 5 = chamfer) and hold it to what is already chosen.
pub fn start_corner_tool(bc: &mut BarCtx, op: u8) {
    set_click_op(&mut tools_in!(bc), &mut *bc.mode_3d, op);
    if bc.armed.click_op() != op {
        return; // pressed again: the tool goes back down, and what was chosen stays chosen
    }
    *bc.status = qymcad_i18n::tr(if op == 5 { "tb-chamfer-sketch-hint" } else { "tb-fillet-sketch-hint" });
    bc.corner.track_px = corner_reach(&*bc.set);
    let si = match *bc.sel {
        Sel::Sketch(si) => si,
        _ => return,
    };
    // WHATEVER STOOD CHOSEN BEFORE THE TOOL WAS TAKEN IS THE SET: the corners those lines make are the corners one
    // answer cuts, and the choice stays lit rather than being asked for again - a person who drew a contour, chose
    // it and took the tool meant that contour.
    //
    // **THE FIELD DOES NOT OPEN UNTIL SOMETHING IS POINTED AT.** It used to open at once, over the middle of the
    // canvas, and the middle of the canvas is where the drawing is: a Shift+click on a line that ran under the box
    // was taken by the box, so a set could not be built out of a part whose lines passed through the middle of the
    // sheet, and the person was left blaming their own aim. A field over the drawing takes clicks away from it, so
    // there is no field until there is something to type into.
    //
    // THE COLOURS OF THE PREVIEW ARE NOT SET HERE: they are read from the scheme where the corner is drawn, on every
    // frame, so that every way of opening a field draws alike and a person who changes the scheme sees it.
    bc.corner.shifted = false; // the tool is single until a Shift is held
    bc.corner.pos = None;
    let chosen: Vec<Id> = bc.sel_sk.items.iter().filter(|(k, _)| *k == 1).map(|(_, id)| *id).collect();
    bc.corner.set.follow(&*bc.project, si, &chosen);
}

/// Turn a click-driven editing operation on or off (1 = trim, 2 = extend, 3 = break).
pub fn set_click_op(t: &mut Tools, mode_3d: &mut bool, op: u8) {
    let cur = t.armed.click_op();
    exit_draw_tools(&mut t.reborrow());
    let Tools { armed, annot: _, cmd: _, corner: _, dim: _, drag: _, gsel: _, inline: _, measure: _, pat: _, pending_import: _, picking: _, place: _, sel_sk, tool } = t;
    **armed = Armed::None;
    sel_sk.constraint = None;
    sel_sk.modify = None;
    **armed = if cur == op { Armed::None } else { Armed::ClickOp(op) };
    // EXTEND TAKES THE CURVE SELECTED BEFORE IT, as every tool takes its selection; nothing selected, it waits for a pick
    tool.extend = if armed.click_op() == 2 { sel_sk.items.iter().find(|(k, _)| *k == 1).map(|(_, id)| *id) } else { None };
    if armed.click_op() != 0 {
        *mode_3d = false;
    }
}

/// Turn the dimension tool on or off (k: 1 = linear, 2 = angular, 3 = radius).
pub fn set_dim_tool(t: &mut Tools, mode_3d: &mut bool, project: &Project, sel: Sel, sketch_ses: SketchSession, status: &mut String, k: u8) {
    let cur = t.armed.dim_kind();
    exit_draw_tools(&mut t.reborrow()); // entering a tool means leaving all the others, in one move
    let Tools { armed, annot: _, cmd: _, corner: _, dim: _, drag: _, gsel: _, inline: _, measure: _, pat: _, pending_import: _, picking: _, place: _, sel_sk: _, tool: _ } = t;
    **armed = if cur == k { Armed::None } else { Armed::Dimension(k) };
    if armed.dim_kind() != 0 {
        *mode_3d = false;
        if edit_si(project, &sketch_ses).is_none() && !matches!(sel, Sel::Sketch(_)) {
            *status = qymcad_i18n::tr("g-open-sketch-dim");
        } else {
            *status = qymcad_i18n::tr("g-dim-hint");
        }
    }
}

/// HOW A NUMBER IN THE OPTIONS BAR IS WRITTEN AND BOUNDED: the limits it is clamped to, whether it is
/// whole, and the unit shown after it.
///
/// Four things about the SHAPE of the field, as opposed to its value, and they trailed twenty-nine call
/// sites as four loose arguments, where `false` and `""` said nothing about which was which.
#[derive(Clone, Copy)]
pub struct NumFormat<'a> {
    pub lo: f64,
    pub hi: f64,
    pub integer: bool,
    pub suffix: &'a str,
    /// Zero is no value for this field, though both signs are: an offset of 0 is the geometry itself.
    pub nonzero: bool,
}

pub fn num_or_expr(p: &mut ExprBarCtx, ui: &mut egui::Ui, key: &'static str, cur: f64, fmt: NumFormat) -> f64 {
    let NumFormat { lo, hi, integer, suffix, nonzero } = fmt;
    let vars = p.project.param_map();
    // THE TEXT IS BORROWED FOR A MOMENT. The drop-down list reads the whole document while the buffer
    // sits in the same `self`, and both cannot be borrowed at once. We work on a copy and put it back.
    let txt = p.bar_exprs.get(key).cloned().unwrap_or_else(|| if integer { format!("{cur:.0}") } else { qymcad_core::expr::fmt_num(cur) });
    let w = (60.0 + suffix.len() as f32 * 6.0).min(96.0);
    // ONE FIELD, AND IN THE FEATURES TOO, not only in sketches. This expression field is a single one
    // for ALL the tool bars (see `expr_fields.rs`), so the list of drivers and the typing rules arrive
    // everywhere at once: in extrude, fillet, chamfer, shell, hole, draft and pattern.
    //
    // A tool bar does not change the document — it shows a preview — so the text lives in `bar_exprs`
    // and is read every frame; the rule that editing text is not editing the model holds by itself here.
    let o = expr_field(ui, p.project, bar_field_id(key), &txt, w, &qymcad_i18n::tr("g-expr-placeholder"));
    // ENTER IN A FIELD OF THE BAR IS THE ENTER THE BAR ASKS FOR: it leaves the field and is marked, so the tool
    // applies at the same press instead of waiting for a second one (see `bar_enter_take`)
    if o.committed && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
        let frame = ui.ctx().cumulative_pass_nr();
        ui.data_mut(|d| d.insert_temp(egui::Id::new(BAR_ENTER), frame));
    }
    let txt = o.text;
    p.bar_exprs.insert(key, txt.clone());
    // A VALUE THAT CANNOT BE TAKEN is refused where it is typed: the reason on the mark beside the field, the old value
    // kept, the Apply and Enter of the command closed (`bar_fields_valid`). It used to be pressed to the nearest limit
    // or rounded and taken without a word: 521 copies became 512, 6.5 became 7.
    let checked = qymcad_core::expr::eval(&txt, &vars).map_err(|e| qymcad_i18n::error_words::expr_error_text(&e)).and_then(|v| {
        if nonzero && v.abs() < 1e-9 {
            Err(qymcad_i18n::tr("cmd-value-zero"))
        } else if integer && (v - v.round()).abs() > 1e-9 {
            Err(qymcad_i18n::tr("cmd-value-whole"))
        } else if v < lo - 1e-9 || v > hi + 1e-9 {
            Err(qymcad_i18n::tr2("cmd-value-out-of-range", "lo", &qymcad_core::expr::fmt_num(lo), "hi", &qymcad_core::expr::fmt_num(hi)))
        } else {
            Ok(v)
        }
    });
    BAR_BAD_KEYS.with(|b| {
        let mut b = b.borrow_mut();
        if checked.is_err() {
            b.insert(key);
        } else {
            b.remove(key);
        }
    });
    let frame = ui.ctx().cumulative_pass_nr();
    ui.data_mut(|d| {
        let bad = d.get_temp_mut_or_default::<std::collections::HashMap<&'static str, u64>>(egui::Id::new(BAR_BAD));
        if checked.is_err() {
            bad.insert(key, frame);
        } else {
            bad.remove(key);
        }
    });
    match checked {
        Ok(v) => v,
        Err(msg) => {
            // the reason on the mark beside the field, as the fields at the geometry give it
            ui.colored_label(p.scheme.pal.error_mild(), egui_phosphor::regular::X).on_hover_text(&msg);
            cur
        }
    }
}

/// The widget of the bar field under `key`: whoever shares its value asks it whether it is being typed in.
pub fn bar_field_id(key: &'static str) -> egui::Id {
    egui::Id::new(("bar_expr", key))
}

/// Where the fields of the tool bars keep which of them hold a value that cannot be taken, and in which frame.
const BAR_BAD: &str = "bar_expr_bad";

/// Where a field of a tool bar marks the frame its Enter was pressed in.
const BAR_ENTER: &str = "bar_expr_enter";

/// WAS ENTER PRESSED IN A FIELD OF THE BAR just now (this frame or the one before)? Taken once: the tool that
/// applies on it clears the mark. The field keeps the keyboard in the frame of the press, so the tool's own check of
/// Enter - which waits for the keyboard to be free - would not see it until a second press.
pub fn bar_enter_take(ctx: &egui::Context) -> bool {
    bar_enter_take_inner(ctx)
}

/// THE BAR'S APPLY BUTTON IS THE BAR'S ENTER: pressed, it marks the same Enter a field of the bar marks, so the tool
/// applies by the one path it applies on from the keyboard (`bar_enter_take`).
pub fn bar_apply_press(ctx: &egui::Context) {
    let frame = ctx.cumulative_pass_nr();
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(BAR_ENTER), frame));
}

fn bar_enter_take_inner(ctx: &egui::Context) -> bool {
    let now = ctx.cumulative_pass_nr();
    let at: Option<u64> = ctx.data_mut(|d| d.remove_temp(egui::Id::new(BAR_ENTER)));
    at.is_some_and(|f| f + 1 >= now) && bar_fields_valid(ctx)
}

thread_local! {
    /// The keys of the bar fields holding a value that cannot be taken, as last drawn - for the tools that act on a
    /// click rather than on Enter, where no frame context is at hand (the offset of a sketch).
    static BAR_BAD_KEYS: std::cell::RefCell<std::collections::HashSet<&'static str>> = std::cell::RefCell::new(std::collections::HashSet::new());
}

/// DOES THE BAR FIELD `key` HOLD A VALUE THAT CANNOT BE TAKEN, as it was last drawn?
pub fn bar_field_bad(key: &str) -> bool {
    BAR_BAD_KEYS.with(|b| b.borrow().contains(key))
}

/// DO THE FIELDS OF THE TOOL BAR DRAWN NOW ALL HOLD VALUES THAT CAN BE TAKEN? A field refused in the frame just drawn
/// (or the one before) closes Apply and Enter; a field of a bar no longer drawn does not count.
pub fn bar_fields_valid(ctx: &egui::Context) -> bool {
    let now = ctx.cumulative_pass_nr();
    ctx.data(|d| d.get_temp::<std::collections::HashMap<&'static str, u64>>(egui::Id::new(BAR_BAD))).is_none_or(|bad| bad.values().all(|&f| f + 2 < now))
}

/// Restore the view to the state it was left in. With nothing to restore (the sub-mode was entered
/// without `borrow_view`), the old behaviour applies: back to 3D, but WITHOUT refitting.
pub fn return_view(pc: &mut PartCtx) {
    match pc.view_restore.take() {
        Some((m, cam, view)) => {
            *pc.mode_3d = m;
            *pc.cam = cam;
            *pc.view = view;
        }
        None => *pc.mode_3d = true,
    }
}

/// A 3x4 row-major transform that is approximately the identity (no rotation and no translation).
pub fn is_identity12(m: &[f64; 12]) -> bool {
    const ID: [f64; 12] = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0];
    m.iter().zip(ID.iter()).all(|(a, b)| (a - b).abs() < 1e-9)
}

/// A large icon button for the left tool bar. Returns whether it was clicked.
/// The icon button of the workbench's left panel. A FIXED size (`add_sized`, not `min_size`): wide
/// glyphs (the move and expand arrows) must not inflate the button and break the grid, or the row
/// stops holding exactly two columns. The size is THE SAME as `sym_button`'s (40x34) — both grids mix
/// inside one `horizontal_wrapped`, and a mismatch of widths (34 against 38) broke the wrap onto two
/// columns. The usable width of the tools panel is 108 - 16 (padding) - 6 (the floating scrollbar) =
/// 86 px: two columns (40+3+40=83) fit with room to spare and hold at exactly two.
/// ENTER WITH NOTHING YET TO APPLY is refused in words: the tool's own ask repeated after "not yet". A key that did
/// nothing and said nothing left a person pressing it again.
pub fn enter_not_ready(ctx: &egui::Context, status: &mut String, ask: &str) {
    if !ctx.egui_wants_keyboard_input() && ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
        *status = qymcad_i18n::tr1("cmd-not-ready", "ask", ask);
    }
}

/// WHAT A TOOL BUTTON ASKS FOR: its tool when the hand is free of it, the hand put down when that tool is already in
/// it - a sketch tool back to the arrow, a command cancelled - as the button of every tool of a professional CAD does.
pub fn take_or_drop(held: bool, ask: BarAsk) -> BarAsk {
    match (held, ask) {
        (false, ask) => ask,
        (true, BarAsk::SketchTool(_)) => BarAsk::SketchSelectMode,
        (true, _) => BarAsk::CancelAllTools(Then::Nothing),
    }
}

pub fn icon_tool(ui: &mut egui::Ui, icon: &str, tip: &str, active: bool) -> bool {
    let btn = egui::Button::new(egui::RichText::new(icon).size(19.0)).selected(active);
    ui.add_sized(egui::vec2(40.0, 34.0), btn).on_hover_text(tip).clicked()
}

/// The direction of an offset vector, as (dir 0/1/2, a signed step) — for reopening a linear pattern.
pub fn arr_dir_of(dx: f64, dy: f64, dz: f64) -> (u8, f64) {
    let (ax, ay, az) = (dx.abs(), dy.abs(), dz.abs());
    if az >= ax && az >= ay {
        (2, dz)
    } else if ay >= ax {
        (1, dy)
    } else {
        (0, dx)
    }
}

/// THE SIZE OF A SKETCH CHAMFER as the tool holds it: the mode, the first leg (`fillet`) and the second value.
pub fn chamfer_legs(prefs: &SketchToolPrefs) -> qymcad_core::model::ChamferLegs {
    qymcad_core::model::ChamferLegs { mode: prefs.chamfer_mode, first: prefs.fillet, second: prefs.chamfer_second }
}

/// THE CHAMFER'S MODE CHOSEN ON THE BAR, with the second value set to what the new mode means: the angle 45 deg, the
/// second leg the first one - a number in degrees is no leg, nor a leg an angle.
pub fn set_chamfer_mode(prefs: &mut SketchToolPrefs, mode: qymcad_core::feature::ChamferMode) {
    if prefs.chamfer_mode == mode {
        return;
    }
    prefs.chamfer_mode = mode;
    prefs.chamfer_second = match mode {
        qymcad_core::feature::ChamferMode::DistAngle => 45.0,
        _ => prefs.fillet,
    };
}

/// The caption of a fillet's field, and the word of its mode on the bar: the radius, the chord or the arc length.
pub fn fillet_label(by: qymcad_core::model::FilletBy) -> &'static str {
    use qymcad_core::model::FilletBy;
    match by {
        FilletBy::Radius => "opt-radius",
        FilletBy::Chord => "opt-fillet-chord",
        FilletBy::ArcLength => "opt-fillet-arc-length",
    }
}

/// THE MOST A CORNER TAKES, in the way its size is given: the cut or the leg of a chamfer, or the radius, the chord or
/// the arc length of a fillet - the largest radius worked into the chord or the arc of the same turn. `None` where the
/// corner is not one of two lines.
pub fn corner_limit_in(project: &Project, si: usize, pid: Id, tool: qymcad_core::model::CornerTool, by: qymcad_core::model::FilletBy) -> Option<f64> {
    let l = project.corner_limit(si, pid, tool)?;
    if tool != qymcad_core::model::CornerTool::Fillet || by == qymcad_core::model::FilletBy::Radius {
        return Some(l);
    }
    project.corner_sweep(si, pid).map(|sweep| qymcad_core::model::FilletSize::of_radius(by, l, sweep))
}

/// THE CORNER TOOL IN HAND, with the way the bar reads a chamfer's first value.
pub fn corner_tool(chamfer: bool, prefs: &SketchToolPrefs) -> qymcad_core::model::CornerTool {
    if chamfer {
        qymcad_core::model::CornerTool::Chamfer(prefs.chamfer_mode)
    } else {
        qymcad_core::model::CornerTool::Fillet
    }
}

/// The size of the fillet the bar holds: the way it is given and the value in that way.
pub fn fillet_size(prefs: &SketchToolPrefs) -> qymcad_core::model::FilletSize {
    qymcad_core::model::FilletSize { by: prefs.fillet_by, value: prefs.fillet }
}

/// The label of a chamfer's first field: the size of an equal chamfer, the first of two legs, or the length along the
/// line the angle is measured from.
pub fn chamfer_d1_label(mode: qymcad_core::feature::ChamferMode) -> &'static str {
    use qymcad_core::feature::ChamferMode;
    match mode {
        ChamferMode::Symmetric => "opt-chamfer-size",
        ChamferMode::TwoDist => "cmd-leg1",
        ChamferMode::DistAngle => "cmd-chamfer-length",
    }
}

/// The label of a chamfer's second field: its meaning depends on the mode — the second leg, or the angle.
pub fn chamfer_d2_label(mode: qymcad_core::feature::ChamferMode) -> &'static str {
    use qymcad_core::feature::ChamferMode;
    match mode {
        ChamferMode::DistAngle => "cmd-angle-deg",
        _ => "cmd-leg2",
    }
}

/// The switch index for a standard (the reverse mapping, used while editing a feature).
pub fn thread_standard_idx(s: qymcad_core::thread::ThreadStandard) -> u8 {
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

/// The plane a mirror or a split has picked, as the node stores it: (`plane` a world plane 0/1/2, `datum` Id, `face`).
/// A world plane gives (0/1/2, 0, None), a datum (0, id, None), a face (0, 0, Some(body, key)) - the node reads its
/// plane off the face itself, one node of the timeline, where a face used to make a datum plane of its own beside it.
/// The same one is used both when creating and when editing.
pub fn resolve_mirror_plane(sp: qymcad_core::feature::SketchPlane) -> (u8, Id, Option<(Id, qymcad_core::feature::FaceKey)>) {
    use qymcad_core::feature::{BasePlane, SketchPlane};
    match sp {
        SketchPlane::World(BasePlane::XY) => (0, 0, None),
        SketchPlane::World(BasePlane::XZ) => (1, 0, None),
        SketchPlane::World(BasePlane::YZ) => (2, 0, None),
        SketchPlane::Datum(id) => (0, id, None),
        SketchPlane::Face(body, key) => (0, 0, Some((body, key))),
    }
}

/// The plane a stored mirror or split stands on, back as the pick of its command - for reopening it.
pub fn op_plane_pick(plane: u8, datum: Id, face: Option<(Id, qymcad_core::feature::FaceKey)>) -> qymcad_core::feature::SketchPlane {
    use qymcad_core::feature::{BasePlane, SketchPlane};
    match (face, datum) {
        (Some((body, key)), _) => SketchPlane::Face(body, key),
        (None, d) if d != 0 => SketchPlane::Datum(d),
        _ => SketchPlane::World(match plane {
            1 => BasePlane::XZ,
            2 => BasePlane::YZ,
            _ => BasePlane::XY,
        }),
    }
}

/// Store the pattern step's expression on the ACTIVE component of the vector (regeneration follows
/// that one) and clear the others. A plain number clears the expression, and then the feature's stored
/// number applies.
pub fn store_arr_component(project: &mut qymcad_core::model::Project, body: Id, keys: [&str; 3], dir: u8, txt: String) {
    let t = txt.trim().to_string();
    let expr = !t.is_empty() && t.parse::<f64>().is_err();
    for (i, k) in keys.iter().enumerate() {
        if i as u8 == dir && expr {
            project.set_feat_dim(body, k, t.clone());
        } else {
            project.set_feat_dim(body, k, String::new());
        }
    }
}

/// The autosave path sits next to the document (`name.autosave.qcad`); an unnamed document goes to temp.
///
/// FROM THE STEM, NOT FROM THE WHOLE NAME. `format!("{path}.autosave.qcad")` wrote the extension twice -
/// `Filter-v2.qcad.autosave.qcad` - and a second autosave over the first gave
/// `Filter-v2.qcad.autosave.qcad.bak`. A name nobody can read is a name nobody checks.
pub fn autosave_path(project_path: &Option<String>) -> String {
    match &project_path {
        Some(p) => {
            let base = std::path::Path::new(&p);
            base.with_extension("").to_string_lossy().into_owned() + ".autosave.qcad"
        }
        None => std::env::temp_dir().join("qymcad-unsaved.autosave.qcad").to_string_lossy().into_owned(),
    }
}

/// The accumulated transform of a component's gizmo (relative to the START transform): a shift along an
/// axis, or a rotation about a ring through the fixed origin; `snap` rounds to the step (the grid or
/// the angle). Unified with the body gizmo.
pub fn comp_giz_accum(comp_giz: &CompGizmo, set: &Settings, snap: bool) -> Option<[f64; 12]> {
    let (_, start, origin, amt) = comp_giz.drag?;
    if let Some(ax) = comp_giz.axis {
        let step = set.snap.grid.max(0.01);
        let dmm = if snap { (amt / step).round() * step } else { amt };
        let mut t = qymcad_core::feature::PLACE_IDENTITY;
        t[[3, 7, 11][ax as usize]] = dmm;
        Some(compose12(&t, &start))
    } else if let Some(ax) = comp_giz.ring {
        let step = set.snap.rot_deg.max(0.1);
        let deg = if snap { (amt / step).round() * step } else { amt };
        Some(compose12(&rot_about_point(ax, deg, origin), &start))
    } else {
        None
    }
}

/// The screen anchor of the active command (where to stick the input field): for extrude and revolve
/// it is at the tip of the arrow; for a fillet or a chamfer at the first selected edge; for a shell
/// or a hole at the centre of the face.
/// The screen anchor of a command popup TO THE SIDE of body `b`: the top-right corner of its
/// projected bounding box, plus a margin. The popup used to be anchored at the CENTRE of the box, so
/// it covered the part and what was needed could not be picked behind it. Now it grows to the right
/// of the body rather than on top of it.
pub fn body_side_anchor(dc: &DrawCtx, b: Id, rect: Rect, basis: &([f64; 3], [f64; 3], [f64; 3])) -> Option<Pos2> {
    let mi = dc.project.mesh_index(b)?;
    let bb = dc.project.bodies[mi].mesh.bounds()?;
    // where the body is DRAWN: through its placement in the context (an imported part stands where it was put)
    let wt = dc.project.body_display_transform(b, current_ctx_id(dc.active_path, dc.project));
    let mut body = Rect::NOTHING;
    for &x in &[bb.min.x, bb.max.x] {
        for &y in &[bb.min.y, bb.max.y] {
            for &z in &[bb.min.z, bb.max.z] {
                body.extend_with(Screen { cam: dc.cam, set: dc.set, rect, basis }.at(qymcad_core::feature::apply12(&wt, [x, y, z])).0);
            }
        }
    }
    if !body.is_positive() {
        return None;
    }
    // BESIDE THE BODY, WHERE THE POPUP FITS CLEAR OF IT: right, left, above, below - the first place where a popup of
    // about 240 x 140 (a caption, a field or four and a button) stands wholly inside the view and off the body. Right
    // of the body used to be the only place, pulled back over the body when it filled the view: the popup covered the
    // face the next hole had to be clicked on. Where no place is clear, right of it as before.
    const W: f32 = 240.0;
    const H: f32 = 140.0;
    let view = rect.shrink(4.0);
    let places = [
        Pos2::new(body.max.x + 14.0, body.min.y.max(view.min.y + 32.0)),
        Pos2::new(body.min.x - 14.0 - W, body.min.y.max(view.min.y + 32.0)),
        Pos2::new(body.min.x, body.min.y - 14.0 - H),
        Pos2::new(body.min.x, body.max.y + 14.0),
    ];
    let clear = |p: &Pos2| {
        let r = Rect::from_min_size(*p + egui::vec2(10.0, -10.0), egui::vec2(W, H));
        view.contains_rect(r) && !r.intersects(body)
    };
    Some(places.into_iter().find(clear).unwrap_or(places[0]))
}

/// Restore a thread operation's axis and radius from a circular edge while the feature is being EDITED
/// (a double click). The axis is oriented into the body, because the kernel gives a rim an arbitrary normal.
pub fn restore_thread_axis(project: &mut Project, thread: &mut ThreadParams, src: Id, edge: u32) {
    if let Some((c, ax0, r)) = project.regen_edges.get(&src).and_then(|es| es.iter().find(|e| e.id == edge && e.is_circular())).map(|e| (e.center, e.axis, e.radius)) {
        let mut ax = ax0;
        if let Some(bb) = project.mesh_index(src).and_then(|mi| project.bodies[mi].mesh.bounds()) {
            let cc = [(bb.min.x + bb.max.x) * 0.5, (bb.min.y + bb.max.y) * 0.5, (bb.min.z + bb.max.z) * 0.5];
            let tob = [cc[0] - c[0], cc[1] - c[1], cc[2] - c[2]];
            if tob[0] * ax[0] + tob[1] * ax[1] + tob[2] * ax[2] < 0.0 {
                ax = [-ax[0], -ax[1], -ax[2]];
            }
        }
        thread.axis = (c, ax);
        thread.radius = r;
    }
}

pub fn delete_contour(project: &mut Project, regen: &mut Rebuilding, sel: &mut Sel, view: &mut View2d, i: usize) {
    project.remove_contour(i);
    *sel = Sel::None;
    invalidate(regen);
    view.initialized = false;
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
pub fn clear_feat_picks(p: FeatPicks) {
    p.thread.src = None;
    p.thread.edge = 0;
    p.mirror.plane = None;
    p.arr.axis_pick = false;
    p.datum.plane_pick = None;
    p.datum.axis_ref = None;
    p.datum.axis_hit = None;
    p.datum.axis_pts.clear();
    p.datum.pt_vert = None;
    p.sweep.prof_sid = 0;
    p.sweep.path_sid = 0;
    p.sweep.pick_path = false;
    p.sweep.prof_cid = 0;
    p.sweep.path_cid = 0;
    p.loft.sids.clear();
    p.loft.cids.clear();
    p.loft.pick = false;
    p.loft.pick_last = None;
    p.loft.ruled = false;
    p.picking.clear();
    p.draft.neutral = 0;
    p.draft.pick_neutral = false;
    p.draft.flip = false;
    p.stitch_parts.clear(); // the sheets picked for stitching are a pick like everything above
    p.recognise.src = None;
    p.trim.keep = None;
    p.trim.tool = None;
}

/// Borrow the view for a flat sub-mode: remember the state it was left in. A repeat call does not
/// overwrite it — the first entry holds the state to come back to.
pub fn borrow_view(cam: Cam3, mode_3d: bool, view: View2d, view_restore: &mut Option<(bool, Cam3, View2d)>) {
    if view_restore.is_none() {
        *view_restore = Some((mode_3d, cam, view));
    }
}

/// THE DOCUMENT CHANGED WITH AUTHORITY, but no undo step is created.
///
/// A third kind of change to the document: neither an edit made by hand nor a rebuild, but
/// NAVIGATION — entering a component changes `active_component`, and that is stored in the file.
/// Nobody expects to undo a move like that ("Undo: entering a subassembly" is nonsense), yet it must
/// not be declared an edit outside a boundary either: that is exactly why the application panicked on
/// entering a subassembly. The method exists so that such places are VISIBLE and named, rather than
/// hidden behind a silent update of the key.
pub fn doc_touched_without_undo(edits: &mut Edits, project: &Project) {
    edits.committed_key = doc_key(project);
    // the context stood in is part of the state a step puts again: a redo of "New part" is taken from this baseline,
    // and one left in the assembly put the part back with the window outside it
    edits.baseline.project.active_component = project.active_component;
}

/// The command did not apply: say so and record the fact, so the operation can be rolled back.
pub fn cmd_fail(cmd_failed: &mut bool, status: &mut String, text: String) {
    *status = text;
    *cmd_failed = true;
}

/// Make sure the active context path is sound: it starts at the root and holds no deleted nodes.
pub fn ensure_active_path(active_path: &mut Vec<Id>, project: &mut Project) {
    let root = project.ensure_root();
    let comp_ids: std::collections::HashSet<Id> = project.components.iter().map(|c| c.id).collect();
    if active_path.first() != Some(&root) {
        *active_path = vec![root];
    }
    active_path.retain(|id| *id == root || comp_ids.contains(id));
    if active_path.is_empty() {
        *active_path = vec![root];
    }
}

/// WHAT A NUMBER FIELD IN A TOOL BAR NEEDS: the text being typed, the document it may name parameters
/// from, and the colours. Three fields - and the field was a method of `App` only because the buffer is.
pub struct ExprBarCtx<'a> {
    pub bar_exprs: &'a mut std::collections::HashMap<&'static str, String>,
    pub project: &'a qymcad_core::model::Project,
    pub scheme: &'a SchemeUi,
}

/// THE LOOK OF A SCHEME FOR THE PANELS, whatever theme the system says it is in. egui keeps a look for its dark theme
/// and one for its light, follows the system's between them, and `set_visuals` fills only the one in use: Windows
/// telling its light theme on the first frames turned a dark scheme's panels into the factory light look. Reported
/// behaviour (Windows 10): the chosen theme did not survive a restart - kept in the settings, not shown.
pub fn put_look(ctx: &egui::Context, pal: &qymcad_scheme::Palette) {
    let v = qymcad_scheme::visuals(pal);
    ctx.set_visuals_of(egui::Theme::Dark, v.clone());
    ctx.set_visuals_of(egui::Theme::Light, v);
}

pub fn apply_theme(scheme: &mut SchemeUi, set: &Settings, ctx: &egui::Context) {
    // A SCHEME SETS BOTH THE CANVAS PALETTE AND THE LOOK OF `egui` ITSELF. These used to be two
    // unrelated things: the theme changed the buttons while the canvas stayed dark — exactly what was
    // reported.
    scheme.pal = scheme.all.iter().find(|p| p.id == set.scheme).cloned().unwrap_or_else(qymcad_scheme::dark);
    // THE LOOK OF `egui` ASKS THE SCHEME rather than choosing between light and dark by itself:
    // otherwise a scheme that colours the interface would colour only the canvas, and half the window
    // would stay factory-coloured. Schemes with no interface colours of their own get exactly that
    // same factory look.
    put_look(ctx, &scheme.pal);
    // THE INTERFACE SCALE IS NOT APPLIED HERE: it has nothing to do with the theme. The coupling was
    // hidden and harmful — because of it "adopt the settings" would work even without its own call to
    // the scale, and the guard would stay silent. The scale is applied by those whose business it is:
    // `adopt_settings` and the slider in the window.
}

/// Ctrl+C and Ctrl+X. While editing a sketch with entities selected, this copies or cuts the
/// GEOMETRY (waiting for the base point to be clicked). Otherwise it acts on a tree node (a sketch, a
/// part or a subassembly).
/// Whether there is anything to copy right now (used to enable the Edit menu items).
pub fn clipboard_can_copy(project: &Project, sel: Sel, sel_sk: &SketchSelection, sketch_ses: SketchSession) -> bool {
    if edit_si(project, &sketch_ses).is_some() {
        return sel_sk.items.iter().any(|(k, _)| *k == 1);
    }
    match sel {
        Sel::Sketch(_) => true,
        Sel::Component(ci) => project.components.get(ci).map(|c| c.id) != Some(project.root),
        _ => false,
    }
}

/// Fillet ALL the corners of the current sketch's contour with the `sk_fillet` radius.
pub fn fillet_all_corners(project: &Project, corner: &mut CornerInput, picking: &mut Picking, sel: Sel, sel_sk: &SketchSelection, status: &mut String, tool_prefs: &SketchToolPrefs) {
    // A COMMAND. With a selection, the popup opens on it straight away; without one, the mode becomes "click a shape".
    if let Sel::Sketch(si) = sel {
        let only: Vec<Id> = sel_sk.items.iter().filter(|(k, _)| *k == 1).map(|(_, id)| *id).collect();
        // the tool is held either way: after Enter it waits for the next shape, as a tool of the sketch stays in hand
        *picking = Picking::FilletAll;
        if only.is_empty() {
            *status = qymcad_i18n::tr("g-fillet-all-hint");
        } else {
            // THE CHOSEN LINES GO IN AS THE WHOLE SET, and the corners they make are the corners one answer cuts:
            // this command is the same act as naming the lines with Shift, said in one word.
            corner.at = Some((si, 0, false));
            corner.pos = None; // the centre of the canvas
            corner.buf = format!("{}", (tool_prefs.fillet * 1000.0).round() / 1000.0);
            corner.focus = true;
            corner.shifted = false;
            corner.set.follow(project, si, &only);
        }
    }
}

/// Whether the selection mode is active (no tool is in hand).
///
/// Three fields had to agree for this to be true, and it was three of the nine that could disagree.
/// Now it is one question to one field.
pub fn in_select_mode(armed: &Armed) -> bool {
    !armed.any()
}

pub fn cancel_feat_cmd(pc: &mut PartCtx) {
    // THE MODE IS READ BEFORE THE CLOSE. `cmd.close()` resets the command to its default state, and
    // `prev_3d` along with it, to `false`. The old code read it AFTERWARDS and so compared the current
    // view against a false "it was flat": working in 3D, Esc out of ANY command threw you into a flat
    // projection with a refit. Reported behaviour: open the editing of an operation, press Esc, and the
    // viewport breaks into some two-dimensional projection — and so with everything.
    let prev_3d = pc.cmd.prev_3d;
    pc.cmd.close(pc.armed); // the command is closed as a whole, not just a field zeroed out
    pc.cmd.edit = None;
    end_feat_cmd_state(crate::feat_picks_in!(pc), pc.cmd, pc.gsel);
    // The view borrowed for the command is RESTORED whole (see `borrow_view`): the camera belongs to
    // the person, and cancelling an action does not touch it. If nothing was borrowed, simply restore
    // the mode, WITHOUT refitting: refitting is exactly what throws away a carefully arranged view.
    if pc.view_restore.is_some() {
        return_view(pc);
    } else if *pc.mode_3d != prev_3d {
        *pc.mode_3d = prev_3d;
    }
    *pc.status = qymcad_i18n::tr("msg-cancelled");
}

/// An edge polyline is nearly STRAIGHT (and so fit to be an axis): every point lies on the line from the
/// first to the last. An arc or a circle (a deviation above 1% of the length plus 0.05 mm) is rejected,
/// otherwise the chord of a circle would become an "axis".
pub fn is_straight_poly(poly: &[[f32; 3]]) -> bool {
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

/// The node index for DELETING the whole chain of body `body`: walk the lineage BACK towards the root,
/// stopping at the DEEPEST body that still HAS a node (for a torus plus a Move that is the torus's
/// node; for a dangling Move whose source is gone, the Move's own node). `delete_feature` then cascades
/// forward and prune finishes the job.
pub fn lineage_delete_ti(project: &qymcad_core::model::Project, body: Id) -> Option<usize> {
    let mut b = body;
    let mut best = project.timeline.iter().position(|n| n.kind.body() == Some(b));
    for _ in 0..256 {
        let Some(src) = project.timeline.iter().find(|n| n.kind.body() == Some(b)).and_then(|n| n.kind.consumed_body()) else { break };
        match project.timeline.iter().position(|n| n.kind.body() == Some(src)) {
            Some(ti) => {
                best = Some(ti);
                b = src;
            }
            None => break, // a source with no node (a dangling one): go no deeper
        }
    }
    best
}

/// Fill in the faces by mesh detection ONLY for bodies WITHOUT a B-rep (a raw imported STL): bodies
/// built by the timeline and those imported from STEP already have their faces from the B-rep topology.
/// The principle: faces are derived from the B-rep, and mesh detection applies only where there is no
/// B-rep at all.
pub fn detect_missing_faces(live: &mut LiveGeom, project: &mut Project) {
    for i in 0..project.bodies.len() {
        let has_brep = project.mesh_id(i).is_some_and(|id| live.shapes.contains_key(&id));
        let empty = project.bodies.get(i).is_none_or(|b| b.faces.is_empty());
        if !has_brep && empty {
            let f = project.bodies[i].mesh.detect_faces(8.0);
            project.bodies[i].faces = f.clone();
            if let Some(body) = project.mesh_id(i) {
                live.faces.insert(body, f); // mesh detection goes into the cache by body Id as well
            }
        }
    }
}

/// WHETHER A LIVE B-REP IS NEEDED RIGHT NOW - as one list, not as a condition scattered across places.
///
/// A project opened from a bundle carries no live B-rep: it is built ON DEMAND. Whoever does not state the
/// demand silently gets NOTHING, and that looks like "the tool does not work". And so it was: a joint anchor
/// on an edge found nothing, a sketch on a face did not show that face's outline, and a thread answered a
/// click on a cylinder with "missed - click a cylindrical face". The list must be a single one, otherwise the
/// next edge-based tool will forget about it all over again.
/// WHAT THE INTERFACE IS IN THE MIDDLE OF: the command being run, what it has picked, the joint being
/// made, whether a click is awaited, the sketch session, and which workbench is showing.
///
/// Six things that together say what the user is doing, as opposed to what the document holds. Whether
/// a live B-rep has to be kept in memory is decided by exactly these.
#[derive(Clone, Copy)]
pub struct Doing<'a> {
    pub armed: &'a Armed,
    pub cmd: &'a FeatCommand,
    pub gsel: &'a GeomSelection,
    pub joint: &'a JointCommand,
    pub picking: Picking,
    pub sketch_ses: SketchSession,
    pub workbench: Workbench,
}

/// The six, taken from a CONTEXT, where the fields are already borrows.
#[macro_export]
macro_rules! doing_in {
    ($x:expr) => {
        $crate::Doing { armed: $x.armed, cmd: &*$x.cmd, gsel: &*$x.gsel, joint: $x.joint, picking: *$x.picking, sketch_ses: *$x.sketch_ses, workbench: $x.workbench }
    };
}

/// THE FLAT SHEET a sketch is drawn on: how it is panned and zoomed, and the rectangle it lands in.
///
/// The pair travels together everywhere a sketch point becomes a screen point.
#[derive(Clone, Copy)]
pub struct Sheet {
    pub view: View2d,
    pub rect: Rect,
}

impl Sheet {
    /// Where a point of the sketch lands on the screen.
    ///
    /// The sheet is held BY VALUE: `View2d` is a handful of numbers, so nothing here borrows the
    /// document, and the frame can be lifted into a local even in a function that goes on to change it.
    pub fn at(&self, p: Point2) -> Pos2 {
        let c = self.rect.center();
        Pos2::new(c.x + (p.x as f32 - self.view.center.x) * self.view.scale, c.y - (p.y as f32 - self.view.center.y) * self.view.scale)
    }
}

pub fn needs_live_brep(d: Doing, project: &Project, set: &Settings) -> bool {
    let Doing { armed, cmd: _, gsel, joint, picking, sketch_ses, workbench } = d;
    // chamfer (4), fillet (5) and thread (24) work by edges and by face axes
    if matches!(armed.cmd_kind(), 4 | 5 | 24) || !gsel.edges.is_empty() {
        return true;
    }
    // the interference check computes the volume of the common part through the kernel - without a live B-rep no pairs can be found
    if set.show_interference && matches!(workbench, Workbench::Assembly) {
        return true;
    }
    // choosing or replacing a sketch plane snaps to edges and vertices
    if picking.is_sketch_plane() || picking.replace_sketch().is_some() {
        return true;
    }
    // A MATE ANCHOR OF ANY KIND, not only an edge or a vertex (both picking a joint and changing an anchor).
    //
    // This used to test `anchor_mode` against {1, 2}, that is, a face did not count as needing geometry. But
    // a SLIDER's default anchor is exactly a face - and its travel axis is taken from the face's PRINCIPAL
    // DIRECTION, which simply does not exist without a live B-rep. The document's first joint was placed
    // while `regen_faces` was still empty: the axis came from the world axes and the part moved off in the
    // wrong direction. The geometry was raised afterwards (a joint already placed demands it), but the part
    // stayed where it had come to rest - a minimal displacement does not move it for nothing. One wrong
    // second, and the assembly is crooked for good.
    if joint.pick_faces || joint.edit_repick.is_some() {
        return true;
    }
    // JOINTS ALREADY PLACED ON EDGES AND VERTICES COUNT TOO.
    //
    // Reported behaviour: the joints do not move and a slider's direction is simply wrong however it is
    // picked. Measured on that document: edges were gathered for 2 bodies out of 138, and all five joints on
    // edges (all three sliders) had no connector frame at all - their bodies are imported, and an import is
    // not raised without being asked. A connector's axis is read from `regen_edges`; no edges means no axis,
    // and the solver then moves the part not along the picked edge but along whatever it substitutes for the
    // emptiness.
    //
    // THE PRICE OF THIS DECISION IS STATED PLAINLY: an assembly whose joints sit on edges raises the live
    // B-rep on opening - the "Preparing B-rep" step that lazy building was meant to avoid. There is no
    // cheaper way: a joint without an axis is not "slightly slower", it is a wrong assembly.
    //
    // FACES TOO. An anchor on a face without live geometry gives a frame with no known roll: a rigid joint
    // then leaves the part with a degree of freedom, and a slider on a flat face does not know the principal
    // direction and travels anywhere. A check by matrix showed exactly that.
    if project.connectors.iter().any(|c| {
        matches!(c.anchor, qymcad_core::feature::AnchorRef::EdgeMid(..) | qymcad_core::feature::AnchorRef::Vertex(..) | qymcad_core::feature::AnchorRef::FaceCenter(..))
            && project.joints.iter().any(|j| j.a == c.id || j.b == c.id)
    }) {
        return true;
    }
    // an open sketch ON A FACE draws that face's outline
    sketch_ses.editing.and_then(|si| project.sketches.iter().find(|s| s.id == si)).is_some_and(|s| matches!(s.plane, qymcad_core::feature::SketchPlane::Face(..)))
}

pub fn finish_drawing(ed: Editing, pending_import: &mut PendingImport, sketch_ses: SketchSession, closed: bool) {
    if let Some(pts) = pending_import.draw_pts.take() {
        let min = if closed { 3 } else { 2 };
        if pts.len() >= min {
            // while editing, fill the active sketch; otherwise start a new one
            if let Some(si) = edit_si(ed.project, &sketch_ses) {
                if ed.project.fill_sketch_polyline(si, pts, closed) {
                    *ed.sel = Sel::Sketch(si);
                } else {
                    *ed.status = qymcad_i18n::tr("io-sketch-has-profile");
                }
            } else {
                ed.project.add_line_sketch(qymcad_i18n::tr("io-sketch"), pts, closed);
                *ed.sel = Sel::Sketch(ed.project.sketches.len() - 1);
            }
            invalidate(ed.regen);
            ed.view.initialized = false;
        }
    }
}

/// FINISHING A FEATURE COMMAND (applied or cancelled) — both the aiming and the subject are cleared.
///
/// The difference from `clear_feat_picks`: the command's subject (the sketch, the profiles, the
/// parameters, the selected edges and faces) must NOT be cleared at the START — it is gathered BEFORE
/// the button is pressed, as a pre-selection. But once the command is over it has to be forgotten, or
/// the next one would pick up someone else's selection.
pub fn end_feat_cmd_state(picks: FeatPicks, cmd: &mut FeatCommand, gsel: &mut GeomSelection) {
    clear_feat_picks(picks);
    cmd.sketch = None;
    cmd.params.clear();
    gsel.profiles.clear();
    gsel.edges.clear();
    gsel.faces.clear();
    gsel.faces_body = None; // the scope of the multiple face selection
                            // UNFINISHED MENU GESTURES end together with the command too: waiting for the second face of a
                            // "between", the memory of the face and edge last pointed at. A wait that outlived its command
                            // would fire in the next one.
    gsel.between_first = None;
    gsel.last_face = None;
    gsel.last_edge = None;
    gsel.described = None;
}

/// THE KEY HINT, ALLOWING FOR FOCUS: "U" while the keyboard is free, and "Alt+U" while the caret sits
/// in an input field. Written the way this system writes keys (`⌥U` on a Mac).
///
/// The rule "hold Alt when focused" would be a secret without this hint, and secret mechanisms go
/// unused: `U` gets pressed once, nothing happens, and it is never tried again.
pub fn hotkey_hint(dc: &DrawCtx, ctx: &egui::Context, action: &str) -> String {
    let k = hotkey_key(dc.set, action);
    if k.is_empty() {
        return String::new();
    }
    // a Ctrl chord types nothing, so it works from a field as it is; only the rest need Alt there
    if ctx.egui_wants_keyboard_input() && !k.starts_with("Ctrl+") {
        key_label(&format!("Alt+{k}"))
    } else {
        key_label(&k)
    }
}

#[cfg(test)]
mod state_is_free_of_the_god_object {
    /// The working part of a file: everything before the first `#[cfg(test)]` module. Copied here rather
    /// than borrowed from the dictionary crate - a record of the interface must not depend on the words
    /// shown to a person.
    fn working_part(text: &str) -> &str {
        text.split("#[cfg(test)]").next().unwrap_or(text)
    }

    /// THE STATE OF THE INTERFACE KNOWS NOTHING ABOUT THE APPLICATION THAT HOLDS IT.
    ///
    /// This is the whole reason the file exists, and the last thing standing between a workbench and a
    /// crate of its own. A record that names `App` - in a signature, in a body, anywhere but a comment -
    /// drags the god object behind it, and the crate would have to depend on the crate declaring it.
    ///
    /// COMMENTS ARE STRIPPED FIRST. The header of this very file names `App` four times while explaining
    /// why it must not; a check that read the raw text would forbid the explanation along with the
    /// dependency.
    #[test]
    pub(crate) fn the_state_names_no_application() {
        // THE WORKING PART ONLY. The check below spells the forbidden word out to look for it, so a scan
        // of the whole file finds its own needle and goes red on itself for ever.
        let src = working_part(include_str!("lib.rs"));
        let mut sins: Vec<String> = Vec::new();
        for (i, line) in src.lines().enumerate() {
            let code = line.split("//").next().unwrap_or("");
            if code.split(|c: char| !c.is_alphanumeric() && c != '_').any(|w| w == "App") {
                sins.push(format!("lib.rs:{}: {}", i + 1, line.trim()));
            }
        }
        assert!(sins.is_empty(), "the state of the interface names the application ({}):\n{}", sins.len(), sins.join("\n"));
    }
}

#[cfg(test)]
mod picking {
    /// A SHEET COPIED FROM A FACE, AT THE SAME DEPTH, IS WHAT A SURFACE TOOL GETS - and only a surface tool.
    #[test]
    fn a_sheet_on_its_face_wins_only_for_the_tools_that_want_a_surface() {
        // the solid met first, the sheet at its depth after it
        assert!(super::nearer_hit(10.0, true, Some((10.0, false)), true), "a surface tool must take the sheet lying on the face");
        assert!(!super::nearer_hit(10.0, true, Some((10.0, false)), false), "a tool for bodies must keep the face of the part");
        // the sheet met first, the solid after it
        assert!(!super::nearer_hit(10.0, false, Some((10.0, true)), true), "a surface tool must not give the sheet up for the face under it");
        // a nearer body is nearer, whatever it is
        assert!(super::nearer_hit(9.0, false, Some((10.0, true)), true), "what stands in front must be taken");
        assert!(!super::nearer_hit(11.0, true, Some((10.0, false)), true), "a sheet behind the face must not be reached through it");
    }
}

#[cfg(test)]
mod tool_buttons {
    use super::{take_or_drop, BarAsk};

    /// A TOOL BUTTON TAKES ITS TOOL WHEN THE HAND IS FREE OF IT AND PUTS IT DOWN WHEN IT IS HELD: a sketch tool back to
    /// the arrow, a command cancelled.
    #[test]
    fn a_tool_button_takes_or_puts_down() {
        assert!(matches!(take_or_drop(false, BarAsk::FeatCmd(4)), BarAsk::FeatCmd(4)), "a free hand takes the command");
        assert!(matches!(take_or_drop(true, BarAsk::FeatCmd(4)), BarAsk::CancelAllTools(crate::Then::Nothing)), "a held command is put down");
        assert!(matches!(take_or_drop(true, BarAsk::SketchTool(3)), BarAsk::SketchSelectMode), "a held sketch tool gives the arrow back");
        assert!(matches!(take_or_drop(false, BarAsk::SketchTool(3)), BarAsk::SketchTool(3)), "a free hand takes the sketch tool");
    }
}

#[cfg(test)]
mod blend_outline {
    use super::{edge_blend_outline, Blend};
    use qymcad_core::geom::{Mesh, Point3};

    /// The top front edge of a 40 x 30 x 10 block: one triangle of the top face, one of the front face, sharing it.
    fn corner() -> Mesh {
        let p = |x, y, z| Point3::new(x, y, z);
        Mesh { verts: vec![p(0.0, 0.0, 10.0), p(40.0, 0.0, 10.0), p(40.0, 30.0, 10.0), p(0.0, 0.0, 0.0)], tris: vec![[0, 1, 2], [1, 0, 3]] }
    }

    fn near(a: [f64; 3], b: [f64; 3]) -> bool {
        (0..3).all(|i| (a[i] - b[i]).abs() < 1e-9)
    }

    /// A FILLET OF RADIUS r ON A SQUARE EDGE meets the top face r back from the edge and the front face r down, and
    /// the lines move with the radius: the preview follows the value before Enter.
    #[test]
    fn a_fillet_outline_follows_its_radius() {
        let edge = [[0.0f32, 0.0, 10.0], [40.0, 0.0, 10.0]];
        for r in [2.0, 4.0] {
            let out = edge_blend_outline(&corner(), &edge, Blend::Round(r));
            assert_eq!(out.len(), 4, "two lines along the faces and two end sections");
            assert!(near(out[0][0], [0.0, r, 10.0]) && near(out[0][1], [40.0, r, 10.0]), "on the top face, r = {r} back: {:?}", out[0]);
            assert!(near(out[1][0], [0.0, 0.0, 10.0 - r]) && near(out[1][1], [40.0, 0.0, 10.0 - r]), "on the front face, r = {r} down: {:?}", out[1]);
            let mid = out[2][4];
            let d = ((mid[1] - r).powi(2) + (mid[2] - (10.0 - r)).powi(2)).sqrt();
            assert!((d - r).abs() < 1e-9, "the section is an arc of radius {r} about the centre: {mid:?}");
        }
    }

    /// THE EDGE IS CUT FINER THAN THE MESH: a block's edge comes in 24 pieces against one side of a triangle, and every
    /// piece still finds its two faces.
    #[test]
    fn an_edge_cut_finer_than_the_mesh_finds_its_faces() {
        let out = edge_blend_outline(&corner(), &[[0.0, 0.0, 10.0], [20.0, 0.0, 10.0], [40.0, 0.0, 10.0]], Blend::Round(2.0));
        assert_eq!(out.first().map(|l| l.len()), Some(3), "a point of the line for every vertex of the edge: {out:?}");
        assert!(near(out[0][1], [20.0, 2.0, 10.0]), "{:?}", out[0]);
    }

    /// A HOLE'S RIMS ARE AS WIDE AS ITS DIAMETER and its bottom as deep as its depth, down from the face.
    #[test]
    fn a_hole_outline_follows_its_diameter_and_depth() {
        let pl = qymcad_core::feature::PlaneFrame::from_origin_normal([20.0, 15.0, 10.0], [0.0, 0.0, 1.0], 0.0).matrix12();
        for (d, depth) in [(6.0, 15.0), (12.0, 5.0)] {
            let out = super::hole_outline(&pl, qymcad_core::model::HoleTool { kind: 0, diameter: d, depth, dia2: 0.0, depth2: 0.0 });
            let p = out[0][0];
            assert!((((p[0] - 20.0).powi(2) + (p[1] - 15.0).powi(2)).sqrt() - d / 2.0).abs() < 1e-9 && (p[2] - 10.0).abs() < 1e-9, "the rim at the face, {d} across: {p:?}");
            assert!((out[1][0][2] - (10.0 - depth)).abs() < 1e-9, "the bottom {depth} down: {:?}", out[1][0]);
        }
    }

    /// A DRAFTED SIDE LEANS OUT BELOW THE NEUTRAL FACE by its depth times tan(angle), and its top edge, on the neutral
    /// face, stays where it is.
    #[test]
    fn a_draft_outline_leans_by_its_angle() {
        let p = |x, y, z| Point3::new(x, y, z);
        let mesh = Mesh { verts: vec![p(0.0, 0.0, 0.0), p(40.0, 0.0, 0.0), p(40.0, 0.0, 10.0), p(0.0, 0.0, 10.0)], tris: vec![[0, 1, 2], [0, 2, 3]] };
        let face = qymcad_core::geom::MeshFace { triangles: vec![0, 1], normal: [0.0, -1.0, 0.0], centroid: p(20.0, 0.0, 5.0), area: 400.0, id: 1 };
        for angle in [3.0f64, 6.0] {
            let out = super::draft_outline(&mesh, &face, [20.0, 15.0, 10.0], [0.0, 0.0, 1.0], angle);
            assert_eq!(out.len(), 4, "the four sides of the face: {out:?}");
            let lean = 10.0 * angle.to_radians().tan();
            let pts: Vec<[f64; 3]> = out.iter().flatten().copied().collect();
            assert!(pts.iter().any(|q| near(*q, [0.0, -lean, 0.0])), "the bottom leans out {lean} at {angle} deg: {pts:?}");
            assert!(pts.iter().any(|q| near(*q, [0.0, 0.0, 10.0])), "the edge on the neutral face stays: {pts:?}");
        }
    }

    /// THE SECOND CLICK OF A POLYGON LIES ON IT in every mode: a vertex (inscribed), the middle of an edge
    /// (circumscribed), the end of the edge begun at the first click (by edge).
    #[test]
    fn a_polygon_passes_through_its_second_click() {
        use qymcad_core::geom::Point2;
        let (a, b) = (Point2::new(0.0, 0.0), Point2::new(15.0, 0.0));
        let vertices = |(c, v): (Point2, Point2)| -> Vec<[f64; 2]> {
            let (r, a0) = (((v.x - c.x).powi(2) + (v.y - c.y).powi(2)).sqrt(), (v.y - c.y).atan2(v.x - c.x));
            (0..6)
                .map(|k| {
                    let t = a0 + std::f64::consts::TAU * k as f64 / 6.0;
                    [c.x + r * t.cos(), c.y + r * t.sin()]
                })
                .collect()
        };
        let on = |p: [f64; 2], q: [f64; 2]| (p[0] - q[0]).abs() < 1e-9 && (p[1] - q[1]).abs() < 1e-9;
        assert!(vertices(super::polygon_from_clicks(a, b, 6, 0)).iter().any(|v| on(*v, [15.0, 0.0])), "inscribed: a vertex at the click");
        let vs = vertices(super::polygon_from_clicks(a, b, 6, 1));
        assert!(
            (0..6).any(|k| {
                let (p, q) = (vs[k], vs[(k + 1) % 6]);
                on([(p[0] + q[0]) / 2.0, (p[1] + q[1]) / 2.0], [15.0, 0.0])
            }),
            "circumscribed: the middle of an edge at the click: {vs:?}"
        );
        let vs = vertices(super::polygon_from_clicks(a, b, 6, 2));
        assert!(vs.iter().any(|v| on(*v, [0.0, 0.0])) && vs.iter().any(|v| on(*v, [15.0, 0.0])), "by edge: both clicks are vertices: {vs:?}");
    }

    /// A FILLET IN AN INNER CORNER lies in the corner, not beyond it. Reported behaviour: with the tool open on a frame
    /// with bosses in its corners, the arcs of the preview lay on the far side of the edge, inside the part, while the
    /// fillet itself built outwards. The floor z = 0 runs on in +y and the wall y = 0 rises in +z from their common
    /// edge; R2 meets the floor 2 out along it and the wall 2 up it, about the centre (y 2, z 2) in the air.
    #[test]
    fn a_fillet_in_an_inner_corner_lies_in_the_corner() {
        let p = |x, y, z| Point3::new(x, y, z);
        let m = Mesh { verts: vec![p(0.0, 0.0, 0.0), p(40.0, 0.0, 0.0), p(40.0, 30.0, 0.0), p(0.0, 0.0, 10.0)], tris: vec![[0, 1, 2], [1, 0, 3]] };
        let edge = [[0.0f32, 0.0, 0.0], [40.0, 0.0, 0.0]];
        let out = edge_blend_outline(&m, &edge, Blend::Round(2.0));
        assert_eq!(out.len(), 4, "{out:?}");
        let lines: Vec<[f64; 3]> = vec![out[0][0], out[1][0]];
        assert!(lines.iter().any(|q| near(*q, [0.0, 2.0, 0.0])) && lines.iter().any(|q| near(*q, [0.0, 0.0, 2.0])), "on the floor 2 out and on the wall 2 up: {lines:?}");
        let mid = out[2][4];
        let k = 2.0 - 2.0 / 2f64.sqrt();
        assert!(near(mid, [0.0, k, k]), "the arc bows into the corner, about the centre in the air: {mid:?}");
        let cut = edge_blend_outline(&m, &edge, Blend::Cut(1.5, 1.5, None));
        let legs: Vec<[f64; 3]> = vec![cut[0][0], cut[1][0]];
        assert!(legs.iter().any(|q| near(*q, [0.0, 1.5, 0.0])) && legs.iter().any(|q| near(*q, [0.0, 0.0, 1.5])), "a chamfer's legs on the floor and the wall: {legs:?}");
    }

    /// A CHAMFER cuts straight across from leg to leg.
    #[test]
    fn a_chamfer_outline_is_cut_straight_at_its_legs() {
        let out = edge_blend_outline(&corner(), &[[0.0, 0.0, 10.0], [40.0, 0.0, 10.0]], Blend::Cut(1.5, 3.0, None));
        assert!(near(out[0][0], [0.0, 1.5, 10.0]) && near(out[1][0], [0.0, 0.0, 7.0]), "{out:?}");
        assert_eq!(out[2], vec![out[0][0], out[1][0]], "the end section is the straight cut");
    }

    /// A CHAMFER BY TWO LEGS lays its first leg on the reference face, as the kernel does: named the upright face, the
    /// 1.5 runs down it and the 3 across the top. Reported behaviour: the preview drew the first leg on both faces.
    #[test]
    fn a_chamfer_by_two_legs_lays_the_first_on_the_reference_face() {
        let out = edge_blend_outline(&corner(), &[[0.0, 0.0, 10.0], [40.0, 0.0, 10.0]], Blend::Cut(1.5, 3.0, Some([0.0, 1.0, 0.0])));
        let legs = [out[0][0], out[1][0]];
        assert!(legs.iter().any(|q| near(*q, [0.0, 0.0, 8.5])) && legs.iter().any(|q| near(*q, [0.0, 3.0, 10.0])), "the legs of a chamfer 1.5 on the upright face and 3 on the top lie at {legs:?}");
    }

    /// An edge with one face found has nothing to be set back from, and draws nothing rather than a guess.
    #[test]
    fn an_edge_with_one_face_draws_nothing() {
        let mut m = corner();
        m.tris.pop();
        assert!(edge_blend_outline(&m, &[[0.0, 0.0, 10.0], [40.0, 0.0, 10.0]], Blend::Round(2.0)).is_empty());
    }
}
