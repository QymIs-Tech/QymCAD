//! THE FRAMES OF A SESSION: stepped, a view brought in, a pixel and its depth, buttons pressed, a run still going.

use super::*;

impl Session {
    /// A STEP IS DONE: the watch of the process, if one is set, looks at the session now.
    pub(crate) fn stepped(&mut self, what: &str) {
        let Some(watch) = WATCH.get() else { return };
        if self.watching || self.win.closed || !self.down.is_empty() {
            return;
        }
        self.watching = true;
        let problems = watch(self);
        self.watching = false;
        assert!(problems.is_empty(), "after {what}, the program is not right:\n{}", problems.join("\n"));
    }

    /// One frame with `events` in it and whatever is held, in a window that is still open.
    pub(crate) fn frame(&mut self, events: Vec<egui::Event>) {
        assert!(!self.win.closed, "the program has closed its window: there is nothing left to act on");
        self.speak();
        self.win.run(&mut self.app, self.held, events, false);
    }

    /// BRING WHAT `at` POINTS TO INTO THE MIDDLE PART OF THE CANVAS, as a person does: what is far off the canvas
    /// is first brought nearer by turning the wheel over the middle of it, and then the view is dragged with
    /// `buttons` and `modifiers` held, a part of the canvas at a time. A wheel turn or a drag that took the place
    /// further away is made the other way next time.
    pub(crate) fn bring_into_view(&mut self, at: &dyn Fn(&Session) -> Pos2, buttons: &[PointerButton], modifiers: Modifiers) {
        let (mut drag_sign, mut wheel_sign) = (1.0, -1.0);
        for _ in 0..48 {
            let canvas = self.app.viewing.view_rect;
            let inner = canvas.shrink2(canvas.size() * 0.1);
            let before = at(self);
            if inner.contains(before) {
                return;
            }
            let centre = canvas.center();
            if before.distance(centre) > canvas.size().length() * 1.5 {
                self.wheel(centre, vec2(0.0, 120.0 * wheel_sign), Modifiers::default());
                self.settle();
                if at(self).distance(centre) >= before.distance(centre) {
                    wheel_sign = -wheel_sign;
                }
                continue;
            }
            let step = ((centre - before) * drag_sign).clamp(-canvas.size() * 0.35, canvas.size() * 0.35);
            self.drag_holding(centre, centre + step, buttons, modifiers);
            self.settle();
            if at(self).distance(centre) >= before.distance(centre) {
                drag_sign = -drag_sign;
            }
        }
        panic!("the point at {:?} could not be brought into the canvas {:?}", at(self), self.app.viewing.view_rect);
    }

    /// A drag from `from` to `to` with every one of `buttons` held and `modifiers` down; no button at all is a
    /// movement with the modifiers held, as a touchpad moves the view.
    pub(crate) fn drag_holding(&mut self, from: Pos2, to: Pos2, buttons: &[PointerButton], modifiers: Modifiers) {
        self.go(from);
        let before = self.held;
        self.held = before | modifiers;
        for b in buttons {
            self.button(*b, true);
        }
        const STEPS: usize = 12;
        for k in 1..=STEPS {
            self.go(from + (to - from) * (k as f32 / STEPS as f32));
        }
        for b in buttons.iter().rev() {
            self.button(*b, false);
        }
        self.held = before;
    }

    /// How long a pixel of the screen is in the 3D view at `p`, and how deep `p` stands.
    pub(crate) fn pixel_and_depth(&self, p: [f64; 3]) -> (f64, f64) {
        let basis = self.app.viewing.cam.basis();
        let scr = qymcad_ui_state::Screen { cam: &self.app.viewing.cam, set: &self.app.set, rect: self.app.viewing.view_rect, basis: &basis };
        let (a, depth) = scr.at(p);
        let (b, _) = scr.at([p[0] + basis.0[0], p[1] + basis.0[1], p[2] + basis.0[2]]);
        (1.0 / (a.distance(b) as f64).max(1e-9), depth)
    }

    /// Every place `text` is written, whole, once the screen is at rest.
    pub(crate) fn places_of(&mut self, text: &str) -> Vec<Rect> {
        self.settle();
        self.win.drawn.iter().filter(|(t, _)| t == text).map(|(_, r)| *r).collect()
    }

    /// Does `hint` come up with the pointer resting over `at`? Only a hint that was not on screen before counts:
    /// the same words written elsewhere say nothing about this button.
    pub(crate) fn hint_comes_up(&mut self, at: Pos2, hint: &str) -> bool {
        let already = self.win.drawn.iter().any(|(t, _)| t == hint);
        self.rest_over(at);
        !already && self.win.drawn.iter().any(|(t, _)| t == hint)
    }

    /// THE POINTER COMES OVER `at` AND RESTS until a hint would be up.
    ///
    /// THREE FRAMES AFTER A SECOND OF REST, measured: the hint of the button left behind closes a frame after the
    /// pointer leaves it, egui opens no second hint while one is open, and a hint that opens is laid out unseen in
    /// its first frame and drawn in the next. The words of all three frames count.
    pub(crate) fn rest_over(&mut self, at: Pos2) {
        self.go(at);
        self.win.clock += 1.0;
        let mut seen: Vec<(String, Rect)> = Vec::new();
        for _ in 0..3 {
            self.frame(Vec::new());
            seen.extend(self.win.drawn.iter().cloned());
        }
        self.win.drawn = seen;
    }

    /// The pointer comes to `at`, in a frame.
    pub(crate) fn go(&mut self, at: Pos2) {
        self.pointer = Some(at);
        self.frame(vec![egui::Event::PointerMoved(at)]);
    }

    /// `button` goes down or up where the pointer stands, in a frame of its own. A press follows the one before
    /// by at least a second: two gestures meant apart are never one double click.
    pub(crate) fn button(&mut self, button: PointerButton, pressed: bool) {
        let at = self.pointer.expect("the pointer has not come over the window: move it first");
        if pressed {
            self.win.clock = self.win.clock.max(self.last_press + 1.0);
            self.last_press = self.win.clock;
        }
        self.down.retain(|b| *b != button);
        if pressed {
            self.down.push(button);
        }
        let event = Self::pressing(at, button, pressed, self.held);
        self.frame(vec![event]);
    }

    /// The event of `button` going down or up at `at`.
    pub(crate) fn pressing(at: Pos2, button: PointerButton, pressed: bool, modifiers: Modifiers) -> egui::Event {
        egui::Event::PointerButton { pos: at, button, pressed, modifiers }
    }

    /// Answer the chooser that is up.
    pub(crate) fn answer(&mut self, path: Option<std::path::PathBuf>) -> &mut Self {
        self.settle();
        let asked = crate::system::with_stand_in(|s| s.asked.take()).flatten();
        let asked = asked.expect("the program has put up no chooser to answer");
        asked.tx.send(path).expect("the program stopped waiting for the chooser");
        self.settle();
        self.stepped("an answer to the chooser");
        self
    }

    /// WHAT KEEPS THE PROGRAM FROM REST, in words; empty when it is quiet.
    pub(crate) fn still_running(&self) -> Vec<String> {
        let app = &self.app;
        let mut still = Vec::new();
        if app.waiting.splash_until.is_some() {
            still.push("the greeting".to_string());
        }
        if let Some(path) = &app.disk.io.startup {
            still.push(format!("the start-up load of {path}"));
        }
        if let Some(busy) = &app.regen.busy {
            still.push(format!("{:?} '{}' for {:?}", busy.kind, busy.label, busy.started.elapsed()));
        }
        still.extend(app.regen.bg.iter().map(|b| format!("in the background {:?} '{}' for {:?}", b.kind, b.label, b.started.elapsed())));
        if app.regen.wanted {
            still.push("a rebuild asked for and not started".to_string());
        }
        // THE FONTS OF THE MACHINE ARE WALKED ON A THREAD: an open list with nothing in it is still being filled
        if app.font_cache.picker.open && app.font_cache.picker.faces.is_empty() {
            still.push("the fonts of the machine are being looked for".to_string());
        }
        // A CHOOSER WAITING FOR ITS ANSWER keeps the frames coming by design: that is rest, with a question open.
        // AN ANIMATION IS THE SAME: a degree of freedom running over its range asks for a frame every time, and
        // waiting for it to stop would mean waiting for ever - the check looks at the program while it moves.
        if self.win.repaint < Duration::from_millis(100) && !app.asking_for_a_file() && app.joint_anim.is_none() {
            still.push(format!("the screen is still moving (a frame asked again in {:?})", self.win.repaint));
        }
        still
    }
}
