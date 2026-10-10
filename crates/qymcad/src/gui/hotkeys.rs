//! THE HOTKEY REFERENCE — the single source for the table in Settings -> Keyboard.
//!
//! There are more than sixty keys in the application, and they lived ONLY in the tooltips of the
//! buttons: the only way to learn them was to hover the mouse over every one. A list typed apart from
//! the handlers would diverge from them at the very first edit — so a test stands next to it that checks
//! the table against THE SOURCE of the handlers: a key appears in
//! `part_hotkey`/`sketch_hotkey`/`assembly_hotkey` and is not in the table (or the other way round) and
//! the test is red.
pub(crate) use qymcad_ui_state::{HotkeyRow, HOTKEYS};
use egui_phosphor::regular as ph;
use crate::gui::WinKind;

/// What the key does - in the language of the person. A free function rather than a method: the row is a
/// record and lives in the state crate, and the WORDS are chosen here, where the dictionary is.
pub(crate) fn hotkey_what(row: &HotkeyRow) -> String {
    crate::i18n::tr(row.what)
}

/// THE AREAS in the order they are shown: the code and the key of its caption.
///
/// Hand-written because the ORDER is a decision - general first, then the workbenches - and no
/// catalogue holds an order. What it must not be is INCOMPLETE: an area the catalogue knows and this
/// list does not would have its keys shown nowhere, silently. The check below holds the set against the
/// catalogue; only the order stays a matter of taste.
pub(crate) const AREAS: [&str; 4] = ["general", "part", "sketch", "assembly"];

/// WHETHER A KEY IS REBINDABLE. The general area is not, and that is not laziness.
///
/// Esc, Enter, Delete, Ctrl+Z, Ctrl+S are an agreement of the whole operating system, not a layout of
/// ours. Letting them be moved means letting a person end up without undo at the very moment it is
/// needed most, with no way at all to notice. What gets moved are the keys of the WORKBENCHES.
pub(crate) fn rebindable(area: &str) -> bool {
    area != "general"
}

/// THE TABLE OF KEYS, the body of Settings -> Keyboard. A reference that can be edited: every key of a workbench is
/// a button, pressing it puts the table into waiting, and the next presses - keys or chords - are recorded.
///
/// Laid out for the question people bring to it, "what is the key for X": a filter on top, the areas below, and in
/// every row the key and what it does. Everything that is not the factory layout is marked, and each mark has its
/// own way back. No scroll of its own: the settings section around it scrolls, and a scroll inside a scroll takes
/// the wheel from the outer one.
pub(crate) fn hotkeys_table(wc: &mut qymcad_ui_state::WinCtx, ui: &mut egui::Ui, ctx: &egui::Context) {
    // THE ROOM OF THE SCROLL BAR is left free: the settings scroll floats its bar over the right edge of what it
    // holds and takes the clicks there, and the clear icon of the filter and the reset icons of the rows stand at
    // that edge. Measured: the bar took x 751..761 of a section ending at 761, and a click on the clear icon at
    // 751.5 went to the bar.
    // A child of its own, since a ui is never narrowed below what it already holds and the hint above the table
    // spans the whole section.
    let bar = ui.spacing().scroll.bar_width + ui.spacing().scroll.bar_outer_margin;
    let width = ui.available_width() - bar;
    ui.vertical(|ui| {
        ui.set_max_width(width);
        table(wc, ui, ctx);
    });
}

/// The filter, the areas and the notes, in the width `hotkeys_table` leaves them.
fn table(wc: &mut qymcad_ui_state::WinCtx, ui: &mut egui::Ui, ctx: &egui::Context) {
    // laid out from the right: the reset button takes what it needs, the filter the rest - no guessed width.
    // Inside a one-row `horizontal`: a right-to-left layout of its own would take the whole remaining height
    // and centre the row in it.
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if !wc.set.hotkeys.is_empty() && ui.button(crate::i18n::tr("hotkeys-reset-all")).clicked() {
                wc.set.hotkeys.clear();
                settle(wc.hotkeys);
            }
            let glass = ui.fonts_mut(|f| f.layout_no_wrap(ph::MAGNIFYING_GLASS.to_string(), egui::TextStyle::Body.resolve(ui.style()), egui::Color32::WHITE).size().x);
            // THE WHOLE FIELD, margins included: `desired_width` is the width of the text alone, and the field's
            // own margins on top of it pushed the row past the section
            let field = (ui.available_width() - glass - ui.spacing().item_spacing.x).max(60.0);
            let side = ui.spacing().interact_size.y;
            // the right margin keeps the text clear of the clear icon drawn over the field's right end, and is
            // kept while the field is empty too, so the text does not move when the icon appears
            let edit = egui::TextEdit::singleline(&mut wc.hotkeys.filter).hint_text(crate::i18n::tr("hotkeys-filter-hint")).margin(egui::Margin { left: 4, right: 4 + side as i8, top: 2, bottom: 2 });
            let resp = ui.add_sized([field, side], edit);
            filter_clear(ui, &resp, &mut wc.hotkeys.filter);
            ui.label(ph::MAGNIFYING_GLASS);
        });
    });
    ui.separator();
    let q = crate::i18n::search::query(&wc.hotkeys.filter);
    let mut shown = 0;
    let cols = columns(wc.set, ui, ui.available_width());
    for area in AREAS {
        let rows: Vec<&HotkeyRow> = HOTKEYS.iter().filter(|r| r.area == area && row_matches(wc.set, r, &q)).collect();
        if rows.is_empty() {
            continue;
        }
        shown += rows.len();
        area_header(wc, ui, area);
        egui::Grid::new(format!("hk_{area}")).num_columns(3).min_col_width(0.0).spacing([GRID_GAP, 4.0]).striped(true).show(ui, |ui| {
            for r in rows {
                key_cell(wc, ui, r, cols.key);
                // A GRID CELL LAYS ITS CONTENT OUT LEFT TO RIGHT (the grid lives in a `horizontal`), so the
                // lines under the description are stacked by an explicit `vertical`. Inside the row's own
                // cell rather than a grid row of their own: an extra row shifted every row below it, and
                // the grid sized each row from the height the previous frame had at that index.
                // The `vertical` is the cell itself, not wrapped in a `scope`: a scope takes the whole cell
                // at the previous frame's row height in the grid's centred layout, and a row that had just
                // lost its waiting line stood 24 pt tall for one frame instead of 18, every row below 6 pt
                // low.
                ui.vertical(|ui| {
                    ui.set_width(cols.what);
                    ui.add(egui::Label::new(hotkey_what(r)).wrap());
                    row_status(wc, ui, r.action);
                });
                row_tools(wc, ui, r);
                ui.end_row();
            }
        });
        ui.add_space(10.0);
    }
    if shown == 0 {
        ui.add(egui::Label::new(egui::RichText::new(crate::i18n::tr1("hotkeys-nothing", "q", wc.hotkeys.filter.trim())).weak()).wrap());
    }
    ui.separator();
    ui.add(egui::Label::new(egui::RichText::new(crate::i18n::tr("hotkeys-note")).weak().small()).wrap());
    // THE FOCUS RULE GOES HERE AND NOT ONLY IN THE HELP. A caret in a field extinguishes
    // bare letters (otherwise `w` in an expression would launch a command), and Alt is the
    // only way to reach a tool from there. Not saying so in the hotkey reference means
    // hiding half the rule: U is pressed in the length field, nothing happens, and the
    // conclusion drawn is about the program.
    ui.add(egui::Label::new(egui::RichText::new(crate::i18n::tr("hotkeys-alt-note")).weak().small()).wrap());
    ui.add(egui::Label::new(egui::RichText::new(crate::i18n::tr("hotkeys-rebind-note")).weak().small()).wrap());
    capture_hotkey(wc, ctx);
}

/// THE TABLE IS LEFT: the settings closed, or another section chosen. Whatever it was in the middle of is dropped,
/// so a later press does not land in a table no longer on screen.
pub(crate) fn hotkeys_left(hk: &mut qymcad_ui_state::HotkeyCapture) {
    settle(hk);
}

/// WHETHER THE KEYBOARD IS THE TABLE'S this frame - asked before the cancel ladder, which runs before anything is
/// drawn.
///
/// While the table waits for a key, every press is the name of a binding. Otherwise Esc, with no field holding the
/// keyboard, answers an open clash question "keep as it was"; the key is taken, so the ladder does not also clear
/// the selection behind the settings.
pub(crate) fn hotkeys_take_keyboard(win: &qymcad_ui_state::Windows, hk: &mut qymcad_ui_state::HotkeyCapture, ctx: &egui::Context) -> bool {
    if hk.action.is_some() {
        return true;
    }
    if hk.clash.is_none() || !win.is(WinKind::Settings) || ctx.egui_wants_keyboard_input() || !ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
        return false;
    }
    hk.clash = None;
    true
}

/// WHETHER A ROW ANSWERS THE FILTER: by its description or by its key.
fn row_matches(set: &qymcad_ui_state::Settings, r: &HotkeyRow, q: &str) -> bool {
    if q.is_empty() || crate::i18n::search::find_in_names(q, &hotkey_what(r), |lang| crate::i18n::tr_in(lang, r.what).unwrap_or_default()).is_some() {
        return true;
    }
    // the stored spelling AND the shown one: a Mac user types the Command sign they see, anybody may type "ctrl"
    let key = qymcad_ui_state::hotkey_key(set, r.action);
    key.to_lowercase().contains(q) || qymcad_ui_state::key_label(&key).to_lowercase().contains(q)
}

fn what_of(action: &str) -> String {
    HOTKEYS.iter().find(|r| r.action == action).map(hotkey_what).unwrap_or_default()
}

/// The gap between the columns of the table.
const GRID_GAP: f32 = 14.0;

/// THE WIDTHS EVERY SECTION SHARES, fixed rather than left to each grid: the sections line up, and nothing that
/// appears in a row - a reset icon, a clash under it - can widen a column a frame later.
struct Columns {
    /// The key buttons: the widest key now bound or caption of the button, and never narrower than `KEY_W`.
    key: f32,
    /// What is left of the table for the descriptions, which wrap inside it.
    what: f32,
}

fn columns(set: &qymcad_ui_state::Settings, ui: &egui::Ui, table: f32) -> Columns {
    let body = egui::TextStyle::Body.resolve(ui.style());
    let mono = egui::TextStyle::Monospace.resolve(ui.style());
    let width = |text: String, font: &egui::FontId| ui.ctx().fonts_mut(|f| f.layout_no_wrap(text, font.clone(), egui::Color32::WHITE).size().x);
    let pad = 2.0 * ui.spacing().button_padding.x;
    // the button also says "press a key" while it waits and "no key" when unbound
    let words = ["hotkeys-press", "hotkeys-unbound"].map(|k| width(crate::i18n::tr(k), &body) + pad);
    let key = HOTKEYS.iter().map(|r| width(qymcad_ui_state::key_label(&qymcad_ui_state::hotkey_key(set, r.action)), &mono) + pad).chain(words).fold(KEY_W, f32::max);
    // THE TWO ROW ICONS AS WIDE AS THEY ARE DRAWN - the glyph and the button's padding, wider than the square they
    // ask for - then the gap between them and the room after the reset icon. Counted as squares, the column came
    // out narrower than drawn, and the table pushed the body of the window past its title bar.
    let icon = |glyph: &str| (width(glyph.to_string(), &body) + pad).max(ui.spacing().interact_size.y);
    let tools = icon(ph::X) + icon(ph::ARROW_COUNTER_CLOCKWISE) + ICON_GAP + RESET_PAD;
    let what = (table - key - tools - 2.0 * GRID_GAP).max(KEY_W);
    Columns { key, what }
}

/// The narrowest key button: a single letter still gets a target worth aiming at.
const KEY_W: f32 = 110.0;

/// UNDER THE DESCRIPTION OF THE ROW BEING REASSIGNED: what the window waits for, why a press was refused (in place
/// of the waiting line), which key clashes. Shown where the person looks - the key they just pressed - and not at the
/// top of a table they may have scrolled far down. Wrapped inside the description column, so a long message never
/// widens the table.
fn row_status(wc: &mut qymcad_ui_state::WinCtx, ui: &mut egui::Ui, action: &str) {
    let clash = wc.hotkeys.clash.clone().filter(|c| c.action == action);
    let waiting = wc.hotkeys.action.as_deref() == Some(action);
    if clash.is_none() && !waiting {
        return;
    }
    if let Some(clash) = clash {
        let old = qymcad_ui_state::key_label(&qymcad_ui_state::hotkey_key(wc.set, &clash.action));
        let holder = what_of(clash.holder);
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(ph::WARNING).color(wc.scheme.pal.warning()));
            ui.add(egui::Label::new(crate::i18n::tr2("hotkeys-taken", "key", &qymcad_ui_state::key_label(&clash.chord), "what", &holder)).wrap());
        });
        // THE CHOICES ON A LINE OF THEIR OWN, under the question: beside it they wrapped wherever the text happened to end
        ui.horizontal_wrapped(|ui| {
            let swap = ui.add_enabled(!old.is_empty(), egui::Button::new(crate::i18n::tr("hotkeys-swap")));
            if swap.on_hover_text(crate::i18n::tr2("hotkeys-swap-tip", "what", &holder, "key", &old)).clicked() {
                qymcad_ui_state::resolve_hotkey_clash(wc.set, &clash, qymcad_ui_state::ClashChoice::Swap);
                wc.hotkeys.clash = None;
            }
            if ui.button(crate::i18n::tr("hotkeys-take")).on_hover_text(crate::i18n::tr1("hotkeys-take-tip", "what", &holder)).clicked() {
                qymcad_ui_state::resolve_hotkey_clash(wc.set, &clash, qymcad_ui_state::ClashChoice::Unbind);
                wc.hotkeys.clash = None;
            }
            if ui.button(crate::i18n::tr("hotkeys-cancel")).clicked() {
                wc.hotkeys.clash = None;
            }
        });
    } else {
        // A REFUSED PRESS SAYS WHY IN PLACE OF THE WAITING LINE: one line under the row, not two. A stop sign, not
        // the clash's warning triangle: there is no choice to make, the key cannot be had. The sign and the words are
        // ONE LABEL in the waiting line's font: as two widgets on a wrapping line the line took the height of a
        // button, and the rows below moved down 3 points when the refusal appeared.
        if wc.hotkeys.note.is_empty() {
            ui.add(egui::Label::new(egui::RichText::new(crate::i18n::tr("hotkeys-waiting")).color(wc.scheme.pal.ui_accent())).wrap());
        } else {
            let font = egui::TextStyle::Body.resolve(ui.style());
            let mut line = egui::text::LayoutJob::default();
            line.append(ph::WARNING_OCTAGON, 0.0, egui::TextFormat::simple(font.clone(), wc.scheme.pal.error()));
            line.append(&wc.hotkeys.note, ui.spacing().item_spacing.x, egui::TextFormat::simple(font, ui.visuals().text_color()));
            ui.add(egui::Label::new(line).wrap());
        }
    }
}

/// The caption of a section, and the way back to the factory keys of that section alone.
fn area_header(wc: &mut qymcad_ui_state::WinCtx, ui: &mut egui::Ui, area: &str) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(crate::i18n::tr(&format!("hotkeys-area-{area}"))).strong());
        if !rebindable(area) {
            ui.label(egui::RichText::new(ph::LOCK_SIMPLE).weak()).on_hover_text(crate::i18n::tr("hotkeys-fixed"));
            return;
        }
        let changed: Vec<&str> = HOTKEYS.iter().filter(|r| r.area == area && wc.set.hotkeys.contains_key(r.action)).map(|r| r.action).collect();
        if !changed.is_empty() && ui.small_button(crate::i18n::tr("hotkeys-reset-area")).clicked() {
            for a in changed {
                wc.set.hotkeys.remove(a);
            }
            settle(wc.hotkeys);
        }
    });
}

/// THE KEY IS A BUTTON. Press it, the program waits for a press, it is recorded. A text field here would be
/// a lie: modifiers would be typed into it as words.
fn key_cell(wc: &mut qymcad_ui_state::WinCtx, ui: &mut egui::Ui, r: &HotkeyRow, width: f32) {
    // SHOWN the way this system writes keys; stored and compared in the portable spelling
    let cur = qymcad_ui_state::key_label(&qymcad_ui_state::hotkey_key(wc.set, r.action));
    if !rebindable(r.area) {
        ui.scope(|ui| {
            ui.set_min_width(width);
            ui.label(egui::RichText::new(&cur).monospace().strong()).on_hover_text(crate::i18n::tr("hotkeys-fixed"));
        });
        return;
    }
    let waiting = wc.hotkeys.action.as_deref() == Some(r.action);
    let changed = wc.set.hotkeys.contains_key(r.action);
    // A KEY THIS SYSTEM KEEPS, brought by a profile from another one: it does not run here (see `hotkey_action`),
    // and saying nothing would leave a key in the table that silently does nothing
    let refused = qymcad_ui_state::KeySeq::parse(&qymcad_ui_state::hotkey_key(wc.set, r.action)).and_then(|k| qymcad_ui_state::hotkey_refusal(r.action, &k));
    let recorded = wc.hotkeys.recorded.label();
    let text = if waiting && recorded.is_empty() {
        egui::RichText::new(crate::i18n::tr("hotkeys-press")).italics()
    } else if waiting {
        // THE CHORDS RECORDED SO FAR, as they are pressed: the key being built is seen while it is built
        egui::RichText::new(crate::i18n::tr1("hotkeys-seq-waiting", "keys", &recorded)).monospace().strong()
    } else if cur.is_empty() {
        egui::RichText::new(crate::i18n::tr("hotkeys-unbound")).italics().weak()
    } else if refused.is_some() {
        egui::RichText::new(&cur).monospace().strong().strikethrough().color(wc.scheme.pal.error_mild())
    } else if changed {
        // A CHANGED KEY LOOKS CHANGED: whoever comes back to the window in a month sees at once what is theirs
        egui::RichText::new(&cur).monospace().strong().color(wc.scheme.pal.ui_accent())
    } else {
        egui::RichText::new(&cur).monospace().strong()
    };
    // A KEY BEING RECORDED KEEPS TO ITS COLUMN, cut short rather than widening the table under the person's eyes
    let mut resp = ui
        .scope(|ui| {
            ui.set_max_width(width);
            ui.add(egui::Button::new(text).selected(waiting).min_size(egui::vec2(width, 0.0)).truncate())
        })
        .inner;
    if let Some(why) = refused {
        resp = resp.on_hover_text(crate::i18n::tr(why));
    }
    if resp.clicked() {
        wc.hotkeys.action = if waiting { None } else { Some(r.action.to_string()) };
        wc.hotkeys.note.clear();
        wc.hotkeys.clash = None;
        wc.hotkeys.recorded = Default::default();
        // A FOCUSED BUTTON TAKES SPACE AND ENTER for a click: the press meant for the binding would
        // switch the waiting straight back off.
        resp.surrender_focus();
    }
}

/// Per row: leave the action without a key, and - where it was changed - put the factory key back.
///
/// Both are icons in slots of one fixed size that are always laid out, shown or not: a button that appears
/// only on a changed row widened the column, and the grid stretched every row of the section to the new width
/// a frame later.
fn row_tools(wc: &mut qymcad_ui_state::WinCtx, ui: &mut egui::Ui, r: &HotkeyRow) {
    ui.horizontal(|ui| {
        if !rebindable(r.area) {
            return;
        }
        // THE GAPS ARE SET HERE, exactly: with no item spacing in the row, `ICON_GAP` alone stands between the icons
        // and `RESET_PAD` alone between the reset icon and the end of the cell, the last column of the table
        ui.spacing_mut().item_spacing.x = 0.0;
        let bound = !qymcad_ui_state::hotkey_key(wc.set, r.action).is_empty();
        if row_icon(ui, bound, ph::X).on_hover_text(crate::i18n::tr("hotkeys-clear")).clicked() {
            qymcad_ui_state::set_hotkey(wc.set, r.action, "");
            settle(wc.hotkeys);
        }
        ui.add_space(ICON_GAP);
        // "restore the factory key" only where it really was changed
        let changed = wc.set.hotkeys.contains_key(r.action);
        let tip = crate::i18n::tr1("hotkeys-default-is", "key", &qymcad_ui_state::key_label(r.key));
        if row_icon(ui, changed, ph::ARROW_COUNTER_CLOCKWISE).on_hover_text(tip).clicked() {
            settle(wc.hotkeys);
            // the factory key held by another row is asked about under this one, as the press of that key would be
            wc.hotkeys.clash = qymcad_ui_state::reset_hotkey(wc.set, r.action);
        }
        ui.add_space(RESET_PAD);
    });
}

/// AN EDIT MADE BY A CLICK ENDS WHATEVER THE WINDOW WAS IN THE MIDDLE OF: the waiting for a key, a clash question,
/// a refusal. Reported behaviour: X or reset clicked while the window waited for a key changed the key, and the
/// window went on waiting - the next press overwrote what the click had just set.
fn settle(hk: &mut qymcad_ui_state::HotkeyCapture) {
    hk.action = None;
    hk.clash = None;
    hk.note.clear();
    hk.recorded = Default::default();
}

/// THE CLEAR ICON INSIDE THE FILTER FIELD, at its right end, while there is something to clear. Placed over the field
/// rather than beside it: a widget beside it in this right-to-left row would push the field and the glass along when
/// it appears. Registered after the field, so it is the one under the pointer there; the click gives the keyboard
/// back to the field.
fn filter_clear(ui: &egui::Ui, field: &egui::Response, filter: &mut String) {
    if filter.is_empty() {
        return;
    }
    let side = field.rect.height();
    let rect = egui::Rect::from_min_size(egui::pos2(field.rect.right() - side, field.rect.top()), egui::vec2(side, side));
    let x = ui.interact(rect, field.id.with("clear"), egui::Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
    let body = egui::TextStyle::Body.resolve(ui.style());
    let color = if x.hovered() { ui.visuals().strong_text_color() } else { ui.visuals().weak_text_color() };
    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, ph::X, egui::FontId::new(body.size * 0.85, body.family), color);
    if x.on_hover_text(crate::i18n::tr("hotkeys-filter-clear")).clicked() {
        filter.clear();
        field.request_focus();
    }
}

/// The room between the clear icon and the reset icon.
const ICON_GAP: f32 = 2.0;

/// The room after the reset icon, the last thing in a row.
const RESET_PAD: f32 = 6.0;

/// A square icon button of one size for every row, framed under the pointer; hidden, it still holds its place.
fn row_icon(ui: &mut egui::Ui, shown: bool, icon: &str) -> egui::Response {
    let side = ui.spacing().interact_size.y;
    ui.add_visible(shown, egui::Button::new(icon).frame_when_inactive(false).min_size(egui::vec2(side, side)))
}

/// THE PRESS THAT ASSIGNS A KEY, while the window waits for one - or for the next chord of it.
fn capture_hotkey(wc: &mut qymcad_ui_state::WinCtx, ctx: &egui::Context) {
    let Some(action) = wc.hotkeys.action.clone() else { return };
    let Some(area) = HOTKEYS.iter().find(|r| r.action == action).map(|r| r.area) else {
        wc.hotkeys.action = None;
        return;
    };
    let (pressed, clipboard, now) = ctx.input(|i| {
        let key = i.events.iter().find_map(|e| match e {
            // a modifier on its own is the start of a chord, not a press: the key that completes it may come in the same frame
            egui::Event::Key { key, pressed: true, repeat: false, modifiers, .. } if !qymcad_ui_state::modifier_key(*key) => Some((*key, qymcad_ui_state::HeldKeys::of_event(i, *modifiers))),
            _ => None,
        });
        // egui turns Ctrl+C/X/V into clipboard events and the key itself never arrives
        (key, i.events.iter().any(|e| matches!(e, egui::Event::Copy | egui::Event::Cut | egui::Event::Paste(_))), i.time)
    });
    if clipboard {
        wc.hotkeys.note = crate::i18n::tr("hotkeys-reserved");
        return;
    }
    let recorded = wc.hotkeys.recorded.pressed.clone();
    let outcome = match pressed {
        Some((key, held)) => capture_outcome(wc.set, area, &action, &recorded, key, held),
        // A PAUSE ENDS THE SEQUENCE: no Enter is needed for a key of one chord, as before sequences
        None if !recorded.is_empty() && now - wc.hotkeys.recorded.at >= qymcad_ui_state::RECORD_WAIT => finish_recording(wc.set, area, &action, &recorded),
        None => Capture::Pending,
    };
    match outcome {
        Capture::Pending => {
            if !recorded.is_empty() {
                ctx.request_repaint_after(std::time::Duration::from_secs_f64((wc.hotkeys.recorded.at + qymcad_ui_state::RECORD_WAIT - now).max(0.0)));
            }
            return;
        }
        Capture::Cancel => wc.hotkeys.action = None,
        Capture::Refused(why) => {
            wc.hotkeys.note = crate::i18n::tr(why);
            return;
        }
        Capture::Record(chords) => {
            wc.hotkeys.recorded = qymcad_ui_state::KeyWait { pressed: chords, at: now, area };
            wc.hotkeys.note.clear();
            ctx.request_repaint_after(std::time::Duration::from_secs_f64(qymcad_ui_state::RECORD_WAIT));
            return;
        }
        Capture::Clash(clash) => {
            wc.hotkeys.clash = Some(clash);
            wc.hotkeys.action = None;
        }
        Capture::Bind(keys) => {
            qymcad_ui_state::set_hotkey(wc.set, &action, &keys);
            wc.hotkeys.action = None;
        }
    }
    wc.hotkeys.recorded = Default::default();
    wc.hotkeys.note.clear();
}

/// What a press in the waiting window comes to.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Capture {
    /// Esc: leave the waiting, change nothing.
    Cancel,
    /// A modifier on its own, or no press at all: the window goes on waiting.
    Pending,
    /// Not assignable; the catalogue key says why.
    Refused(&'static str),
    /// The chords of the key so far: the window waits for the next one, or for Enter.
    Record(Vec<qymcad_ui_state::Chord>),
    /// Taken in the same area - the person decides.
    Clash(qymcad_ui_state::HotkeyClash),
    /// Recorded as is (empty: left without a key).
    Bind(String),
}

/// THE DECISION, apart from the window, so the tests can ask it without a frame. `recorded` holds the chords
/// pressed for this key so far.
pub(super) fn capture_outcome(
    set: &qymcad_ui_state::Settings,
    area: &'static str,
    action: &str,
    recorded: &[qymcad_ui_state::Chord],
    key: egui::Key,
    held: impl Into<qymcad_ui_state::HeldKeys>,
) -> Capture {
    let held = held.into();
    if qymcad_ui_state::modifier_key(key) {
        return Capture::Pending;
    }
    let bare = !held.mods.any();
    match key {
        egui::Key::Escape if bare => return Capture::Cancel, // leaving the mode rather than assigning Esc
        // the gesture of every field: erase - the last chord, or with none yet, the key itself
        egui::Key::Backspace | egui::Key::Delete if bare && !recorded.is_empty() => return Capture::Record(recorded[..recorded.len() - 1].to_vec()),
        egui::Key::Backspace | egui::Key::Delete if bare => return Capture::Bind(String::new()),
        egui::Key::Enter if bare && !recorded.is_empty() => return finish_recording(set, area, action, recorded),
        _ => {}
    }
    if held.alt.left() {
        return Capture::Refused("hotkeys-no-alt");
    }
    let mut chords = recorded.to_vec();
    chords.push(qymcad_ui_state::Chord::of_held(qymcad_ui_state::platform_keys::Os::current(), held, key));
    // judged as it grows: a refused chord is said at once, under the row, and the chords before it are kept
    if let Some(why) = qymcad_ui_state::hotkey_refusal(action, &qymcad_ui_state::KeySeq { chords: chords.clone() }) {
        return Capture::Refused(why);
    }
    if chords.len() == qymcad_ui_state::SEQ_MAX {
        return finish_recording(set, area, action, &chords);
    }
    Capture::Record(chords)
}

/// THE KEY RECORDED IS COMPLETE: it is bound, or asked about when another action of the area holds it.
pub(super) fn finish_recording(set: &qymcad_ui_state::Settings, area: &str, action: &str, chords: &[qymcad_ui_state::Chord]) -> Capture {
    let name = qymcad_ui_state::KeySeq { chords: chords.to_vec() }.name();
    match qymcad_ui_state::hotkey_taken_by(set, area, &name, action) {
        Some(holder) => Capture::Clash(qymcad_ui_state::HotkeyClash { action: action.to_string(), chord: name, holder }),
        None => Capture::Bind(name),
    }
}

#[cfg(test)]
mod tests {
    /// THE HOTKEY REFERENCE IS FULLY TRANSLATED — the first area closed completely.
    ///
    /// As a check of its own rather than "inside the general counter": a closed area must stay closed
    /// even while the general ceiling is still high.
    #[test]
    fn the_hotkey_reference_is_fully_translated() {
        let src = include_str!("hotkeys.rs");
        let code = src.split("#[cfg(test)]").next().expect("the working part");
        assert_eq!(qymcad_i18n::ratchet::russian_literals(code), 0, "the hotkey reference must go through the catalogue in its entirety");
    }

    use super::HOTKEYS;

    /// Cut the body of a handler function out of the source.
    fn body_of<'a>(src: &'a str, sig: &str) -> &'a str {
        let a = src.find(sig).unwrap_or_else(|| panic!("the handler {sig} was not found"));
        let b = src[a..].find("\n    }\n").map(|i| a + i).unwrap_or(src.len());
        &src[a..b]
    }

    /// THE ACTIONS really handled in the body of a handler (the literals of the `match` arms).
    fn actions_in(body: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut rest = body;
        while let Some(i) = rest.find('"') {
            let after = &rest[i + 1..];
            let Some(j) = after.find('"') else { break };
            let lit = &after[..j];
            rest = &after[j + 1..];
            if lit.contains('.') && !lit.contains(' ') && !out.contains(&lit.to_string()) {
                out.push(lit.to_string());
            }
        }
        out
    }

    /// WHERE THE ACTIONS OF AN AREA ARE HANDLED. Two of them for the sketch: the drawing tools are named by a
    /// table in the workbench crate ("this action means that tool"), and what is left in the window handles
    /// the rest. A guard that read only the window would call every tool of the reference a phantom.
    /// WHERE THE ACTIONS OF AN AREA LIVE, and whether that place is the one that HEARS THE KEY.
    ///
    /// Two places for the sketch: the window is handed the action, and the workbench crate holds the table of
    /// "this action means that drawing tool". The guards that compare the reference with the code must read both.
    /// The bool says whether the place is a handler the key handler calls with the action it matched.
    const HANDLERS: [(&str, &str, bool); 4] = [
        ("part", "pub(super) fn part_hotkey(&mut self, action: &str) {", true),
        ("assembly", "pub(super) fn assembly_hotkey(&mut self, action: &str) {", true),
        ("sketch", "pub(super) fn sketch_hotkey(&mut self, action: &str)", true),
        ("sketch", "pub fn tool_for_action(action: &str) -> Option<u8>", false),
    ];

    fn handler_sources() -> [(&'static str, &'static str, &'static str, bool); 4] {
        let gui = include_str!("../gui.rs");
        let sketching = crate::gui::sketch_source::SKETCH;
        [
            (HANDLERS[0].0, HANDLERS[0].1, gui, HANDLERS[0].2),
            (HANDLERS[1].0, HANDLERS[1].1, gui, HANDLERS[1].2),
            (HANDLERS[2].0, HANDLERS[2].1, sketching, HANDLERS[2].2),
            (HANDLERS[3].0, HANDLERS[3].1, sketching, HANDLERS[3].2),
        ]
    }

    /// THE REFERENCE IS CHECKED AGAINST THE CODE: every action of a handler is in the table.
    ///
    /// The check goes by ACTIONS and not by keys, and after rebinding it cannot go otherwise: the key
    /// now comes from the settings, and it is not in the source of the handler and must not be. The
    /// meaning of the guard did not change from that, it grew more precise — it catches the table
    /// diverging from the code rather than from a letter.
    #[test]
    fn every_handled_action_is_documented() {
        for (area, sig, src, _) in handler_sources() {
            for a in actions_in(body_of(src, sig)) {
                assert!(HOTKEYS.iter().any(|r| r.area == area && r.action == a), "the action {a} is handled in \"{area}\" and is not in the reference — the hotkey window will lie");
            }
        }
    }

    /// AND THE OTHER WAY ROUND: the reference holds no phantom actions the code does not handle.
    #[test]
    fn the_reference_lists_no_phantom_actions() {
        // EVERY PLACE OF THE AREA AT ONCE. An area may be handled in more than one place - the sketch names its
        // drawing tools in the workbench crate and the rest in the window - and an action found in either of
        // them is handled.
        for area in HANDLERS.iter().map(|h| h.0).collect::<std::collections::BTreeSet<_>>() {
            let mut acts: Vec<String> = Vec::new();
            for (a, sig, src, _) in handler_sources() {
                if a == area {
                    acts.extend(actions_in(body_of(src, sig)));
                }
            }
            for r in HOTKEYS.iter().filter(|r| r.area == area) {
                assert!(acts.contains(&r.action.to_string()), "the reference promises \"{}\" in \"{area}\" and the code handles no such action", r.action);
            }
        }
    }

    /// THE HANDLERS DO NOT MATCH THE KEY THEMSELVES.
    ///
    /// Let one of them go back to `match key { Key::E => ... }` and rebinding will start working in one
    /// workbench and silently not in another. That is the worst kind of breakage: the program does not
    /// crash, it quietly disobeys. Which keys lead to an action is decided once, by `hotkey_presses`, and every
    /// handler is called from there with the action.
    #[test]
    fn no_handler_matches_a_raw_key() {
        let input = include_str!("input.rs");
        let keys = body_of(input, "pub(super) fn handle_tool_hotkeys(");
        assert!(keys.contains("qymcad_ui_state::hotkey_presses("), "the key handler no longer matches the keys through `hotkey_presses`");
        for (area, sig, src, called) in handler_sources() {
            let body = body_of(src, sig);
            assert!(!called || keys.contains(&format!("self.{area}_hotkey(action)")), "the handler \"{area}\" is not called with the action the keys matched");
            // COMMENTS EXCLUDED: `Key::E` stands in them lawfully, as an explanation of why it is no
            // longer done that way. A guard that trips over an explanation teaches people to erase
            // explanations.
            let code: String = body.lines().map(|l| l.split("//").next().unwrap_or("")).collect::<Vec<_>>().join("\n");
            assert!(!code.contains("Key::"), "the handler \"{area}\" matches the key itself again — rebinding will not get past it");
        }
    }

    /// EVERY ROW OF THE REFERENCE HAS TEXT IN EVERY LANGUAGE.
    ///
    /// The reference stores KEYS and not phrases, and a missing translation would show up as a string
    /// like `hotkey-part-e` — that is, the hotkey window would lie in a way other than the tests above
    /// are afraid of.
    #[test]
    fn every_row_is_translated_in_every_language() {
        let prev = crate::i18n::language();
        let mut holes: Vec<String> = Vec::new();
        for (code, _) in crate::i18n::available() {
            crate::i18n::set_language(&code);
            for key in super::AREAS.iter().map(|a| format!("hotkeys-area-{a}")).chain(["settings-hotkeys".into(), "hotkeys-note".into()]).chain(HOTKEYS.iter().map(|r| r.what.to_string())) {
                let text = crate::i18n::tr(&key);
                if text == key || text.trim().is_empty() {
                    holes.push(format!("{code}: {key}"));
                }
            }
        }
        crate::i18n::set_language(&prev);
        assert!(holes.is_empty(), "the reference would show keys instead of words:\n{}", holes.join("\n"));
    }

    /// AND NO PHRASES ARE LEFT IN THE REFERENCE ITSELF — only keys. A guard against their return.
    #[test]
    fn the_reference_holds_keys_not_phrases() {
        let src = include_str!("hotkeys.rs");
        // the WORKING part of the file only: the guard is about the reference table, not about what
        // the tests below happen to quote
        let code = src.split("#[cfg(test)]").next().expect("the working part");
        let cyr: Vec<&str> =
            code.lines().filter(|l| !l.trim_start().starts_with("//")).filter(|l| l.contains('"') && l.chars().any(|c| ('а'..='я').contains(&c) || ('А'..='Я').contains(&c))).collect();
        assert!(cyr.is_empty(), "a phrase has appeared in the reference instead of a key again:\n{}", cyr.join("\n"));
    }

    /// The table is drawn in Settings -> Keyboard — otherwise the reference exists only in the code.
    #[test]
    fn the_table_is_drawn_in_the_keyboard_settings() {
        let panels = crate::gui::panels_source::PANELS;
        let body = &panels[panels.find("Sec::Keyboard => {").expect("the keyboard section")..];
        let body = &body[..body.find("Sec::Appearance => {").expect("the section after it")];
        assert!(body.contains("crate::gui::hotkeys::hotkeys_table(wc, ui, ctx);"), "the keyboard section must draw the table of keys");
    }
}

/// THE SETTINGS OPENED AT THE KEYBOARD SECTION, as the start screen opens them - for the checks that draw the table.
#[cfg(test)]
pub(crate) fn open_keyboard_settings(app: &mut super::App) {
    app.win.open(WinKind::Settings);
    app.scheme.section = qymcad_ui_state::settings_sections::SettingsSection::Keyboard;
}

/// One frame of the settings window, which holds the table of keys while the keyboard section is chosen.
#[cfg(test)]
pub(crate) fn draw_settings(app: &mut super::App, ctx: &egui::Context) {
    let mut asks = Vec::new();
    crate::gui::panels_windows::settings_window(&mut app.win_ctx(&mut asks), ctx);
    app.do_win_asks(asks, ctx);
}

#[cfg(test)]
mod areas_are_complete {
    /// EVERY AREA THE CATALOGUE KNOWS IS SHOWN.
    ///
    /// A hand-written list beside a full one goes stale in silence: the keys of a forgotten area appear
    /// in no panel, and nothing says so. The order here is a decision and stays by hand; the SET is read
    /// out of the catalogue.
    #[test]
    fn the_shown_areas_are_the_ones_the_catalogue_has() {
        let mut from_catalogue: Vec<&str> = qymcad_ui_state::HOTKEYS.iter().map(|r| r.area).collect();
        from_catalogue.sort_unstable();
        from_catalogue.dedup();
        let mut shown: Vec<&str> = super::AREAS.to_vec();
        shown.sort_unstable();
        assert!(!from_catalogue.is_empty(), "the catalogue was not read at all");
        assert_eq!(shown, from_catalogue, "an area of hotkeys is in the catalogue and in no panel (or the other way round): its keys are shown nowhere");
    }
}
