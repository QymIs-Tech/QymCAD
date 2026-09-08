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
}

impl Default for View2d {
    fn default() -> Self {
        Self { center: Vec2::ZERO, scale: 4.0, initialized: false }
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
    pub norm: std::cell::RefCell<Cached<Vec<Vec<[f64; 3]>>>>,
    /// The key of the scene uploaded into the GPU vertex buffer (re-uploaded only on a change). It does NOT depend on the camera.
    pub gpu_scene_key: std::cell::Cell<u64>,
    /// The cache of the visible scene's world bounding sphere (a centre and a radius) keyed by the scene - for
    /// tight near and far planes in perspective (the z buffer's precision). Recomputed only when the scene changes,
    /// not every frame.
    pub bounds: std::cell::Cell<(u64, Option<([f64; 3], f64)>)>,
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
    pub mesh_bounds: std::cell::RefCell<Cached<std::collections::HashMap<usize, ([f64; 3], [f64; 3])>>>,
    /// The sketch diagnostics cache. The key is a fingerprint of the geometry and the constraints. The rank
    /// analysis builds a FULL Jacobian and runs Gaussian elimination (O(m * nv^2)) - without a cache that was
    /// computed EVERY FRAME several times over (the panel, the list, the overlay, the glyphs, the tree), and on
    /// large sketches the interface hung. It is recomputed only on an edit.
    /// A `RefCell`, because drawing goes through `&self` and needs that very cache.
    pub sk_status: std::cell::RefCell<Option<(usize, u64, SketchDiag)>>,
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
            bounds: std::cell::Cell::new((u64::MAX, None)),
            consumed: std::cell::RefCell::new(Cached { rev: u64::MAX, value: std::collections::HashSet::new() }),
            shown_bodies: std::cell::RefCell::new(Cached { rev: u64::MAX, value: ShownBodies::default() }),
            bbox_world: std::cell::RefCell::new(Cached { rev: u64::MAX, value: std::collections::HashMap::new() }),
            pick_edges: std::cell::RefCell::new(Cached { rev: u64::MAX, value: std::collections::HashMap::new() }),
            sk_status: std::cell::RefCell::new(None),
            section_caps: std::cell::RefCell::new(Cached { rev: 0, value: std::rc::Rc::new(Vec::new()) }),
            mesh_bounds: std::cell::RefCell::new(Cached { rev: 0, value: std::collections::HashMap::new() }),
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
}

impl WinKind {
    /// EVERY KIND, so that a walk over the windows cannot silently miss one added later. A guard checks that the
    /// count here matches the number of variants declared above.
    pub const ALL: [WinKind; 10] = [
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
    /// THE REBUILD WAS STOPPED BY A PERSON - it no longer starts by itself.
    ///
    /// The dirty marks on the nodes remain after a cancellation (the document really has not been rebuilt), and the
    /// scheduler looks at exactly those. Without this mark the next frame would start precisely the work that was
    /// just stopped. It is cleared by ANY edit of the document and by an explicit "rebuild everything": someone
    /// changed their mind, so computing again is allowed.
    pub paused: bool,
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
#[derive(Clone, Copy, Default, PartialEq)]
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
#[derive(Clone, Copy, Default, PartialEq)]
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
            PlacingShape::Rect(a, b, ids) => Some((*a, *b, ids.clone())),
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
pub struct SketchPattern {
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
pub struct SketchTool {
    /// the points clicked with THE CURRENT tool (an arc has three, a line two, and so on)
    pub pts: Vec<Point2>,
    /// construction geometry is being drawn
    pub construction: bool,
    /// PROJECTION: take not one edge but THE WHOLE outline of the face the sketch stands on. By a switch of its
    /// own rather than a guess from the click: "clicked inside the face" is indistinguishable from a miss past an
    /// edge on outlines with complicated cut-outs.
    pub proj_face: bool,
    pub move_base: Option<Point2>,
    /// the tangent edge given for a circle (picked with THIS tool rather than globally)
    pub circ_tan: Option<EdgeRef>,
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

/// TYPING A CORNER FILLET'S RADIUS IN A SKETCH: the corner is chosen and the radius is still being typed on the
/// canvas. Five fields described ONE unfinished input, and "where we are typing" could outlive "what we are typing".
#[derive(Clone, Default, PartialEq)]
pub struct CornerInput {
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
    /// an existing boolean node is being edited (its index in the timeline)
    pub edit: Option<usize>,
}

/// THE MIRROR'S PARAMETERS: a given plane OR a given part - the mirroring goes either by a plane or over a whole
/// part, and as separate fields both could end up given at once.
#[derive(Clone, Default)]
pub struct MirrorParams {
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
        self.buttons.iter().any(|b| resp.dragged_by(*b)) && self.buttons.iter().all(|b| resp.dragged_by(*b) || ctx.input(|i| i.pointer.button_down(*b)))
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
}

impl MouseNav {
    /// EVERY LAYOUT. The one list the settings window, the checks and the catalogue walk over.
    pub const ALL: [MouseNav; 11] = [
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
        }
    }

    /// TURNING THE MODEL.
    pub fn rotate(self) -> Gesture {
        match self {
            MouseNav::QymCad => Gesture::any(),
            MouseNav::Cad => Gesture::of(&[MIDDLE, LEFT]),
            MouseNav::Blender => Gesture::of(&[MIDDLE]),
            MouseNav::Gesture | MouseNav::OpenScad | MouseNav::OpenInventor => Gesture::of(&[LEFT]),
            MouseNav::MayaGesture => Gesture::of(&[LEFT]).alt(),
            MouseNav::OpenCascade => Gesture::of(&[MIDDLE, RIGHT]),
            MouseNav::Revit => Gesture::of(&[MIDDLE]).shift(),
            MouseNav::TinkerCad => Gesture::of(&[RIGHT]),
            MouseNav::Touchpad => Gesture::of(&[]).alt(),
        }
    }

    /// MOVING THE VIEW SIDEWAYS.
    pub fn pan(self) -> Gesture {
        match self {
            MouseNav::QymCad => Gesture::any().shift(),
            MouseNav::Cad | MouseNav::OpenCascade | MouseNav::OpenInventor | MouseNav::Revit | MouseNav::TinkerCad => Gesture::of(&[MIDDLE]),
            MouseNav::Blender => Gesture::of(&[MIDDLE]).shift(),
            MouseNav::Gesture | MouseNav::OpenScad => Gesture::of(&[RIGHT]),
            MouseNav::MayaGesture => Gesture::of(&[MIDDLE]).alt(),
            MouseNav::Touchpad => Gesture::of(&[]).shift(),
        }
    }

    /// ZOOMING. Every layout but one puts it on the wheel; the touchpad has no wheel to put it on.
    pub fn zoom_gesture(self) -> Gesture {
        match self {
            MouseNav::Touchpad => Gesture::of(&[]).ctrl().shift(),
            _ => Gesture::NONE,
        }
    }

    /// Whether the wheel zooms under this layout.
    pub fn wheel_zooms(self) -> bool {
        self != MouseNav::Touchpad
    }

    /// SELECTING NEEDS SHIFT under one layout, because there the bare left button turns the model.
    pub fn select_needs_shift(self) -> bool {
        self == MouseNav::OpenInventor
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
    /// Off by default, and that is a change: the program used to reopen it always, with nothing to say
    /// otherwise. Reopening is right for somebody who works on one thing for weeks and wrong for somebody
    /// who opens the CAD to try something - and neither can be guessed from here.
    #[serde(default)]
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
}

/// The factory layout: ours.
fn default_zoom_at() -> ZoomAt {
    ZoomAt::Cursor // what every CAD a person comes from does
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
            defaults: Defaults::default(),
            ui_scale: default_ui_scale(),
            recent: Vec::new(),
            recent_limit: default_recent_limit(),
            pick_precision: default_pick_precision(),
            open_last: false,
            show_start_screen: true,
            mouse_nav: default_mouse_nav(),
            zoom_at: default_zoom_at(),
            zoom_editing: default_zoom_editing(),
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
    pub modify: Option<u8>,
    /// WHAT THE MIRROR IS ABOUT TO REFLECT, while the axis is being pointed at.
    ///
    /// Non-empty means the tool is on its SECOND step: it has been told what, and waits to be told about
    /// what. It used to have no second step at all - the axis was whichever line happened to be in the
    /// selection (and got mirrored too, being in the same set), or silently Y.
    pub mirror_of: Vec<Id>,
}

impl SketchSelection {
    /// Clear the selection, together with whatever was waiting for it. A deferred action without a selection means nothing.
    pub fn clear(&mut self) {
        *self = Self::default();
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
#[derive(Clone, Default, PartialEq)]
pub struct DimTool {
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

/// THE HOLE COMMAND: the kind, how it is placed, and the sketch holding the points.
#[derive(Clone, Copy, Default)]
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
#[derive(Clone, Copy, Default)]
pub struct DraftParams {
    pub neutral: u32,
    pub pick_neutral: bool,
    pub flip: bool,
}

/// PRIMITIVE PARAMETERS: how many sides (for a prism) and where it goes.
#[derive(Clone, Copy, Default)]
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
    /// typed input: (body, axis, is it a rotation?) plus its buffer and focus
    pub num: Option<(usize, u8, bool)>,
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
        match *self { InlineEdit::Note(i) => Some(i), _ => None }
    }
    pub fn text(&self) -> Option<usize> {
        match *self { InlineEdit::Text(i) => Some(i), _ => None }
    }
    pub fn dim(&self) -> Option<usize> {
        match *self { InlineEdit::Dim(i) => Some(i), _ => None }
    }
    pub fn circle(&self) -> Option<Id> {
        match *self { InlineEdit::Circle(i) => Some(i), _ => None }
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
        match *self { Picking::SketchPlane(f) => f, _ => None }
    }
    pub fn set_plane_face(&mut self, f: Option<Id>) {
        if self.is_sketch_plane() {
            *self = Picking::SketchPlane(f);
        }
    }
    pub fn replace_sketch(&self) -> Option<usize> {
        match *self { Picking::ReplaceSketch(i) => Some(i), _ => None }
    }
    pub fn fillet_all(&self) -> bool {
        *self == Picking::FilletAll
    }
    pub fn contour(&self) -> Option<ContourSlot> {
        match *self { Picking::Contour(s) => Some(s), _ => None }
    }
    /// Which command is waiting for a sketch, if any.
    pub fn sketch_for(&self) -> Option<u8> {
        match *self { Picking::SketchFor(k) => Some(k), _ => None }
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
            Picking::None | Picking::FilletAll | Picking::Contour(_) => None,
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
}

/// THE CLIPBOARD: sketch geometry and tree nodes are different things, but they share one state of "what has been copied".
#[derive(Clone, Default)]
pub struct Clipboard {
    pub geom: Option<qymcad_core::model::GeomClip>,
    pub geom_pending: Option<(Vec<Id>, bool)>,
    pub geom_place: bool,
    pub tree: Option<TreeClip>,
    pub tree_multi: Option<(Vec<Id>, bool)>,
    pub os_ping: bool,
}

/// The orbit camera for the software 3D view (the projection comes from `Settings::projection`).
#[derive(Clone, Copy)]
pub struct Cam3 {
    pub yaw: f64,
    pub pitch: f64,
    pub scale: f32,
    pub target: [f64; 3],
    pub init: bool,
}

impl Default for Cam3 {
    fn default() -> Self {
        // Front isometric: the camera sits at +X -Y +Z (front = -Y towards the viewer), the usual choice.
        // +Y runs up and away, so the "top" of a sketch on the XY plane matches the "top" in 3D.
        Self { yaw: -0.7, pitch: 0.6, scale: 4.0, target: [0.0; 3], init: false }
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
        (right, up, fwd)
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
        Self { label: LabelKey(label), key: key.to_string(), val, txt: format!("{val:.2}"), lo, hi, at: None }
    }

    /// A field AT THE GEOMETRY: the same thing, but with a place of its own in space.
    pub fn at(mut self, p: [f64; 3]) -> Self {
        self.at = Some(p);
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
    pub opaque: Vec<GpuVert>,
    pub transp: Vec<GpuVert>,
}

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
    Rect(Point2, Point2, Vec<Id>),
    /// a polygon by its inscribed or circumscribed circle
    Poly(Id),
    /// an ellipse: the entity and its centre
    Ellipse(Id, Point2),
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
#[derive(Clone, Copy)]
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

/// The result of a background (worker thread) import or export, arriving at the interface over a channel.
pub enum JobResult {
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
pub struct RegenPulse {
    /// The node being computed right now, and the total number of nodes.
    pub done: std::sync::atomic::AtomicUsize,
    pub total: std::sync::atomic::AtomicUsize,
    pub stop: std::sync::atomic::AtomicBool,
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

/// A vertex of a body for the GPU. The colour and the normal do NOT depend on the camera (the light is
/// of the world), so the buffer is re-uploaded only when the scene changes and not when it is rotated.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuVert {
    /// The position in world coordinates (with the transform of the owning component already applied).
    pub pos: [f32; 3],
    /// The normal of the face (the same for all 3 vertices of a triangle) — for culling back faces in
    /// the fragment.
    pub nrm: [f32; 3],
    /// The shaded colour as rgba8 (sRGB bytes, as in `Color32`): the low byte is r, the high one is a.
    pub color: u32,
    pub _pad: u32,
}

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
    NewFromTemplate(String),
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
        Self {
            splash_until: Some(std::time::Instant::now() + SPLASH_MIN),
            save_since: None,
            save_shown: None,
        }
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
        Self {
            pairs: Vec::new(),
            rev: u64::MAX,
        }
    }
}

/// THE INTERFACE'S DEFERRED INTENTIONS: an action is decided within a frame but carried out at a safe point, not
/// in the middle of drawing. Keeping them as separate fields would mean that "delete" and "navigate" could drift
/// apart from whatever produced them.
#[derive(Clone, Default)]
pub struct DeferredUi {
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
    pub font_cache: Option<Vec<u8>>,
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
            font_cache: None,
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
    pub font_cache: &'a mut Option<Vec<u8>>,
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
    pub stl_export: &'a mut Option<ExportTarget>,
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
#[derive(Clone, Copy)]
pub enum TreeAsk {
    /// Reopen the command of a feature (a double click on its row, or a datum row's pencil).
    EditFeature(Id),
    /// Enter the sketch for editing.
    EnterSketch(usize),
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
    /// Start a sketch on one of the base planes of the component.
    SketchOnBasePlane(qymcad_core::feature::BasePlane),
    /// Step inside the component.
    EnterComponent(Id),
    /// Reopen the pattern command of the component.
    EditCompArray(Id),
    /// Export a component to STEP.
    ExportStep(Id),
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
    pub stl_export: &'a mut Option<ExportTarget>,
    pub tool: &'a mut SketchTool,
    pub tool_prefs: &'a mut SketchToolPrefs,
    pub view: &'a mut View2d,
    pub win: &'a mut Windows,
    pub workbench: &'a mut Workbench,
    /// WHAT THE BAR ASKS THE APPLICATION TO DO. A button does not open a file, start a command or undo a
    /// step by itself - it names the request, and the frame carries it out after the drawing.
    pub ask: &'a mut Vec<BarAsk>,
}

/// The named things a bar can ask for.
#[derive(Clone)]
pub enum BarAsk {
    /// Leave the document: new, open, a template, exit.
    Nav(Nav),
    /// Save, or save under a new name.
    Save,
    SaveAs,
    /// Bring a file in: a drawing, a mesh, a solid, a vector, a font.
    PickDxf,
    PickStl,
    PickStep,
    PickSvg,
    PickFont,
    /// Write the whole document out as a solid.
    ExportStep(ExportTarget),
    /// The undo history.
    Undo,
    Redo,
    /// The clipboard.
    Clipboard { cut: bool },
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
    MirrorPart(Id),
    ToggleMeasure3d,
    /// Step into a component.
    EnterComponent(Id),
    /// Start an array of components.
    CompArray(u8),
    /// Put every tool down.
    CancelAllTools,
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
    pub regen: &'a mut Rebuilding,
    pub scheme: &'a mut SchemeUi,
    pub sel: &'a mut Sel,
    pub set: &'a mut Settings,
    pub status: &'a mut String,
    pub stl_export: &'a mut Option<ExportTarget>,
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
    GoTo { owner: Option<Id>, sel: Sel },
    /// Bring a part of the library into the document.
    InsertPart(PartSource),
    /// Write the settings out, or read them in.
    ExportSettings,
    ImportSettings,
    /// Write a mesh out with the chosen deflection.
    ExportStl(ExportTarget, f64),
    /// Carry out a deletion the confirmation asked about.
    Delete(Sel),
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
    /// Reopen the command of an existing feature (a double click on its row).
    EditFeature(Id),
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
    Point2::new(
        (view.center.x + (s.x - c.x) / view.scale) as f64,
        (view.center.y - (s.y - c.y) / view.scale) as f64,
    )
}

/// The index of the note under a screen point (a rough bounding box of the text).
pub fn note_at(project: &Project, view: &View2d, rect: Rect, pos: Pos2, si: usize) -> Option<usize> {
    let s = project.sketches.get(si)?;
    for (i, n) in s.notes.iter().enumerate() {
        let sp = (Sheet { view: *view, rect: rect }).at(Point2::new(n.x, n.y));
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
    let sh = Sheet { view: *view, rect: rect };
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
    let sh = Sheet { view: *view, rect: rect };
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
    sel_sk.items
        .iter()
        .filter(|(k, _)| *k == 1)
        .filter_map(|(_, eid)| s.entities.iter().find(|e| e.id == *eid).and_then(|e| match e.kind {
            EntityKind::Circle { center, .. } | EntityKind::Arc { center, .. } => Some(center),
            _ => None,
        }))
        .collect()
}

/// The text object under a screen point (by the bounding box of its glyphs). For selecting, moving and editing.
pub fn text_at(project: &Project, view: &View2d, rect: Rect, pos: Pos2, si: usize) -> Option<usize> {
    let sh = Sheet { view: *view, rect: rect };
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
    use qymcad_core::model::EntityKind;
    let s = dc.project.sketches.get(si)?;
    s.entities.iter().find_map(|e| match e.kind {
        EntityKind::Circle { center, r } if center == c => Some(r),
        EntityKind::Arc { center, a, .. } if center == c => sketch_pt(dc.project, si, a)
            .zip(sketch_pt(dc.project, si, center))
            .map(|(pa, pc)| ((pa.x - pc.x).powi(2) + (pa.y - pc.y).powi(2)).sqrt()),
        _ => None,
    })
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
    corner.clear();
    measure.clear(); // both the FLAG and the points collected: otherwise they outlived the exit
    tool.move_base = None;
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
    if let Some(b) = bv.project.bodies.get(mi) {
        if b.mesh.tris.is_empty() {
            return false;
        }
        // THE BODY OF A RED NODE IS A GHOST. Only that body ITSELF is hidden: the source must stay on
        // screen, otherwise a failed operation wipes out the whole part (its body having been consumed
        // by the node that failed to build). That is the rule in full: on failure we show what was
        // there BEFORE, not emptiness and not two bodies at once.
        let red = |id: Id| bv.project.timeline.iter().any(|n| n.kind.bodies().contains(&id) && bv.project.regen_errors.contains_key(&n.id));
        if red(b.id) {
            return false;
        }
        if body_is_consumed(bv.cache, bv.project, bv.regen, b.id) && bv.project.timeline.iter().any(|n| n.kind.consumed().contains(&b.id) && bv.project.regen_errors.contains_key(&n.id)) {
            return true; // the consuming node is red, so the source stays visible
        }
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
    if bv.project.mesh_id(mi).is_some_and(|b| body_is_consumed(bv.cache, bv.project, bv.regen, b) && Some(b) != edit_src_body(bv.cmd, bv.project)) {
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
        .find(|n| matches!(n.kind, FK::DatumPoint { point } if point == datum_id) || matches!(n.kind, FK::DatumAxis { axis } if axis == datum_id) || matches!(n.kind, FK::Plane { plane } if plane == datum_id))
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
        [
            vec[0] * c + cr[0] * s + axis[0] * d * (1.0 - c),
            vec[1] * c + cr[1] * s + axis[1] * d * (1.0 - c),
            vec[2] * c + cr[2] * s + axis[2] * d * (1.0 - c),
        ]
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
/// THE TALLEST BODY ON SCREEN, along Z. Says whether the shapes came back after a reload.
pub fn tallest_body(pn: &Painting) -> f64 {
    visible_mesh_items(pn).iter().filter_map(|m| m.mesh.bounds()).map(|bb| bb.max.z - bb.min.z).fold(0.0, f64::max)
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
        pn.project
            .bodies
            .iter()
            .map(|b| &b.mesh)
            .enumerate()
            .filter(|(mi, _)| body_shown(pn.body_view(), *mi))
            .filter(|(mi, _)| !pn.project.mesh_id(*mi).is_some_and(|b| edit_hide.contains(&b))) // the feature being edited plus its descendant chain
            .map(|(mi, m)| {
                let mut wt = pn.project.mesh_id(mi).map(|b| pn.project.body_display_transform(b, ctx)).unwrap_or(qymcad_core::feature::PLACE_IDENTITY);
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
                    ghost: body_is_ghost(&DrawCtx { cam: &pn.cam, set: pn.set, scheme: pn.scheme, project: pn.project, active_path: pn.active_path }, mi),
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
        Sel::None | Sel::Face(..) | Sel::Contour(_) | Sel::Joint(_) => false,
    }
}

/// WHERE A SEGMENT CROSSES ONE OF THE SKETCH AXES: 0 is the line x = 0, 1 is the line y = 0.
///
/// The axes are infinite, so this is not `seg_seg_intersect` against some long enough stand-in segment -
/// "long enough" is a number pulled out of the air, and at a far enough zoom it stops being long enough.
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
        Sel::None | Sel::Face(..) | Sel::Contour(_) | Sel::Joint(_) => false,
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

/// THE WHEEL OVER THE 3D VIEWPORT: zoom towards whatever the rule says to hold still.
///
/// `part` is where the open command's fields stand, when one is open - the workbench knows that and this
/// module does not, so it is handed in.
pub fn wheel_zoom_3d(cam: &mut Cam3, set: &Settings, rect: Rect, cursor: Option<Pos2>, part: Option<Pos2>, scroll: f32) {
    zoom_cam_3d(cam, rect, zoom_anchor(set, rect, cursor, part), (scroll * 0.002).exp(), 0.05, 400.0);
}

/// The same over the flat sheet of a sketch.
pub fn wheel_zoom_2d(view: &mut View2d, set: &Settings, rect: Rect, cursor: Option<Pos2>, scroll: f32) {
    zoom_view_2d(view, rect, zoom_anchor(set, rect, cursor, None), (scroll * 0.002).exp(), 0.02, 800.0);
}

/// PANNING THE SHEET WITH THE MIDDLE BUTTON. In a sketch the left button is busy - it draws and it grabs.
pub fn pan_sheet_2d(view: &mut View2d, ctx: &egui::Context) {
    if !ctx.input(|i| i.pointer.middle_down()) {
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
}

/// A VALUE KEPT UNTIL THE THING IT WAS COMPUTED FROM CHANGES.
///
/// Nine cache fields were `(u64, T)`, and every one of them carried a doc comment explaining that the first
/// place is the revision - `(the revision, the bodies)`, `(the geometry revision, a body mapped to ...)`.
/// A comment saying what a type should say is a comment nothing checks. Two named fields say it once.
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
    pub regen: &'a mut Rebuilding,
    pub sel: &'a mut Sel,
    pub status: &'a mut String,
    pub view: &'a mut View2d,
}

impl Editing<'_> {
    /// A shorter borrow of the same five, so one gesture can hand them on to another.
    pub fn reborrow(&mut self) -> Editing<'_> {
        Editing { project: self.project, regen: self.regen, sel: self.sel, status: self.status, view: self.view }
    }
}

/// The five, taken from the application, where they live in four different records.
#[macro_export]
macro_rules! editing_of {
    ($x:expr) => {
        $crate::Editing {
            project: &mut $x.project,
            regen: &mut $x.regen,
            sel: &mut $x.chosen.sel,
            status: &mut $x.status,
            view: &mut $x.viewing.view,
        }
    };
}

/// The same, from a CONTEXT, where the fields are already borrows and are reborrowed.
#[macro_export]
macro_rules! editing_in {
    ($x:expr) => {
        $crate::Editing {
            project: &mut *$x.project,
            regen: &mut *$x.regen,
            sel: &mut *$x.sel,
            status: &mut *$x.status,
            view: &mut *$x.view,
        }
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
    Modify(u8),
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
        if let Armed::Draw(k) = self { *k } else { 0 }
    }

    /// The code of the click-acting tool, or zero.
    pub fn click_op(&self) -> u8 {
        if let Armed::ClickOp(k) = self { *k } else { 0 }
    }

    /// The code of the editing button in hand, or zero.
    pub fn modify(&self) -> u8 {
        if let Armed::Modify(k) = self { *k } else { 0 }
    }

    /// The code of the move tool, or zero.
    pub fn move_op(&self) -> u8 {
        if let Armed::Move(k) = self { *k } else { 0 }
    }

    /// The code of the sketch pattern, or zero.
    pub fn pat_op(&self) -> u8 {
        if let Armed::Pattern(k) = self { *k } else { 0 }
    }

    /// The kind of dimension being placed, or zero.
    pub fn dim_kind(&self) -> u8 {
        if let Armed::Dimension(k) = self { *k } else { 0 }
    }

    /// The kind of feature command being run, or zero.
    pub fn cmd_kind(&self) -> u8 {
        if let Armed::Command(k) = self { *k } else { 0 }
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
pub struct FeatParams {
    pub stitch_parts: Vec<Id>,
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
        if let Some(ViewTurn { to, .. }) = self.view_anim.take() {
            self.cam.yaw = to.0;
            self.cam.pitch = to.1;
        }
    }
}

impl Default for Viewing {
    /// `Rect::NOTHING` rather than a zero rectangle: an empty rect at the origin would read as a canvas
    /// one pixel wide at the corner, and the barrier that uses it would let a frame through.
    fn default() -> Self {
        Viewing {
            cam: Cam3::default(),
            view: View2d::default(),
            mode_3d: false,
            view_rect: egui::Rect::NOTHING,
            view_anim: None,
            view_restore: None,
            view_dragging: false,
            nav_stash: Vec::new(),
        }
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
        $crate::BodyView {
            armed: &$x.tools.armed,
            active_path: &$x.active_path,
            cache: &$x.cache,
            cmd: &$x.tools.cmd,
            project: &$x.project,
            regen: &$x.regen,
            sketch_ses: $x.sketch_ses,
            win: &$x.win,
        }
    };
}

/// The same, from a CONTEXT, where the fields are already borrows.
#[macro_export]
macro_rules! body_view_in {
    ($x:expr) => {
        $crate::BodyView {
            armed: $x.armed,
            active_path: $x.active_path,
            cache: &*$x.cache,
            cmd: &*$x.cmd,
            project: &*$x.project,
            regen: &*$x.regen,
            sketch_ses: *$x.sketch_ses,
            win: &*$x.win,
        }
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
        BodyView {
            armed: self.armed,
            active_path: self.active_path,
            cache: self.cache,
            cmd: self.cmd,
            project: self.project,
            regen: self.regen,
            sketch_ses: self.sketch_ses,
            win: self.win,
        }
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
}

/// WHETHER THE KEY in this area is taken by somebody else - the name of the neighbouring action.
///
/// Two commands on one key is not "the last one wins" but a silently lost tool: the habitual key is
/// pressed, something else arrives, and it is not clear what broke. So rebinding asks here first.
pub fn hotkey_taken_by(set: &Settings, area: &str, key: &str, except: &str) -> Option<&'static str> {
    HOTKEYS.iter().filter(|r| r.area == area && r.action != except).find(|r| hotkey_key(set, r.action) == key).map(|r| r.action)
}

pub fn hotkey_key(set: &Settings, action: &str) -> String {
    if let Some(k) = set.hotkeys.get(action) {
        return k.clone();
    }
    HOTKEYS.iter().find(|r| r.action == action).map(|r| r.key.to_string()).unwrap_or_default()
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
    /// The DEFAULT key, exactly as in `egui::Key`. What is actually pressed — see `App::hotkey_key`.
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
    let changed: Vec<String> = vars.iter().filter(|(k, v)| params_seen.get(*k).is_none_or(|old| (*old - **v).abs() > 1e-12)).map(|(k, _)| k.clone()).collect();
    for name in &changed {
        project.mark_param_dependents_dirty_for(name);
    }
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
    v_norm([
        wt[0] * n[0] + wt[1] * n[1] + wt[2] * n[2],
        wt[4] * n[0] + wt[5] * n[1] + wt[6] * n[2],
        wt[8] * n[0] + wt[9] * n[1] + wt[10] * n[2],
    ])
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

/// Refresh the cache of smoothed vertex normals for the current `geom_rev` (lazily, indexed as
/// `project.meshes` is). The heavy pass over the triangles is done ONCE per change of geometry rather
/// than every frame.
pub fn ensure_vertex_normals(cache: &Caches, project: &Project, regen: &Rebuilding) {
    let mut c = cache.norm.borrow_mut();
    if c.rev == regen.geom_rev && c.value.len() == project.bodies.len() {
        return;
    }
    c.value = project.bodies.iter().map(|b| &b.mesh).map(|m| m.vertex_normals()).collect();
    c.rev = regen.geom_rev;
}

/// The scene key for the GPU buffer: the same as in `view_key`, BUT without the camera, the size or the
/// dragging flag (the vertices and their colours do not depend on the camera). When it changes, the
/// vertex buffer is uploaded anew.
pub fn gpu_scene_key(pn: &Painting) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    view_rev(pn.regen).hash(&mut h);
    (pn.set.shading == Shading::Smooth).hash(&mut h); // smooth against flat gives different vertex colours, so the buffer is re-uploaded
    pn.scheme.pal.fingerprint().hash(&mut h); // the bodies' colours live IN THE BUFFER: a change of scheme must re-upload it
    if let Some((o, n)) = section_eff(pn.section) {
        for v in o.iter().chain(n.iter()) {
            v.to_bits().hash(&mut h); // the section moved or turned, so the buffer is rebuilt
        }
    }
    for c in &pn.project.components {
        for v in c.transform {
            v.to_bits().hash(&mut h);
        }
        c.visible.hash(&mut h);
    }
    let mut hl: Vec<usize> = highlight_mesh_set(pn.project, &pn.sel).into_iter().collect();
    hl.sort_unstable();
    hl.hash(&mut h);
    for (i, v) in pn.project.bodies.iter().map(|b| b.visible).enumerate() {
        if v {
            (i as u32).hash(&mut h);
        }
    }
    pn.win.context.hash(&mut h);
    current_ctx_id(pn.active_path, pn.project).hash(&mut h);
    pn.cmd.edit.hash(&mut h); // editing a feature hides its result and its chain — see `view_key`
    // the body gizmo's preview: while dragging, the body's transform changes frame by frame, and the scene must see it
    if let Some((dmi, _, _)) = pn.body_giz.drag {
        (dmi as u64).hash(&mut h);
        if let Some(accum) = body_giz_accum(pn.body_giz, pn.set, pn.body_giz.snap) {
            for v in accum {
                v.to_bits().hash(&mut h);
            }
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
    // The selection highlight affects the texture, so the WHOLE set of highlighted bodies goes into the
    // cache key (a body highlights itself; a component or subassembly highlights its whole subtree).
    // Only `Sel::Mesh` and `Sel::Face` used to be hashed, so a click on a body inside an assembly (which
    // is a `Sel::Component`) did not invalidate the raster, and the highlight appeared only after the
    // camera moved.
    let mut hl: Vec<usize> = highlight_mesh_set(pn.project, &pn.sel).into_iter().collect();
    hl.sort_unstable();
    hl.hash(&mut h);
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
    if let Some((c, r)) = scene_sphere_cached(pn, scene_key) {
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
    project
        .bodies
        .get(mi)
        .and_then(|b| b.mesh.bounds())
        .map(|b| [(b.min.x + b.max.x) / 2.0, (b.min.y + b.max.y) / 2.0, (b.min.z + b.max.z) / 2.0])
        .unwrap_or([0.0, 0.0, 0.0])
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

/// The world bounding sphere (centre, radius) of the visible scene, taken from `visible_mesh_items`
/// (exactly what gets drawn). Cached by the scene key: recomputed only when the scene changes, not
/// every frame.
pub fn scene_sphere_cached(pn: &Painting, scene_key: u64) -> Option<([f64; 3], f64)> {
    let (k, cached) = pn.cache.bounds.get();
    if k == scene_key {
        return cached;
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
        (c, r)
    });
    pn.cache.bounds.set((scene_key, sphere));
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
        Some(Screen { cam: dc.cam, set: dc.set, rect: rect, basis: basis }.at(w).0)
    };
    Some((pt(j.a)?, pt(j.b)?))
}

/// Whether a joint is visible in the current view: the joints tick box is on, the workbench is Assembly,
/// and the joint's home is the active context (the joints of nested subassemblies are not shown in the
/// parent - they get in the way).
pub fn joint_visible(active_path: &[Id], project: &Project, set: &Settings, workbench: Workbench, j: &qymcad_core::feature::Joint) -> bool {
    set.show_joints
        && matches!(workbench, Workbench::Assembly)
        && project.joint_in_context(j, current_ctx_id(active_path, project))
}



/// The geometry of the DOF gizmo handles for a kind of joint: (slot [0=angle, 1=offset, 2=offset2],
/// whether it is a ring, the frame axis 0/1/2). It matches `JointKind::motion`: the angle turns about Z;
/// translations and extra angles follow the axes, as in `motion`.
pub fn joint_slot_geom(kind: qymcad_core::feature::JointKind) -> Vec<(u8, bool, u8)> {
    use qymcad_core::feature::JointKind::*;
    match kind {
        Rigid => vec![],
        Revolute => vec![(0, true, 2)],                        // an angle about Z
        Slider => vec![(1, false, 2)],                         // a shift along Z
        Cylindrical => vec![(0, true, 2), (1, false, 2)],      // an angle and a shift along Z
        PinSlot => vec![(0, true, 2), (1, false, 0)],          // an angle about Z plus a shift along X
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
    let fp = sketch_fingerprint(project, si);
    if let Some((csi, cfp, d)) = &*cache.sk_status.borrow() {
        if *csi == si && *cfp == fp {
            return d.clone();
        }
    }
    let d = SketchDiag {
        dof: project.sketch_dof(si),
        free: project.sketch_free_points(si),
        conflicts: project.sketch_conflicts(si).into_iter().collect(),
        redundant: project.sketch_redundant_constraints(si).into_iter().collect(),
    };
    *cache.sk_status.borrow_mut() = Some((si, fp, d.clone()));
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
    if s.constraints.iter().any(tangency) {
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

/// The imprint of a sketch, for the status cache: the coordinates of the points plus the number of
/// entities, constraints and splines. O(n) per frame is pennies against the Jacobian. Any edit or drag
/// changes the imprint, which forces a recount.
pub fn sketch_fingerprint(project: &qymcad_core::model::Project, si: usize) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    if let Some(s) = project.sketches.get(si) {
        for p in &s.points {
            p.id.hash(&mut h);
            p.x.to_bits().hash(&mut h);
            p.y.to_bits().hash(&mut h);
        }
        s.entities.len().hash(&mut h);
        s.constraints.len().hash(&mut h);
        s.splines.len().hash(&mut h);
        // the values of the dimensions (a number edited without the points moving - before solving)
        for c in &s.constraints {
            if let Some(d) = c.dim_value() {
                d.to_bits().hash(&mut h);
            }
        }
    }
    h.finish()
}

/// Whether a sketch (by id) is shown in the visibility context.
pub fn sketch_in_ctx(
    active_path: &[Id],
    project: &qymcad_core::model::Project,
    sketch_ses: SketchSession,
    sketch_id: Id,
) -> bool {
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
        CompPatternKind::Circular { origin: [0.0; 3], dir: unit(carr.axis), angle, count }
    } else {
        CompPatternKind::Linear { dir: unit(carr.dir), step: cmd_val(cmd, "cstep"), count }
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
        parts.push(qymcad_i18n::tr2("m3-value-mm", "label", label, "v", &qymcad_i18n::num(v, 3)));
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
        let (a, b2) = ((b.0[k] - o[k]) * n[k], (b.1[k] - o[k]) * n[k]);
        lo += a.min(b2);
        hi += a.max(b2);
    }
    lo <= 0.0 && hi >= 0.0
}

/// The world bounding box of mesh `mi` (cached by `geom_rev` — computed once per change of the scene).
pub fn mesh_world_bounds(pn: &Painting, mi: usize) -> Option<([f64; 3], [f64; 3])> {
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
    let mut map: std::collections::HashMap<usize, ([f64; 3], [f64; 3])> = std::collections::HashMap::new();
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
        map.insert(i, (mn, mx));
    }
    let out = map.get(&mi).copied();
    pn.cache.mesh_bounds.borrow_mut().put(key, map);
    out
}



pub fn regenerate_now(rc: &mut RebuildCtx) {
    prune_dangling_features(rc.live, rc.project); // anti-ghost: on EVERY regen, orphan meshes and dangling features go
    // THE REBUILD GRAPH: a parameter may have changed (including a named sketch dimension - those are in
    // `param_map`) -> ONLY the features that refer to it are rebuilt. This used to mark ALL features
    // carrying expressions, and it fired on EVERY rebuild: any trifle dragged the whole parametrics of
    // the project through a recount.
    mark_changed_params_dirty(rc.params_seen, rc.project);
    let _gate = qymcad_kernel::kernel_gate();
    let kernel = qymcad_kernel::OcctKernel { shapes: std::cell::RefCell::new(std::mem::take(&mut rc.live.shapes)), quality_k: rc.project.geom_quality.deflection_k() };
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
    // of handing dead geometry outside. It is cleaned by whether a mesh exists in the project.
    // The exception is IMPORTED bodies. Their B-rep cannot be rebuilt from a recipe - only by parsing
    // the embedded STEP again (tens of seconds), so their shape is kept even while the body temporarily
    // does not build (a rollback or a suppression): move the rollback bar back, and the import is on
    // screen instantly.
    let imports: std::collections::HashSet<Id> = rc
        .project
        .timeline
        .iter()
        .filter_map(|n| match n.kind {
            qymcad_core::feature::FeatureKind::Import { body, .. } => Some(body),
            _ => None,
        })
        .collect();
    rc.live.shapes.retain(|body, _| rc.project.mesh_index(*body).is_some() || imports.contains(body));
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
    let missing: Vec<Id> = rc
        .project
        .timeline
        .iter()
        .filter_map(|n| n.kind.body().map(|b| (n.id, b)))
        .filter(|(_, b)| !rc.live.shapes.contains_key(b))
        .map(|(id, _)| id)
        .collect();
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
    rc.live.ready = rc.project.timeline.iter().filter_map(|n| n.kind.body()).all(|b| rc.live.shapes.contains_key(&b));
    fill_model_edges_for_anchors(rc); // anchors on edges are dead after opening otherwise (see io_jobs)
    if was_clean {
        rc.edits.saved_key = edit_key(&DrawCtx { cam: rc.cam, set: rc.set, scheme: rc.scheme, project: rc.project, active_path: rc.active_path }); // rebuilding the cache is not an edit made by hand
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
    restore(rc, before);
    rc.edits.committed_key = doc_key(rc.project);
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

pub fn restore(rc: &mut RebuildCtx, snap: Snapshot) {
    // A snapshot carries THE MESHES but not the live B-rep (`Shape` is not cloneable). `shapes` used
    // to be left over from the undone state: old geometry on screen, new geometry in the kernel, and
    // the NEXT operation built on the undone shape. Silently, because nodes are not dirty after an
    // undo. What gets rebuilt is exactly the bodies whose RECIPE differs between the states (not a
    // forced pass over the whole document — on an assembly of a thousand imports that is tens of
    // seconds).
    let changed = rc.project.changed_bodies_vs(&snap.project);
    let mut restored = snap.project;
    restored.take_source_data_from(rc.project); // the source bytes come from the live document
    // the derived topology caches never went into the snapshot — bring them back from the live state
    // for the bodies the edit did not touch (the changed ones are rebuilt below anyway)
    let (rf, re) = (std::mem::take(&mut rc.project.regen_faces), std::mem::take(&mut rc.project.regen_edges));
    *rc.project = restored;
    rc.project.regen_faces = rf;
    rc.project.regen_edges = re;
    for b in &changed {
        rc.live.shapes.remove(b);
        rc.live.faces.remove(b); // the face cache must not outlive an undo either
    }
    let dirty: Vec<Id> = rc.project.timeline.iter().filter(|n| n.kind.body().is_some_and(|b| changed.contains(&b))).map(|n| n.id).collect();
    for n in &mut rc.project.timeline {
        if dirty.contains(&n.id) {
            n.dirty = true;
        }
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
    if rc.regen.ui_running {
        rc.regen.wanted = true;
        return;
    }
    regenerate_now(rc);
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
    let asked = std::mem::take(&mut rc.regen.pending);
    // THE REBUILD WAS STOPPED BY HAND — it does not start again on its own. An explicit request
    // (`asked`) clears the mark: "Rebuild everything" and any edit of the document both mean
    // "compute again".
    if rc.regen.paused && !asked {
        return;
    }
    rc.regen.paused = false;
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
    let kernel = qymcad_kernel::OcctKernel { shapes: std::cell::RefCell::new(std::mem::take(&mut rc.live.shapes)), quality_k: rc.project.geom_quality.deflection_k() };
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
    let mut missing: Vec<Id> = project.timeline.iter().filter_map(|n| n.kind.body()).filter(|b| !live.shapes.contains_key(b)).collect();
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
/// document: 138 bodies, faces on all 138, EDGES ON TWO, live B-rep on all 138.
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
    let saved = project.joints.iter().find(|j| j.id == joint).and_then(|j| j.drive[slot]);
    *joint_anim = Some(JointAnim { joint, slot, from, to, t: 0.0, forward: true, saved });
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
    project
        .body_owner(body)
        .and_then(|o| project.components.iter().find(|c| c.id == o))
        .map(|c| qymcad_i18n::name(&c.name))
        .unwrap_or_else(|| "?".into())
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
    /// WHAT WAS SET BEFORE THE SWEEP — to put back when it is stopped.
    ///
    /// A sweep is a PREVIEW, not an edit: leaving the part wherever the stop caught it would silently
    /// change the document by pressing a "have a look" button.
    pub saved: Option<f64>,
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
    field(ui, project, id, model, w, hint, &|_| true, true, false)
}

/// THE NAME FIELD. The same thing, but WITHOUT the list of drivers.
///
/// A name is not a formula: there is nothing to substitute other names into it, and the list would also
/// take Enter for itself — the very key a name is confirmed with. In grown-up CAD autocompletion lives
/// in the expression field only, and that is no trifle: a measurement showed that with the list open on
/// the name "h" Enter went to the list and the edit was not applied at all.
pub fn name_field(ui: &mut egui::Ui, project: &Project, id: egui::Id, model: &str, w: f32, hint: &str, valid: &dyn Fn(&str) -> bool) -> ExprOut {
    field(ui, project, id, model, w, hint, valid, false, false)
}

/// THE SAME FIELD, BUT WITH A CHECK BEFORE THE COMMIT.
///
/// `valid` decides whether what was typed can be accepted. A refusal DOES NOT ERASE the text and does
/// not release the focus — otherwise out comes exactly the reported trouble: typing `len` and having
/// the `n` deleted automatically. A person finishes the name or presses Escape, and meanwhile the
/// program says what is wrong.
pub fn field(
    ui: &mut egui::Ui,
    project: &Project,
    id: egui::Id,
    model: &str,
    w: f32,
    hint: &str,
    valid: &dyn Fn(&str) -> bool,
    with_list: bool,
    autofocus: bool,
) -> ExprOut {
    let mut st: FieldState = ui.data_mut(|d| d.get_temp(id)).unwrap_or_default();
    let mut buf = st.buf.clone().unwrap_or_else(|| model.to_string());

    // THE KEYS ARE TAKEN BEFORE THE FIELD IS DRAWN. While the list is open the arrows belong to IT;
    // otherwise they simply move the caret in the text and a row cannot be chosen from the keyboard —
    // which is how it used to be.
    let focused = ui.memory(|m| m.has_focus(id));
    let (mut go_down, mut go_up, mut take, mut esc, mut open_list) = (false, false, false, false, false);
    if focused {
        ui.input_mut(|i| {
            open_list = i.consume_key(egui::Modifiers::COMMAND, egui::Key::Space);
            esc = i.consume_key(egui::Modifiers::NONE, egui::Key::Escape);
            if st.open {
                go_down = i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown);
                go_up = i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp);
                // with the list open, Enter and Tab INSERT what is chosen rather than closing the
                // popup of the tool: the formula is still being written.
                take = i.consume_key(egui::Modifiers::NONE, egui::Key::Enter) || i.consume_key(egui::Modifiers::NONE, egui::Key::Tab);
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
        let area = egui::Area::new(id.with("drv"))
            .order(egui::Order::Tooltip)
            .fixed_pos(resp.rect.left_bottom() + egui::vec2(0.0, 2.0))
            .constrain(true);
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

/// Which action a key press means in this area. It reads the settings and nothing else - the whole
/// application was never needed for a table lookup.
pub fn hotkey_action(set: &Settings, area: &str, key: egui::Key) -> Option<&'static str> {
    let pressed = key.name();
    HOTKEYS.iter().filter(|r| r.area == area).find(|r| hotkey_key(set, r.action) == pressed).map(|r| r.action)
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
    sel_sk.items
        .iter()
        .filter(|(k, _)| *k == 1)
        .filter_map(|(_, eid)| s.entities.iter().find(|e| e.id == *eid).and_then(|e| match e.kind {
            EntityKind::Line { a, b } => Some((a, b)),
            _ => None,
        }))
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
    view.scale = (rect.width() / w).min(rect.height() / h) * 0.85;
    view.center = Vec2::new(((b.0.x + b.1.x) / 2.0) as f32, ((b.0.y + b.1.y) / 2.0) as f32);
    view.initialized = true;
}

/// The bytes of the default system font (used for text); cached.
pub fn default_font(font_cache: &mut Option<Vec<u8>>) -> Option<Vec<u8>> {
    if let Some(f) = font_cache {
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
            *font_cache = Some(b.clone());
            return Some(b);
        }
    }
    None
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
        inline.clear();
    }
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
    // DRIVEN GEOMETRY IS DELETED AS A WHOLE PROJECTION. One segment of a projected contour cannot be
    // removed on its own: it is not "geometry" but a view of an edge of the part — the record left
    // behind would bring it straight back.
    let doomed: Vec<Id> = {
        let s = &project.sketches[si];
        s.projections.iter().filter(|p| p.entities.iter().any(|e| eids.contains(e))).map(|p| p.id).collect()
    };
    for pid in doomed {
        project.remove_sketch_projection(si, pid);
    }
    let eids: Vec<Id> = {
        let s = &project.sketches[si];
        eids.into_iter().filter(|e| s.entities.iter().any(|x| x.id == *e)).collect()
    };
    let sys: std::collections::HashSet<Id> = {
        let s = &project.sketches[si];
        s.immovable_points().into_iter().collect() // system and driven points: they are not deleted one by one
    };
    let pids: Vec<Id> = sel_sk.items.iter().filter(|(k, id)| *k == 0 && !sys.contains(id)).map(|(_, id)| *id).collect();
    if !eids.is_empty() {
        project.delete_entities(si, &eids);
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
pub fn bake_text_glyphs(font_cache: &mut Option<Vec<u8>>, x: f64, y: f64, height: f64, text: &str) -> Vec<Vec<Point2>> {
    let Some(font) = default_font(font_cache) else { return Vec::new() };
    qymcad_core::text::text_outline_contours(&font, text, height, x, y).into_iter().map(|c| c.points).collect()
}

/// The popup for editing a text object: the string plus the height. On apply the glyphs are re-baked and updated.
pub fn text_obj_editor(ed: Editing, annot: &mut AnnotEdit, font_cache: &mut Option<Vec<u8>>, inline: &mut InlineEdit, ctx: &egui::Context, rect: Rect) {
    let Sel::Sketch(si) = *ed.sel else {
        inline.clear();
        return;
    };
    let Some(ti) = inline.text() else { return };
    let Some((bx, _by, _, maxy)) = ed.project.sketch_text_bbox(si, ti) else {
        inline.clear();
        return;
    };
    let at = (Sheet { view: *ed.view, rect: rect }).at(Point2::new(bx, maxy));
    let (mut apply, mut close) = (false, false);
    egui::Area::new(egui::Id::new(("textedit", si, ti))).fixed_pos(clamp_popup(at, rect) + egui::vec2(0.0, -34.0)).order(egui::Order::Foreground).show(ctx, |ui| {
        egui::Frame::popup(ui.style()).show(ui, |ui| {
            ui.horizontal(|ui| {
                let r = ui.add(egui::TextEdit::singleline(&mut annot.text_buf).desired_width(160.0));
                ui.label(qymcad_i18n::tr("g-height-short"));
                ui.add(egui::DragValue::new(&mut annot.text_h).speed(0.2).range(1.0..=1000.0).suffix(qymcad_i18n::tr("unit-mm-suffix")));
                if (r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))) || ui.button(egui_phosphor::regular::CHECK).clicked() {
                    apply = true;
                }
            });
        });
    });
    if apply {
        let (x, y, angle) = {
            let t = &ed.project.sketches[si].texts[ti];
            (t.x, t.y, t.angle)
        };
        let (txt, h) = (annot.text_buf.clone(), annot.text_h);
        let glyphs = bake_text_glyphs(font_cache, x, y, h, &txt);
        ed.project.set_sketch_text(si, ti, qymcad_core::model::TextSpec { at: qymcad_core::geom::Point2::new(x, y), height: h, angle, text: txt, glyphs });
        invalidate(ed.regen);
        close = true;
    }
    if close || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        inline.clear();
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
            if *driven { qymcad_i18n::tr2("con-dim-driven", "what", pfx, "value", &val) } else { qymcad_i18n::tr2("con-dim", "what", pfx, "value", &val) }
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
    let lines = |constr: bool| {
        s.entities
            .iter()
            .filter(move |e| e.construction == constr && matches!(e.kind, qymcad_core::model::EntityKind::Line { .. }))
            .map(|e| e.id)
    };
    lines(true).chain(lines(false)).collect()
}

/// The closed contour of sketch `si` under a screen point (point-in-polygon; the smaller area wins).
pub fn contour_under_2d(project: &Project, view: &View2d, rect: Rect, screen: Pos2, si: usize) -> Option<Id> {
    let mut best: Option<(f64, Id)> = None;
    for cid in sketch_closed_contours(project, si) {
        let ci = project.contour_index(cid)?;
        let pts: Vec<Pos2> = project.contours[ci].points.iter().map(|p| (Sheet { view: *view, rect: rect }).at(*p)).collect();
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
    /// The round ones, as a centre and a radius.
    pub circles: Vec<(Point2, f64)>,
}

pub fn active_edges(dc: &DrawCtx, si: usize) -> ActiveEdges {
    use qymcad_core::model::EntityKind;
    let mut lines = Vec::new();
    let mut circs = Vec::new();
    if let Some(s) = dc.project.sketches.get(si) {
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
    ActiveEdges { lines, circles: circs }
}

/// The entity nearest to a screen point (within 12 px): for a line the distance to the segment, for
/// a circle or an arc the distance to the rim.
pub fn entity_near(pick: &PickCtx, rect: Rect, pos: Pos2, si: usize) -> Option<Id> {
    let s = pick.project.sketches.get(si)?;
    let scr = |id: Id| s.points.iter().find(|q| q.id == id).map(|q| (Sheet { view: *pick.view, rect: rect }).at(Point2::new(q.x, q.y)));
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
    let constr = project
        .sketches
        .get(si)
        .and_then(|s| s.entities.iter().find(|e| e.id == line))
        .map(|e| e.construction)
        .unwrap_or(false);
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
    let name_of = |nid: Id| -> String {
        project.timeline.iter().find(|n| n.id == nid).map(|n| qymcad_i18n::name(&n.name)).unwrap_or_else(|| format!("#{nid}"))
    };
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

/// A rubber-band selection of sketch entities: left to right means enclosure (wholly inside), right to
/// left means crossing (merely touched) — the usual CAD convention.
pub fn box_select_sketch(ed: Editing, sel_sk: &mut SketchSelection, rect: Rect, a: Pos2, b: Pos2, si: usize) {
    let sh = Sheet { view: *ed.view, rect: rect };
    use qymcad_core::model::EntityKind;
    let crossing = b.x < a.x;
    let (x0, x1) = (a.x.min(b.x), a.x.max(b.x));
    let (y0, y1) = (a.y.min(b.y), a.y.max(b.y));
    let inside = |p: Pos2| p.x >= x0 && p.x <= x1 && p.y >= y0 && p.y <= y1;
    // gather the hits first (`s` holds a borrow of self, so they are pushed afterwards)
    let (mut add_ent, mut add_pt): (Vec<Id>, Vec<Id>) = (Vec::new(), Vec::new());
    {
        let Some(s) = ed.project.sketches.get(si) else { return };
        let scr = |id: Id| s.points.iter().find(|q| q.id == id).map(|q| sh.at(Point2::new(q.x, q.y)));
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
        let pts: Vec<Pos2> = c.points.iter().map(|p| (Sheet { view: *pick.view, rect: rect }).at(*p)).collect();
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
/// `op`: 0 delete, 1 mirror, 2 linear array, 3 circular array, 4 fillet, 5 chamfer, 6 offset.
pub fn try_modify(ed: Editing, sel_sk: &mut SketchSelection, sk_pat: SketchPattern, tool_prefs: &SketchToolPrefs, op: u8) -> bool {
    let Sel::Sketch(si) = *ed.sel else { return false };
    let eids: Vec<Id> = sel_sk.items.iter().filter(|(k, _)| *k == 1).map(|(_, id)| *id).collect();
    if eids.is_empty() {
        return false;
    }
    let ok = match op {
        0 => {
            ed.project.delete_entities(si, &eids);
            sel_sk.clear(); // the selection and whatever was waiting for it
            true
        }
        // THE MIRROR IS NOT APPLIED FROM A CLICK. It has two steps of its own (`modify_button` takes the
        // selection, `mirror_about_axis` or `mirror_about_line` finishes it), and applying here would mean
        // mirroring on the FIRST entity clicked - which is how several things could never be mirrored
        // together, and how a misclick became an edit.
        1 => return false,
        2 => {
            ed.project.array_linear(si, &eids, sk_pat.dx, sk_pat.dy, sk_pat.count);
            true
        }
        3 => {
            let (cx, cy) = if let Some(p) = sel_point_ids(sel_sk).first().and_then(|id| sketch_pt(ed.project, si, *id)) {
                (p.x, p.y)
            } else {
                ed.project.entities_centroid(si, &eids)
            };
            ed.project.array_circular(si, &eids, cx, cy, sk_pat.count, sk_pat.angle);
            true
        }
        4 => eids.len() >= 2 && ed.project.fillet_lines(si, eids[0], eids[1], tool_prefs.fillet) && {
            sel_sk.clear(); // the selection and whatever was waiting for it
            true
        },
        5 => eids.len() >= 2 && ed.project.chamfer_lines(si, eids[0], eids[1], tool_prefs.fillet) && {
            sel_sk.clear(); // the selection and whatever was waiting for it
            true
        },
        6 => ed.project.offset_entities(si, &eids, tool_prefs.offset) > 0,
        _ => false,
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
    pub sel_sk: &'a mut SketchSelection,
    pub regen: &'a mut Rebuilding,
    pub status: &'a mut String,
    pub tool_prefs: &'a mut SketchToolPrefs,
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
    field(ui, project, id, model, w, hint, &|_| true, true, want_focus)
}

/// The editing button: with a ready selection it applies at once, otherwise it waits for one (Esc cancels).
pub fn modify_button(mut ed: Editing, t: &mut Tools, sk_pat: SketchPattern, tool_prefs: &SketchToolPrefs, op: u8) {
    exit_draw_tools(&mut t.reborrow());
    let Tools { armed, annot: _, cmd: _, corner: _, dim: _, drag: _, gsel: _, inline: _, measure: _, pat: _, pending_import: _, picking: _, place: _, sel_sk, tool: _ } = t;
    sel_sk.constraint = None;
    **armed = Armed::Modify(match op {
        4 => 1,
        5 => 2,
        6 => 3,
        1 => 4,
        2 => 5,
        3 => 6,
        _ => 0,
    });
    // THE MIRROR HAS TWO STEPS OF ITS OWN: what, then about what. Pressing the button with geometry
    // selected answers the first and asks the second; pressing it with a selection made while the tool was
    // already in hand does the same, which is why the button is the way forward rather than a stray click.
    if op == 1 {
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
    if try_modify(ed.reborrow(), sel_sk, sk_pat, tool_prefs, op) {
        sel_sk.modify = None;
    } else {
        sel_sk.clear(); // the selection, and whatever was waiting on it: pick anew, with nothing stuck from before
        sel_sk.modify = Some(op);
        *ed.status = qymcad_i18n::tr("g-pick-for-op");
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

/// The two doors above meet here: reflect what is held about the line through (ax, ay) and (bx, by).
fn mirror_about(ed: Editing, sel_sk: &mut SketchSelection, ax: f64, ay: f64, bx: f64, by: f64) {
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

/// Turn a click-driven editing operation on or off (1 = trim, 2 = extend, 3 = break).
pub fn set_click_op(t: &mut Tools, mode_3d: &mut bool, op: u8) {
    let cur = t.armed.click_op();
    exit_draw_tools(&mut t.reborrow());
    let Tools { armed, annot: _, cmd: _, corner: _, dim: _, drag: _, gsel: _, inline: _, measure: _, pat: _, pending_import: _, picking: _, place: _, sel_sk, tool: _ } = t;
    **armed = Armed::None;
    sel_sk.constraint = None;
    sel_sk.modify = None;
    **armed = if cur == op { Armed::None } else { Armed::ClickOp(op) };
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
}

pub fn num_or_expr(p: &mut ExprBarCtx, ui: &mut egui::Ui, key: &'static str, cur: f64, fmt: NumFormat) -> f64 {
    let NumFormat { lo, hi, integer, suffix } = fmt;
    let vars = p.project.param_map();
    // THE TEXT IS BORROWED FOR A MOMENT. The drop-down list reads the whole document while the buffer
    // sits in the same `self`, and both cannot be borrowed at once. We work on a copy and put it back.
    let txt = p
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
    let o = expr_field(ui, p.project, egui::Id::new(("bar_expr", key)), &txt, w, &qymcad_i18n::tr("g-expr-placeholder"));
    let txt = o.text;
    p.bar_exprs.insert(key, txt.clone());
    match qymcad_core::expr::eval(&txt, &vars) {
        Ok(v) => {
            let v = if integer { v.round() } else { v };
            v.clamp(lo, hi)
        }
        Err(_) => {
            ui.colored_label(p.scheme.pal.error_mild(), egui_phosphor::regular::X);
            cur
        }
    }
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

/// Resolve the mirror plane the command has picked into (`plane` as a u8 world plane, `datum` Id): a
/// world plane gives (0/1/2, 0); a datum gives (0, id); a face creates a datum plane from that face
/// (offset 0) and gives (0, id). The same one is used both when creating and when editing.
pub fn resolve_mirror_plane(project: &mut qymcad_core::model::Project, sp: qymcad_core::feature::SketchPlane) -> (u8, Id) {
    use qymcad_core::feature::{BasePlane, SketchPlane};
    match sp {
        SketchPlane::World(BasePlane::XY) => (0, 0),
        SketchPlane::World(BasePlane::XZ) => (1, 0),
        SketchPlane::World(BasePlane::YZ) => (2, 0),
        SketchPlane::Datum(id) => (0, id),
        SketchPlane::Face(body, key) => (0, project.add_plane_from_face(body, key, 0.0)),
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
    let (mut sx1, mut sy0) = (f32::MIN, f32::MAX);
    for &x in &[bb.min.x, bb.max.x] {
        for &y in &[bb.min.y, bb.max.y] {
            for &z in &[bb.min.z, bb.max.z] {
                let p = Screen { cam: dc.cam, set: dc.set, rect: rect, basis: basis }.at([x, y, z]).0;
                sx1 = sx1.max(p.x);
                sy0 = sy0.min(p.y);
            }
        }
    }
    (sx1 > f32::MIN).then_some(Pos2::new(sx1 + 14.0, sy0))
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


pub fn apply_theme(scheme: &mut SchemeUi, set: &Settings, ctx: &egui::Context) {
    // A SCHEME SETS BOTH THE CANVAS PALETTE AND THE LOOK OF `egui` ITSELF. These used to be two
    // unrelated things: the theme changed the buttons while the canvas stayed dark — exactly what was
    // reported.
    scheme.pal = scheme.all.iter().find(|p| p.id == set.scheme).cloned().unwrap_or_else(qymcad_scheme::dark);
    // THE LOOK OF `egui` ASKS THE SCHEME rather than choosing between light and dark by itself:
    // otherwise a scheme that colours the interface would colour only the canvas, and half the window
    // would stay factory-coloured. Schemes with no interface colours of their own get exactly that
    // same factory look.
    ctx.set_visuals(qymcad_scheme::visuals(&scheme.pal));
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
pub fn fillet_all_corners(corner: &mut CornerInput, picking: &mut Picking, sel: Sel, sel_sk: &SketchSelection, status: &mut String, tool_prefs: &SketchToolPrefs) {
    // A COMMAND. With a selection, the popup opens on it straight away; without one, the mode becomes "click a shape".
    if let Sel::Sketch(si) = sel {
        let only: std::collections::HashSet<Id> = sel_sk.items.iter().filter(|(k, _)| *k == 1).map(|(_, id)| *id).collect();
        if only.is_empty() {
            *picking = Picking::FilletAll;
            *status = qymcad_i18n::tr("g-fillet-all-hint");
        } else {
            corner.at = Some((si, 0, false));
            corner.only = Some(only);
            corner.pos = None; // the centre of the canvas
            corner.buf = format!("{}", (tool_prefs.fillet * 1000.0).round() / 1000.0);
            corner.focus = true;
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
        $crate::Doing {
            armed: $x.armed,
            cmd: &*$x.cmd,
            gsel: &*$x.gsel,
            joint: $x.joint,
            picking: *$x.picking,
            sketch_ses: *$x.sketch_ses,
            workbench: $x.workbench,
        }
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
        Pos2::new(
            c.x + (p.x as f32 - self.view.center.x) * self.view.scale,
            c.y - (p.y as f32 - self.view.center.y) * self.view.scale,
        )
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
        matches!(
            c.anchor,
            qymcad_core::feature::AnchorRef::EdgeMid(..)
                | qymcad_core::feature::AnchorRef::Vertex(..)
                | qymcad_core::feature::AnchorRef::FaceCenter(..)
        ) && project.joints.iter().any(|j| j.a == c.id || j.b == c.id)
    }) {
        return true;
    }
    // an open sketch ON A FACE draws that face's outline
    sketch_ses
        .editing
        .and_then(|si| project.sketches.iter().find(|s| s.id == si))
        .is_some_and(|s| matches!(s.plane, qymcad_core::feature::SketchPlane::Face(..)))
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
/// in an input field.
///
/// The rule "hold Alt when focused" would be a secret without this hint, and secret mechanisms go
/// unused: `U` gets pressed once, nothing happens, and it is never tried again.
pub fn hotkey_hint(dc: &DrawCtx, ctx: &egui::Context, action: &str) -> String {
    let k = hotkey_key(dc.set, action);
    if k.is_empty() {
        return String::new();
    }
    if ctx.egui_wants_keyboard_input() {
        format!("Alt+{k}")
    } else {
        k
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
