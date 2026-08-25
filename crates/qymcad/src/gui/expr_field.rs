//! THE SINGLE EXPRESSION FIELD AND ITS LIST OF DRIVERS.
//!
//! The main law:
//!
//! > EDITING TEXT IS NOT EDITING THE MODEL.
//!
//! While a person types, the document does not change: the text lives in the buffer of the field and
//! travels into the model ONCE, on commit (Enter, Tab, losing focus). Escape throws the buffer away.
//!
//! That is how grown-up CAD is built, and not for reasons of taste. The name of a driver used to be
//! written into the model on every letter pressed — and that produced three troubles at once: a rebuild
//! of the whole project per letter, formulas destroyed halfway (`w` -> `wi` -> `wid`: the name `w` did
//! not exist for a second) and the erasure of what had been typed whenever the model refused to accept
//! an unfinished name.
//!
//! The list of drivers lives here as well, because the keyboard is shared: while the list is open, the
//! arrows and Enter belong to it and not to the field.
use crate::gui::{current_token, insert_driver, App};
use qymcad_core::model::Project;

/// The state of one field between frames. It sits in the temporary memory of egui under the `id` of the
/// field: that way the widget needs no access to `App`, so it can be called even from where `App` is
/// already borrowed whole.
#[derive(Clone, Default)]
struct FieldState {
    /// The text being typed. `None` means the field is not being edited and what is in the model is
    /// shown.
    buf: Option<String>,
    /// The list is open.
    open: bool,
    /// The chosen row of the list (for the arrows).
    sel: usize,
}

/// What the field told the frame.
pub(super) struct ExprOut {
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

/// The height of the drop-down list; past that it scrolls.
const LIST_MAX_H: f32 = 220.0;
/// How many rows are shown before saying "N more".
const LIST_MAX_ROWS: usize = 8;

/// THE EXPRESSION FIELD. `model` is what is written in the document right now; the field shows it
/// until an edit is begun.
pub(super) fn expr_field(ui: &mut egui::Ui, project: &Project, id: egui::Id, model: &str, w: f32, hint: &str) -> ExprOut {
    field(ui, project, id, model, w, hint, &|_| true, true, false)
}

/// THE SAME FIELD, BUT IT TAKES FOCUS ITSELF AND SELECTS THE FORMER VALUE.
///
/// That is how the dimension popup opens: a dimension is clicked and a new number is typed straight
/// away, with no aiming of the mouse at the field and no erasing of the old one. This behaviour must
/// not be lost when moving onto the shared field.
pub(super) fn expr_field_autofocus(ui: &mut egui::Ui, project: &Project, id: egui::Id, model: &str, w: f32, hint: &str, want_focus: bool) -> ExprOut {
    field(ui, project, id, model, w, hint, &|_| true, true, want_focus)
}

/// THE NAME FIELD. The same thing, but WITHOUT the list of drivers.
///
/// A name is not a formula: there is nothing to substitute other names into it, and the list would also
/// take Enter for itself — the very key a name is confirmed with. In grown-up CAD autocompletion lives
/// in the expression field only, and that is no trifle: a measurement showed that with the list open on
/// the name "h" Enter went to the list and the edit was not applied at all.
pub(super) fn name_field(ui: &mut egui::Ui, project: &Project, id: egui::Id, model: &str, w: f32, hint: &str, valid: &dyn Fn(&str) -> bool) -> ExprOut {
    field(ui, project, id, model, w, hint, valid, false, false)
}

/// THE KEY UNDER WHICH A FIELD SAYS "MY LIST IS OPEN RIGHT NOW".
const LIST_OPEN: &str = "qym_expr_list_open";

/// The field reports its open list, once per frame it is drawn with one.
fn note_list_open(ctx: &egui::Context) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(LIST_OPEN), true));
}

/// WAS A DRIVER LIST OPEN WHEN THE KEY WAS PRESSED — asked once, and the answer is taken away.
///
/// The frame's keys are handled BEFORE anything is drawn, so a field cannot answer for a frame that has not
/// been painted yet: the answer is about the frame the person was looking at when they pressed. It is taken
/// away rather than read, so that a field which has gone (the popup closed) cannot leave a stale "open"
/// behind and swallow somebody else's Escape.
pub(super) fn take_list_open(ctx: &egui::Context) -> bool {
    ctx.data_mut(|d| {
        let id = egui::Id::new(LIST_OPEN);
        let was = d.get_temp::<bool>(id).unwrap_or(false);
        d.insert_temp(id, false);
        was
    })
}

/// THE CARET'S POSITION IN THE BUFFER, IN BYTES.
///
/// egui counts the caret in CHARACTERS while the text functions work in bytes; names may be written in any
/// alphabet, so confusing the two cuts a string in the middle of a letter. With no state yet (the first
/// frame) the caret is taken to be at the end — that is where a person who has just typed something is.
fn caret_byte(ctx: &egui::Context, id: egui::Id, buf: &str) -> usize {
    let chars = egui::TextEdit::load_state(ctx, id).and_then(|s| s.cursor.char_range()).map(|r| r.primary.index).unwrap_or_else(|| buf.chars().count());
    buf.char_indices().nth(chars).map(|(i, _)| i).unwrap_or(buf.len())
}

/// THE SAME FIELD, BUT WITH A CHECK BEFORE THE COMMIT.
///
/// `valid` decides whether what was typed can be accepted. A refusal DOES NOT ERASE the text and does
/// not release the focus — otherwise out comes exactly the reported trouble: typing `len` and having
/// the `n` deleted automatically. A person finishes the name or presses Escape, and meanwhile the
/// program says what is wrong.
fn field(
    ui: &mut egui::Ui,
    project: &Project,
    id: egui::Id,
    model: &str,
    w: f32,
    hint: &str,
    valid: &dyn Fn(&str) -> bool,
    with_list: bool,
    autofocus: bool,
) -> ExprOut {
    let mut st: FieldState = ui.data_mut(|d| d.get_temp(id)).unwrap_or_default();
    let mut buf = st.buf.clone().unwrap_or_else(|| model.to_string());

    // THE KEYS ARE TAKEN BEFORE THE FIELD IS DRAWN. While the list is open the arrows belong to IT;
    // otherwise they simply move the caret in the text and a row cannot be chosen from the keyboard —
    // which is how it used to be.
    let focused = ui.memory(|m| m.has_focus(id));
    let (mut go_down, mut go_up, mut take, mut esc, mut open_list) = (false, false, false, false, false);
    if focused {
        ui.input_mut(|i| {
            open_list = i.consume_key(egui::Modifiers::COMMAND, egui::Key::Space);
            esc = i.consume_key(egui::Modifiers::NONE, egui::Key::Escape);
            if st.open {
                go_down = i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown);
                go_up = i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp);
                // with the list open, Enter and Tab INSERT what is chosen rather than closing the
                // popup of the tool: the formula is still being written.
                take = i.consume_key(egui::Modifiers::NONE, egui::Key::Enter) || i.consume_key(egui::Modifiers::NONE, egui::Key::Tab);
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
        let area = egui::Area::new(id.with("drv"))
            .order(egui::Order::Tooltip)
            .fixed_pos(resp.rect.left_bottom() + egui::vec2(0.0, 2.0))
            .constrain(true);
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
                                    t = if d.ambiguous { t.color(ui.visuals().warn_fg_color) } else { t.weak() };
                                    ui.label(t);
                                }
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    let val = d.value.map(|v| format!("{v:.3}")).unwrap_or_else(|| crate::i18n::tr("par-no-value"));
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
                        ui.label(egui::RichText::new(crate::i18n::tr1("par-more", "n", &(hits.len() - LIST_MAX_ROWS).to_string())).weak().small());
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
    let mut committed = resp.lost_focus() && !took && !escaped;
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

impl App {
    /// The caption "= 42.500" or the reason for the error in words. One for every field: it is computed
    /// and shown the same way wherever the field stands.
    pub(super) fn expr_value_label(&self, ui: &mut egui::Ui, text: &str) {
        if text.trim().is_empty() {
            return;
        }
        match self.project.eval_expr(text) {
            Ok(v) => {
                ui.label(egui::RichText::new(format!("= {v:.3}")).weak().small());
            }
            Err(e) => {
                // THE REASON IN WORDS AND NOT AN ICON: what exactly did not add up is what needs
                // knowing.
                let msg = crate::i18n::expr_error_text(&e);
                ui.label(egui::RichText::new(&msg).color(self.scheme.pal.error_mild()).small()).on_hover_text(&msg);
            }
        }
    }
}
