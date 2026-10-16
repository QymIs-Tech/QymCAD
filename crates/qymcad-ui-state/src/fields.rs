//! THE FIELDS OF EXPRESSIONS: at the geometry, in the bar, their lists of names.

use super::*;

/// The TextEdit used by dimension popups: it SELECTS all the text when it gains focus (automatically,
/// or by a click or Tab) — a new value overwrites the old one without Ctrl+A. `autofocus` is one-shot:
/// it asks for focus on this frame. Returns the Response (for lost_focus and Enter). The same one is
/// used in sketches, parts and assemblies.
/// THE FIELD OF A NAME BEING CHANGED IN PLACE: when it opens (`opening`) it takes the focus with the whole name
/// selected, so what is typed stands INSTEAD of the old name, as in any tree. Reported behaviour: after F2 and
/// "Contour" the sketch was called "Sketch 1Contour".
pub fn rename_field(ui: &mut egui::Ui, text: &mut String, width: f32, opening: bool) -> egui::Response {
    let mut out = egui::TextEdit::singleline(text).desired_width(width).show(ui);
    if opening {
        out.response.request_focus();
        let range = egui::text::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(text.chars().count()));
        out.state.cursor.set_char_range(Some(range));
        out.state.store(ui.ctx(), out.response.id);
    }
    out.response.response
}

pub fn focus_edit(ui: &mut egui::Ui, text: &mut String, width: f32, hint: &str, autofocus: bool) -> egui::Response {
    let mut out = egui::TextEdit::singleline(text).desired_width(width).hint_text(hint).show(ui);
    if autofocus {
        out.response.request_focus();
    }
    if autofocus || out.response.gained_focus() {
        let end = text.chars().count();
        let range = egui::text::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(end));
        out.state.cursor.set_char_range(Some(range));
        out.state.store(ui.ctx(), out.response.id);
    }
    // ENTER RELEASES THE FOCUS.
    //
    // While the field holds it, `wants_keyboard_input` suppresses ALL the hotkeys and the command's
    // own Enter: a number is typed, Enter is pressed a second time — and nothing happens, leaving
    // the tick mark to be aimed at with the mouse. Releasing the focus gives the keyboard back to
    // the command: the next Enter applies it, and letters open the sub-modes again (re-pick the
    // contour and so on).
    if out.response.lost_focus() {
        // `lost_focus` also happens on a click elsewhere — then there is nothing to release, the focus has already gone
    } else if ui.input(|i| i.key_pressed(egui::Key::Enter)) && out.response.has_focus() {
        out.response.surrender_focus();
    }
    out.response.response
}

pub fn dim_expr_field_in(rc: &mut RebuildCtx, ui: &mut egui::Ui, id: Id, key: &str, place: &str) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("ƒ=").weak());
        // THE SAME FIELD AS ON A SKETCH DIMENSION AND IN THE PARAMETER TABLE. It was asked for plainly:
        // features must have all of this too. Different behaviour in two similar places is a defect, even
        // when both of them work.
        let model = rc.project.feat_dim(id, key).unwrap_or("").to_string();
        // THE WIDGET NAME COMES FROM THE PLACE IT IS DRAWN IN. An absolute key ("featdim", the joint, the
        // slot) coincided between the joint popup and the row of the right-hand panel, and the two can be
        // on screen AT THE SAME TIME: egui honestly complained in red right in the frame - "First use of
        // widget ID... Second use of widget ID D898..." - and one of the two fields stopped responding.
        // The name of the parent is appended to the key, and the two places stop arguing.
        let fid = if place.is_empty() { egui::Id::new(("featdim", id, key)) } else { egui::Id::new(("featdim", id, key, place)) };
        let o = expr_field(ui, rc.project, fid, &model, 120.0, &qymcad_i18n::tr("sk-expr-placeholder"));
        if o.committed && o.text != model {
            // ONE EDIT, ONE UNDO STEP. The edit used to go into the model on losing focus, past
            // `App::edit`, and Ctrl+Z never saw it.
            begin_edit(rc.edits, rc.project, qymcad_i18n::tr("par-edit-step"));
            rc.project.set_feat_dim(id, key, o.text.clone());
            commit_edit(rc);
            mark_dirty_for_rebuild(rc); // the document is marked; the planner does the counting
        }
        // WHAT CAME OUT OF WHAT WAS TYPED, IN WORDS, RIGHT HERE.
        //
        // The field can REFUSE: a bad expression does not reach the model, the letters survive and the
        // caret stays where it was (`ExprOut::refused`). But there was nobody to say WHAT was wrong, and
        // a refusal looked like the program not listening: Enter is pressed, nothing happens, no
        // explanation follows. Exactly the silent refusal that counts as the worst answer in this tree.
        //
        // The speaker (`expr_value_label`) had been written and NEVER ONCE CALLED - a compiler warning
        // said the method was never used, and it had drowned among 175 others. This is where it gets
        // wired up: for feature dimensions and joint values it is the only place an answer is visible.
        expr_value_label(&DrawCtx { cam: rc.cam, set: rc.set, scheme: rc.scheme, project: rc.project, active_path: rc.active_path }, ui, &o.text);
        // A DRIVER NAME BELONGS TO A FEATURE PARAMETER TOO. It was asked for plainly: features must have
        // all of this, not sketches alone. The field is the same, in the same place, with the same
        // behaviour: a buffer, a refusal that loses no letters, and the owner named.
        let target = qymcad_core::model::DimTarget::Feature { node: id, key: key.to_string() };
        let cur_name = rc.project.name_of_target(&target);
        // THE NAME CHECK IS COMPUTED BEFORE THE EDIT. The closure holds `rc.project` while applying
        // asks for `&mut self`: both cannot be borrowed at once, so the verdict is computed up front and
        // the ready answer is what gets used further on.
        ui.label(qymcad_i18n::tr("sk-driver-label")).on_hover_text(qymcad_i18n::tr("sk-driver-name-hint"));
        let nid = if place.is_empty() { egui::Id::new(("featdrv", id, key)) } else { egui::Id::new(("featdrv", id, key, place)) };
        let rn = {
            let ok = |nm: &str| nm.is_empty() || (qymcad_core::drivers::check_ident(nm).is_ok() && !rc.project.driver_name_taken_by(nm, &target));
            name_field(ui, rc.project, nid, &cur_name, 100.0, &qymcad_i18n::tr("sk-name-placeholder"), &ok)
        };
        let nm = rn.text.trim().to_string();
        let name_ok = nm.is_empty() || (qymcad_core::drivers::check_ident(&nm).is_ok() && !rc.project.driver_name_taken_by(&nm, &target));
        if rn.committed && nm != cur_name.trim() {
            begin_edit(rc.edits, rc.project, qymcad_i18n::tr("sk-driver-step"));
            let old = cur_name.trim().to_string();
            if !old.is_empty() && !nm.is_empty() {
                let _ = rc.project.rename_driver(&old, &nm);
            } else {
                rc.project.name_dim(nm.clone(), target.clone());
            }
            commit_edit(rc);
            if !nm.is_empty() {
                rc.project.mark_param_dependents_dirty_for(&nm);
            }
            mark_dirty_for_rebuild(rc);
        }
        if !nm.is_empty() && !name_ok {
            let who = rc.project.name_owner(&nm).map(|o| if o.path.is_empty() { qymcad_i18n::tr("par-owner-project") } else { o.path });
            let msg = match who {
                Some(w) => qymcad_i18n::tr2("par-name-taken", "name", &nm, "where", &w),
                None => qymcad_i18n::tr("sk-driver-name-bad"),
            };
            crate::say(ui, &rc.scheme.pal, &crate::Notice::warning(msg), crate::NoticeSize::Small).on_hover_text(qymcad_i18n::tr("sk-driver-name-taken-hint"));
        }
        let shown = o.text;
        if !shown.trim().is_empty() {
            match qymcad_core::expr::eval(&shown, &rc.project.param_map()) {
                Ok(v) => {
                    ui.label(egui::RichText::new(format!("= {v:.3}")).weak().small());
                }
                Err(_) => {
                    crate::tone_mark(ui, &rc.scheme.pal, crate::Tone::Refusal);
                }
            }
        }
    });
}

/// The caption "= 42.500" or the reason for the error in words. One for every field: it is computed
/// and shown the same way wherever the field stands.
pub fn expr_value_label(dc: &DrawCtx, ui: &mut egui::Ui, text: &str) {
    if text.trim().is_empty() {
        return;
    }
    match dc.project.eval_expr(text) {
        Ok(v) => {
            ui.label(egui::RichText::new(format!("= {v:.3}")).weak().small());
        }
        Err(e) => {
            // THE REASON IN WORDS AND NOT AN ICON: what exactly did not add up is what needs
            // knowing.
            let msg = qymcad_i18n::error_words::expr_error_text(&e);
            crate::say(ui, &dc.scheme.pal, &crate::Notice::refusal(msg.clone()), crate::NoticeSize::Small).on_hover_text(&msg);
        }
    }
}

/// THE EXPRESSION FIELD. `model` is what is written in the document right now; the field shows it
/// until an edit is begun.
pub fn expr_field(ui: &mut egui::Ui, project: &Project, id: egui::Id, model: &str, w: f32, hint: &str) -> ExprOut {
    field(ui, project, id, model, w, hint, FieldRules { valid: &|_| true, list: NameList::Offered, focus: Focus::Waits })
}

/// THE NAME FIELD. The same thing, but WITHOUT the list of drivers.
///
/// A name is not a formula: there is nothing to substitute other names into it, and the list would also
/// take Enter for itself — the very key a name is confirmed with. In grown-up CAD autocompletion lives
/// in the expression field only, and that is no trifle: a measurement showed that with the list open on
/// the name "h" Enter went to the list and the edit was not applied at all.
pub fn name_field(ui: &mut egui::Ui, project: &Project, id: egui::Id, model: &str, w: f32, hint: &str, valid: &dyn Fn(&str) -> bool) -> ExprOut {
    field(ui, project, id, model, w, hint, FieldRules { valid, list: NameList::Not, focus: Focus::Waits })
}

/// Whether a field offers the names of the parameters while a name is typed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NameList {
    Offered,
    Not,
}

/// Whether a field takes the keyboard as soon as it is drawn, selecting what it holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Focus {
    Takes,
    Waits,
}

/// How a field treats what is typed into it: what it accepts, whether it offers names, whether it takes the keyboard.
pub struct FieldRules<'a> {
    pub valid: &'a dyn Fn(&str) -> bool,
    pub list: NameList,
    pub focus: Focus,
}

/// THE SAME FIELD, BUT WITH A CHECK BEFORE THE COMMIT.
///
/// `valid` decides whether what was typed can be accepted. A refusal DOES NOT ERASE the text and does
/// not release the focus — otherwise out comes exactly the reported trouble: typing `len` and having
/// the `n` deleted automatically. A person finishes the name or presses Escape, and meanwhile the
/// program says what is wrong.
pub fn field(ui: &mut egui::Ui, project: &Project, id: egui::Id, model: &str, w: f32, hint: &str, rules: FieldRules) -> ExprOut {
    let FieldRules { valid, list, focus } = rules;
    let (with_list, autofocus) = (list == NameList::Offered, focus == Focus::Takes);
    let mut st: FieldState = ui.data_mut(|d| d.get_temp(id)).unwrap_or_default();
    // A VALUE SET FROM ELSEWHERE REACHES A FIELD NOBODY HAS TYPED INTO: the field that took the focus when the tool
    // opened keeps its text as a buffer, and a pick that sizes the tool (a thread sized to the cylinder picked) was
    // written back over by that stale buffer. What a person typed is never replaced.
    if st.buf.is_some() && st.buf == st.base && st.base.as_deref() != Some(model) {
        (st.buf, st.base) = (None, None);
    }
    let mut buf = st.buf.clone().unwrap_or_else(|| model.to_string());

    // THE KEYS ARE TAKEN BEFORE THE FIELD IS DRAWN. While the list is open the arrows belong to IT;
    // otherwise they simply move the caret in the text and a row cannot be chosen from the keyboard —
    // which is how it used to be.
    let focused = ui.memory(|m| m.has_focus(id));
    let (mut go_down, mut go_up, mut take, mut esc, mut open_list) = (false, false, false, false, false);
    // A NAME TYPED IN FULL HAS NOTHING LEFT TO INSERT: with the first row - untouched by the arrows - already the word
    // under the caret, Enter is left to the field and applies the value; a second Enter for "pa" typed whole was a key
    // pressed for nothing. A row reached with the arrows is a choice, and Enter inserts it.
    let whole = st.open && st.sel == 0 && {
        let caret = caret_byte(ui.ctx(), id, &buf);
        let word = current_token(&buf, caret).2;
        project.drivers_matching(word).get(st.sel).is_some_and(|d| d.name == word)
    };
    if focused {
        ui.input_mut(|i| {
            open_list = i.consume_key(egui::Modifiers::COMMAND, egui::Key::Space);
            esc = i.consume_key(egui::Modifiers::NONE, egui::Key::Escape);
            if st.open {
                go_down = i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown);
                go_up = i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp);
                // with the list open, Enter and Tab INSERT what is chosen rather than closing the
                // popup of the tool: the formula is still being written.
                take = (!whole && i.consume_key(egui::Modifiers::NONE, egui::Key::Enter)) || i.consume_key(egui::Modifiers::NONE, egui::Key::Tab);
            }
        });
    }
    // ESCAPE IS DECIDED BY THE STATE FROM THE PREVIOUS FRAME: if the list is open it gets closed,
    // otherwise the edit is cancelled. Grown-up CAD does the same: the first Escape removes the list and
    // the popup of the tool stays.
    let close_list = esc && st.open;
    let escaped = esc && !st.open;

    let h = ui.spacing().interact_size.y;
    // `add_sized` AND NOT `desired_width`: inside an `egui::Grid` a request for width is ignored and the
    // field collapses to 32 points — measured, and that is exactly why the parameters window stayed
    // narrow.
    let resp = ui.add_sized(egui::vec2(w, h), egui::TextEdit::singleline(&mut buf).id(id).hint_text(hint));
    // IT STEPS INTO THE FIELD ITSELF AND SELECTS THE FORMER VALUE — the way the dimension popup opened
    // before it moved onto the shared field. The selection is set through the state of the `TextEdit`:
    // `add_sized` returns only the response, yet the width has to be given to the field with it (inside
    // a grid `desired_width` is ignored).
    if autofocus {
        resp.request_focus();
        if let Some(mut ts) = egui::TextEdit::load_state(ui.ctx(), id) {
            let end = buf.chars().count();
            ts.cursor.set_char_range(Some(egui::text::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(end))));
            ts.store(ui.ctx(), id);
        }
    }

    // WHERE THE CARET STANDS — asked of the field itself, right after it has been drawn. The list and the
    // insertion both follow the word under the caret, and a formula is edited in its middle as often as at
    // its end.
    let caret = caret_byte(ui.ctx(), id, &buf);

    if resp.gained_focus() || resp.changed() {
        if st.buf.is_none() {
            st.base = Some(model.to_string());
        }
        st.buf = Some(buf.clone());
    }
    if resp.changed() {
        st.sel = 0;
        st.open = false;
        // THE LIST OPENS ON TYPING AND NOT ON FOCUS. The former one climbed onto the screen at a single
        // click in the field and covered the geometry, though nothing had been asked yet.
        st.open = with_list && !current_token(&buf, caret).2.is_empty();
    }
    if open_list && with_list {
        st.open = true;
    }
    if close_list {
        st.open = false;
    }

    let hits = if st.open { project.drivers_matching(current_token(&buf, caret).2) } else { Vec::new() };
    if hits.is_empty() {
        st.open = false;
    }
    if st.open {
        note_list_open(ui.ctx());
    }
    if !hits.is_empty() {
        st.sel = st.sel.min(hits.len().min(LIST_MAX_ROWS) - 1);
        if go_down {
            st.sel = (st.sel + 1).min(hits.len().min(LIST_MAX_ROWS) - 1);
        }
        if go_up {
            st.sel = st.sel.saturating_sub(1);
        }
    }

    // THE LIST GOES IN A LAYER ABOVE THE POPUPS OF THE TOOLS. Dimension popups live in
    // `Order::Foreground`, and the former list, drawn there too, went BEHIND them: within one layer the
    // area being interacted with wins. `Order::Tooltip` is higher, so the list is always visible.
    let mut picked: Option<String> = None;
    if !hits.is_empty() {
        let area = egui::Area::new(id.with("drv")).order(egui::Order::Tooltip).fixed_pos(resp.rect.left_bottom() + egui::vec2(0.0, 2.0)).constrain(true);
        area.show(ui.ctx(), |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_min_width(resp.rect.width().max(240.0));
                egui::ScrollArea::vertical().max_height(LIST_MAX_H).show(ui, |ui| {
                    for (k, d) in hits.iter().take(LIST_MAX_ROWS).enumerate() {
                        let row = ui
                            .horizontal(|ui| {
                                // THE ONE ENTER WILL TAKE IS HIGHLIGHTED. Without this the arrows
                                // move something invisible and the key is pressed blind.
                                let hit = ui.selectable_label(k == st.sel, egui::RichText::new(&d.name).strong()).clicked();
                                if !d.path.is_empty() {
                                    // THE PATH ANSWERS "WHICH OF THE NAMESAKES". On an ambiguous one
                                    // it is highlighted too: a bare name in a formula will take who
                                    // knows which.
                                    let mut t = egui::RichText::new(&d.path).small();
                                    t = if d.ambiguous { t.color(crate::tone_colour_in(ui, crate::Tone::Warning)) } else { t.weak() };
                                    ui.label(t);
                                }
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    let val = d.value.map(|v| format!("{v:.3}")).unwrap_or_else(|| qymcad_i18n::tr("par-no-value"));
                                    ui.label(egui::RichText::new(val).weak().small());
                                });
                                hit
                            })
                            .inner;
                        if row {
                            picked = Some(d.name.clone());
                        }
                    }
                    if hits.len() > LIST_MAX_ROWS {
                        ui.label(egui::RichText::new(qymcad_i18n::tr1("par-more", "n", &(hits.len() - LIST_MAX_ROWS).to_string())).weak().small());
                    }
                });
            });
        });
        if take {
            picked = hits.get(st.sel).map(|d| d.name.clone());
        }
    }

    let took = picked.is_some();
    if let Some(name) = picked {
        let (next, at) = insert_driver(&buf, &name, caret);
        buf = next;
        st.buf = Some(buf.clone());
        st.open = false;
        // THE CARET GOES BEHIND THE INSERTED NAME so that typing carries on from there and not from the
        // end of the line.
        if let Some(mut ts) = egui::TextEdit::load_state(ui.ctx(), id) {
            let ch = egui::text::CCursor::new(buf[..at].chars().count());
            ts.cursor.set_char_range(Some(egui::text::CCursorRange::two(ch, ch)));
            ts.store(ui.ctx(), id);
        }
        // THE FOCUS IS RETURNED TO THE FIELD. A click on a row of the list takes it away while the
        // formula is still being written — and without the return the next letter would fly off
        // nowhere.
        resp.request_focus();
    }

    // THE KEYS ARE LEFT WITH THE FIELD AND NOT WITH THE FOCUS SYSTEM.
    //
    // A measurement (a probe inside a frame): on the frame carrying Escape the field is already NOT
    // focused, though nobody left it — `focused=false, esc=true, lost=true`. The cause is egui itself:
    // it extinguishes focus on Escape at the start of the frame, before any widget. While that holds,
    // the field can neither close its list nor tell a cancellation from a commit: Escape would look like
    // a loss of focus, that is, like AGREEMENT.
    //
    // The regular remedy is the filter lock: `escape` stays with the field always, `tab` while the list
    // is open (there Tab inserts what is chosen); with the list closed Tab walks the fields again as it
    // ought to.
    if ui.memory(|m| m.has_focus(id)) {
        let filter = egui::EventFilter { tab: st.open, horizontal_arrows: true, vertical_arrows: true, escape: true };
        ui.memory_mut(|m| m.set_focus_lock_filter(id, filter));
    }

    // COMMIT AND CANCELLATION. With the list CLOSED, Escape cancels the whole edit; with it open Escape
    // closes the list (above) and the popup of the tool stays where it is.
    // THE COMMIT IS AN EDGE, NOT A STATE. It used to hang on `lost_focus()` alone, and that worked while
    // the flag lasted exactly one frame. Since egui 0.35 it stays raised longer, and the edit went into
    // the model TWICE - the guard beside this file caught it on the very first run after the upgrade.
    //
    // What is asked instead is the transition: the field HAD the focus and no longer has it. That is the
    // moment a person finished typing, and it happens once however long any flag lingers.
    let has_focus = ui.memory(|m| m.has_focus(id));
    let had_focus = ui.data_mut(|d| {
        let key = id.with("was-focused");
        let was = d.get_temp::<bool>(key).unwrap_or(false);
        d.insert_temp(key, has_focus);
        was
    });
    let mut committed = had_focus && !has_focus && !took && !escaped;
    let cancelled = escaped;
    if escaped {
        committed = false;
        resp.surrender_focus();
    }
    // A REFUSAL LEAVES A PERSON WHERE THEY WERE: the text is intact, the caret is in the field. Losing
    // what was typed is the worst thing a field can do, because it is not clear who did it.
    let refused = committed && !valid(buf.trim());
    if refused {
        committed = false;
        resp.request_focus();
    }
    if committed || cancelled {
        st.buf = None;
        st.base = None;
        st.open = false;
        st.sel = 0;
    }
    let text = if cancelled { model.to_string() } else { buf };
    ui.data_mut(|d| d.insert_temp(id, st));
    // THE REFUSAL DOES ITS WORK RIGHT HERE: the letters are intact, the focus is in the field. There is
    // no point handing it outwards — the reason is told by `expr_value_label` next to the field, not by
    // the caller acting on a flag.
    ExprOut { resp, text, committed, cancelled }
}

/// What the field told the frame.
pub struct ExprOut {
    pub resp: egui::Response,
    /// The text that is in the field right now.
    pub text: String,
    /// COMMITTED. This flag alone is what makes the caller touch the model — and only inside
    /// `App::edit(...)`, so that the edit becomes one step of undo.
    pub committed: bool,
    /// Cancelled by Escape: the buffer was thrown away, the model was not touched.
    ///
    /// ONLY THE BEHAVIOUR CHECK READS IT — and that is the right consumer: the contract that Escape
    /// cancels the edit rather than closing the list is held by that check and not by eye. The ban is
    /// lifted narrowly so that the field does not have to be invented anew when the contract is needed
    /// in code.
    #[allow(dead_code)]
    pub cancelled: bool,
}

/// The field reports its open list, once per frame it is drawn with one.
pub fn note_list_open(ctx: &egui::Context) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(LIST_OPEN), true));
}

/// THE CARET'S POSITION IN THE BUFFER, IN BYTES.
///
/// egui counts the caret in CHARACTERS while the text functions work in bytes; names may be written in any
/// alphabet, so confusing the two cuts a string in the middle of a letter. With no state yet (the first
/// frame) the caret is taken to be at the end — that is where a person who has just typed something is.
pub fn caret_byte(ctx: &egui::Context, id: egui::Id, buf: &str) -> usize {
    let chars = egui::TextEdit::load_state(ctx, id).and_then(|s| s.cursor.char_range()).map(|r| r.primary.index.0).unwrap_or_else(|| buf.chars().count());
    buf.char_indices().nth(chars).map(|(i, _)| i).unwrap_or(buf.len())
}

/// THE WORD UNDER THE CARET — that is what the driver is looked up by.
///
/// The field holds an expression (`w*2+len`), not a single name, so the suggestions follow one fragment
/// rather than the whole string: after `w*2+` the list must not empty out just as a new name is being
/// started.
///
/// The word is looked for on BOTH sides of the caret. A formula gets edited in its middle as often as at
/// its end, and searching from the end of the whole string answered the wrong question there: with the
/// caret after `le` in `10+le*2` the search ran on `2` and offered nothing at all.
///
/// `caret` is a BYTE offset. Returns (the start of the word, its end, the word itself).
pub fn current_token(text: &str, caret: usize) -> (usize, usize, &str) {
    let caret = caret.min(text.len());
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    let start = text[..caret].rfind(|c: char| !is_word(c)).map(|i| i + c_len(text, i)).unwrap_or(0);
    let end = caret + text[caret..].find(|c: char| !is_word(c)).unwrap_or(text.len() - caret);
    (start, end, &text[start..end])
}

/// Put a driver's name in place of the word under the caret. Returns the new text and the byte offset
/// the caret lands on — right behind the name, so that typing carries on from there.
///
/// What comes after the word is KEPT. It used to be dropped: the result was assembled as head plus name,
/// so choosing `len` in `10+le*2` gave `10+len` and the `*2` was gone.
pub fn insert_driver(text: &str, name: &str, caret: usize) -> (String, usize) {
    let (start, end, _) = current_token(text, caret);
    (format!("{}{}{}", &text[..start], name, &text[end..]), start + name.len())
}

/// The length of a character in bytes, from its first byte — so that a string is never cut in the middle
/// of a letter (names may be written in any alphabet, and `i + 1` panics there).
pub fn c_len(text: &str, i: usize) -> usize {
    text[i..].chars().next().map(|c| c.len_utf8()).unwrap_or(1)
}

/// The state of one field between frames. It sits in the temporary memory of egui under the `id` of the
/// field: that way the widget needs no access to `App`, so it can be called even from where `App` is
/// already borrowed whole.
#[derive(Clone, Default)]
pub struct FieldState {
    /// The text being typed. `None` means the field is not being edited and what is in the model is
    /// shown.
    pub buf: Option<String>,
    /// The list is open.
    pub open: bool,
    /// The chosen row of the list (for the arrows).
    pub sel: usize,
    /// What the model held when the buffer was taken: a buffer nobody has typed into since yields to a model
    /// changed from elsewhere.
    pub base: Option<String>,
}

/// The height of the drop-down list; past that it scrolls.
pub const LIST_MAX_H: f32 = 220.0;

/// How many rows are shown before saying "N more".
pub const LIST_MAX_ROWS: usize = 8;

/// THE KEY UNDER WHICH A FIELD SAYS "MY LIST IS OPEN RIGHT NOW".
pub const LIST_OPEN: &str = "qym_expr_list_open";

/// HOW A NUMBER IN THE OPTIONS BAR IS WRITTEN AND BOUNDED: the limits it is clamped to, whether it is
/// whole, and the unit shown after it.
///
/// Four things about the SHAPE of the field, as opposed to its value, and they trailed twenty-nine call
/// sites as four loose arguments, where `false` and `""` said nothing about which was which.
#[derive(Clone, Copy)]
pub struct NumFormat<'a> {
    pub lo: f64,
    pub hi: f64,
    pub integer: bool,
    pub suffix: &'a str,
    /// Zero is no value for this field, though both signs are: an offset of 0 is the geometry itself.
    pub nonzero: bool,
}

pub fn num_or_expr(p: &mut ExprBarCtx, ui: &mut egui::Ui, key: &'static str, cur: f64, fmt: NumFormat) -> f64 {
    let NumFormat { lo, hi, integer, suffix, nonzero } = fmt;
    let vars = p.project.param_map();
    // THE TEXT IS BORROWED FOR A MOMENT. The drop-down list reads the whole document while the buffer
    // sits in the same `self`, and both cannot be borrowed at once. We work on a copy and put it back.
    let txt = p.bar_exprs.get(key).cloned().unwrap_or_else(|| if integer { format!("{cur:.0}") } else { qymcad_core::expr::fmt_num(cur) });
    let w = (60.0 + suffix.len() as f32 * 6.0).min(96.0);
    // ONE FIELD, AND IN THE FEATURES TOO, not only in sketches. This expression field is a single one
    // for ALL the tool bars (see `expr_fields.rs`), so the list of drivers and the typing rules arrive
    // everywhere at once: in extrude, fillet, chamfer, shell, hole, draft and pattern.
    //
    // A tool bar does not change the document — it shows a preview — so the text lives in `bar_exprs`
    // and is read every frame; the rule that editing text is not editing the model holds by itself here.
    let o = expr_field(ui, p.project, bar_field_id(key), &txt, w, &qymcad_i18n::tr("g-expr-placeholder"));
    // ENTER IN A FIELD OF THE BAR IS THE ENTER THE BAR ASKS FOR: it leaves the field and is marked, so the tool
    // applies at the same press instead of waiting for a second one (see `bar_enter_take`)
    if o.committed && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
        let frame = ui.ctx().cumulative_pass_nr();
        ui.data_mut(|d| d.insert_temp(egui::Id::new(BAR_ENTER), frame));
    }
    let txt = o.text;
    p.bar_exprs.insert(key, txt.clone());
    // A VALUE THAT CANNOT BE TAKEN is refused where it is typed: the reason on the mark beside the field, the old value
    // kept, the Apply and Enter of the command closed (`bar_fields_valid`). It used to be pressed to the nearest limit
    // or rounded and taken without a word: 521 copies became 512, 6.5 became 7.
    let checked = qymcad_core::expr::eval(&txt, &vars).map_err(|e| qymcad_i18n::error_words::expr_error_text(&e)).and_then(|v| {
        if nonzero && v.abs() < 1e-9 {
            Err(qymcad_i18n::tr("cmd-value-zero"))
        } else if integer && (v - v.round()).abs() > 1e-9 {
            Err(qymcad_i18n::tr("cmd-value-whole"))
        } else if v < lo - 1e-9 || v > hi + 1e-9 {
            Err(qymcad_i18n::tr2("cmd-value-out-of-range", "lo", &qymcad_core::expr::fmt_num(lo), "hi", &qymcad_core::expr::fmt_num(hi)))
        } else {
            Ok(v)
        }
    });
    BAR_BAD_KEYS.with(|b| {
        let mut b = b.borrow_mut();
        if checked.is_err() {
            b.insert(key);
        } else {
            b.remove(key);
        }
    });
    let frame = ui.ctx().cumulative_pass_nr();
    ui.data_mut(|d| {
        let bad = d.get_temp_mut_or_default::<std::collections::HashMap<&'static str, u64>>(egui::Id::new(BAR_BAD));
        if checked.is_err() {
            bad.insert(key, frame);
        } else {
            bad.remove(key);
        }
    });
    match checked {
        Ok(v) => v,
        Err(msg) => {
            // the reason on the mark beside the field, as the fields at the geometry give it
            crate::tone_mark(ui, &p.scheme.pal, crate::Tone::Refusal).on_hover_text(&msg);
            cur
        }
    }
}

/// The widget of the bar field under `key`: whoever shares its value asks it whether it is being typed in.
pub fn bar_field_id(key: &'static str) -> egui::Id {
    egui::Id::new(("bar_expr", key))
}

/// Where the fields of the tool bars keep which of them hold a value that cannot be taken, and in which frame.
pub(crate) const BAR_BAD: &str = "bar_expr_bad";

/// Where a field of a tool bar marks the frame its Enter was pressed in.
pub(crate) const BAR_ENTER: &str = "bar_expr_enter";

/// WAS ENTER PRESSED IN A FIELD OF THE BAR just now (this frame or the one before)? Taken once: the tool that
/// applies on it clears the mark. The field keeps the keyboard in the frame of the press, so the tool's own check of
/// Enter - which waits for the keyboard to be free - would not see it until a second press.
pub fn bar_enter_take(ctx: &egui::Context) -> bool {
    bar_enter_take_inner(ctx)
}

/// THE BAR'S APPLY BUTTON IS THE BAR'S ENTER: pressed, it marks the same Enter a field of the bar marks, so the tool
/// applies by the one path it applies on from the keyboard (`bar_enter_take`).
pub fn bar_apply_press(ctx: &egui::Context) {
    let frame = ctx.cumulative_pass_nr();
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(BAR_ENTER), frame));
}

pub(crate) fn bar_enter_take_inner(ctx: &egui::Context) -> bool {
    let now = ctx.cumulative_pass_nr();
    let at: Option<u64> = ctx.data_mut(|d| d.remove_temp(egui::Id::new(BAR_ENTER)));
    at.is_some_and(|f| f + 1 >= now) && bar_fields_valid(ctx)
}

/// DOES THE BAR FIELD `key` HOLD A VALUE THAT CANNOT BE TAKEN, as it was last drawn?
pub fn bar_field_bad(key: &str) -> bool {
    BAR_BAD_KEYS.with(|b| b.borrow().contains(key))
}

/// DO THE FIELDS OF THE TOOL BAR DRAWN NOW ALL HOLD VALUES THAT CAN BE TAKEN? A field refused in the frame just drawn
/// (or the one before) closes Apply and Enter; a field of a bar no longer drawn does not count.
pub fn bar_fields_valid(ctx: &egui::Context) -> bool {
    let now = ctx.cumulative_pass_nr();
    ctx.data(|d| d.get_temp::<std::collections::HashMap<&'static str, u64>>(egui::Id::new(BAR_BAD))).is_none_or(|bad| bad.values().all(|&f| f + 2 < now))
}
