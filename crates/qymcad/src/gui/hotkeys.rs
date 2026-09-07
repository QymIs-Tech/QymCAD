//! THE HOTKEY REFERENCE — the single source for the Help -> Hotkeys window.
//!
//! There are more than sixty keys in the application, and they lived ONLY in the tooltips of the
//! buttons: the only way to learn them was to hover the mouse over every one. A list typed apart from
//! the handlers would diverge from them at the very first edit — so a test stands next to it that checks
//! the table against THE SOURCE of the handlers: a key appears in
//! `part_hotkey`/`sketch_hotkey`/`assembly_hotkey` and is not in the table (or the other way round) and
//! the test is red.
pub(crate) use qymcad_ui_state::{HotkeyRow, HOTKEYS};
use super::App;
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

impl App {
    /// The Help -> Hotkeys window: the door that builds the narrow context and hands it to the free function below.
    pub(super) fn hotkeys_window(&mut self, ctx: &egui::Context) {
        let mut asks = Vec::new();
        hotkeys_window(&mut self.win_ctx(&mut asks), ctx);
        self.do_win_asks(asks, ctx);
    }

}

/// THE HOTKEY WINDOW. A reference that can be edited: every key of a workbench is a button, pressing it puts
/// the window into waiting, and the next press is recorded.
pub(crate) fn hotkeys_window(wc: &mut qymcad_ui_state::WinCtx, ctx: &egui::Context) {
    if !wc.win.is(WinKind::Hotkeys) {
        return;
    }
    let mut open = true;
    egui::Window::new(crate::i18n::tr("hotkeys-title")).open(&mut open).resizable(true).default_width(520.0).show(ctx, |ui| {
        egui::ScrollArea::vertical().max_height(560.0).show(ui, |ui| {
            for area in AREAS {
                ui.label(egui::RichText::new(crate::i18n::tr(&format!("hotkeys-area-{area}"))).strong());
                egui::Grid::new(format!("hk_{area}")).num_columns(3).spacing([14.0, 3.0]).striped(true).show(ui, |ui| {
                    for r in HOTKEYS.iter().filter(|r| r.area == area) {
                        let cur = qymcad_ui_state::hotkey_key(wc.set, r.action);
                        let waiting = wc.hotkeys.action.as_deref() == Some(r.action);
                        if rebindable(area) {
                            // THE KEY IS A BUTTON. Press it, the program waits for a press, it is
                            // recorded. A text field here would be a lie: modifiers would be typed
                            // into it as words.
                            let label = if waiting { crate::i18n::tr("hotkeys-press") } else { cur.clone() };
                            if ui.add(egui::Button::new(egui::RichText::new(label).monospace().strong()).min_size(egui::vec2(84.0, 0.0))).clicked() {
                                wc.hotkeys.action = if waiting { None } else { Some(r.action.to_string()) };
                                wc.hotkeys.note.clear();
                            }
                        } else {
                            ui.label(egui::RichText::new(&cur).monospace().strong());
                        }
                        ui.label(crate::gui::hotkeys::hotkey_what(r));
                        // "restore the factory key" only where it really was changed
                        if rebindable(area) && wc.set.hotkeys.contains_key(r.action) {
                            if ui.small_button(crate::i18n::tr("hotkeys-reset-one")).on_hover_text(crate::i18n::tr1("hotkeys-default-is", "key", r.key)).clicked() {
                                wc.set.hotkeys.remove(r.action);
                            }
                        } else {
                            ui.label("");
                        }
                        ui.end_row();
                    }
                });
                ui.add_space(8.0);
            }
            ui.label(egui::RichText::new(crate::i18n::tr("hotkeys-note")).weak().small());
            // THE FOCUS RULE GOES HERE AND NOT ONLY IN THE HELP. A caret in a field extinguishes
            // bare letters (otherwise `w` in an expression would launch a command), and Alt is the
            // only way to reach a tool from there. Not saying so in the hotkey reference means
            // hiding half the rule: U is pressed in the length field, nothing happens, and the
            // conclusion drawn is about the program.
            ui.label(egui::RichText::new(crate::i18n::tr("hotkeys-alt-note")).weak().small());
            ui.label(egui::RichText::new(crate::i18n::tr("hotkeys-rebind-note")).weak().small());
            if !wc.hotkeys.note.is_empty() {
                ui.label(egui::RichText::new(&wc.hotkeys.note).color(wc.scheme.pal.error_mild()).small());
            }
            if !wc.set.hotkeys.is_empty() && ui.button(crate::i18n::tr("hotkeys-reset-all")).clicked() {
                wc.set.hotkeys.clear();
                wc.hotkeys.note.clear();
            }
        });
    });
    wc.win.set(WinKind::Hotkeys, open);
    capture_hotkey(wc, ctx);
}

/// THE PRESS THAT ASSIGNS A KEY, while the window waits for one.
fn capture_hotkey(wc: &mut qymcad_ui_state::WinCtx, ctx: &egui::Context) {
    let Some(action) = wc.hotkeys.action.clone() else { return };
    let Some(area) = HOTKEYS.iter().find(|r| r.action == action).map(|r| r.area) else {
        wc.hotkeys.action = None;
        return;
    };
    let pressed: Option<egui::Key> = ctx.input(|i| i.events.iter().find_map(|e| matches!(e, egui::Event::Key { pressed: true, .. }).then(|| if let egui::Event::Key { key, .. } = e { Some(*key) } else { None }).flatten()));
    let Some(key) = pressed else { return };
    if key == egui::Key::Escape {
        wc.hotkeys.action = None; // leaving the mode rather than assigning Esc
        return;
    }
    if matches!(key, egui::Key::Enter | egui::Key::Delete | egui::Key::Tab | egui::Key::Backspace) {
        wc.hotkeys.note = crate::i18n::tr("hotkeys-reserved");
        return;
    }
    let name = key.name().to_string();
    if let Some(other) = qymcad_ui_state::hotkey_taken_by(wc.set, area, &name, &action) {
        let what = HOTKEYS.iter().find(|r| r.action == other).map(crate::gui::hotkeys::hotkey_what).unwrap_or_default();
        wc.hotkeys.note = crate::i18n::tr2("hotkeys-taken", "key", &name, "what", &what);
        return;
    }
    // it matches the factory key - no override is needed, the record is kept CLEAN
    if HOTKEYS.iter().any(|r| r.action == action && r.key == name) {
        wc.set.hotkeys.remove(&action);
    } else {
        wc.set.hotkeys.insert(action, name);
    }
    wc.hotkeys.action = None;
    wc.hotkeys.note.clear();
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

    const HANDLERS: [(&str, &str); 3] = [
        ("part", "pub(super) fn part_hotkey(&mut self, key: egui::Key) {"),
        ("assembly", "pub(super) fn assembly_hotkey(&mut self, key: egui::Key) {"),
        ("sketch", "pub(super) fn sketch_hotkey(&mut self, key: egui::Key)"),
    ];

    fn handler_sources() -> [(&'static str, &'static str, &'static str); 3] {
        let gui = include_str!("../gui.rs");
        let sketching = crate::gui::sketch_source::SKETCH;
        [(HANDLERS[0].0, HANDLERS[0].1, gui), (HANDLERS[1].0, HANDLERS[1].1, gui), (HANDLERS[2].0, HANDLERS[2].1, sketching)]
    }

    /// THE REFERENCE IS CHECKED AGAINST THE CODE: every action of a handler is in the table.
    ///
    /// The check goes by ACTIONS and not by keys, and after rebinding it cannot go otherwise: the key
    /// now comes from the settings, and it is not in the source of the handler and must not be. The
    /// meaning of the guard did not change from that, it grew more precise — it catches the table
    /// diverging from the code rather than from a letter.
    #[test]
    fn every_handled_action_is_documented() {
        for (area, sig, src) in handler_sources() {
            for a in actions_in(body_of(src, sig)) {
                assert!(
                    HOTKEYS.iter().any(|r| r.area == area && r.action == a),
                    "the action {a} is handled in \"{area}\" and is not in the reference — the hotkey window will lie"
                );
            }
        }
    }

    /// AND THE OTHER WAY ROUND: the reference holds no phantom actions the code does not handle.
    #[test]
    fn the_reference_lists_no_phantom_actions() {
        for (area, sig, src) in handler_sources() {
            let acts = actions_in(body_of(src, sig));
            for r in HOTKEYS.iter().filter(|r| r.area == area) {
                assert!(acts.contains(&r.action.to_string()), "the reference promises \"{}\" in \"{area}\" and the code handles no such action", r.action);
            }
        }
    }

    /// THE HANDLERS DO NOT MATCH THE KEY THEMSELVES.
    ///
    /// Let one of them go back to `match key { Key::E => ... }` and rebinding will start working in one
    /// workbench and silently not in another. That is the worst kind of breakage: the program does not
    /// crash, it quietly disobeys.
    #[test]
    fn no_handler_matches_a_raw_key() {
        for (area, sig, src) in handler_sources() {
            let body = body_of(src, sig);
            assert!(body.contains("hotkey_action("), "the handler \"{area}\" has stopped asking `hotkey_action`");
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
            for key in super::AREAS.iter().map(|a| format!("hotkeys-area-{a}")).chain(["hotkeys-title".into(), "hotkeys-note".into()]).chain(HOTKEYS.iter().map(|r| r.what.to_string())) {
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
        let cyr: Vec<&str> = code
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .filter(|l| l.contains('"') && l.chars().any(|c| ('а'..='я').contains(&c) || ('А'..='Я').contains(&c)))
            .collect();
        assert!(cyr.is_empty(), "a phrase has appeared in the reference instead of a key again:\n{}", cyr.join("\n"));
    }

    /// The window opens from the Help menu — otherwise the reference exists only in the code.
    #[test]
    fn the_window_is_reachable_from_the_menu() {
        let panels = crate::gui::panels_source::PANELS;
        assert!(panels.contains(".win.open(WinKind::Hotkeys);"), "the window must open from the Help menu");
        assert!(include_str!("../gui.rs").contains("self.hotkeys_window(ctx);"), "the window must be drawn in the frame");
    }
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
        assert_eq!(
            shown, from_catalogue,
            "an area of hotkeys is in the catalogue and in no panel (or the other way round): its keys are shown nowhere"
        );
    }
}
