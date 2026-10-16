//! A SESSION STARTED, WAITED ON AND QUIT.

use super::*;

impl Session {
    /// START THE PROGRAM on a machine of its defaults, and wait until it is ready for a person.
    pub fn start() -> Session {
        Session::start_on(Machine::default())
    }

    /// START THE PROGRAM on `machine`, the way the window starts it, and wait until it is ready for a person.
    ///
    /// # Panics
    /// When `machine.home` is not the home of the sessions started before in this process: the program's own
    /// folders are one per process.
    pub fn start_on(machine: Machine) -> Session {
        assert!(qymcad_paths::keep_under(machine.home.clone()), "one process keeps one home for the program; {} was asked for after another", machine.home.display());
        super::super::prepare_the_process();
        qymcad_i18n::stand_for_system_locale(&machine.locale);
        crate::system::stand_in(true);
        let win = Window::new(egui::vec2(machine.screen.0, machine.screen.1));
        let app = Box::new(super::super::start_the_program(&win.ctx, Some(&machine.kept)));
        let mut session = Session {
            app,
            win,
            kept: machine.kept,
            budget: Duration::from_secs(120),
            pointer: None,
            held: Modifiers::default(),
            last_press: f64::NEG_INFINITY,
            down: Vec::new(),
            locale: machine.locale,
            watching: false,
        };
        if !machine.greeting {
            // the greeting has been on screen for its time: it goes on the first frame, through its own branch
            session.app.waiting.splash_until = Some(Instant::now());
        }
        session.settle();
        session
    }

    /// A MACHINE LIKE THIS SESSION'S: its locale and its settings as they are now, for a second start of the program
    /// that reads what this one wrote in the same language.
    pub fn same_machine(&self) -> Machine {
        let mut kept = Kept::default();
        eframe::set_value(&mut kept, "settings", &self.app.set);
        Machine { locale: self.locale.clone(), kept, ..Machine::default() }
    }

    /// CLOSE THE WINDOW as a person does, with its cross, and keep what the program keeps on the way out.
    ///
    /// A program with unsaved work refuses to close and asks what to do with it; then the session is given back,
    /// still running, with the question on screen.
    pub fn quit(mut self) -> Result<Kept, Box<Session>> {
        if !self.win.closed {
            self.settle();
            self.speak();
            self.win.run(&mut self.app, egui::Modifiers::default(), Vec::new(), true);
            if self.win.close_refused && !self.win.closed {
                self.settle();
                return Err(Box::new(self));
            }
        }
        eframe::App::save(&mut *self.app, &mut self.kept);
        Ok(std::mem::take(&mut self.kept))
    }

    /// HAS THE PROGRAM CLOSED ITS WINDOW by itself - Quit, or an answer to the question it asked on closing?
    /// What it keeps is then taken by [`Session::quit`].
    pub fn closed(&self) -> bool {
        self.win.closed
    }

    /// LOOK AT EVERY STEP OF EVERY SESSION OF THIS PROCESS with `watch`: after each gesture - a click, a key, a drag,
    /// typing, an answer to a chooser - the watch is run on the session, and a problem it answers fails the check at
    /// that step. Set once per process; `false` when another watch was set first.
    pub fn watch_every_step(watch: fn(&mut Session) -> Vec<String>) -> bool {
        WATCH.set(watch).is_ok()
    }

    /// HOW LONG ONE WAIT MAY TAKE before the check fails, naming what was still running. Two minutes unless said.
    pub fn budget(&mut self, budget: Duration) -> &mut Self {
        self.budget = budget;
        self
    }

    /// WAIT UNTIL THE PROGRAM IS QUIET, as a person waits: frames go on being drawn until no rebuild, load or
    /// write is under way and nothing on screen is still moving.
    ///
    /// # Panics
    /// When the budget runs out first, with what was still running.
    pub fn settle(&mut self) -> &mut Self {
        if !self.down.is_empty() || self.win.closed {
            // a button held is a gesture not over, the screen meant to move under it; a closed window has no frames
            return self;
        }
        let began = Instant::now();
        let mut quiet_frames = 0;
        while quiet_frames < 2 {
            self.frame(Vec::new());
            let still = self.still_running();
            quiet_frames = if still.is_empty() { quiet_frames + 1 } else { 0 };
            if !still.is_empty() {
                assert!(began.elapsed() < self.budget, "the program did not come to rest in {:?}: {}", self.budget, still.join("; "));
                std::thread::sleep(self.win.repaint.min(Duration::from_millis(16)));
            }
        }
        self
    }

    /// WAIT FOR `what` TO HAPPEN, as a person does - frames drawn, the clock on the wall moving - until `happened` says
    /// it has, for no longer than `deadline`. A probe that slept a fixed span and then looked passed on a fast machine
    /// and went red now and then on a runner of the CI, where the same work came later: an autosave of 1 s and an
    /// extrude did not fit in 2.5 s there. The deadline is the bound of a fault, not of a slow machine.
    ///
    /// # Panics
    /// When `deadline` passes first, naming what was waited for.
    pub fn wait_for(&mut self, what: &str, deadline: Duration, mut happened: impl FnMut(&mut Self) -> bool) -> &mut Self {
        let began = Instant::now();
        while !happened(self) {
            assert!(began.elapsed() < deadline, "waited {deadline:?} for {what}, and it did not happen");
            assert!(!self.win.closed, "the window closed while waiting for {what}");
            std::thread::sleep(Duration::from_millis(16));
            self.frame(Vec::new());
        }
        self.settle()
    }

    /// LET `span` OF REAL TIME PASS with the window drawing its frames, as a person waits with the program in front
    /// of them: what the program times by the clock on the wall - autosave, the turn of the view - moves on too.
    pub fn pause(&mut self, span: Duration) -> &mut Self {
        let began = Instant::now();
        while began.elapsed() < span {
            std::thread::sleep(Duration::from_millis(16).min(span.saturating_sub(began.elapsed())));
            if self.win.closed {
                break;
            }
            self.frame(Vec::new());
        }
        self.settle()
    }
}
