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
        assert_eq!(app.hotkey_action("part", egui::Key::E), Some("part.extrude"), "the factory E in a Part must extrude");
        assert_eq!(app.hotkey_action("sketch", egui::Key::L), Some("sketch.line"), "the factory L in a Sketch must draw a line");
        assert_eq!(app.hotkey_action("part", egui::Key::Z), None, "a free key is not obliged to mean anything");
    }

    /// THE POINT: after a rebind THE NEW key works and the old one stops.
    #[test]
    fn a_reassigned_key_takes_over_and_the_old_one_stops() {
        let mut app = App::default();
        app.set.hotkeys.insert("part.extrude".into(), "W".into());
        assert_eq!(app.hotkey_action("part", egui::Key::W), Some("part.extrude"), "the new key does not work — the rebinding is useless");
        assert_eq!(app.hotkey_action("part", egui::Key::E), None, "the old key still extrudes — now there are two of them");
    }

    /// A REBINDING LIVES IN THE SETTINGS, and so survives a restart.
    #[test]
    fn a_reassignment_survives_a_restart() {
        let mut app = App::default();
        app.set.hotkeys.insert("sketch.circle".into(), "Z".into());
        let ron = ron::ser::to_string(&app.set).expect("the settings serialise");
        let back: super::super::Settings = ron::from_str(&ron).expect("and read back");
        let mut restarted = App::default();
        restarted.set = back;
        assert_eq!(restarted.hotkey_action("sketch", egui::Key::Z), Some("sketch.circle"), "after a restart the key went back to the factory one — the edit is lost");
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
        assert_eq!(app.hotkey_taken_by("part", "F", "part.extrude"), Some("part.fillet"), "the key F being taken in a Part went unnoticed");
        assert_eq!(app.hotkey_taken_by("part", "Z", "part.extrude"), None, "a free key was called taken");
        // one letter in DIFFERENT workbenches is not a conflict: F in a Sketch and F in a Part live apart
        assert_eq!(app.hotkey_taken_by("sketch", "B", "sketch.line"), None, "a key of another workbench was counted as taken");
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
}
