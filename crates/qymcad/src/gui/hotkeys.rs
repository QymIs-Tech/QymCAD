//! THE HOTKEY REFERENCE — the single source for the Help -> Hotkeys window.
//!
//! There are more than sixty keys in the application, and they lived ONLY in the tooltips of the
//! buttons: the only way to learn them was to hover the mouse over every one. A list typed apart from
//! the handlers would diverge from them at the very first edit — so a test stands next to it that checks
//! the table against THE SOURCE of the handlers: a key appears in
//! `part_hotkey`/`sketch_hotkey`/`assembly_hotkey` and is not in the table (or the other way round) and
//! the test is red.
use super::App;

/// One row of the reference. THERE IS NO TEXT HERE — only keys of the language catalogue: the reference
/// is read by a person, so it must speak their language like the rest of the interface.
pub(crate) struct HotkeyRow {
    /// Where it acts: `general`, `part`, `sketch`, `assembly`. A code and not a caption: the check
    /// against the handlers goes by it, and it cannot depend on the language.
    pub area: &'static str,
    /// WHAT IT DOES — a stable code of the action, and it is this that comes first.
    ///
    /// A key is rebindable, an action is not. While the handler matched THE KEY, rebinding was
    /// inexpressible: "E" meant "extrude" right inside the `match`. Now the key leads to an action and
    /// the action to a branch of the handler, and the first can be moved without touching the second.
    ///
    /// It is also the key the rebinding is stored under in the settings. So it must NOT contain the
    /// letter of the key: `part.extrude` and not `part.e`, otherwise the record "part.e = X" would lie
    /// to itself.
    ///
    /// WITH A DOT AND NOT A HYPHEN, and that is not taste: the keys of the language catalogue are
    /// written with hyphens, and the guard against a key reaching the screen untranslated catches ANY
    /// literal of that shape. The code of an action is not text for a person and must not be translated;
    /// the dot tells one from the other by eye and in the guards.
    pub action: &'static str,
    /// The DEFAULT key, exactly as in `egui::Key`. What is actually pressed — see `App::hotkey_key`.
    pub key: &'static str,
    /// A catalogue key (`hotkey-<area>-<key>`), not a phrase.
    pub what: &'static str,
}

impl HotkeyRow {
    /// What the key does — in the language of the person.
    pub fn what(&self) -> String {
        crate::i18n::tr(self.what)
    }
}

/// THE AREAS in the order they are shown: the code and the key of its caption.
pub(crate) const AREAS: [&str; 4] = ["general", "part", "sketch", "assembly"];

/// EVERY HOTKEY. `key` is exactly the name of the `egui::Key` variant (the test checks against the code
/// by it), except for the rows of the general area, where the keys are handled apart from the `match`
/// table.
pub(crate) const HOTKEYS: &[HotkeyRow] = &[
    // --- General ---
    HotkeyRow { area: "general", action: "general.esc", key: "Esc", what: "hotkey-general-esc" },
    HotkeyRow { area: "general", action: "general.enter", key: "Enter", what: "hotkey-general-enter" },
    HotkeyRow { area: "general", action: "general.delete", key: "Delete", what: "hotkey-general-delete" },
    HotkeyRow { area: "general", action: "general.undo-redo", key: "Ctrl+Z / Ctrl+Y", what: "hotkey-general-ctrl-z-ctrl-y" },
    HotkeyRow { area: "general", action: "general.save", key: "Ctrl+S", what: "hotkey-general-ctrl-s" },
    // --- Part ---
    HotkeyRow { area: "part", action: "part.sketch-pick", key: "K", what: "hotkey-part-k" },
    HotkeyRow { area: "part", action: "part.datum-plane", key: "D", what: "hotkey-part-d" },
    HotkeyRow { area: "part", action: "part.extrude", key: "E", what: "hotkey-part-e" },
    HotkeyRow { area: "part", action: "part.cut", key: "Q", what: "hotkey-part-q" },
    HotkeyRow { area: "part", action: "part.revolve", key: "R", what: "hotkey-part-r" },
    HotkeyRow { area: "part", action: "part.fillet", key: "F", what: "hotkey-part-f" },
    HotkeyRow { area: "part", action: "part.chamfer", key: "C", what: "hotkey-part-c" },
    HotkeyRow { area: "part", action: "part.shell", key: "H", what: "hotkey-part-h" },
    HotkeyRow { area: "part", action: "part.hole", key: "O", what: "hotkey-part-o" },
    HotkeyRow { area: "part", action: "part.mirror", key: "M", what: "hotkey-part-m" },
    HotkeyRow { area: "part", action: "part.box", key: "B", what: "hotkey-part-b" },
    HotkeyRow { area: "part", action: "part.cylinder", key: "Y", what: "hotkey-part-y" },
    HotkeyRow { area: "part", action: "part.measure", key: "I", what: "hotkey-part-i" },
    HotkeyRow { area: "part", action: "part.contour-reselect", key: "U", what: "hotkey-part-u" },
    // --- Sketch ---
    HotkeyRow { area: "sketch", action: "sketch.select", key: "S", what: "hotkey-sketch-s" },
    HotkeyRow { area: "sketch", action: "sketch.line", key: "L", what: "hotkey-sketch-l" },
    HotkeyRow { area: "sketch", action: "sketch.rect", key: "R", what: "hotkey-sketch-r" },
    HotkeyRow { area: "sketch", action: "sketch.circle", key: "C", what: "hotkey-sketch-c" },
    HotkeyRow { area: "sketch", action: "sketch.arc", key: "A", what: "hotkey-sketch-a" },
    HotkeyRow { area: "sketch", action: "sketch.point", key: "P", what: "hotkey-sketch-p" },
    HotkeyRow { area: "sketch", action: "sketch.polygon", key: "G", what: "hotkey-sketch-g" },
    HotkeyRow { area: "sketch", action: "sketch.slot", key: "O", what: "hotkey-sketch-o" },
    HotkeyRow { area: "sketch", action: "sketch.ellipse", key: "E", what: "hotkey-sketch-e" },
    HotkeyRow { area: "sketch", action: "sketch.spline", key: "N", what: "hotkey-sketch-n" },
    HotkeyRow { area: "sketch", action: "sketch.text", key: "T", what: "hotkey-sketch-t" },
    HotkeyRow { area: "sketch", action: "sketch.dim", key: "D", what: "hotkey-sketch-d" },
    HotkeyRow { area: "sketch", action: "sketch.corner-fillet", key: "F", what: "hotkey-sketch-f" },
    HotkeyRow { area: "sketch", action: "sketch.trim", key: "K", what: "hotkey-sketch-k" },
    HotkeyRow { area: "sketch", action: "sketch.mirror", key: "M", what: "hotkey-sketch-m" },
    HotkeyRow { area: "sketch", action: "sketch.construction", key: "X", what: "hotkey-sketch-x" },
    // --- Assembly ---
    HotkeyRow { area: "assembly", action: "assembly.datum-plane", key: "D", what: "hotkey-assembly-d" },
    HotkeyRow { area: "assembly", action: "assembly.new-part", key: "N", what: "hotkey-assembly-n" },
    HotkeyRow { area: "assembly", action: "assembly.new-subassembly", key: "U", what: "hotkey-assembly-u" },
    HotkeyRow { area: "assembly", action: "assembly.insert", key: "I", what: "hotkey-assembly-i" },
    HotkeyRow { area: "assembly", action: "assembly.rigid-joint", key: "J", what: "hotkey-assembly-j" },
];

/// WHETHER A KEY IS REBINDABLE. The general area is not, and that is not laziness.
///
/// Esc, Enter, Delete, Ctrl+Z, Ctrl+S are an agreement of the whole operating system, not a layout of
/// ours. Letting them be moved means letting a person end up without undo at the very moment it is
/// needed most, with no way at all to notice. What gets moved are the keys of the WORKBENCHES.
pub(crate) fn rebindable(area: &str) -> bool {
    area != "general"
}

impl App {
    /// The key of an action WITH REBINDING TAKEN INTO ACCOUNT: the one set by a person if there is
    /// one, otherwise the factory key.
    pub(crate) fn hotkey_key(&self, action: &str) -> String {
        if let Some(k) = self.set.hotkeys.get(action) {
            return k.clone();
        }
        HOTKEYS.iter().find(|r| r.action == action).map(|r| r.key.to_string()).unwrap_or_default()
    }

    /// WHAT TO DO ON THIS KEY in this area — the only path from a press to an action.
    ///
    /// The handlers ask HERE rather than matching `Key::` at home: otherwise rebinding would work in one
    /// workbench and silently not in another, and that would diverge at the very first edit.
    pub(crate) fn hotkey_action(&self, area: &str, key: egui::Key) -> Option<&'static str> {
        let pressed = key.name();
        HOTKEYS.iter().filter(|r| r.area == area).find(|r| self.hotkey_key(r.action) == pressed).map(|r| r.action)
    }

    /// WHETHER THE KEY in this area is taken by somebody else — the name of the neighbouring action.
    ///
    /// Two commands on one key is not "the last one wins" but a silently lost tool: the habitual key is
    /// pressed, something else arrives, and it is not clear what broke. So rebinding asks here first.
    pub(crate) fn hotkey_taken_by(&self, area: &str, key: &str, except: &str) -> Option<&'static str> {
        HOTKEYS.iter().filter(|r| r.area == area && r.action != except).find(|r| self.hotkey_key(r.action) == key).map(|r| r.action)
    }

    /// The Help -> Hotkeys window: a table by areas.
    pub(super) fn hotkeys_window(&mut self, ctx: &egui::Context) {
        if !self.win.hotkeys {
            return;
        }
        let mut open = true;
        egui::Window::new(crate::i18n::tr("hotkeys-title")).open(&mut open).resizable(true).default_width(520.0).show(ctx, |ui| {
            egui::ScrollArea::vertical().max_height(560.0).show(ui, |ui| {
                for area in AREAS {
                    ui.label(egui::RichText::new(crate::i18n::tr(&format!("hotkeys-area-{area}"))).strong());
                    egui::Grid::new(format!("hk_{area}")).num_columns(3).spacing([14.0, 3.0]).striped(true).show(ui, |ui| {
                        for r in HOTKEYS.iter().filter(|r| r.area == area) {
                            let cur = self.hotkey_key(r.action);
                            let waiting = self.hotkey_capture.as_deref() == Some(r.action);
                            if rebindable(area) {
                                // THE KEY IS A BUTTON. Press it, the program waits for a press, it is
                                // recorded. A text field here would be a lie: modifiers would be typed
                                // into it as words.
                                let label = if waiting { crate::i18n::tr("hotkeys-press") } else { cur.clone() };
                                if ui.add(egui::Button::new(egui::RichText::new(label).monospace().strong()).min_size(egui::vec2(84.0, 0.0))).clicked() {
                                    self.hotkey_capture = if waiting { None } else { Some(r.action.to_string()) };
                                    self.hotkey_note.clear();
                                }
                            } else {
                                ui.label(egui::RichText::new(&cur).monospace().strong());
                            }
                            ui.label(r.what());
                            // "restore the factory key" only where it really was changed
                            if rebindable(area) && self.set.hotkeys.contains_key(r.action) {
                                if ui.small_button(&crate::i18n::tr("hotkeys-reset-one")).on_hover_text(crate::i18n::tr1("hotkeys-default-is", "key", r.key)).clicked() {
                                    self.set.hotkeys.remove(r.action);
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
                if !self.hotkey_note.is_empty() {
                    ui.label(egui::RichText::new(&self.hotkey_note).color(self.scheme.pal.error_mild()).small());
                }
                if !self.set.hotkeys.is_empty() && ui.button(&crate::i18n::tr("hotkeys-reset-all")).clicked() {
                    self.set.hotkeys.clear();
                    self.hotkey_note.clear();
                }
            });
        });
        self.win.hotkeys = open;
        self.capture_hotkey(ctx);
    }

    /// CATCH THE KEY PRESSED while the window waits for an assignment.
    ///
    /// The refusals are honest and carry a reason: a taken key does NOT silently outbid its neighbour
    /// (otherwise a tool disappears and it is not clear what broke), and the service keys
    /// Esc/Enter/Delete/Tab are not assignable at all — Esc must stay the way out even of this very
    /// mode.
    fn capture_hotkey(&mut self, ctx: &egui::Context) {
        let Some(action) = self.hotkey_capture.clone() else { return };
        let Some(area) = HOTKEYS.iter().find(|r| r.action == action).map(|r| r.area) else {
            self.hotkey_capture = None;
            return;
        };
        let pressed: Option<egui::Key> = ctx.input(|i| i.events.iter().find_map(|e| matches!(e, egui::Event::Key { pressed: true, .. }).then(|| if let egui::Event::Key { key, .. } = e { Some(*key) } else { None }).flatten()));
        let Some(key) = pressed else { return };
        if key == egui::Key::Escape {
            self.hotkey_capture = None; // leaving the mode rather than assigning Esc
            return;
        }
        if matches!(key, egui::Key::Enter | egui::Key::Delete | egui::Key::Tab | egui::Key::Backspace) {
            self.hotkey_note = crate::i18n::tr("hotkeys-reserved");
            return;
        }
        let name = key.name().to_string();
        if let Some(other) = self.hotkey_taken_by(area, &name, &action) {
            let what = HOTKEYS.iter().find(|r| r.action == other).map(|r| r.what()).unwrap_or_default();
            self.hotkey_note = crate::i18n::tr2("hotkeys-taken", "key", &name, "what", &what);
            return;
        }
        // it matches the factory key — no override is needed, the record is kept CLEAN
        if HOTKEYS.iter().any(|r| r.action == action && r.key == name) {
            self.set.hotkeys.remove(&action);
        } else {
            self.set.hotkeys.insert(action, name);
        }
        self.hotkey_capture = None;
        self.hotkey_note.clear();
    }
}

#[cfg(test)]
mod tests {
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
        let sketching = include_str!("sketching.rs");
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
        assert!(panels.contains("self.win.hotkeys = true;"), "the window must open from the Help menu");
        assert!(include_str!("../gui.rs").contains("self.hotkeys_window(ctx);"), "the window must be drawn in the frame");
    }
}
