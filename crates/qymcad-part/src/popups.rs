//! THE FIELDS AT THE GEOMETRY: where the popup of a command stands, and the popups themselves.

use qymcad_ui_state::Notice;
use super::*;

/// HOW FAR BESIDE ITS ARROW THE FIELDS OF A COMMAND STAND, on the screen: clear of the arrow and of the ball at its tip
/// (5 px), near enough to be read as the arrow's.
pub(crate) const ARROW_CLEARANCE: f32 = 40.0;

/// WHERE THE FIELDS OF A COMMAND WITH AN ARROW STAND: beside the tip of the arrow from `base` to `tip`, across it on the
/// screen - on its right where the box fits in `rect`, on its left where it does not (`clamp_popup` keeps 200 px for the
/// box and pushed it back against the arrow). Reported behaviour: the length field of an extrusion stood against the tip
/// of its arrow, and the arrow could hardly be seen or taken.
pub(crate) fn beside_the_arrow(base: Pos2, tip: Pos2, rect: Rect) -> Pos2 {
    const BOX_WIDTH: f32 = 200.0; // as `clamp_popup` keeps for it
    let d = tip - base;
    let across = if d.length() < 1.0 { egui::vec2(1.0, 0.0) } else { egui::vec2(-d.y, d.x).normalized() };
    let right = if across.x < 0.0 { -across } else { across };
    let on_right = tip + right * ARROW_CLEARANCE;
    if on_right.x + BOX_WIDTH <= rect.right() - 4.0 {
        on_right
    } else {
        // the box's left edge so far left that its right edge stands clear of the arrow
        tip - right * ARROW_CLEARANCE - egui::vec2(BOX_WIDTH, 0.0)
    }
}

/// HOW FAR FROM THE ARROWS AND RINGS OF A DATUM ITS FIELDS STAND, on the screen.
pub(crate) const HANDLE_CLEARANCE: f32 = 24.0;

/// WHERE THE FIELDS OF A DATUM STAND: beside the place its arrows and rings took on the screen (`handles_seen`), clear of
/// it by `HANDLE_CLEARANCE` - on its right, on its left, under it, over it: the first side where the whole box of `size`
/// fits in the view. None when no side fits. Reported behaviour: the fields of a point and of an axis by hand stood over
/// their arrows - a box laid 270 px left of the point is 230 px wide only without the axis's six fields, and an arrow
/// along Y or Z runs left as often as right.
pub(crate) fn clear_of_the_handles(handles: Rect, size: egui::Vec2, rect: Rect) -> Option<Pos2> {
    let view = Rect::from_min_max(rect.min + egui::vec2(4.0, 32.0), rect.max - egui::vec2(4.0, 4.0));
    let h = handles.expand(HANDLE_CLEARANCE);
    let mid_y = (h.center().y - size.y / 2.0).clamp(view.top(), (view.bottom() - size.y).max(view.top()));
    let mid_x = (h.center().x - size.x / 2.0).clamp(view.left(), (view.right() - size.x).max(view.left()));
    [Pos2::new(h.right(), mid_y), Pos2::new(h.left() - size.x, mid_y), Pos2::new(mid_x, h.bottom()), Pos2::new(mid_x, h.top() - size.y)]
        .into_iter()
        .find(|p| view.contains_rect(Rect::from_min_size(*p, size)))
}

pub fn cmd_anchor_screen(pc: &mut qymcad_ui_state::PartCtx, rect: Rect) -> Option<Pos2> {
    let basis = pc.cam.basis();
    let scr = qymcad_ui_state::Screen { cam: pc.cam, set: pc.set, rect, basis: &basis };
    match pc.armed.cmd_kind() {
        1 | 3 => {
            let (base, dir, h) = qymcad_ui_state::feat_cmd_axis(pc.cmd, pc.gsel, pc.project)?;
            let dir = if pc.armed.cmd_kind() == 1 && pc.feat.flip { [-dir[0], -dir[1], -dir[2]] } else { dir };
            if pc.armed.cmd_kind() == 3 {
                return Some(scr.at(base).0);
            }
            let tip = [base[0] + dir[0] * h, base[1] + dir[1] * h, base[2] + dir[2] * h];
            Some(beside_the_arrow(scr.at(base).0, scr.at(tip).0, rect))
        }
        // EVERYTHING AIMED BY CLICKING A BODY KEEPS ITS POPUP BESIDE THE BODY.
        //
        // The fillet and the chamfer already did that, while the hole put its field right at the face
        // and the thread at the rim: that is, over what has to be aimed at next. The cuts had no anchor
        // at all - there was nowhere to show the offset field, although the command has that parameter.
        //
        // The shell, the draft, pushing and removing a face, thickening - all belong here. What helps
        // with aiming is not the popup but THE HANDLE: it sits on the geometry and covers nothing.
        4 | 5 | 6 | 7 | 17 | 18 | 23 | 24 | 25 | 26 | 27 | 28 | 29 | 30 | 31 | 32 | 33 | 34 | 35 | 36 => {
            let b =
                pc.recognise.src.or_else(|| pc.stitch_parts.first().copied()).or(pc.trim.keep.map(|(b, _)| b)).or(pc.gsel.faces_body).or_else(|| {
                    qymcad_ui_state::op_target_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, *pc.sel)
                })?;
            qymcad_ui_state::body_side_anchor(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, b, rect, &basis)
        }
        // datum plane (20): at the origin of the picked reference; point (21): at (x,y,z); axis (22): at
        // the origin of the reference or of the hand-typed one
        20 => {
            // NOTHING PICKED YET: in the corner of the view, clear of everything a plane or a face is to be picked on - at
            // the origin it stood over the faces of a part drawn there
            let Some((o, n)) =
                pc.datum.plane_pick.as_ref().and_then(|sp| {
                    qymcad_ui_state::mirror_plane_world(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, sp)
                })
            else {
                return Some(rect.min + egui::vec2(16.0, 48.0));
            };
            // beside the arrow of the offset, from the reference to where the plane will stand, as an extrusion's fields
            // stand beside its arrow
            let d = qymcad_ui_state::cmd_val(pc.cmd, "dist");
            let tip = [o[0] + n[0] * d, o[1] + n[1] * d, o[2] + n[2] * d];
            Some(beside_the_arrow(scr.at(o).0, scr.at(tip).0, rect))
        }
        // A POINT AND THE ORIGIN OF AN AXIS BY HAND are moved by the arrows and rings out of them: the fields stand clear of
        // those (`clear_of_the_handles`), at the point only where there are none
        21 => Some(scr.at([qymcad_ui_state::cmd_val(pc.cmd, "x"), qymcad_ui_state::cmd_val(pc.cmd, "y"), qymcad_ui_state::cmd_val(pc.cmd, "z")]).0),
        22 => {
            let p = if pc.datum.axis_mode == 1 {
                [qymcad_ui_state::cmd_val(pc.cmd, "ox"), qymcad_ui_state::cmd_val(pc.cmd, "oy"), qymcad_ui_state::cmd_val(pc.cmd, "oz")]
            } else {
                pc.datum.axis_ref.map(|(o, _)| o).unwrap_or([0.0, 0.0, 0.0])
            };
            Some(scr.at(p).0)
        }
        // primitives: the popup goes BESIDE - from the body while editing (like the chamfer), from the
        // placement point while creating (it used to sit at the centre of the primitive and covered it).
        10..=15 => {
            if pc.cmd.edit.is_some() {
                if let Some(b) = qymcad_ui_state::selected_body(pc.project, pc.sel) {
                    if let Some(a) =
                        qymcad_ui_state::body_side_anchor(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path }, b, rect, &basis)
                    {
                        return Some(a);
                    }
                }
            }
            let p = scr.at(pc.prim.place.unwrap_or([0.0, 0.0, 0.0])).0;
            Some(Pos2::new(p.x + 90.0, p.y - 20.0)) // beside the placement point, not over the primitive
        }
        _ => None,
    }
}

/// The on-screen input field or fields for the dimensions of the active command, at the geometry (just
/// like sketch dimensions): a number OR an expression such as `w/2+3`. Enter applies, Esc cancels. One
/// mechanism for every part tool.
/// THE TWO BUTTONS A COMMAND ENDS WITH, in the colours the scheme gives them.
///
/// Reported behaviour: "highlight the Enter (apply) and Esc (cancel) buttons in the tool popups with
/// different colours, green and red say, so that it is clear". They used to differ by a tick glyph and a
/// bold face - a difference one READS rather than sees, and at the moment of pressing nobody is reading.
///
/// THE COLOUR GOES ON THE TEXT AND THE OUTLINE, not on the fill. A filled patch would need a text colour
/// contrasting against the patch itself, which is a second decision nobody guards; text on the panel
/// background is exactly the pair the scheme's legibility check already measures.
///
/// ONE PAIR FOR THREE PLACES. The point of the pair is that the two DIFFER, and a decision written out at
/// each of the three sites drifts at the first edit of one of them.
/// WHAT A COMMAND BAR SAYS, as opposed to what it offers to press.
///
/// Reported behaviour: "messages like these must be written on a new line. The text with the
/// information - a new line; if there is text with errors, that goes on a new line too, so on the third
/// one. And this is scale 1 - very small and unreadable even on my 2K monitor."
///
/// MEASURED ON THE THREAD BAR, opened with its own defaults - M10, coarse pitch, 0.20 mm of fit. It says
/// three sentences at once, 340 characters of prose, and every one of them stood IN the row of controls:
/// the row wraps by itself, so a sentence broke wherever the row happened to end and carried on under a
/// button. A person reading it has to find where it went.
///
/// The sentences are gathered here while the controls are drawn, and laid out under them: what the tool
/// tells on one line, what will not build on the next. Off that row they are also written at the ordinary
/// size - the small size was the price of squeezing prose in beside the buttons, and there is no such
/// price to pay on a line of one's own.
/// ONE SENTENCE OF A BAR: its words, and the longer explanation the hover shows, if any.
pub(crate) struct Said {
    pub(crate) text: String,
    pub(crate) hover: Option<String>,
}

#[derive(Default)]
pub(crate) struct BarSays {
    /// what the tool is doing, or waiting for
    pub(crate) told: Vec<Said>,
    /// what builds, but not the way a person would expect it to
    pub(crate) warned: Vec<Said>,
    /// what will not build the way it is set
    pub(crate) wrong: Vec<Said>,
}

impl BarSays {
    /// What the tool is doing, or what it is waiting for.
    pub(crate) fn tell(&mut self, text: impl Into<String>) {
        self.told.push(Said { text: text.into(), hover: None });
    }

    /// The same, with the longer explanation the hover shows.
    pub(crate) fn tell_hover(&mut self, text: impl Into<String>, hover: impl Into<String>) {
        self.told.push(Said { text: text.into(), hover: Some(hover.into()) });
    }

    /// What builds, with something to know first: the icon of a warning in front, in its colour.
    pub(crate) fn warn(&mut self, text: impl Into<String>) {
        let icon = qymcad_ui_state::tone_icon(qymcad_ui_state::Tone::Warning).unwrap_or_default();
        self.warned.push(Said { text: format!("{icon} {}", text.into()), hover: None });
    }

    /// What will not build as it is set, and what to change.
    pub(crate) fn wrong(&mut self, text: impl Into<String>) {
        self.wrong.push(Said { text: text.into(), hover: None });
    }

    /// Lay the sentences under the controls, one line for each kind.
    ///
    /// A line that has nothing to say takes no room at all: an empty row reserved for a hint that rarely
    /// comes would push the viewport down by its height on every command.
    pub(crate) fn show(self, ui: &mut egui::Ui, pal: &qymcad_scheme::Palette) {
        let tone = |t| qymcad_ui_state::tone_colour(pal, t);
        for (line, colour) in [(self.told, pal.hint()), (self.warned, tone(qymcad_ui_state::Tone::Warning)), (self.wrong, tone(qymcad_ui_state::Tone::Refusal))] {
            if line.is_empty() {
                continue;
            }
            ui.horizontal_wrapped(|ui| {
                for (i, said) in line.into_iter().enumerate() {
                    if i > 0 {
                        ui.separator();
                    }
                    let r = ui.label(egui::RichText::new(said.text).color(colour));
                    if let Some(h) = said.hover {
                        r.on_hover_text(h);
                    }
                }
            });
        }
    }
}

pub(crate) fn confirm_button(pal: &qymcad_scheme::Palette, ui: &mut egui::Ui, enabled: bool, label: String) -> bool {
    let text = egui::RichText::new(label).color(pal.confirm()).strong();
    ui.add_enabled(enabled, egui::Button::new(text).stroke(egui::Stroke::new(1.0, pal.confirm()))).clicked()
}

pub(crate) fn refuse_button(pal: &qymcad_scheme::Palette, ui: &mut egui::Ui, label: String) -> bool {
    let text = egui::RichText::new(label).color(pal.refuse());
    ui.add(egui::Button::new(text).stroke(egui::Stroke::new(1.0, pal.refuse()))).clicked()
}

pub fn feat_cmd_popup(pc: &mut qymcad_ui_state::PartCtx, ctx: &egui::Context, rect: Rect) {
    if pc.carr.mode != 0 {
        comp_array_popup(pc, ctx, rect); // a pattern of components takes its values at the geometry as every tool does
        return;
    }
    if pc.armed.cmd_kind() == 0 || pc.cmd.params.is_empty() || !*pc.mode_3d {
        return;
    }
    let Some(anchor) = cmd_anchor_screen(pc, rect) else { return };
    let ready = cmd_ready(pc);
    // A PATTERN SIZES THE BODY PICKED: before the pick there is nothing to size, and a popup beside the body - pressed
    // against the edge of the view - would lie over the very body to be clicked
    if matches!(pc.armed.cmd_kind(), 17 | 18) && !ready {
        return;
    }
    let vars = pc.project.param_map();
    // in a symmetric chamfer the d2 field (the second leg or the angle) takes no part, so it is hidden
    let hide_d2 = pc.armed.cmd_kind() == 5 && pc.chamfer.mode == qymcad_core::feature::ChamferMode::Symmetric;
    // WHAT THE GEOMETRY REFUSES is said beside the first field, as a limit of the field itself is (`trial`)
    let trial = trial_refusal(pc, ctx);
    let checking = trial == Trial::Checking; // the worker has not answered yet: nothing is applied until it has
    let mut refused = trial.refusal();
    let mut params = std::mem::take(&mut pc.cmd.params);
    let mut apply = false;
    // The cancel is taken here and acted on AFTER the popup is drawn: `qymcad_ui_state::cancel_feat_cmd` closes the
    // command whole, and the closure below is still holding its parameters.
    let mut cancel = false;
    // THE FIELDS AT THE GEOMETRY, EACH AT ITS OWN PLACE. The radius at a vertex is shown AT THAT VERTEX:
    // six identical fields in a common column cannot be told apart, and which of them is which corner is
    // the only thing that matters here.
    let basis = pc.cam.basis();
    for (i, p) in params.iter_mut().enumerate() {
        let Some(w) = p.at else { continue };
        let at = qymcad_ui_state::clamp_popup(qymcad_ui_state::Screen { cam: &*pc.cam, set: pc.set, rect, basis: &basis }.at(w).0, rect);
        egui::Area::new(egui::Id::new(("feat_cmd_at", i))).fixed_pos(at + egui::vec2(8.0, -8.0)).order(qymcad_ui_state::AT_GEOMETRY).show(ctx, |ui| {
            qymcad_ui_state::at_geometry_ui(ui);
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(qymcad_i18n::tr(p.label_key()));
                    qymcad_ui_state::focus_edit(ui, &mut p.txt, 56.0, &qymcad_i18n::tr("f-number-or-expr"), false);
                    match field_value(p, &vars).and_then(|v| refused.clone().filter(|(k, _)| *k == p.key).map_or(Ok(v), |(_, m)| Err(m))) {
                        Ok(v) => p.val = v,
                        Err(msg) => {
                            qymcad_ui_state::tone_mark(ui, &pc.scheme.pal, qymcad_ui_state::Tone::Refusal).on_hover_text(msg);
                        }
                    }
                });
            });
        });
    }
    let want_focus = std::mem::take(&mut pc.cmd.focus); // a one-shot auto-focus of field 0 (Enter plus selection)
                                                        // the box of a datum beside its handles, by its size as the frame before drew it
    let size = ctx.memory(|m| m.area_rect(egui::Id::new("feat_cmd_popup"))).map_or(egui::vec2(230.0, 160.0), |r| r.size());
    let at = qymcad_ui_state::handles_seen(ctx)
        .filter(|_| matches!(pc.armed.cmd_kind(), 20..=22))
        .and_then(|h| clear_of_the_handles(h, size, rect))
        .unwrap_or_else(|| qymcad_ui_state::clamp_popup(anchor, rect) + egui::vec2(10.0, -10.0));
    egui::Area::new(egui::Id::new("feat_cmd_popup")).fixed_pos(at).order(qymcad_ui_state::AT_GEOMETRY).show(ctx, |ui| {
        qymcad_ui_state::at_geometry_ui(ui);
        egui::Frame::popup(ui.style()).show(ui, |ui| {
            // WHAT EXACTLY IS WRONG AND IN WHICH FIELD. There used to be only a flag saying whether all
            // was well, and the bottom of the popup printed that a dimension expression could not be
            // evaluated - the same message for a typo, for an unknown name and for a division by zero.
            // With two or three fields there is no telling which one holds the mistake.
            let mut bad: Option<(String, String)> = None;
            let mut enter = false; // Enter was pressed in at least one field
            for (i, p) in params.iter_mut().enumerate() {
                if hide_d2 && p.key == "d2" {
                    continue; // d2 is hidden in the symmetric mode
                }
                if p.at.is_some() {
                    continue; // a field AT THE GEOMETRY is drawn by its own window below
                }
                ui.horizontal(|ui| {
                    ui.label(qymcad_i18n::tr(p.label_key()));
                    // THE SAME FIELD AS IN A SKETCH AND IN THE PARAMETER TABLE.
                    //
                    // A private `qymcad_ui_state::focus_edit` used to stand here - and the list of drivers did not exist in
                    // the popups of the part tools AT ALL. It was reported plainly: there is no drop-down
                    // with a search of parameters and drivers in the popups of the sketcher and part tools.
                    // The assumption at the time was that "the field in the bars is one for everybody, so
                    // it is wired everywhere". Not everywhere: the popup at the geometry draws fields of
                    // its own, and the check showed that only once it went THROUGH THE FRAME of every
                    // tool.
                    let fid = egui::Id::new(("cmdparam", pc.armed.cmd_kind(), p.key.clone()));
                    let o = qymcad_ui_state::expr_field_autofocus(ui, &*pc.project, fid, &p.txt, 74.0, &qymcad_i18n::tr("f-number-or-expr"), i == 0 && want_focus);
                    p.txt = o.text;
                    let te = o.resp;
                    match field_value(p, &vars).and_then(|v| refused.take_if(|(k, _)| *k == p.key).map_or(Ok(v), |(_, m)| Err(m))) {
                        Ok(v) => p.val = v,
                        Err(msg) => {
                            // a broken expression or a value past the limits leaves `p.val` alone (the old value
                            // stays), marks the field and blocks Apply
                            if bad.is_none() {
                                bad = Some((qymcad_i18n::tr(p.label_key()), msg.clone()));
                            }
                            qymcad_ui_state::tone_mark(ui, &pc.scheme.pal, qymcad_ui_state::Tone::Refusal).on_hover_text(&msg);
                        }
                    }
                    if te.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        enter = true;
                    }
                });
            }
            // the refusal of a first field drawn at the geometry, marked there, closes Apply here too
            if let (None, Some((key, msg))) = (&bad, refused.take()) {
                let label = params.iter().find(|p| p.key == key).map(|p| qymcad_i18n::tr(p.label_key())).unwrap_or_default();
                bad = Some((label, msg));
            }
            // Enter applies ONLY when EVERY field is valid (a stale value would go through otherwise), and the trial has
            // answered
            let all_ok = bad.is_none() && qymcad_ui_state::bar_fields_valid(ui.ctx()) && !checking;
            if enter && all_ok {
                apply = true;
            }
            // ENTER REFUSED IS SAID WHERE A PERSON LOOKS FOR IT: the reason under the fields is small and in the
            // popup, and the status line kept the command's own ask as if the key had not been pressed
            if enter && !all_ok {
                *pc.status = match &bad {
                    Some((field, msg)) => Notice::refusal(format!("{field}: {msg}")),
                    None if checking => Notice::info(qymcad_i18n::tr("cmd-trial-checking")),
                    None => Notice::refusal(qymcad_i18n::tr("sk-fix-bar-field")), // a field of the bar above, the sides of a prism
                };
            }
            ui.horizontal(|ui| {
                if confirm_button(&pc.scheme.pal, ui, ready && all_ok, format!("{} {}", ph::CHECK, qymcad_i18n::tr("cmd-apply-enter"))) {
                    apply = true;
                }
                // THE BUTTON DOES WHAT THE KEY DOES. Reported behaviour: pressing it changes nothing -
                // the popup stays, the command stays. The click was read into an empty body, under a
                // comment claiming the cancel happened below; below there was only the apply. A button
                // that answers a click with nothing is worse than no button: it says the way out is
                // here, and it is not.
                if refuse_button(&pc.scheme.pal, ui, format!("{} {}", ph::X, qymcad_i18n::tr("cmd-cancel-esc"))) {
                    cancel = true;
                }
            });
            // AN ERROR MATTERS MORE THAN A HINT. The reason used to be shown only once the command was
            // ALREADY ready (a face or a contour picked) - that is, a typo in a field was reported after
            // everything else had been done. A typo concerns what is being typed right now, and there is
            // nothing to wait for.
            if let Some((field, msg)) = &bad {
                qymcad_ui_state::say(ui, &pc.scheme.pal, &qymcad_ui_state::Notice::refusal(format!("{field}: {msg}")), qymcad_ui_state::NoticeSize::Small);
            } else if checking {
                ui.label(egui::RichText::new(qymcad_i18n::tr("cmd-trial-checking")).weak().small());
            } else if !ready {
                ui.label(egui::RichText::new(cmd_hint(&*pc.armed, &*pc.gsel, &*pc.trim)).weak().small());
            }
        });
    });
    pc.cmd.params = params;
    if cancel {
        qymcad_ui_state::cancel_feat_cmd(pc);
    } else if apply {
        apply_feat_cmd(pc);
    }
}

/// Exact numeric input at the body gizmo: an EXPRESSION field at the geometry, as in the sketcher. Type
/// millimetres (a shift along an axis) or degrees (a rotation about an axis), and Enter applies a
/// PARAMETRIC Move feature; Esc cancels.
pub fn body_num_popup(pc: &mut qymcad_ui_state::PartCtx, ctx: &egui::Context, rect: Rect, basis: &([f64; 3], [f64; 3], [f64; 3])) {
    let Some((on, ax, rot)) = pc.body_giz.num else { return };
    let (o, _) = match on {
        qymcad_ui_state::GizmoOf::Body(mi) => qymcad_ui_state::body_gizmo_geometry(&*pc.body_giz, *pc.cam, &*pc.project, pc.set, mi),
        qymcad_ui_state::GizmoOf::Part(comp) => qymcad_ui_state::gizmo_geometry(*pc.cam, *pc.comp_giz, &*pc.project, comp),
    };
    let at = qymcad_ui_state::clamp_popup(qymcad_ui_state::Screen { cam: &*pc.cam, set: pc.set, rect, basis }.at(o).0, rect);
    let axn = ["X", "Y", "Z"][(ax as usize).min(2)];
    let label = if rot { qymcad_i18n::tr1("cmd-rotation-axis", "axis", axn) } else { qymcad_i18n::tr1("cmd-offset-axis", "axis", axn) };
    let preview = pc.project.eval_expr(&pc.body_giz.num_buf); // the preview lags by one frame - the buffer changes inside the field
    let mut apply = false;
    egui::Area::new(egui::Id::new("body_num")).fixed_pos(at + egui::vec2(12.0, -12.0)).order(qymcad_ui_state::AT_GEOMETRY).show(ctx, |ui| {
        qymcad_ui_state::at_geometry_ui(ui);
        egui::Frame::popup(ui.style()).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(label).small());
                let r = qymcad_ui_state::focus_edit(ui, &mut pc.body_giz.num_buf, 74.0, &qymcad_i18n::tr("f-expression"), std::mem::take(&mut pc.body_giz.num_focus));
                if r.lost_focus() {
                    apply = true; // Enter OR a click elsewhere commits, as with renaming in place
                }
                if ui.small_button(ph::CHECK).clicked() {
                    apply = true;
                }
            });
            match &preview {
                Ok(v) => {
                    ui.label(egui::RichText::new(format!("= {v:.3}")).weak().small());
                }
                Err(e) if !pc.body_giz.num_buf.is_empty() => {
                    // THE REASON, not "it did not evaluate". A general phrase is the same for a typo, for
                    // an unknown name and for a division by zero - that is, it says nothing.
                    qymcad_ui_state::say(ui, &pc.scheme.pal, &qymcad_ui_state::Notice::refusal(qymcad_i18n::error_words::expr_error_text(e)), qymcad_ui_state::NoticeSize::Small);
                }
                _ => {}
            }
        });
    });
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        pc.body_giz.num = None;
        return;
    }
    if apply {
        if let Ok(v) = pc.project.eval_expr(&pc.body_giz.num_buf) {
            if v.abs() > 1e-9 {
                let t = if rot {
                    qymcad_ui_state::rot_about_point(ax, v, o)
                } else {
                    let mut t = qymcad_core::feature::PLACE_IDENTITY;
                    t[[3, 7, 11][(ax as usize).min(2)]] = v;
                    t
                };
                match on {
                    qymcad_ui_state::GizmoOf::Body(mi) => apply_body_move(pc, mi, t),
                    // a part: the same motion in the assembly it stands in, its group going with it
                    qymcad_ui_state::GizmoOf::Part(comp) => {
                        qymcad_ui_state::begin_edit(pc.edits, pc.project, qymcad_i18n::tr("status-move-component"));
                        let to = qymcad_core::feature::mat_mul12(&t, &pc.project.component_transform(comp));
                        if !pc.project.move_component_by_hand(comp, to) {
                            *pc.status = Notice::info(qymcad_i18n::tr("j-group-grounded"));
                        }
                        qymcad_ui_state::invalidate_placement(pc.regen);
                        qymcad_ui_state::commit_edit(&mut pc.rebuild());
                    }
                }
            }
        }
        pc.body_giz.num = None;
    }
}

/// THE VALUES OF A PATTERN OF COMPONENTS - the step of a linear one, the angle of a circular one short of a full turn
/// - as expression fields, the same ones the popup of every tool has; drawn at the geometry and in the bar (`place`
///   keeps the two apart). Gives whether Enter was pressed in a field whose value can be taken. Reported behaviour: the
///   step of a linear pattern of parts could not be set - it stood in the command with no field to type it into.
pub(crate) fn comp_array_fields(pc: &mut qymcad_ui_state::PartCtx, ui: &mut egui::Ui, place: &str) -> bool {
    let vars = pc.project.param_map();
    let mut enter = false;
    let mut params = std::mem::take(&mut pc.cmd.params);
    for p in params.iter_mut() {
        ui.label(qymcad_i18n::tr(p.label_key()));
        let o = qymcad_ui_state::expr_field_autofocus(ui, &*pc.project, egui::Id::new(("carr_param", place, p.key.clone())), &p.txt, 74.0, &qymcad_i18n::tr("f-number-or-expr"), false);
        p.txt = o.text;
        match field_value(p, &vars) {
            Ok(v) => {
                p.val = v;
                enter |= o.resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            }
            Err(msg) => {
                qymcad_ui_state::tone_mark(ui, &pc.scheme.pal, qymcad_ui_state::Tone::Refusal).on_hover_text(msg);
            }
        }
    }
    pc.cmd.params = params;
    enter
}

/// THE POPUP OF A PATTERN OF COMPONENTS, beside its source: the step or the angle typed at the geometry; Enter
/// applies the pattern.
pub(crate) fn comp_array_popup(pc: &mut qymcad_ui_state::PartCtx, ctx: &egui::Context, rect: Rect) {
    if pc.cmd.params.is_empty() || !*pc.mode_3d {
        return;
    }
    let here = qymcad_ui_state::current_ctx_id(pc.active_path, pc.project);
    let m = pc.project.relative_transform(pc.carr.src, here);
    let basis = pc.cam.basis();
    let at = qymcad_ui_state::Screen { cam: &*pc.cam, set: pc.set, rect, basis: &basis }.at([m[3], m[7], m[11]]).0;
    let mut apply = false;
    egui::Area::new(egui::Id::new("comp_array_popup")).fixed_pos(qymcad_ui_state::clamp_popup(at, rect) + egui::vec2(12.0, -12.0)).order(qymcad_ui_state::AT_GEOMETRY).show(ctx, |ui| {
        qymcad_ui_state::at_geometry_ui(ui);
        egui::Frame::popup(ui.style()).show(ui, |ui| {
            ui.horizontal(|ui| {
                apply = comp_array_fields(pc, ui, "popup");
            });
        });
    });
    if apply {
        apply_comp_array(pc);
    }
}
