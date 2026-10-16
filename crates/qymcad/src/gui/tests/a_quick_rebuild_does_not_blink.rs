//! A QUICK REBUILD DOES NOT BLINK.
//!
//! Reported behaviour: on quick rebuilds the screen blinks - the dimming of the viewport and the spinner come up for one
//! or two frames. Every background rebuild painted its sign on the first frame it was in flight, and one landing in
//! 20-50 ms was on screen for a frame or two.
//!
//! Checked by the frames of the window, each read for what it painted, after an edit made by hand: a reopened
//! extrusion given a new height and Enter. A sign goes up only once its rebuild has run `REBUILD_SIGN_AFTER`, and once
//! up it stays `REBUILD_SIGN_HOLDS` at least. The barrier of a loud rebuild does not wait for its paint.
#[cfg(test)]
pub(in crate::gui) mod tests {
    use crate::gui::hand::Hand;
    use crate::gui::App;
    use qymcad_ui_state::{REBUILD_SIGN_AFTER, REBUILD_SIGN_HOLDS};
    use std::ops::Range;
    use std::time::{Duration, Instant};

    /// A SIGN OF A REBUILD, as a frame shows it.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(in crate::gui) enum Sign {
        /// the faint veil of a quiet rebuild over the canvas
        Veil,
        /// the eight dots turning in the middle of the canvas
        Spinner,
        /// the dimming of a loud one over the panels
        Dimming,
        /// the card of a loud one, a rounded plate of the panel's colour
        Card,
    }

    /// What a frame drew that a sign is told by: the fills of square rectangles and of rounded ones, and the dots of the
    /// spinner's size.
    #[derive(Default)]
    struct Marks {
        fills: Vec<egui::Color32>,
        plates: Vec<egui::Color32>,
        dots: Vec<egui::Pos2>,
    }

    fn marks_of(s: &egui::Shape, marks: &mut Marks) {
        match s {
            egui::Shape::Rect(r) if r.corner_radius.nw >= 10 => marks.plates.push(r.fill),
            egui::Shape::Rect(r) => marks.fills.push(r.fill),
            egui::Shape::Circle(c) if (c.radius - 3.0).abs() < 1e-3 => marks.dots.push(c.center),
            egui::Shape::Vec(v) => v.iter().for_each(|x| marks_of(x, marks)),
            _ => {}
        }
    }

    /// THE SIGNS OF A REBUILD THE LAST FRAME PAINTED, told by their look.
    pub(in crate::gui) fn signs_painted(h: &Hand) -> Vec<Sign> {
        let mut marks = Marks::default();
        h.shapes_drawn().iter().for_each(|cs| marks_of(&cs.shape, &mut marks));
        let scrim = h.app.scheme.pal.scrim();
        let centre = h.app.viewing.view_rect.center();
        let mut out = Vec::new();
        if marks.fills.contains(&qymcad_scheme::a(scrim, crate::gui::render::QUIET_VEIL_ALPHA)) {
            out.push(Sign::Veil);
        }
        if marks.dots.iter().filter(|d| (d.distance(centre) - 16.0).abs() < 0.5).count() >= 8 {
            out.push(Sign::Spinner);
        }
        if marks.fills.contains(&qymcad_scheme::a(scrim, crate::gui::render::DIM_ALPHA)) {
            out.push(Sign::Dimming);
        }
        if marks.plates.contains(&h.app.scheme.pal.panel_bg()) {
            out.push(Sign::Card);
        }
        out
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Kind {
        Quiet,
        Loud,
    }

    /// ONE FRAME WATCHED: when it ran, from the start of the rebuild; whether the rebuild was still in flight after it;
    /// what it painted.
    struct Frame {
        when: Range<Duration>,
        running: bool,
        painted: Vec<Sign>,
    }

    /// The frames of the window around one rebuild, and how long the rebuild took, to the frame that saw it landed.
    struct Watched {
        kind: Kind,
        frames: Vec<Frame>,
        took: Duration,
    }

    impl Watched {
        /// The frames painted before the rebuild had run the limit.
        fn early(&self) -> Vec<String> {
            self.frames.iter().filter(|f| !f.painted.is_empty() && f.when.end < REBUILD_SIGN_AFTER).map(|f| format!("{:?} at {:?}", f.painted, f.when)).collect()
        }

        fn painted(&self) -> Vec<String> {
            self.frames.iter().filter(|f| !f.painted.is_empty()).map(|f| format!("{:?} at {:?}", f.painted, f.when)).collect()
        }
    }

    /// `n` parts, a rectangle extruded each; a rebuild in the worker waits `hold_ms` before it starts, when given.
    fn parts(n: usize, hold_ms: Option<u64>) -> App {
        let mut app = App::default();
        if let Some(ms) = hold_ms {
            app.project.meta.comment = format!("{}{ms}", crate::gui::io_jobs::REBUILD_HOLDS_FOR_TEST);
        }
        for k in 0..n {
            crate::gui::joint_flow::tests::add_part_at(&mut app, 100.0 * k as f64);
        }
        crate::gui::io_jobs::regenerate_now(&mut app.rebuild_ctx());
        app
    }

    /// The extrusion of the first part reopened from the tree, as a person reopens it, and given a new height.
    fn reopen_the_first_extrusion(h: &mut Hand) {
        let name = h.app.project.components.iter().find(|c| c.name == "Part 0").map(|c| crate::i18n::name(&c.name)).expect("the first part");
        assert!(h.double_click_word(&name), "the tree shows the part as {name:?}");
        let ti = h.app.project.timeline.iter().position(|n| n.kind.body().is_some()).expect("the extrusion of the first part");
        let label = crate::gui::panels_tree::feature_row_label(&h.app.project, ti);
        assert!(h.double_click_word(&label), "the tree shows the extrusion as {label:?}");
        assert!(h.app.tools.cmd.edit.is_some(), "a double click on the row reopens the extrusion: {}", h.app.status);
        h.set("height", 14.0);
    }

    /// Enter pressed and released, each in a frame of its own: the events of two frames.
    fn enter() -> Vec<Vec<egui::Event>> {
        [true, false].into_iter().map(|pressed| vec![egui::Event::Key { key: egui::Key::Enter, physical_key: None, pressed, repeat: false, modifiers: Default::default() }]).collect()
    }

    /// FRAMES FROM ENTER ON, as fast as the window draws them, until the rebuild Enter asked for has landed and its sign
    /// has had the time to go. Nothing is waited for after Enter, as `Hand::key` waits; the frames of the key are watched
    /// too - a rebuild of 7 ms starts and lands within them.
    fn enter_and_watch(h: &mut Hand) -> Watched {
        struct Seen {
            when: Range<Instant>,
            running: bool,
            painted: Vec<Sign>,
        }
        let mut seen = Vec::new();
        let mut started = None;
        let mut kind = None;
        let mut landed = None;
        let mut keys = enter().into_iter();
        let began = Instant::now();
        while began.elapsed() < Duration::from_secs(60) && (started.is_some() || began.elapsed() < Duration::from_secs(5)) {
            let from = Instant::now();
            h.frame(keys.next().unwrap_or_default());
            let to = Instant::now();
            let busy = h.app.regen.busy.as_ref().filter(|b| b.kind == crate::gui::BgKind::Regen);
            if let Some(b) = busy {
                started.get_or_insert(b.started);
                kind.get_or_insert(if b.quiet { Kind::Quiet } else { Kind::Loud });
            }
            let running = busy.is_some();
            seen.push(Seen { when: from..to, running, painted: signs_painted(h) });
            if started.is_some() && !running && landed.is_none() {
                landed = Some(to);
            }
            if landed.is_some_and(|l| to > l + REBUILD_SIGN_HOLDS + Duration::from_millis(200)) {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        let started = started.expect("setup: Enter asked for no rebuild in the background");
        let at = |t: Instant| t.saturating_duration_since(started);
        let frames = seen.into_iter().map(|s| Frame { when: at(s.when.start)..at(s.when.end), running: s.running, painted: s.painted }).collect();
        Watched { kind: kind.expect("the kind of the rebuild"), frames, took: at(landed.expect("setup: the rebuild did not land in a minute")) }
    }

    /// A QUICK EDIT PAINTS NOTHING, quiet or loud: no frame shows a sign before the limit, and a rebuild landing before
    /// it shows none at all. Measured on this path in a test build: the rebuild of one extrusion lands in 15-40 ms.
    fn a_quick_edit_paints_nothing(n: usize, kind: Kind) {
        let mut app = parts(n, None);
        let mut h = Hand::new(&mut app);
        reopen_the_first_extrusion(&mut h);
        let w = enter_and_watch(&mut h);
        assert_eq!(w.kind, kind, "setup: the edit of the first of {n} parts is to rebuild as {kind:?}");
        let early = w.early();
        assert!(early.is_empty(), "a {kind:?} rebuild that took {:?} painted its sign before {REBUILD_SIGN_AFTER:?}: {early:?} - the window blinks", w.took);
        if w.took < REBUILD_SIGN_AFTER {
            let painted = w.painted();
            assert!(painted.is_empty(), "a {kind:?} rebuild of {:?} painted a sign: {painted:?}", w.took);
        }
    }

    #[test]
    fn a_quick_quiet_edit_paints_nothing() {
        a_quick_edit_paints_nothing(2, Kind::Quiet);
    }

    #[test]
    fn a_quick_loud_edit_paints_nothing() {
        a_quick_edit_paints_nothing(1, Kind::Loud);
    }

    /// A SLOW REBUILD IS SHOWN FROM THE LIMIT ON, and its sign stays its time after the rebuild lands: held 400 ms in the
    /// worker, it lands 100 ms after its sign went up, and the sign stays 200 ms more.
    fn a_slow_rebuild_is_shown_and_held(n: usize, kind: Kind) {
        let mut app = parts(n, Some(400));
        let mut h = Hand::new(&mut app);
        reopen_the_first_extrusion(&mut h);
        let w = enter_and_watch(&mut h);
        assert_eq!(w.kind, kind, "setup: the edit of the first of {n} parts is to rebuild as {kind:?}");
        let look = match kind {
            Kind::Quiet => vec![Sign::Veil, Sign::Spinner],
            Kind::Loud => vec![Sign::Dimming, Sign::Card],
        };
        let early = w.early();
        assert!(early.is_empty(), "the sign of a {kind:?} rebuild was painted before {REBUILD_SIGN_AFTER:?}: {early:?}");
        let first = w.frames.iter().find(|f| !f.painted.is_empty()).unwrap_or_else(|| panic!("a {kind:?} rebuild of {:?} painted no sign at all", w.took));
        let wrong: Vec<String> = w.frames.iter().filter(|f| !f.painted.is_empty() && f.painted != look).map(|f| format!("{:?} at {:?}", f.painted, f.when)).collect();
        assert!(wrong.is_empty(), "the sign of a {kind:?} rebuild is {look:?}, and frames painted {wrong:?}");
        let shown = first.when.clone();
        let missing: Vec<String> = w
            .frames
            .iter()
            .filter(|f| f.painted.is_empty() && f.when.start > shown.end && ((f.running && f.when.start >= REBUILD_SIGN_AFTER) || f.when.end < shown.start + REBUILD_SIGN_HOLDS))
            .map(|f| format!("{:?}", f.when))
            .collect();
        assert!(missing.is_empty(), "the sign went up at {shown:?}, the rebuild landed at {:?}, and frames painted nothing at {missing:?} - it flashed off", w.took);
        let stale: Vec<String> = w.frames.iter().filter(|f| !f.painted.is_empty() && !f.running && f.when.start > shown.end + REBUILD_SIGN_HOLDS).map(|f| format!("{:?}", f.when)).collect();
        assert!(stale.is_empty(), "the sign went up at {shown:?}, the rebuild landed at {:?}, and the sign was still up at {stale:?}", w.took);
    }

    #[test]
    fn a_slow_quiet_rebuild_is_shown_and_held() {
        a_slow_rebuild_is_shown_and_held(2, Kind::Quiet);
    }

    #[test]
    fn a_slow_loud_rebuild_is_shown_and_held() {
        a_slow_rebuild_is_shown_and_held(1, Kind::Loud);
    }

    /// A LOUD REBUILD MUTES THE PANELS FROM ITS FIRST FRAME, its sign not yet painted: a click on the File menu while it
    /// runs opens nothing. Held 200 ms in the worker, under the limit.
    #[test]
    fn a_loud_rebuild_mutes_the_panels_before_its_sign_is_up() {
        let mut app = parts(1, Some(200));
        let mut h = Hand::new(&mut app);
        reopen_the_first_extrusion(&mut h);
        let file = crate::i18n::tr("menu-file");
        let new_project = crate::i18n::tr("file-new");
        let at = h.written_at(&file).unwrap_or_else(|| panic!("the menu bar has no {file:?}")).center();
        for events in enter() {
            h.frame(events);
        }
        h.frame(vec![egui::Event::PointerMoved(at)]);
        let started = h.app.regen.busy.as_ref().filter(|b| b.kind == crate::gui::BgKind::Regen && !b.quiet).map(|b| b.started).expect("setup: Enter asked for a loud rebuild in the background");
        let mut painted = signs_painted(&h);
        let button = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        h.frame(vec![button(true)]);
        painted.extend(signs_painted(&h));
        h.frame(vec![button(false)]);
        painted.extend(signs_painted(&h));
        let clicked = started.elapsed();
        assert!(h.app.regen.busy.is_some() && clicked < REBUILD_SIGN_AFTER, "setup: the click is to land while the rebuild runs and before its sign is due, and it landed at {clicked:?}");
        assert!(painted.is_empty(), "the sign was painted {clicked:?} into the rebuild: {painted:?}");
        h.frame(Vec::new());
        assert!(!h.shows(&new_project), "the File menu opened under a loud rebuild - an edit from there lands on a stale copy of the document");

        // THE SAME CLICK ONCE THE REBUILD HAS LANDED opens the menu, or the one above proves nothing
        h.close_window();
        h.click_screen(at);
        assert!(h.shows(&new_project), "setup: a click on {file:?} with no rebuild running is to open the menu");
    }
}
