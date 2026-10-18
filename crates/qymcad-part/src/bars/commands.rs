//! THE COMMANDS OF THE SKETCH AND OF THE PART: their rows of the catalogue, in the order of the bar, and what a press
//! of each does - the same for its button, its row of "More" and the search.

use super::*;
use qymcad_ui_state::catalogue::{Command, Launch, Shelf};
use qymcad_ui_state::{EditTool, IconId};

const SKETCH: &[&str] = &["sketch"];
const PART: &[&str] = &["part"];
/// The datums stand on the bar of the part and of the assembly: a mirrored copy of a component and the section of
/// the view take them there too.
const PART_AND_ASSEMBLY: &[&str] = &["part", "assembly"];

const CREATE: &str = "tb-group-create";
const KIND: &str = "tb-type";
const EDIT: &str = "tb-group-edit";
const DIMS: &str = "tb-group-dim";
const CONSTRAINTS: &str = "tb-group-constraints";
const FROM_SKETCH: &str = "tb-group-sketch3d";
const PRIMITIVES: &str = "tb-group-prim";
const BODY: &str = "tb-body";

/// THE COMMANDS OF A SKETCH, in the order of its bar: from drawing to constraining.
pub const SKETCH_COMMANDS: &[Command] = &[
    Command { code: "sketch.select", benches: SKETCH, launch: Launch::SketchSelect, name_key: "", icon: IconId::SketchSelect, hint_key: "tb-select-hint", group: CREATE, shelf: Shelf::Bar },
    Command { code: "sketch.point", benches: SKETCH, launch: Launch::SkTool(5), name_key: "", icon: IconId::SketchPoint, hint_key: "tb-point-hint", group: CREATE, shelf: Shelf::Bar },
    Command { code: "sketch.line", benches: SKETCH, launch: Launch::SkTool(1), name_key: "", icon: IconId::SketchLine, hint_key: "tb-line-hint", group: CREATE, shelf: Shelf::Bar },
    Command { code: "sketch.rect", benches: SKETCH, launch: Launch::SkTool(2), name_key: "", icon: IconId::SketchRect, hint_key: "tb-rect-hint", group: CREATE, shelf: Shelf::Bar },
    Command { code: "sketch.circle", benches: SKETCH, launch: Launch::SkTool(3), name_key: "", icon: IconId::SketchCircle, hint_key: "tb-circle-hint", group: CREATE, shelf: Shelf::Bar },
    Command {
        code: "sketch.circle3",
        benches: SKETCH,
        launch: Launch::SkTool(10),
        name_key: "cmdname-circle-3pt",
        icon: IconId::SketchCircle3Pt,
        hint_key: "tb-circle-3pt",
        group: CREATE,
        shelf: Shelf::Bar,
    },
    Command { code: "sketch.arc", benches: SKETCH, launch: Launch::SkTool(4), name_key: "", icon: IconId::SketchArc, hint_key: "tb-arc-hint", group: CREATE, shelf: Shelf::Bar },
    Command { code: "sketch.polygon", benches: SKETCH, launch: Launch::SkTool(6), name_key: "", icon: IconId::SketchPolygon, hint_key: "tb-polygon-hint", group: CREATE, shelf: Shelf::Bar },
    Command { code: "sketch.slot", benches: SKETCH, launch: Launch::SkTool(7), name_key: "", icon: IconId::SketchSlot, hint_key: "tb-slot-hint", group: CREATE, shelf: Shelf::Bar },
    Command { code: "sketch.ellipse", benches: SKETCH, launch: Launch::SkTool(8), name_key: "", icon: IconId::SketchEllipse, hint_key: "tb-ellipse-hint", group: CREATE, shelf: Shelf::Bar },
    Command { code: "sketch.spline", benches: SKETCH, launch: Launch::SkTool(9), name_key: "", icon: IconId::SketchSpline, hint_key: "tb-spline-hint", group: CREATE, shelf: Shelf::Bar },
    Command { code: "sketch.text", benches: SKETCH, launch: Launch::SkTool(11), name_key: "", icon: IconId::SketchText, hint_key: "tb-text-hint", group: CREATE, shelf: Shelf::Bar },
    Command {
        code: "sketch.construction",
        benches: SKETCH,
        launch: Launch::Construction,
        name_key: "",
        icon: IconId::SketchConstruction,
        hint_key: "tb-construction-hint",
        group: KIND,
        shelf: Shelf::Bar,
    },
    Command { code: "sketch.delete", benches: SKETCH, launch: Launch::Modify(EditTool::Delete), name_key: "", icon: IconId::SketchDelete, hint_key: "tb-delete-hint", group: EDIT, shelf: Shelf::Bar },
    Command {
        code: "sketch.mirror",
        benches: SKETCH,
        launch: Launch::Modify(EditTool::Mirror),
        name_key: "",
        icon: IconId::SketchMirror,
        hint_key: "tb-mirror-sketch-hint",
        group: EDIT,
        shelf: Shelf::Bar,
    },
    Command {
        code: "sketch.pattern-linear",
        benches: SKETCH,
        launch: Launch::SkPattern(1),
        name_key: "cmdname-sk-pattern-linear",
        icon: IconId::SketchArrayLinear,
        hint_key: "tb-lin-array-hint",
        group: EDIT,
        shelf: Shelf::Bar,
    },
    Command {
        code: "sketch.pattern-circular",
        benches: SKETCH,
        launch: Launch::SkPattern(2),
        name_key: "cmdname-sk-pattern-circular",
        icon: IconId::SketchArrayCircular,
        hint_key: "tb-circ-array-hint",
        group: EDIT,
        shelf: Shelf::Bar,
    },
    // the projection is the tool of click 6 - of edges and of a body alike, one tool with one row
    Command { code: "sketch.project", benches: SKETCH, launch: Launch::ClickOp(6), name_key: "", icon: IconId::SketchProject, hint_key: "tb-project-body-hint", group: EDIT, shelf: Shelf::Bar },
    Command { code: "sketch.corner", benches: SKETCH, launch: Launch::ClickOp(4), name_key: "", icon: IconId::SketchFillet, hint_key: "tb-fillet-sketch-hint", group: EDIT, shelf: Shelf::Bar },
    Command {
        code: "sketch.corner-chamfer",
        benches: SKETCH,
        launch: Launch::ClickOp(5),
        name_key: "cmdname-corner-chamfer",
        icon: IconId::SketchChamfer,
        hint_key: "tb-chamfer-sketch-hint",
        group: EDIT,
        shelf: Shelf::Bar,
    },
    Command {
        code: "sketch.fillet-all",
        benches: SKETCH,
        launch: Launch::FilletAll,
        name_key: "cmdname-fillet-all",
        icon: IconId::SketchFilletAll,
        hint_key: "tb-fillet-all-hint",
        group: EDIT,
        shelf: Shelf::Bar,
    },
    Command { code: "sketch.offset", benches: SKETCH, launch: Launch::Modify(EditTool::Offset), name_key: "", icon: IconId::SketchOffset, hint_key: "tb-offset-hint", group: EDIT, shelf: Shelf::Bar },
    Command { code: "sketch.move", benches: SKETCH, launch: Launch::SkMove(1), name_key: "cmdname-sk-move", icon: IconId::SketchMove, hint_key: "tb-move-hint", group: EDIT, shelf: Shelf::Bar },
    Command { code: "sketch.copy", benches: SKETCH, launch: Launch::SkMove(2), name_key: "cmdname-sk-copy", icon: IconId::SketchCopy, hint_key: "tb-copy-hint", group: EDIT, shelf: Shelf::Bar },
    Command {
        code: "sketch.rotate",
        benches: SKETCH,
        launch: Launch::SkMove(3),
        name_key: "cmdname-sk-rotate",
        icon: IconId::SketchRotate,
        hint_key: "tb-rotate-hint",
        group: EDIT,
        shelf: Shelf::Bar,
    },
    Command { code: "sketch.trim", benches: SKETCH, launch: Launch::ClickOp(1), name_key: "", icon: IconId::SketchTrim, hint_key: "tb-trim-hint", group: EDIT, shelf: Shelf::Bar },
    Command { code: "sketch.extend", benches: SKETCH, launch: Launch::ClickOp(2), name_key: "", icon: IconId::SketchExtend, hint_key: "tb-extend-hint", group: EDIT, shelf: Shelf::Bar },
    Command { code: "sketch.break", benches: SKETCH, launch: Launch::ClickOp(3), name_key: "", icon: IconId::SketchBreak, hint_key: "tb-break-hint", group: EDIT, shelf: Shelf::Bar },
    Command { code: "sketch.dim", benches: SKETCH, launch: Launch::Dim(1), name_key: "", icon: IconId::SketchDimLinear, hint_key: "tb-dim-hint", group: DIMS, shelf: Shelf::Bar },
    Command {
        code: "sketch.dim-angle",
        benches: SKETCH,
        launch: Launch::Dim(2),
        name_key: "cmdname-dim-angle",
        icon: IconId::SketchDimAngle,
        hint_key: "tb-dim-angle-hint",
        group: DIMS,
        shelf: Shelf::Bar,
    },
    Command {
        code: "sketch.dim-radius",
        benches: SKETCH,
        launch: Launch::Dim(3),
        name_key: "cmdname-dim-radius",
        icon: IconId::SketchDimRadius,
        hint_key: "tb-dim-radius-hint",
        group: DIMS,
        shelf: Shelf::Bar,
    },
    Command { code: "sketch.measure", benches: SKETCH, launch: Launch::Measure2d, name_key: "", icon: IconId::SketchMeasure, hint_key: "tb-measure-hint", group: DIMS, shelf: Shelf::Bar },
    Command {
        code: "sketch.con-coincident",
        benches: SKETCH,
        launch: Launch::Constraint(0),
        name_key: "con-coincident",
        icon: IconId::ConstraintCoincident,
        hint_key: "con-coincident-hint",
        group: CONSTRAINTS,
        shelf: Shelf::Bar,
    },
    Command {
        code: "sketch.con-horizontal",
        benches: SKETCH,
        launch: Launch::Constraint(1),
        name_key: "con-horizontal",
        icon: IconId::ConstraintHorizontal,
        hint_key: "con-horizontal-hint",
        group: CONSTRAINTS,
        shelf: Shelf::Bar,
    },
    Command {
        code: "sketch.con-vertical",
        benches: SKETCH,
        launch: Launch::Constraint(2),
        name_key: "con-vertical",
        icon: IconId::ConstraintVertical,
        hint_key: "con-vertical-hint",
        group: CONSTRAINTS,
        shelf: Shelf::Bar,
    },
    Command {
        code: "sketch.con-parallel",
        benches: SKETCH,
        launch: Launch::Constraint(3),
        name_key: "con-parallel",
        icon: IconId::ConstraintParallel,
        hint_key: "con-parallel-hint",
        group: CONSTRAINTS,
        shelf: Shelf::Bar,
    },
    Command {
        code: "sketch.con-perpendicular",
        benches: SKETCH,
        launch: Launch::Constraint(4),
        name_key: "con-perpendicular",
        icon: IconId::ConstraintPerpendicular,
        hint_key: "con-perpendicular-hint",
        group: CONSTRAINTS,
        shelf: Shelf::Bar,
    },
    Command {
        code: "sketch.con-equal",
        benches: SKETCH,
        launch: Launch::Constraint(5),
        name_key: "con-equal",
        icon: IconId::ConstraintEqual,
        hint_key: "con-equal",
        group: CONSTRAINTS,
        shelf: Shelf::Bar,
    },
    Command {
        code: "sketch.con-collinear",
        benches: SKETCH,
        launch: Launch::Constraint(7),
        name_key: "con-collinear",
        icon: IconId::ConstraintCollinear,
        hint_key: "con-collinear-hint",
        group: CONSTRAINTS,
        shelf: Shelf::Bar,
    },
    Command {
        code: "sketch.con-concentric",
        benches: SKETCH,
        launch: Launch::Constraint(8),
        name_key: "con-concentric",
        icon: IconId::ConstraintConcentric,
        hint_key: "con-concentric-hint",
        group: CONSTRAINTS,
        shelf: Shelf::Bar,
    },
    Command {
        code: "sketch.con-tangent",
        benches: SKETCH,
        launch: Launch::Constraint(9),
        name_key: "con-tangent",
        icon: IconId::ConstraintTangent,
        hint_key: "con-tangent-hint",
        group: CONSTRAINTS,
        shelf: Shelf::Bar,
    },
    Command {
        code: "sketch.con-symmetric",
        benches: SKETCH,
        launch: Launch::Constraint(10),
        name_key: "con-symmetric",
        icon: IconId::ConstraintSymmetric,
        hint_key: "con-symmetric-hint",
        group: CONSTRAINTS,
        shelf: Shelf::Bar,
    },
    Command {
        code: "sketch.con-midpoint",
        benches: SKETCH,
        launch: Launch::Constraint(11),
        name_key: "con-midpoint",
        icon: IconId::ConstraintMidpoint,
        hint_key: "con-midpoint-hint",
        group: CONSTRAINTS,
        shelf: Shelf::Bar,
    },
    Command { code: "sketch.con-fix", benches: SKETCH, launch: Launch::Constraint(6), name_key: "con-fix", icon: IconId::ConstraintLock, hint_key: "con-fix", group: CONSTRAINTS, shelf: Shelf::Bar },
];

/// THE COMMANDS OF A PART, in the order of its bar: a sketch and the datums, what is made from a sketch, the
/// primitives, the operations on a body.
///
/// SKETCHES ARE NOT IN ASSEMBLIES. The button once stood there marked "skeleton", but nothing could refer to that
/// skeleton - a joint anchor understands an origin, a base plane, a face, an edge and a vertex, never a sketch - and
/// there is no extrusion in an assembly. The datums do stand there: a mirrored copy of a component and the section
/// of the view take them.
pub const PART_COMMANDS: &[Command] = &[
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
        launch: Launch::Feat(20),
        name_key: "cmdname-datum-plane",
        icon: IconId::DatumPlane,
        hint_key: "g-datum-plane-hint",
        group: CREATE,
        shelf: Shelf::Bar,
    },
    Command {
        code: "part.datum-point",
        benches: PART_AND_ASSEMBLY,
        launch: Launch::Feat(21),
        name_key: "cmdname-datum-point",
        icon: IconId::DatumPoint,
        hint_key: "g-datum-point-hint",
        group: CREATE,
        shelf: Shelf::Bar,
    },
    Command {
        code: "part.datum-axis",
        benches: PART_AND_ASSEMBLY,
        launch: Launch::Feat(22),
        name_key: "cmdname-datum-axis",
        icon: IconId::DatumAxis,
        hint_key: "g-datum-axis-hint",
        group: CREATE,
        shelf: Shelf::Bar,
    },
    Command { code: "part.extrude", benches: PART, launch: Launch::Feat(1), name_key: "", icon: IconId::PartExtrude, hint_key: "tb-extrude-hint", group: FROM_SKETCH, shelf: Shelf::Bar },
    Command { code: "part.revolve", benches: PART, launch: Launch::Feat(3), name_key: "", icon: IconId::PartRevolve, hint_key: "tb-revolve-hint", group: FROM_SKETCH, shelf: Shelf::Bar },
    Command { code: "part.sweep", benches: PART, launch: Launch::Feat(8), name_key: "", icon: IconId::PartSweep, hint_key: "tb-sweep-hint", group: FROM_SKETCH, shelf: Shelf::Bar },
    Command { code: "part.loft", benches: PART, launch: Launch::Feat(9), name_key: "", icon: IconId::PartLoft, hint_key: "tb-loft-hint", group: FROM_SKETCH, shelf: Shelf::Bar },
    // THE SIX PRIMITIVES, a name of its own each: they share one article, and the search would show six rows
    // reading "Primitives"
    Command { code: "part.box", benches: PART, launch: Launch::Prim(10), name_key: "cmdname-box", icon: IconId::PartBox, hint_key: "tb-box-hint", group: PRIMITIVES, shelf: Shelf::Bar },
    Command {
        code: "part.cylinder",
        benches: PART,
        launch: Launch::Prim(11),
        name_key: "cmdname-cylinder",
        icon: IconId::PartCylinder,
        hint_key: "tb-cylinder-hint",
        group: PRIMITIVES,
        shelf: Shelf::Bar,
    },
    Command { code: "part.sphere", benches: PART, launch: Launch::Prim(12), name_key: "cmdname-sphere", icon: IconId::PartSphere, hint_key: "tb-sphere-hint", group: PRIMITIVES, shelf: Shelf::Bar },
    Command { code: "part.cone", benches: PART, launch: Launch::Prim(13), name_key: "cmdname-cone", icon: IconId::PartCone, hint_key: "tb-cone-hint", group: PRIMITIVES, shelf: Shelf::Bar },
    Command { code: "part.torus", benches: PART, launch: Launch::Prim(14), name_key: "cmdname-torus", icon: IconId::PartTorus, hint_key: "tb-torus-hint", group: PRIMITIVES, shelf: Shelf::Bar },
    Command { code: "part.prism", benches: PART, launch: Launch::Prim(15), name_key: "cmdname-prism", icon: IconId::PartPrism, hint_key: "tb-prism-hint", group: PRIMITIVES, shelf: Shelf::Bar },
    Command { code: "part.fillet", benches: PART, launch: Launch::Feat(4), name_key: "", icon: IconId::PartFillet, hint_key: "tb-fillet-body-hint", group: BODY, shelf: Shelf::Bar },
    Command { code: "part.chamfer", benches: PART, launch: Launch::Feat(5), name_key: "", icon: IconId::PartChamfer, hint_key: "tb-chamfer-body-hint", group: BODY, shelf: Shelf::Bar },
    Command { code: "part.shell", benches: PART, launch: Launch::Feat(6), name_key: "", icon: IconId::PartShell, hint_key: "tb-shell-hint", group: BODY, shelf: Shelf::Bar },
    Command { code: "part.section", benches: PART, launch: Launch::Section, name_key: "cmdname-section", icon: IconId::PartSection, hint_key: "tb-section-hint-bar", group: BODY, shelf: Shelf::Bar },
    Command { code: "part.hole", benches: PART, launch: Launch::Feat(7), name_key: "", icon: IconId::PartHole, hint_key: "tb-hole-hint", group: BODY, shelf: Shelf::Bar },
    Command { code: "part.draft", benches: PART, launch: Launch::Feat(23), name_key: "", icon: IconId::PartDraft, hint_key: "tb-draft-hint", group: BODY, shelf: Shelf::Bar },
    Command { code: "part.push-face", benches: PART, launch: Launch::Feat(25), name_key: "", icon: IconId::PartPushFace, hint_key: "tb-push-face-hint", group: BODY, shelf: Shelf::Bar },
    Command { code: "part.remove-face", benches: PART, launch: Launch::Feat(26), name_key: "", icon: IconId::PartRemoveFace, hint_key: "tb-remove-face-hint", group: BODY, shelf: Shelf::Bar },
    Command { code: "part.thicken", benches: PART, launch: Launch::Feat(28), name_key: "", icon: IconId::PartThicken, hint_key: "tb-thicken-hint", group: BODY, shelf: Shelf::Bar },
    // the bridge from the parametric side into the design layer: a face becomes a surface
    Command { code: "part.face-copy", benches: PART, launch: Launch::Feat(30), name_key: "", icon: IconId::PartFaceCopy, hint_key: "tb-face-copy-hint", group: BODY, shelf: Shelf::Bar },
    Command { code: "part.offset-surface", benches: PART, launch: Launch::Feat(36), name_key: "", icon: IconId::PartOffsetSurface, hint_key: "tb-offset-surface-hint", group: BODY, shelf: Shelf::Bar },
    Command {
        code: "part.surface-replace",
        benches: PART,
        launch: Launch::Feat(31),
        name_key: "",
        icon: IconId::PartSurfaceReplace,
        hint_key: "tb-surface-replace-hint",
        group: BODY,
        shelf: Shelf::Bar,
    },
    Command { code: "part.patch", benches: PART, launch: Launch::Feat(32), name_key: "", icon: IconId::PartPatch, hint_key: "tb-patch-hint", group: BODY, shelf: Shelf::Bar },
    Command { code: "part.stitch", benches: PART, launch: Launch::Feat(33), name_key: "", icon: IconId::PartStitch, hint_key: "tb-stitch-hint", group: BODY, shelf: Shelf::Bar },
    Command { code: "part.trim", benches: PART, launch: Launch::Feat(34), name_key: "", icon: IconId::PartTrimSurface, hint_key: "tb-trim-surface-hint", group: BODY, shelf: Shelf::Bar },
    Command { code: "part.recognise", benches: PART, launch: Launch::Feat(35), name_key: "", icon: IconId::PartRecognise, hint_key: "tb-recognise-hint", group: BODY, shelf: Shelf::Bar },
    Command { code: "part.split-body", benches: PART, launch: Launch::Feat(27), name_key: "", icon: IconId::PartSplitBody, hint_key: "tb-split-body-hint", group: BODY, shelf: Shelf::Bar },
    Command { code: "part.measure", benches: PART, launch: Launch::Measure3d, name_key: "", icon: IconId::PartMeasure, hint_key: "tb-measure3d-hint", group: BODY, shelf: Shelf::Bar },
    Command { code: "part.split-face", benches: PART, launch: Launch::Feat(29), name_key: "", icon: IconId::PartSplitFace, hint_key: "tb-split-face-hint", group: BODY, shelf: Shelf::Bar },
    Command { code: "part.thread", benches: PART, launch: Launch::Feat(24), name_key: "", icon: IconId::PartThread, hint_key: "tb-thread-hint", group: BODY, shelf: Shelf::Bar },
    Command { code: "part.boolean", benches: PART, launch: Launch::Boolean, name_key: "", icon: IconId::PartBooleanBodies, hint_key: "tb-bool-bodies-hint", group: BODY, shelf: Shelf::Bar },
    // THE MIRROR AND THE PATTERNS OF A BODY stand under "Body". The patterns of components are a tool of the
    // assembly: two buttons that look alike and do different things are worse than duplicates
    Command { code: "part.mirror", benches: PART, launch: Launch::Feat(16), name_key: "", icon: IconId::PartMirror, hint_key: "tb-mirror-body-hint", group: BODY, shelf: Shelf::Bar },
    Command { code: "part.array-linear", benches: PART, launch: Launch::Feat(17), name_key: "", icon: IconId::PartArrayLinear, hint_key: "tb-lin-array-body-hint", group: BODY, shelf: Shelf::Bar },
    Command {
        code: "part.array-circular",
        benches: PART,
        launch: Launch::Feat(18),
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

/// PRESS A COMMAND OF THE SKETCH OR OF THE PART: what its button did, through the doors its button used. A tool in
/// hand pressed again is put down, as every tool button does.
pub fn press(launch: Launch, bc: &mut qymcad_ui_state::BarCtx) {
    use qymcad_ui_state::{take_or_drop, BarAsk};
    match launch {
        Launch::SketchSelect => bc.ask.push(BarAsk::SketchSelectMode),
        Launch::SkTool(n) => bc.ask.push(take_or_drop(bc.armed.draw_kind() == n, BarAsk::SketchTool(n))),
        // WHAT IS SELECTED IS TURNED, as the construction switch of the professional systems does; with nothing
        // selected the switch says what is drawn next
        Launch::Construction => qymcad_ui_state::construction_toggle(qymcad_ui_state::editing_in!(bc), &*bc.sel_sk, &*bc.sketch_ses, &mut bc.tool.construction),
        Launch::Modify(tool) => qymcad_ui_state::modify_button(qymcad_ui_state::editing_in!(bc), &mut qymcad_ui_state::tools_in!(bc), *bc.sk_pat, &*bc.tool_prefs, tool),
        Launch::SkPattern(n) => start_pattern(&mut qymcad_ui_state::tools_in!(bc), &mut *bc.status, n),
        Launch::ClickOp(n @ (4 | 5)) => qymcad_ui_state::start_corner_tool(bc, n),
        Launch::ClickOp(n) => {
            qymcad_ui_state::set_click_op(&mut qymcad_ui_state::tools_in!(bc), &mut *bc.mode_3d, n);
            if n == 6 {
                *bc.status = qymcad_ui_state::Notice::info(qymcad_i18n::tr("tb-project-hint"));
            }
        }
        // "FILLET ALL" IS A TOOL IN HAND like its neighbours: pressed, it puts down what is held and takes what is
        // selected or waits for a shape; pressed again, it is put down
        Launch::FilletAll => {
            let held = qymcad_ui_state::catalogue::pressed(launch, bc);
            qymcad_ui_state::exit_draw_tools(&mut qymcad_ui_state::tools_in!(bc));
            if !held {
                qymcad_ui_state::fillet_all_corners(&*bc.project, &mut *bc.corner, &mut *bc.picking, *bc.sel, &*bc.sel_sk, &mut *bc.status, &*bc.tool_prefs);
            }
        }
        Launch::SkMove(n) => start_move_tool(&mut qymcad_ui_state::tools_in!(bc), &mut *bc.status, n),
        Launch::Dim(n) => qymcad_ui_state::set_dim_tool(&mut qymcad_ui_state::tools_in!(bc), &mut *bc.mode_3d, &*bc.project, *bc.sel, *bc.sketch_ses, &mut *bc.status, n),
        Launch::Measure2d => {
            let on = !bc.armed.measuring();
            // THROUGH THE DOOR, in the order every other tool uses. Setting the flag here and asking for "back to
            // selection" afterwards turned the tool on and then straight off again.
            qymcad_ui_state::set_measure(&mut qymcad_ui_state::tools_in!(bc), on);
            if on {
                *bc.mode_3d = false;
                *bc.status = qymcad_ui_state::Notice::info(qymcad_i18n::tr("in-measure-hint"));
            } else {
                *bc.status = qymcad_ui_state::Notice::info(qymcad_i18n::tr("in-measure-off"));
            }
        }
        Launch::Constraint(n) => bc.ask.push(BarAsk::Constraint(n)),
        Launch::SketchPick => bc.ask.push(BarAsk::ToggleSketchPick),
        Launch::Feat(n) => {
            if n == 1 {
                bc.feat.op = 0; // the button of the extrusion adds; the cut is the same command with its own key
            }
            bc.ask.push(take_or_drop(tool_is_taken(bc, n), BarAsk::FeatCmd(n)));
        }
        Launch::Prim(n) => bc.ask.push(take_or_drop(bc.armed.cmd_kind() == n, BarAsk::PrimCmd(n))),
        Launch::Section => bc.ask.push(BarAsk::ToggleSection),
        Launch::Measure3d => bc.ask.push(BarAsk::ToggleMeasure3d),
        Launch::Boolean => {
            if bc.boolean.pick.is_some() {
                // pressed again with the tool in hand: put down, as every tool button does
                bc.ask.push(BarAsk::CancelAllTools(qymcad_ui_state::Then::Nothing));
            } else {
                // taking the boolean puts down the tool in hand first, or both bars stood at once
                let a = qymcad_ui_state::selected_body(&*bc.project, &*bc.sel);
                bc.ask.push(BarAsk::CancelAllTools(qymcad_ui_state::Then::Boolean(a)));
            }
        }
        // the commands of the assembly and the one with no button are pressed elsewhere
        Launch::NewPart
        | Launch::NewSubassembly
        | Launch::InsertComponent
        | Launch::CompArray(_)
        | Launch::MirrorPart
        | Launch::Joint
        | Launch::Ground
        | Launch::Group
        | Launch::Width
        | Launch::Tangent
        | Launch::Relation
        | Launch::PieceToPart => {}
    }
}
