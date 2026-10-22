//! The bar of a feature command, described as data, tool by tool: its modes, switches, the references it picks and what
//! it says. The frame draws it (`qymcad_ui_state::tool_in_hand`); what was touched comes back here (`act`).

use qymcad_ui_state::{AsLit, Mode, Did, Notice, Opt, OptKey};
use super::*;

/// The name the bar gives the command `kind`.
fn command_title(kind: qymcad_ui_state::PartTool) -> String {
    qymcad_i18n::tr(match kind {
        qymcad_ui_state::PartTool::Extrude => "cmd-extrude",
        qymcad_ui_state::PartTool::Revolve => "cmd-revolve",
        qymcad_ui_state::PartTool::Sweep => "cmd-sweep",
        qymcad_ui_state::PartTool::Loft => "cmd-loft",
        qymcad_ui_state::PartTool::Fillet => "cmd-fillet",
        qymcad_ui_state::PartTool::Chamfer => "cmd-chamfer",
        qymcad_ui_state::PartTool::Shell => "cmd-shell",
        qymcad_ui_state::PartTool::Hole => "cmd-hole",
        qymcad_ui_state::PartTool::Box => "cmd-box",
        qymcad_ui_state::PartTool::Cylinder => "cmd-cylinder",
        qymcad_ui_state::PartTool::Sphere => "cmd-sphere",
        qymcad_ui_state::PartTool::Cone => "cmd-cone",
        qymcad_ui_state::PartTool::Torus => "cmd-torus",
        qymcad_ui_state::PartTool::Prism => "cmd-prism",
        qymcad_ui_state::PartTool::Mirror => "cmd-mirror",
        qymcad_ui_state::PartTool::LinearPattern => "cmd-linear-array",
        qymcad_ui_state::PartTool::CircularPattern => "cmd-circular-array",
        qymcad_ui_state::PartTool::DatumPlane => "cmd-plane",
        qymcad_ui_state::PartTool::DatumPoint => "cmd-point",
        qymcad_ui_state::PartTool::DatumAxis => "cmd-axis",
        qymcad_ui_state::PartTool::Draft => "cmd-draft",
        qymcad_ui_state::PartTool::Thread => "cmd-thread",
        qymcad_ui_state::PartTool::PushFace => "f-push-face",
        qymcad_ui_state::PartTool::RemoveFace => "feat-name-remove-face",
        qymcad_ui_state::PartTool::SplitBody => "feat-name-split-body",
        qymcad_ui_state::PartTool::Thicken => "feat-name-thicken",
        qymcad_ui_state::PartTool::SplitFace => "feat-name-split-face",
        qymcad_ui_state::PartTool::FaceCopy => "feat-name-face-copy",
        qymcad_ui_state::PartTool::OffsetSurface => "feat-name-offset-surface",
        qymcad_ui_state::PartTool::SurfaceReplace => "f-surface-replace",
        qymcad_ui_state::PartTool::Patch => "feat-name-patch",
        qymcad_ui_state::PartTool::Stitch => "f-stitch",
        qymcad_ui_state::PartTool::Trim => "feat-name-trim",
        qymcad_ui_state::PartTool::Recognise => "f-recognise",
    })
}

/// WHAT THE COMMAND IN HAND FOLLOWS EVERY FRAME, apart from what is touched in its bar: the side a cut goes to, the path
/// or the sections picked in the tree, the sketch of a hole, what a recognition found.
pub(crate) fn command_step(pc: &mut qymcad_ui_state::PartCtx, ctx: &egui::Context) {
    match pc.armed.part_tool() {
        Some(qymcad_ui_state::PartTool::Extrude) => {
            // THE BUTTON SHOWS AND TURNS THE DIRECTION IN FORCE. A cut on a face goes into the body by itself, worked out
            // only at Enter, so the button stood released over a cut already turned into the body and its first press
            // turned it... into the body. Reported behaviour: a square on the top cut 5 up, away from the block, still took
            // 500 mm^3 out of it.
            if pc.feat.flip_auto {
                if let Some(si) = pc.cmd.sketch.filter(|i| *i < pc.project.sketches.len()) {
                    pc.feat.flip = pc.armed.part_tool().is_some_and(|t| smart_flip(t, pc.feat.op, &pc.project.sketches[si].plane));
                }
            }
            if pc.feat.op == 0 && pc.cmd.extent.through() {
                pc.cmd.extent = qymcad_ui_state::ExtentMode::Length; // "through all" only applies to operations on a body
            }
        }
        // WHAT WAS FOUND, before Enter: the regions by kind, and those that fit no surface
        Some(qymcad_ui_state::PartTool::Recognise) => recognise_preview(pc, ctx),
        // THE PATH OF A SWEEP is the last one picked: edges of a body clicked in the view let go of a path sketch, and a
        // sketch is taken from the tree selection while the pick is active (as for a hole driven by a sketch)
        Some(qymcad_ui_state::PartTool::Sweep) => {
            if !pc.gsel.edges.is_empty() {
                (pc.sweep.path_sid, pc.sweep.path_cid) = (0, 0);
            }
            let taken = match *pc.sel {
                qymcad_ui_state::Sel::Sketch(si) if pc.sweep.pick_path => pc.project.sketches.get(si).map(|s| s.id).filter(|id| *id != pc.sweep.prof_sid),
                _ => None,
            };
            if let Some(id) = taken {
                (pc.sweep.path_sid, pc.sweep.path_cid, pc.sweep.pick_path) = (id, 0, false);
                // a sketch taken for the path lets go of edges picked for it: the last pick is the path
                pc.gsel.edges.clear();
                pc.gsel.described = None;
            }
        }
        // A new section is taken on the EDGE of a change in the tree selection (while the pick is active). Only when the
        // selected sketch changes - otherwise a section removed by its cross would come straight back while the same
        // sketch stayed selected in the tree.
        Some(qymcad_ui_state::PartTool::Loft) if pc.loft.pick => {
            let cur = if let qymcad_ui_state::Sel::Sketch(si) = *pc.sel { pc.project.sketches.get(si).map(|s| s.id) } else { None };
            if cur != pc.loft.pick_last {
                pc.loft.pick_last = cur;
                if let Some(sid) = cur.filter(|sid| !pc.loft.sids.contains(sid) && !pc.project.sweep_profile_contours(*sid).is_empty()) {
                    pc.loft.sids.push(sid);
                    pc.loft.cids.push(0);
                }
            }
        }
        // "from a sketch": the selected sketch is taken
        Some(qymcad_ui_state::PartTool::Hole) if pc.hole.mode == 1 => {
            if let qymcad_ui_state::Sel::Sketch(si) = *pc.sel {
                if let Some(id) = pc.project.sketches.get(si).map(|s| s.id) {
                    pc.hole.sketch = Some(id);
                }
            }
        }
        // the third direction only sits on top of the second (a full 3D grid)
        Some(qymcad_ui_state::PartTool::LinearPattern) if !pc.arr.two => pc.arr.three = false,
        _ => {}
    }
}

/// THE COMMAND IN HAND AS ITS BAR DESCRIBES IT.
pub(crate) fn describe(pc: &mut qymcad_ui_state::PartCtx, ctx: &egui::Context, kind: qymcad_ui_state::PartTool) -> qymcad_ui_state::Described {
    let mut says = Vec::new();
    let bar = match kind {
        qymcad_ui_state::PartTool::Extrude => extrude_opts(pc, &mut says, ctx),
        qymcad_ui_state::PartTool::Recognise => recognise_opts(pc, &mut says),
        qymcad_ui_state::PartTool::Revolve => revolve_opts(pc, ctx),
        qymcad_ui_state::PartTool::Sweep => sweep_opts(pc, &mut says),
        qymcad_ui_state::PartTool::Loft => loft_opts(pc, &mut says),
        qymcad_ui_state::PartTool::Chamfer => chamfer_opts(pc, &mut says),
        qymcad_ui_state::PartTool::Patch => vec![Opt::modes(OptKey::of("patch"), None, &["cmd-patch-flat", "cmd-patch-tangent"], usize::from(pc.opts.patch_tangent)), edges_readout(pc)],
        qymcad_ui_state::PartTool::Shell => vec![
            Opt::modes(OptKey::of("shell_side"), Some("cmd-thickness"), &["cmd-inwards", "cmd-outwards", "cmd-centred"], shell_side_at(pc.opts.shell_side)),
            Opt::Readout(qymcad_i18n::tr1("cmd-open-faces-n", "n", &pc.gsel.faces.len().to_string())),
        ],
        qymcad_ui_state::PartTool::Draft => draft_opts(pc, &mut says),
        qymcad_ui_state::PartTool::Thread => thread_opts(pc, &mut says),
        qymcad_ui_state::PartTool::Hole => hole_opts(pc, &mut says),
        qymcad_ui_state::PartTool::Prism => {
            says.push(Notice::info(qymcad_i18n::tr("cmd-dia-height-hint")));
            vec![]
        }
        k if k.is_a_primitive() => {
            says.push(Notice::info(qymcad_i18n::tr("cmd-sizes-hint")));
            vec![]
        }
        qymcad_ui_state::PartTool::Mirror => vec![
            Opt::switch(OptKey::of("keep"), qymcad_i18n::tr("cmd-with-original"), pc.opts.mirror_keep.lit()),
            Opt::Readout(qymcad_i18n::tr1("cmd-plane-is", "what", &plane_named(pc, pc.mirror.plane.as_ref()))),
        ],
        qymcad_ui_state::PartTool::LinearPattern => linear_array_opts(taken_line(pc), &mut says),
        qymcad_ui_state::PartTool::CircularPattern => circular_array_opts(&*pc.project, &*pc.arr, taken_line(pc), NoAxisPicked::WorldZ),
        qymcad_ui_state::PartTool::DatumPlane => {
            says.push(Notice::info(qymcad_i18n::tr("cmd-offset-field-hint")));
            vec![Opt::Readout(qymcad_i18n::tr1("cmd-from-is", "what", &plane_named(pc, pc.datum.plane_pick.as_ref())))]
        }
        qymcad_ui_state::PartTool::DatumPoint => datum_point_opts(pc, &mut says),
        qymcad_ui_state::PartTool::DatumAxis => datum_axis_opts(pc, &mut says),
        _ => {
            // what the command waits for, and what it has taken, in the bar where the picks are named
            let mut v = vec![Opt::Readout(cmd_hint(&*pc.armed, &*pc.gsel, &*pc.trim))];
            v.extend(taken_line(pc).map(Opt::Readout));
            v
        }
    };
    let gate = command_gate(pc, ctx).with_fields(&pc.cmd.params, pc.project);
    qymcad_ui_state::Described {
        head: qymcad_ui_state::Head { icon: ph::CUBE, name: command_title(kind), ends: qymcad_ui_state::Ends::ApplyCancel, after: qymcad_ui_state::AfterApply::Select },
        bar,
        gate,
        says,
        boxed: None,
    }
}

/// The sides a shell's wall goes to, in the order its bar shows them.
const SHELL_SIDES: [qymcad_core::feature::ShellSide; 3] = [qymcad_core::feature::ShellSide::Inward, qymcad_core::feature::ShellSide::Outward, qymcad_core::feature::ShellSide::Centred];

/// The place of `side` among the sides of a shell's bar.
fn shell_side_at(side: qymcad_core::feature::ShellSide) -> usize {
    SHELL_SIDES.iter().position(|s| *s == side).unwrap_or(0)
}

/// The operation of a command on the part's single body: Add (material into it, seeding it if it is the first), Cut or
/// Intersect. A part is one body, so the former "New" and "Join" are one "Add".
fn operation(op: u8) -> Opt {
    let at = match op {
        2 => 1,
        3 => 2,
        _ => 0,
    };
    Opt::modes(OptKey::of("op"), Some("cmd-operation"), &["cmd-add", "cmd-cut", "cmd-intersect"], at)
}

/// How many edges the command has taken - or, when the selection is described, what is recorded, in words: a count is a
/// snapshot of today, while "every edge of this face" will take a fifth one tomorrow.
fn edges_readout(pc: &qymcad_ui_state::PartCtx) -> Opt {
    Opt::Readout(match &pc.gsel.described {
        Some(_) => qymcad_i18n::tr1("expand-described", "what", &qymcad_i18n::tr("expand-face-edges")),
        None => qymcad_i18n::tr1("cmd-edges-n", "n", &pc.gsel.edges.len().to_string()),
    })
}

/// A plane picked for a command, in words: a base plane, a datum by its name, a face of a body.
fn plane_named(pc: &qymcad_ui_state::PartCtx, plane: Option<&qymcad_core::feature::SketchPlane>) -> String {
    use qymcad_core::feature::SketchPlane;
    match plane {
        Some(SketchPlane::World(bp)) => qymcad_i18n::tr1("cmd-world-plane", "plane", ["XY", "XZ", "YZ"][*bp as usize]),
        Some(SketchPlane::Datum(id)) => {
            pc.project.planes.iter().find(|p| p.id == *id).map(|p| qymcad_i18n::tr1("cmd-datum-named", "name", &qymcad_i18n::name(&p.name))).unwrap_or_else(|| qymcad_i18n::tr("cmd-datum"))
        }
        Some(SketchPlane::Face(b, _)) => qymcad_i18n::tr1("cmd-body-face-n", "b", &b.to_string()),
        None => qymcad_i18n::tr("cmd-pick-plane"),
    }
}

/// THE EXTENTS OF AN EXTRUSION, in the order of its bar.
const EXTENTS: [qymcad_ui_state::ExtentMode; 5] = {
    use qymcad_ui_state::ExtentMode as E;
    [E::Length, E::Symmetric, E::TwoSided, E::Through, E::ThroughBothWays]
};

fn extrude_opts(pc: &qymcad_ui_state::PartCtx, says: &mut Vec<Notice>, ctx: &egui::Context) -> Vec<Opt> {
    let through = if pc.feat.op != 0 { qymcad_ui_state::Can::Yes } else { qymcad_ui_state::Can::No };
    let extent = EXTENTS.iter().position(|e| *e == pc.cmd.extent);
    let mut v = vec![
        operation(pc.feat.op),
        Opt::Modes {
            key: OptKey::of("extent"),
            caption: Some(qymcad_i18n::tr("cmd-extent")),
            choices: vec![
                Mode::new(qymcad_i18n::tr("cmd-to-length")),
                Mode::new(qymcad_i18n::tr("cmd-symmetric")),
                Mode::new(qymcad_i18n::tr("cmd-two-sides")),
                // "through all" only applies to operations on a body: dimmed, not hidden, so the group keeps its place
                Mode::new(qymcad_i18n::tr("cmd-through-all")).can(through),
                Mode::new(qymcad_i18n::tr("cmd-through-both")).can(through),
            ],
            current: extent,
        },
        Opt::switch(OptKey::of("flip"), format!("{} {}", ph::ARROWS_DOWN_UP, qymcad_i18n::tr("cmd-flip-btn")), pc.feat.flip.lit()).hint(qymcad_i18n::tr("cmd-reverse-hint")),
    ];
    if pc.cmd.extent.two_sided() {
        // the second side's distance is an expression field at the geometry (a popup), not here
        says.push(Notice::info(qymcad_i18n::tr("cmd-second-side-note")));
    }
    v.extend(contours_again(pc, ctx));
    v
}

/// THE WAY BACK TO THE CONTOURS of an extrusion or a revolution, in its 3D step: how many are taken, and the button that
/// goes back to pick them in the flat half-sketcher - where contours are already being picked it is not offered.
fn contours_again(pc: &qymcad_ui_state::PartCtx, ctx: &egui::Context) -> Vec<Opt> {
    if pc.cmd.sketch.is_none() || !*pc.mode_3d {
        return vec![];
    }
    // THE KEY IN THE HINT FOLLOWS THE FOCUS STATE: "U" or "Alt+U". Otherwise the rule that a focused field needs Alt would
    // stay a secret, and someone who pressed `U` in a field and got nothing would not try a second time.
    let dc = qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: &*pc.project, active_path: pc.active_path };
    let hint = format!("{}  ({})", qymcad_i18n::tr("cmd-back-to-contours"), qymcad_ui_state::hotkey_hint(&dc, ctx, "part.contour-reselect"));
    vec![
        Opt::Readout(qymcad_i18n::tr1("cmd-profiles-n", "n", &pc.gsel.profiles.len().to_string())),
        Opt::press(OptKey::of("contours"), format!("{} {}", ph::POLYGON, qymcad_i18n::tr("cmd-pick-contours"))).hint(hint),
    ]
}

fn recognise_opts(pc: &qymcad_ui_state::PartCtx, says: &mut Vec<Notice>) -> Vec<Opt> {
    let mut v = Vec::new();
    // WHAT COMES OUT OF THE MESH: the exact surfaces found on it, or the mesh as it is - a polyhedron of its flat faces
    if pc.cmd.edit.is_none() {
        v.push(Opt::modes(OptKey::of("recognise"), Some("cmd-operation"), &["cmd-recognise-exact", "cmd-recognise-asis"], usize::from(pc.feat.op == 1)));
    }
    // A HEAVY MESH SAYS SO before Enter: how many triangles, and that the field "Simplify" makes it lighter
    if let Some(n) = pc.recognise.src.and_then(|src| pc.project.mesh_index(src)).map(|mi| pc.project.bodies[mi].mesh.tris.len()).filter(|&n| n >= HEAVY_MESH) {
        says.push(Notice::warning(qymcad_i18n::tr1("recognise-heavy", "n", &n.to_string())));
    }
    if pc.recognise.src.is_some() {
        match pc.recognise.ready(|f| f.counts) {
            Some(counts) => {
                let keys = ["recognise-planes", "recognise-cylinders", "recognise-cones", "recognise-spheres", "recognise-tori", "recognise-none", "recognise-free", "recognise-helices"];
                v.extend(keys.iter().enumerate().filter(|(k, _)| counts[*k] > 0).map(|(k, key)| Opt::Readout(qymcad_i18n::tr1(key, "n", &counts[k].to_string()))));
            }
            None => v.push(Opt::Readout(qymcad_i18n::tr("recognise-counting"))),
        }
    }
    v
}

fn revolve_opts(pc: &qymcad_ui_state::PartCtx, ctx: &egui::Context) -> Vec<Opt> {
    let mut v = vec![
        operation(pc.feat.op),
        // how the angle is laid out: to one side, or symmetrically about half of it; Flip reverses it
        Opt::modes(OptKey::of("angle"), Some("cmd-angle"), &["cmd-one-side", "cmd-symmetric"], usize::from(pc.cmd.extent.symmetric())),
        Opt::switch(OptKey::of("flip"), format!("{} {}", ph::ARROWS_DOWN_UP, qymcad_i18n::tr("cmd-flip-btn")), pc.feat.flip.lit()).hint(qymcad_i18n::tr("cmd-flip-angle-hint")),
    ];
    // the axis comes from a sketch CENTRELINE, from an edge, a cylindrical face or a datum, or from the sketch's own axes
    let axis_src = if pc.rev.axis_line != 0 {
        Some(qymcad_i18n::tr("cmd-sketch-centreline"))
    } else if pc.rev.axis_datum != 0 {
        Some(pc.project.datum_axes.iter().find(|d| d.id == pc.rev.axis_datum).map(|d| qymcad_i18n::name(&d.name)).unwrap_or_else(|| qymcad_i18n::tr("cmd-axis-lower")))
    } else {
        None
    };
    match axis_src {
        Some(name) => {
            v.push(Opt::Readout(format!("{} {}: {name}", ph::CROSSHAIR, qymcad_i18n::tr("cmd-axis"))));
            let pair = format!("{}/{}", qymcad_ui_state::sketch_axis_name(&*pc.project, pc.cmd.sketch, 0), qymcad_ui_state::sketch_axis_name(&*pc.project, pc.cmd.sketch, 1));
            v.push(Opt::press(OptKey::of("axis_reset"), format!("{} {pair}", ph::X)).hint(qymcad_i18n::tr("cmd-reset-axis-sketch")));
        }
        // The two axes of the sketch's own plane, named by where they point in the world: on the front plane the second
        // one IS the world Z, and calling it Y hid the axis a person came looking for.
        None => {
            let choices = (0u8..2)
                .map(|a| {
                    let name = qymcad_ui_state::sketch_axis_name(&*pc.project, pc.cmd.sketch, a);
                    Mode::new(name).hint(qymcad_i18n::tr1("cmd-revolve-about-axis", "axis", name))
                })
                .collect();
            v.push(Opt::Modes { key: OptKey::of("sketch_axis"), caption: Some(qymcad_i18n::tr("cmd-axis")), choices, current: Some(usize::from(pc.rev.axis)) });
        }
    }
    // A sketch CENTRELINE is a reliable choice made BY A BUTTON (with no click in 3D); with SEVERAL lines the choice is
    // made BY CLICKING the line itself in the sketch rather than from a list of numbered lines.
    let axis_lines = pc.cmd.sketch.map(|si| qymcad_ui_state::profile_axis_lines(&*pc.project, si)).unwrap_or_default();
    let si = pc.cmd.sketch.unwrap_or(0);
    match axis_lines.as_slice() {
        [] => {}
        [l] => {
            let name = qymcad_ui_state::axis_line_label(&*pc.project, si, *l, 1);
            v.push(Opt::switch(OptKey::of("axis_line"), format!("{} {name}", ph::LINE_SEGMENT), (pc.rev.axis_line == *l).lit()).hint(qymcad_i18n::tr("cmd-revolve-any-line-hint")));
        }
        lines => {
            let taken = lines.iter().position(|l| *l == pc.rev.axis_line).map(|i| qymcad_ui_state::axis_line_label(&*pc.project, si, pc.rev.axis_line, i + 1));
            v.push(Opt::pick(OptKey::of("axis_line_pick"), qymcad_i18n::tr("cmd-pick-axis-sketch"), taken, pc.rev.pick_line.lit()).hint(qymcad_i18n::tr("cmd-revolve-flat-hint")));
        }
    }
    // the regular 3D axis pick: a STRAIGHT edge of a body, a CYLINDRICAL face or a datum axis
    v.push(Opt::pick(OptKey::of("axis_3d"), qymcad_i18n::tr("cmd-pick-axis-3d"), None, pc.rev.pick_axis.lit()).hint(qymcad_i18n::tr("cmd-revolve-axis-hint")));
    v.extend(contours_again(pc, ctx));
    v
}

/// THE CONTOURS OF A SKETCH a pick chooses among: the candidates, the one picked, and the words of the pick.
struct ContourChoice {
    cands: Vec<Id>,
    picked: Id,
    words: &'static str,
    hint: &'static str,
}

/// THE PICK OF A CONTOUR of a sketch of several, through the half-sketcher: which one of how many, as a pick.
fn contour_pick(pc: &qymcad_ui_state::PartCtx, key: OptKey, slot: qymcad_ui_state::ContourSlot, choice: ContourChoice) -> Option<Opt> {
    if choice.cands.len() < 2 {
        return None;
    }
    let cur = choice.cands.iter().position(|c| *c == choice.picked).map_or(1, |i| i + 1);
    let word = qymcad_i18n::tr2(choice.words, "cur", &cur.to_string(), "total", &choice.cands.len().to_string());
    Some(Opt::pick(key, word, None, (pc.picking.contour() == Some(slot)).lit()).hint(qymcad_i18n::tr(choice.hint)))
}

fn sweep_opts(pc: &qymcad_ui_state::PartCtx, says: &mut Vec<Notice>) -> Vec<Opt> {
    let prof_name = pc.project.sketches.iter().find(|s| s.id == pc.sweep.prof_sid).map(|s| qymcad_i18n::name(&s.name)).unwrap_or_else(|| "-".into());
    let mut v = vec![operation(pc.feat.op), Opt::Readout(qymcad_i18n::tr1("cmd-profile-is", "name", &prof_name))];
    let prof = ContourChoice { cands: pc.project.sweep_profile_contours(pc.sweep.prof_sid), picked: pc.sweep.prof_cid, words: "cmd-pick-contour-n", hint: "cmd-open-sketch-flat" };
    v.extend(contour_pick(pc, OptKey::of("profile_contour"), qymcad_ui_state::ContourSlot::SweepProfile, prof));
    // THE PATH: edges of a body picked in the view, or the sketch picked in the tree
    let path = if pc.gsel.edges.is_empty() {
        pc.project.sketches.iter().find(|s| s.id == pc.sweep.path_sid).filter(|_| pc.sweep.path_sid != 0).map(|s| qymcad_i18n::name(&s.name))
    } else {
        Some(qymcad_i18n::tr1("cmd-path-edges-n", "n", &pc.gsel.edges.len().to_string()))
    };
    v.push(Opt::pick(OptKey::of("path"), qymcad_i18n::tr("cmd-pick-path-sketch"), path, pc.sweep.pick_path.lit()).hint(qymcad_i18n::tr("cmd-sweep-pick-path")));
    let path = ContourChoice { cands: pc.project.sweep_path_contours(pc.sweep.path_sid), picked: pc.sweep.path_cid, words: "cmd-pick-contour-n", hint: "cmd-open-path-flat" };
    if pc.gsel.edges.is_empty() {
        v.extend(contour_pick(pc, OptKey::of("path_contour"), qymcad_ui_state::ContourSlot::SweepPath, path));
    }
    says.push(Notice::info(qymcad_i18n::tr("cmd-sweep-auto-hint")));
    v
}

fn loft_opts(pc: &qymcad_ui_state::PartCtx, says: &mut Vec<Notice>) -> Vec<Opt> {
    let mut v = Vec::new();
    // the sections in order: the number and the name, a contour switch, and the way to take it out
    for (i, sid) in pc.loft.sids.iter().enumerate() {
        let name = pc.project.sketches.iter().find(|s| s.id == *sid).map(|s| qymcad_i18n::name(&s.name)).unwrap_or_else(|| "-".into());
        v.push(Opt::Readout(format!("{}:{name}", i + 1)));
        let section =
            ContourChoice { cands: pc.project.sweep_profile_contours(*sid), picked: pc.loft.cids.get(i).copied().unwrap_or(0), words: "cmd-pick-contour-short", hint: "cmd-open-section-flat" };
        v.extend(contour_pick(pc, OptKey::at("section_contour", i), qymcad_ui_state::ContourSlot::LoftSection(i), section));
        v.push(Opt::press(OptKey::at("section_remove", i), ph::X).hint(qymcad_i18n::tr("cmd-remove-section")));
    }
    v.push(Opt::switch(OptKey::of("add_section"), format!("{} {}", ph::PLUS, qymcad_i18n::tr("cmd-add-section")), pc.loft.pick.lit()).hint(qymcad_i18n::tr("cmd-loft-pick-hint")));
    // the kind of surface between the sections, and the kind of result: a separate body, a boolean with the active body,
    // or a surface through the sections - the same loft, not closed into a body
    v.push(Opt::modes(OptKey::of("faces"), Some("cmd-faces"), &["cmd-smooth", "cmd-ruled"], usize::from(pc.loft.ruled)));
    v.push(Opt::modes(OptKey::of("result"), Some("cmd-result"), &["cmd-add", "cmd-cut", "cmd-union", "cmd-intersection", "cmd-surface"], usize::from(pc.loft.result)));
    if pc.loft.result != 0 && pc.loft.result != 4 {
        let dc = qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: &*pc.project, active_path: pc.active_path };
        says.push(if qymcad_ui_state::current_body(&dc).is_some() {
            Notice::info(qymcad_i18n::tr("cmd-bool-with-active"))
        } else {
            Notice::refusal(qymcad_i18n::tr("cmd-no-active-body"))
        });
    }
    v.push(Opt::Readout(qymcad_i18n::tr1("cmd-sections-n", "n", &pc.loft.sids.len().to_string())));
    v
}

/// The modes of a chamfer in the order its bar shows them.
const CHAMFER_MODES: [qymcad_core::feature::ChamferMode; 3] = [qymcad_core::feature::ChamferMode::Symmetric, qymcad_core::feature::ChamferMode::TwoDist, qymcad_core::feature::ChamferMode::DistAngle];

fn chamfer_opts(pc: &qymcad_ui_state::PartCtx, says: &mut Vec<Notice>) -> Vec<Opt> {
    use qymcad_core::feature::ChamferMode;
    let mut v = vec![Opt::modes(
        OptKey::of("chamfer"),
        Some("cmd-mode"),
        &["cmd-symmetric", "cmd-two-distances", "cmd-leg-angle"],
        CHAMFER_MODES.iter().position(|m| *m == pc.chamfer.mode).unwrap_or(0),
    )];
    if pc.chamfer.mode != ChamferMode::Symmetric {
        v.push(Opt::switch(OptKey::of("side"), format!("{} {}", ph::ARROWS_DOWN_UP, qymcad_i18n::tr("cmd-side-btn")), pc.chamfer.flip.lit()).hint(qymcad_i18n::tr("cmd-swap-ref-face")));
        // picking the reference face by hand: a click on a face in 3D. It overrides "Side" for the edges adjacent to that
        // face; letting it go gives the side back.
        let taken = (pc.chamfer.ref_face != 0).then(String::new);
        v.push(Opt::pick(OptKey::of("ref_face"), format!("{} {}", ph::CUBE, qymcad_i18n::tr("cmd-ref-face")), taken, pc.chamfer.pick_ref.lit()).hint(qymcad_i18n::tr("cmd-chamfer-ref-hint")));
        says.push(Notice::info(qymcad_i18n::tr("cmd-asym-note")));
    }
    v.push(edges_readout(pc));
    v
}

fn draft_opts(pc: &qymcad_ui_state::PartCtx, says: &mut Vec<Notice>) -> Vec<Opt> {
    says.push(Notice::info(qymcad_i18n::tr("cmd-angle-field-hint")));
    let taken = (pc.draft.neutral != 0).then(String::new);
    vec![
        Opt::Readout(qymcad_i18n::tr1("cmd-draft-faces-n", "n", &pc.gsel.faces.len().to_string())),
        Opt::pick(OptKey::of("neutral"), format!("{} {}", ph::CUBE, qymcad_i18n::tr("cmd-neutral-face")), taken, pc.draft.pick_neutral.lit()).hint(qymcad_i18n::tr("cmd-draft-neutral-hint")),
        Opt::switch(OptKey::of("flip"), format!("{} {}", ph::ARROWS_DOWN_UP, qymcad_i18n::tr("cmd-flip-back")), pc.draft.flip.lit()).hint(qymcad_i18n::tr("cmd-flip-draft-hint")),
    ]
}

/// The standards of a thread, by the words of their choices (a plain name, or a key of the catalogue): the geometry of
/// each is computed by the model core.
/// The words of the forms of a thread in the order its bar shows them: the letters of a standard as they are written,
/// the words of the catalogue for the rest.
fn thread_form_word(at: usize) -> String {
    match at {
        0 => "M".into(),
        1 => "Tr".into(),
        2 => "ACME".into(),
        3 => "Rd".into(),
        4 => qymcad_i18n::tr("cmd-stop"),
        _ => qymcad_i18n::tr("cmd-custom"),
    }
}

fn thread_opts(pc: &qymcad_ui_state::PartCtx, says: &mut Vec<Notice>) -> Vec<Opt> {
    use qymcad_core::thread::ThreadStandard as TS;
    let kind = vec![
        Mode::new(qymcad_i18n::tr("cmd-thread-external")).hint(qymcad_i18n::tr("cmd-on-cylinder")),
        Mode::new(qymcad_i18n::tr("cmd-thread-internal")).hint(qymcad_i18n::tr("cmd-in-hole")),
    ];
    let mut v = vec![
        Opt::Modes { key: OptKey::of("thread_kind"), caption: Some(qymcad_i18n::tr("cmd-kind")), choices: kind, current: Some(usize::from(pc.thread.internal)) },
        // The number of starts is a dimension of the part as well, and it must be parametric: a two-start thread driven
        // by a global variable is an ordinary thing.
        Opt::Number { key: OptKey::of("thread_starts"), label: "cmd-starts", value: f64::from(pc.thread.starts.max(1)), limits: qymcad_ui_state::Limits::between(1.0..=12.0) },
        Opt::switch(OptKey::of("left"), format!("{} {}", ph::ARROWS_COUNTER_CLOCKWISE, qymcad_i18n::tr("cmd-left-thread")), pc.thread.left.lit()).hint(qymcad_i18n::tr("cmd-thread-left")),
        // THE MODE: a thread (a groove) or an AUGER (a helical ribbon outwards)
        Opt::Modes {
            key: OptKey::of("auger"),
            caption: None,
            choices: vec![
                Mode::new(format!("{} {}", ph::SPIRAL, qymcad_i18n::tr("cmd-thread-btn"))),
                Mode::new(format!("{} {}", ph::SPIRAL, qymcad_i18n::tr("cmd-auger-btn"))).hint(qymcad_i18n::tr("cmd-auger-hint")),
            ],
            current: Some(usize::from(pc.thread.auger)),
        },
    ];
    if pc.thread.auger {
        says.push(Notice::info(qymcad_i18n::tr("cmd-auger-fields-hint")));
    } else {
        let standards = [TS::MetricIso, TS::TrapezoidalTr, TS::Acme, TS::RoundRd, TS::Buttress, TS::Custom];
        let choices = standards.iter().enumerate().map(|(at, std)| Mode::new(thread_form_word(at)).hint(qymcad_i18n::tr(std.label()))).collect();
        v.push(Opt::Modes { key: OptKey::of("standard"), caption: Some(qymcad_i18n::tr("cmd-standard")), choices, current: Some(usize::from(pc.thread.form)) });
        thread_says(pc, says);
    }
    // what is taken - or what to take - is named in the bar itself, where the picks are named
    v.push(Opt::Readout(if pc.thread.edge == 0 {
        qymcad_i18n::tr("cmd-thread-pick-hint")
    } else {
        qymcad_i18n::tr1("cmd-actual-diameter", "d", &qymcad_i18n::num(pc.thread.radius * 2.0, 1))
    }));
    v
}

/// WHAT A THREAD SAYS before it is made: the geometry of the size chosen, what the mating part needs, and what of the
/// profile does not fit the pitch.
fn thread_says(pc: &qymcad_ui_state::PartCtx, says: &mut Vec<Notice>) {
    let spec = thread_spec(&*pc.cmd, *pc.thread);
    let g = spec.geometry();
    says.push(Notice::info(qymcad_i18n::trn(
        "cmd-thread-geom",
        &[
            ("pitch", &qymcad_i18n::num(g.pitch, 2)),
            ("d2", &qymcad_i18n::num(g.pitch_d, 2)),
            ("d3", &qymcad_i18n::num(g.minor_d, 2)),
            ("depth", &qymcad_i18n::num(g.depth, 2)),
        ],
    )));
    // WHAT THE MATING PART NEEDS, said out loud: a person must see what diameter of shaft or hole this thread wants to make
    // its counterpart - a table does not know about the fit that was typed in here.
    let (own, mate) = spec.blank_diameters();
    let key = if pc.thread.internal { "cmd-thread-mate-internal" } else { "cmd-thread-mate-external" };
    says.push(Notice::info(qymcad_i18n::tr2(key, "own", &qymcad_i18n::num(own, 2), "mate", &qymcad_i18n::num(mate, 2))));
    // THE CLEARANCE THAT DOES NOT FIT is named: it saturates against the pitch without a word, and two different numbers
    // typed in then give one and the same body. What is left over is said as a DIAMETER correction (`radial_relief`
    // converts the clearance along the flank; twice it, since a diameter has two sides).
    if let Some((asked, given)) = spec.fit_overflow() {
        says.push(Notice::refusal(qymcad_i18n::trn(
            "cmd-thread-fit-capped",
            &[("asked", &qymcad_i18n::num(asked, 2)), ("given", &qymcad_i18n::num(given, 3)), ("rest", &qymcad_i18n::num(spec.radial_relief() * 2.0, 2))],
        )));
    }
    // A PROFILE THAT DOES NOT FIT THE PITCH is said out loud, with the numbers to change: the passes overlap and leave
    // flat plates that mate with nothing.
    if let Some((width, max_depth, min_pitch)) = spec.profile_overflow() {
        says.push(Notice::refusal(qymcad_i18n::trn(
            "cmd-thread-too-wide",
            &[
                ("width", &qymcad_i18n::num(width, 2)),
                ("pitch", &qymcad_i18n::num(g.pitch, 2)),
                ("depth", &qymcad_i18n::num(max_depth, 2)),
                ("minpitch", &qymcad_i18n::num(min_pitch, 2)),
            ],
        )));
    }
}

fn hole_opts(pc: &qymcad_ui_state::PartCtx, says: &mut Vec<Notice>) -> Vec<Opt> {
    let mut v = vec![
        Opt::modes(OptKey::of("placement"), Some("cmd-placement"), &["cmd-by-face", "cmd-by-sketch"], usize::from(pc.hole.mode)),
        Opt::modes(OptKey::of("hole_kind"), Some("cmd-kind"), &["cmd-simple", "cmd-counterbore", "cmd-countersink"], usize::from(pc.hole.kind)),
    ];
    if pc.hole.mode == 1 {
        // "from a sketch": the sketch taken and the number of its marker points, and a flip of the normal
        let n = pc.hole.sketch.map(|sid| pc.project.sketch_isolated_points(sid).len()).unwrap_or(0);
        v.push(Opt::Readout(match pc.hole.sketch {
            Some(sid) => pc
                .project
                .sketches
                .iter()
                .find(|s| s.id == sid)
                .map(|s| qymcad_i18n::tr2("cmd-sketch-points", "name", &qymcad_i18n::name(&s.name), "n", &n.to_string()))
                .unwrap_or_else(|| qymcad_i18n::tr("cmd-sketch-unset")),
            None => qymcad_i18n::tr("cmd-pick-sketch-points"),
        }));
        v.push(Opt::switch(OptKey::of("flip"), qymcad_i18n::tr("cmd-flip"), pc.hole.flip.lit()));
    } else {
        v.extend(taken_line(pc).map(Opt::Readout));
        says.push(Notice::info(qymcad_i18n::tr("cmd-hole-face-hint")));
    }
    v
}

/// The three directions a pattern repeats along, as a group of choices.
pub(crate) fn direction(key: OptKey, caption: Option<&str>, dir: u8) -> Opt {
    Opt::Modes { key, caption: caption.map(qymcad_i18n::tr), choices: ["X", "Y", "Z"].into_iter().map(Mode::new).collect(), current: Some(usize::from(dir)) }
}

/// THE BAR OF A LINEAR PATTERN, of bodies or of parts: what it repeats (`taken`). Its directions - the axis, the copies
/// and the step of each, the second and the third switched on by opening them - are sections of its box
/// (`layouts::directions`); ghosts preview the copies.
pub(super) fn linear_array_opts(taken: Option<String>, says: &mut Vec<Notice>) -> Vec<Opt> {
    says.push(Notice::info(qymcad_i18n::tr("cmd-pitch-field-hint")));
    taken.map(Opt::Readout).into_iter().collect()
}

/// WHAT A CIRCULAR PATTERN TURNS ABOUT while no axis of its own is picked: world Z for a pattern of bodies, one of the
/// three axes of the assembly, chosen in the bar, for a pattern of parts.
pub(super) enum NoAxisPicked {
    WorldZ,
    OneOf(u8),
}

/// THE OPTIONS OF A CIRCULAR PATTERN, of bodies or of parts: what it repeats (`taken`), the axis - CLICK-PICKED, as the
/// mirror plane is, rather than chosen from a combo box - and whether it is a full turn; the angle is a field at the
/// geometry otherwise.
pub(super) fn circular_array_opts(project: &Project, arr: &qymcad_ui_state::ArrayParams, taken: Option<String>, unpicked: NoAxisPicked) -> Vec<Opt> {
    let mut v: Vec<Opt> = taken.map(Opt::Readout).into_iter().collect();
    let axis = project.datum_axes.iter().find(|d| d.id == arr.axis).filter(|_| arr.axis != 0).map(|d| qymcad_i18n::name(&d.name));
    let picked = axis.is_some();
    let ask = match &unpicked {
        NoAxisPicked::WorldZ => qymcad_i18n::tr("cmd-axis-world-z"),
        NoAxisPicked::OneOf(_) => qymcad_i18n::tr("cmd-pick-axis"),
    };
    v.push(Opt::pick(OptKey::of("axis"), ask, axis, arr.axis_pick.lit()).hint(qymcad_i18n::tr("cmd-revolve-pick-axis3d")));
    if let (false, NoAxisPicked::OneOf(of)) = (picked, unpicked) {
        v.push(direction(OptKey::of("carr_axis"), None, of));
    }
    v
}

fn datum_point_opts(pc: &qymcad_ui_state::PartCtx, says: &mut Vec<Notice>) -> Vec<Opt> {
    // A datum POINT: X/Y/Z coordinates, or associatively at a vertex (by a click)
    let choices = vec![Mode::new(qymcad_i18n::tr("cmd-coordinates")), Mode::new(qymcad_i18n::tr("cmd-to-vertex")).hint(qymcad_i18n::tr("cmd-vertex-assoc-hint"))];
    let mut v = vec![Opt::Modes { key: OptKey::of("point_method"), caption: Some(qymcad_i18n::tr("cmd-method")), choices, current: Some(usize::from(pc.datum.pt_mode)) }];
    if pc.datum.pt_mode == 1 {
        v.push(Opt::Readout(if pc.datum.pt_vert.is_some() { format!("{} {}", qymcad_i18n::tr("cmd-vertex-picked"), ph::CHECK) } else { qymcad_i18n::tr("cmd-pick-vertex") }));
    } else {
        says.push(Notice::info(qymcad_i18n::tr("cmd-xyz-hint")));
    }
    v
}

/// The ways of a datum axis in the order its bar shows them, by the number the command keeps for each.
const AXIS_MODES: [u8; 3] = [0, 2, 1];

fn datum_axis_opts(pc: &qymcad_ui_state::PartCtx, says: &mut Vec<Notice>) -> Vec<Opt> {
    // A datum AXIS: by a click on an edge or a face, by two points, or by hand (origin and direction at the geometry)
    let mut v = vec![Opt::Modes {
        key: OptKey::of("axis_method"),
        caption: Some(qymcad_i18n::tr("cmd-method")),
        choices: ["cmd-by-edge-face", "cmd-two-points", "cmd-manual"].into_iter().map(|w| Mode::new(qymcad_i18n::tr(w))).collect(),
        current: AXIS_MODES.iter().position(|m| *m == pc.datum.axis_mode),
    }];
    match pc.datum.axis_mode {
        0 => v.push(Opt::Readout(if pc.datum.axis_ref.is_some() { format!("{} {}", qymcad_i18n::tr("cmd-axis-picked"), ph::CHECK) } else { qymcad_i18n::tr("cmd-pick-edge-cyl") })),
        2 => v.push(Opt::Readout(qymcad_i18n::tr1("cmd-points-n", "n", &pc.datum.axis_pts.len().to_string()))),
        _ => says.push(Notice::info(qymcad_i18n::tr("cmd-origin-dir-hint"))),
    }
    v
}

/// WHAT WAS DONE TO AN OPTION OF THE COMMAND IN HAND, done to the command.
pub(crate) fn act(pc: &mut qymcad_ui_state::PartCtx, did: Did) {
    match did {
        Did::Chose { key, index } => chose(pc, key, index),
        Did::Switched { key } => switched(pc, key),
        Did::Pressed { key } => pressed(pc, key),
        Did::Cleared { key } => cleared(pc, key),
        Did::Typed { key, value } => {
            if key.name == "thread_starts" {
                pc.thread.starts = (value.round() as u32).max(1);
            }
        }
        // no command of a part has a text typed
        Did::Retyped { .. } | Did::Entered { .. } => {}
    }
}

/// A choice of a group of the bar taken.
fn chose(pc: &mut qymcad_ui_state::PartCtx, key: OptKey, index: usize) {
    use qymcad_ui_state::ExtentMode as E;
    let to_u8 = |n: usize, most: u8| u8::try_from(n).unwrap_or(most).min(most);
    match key.name {
        "op" => pc.feat.op = [0, 2, 3][index.min(2)],
        "extent" => pc.cmd.extent = EXTENTS[index.min(EXTENTS.len() - 1)],
        "angle" => pc.cmd.extent = if index == 1 { E::Symmetric } else { E::Length },
        "recognise" => pc.feat.op = u8::from(index == 1),
        "sketch_axis" => pc.rev.axis = u8::from(index == 1),
        "faces" => pc.loft.ruled = index == 1,
        "result" => pc.loft.result = to_u8(index, 4),
        "chamfer" => {
            pc.chamfer.mode = CHAMFER_MODES[index.min(2)];
            // the meaning of field d2 changed (mm <-> degrees), so both the label AND the value at the geometry are
            // updated (45 mm as a second leg, or 1.5 deg as an angle, are both meaningless, hence the mode's default)
            let def = if pc.chamfer.mode == qymcad_core::feature::ChamferMode::DistAngle { 45.0 } else { 1.5 };
            if let Some(p) = pc.cmd.params.iter_mut().find(|p| p.key == "d2") {
                p.set_label(qymcad_ui_state::chamfer_d2_label(pc.chamfer.mode));
                (p.val, p.txt) = (def, format!("{def:.2}"));
            }
        }
        "patch" => pc.opts.patch_tangent = index == 1,
        "shell_side" => pc.opts.shell_side = SHELL_SIDES[index.min(2)],
        "thread_kind" => pc.thread.internal = index == 1,
        "auger" => {
            pc.thread.auger = index == 1;
            qymcad_ui_state::set_thread_params(&mut *pc.cmd, *pc.thread); // the modes have different fields
        }
        "standard" => {
            pc.thread.form = to_u8(index, 5);
            sync_custom_params(&mut *pc.cmd, *pc.thread); // "custom" opens the angle and the depth, the others hide them
        }
        "placement" => pc.hole.mode = u8::from(index == 1),
        "hole_kind" => pc.hole.kind = to_u8(index, 2),
        "dir" => pc.arr.dir = to_u8(index, 2),
        "dir2" => pc.arr.dir2 = to_u8(index, 2),
        "dir3" => pc.arr.dir3 = to_u8(index, 2),
        "point_method" => {
            pc.datum.pt_mode = u8::from(index == 1);
            sync_datum_point_params(&*pc.armed, &mut *pc.cmd, &*pc.datum);
        }
        "axis_method" => pc.datum.axis_mode = AXIS_MODES[index.min(2)],
        _ => {}
    }
}

/// A switch of the bar turned over, or a pick begun or given up.
fn switched(pc: &mut qymcad_ui_state::PartCtx, key: OptKey) {
    match (key.name, pc.armed.part_tool()) {
        ("flip", Some(qymcad_ui_state::PartTool::Extrude)) => {
            let f = !pc.feat.flip;
            pc.feat.set_flip(f); // the direction was set by hand
        }
        ("flip", Some(qymcad_ui_state::PartTool::Revolve)) => pc.feat.flip = !pc.feat.flip,
        ("flip", Some(qymcad_ui_state::PartTool::Draft)) => pc.draft.flip = !pc.draft.flip,
        ("flip", Some(qymcad_ui_state::PartTool::Hole)) => pc.hole.flip = !pc.hole.flip,
        ("axis_line", _) => {
            let l = pc.cmd.sketch.map(|si| qymcad_ui_state::profile_axis_lines(&*pc.project, si)).unwrap_or_default().first().copied().unwrap_or(0);
            pc.rev.axis_line = if pc.rev.axis_line == l { 0 } else { l };
            if pc.rev.axis_line != 0 {
                pc.rev.axis_datum = 0;
            }
        }
        ("axis_line_pick", _) => {
            pc.rev.pick_line = !pc.rev.pick_line;
            if pc.rev.pick_line {
                pc.rev.pick_axis = false;
                *pc.mode_3d = false; // the flat half-sketcher: the lines are visible and clickable
                pc.view.initialized = false;
                *pc.status = Notice::info(qymcad_i18n::tr("cmd-revolve-pick-line"));
            }
        }
        ("axis_3d", _) => {
            pc.rev.pick_axis = !pc.rev.pick_axis;
            if pc.rev.pick_axis {
                pc.rev.pick_line = false;
                // the axis candidates (datum axes, edges) are drawn and hit-tested ONLY in 3D: staying in the flat
                // half-sketcher would make the pick do nothing - there would be nothing to click
                *pc.mode_3d = true;
                pc.view.initialized = false;
                refresh_axis_edges(pc); // the straight edges of every visible body are axis candidates
                *pc.status = Notice::info(qymcad_i18n::tr("cmd-pick-axis-hint"));
            }
        }
        ("profile_contour", _) => begin_contour_pick(pc, qymcad_ui_state::ContourSlot::SweepProfile, pc.sweep.prof_sid),
        ("path_contour", _) => begin_contour_pick(pc, qymcad_ui_state::ContourSlot::SweepPath, pc.sweep.path_sid),
        ("path", _) => pc.sweep.pick_path = !pc.sweep.pick_path,
        ("section_contour", _) => {
            if let Some(sid) = pc.loft.sids.get(key.at).copied() {
                begin_contour_pick(pc, qymcad_ui_state::ContourSlot::LoftSection(key.at), sid);
            }
        }
        ("add_section", _) => {
            pc.loft.pick = !pc.loft.pick;
            pc.loft.pick_last = None; // the pick is on, so the current selection may be taken on the next edge
        }
        ("side", _) => pc.chamfer.flip = !pc.chamfer.flip,
        ("ref_face", _) => pc.chamfer.pick_ref = !pc.chamfer.pick_ref,
        ("neutral", _) => pc.draft.pick_neutral = !pc.draft.pick_neutral,
        ("left", _) => pc.thread.left = !pc.thread.left,
        ("keep", _) => pc.opts.mirror_keep = !pc.opts.mirror_keep,
        ("dir2_on" | "dir3_on", _) => qymcad_ui_state::switch_direction(pc.arr, pc.arr.dir, qymcad_ui_state::MoreDirection::of(key.name)),
        ("axis", _) => {
            pc.arr.axis_pick = !pc.arr.axis_pick;
            if pc.arr.axis_pick {
                refresh_axis_edges(pc); // the straight edges of EVERY visible body, as axis candidates
                *pc.status = Notice::info(qymcad_i18n::tr("cmd-array-axis-hint"));
            }
        }
        _ => {}
    }
}

/// A button of the bar pressed.
fn pressed(pc: &mut qymcad_ui_state::PartCtx, key: OptKey) {
    match key.name {
        "contours" => enter_contour_reselect(pc),
        "axis_reset" => (pc.rev.axis_datum, pc.rev.axis_line, pc.rev.pick_axis) = (0, 0, false),
        "section_remove" if key.at < pc.loft.sids.len() => {
            pc.loft.sids.remove(key.at);
            pc.loft.cids.remove(key.at);
        }
        _ => {}
    }
}

/// What a pick of the bar has taken, let go.
fn cleared(pc: &mut qymcad_ui_state::PartCtx, key: OptKey) {
    match key.name {
        "axis_line_pick" => pc.rev.axis_line = 0,
        "path" => (pc.sweep.path_sid, pc.sweep.path_cid) = (0, 0),
        "ref_face" => (pc.chamfer.ref_face, pc.chamfer.pick_ref) = (0, false),
        "neutral" => (pc.draft.neutral, pc.draft.pick_neutral) = (0, false),
        "axis" => (pc.arr.axis, pc.arr.axis_pick) = (0, false),
        _ => {}
    }
}
