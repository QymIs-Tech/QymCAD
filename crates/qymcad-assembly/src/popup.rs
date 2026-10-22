//! THE POPUP OF A JOINT at the geometry, and its values stepped.

use qymcad_ui_state::Notice;
use super::*;

/// Where the anchors of a mate stood in the world when the box was laid last, kept between frames.
#[derive(Clone, Copy)]
pub(crate) struct PopupPin {
    pub(crate) joint: Id,
    pub(crate) anchors: Anchors,
}

/// THE TWO ANCHORS OF A MATE in the world, in the context worked in.
#[derive(Clone, Copy)]
pub(crate) struct Anchors {
    pub(crate) a: [f64; 3],
    pub(crate) b: [f64; 3],
}

/// Where the anchors of mate `j` stand in the world, in the context worked in.
fn anchors_of(jc: &qymcad_ui_state::JointCtx, j: &qymcad_core::feature::Joint) -> Option<Anchors> {
    let here = qymcad_ui_state::current_ctx_id(jc.active_path, jc.project);
    let at = |cid: Id| {
        let conn = jc.project.connector(cid)?;
        let fr = jc.project.connector_frame(conn)?;
        Some(qymcad_core::feature::apply12(&jc.project.relative_transform(conn.owner, here), fr.origin))
    };
    Some(Anchors { a: at(j.a)?, b: at(j.b)? })
}

/// WHAT THE BOX OF MATE `j` STANDS BY: its two anchors - except while the run of that same mate goes on: then the anchors
/// as they stood in the world when the run began, its side held. The box follows the view, not the motion it causes
/// itself: a run of the travel of a cylindrical mate carries anchor B along the axis, and a box by the moving anchors
/// travelled 21-41 px over one second of the run, the button that stops the run riding along under the pointer. Pinned
/// on the screen instead, it stood still while the view moved, and the mate went from under it.
pub(crate) fn popup_place(ctx: &egui::Context, jc: &qymcad_ui_state::JointCtx, j: &qymcad_core::feature::Joint) -> Option<qymcad_ui_state::Place> {
    let id = egui::Id::new("joint_edit_popup_pin");
    let running = jc.joint_anim.as_ref().is_some_and(|a| a.joint == j.id);
    let pinned = ctx.data(|d| d.get_temp::<PopupPin>(id)).filter(|p| p.joint == j.id && running).map(|p| p.anchors);
    let anchors = pinned.or_else(|| anchors_of(jc, j))?;
    ctx.data_mut(|d| d.insert_temp(id, PopupPin { joint: j.id, anchors }));
    let place = qymcad_ui_state::Place::by(qymcad_ui_state::By::Pair { a: anchors.a, b: anchors.b });
    Some(if running { place.held() } else { place })
}

/// The keys of the fields of the three slots of a joint, in the order of the slots.
pub(crate) const SLOT_KEYS: [&str; 3] = ["angle", "offset", "offset2"];

/// The two anchors of a joint as its box names them: the key of the tag and the prefix of the keys of their fields.
const ANCHORS: [AnchorTag; 2] = [AnchorTag { tag: "j-tag-a", prefix: "a" }, AnchorTag { tag: "j-tag-b", prefix: "b" }];

/// ONE ANCHOR OF A JOINT in its box: the catalogue key of its tag, and the prefix of the keys of its fields.
struct AnchorTag {
    tag: &'static str,
    prefix: &'static str,
}

/// THE BOX OF A JOINT AT THE GEOMETRY (a double click on the glyph), as the frame knows it: the joint, and the head of its
/// bar the frame tells the fields of its box apart by. Its values - the angle, the offset and the second offset - are
/// fields that take a formula, each pinned or let go by its lock; the side, driving from the root, the run of a degree,
/// "keep as it stands"; the anchors - the attachment point, the turn, the axis, the offsets; the limits of the free
/// degrees. A single click does NOT open the box - it only shows the gizmo of the freedoms.
pub(crate) struct JointBox<'a> {
    pub(crate) j: &'a qymcad_core::feature::Joint,
    pub(crate) head: &'a qymcad_ui_state::Head,
}

/// The slots of joint `j` shown in its box: a rigid joint has an angle as well - no freedom, but the turn about its
/// axis is a value of it, and without the field there was nothing to turn a fastened part with.
pub(crate) fn shown_slots(j: &qymcad_core::feature::Joint) -> Vec<usize> {
    use qymcad_core::feature::JointKind;
    let has = [
        matches!(j.kind, JointKind::Rigid | JointKind::Revolute | JointKind::Cylindrical | JointKind::PinSlot | JointKind::Ball | JointKind::Planar),
        matches!(j.kind, JointKind::Rigid | JointKind::Slider | JointKind::Cylindrical | JointKind::PinSlot | JointKind::Planar | JointKind::Ball),
        matches!(j.kind, JointKind::Planar | JointKind::Ball),
    ];
    (0..3).filter(|s| has[*s]).collect()
}

/// The value slot `slot` of joint `j` holds: what drives it, or - free - where the part ended up.
fn slot_value(j: &qymcad_core::feature::Joint, slot: usize) -> f64 {
    j.drive[slot].unwrap_or(match slot {
        0 => j.angle,
        1 => j.offset,
        _ => j.offset2,
    })
}

/// THE FIELDS OF JOINT `j` AS THE DOCUMENT HOLDS THEM: its slots - with their formulas - the turn and the offsets of
/// both anchors, and the limits switched on of its free degrees.
fn joint_fields(project: &Project, j: &qymcad_core::feature::Joint) -> Vec<qymcad_ui_state::CmdParam> {
    use qymcad_ui_state::CmdParam;
    let mut v = Vec::new();
    for slot in shown_slots(j) {
        let w = joint_slot_words(j.kind, slot);
        let value = slot_value(j, slot);
        let mut p = CmdParam::new(w.name, SLOT_KEYS[slot], value, -100000.0, 100000.0);
        p.txt = project.feat_dim(j.id, SLOT_KEYS[slot]).filter(|t| !t.is_empty()).map_or_else(|| qymcad_core::expr::fmt_num(value), str::to_string);
        v.push(p);
    }
    // THE ANCHORS OF A JOINT - tuning them IS the answer to "why did the part end up in the wrong place": the turn of the
    // secondary axis and an offset along all three axes, as an anchor almost never sits at the centre of a face exactly
    for (a, cid) in ANCHORS.iter().zip([j.a, j.b]) {
        let Some(c) = project.connector(cid) else { continue };
        v.push(field_of(CmdParam::new("j-turn", &format!("{}-turn", a.prefix), c.rot_deg, -3600.0, 3600.0), a.tag));
        for (k, label) in ["j-offset-x", "j-offset-y", "j-offset-z"].into_iter().enumerate() {
            v.push(field_of(CmdParam::new(label, &format!("{}-{k}", a.prefix), c.offset_xyz[k], -100000.0, 100000.0), a.tag));
        }
    }
    let free = j.kind.free_slots();
    for slot in (0..3).filter(|s| free[*s]) {
        let name = joint_slot_words(j.kind, slot).name;
        if let Some(v0) = j.limit_min[slot] {
            v.push(field_of(CmdParam::new("j-min", &format!("min{slot}"), v0, -100000.0, 100000.0), name));
        }
        if let Some(v1) = j.limit_max[slot] {
            v.push(field_of(CmdParam::new("j-max", &format!("max{slot}"), v1, -100000.0, 100000.0), name));
        }
    }
    v
}

/// A field told apart by whose value it is, its number written plainly.
fn field_of(p: qymcad_ui_state::CmdParam, whose: &'static str) -> qymcad_ui_state::CmdParam {
    let mut p = p.of(whose);
    p.txt = qymcad_core::expr::fmt_num(p.val);
    p
}

/// THE FIELDS OF THE BOX FOLLOW THE JOINT: made anew for another joint, and a value changed elsewhere - the solve, a
/// drag of the gizmo, a run - written into its field; a field being typed in, or holding the value the joint holds,
/// keeps its text - a formula stays a formula.
pub(crate) fn joint_fields_follow(ctx: &egui::Context, project: &Project, joint: &mut qymcad_ui_state::JointCommand, at: &JointBox) {
    let j = at.j;
    let want = joint_fields(project, j);
    let mine = joint.fields.joint == j.id;
    let focused = ctx.memory(|m| m.focused());
    let kept = std::mem::take(&mut joint.fields.fields);
    joint.fields = qymcad_ui_state::JointFields {
        joint: j.id,
        fields: want
            .into_iter()
            .map(|w| match kept.iter().find(|p| mine && p.key == w.key) {
                Some(p) if focused == Some(qymcad_ui_state::tool_field_id(at.head, &p.key)) || p.txt.trim() == w.txt.trim() => p.clone(),
                _ => w,
            })
            .collect(),
    };
}

/// WHERE THE TYPING IN THE BOX BEGINS AND ENDS - a field taking the keyboard and giving it up - is where its step of
/// undo opens and closes (`joint_values_step`): the field writes the value while it is typed.
pub(crate) fn joint_typing_marks(ctx: &egui::Context, fields: &[qymcad_ui_state::CmdParam], head: &qymcad_ui_state::Head) {
    let focused = ctx.memory(|m| m.focused());
    let now = fields.iter().any(|p| focused == Some(qymcad_ui_state::tool_field_id(head, &p.key)));
    let id = egui::Id::new("a field of the joint box holds the keyboard");
    let was = ctx.data(|d| d.get_temp::<bool>(id)).unwrap_or(false);
    ctx.data_mut(|d| d.insert_temp(id, now));
    let mark = match (was, now) {
        (false, true) => "joint_values_began",
        (true, false) => "joint_values_ended",
        _ => return,
    };
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(mark), true));
}

/// THE VALUES TYPED INTO THE BOX, written into the joint: a slot typed is pinned at its value and keeps its formula, as
/// a parameter drives it later; an anchor moves; a limit is set. Only a field whose text differs from what the joint
/// holds is written: the number a field shows is rounded, and taking it back would pin a free degree where the solve
/// left it, a hair off, every frame.
pub(crate) fn joint_fields_take(project: &mut Project, fields: &[qymcad_ui_state::CmdParam], j: &qymcad_core::feature::Joint) -> Taken {
    let held = joint_fields(project, j);
    let vars = project.param_map();
    let typed = |key: &str| {
        let p = fields.iter().find(|p| p.key == key)?;
        let shown = held.iter().find(|w| w.key == key)?;
        (p.txt.trim() != shown.txt.trim()).then(|| qymcad_ui_state::field_value(p, &vars).ok()).flatten().map(|v| Typed { value: v, text: p.txt.trim().to_string() })
    };
    let (mut changed, mut refused) = (false, None);
    for slot in shown_slots(j) {
        let Some(t) = typed(SLOT_KEYS[slot]) else { continue };
        // WRITTEN THROUGH THE ONE DOOR OF A FEATURE'S NUMBER, a plain number standing as the number: the last thing typed
        // drives, a formula as a parameter drives it later. What the door refuses - a formula coming back to the name of
        // this very number - pins nothing and is said.
        let at = qymcad_core::model::FeatNumber { node: j.id, key: SLOT_KEYS[slot] };
        if let Err(e) = project.write_feat_formula(&at, &t.text, qymcad_core::model::PlainNumber::Stands) {
            refused.get_or_insert(e);
            continue;
        }
        project.set_joint_drive(j.id, slot, Some(t.value));
        changed = true;
    }
    for (a, cid) in ANCHORS.iter().zip([j.a, j.b]) {
        let Some(c) = project.connector(cid).cloned() else { continue };
        let rot = typed(&format!("{}-turn", a.prefix));
        let off: [Option<Typed>; 3] = std::array::from_fn(|k| typed(&format!("{}-{k}", a.prefix)));
        if rot.is_some() || off.iter().any(Option::is_some) {
            let offset: [f64; 3] = std::array::from_fn(|k| off[k].as_ref().map_or(c.offset_xyz[k], |t| t.value));
            changed |= project.set_connector_placement(cid, c.point, rot.map_or(c.rot_deg, |t| t.value), offset);
        }
    }
    let min: Vec<Option<Typed>> = (0..3).map(|s| typed(&format!("min{s}"))).collect();
    let max: Vec<Option<Typed>> = (0..3).map(|s| typed(&format!("max{s}"))).collect();
    if let Some(jj) = project.joints.iter_mut().find(|x| x.id == j.id) {
        for slot in 0..3 {
            changed |= set_limit(&mut jj.limit_min[slot], min[slot].as_ref());
            changed |= set_limit(&mut jj.limit_max[slot], max[slot].as_ref());
        }
    }
    Taken { changed, refused }
}

/// WHAT THE FIELDS OF THE BOX OF A JOINT CAME TO IN A FRAME: whether the joint changed, and the first formula the
/// document refused.
pub(crate) struct Taken {
    pub(crate) changed: bool,
    pub(crate) refused: Option<qymcad_core::errors::ExprError>,
}

/// A VALUE TYPED INTO A FIELD OF THE BOX: what it works out to, and the text it was typed as.
struct Typed {
    value: f64,
    text: String,
}

/// A limit switched on takes the value typed in its field; whether it changed.
fn set_limit(limit: &mut Option<f64>, typed: Option<&Typed>) -> bool {
    match (limit.as_mut(), typed) {
        (Some(v), Some(t)) => {
            *v = t.value;
            true
        }
        _ => false,
    }
}

/// THE LAYOUT OF THE BOX OF JOINT `j`, as a mate dialog lays it: at the top the degrees of the joint - the value and its
/// lock - with the side and driving from the root; a section for each part - where its anchor sits, its axis, its turn,
/// its offset along X, Y and Z on one line; the limits of the free degrees, folded; at the foot the run of a degree and
/// "keep as it stands".
pub(crate) fn joint_layout(jc: &qymcad_ui_state::JointCtx, j: &qymcad_core::feature::Joint) -> qymcad_ui_state::Layout {
    use qymcad_core::feature::{AnchorRef, JointKind};
    use qymcad_ui_state::{AsLit, Folded, Opt, OptKey, Row, Section};
    let project = &*jc.project;
    let mut top = Vec::new();
    // A VALUE IS PINNED OR FREE: free, the field shows where the part ended up; typed, the degree is pinned; the lock
    // lets it go again
    for slot in shown_slots(j) {
        let w = joint_slot_words(j.kind, slot);
        let (icon, hint) = if j.drive[slot].is_some() { (ph::LOCK, "j-value-set") } else { (ph::LOCK_OPEN, "j-value-free") };
        let lock = Opt::switch(OptKey::at("drive", slot), icon, j.drive[slot].is_some().lit()).hint(qymcad_i18n::tr(hint));
        top.push(Row::new(qymcad_i18n::tr(w.row)).field(SLOT_KEYS[slot]).unit(w.unit).opt(lock));
    }
    if project.joint_home(j).is_some_and(|h| h != project.root) {
        top.push(Row::new("").opt(Opt::switch(OptKey::of("global"), qymcad_i18n::tr("j-drive-from-root"), j.global.lit()).hint(qymcad_i18n::tr("j-expose-hint"))));
    }
    // THE SIDE OF THE JOINT GOES THROUGH THE CORE'S DOOR: what is shown is the side IN EFFECT, and a click asks for the
    // opposite one. A tick on the field itself was cleared by the next solve, which writes its own answer there.
    let face = |cid| project.connector(cid).is_some_and(|c| matches!(c.anchor, AnchorRef::FaceCenter(..)));
    if matches!(j.kind, JointKind::Rigid) && face(j.a) && face(j.b) {
        top.push(Row::new("").opt(Opt::switch(OptKey::of("flip_side"), qymcad_i18n::tr("j-flip-side"), project.joint_side_flipped(j.id).lit()).hint(qymcad_i18n::tr("j-coplanar-hint"))));
    }
    let mut sections = vec![Section::plain(top)];
    for (i, (a, cid)) in ANCHORS.iter().zip([j.a, j.b]).enumerate() {
        let Some(c) = project.connector(cid) else { continue };
        let part = project.components.iter().find(|p| p.id == c.owner).map(|p| qymcad_i18n::name(&p.name)).unwrap_or_default();
        let title = qymcad_i18n::tr2("box-part-of", "tag", &qymcad_i18n::tr(a.tag), "name", &part);
        let mut rows = Vec::new();
        // an attachment point means something only where the geometry has a length along its axis
        if matches!(c.anchor, AnchorRef::FaceCenter(..) | AnchorRef::EdgeMid(..)) {
            let words: Vec<&str> = ATTACH_POINTS.iter().map(|p| p.label()).collect();
            rows.push(Row::new(qymcad_i18n::tr("box-point")).opt(Opt::modes(OptKey::at("attach", i), None, &words, ATTACH_POINTS.iter().position(|p| *p == c.point).unwrap_or(0))));
        }
        // THE SECONDARY AXIS BY A PICK: derived from the geometry, but a square face has no long side, and an edge is
        // pointed at by hand then; the turn by a quarter is the common case of the turn
        let armed = jc.joint.axis_pick == Some(cid);
        let mut axis = Row::new(qymcad_i18n::tr("box-axis"))
            .opt(Opt::pick(OptKey::at("conn_axis", i), qymcad_i18n::tr("j-axis-pick"), None, armed.lit()).hint(qymcad_i18n::tr("j-axis-pick-hint")))
            .opt(Opt::press(OptKey::at("roll", i), format!("{} 90{}", ph::ARROW_CLOCKWISE, qymcad_i18n::tr("unit-deg-suffix"))).hint(qymcad_i18n::tr("j-roll-hint")));
        if c.axis_ref.is_some() {
            axis = axis.opt(Opt::press(OptKey::at("conn_axis_auto", i), qymcad_i18n::tr("j-axis-auto")).hint(qymcad_i18n::tr("j-axis-auto-hint")));
        }
        rows.push(axis);
        rows.push(Row::new(qymcad_i18n::tr("box-turn")).field(&format!("{}-turn", a.prefix)).unit("unit-deg-suffix"));
        let mut offset = Row::new(qymcad_i18n::tr("box-offset")).unit("unit-mm-suffix");
        for (k, tag) in ["X", "Y", "Z"].into_iter().enumerate() {
            offset = offset.tagged(tag, &format!("{}-{k}", a.prefix));
        }
        rows.push(offset);
        sections.push(Section::titled(title, Folded::No, rows));
    }
    // the limits of the free degrees, each switched on by its tick and typed in its field - folded as the box opens
    let free = j.kind.free_slots();
    let limits: Vec<Row> = (0..3)
        .filter(|s| free[*s])
        .map(|slot| {
            let w = joint_slot_words(j.kind, slot);
            let end = |key: &'static str, end: &str, on: bool| Opt::switch(OptKey::at(key, slot), qymcad_i18n::tr(end), on.lit());
            Row::new(qymcad_i18n::tr(w.row))
                .opt(end("limit_min", "j-min", j.limit_min[slot].is_some()))
                .field(&format!("min{slot}"))
                .opt(end("limit_max", "j-max", j.limit_max[slot].is_some()))
                .field(&format!("max{slot}"))
                .unit(w.unit)
        })
        .collect();
    if !limits.is_empty() {
        sections.push(Section::titled(qymcad_i18n::tr("box-limits"), Folded::Yes, limits));
    }
    qymcad_ui_state::Layout { sections, foot: joint_foot(jc, j) }
}

/// THE FOOT OF THE BOX OF A JOINT: the run of a degree - an assembled mechanism watched moving, a degree with nowhere to
/// run getting no button - and "keep as it stands": the joint declares the current placement its own, for parts placed
/// by hand or imported where they belong.
fn joint_foot(jc: &qymcad_ui_state::JointCtx, j: &qymcad_core::feature::Joint) -> Vec<qymcad_ui_state::Opt> {
    use qymcad_ui_state::{Opt, OptKey};
    let project = &*jc.project;
    let mut v = Vec::new();
    if jc.joint_anim.as_ref().is_some_and(|a| a.joint == j.id) {
        v.push(Opt::press(OptKey::of("anim_stop"), format!("{} {}", ph::STOP, qymcad_i18n::tr("j-anim-stop"))));
    } else {
        for slot in (0..3).filter(|s| project.joint_anim_range(j.id, *s).is_some()) {
            let word = qymcad_i18n::tr(["j-anim-angle", "j-anim-offset", "j-anim-offset2"][slot]);
            v.push(Opt::press(OptKey::at("anim", slot), format!("{} {word}", ph::PLAY)).hint(qymcad_i18n::tr("j-anim-hint")));
        }
    }
    let as_built = j.as_built.is_some();
    v.push(Opt::press(OptKey::of("as_built"), qymcad_i18n::tr(if as_built { "j-as-built-again" } else { "j-as-built" })).hint(qymcad_i18n::tr("j-as-built-hint")));
    if as_built {
        v.push(Opt::press(OptKey::of("as_built_off"), qymcad_i18n::tr("j-as-built-off")));
    }
    v
}

/// The points an anchor sits at along its axis, in the order the box offers them.
const ATTACH_POINTS: [qymcad_core::asm::connector::AttachPoint; 3] =
    [qymcad_core::asm::connector::AttachPoint::Middle, qymcad_core::asm::connector::AttachPoint::Start, qymcad_core::asm::connector::AttachPoint::End];

/// WHAT WAS DONE TO AN OPTION OF THE BOX OF JOINT `j`, done to the joint; whether the document changed.
pub(crate) fn joint_opt_act(jc: &mut qymcad_ui_state::JointCtx, j: &qymcad_core::feature::Joint, did: qymcad_ui_state::Did) -> bool {
    use qymcad_ui_state::Did;
    let anchor = |i: usize| [j.a, j.b][i.min(1)];
    match did {
        Did::Switched { key } => match key.name {
            "drive" => {
                let slot = key.at.min(2);
                jc.project.set_joint_drive(j.id, slot, j.drive[slot].is_none().then(|| slot_value(j, slot)));
                true
            }
            "global" => {
                if let Some(jj) = jc.project.joints.iter_mut().find(|x| x.id == j.id) {
                    jj.global = !jj.global;
                }
                true
            }
            "flip_side" => {
                jc.project.flip_joint_side(j.id);
                true
            }
            "conn_axis" => {
                let cid = anchor(key.at);
                jc.joint.axis_pick = if jc.joint.axis_pick == Some(cid) { None } else { Some(cid) };
                false
            }
            "limit_min" | "limit_max" => {
                let slot = key.at.min(2);
                if let Some(jj) = jc.project.joints.iter_mut().find(|x| x.id == j.id) {
                    let limit = if key.name == "limit_min" { &mut jj.limit_min[slot] } else { &mut jj.limit_max[slot] };
                    *limit = if limit.is_some() { None } else { Some(0.0) };
                }
                true
            }
            _ => false,
        },
        Did::Pressed { key } => match key.name {
            "anim_stop" => {
                stop_joint_anim(jc);
                false
            }
            "anim" => {
                qymcad_ui_state::start_joint_anim(jc.joint_anim, jc.project, j.id, key.at.min(2));
                false
            }
            "as_built" => {
                if jc.project.set_joint_as_built(j.id) {
                    *jc.status = Notice::info(qymcad_i18n::tr("j-as-built-ok"));
                    true
                } else {
                    // NO SILENT REFUSAL: the anchor may have failed to resolve, and that is learned here rather than from
                    // a part that does not move
                    *jc.status = Notice::error(qymcad_i18n::tr("j-as-built-failed"));
                    false
                }
            }
            "as_built_off" => {
                jc.project.clear_joint_as_built(j.id);
                *jc.status = Notice::info(qymcad_i18n::tr("j-as-built-cleared"));
                true
            }
            // THE TURN by a quarter, the common case; any other turn is typed in its field
            "roll" => match jc.project.connector(anchor(key.at)).cloned() {
                Some(c) => jc.project.set_connector_placement(c.id, c.point, (c.rot_deg + 90.0).rem_euclid(360.0), c.offset_xyz),
                None => false,
            },
            "conn_axis_auto" => jc.project.set_connector_axis_ref(anchor(key.at), None),
            _ => false,
        },
        Did::Chose { key, index } if key.name == "attach" => match jc.project.connector(anchor(key.at)).cloned() {
            Some(c) => jc.project.set_connector_placement(c.id, ATTACH_POINTS[index.min(2)], c.rot_deg, c.offset_xyz),
            None => false,
        },
        _ => false,
    }
}

/// THE VALUES OF A JOINT TYPED OR DRAGGED IN A PANEL ARE ONE STEP OF UNDO. The field writes into the document while it
/// is typed or dragged, before it reports a change, and a rebuild in between takes the written value as the committed
/// state - so the step opens where the typing or the drag begins (`before` is the joints as they stood before the widgets
/// of this frame) and closes where it ends. A change of any other widget of the panel is laid as a step of its own.
/// Called after the widgets and before the solve, so the step begins before the part moves.
pub fn joint_values_step(ctx: &egui::Context, edits: &mut qymcad_ui_state::Edits, project: &mut qymcad_core::model::Project, before: Vec<qymcad_core::feature::Joint>, changed: bool) {
    let take = |key: &str| ctx.data_mut(|d| d.remove_temp::<bool>(egui::Id::new(key))).unwrap_or(false);
    let (began, ended) = (take("joint_values_began"), take("joint_values_ended"));
    let open_id = egui::Id::new("joint_values_open");
    let mut open = ctx.data(|d| d.get_temp::<bool>(open_id)).unwrap_or(false) && edits.open.is_some();
    if began && !open && edits.open.is_none() {
        let after = std::mem::replace(&mut project.joints, before);
        qymcad_ui_state::begin_edit(edits, project, qymcad_i18n::tr("status-edit-joint"));
        project.joints = after;
        open = true;
    }
    if open && ended {
        qymcad_ui_state::close_edit(edits, project);
        open = false;
    } else if changed && !open {
        qymcad_ui_state::settle_step(edits, project, qymcad_i18n::tr("status-edit-joint"), false);
    }
    ctx.data_mut(|d| d.insert_temp(open_id, open));
}
