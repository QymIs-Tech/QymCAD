//! THE BARS OF A JOINT: editing it, making it, the tool bar - each described as data and drawn by the frame.

use qymcad_core::feature::{JointKind, RelationKind};
use qymcad_ui_state::{AfterApply, AsLit, Described, Did, Ends, Gate, Head, Notice, Opt, OptKey};
use super::*;

/// Every kind of joint, in the order the bars offer them. THERE IS ONE SET OF KINDS: a joint states the degrees of
/// freedom, and that is all; the rigid one is among them.
pub(crate) const JOINT_KINDS: [JointKind; 8] =
    [JointKind::Rigid, JointKind::Revolute, JointKind::Slider, JointKind::Cylindrical, JointKind::Planar, JointKind::Ball, JointKind::PinSlot, JointKind::Parallel];

/// Every kind of relation, in the order its bar offers them.
const RELATION_KINDS: [RelationKind; 4] = [RelationKind::Gear, RelationKind::RackPinion, RelationKind::Screw, RelationKind::Linear];

/// The kinds of joint as a group of the bar, `now` in force.
fn joint_kinds(key: OptKey, now: JointKind) -> Opt {
    let words: Vec<&str> = JOINT_KINDS.iter().map(|k| k.label()).collect();
    Opt::modes(key, Some("j-kind"), &words, JOINT_KINDS.iter().position(|k| *k == now).unwrap_or(0))
}

/// THE SORT OF ANCHOR IS INFERRED UNDER THE CURSOR, and the bar says so: a face gives its centre, an edge its midpoint.
/// A professional CAD has no switches of face, edge and vertex - while the sort was declared in advance, a click
/// regularly produced an anchor other than the one intended.
fn anchor_inferred() -> Opt {
    Opt::Readout(format!("{}: {}", qymcad_i18n::tr("j-anchor"), qymcad_i18n::tr("j-anchor-inferred")))
}

/// The name the bar of JOINT EDIT MODE gives joint `j`; the frame tells the fields of its box apart by it.
pub(crate) fn joint_edit_head(j: &qymcad_core::feature::Joint) -> Head {
    // a joint being made or reopened is a command, as every tool is: Apply keeps what was done, Cancel and Esc take it away
    Head { icon: ph::LINK, name: qymcad_i18n::tr1("jt-editing", "name", &qymcad_i18n::name(&j.name)), ends: Ends::ApplyCancel, after: AfterApply::Select }
}

/// The top bar of JOINT EDIT MODE (a double click on the glyph): the joint's name, its kind, the anchors A and B - each
/// re-picked on the fly - and the console that flips the axis and swaps the roles. The values (angle, offset, limits,
/// side) live in its box at the glyph.
pub(crate) fn joint_edit_described(jc: &qymcad_ui_state::JointCtx, j: &qymcad_core::feature::Joint) -> Described {
    let desc = |cid: Id| jc.project.connector(cid).map(|c| qymcad_ui_state::anchor_desc(&jc.draw(), &c.anchor)).unwrap_or_default();
    let desc_a = desc(j.a);
    let desc_b = desc(j.b);
    let repick = jc.joint.edit_repick.map(|(_, is_b)| usize::from(is_b));
    let mut says = Vec::new();
    // A JOINT WITHOUT AN ANCHOR SAYS SO OUT LOUD. The solver drops such joints from the problem: the assembly looks
    // assembled and the parts do not move. The reason comes from the single source (`joint_faults`) the solver report
    // and the list of joints speak from as well.
    if let Some((_, why)) = jc.project.joint_faults().iter().find(|(id, _)| *id == j.id) {
        says.push(Notice::error(format!("{} {}", qymcad_i18n::tr(why), qymcad_i18n::tr(&format!("{why}-hint")))));
    }
    if let Some(side) = repick {
        let t = qymcad_i18n::tr("j-place-lower");
        says.push(Notice::info(qymcad_i18n::tr2("jt-click-new", "what", &t, "side", ["A", "B"][side])));
    }
    // THE JOINT CONSOLE: flip the main axis, and swap the roles of the parts. Both fix what almost never comes out right
    // the first time and used to be correctable only by recreating the joint - with its drivers, limits and name.
    let bar = vec![
        joint_kinds(OptKey::of("kind"), j.kind),
        anchor_inferred(),
        Opt::switch(OptKey::at("repick", 0), format!("{} A: {desc_a}", ph::MAGNET), (repick == Some(0)).lit()).hint(qymcad_i18n::tr("jt-swap-anchor")),
        Opt::switch(OptKey::at("repick", 1), format!("{} B: {desc_b}", ph::MAGNET), (repick == Some(1)).lit()).hint(qymcad_i18n::tr("jt-swap-anchor")),
        Opt::press(OptKey::of("flip_axis"), format!("{} {}", ph::ARROWS_DOWN_UP, qymcad_i18n::tr("jt-flip-axis"))).hint(qymcad_i18n::tr("jt-flip-axis-hint")),
        Opt::press(OptKey::of("swap_roles"), format!("{} {}", ph::SWAP, qymcad_i18n::tr("jt-swap-roles"))).hint(qymcad_i18n::tr("jt-swap-roles-hint")),
    ];
    Described { head: joint_edit_head(j), bar, gate: Gate::Open, says, boxed: None }
}

/// FINISH A JOINT JUST MADE: kept, its open operation closes as one step «Joint»; not kept, the operation is rolled back
/// and the document is as it was before the first pick. Nothing happens when no joint is being made.
pub fn joint_create_finish(jc: &mut qymcad_ui_state::JointCtx, keep: bool) {
    if !std::mem::take(&mut jc.joint.creating) {
        return;
    }
    if keep {
        qymcad_ui_state::close_edit(jc.edits, jc.project); // closed, not committed: committing drops the asked rebuild
    } else {
        qymcad_ui_state::abort_edit(&mut jc.rebuild());
    }
    qymcad_ui_state::exit_joint_edit(jc.joint, jc.status);
    // a joint kept leaves the selection in hand, as every command applied does (`AfterApply::Select`)
    if keep {
        *jc.status = Notice::info(qymcad_i18n::tr("j-created"));
    }
}

/// APPLY WITH THE JOINT CONSOLE OPEN: a joint just made is kept as one step, an edited one is left as edited.
pub fn joint_edit_keep(jc: &mut qymcad_ui_state::JointCtx) {
    if jc.joint.creating {
        joint_create_finish(jc, true);
    } else {
        qymcad_ui_state::exit_joint_edit(jc.joint, jc.status);
    }
}

/// Esc OR CANCEL WITH THE JOINT CONSOLE OPEN: a joint just made is taken away, an edited one comes back as it stood when
/// it was opened.
pub fn joint_edit_leave(jc: &mut qymcad_ui_state::JointCtx) {
    if jc.joint.creating {
        joint_create_finish(jc, false);
        return;
    }
    if let Some(from) = jc.joint.edit_from.take().filter(|f| Some(f.joint) == jc.joint.edit) {
        qymcad_ui_state::joint_edit_cancel(&mut jc.rebuild(), from);
    }
    qymcad_ui_state::exit_joint_edit(jc.joint, jc.status);
}

/// THE ASSEMBLY TOOL IN HAND as its bar describes it, and how it is put down.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum AsmTool {
    Tangent,
    Width,
    Group,
    Ground,
    Anchor,
    Relation,
    Joint,
}

impl AsmTool {
    /// The tool of the assembly in hand now; none when nothing is.
    pub(crate) fn of(joint: &qymcad_ui_state::JointCommand) -> Option<AsmTool> {
        if joint.tangent_pick.is_some() {
            Some(AsmTool::Tangent)
        } else if joint.width_pick.is_some() {
            Some(AsmTool::Width)
        } else if joint.group_pick.is_some() {
            Some(AsmTool::Group)
        } else if joint.ground_pick {
            Some(AsmTool::Ground)
        } else if joint.conn_pick {
            Some(AsmTool::Anchor)
        } else if joint.relation_pick.is_some() {
            Some(AsmTool::Relation)
        } else if joint.pick_faces {
            Some(AsmTool::Joint)
        } else {
            None
        }
    }
}

/// THE ASSEMBLY TOOL AS ITS BAR DESCRIBES IT: what it asks for, what it has taken, its options.
pub(crate) fn asm_tool_described(jc: &qymcad_ui_state::JointCtx, tool: &AsmTool) -> Described {
    let head = |icon, name: &str, ends| Head { icon, name: qymcad_i18n::tr(name), ends, after: AfterApply::Select };
    // what is pointed at, not typed: Enter before it has nothing to make
    let asking = |head: Head, mut bar: Vec<Opt>, ask: String| {
        bar.push(Opt::Readout(ask.clone()));
        Described { head, bar, gate: Gate::Waiting(ask), says: vec![], boxed: None }
    };
    let open = |head: Head, bar: Vec<Opt>| Described { head, bar, gate: Gate::Open, says: vec![], boxed: None };
    match tool {
        // what to pick; once the second surface is picked the condition stands as a preview, and Apply keeps it
        AsmTool::Tangent => {
            let sel = jc.joint.tangent_pick.as_deref().unwrap_or_default();
            let head = head(ph::CIRCLE_HALF_TILT, "j-tangent-made", Ends::ApplyCancel);
            match sel.len() {
                0 => asking(head, vec![], qymcad_i18n::tr("j-tangent-pick")),
                1 => asking(head, vec![], qymcad_i18n::tr("j-tangent-second")),
                _ => open(head, vec![Opt::Readout(qymcad_i18n::tr("j-tangent-made-ok"))]),
            }
        }
        // how many are taken; Enter before enough of them are asks for what is missing - two walls and the part between
        // them, two parts at least to fasten
        AsmTool::Width => {
            let n = jc.joint.width_pick.as_ref().map_or(0, |s| s.len());
            let head = head(ph::ARROWS_OUT_LINE_HORIZONTAL, "j-width-made", Ends::ApplyCancel);
            let taken = vec![Opt::Readout(qymcad_i18n::tr1("j-width-picked", "n", &n.to_string()))];
            if n < 3 {
                asking(head, taken, qymcad_i18n::tr("j-width-need-three"))
            } else {
                open(head, taken)
            }
        }
        AsmTool::Group => {
            let n = jc.joint.group_pick.as_ref().map_or(0, |s| s.len());
            let head = head(ph::SELECTION_ALL, "j-group-made", Ends::ApplyCancel);
            let taken = vec![Opt::Readout(qymcad_i18n::tr1("j-group-picked", "n", &n.to_string()))];
            if n < 2 {
                asking(head, taken, qymcad_i18n::tr("j-group-need-two"))
            } else {
                open(head, taken)
            }
        }
        // grounding is done by the click itself, so Enter has nothing to make; Done puts it down. What the clicks have
        // grounded is named in the bar and not only in the status line.
        AsmTool::Ground => {
            let project = &*jc.project;
            let grounded: Vec<String> = project.components.iter().filter(|c| c.parent.is_some() && project.is_grounded(c.id)).map(|c| qymcad_i18n::name(&c.name)).collect();
            let bar = if grounded.is_empty() { vec![] } else { vec![Opt::Readout(qymcad_i18n::tr1("j-grounded-list", "names", &grounded.join(", ")))] };
            asking(head(ph::ANCHOR, "jt-ground-btn", Ends::Close), bar, qymcad_i18n::tr("j-ground-click"))
        }
        // the anchor is laid by the click
        AsmTool::Anchor => asking(head(ph::CROSSHAIR, "j-conn-new", Ends::ApplyCancel), vec![anchor_inferred()], qymcad_i18n::tr("j-conn-pick")),
        // the kind, the number, the direction; Apply once the mates it needs are picked
        AsmTool::Relation => {
            let pick = jc.joint.relation_pick.clone().unwrap_or_default();
            let need = relation_picks_needed(pick.kind);
            let have = if need == 1 { pick.picks.len() / 2 } else { pick.picks.len() };
            let picked = qymcad_i18n::tr2("j-relation-picked", "n", &have.to_string(), "need", &need.to_string());
            let words: Vec<&str> = RELATION_KINDS.iter().map(|k| k.label()).collect();
            // THE NUMBER MEANS DIFFERENT THINGS FOR DIFFERENT KINDS, and the caption must say so: for gears and linear
            // relations it is a ratio, for a rack and a screw it is the travel per turn in millimetres
            let caption = qymcad_i18n::tr(if pick.kind.value_is_per_turn() { "j-relation-per-turn" } else { "j-relation-ratio" });
            let bar = vec![
                Opt::modes(OptKey::of("relation_kind"), Some("j-kind"), &words, RELATION_KINDS.iter().position(|k| *k == pick.kind).unwrap_or(0)),
                Opt::Number { key: OptKey::of("relation_ratio"), caption, value: pick.value, limits: qymcad_ui_state::Limits::between(-10000.0..=10000.0) },
                Opt::switch(OptKey::of("relation_reverse"), qymcad_i18n::tr("j-relation-reverse"), pick.reversed.lit()),
            ];
            let head = head(ph::GEAR_SIX, "j-relation-btn", Ends::ApplyCancel);
            if have >= need {
                let mut d = open(head, bar);
                d.bar.push(Opt::Readout(picked));
                d
            } else {
                asking(head, bar, picked)
            }
        }
        // WHAT IS BEING ASKED FOR is A PLACE ON THE PART: the sort of anchor is inferred under the cursor. BY ORIGINS is
        // a fourth sort of anchor, a toggle of the bar - the same picking by a click, the same cancel. "KEEP AS IT
        // STANDS" is chosen here rather than after creation (`new_as_built`). The values of the joint are typed in
        // the box at the joint just made.
        AsmTool::Joint => {
            let target = qymcad_i18n::tr(if jc.joint.anchor_mode == 3 { "j-origin-lower" } else { "j-place-lower" });
            let ask = qymcad_i18n::tr1(if jc.joint.pick_first.is_some() { "jt-click-b" } else { "jt-click-a" }, "what", &target);
            let bar = vec![
                joint_kinds(OptKey::of("joint_kind"), jc.joint.new_kind),
                anchor_inferred(),
                Opt::switch(OptKey::of("by_origin"), qymcad_i18n::tr("j-origin"), (jc.joint.anchor_mode == 3).lit()).hint(qymcad_i18n::tr("j-origin-hint")),
                Opt::switch(OptKey::of("as_built"), qymcad_i18n::tr("j-as-built"), jc.joint.new_as_built.lit()).hint(qymcad_i18n::tr("jt-as-built-hint")),
            ];
            // a joint is laid by its second pick: Enter before it has nothing to make
            asking(head(ph::MAGNET, "jt-joint-btn", Ends::ApplyCancel), bar, ask)
        }
    }
}

/// WHAT WAS DONE TO AN OPTION OF THE ASSEMBLY TOOL IN HAND, done to the tool.
pub(crate) fn asm_tool_act(jc: &mut qymcad_ui_state::JointCtx, did: Did) {
    match did {
        Did::Chose { key, index } if key.name == "joint_kind" => {
            // THE SORT OF ANCHOR IS NOT CHOSEN BY THE KIND: a change of kind used to move it silently, and the next click
            // landed somewhere other than where it was aimed
            jc.joint.new_kind = JOINT_KINDS[index.min(JOINT_KINDS.len() - 1)];
            jc.joint.anchor_mode = 0;
        }
        Did::Chose { key, index } if key.name == "relation_kind" => {
            if let Some(p) = jc.joint.relation_pick.as_mut() {
                let value = p.value;
                p.set(RELATION_KINDS[index.min(RELATION_KINDS.len() - 1)], value);
            }
        }
        Did::Typed { key, value } if key.name == "relation_ratio" => {
            if let Some(p) = jc.joint.relation_pick.as_mut() {
                let kind = p.kind;
                p.set(kind, value);
            }
        }
        Did::Switched { key } => match key.name {
            "relation_reverse" => {
                if let Some(p) = jc.joint.relation_pick.as_mut() {
                    p.reversed = !p.reversed;
                }
            }
            // A TOGGLE, NOT A ONE-WAY CHOICE: it used to be possible to enter this and impossible to leave
            "by_origin" => jc.joint.anchor_mode = if jc.joint.anchor_mode == 3 { 0 } else { 3 },
            "as_built" => jc.joint.new_as_built = !jc.joint.new_as_built,
            _ => {}
        },
        _ => {}
    }
}
