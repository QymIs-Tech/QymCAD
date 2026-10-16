//! The records of the window itself: the 2D view, the workbench, the caches, the windows, the edits, the rebuild, the scheme.

use crate::Notice;
use super::*;

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
    /// The pass of egui (`cumulative_pass_nr`) the scene was handed to the card in. A pass that is discarded takes
    /// its paint callback with it, so a later pass of the same frame hands the scene over again.
    pub gpu_scene_pass: std::cell::Cell<u64>,
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
    /// THE FLOOR OF THE SOFTWARE RASTER, pictures of its own keyed by the camera, the frame and the colours
    /// (`qymcad_render::FloorFrame::key`), the latest first. The floor does not change with the model: while a person
    /// edits with the view standing still, every frame of a preview is drawn again and the floor is not. Worked out anew
    /// each frame, it made the frames of an extrusion three times slower under a loaded processor. Several are kept
    /// because the canvas changes its size as a command's panel or a window opens and closes - four sizes in one
    /// extrusion by hand - and each size is a picture of its own.
    pub floor: std::cell::RefCell<Vec<Cached<Vec<egui::Color32>>>>,
    /// THE FAULTS OF THE DOCUMENT and the key they were gathered for (`document_faults`): the status line asks for
    /// them every frame, and gathering walks every node, mate, sketch and parameter.
    pub faults: std::cell::RefCell<Option<crate::FaultMemo>>,
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
            gpu_scene_pass: std::cell::Cell::new(0),
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
            floor: std::cell::RefCell::new(Vec::new()),
            faults: std::cell::RefCell::new(None),
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
    /// The icon theme manager window.
    IconManager,
    /// The developer icon theme packager modal.
    IconPackager,
    /// The errors and warnings of the document, each with the way to it - opened from the status line.
    Faults,
}

impl WinKind {
    /// EVERY KIND, so that a walk over the windows cannot silently miss one added later. A guard checks that the
    /// count here matches the number of variants declared above.
    pub const ALL: [WinKind; 14] = [
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
        WinKind::IconManager,
        WinKind::IconPackager,
        WinKind::Faults,
    ];
}

#[derive(Default)]
pub struct Windows {
    /// WHICH WINDOWS ARE OPEN. Private on purpose: the only ways in are `is`, `set`, `open`, `close`, `toggle` and
    /// `close_all`, so no caller can invent an eleventh way of asking the same question.
    pub(crate) open: std::collections::BTreeSet<WinKind>,
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
    /// Persistent state of the icon theme manager window.
    pub icon_manager: crate::icons::IconManagerState,
    /// Persistent state of the developer icon theme packager modal.
    pub icon_packager: crate::icons::PackagerDialogState,
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

/// WHAT THE OPEN EDIT IS. Work in hand - a sketch, a command, a drag - takes Ctrl+Z as a step of the history under it. A
/// question to the person - an import waiting for its scale - is one step not yet taken: Ctrl+Z takes it back as its
/// Cancel does, and the history under it is not touched.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum OpenEdit {
    #[default]
    Working,
    /// The open edit waits for an answer.
    Asking,
    /// Ctrl+Z came while it waited: its window cancels it on its next frame.
    AskingDropped,
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
    /// What the open edit is: work in hand or a question waiting for its answer.
    pub open_kind: OpenEdit,
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
        self.open_kind = OpenEdit::Working;
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
            open_kind: OpenEdit::Working,
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
    pub line: crate::Notice,
    /// What stood on the status line before a quiet rebuild wrote its progress over it - put back when it ends well.
    pub over: crate::Notice,
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
    pub note: crate::Notice,
}

impl Default for SchemeUi {
    fn default() -> Self {
        Self {
            pal: qymcad_scheme::dark(),
            all: qymcad_scheme::builtin(),
            edit: SchemeEdit::default(),
            section: settings_sections::SettingsSection::General,
            search: String::new(),
            note: Notice::default(),
        }
    }
}
