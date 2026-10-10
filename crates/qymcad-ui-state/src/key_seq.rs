//! KEY SEQUENCES: a binding made of chords pressed one after another - `G, G`, `G, C`, `Ctrl+T, F`. A single
//! chord is a sequence of one.
//!
//! How a press is matched against the bindings of the area:
//!
//! * the LONGEST match wins. `G` and `G, G` may both be bound: a G with no longer binding behind it runs at once,
//!   a G that starts a longer one waits for the next chord;
//! * the wait lasts `Settings::key_wait_ms` from the last press (400 ms out of the box). When it runs out, the
//!   chords pressed so far run if they are a binding themselves, and are dropped if not;
//! * a chord that continues nothing ends the wait, runs nothing, and is then heard as a fresh press;
//! * Esc ends the wait and does nothing else.
//!
//! The rule for a text field applies to the FIRST chord only (see `pressed_chord`): once a sequence is under way
//! the program is waiting for it, and the next chord is read as it is pressed and kept from the field.

use crate::{hotkey_key, hotkey_refusal, key_label, Chord, Settings, HOTKEYS};

/// The most chords a binding may hold.
pub const SEQ_MAX: usize = 4;

/// Between the chords of a stored sequence: `Ctrl+T, F`.
const SEP: &str = ", ";

/// A BINDING: one chord or several, pressed in order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct KeySeq {
    pub chords: Vec<Chord>,
}

impl From<Chord> for KeySeq {
    fn from(c: Chord) -> Self {
        KeySeq { chords: vec![c] }
    }
}

impl From<&Chord> for KeySeq {
    fn from(c: &Chord) -> Self {
        KeySeq { chords: vec![*c] }
    }
}

impl From<&KeySeq> for KeySeq {
    fn from(s: &KeySeq) -> Self {
        s.clone()
    }
}

impl KeySeq {
    /// Reads the stored spelling: chords separated by commas. `None` for anything that is not a binding: an empty
    /// record (an unbound action), a description like `Ctrl+Z / Ctrl+Y`, more than `SEQ_MAX` chords.
    pub fn parse(s: &str) -> Option<KeySeq> {
        let chords = s.split(',').map(Chord::parse).collect::<Option<Vec<Chord>>>()?;
        (!chords.is_empty() && chords.len() <= SEQ_MAX).then_some(KeySeq { chords })
    }

    /// The stored spelling. A single chord reads as it always has: `Ctrl+J`.
    pub fn name(&self) -> String {
        self.chords.iter().map(Chord::name).collect::<Vec<_>>().join(SEP)
    }

    /// Whether the chords pressed so far are the start of this sequence (or the whole of it).
    pub fn starts_with(&self, pressed: &[Chord]) -> bool {
        self.chords.len() >= pressed.len() && self.chords[..pressed.len()] == *pressed
    }
}

/// WHAT THE CHORDS PRESSED SO FAR MEAN in an area.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SeqMatch {
    /// The action bound to exactly these chords.
    pub exact: Option<&'static str>,
    /// Whether a longer binding starts with them, so the next chord may still change the answer.
    pub longer: bool,
}

/// Matches the chords against the area's bindings. A binding this system refuses is not one: it neither runs nor
/// makes a press wait.
pub fn seq_match(set: &Settings, area: &str, pressed: &[Chord]) -> SeqMatch {
    let mut m = SeqMatch::default();
    for r in HOTKEYS.iter().filter(|r| r.area == area) {
        let Some(seq) = KeySeq::parse(&hotkey_key(set, r.action)) else { continue };
        if !seq.starts_with(pressed) || hotkey_refusal(r.action, &seq).is_some() {
            continue;
        }
        if seq.chords.len() == pressed.len() {
            m.exact = m.exact.or(Some(r.action));
        } else {
            m.longer = true;
        }
    }
    m
}

/// A SEQUENCE UNDER WAY: the chords pressed, and when the last one was.
#[derive(Clone, Debug, Default)]
pub struct KeyWait {
    /// Empty while nothing is waited for.
    pub pressed: Vec<Chord>,
    /// The frame clock (`InputState::time`) at the last press, in seconds.
    pub at: f64,
    /// The area the chords were pressed in. A sequence never runs in another one.
    pub area: &'static str,
}

impl KeyWait {
    pub fn waiting(&self) -> bool {
        !self.pressed.is_empty()
    }

    /// The chords pressed so far, the way this system writes keys (Cmd as its sign on a Mac); empty while nothing is waited for.
    pub fn label(&self) -> String {
        if !self.waiting() {
            return String::new();
        }
        key_label(&KeySeq { chords: self.pressed.clone() }.name())
    }

    fn end(&mut self) {
        self.pressed.clear();
    }
}

/// WHAT THE FRAME'S KEYBOARD BROUGHT, as far as the bindings care.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Press {
    /// No key went down, or only a modifier.
    Nothing,
    Escape,
    /// A key that carries no binding (an arrow, Enter, Tab): ends a sequence under way.
    Other,
    /// A chord that may start a sequence where it was pressed.
    Start(Chord),
    /// A chord that may only go on with a sequence under way - a bare letter in a text field. Continuing nothing,
    /// it is left to the field.
    Continue(Chord),
}

/// WHAT A PRESS CAME TO.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SeqStep {
    /// A binding whose wait ran out before this press: it runs first.
    pub late: Option<&'static str>,
    /// The binding this press completed.
    pub run: Option<&'static str>,
    /// Whether the press belonged to a sequence under way. Such a press reaches nothing else: neither the field it
    /// would type into nor the cancel ladder.
    pub taken: bool,
}

impl SeqStep {
    /// The actions to run, in order.
    pub fn actions(&self) -> impl Iterator<Item = &'static str> {
        self.late.into_iter().chain(self.run)
    }
}

/// HOW LONG A SEQUENCE WAITS for its next chord, and when a press happened - both on the frame clock, in seconds.
#[derive(Clone, Copy, Debug)]
pub struct SeqClock {
    pub now: f64,
    pub timeout: f64,
}

/// ONE STEP OF MATCHING: the wait is checked against the clock, then the press is matched. Pure, so every turn of
/// it is checked without a frame.
pub fn seq_step(set: &Settings, area: &'static str, wait: &mut KeyWait, press: Press, clock: SeqClock) -> SeqStep {
    let mut out = SeqStep::default();
    if wait.waiting() && (wait.area != area || clock.now - wait.at >= clock.timeout) {
        // a wait that ran out runs what it holds, in the area it was pressed in and nowhere else
        if wait.area == area {
            out.late = seq_match(set, area, &wait.pressed).exact;
        }
        wait.end();
    }
    let chord = match press {
        Press::Nothing => return out,
        Press::Escape | Press::Other => {
            out.taken = press == Press::Escape && wait.waiting();
            wait.end();
            return out;
        }
        Press::Start(c) | Press::Continue(c) => c,
    };
    if wait.waiting() {
        let mut pressed = wait.pressed.clone();
        pressed.push(chord);
        if let Some(run) = follow(set, area, wait, pressed, clock.now) {
            out.run = run;
            out.taken = true;
            return out;
        }
        // continuing nothing: the wait ends and runs nothing, the press is heard afresh
        wait.end();
    }
    if let Press::Start(c) = press {
        out.run = follow(set, area, wait, vec![c], clock.now).flatten();
    }
    out
}

/// The chords pressed, matched: `Some(Some(action))` runs it, `Some(None)` waits for the next chord, `None` means
/// they lead nowhere.
fn follow(set: &Settings, area: &'static str, wait: &mut KeyWait, pressed: Vec<Chord>, now: f64) -> Option<Option<&'static str>> {
    let m = seq_match(set, area, &pressed);
    if m.longer {
        *wait = KeyWait { pressed, at: now, area };
        return Some(None);
    }
    wait.end();
    m.exact.map(Some)
}

/// THE FRAME'S PRESS MATCHED in `area` (`None` where no bindings are heard), with the frame's input brought in
/// line: a taken press is removed - the key and the text it would type - so no field and no later handler sees
/// it, and a sequence under way asks for a frame at its deadline, so it ends with no input coming.
pub fn hotkey_presses(ctx: &egui::Context, set: &Settings, area: Option<&'static str>, wait: &mut KeyWait) -> SeqStep {
    let press = crate::pressed_chord(ctx);
    let clock = SeqClock { now: ctx.input(|i| i.time), timeout: f64::from(set.key_wait_ms) / 1000.0 };
    let Some(area) = area else {
        wait.end();
        return SeqStep::default();
    };
    let step = seq_step(set, area, wait, press, clock);
    if step.taken {
        ctx.input_mut(|i| take_press(&mut i.events));
    }
    if wait.waiting() {
        ctx.request_repaint_after(std::time::Duration::from_secs_f64((wait.at + clock.timeout - clock.now).max(0.0)));
    }
    step
}

/// Removes the first key that went down, and the first text after it - the character that key typed.
fn take_press(events: &mut Vec<egui::Event>) {
    let Some(k) = events.iter().position(|e| matches!(e, egui::Event::Key { key, pressed: true, repeat: false, .. } if !crate::modifier_key(*key))) else { return };
    events.remove(k);
    if let Some(t) = events[k..].iter().position(|e| matches!(e, egui::Event::Text(_))) {
        events.remove(k + t);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::Key;

    fn set_with(binds: &[(&str, &str)]) -> Settings {
        let mut set = Settings::default();
        for (action, key) in binds {
            set.hotkeys.insert(action.to_string(), key.to_string());
        }
        set
    }

    fn g() -> Chord {
        Chord::from(Key::G)
    }

    const AT: SeqClock = SeqClock { now: 10.0, timeout: 0.3 };

    fn at(now: f64) -> SeqClock {
        SeqClock { now, ..AT }
    }

    #[test]
    fn a_sequence_reads_back_as_written() {
        for s in ["G, G", "Ctrl+T, F", "Control+Ctrl+Shift+J, K", "AltGr+F", "Shift+AltGr+F, 1, F5, Q"] {
            assert_eq!(KeySeq::parse(s).map(|k| k.name()).as_deref(), Some(s));
        }
        assert_eq!(KeySeq::parse("Ctrl+T,F").map(|k| k.name()).as_deref(), Some("Ctrl+T, F"), "the spacing is not part of the binding");
        assert_eq!(KeySeq::parse("A, B, C, D, E"), None, "more than {SEQ_MAX} chords");
        assert_eq!(KeySeq::parse("G, "), None, "an empty step");
        assert_eq!(KeySeq::parse("Ctrl+Z / Ctrl+Y"), None);
        assert_eq!(KeySeq::parse(""), None);
    }

    /// A SINGLE-CHORD SETTING FROM BEFORE SEQUENCES reads unchanged, as a sequence of one.
    #[test]
    fn a_single_chord_setting_reads_as_before() {
        for s in ["W", "Ctrl+J", "Control+Ctrl+Shift+J", "F5"] {
            let k = KeySeq::parse(s).expect(s);
            assert_eq!(k.chords, vec![Chord::parse(s).expect(s)]);
            assert_eq!(k.name(), s);
        }
    }

    /// G, G runs its action; G alone waits and then, with nothing bound to it, runs nothing.
    #[test]
    fn a_sequence_runs_on_its_last_chord() {
        let set = set_with(&[("part.extrude", "G, G")]);
        let mut wait = KeyWait::default();
        let first = seq_step(&set, "part", &mut wait, Press::Start(g()), at(10.0));
        assert_eq!(first, SeqStep::default(), "the first G ran something or was taken");
        assert!(wait.waiting(), "G starts G, G and is not waited on");
        let second = seq_step(&set, "part", &mut wait, Press::Start(g()), at(10.1));
        assert_eq!(second, SeqStep { late: None, run: Some("part.extrude"), taken: true });
        assert!(!wait.waiting());
        seq_step(&set, "part", &mut wait, Press::Start(g()), at(11.0));
        assert_eq!(seq_step(&set, "part", &mut wait, Press::Nothing, at(11.31)), SeqStep::default(), "the wait ran out and G alone means nothing");
        assert!(!wait.waiting(), "the wait outlived its timeout");
    }

    /// THE LONGEST MATCH WINS, and the shorter binding runs when no longer one follows in time.
    #[test]
    fn a_bound_prefix_runs_when_the_wait_runs_out() {
        let set = set_with(&[("part.extrude", "G, G"), ("part.cut", "G")]);
        let mut wait = KeyWait::default();
        assert_eq!(seq_step(&set, "part", &mut wait, Press::Start(g()), at(10.0)).run, None, "G ran at once though G, G may follow");
        assert_eq!(seq_step(&set, "part", &mut wait, Press::Nothing, at(10.2)), SeqStep::default(), "G ran before its wait was up");
        assert_eq!(seq_step(&set, "part", &mut wait, Press::Nothing, at(10.3)).late, Some("part.cut"), "G was not run when the wait ran out");
        assert!(!wait.waiting());
        seq_step(&set, "part", &mut wait, Press::Start(g()), at(12.0));
        assert_eq!(seq_step(&set, "part", &mut wait, Press::Start(g()), at(12.1)).actions().collect::<Vec<_>>(), vec!["part.extrude"], "G, G ran the short binding too");
    }

    /// A PRESS WITH NO LONGER BINDING BEHIND IT RUNS AT ONCE - no waiting for a sequence that cannot come.
    #[test]
    fn a_chord_that_starts_nothing_longer_runs_at_once() {
        let set = set_with(&[("part.extrude", "G, G"), ("part.cut", "C")]);
        let mut wait = KeyWait::default();
        assert_eq!(seq_step(&set, "part", &mut wait, Press::Start(Chord::from(Key::C)), at(10.0)).run, Some("part.cut"));
        assert!(!wait.waiting());
    }

    /// A CHORD THAT CONTINUES NOTHING ends the wait, runs nothing that waited, and is heard afresh.
    #[test]
    fn a_chord_that_continues_nothing_is_heard_afresh() {
        let set = set_with(&[("part.extrude", "G, G"), ("part.cut", "G"), ("part.hole", "O")]);
        let mut wait = KeyWait::default();
        seq_step(&set, "part", &mut wait, Press::Start(g()), at(10.0));
        let o = seq_step(&set, "part", &mut wait, Press::Start(Chord::from(Key::O)), at(10.1));
        assert_eq!(o, SeqStep { late: None, run: Some("part.hole"), taken: false }, "the waiting G ran, or O was lost");
        assert!(!wait.waiting());
        // a bare letter in a field continues nothing: it ends the wait and goes to the field
        seq_step(&set, "part", &mut wait, Press::Start(g()), at(11.0));
        assert_eq!(seq_step(&set, "part", &mut wait, Press::Continue(Chord::from(Key::O)), at(11.1)), SeqStep::default());
        assert!(!wait.waiting());
        // and it starts nothing either
        assert_eq!(seq_step(&set, "part", &mut wait, Press::Continue(g()), at(12.0)), SeqStep::default());
        assert!(!wait.waiting());
    }

    /// A BARE STEP IN A FIELD GOES ON WITH A SEQUENCE under way, and is taken from the field.
    #[test]
    fn a_bare_step_continues_a_sequence_from_a_field() {
        let set = set_with(&[("part.extrude", "Ctrl+T, F")]);
        let mut wait = KeyWait::default();
        let ctrl_t = Chord { ctrl: true, ..Chord::from(Key::T) };
        seq_step(&set, "part", &mut wait, Press::Start(ctrl_t), at(10.0));
        assert_eq!(seq_step(&set, "part", &mut wait, Press::Continue(Chord::from(Key::F)), at(10.1)), SeqStep { late: None, run: Some("part.extrude"), taken: true });
    }

    /// ESC ENDS THE WAIT, runs nothing, and is taken from the cancel ladder; with nothing waiting it is left alone.
    #[test]
    fn escape_ends_the_wait_and_nothing_else() {
        let set = set_with(&[("part.extrude", "G, G"), ("part.cut", "G")]);
        let mut wait = KeyWait::default();
        seq_step(&set, "part", &mut wait, Press::Start(g()), at(10.0));
        assert_eq!(seq_step(&set, "part", &mut wait, Press::Escape, at(10.1)), SeqStep { late: None, run: None, taken: true });
        assert!(!wait.waiting());
        assert_eq!(seq_step(&set, "part", &mut wait, Press::Nothing, at(11.0)), SeqStep::default(), "the dropped G ran later");
        assert_eq!(seq_step(&set, "part", &mut wait, Press::Escape, at(12.0)), SeqStep::default(), "an Esc with nothing waiting was kept from the ladder");
        seq_step(&set, "part", &mut wait, Press::Start(g()), at(13.0));
        assert_eq!(seq_step(&set, "part", &mut wait, Press::Other, at(13.1)), SeqStep::default(), "an arrow key is no part of a sequence");
        assert!(!wait.waiting());
    }

    /// A WAIT RUN OUT IN THE FRAME OF A NEW PRESS runs what waited first, then hears the press.
    #[test]
    fn a_press_after_the_deadline_runs_what_waited_first() {
        let set = set_with(&[("part.extrude", "G, G"), ("part.cut", "G"), ("part.hole", "O")]);
        let mut wait = KeyWait::default();
        seq_step(&set, "part", &mut wait, Press::Start(g()), at(10.0));
        let late = seq_step(&set, "part", &mut wait, Press::Start(Chord::from(Key::O)), at(10.5));
        assert_eq!(late.actions().collect::<Vec<_>>(), vec!["part.cut", "part.hole"]);
    }

    /// A SEQUENCE BELONGS TO ITS AREA: begun in a Part and ended in a sketch, it runs nothing in either.
    #[test]
    fn a_sequence_does_not_cross_areas() {
        let set = set_with(&[("part.extrude", "G, G"), ("part.cut", "G"), ("sketch.line", "G")]);
        let mut wait = KeyWait::default();
        seq_step(&set, "part", &mut wait, Press::Start(g()), at(10.0));
        assert_eq!(seq_step(&set, "sketch", &mut wait, Press::Nothing, at(10.1)), SeqStep::default(), "the part's G ran in the sketch");
        assert!(!wait.waiting());
    }

    /// A REFUSED BINDING NEITHER RUNS NOR WAITS: Ctrl+S, F would keep every save waiting.
    #[test]
    fn a_refused_sequence_neither_runs_nor_waits() {
        let set = set_with(&[("part.extrude", "Ctrl+S, F")]);
        let mut wait = KeyWait::default();
        seq_step(&set, "part", &mut wait, Press::Start(Chord { ctrl: true, ..Chord::from(Key::S) }), at(10.0));
        assert!(!wait.waiting(), "Ctrl+S waits for an F");
    }

    #[test]
    fn the_wait_is_shown_as_pressed() {
        let mut wait = KeyWait::default();
        assert_eq!(wait.label(), "");
        wait.pressed = vec![Chord { ctrl: true, ..Chord::from(Key::T) }, g()];
        assert_eq!(wait.label(), key_label("Ctrl+T, G"));
    }

    #[test]
    fn a_taken_press_takes_its_text_along() {
        let key = |key, pressed| egui::Event::Key { key, physical_key: None, pressed, repeat: false, modifiers: egui::Modifiers::NONE };
        let mut events = vec![key(Key::AltLeft, true), egui::Event::Text("x".into()), key(Key::G, true), egui::Event::Text("g".into()), egui::Event::Text("h".into()), key(Key::G, false)];
        take_press(&mut events);
        assert_eq!(events, vec![key(Key::AltLeft, true), egui::Event::Text("x".into()), egui::Event::Text("h".into()), key(Key::G, false)]);
    }

    /// ONLY THE FIRST CHORD IS HELD TO THE SYSTEM'S TABLE: Ctrl+S stays save, so it starts no sequence, while a later
    /// Ctrl+S is pressed when the program already waits for it. A modifier the keyboard does not have is refused
    /// anywhere in the sequence.
    #[test]
    fn the_first_chord_answers_to_the_system() {
        use crate::{hotkey_refusal_on, platform_keys::Os};
        let on = |os, s: &str| hotkey_refusal_on(os, "part.extrude", KeySeq::parse(s).expect(s));
        for os in Os::ALL {
            assert_eq!(on(os, "Ctrl+S, F"), Some("hotkeys-reserved"), "{os:?}");
            assert_eq!(on(os, "G, Ctrl+S"), None, "{os:?}");
            assert_eq!(on(os, "G, Space"), Some("hotkeys-reserved"), "{os:?}: every chord must be a bindable key");
            assert_eq!(on(os, "G, X"), None, "{os:?}: a bare X later in a sequence is not the construction toggle");
        }
        assert_eq!(on(Os::Linux, "G, Control+J"), Some("hotkeys-mac-only"));
        assert_eq!(on(Os::Mac, "G, Control+J"), None);
    }

    /// ALTGR IS NOT ALT AND NOT NOTHING: AltGr+X is not the bare X of the construction toggle, and Ctrl+AltGr+S is
    /// still Ctrl+S to the save that does not look at what else is held.
    #[test]
    fn altgr_is_a_modifier_of_the_system_tables() {
        use crate::{hotkey_refusal_on, platform_keys::Os};
        for os in Os::ALL {
            assert_eq!(hotkey_refusal_on(os, "part.extrude", Chord::parse("AltGr+X").expect("AltGr+X")), None, "{os:?}");
            assert_eq!(hotkey_refusal_on(os, "part.extrude", Chord::parse("Ctrl+AltGr+S").expect("Ctrl+AltGr+S")), Some("hotkeys-reserved"), "{os:?}");
        }
    }

    /// THE RIGHT ALT IS ALTGR, told apart by its key; Windows' left Ctrl sent along with it is nobody's press.
    #[test]
    fn the_right_alt_is_altgr() {
        use crate::{platform_keys::Os, AltHeld, HeldKeys};
        let right = HeldKeys { mods: egui::Modifiers::ALT, alt: AltHeld::Right };
        let left = HeldKeys { mods: egui::Modifiers::ALT, alt: AltHeld::Left };
        assert_eq!(Chord::of_held(Os::Linux, right, Key::F).name(), "AltGr+F");
        assert_eq!(Chord::of_held(Os::Linux, left, Key::F).name(), "F", "the left Alt is no part of a chord");
        let windows = HeldKeys { mods: egui::Modifiers { ctrl: true, command: true, ..egui::Modifiers::ALT }, alt: AltHeld::Right };
        assert_eq!(Chord::of_held(Os::Windows, windows, Key::F).name(), "AltGr+F", "the Ctrl Windows sends with AltGr was taken for a press");
        assert_eq!(Chord::of_held(Os::Linux, windows, Key::F).name(), "Ctrl+AltGr+F", "off Windows a Ctrl held with AltGr is a real one");
        assert_eq!(Chord::parse("Ctrl+Shift+AltGr+F").map(|c| c.name()).as_deref(), Some("Ctrl+Shift+AltGr+F"));
    }

    /// The two Alt keys as the frame reports them.
    #[test]
    fn the_two_alts_are_told_apart_by_their_keys() {
        use crate::AltHeld;
        let ctx = egui::Context::default();
        let alt = |key, pressed| egui::Event::Key { key, physical_key: None, pressed, repeat: false, modifiers: egui::Modifiers::ALT };
        let held = |events: Vec<egui::Event>, mods| {
            let mut out = AltHeld::None;
            let _ = ctx.run_ui(egui::RawInput { modifiers: mods, events, ..Default::default() }, |ui| out = ui.input(|i| AltHeld::of(i, i.modifiers.alt)));
            out
        };
        assert_eq!(held(vec![alt(Key::AltRight, true)], egui::Modifiers::ALT), AltHeld::Right);
        assert_eq!(held(vec![alt(Key::AltLeft, true)], egui::Modifiers::ALT), AltHeld::Both);
        assert_eq!(held(vec![alt(Key::AltRight, false), alt(Key::AltLeft, false)], egui::Modifiers::NONE), AltHeld::None);
        assert_eq!(held(Vec::new(), egui::Modifiers::ALT), AltHeld::Left, "an Alt with no key seen is the left one");
    }
}
