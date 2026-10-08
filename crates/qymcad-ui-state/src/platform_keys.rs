//! THE COMBINATIONS EACH SYSTEM KEEPS - one table per system, holding every combination a binding is refused on
//! that system, so a new rule for one of them is a row in its table rather than a condition threaded through the
//! rebinding code.
//!
//! What a binding's modifiers are on each system:
//!
//! * "Ctrl" is the command key: Ctrl on Linux and Windows, Cmd on a Mac. One record means the same keys on every
//!   system, which is what lets one factory layout serve all three.
//! * "Control" is the Mac's own Control key, a modifier of its own there. Linux and Windows have no key that is
//!   not already "Ctrl", so a Control binding brought by a profile from a Mac does not run on them.
//!
//! What the tables hold, and who takes the key first:
//!
//! * a text field: egui erases with Ctrl+H, Ctrl+U and Ctrl+W on Linux and Windows; on a Mac it erases with
//!   Control+H/K/U/W and walks the caret with Control+A/B/E/F/N/P - a tool there would run while the expression
//!   lost a word or the caret jumped;
//! * the system and the window: on a Mac the application menu winit installs hides the program on Cmd+H and quits
//!   it on Cmd+Q, and macOS itself takes the screenshots (Shift+Cmd+3/4/5), logging out (Shift+Cmd+Q), locking the
//!   screen (Control+Cmd+Q) and the keyboard-navigation keys (Control+F3..F8);
//! * the program's own General keys (`GENERAL`), the same on every system today and listed in each table, so a
//!   system that ever answers them differently changes one table.
//!
//! The keys that cannot carry a binding at all (Space, F1, F2, Enter, Esc, Tab, the arrows - see
//! `Chord::bindable_key`) and Alt (the way from a text field to a bare key) are the program's rules, not a
//! system's, and stay beside the bindings.

use crate::Chord;

/// The system the program runs on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Os {
    Linux,
    Windows,
    Mac,
}

impl Os {
    /// Every system the program is built for - a table must exist for each.
    pub const ALL: [Os; 3] = [Os::Linux, Os::Windows, Os::Mac];

    pub const fn current() -> Os {
        if cfg!(target_os = "macos") {
            Os::Mac
        } else if cfg!(target_os = "windows") {
            Os::Windows
        } else {
            Os::Linux
        }
    }
}

/// WHICH PRESSES A ROW STANDS FOR besides its own combination.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Held {
    /// Only this combination: a menu item answers to its exact chord, so Shift+Cmd+H is not Cmd+H.
    Exact,
    /// This combination with any other modifier held as well: a field erasing under Ctrl does not look at Shift.
    Any,
    /// The same, except with Shift: the caret keys of a Mac field ask for Control without Shift.
    AnyButShift,
}

/// One combination a system keeps.
pub struct Kept {
    /// In the stored spelling (`Chord::name`).
    pub chord: &'static str,
    pub held: Held,
    /// The catalogue key of the reason shown in the window.
    pub why: &'static str,
}

impl Kept {
    const fn new(chord: &'static str, held: Held, why: &'static str) -> Self {
        Kept { chord, held, why }
    }

    /// Whether `pressed` is this row's combination, with what else `held` allows.
    pub fn covers(&self, pressed: &Chord) -> bool {
        let Some(k) = Chord::parse(self.chord) else { return false };
        if k.key != pressed.key {
            return false;
        }
        let mods = |c: &Chord| [c.control, c.ctrl, c.shift, c.altgr];
        let (want, got) = (mods(&k), mods(pressed));
        match self.held {
            Held::Exact => want == got,
            Held::Any => want.iter().zip(got).all(|(w, g)| !w || g),
            Held::AnyButShift => !pressed.shift && want.iter().zip(got).all(|(w, g)| !w || g),
        }
    }
}

/// THE RULES OF ONE SYSTEM.
pub struct PlatformKeys {
    pub os: Os,
    /// Whether the keyboard has a Control key apart from the binding's Ctrl.
    pub has_control: bool,
    /// Every combination refused on this system, in groups that several systems share.
    pub kept: &'static [&'static [Kept]],
}

const RESERVED: &str = "hotkeys-reserved";
const FIELD_EDITS: &str = "hotkeys-field-edits";
const MAC_FIELD_EDITS: &str = "hotkeys-mac-field-edits";

/// THE PROGRAM'S GENERAL KEYS: select all, copy, the search, save, paste, cut, redo, undo under Ctrl, and bare X,
/// the construction toggle. The Ctrl handlers ask for Ctrl and the letter without looking at Shift (Ctrl+Shift+Z is
/// redo, Ctrl+Shift+S is "save as"), so a tool on any of those forms would fire TOGETHER with them; the toggle asks
/// for no modifier at all, so Shift+X is free. Held to the handlers by a check that reads them.
pub const GENERAL: [Kept; 9] = [
    Kept::new("Ctrl+A", Held::Any, RESERVED),
    Kept::new("Ctrl+C", Held::Any, RESERVED),
    Kept::new("Ctrl+K", Held::Any, RESERVED),
    Kept::new("Ctrl+S", Held::Any, RESERVED),
    Kept::new("Ctrl+V", Held::Any, RESERVED),
    Kept::new("Ctrl+X", Held::Any, RESERVED),
    Kept::new("Ctrl+Y", Held::Any, RESERVED),
    Kept::new("Ctrl+Z", Held::Any, RESERVED),
    Kept::new("X", Held::Exact, RESERVED),
];

/// egui's text field erases under Ctrl on Linux and Windows: H a character, U the line before the caret, W the word
/// before it (K, the line after it, is the search's already - a General key).
const FIELD_ERASERS: [Kept; 3] = [Kept::new("Ctrl+H", Held::Any, FIELD_EDITS), Kept::new("Ctrl+U", Held::Any, FIELD_EDITS), Kept::new("Ctrl+W", Held::Any, FIELD_EDITS)];

/// egui's text field on a Mac, under Control: H, K, U, W erase (Shift or not), A and E go to the start and the
/// end of the line, B and F a character back and forth, P and N a line up and down (without Shift).
const MAC_FIELD: [Kept; 10] = [
    Kept::new("Control+H", Held::Any, MAC_FIELD_EDITS),
    Kept::new("Control+K", Held::Any, MAC_FIELD_EDITS),
    Kept::new("Control+U", Held::Any, MAC_FIELD_EDITS),
    Kept::new("Control+W", Held::Any, MAC_FIELD_EDITS),
    Kept::new("Control+A", Held::AnyButShift, MAC_FIELD_EDITS),
    Kept::new("Control+E", Held::AnyButShift, MAC_FIELD_EDITS),
    Kept::new("Control+B", Held::AnyButShift, MAC_FIELD_EDITS),
    Kept::new("Control+F", Held::AnyButShift, MAC_FIELD_EDITS),
    Kept::new("Control+P", Held::AnyButShift, MAC_FIELD_EDITS),
    Kept::new("Control+N", Held::AnyButShift, MAC_FIELD_EDITS),
];

/// What macOS and the application menu take before the program hears a key.
const MAC_SYSTEM: [Kept; 13] = [
    Kept::new("Ctrl+H", Held::Exact, "hotkeys-os-hide"),
    Kept::new("Ctrl+Q", Held::Exact, "hotkeys-os-quit"),
    Kept::new("Ctrl+Shift+Q", Held::Exact, "hotkeys-os-system"),
    Kept::new("Control+Ctrl+Q", Held::Exact, "hotkeys-os-system"),
    Kept::new("Ctrl+Shift+3", Held::Exact, "hotkeys-os-system"),
    Kept::new("Ctrl+Shift+4", Held::Exact, "hotkeys-os-system"),
    Kept::new("Ctrl+Shift+5", Held::Exact, "hotkeys-os-system"),
    Kept::new("Control+F3", Held::Exact, "hotkeys-os-system"),
    Kept::new("Control+F4", Held::Exact, "hotkeys-os-system"),
    Kept::new("Control+F5", Held::Exact, "hotkeys-os-system"),
    Kept::new("Control+F6", Held::Exact, "hotkeys-os-system"),
    Kept::new("Control+F7", Held::Exact, "hotkeys-os-system"),
    Kept::new("Control+F8", Held::Exact, "hotkeys-os-system"),
];

pub const LINUX: PlatformKeys = PlatformKeys { os: Os::Linux, has_control: false, kept: &[&GENERAL, &FIELD_ERASERS] };

/// The same as Linux today - and a table of its own, so a Windows rule never has to be a Linux one too.
pub const WINDOWS: PlatformKeys = PlatformKeys { os: Os::Windows, has_control: false, kept: &[&GENERAL, &FIELD_ERASERS] };

pub const MAC: PlatformKeys = PlatformKeys { os: Os::Mac, has_control: true, kept: &[&GENERAL, &MAC_SYSTEM, &MAC_FIELD] };

pub fn platform_keys(os: Os) -> &'static PlatformKeys {
    match os {
        Os::Linux => &LINUX,
        Os::Windows => &WINDOWS,
        Os::Mac => &MAC,
    }
}

impl PlatformKeys {
    /// Why this system will not let a chord reach a tool - a catalogue key - or `None`.
    pub fn refusal(&self, chord: &Chord) -> Option<&'static str> {
        self.missing_modifier(chord).or_else(|| self.kept.iter().flat_map(|g| g.iter()).find(|k| k.covers(chord)).map(|k| k.why))
    }

    /// Why no keyboard of this system can press the chord at all - a modifier it has no key for - or `None`.
    pub fn missing_modifier(&self, chord: &Chord) -> Option<&'static str> {
        (chord.control && !self.has_control).then_some("hotkeys-mac-only")
    }

    /// Every row, for the checks.
    pub fn rows(&self) -> impl Iterator<Item = &'static Kept> {
        self.kept.iter().flat_map(|g| g.iter())
    }
}
