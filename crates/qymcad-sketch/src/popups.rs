//! THE FIELDS AT THE GEOMETRY of a sketch: the sizes of a new shape, of a corner, of a turn, the editor of a dimension.

use qymcad_ui_state::Notice;
use super::*;

pub fn corner_input_popup(cc: &mut qymcad_ui_state::CornerCtx, ctx: &egui::Context, rect: Rect) {
    // THE SIDE OF A CORNER WITH MORE THAN ONE TO GIVE IS WHERE THE POINTER IS, while the field is open: the corner of a
    // crossing follows it into the quarter it stands in, and the first leg of a chamfer is the line it is nearer to.
    // Reported behaviour: "the side of the fillet or the chamfer is chosen by where the cursor is, with the preview
    // drawn there".
    let following = cc.corner.aim == qymcad_ui_state::Aim::Following;
    if let (true, Some((si, _, _)), Some(pos)) = (following, cc.corner.at, ctx.input(|i| i.pointer.hover_pos()).filter(|p| rect.contains(*p))) {
        let w = qymcad_ui_state::to_world(cc.view, rect, pos);
        if cc.corner.set.steer(&*cc.project, si, (w.x, w.y)) == qymcad_ui_state::Steered::Yes {
            cc.corner.near = Some(Point2::new(w.x, w.y));
        }
    }
    // THE PREVIEW IS DRAWN FIRST, AND ALWAYS: it belongs to the sheet and not to the field, and a person deciding
    // where to click cannot see it if the field has to be open before it appears.
    draw_the_corner_preview(cc, ctx, rect);
    // A CORNER FOLLOWING THE POINTER HAS NO BOX YET: the box would stand beside a corner that moves, and run from the
    // pointer that goes to it. A click fixes the corner, and the box opens then.
    if following && cc.corner.at.is_some() {
        *cc.status = Notice::info(qymcad_i18n::tr("sk-corner-click-to-fix"));
        return;
    }
    // THE FIELD OF THE WHOLE SET, not of one corner: one value cuts every corner the chosen lines make, and the
    // set is beside the field rather than under the pointer, so it does not move with it (Enter applies, Esc
    // cancels, and both act on the whole set as one step of undo).
    let Some((si, pid, chamfer)) = cc.corner.at else { return };

    // THE WHOLE SET, AS IT STANDS THIS FRAME: what one answer will cut. Read on every frame, because a corner of
    // the set can be one line further on or another the moment a line is chosen or let go.
    let standing = cc.corner.set.standing();

    // **A BOX WITH NOTHING TO CUT IN IT IS NOT DRAWN.** The box is the value for the set, and where there is no
    // corner in the set there is nothing the value is for: the amber corner under the cursor says what a click would
    // name, and a box standing over the drawing with a number in it invites a number to be typed for a set that does
    // not exist. It is also what it was before - the box used to open on the tool being taken, over the middle of
    // the drawing, with the set empty and nothing to answer.
    if standing.is_empty() {
        return;
    }

    let at = cc.corner.pos.unwrap_or_else(|| rect.center());
    let want_focus = std::mem::take(&mut cc.corner.focus);
    let enter = ctx.input(|i| i.key_pressed(egui::Key::Enter));
    let (mut apply, mut cancel) = (false, false);
    let mut buf = std::mem::take(&mut cc.corner.buf);

    // ONE VALUE CUTS THE WHOLE SET, so it is held by the tightest corner in it AND by what the lines between the
    // corners spend on themselves. A value that fits one corner and overruns the one beside it would leave a
    // half-cut drawing behind, and rounding every corner of a rectangle takes half its short side for that reason.
    let limit = cc.corner.set.limit(cc.project, si, qymcad_ui_state::corner_tool(chamfer, cc.tool_prefs), cc.tool_prefs.fillet_by);
    // WHY THIS VALUE CANNOT BE TAKEN, said of the value as it is typed and again at Enter - one sentence, read in
    // two places, so that what the box says beside the field is the very reason Enter refuses it for.
    let judge = |project: &qymcad_core::model::Project, text: &str| -> Option<String> {
        match qymcad_core::expr::eval(text.trim(), &project.param_map()) {
            Err(e) => Some(qymcad_i18n::error_words::expr_error_text(&e)),
            Ok(v) if !v.is_finite() || v <= 1e-6 => Some(qymcad_i18n::tr("cmd-value-zero")),
            // A VALUE THE CORNERS CANNOT TAKE says so as it is typed: past the leg or the radius they hold, and
            // nothing is drawn to promise what cannot be built.
            Ok(v) if limit.is_some_and(|l| v >= l * (1.0 - 1e-9)) => Some(qymcad_i18n::tr("sk-fillet-too-big")),
            Ok(_) if standing.is_empty() => Some(qymcad_i18n::tr("sk-no-corner-here")),
            Ok(_) => None,
        }
    };
    // THE SECOND VALUE OF A CHAMFER of two legs or of a leg and an angle: the corners of the set are cut by both,
    // and one answer still cuts all of them.
    let second_used = chamfer && cc.tool_prefs.chamfer_mode != qymcad_core::feature::ChamferMode::Symmetric;
    let angle = cc.tool_prefs.chamfer_mode == qymcad_core::feature::ChamferMode::DistAngle;
    let judge2 = |project: &qymcad_core::model::Project, text: &str| -> Option<String> {
        match qymcad_core::expr::eval(text.trim(), &project.param_map()) {
            Err(e) => Some(qymcad_i18n::error_words::expr_error_text(&e)),
            Ok(v) if !v.is_finite() || v <= 1e-6 => Some(qymcad_i18n::tr("cmd-value-zero")),
            Ok(v) if angle && v.to_radians() >= std::f64::consts::PI => Some(qymcad_i18n::tr("sk-fillet-too-big")),
            Ok(_) => None,
        }
    };
    // WHY THE VALUE AS TYPED IS REFUSED, said in the box and not only under the pointer. A tooltip exists only while
    // the pointer rests on the glyph beside it, and the reason belongs to the value: a person reading back what they
    // typed has their eyes on the field, not on a warning mark they have to find first.
    let mut said: Option<String> = None;
    let bar_typed = |key: &'static str| ctx.memory(|m| m.has_focus(qymcad_ui_state::bar_field_id(key)));
    let area = egui::Id::new(("cornerinput", si, pid));
    let size = ctx.memory(|m| m.area_rect(area)).map_or(CORNER_BOX_GUESS, |r| r.size());
    let at = corner_box_place(cc, rect, si, standing.first().copied(), chamfer, size).unwrap_or_else(|| qymcad_ui_state::clamp_popup(at, rect) + egui::vec2(10.0, -10.0));
    egui::Area::new(area).fixed_pos(at).order(egui::Order::Foreground).show(ctx, |ui| {
        egui::Frame::popup(ui.style()).show(ui, |ui| {
            ui.horizontal(|ui| {
                // "R of every corner" is the field of FILLET ALL, and of nothing else: it is the one that rounds a
                // whole contour rather than a corner of it, and the zero in the field it is opened with is what says
                // so.
                ui.label(if chamfer {
                    qymcad_i18n::tr(qymcad_ui_state::chamfer_d1_label(cc.tool_prefs.chamfer_mode))
                } else if pid == 0 {
                    qymcad_i18n::tr("sk-r-all-corners")
                } else {
                    qymcad_i18n::tr(qymcad_ui_state::fillet_label(cc.tool_prefs.fillet_by))
                });
                let r0 = qymcad_ui_state::focus_edit(ui, &mut buf, 64.0, "", want_focus);
                // THE REASON, TAKEN FROM THE TEXT AS IT IS TYPED - before the value of the bar is put over it below.
                // Reading it after would read a buffer that has already been given back, and a value a person has
                // typed and is still typing would be judged by something else than what they wrote.
                said = judge(&*cc.project, &buf);
                if let Some(why) = &said {
                    qymcad_ui_state::tone_mark(ui, &cc.scheme.pal, qymcad_ui_state::Tone::Refusal).on_hover_text(why);
                }
                // ONE VALUE IN TWO PLACES: the radius on the bar and the one at the corners. Typed at the corners it
                // is the bar's too; while the corner field is not the one being typed in, it shows the bar's. Two
                // fields apart, a radius typed on the bar was not the one Enter applied - the corners kept the radius
                // of the time before.
                if r0.has_focus() {
                    if let Some(v) = parse_num(cc.project, &buf).filter(|v| *v > 1e-6) {
                        cc.tool_prefs.fillet = v;
                    }
                } else if bar_typed("sk_fillet") && !enter && !r0.lost_focus() && cc.corner.why.is_none() && parse_num(cc.project, &buf).is_none_or(|v| (v - cc.tool_prefs.fillet).abs() > 1e-12) {
                    // the bar's value is taken only by a field at rest: in the frame of Enter it would put the old radius
                    // over the text being applied, and after a refusal over the text refused
                    buf = qymcad_core::expr::fmt_num(cc.tool_prefs.fillet);
                }
                // Enter in the field at the corners applies it, or Enter in the radius field of the bar (below)
                if enter && (r0.lost_focus() || r0.has_focus()) {
                    apply = true;
                }
                // Enter in the radius field of the bar is this Enter too
                if qymcad_ui_state::bar_enter_take(ui.ctx()) {
                    apply = true;
                }
                // THE SECOND VALUE OF A CHAMFER of two legs or of a leg and an angle, beside the first and before the
                // buttons, so Tab goes from the one field to the other; it is the bar's too, as the first leg is.
                // Every corner of the set is cut by both values, and Enter in either applies.
                if second_used {
                    ui.label(qymcad_i18n::tr(qymcad_ui_state::chamfer_d2_label(cc.tool_prefs.chamfer_mode)));
                    let r2 = qymcad_ui_state::focus_edit(ui, &mut cc.corner.buf2, 64.0, "", false);
                    if let Some(why) = judge2(&*cc.project, &cc.corner.buf2) {
                        qymcad_ui_state::tone_mark(ui, &cc.scheme.pal, qymcad_ui_state::Tone::Refusal).on_hover_text(why);
                    }
                    if r2.has_focus() {
                        if let Some(v) = parse_num(cc.project, &cc.corner.buf2).filter(|v| *v > 1e-6) {
                            cc.tool_prefs.chamfer_second = v;
                        }
                    } else if bar_typed("sk_chamfer2") && !enter && !r2.lost_focus() && parse_num(cc.project, &cc.corner.buf2).is_none_or(|v| (v - cc.tool_prefs.chamfer_second).abs() > 1e-12) {
                        cc.corner.buf2 = qymcad_core::expr::fmt_num(cc.tool_prefs.chamfer_second);
                    }
                    if enter && (r2.lost_focus() || r2.has_focus()) {
                        apply = true;
                    }
                }
                if ui.button(ph::CHECK).clicked() {
                    apply = true;
                }
                if ui.button(ph::X).clicked() {
                    cancel = true;
                }
            });
            // HOW MUCH IS IN HAND, said beside the field: a person typing one value for four corners has to know
            // that it is four, and the one answer that cuts them is one Enter.
            if !standing.is_empty() {
                ui.label(qymcad_i18n::tr1("sk-corner-set-n", "n", &standing.len().to_string()));
            }
            // WHY A VALUE IS REFUSED IS SAID IN THE BOX AND NOT ONLY UNDER THE POINTER. The reason belongs to the
            // value being typed, and a person reading it back has their eyes on the field rather than hunting the
            // sheet for a warning glyph: the words were a tooltip, and the tooltip only existed while the pointer
            // rested on the glyph beside it.
            if let Some(why) = &said {
                qymcad_ui_state::say(ui, &cc.scheme.pal, &qymcad_ui_state::Notice::refusal(why.clone()), qymcad_ui_state::NoticeSize::Line);
            }
        });
    });
    cc.corner.buf = buf;
    let refused = judge(&*cc.project, &cc.corner.buf).or_else(|| second_used.then(|| judge2(&*cc.project, &cc.corner.buf2)).flatten()); // taken as zero, it closed the field and did nothing
    if apply && refused.is_some() {
        *cc.status = Notice::refusal(refused.clone().unwrap_or_default());
        cc.corner.why = refused;
        apply = false;
    }
    if apply {
        let r = parse_num(cc.project, &cc.corner.buf.clone()).unwrap_or(0.0);
        // THE BOUNDARY OF AN OPERATION: one step of undo, named after the tool. A person naming four corners of a
        // shape meant one act, and undo taking back one corner of it would leave the drawing in a state nobody
        // asked for.
        // the lines cut at a crossing to name the corner are the start of this step: the cut joins it, and one undo
        // takes back the whole of it
        let joins = cc.corner.crossing == qymcad_ui_state::CrossingCut::Cut;
        if !joins {
            qymcad_ui_state::begin_edit(&mut *cc.edits, &*cc.project, qymcad_i18n::tr(if chamfer { "tool-chamfer" } else { "tool-fillet" }));
        }
        if r > 1e-6 {
            cc.tool_prefs.fillet = r; // sticky: the next corner offers the same value
                                      // THE SIZE AS IT WAS GIVEN, not only as a radius: a fillet taken by the length of its arc or by the chord
                                      // between its ends is a different drawing, and a chamfer of two legs is two numbers.
            let size = qymcad_core::model::FilletSize { by: cc.tool_prefs.fillet_by, value: r };
            if chamfer && second_used {
                cc.tool_prefs.chamfer_second = parse_num(cc.project, &cc.corner.buf2.clone()).unwrap_or(cc.tool_prefs.chamfer_second);
            }
            let cut: usize = if pid == 0 {
                // "ROUND EVERY CORNER" OF A SHAPE, which is one command about a whole set of lines rather than about
                // the corners the set happens to name: it is the only corner cut left to the drawing to find.
                let only = cc.corner.only.take().or_else(|| {
                    let sel: std::collections::HashSet<Id> = cc.sel_sk.items.iter().filter(|(k, _)| *k == 1).map(|(_, id)| *id).collect();
                    (!sel.is_empty()).then_some(sel)
                });
                cc.project.fillet_all_corners_by(si, size, only.as_ref())
            } else {
                // ONE ANSWER CUTS THE WHOLE SET, OR NOTHING: the core cuts every corner by the size as the bar gives it,
                // the same cut the preview drew, and leaves the sketch as it was where one corner does not take it
                let corners: Vec<qymcad_core::model::CornerAt> = standing.iter().map(|&(point, pair)| qymcad_core::model::CornerAt { point, pair }).collect();
                let cut = corner_cut(cc, chamfer, r);
                cc.project.cut_corner_set(si, &corners, cut, cc.corner.near)
            };
            if cut > 0 {
                // THE CHOSEN LINES THAT THE KNIFE HAS CONSUMED GO OUT OF THE SELECTION: a line that is no longer
                // there cannot stand lit, and the selection would show a piece of the drawing that is not in it.
                // What is left is lit as it is - a corner that was cut away took its lines with it, and the ones
                // that remain are what a further click would work on.
                cc.sel_sk.items.retain(|&(_, id)| qymcad_ui_state::pick_still_stands(&*cc.project, si, id));
                qymcad_ui_state::invalidate(cc.regen);
                *cc.status = if pid != 0 && cut > 1 {
                    // MORE THAN ONE CORNER CUT AT ONCE IS SAID AS MANY: "done" would read as the one corner the
                    // pointer was on, and the set is not visible in a word that size.
                    Notice::info(qymcad_i18n::tr1("sk-filleted-n", "n", &cut.to_string()))
                } else {
                    Notice::info(qymcad_i18n::tr("sk-done"))
                };
            } else {
                *cc.status = Notice::refusal(qymcad_i18n::tr("sk-fillet-too-big"));
                cc.corner.why = Some(qymcad_i18n::tr("sk-fillet-too-big"));
            }
            if joins {
                qymcad_ui_state::fold_into_last_step(&mut *cc.edits, &*cc.project);
            } else {
                qymcad_ui_state::close_edit(&mut *cc.edits, &*cc.project);
            }
            if cut > 0 {
                cc.corner.clear(); // ALL of the input, so nothing of this set travels into the next call
            }
            // A VALUE THE CORNERS CANNOT TAKE keeps the field open with it, the reason said: closed, it left a
            // person no field to put a smaller one in, and no words beside it.
        } else {
            // a size of nothing cuts nothing: the lines cut at a crossing for it are whole again
            if joins {
                qymcad_ui_state::take_back_last_step(&mut *cc.edits, &mut *cc.project, &mut *cc.regen);
            } else {
                qymcad_ui_state::close_edit(&mut *cc.edits, &*cc.project);
            }
            cc.corner.clear();
        }
    }
    if cancel || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        // the lines cut at a crossing for a corner never cut are whole again
        if cc.corner.crossing != qymcad_ui_state::CrossingCut::None {
            qymcad_ui_state::take_back_last_step(&mut *cc.edits, &mut *cc.project, &mut *cc.regen);
        }
        cc.corner.clear();
    }
}

/// An ellipse: width by height (the full axes are twice the semi-axes).
pub fn ellipse_input_popup(pl: &mut qymcad_ui_state::PlaceCtx, ctx: &egui::Context, rect: Rect, si: usize) {
    // an ellipse: width by height (the full axes are twice the major and twice the minor semi-axis) - the
    // entity is found through its centre handle
    if let Some((handle, click)) = pl.place.ellipse() {
        let cur = pl.project.ellipse_axes(si, handle);
        if let Some((rx, ry)) = cur {
            let (w0, h0) = (2.0 * rx, 2.0 * ry);
            let at = (qymcad_ui_state::Sheet { view: *pl.view, rect }).at(Point2::new(click.x + rx, click.y + ry));
            let want_focus = std::mem::take(&mut pl.place.focus);
            if want_focus {
                pl.place.buf[0] = format!("{}", (w0 * 1000.0).round() / 1000.0);
                pl.place.buf[1] = format!("{}", (h0 * 1000.0).round() / 1000.0);
            }
            let enter = ctx.input(|i| i.key_pressed(egui::Key::Enter));
            let (mut chg, mut close, mut got_focus) = (false, false, false);
            let mut buf = [std::mem::take(&mut pl.place.buf[0]), std::mem::take(&mut pl.place.buf[1])];
            egui::Area::new(egui::Id::new(("ellinput", si))).fixed_pos(qymcad_ui_state::clamp_popup(at, rect) + egui::vec2(10.0, -10.0)).order(egui::Order::Foreground).show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(qymcad_i18n::tr("sk-width-short"));
                        let r0 = qymcad_ui_state::focus_edit(ui, &mut buf[0], 60.0, "", want_focus);
                        chg |= r0.changed();
                        got_focus |= r0.has_focus();
                        ui.label(qymcad_i18n::tr("sk-height-short"));
                        let r1 = qymcad_ui_state::focus_edit(ui, &mut buf[1], 60.0, "", false);
                        chg |= r1.changed();
                        got_focus |= r1.has_focus();
                        if (r0.lost_focus() || r1.lost_focus()) && enter {
                            close = true;
                        }
                        if ui.button(ph::CHECK).clicked() {
                            close = true;
                        }
                    });
                });
            });
            pl.place.buf = buf.clone();
            if got_focus {
                pl.place.focus = false;
            }
            if chg {
                let nw = parse_num(pl.project, &buf[0]).unwrap_or(w0).max(0.02);
                let nh = parse_num(pl.project, &buf[1]).unwrap_or(h0).max(0.02);
                pl.project.set_ellipse_axes(si, handle, nw / 2.0, nh / 2.0);
                qymcad_ui_state::invalidate(pl.regen);
            }
            if close || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                pl.place.clear();
                pl.place.focus = false;
            }
        } else {
            pl.place.clear();
        }
    }
}

/// A rectangle: width by height. From two corners it is rebuilt from the anchor corner so the signs
/// are kept; from the centre it grows about the centre, so the centre never travels while the numbers
/// are typed.
pub fn rect_input_popup(pl: &mut qymcad_ui_state::PlaceCtx, ctx: &egui::Context, rect: Rect, si: usize) {
    // WHAT STAYS PUT while the numbers are typed: the first corner, or the centre.
    enum Anchor {
        Corner { fixed: Point2, signs: Point2 },
        Center { at: Point2 },
    }
    // The numbers the fields open on, where the popup stands, and what is being rebuilt.
    struct Active {
        anchor: Anchor,
        ids: Vec<Id>,
        w0: f64,
        h0: f64,
        at: Point2,
    }
    let active = pl
        .place
        .rect()
        .map(|(a, b, ids)| {
            let signs = Point2::new((b.x - a.x).signum(), (b.y - a.y).signum());
            Active { anchor: Anchor::Corner { fixed: a, signs }, ids, w0: (b.x - a.x).abs(), h0: (b.y - a.y).abs(), at: Point2::new(a.x.max(b.x), a.y.max(b.y)) }
        })
        .or_else(|| {
            pl.place.rect_center().map(|(center, corner, ids)| Active {
                anchor: Anchor::Center { at: center },
                ids,
                w0: 2.0 * (corner.x - center.x).abs(),
                h0: 2.0 * (corner.y - center.y).abs(),
                at: Point2::new(center.x.max(corner.x), center.y.max(corner.y)),
            })
        });
    // a rectangle: width by height (text fields, with auto-focus, Tab and Enter)
    if let Some(active) = active {
        let Active { anchor, ids, w0, h0, at } = active;
        let want_focus = std::mem::take(&mut pl.place.focus);
        if want_focus {
            pl.place.buf[0] = format!("{}", (w0 * 1000.0).round() / 1000.0);
            pl.place.buf[1] = format!("{}", (h0 * 1000.0).round() / 1000.0);
        }
        let enter = ctx.input(|i| i.key_pressed(egui::Key::Enter));
        let (mut chg, mut close, mut got_focus) = (false, false, false);
        let mut buf = [std::mem::take(&mut pl.place.buf[0]), std::mem::take(&mut pl.place.buf[1])];
        egui::Area::new(egui::Id::new(("rectinput", si)))
            .fixed_pos(qymcad_ui_state::clamp_popup((qymcad_ui_state::Sheet { view: *pl.view, rect }).at(at), rect) + egui::vec2(10.0, -10.0))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(qymcad_i18n::tr("sk-width-short"));
                        let r0 = qymcad_ui_state::focus_edit(ui, &mut buf[0], 60.0, "", want_focus);
                        chg |= r0.changed();
                        got_focus |= r0.has_focus();
                        ui.label(qymcad_i18n::tr("sk-height-short"));
                        let r1 = qymcad_ui_state::focus_edit(ui, &mut buf[1], 60.0, "", false);
                        chg |= r1.changed();
                        got_focus |= r1.has_focus();
                        if (r0.lost_focus() || r1.lost_focus()) && enter {
                            close = true;
                        }
                        if ui.button(ph::CHECK).clicked() {
                            close = true;
                        }
                    });
                });
            });
        pl.place.buf = buf.clone();
        pl.place.typed |= chg;
        if got_focus {
            pl.place.focus = false;
        }
        if chg {
            let nw = parse_num(pl.project, &buf[0]).unwrap_or(w0).max(0.01);
            let nh = parse_num(pl.project, &buf[1]).unwrap_or(h0).max(0.01);
            match anchor {
                Anchor::Corner { fixed, signs } => {
                    let nb = Point2::new(fixed.x + if signs.x < 0.0 { -nw } else { nw }, fixed.y + if signs.y < 0.0 { -nh } else { nh });
                    pl.project.delete_entities(si, &ids);
                    let nids = pl.project.add_rect_entity(si, fixed.x, fixed.y, nb.x, nb.y, qymcad_core::feature::Purpose::Real);
                    pl.place.set(qymcad_ui_state::PlacingShape::Rect { a: fixed, b: nb, ids: nids });
                }
                Anchor::Center { at: center } => {
                    let nb = Point2::new(center.x + nw / 2.0, center.y + nh / 2.0);
                    pl.project.delete_entities(si, &ids);
                    let nids = pl.project.add_rect_from_centre(si, center, nb, qymcad_core::feature::Purpose::Real);
                    pl.place.set(qymcad_ui_state::PlacingShape::RectCenter { center, corner: nb, ids: nids });
                }
            }
            qymcad_ui_state::invalidate(pl.regen);
        }
        // A WIDTH OR A HEIGHT TYPED IS LAID AS DIMENSIONS when the fields are closed with Enter or the tick, as the
        // diameter of a circle is: a size typed by hand is a size meant. Closed untouched, or with Esc, the rectangle is
        // left free.
        if close && pl.place.typed {
            if let Some(side) = pl.place.rect().or_else(|| pl.place.rect_center()).and_then(|(_, _, ids)| ids.first().copied()) {
                pl.project.dimension_rect(si, side);
                pl.project.solve_sketch(si);
                qymcad_ui_state::invalidate(pl.regen);
            }
        }
        if close || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            pl.place.clear();
            pl.place.focus = false;
        }
    }
}

/// A polygon: the radius of the construction circle plus the rotation angle.
pub fn poly_input_popup(pl: &mut qymcad_ui_state::PlaceCtx, ctx: &egui::Context, rect: Rect, si: usize) {
    // a polygon: the radius of the construction circle plus the rotation angle (it is rebuilt)
    if let Some(cid) = pl.place.poly() {
        if let (Some((cx, cy, r)), Some(ang)) = (pl.project.polygon_circle(si, cid), pl.project.polygon_angle(si, cid)) {
            let at = (qymcad_ui_state::Sheet { view: *pl.view, rect }).at(Point2::new(cx + r, cy));
            let want_focus = std::mem::take(&mut pl.place.focus);
            if want_focus {
                pl.place.buf[0] = format!("{}", (r * 1000.0).round() / 1000.0);
                pl.place.buf[1] = format!("{}", (ang.to_degrees() * 100.0).round() / 100.0);
            }
            let enter = ctx.input(|i| i.key_pressed(egui::Key::Enter));
            let (mut chg, mut close, mut got_focus) = (false, false, false);
            let mut buf = [std::mem::take(&mut pl.place.buf[0]), std::mem::take(&mut pl.place.buf[1])];
            egui::Area::new(egui::Id::new(("polyinput", si))).fixed_pos(qymcad_ui_state::clamp_popup(at, rect) + egui::vec2(10.0, -10.0)).order(egui::Order::Foreground).show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(qymcad_i18n::tr("sk-radius"));
                        let r0 = qymcad_ui_state::focus_edit(ui, &mut buf[0], 60.0, "", want_focus);
                        chg |= r0.changed();
                        got_focus |= r0.has_focus();
                        ui.label(qymcad_i18n::tr("sk-angle-deg"));
                        let r1 = qymcad_ui_state::focus_edit(ui, &mut buf[1], 50.0, "", false);
                        chg |= r1.changed();
                        got_focus |= r1.has_focus();
                        if (r0.lost_focus() || r1.lost_focus()) && enter {
                            close = true;
                        }
                        if ui.button(ph::CHECK).clicked() {
                            close = true;
                        }
                    });
                });
            });
            pl.place.buf = buf.clone();
            if got_focus {
                pl.place.focus = false;
            }
            if chg {
                let nr = parse_num(pl.project, &buf[0]).unwrap_or(r).max(0.01);
                let na = parse_num(pl.project, &buf[1]).map(|d| d.to_radians()).unwrap_or(ang);
                pl.project.set_polygon_radius(si, cid, nr);
                pl.project.set_polygon_angle(si, cid, na);
                qymcad_ui_state::invalidate(pl.regen);
            }
            if close || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                pl.place.clear();
                pl.place.focus = false;
            }
        } else {
            pl.place.clear();
        }
    }
}

/// The pop-up entry of sizes right after a rectangle or a polygon is built.
/// THE POPUP OF THE CORNER TOOL (a fillet radius or a chamfer leg): Enter applies, Esc cancels.
///
/// `armed` is the tool in hand, and it is here rather than in `Editing` because the preview on the sheet is drawn
/// whether or not the field is up: before the first click there is no field, and the amber corner under the cursor
/// is drawn as the chamfer or the fillet the tool in hand would cut it as.
pub fn place_input_popup(ed: qymcad_ui_state::Editing, mut tools: qymcad_ui_state::PopupTools, looks: qymcad_ui_state::PopupLooks, ctx: &egui::Context, rect: Rect) {
    let qymcad_ui_state::PopupTools { corner, place, sel_sk, armed } = &mut tools;
    corner_input_popup(
        &mut qymcad_ui_state::CornerCtx {
            corner,
            project: &mut *ed.project,
            edits: &mut *ed.edits,
            sel_sk,
            view: &*ed.view,
            regen: &mut *ed.regen,
            status: ed.status,
            tool_prefs: looks.tool_prefs,
            scheme: looks.scheme,
            set: looks.set,
            sel: ed.sel,
            armed,
        },
        ctx,
        rect,
    );
    let qymcad_ui_state::Sel::Sketch(si) = *ed.sel else {
        place.clear(); // everything unfinished in the drawing at once (otherwise one of the three is forgotten)
        return;
    };
    offset_box(OffsetCtx { project: &mut *ed.project, edits: &mut *ed.edits, regen: &mut *ed.regen, sel_sk, status: &mut *ed.status, tool_prefs: &mut *looks.tool_prefs }, &*ed.view, si, ctx, rect);
    let pl = &mut qymcad_ui_state::PlaceCtx { place, project: ed.project, view: &*ed.view, regen: ed.regen };
    ellipse_input_popup(pl, ctx, rect, si);
    rect_input_popup(pl, ctx, rect, si);
    rect_dims_popup(pl, ctx, rect, si);
    poly_input_popup(pl, ctx, rect, si);
}

/// A RECTANGLE REOPENED BY A DOUBLE CLICK: its width and its height, typed into the dimensions it holds them by, as the
/// diameter of a circle is. A value typed is the dimension's at once and the rectangle follows - from the corner it was
/// drawn from, or about its centre; Enter, the tick or Esc closes the fields.
pub fn rect_dims_popup(pl: &mut qymcad_ui_state::PlaceCtx, ctx: &egui::Context, rect: Rect, si: usize) {
    let Some(dims) = pl.place.rect_dims() else { return };
    let dim = |p: &qymcad_core::model::Project, ci: usize| match p.sketches.get(si).and_then(|s| s.constraints.get(ci)) {
        Some(c @ qymcad_core::model::Constraint::Distance { .. }) => Some(c.clone()),
        _ => None,
    };
    let (Some(qymcad_core::model::Constraint::Distance { d: w0, a: at, .. }), Some(qymcad_core::model::Constraint::Distance { d: h0, .. })) =
        (dim(pl.project, dims.width), dim(pl.project, dims.height))
    else {
        pl.place.clear(); // the dimensions went from under the fields
        return;
    };
    let want_focus = std::mem::take(&mut pl.place.focus);
    if want_focus {
        pl.place.buf = [qymcad_core::expr::fmt_num(w0), qymcad_core::expr::fmt_num(h0)];
    }
    let Some(corner) = qymcad_ui_state::sketch_pt(pl.project, si, at) else { return };
    let enter = ctx.input(|i| i.key_pressed(egui::Key::Enter));
    let (mut chg, mut close) = (false, false);
    let mut buf = std::mem::take(&mut pl.place.buf);
    egui::Area::new(egui::Id::new(("rectdims", si)))
        .fixed_pos(qymcad_ui_state::clamp_popup((qymcad_ui_state::Sheet { view: *pl.view, rect }).at(corner), rect) + egui::vec2(10.0, -10.0))
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(qymcad_i18n::tr("sk-width-short"));
                    let r0 = qymcad_ui_state::focus_edit(ui, &mut buf[0], 60.0, "", want_focus);
                    ui.label(qymcad_i18n::tr("sk-height-short"));
                    let r1 = qymcad_ui_state::focus_edit(ui, &mut buf[1], 60.0, "", false);
                    chg = r0.changed() || r1.changed();
                    if ((r0.lost_focus() || r1.lost_focus()) && enter) || ui.button(ph::CHECK).clicked() {
                        close = true;
                    }
                });
            });
        });
    if chg {
        let mut set = |ci: usize, text: &str| {
            if let Some(v) = parse_num(pl.project, text).filter(|v| *v > 0.01) {
                if let Some(qymcad_core::model::Constraint::Distance { d, .. }) = pl.project.sketches[si].constraints.get_mut(ci) {
                    *d = v;
                }
            }
        };
        set(dims.width, &buf[0]);
        set(dims.height, &buf[1]);
        pl.project.solve_sketch(si);
        qymcad_ui_state::invalidate(pl.regen);
    }
    pl.place.buf = buf;
    if close || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        pl.place.clear();
    }
}

/// A DOUBLE CLICK ON A SHAPE OPENS ITS SIZE: a side of a rectangle its width and height, a circle its diameter (laid
/// as a dimension when it has none), an arc its radius, the circle a polygon hangs on - or the polygon itself - its
/// radius and turn. Answers whether a shape was there.
pub fn open_shape_size(sk: &mut qymcad_ui_state::SketchCtx, rect: Rect, pos: Pos2, si: usize) -> bool {
    use qymcad_ui_state::{InlineEdit, PlacingShape};
    if let Some((1, eid)) = sketch_hit(&sk.pick(), rect, pos, si) {
        if let Some(dims) = sk.project.rect_dims(si, eid) {
            sk.project.solve_sketch(si);
            sk.place.set(PlacingShape::RectDims(dims));
            sk.place.focus = true;
            return true;
        }
    }
    if let Some(eid) = qymcad_pick::nearest_circle_entity(&sk.pick(), rect, pos, si) {
        // a circle has its diameter edited, or the field of its size opened; an arc the field of its radius
        let center = sk.project.sketches[si].entities.iter().find(|e| e.id == eid).and_then(|e| match e.kind {
            qymcad_core::model::EntityKind::Circle { center, .. } => Some(center),
            _ => None,
        });
        // the circumscribed circle of a polygon (the vertices hang on it) opens the polygon popup (the radius plus the
        // angle), while an ordinary circle gets a diameter
        let is_poly_rim = center.is_some_and(|c| sk.project.sketches[si].constraints.iter().any(|x| matches!(x, qymcad_core::model::Constraint::PointOnCircle { c: cc, .. } if *cc == c)));
        if let (true, Some(c)) = (is_poly_rim, center) {
            sk.place.set(PlacingShape::Poly(c));
            sk.place.focus = true;
        } else if let Some(ci) = center.and_then(|c| sk.project.sketches[si].constraints.iter().position(|x| matches!(x, qymcad_core::model::Constraint::Diameter { c: cc, .. } if *cc == c))) {
            // a circle with its diameter laid: the dimension is edited
            *sk.inline = InlineEdit::Dim(ci);
            sk.dim.focus = true;
        } else {
            // A CIRCLE OR AN ARC WITH NO SIZE OF ITS OWN opens the field of its size, and a size is laid only when a value
            // is typed there - as at a circle just drawn. Reported behaviour: "a double click, if the size was not
            // changed, must not put a dimension line on the circle".
            *sk.inline = InlineEdit::Circle(eid);
            sk.dim.focus = true;
        }
        return true;
    }
    if let Some(cid) = qymcad_ui_state::polygon_under(&*sk.project, &*sk.view, rect, pos, si) {
        sk.place.set(PlacingShape::Poly(cid)); // editing the radius of the construction circle
        sk.place.focus = true;
        return true;
    }
    false
}

/// The ANGLE popup for the rotate tool (`move_op == 3`), placed at the picked centre. Enter or the tick
/// applies `rotate_entities`; editing the value gives a live preview (the ghost in `draw_move_preview`).
pub fn sketch_rotate_popup(sk: &mut qymcad_ui_state::SketchCtx, ctx: &egui::Context, rect: Rect) {
    if sk.armed.move_op() != 3 {
        return;
    }
    let qymcad_ui_state::Sel::Sketch(si) = *sk.sel else { return };
    let Some(base) = sk.tool.move_base else { return };
    let eids: Vec<Id> = sk.sel_sk.items.iter().filter(|(k, _)| *k == 1).map(|(_, id)| *id).collect();
    let texts = qymcad_ui_state::sel_text_indices(&*sk.project, &*sk.sel_sk, si);
    if eids.is_empty() && texts.is_empty() {
        return;
    }
    let rot = &mut *sk.rot;
    let at = (qymcad_ui_state::Sheet { view: *sk.view, rect }).at(base);
    let want_focus = std::mem::take(&mut rot.focus);
    let enter = ctx.input(|i| i.key_pressed(egui::Key::Enter));
    let (mut chg, mut apply, mut got_focus) = (false, false, false);
    let mut buf = std::mem::take(&mut rot.buf);
    // AN ANGLE THAT CANNOT BE TAKEN is refused where it is typed: the reason on the mark beside the field, the tool
    // and what was typed staying. It used to be applied as 0 or taken away with the popup.
    let checked = angle_of(sk.project, &buf);
    egui::Area::new(egui::Id::new(("rotinput", si))).fixed_pos(qymcad_ui_state::clamp_popup(at, rect) + egui::vec2(12.0, -12.0)).order(egui::Order::Foreground).show(ctx, |ui| {
        egui::Frame::popup(ui.style()).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(format!("{} {}", ph::ARROWS_CLOCKWISE, qymcad_i18n::tr("sk-angle-deg")));
                let r = qymcad_ui_state::focus_edit(ui, &mut buf, 64.0, &qymcad_i18n::tr("sk-angle-placeholder"), want_focus);
                chg |= r.changed();
                got_focus |= r.has_focus();
                if r.lost_focus() && enter {
                    apply = true;
                }
                if let Err(msg) = &checked {
                    qymcad_ui_state::tone_mark(ui, &sk.scheme.pal, qymcad_ui_state::Tone::Refusal).on_hover_text(msg);
                }
                if ui.button(ph::CHECK).on_hover_text(qymcad_i18n::tr("sk-apply-enter")).clicked() {
                    apply = true;
                }
            });
        });
    });
    rot.buf = buf;
    if got_focus {
        rot.focus = false;
    }
    if chg || want_focus {
        rot.angle = parse_num(sk.project, &rot.buf.clone()).unwrap_or(0.0); // the live preview
    }
    if let (true, Err(msg)) = (apply, angle_of(sk.project, &sk.rot.buf.clone())) {
        *sk.status = Notice::refusal(msg);
        sk.rot.focus = true;
        return;
    }
    if apply {
        if let Some(v) = parse_num(sk.project, &sk.rot.buf.clone()) {
            // THE BOUNDARY OF AN OPERATION: applying the angle is the act, so it makes one step of undo named
            // after the tool, as the target click of a move does.
            qymcad_ui_state::begin_edit(&mut *sk.edits, &*sk.project, qymcad_i18n::tr("tool-rotate"));
            // WHERE A FREE TURN WOULD HAVE PUT THE POINTS, and where the constraints let them go.
            //
            // A rectangle carries two horizontal and two vertical constraints, so it CANNOT turn: the solver
            // brings it back to upright, merely relocated - measured, a 90 degree turn about zero left the
            // corner at (-5, 5) instead of (0, 0). Saying "Rotated by 90 deg" over that is the same lie the
            // move tool used to tell, and it is told out loud here instead.
            let ids = sk.project.entity_point_ids(si, &eids);
            let at = |p: &qymcad_core::model::Project| -> Vec<(f64, f64)> {
                p.sketches.get(si).map(|s| ids.iter().filter_map(|id| s.points.iter().find(|q| q.id == *id).map(|q| (q.x, q.y))).collect()).unwrap_or_default()
            };
            let before = at(sk.project);
            let (sn, cs) = (v.to_radians().sin(), v.to_radians().cos());
            let want: Vec<(f64, f64)> = before
                .iter()
                .map(|(x, y)| {
                    let (dx, dy) = (x - base.x, y - base.y);
                    (base.x + dx * cs - dy * sn, base.y + dx * sn + dy * cs)
                })
                .collect();
            if !eids.is_empty() {
                sk.project.rotate_entities(si, &eids, base.x, base.y, v);
            }
            for &ti in &texts {
                sk.project.rotate_sketch_text(si, ti, base.x, base.y, v);
            }
            let after = at(sk.project);
            let mean = |a: &[(f64, f64)], b: &[(f64, f64)]| -> f64 {
                if a.len() != b.len() || a.is_empty() {
                    return 0.0;
                }
                a.iter().zip(b).map(|((x0, y0), (x1, y1))| (x1 - x0).hypot(y1 - y0)).sum::<f64>() / a.len() as f64
            };
            let asked = mean(&before, &want); // how far the turn alone would have carried the points
            let off = mean(&after, &want); // how far the result stands from that
                                           // a tenth of what was asked, and at least a hundredth of a millimetre: below that it did turn
            *sk.status = if asked > 1e-9 && off > (asked * 0.1).max(1e-2) {
                Notice::warning(qymcad_i18n::tr("sk-turn-held"))
            } else {
                Notice::info(qymcad_i18n::tr1("sk-rotated-by", "a", &v.to_string()))
            };
            qymcad_ui_state::commit_edit(&mut sk.rebuild());
        }
        *sk.armed = qymcad_ui_state::Armed::None;
        sk.tool.move_base = None;
        qymcad_ui_state::invalidate(&mut *sk.regen);
    }
}

/// The in-place editors of dimension values in the viewport (a double click, or the radius tool).
pub fn dim_editor(sk: &mut qymcad_ui_state::SketchCtx, ctx: &egui::Context, rect: Rect) {
    use qymcad_core::model::{Constraint, EntityKind};
    let qymcad_ui_state::Sel::Sketch(si) = *sk.sel else {
        sk.inline.clear();
        sk.inline.clear();
        return;
    };
    // the radius of a circle or an arc entity
    if let Some(eid) = sk.inline.circle() {
        let info = sk.project.sketches.get(si).and_then(|s| s.entities.iter().find(|e| e.id == eid)).and_then(|e| match e.kind {
            EntityKind::Circle { center, r } => Some((center, r, false)),
            EntityKind::Arc { center, a, .. } => qymcad_ui_state::sketch_pt(&*sk.project, si, a)
                .zip(qymcad_ui_state::sketch_pt(&*sk.project, si, center))
                .map(|(pa, c)| (center, ((pa.x - c.x).powi(2) + (pa.y - c.y).powi(2)).sqrt(), true)),
            _ => None,
        });
        if let (Some((_center, r, is_arc)), Some(cp)) = (info, info.and_then(|(c, ..)| qymcad_ui_state::sketch_pt(&*sk.project, si, c))) {
            let at = (qymcad_ui_state::Sheet { view: *sk.view, rect }).at(Point2::new(cp.x + r, cp.y));
            let mut rr = r;
            let (mut chg, mut close) = (false, false);
            let want_focus = sk.dim.focus;
            let mut got_focus = false;
            let enter = ctx.input(|i| i.key_pressed(egui::Key::Enter));
            // the text buffer of the radius, with auto-focus and Enter, as with a linear dimension
            let mut buf = std::mem::take(&mut sk.dim.buf);
            if want_focus {
                // THROUGH THE COMMON DOOR: no longer than four digits, with no tail. A raw `format!("{}")`
                // printed the whole truth about an f64 - "12.750000000000002" in the radius field.
                buf = qymcad_core::expr::fmt_num(if is_arc { r } else { 2.0 * r });
                // a radius for an arc, a diameter for a circle
                sk.dim.typed = false;
            }
            let mut buf_changed = false;
            egui::Area::new(egui::Id::new(("circedit", si, eid))).fixed_pos(qymcad_ui_state::clamp_popup(at, rect) + egui::vec2(8.0, -10.0)).order(egui::Order::Foreground).show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(if is_arc { qymcad_i18n::tr("sk-radius") } else { qymcad_i18n::tr("sk-diameter") });
                        let rsp = qymcad_ui_state::focus_edit(ui, &mut buf, 80.0, &qymcad_i18n::tr("sk-mm"), want_focus);
                        if rsp.changed() {
                            buf_changed = true;
                        }
                        got_focus = rsp.has_focus();
                        if rsp.lost_focus() && enter {
                            close = true;
                        }
                        if ui.button(ph::CHECK).clicked() {
                            close = true;
                        }
                    });
                });
            });
            if got_focus {
                sk.dim.focus = false;
            }
            sk.dim.buf = buf.clone();
            // APPLIED ON COMMIT, NOT ON EVERY LETTER, exactly as a linear dimension is.
            //
            // Reported behaviour: "you place a dimension, try to type it in, and while you are erasing, 100
            // becomes 1 - and at that moment the geometry gets thrown about". The linear field had already
            // learnt this; the field one over, the radius and diameter of a circle, still rebuilt the
            // circle on every keystroke, so erasing "100" really did build it at 10 and at 1 on the way.
            //
            // The value is evaluated when an expression is typed, and stored in the arc's radius dimension - put
            // there by the tool when the arc had none.
            // the model is not touched while the text is being typed - see below; only a value typed is laid
            sk.dim.typed |= buf_changed;
            if close && sk.dim.typed {
                match parse_num(&*sk.project, &buf) {
                    // A radius of zero or less is not a radius. It used to be clamped to 0.01 silently,
                    // which answers a typing slip with a circle nobody can see.
                    Some(v) if (if is_arc { v } else { v * 0.5 }) > 0.0 => {
                        rr = if is_arc { v } else { v * 0.5 }; // Ø -> r
                        chg = true;
                    }
                    Some(_) => {
                        *sk.status = Notice::refusal(qymcad_i18n::tr("sk-dim-must-be-positive"));
                        close = false; // the field stays open on the text that was refused
                    }
                    None => {}
                }
            }
            if chg {
                // THE SIZE TYPED IS ONE STEP OF UNDO, named as a dimension is
                qymcad_ui_state::begin_edit(&mut *sk.edits, &*sk.project, qymcad_i18n::tr("sk-dim"));
                if is_arc {
                    // a fillet arc has its radius edited through the dimension constraint (parametrically);
                    // failing that, the fillet is recomputed geometrically; failing that, it is a plain arc
                    if !sk.project.set_fillet_radius_dim(si, eid, rr) && !sk.project.set_fillet_radius(si, eid, rr) {
                        sk.project.put_arc_radius_dim(si, eid, rr); // a plain arc gets its dimension, as a circle has
                    }
                } else {
                    // A DIAMETER TYPED IS LAID AS A DIMENSION, of the value typed
                    let centre = sk.project.sketches.get_mut(si).and_then(|s| s.entities.iter_mut().find(|e| e.id == eid)).and_then(|e| match &mut e.kind {
                        EntityKind::Circle { r, center } => {
                            *r = rr;
                            Some(*center)
                        }
                        _ => None,
                    });
                    if let Some(ci) = centre.and_then(|c| sk.project.ensure_diameter(si, c, true)) {
                        if let Some(Constraint::Diameter { d, .. }) = sk.project.sketches[si].constraints.get_mut(ci) {
                            *d = 2.0 * rr;
                        }
                    }
                    // THE SOLVER, not a rebuild. The radius of a circle is a variable of the solve (it is
                    // keyed by the centre), so a radius written straight into the entity is a value the
                    // solver has not seen: a tangency or an equality holding this circle would go on being
                    // drawn as satisfied while it no longer is.
                    sk.project.solve_sketch(si);
                }
                qymcad_ui_state::invalidate(&mut *sk.regen);
                qymcad_ui_state::commit_edit_if_changed(&mut sk.rebuild());
            }
            if close || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                sk.inline.clear();
                sk.dim.buf.clear();
                sk.dim.focus = false;
                sk.dim.typed = false;
            }
        } else {
            sk.inline.clear();
        }
    }
    let Some(ci) = sk.inline.dim() else { return };
    let Some(at) = dim_label_pos(sk, rect, si, ci) else {
        sk.inline.clear();
        return;
    };
    let mut changed = false;
    let mut close = false;
    let want_focus = sk.dim.focus; // focus is requested until the field actually takes it
    let mut got_focus = false;
    let enter = ctx.input(|i| i.key_pressed(egui::Key::Enter));
    // the current state of the dimension: the value, the expression, whether it is driven, whether it is
    // an angle
    let (cur_val, cur_expr, is_driven, is_angle) = match sk.project.sketches[si].constraints.get(ci) {
        // `DistancePL` stores a SIGNED d (which side); the magnitude is what gets shown and edited
        Some(Constraint::DistancePL { d, expr, driven, .. }) => (d.abs(), expr.clone(), driven, false),
        Some(Constraint::Distance { d, expr, driven, .. }) | Some(Constraint::Diameter { d, expr, driven, .. }) | Some(Constraint::EdgeDistance { d, expr, driven, .. }) => {
            (*d, expr.clone(), driven, false)
        }
        Some(Constraint::ArcLength { len, expr, driven, .. }) => (*len, expr.clone(), driven, false),
        Some(Constraint::Angle { deg, expr, driven, .. }) | Some(Constraint::AngleLines { deg, expr, driven, .. }) => (*deg, expr.clone(), driven, true),
        _ => {
            sk.inline.clear();
            return;
        }
    };
    // diameter or radius: which of the two the field shows
    let diam_mode = matches!(sk.project.sketches[si].constraints.get(ci), Some(Constraint::Diameter { diam: true, .. }));
    let is_diameter = matches!(sk.project.sketches[si].constraints.get(ci), Some(Constraint::Diameter { .. }));
    // the buffer of the field: on opening it holds the current expression, or the number
    let mut buf = std::mem::take(&mut sk.dim.buf);
    if want_focus {
        buf = if cur_expr.trim().is_empty() { qymcad_core::expr::fmt_num(cur_val) } else { cur_expr.clone() };
    }
    let eval_res = if buf.trim().is_empty() { None } else { Some(sk.project.eval_expr(&buf)) };
    let (mut toggle_driven, mut toggle_diam) = (false, false);
    let mut name_taken = false;
    let is_edge = matches!(sk.project.sketches[si].constraints.get(ci), Some(Constraint::EdgeDistance { .. }));
    let mut toggle_edge = false;
    let label = if is_angle {
        qymcad_i18n::tr("sk-angle")
    } else if is_diameter {
        if diam_mode { "Ø" } else { "R" }.to_string()
    } else if is_edge {
        qymcad_i18n::tr("sk-tangent-short")
    } else {
        qymcad_i18n::tr("sk-dim")
    };
    // A DRIVER BELONGS TO ANY DIMENSION, NOT ONLY TO A LINEAR ONE.
    //
    // This used to read "only `Constraint::Distance`", and the name field appeared solely on a distance
    // between two points. The question came up plainly: why does a drive field exist in some places and
    // not in others? There was no explainable logic behind it: the driver name WAS STORED as a pair of
    // points, so an angle, a diameter, a distance to a line, an arc length or a tangent gap simply had
    // nothing to be named by. Now a driver is identified by a set of entities, and any dimension can be
    // named.
    let dim_refs: Option<Vec<Id>> = sk.project.sketches[si].constraints.get(ci).and_then(Project::dim_refs);
    let sid = sk.project.sketches[si].id;
    let cur_name: String = dim_refs
        .as_ref()
        .map(|r| {
            let t = qymcad_core::model::DimTarget::Sketch { sketch: sid, refs: Project::dim_key_pub(r) };
            qymcad_i18n::name(&sk.project.name_of_target(&t))
        })
        .unwrap_or_default();
    // THE TEXT LIVES IN THE BUFFER OF THE FIELD ITSELF WHILE IT IS BEING TYPED. The model is touched
    // once, on Enter.
    let key_refs = dim_refs.clone().unwrap_or_default();
    // WHAT WAS ASKED FOR: a new driver name. It is applied AFTER the drawing - during it the document is
    // lent to the field (which reads the project), and there is no reason to change the model mid-frame.
    let mut name_commit: Option<String> = None;
    let mut name_owner: Option<String> = None;
    egui::Area::new(egui::Id::new(("dimedit", si, ci))).fixed_pos(qymcad_ui_state::clamp_popup(at, rect) + egui::vec2(8.0, -10.0)).order(egui::Order::Foreground).show(ctx, |ui| {
        egui::Frame::popup(ui.style()).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(label);
                if is_diameter && ui.button(if diam_mode { "->R" } else { "->Ø" }).on_hover_text(qymcad_i18n::tr("sk-toggle-rad-dia")).clicked() {
                    toggle_diam = true;
                }
                if is_edge && ui.button(qymcad_i18n::tr("sk-edge-toggle")).on_hover_text(qymcad_i18n::tr("sk-near-far-edge")).clicked() {
                    toggle_edge = true;
                }
                if *is_driven {
                    ui.label(egui::RichText::new(qymcad_i18n::tr1("sk-driven-value", "v", &qymcad_i18n::num(cur_val, 3))).color(sk.scheme.pal.text_dim()));
                } else {
                    // ONE field for the whole project: a number or an expression (50, w/2, len+5), the list
                    // of drivers, auto-focus with the previous text selected, and Enter to apply.
                    let fid = egui::Id::new(("dimval", si, ci));
                    let o = qymcad_ui_state::expr_field_autofocus(ui, &*sk.project, fid, &buf, 100.0, &qymcad_i18n::tr("sk-expr-example"), want_focus);
                    buf = o.text;
                    got_focus = o.resp.has_focus();
                    if o.committed && enter {
                        close = true;
                    }
                }
                if ui.selectable_label(*is_driven, qymcad_i18n::tr("sk-ref-short")).on_hover_text(qymcad_i18n::tr("sk-driven-hint")).clicked() {
                    toggle_driven = true;
                }
                if ui.button(ph::CHECK).clicked() {
                    close = true;
                }
            });
            if !is_driven {
                match &eval_res {
                    Some(Ok(v)) => {
                        ui.label(egui::RichText::new(format!("= {v:.3}")).weak().small());
                    }
                    Some(Err(e)) => {
                        // THROUGH THE COMMON DOOR: the `Display` of the error is in one language only, and
                        // an interface in another showed a raw "unexpected token: /".
                        qymcad_ui_state::say(ui, &sk.scheme.pal, &qymcad_ui_state::Notice::refusal(qymcad_i18n::error_words::expr_error_text(e)), qymcad_ui_state::NoticeSize::Small);
                    }
                    None => {}
                }
                ui.label(egui::RichText::new(qymcad_i18n::tr("sk-enter-apply-expr")).weak().small());
            }
            if dim_refs.is_some() {
                // WHETHER THE NAME WILL DO. An empty one is legitimate: the dimension simply stops being a
                // driver.
                let ok = |nm: &str| nm.is_empty() || (qymcad_core::drivers::check_ident(nm).is_ok() && !sk.project.driver_name_taken(nm, sid, &key_refs));
                ui.horizontal(|ui| {
                    // THE CAPTION IS TRANSLATABLE. A raw "driver:" used to stand here - the only string of
                    // the popup that went around the language catalogue.
                    ui.label(qymcad_i18n::tr("sk-driver-label")).on_hover_text(qymcad_i18n::tr("sk-driver-name-hint"));
                    let id = egui::Id::new(("dimdrv", si, ci));
                    let r = qymcad_ui_state::name_field(ui, &*sk.project, id, &cur_name, 110.0, &qymcad_i18n::tr("sk-name-placeholder"), &ok);
                    let nm = r.text.trim().to_string();
                    if r.committed && nm != cur_name.trim() {
                        name_commit = Some(nm.clone());
                    }
                    // A TAKEN NAME GIVES A YELLOW LINE BELOW AND BLOCKS APPLYING. Not a single letter is
                    // lost while this happens: type `len`, `lena` or `length` - until the name is free, the
                    // popup simply refuses to apply and says why.
                    name_taken = !nm.is_empty() && !ok(&nm);
                    if name_taken {
                        name_owner = sk.project.name_owner(&nm).map(|o| if o.path.is_empty() { qymcad_i18n::tr("par-owner-project") } else { o.path });
                    }
                });
                if name_taken {
                    // WHOSE NAME IT IS, not merely "taken": otherwise the namesake has to be hunted for
                    // across the whole project by hand. A name with no owner is simply unusable in itself.
                    let msg = match &name_owner {
                        Some(w) => qymcad_i18n::tr2("par-name-taken", "name", "", "where", w),
                        None => qymcad_i18n::tr("sk-driver-name-bad"),
                    };
                    qymcad_ui_state::say(ui, &sk.scheme.pal, &qymcad_ui_state::Notice::warning(msg), qymcad_ui_state::NoticeSize::Small).on_hover_text(qymcad_i18n::tr("sk-driver-name-taken-hint"));
                }
            }
        });
    });
    if got_focus || *is_driven {
        sk.dim.focus = false; // focus was taken (or there is nothing to focus) - stop asking
    }
    sk.dim.buf = buf.clone();
    // a snapshot of the dimension AND of the point positions BEFORE the edit, so it can be ROLLED BACK if
    // the new value CONFLICTS (over-defines the sketch). Otherwise the solver silently tilted a vertical
    // or moved a fixed point - the soft least-squares compromise. An inconsistent dimension is rejected
    // rather than allowed to break the sketch.
    let old_con = sk.project.sketches[si].constraints.get(ci).cloned();
    // WHAT THE FIELD DOES TO THE DOCUMENT IS ONE OPERATION, named: the value of a dimension just made joins the step
    // that made it; any other change of a dimension is a step of its own rather than an unnamed one the frame notices
    let fold = sk.dim.fresh.is_some_and(|(fc, steps)| fc == ci && steps == sk.edits.undo.len());
    // THE TEXT DIFFERS FROM WHAT IS STORED, so there is something to apply on commit. TEXTS are compared
    // rather than the number parsed again: parsing a value in the sketcher must go through one door
    // (`parse_num`), and a second `parse::<f64>()` is caught at once by the ratchet in `expr_fields.rs`.
    let shown_before = if cur_expr.trim().is_empty() { qymcad_core::expr::fmt_num(cur_val) } else { cur_expr.clone() };
    let value_differs = buf.trim() != shown_before.trim();
    // A VALUE THAT CANNOT BE TAKEN KEEPS THE FIELD OPEN with what was typed in it, and the status line says why: an
    // empty field, or a formula that does not evaluate - an unknown name included, since a name that is not there
    // yet would hold the dimension at a value nobody typed
    if close && !is_driven && value_differs {
        let why = match &eval_res {
            None => Some(qymcad_i18n::tr("sk-dim-value-empty")),
            Some(Err(e)) => Some(qymcad_i18n::error_words::expr_error_text(e)),
            Some(Ok(_)) => None,
        };
        if let Some(why) = why {
            close = false;
            *sk.status = Notice::refusal(why);
        }
    }
    let apply_value = close && !is_driven && value_differs;
    let acting = !fold && (apply_value || toggle_driven || toggle_diam || toggle_edge);
    if acting {
        qymcad_ui_state::begin_edit(&mut *sk.edits, &*sk.project, qymcad_i18n::tr("sk-dim"));
    }
    // apply the changes to the constraint
    let mut new_buf: Option<String> = None; // refresh the field after a value edit (a diameter/radius swap, say)
    if let Some(c) = sk.project.sketches[si].constraints.get_mut(ci) {
        if toggle_driven {
            match c {
                Constraint::Distance { driven, .. }
                | Constraint::DistancePL { driven, .. }
                | Constraint::Angle { driven, .. }
                | Constraint::Diameter { driven, .. }
                | Constraint::AngleLines { driven, .. }
                | Constraint::ArcLength { driven, .. }
                | Constraint::EdgeDistance { driven, .. } => *driven = !*driven,
                _ => {}
            }
            changed = true;
        }
        if toggle_diam {
            if let Constraint::Diameter { d, diam, expr, .. } = c {
                *d = if *diam { *d * 0.5 } else { *d * 2.0 }; // diameter to radius: the value is recomputed
                *diam = !*diam;
                if expr.trim().is_empty() {
                    new_buf = Some(qymcad_core::expr::fmt_num(*d)); // refresh the field
                }
            }
            changed = true;
        }
    }
    // THE VALUE IS APPLIED ON COMMIT, NOT ON EVERY LETTER.
    //
    // Every keystroke used to edit the constraint, solve the sketch again and mark the document for a
    // rebuild: typing "125" meant three rebuilds, and on the intermediate "1" and "12" the sketch honestly
    // rebuilt itself to a different size. The answer is visible anyway - the line "= 42.500" lives under
    // the field, and it costs the document nothing.
    //
    // AND A LENGTH THAT CANNOT EXIST IS REFUSED rather than handed over - see `set_dim_value`. A typed
    // EXPRESSION is judged by what it evaluates to now: it may change later with its parameter, and that is
    // the parameter's business, but a formula that reads -5 today is refused today.
    if apply_value {
        let t = buf.trim();
        // THE ONE BARE PARSE OF THE SKETCHER, and it is asked once: is this a number or a formula? The
        // answer decides both what value goes in and whether the formula is kept beside it.
        let as_number = t.parse::<f64>().ok();
        match as_number.or_else(|| parse_num(&*sk.project, t)) {
            Some(v) => {
                let took = set_dim_value(&mut *sk.project, si, ci, v);
                if took.is_ok() {
                    if as_number.is_none() {
                        // a formula keeps its text beside the value it evaluates to now
                        if let Some(e) = sk.project.sketches[si].constraints.get_mut(ci).and_then(Constraint::expr_mut) {
                            *e = buf.clone();
                        }
                    }
                    changed = true;
                } else if let Err(key) = took {
                    // refused: the field stays open with what was typed, and the status line says why
                    *sk.status = Notice::refusal(qymcad_i18n::tr1(key, "max", &qymcad_i18n::num(MAX_SKETCH_LENGTH, 0)));
                    close = false;
                }
            }
            // not a number and not a formula that evaluates yet - the text is kept, and it will be read
            // again when the parameter it names appears
            None => {
                if let Some(e) = sk.project.sketches[si].constraints.get_mut(ci).and_then(Constraint::expr_mut) {
                    *e = buf.clone();
                    changed = true;
                }
            }
        }
    }
    if let Some(nb) = new_buf {
        sk.dim.buf = nb; // the field shows the new value (after a diameter/radius swap)
    }
    if toggle_edge {
        // near edge against far edge: the non-zero m values are inverted and d is recomputed from the
        // geometry, so nothing jumps
        let edge = match sk.project.sketches[si].constraints.get(ci) {
            Some(Constraint::EdgeDistance { c1, c2, m1, m2, .. }) => Some((c1, c2, m1, m2)),
            _ => None,
        };
        if let Some((c1, c2, m1, m2)) = edge {
            let (nm1, nm2) = (if *m1 != 0 { -*m1 } else { 0 }, if *m2 != 0 { -*m2 } else { 0 });
            let nd = sk.project.measure_edge_distance(si, *c1, nm1, *c2, nm2);
            if let Some(Constraint::EdgeDistance { m1, m2, d, expr, .. }) = sk.project.sketches[si].constraints.get_mut(ci) {
                *m1 = nm1;
                *m2 = nm2;
                *d = nd;
                expr.clear();
            }
            changed = true;
        }
    }
    // THE DRIVER NAME GOES IN AS ONE OPERATION, ON ENTER.
    //
    // This was the main trouble that got reported: the name was written into the model on EVERY letter,
    // and `mark_param_dependents_dirty()` was called right after, marking EVERY node carrying dimensions
    // dirty - that is, the whole project was rebuilt per letter. Now the edit arrives once.
    if let (Some(refs), Some(nm)) = (dim_refs.clone(), name_commit) {
        let mut ed = qymcad_ui_state::edit_over(sk.rebuild(), qymcad_i18n::tr("sk-driver-step"));
        let old = cur_name.trim().to_string();
        if !old.is_empty() && !nm.is_empty() {
            // RENAMING CARRIES THE FORMULAS WITH IT - otherwise the expressions stay pointed at a name
            // that no longer exists, and the model breaks silently.
            let _ = ed.project().rename_driver(&old, &nm);
        } else {
            ed.project().add_named_dim(nm.clone(), sid, refs);
        }
        drop(ed);
        // ONLY WHAT DEPENDS ON THIS NAME is marked, not the whole timeline of every body; it is rebuilt when the sketch
        // closes (`rebuild_after_sketch`). Asked for here, a name nothing used yet rebuilt 0 nodes for 0.8 s under the
        // sign of a rebuild.
        if !nm.is_empty() {
            sk.project.mark_param_dependents_dirty_for(&nm);
        }
    }
    if changed {
        // a snapshot of the positions BEFORE solving (the sketch is consistent right now)
        let old_pts: Vec<(f64, f64)> = sk.project.sketches[si].points.iter().map(|p| (p.x, p.y)).collect();
        let resid = sk.project.solve_sketch(si);
        // a large residual means the system is INCONSISTENT (the new dimension conflicts with a vertical,
        // a fixed point or another constraint). The solver would converge to about 1e-6; a threshold of
        // 1e-2 mm is above the noise and below any real conflict. The dimension and the positions are
        // rolled back and solved again, so the sketch returns to a consistent state instead of breaking
        // silently.
        if resid > 1e-2 {
            if let (Some(oc), Some(c)) = (old_con.clone(), sk.project.sketches[si].constraints.get_mut(ci)) {
                *c = oc;
            }
            for (p, (x, y)) in sk.project.sketches[si].points.iter_mut().zip(old_pts) {
                p.x = x;
                p.y = y;
            }
            sk.project.solve_sketch(si);
            *sk.status = Notice::refusal(qymcad_i18n::tr1("sk-dim-incompatible", "r", &qymcad_i18n::num(resid, 2)));
        }
        qymcad_ui_state::invalidate(&mut *sk.regen);
    }
    // the value typed into a dimension just made is part of making it
    if changed && fold {
        qymcad_ui_state::fold_into_last_step(&mut *sk.edits, &*sk.project);
    }
    if acting {
        qymcad_ui_state::commit_edit_if_changed(&mut sk.rebuild());
    }
    if close || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        sk.inline.clear();
        sk.dim.buf.clear();
        sk.dim.focus = false;
        sk.dim.fresh = None;
        // a driver whose value moved reaches the bodies that consume it when the sketch closes: the scheduler compares
        // the values against those of the last rebuild (`mark_changed_params_dirty`)
    }
}
