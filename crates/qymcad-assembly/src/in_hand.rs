//! THE TOOLS OF AN ASSEMBLY IN HAND, as the frame drives them (`qymcad_ui_state::tool_in_hand`): a joint made or reopened,
//! its bar and its box at the glyph, and the tools that pick: the tangent, the width, the group, grounding, an anchor, a
//! relation, a joint. The bar, the box, the keys and the words are the frame's.

use qymcad_ui_state::{Described, Did, FramedTool, NotApplied, Notice, Stepped};
use super::*;

/// THE TOOL OF AN ASSEMBLY IN HAND, if one is: a joint made or reopened first, then a tool that picks - in the assembly,
/// in 3D.
pub fn in_hand<'a>(jc: &'a mut qymcad_ui_state::JointCtx<'_>) -> Option<Box<dyn FramedTool + 'a>> {
    if !matches!(jc.workbench, qymcad_ui_state::Workbench::Assembly) || !jc.mode_3d {
        return None;
    }
    if let Some(jid) = jc.joint.edit {
        match jc.project.joints.iter().find(|x| x.id == jid).cloned() {
            Some(j) => return Some(Box::new(JointEdit { jc, j, seen: Seen::default(), asks: Vec::new() })),
            None => jc.joint.edit = None, // the joint is gone (deleted) - leave the edit
        }
    }
    let tool = AsmTool::of(jc.joint)?;
    Some(Box::new(AsmTools { jc, tool, own: Vec::new(), free: Vec::new(), asks: Vec::new() }))
}

/// WHAT THE BOX OF A JOINT SAW AS THE FRAME BEGAN: the context the keyboard is asked of, the joints before the widgets of
/// this frame - a step of undo opens from them - and whether an option of the box changed the document.
#[derive(Default)]
struct Seen {
    ctx: Option<egui::Context>,
    before: Vec<qymcad_core::feature::Joint>,
    changed: bool,
}

/// A JOINT MADE OR REOPENED: its bar - the kind, the anchors re-picked, the axis flipped, the roles swapped - and its box
/// at the glyph with its values. Apply keeps what was done, Cancel and Esc take it away.
struct JointEdit<'a, 'b> {
    jc: &'a mut qymcad_ui_state::JointCtx<'b>,
    j: qymcad_core::feature::Joint,
    seen: Seen,
    /// what the frame kept of the names typed under the values
    asks: Vec<qymcad_ui_state::FieldAsk>,
}

impl FramedTool for JointEdit<'_, '_> {
    fn describe(&mut self, ctx: &egui::Context) -> Described {
        let jc = &mut *self.jc;
        qymcad_ui_state::joint_edit_remember(jc.edits, jc.project, jc.joint);
        let mut d = joint_edit_described(jc, &self.j);
        let place = popup_place(ctx, jc, &self.j).unwrap_or(qymcad_ui_state::Place::by(qymcad_ui_state::By::ViewCorner));
        joint_fields_follow(ctx, jc.project, jc.joint, &JointBox { j: &self.j, head: &d.head });
        let mut spec = qymcad_ui_state::BoxSpec::by(place);
        spec.layout = joint_layout(jc, &self.j);
        // A VALUE OF A JOINT IS NAMED AS A DIMENSION IS, in the row under its field: a formula reads it by the name
        spec.names = shown_slots(&self.j)
            .into_iter()
            .map(|slot| {
                let target = qymcad_core::model::DimTarget::Feature { node: self.j.id, key: SLOT_KEYS[slot].to_string() };
                let had = qymcad_i18n::name(&jc.project.name_of_target(&target));
                qymcad_ui_state::FieldName::of(SLOT_KEYS[slot], had, Some(target))
            })
            .collect();
        d.boxed = Some(spec);
        self.seen = Seen { ctx: Some(ctx.clone()), before: jc.project.joints.clone(), changed: false };
        d
    }

    fn fields(&mut self) -> &mut Vec<qymcad_ui_state::CmdParam> {
        &mut self.jc.joint.fields.fields
    }

    fn look(&self) -> qymcad_ui_state::DrawCtx<'_> {
        self.jc.draw()
    }

    fn took(&mut self, did: Did) {
        let jc = &mut *self.jc;
        let jid = self.j.id;
        match did {
            Did::Chose { key, index } if key.name == "kind" => {
                if qymcad_ui_state::change_joint_kind(jc.project, jc.status, jid, JOINT_KINDS[index.min(JOINT_KINDS.len() - 1)]) {
                    qymcad_ui_state::mark_dirty_for_rebuild(&mut jc.rebuild()); // the document is marked; the planner does the counting
                }
            }
            Did::Switched { key } if key.name == "repick" => {
                let repick = jc.joint.edit_repick.map(|(_, is_b)| usize::from(is_b));
                jc.joint.edit_repick = if repick == Some(key.at) { None } else { Some((jid, key.at == 1)) };
            }
            Did::Pressed { key } if key.name == "flip_axis" => joint_hud_flip_axis(jc, jid),
            Did::Pressed { key } if key.name == "swap_roles" => joint_hud_swap_roles(jc, jid),
            d => self.seen.changed |= joint_opt_act(jc, &self.j, d),
        }
    }

    fn apply(&mut self) -> Result<(), NotApplied> {
        // the names typed under the values go in with the joint, in the step of undo its editing opened
        for ask in &self.asks {
            let target = qymcad_core::model::DimTarget::Feature { node: self.j.id, key: ask.field.clone() };
            if ask.renamed(&qymcad_i18n::name(&self.jc.project.name_of_target(&target))) {
                qymcad_ui_state::give_name(self.jc.project, target, &ask.name);
            }
        }
        joint_edit_keep(self.jc);
        Ok(())
    }

    fn take_asks(&mut self, asks: Vec<qymcad_ui_state::FieldAsk>) {
        self.asks = asks;
    }

    fn cancel(&mut self) {
        joint_edit_leave(self.jc);
    }

    /// A pick being made is let go first - the axis of an anchor, an anchor picked anew - and the editing stays.
    fn back(&mut self) -> Stepped {
        let jc = &mut *self.jc;
        if jc.joint.axis_pick.take().is_some() {
            *jc.status = Notice::info(qymcad_i18n::tr("in-axis-pick-cancelled"));
            return Stepped::Back;
        }
        if jc.joint.edit_repick.take().is_some() {
            *jc.status = Notice::info(qymcad_i18n::tr("in-anchor-swap-cancelled"));
            return Stepped::Back;
        }
        Stepped::NothingLeft
    }

    /// The values typed into the box are written into the joint; where the typing begins and ends is where its step of
    /// undo opens and closes.
    fn after(&mut self) {
        let Some(ctx) = self.seen.ctx.clone() else { return };
        let jc = &mut *self.jc;
        let head = joint_edit_head(&self.j);
        joint_typing_marks(&ctx, &jc.joint.fields.fields, &head);
        let taken = joint_fields_take(jc.project, &jc.joint.fields.fields, &self.j);
        if let Some(e) = &taken.refused {
            *jc.status = Notice::refusal(qymcad_i18n::error_words::expr_error_text(e));
        }
        let changed = self.seen.changed || taken.changed;
        joint_values_step(&ctx, jc.edits, jc.project, std::mem::take(&mut self.seen.before), changed);
        if changed {
            qymcad_ui_state::mark_dirty_for_rebuild(&mut jc.rebuild()); // the document is marked; the planner does the counting
        }
    }
}

/// A TOOL OF THE ASSEMBLY THAT PICKS: the tangent, the width, the group, grounding, an anchor, a relation, a joint.
struct AsmTools<'a, 'b> {
    jc: &'a mut qymcad_ui_state::JointCtx<'b>,
    tool: AsmTool,
    /// no field of its own: the box shows the numbers of the bar alone, not the fields of the joint edited last
    own: Vec<qymcad_ui_state::CmdParam>,
    /// the numbers of the bar - the ratio of a relation - named as parameters
    free: Vec<&'static str>,
    asks: Vec<qymcad_ui_state::FieldAsk>,
}

impl FramedTool for AsmTools<'_, '_> {
    fn describe(&mut self, _ctx: &egui::Context) -> Described {
        let mut d = asm_tool_described(self.jc, &self.tool);
        self.free = qymcad_ui_state::name_bar_numbers(&mut d);
        d
    }

    /// No tool that picks has fields of its own: the box holds the numbers of its bar.
    fn fields(&mut self) -> &mut Vec<qymcad_ui_state::CmdParam> {
        &mut self.own
    }

    fn take_asks(&mut self, asks: Vec<qymcad_ui_state::FieldAsk>) {
        self.asks = asks;
    }

    fn bar_texts(&mut self) -> Option<&mut std::collections::HashMap<&'static str, String>> {
        Some(&mut *self.jc.bar_exprs)
    }

    fn look(&self) -> qymcad_ui_state::DrawCtx<'_> {
        self.jc.draw()
    }

    fn took(&mut self, did: Did) {
        asm_tool_act(self.jc, did);
    }

    /// The tangent, the width, the group and the relation are made of what was picked; grounding, an anchor and a joint
    /// take their clicks as they come, and Enter has nothing more to make.
    fn apply(&mut self) -> Result<(), NotApplied> {
        let jc = &mut *self.jc;
        match self.tool {
            AsmTool::Tangent => {
                if !jc.joint.tangent_pick.as_ref().is_some_and(|s| s.len() >= 2) {
                    return Err(NotApplied::Refused(qymcad_i18n::tr("j-tangent-pick")));
                }
                tangent_finish(jc, true);
                Ok(())
            }
            AsmTool::Width => width_pick_confirm(jc),
            AsmTool::Group => group_pick_confirm(jc),
            AsmTool::Relation => {
                relation_pick_confirm(jc)?;
                if self.asks.iter().any(|a| !a.name.trim().is_empty()) {
                    qymcad_ui_state::begin_edit(&mut *jc.edits, &*jc.project, qymcad_i18n::tr("sk-driver-step"));
                    qymcad_ui_state::name_free_values(jc.project, &self.asks, &self.free);
                    qymcad_ui_state::commit_edit_if_changed(&mut jc.rebuild());
                }
                Ok(())
            }
            AsmTool::Ground | AsmTool::Anchor | AsmTool::Joint => Ok(()),
        }
    }

    /// The tool is put down; a tangent already made of two picks is taken away with it.
    fn cancel(&mut self) {
        let jc = &mut *self.jc;
        if self.tool == AsmTool::Tangent && jc.joint.tangent_pick.as_ref().is_some_and(|s| s.len() >= 2) {
            tangent_finish(jc, false);
            return;
        }
        self.put_down();
        let off = match self.tool {
            AsmTool::Tangent => "j-tangent-off",
            AsmTool::Width => "j-width-off",
            AsmTool::Group => "j-group-off",
            AsmTool::Ground => "j-ground-off",
            AsmTool::Anchor => "j-conn-off",
            AsmTool::Relation => "j-relation-off",
            AsmTool::Joint => "j-cancelled",
        };
        *self.jc.status = Notice::info(qymcad_i18n::tr(off));
    }

    /// What the tool picked is let go: grounding, an anchor and a joint take their clicks as they come, and the tangent,
    /// the width, the group and the relation have already let go of what they made of their picks.
    fn put_down(&mut self) {
        let j = &mut *self.jc.joint;
        match self.tool {
            AsmTool::Tangent => j.tangent_pick = None,
            AsmTool::Width => j.width_pick = None,
            AsmTool::Group => j.group_pick = None,
            AsmTool::Ground => j.ground_pick = false,
            AsmTool::Anchor => j.conn_pick = false,
            AsmTool::Relation => j.relation_pick = None,
            AsmTool::Joint => (j.pick_faces, j.pick_first) = (false, None),
        }
    }
}
