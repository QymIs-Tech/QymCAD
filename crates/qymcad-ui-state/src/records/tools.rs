//! The tool in hand and what the tools hold: the armed tool, recognition, the side tools, the view, the drawing tools.

use super::*;

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
    /// THE HANDLE OF THE COMMAND BEING DRAGGED, by the value it drives (`handles`).
    pub handle_drag: Option<Handle>,
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
