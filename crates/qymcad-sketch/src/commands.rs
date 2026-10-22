//! THE COMMANDS OF THE SKETCH: their rows of the catalogue, in the order of the bar, and what a press of each does -
//! the same for its button, its row of "More" and the search.

use super::*;
use qymcad_ui_state::catalogue::{Command, Launch, Offer, Shelf};
use qymcad_ui_state::{EditTool, IconId};

const SKETCH: &[&str] = &["sketch"];

const CREATE: &str = "tb-group-create";
const KIND: &str = "tb-type";
const EDIT: &str = "tb-group-edit";
const DIMS: &str = "tb-group-dim";
const CONSTRAINTS: &str = "tb-group-constraints";

/// THE COMMANDS OF A SKETCH offered to the shell: its rows, their press and its switches on the bar of the path.
pub const OFFER: Offer = Offer { rows: ROWS, press, switches: path_switches };

/// THE COMMANDS OF A SKETCH, in the order of its bar: from drawing to constraining.
const ROWS: &[Command] = &[
    Command { code: "sketch.select", benches: SKETCH, launch: Launch::SketchSelect, name_key: "", icon: IconId::SketchSelect, hint_key: "tb-select-hint", group: CREATE, shelf: Shelf::Bar },
    Command {
        code: "sketch.point",
        benches: SKETCH,
        launch: Launch::SkTool(qymcad_ui_state::SketchShape::Point),
        name_key: "",
        icon: IconId::SketchPoint,
        hint_key: "tb-point-hint",
        group: CREATE,
        shelf: Shelf::Bar,
    },
    Command {
        code: "sketch.line",
        benches: SKETCH,
        launch: Launch::SkTool(qymcad_ui_state::SketchShape::Line),
        name_key: "",
        icon: IconId::SketchLine,
        hint_key: "tb-line-hint",
        group: CREATE,
        shelf: Shelf::Bar,
    },
    Command {
        code: "sketch.rect",
        benches: SKETCH,
        launch: Launch::SkTool(qymcad_ui_state::SketchShape::Rect),
        name_key: "",
        icon: IconId::SketchRect,
        hint_key: "tb-rect-hint",
        group: CREATE,
        shelf: Shelf::Bar,
    },
    Command {
        code: "sketch.circle",
        benches: SKETCH,
        launch: Launch::SkTool(qymcad_ui_state::SketchShape::Circle),
        name_key: "",
        icon: IconId::SketchCircle,
        hint_key: "tb-circle-hint",
        group: CREATE,
        shelf: Shelf::Bar,
    },
    Command {
        code: "sketch.arc",
        benches: SKETCH,
        launch: Launch::SkTool(qymcad_ui_state::SketchShape::Arc),
        name_key: "",
        icon: IconId::SketchArc,
        hint_key: "tb-arc-hint",
        group: CREATE,
        shelf: Shelf::Bar,
    },
    Command {
        code: "sketch.polygon",
        benches: SKETCH,
        launch: Launch::SkTool(qymcad_ui_state::SketchShape::Polygon),
        name_key: "",
        icon: IconId::SketchPolygon,
        hint_key: "tb-polygon-hint",
        group: CREATE,
        shelf: Shelf::Bar,
    },
    Command {
        code: "sketch.slot",
        benches: SKETCH,
        launch: Launch::SkTool(qymcad_ui_state::SketchShape::Slot),
        name_key: "",
        icon: IconId::SketchSlot,
        hint_key: "tb-slot-hint",
        group: CREATE,
        shelf: Shelf::Bar,
    },
    Command {
        code: "sketch.ellipse",
        benches: SKETCH,
        launch: Launch::SkTool(qymcad_ui_state::SketchShape::Ellipse),
        name_key: "",
        icon: IconId::SketchEllipse,
        hint_key: "tb-ellipse-hint",
        group: CREATE,
        shelf: Shelf::Bar,
    },
    Command {
        code: "sketch.spline",
        benches: SKETCH,
        launch: Launch::SkTool(qymcad_ui_state::SketchShape::Spline),
        name_key: "",
        icon: IconId::SketchSpline,
        hint_key: "tb-spline-hint",
        group: CREATE,
        shelf: Shelf::Bar,
    },
    Command {
        code: "sketch.text",
        benches: SKETCH,
        launch: Launch::SkTool(qymcad_ui_state::SketchShape::Text),
        name_key: "",
        icon: IconId::SketchText,
        hint_key: "tb-text-hint",
        group: CREATE,
        shelf: Shelf::Bar,
    },
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
        launch: Launch::SkPattern(qymcad_ui_state::PatternKind::Linear),
        name_key: "cmdname-sk-pattern-linear",
        icon: IconId::SketchArrayLinear,
        hint_key: "tb-lin-array-hint",
        group: EDIT,
        shelf: Shelf::Bar,
    },
    Command {
        code: "sketch.pattern-circular",
        benches: SKETCH,
        launch: Launch::SkPattern(qymcad_ui_state::PatternKind::Circular),
        name_key: "cmdname-sk-pattern-circular",
        icon: IconId::SketchArrayCircular,
        hint_key: "tb-circ-array-hint",
        group: EDIT,
        shelf: Shelf::Bar,
    },
    // the projection is the tool of click 6 - of edges and of a body alike, one tool with one row
    Command {
        code: "sketch.project",
        benches: SKETCH,
        launch: Launch::ClickOp(qymcad_ui_state::ClickTool::Project),
        name_key: "",
        icon: IconId::SketchProject,
        hint_key: "tb-project-body-hint",
        group: EDIT,
        shelf: Shelf::Bar,
    },
    Command {
        code: "sketch.corner",
        benches: SKETCH,
        launch: Launch::ClickOp(qymcad_ui_state::ClickTool::Corner),
        name_key: "",
        icon: IconId::SketchFillet,
        hint_key: "tb-fillet-sketch-hint",
        group: EDIT,
        shelf: Shelf::Bar,
    },
    Command {
        code: "sketch.corner-chamfer",
        benches: SKETCH,
        launch: Launch::ClickOp(qymcad_ui_state::ClickTool::CornerChamfer),
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
    Command {
        code: "sketch.move",
        benches: SKETCH,
        launch: Launch::SkMove(qymcad_ui_state::MoveKind::Move),
        name_key: "cmdname-sk-move",
        icon: IconId::SketchMove,
        hint_key: "tb-move-hint",
        group: EDIT,
        shelf: Shelf::Bar,
    },
    Command {
        code: "sketch.copy",
        benches: SKETCH,
        launch: Launch::SkMove(qymcad_ui_state::MoveKind::Copy),
        name_key: "cmdname-sk-copy",
        icon: IconId::SketchCopy,
        hint_key: "tb-copy-hint",
        group: EDIT,
        shelf: Shelf::Bar,
    },
    Command {
        code: "sketch.rotate",
        benches: SKETCH,
        launch: Launch::SkMove(qymcad_ui_state::MoveKind::Rotate),
        name_key: "cmdname-sk-rotate",
        icon: IconId::SketchRotate,
        hint_key: "tb-rotate-hint",
        group: EDIT,
        shelf: Shelf::Bar,
    },
    Command {
        code: "sketch.trim",
        benches: SKETCH,
        launch: Launch::ClickOp(qymcad_ui_state::ClickTool::Trim),
        name_key: "",
        icon: IconId::SketchTrim,
        hint_key: "tb-trim-hint",
        group: EDIT,
        shelf: Shelf::Bar,
    },
    Command {
        code: "sketch.extend",
        benches: SKETCH,
        launch: Launch::ClickOp(qymcad_ui_state::ClickTool::Extend),
        name_key: "",
        icon: IconId::SketchExtend,
        hint_key: "tb-extend-hint",
        group: EDIT,
        shelf: Shelf::Bar,
    },
    Command {
        code: "sketch.break",
        benches: SKETCH,
        launch: Launch::ClickOp(qymcad_ui_state::ClickTool::Break),
        name_key: "",
        icon: IconId::SketchBreak,
        hint_key: "tb-break-hint",
        group: EDIT,
        shelf: Shelf::Bar,
    },
    Command {
        code: "sketch.dim",
        benches: SKETCH,
        launch: Launch::Dim(qymcad_ui_state::DimKind::Linear),
        name_key: "",
        icon: IconId::SketchDimLinear,
        hint_key: "tb-dim-hint",
        group: DIMS,
        shelf: Shelf::Bar,
    },
    Command {
        code: "sketch.dim-angle",
        benches: SKETCH,
        launch: Launch::Dim(qymcad_ui_state::DimKind::Angular),
        name_key: "cmdname-dim-angle",
        icon: IconId::SketchDimAngle,
        hint_key: "tb-dim-angle-hint",
        group: DIMS,
        shelf: Shelf::Bar,
    },
    Command {
        code: "sketch.dim-radius",
        benches: SKETCH,
        launch: Launch::Dim(qymcad_ui_state::DimKind::Radial),
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
        launch: Launch::Constraint(qymcad_ui_state::ConstraintKind::Coincident),
        name_key: "con-coincident",
        icon: IconId::ConstraintCoincident,
        hint_key: "con-coincident-hint",
        group: CONSTRAINTS,
        shelf: Shelf::Bar,
    },
    Command {
        code: "sketch.con-horizontal",
        benches: SKETCH,
        launch: Launch::Constraint(qymcad_ui_state::ConstraintKind::Horizontal),
        name_key: "con-horizontal",
        icon: IconId::ConstraintHorizontal,
        hint_key: "con-horizontal-hint",
        group: CONSTRAINTS,
        shelf: Shelf::Bar,
    },
    Command {
        code: "sketch.con-vertical",
        benches: SKETCH,
        launch: Launch::Constraint(qymcad_ui_state::ConstraintKind::Vertical),
        name_key: "con-vertical",
        icon: IconId::ConstraintVertical,
        hint_key: "con-vertical-hint",
        group: CONSTRAINTS,
        shelf: Shelf::Bar,
    },
    Command {
        code: "sketch.con-parallel",
        benches: SKETCH,
        launch: Launch::Constraint(qymcad_ui_state::ConstraintKind::Parallel),
        name_key: "con-parallel",
        icon: IconId::ConstraintParallel,
        hint_key: "con-parallel-hint",
        group: CONSTRAINTS,
        shelf: Shelf::Bar,
    },
    Command {
        code: "sketch.con-perpendicular",
        benches: SKETCH,
        launch: Launch::Constraint(qymcad_ui_state::ConstraintKind::Perpendicular),
        name_key: "con-perpendicular",
        icon: IconId::ConstraintPerpendicular,
        hint_key: "con-perpendicular-hint",
        group: CONSTRAINTS,
        shelf: Shelf::Bar,
    },
    Command {
        code: "sketch.con-equal",
        benches: SKETCH,
        launch: Launch::Constraint(qymcad_ui_state::ConstraintKind::Equal),
        name_key: "con-equal",
        icon: IconId::ConstraintEqual,
        hint_key: "con-equal",
        group: CONSTRAINTS,
        shelf: Shelf::Bar,
    },
    Command {
        code: "sketch.con-collinear",
        benches: SKETCH,
        launch: Launch::Constraint(qymcad_ui_state::ConstraintKind::Collinear),
        name_key: "con-collinear",
        icon: IconId::ConstraintCollinear,
        hint_key: "con-collinear-hint",
        group: CONSTRAINTS,
        shelf: Shelf::Bar,
    },
    Command {
        code: "sketch.con-concentric",
        benches: SKETCH,
        launch: Launch::Constraint(qymcad_ui_state::ConstraintKind::Concentric),
        name_key: "con-concentric",
        icon: IconId::ConstraintConcentric,
        hint_key: "con-concentric-hint",
        group: CONSTRAINTS,
        shelf: Shelf::Bar,
    },
    Command {
        code: "sketch.con-tangent",
        benches: SKETCH,
        launch: Launch::Constraint(qymcad_ui_state::ConstraintKind::Tangent),
        name_key: "con-tangent",
        icon: IconId::ConstraintTangent,
        hint_key: "con-tangent-hint",
        group: CONSTRAINTS,
        shelf: Shelf::Bar,
    },
    Command {
        code: "sketch.con-symmetric",
        benches: SKETCH,
        launch: Launch::Constraint(qymcad_ui_state::ConstraintKind::Symmetric),
        name_key: "con-symmetric",
        icon: IconId::ConstraintSymmetric,
        hint_key: "con-symmetric-hint",
        group: CONSTRAINTS,
        shelf: Shelf::Bar,
    },
    Command {
        code: "sketch.con-midpoint",
        benches: SKETCH,
        launch: Launch::Constraint(qymcad_ui_state::ConstraintKind::Midpoint),
        name_key: "con-midpoint",
        icon: IconId::ConstraintMidpoint,
        hint_key: "con-midpoint-hint",
        group: CONSTRAINTS,
        shelf: Shelf::Bar,
    },
    Command {
        code: "sketch.con-fix",
        benches: SKETCH,
        launch: Launch::Constraint(qymcad_ui_state::ConstraintKind::Fix),
        name_key: "con-fix",
        icon: IconId::ConstraintLock,
        hint_key: "con-fix",
        group: CONSTRAINTS,
        shelf: Shelf::Bar,
    },
];

/// PRESS A COMMAND OF THE SKETCH: what its button did, through the doors its button used. A tool in hand pressed again
/// never reaches here: it is put down by the one door every press goes through (`catalogue::again`).
pub fn press(launch: Launch, bc: &mut qymcad_ui_state::BarCtx) {
    use qymcad_ui_state::BarAsk;
    // A TOOL OF A SKETCH WITH NO SKETCH OPEN IS REFUSED IN WORDS: the search offers every command, and taken in a part
    // a dimension turned the view flat with no bar, swallowed every click and left the view flat after Esc
    if qymcad_ui_state::edit_si(&*bc.project, &*bc.sketch_ses).is_none() {
        *bc.status = qymcad_ui_state::Notice::refusal(qymcad_i18n::tr("sk-tool-outside-sketch"));
        return;
    }
    match launch {
        Launch::SketchSelect => bc.ask.push(BarAsk::SketchSelectMode),
        Launch::SkTool(n) => bc.ask.push(BarAsk::SketchTool(n)),
        // WHAT IS SELECTED IS TURNED, as the construction switch of the professional systems does; with nothing
        // selected the switch says what is drawn next
        Launch::Construction => qymcad_ui_state::construction_toggle(qymcad_ui_state::editing_in!(bc), &*bc.sel_sk, &*bc.sketch_ses, &mut bc.tool.construction),
        Launch::Modify(tool) => qymcad_ui_state::modify_button(qymcad_ui_state::editing_in!(bc), &mut qymcad_ui_state::tools_in!(bc), *bc.sk_pat, &*bc.tool_prefs, tool),
        Launch::SkPattern(n) => start_pattern(&mut qymcad_ui_state::tools_in!(bc), &mut *bc.status, n),
        Launch::ClickOp(n) if n.of_corner() => qymcad_ui_state::start_corner_tool(bc, n),
        Launch::ClickOp(n) => {
            qymcad_ui_state::set_click_op(&mut qymcad_ui_state::tools_in!(bc), &mut *bc.mode_3d, n);
            if n == qymcad_ui_state::ClickTool::Project {
                *bc.status = qymcad_ui_state::Notice::info(qymcad_i18n::tr("tb-project-hint"));
            }
        }
        // "FILLET ALL" IS A TOOL IN HAND like its neighbours: pressed, it puts down what is held and takes what is
        // selected or waits for a shape
        Launch::FilletAll => {
            qymcad_ui_state::exit_draw_tools(&mut qymcad_ui_state::tools_in!(bc));
            qymcad_ui_state::fillet_all_corners(&*bc.project, &mut *bc.corner, &mut *bc.picking, *bc.sel, &*bc.sel_sk, &mut *bc.status);
        }
        Launch::SkMove(n) => start_move_tool(&mut qymcad_ui_state::tools_in!(bc), &mut *bc.status, n),
        Launch::Dim(n) => qymcad_ui_state::set_dim_tool(&mut qymcad_ui_state::tools_in!(bc), &mut *bc.mode_3d, &*bc.project, *bc.sel, *bc.sketch_ses, &mut *bc.status, n),
        Launch::Measure2d => {
            // THROUGH THE DOOR, in the order every other tool uses. Setting the flag here and asking for "back to
            // selection" afterwards turned the tool on and then straight off again.
            qymcad_ui_state::set_measure(&mut qymcad_ui_state::tools_in!(bc), true);
            *bc.mode_3d = false;
            *bc.status = qymcad_ui_state::Notice::info(qymcad_i18n::tr("in-measure-hint"));
        }
        Launch::Constraint(n) => bc.ask.push(BarAsk::Constraint(n)),
        // the commands of the part and of the assembly are pressed by their own workbenches
        _ => {}
    }
}

/// Turn on interactive moving, copying or rotating: pick, then the base point or centre, then the target (for a
/// rotation, the angle in the popup).
pub fn start_move_tool(t: &mut qymcad_ui_state::Tools, status: &mut qymcad_ui_state::Notice, op: qymcad_ui_state::MoveKind) {
    qymcad_ui_state::exit_draw_tools(&mut t.reborrow());
    let qymcad_ui_state::Tools { armed, annot: _, cmd: _, corner: _, dim: _, drag: _, gsel: _, inline: _, measure: _, pat: _, pending_import: _, picking: _, place: _, sel_sk, tool } = t;
    sel_sk.modify = None;
    **armed = qymcad_ui_state::Armed::Move(op);
    tool.move_base = None;
    let has_sel = sel_sk.items.iter().any(|it| it.kind == qymcad_ui_state::ItemKind::Curve);
    let what = if op == qymcad_ui_state::MoveKind::Rotate { qymcad_i18n::tr("f-rotation-centre") } else { qymcad_i18n::tr("f-base-point") };
    *status = Notice::info(if has_sel { qymcad_i18n::tr1("cmd-click-what", "what", &what) } else { qymcad_i18n::tr1("cmd-select-then-click", "what", &what) });
}

/// Turn on the array feature, linear or circular: pick, then parameters, then Enter, with a preview.
pub fn start_pattern(t: &mut qymcad_ui_state::Tools, status: &mut qymcad_ui_state::Notice, op: qymcad_ui_state::PatternKind) {
    qymcad_ui_state::exit_draw_tools(&mut t.reborrow()); // drops the other modes (move, click-op) and `pat.op`
    let qymcad_ui_state::Tools { armed, annot: _, cmd: _, corner: _, dim: _, drag: _, gsel: _, inline: _, measure: _, pat, pending_import: _, picking: _, place: _, sel_sk, tool: _ } = t;
    sel_sk.modify = None;
    **armed = qymcad_ui_state::Armed::Pattern(op);
    pat.edit = None;
    pat.center = None;
    *status = Notice::info(qymcad_i18n::tr(if op == qymcad_ui_state::PatternKind::Circular { "msg-circ-array-sketch" } else { "msg-lin-array-sketch" }));
}

/// THE SWITCHES OF A SKETCH on the bar of the path: while a shape is drawn point by point, closing, finishing or
/// dropping it; while a sketch is open, the snap of the cursor with its grid and the automatic constraints.
pub fn path_switches(bc: &mut qymcad_ui_state::BarCtx, row: &mut qymcad_ui_state::Switches) {
    if let Some(n) = bc.pending_import.draw_pts.as_ref().map(Vec::len) {
        row.group(|ui| {
            ui.label(qymcad_i18n::tr1("wb-points-n", "n", &n.to_string()));
            if ui.button(qymcad_i18n::tr("wb-close")).clicked() {
                qymcad_ui_state::finish_drawing(qymcad_ui_state::editing_in!(bc), &mut *bc.pending_import, *bc.sketch_ses, true);
            }
            if ui.button(qymcad_i18n::tr("wb-line")).clicked() {
                qymcad_ui_state::finish_drawing(qymcad_ui_state::editing_in!(bc), &mut *bc.pending_import, *bc.sketch_ses, false);
            }
            if ui.button(qymcad_i18n::tr("wb-cancel")).clicked() {
                bc.pending_import.draw_pts = None;
            }
            qymcad_ui_state::Drew::Something
        });
    }
    if bc.sketch_ses.editing.is_some() {
        row.group(|ui| {
            qymcad_ui_state::snap_and_grid(bc.set, ui);
            ui.toggle_value(&mut bc.set.auto_constrain, format!("{} {}", ph::MAGIC_WAND, qymcad_i18n::tr("wb-auto-constraints"))).on_hover_text(qymcad_i18n::tr("wb-auto-constraints-hint"));
            qymcad_ui_state::Drew::Something
        });
    }
}

/// A DOUBLE CLICK ON A COPY OF AN ARRAY reopens the array `pi` of the sketch open: its count and its step, or its count
/// and its angle and centre, with a preview. Re-opening an array takes the pattern tool, so the previous one goes first.
pub fn reopen_pattern(sk: &mut qymcad_ui_state::SketchCtx, pi: usize) {
    use qymcad_core::model::PatternKind;
    let Some(si) = qymcad_ui_state::edit_si(sk.project, sk.sketch_ses) else { return };
    qymcad_ui_state::exit_draw_tools(&mut qymcad_ui_state::tools_in!(sk));
    sk.pat.edit = Some(pi);
    match sk.project.sketches[si].patterns[pi].kind {
        PatternKind::Linear { dx, dy, count, dx2, dy2, count2 } => {
            *sk.armed = qymcad_ui_state::Armed::Pattern(qymcad_ui_state::PatternKind::Linear);
            sk.sk_pat.dx = dx;
            sk.sk_pat.dy = dy;
            sk.sk_pat.count = count;
            sk.sk_pat.dx2 = dx2;
            sk.sk_pat.dy2 = dy2;
            sk.sk_pat.count2 = count2.max(1);
            sk.pat.center = None;
        }
        PatternKind::Circular { cx, cy, count, total_deg } => {
            *sk.armed = qymcad_ui_state::Armed::Pattern(qymcad_ui_state::PatternKind::Circular);
            sk.sk_pat.count = count;
            sk.sk_pat.angle = total_deg;
            sk.pat.center = Some(qymcad_core::geom::Point2::new(cx, cy));
            // the centre from the record (it can be re-picked)
        }
    }
    *sk.status = Notice::info(qymcad_i18n::tr("sk-array-edit-hint"));
}
