//! The records of the commands of the tools: their targets, parameters, inputs and options.

use crate::Notice;
use super::*;

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
    pub fn take(self, boolean: &mut BoolCommand, status: &mut Notice) {
        if let Then::Boolean(a) = self {
            boolean.pick = a.map(|a| (a, 0));
            boolean.b = None;
            *status = Notice::info(qymcad_i18n::tr(if a.is_some() { "tb-bool-pick-b" } else { "tb-pick-body-a-first" }));
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
            layout: Default::default(),
            bar: BarSetting::default(),
            bar_sets: Default::default(),
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
            active_icon_packs: Vec::new(),
            inactive_icon_packs: Vec::new(),
            icon_dev_watch: false,
            watched_icon_packs: Vec::new(),
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

/// THE OFFSET, AIMED: the copy follows the pointer to one side until a click fixes it there, and the box of its
/// distance opens beside it and stays, so the pointer can go to it - the way the corner of a fillet is chosen.
/// Reported behaviour: "a popup with a field, the same choice as the fillet: the side picked, a click fixes it, then
/// the mouse can go to the popup".
#[derive(Clone, Default, PartialEq)]
pub struct OffsetAim {
    /// where the click fixed the side, on the sheet; none while the copy follows the pointer
    pub at: Option<qymcad_core::geom::Point2>,
    /// the distance as typed in the box
    pub buf: String,
    /// the box takes the keyboard on the frame it opens
    pub focus: bool,
}

impl SketchSelection {
    /// Clear the selection, together with whatever was waiting for it. A deferred action without a selection means nothing.
    pub fn clear(&mut self) {
        *self = Self::default();
    }
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
