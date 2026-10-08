//! HOTKEYS CAN BE REBOUND.
//!
//! The reference sheet was for viewing only, and it could not have been otherwise: the handlers
//! matched on THE KEY (`match key { Key::E => extrude }`), that is, the letter and the meaning were
//! one and the same. Moving the letter would have meant rewriting the `match` — rebinding was
//! inexpressible by construction.
//!
//! Now THE ACTION comes first: a key leads to an action (`hotkey_action`), and the action to a branch
//! of the handler. The first link is what moves; the second is not touched at all.
#[cfg(test)]
mod tests {
    use super::super::hotkeys::{rebindable, HOTKEYS};
    use super::super::App;

    /// THE FACTORY LAYOUT IS IN FORCE while nobody has touched it.
    #[test]
    fn out_of_the_box_the_default_layout_is_in_force() {
        let app = App::default();
        assert_eq!(qymcad_ui_state::hotkey_action(&app.set, "part", egui::Key::E), Some("part.extrude"), "the factory E in a Part must extrude");
        assert_eq!(qymcad_ui_state::hotkey_action(&app.set, "sketch", egui::Key::L), Some("sketch.line"), "the factory L in a Sketch must draw a line");
        assert_eq!(qymcad_ui_state::hotkey_action(&app.set, "part", egui::Key::Z), None, "a free key is not obliged to mean anything");
    }

    /// THE POINT: after a rebind THE NEW key works and the old one stops.
    #[test]
    fn a_reassigned_key_takes_over_and_the_old_one_stops() {
        let mut app = App::default();
        app.set.hotkeys.insert("part.extrude".into(), "W".into());
        assert_eq!(qymcad_ui_state::hotkey_action(&app.set, "part", egui::Key::W), Some("part.extrude"), "the new key does not work — the rebinding is useless");
        assert_eq!(qymcad_ui_state::hotkey_action(&app.set, "part", egui::Key::E), None, "the old key still extrudes — now there are two of them");
    }

    /// A REBINDING LIVES IN THE SETTINGS, and so survives a restart.
    #[test]
    fn a_reassignment_survives_a_restart() {
        let mut app = App::default();
        app.set.hotkeys.insert("sketch.circle".into(), "Z".into());
        let ron = ron::ser::to_string(&app.set).expect("the settings serialise");
        let back: super::super::Settings = ron::from_str(&ron).expect("and read back");
        let restarted = App { set: back, ..Default::default() };
        assert_eq!(qymcad_ui_state::hotkey_action(&restarted.set, "sketch", egui::Key::Z), Some("sketch.circle"), "after a restart the key went back to the factory one — the edit is lost");
    }

    /// ONLY THE DIFFERENCES ARE STORED. A full layout in the record would mean that a new tool of the
    /// program never appears for anyone who has ever touched the keys: its action is simply not in the
    /// record.
    #[test]
    fn only_the_differences_are_stored() {
        let app = App::default();
        assert!(app.set.hotkeys.is_empty(), "the factory layout must not be stored — it is known anyway");
    }

    /// A TAKEN KEY IS REPORTED BEFORE IT IS ASSIGNED. Two commands on one key is not "the last one
    /// wins", it is a silently lost tool.
    #[test]
    fn a_taken_key_is_reported_before_it_is_assigned() {
        let app = App::default();
        assert_eq!(qymcad_ui_state::hotkey_taken_by(&app.set, "part", "F", "part.extrude"), Some("part.fillet"), "the key F being taken in a Part went unnoticed");
        assert_eq!(qymcad_ui_state::hotkey_taken_by(&app.set, "part", "Z", "part.extrude"), None, "a free key was called taken");
        // one letter in DIFFERENT workbenches is not a conflict: F in a Sketch and F in a Part live apart
        assert_eq!(qymcad_ui_state::hotkey_taken_by(&app.set, "sketch", "B", "sketch.line"), None, "a key of another workbench was counted as taken");
    }

    /// THE SYSTEM KEYS ARE NOT REBOUND — and that shows in the data, not in the good will of a window.
    #[test]
    fn the_system_keys_are_not_offered_for_rebinding() {
        assert!(!rebindable("general"), "the general area was given up for rebinding: Esc and Ctrl+Z belong to the system, not to us");
        for a in ["part", "sketch", "assembly"] {
            assert!(rebindable(a), "the \"{a}\" workbench must be rebindable");
        }
    }

    /// EVERY ACTION HAS A NAME OF ITS OWN, AND THE KEY LETTER IS NOT IN IT.
    ///
    /// The action name is the key of the settings record. Call it `part.e` and after a rebind to W the
    /// record "part.e = W" becomes a lie about itself; and a duplicate name would quietly glue two
    /// commands together.
    #[test]
    fn action_names_are_unique_and_say_nothing_about_the_key() {
        let mut seen: Vec<&str> = Vec::new();
        for r in HOTKEYS {
            assert!(!seen.contains(&r.action), "the action \"{}\" is declared twice", r.action);
            seen.push(r.action);
            let tail = r.action.split_once('.').map(|(_, t)| t).unwrap_or("");
            assert!(tail.len() > 1, "the action name \"{}\" is made of the key letter — after a rebind it will lie", r.action);
        }
    }

    /// AND THE FACTORY KEYS WITHIN ONE WORKBENCH DO NOT ARGUE WITH EACH OTHER.
    #[test]
    fn the_default_layout_has_no_conflicts() {
        for area in super::super::hotkeys::AREAS {
            let mut used: Vec<(&str, &str)> = Vec::new();
            for r in HOTKEYS.iter().filter(|r| r.area == area) {
                if let Some((k, other)) = used.iter().find(|(k, _)| *k == r.key) {
                    panic!("in \"{area}\" the key {k} is taken twice: \"{other}\" and \"{}\"", r.action);
                }
                used.push((r.key, r.action));
            }
        }
    }

    use super::super::hotkeys::{capture_outcome, finish_recording, Capture};
    use egui::{Key, Modifiers};
    use qymcad_ui_state::{reset_hotkey, resolve_hotkey_clash, Chord, ClashChoice, HotkeyClash};

    /// A CHORD READS BACK AS IT IS WRITTEN, and the order of the modifiers in a hand-edited record does not matter.
    #[test]
    fn a_chord_reads_back_as_written() {
        for s in ["W", "Shift+W", "Ctrl+W", "Ctrl+Shift+F5", "7"] {
            assert_eq!(Chord::parse(s).map(|c| c.name()).as_deref(), Some(s), "{s} did not survive a round trip");
        }
        assert_eq!(Chord::parse("Shift+Ctrl+W"), Chord::parse("Ctrl+Shift+W"), "the same chord written the other way round is another key");
        for s in ["", "Ctrl+", "Ctrl+Z / Ctrl+Y", "Hyper+W", "NoSuchKey"] {
            assert_eq!(Chord::parse(s), None, "{s:?} was read as a chord");
        }
    }

    /// THE WINDOW RECORDS A CHORD, NOT ONLY A LETTER.
    #[test]
    fn the_window_records_a_chord() {
        let app = App::default();
        let mods = Modifiers { shift: true, ..Modifiers::COMMAND };
        let ctrl_shift_j = Chord::of_press(mods, Key::J);
        assert_eq!(capture_outcome(&app.set, "part", "part.extrude", &[], Key::J, mods), Capture::Record(vec![ctrl_shift_j]));
        assert_eq!(finish_recording(&app.set, "part", "part.extrude", &[ctrl_shift_j]), Capture::Bind("Ctrl+Shift+J".into()));
        assert_eq!(capture_outcome(&app.set, "part", "part.extrude", &[], Key::W, Modifiers::NONE), Capture::Record(vec![Chord::from(Key::W)]));
    }

    /// WHAT THE SYSTEM HOLDS IS REFUSED: undo, the clipboard, Space, F1 - and Alt, which reaches keys from a field.
    #[test]
    fn the_window_refuses_what_belongs_to_the_system() {
        let app = App::default();
        for (key, mods) in [
            (Key::Z, Modifiers::COMMAND),
            (Key::S, Modifiers::COMMAND),
            (Key::K, Modifiers::COMMAND),
            (Key::Space, Modifiers::NONE),
            (Key::F1, Modifiers::NONE),
            (Key::Enter, Modifiers::NONE),
            (Key::ArrowUp, Modifiers::NONE),
        ] {
            assert!(matches!(capture_outcome(&app.set, "part", "part.extrude", &[], key, mods), Capture::Refused(_)), "{key:?} with {mods:?} was accepted for a tool");
        }
        assert_eq!(capture_outcome(&app.set, "part", "part.extrude", &[], Key::W, Modifiers::ALT), Capture::Refused("hotkeys-no-alt"));
        // bare X belongs to the general area - except to the action whose factory key it is
        assert!(matches!(capture_outcome(&app.set, "sketch", "sketch.line", &[], Key::X, Modifiers::NONE), Capture::Refused(_)), "bare X went to a tool, and it toggles construction everywhere");
        assert!(
            !matches!(capture_outcome(&app.set, "sketch", "sketch.construction", &[], Key::X, Modifiers::NONE), Capture::Refused(_)),
            "the construction toggle cannot be put back on its own factory key"
        );
    }

    /// THE GENERAL CTRL LETTERS ARE REFUSED WITH SHIFT AS WELL. Their handlers do not look at Shift: Ctrl+Shift+A
    /// still selects all, Ctrl+Shift+K still opens the search, Ctrl+Shift+Y still redoes - a tool there would
    /// fire together with them. Shift+X is free: the construction toggle asks for a bare X.
    #[test]
    fn the_general_ctrl_letters_are_refused_with_shift_too() {
        let app = App::default();
        let ctrl_shift = Modifiers { shift: true, ..Modifiers::COMMAND };
        for key in [Key::A, Key::C, Key::K, Key::S, Key::V, Key::X, Key::Y, Key::Z] {
            for mods in [Modifiers::COMMAND, ctrl_shift] {
                assert_eq!(capture_outcome(&app.set, "part", "part.extrude", &[], key, mods), Capture::Refused("hotkeys-reserved"), "{key:?} with {mods:?} went to a tool");
            }
        }
        assert_eq!(capture_outcome(&app.set, "part", "part.extrude", &[], Key::X, Modifiers::SHIFT), Capture::Record(vec![Chord::of_press(Modifiers::SHIFT, Key::X)]));
    }

    /// ON LINUX AND WINDOWS THE LETTERS A FIELD EDITS WITH ARE REFUSED UNDER CTRL: egui erases with Ctrl+H, Ctrl+U
    /// and Ctrl+W inside a field, Shift or not, and a tool there would run while the expression lost a word.
    #[test]
    fn on_linux_and_windows_the_letters_a_field_edits_with_are_refused() {
        use qymcad_ui_state::{hotkey_refusal_on, platform_keys::Os};
        for os in [Os::Linux, Os::Windows] {
            for key in [Key::H, Key::U, Key::W] {
                for shift in [false, true] {
                    let chord = Chord { ctrl: true, shift, ..Chord::from(key) };
                    assert_eq!(hotkey_refusal_on(os, "part.extrude", chord), Some("hotkeys-field-edits"), "{os:?}: {} went to a tool", chord.name());
                }
            }
            // the bare letters stay free: a field types them, and Alt reaches the tool from there
            assert_eq!(hotkey_refusal_on(os, "part.extrude", Chord::from(Key::W)), None);
        }
    }

    /// ON A MAC THE BINDING'S CTRL IS CMD, which a field does not edit with: Cmd+U and Cmd+W are free there. What the
    /// window takes is Cmd+H (hide) and Cmd+Q (quit) - each exactly, so Shift+Cmd+H is free.
    #[test]
    fn on_a_mac_cmd_h_and_cmd_q_are_kept_and_the_field_letters_are_free() {
        use qymcad_ui_state::{hotkey_refusal_on, platform_keys::Os};
        let cmd = |key| Chord { ctrl: true, ..Chord::from(key) };
        assert_eq!(hotkey_refusal_on(Os::Mac, "part.extrude", cmd(Key::H)), Some("hotkeys-os-hide"));
        assert_eq!(hotkey_refusal_on(Os::Mac, "part.extrude", cmd(Key::Q)), Some("hotkeys-os-quit"));
        for key in [Key::U, Key::W] {
            assert_eq!(hotkey_refusal_on(Os::Mac, "part.extrude", cmd(key)), None, "Cmd+{key:?} edits nothing on a Mac and was refused");
        }
        assert_eq!(hotkey_refusal_on(Os::Mac, "part.extrude", Chord { ctrl: true, shift: true, ..Chord::from(Key::H) }), None, "Shift+Cmd+H is no menu item");
        // the General letters are the program's own on every system
        assert_eq!(hotkey_refusal_on(Os::Mac, "part.extrude", cmd(Key::S)), Some("hotkeys-reserved"));
    }

    /// EVERY SYSTEM HAS A TABLE OF ITS OWN, and every reason in a table has words.
    #[test]
    fn every_system_has_its_own_table() {
        use qymcad_ui_state::platform_keys::{platform_keys, Os};
        for os in Os::ALL {
            let t = platform_keys(os);
            assert_eq!(t.os, os, "{os:?} reads the table of {:?}", t.os);
            for k in t.rows() {
                assert!(Chord::parse(k.chord).is_some(), "{os:?}: the row {} is no chord and refuses nothing", k.chord);
                assert_ne!(crate::i18n::tr(k.why), k.why, "{os:?}: the reason {} has no words", k.why);
            }
        }
    }

    /// THE MAC'S CONTROL KEY IS RECORDED AS ITSELF: Control+J, written apart from Cmd+J, refused nowhere on a Mac but
    /// the letters its field edits with, and refused on the systems that have no such key.
    #[test]
    fn the_macs_control_key_is_recorded_as_itself() {
        use qymcad_ui_state::{hotkey_refusal_on, platform_keys::Os};
        let control = Chord::of_press(Modifiers { ctrl: true, ..Modifiers::NONE }, Key::J);
        assert_eq!(control.name(), "Control+J");
        let both = Chord::of_press(Modifiers { ctrl: true, mac_cmd: true, command: true, ..Modifiers::NONE }, Key::J);
        assert_eq!(both.name(), "Control+Ctrl+J");
        let cmd = Chord::of_press(Modifiers { mac_cmd: true, command: true, ..Modifiers::NONE }, Key::J);
        assert_eq!(cmd.name(), "Ctrl+J");
        // off a Mac the Ctrl key is the command key, and nothing else
        assert_eq!(Chord::of_press(Modifiers::COMMAND, Key::J).name(), "Ctrl+J");
        assert_eq!(Chord::parse("Control+Ctrl+J"), Some(both));
        assert_eq!(hotkey_refusal_on(Os::Mac, "part.extrude", control), None);
        assert_eq!(hotkey_refusal_on(Os::Mac, "part.extrude", both), None);
        for os in [Os::Linux, Os::Windows] {
            assert_eq!(hotkey_refusal_on(os, "part.extrude", control), Some("hotkeys-mac-only"), "{os:?} has no Control key apart from Ctrl");
        }
    }

    /// ON A MAC THE FIELD'S CONTROL LETTERS AND THE SYSTEM'S COMBINATIONS ARE KEPT, each as egui and macOS answer
    /// them: the erasers with Shift too, the caret keys without it, the menu and system items exactly.
    #[test]
    fn on_a_mac_the_field_and_the_system_keep_their_combinations() {
        use qymcad_ui_state::{hotkey_refusal_on, platform_keys::Os};
        let on_mac = |s: &str| hotkey_refusal_on(Os::Mac, "part.extrude", Chord::parse(s).expect(s));
        for s in ["Control+H", "Control+Shift+W", "Control+Ctrl+U", "Control+A", "Control+N"] {
            assert_eq!(on_mac(s), Some("hotkeys-mac-field-edits"), "{s} edits a field and went to a tool");
        }
        assert_eq!(on_mac("Control+Shift+A"), None, "Shift with Control+A moves no caret");
        for s in ["Ctrl+Shift+3", "Ctrl+Shift+4", "Ctrl+Shift+5", "Ctrl+Shift+Q", "Control+Ctrl+Q", "Control+F3", "Control+F8"] {
            assert_eq!(on_mac(s), Some("hotkeys-os-system"), "{s} belongs to macOS and went to a tool");
        }
        for s in ["Ctrl+3", "Ctrl+Shift+6", "Control+F9", "Control+J", "Control+Shift+F3"] {
            assert_eq!(on_mac(s), None, "{s} is free on a Mac and was refused");
        }
    }

    /// THE FACTORY LAYOUT IS UNIVERSAL: no factory key of a workbench is refused on any system. Its own action is
    /// exempt from refusal, so this is asked of the tables directly.
    #[test]
    fn the_factory_keys_are_free_on_every_system() {
        use qymcad_ui_state::platform_keys::{platform_keys, Os};
        for os in Os::ALL {
            for r in HOTKEYS.iter().filter(|r| rebindable(r.area)) {
                let chord = Chord::parse(r.key).expect(r.key);
                let why = platform_keys(os).refusal(&chord);
                // bare X: the sketch's construction toggle is the General X itself
                assert!(why.is_none() || r.key == "X", "{os:?}: the factory key {} of {} is refused ({why:?})", r.key, r.action);
            }
        }
    }

    /// THE GENERAL CTRL LETTERS ARE THE ONES ITS HANDLERS LISTEN TO. A shortcut added to the frame and forgotten
    /// here would be offered to a tool, and both would fire; one dropped from the frame would stay refused for
    /// nothing. So the list is read against every `Ctrl + letter` the handlers ask for.
    #[test]
    fn the_general_ctrl_letters_are_the_ones_the_handlers_hear() {
        let mut heard: Vec<String> = Vec::new();
        for src in [include_str!("input.rs"), include_str!("../gui.rs"), include_str!("../../../qymcad-ui-state/src/lib.rs")] {
            for line in src.lines().filter(|l| !l.trim_start().starts_with("//") && (l.contains("modifiers.command") || l.contains("cmd &&"))) {
                for part in line.split("key_pressed(egui::Key::").skip(1) {
                    let name: String = part.chars().take_while(|c| c.is_ascii_alphanumeric()).collect();
                    if name.len() == 1 && !heard.contains(&name) {
                        heard.push(name);
                    }
                }
            }
        }
        heard.sort();
        let mut listed: Vec<String> = qymcad_ui_state::platform_keys::GENERAL.iter().filter_map(|k| Chord::parse(k.chord)).filter(|c| c.ctrl).map(|c| c.key.name().to_string()).collect();
        listed.sort();
        assert_eq!(heard, listed, "the Ctrl letters the frame handles and the ones refused to a tool have parted");
    }

    /// ESC LEAVES, BACKSPACE ERASES.
    #[test]
    fn escape_leaves_and_backspace_erases() {
        let app = App::default();
        assert_eq!(capture_outcome(&app.set, "part", "part.extrude", &[], Key::Escape, Modifiers::NONE), Capture::Cancel);
        assert_eq!(capture_outcome(&app.set, "part", "part.extrude", &[], Key::Backspace, Modifiers::NONE), Capture::Bind(String::new()));
    }

    /// A TAKEN KEY IS A QUESTION, NOT A REFUSAL.
    #[test]
    fn a_taken_key_becomes_a_question() {
        let app = App::default();
        let f = [Chord::from(Key::F)];
        assert_eq!(
            capture_outcome(&app.set, "part", "part.extrude", &f, Key::Enter, Modifiers::NONE),
            Capture::Clash(HotkeyClash { action: "part.extrude".into(), chord: "F".into(), holder: "part.fillet" })
        );
    }

    /// SWAP: each gets the other's key, and the record holds both as differences from the factory.
    #[test]
    fn a_swap_gives_each_the_others_key() {
        let mut app = App::default();
        let clash = HotkeyClash { action: "part.extrude".into(), chord: "F".into(), holder: "part.fillet" };
        resolve_hotkey_clash(&mut app.set, &clash, ClashChoice::Swap);
        assert_eq!(qymcad_ui_state::hotkey_action(&app.set, "part", Key::F), Some("part.extrude"));
        assert_eq!(qymcad_ui_state::hotkey_action(&app.set, "part", Key::E), Some("part.fillet"));
        // and swapping back leaves the record clean
        let back = HotkeyClash { action: "part.extrude".into(), chord: "E".into(), holder: "part.fillet" };
        resolve_hotkey_clash(&mut app.set, &back, ClashChoice::Swap);
        assert!(app.set.hotkeys.is_empty(), "swapped back to the factory keys and the record still holds {:?}", app.set.hotkeys);
    }

    /// TAKE: the asking action gets the key, the holder is left with none - and no key at all runs it.
    #[test]
    fn taking_a_key_leaves_the_holder_without_one() {
        let mut app = App::default();
        let clash = HotkeyClash { action: "part.extrude".into(), chord: "F".into(), holder: "part.fillet" };
        resolve_hotkey_clash(&mut app.set, &clash, ClashChoice::Unbind);
        assert_eq!(qymcad_ui_state::hotkey_action(&app.set, "part", Key::F), Some("part.extrude"));
        assert_eq!(qymcad_ui_state::hotkey_key(&app.set, "part.fillet"), "", "the fillet kept a key");
        assert_eq!(qymcad_ui_state::hotkey_action(&app.set, "part", Key::E), None, "the old key of the extrusion still runs something");
    }

    /// A RESET ONTO A KEY ANOTHER COMMAND HOLDS IS A QUESTION, and nothing changes until it is answered.
    ///
    /// Reported behaviour: after a swap, the reset of one row put its factory key back while the other row still held
    /// it - two commands on one key. The reset is the press of the factory key and meets the same clash.
    #[test]
    fn a_reset_onto_a_held_key_becomes_a_question() {
        let mut app = App::default();
        let swap = HotkeyClash { action: "part.extrude".into(), chord: "F".into(), holder: "part.fillet" };
        resolve_hotkey_clash(&mut app.set, &swap, ClashChoice::Swap);
        let back = HotkeyClash { action: "part.extrude".into(), chord: "E".into(), holder: "part.fillet" };
        assert_eq!(reset_hotkey(&mut app.set, "part.extrude"), Some(back.clone()), "the reset onto E, held by the fillet, asked nothing");
        assert_eq!(qymcad_ui_state::hotkey_key(&app.set, "part.extrude"), "F", "the reset changed the key before the question was answered");
        resolve_hotkey_clash(&mut app.set, &back, ClashChoice::Swap);
        assert!(app.set.hotkeys.is_empty(), "the swap answering the reset left the record holding {:?}", app.set.hotkeys);
    }

    /// A RESET ONTO A FREE KEY IS APPLIED AT ONCE: after "Take it" each row goes back by its own reset.
    #[test]
    fn a_reset_onto_a_free_key_is_applied_at_once() {
        let mut app = App::default();
        let take = HotkeyClash { action: "part.extrude".into(), chord: "F".into(), holder: "part.fillet" };
        resolve_hotkey_clash(&mut app.set, &take, ClashChoice::Unbind);
        assert_eq!(reset_hotkey(&mut app.set, "part.fillet"), Some(HotkeyClash { action: "part.fillet".into(), chord: "F".into(), holder: "part.extrude" }), "F is held by the extrusion");
        assert_eq!(reset_hotkey(&mut app.set, "part.extrude"), None, "E is free and the reset asked a question");
        assert_eq!(reset_hotkey(&mut app.set, "part.fillet"), None, "F is free once the extrusion is back on E");
        assert!(app.set.hotkeys.is_empty(), "both reset and the record still holds {:?}", app.set.hotkeys);
    }

    /// NO FACTORY KEY STANDS TWICE IN ONE AREA. The reset of a section and of every key asks nothing, since it puts
    /// the whole area on its factory keys; a factory layout with one key twice would make them two commands on it.
    #[test]
    fn no_factory_key_stands_twice_in_an_area() {
        let mut twice = Vec::new();
        for (i, a) in HOTKEYS.iter().enumerate() {
            let chord = Chord::parse(a.key);
            if let Some(b) = HOTKEYS[..i].iter().find(|b| b.area == a.area && !a.key.is_empty() && Chord::parse(b.key) == chord) {
                twice.push(format!("{} and {} on {} in {}", b.action, a.action, a.key, a.area));
            }
        }
        assert!(twice.is_empty(), "factory keys standing twice: {twice:?}");
    }

    /// EVERY FACTORY KEY OF A WORKBENCH IS A CHORD THE DISPATCHER CAN HEAR. A factory key the parser does not
    /// read would be a tool that never runs.
    #[test]
    fn every_factory_workbench_key_is_a_bindable_chord() {
        for r in HOTKEYS.iter().filter(|r| rebindable(r.area)) {
            let c = Chord::parse(r.key).unwrap_or_else(|| panic!("the factory key {} of {} is not a chord", r.key, r.action));
            assert!(c.bindable_key(), "the factory key {} of {} is one the dispatcher never hears", r.key, r.action);
        }
    }

    /// ON A MAC THE KEYS ARE WRITTEN AS A MAC WRITES THEM: ⌘ for the command key the binding's "Ctrl" stands
    /// for, ⌥ for Alt, in Apple's order - and the general rows, written as text, change with the rest.
    #[test]
    fn a_mac_writes_keys_with_symbols() {
        use qymcad_ui_state::{key_label_in, KeyStyle::MacSymbols};
        assert_eq!(key_label_in("Ctrl+W", MacSymbols), "⌘W");
        assert_eq!(key_label_in("Ctrl+Shift+F5", MacSymbols), "⇧⌘F5", "Shift goes before Command, as on every Mac menu");
        assert_eq!(key_label_in("Alt+U", MacSymbols), "⌥U");
        assert_eq!(key_label_in("Ctrl+Z / Ctrl+Y", MacSymbols), "⌘Z / ⌘Y");
        assert_eq!(key_label_in("E", MacSymbols), "E");
        assert_eq!(key_label_in("", MacSymbols), "", "an action without a key stays without a label");
    }

    /// WITHOUT THE SYMBOL FONT, WORDS - a box where ⌥ should be tells nobody anything.
    #[test]
    fn a_mac_without_the_symbols_writes_words() {
        use qymcad_ui_state::{key_label_in, KeyStyle::MacWords};
        assert_eq!(key_label_in("Ctrl+Shift+W", MacWords), "Shift+Cmd+W");
        assert_eq!(key_label_in("Alt+U", MacWords), "Option+U");
    }

    /// ONLY THE LABEL CHANGES. Off a Mac it is the stored spelling as it is, and the symbols a Mac is given are
    /// the four the program asks Apple Symbols for - nothing that would turn out a box.
    #[test]
    fn off_a_mac_the_label_is_the_stored_spelling() {
        use qymcad_ui_state::{key_label_in, KeyStyle};
        assert_eq!(key_label_in("Ctrl+Shift+W", KeyStyle::Plain), "Ctrl+Shift+W");
        if !cfg!(target_os = "macos") {
            assert_eq!(qymcad_ui_state::key_style(), KeyStyle::Plain, "keys off a Mac are written in Mac style");
        }
        for r in HOTKEYS {
            let shown = key_label_in(r.key, KeyStyle::MacSymbols);
            assert!(shown.chars().all(|c| c.is_ascii() || "⌃⌘⇧⌥".contains(c)), "{} is shown on a Mac with a glyph no loaded font promises: {shown}", r.key);
        }
    }

    /// THE TABLE KEEPS ITS WIDTH WHEN A ROW IS CHANGED. Reported behaviour: pressing a row's X brought up its
    /// reset button and the stripes of the section ran past the table. The reset was a word, wider than the
    /// column of the X, and it appeared on that row alone: the column, and every stripe with it, widened.
    ///
    /// Driven by a click through whole frames; the stripes are measured before the click and in each frame after.
    #[test]
    fn clearing_a_key_does_not_widen_the_table() {
        let prev = qymcad_i18n::language();
        qymcad_i18n::set_language("en");
        let mut app = App::default();
        app.win.open(crate::gui::WinKind::Hotkeys);
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        let screen = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1400.0, 900.0));
        let mut time = 0.0;
        let mut frame = |app: &mut App, events: Vec<egui::Event>| {
            time += 1.0 / 60.0;
            let input = egui::RawInput { screen_rect: Some(screen), time: Some(time), events, ..Default::default() };
            let out = ctx.run_ui(input, |ui| app.hotkeys_window(ui.ctx()));
            let mut shapes = Vec::new();
            out.shapes.into_iter().for_each(|c| flat(c.shape, &mut shapes));
            shapes
        };
        for _ in 0..10 {
            frame(&mut app, Vec::new()); // the window fades in and the grid learns its columns
        }
        let shapes = frame(&mut app, Vec::new());
        let what = super::super::hotkeys::hotkey_what(HOTKEYS.iter().find(|r| r.action == "part.extrude").expect("the extrude row"));
        let row = text_rect(&shapes, &what).expect("the extrude row is drawn");
        let cross = shapes
            .iter()
            .filter_map(|s| match s {
                egui::Shape::Text(t) if t.galley.text() == egui_phosphor::regular::X && (t.pos.y + t.galley.size().y * 0.5 - row.center().y).abs() < 6.0 => Some(t.pos + t.galley.size() * 0.5),
                _ => None,
            })
            .next()
            .expect("the X of the extrude row is drawn");
        let before = stripes(&shapes, row);
        assert!(!before.is_empty(), "no stripe was found - the check would measure nothing");

        frame(&mut app, vec![egui::Event::PointerMoved(cross)]);
        let press = |pressed| egui::Event::PointerButton { pos: cross, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        frame(&mut app, vec![press(true)]);
        let mut after = vec![frame(&mut app, vec![press(false)])];
        for _ in 0..3 {
            after.push(frame(&mut app, Vec::new()));
        }
        qymcad_i18n::set_language(&prev);
        assert_eq!(qymcad_ui_state::hotkey_key(&app.set, "part.extrude"), "", "the click on X left the key in place");
        for (i, shapes) in after.iter().enumerate() {
            assert_eq!(stripes(shapes, row), before, "frame {i} after the click: the stripes changed width");
        }
        assert!(
            after.last().is_some_and(|s| s.iter().any(|s| matches!(s, egui::Shape::Text(t) if t.galley.text() == egui_phosphor::regular::ARROW_COUNTER_CLOCKWISE))),
            "the way back to the factory key is not drawn as its icon"
        );

        fn flat(s: egui::Shape, out: &mut Vec<egui::Shape>) {
            match s {
                egui::Shape::Vec(v) => v.into_iter().for_each(|s| flat(s, out)),
                s => out.push(s),
            }
        }
        fn text_rect(shapes: &[egui::Shape], text: &str) -> Option<egui::Rect> {
            shapes.iter().find_map(|s| match s {
                egui::Shape::Text(t) if t.galley.text() == text => Some(egui::Rect::from_min_size(t.pos, t.galley.size())),
                _ => None,
            })
        }
        /// The left and right edges, to a tenth of a point, of the filled bands under the description and the rows next to it.
        fn stripes(shapes: &[egui::Shape], row: egui::Rect) -> Vec<(i32, i32)> {
            let mut v: Vec<(i32, i32)> = shapes
                .iter()
                .filter_map(|s| match s {
                    egui::Shape::Rect(r) if r.rect.min.x < row.min.x && r.rect.max.x > row.max.x && (r.rect.center().y - row.center().y).abs() < 3.0 * row.height() => {
                        Some(((r.rect.min.x * 10.0).round() as i32, (r.rect.max.x * 10.0).round() as i32))
                    }
                    _ => None,
                })
                .collect();
            v.sort();
            v.dedup();
            v
        }
    }

    /// A MODIFIER ON ITS OWN IS NOT A PRESS. egui reports Cmd, Ctrl, Shift and Alt as keys when they go down; judged
    /// as a whole chord, the Cmd held before the letter was refused.
    #[test]
    fn a_modifier_alone_is_not_a_press() {
        let app = App::default();
        for (key, mods) in [
            (Key::SuperLeft, Modifiers::MAC_CMD),
            (Key::SuperRight, Modifiers::MAC_CMD),
            (Key::ControlLeft, Modifiers::COMMAND),
            (Key::ControlRight, Modifiers::COMMAND),
            (Key::ShiftLeft, Modifiers::SHIFT),
            (Key::ShiftRight, Modifiers::SHIFT),
            (Key::AltLeft, Modifiers::ALT),
            (Key::AltRight, Modifiers::ALT),
        ] {
            assert_eq!(capture_outcome(&app.set, "part", "part.extrude", &[], key, mods), Capture::Pending, "{key:?} alone was judged as a chord");
        }
    }

    /// THE WINDOW DRIVEN THROUGH WHOLE FRAMES, in English, on a screen of a fixed size.
    struct Frames {
        app: App,
        ctx: egui::Context,
        time: f64,
        lang: String,
    }

    impl Frames {
        fn open() -> Self {
            Self::open_in("en")
        }

        fn open_in(code: &str) -> Self {
            let lang = qymcad_i18n::language();
            qymcad_i18n::set_language(code);
            let mut app = App::default();
            app.win.open(crate::gui::WinKind::Hotkeys);
            let ctx = egui::Context::default();
            super::super::install_fonts(&ctx);
            let mut w = Frames { app, ctx, time: 0.0, lang };
            for _ in 0..10 {
                w.frame(Vec::new()); // the window fades in and the grid learns its columns
            }
            w
        }

        fn frame(&mut self, events: Vec<egui::Event>) -> Vec<egui::Shape> {
            self.time += 1.0 / 60.0;
            let screen = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1400.0, 900.0));
            let input = egui::RawInput { screen_rect: Some(screen), time: Some(self.time), events, ..Default::default() };
            let app = &mut self.app;
            let out = self.ctx.run_ui(input, |ui| app.hotkeys_window(ui.ctx()));
            let mut shapes = Vec::new();
            out.shapes.into_iter().for_each(|c| flat_shape(c.shape, &mut shapes));
            shapes
        }

        fn click(&mut self, at: egui::Pos2) {
            self.frame(vec![egui::Event::PointerMoved(at)]);
            let press = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
            self.frame(vec![press(true)]);
            self.frame(vec![press(false)]);
        }

        /// A click on the empty filter field and `text` typed into it.
        fn type_filter(&mut self, text: &str) {
            let shapes = self.frame(Vec::new());
            let hint = text_rect(&shapes, &crate::i18n::tr("hotkeys-filter-hint")).expect("the empty filter shows its hint");
            self.click(hint.center());
            self.frame(vec![egui::Event::Text(text.into())]);
        }

        /// A frame in which the key goes down.
        fn key_down(&mut self, key: Key, modifiers: Modifiers) -> Vec<egui::Shape> {
            self.frame(vec![egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers }])
        }

        /// A frame in which the key comes up.
        fn key_up(&mut self, key: Key) -> Vec<egui::Shape> {
            self.frame(vec![egui::Event::Key { key, physical_key: None, pressed: false, repeat: false, modifiers: Modifiers::NONE }])
        }

        /// A click on the key button of the extrude row, which shows `key`, so the window waits for its key; the row's
        /// description.
        fn wait_for_extrude_key(&mut self, key: &str) -> String {
            let what = super::super::hotkeys::hotkey_what(HOTKEYS.iter().find(|r| r.action == "part.extrude").expect("the extrude row"));
            let shapes = self.frame(Vec::new());
            let row = text_rect(&shapes, &what).expect("the extrude row is drawn");
            let key = shapes
                .iter()
                .find_map(|s| match s {
                    egui::Shape::Text(t) if t.galley.text() == key && t.pos.x < row.min.x && (t.pos.y + t.galley.size().y * 0.5 - row.center().y).abs() < 6.0 => Some(t.pos + t.galley.size() * 0.5),
                    _ => None,
                })
                .expect("the key button of the extrude row is drawn");
            self.click(key);
            what
        }
    }

    impl Drop for Frames {
        fn drop(&mut self) {
            qymcad_i18n::set_language(&self.lang);
        }
    }

    fn flat_shape(s: egui::Shape, out: &mut Vec<egui::Shape>) {
        match s {
            egui::Shape::Vec(v) => v.into_iter().for_each(|s| flat_shape(s, out)),
            s => out.push(s),
        }
    }

    /// The colour `text` is drawn in: the one its words ask for, else the painter's own.
    fn text_color(shapes: &[egui::Shape], text: &str) -> Option<egui::Color32> {
        shapes.iter().find_map(|s| match s {
            egui::Shape::Text(t) if t.galley.text() == text => {
                let asked = t.galley.job.sections.first().map(|s| s.format.color).filter(|c| *c != egui::Color32::PLACEHOLDER);
                Some(t.override_text_color.or(asked).unwrap_or(t.fallback_color))
            }
            _ => None,
        })
    }

    /// The text drawn whose words hold `text`.
    fn text_with<'a>(shapes: &'a [egui::Shape], text: &str) -> Option<&'a egui::epaint::TextShape> {
        shapes.iter().find_map(|s| match s {
            egui::Shape::Text(t) if t.galley.text().contains(text) => Some(t),
            _ => None,
        })
    }

    fn text_rect(shapes: &[egui::Shape], text: &str) -> Option<egui::Rect> {
        shapes.iter().find_map(|s| match s {
            egui::Shape::Text(t) if t.galley.text() == text => Some(egui::Rect::from_min_size(t.pos, t.galley.size())),
            _ => None,
        })
    }

    /// WHY A PRESS WAS REFUSED TAKES THE PLACE OF THE WAITING LINE, behind a stop sign in words of the row's colour, and holding a modifier
    /// refuses nothing yet.
    ///
    /// Reported behaviour: the refusal stood on a line of its own under the waiting line, and before that in a column
    /// of its own beside it; it also showed as soon as Cmd went down, before the letter. egui reports the modifier
    /// key itself as a press. Driven by a click on the key and key presses through whole frames.
    #[test]
    fn the_refusal_takes_the_place_of_the_waiting_line() {
        let mut w = Frames::open();
        let what = w.wait_for_extrude_key("E");
        let waiting = crate::i18n::tr("hotkeys-waiting");
        let refused = crate::i18n::tr("hotkeys-reserved");
        let rows: Vec<_> = HOTKEYS.iter().filter(|r| r.area == "part").collect();
        let at = rows.iter().position(|r| r.action == "part.extrude").expect("the extrude row");
        let next = super::super::hotkeys::hotkey_what(rows.get(at + 1).expect("a row under the extrusion"));
        let shapes = w.frame(Vec::new());
        let line = text_rect(&shapes, &waiting).expect("the click on the key did not make the window wait");
        let below = text_rect(&shapes, &next).expect("the row under the extrusion is drawn").min.y;
        for (key, mods) in [(Key::SuperLeft, Modifiers::MAC_CMD), (Key::ControlLeft, Modifiers::COMMAND)] {
            w.key_down(key, mods);
            let shapes = w.frame(Vec::new());
            assert!(text_rect(&shapes, &waiting).is_some(), "{key:?} going down ended the waiting");
            assert!(text_rect(&shapes, &refused).is_none(), "{key:?} held alone already says the key is refused");
            w.key_up(key);
        }
        w.key_down(Key::Z, Modifiers::COMMAND); // undo: the system's
        w.key_up(Key::Z);
        let shapes = w.frame(Vec::new());
        // THE BLOCK UNDER THE ROW KEEPS ITS HEIGHT: the row below stays where it stood while the window waited
        let y = text_rect(&shapes, &next).expect("the row under the extrusion is drawn").min.y;
        assert!((y - below).abs() < 0.5, "the refusal moved the row under the extrusion from y {below} to {y}");
        let why = text_with(&shapes, &refused).expect("the refusal of Ctrl+Z is not drawn");
        assert!(text_rect(&shapes, &waiting).is_none(), "the waiting line still stands beside the refusal");
        assert!((why.pos - line.min).length() < 0.5, "the refusal starts at {:?}, the waiting line stood at {:?}", why.pos, line.min);
        // THE STOP SIGN OPENS THE LINE: a refusal is not the clash, which asks a question under a warning triangle
        let stop = egui_phosphor::regular::WARNING_OCTAGON;
        assert!(why.galley.text().starts_with(stop), "the refusal is drawn without its stop sign: {:?}", why.galley.text());
        // THE SIGN SAYS STOP, the words are read in the colour of the row's own: red words beside a red sign shout
        let plain = text_color(&shapes, &what).expect("the extrude row is drawn");
        let color_of = |part: &str| why.galley.job.sections.iter().find(|s| why.galley.text().get(s.byte_range.start.0..s.byte_range.end.0) == Some(part)).map(|s| s.format.color);
        assert_eq!(color_of(&refused), Some(plain), "the words of the refusal are coloured, the row's description is not");
        assert!(color_of(stop).is_some_and(|c| c != plain), "the stop sign is drawn in the colour of plain words");
        assert!(text_rect(&shapes, &crate::i18n::tr("hotkeys-press")).is_some(), "the refused press ended the waiting");
    }

    /// THE ROWS GO BACK TO THEIR PLACES ONCE A KEY IS ASSIGNED.
    ///
    /// Reported behaviour: after a key was changed, empty bands stood between the rows below it. The lines under
    /// the row were a grid row of their own; when they went, every row below moved up one index, and the grid laid
    /// each out in the height the previous frame had at that index.
    #[test]
    fn the_rows_below_stay_in_place_after_a_key_is_assigned() {
        let mut w = Frames::open();
        let rows: Vec<_> = HOTKEYS.iter().filter(|r| r.area == "part").collect();
        let at = rows.iter().position(|r| r.action == "part.extrude").expect("the extrude row");
        let next = super::super::hotkeys::hotkey_what(rows.get(at + 1).expect("a row under the extrusion"));
        let before = text_rect(&w.frame(Vec::new()), &next).expect("the row under the extrusion is drawn").min.y;
        w.wait_for_extrude_key("E");
        w.key_down(Key::W, Modifiers::NONE);
        w.key_up(Key::W);
        w.key_down(Key::Enter, Modifiers::NONE);
        assert_eq!(qymcad_ui_state::hotkey_key(&w.app.set, "part.extrude"), "W", "W and Enter did not assign W");
        let mut after = vec![w.key_up(Key::Enter)];
        for _ in 0..3 {
            after.push(w.frame(Vec::new()));
        }
        for (i, shapes) in after.iter().enumerate() {
            let y = text_rect(shapes, &next).expect("the row under the extrusion is drawn").min.y;
            assert!((y - before).abs() < 0.5, "frame {i} after the key was assigned: the row under it stands at {y}, it stood at {before}");
        }
    }

    /// THE CENTRE OF A ROW ICON drawn within the height of a row: from the top of its description to the bottom of the
    /// last line under it. The grid centres every cell in the row's height, so while the window waits for a key the
    /// icons stand halfway down the waiting line, not on the line of the description.
    fn icon_on_row(shapes: &[egui::Shape], glyph: &str, cell: egui::Rect) -> Option<egui::Pos2> {
        shapes.iter().find_map(|s| match s {
            egui::Shape::Text(t) if t.galley.text() == glyph && t.pos.x > cell.min.x && cell.y_range().contains(t.pos.y + t.galley.size().y * 0.5) => Some(t.pos + t.galley.size() * 0.5),
            _ => None,
        })
    }

    /// X AND RESET END THE WAITING. Reported behaviour: with the window waiting for a key, a click on X cleared the
    /// key and the window went on waiting; a click on reset put the factory key back, the X came back, and the window
    /// still waited - the next press would overwrite what the click had set. Driven by clicks through whole frames.
    #[test]
    fn a_click_on_x_or_reset_ends_the_waiting() {
        let mut w = Frames::open();
        let what = w.wait_for_extrude_key("E");
        let waiting = crate::i18n::tr("hotkeys-waiting");
        let shapes = w.frame(Vec::new());
        let line = text_rect(&shapes, &waiting).expect("the click on the key did not make the window wait");
        let row = text_rect(&shapes, &what).expect("the extrude row is drawn");
        let x = icon_on_row(&shapes, egui_phosphor::regular::X, row.union(line)).expect("the X of the extrude row is drawn");
        w.click(x);
        let shapes = w.frame(Vec::new());
        assert_eq!(qymcad_ui_state::hotkey_key(&w.app.set, "part.extrude"), "", "the click on X left the key in place");
        assert!(text_rect(&shapes, &waiting).is_none(), "the window still waits for a key after X was clicked");

        let unbound = crate::i18n::tr("hotkeys-unbound");
        w.wait_for_extrude_key(&unbound);
        let shapes = w.frame(Vec::new());
        let line = text_rect(&shapes, &waiting).expect("the click on the unbound key did not make the window wait");
        let row = text_rect(&shapes, &what).expect("the extrude row is drawn");
        let reset = icon_on_row(&shapes, egui_phosphor::regular::ARROW_COUNTER_CLOCKWISE, row.union(line)).expect("the reset of the extrude row is drawn");
        w.click(reset);
        let shapes = w.frame(Vec::new());
        assert_eq!(qymcad_ui_state::hotkey_key(&w.app.set, "part.extrude"), "E", "the click on reset did not put the factory key back");
        assert!(text_rect(&shapes, &waiting).is_none(), "the window still waits for a key after reset was clicked");
    }

    /// THE RESET OF A SWAPPED ROW ASKS UNDER THAT ROW, and Swap answers it with both rows back on their factory keys.
    /// Driven by a click on the extrude key, a press of F, a click on Swap, a click on the extrude row's reset and a
    /// click on Swap again, through whole frames.
    #[test]
    fn the_reset_of_a_swapped_row_asks_and_swaps_back() {
        let mut w = Frames::open();
        let what = w.wait_for_extrude_key("E");
        w.key_down(Key::F, Modifiers::NONE);
        w.key_up(Key::F);
        w.key_down(Key::Enter, Modifiers::NONE);
        let swap = text_rect(&w.frame(Vec::new()), &crate::i18n::tr("hotkeys-swap")).expect("the clash of F is not asked");
        w.click(swap.center());
        assert_eq!(qymcad_ui_state::hotkey_key(&w.app.set, "part.fillet"), "E", "Swap did not give the fillet E");
        // the grid lays a row out in the height it had a frame before, with the question still under it
        for _ in 0..3 {
            w.frame(Vec::new());
        }
        let shapes = w.frame(Vec::new());
        let row = text_rect(&shapes, &what).expect("the extrude row is drawn");
        let reset = icon_on_row(&shapes, egui_phosphor::regular::ARROW_COUNTER_CLOCKWISE, row).expect("the reset of the extrude row is drawn");
        w.click(reset);
        let holder = super::super::hotkeys::hotkey_what(HOTKEYS.iter().find(|r| r.action == "part.fillet").expect("the fillet row"));
        let shapes = w.frame(Vec::new());
        let question = text_rect(&shapes, &crate::i18n::tr2("hotkeys-taken", "key", &qymcad_ui_state::key_label("E"), "what", &holder)).expect("the reset onto E, held by the fillet, asked nothing");
        let row = text_rect(&shapes, &what).expect("the extrude row is drawn");
        assert!(question.min.y >= row.max.y - 0.5, "the question {question:?} does not stand under the extrude row {row:?}");
        assert_eq!(qymcad_ui_state::hotkey_key(&w.app.set, "part.extrude"), "F", "the reset changed the key before the question was answered");
        let swap = text_rect(&shapes, &crate::i18n::tr("hotkeys-swap")).expect("Swap is not offered for the reset");
        w.click(swap.center());
        assert_eq!(qymcad_ui_state::hotkey_key(&w.app.set, "part.extrude"), "E", "Swap did not put the extrusion back on E");
        assert_eq!(qymcad_ui_state::hotkey_key(&w.app.set, "part.fillet"), "F", "Swap did not put the fillet back on F");
    }

    /// A KEY OF SEVERAL PRESSES IS RECORDED PRESS BY PRESS, shown on the key while it is built, and saved by Enter.
    /// Driven by a click on the extrude key and presses through whole frames.
    #[test]
    fn a_sequence_is_recorded_and_saved_by_enter() {
        let mut w = Frames::open();
        w.wait_for_extrude_key("E");
        w.key_down(Key::G, Modifiers::NONE);
        w.key_up(Key::G);
        w.key_down(Key::G, Modifiers::NONE);
        // the press is heard after the window is drawn, so the frame after it shows it
        let shapes = w.key_up(Key::G);
        let built = crate::i18n::tr1("hotkeys-seq-waiting", "keys", &qymcad_ui_state::key_label("G, G"));
        assert!(text_rect(&shapes, &built).is_some(), "the key being recorded is not shown as it grows");
        assert_eq!(qymcad_ui_state::hotkey_key(&w.app.set, "part.extrude"), "E", "the key changed before it was finished");
        w.key_down(Key::Enter, Modifiers::NONE);
        assert_eq!(qymcad_ui_state::hotkey_key(&w.app.set, "part.extrude"), "G, G", "G, G and Enter did not record G, G");
        assert_eq!(w.app.hotkeys.action, None, "the window still waits after Enter");
    }

    /// BACKSPACE TAKES BACK THE LAST PRESS, and with nothing pressed leaves the action without a key.
    #[test]
    fn backspace_takes_back_the_last_press() {
        let mut w = Frames::open();
        w.wait_for_extrude_key("E");
        for key in [Key::G, Key::C, Key::Backspace, Key::V, Key::Enter] {
            w.key_down(key, Modifiers::NONE);
            w.key_up(key);
        }
        assert_eq!(qymcad_ui_state::hotkey_key(&w.app.set, "part.extrude"), "G, V", "Backspace did not take back the C");
        w.wait_for_extrude_key(&qymcad_ui_state::key_label("G, V"));
        w.key_down(Key::Backspace, Modifiers::NONE);
        assert_eq!(qymcad_ui_state::hotkey_key(&w.app.set, "part.extrude"), "", "Backspace with nothing pressed did not clear the key");
    }

    /// A PAUSE SAVES, AND SO DOES THE LAST PRESS A KEY CAN HOLD: no Enter is needed for a key of one press.
    #[test]
    fn a_pause_or_the_longest_key_saves() {
        let mut w = Frames::open();
        w.wait_for_extrude_key("E");
        w.key_down(Key::W, Modifiers::NONE);
        w.key_up(Key::W);
        for _ in 0..30 {
            w.frame(Vec::new());
        }
        assert_eq!(qymcad_ui_state::hotkey_key(&w.app.set, "part.extrude"), "E", "half a second after the press the key was taken as finished");
        for _ in 0..40 {
            w.frame(Vec::new());
        }
        assert_eq!(qymcad_ui_state::hotkey_key(&w.app.set, "part.extrude"), "W", "a second after the press W was not saved");

        w.wait_for_extrude_key("W");
        for key in [Key::G, Key::G, Key::G, Key::G] {
            w.key_down(key, Modifiers::NONE);
            w.key_up(key);
        }
        assert_eq!(qymcad_ui_state::hotkey_key(&w.app.set, "part.extrude"), "G, G, G, G", "the fourth press did not finish the key");
        assert_eq!(w.app.hotkeys.action, None);
    }

    /// A KEY THAT STARTS ANOTHER IS NO CLASH - `G` beside `G, G` - while the same sequence twice is.
    #[test]
    fn only_the_same_sequence_clashes() {
        let mut app = App::default();
        qymcad_ui_state::set_hotkey(&mut app.set, "part.fillet", "G");
        let g = Chord::from(Key::G);
        assert_eq!(finish_recording(&app.set, "part", "part.extrude", &[g, g]), Capture::Bind("G, G".into()));
        qymcad_ui_state::set_hotkey(&mut app.set, "part.extrude", "G, G");
        assert_eq!(finish_recording(&app.set, "part", "part.hole", &[g, g]), Capture::Clash(HotkeyClash { action: "part.hole".into(), chord: "G, G".into(), holder: "part.extrude" }));
        assert_eq!(qymcad_ui_state::hotkey_taken_by(&app.set, "part", "G,G", "part.hole"), Some("part.extrude"), "the same sequence written without a space is another key");
    }

    /// A REFUSED PRESS IS SAID AT ONCE and the presses before it are kept: Ctrl+S cannot start a key, and after G it
    /// can be its second press.
    #[test]
    fn a_refused_press_keeps_what_was_recorded() {
        let app = App::default();
        let g = Chord::from(Key::G);
        assert_eq!(capture_outcome(&app.set, "part", "part.extrude", &[], Key::S, Modifiers::COMMAND), Capture::Refused("hotkeys-reserved"));
        assert_eq!(capture_outcome(&app.set, "part", "part.extrude", &[g], Key::S, Modifiers::COMMAND), Capture::Record(vec![g, Chord::of_press(Modifiers::COMMAND, Key::S)]));
        assert_eq!(capture_outcome(&app.set, "part", "part.extrude", &[g], Key::Space, Modifiers::NONE), Capture::Refused("hotkeys-reserved"));
    }

    /// THE TABLE KEEPS ITS WIDTH WHILE A LONG KEY IS RECORDED: the descriptions do not move aside under the person's
    /// eyes.
    #[test]
    fn the_table_keeps_its_width_while_a_key_is_recorded() {
        let mut w = Frames::open();
        let what = w.wait_for_extrude_key("E");
        let before = text_rect(&w.frame(Vec::new()), &what).expect("the extrude row is drawn");
        let ctrl_shift = Modifiers { shift: true, ..Modifiers::COMMAND };
        for key in [Key::F5, Key::F6, Key::F7] {
            w.key_down(key, ctrl_shift);
            w.key_up(key);
        }
        assert_eq!(w.app.hotkeys.recorded.pressed.len(), 3, "the presses were not recorded");
        for i in 0..3 {
            let now = text_rect(&w.frame(Vec::new()), &what).expect("the extrude row is drawn");
            assert!((now.min.x - before.min.x).abs() < 0.5, "frame {i}: the description moved from x {} to {} while the key was recorded", before.min.x, now.min.x);
        }
    }

    /// THE CHOICES OF A CLASH STAND ON A LINE OF THEIR OWN, under the question. Driven by a click on the extrude key
    /// and a press of F, the fillet's key, through whole frames.
    #[test]
    fn the_clash_choices_stand_on_their_own_line() {
        let mut w = Frames::open();
        let what = w.wait_for_extrude_key("E");
        w.key_down(Key::F, Modifiers::NONE);
        w.key_up(Key::F);
        w.key_down(Key::Enter, Modifiers::NONE);
        let shapes = w.frame(Vec::new());
        let holder = super::super::hotkeys::hotkey_what(HOTKEYS.iter().find(|r| r.action == "part.fillet").expect("the fillet row"));
        let question = text_rect(&shapes, &crate::i18n::tr2("hotkeys-taken", "key", &qymcad_ui_state::key_label("F"), "what", &holder)).expect("the clash question is not drawn");
        let row = text_rect(&shapes, &what).expect("the extrude row is drawn");
        let choices: Vec<egui::Rect> =
            ["hotkeys-swap", "hotkeys-take", "hotkeys-cancel"].iter().map(|k| text_rect(&shapes, &crate::i18n::tr(k)).unwrap_or_else(|| panic!("the choice {k} is not drawn"))).collect();
        for (k, c) in ["hotkeys-swap", "hotkeys-take", "hotkeys-cancel"].iter().zip(&choices) {
            assert!(c.min.y >= question.max.y - 0.5, "the choice {k} at {c:?} stands beside the question {question:?}, not under it");
            assert!((c.center().y - choices[0].center().y).abs() < 1.0, "the choice {k} at {c:?} is not on the line of Swap {:?}", choices[0]);
        }
        assert!((choices[0].min.x - row.min.x).abs() < 12.0, "Swap starts at x {}, the description at {}: the line does not start under it", choices[0].min.x, row.min.x);
    }

    /// THE FILTER IS CLEARED BY THE X INSIDE ITS FIELD, which shows only while there is something to clear. Driven
    /// by a click on the field, typed text and a click on the X through whole frames.
    #[test]
    fn the_x_inside_the_filter_clears_it() {
        let mut w = Frames::open();
        let what = super::super::hotkeys::hotkey_what(HOTKEYS.iter().find(|r| r.action == "part.extrude").expect("the extrude row"));
        let shapes = w.frame(Vec::new());
        let hint = text_rect(&shapes, &crate::i18n::tr("hotkeys-filter-hint")).expect("the empty filter shows its hint");
        let on_field = |shapes: &[egui::Shape]| {
            shapes.iter().find_map(|s| match s {
                egui::Shape::Text(t) if t.galley.text() == egui_phosphor::regular::X && t.pos.x > hint.max.x && (t.pos.y + t.galley.size().y * 0.5 - hint.center().y).abs() < 6.0 => {
                    Some(t.pos + t.galley.size() * 0.5)
                }
                _ => None,
            })
        };
        assert!(on_field(&shapes).is_none(), "the empty filter already shows a clear icon");
        w.type_filter("no such command");
        let shapes = w.frame(Vec::new());
        assert!(text_rect(&shapes, &what).is_none(), "the filter did not take the typed text: the extrude row is still drawn");
        let x = on_field(&shapes).expect("the filter holds text and shows no clear icon inside its field");
        w.click(x);
        let shapes = w.frame(Vec::new());
        assert!(text_rect(&shapes, &what).is_some(), "the click on the clear icon did not bring the rows back");
        assert!(on_field(&shapes).is_none(), "the cleared filter still shows its clear icon");
    }

    /// THE FILTER FINDS A ROW BY ITS ENGLISH DESCRIPTION, one way only.
    #[test]
    fn the_filter_finds_a_row_by_its_english_description() {
        let row = |action: &str| HOTKEYS.iter().find(|r| r.action == action).expect("the row");
        let russian: String = crate::i18n::tr_in("ru", row("part.mirror").what).expect("the Russian description of the Mirror row").to_lowercase().chars().take(4).collect();
        let mut wrong = Vec::new();
        for (ui, typed, found) in [("ru", "mirror", true), ("ru", "MIRR", true), ("en", russian.as_str(), false)] {
            let mut w = Frames::open_in(ui);
            let mirror = super::super::hotkeys::hotkey_what(row("part.mirror"));
            let hole = super::super::hotkeys::hotkey_what(row("part.hole"));
            w.type_filter(typed);
            let shapes = w.frame(Vec::new());
            if text_rect(&shapes, &mirror).is_some() != found {
                wrong.push(format!("interface `{ui}`, `{typed}` typed: the Mirror row `{mirror}` drawn {}, expected {found}", !found));
            }
            if text_rect(&shapes, &hole).is_some() {
                wrong.push(format!("interface `{ui}`, `{typed}` typed: the Hole row `{hole}` is still drawn"));
            }
        }
        assert!(wrong.is_empty(), "{wrong:#?}");
    }
}
