//! THE KEYS: chords, their clashes and their table.

use super::*;

/// WHILE A KEY IS BEING REASSIGNED: what is being waited for, and why the last press was refused.
///
/// Two fields on the application that only ever moved together and only ever meant something while one
/// window was open.
#[derive(Default)]
pub struct HotkeyCapture {
    /// The action a key is being assigned to right now - the window is waiting for a press.
    pub action: Option<String>,
    /// Why the last press was refused. Shown in that same window; empty means nothing was refused.
    pub note: String,
    /// A PRESS THAT HIT A TAKEN KEY, held until the person decides: swap the two, take the key away from
    /// the other action, or keep things as they were. Refusing outright sent people hunting for the
    /// other row to free it first - two edits for one intention.
    pub clash: Option<HotkeyClash>,
    /// The filter typed above the table: a word of the description or a key.
    pub filter: String,
}

/// A key asked for by one action while another already holds it in the same area.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HotkeyClash {
    /// The action being assigned.
    pub action: String,
    /// The chord pressed for it, in its stored spelling.
    pub chord: String,
    /// The action that holds that chord now.
    pub holder: &'static str,
}

/// A KEY WITH ITS MODIFIERS: `W`, `Shift+W`, `Ctrl+J`, `Ctrl+Shift+F5`, `Control+J`.
///
/// Ctrl stands for the platform's command key (Cmd on a Mac), as everywhere else in the program: the factory
/// layout and a profile mean the same keys on every system. Control is the Mac's own Control key, a modifier of
/// its own there; what each system refuses is in `platform_keys`. ALT IS NOT A MODIFIER OF A BINDING: it is the
/// way to reach a bare binding from inside a text field (Alt+U instead of U), and a chord that needed Alt would be
/// unreachable exactly there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Chord {
    /// The Mac's Control key (never set elsewhere: off a Mac the Ctrl key is `ctrl`).
    pub control: bool,
    pub ctrl: bool,
    pub shift: bool,
    pub key: egui::Key,
}

impl From<egui::Key> for Chord {
    fn from(key: egui::Key) -> Self {
        Chord { control: false, ctrl: false, shift: false, key }
    }
}

impl Chord {
    /// THE CHORD OF A PRESS, Alt aside. egui reports the command key as `command` everywhere and the physical
    /// Ctrl key as `ctrl`; off a Mac they are one key, on a Mac `command` is Cmd (`mac_cmd`) and `ctrl` is Control.
    /// So Control is `ctrl` that is not the command key, or `ctrl` held beside Cmd.
    pub fn of_press(mods: egui::Modifiers, key: egui::Key) -> Chord {
        Chord { control: mods.ctrl && (mods.mac_cmd || !mods.command), ctrl: mods.command, shift: mods.shift, key }
    }

    /// Reads the stored spelling. `None` for anything that is not a chord: an empty record (an unbound
    /// action), a description like `Ctrl+Z / Ctrl+Y`, a key egui does not know.
    pub fn parse(s: &str) -> Option<Chord> {
        let mut c = Chord::from(egui::Key::Escape);
        let mut parts = s.split('+').map(str::trim).peekable();
        while let Some(p) = parts.next() {
            if parts.peek().is_none() {
                c.key = egui::Key::from_name(p)?;
                return Some(c);
            }
            match p {
                "Control" => c.control = true,
                "Ctrl" => c.ctrl = true,
                "Shift" => c.shift = true,
                _ => return None,
            }
        }
        None
    }

    /// The stored and the shown spelling at once - one form, so a record reads the way the window shows it.
    pub fn name(&self) -> String {
        let mut s = String::new();
        if self.control {
            s.push_str("Control+");
        }
        if self.ctrl {
            s.push_str("Ctrl+");
        }
        if self.shift {
            s.push_str("Shift+");
        }
        s.push_str(self.key.name());
        s
    }

    /// WHETHER THE KEY ITSELF CAN CARRY A BINDING: letters, digits and F3-F12.
    ///
    /// Everything else already means something that must not move. Space opens the search, F1 is help, F2
    /// renames, the arrows and Home/End walk a caret through a field, Tab walks the focus, Enter, Esc and
    /// Delete are the ladder of every dialogue. Bound to a tool, any of them would quietly stop doing the
    /// thing every program does with it.
    pub fn bindable_key(&self) -> bool {
        use egui::Key as K;
        let n = self.key.name();
        let letter_or_digit = n.len() == 1 && n.chars().all(|c| c.is_ascii_alphanumeric());
        letter_or_digit || matches!(self.key, K::F3 | K::F4 | K::F5 | K::F6 | K::F7 | K::F8 | K::F9 | K::F10 | K::F11 | K::F12)
    }
}

/// WHY A CHORD CANNOT GO TO THIS ACTION on this system - a catalogue key, or `None` when it can.
pub fn hotkey_refusal(action: &str, chord: &Chord) -> Option<&'static str> {
    hotkey_refusal_on(platform_keys::Os::current(), action, chord)
}

/// The same for a given system, so the rules of each can be checked on any of them.
///
/// An action's own factory key is never refused: bare X is the factory key of the sketch's construction
/// toggle, and refusing it would make "put it back as it was" impossible by hand.
pub fn hotkey_refusal_on(os: platform_keys::Os, action: &str, chord: &Chord) -> Option<&'static str> {
    if !chord.bindable_key() {
        return Some("hotkeys-reserved");
    }
    let factory = HOTKEYS.iter().find(|r| r.action == action).is_some_and(|r| Chord::parse(r.key) == Some(*chord));
    // everything else is the system's table (see `platform_keys`)
    platform_keys::platform_keys(os).refusal(chord).filter(|_| !factory)
}

/// WHETHER THE KEY in this area is taken by somebody else - the name of the neighbouring action.
///
/// Two commands on one key is not "the last one wins" but a silently lost tool: the habitual key is
/// pressed, something else arrives, and it is not clear what broke. So rebinding asks here first.
/// Compared as CHORDS, not as text: `Shift+Ctrl+W` written by hand and `Ctrl+Shift+W` are one key.
pub fn hotkey_taken_by(set: &Settings, area: &str, key: &str, except: &str) -> Option<&'static str> {
    let want = Chord::parse(key)?;
    HOTKEYS.iter().filter(|r| r.area == area && r.action != except).find(|r| Chord::parse(&hotkey_key(set, r.action)) == Some(want)).map(|r| r.action)
}

/// The key bound to an action now: the person's record, else the factory one. EMPTY means the action
/// was deliberately left without a key.
pub fn hotkey_key(set: &Settings, action: &str) -> String {
    if let Some(k) = set.hotkeys.get(action) {
        return k.clone();
    }
    HOTKEYS.iter().find(|r| r.action == action).map(|r| r.key.to_string()).unwrap_or_default()
}

/// RECORDS A BINDING keeping the record clean: the factory key removes the entry rather than repeating it.
pub fn set_hotkey(set: &mut Settings, action: &str, key: &str) {
    if HOTKEYS.iter().any(|r| r.action == action && r.key == key) {
        set.hotkeys.remove(action);
    } else {
        set.hotkeys.insert(action.to_string(), key.to_string());
    }
}

/// HOW A CLASH IS SETTLED, as chosen in the window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClashChoice {
    /// The holder gets the key the asking action had: nothing ends up without a key.
    Swap,
    /// The asking action takes the key, the holder is left with none.
    Unbind,
}

/// SETTLES A CLASH: the asking action always gets its chord, the holder gets what the choice says.
pub fn resolve_hotkey_clash(set: &mut Settings, clash: &HotkeyClash, choice: ClashChoice) {
    let old = hotkey_key(set, &clash.action);
    let for_holder = match choice {
        ClashChoice::Swap => old,
        ClashChoice::Unbind => String::new(),
    };
    set_hotkey(set, &clash.action, &clash.chord);
    set_hotkey(set, clash.holder, &for_holder);
}

/// RESETS A BINDING TO THE FACTORY KEY unless another action of the area holds that key: then nothing changes and
/// the clash is handed back, to be asked like the press of that key. Applied regardless, two commands would share it.
pub fn reset_hotkey(set: &mut Settings, action: &str) -> Option<HotkeyClash> {
    let row = HOTKEYS.iter().find(|r| r.action == action)?;
    if let Some(holder) = hotkey_taken_by(set, row.area, row.key, action) {
        return Some(HotkeyClash { action: action.to_string(), chord: row.key.to_string(), holder });
    }
    set.hotkeys.remove(action);
    None
}

/// The thread standard for the index of the command bar's switch.
pub fn thread_standard(idx: u8) -> qymcad_core::thread::ThreadStandard {
    use qymcad_core::thread::ThreadStandard as S;
    match idx {
        1 => S::TrapezoidalTr,
        2 => S::Acme,
        3 => S::RoundRd,
        4 => S::Buttress,
        5 => S::Custom,
        _ => S::MetricIso,
    }
}

/// One row of the reference. THERE IS NO TEXT HERE — only keys of the language catalogue: the reference
/// is read by a person, so it must speak their language like the rest of the interface.
pub struct HotkeyRow {
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
    /// The DEFAULT key, in the spelling of `Chord::name`. What is actually pressed — see `hotkey_key`.
    pub key: &'static str,
    /// A catalogue key (`hotkey-<area>-<key>`), not a phrase.
    pub what: &'static str,
}

/// EVERY HOTKEY. `key` is exactly the name of the `egui::Key` variant (the test checks against the code
/// by it), except for the rows of the general area, where the keys are handled apart from the `match`
/// table.
pub const HOTKEYS: &[HotkeyRow] = &[
    // --- General ---
    HotkeyRow { area: "general", action: "general.esc", key: "Esc", what: "hotkey-general-esc" },
    HotkeyRow { area: "general", action: "general.enter", key: "Enter", what: "hotkey-general-enter" },
    HotkeyRow { area: "general", action: "general.delete", key: "Delete", what: "hotkey-general-delete" },
    HotkeyRow { area: "general", action: "general.rename", key: "F2", what: "hotkey-general-f2" },
    HotkeyRow { area: "general", action: "general.exit-context", key: "Ctrl+Enter", what: "hotkey-general-ctrl-enter" },
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
