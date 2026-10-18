//! THE NARROW CONTEXTS a workbench or a panel works through, and what they ask back.

use super::*;

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
    /// THE NODE WHOSE ROW WAS LAST BROUGHT INTO VIEW: the row of a selection made elsewhere - the viewport, the window
    /// of faults, a path in the window of parameters - shows itself once, then stays where the person scrolls it.
    pub shown: Option<Id>,
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
    /// THE ROWS OF THE TREE PICKED TOGETHER when the delete was asked, by their ids: the question names them all, and
    /// a yes deletes each. Empty when one thing is asked.
    pub delete_group: Vec<Id>,
    /// What a group delete has still to delete, one node a frame (`next_of_a_group_delete`).
    pub delete_queue: DeleteQueue,
}

/// THE NODES A GROUP DELETE HAS STILL TO DELETE, by their ids, and whether what stands on them goes too. One a frame,
/// each found again by its id: deleting one moves the places of the rest in their lists.
#[derive(Clone, Default)]
pub struct DeleteQueue {
    pub ids: Vec<Id>,
    pub dependents: bool,
    /// THE ONE STEP OF UNDO the whole group is, open from the yes until the last node is deleted: each delete opens a
    /// step of its own inside it, and a nested step only counts down (`commit_edit`). Its name while open.
    pub step: Option<String>,
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
    pub status: &'a mut Notice,
    pub sel: &'a mut Sel,
    pub active_path: &'a Vec<Id>,
    pub cam: &'a Cam3,
    pub scheme: &'a SchemeUi,
    pub set: &'a Settings,
    /// THE SKETCH OPEN FOR EDITING: while one is, the scheduler waits for it to close (`rebuild_after_sketch`)
    pub sketch_ses: &'a SketchSession,
}

/// WHAT THE PART WORKBENCH TOUCHES.
///
/// Large, and honestly so: fifteen of these fields are the PARAMETER RECORDS OF ITS OWN TOOLS - the
/// chamfer, the shell, the array, the thread, the hole, the sweep and the rest. A workbench holds its own
/// tools; that is what makes it a workbench rather than a folder. The rest is what any tool needs: the
/// document, the command being run, the status line, the selection, the view.
pub struct PartCtx<'a> {
    pub armed: &'a mut Armed,
    pub status: &'a mut Notice,
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
            sketch_ses: self.sketch_ses,
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
    pub status: Notice,
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
            status: Notice::default(),
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
    pub status: &'a mut Notice,
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
            sketch_ses: self.sketch_ses,
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
    pub status: &'a mut Notice,
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
            sketch_ses: self.sketch_ses,
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
    pub status: &'a mut Notice,
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
    /// What the line says, and the rebuild's own line beside it.
    pub line: StatusLine<'a>,
    /// The badge leads to the window, so the line must be able to open it.
    pub win: &'a mut Windows,
    /// A fault of a parameter opens the table narrowed to its name.
    pub par_search: &'a mut String,
    /// A click on a single fault goes there: the request is carried out after the frame, as the windows' are.
    pub ask: &'a mut Vec<WinAsk>,
}

/// WHAT THE STATUS LINE SAYS, written when a click goes straight to a fault, and THE REBUILD'S OWN LINE
/// (`Rebuilding::line`): the words of a fault are about the document, so they are written to both, and the end of the
/// next rebuild replaces them as it replaces its own - a fault fixed must not stay red over a document that built.
pub struct StatusLine<'a> {
    pub said: &'a mut Notice,
    pub rebuild: &'a mut Notice,
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
    /// the sketch open for editing, if any: a parameter edited over it is rebuilt when it closes
    pub sketch_ses: &'a SketchSession,
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
    pub status: &'a mut Notice,
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
    /// because stepping in clears the selection - and writes the context on the status line, so the words
    /// of a fault led to travel with it and are said after the step.
    GoTo {
        owner: Option<Id>,
        sel: Sel,
        said: Option<Notice>,
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
            sketch_ses: self.sketch_ses,
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
    pub status: &'a mut Notice,
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
            sketch_ses: self.sketch_ses,
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
