//! THE JOINTS OF AN ASSEMBLY: their gizmos, their animation, their editing, the tools of an assembly.

use crate::Notice;
use super::*;

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

/// Font-independent glyphs for constraints, dimensions and tools (drawn as lines).
#[derive(Clone, Copy, PartialEq, Debug)]
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
pub fn change_joint_kind(project: &mut Project, status: &mut Notice, jid: Id, newk: qymcad_core::feature::JointKind) -> bool {
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
    *status = Notice::info(qymcad_i18n::tr1("g-joint-kind-set", "kind", &qymcad_i18n::tr(newk.label())));
    true
}

/// Leave the joint editing mode (Esc or Done).
pub fn exit_joint_edit(joint: &mut JointCommand, status: &mut Notice) {
    joint.edit = None;
    joint.edit_repick = None;
    *status = Notice::info(qymcad_i18n::tr("g-joint-edit-done"));
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
    pub status: &'a mut Notice,
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
            // the joints of an assembly are not worked inside a sketch
            sketch_ses: &SketchSession::CLOSED,
        }
    }
}
