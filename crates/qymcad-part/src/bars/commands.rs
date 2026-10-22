//! THE COMMANDS OF THE PART: their rows of the catalogue, in the order of the bar, and what a press of each does - the
//! same for its button, its row of "More" and the search.

use super::*;
use qymcad_ui_state::catalogue::{Command, Launch, Offer, Shelf};
use qymcad_ui_state::IconId;

const PART: &[&str] = &["part"];
/// The datums stand on the bar of the part and of the assembly: a mirrored copy of a component and the section of
/// the view take them there too.
const PART_AND_ASSEMBLY: &[&str] = &["part", "assembly"];

const CREATE: &str = "tb-group-create";
const FROM_SKETCH: &str = "tb-group-sketch3d";
const PRIMITIVES: &str = "tb-group-prim";
const BODY: &str = "tb-body";

/// THE COMMANDS OF A PART offered to the shell: its rows, their press and its switches on the bar of the path.
pub const OFFER: Offer = Offer { rows: ROWS, press, switches: path_switches };

/// THE COMMANDS OF A PART, in the order of its bar: a sketch and the datums, what is made from a sketch, the
/// primitives, the operations on a body.
///
/// SKETCHES ARE NOT IN ASSEMBLIES. The button once stood there marked "skeleton", but nothing could refer to that
/// skeleton - a joint anchor understands an origin, a base plane, a face, an edge and a vertex, never a sketch - and
/// there is no extrusion in an assembly. The datums do stand there: a mirrored copy of a component and the section
/// of the view take them.
const ROWS: &[Command] = &[
    Command {
        code: "part.sketch",
        benches: PART,
        launch: Launch::SketchPick,
        name_key: "cmdname-new-sketch",
        icon: IconId::SketchPickPlane,
        hint_key: "g-sketch-pick-hint",
        group: CREATE,
        shelf: Shelf::Bar,
    },
    Command {
        code: "part.datum-plane",
        benches: PART_AND_ASSEMBLY,
        launch: Launch::Feat(qymcad_ui_state::PartTool::DatumPlane),
        name_key: "cmdname-datum-plane",
        icon: IconId::DatumPlane,
        hint_key: "g-datum-plane-hint",
        group: CREATE,
        shelf: Shelf::Bar,
    },
    Command {
        code: "part.datum-point",
        benches: PART_AND_ASSEMBLY,
        launch: Launch::Feat(qymcad_ui_state::PartTool::DatumPoint),
        name_key: "cmdname-datum-point",
        icon: IconId::DatumPoint,
        hint_key: "g-datum-point-hint",
        group: CREATE,
        shelf: Shelf::Bar,
    },
    Command {
        code: "part.datum-axis",
        benches: PART_AND_ASSEMBLY,
        launch: Launch::Feat(qymcad_ui_state::PartTool::DatumAxis),
        name_key: "cmdname-datum-axis",
        icon: IconId::DatumAxis,
        hint_key: "g-datum-axis-hint",
        group: CREATE,
        shelf: Shelf::Bar,
    },
    Command {
        code: "part.extrude",
        benches: PART,
        launch: Launch::Feat(qymcad_ui_state::PartTool::Extrude),
        name_key: "",
        icon: IconId::PartExtrude,
        hint_key: "tb-extrude-hint",
        group: FROM_SKETCH,
        shelf: Shelf::Bar,
    },
    Command {
        code: "part.revolve",
        benches: PART,
        launch: Launch::Feat(qymcad_ui_state::PartTool::Revolve),
        name_key: "",
        icon: IconId::PartRevolve,
        hint_key: "tb-revolve-hint",
        group: FROM_SKETCH,
        shelf: Shelf::Bar,
    },
    Command {
        code: "part.sweep",
        benches: PART,
        launch: Launch::Feat(qymcad_ui_state::PartTool::Sweep),
        name_key: "",
        icon: IconId::PartSweep,
        hint_key: "tb-sweep-hint",
        group: FROM_SKETCH,
        shelf: Shelf::Bar,
    },
    Command {
        code: "part.loft",
        benches: PART,
        launch: Launch::Feat(qymcad_ui_state::PartTool::Loft),
        name_key: "",
        icon: IconId::PartLoft,
        hint_key: "tb-loft-hint",
        group: FROM_SKETCH,
        shelf: Shelf::Bar,
    },
    // THE SIX PRIMITIVES, a name of its own each: they share one article, and the search would show six rows
    // reading "Primitives"
    Command {
        code: "part.box",
        benches: PART,
        launch: Launch::Prim(qymcad_ui_state::PartTool::Box),
        name_key: "cmdname-box",
        icon: IconId::PartBox,
        hint_key: "tb-box-hint",
        group: PRIMITIVES,
        shelf: Shelf::Bar,
    },
    Command {
        code: "part.cylinder",
        benches: PART,
        launch: Launch::Prim(qymcad_ui_state::PartTool::Cylinder),
        name_key: "cmdname-cylinder",
        icon: IconId::PartCylinder,
        hint_key: "tb-cylinder-hint",
        group: PRIMITIVES,
        shelf: Shelf::Bar,
    },
    Command {
        code: "part.sphere",
        benches: PART,
        launch: Launch::Prim(qymcad_ui_state::PartTool::Sphere),
        name_key: "cmdname-sphere",
        icon: IconId::PartSphere,
        hint_key: "tb-sphere-hint",
        group: PRIMITIVES,
        shelf: Shelf::Bar,
    },
    Command {
        code: "part.cone",
        benches: PART,
        launch: Launch::Prim(qymcad_ui_state::PartTool::Cone),
        name_key: "cmdname-cone",
        icon: IconId::PartCone,
        hint_key: "tb-cone-hint",
        group: PRIMITIVES,
        shelf: Shelf::Bar,
    },
    Command {
        code: "part.torus",
        benches: PART,
        launch: Launch::Prim(qymcad_ui_state::PartTool::Torus),
        name_key: "cmdname-torus",
        icon: IconId::PartTorus,
        hint_key: "tb-torus-hint",
        group: PRIMITIVES,
        shelf: Shelf::Bar,
    },
    Command {
        code: "part.prism",
        benches: PART,
        launch: Launch::Prim(qymcad_ui_state::PartTool::Prism),
        name_key: "cmdname-prism",
        icon: IconId::PartPrism,
        hint_key: "tb-prism-hint",
        group: PRIMITIVES,
        shelf: Shelf::Bar,
    },
    Command {
        code: "part.fillet",
        benches: PART,
        launch: Launch::Feat(qymcad_ui_state::PartTool::Fillet),
        name_key: "",
        icon: IconId::PartFillet,
        hint_key: "tb-fillet-body-hint",
        group: BODY,
        shelf: Shelf::Bar,
    },
    Command {
        code: "part.chamfer",
        benches: PART,
        launch: Launch::Feat(qymcad_ui_state::PartTool::Chamfer),
        name_key: "",
        icon: IconId::PartChamfer,
        hint_key: "tb-chamfer-body-hint",
        group: BODY,
        shelf: Shelf::Bar,
    },
    Command {
        code: "part.shell",
        benches: PART,
        launch: Launch::Feat(qymcad_ui_state::PartTool::Shell),
        name_key: "",
        icon: IconId::PartShell,
        hint_key: "tb-shell-hint",
        group: BODY,
        shelf: Shelf::Bar,
    },
    Command { code: "part.section", benches: PART, launch: Launch::Section, name_key: "cmdname-section", icon: IconId::PartSection, hint_key: "tb-section-hint-bar", group: BODY, shelf: Shelf::Bar },
    Command { code: "part.hole", benches: PART, launch: Launch::Feat(qymcad_ui_state::PartTool::Hole), name_key: "", icon: IconId::PartHole, hint_key: "tb-hole-hint", group: BODY, shelf: Shelf::Bar },
    Command {
        code: "part.draft",
        benches: PART,
        launch: Launch::Feat(qymcad_ui_state::PartTool::Draft),
        name_key: "",
        icon: IconId::PartDraft,
        hint_key: "tb-draft-hint",
        group: BODY,
        shelf: Shelf::Bar,
    },
    Command {
        code: "part.push-face",
        benches: PART,
        launch: Launch::Feat(qymcad_ui_state::PartTool::PushFace),
        name_key: "",
        icon: IconId::PartPushFace,
        hint_key: "tb-push-face-hint",
        group: BODY,
        shelf: Shelf::Bar,
    },
    Command {
        code: "part.remove-face",
        benches: PART,
        launch: Launch::Feat(qymcad_ui_state::PartTool::RemoveFace),
        name_key: "",
        icon: IconId::PartRemoveFace,
        hint_key: "tb-remove-face-hint",
        group: BODY,
        shelf: Shelf::Bar,
    },
    Command {
        code: "part.thicken",
        benches: PART,
        launch: Launch::Feat(qymcad_ui_state::PartTool::Thicken),
        name_key: "",
        icon: IconId::PartThicken,
        hint_key: "tb-thicken-hint",
        group: BODY,
        shelf: Shelf::Bar,
    },
    // the bridge from the parametric side into the design layer: a face becomes a surface
    Command {
        code: "part.face-copy",
        benches: PART,
        launch: Launch::Feat(qymcad_ui_state::PartTool::FaceCopy),
        name_key: "",
        icon: IconId::PartFaceCopy,
        hint_key: "tb-face-copy-hint",
        group: BODY,
        shelf: Shelf::Bar,
    },
    Command {
        code: "part.offset-surface",
        benches: PART,
        launch: Launch::Feat(qymcad_ui_state::PartTool::OffsetSurface),
        name_key: "",
        icon: IconId::PartOffsetSurface,
        hint_key: "tb-offset-surface-hint",
        group: BODY,
        shelf: Shelf::Bar,
    },
    Command {
        code: "part.surface-replace",
        benches: PART,
        launch: Launch::Feat(qymcad_ui_state::PartTool::SurfaceReplace),
        name_key: "",
        icon: IconId::PartSurfaceReplace,
        hint_key: "tb-surface-replace-hint",
        group: BODY,
        shelf: Shelf::Bar,
    },
    Command {
        code: "part.patch",
        benches: PART,
        launch: Launch::Feat(qymcad_ui_state::PartTool::Patch),
        name_key: "",
        icon: IconId::PartPatch,
        hint_key: "tb-patch-hint",
        group: BODY,
        shelf: Shelf::Bar,
    },
    Command {
        code: "part.stitch",
        benches: PART,
        launch: Launch::Feat(qymcad_ui_state::PartTool::Stitch),
        name_key: "",
        icon: IconId::PartStitch,
        hint_key: "tb-stitch-hint",
        group: BODY,
        shelf: Shelf::Bar,
    },
    Command {
        code: "part.trim",
        benches: PART,
        launch: Launch::Feat(qymcad_ui_state::PartTool::Trim),
        name_key: "",
        icon: IconId::PartTrimSurface,
        hint_key: "tb-trim-surface-hint",
        group: BODY,
        shelf: Shelf::Bar,
    },
    Command {
        code: "part.recognise",
        benches: PART,
        launch: Launch::Feat(qymcad_ui_state::PartTool::Recognise),
        name_key: "",
        icon: IconId::PartRecognise,
        hint_key: "tb-recognise-hint",
        group: BODY,
        shelf: Shelf::Bar,
    },
    Command {
        code: "part.split-body",
        benches: PART,
        launch: Launch::Feat(qymcad_ui_state::PartTool::SplitBody),
        name_key: "",
        icon: IconId::PartSplitBody,
        hint_key: "tb-split-body-hint",
        group: BODY,
        shelf: Shelf::Bar,
    },
    Command { code: "part.measure", benches: PART, launch: Launch::Measure3d, name_key: "", icon: IconId::PartMeasure, hint_key: "tb-measure3d-hint", group: BODY, shelf: Shelf::Bar },
    Command {
        code: "part.split-face",
        benches: PART,
        launch: Launch::Feat(qymcad_ui_state::PartTool::SplitFace),
        name_key: "",
        icon: IconId::PartSplitFace,
        hint_key: "tb-split-face-hint",
        group: BODY,
        shelf: Shelf::Bar,
    },
    Command {
        code: "part.thread",
        benches: PART,
        launch: Launch::Feat(qymcad_ui_state::PartTool::Thread),
        name_key: "",
        icon: IconId::PartThread,
        hint_key: "tb-thread-hint",
        group: BODY,
        shelf: Shelf::Bar,
    },
    Command { code: "part.boolean", benches: PART, launch: Launch::Boolean, name_key: "", icon: IconId::PartBooleanBodies, hint_key: "tb-bool-bodies-hint", group: BODY, shelf: Shelf::Bar },
    // THE MIRROR AND THE PATTERNS OF A BODY stand under "Body". The patterns of components are a tool of the
    // assembly: two buttons that look alike and do different things are worse than duplicates
    Command {
        code: "part.mirror",
        benches: PART,
        launch: Launch::Feat(qymcad_ui_state::PartTool::Mirror),
        name_key: "",
        icon: IconId::PartMirror,
        hint_key: "tb-mirror-body-hint",
        group: BODY,
        shelf: Shelf::Bar,
    },
    Command {
        code: "part.array-linear",
        benches: PART,
        launch: Launch::Feat(qymcad_ui_state::PartTool::LinearPattern),
        name_key: "",
        icon: IconId::PartArrayLinear,
        hint_key: "tb-lin-array-body-hint",
        group: BODY,
        shelf: Shelf::Bar,
    },
    Command {
        code: "part.array-circular",
        benches: PART,
        launch: Launch::Feat(qymcad_ui_state::PartTool::CircularPattern),
        name_key: "",
        icon: IconId::PartArrayCircular,
        hint_key: "tb-circ-array-body-hint",
        group: BODY,
        shelf: Shelf::Bar,
    },
    // one of the several bodies a part shows made a part of its own; the right button on the body does the same
    Command {
        code: "part.piece-to-part",
        benches: PART,
        launch: Launch::PieceToPart,
        name_key: "act-piece-to-part",
        icon: IconId::PartSplitBody,
        hint_key: "act-piece-to-part",
        group: BODY,
        shelf: Shelf::SearchOnly,
    },
];

/// PRESS A COMMAND OF THE PART: what its button did, through the doors its button used. A tool in
/// hand pressed again is put down, as every tool button does.
pub fn press(launch: Launch, bc: &mut qymcad_ui_state::BarCtx) {
    use qymcad_ui_state::{take_or_drop, BarAsk};
    match launch {
        Launch::SketchPick => bc.ask.push(BarAsk::ToggleSketchPick),
        Launch::Feat(n) => {
            if n == qymcad_ui_state::PartTool::Extrude {
                bc.feat.op = 0; // the button of the extrusion adds; the cut is the same command with its own key
            }
            bc.ask.push(take_or_drop(tool_is_taken(bc, n), BarAsk::FeatCmd(n)));
        }
        Launch::Prim(n) => bc.ask.push(take_or_drop(bc.armed.part_tool() == Some(n), BarAsk::PrimCmd(n))),
        Launch::Section => bc.ask.push(BarAsk::ToggleSection),
        Launch::Measure3d => bc.ask.push(BarAsk::ToggleMeasure3d),
        Launch::Boolean => {
            if bc.boolean.pick.is_some() {
                // pressed again with the tool in hand: put down, as every tool button does
                bc.ask.push(BarAsk::CancelAllTools(qymcad_ui_state::Then::Nothing));
            } else {
                // taking the boolean puts down the tool in hand first, or both bars stood at once; two bodies held by
                // Shift are its A and B, one body selected is its A
                let held: Vec<Id> = bc.held.iter().copied().filter(|id| bc.project.mesh_index(*id).is_some()).collect();
                let bodies = match held.as_slice() {
                    [a, b, ..] => qymcad_ui_state::BoolBodies { a: Some(*a), b: Some(*b) },
                    _ => qymcad_ui_state::BoolBodies { a: qymcad_ui_state::selected_body(&*bc.project, &*bc.sel), b: None },
                };
                bc.ask.push(BarAsk::CancelAllTools(qymcad_ui_state::Then::Boolean(bodies)));
            }
        }
        // the commands of the sketch and of the assembly, and the one with no button, are pressed elsewhere
        _ => {}
    }
}

/// THE SWITCHES OF THE PART on the bar of the path: the snap of the gizmo in 3D.
pub fn path_switches(bc: &mut qymcad_ui_state::BarCtx, row: &mut qymcad_ui_state::Switches) {
    qymcad_ui_state::gizmo_snap_switches(bc, row, qymcad_ui_state::Workbench::Part);
}
