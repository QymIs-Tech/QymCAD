//! Smaller records: the scheme being edited, a relation picked, the save dialog, joint drags, labels, meshes and edges cached, the edit in hand.

use crate::Notice;
use super::*;

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
pub struct LabelKey(pub(crate) &'static str);

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
    pub status: &'a mut Notice,
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
    *ed.status = Notice::info(qymcad_i18n::tr(if now { "in-made-construction" } else { "in-made-normal" }));
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
