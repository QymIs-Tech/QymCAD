//! A PERSON'S HAND: the only way for a test to touch the program.
//!
//! The tests of the application used to cheat — they reached straight into the fields
//! (`app.tools.gsel.edges.insert(...)`). Such a test checks what a person cannot do and skips what they do
//! every time: did the click land, what was under the cursor, which tool is open. Hence the class of
//! breakages that was caught by hand and not by the run.
//!
//! The hand can do exactly what a person can: press a tool button, click a point in the scene, press
//! Enter or Esc. It has nothing else — and that is its main property.

#[cfg(test)]
mod presses;
#[cfg(test)]
mod sketch_text;
#[cfg(test)]
pub(super) use presses::escape;

/// A CARRY ON THE SCREEN: where the button goes down and where the hand holds it, the button still down.
#[cfg(test)]
pub(super) struct Carry {
    pub from: egui::Pos2,
    pub to: egui::Pos2,
}

/// A DRAG ON THE SKETCH: from where to where, in the sketch's own coordinates, by which button and with which keys held.
#[cfg(test)]
pub(super) struct Drag2d {
    pub from: (f64, f64),
    pub to: (f64, f64),
    pub button: egui::PointerButton,
    pub modifiers: egui::Modifiers,
}

/// A DRAG OF THE VIEW: from where to where on the screen, by which button and with which keys held.
#[cfg(test)]
pub(super) struct ViewDrag {
    pub button: egui::PointerButton,
    pub modifiers: egui::Modifiers,
    pub from: egui::Pos2,
    pub to: egui::Pos2,
}

/// A FORMULA TYPED INTO A FIELD OF A COMMAND: the field, and what is typed into it.
#[cfg(test)]
pub(super) struct Typed<'a> {
    pub field: &'a str,
    pub text: &'a str,
}

/// A RUN OF WORDS AS THE FRAME PAINTED IT: what it says, where it stands and in which colour.
#[cfg(test)]
pub(super) struct PaintedWords {
    pub text: String,
    pub rect: egui::Rect,
    pub colour: egui::Color32,
}

/// THE RUNS OF WORDS `shapes` PAINT, with their place and colour - for a frame the hand did not run itself as well.
#[cfg(test)]
pub(super) fn painted_in(shapes: &[egui::epaint::ClippedShape]) -> Vec<PaintedWords> {
    fn collect(s: &egui::Shape, out: &mut Vec<PaintedWords>) {
        match s {
            egui::Shape::Text(t) => {
                let section = t.galley.job.sections.first().map(|s| s.format.color).filter(|c| *c != egui::Color32::PLACEHOLDER);
                let colour = t.override_text_color.or(section).unwrap_or(t.fallback_color);
                out.push(PaintedWords { text: t.galley.text().to_string(), rect: egui::Rect::from_min_size(t.pos, t.galley.size()), colour });
            }
            egui::Shape::Vec(v) => v.iter().for_each(|x| collect(x, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for cs in shapes {
        collect(&cs.shape, &mut out);
    }
    out
}

#[cfg(test)]
/// A FILLED GROUND OF THE SCREEN and its colour.
pub(super) struct Ground {
    pub rect: egui::Rect,
    pub fill: egui::Color32,
}

#[cfg(test)]
pub(super) struct Hand<'a> {
    pub app: &'a mut super::App,
    pub rect: egui::Rect,
    /// THE WINDOW the mouse and the keys go into, kept for the whole gesture: egui tells a click from a drag,
    /// and a double click from two clicks, by what it saw in the frames before.
    win: super::window::Window,
    /// the longest frame since it was last asked for (`worst_frame`): what a person feels as the window standing still
    worst: std::time::Duration,
}

#[cfg(test)]
thread_local! {
    /// Where the button of each hint was found last, looked at first the next time.
    static SEEN: std::cell::RefCell<std::collections::HashMap<String, egui::Pos2>> = Default::default();
}

#[cfg(test)]
impl<'a> Hand<'a> {
    /// The size of the window whole frames are drawn into - the one the other whole-frame checks use.
    const SCREEN: egui::Vec2 = egui::vec2(1400.0, 900.0);

    /// Take the program in hand: a 900x700 frame, a 3D view, the camera fitted to the scene.
    pub fn new(app: &'a mut super::App) -> Self {
        app.viewing.mode_3d = true;
        app.viewing.cam.init = true;
        let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 700.0));
        Self { app, rect, win: super::window::Window::new(Self::SCREEN), worst: std::time::Duration::ZERO }
    }

    /// ONE WHOLE FRAME OF THE WINDOW with `events` in it, a sixtieth of a second after the one before.
    ///
    /// The frame the live window runs (`App::draw_frame`, panels and all), not the handler of the canvas: the
    /// frame decides first who gets a press or a key (a popup, a field holding the keyboard, a panel lying over
    /// the canvas), and it is also the place that notices an edit made outside an operation boundary.
    pub fn frame(&mut self, events: Vec<egui::Event>) -> &mut Self {
        self.frame_holding(egui::Modifiers::default(), events)
    }

    /// The same frame with `modifiers` held down: the program reads Ctrl from the frame's own state, not from
    /// the key event.
    fn frame_holding(&mut self, modifiers: egui::Modifiers, events: Vec<egui::Event>) -> &mut Self {
        if self.win.clock == 0.0 {
            super::install_fonts(&self.win.ctx);
            // the hand comes to the window after the greeting, not during its five seconds
            self.app.waiting.splash_until = None;
            for _ in 0..3 {
                self.run(egui::Modifiers::default(), Vec::new()); // the panels settle and the canvas learns its rectangle
            }
        }
        self.run(modifiers, events)
    }

    fn run(&mut self, modifiers: egui::Modifiers, events: Vec<egui::Event>) -> &mut Self {
        let meter = qymcad_core::time_budget::Meter::start(qymcad_core::time_budget::Doer::Thread);
        self.win.run(self.app, modifiers, events, false);
        self.worst = self.worst.max(meter.cost().work);
        self
    }

    /// THE LONGEST FRAME SINCE THIS WAS LAST ASKED, by its work (`time_budget::Cost`): the window standing still under a
    /// step of the hand, not the machine busy with others. The time of a whole gesture holds the hand's own looking -
    /// over the hint of every button, four frames each - which a person who knows the panel does not do; the longest
    /// frame is what the person waits.
    pub fn worst_frame(&mut self) -> std::time::Duration {
        std::mem::take(&mut self.worst)
    }

    /// PRESS THE BUTTON WHOSE HINT IS `hint`, found as a person finds an icon they do not know: the hand goes
    /// over the buttons of the window and reads the hint that comes up under the pointer. A panel longer than
    /// the window is scrolled with the wheel. Answers whether such a button was found.
    ///
    /// A hint comes up once the pointer has stood still for half a second, so the hand rests a second over each
    /// button. Where a button was found last time is looked at first; it is still read before it is pressed.
    pub fn press_hint(&mut self, hint: &str) -> bool {
        match self.find_hint(hint) {
            Some(at) => {
                self.press_screen(at);
                true
            }
            None => false,
        }
    }

    /// KNOW WHERE THE BUTTON OF `hint` STANDS, pressing nothing: the hand looks for it as `press_hint` does and
    /// remembers the place. A person knows where the buttons of the panel are; a probe that times a tool learns them
    /// first, or it times the looking - over the hint of every button, four frames each, 6 s of a frame of 30 ms.
    pub fn know_hint(&mut self, hint: &str) -> bool {
        self.find_hint(hint).is_some()
    }

    fn find_hint(&mut self, hint: &str) -> Option<egui::Pos2> {
        self.frame(Vec::new());
        self.rebuild_gone();
        let known = SEEN.with(|s| s.borrow().get(hint).copied());
        if let Some(at) = known.filter(|at| self.win.plates.contains(at)) {
            if self.hint_comes_up(at, hint) {
                return Some(at);
            }
        }
        // down the panel from where it stands; and where the button was not below, from the top of the panel down: a
        // button looked for before may have led the panel down past this one
        if let Some(at) = self.hint_down_the_panel(hint) {
            return Some(at);
        }
        self.panel_to_its_top();
        if let Some(at) = self.hint_down_the_panel(hint) {
            return Some(at);
        }
        self.hint_in_more(hint)
    }

    /// THE ROW OF A "MORE" LIST WITH THE HINT `hint`: each "More" of the bars opened in turn and its rows rested over; a
    /// list without the row is closed again by its button, the one with it left open for the press that follows.
    fn hint_in_more(&mut self, hint: &str) -> Option<egui::Pos2> {
        for more in self.win.mores.clone() {
            let bar = self.win.plates.clone();
            self.press_screen(more);
            // A LIST LAID OUT IN ITS FIRST FRAME STANDS ELSEWHERE IN THE NEXT: it is placed before its size is known,
            // then set against its button. Measured: the rows of the first frame stood 300 points above the list. And the
            // pointer is taken off "More" to rest where nothing is, or the hint of "More" stays up over the first row and
            // egui opens no second one.
            let aside = egui::pos2(self.win.screen.x - 8.0, 10.0);
            self.frame(vec![egui::Event::PointerMoved(aside)]);
            self.win.clock += 1.0;
            for _ in 0..3 {
                self.frame(Vec::new());
            }
            let rows: Vec<egui::Pos2> = self.win.plates.iter().copied().filter(|p| !bar.contains(p)).collect();
            if let Some(at) = rows.into_iter().find(|&at| self.hint_comes_up(at, hint)) {
                return Some(at);
            }
            self.press_screen(more);
        }
        None
    }

    /// A REBUILD UNDER WAY REFUSES INPUT: its card lies over the window, nothing under it is pressed and no hint comes up.
    /// A person waits for the card to go before pressing or looking for anything; so does the hand, a minute at most.
    /// Measured in a run of the whole crate, the kernel held by the checks beside it: the card stood over the window
    /// for the whole search of a button, 70 buttons rested over and none answered.
    fn rebuild_gone(&mut self) {
        crate::gui::session::waiting::until("the card of the rebuild to go before a button is looked for", || {
            if self.app.regen.busy.is_none() {
                return true;
            }
            self.frame(Vec::new());
            false
        });
    }

    /// The button of `hint` looked for down the panel, a notch of the wheel at a time; it is remembered where found.
    fn hint_down_the_panel(&mut self, hint: &str) -> Option<egui::Pos2> {
        let mut scrolled = 0;
        loop {
            for at in self.win.plates.clone() {
                if self.hint_comes_up(at, hint) {
                    SEEN.with(|s| s.borrow_mut().insert(hint.to_string(), at));
                    return Some(at);
                }
            }
            if !self.panel_turned(-200.0) {
                return None; // the panel went no further: there is no such button below
            }
            scrolled += 1;
            if scrolled > 10 {
                return None;
            }
        }
    }

    /// The panel the buttons stand in turned back to its top.
    fn panel_to_its_top(&mut self) {
        for _ in 0..12 {
            if !self.panel_turned(200.0) {
                break;
            }
        }
    }

    /// The panel turned by a notch of the wheel, `by` points up (positive) or down; answers whether it moved.
    fn panel_turned(&mut self, by: f32) -> bool {
        let Some(over) = self.win.plates.first().copied() else { return false };
        let before = self.win.plates.clone();
        let wheel = egui::Event::MouseWheel { unit: egui::MouseWheelUnit::Point, delta: egui::vec2(0.0, by), phase: egui::TouchPhase::Move, modifiers: Default::default() };
        self.frame(vec![egui::Event::PointerMoved(over), wheel]);
        for _ in 0..10 {
            self.frame(vec![egui::Event::PointerMoved(over)]); // the scroll is smoothed over frames
        }
        self.win.plates != before
    }

    /// Does `hint` come up with the pointer resting over `at`? Only a hint that was not on screen before counts:
    /// the same words written elsewhere say nothing about this button.
    ///
    /// THREE FRAMES OF RESTING, measured: the hint of the button left behind closes a frame after the pointer
    /// leaves it, and egui opens no second hint while one is open; a hint that opens is laid out unseen in its
    /// first frame and drawn in the next.
    fn hint_comes_up(&mut self, at: egui::Pos2, hint: &str) -> bool {
        self.hint_at(at).iter().any(|t| t == hint)
    }

    /// THE HINT THAT COMES UP with the pointer resting over `at`: the words a frame of the rest draws more times than the
    /// frame before it did - a hint that repeats a line on screen is drawn a second time.
    pub fn hint_at(&mut self, at: egui::Pos2) -> Vec<String> {
        let before: Vec<String> = self.win.drawn.iter().map(|(t, _)| t.clone()).collect();
        self.frame(vec![egui::Event::PointerMoved(at)]);
        self.win.clock += 1.0;
        let mut came: Vec<String> = Vec::new();
        for _ in 0..3 {
            self.frame(Vec::new());
            let now: Vec<&String> = self.win.drawn.iter().map(|(t, _)| t).collect();
            for t in &now {
                let more = now.iter().filter(|d| **d == *t).count() > before.iter().filter(|b| *b == *t).count();
                if !t.is_empty() && more && !came.contains(*t) {
                    came.push(t.to_string());
                }
            }
        }
        came
    }

    /// A press and a release of the left button at a point of the screen, each in a frame of its own.
    fn press_screen(&mut self, at: egui::Pos2) {
        self.frame(vec![egui::Event::PointerMoved(at)]);
        let button = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        self.frame(vec![button(true)]);
        self.frame(vec![button(false)]);
    }

    /// CLICK A POINT OF THE SCREEN: a press and a release of the left button there, each in a frame of its own - a glyph
    /// or a caption that has no place in the sketch's own coordinates.
    pub fn click_screen(&mut self, at: egui::Pos2) -> &mut Self {
        self.press_screen(at);
        self
    }

    /// WHERE THE NUMBER FIELD NEAREST TO `near` STANDS, the next frame drawn - a field a person drags or types into.
    pub fn number_near(&mut self, near: egui::Pos2) -> Option<egui::Rect> {
        self.frame(Vec::new());
        self.win.widgets.iter().filter(|w| w.kind == super::window::Kind::Number).map(|w| w.rect).min_by(|a, b| a.center().distance(near).total_cmp(&b.center().distance(near)))
    }

    /// WHERE THE WINDOW TITLED `title` STANDS, the next frame drawn, as a screen reader is told about it: the whole
    /// window, its title bar included.
    pub fn window_titled(&mut self, title: &str) -> Option<egui::Rect> {
        self.frame(Vec::new());
        self.win.widgets.iter().find(|w| w.kind == super::window::Kind::Window && w.label == title).map(|w| w.rect)
    }

    /// CLOSE THE WINDOW TITLED `title` INSIDE THE PROGRAM with the cross on its title bar, as a person does when done with
    /// it. Answers whether such a window with a cross was on screen.
    pub fn close_titled(&mut self, title: &str) -> bool {
        let Some(window) = self.window_titled(title) else { return false };
        // the cross is the framework's own button, and it tells a screen reader what it does in these words
        let cross = self
            .win
            .widgets
            .iter()
            .find(|w| w.kind == super::window::Kind::Button && w.label == "Close window" && window.contains(w.rect.center()) && w.rect.center().y < window.min.y + 40.0)
            .map(|w| w.rect.center());
        cross.inspect(|at| self.press_screen(*at)).is_some()
    }

    /// DOUBLE-CLICK A POINT OF THE SCREEN: the hand rests over it, then presses and releases twice, each in a frame
    /// of its own - four sixtieths of a second, well inside the time egui allows a double click.
    pub fn double_click_screen(&mut self, at: egui::Pos2) -> &mut Self {
        self.win.clock += 1.0;
        self.press_screen(at);
        let button = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        self.frame(vec![button(true)]);
        self.frame(vec![button(false)])
    }

    /// PRESS THE TICK BEFORE `word` - the checkbox standing on the line of the words holding `word` nearest to
    /// `near`, the last one to the left of them, as the tick of a heading or a row of the tree stands. Answers whether
    /// such a tick was on screen.
    pub fn press_tick_before(&mut self, word: &str, near: egui::Pos2) -> bool {
        self.frame(Vec::new());
        let Some(line) = self.win.drawn.iter().filter(|(t, _)| t.contains(word)).map(|(_, r)| *r).min_by(|a, b| a.center().distance(near).total_cmp(&b.center().distance(near))) else {
            return false;
        };
        let Some(at) = self
            .win
            .widgets
            .iter()
            .filter(|w| w.kind == super::window::Kind::CheckBox && w.rect.center().y > line.min.y && w.rect.center().y < line.max.y && w.rect.max.x <= line.min.x + 1.0)
            .max_by(|a, b| a.rect.max.x.total_cmp(&b.rect.max.x))
            .map(|w| w.rect.center())
        else {
            return false;
        };
        self.win.clock += 1.0;
        self.press_screen(at);
        true
    }

    /// IS `word` WRITTEN ANYWHERE in the window, the next frame drawn.
    pub fn shows(&mut self, word: &str) -> bool {
        self.written_at(word).is_some()
    }

    /// A NOTCH OF THE WHEEL over a point of the sketch: up brings the sheet nearer, down takes it away, as by hand.
    pub fn wheel2d(&mut self, at: (f64, f64), up: bool) -> &mut Self {
        let over = self.screen2d(at);
        let delta = egui::vec2(0.0, if up { 120.0 } else { -120.0 });
        let wheel = egui::Event::MouseWheel { unit: egui::MouseWheelUnit::Point, delta, phase: egui::TouchPhase::Move, modifiers: Default::default() };
        self.frame(vec![egui::Event::PointerMoved(over)]); // the hand comes over the view first
        self.frame(vec![egui::Event::PointerMoved(over), wheel]);
        // the wheel is smoothed over frames
        for _ in 0..10 {
            self.frame(vec![egui::Event::PointerMoved(over)]);
        }
        self
    }

    /// EVERY WORD THE LAST FRAME WROTE, with where it stands.
    pub fn words_drawn(&self) -> &[(String, egui::Rect)] {
        &self.win.drawn
    }

    /// WHAT THE LAST FRAME DREW, shape by shape: a sign laid over the window is told by its look.
    pub fn shapes_drawn(&self) -> &[egui::epaint::ClippedShape] {
        &self.win.shapes
    }

    /// EVERY RUN OF WORDS THE NEXT FRAME PAINTS, with its place and its colour: a warning is told from information by
    /// the colour on screen, which is what a person sees, not by the field behind it.
    pub fn painted_words(&mut self) -> Vec<PaintedWords> {
        // words laid in a plate that has just come up are laid unseen in its first frame and shown from the next
        self.frame(Vec::new()).frame(Vec::new());
        painted_in(&self.win.shapes)
    }

    /// THE STATUS LINE AS THE NEXT FRAME PAINTS IT: the runs of words along the bottom of the window.
    pub fn status_painted(&mut self) -> Vec<PaintedWords> {
        let bottom = Self::SCREEN.y;
        self.painted_words().into_iter().filter(|w| w.rect.min.y > bottom - 40.0).collect()
    }

    /// THE BADGE OF `tone` IN THE STATUS LINE as the next frame paints it: its run of words - the icon of the tone and a
    /// number - or `None` when the line shows no such badge.
    pub fn status_badge(&mut self, tone: qymcad_ui_state::Tone) -> Option<PaintedWords> {
        let icon = qymcad_ui_state::tone_icon(tone)?;
        self.status_painted().into_iter().find(|w| w.text.strip_prefix(icon).is_some_and(|n| n.trim().parse::<usize>().is_ok()))
    }

    /// FRAMES UNTIL THE REBUILD A GESTURE STARTED HAS ENDED (`session::waiting::until`): a person waits for the card to go
    /// before reading the line.
    pub fn settle(&mut self) -> &mut Self {
        self.frame(Vec::new()); // the frame that notices the document changed and starts its rebuild
        self.rebuild_landed("the rebuild a gesture started to land");
        self.frame(Vec::new())
    }

    /// Frames until no rebuild runs and none is asked for.
    fn rebuild_landed(&mut self, what: &str) {
        crate::gui::session::waiting::until(what, || {
            if self.app.regen.busy.is_none() && !self.app.regen.wanted {
                return true;
            }
            self.run(egui::Modifiers::default(), Vec::new());
            false
        });
    }

    /// WHERE THE PANEL `key` STANDS, the next frame drawn; `None` when that frame does not draw it.
    pub fn laid_out(&mut self, key: &str) -> Option<egui::Rect> {
        self.frame(Vec::new());
        qymcad_shell::laid_out(&self.win.ctx, key)
    }

    /// THE GROUND `word` IS WRITTEN ON in the last frame, the place of it nearest to `near`: the smallest filled rectangle
    /// holding the words, and its colour.
    pub fn ground_of(&self, word: &str, near: egui::Pos2) -> Option<Ground> {
        fn grounds(s: &egui::Shape, out: &mut Vec<Ground>) {
            match s {
                egui::Shape::Rect(r) if r.fill.a() > 0 => out.push(Ground { rect: r.rect, fill: r.fill }),
                egui::Shape::Vec(v) => v.iter().for_each(|x| grounds(x, out)),
                _ => {}
            }
        }
        let text = self.win.drawn.iter().filter(|(t, _)| t == word).map(|(_, r)| *r).min_by(|a, b| a.center().distance(near).total_cmp(&b.center().distance(near)))?;
        let mut all = Vec::new();
        for cs in &self.win.shapes {
            grounds(&cs.shape, &mut all);
        }
        all.into_iter().filter(|g| g.rect.expand(0.5).contains_rect(text)).min_by(|a, b| a.rect.area().total_cmp(&b.rect.area()))
    }

    /// The size of the window, in points.
    pub fn screen(&self) -> egui::Vec2 {
        self.win.screen
    }

    /// THE BUTTONS OF THE BARS OF TOOLS the last frame drew.
    pub fn bar_buttons(&self) -> Vec<egui::Rect> {
        qymcad_shell::noted_buttons(&self.win.ctx)
    }

    /// WHERE `word` IS WRITTEN, the next frame drawn - the place written first, when it is written in several.
    pub fn written_at(&mut self, word: &str) -> Option<egui::Rect> {
        self.frame(Vec::new());
        self.win.drawn.iter().find(|(t, _)| t == word).map(|(_, r)| *r)
    }

    /// WHERE `word` IS WRITTEN NEAREST TO `near`, the next frame drawn - the same words may stand in a panel and in a
    /// window at once.
    pub fn written_near(&mut self, word: &str, near: egui::Pos2) -> Option<egui::Rect> {
        self.frame(Vec::new());
        self.win.drawn.iter().filter(|(t, _)| t == word).map(|(_, r)| *r).min_by(|a, b| a.center().distance(near).total_cmp(&b.center().distance(near)))
    }

    /// PRESS WHERE `word` IS WRITTEN - the one nearest to `near` when the frame wrote it in several places.
    /// Answers whether the word was on screen.
    pub fn press_word(&mut self, word: &str, near: egui::Pos2) -> bool {
        self.press_word_holding(word, near, egui::Modifiers::NONE)
    }

    /// WHAT THE TRIAL BUILD SAYS of the command as it stands, waited for the way a person waits for the preview: the
    /// words of its refusal, if it refuses. A trial runs on a worker; a check that ends without waiting for it leaves the
    /// kernel working while the process exits.
    pub fn trial_says(&mut self) -> Option<String> {
        let ctx = egui::Context::default();
        let mut said = None;
        crate::gui::session::waiting::until("the trial build of the command to answer", || match qymcad_part::trial_refusal(&mut self.app.part_ctx(), &ctx) {
            qymcad_part::Trial::Checking => false,
            qymcad_part::Trial::Refused(_, words) => {
                said = Some(words);
                true
            }
            qymcad_part::Trial::Clear => true,
        });
        said
    }

    /// DOUBLE-CLICK WHERE `word` IS WRITTEN - a row of the tree reopens its feature or enters its part this way. The
    /// icon drawn before a word by a font of its own is not part of the words on screen, so it is left out of `word`.
    /// Answers whether the word was on screen.
    pub fn double_click_word(&mut self, word: &str) -> bool {
        let word: String = word.chars().filter(|c| !('\u{e000}'..='\u{f8ff}').contains(c)).collect();
        let Some(at) = self.written_at(word.trim()) else { return false };
        self.double_click_screen(at.center());
        self.close_window();
        true
    }

    /// CTRL WITH `key`, pressed and released in whole frames; the window closes after it.
    pub fn ctrl(&mut self, key: egui::Key) -> &mut Self {
        self.chord(egui::Modifiers::COMMAND, key);
        self.close_window()
    }

    /// `key` WITH `modifiers` held, pressed and released in whole frames.
    pub fn chord(&mut self, modifiers: egui::Modifiers, key: egui::Key) -> &mut Self {
        let event = |pressed| egui::Event::Key { key, physical_key: None, pressed, repeat: false, modifiers };
        self.frame_holding(modifiers, vec![event(true)]);
        self.frame_holding(modifiers, vec![event(false)])
    }

    /// CTRL+Z.
    pub fn undo(&mut self) -> &mut Self {
        self.ctrl(egui::Key::Z)
    }

    /// AIM THE FLAT VIEW at `centre` of the sketch, at the scale the sketch tools use - what a person does by
    /// panning before clicking something near the edge. The 2D twin of `look_at`.
    pub fn look2d(&mut self, centre: (f64, f64)) -> &mut Self {
        self.app.viewing.view.scale = 6.0;
        self.app.viewing.view.center = super::Vec2::new(centre.0 as f32, centre.1 as f32);
        self.app.viewing.view.initialized = true;
        self
    }

    /// CLICK THE SKETCH CANVAS WITH THE MOUSE at a point of the sketch: the hand comes over the point, rests,
    /// presses and releases, each in a frame of its own.
    ///
    /// The rest is not decoration. egui takes two clicks less than 0.3 s apart for a DOUBLE click, and a double
    /// click on the canvas opens the editor of whatever lies under it - a pick and a base point clicked on one
    /// spot would do exactly that.
    pub fn mouse2d(&mut self, x: f64, y: f64) -> &mut Self {
        self.in_view2d(&[(x, y)]);
        self.press_at2d(egui::Modifiers::default(), (x, y))
    }

    /// THE RIGHT BUTTON ON A PLACE OF THE SKETCH: the hand rests over it, then presses and releases the right button,
    /// each in a frame of its own - the menu of the right button comes up there.
    pub fn right_click2d(&mut self, x: f64, y: f64) -> &mut Self {
        self.in_view2d(&[(x, y)]);
        let at = self.rest_over2d((x, y));
        let button = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Secondary, pressed, modifiers: Default::default() };
        self.frame(vec![button(true)]);
        self.frame(vec![button(false)]);
        self.frame(Vec::new())
    }

    /// The hand rests over a place already in view, then presses and releases with `modifiers` held.
    fn press_at2d(&mut self, modifiers: egui::Modifiers, place: (f64, f64)) -> &mut Self {
        let at = self.rest_over2d(place);
        let button = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers };
        self.frame_holding(modifiers, vec![button(true)]);
        self.frame_holding(modifiers, vec![button(false)])
    }

    /// PICK ITEMS OF THE SKETCH WITH THE MOUSE: the first with a click, every next one with Shift and a click, as
    /// a person adds to a selection. An item is a point or a curve. Answers whether a place to click was
    /// found for every one - found before the first click, so a hand that cannot reach the second item does not
    /// pick the first one either. The window closes after it.
    pub fn select2d(&mut self, items: &[qymcad_ui_state::SketchItem]) -> bool {
        let places = self.places_on2d(items);
        let all: Vec<(f64, f64)> = places.iter().flatten().copied().collect();
        let n = all.len().max(1) as f64;
        self.look2d(all.iter().fold((0.0, 0.0), |(sx, sy), (x, y)| (sx + x / n, sy + y / n)));
        self.in_view2d(&all);
        let spots: Option<Vec<(f64, f64)>> = items.iter().zip(&places).map(|(item, places)| places.iter().copied().find(|p| self.picks2d(*p) == Some(*item))).collect();
        let Some(spots) = spots else {
            self.close_window();
            return false;
        };
        for (k, spot) in spots.into_iter().enumerate() {
            let modifiers = if k == 0 { egui::Modifiers::default() } else { egui::Modifiers::SHIFT };
            self.press_at2d(modifiers, spot);
        }
        self.close_window();
        true
    }

    /// A PLACE TO CLICK ON AN ITEM of the sketch (a point, or a line or a circle), found as a person finds it: along
    /// the item until a click there would pick it. The view is aimed at the item first, at the scale of the
    /// sketch tools; the window closes after it.
    pub fn spot2d(&mut self, item: qymcad_ui_state::SketchItem) -> Option<(f64, f64)> {
        let places = self.places_on2d(&[item]).remove(0);
        let n = places.len().max(1) as f64;
        self.look2d(places.iter().fold((0.0, 0.0), |(sx, sy), (x, y)| (sx + x / n, sy + y / n)));
        self.in_view2d(&places);
        let spot = places.iter().copied().find(|p| self.picks2d(*p) == Some(item));
        self.close_window();
        spot
    }

    /// WHETHER A CLICK AT `place` WOULD PICK `item` - the hand looks where it is about to click and sees what lights up
    /// under the pointer. The place is brought into view first.
    pub fn would_pick2d(&mut self, place: (f64, f64), item: qymcad_ui_state::SketchItem) -> bool {
        self.in_view2d(&[place]);
        self.picks2d(place) == Some(item)
    }

    /// The places along each item a hand would try: a point is its own place, a line is tried from the middle
    /// outwards.
    fn places_on2d(&self, items: &[qymcad_ui_state::SketchItem]) -> Vec<Vec<(f64, f64)>> {
        let Some(si) = qymcad_ui_state::edit_si(&self.app.project, &self.app.sketch_ses) else { return items.iter().map(|_| Vec::new()).collect() };
        let sk = &self.app.project.sketches[si];
        let at = |id: u64| sk.points.iter().find(|p| p.id == id).map(|p| (p.x, p.y));
        items
            .iter()
            .map(|qymcad_ui_state::SketchItem { kind, id }| match kind {
                qymcad_ui_state::ItemKind::Point => at(*id).into_iter().collect(),
                _ => sk
                    .entities
                    .iter()
                    .find(|e| e.id == *id)
                    // a curve of any kind tried from its middle outwards
                    .and_then(|e| qymcad_core::model::curve::Curve::of(sk, e))
                    .map(|c| [0.5, 0.37, 0.63, 0.25, 0.75, 0.15, 0.85, 0.05, 0.95].iter().map(|t| c.at(*t)).map(|p| (p.x, p.y)).collect())
                    .unwrap_or_default(),
            })
            .collect()
    }

    /// WHAT A CLICK AT A PLACE WOULD PICK, as the hand sees it light up under the pointer. A label, a note, a
    /// constraint glyph and a dimension caption take a click before the geometry does, in the order the window
    /// asks them; any of those answers "not the item".
    fn picks2d(&mut self, place: (f64, f64)) -> Option<qymcad_ui_state::SketchItem> {
        let si = qymcad_ui_state::edit_si(&self.app.project, &self.app.sketch_ses)?;
        let (rect, pos) = (self.app.viewing.view_rect, self.screen2d(place));
        if qymcad_ui_state::text_at(&self.app.project, &self.app.viewing.view, rect, pos, si).is_some() || qymcad_ui_state::note_at(&self.app.project, &self.app.viewing.view, rect, pos, si).is_some()
        {
            return None;
        }
        let mut sk = self.app.sketch_ctx();
        if crate::gui::sketching::constraint_glyph_at(&mut sk, rect, pos, si).or_else(|| crate::gui::sketching::dim_at(&mut sk, rect, pos, si)).is_some() {
            return None;
        }
        crate::gui::sketching::sketch_hit(&self.app.pick_ctx(), rect, pos, si)
    }

    /// THE SKETCH IN FRONT OF THE HAND, with `places` in view.
    ///
    /// Looked at flat: `Hand::new` stands the view in 3D, and in a whole frame a click on a 3D view goes to the
    /// 3D viewport, while a person working on a sketch has the flat canvas in front of them. A place out of view
    /// is brought into it first, as a person scrolls to what they cannot see: the view is centred on the middle
    /// of the places at the same scale. The margin keeps them off the edge, where a press lands on the frame of
    /// the canvas.
    fn in_view2d(&mut self, places: &[(f64, f64)]) {
        self.app.viewing.mode_3d = false;
        self.frame(Vec::new()); // the canvas as this frame lays it out, with the view as it now stands
        self.rebuild_gone();
        let canvas = self.app.viewing.view_rect.shrink(40.0);
        if places.iter().any(|p| !canvas.contains(self.screen2d(*p))) {
            let n = places.len().max(1) as f64;
            let (cx, cy) = places.iter().fold((0.0, 0.0), |(sx, sy), (x, y)| (sx + x / n, sy + y / n));
            self.app.viewing.view.center = super::Vec2::new(cx as f32, cy as f32);
            self.frame(Vec::new());
        }
    }

    /// WHERE THE BOX OF THE CORNER TOOLS STANDS on screen, as the last frame laid it out; `None` while it is not up. The
    /// box of a set of corners, or the one of "fillet all", which is opened for the whole shape (point 0).
    pub fn corner_box(&self) -> Option<egui::Rect> {
        let si = qymcad_ui_state::edit_si(&self.app.project, &self.app.sketch_ses)?;
        [qymcad_ui_state::CORNER_SET, 0].into_iter().find_map(|pid| self.win.ctx.memory(|m| m.area_rect(egui::Id::new(("cornerinput", si, pid)))))
    }

    /// Where a place of the sketch stands on screen, on the canvas as the last frame laid it out.
    pub fn on_screen2d(&self, place: (f64, f64)) -> egui::Pos2 {
        self.screen2d(place)
    }

    fn screen2d(&self, (x, y): (f64, f64)) -> egui::Pos2 {
        (qymcad_ui_state::Sheet { view: self.app.viewing.view, rect: self.app.viewing.view_rect }).at(qymcad_core::geom::Point2::new(x, y))
    }

    /// The hand comes over a place and rests there a second - the pause between two gestures of a person - with
    /// two frames of hover, the second being the one the snap reads.
    /// THE HAND COMES OVER A POINT OF THE SKETCH AND RESTS there, pressing nothing: what stands lit under it is what
    /// a click would take.
    pub fn hover2d(&mut self, x: f64, y: f64) -> &mut Self {
        self.in_view2d(&[(x, y)]);
        self.rest_over2d((x, y));
        self
    }

    fn rest_over2d(&mut self, place: (f64, f64)) -> egui::Pos2 {
        let at = self.screen2d(place);
        self.win.clock += 1.0;
        self.frame(vec![egui::Event::PointerMoved(at)]);
        self.frame(vec![egui::Event::PointerMoved(at)]);
        at
    }

    /// DOUBLE-CLICK THE SKETCH CANVAS WITH THE MOUSE at a point of the sketch: the hand rests over the point,
    /// then presses and releases twice, each in a frame of its own - four sixtieths of a second, well inside the
    /// time egui allows a double click. The window closes after it.
    pub fn double_click2d(&mut self, x: f64, y: f64) -> &mut Self {
        self.mouse2d(x, y); // the first click, after the rest
        let at = self.screen2d((x, y));
        let button = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        self.frame(vec![button(true)]);
        self.frame(vec![button(false)]);
        self.close_window()
    }

    /// PRESS AND RELEASE A KEY, in whole frames.
    pub fn key(&mut self, key: egui::Key) -> &mut Self {
        let event = |pressed| egui::Event::Key { key, physical_key: None, pressed, repeat: false, modifiers: Default::default() };
        self.frame(vec![event(true)]);
        self.frame(vec![event(false)]);
        self.close_window() // a key is a gesture of whole frames: what its frames started is waited for, as with a click
    }

    /// TYPE `text` on the keyboard, into whatever field holds the focus.
    pub fn type_text(&mut self, text: &str) -> &mut Self {
        // a box opened by the last frame is laid unseen in it and takes the keys from the next, as a person sees it
        self.frame(Vec::new());
        self.frame(vec![egui::Event::Text(text.to_string())])
    }

    /// THE WINDOW CLOSES WITH THE GESTURE - every gesture made of whole frames ends here.
    ///
    /// A frame marks the program as running in a live window, and in a live window a rebuild goes into a
    /// thread that the next frame polls. The checks around a gesture drive the program without frames, so the
    /// hand waits for what its own frames started, as a person waits for the rebuild, and takes the mark away:
    /// a rebuild asked for afterwards has to happen where it is asked. Measured on the end-to-end run with the
    /// mark left on: 101 problems further down, every one a body of a later step that never got built.
    pub fn close_window(&mut self) -> &mut Self {
        self.rebuild_landed("the rebuild the frames of the hand started to land before the window closes");
        self.app.drain_bg_for_test();
        self.app.regen.ui_running = false;
        self
    }

    /// Aim the camera so that the whole body is visible — otherwise a click outside the frame means
    /// nothing.
    pub fn look_at(&mut self, target: [f64; 3], scale: f32) -> &mut Self {
        self.app.viewing.cam.target = target;
        self.app.viewing.cam.scale = scale;
        self
    }

    /// Press a tool button in the panel.
    pub fn tool(&mut self, kind: qymcad_ui_state::PartTool) -> &mut Self {
        self.app.start_feat_cmd(kind);
        // WAIT FOR THE LIVE B-rep TO BE PREPARED, the way a person waits for it. After a file is
        // opened the kernel comes up in the background, and until then the body has neither edges nor
        // faces: in the program a frame waits and shows an overlay, while a test without the wait would
        // decide the tool "cannot be clicked with".
        crate::gui::commands::refresh_edges(&mut self.app.part_ctx());
        self.app.drain_bg_for_test();
        qymcad_ui_state::rebuild_if_dirty(&mut self.app.rebuild_ctx());
        crate::gui::commands::refresh_edges(&mut self.app.part_ctx());
        self
    }

    /// CLICK A PLACE IN THE SCENE. The point is given in world coordinates — that way the test says
    /// "on this face" rather than "at these pixels"; the screen point is computed by the same
    /// projection the drawing uses.
    pub fn click(&mut self, world: [f64; 3]) -> &mut Self {
        let basis = self.app.viewing.cam.basis();
        let pos = qymcad_ui_state::Screen { cam: &self.app.viewing.cam, set: &self.app.set, rect: self.rect, basis: &basis }.at(world).0;
        crate::gui::commands::refresh_edges(&mut self.app.part_ctx()); // the same thing a frame does before accepting a click
        self.app.viewport_3d_click_at(pos, self.rect, &basis);
        self
    }

    /// THE RIGHT BUTTON ON A PLACE OF THE SCENE, given in world coordinates, pressed and released in whole frames -
    /// the menu of what is under it opens as it does by hand.
    pub fn right_click(&mut self, world: [f64; 3]) -> &mut Self {
        self.frame(Vec::new()); // the window lays the canvas out, where the point is then seen
        let basis = self.app.viewing.cam.basis();
        let at = qymcad_ui_state::Screen { cam: &self.app.viewing.cam, set: &self.app.set, rect: self.app.viewing.view_rect, basis: &basis }.at(world).0;
        self.win.clock += 1.0;
        self.frame(vec![egui::Event::PointerMoved(at)]);
        let button = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Secondary, pressed, modifiers: Default::default() };
        self.frame(vec![button(true)]);
        self.frame(vec![button(false)])
    }

    /// Type a number into a field of the command — the same as typing it in the popup at the
    /// geometry.
    pub fn set(&mut self, key: &str, v: f64) -> &mut Self {
        if let Some(p) = self.app.tools.cmd.params.iter_mut().find(|p| p.key == key) {
            p.val = v;
            p.txt = format!("{v}");
        }
        self
    }

    /// Type a formula into a field of the command, as into the popup at the geometry - a name of a parameter where
    /// `set` types a number.
    pub fn formula(&mut self, typed: Typed) -> &mut Self {
        if let Some(p) = self.app.tools.cmd.params.iter_mut().find(|p| p.key == typed.field) {
            p.txt = typed.text.to_string();
        }
        self
    }

    /// TURN THE PART to see a face that is not visible from the current side.
    ///
    /// That is what a person does: cannot click the bottom, so they turn the model. Without this part
    /// of the scenarios were out of reach of the checks, and a limitation of the checks was taken for a
    /// breakage of the program: the bottom face "could not be picked" simply because it was behind the
    /// part.
    pub fn orbit(&mut self, yaw: f64, pitch: f64) -> &mut Self {
        self.app.viewing.cam.yaw = yaw;
        self.app.viewing.cam.pitch = pitch;
        self
    }

    /// LOOK FROM BELOW — the commonest turn: there is no other way to get at the bottom of a part.
    pub fn look_from_below(&mut self) -> &mut Self {
        self.orbit(-0.7, -0.9)
    }

    /// THE 2D CANVAS READY FOR CLICKING, without touching what is in hand.
    ///
    /// `sk_tool` sets the view too, but it also puts a tool down - and with a tool in hand a drag belongs to
    /// the tool rather than to the drawing. Tests that check dragging need the view without the tool.
    pub fn canvas(app: &'a mut super::App) -> Self {
        let hand = Self::new(app);
        hand.app.viewing.mode_3d = false;
        hand.app.viewing.view.scale = 6.0;
        hand.app.viewing.view.center = super::Vec2::new(0.0, 0.0);
        hand.app.viewing.view.initialized = true;
        hand
    }

    /// TAKE DRAWING TOOL `t` OF A SKETCH by its button in the panel. The button is found by its hint and pressed, and it
    /// acts as a button does: a tool already in hand is put down by it. The view is the flat canvas at the scale the
    /// sketch checks draw at. The arrow that puts a tool down is `sk_select`.
    ///
    /// The panel holds the sketch tools only while a sketch is open for editing, so a check that has not opened
    /// one fails here, naming the hint it looked for.
    pub fn sk_tool(&mut self, t: qymcad_ui_state::SketchShape) -> &mut Self {
        use qymcad_ui_state::SketchShape as S;
        self.flat_canvas();
        let key = match t {
            S::Line => "tb-line-hint",
            S::Rect => "tb-rect-hint",
            S::Circle => "tb-circle-hint",
            S::Arc => "tb-arc-hint",
            S::Point => "tb-point-hint",
            S::Polygon => "tb-polygon-hint",
            S::Slot => "tb-slot-hint",
            S::Ellipse => "tb-ellipse-hint",
            S::Spline => "tb-spline-hint",
            S::Text => "tb-text-hint",
        };
        self.press_hint_or_fail(key)
    }

    /// Press the button of the hint under `key` and close the window, or fail naming the hint.
    fn press_hint_or_fail(&mut self, key: &str) -> &mut Self {
        let hint = qymcad_i18n::tr(key);
        assert!(self.press_hint(&hint), "no button with the hint {hint:?} in the window - is the sketch open for editing?");
        self.close_window()
    }

    /// CLICK THE SKETCH CANVAS in its own coordinates — that way the test says "right here" rather
    /// than "at these pixels". A click of the mouse in whole frames of the window (`mouse2d`), and the window
    /// closes after it.
    pub fn click2d(&mut self, x: f64, y: f64) -> &mut Self {
        self.mouse2d(x, y);
        // a box the click opened is drawn from the next frame, laid unseen in it and shown from the one after, as a
        // person sees it
        self.frame(Vec::new()).frame(Vec::new());
        self.close_window()
    }

    /// SELECTION MODE in a sketch — the arrow of the panel, which puts down the drawing tool. The view is the flat
    /// canvas, as for every tool of the panel.
    pub fn sk_select(&mut self) -> &mut Self {
        self.flat_canvas();
        self.press_hint_or_fail("tb-select-hint")
    }

    /// THE FLAT CANVAS at the scale the sketch checks draw at: the panel of a sketch is pressed over it.
    fn flat_canvas(&mut self) {
        self.app.viewing.mode_3d = false;
        self.app.viewing.view.scale = 6.0;
        self.app.viewing.view.center = super::Vec2::new(0.0, 0.0);
        self.app.viewing.view.initialized = true;
    }

    /// TAKE THE MOVE TOOL by its button in the panel: move, copy or rotate.
    ///
    /// ITS DOOR IS CLICKS, NOT A DRAG: one click picks the shape, one sets the base point, one sets the
    /// target (for a rotation the angle is typed in the popup instead). A drag with this tool in hand is not its
    /// door: `drag2d` pulls a POINT, and the tool takes no part in it.
    pub fn sk_move_tool(&mut self, op: qymcad_ui_state::MoveKind) -> &mut Self {
        self.press_hint_or_fail(match op {
            qymcad_ui_state::MoveKind::Move => "tb-move-hint",
            qymcad_ui_state::MoveKind::Copy => "tb-copy-hint",
            qymcad_ui_state::MoveKind::Rotate => "tb-rotate-hint",
        })
    }

    /// THE SEGMENTS OF `colour` THE LAST FRAME DREW, where they stand on screen: a dashed line is drawn as its dashes.
    pub fn segments_in(&self, colour: egui::Color32) -> Vec<[egui::Pos2; 2]> {
        fn walk(s: &egui::Shape, colour: egui::Color32, out: &mut Vec<[egui::Pos2; 2]>) {
            match s {
                egui::Shape::LineSegment { points, stroke } if stroke.color == colour => out.push(*points),
                egui::Shape::Vec(v) => v.iter().for_each(|x| walk(x, colour, out)),
                _ => {}
            }
        }
        let mut out = Vec::new();
        self.win.shapes.iter().for_each(|cs| walk(&cs.shape, colour, &mut out));
        out
    }

    /// HOW MANY STROKES OF `colour` THE FRAME DRAWS NEAR THE POINTER, the hand resting over `place` of the sketch: a
    /// path or a segment with a point within 40 px of it. What follows the pointer is drawn there.
    pub fn strokes_near2d(&mut self, place: (f64, f64), colour: egui::Color32) -> usize {
        fn near(s: &egui::Shape, at: egui::Pos2, colour: egui::Color32) -> usize {
            match s {
                egui::Shape::Path(p) if p.stroke.color == egui::epaint::ColorMode::Solid(colour) && p.points.iter().any(|q| q.distance(at) < 40.0) => 1,
                egui::Shape::LineSegment { points, stroke } if stroke.color == colour && points.iter().any(|q| q.distance(at) < 40.0) => 1,
                egui::Shape::Vec(v) => v.iter().map(|x| near(x, at, colour)).sum(),
                _ => 0,
            }
        }
        // on the sheet of the sketch, as the other `...2d` hands are: a new hand starts in the 3D view
        self.app.viewing.mode_3d = false;
        let at = self.rest_over2d(place);
        let n = self.win.shapes.iter().map(|cs| near(&cs.shape, at, colour)).sum();
        self.close_window();
        n
    }

    /// SHIFT AND A CLICK on the sketch: what is under the pointer joins the selection, as a person adds to one. The
    /// window closes after it.
    pub fn shift_click2d(&mut self, x: f64, y: f64) -> &mut Self {
        self.press_at2d(egui::Modifiers::SHIFT, (x, y));
        self.close_window()
    }

    /// TURN WHAT IS SELECTED ALREADY by `deg` about `centre`: the tool taken with the selection standing, the click that
    /// sets the centre, the angle typed into the popup and Enter.
    pub fn sk_rotate_selected(&mut self, centre: (f64, f64), deg: f64) -> &mut Self {
        self.sk_move_tool(qymcad_ui_state::MoveKind::Rotate);
        self.mouse2d(centre.0, centre.1);
        self.frame(Vec::new());
        self.type_text(&format!("{deg}"));
        self.key(egui::Key::Enter);
        self.close_window()
    }

    /// MOVE OR COPY THE SHAPE lying under `on`, from `from` to `to` - the tool, then its three
    /// clicks, in the order a person makes them and through the whole frame of the window.
    pub fn sk_move(&mut self, op: qymcad_ui_state::MoveKind, on: (f64, f64), from: (f64, f64), to: (f64, f64)) -> &mut Self {
        self.sk_select(); // put down whatever was in hand: the canvas is in view and the sketch is the selection
        self.look2d(((on.0 + to.0) / 2.0, (on.1 + to.1) / 2.0)); // the shape and where it goes, both in view
        self.drop_sketch_selection();
        self.sk_move_tool(op);
        self.mouse2d(on.0, on.1); // pick what is being moved
        self.mouse2d(from.0, from.1); // the base point
        self.mouse2d(to.0, to.1); // where it goes
        self.close_window()
    }

    /// TURN THE SHAPE lying under `on` by `deg` about `centre`: the tool, the click that picks the shape, the
    /// click that sets the centre, then the angle typed into the popup and Enter - all of it keys and clicks in
    /// whole frames.
    ///
    /// The popup is where the turn is applied, so it is what the hand types into. Setting the centre asks the
    /// field for the focus with its text selected; a frame gives it, the digits replace the text, and Enter in
    /// that field applies the rotation.
    pub fn sk_rotate(&mut self, on: (f64, f64), centre: (f64, f64), deg: f64) -> &mut Self {
        self.sk_select();
        self.look2d(((on.0 + centre.0) / 2.0, (on.1 + centre.1) / 2.0));
        self.drop_sketch_selection();
        self.sk_move_tool(qymcad_ui_state::MoveKind::Rotate);
        self.mouse2d(on.0, on.1); // pick what is being turned
        self.mouse2d(centre.0, centre.1); // the centre of the turn - the popup opens here
        self.frame(Vec::new()); // the field takes the focus
        self.type_text(&format!("{deg}"));
        self.key(egui::Key::Enter);
        self.close_window()
    }

    /// NAME A SET OF CORNERS AND CUT THEM ALL WITH ONE ANSWER: the corner tool by its button, then a click with
    /// Shift on each of the lines, then the value typed into the field that stands up at the first corner, and
    /// Enter - every one of it a key or a click in a whole frame of the window.
    ///
    /// `lines` are `(1, id)` items of the sketch. A place to click is found for each of them BEFORE the first click
    /// and looked up again before every one of the next, because the field stands where the first corner was named
    /// and its buttons take a click before a line does. Answers whether a place was found for all of them: a hand
    /// that cannot reach the third line must not have taken the tool either, since it would be standing over half
    /// a contour.
    pub fn sk_corner_set(&mut self, key: &str, lines: &[qymcad_ui_state::SketchItem], value: f64) -> bool {
        self.sk_corner_set_in(key, None, lines, value)
    }

    /// THE SAME IN A MODE OF THE TOOL: after the tool is taken, the word of the mode on its bar is pressed - "two
    /// distances", "chord" - as a person picks it before naming the corners.
    pub fn sk_corner_set_in(&mut self, key: &str, mode: Option<&str>, lines: &[qymcad_ui_state::SketchItem], value: f64) -> bool {
        if !self.sk_corner_typed(key, mode, lines, value) {
            return false;
        }
        self.key(egui::Key::Enter);
        self.close_window();
        true
    }

    /// THE SAME UP TO THE VALUE TYPED, and no Enter: the tool stands with its set named and the value in its field,
    /// as a person looks at the preview before answering. The window stays open.
    pub fn sk_corner_typed(&mut self, key: &str, mode: Option<&str>, lines: &[qymcad_ui_state::SketchItem], value: f64) -> bool {
        let places = self.places_on2d(lines);
        let all: Vec<(f64, f64)> = places.iter().flatten().copied().collect();
        let n = all.len().max(1) as f64;
        self.sk_select(); // the canvas is in view and the sketch is the selection, whatever was in hand
        self.look2d(all.iter().fold((0.0, 0.0), |(sx, sy), (x, y)| (sx + x / n, sy + y / n)));
        self.in_view2d(&all);
        if !lines.iter().zip(&places).all(|(item, places)| places.iter().any(|p| self.picks2d(*p) == Some(*item))) {
            self.close_window();
            return false;
        }
        self.press_hint_or_fail(key);
        if let Some(word) = mode {
            assert!(self.press_word(&crate::i18n::tr(word), egui::Pos2::ZERO), "the mode {word} is not on the bar of the tool");
        }
        for (item, places) in lines.iter().zip(&places) {
            // THE FIELD OF THE CORNER ALREADY NAMED STANDS WHERE IT WAS NAMED, and a click on its buttons answers
            // the field rather than joining the set - so the place is looked up again now, not only before the tool
            // was taken. The line goes in the set or the hand does not go on.
            let Some(spot) = places.iter().copied().find(|p| self.picks2d(*p) == Some(*item)) else {
                self.close_window();
                return false;
            };
            self.press_at2d(egui::Modifiers::SHIFT, spot);
        }
        self.frame(Vec::new()); // the field takes the focus
        self.type_text(&format!("{value}"));
        true
    }

    /// THE FIELDS OF THE TOP BAR THAT HAVE NO CAPTION BEFORE THEM, the next frame drawn: a field above the canvas with no
    /// words to its left on the same row, or with another field between it and the nearest words.
    pub fn bar_fields_without_caption(&mut self) -> Vec<egui::Rect> {
        self.frame(Vec::new());
        let top = self.app.viewing.view_rect.top();
        let field = |k: &super::window::Kind| matches!(k, super::window::Kind::TextField | super::window::Kind::Number);
        let fields: Vec<egui::Rect> = self.win.widgets.iter().filter(|w| field(&w.kind) && w.rect.bottom() <= top).map(|w| w.rect).collect();
        let row = |a: &egui::Rect, b: &egui::Rect| (a.center().y - b.center().y).abs() < 6.0;
        fields
            .iter()
            .filter(|f| {
                let caption = self
                    .win
                    .drawn
                    .iter()
                    .filter(|(t, r)| !t.trim().is_empty() && t.trim().parse::<f64>().is_err() && row(r, f) && r.right() <= f.left() + 1.0)
                    .map(|(_, r)| r.right())
                    .fold(f32::NEG_INFINITY, f32::max);
                caption.is_infinite() || fields.iter().any(|g| g != *f && row(g, f) && g.left() >= caption && g.right() <= f.left() + 1.0)
            })
            .copied()
            .collect()
    }

    /// TYPE `text` INTO THE FIELD THAT STANDS RIGHT AFTER `word`, all of what it held replaced, with no Enter. Answers
    /// whether the word and a field after it were on screen.
    pub fn type_after_word(&mut self, word: &str, text: &str) -> bool {
        // a box opened by the last frame is laid unseen in it and shown from the next, as a person sees it
        self.frame(Vec::new());
        let Some(at) = self.written_at(word) else { return false };
        let field = |k: &super::window::Kind| matches!(k, super::window::Kind::TextField | super::window::Kind::Number);
        let Some(f) = self
            .win
            .widgets
            .iter()
            .filter(|w| field(&w.kind) && w.rect.left() >= at.right() - 1.0 && (w.rect.center().y - at.center().y).abs() < 6.0)
            .map(|w| w.rect)
            .min_by(|a, b| a.left().total_cmp(&b.left()))
        else {
            return false;
        };
        self.press_screen(f.center());
        self.chord(egui::Modifiers::COMMAND, egui::Key::A);
        self.type_text(text);
        true
    }

    /// TYPE `value` INTO THE FIELD SHOWING `hint` in grey while empty - the first such in reading order - all of what it
    /// held replaced, with no Enter. Answers whether such a field was on screen.
    pub fn type_into_hinted(&mut self, hint: &str, value: f64) -> bool {
        self.frame(Vec::new());
        let Some(f) = self
            .win
            .widgets
            .iter()
            .filter(|w| w.kind == super::window::Kind::TextField && w.placeholder == hint)
            .map(|w| w.rect)
            .min_by(|a, b| a.min.y.total_cmp(&b.min.y).then(a.min.x.total_cmp(&b.min.x)))
        else {
            return false;
        };
        self.press_screen(f.center());
        self.chord(egui::Modifiers::COMMAND, egui::Key::A);
        self.type_text(&format!("{value}"));
        true
    }

    /// THE SMALL RINGS OF `radius` px THE LAST FRAME DREW, where they stand on screen: the marks a preview puts where a
    /// line will be cut.
    pub fn rings_drawn(&self, radius: f32) -> Vec<egui::Pos2> {
        self.circles_drawn(radius, |_| true)
    }

    /// THE FILLED DOTS OF `radius` px THE LAST FRAME DREW, where they stand on screen: the points of a sketch, and not
    /// the outlined marker of the origin, which is a ring of the same 3.5 px.
    pub fn dots_drawn(&self, radius: f32) -> Vec<egui::Pos2> {
        self.circles_drawn(radius, |c| c.fill != egui::Color32::TRANSPARENT)
    }

    /// THE BADGES IN `colour` THE LAST FRAME DREW, by their centres: the filled squares of 14 px a constraint is shown in,
    /// the live preview of a tie that a drawing tool is about to lay among them.
    pub fn badges_in(&self, colour: egui::Color32) -> Vec<egui::Pos2> {
        fn walk(s: &egui::Shape, colour: egui::Color32, out: &mut Vec<egui::Pos2>) {
            match s {
                egui::Shape::Rect(r) if r.fill == colour && (r.rect.width() - 14.0).abs() < 0.5 && (r.rect.height() - 14.0).abs() < 0.5 => out.push(r.rect.center()),
                egui::Shape::Vec(v) => v.iter().for_each(|x| walk(x, colour, out)),
                _ => {}
            }
        }
        let mut out = Vec::new();
        self.win.shapes.iter().for_each(|cs| walk(&cs.shape, colour, &mut out));
        out
    }

    /// THE FILLED DOTS OF `radius` px IN `colour` THE LAST FRAME DREW: the points of a sketch drawn free, or defined.
    pub fn dots_in(&self, radius: f32, colour: egui::Color32) -> Vec<egui::Pos2> {
        fn walk(s: &egui::Shape, dot: &egui::epaint::CircleShape, out: &mut Vec<egui::Pos2>) {
            match s {
                egui::Shape::Circle(c) if (c.radius - dot.radius).abs() < 1e-3 && c.fill == dot.fill => out.push(c.center),
                egui::Shape::Vec(v) => v.iter().for_each(|x| walk(x, dot, out)),
                _ => {}
            }
        }
        let dot = egui::epaint::CircleShape::filled(egui::Pos2::ZERO, radius, colour);
        let mut out = Vec::new();
        self.win.shapes.iter().for_each(|cs| walk(&cs.shape, &dot, &mut out));
        out
    }

    fn circles_drawn(&self, radius: f32, takes: fn(&egui::epaint::CircleShape) -> bool) -> Vec<egui::Pos2> {
        fn walk(s: &egui::Shape, radius: f32, takes: fn(&egui::epaint::CircleShape) -> bool, out: &mut Vec<egui::Pos2>) {
            match s {
                egui::Shape::Circle(c) if (c.radius - radius).abs() < 1e-3 && takes(c) => out.push(c.center),
                egui::Shape::Vec(v) => v.iter().for_each(|x| walk(x, radius, takes, out)),
                _ => {}
            }
        }
        let mut out = Vec::new();
        self.win.shapes.iter().for_each(|cs| walk(&cs.shape, radius, takes, &mut out));
        out
    }

    /// A LEFTOVER SELECTION IS DROPPED WITH Esc before a shape is picked, as a person drops it: the move tool
    /// works on what is already selected, and with something selected its first click would set the base point
    /// rather than pick.
    ///
    /// Only when something IS selected - with nothing selected the same key goes down the ladder and leaves the
    /// sketch unselected.
    fn drop_sketch_selection(&mut self) {
        if !self.app.tools.sel_sk.items.is_empty() {
            self.key(egui::Key::Escape);
        }
    }

    /// PRESS A CONSTRAINT BUTTON of the panel, found by its hint - the hint its button carries.
    pub fn constraint(&mut self, code: qymcad_ui_state::ConstraintKind) -> &mut Self {
        self.press_hint_or_fail(qymcad_sketch::constraint_hint_key(code))
    }

    /// COPY THE SELECTION WITH Ctrl+C - egui turns the keys into a copy event, and that is what the window
    /// reads.
    pub fn copy(&mut self) -> &mut Self {
        self.frame_holding(egui::Modifiers::COMMAND, vec![egui::Event::Copy]);
        self.close_window()
    }

    /// Ctrl+X, as egui hands it over: an event of its own.
    pub fn cut(&mut self) -> &mut Self {
        self.frame_holding(egui::Modifiers::COMMAND, vec![egui::Event::Cut]);
        self.close_window()
    }

    /// Ctrl+V, AS egui HANDS IT OVER IN A LIVE WINDOW: a paste event with the text of the system clipboard, and only when
    /// that clipboard holds some - with it empty the keys bring nothing at all. The system clipboard of the hand is what
    /// the program put there (`Window::copied`). Reported behaviour: "Ctrl+V does not paste (Edit -> Paste works)" - a
    /// copy of sketch geometry left the system clipboard empty, and a paste event handed over regardless hid it.
    ///
    /// A FRAME GOES BY FIRST, as in a live window, which draws on between a click and the next key: what a click asked of
    /// the system is handed over in the frame after it.
    pub fn paste(&mut self) -> &mut Self {
        self.frame(Vec::new());
        let events = self.win.copied.last().filter(|t| !t.is_empty()).map(|t| vec![egui::Event::Paste(t.clone())]).unwrap_or_default();
        self.frame_holding(egui::Modifiers::COMMAND, events);
        self.close_window()
    }

    /// DRAG WITH THE MOUSE from one place of the sketch to another: press, lead, release, in whole frames, and
    /// the window closes after it. What is taken is what the window takes - a point, a dimension, a label, or
    /// nothing and a band of selection.
    ///
    /// THE HAND LEADS IN STEPS OF 3 px. egui calls it a drag once the pointer is about 6 px from where it was
    /// pressed, and what is taken is looked for under the pointer AT THAT MOMENT, not under the press: a hand
    /// that jumped 40 px in one frame would start its drag with the point already out of reach, which is not what
    /// a person's hand does.
    pub fn drag2d(&mut self, from: (f64, f64), to: (f64, f64)) -> &mut Self {
        self.drag2d_held(Drag2d { from, to, button: egui::PointerButton::Primary, modifiers: egui::Modifiers::NONE })
    }

    /// THE SAME DRAG WITH `drag.button`, the keys of `drag.modifiers` held through every frame of it.
    pub fn drag2d_held(&mut self, drag: Drag2d) -> &mut Self {
        let Drag2d { from, to, button, modifiers } = drag;
        self.in_view2d(&[from, to]);
        let a = self.rest_over2d(from);
        let b = self.screen2d(to);
        let press = |pos, pressed| egui::Event::PointerButton { pos, button, pressed, modifiers };
        self.frame_holding(modifiers, vec![press(a, true)]);
        let steps = ((b - a).length() / 3.0).ceil().max(1.0) as usize;
        for k in 1..=steps {
            self.frame_holding(modifiers, vec![egui::Event::PointerMoved(a + (b - a) * (k as f32 / steps as f32))]);
        }
        self.frame_holding(modifiers, vec![press(b, false)]);
        self.close_window()
    }

    /// A DRAG NOT LET GO: the left button pressed at `from` and led to `to` in steps of 3 px, still held - what the
    /// window shows in the middle of a drag. `release2d` ends it.
    pub fn drag2d_begun(&mut self, from: (f64, f64), to: (f64, f64)) -> &mut Self {
        self.in_view2d(&[from, to]);
        let a = self.rest_over2d(from);
        let b = self.screen2d(to);
        self.frame(vec![egui::Event::PointerButton { pos: a, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE }]);
        let steps = ((b - a).length() / 3.0).ceil().max(1.0) as usize;
        for k in 1..=steps {
            self.frame(vec![egui::Event::PointerMoved(a + (b - a) * (k as f32 / steps as f32))]);
        }
        self
    }

    /// LET GO of the left button at `at`, a drag begun by `drag2d_begun` ends there.
    pub fn release2d(&mut self, at: (f64, f64)) -> &mut Self {
        let pos = self.screen2d(at);
        self.frame(vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE }])
    }

    /// DRAG WITH THE MOUSE from one point of the scene to another, given in world coordinates and seen through the
    /// canvas the last frame laid out: press, lead in steps of 3 px, release, in whole frames. What is taken is what
    /// the window takes - a handle of a tool, a gizmo, or nothing and the camera turns.
    pub fn drag3d(&mut self, from: [f64; 3], to: [f64; 3]) -> &mut Self {
        self.frame(Vec::new());
        let basis = self.app.viewing.cam.basis();
        let scr = qymcad_ui_state::Screen { cam: &self.app.viewing.cam, set: &self.app.set, rect: self.app.viewing.view_rect, basis: &basis };
        let (a, b) = (scr.at(from).0, scr.at(to).0);
        self.drag_screen(a, b)
    }

    /// DRAG WITH THE MOUSE from one point of the screen to another: the hand comes over `from`, presses, leads in
    /// steps of 3 px and releases, in whole frames. What is taken is what lies under `from` - the title of a
    /// window, a handle, or the canvas.
    pub fn drag_screen(&mut self, from: egui::Pos2, to: egui::Pos2) -> &mut Self {
        self.carry(Carry { from, to }).let_go(to)
    }

    /// CARRY WITH THE MOUSE from `from` to `to` and hold there, the button still down: the frames show what is
    /// carried under the pointer. `drag_screen` is this and the release.
    pub fn carry(&mut self, carry: Carry) -> &mut Self {
        let Carry { from, to } = carry;
        self.win.clock += 1.0;
        self.frame(vec![egui::Event::PointerMoved(from)]);
        self.frame(vec![Self::primary_down(from)]);
        let steps = ((to - from).length() / 3.0).ceil().max(1.0) as usize;
        for k in 1..=steps {
            self.frame(vec![egui::Event::PointerMoved(from + (to - from) * (k as f32 / steps as f32))]);
        }
        self
    }

    /// Let the primary button go at `at`, after `carry`.
    pub fn let_go(&mut self, at: egui::Pos2) -> &mut Self {
        self.frame(vec![egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() }])
    }

    /// The primary button going down at `pos`.
    fn primary_down(pos: egui::Pos2) -> egui::Event {
        egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() }
    }

    /// A PICTURE OF THE WINDOW as the last frame drew it, a pixel to a point - the size the letters of the frame were
    /// drawn for - cut to `area`: the pictures of the help that show a gesture are taken from the frames it made.
    pub fn picture(&self, area: egui::Rect) -> egui::ColorImage {
        let area = area.intersect(egui::Rect::from_min_size(egui::Pos2::ZERO, self.win.screen));
        let size = [self.win.screen.x as usize, self.win.screen.y as usize];
        let prims = self.win.ctx.tessellate(self.win.shapes.clone(), 1.0);
        let whole = super::help_raster::paint(&prims, &self.win.textures, size, egui::Color32::BLACK, 1.0);
        let [x0, y0] = [area.min.x as usize, area.min.y as usize];
        let [w, h] = [area.width() as usize, area.height() as usize];
        let pixels = (y0..y0 + h).flat_map(|y| (x0..x0 + w).map(move |x| y * size[0] + x)).map(|i| whole.pixels[i]).collect();
        egui::ColorImage::new([w, h], pixels)
    }

    /// WHERE THE AREA `id` STANDS, as the last frame laid it out: a window or a box of fields at the geometry.
    pub fn area_rect(&self, id: egui::Id) -> Option<egui::Rect> {
        self.win.ctx.memory(|m| m.area_rect(id))
    }

    /// THE ID OF THE BOX OF THE TOOL IN HAND, as the frame of every tool lays it; none while the tool has no box.
    pub fn tool_box_id(&self) -> Option<egui::Id> {
        qymcad_ui_state::tool_box_id(&self.win.ctx)
    }

    /// IS ANY TEXT OF A LABEL SELECTED after the last frame: what a drag across words leaves selected.
    pub fn text_selected(&self) -> bool {
        self.win.ctx.plugin_opt::<egui::text_selection::LabelSelectionState>().is_some_and(|h| h.lock().has_selection())
    }

    /// THE LAYER ON TOP AT `at` in the last frame: what a click there reaches first.
    pub fn top_layer_at(&self, at: egui::Pos2) -> Option<egui::LayerId> {
        self.win.ctx.layer_id_at(at)
    }

    /// Where the window `id` stands, as the last frame laid it out.
    pub fn window_at(&self, id: &str) -> Option<egui::Rect> {
        self.win.ctx.memory(|m| m.area_rect(egui::Id::new(id)))
    }

    /// A DRAG OF THE VIEW as a person makes it: the hand comes over `from`, presses `button` with `modifiers` held, leads
    /// in steps of 3 px to `to` and lets go, in whole frames. Which button and which keys turn or move the view is the
    /// layout's to say (`MouseNav::rotates`, `MouseNav::pans`).
    pub fn drag_view(&mut self, drag: ViewDrag) -> &mut Self {
        let ViewDrag { button, modifiers, from, to } = drag;
        self.win.clock += 1.0;
        self.frame_holding(modifiers, vec![egui::Event::PointerMoved(from)]);
        let press = |pos, pressed| egui::Event::PointerButton { pos, button, pressed, modifiers };
        self.frame_holding(modifiers, vec![press(from, true)]);
        let steps = ((to - from).length() / 3.0).ceil().max(1.0) as usize;
        for k in 1..=steps {
            self.frame_holding(modifiers, vec![egui::Event::PointerMoved(from + (to - from) * (k as f32 / steps as f32))]);
        }
        self.frame_holding(modifiers, vec![press(to, false)]);
        self.frame(Vec::new())
    }

    /// NOTCHES OF THE WHEEL over a place of the 3D view: forward (`notches` above zero) brings the model nearer.
    pub fn wheel3d(&mut self, over: egui::Pos2, notches: i32) -> &mut Self {
        self.frame(vec![egui::Event::PointerMoved(over)]); // the hand comes over the view first
        for _ in 0..notches.unsigned_abs() {
            let delta = egui::vec2(0.0, 120.0 * notches.signum() as f32);
            let wheel = egui::Event::MouseWheel { unit: egui::MouseWheelUnit::Point, delta, phase: egui::TouchPhase::Move, modifiers: Default::default() };
            self.frame(vec![egui::Event::PointerMoved(over), wheel]);
            // the wheel is smoothed over frames
            for _ in 0..10 {
                self.frame(vec![egui::Event::PointerMoved(over)]);
            }
        }
        self
    }

    /// Whether a text field holds the keyboard - a caret blinking in it.
    pub fn typing(&self) -> bool {
        self.win.ctx.egui_wants_keyboard_input()
    }

    /// WHAT THE OPERATION DOES TO THE BODY: 0 add, 2 cut — the same as the switch in the top bar of
    /// the extrude.
    pub fn op(&mut self, op: u8) -> &mut Self {
        self.app.feat.op = op;
        self
    }

    /// TAKE THE JOINT TOOL AND CHOOSE THE KIND — the same door the workbench button and the `J` key
    /// use.
    ///
    /// The hand had NO assembly actions at all, and that cost dearly: the mates workbench was never
    /// once touched the way a person touches it. The checks called `add_joint` directly and therefore
    /// saw neither that a click on a part was declared a miss nor that an anchor on an edge was born
    /// dead — both were found by hand in five minutes.
    pub fn mate(&mut self, kind: qymcad_core::feature::JointKind) -> &mut Self {
        self.app.workbench = super::Workbench::Assembly;
        self.app.viewing.mode_3d = true;
        self.app.side.joint.new_kind = kind;
        self.app.arm_joint_pick_for_test();
        crate::gui::commands::refresh_edges(&mut self.app.part_ctx());
        self
    }

    /// THE ANCHOR MODE — the switch in the assembling bar: 0 face, 1 edge, 2 vertex, 3 origin of the
    /// part.
    ///
    /// Taken AFTER the tool: taking the tool sets the mode by the kind of joint (an edge for the
    /// coaxial ones, a face for the rest), and a person's choice must lie on top.
    pub fn anchor(&mut self, mode: u8) -> &mut Self {
        qymcad_assembly::set_joint_anchor_mode_for_test(&mut self.app.joint_ctx(), mode);
        self
    }

    /// Enter applies the command.
    pub fn enter(&mut self) -> &mut Self {
        self.app.apply_feat_cmd();
        qymcad_ui_state::rebuild_if_dirty(&mut self.app.rebuild_ctx());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::Hand;
    use super::super::App;

    /// THE HAND REALLY WORKS AS A HAND: tool button -> click on an edge -> Enter -> a fillet in the
    /// timeline.
    ///
    /// Not one reach into a field: had the click missed, or had the click handling not understood that
    /// the edge tool was open, no node would have appeared — and the test would show that, as a person
    /// would.
    #[test]
    fn a_fillet_is_made_by_clicking_like_a_person() {
        let mut app = App::default();
        super::super::joint_flow::tests::add_part_at(&mut app, 0.0);
        qymcad_ui_state::rebuild_if_dirty(&mut app.rebuild_ctx());
        let body = app.project.mesh_id(0).expect("the body");
        if let Some(owner) = app.project.body_owner(body) {
            app.enter_component(owner);
        }
        let edge = app.project.regen_edges[&body].iter().filter(|e| (e.a[2] - e.b[2]).abs() < 1e-6).max_by(|x, y| x.mid[2].total_cmp(&y.mid[2])).cloned().expect("the top edge");

        let mut hand = Hand::new(&mut app);
        hand.look_at([10.0, 10.0, 5.0], 9.0).tool(qymcad_ui_state::PartTool::Fillet).click(edge.mid).enter();

        let made = app.project.timeline.iter().any(|n| matches!(n.kind, qymcad_core::feature::FeatureKind::Fillet { .. }));
        assert!(made, "a click on an edge and Enter must create a fillet; status: {}", app.status);
        assert!(app.project.regen_errors.is_empty(), "and it must build: {:?}", app.project.regen_errors);
    }

    /// A BUTTON IS LOOKED FOR ONCE THE REBUILD HAS GONE: while the model rebuilds, its card lies over the window and no
    /// hint comes up under the pointer. The kernel held by a neighbour for 8 s - as a heavy check beside this one holds
    /// it - and the rectangle is still found by its hint.
    ///
    /// Measured in a run of the whole crate: the card "Rebuilding the model" stood over the window for the whole search,
    /// 70 buttons rested over, and the rectangle was reported missing from a panel that had it.
    #[test]
    fn a_button_is_found_after_the_rebuild_card_goes() {
        let (held, holding) = std::sync::mpsc::channel();
        let neighbour = std::thread::spawn(move || {
            let _gate = qymcad_kernel::kernel_gate();
            held.send(()).expect("the probe waits for the kernel to be held");
            std::thread::sleep(std::time::Duration::from_secs(8));
        });
        holding.recv().expect("the neighbour holds the kernel");
        let (mut app, _ctx) = crate::gui::import_door::tests::running();
        let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        let mut hand = Hand::new(&mut app);
        hand.app.enter_sketch_edit(si);
        hand.sk_tool(qymcad_ui_state::SketchShape::Rect);
        assert_eq!(hand.app.tools.armed.drawing(), Some(qymcad_ui_state::SketchShape::Rect), "the rectangle was not taken");
        neighbour.join().expect("the neighbour lets the kernel go");
    }
}
