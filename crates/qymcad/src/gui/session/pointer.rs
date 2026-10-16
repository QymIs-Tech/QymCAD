//! THE POINTER OF A SESSION: moved, pressed, clicked, dragged, the wheel.

use super::*;

impl Session {
    /// BRING THE POINTER TO `at`.
    pub fn move_to(&mut self, at: Pos2) -> &mut Self {
        self.settle();
        self.go(at);
        self
    }

    /// PRESS `button` where the pointer stands.
    ///
    /// # Panics
    /// When the pointer has not come over the window yet.
    pub fn press(&mut self, button: PointerButton) -> &mut Self {
        self.settle();
        self.button(button, true);
        self
    }

    /// LET GO OF `button` where the pointer stands.
    ///
    /// # Panics
    /// When the pointer has not come over the window yet.
    pub fn release(&mut self, button: PointerButton) -> &mut Self {
        self.settle();
        self.button(button, false);
        self.stepped("a button let go");
        self
    }

    /// CLICK THE LEFT BUTTON at `at`.
    pub fn click(&mut self, at: Pos2) -> &mut Self {
        self.click_with(at, PointerButton::Primary, Modifiers::default())
    }

    /// CLICK `button` at `at` with `modifiers` held for the click.
    pub fn click_with(&mut self, at: Pos2, button: PointerButton, modifiers: Modifiers) -> &mut Self {
        self.settle();
        self.go(at);
        let before = self.held;
        self.held = before | modifiers;
        self.button(button, true);
        self.button(button, false);
        self.held = before;
        self.stepped("a click");
        self
    }

    /// DOUBLE-CLICK THE LEFT BUTTON at `at`: two clicks a few frames apart, well inside the time a double click
    /// is allowed.
    pub fn double_click(&mut self, at: Pos2) -> &mut Self {
        self.settle();
        self.go(at);
        self.button(PointerButton::Primary, true);
        self.button(PointerButton::Primary, false);
        self.frame(vec![Self::pressing(at, PointerButton::Primary, true, self.held)]);
        self.frame(vec![Self::pressing(at, PointerButton::Primary, false, self.held)]);
        self.stepped("a double click");
        self
    }

    /// DRAG WITH `button` from `from` to `to`, `modifiers` held: pressed, carried over in even steps a frame
    /// each, and let go.
    pub fn drag(&mut self, from: Pos2, to: Pos2, button: PointerButton, modifiers: Modifiers) -> &mut Self {
        self.settle();
        self.drag_holding(from, to, &[button], modifiers);
        self.stepped("a drag");
        self
    }

    /// DRAG WITH SEVERAL BUTTONS AT ONCE, all of them held from `from` to `to`; no button at all is a movement
    /// with only `modifiers` down, the way a touchpad moves the view.
    pub fn drag_with(&mut self, from: Pos2, to: Pos2, buttons: &[PointerButton], modifiers: Modifiers) -> &mut Self {
        self.settle();
        self.drag_holding(from, to, buttons, modifiers);
        self.stepped("a drag");
        self
    }

    /// TURN THE WHEEL with the pointer at `at`, by `delta` points (positive `y` is away from the person, which
    /// scrolls up), `modifiers` held.
    pub fn wheel(&mut self, at: Pos2, delta: Vec2, modifiers: Modifiers) -> &mut Self {
        self.settle();
        self.go(at);
        let before = self.held;
        self.held = before | modifiers;
        let event = egui::Event::MouseWheel { unit: egui::MouseWheelUnit::Point, delta, phase: egui::TouchPhase::Move, modifiers: self.held };
        self.frame(vec![event]);
        self.held = before;
        self.stepped("a turn of the wheel");
        self
    }

    /// HOLD `modifiers` DOWN until [`Session::let_go`], across every gesture in between.
    pub fn hold(&mut self, modifiers: Modifiers) -> &mut Self {
        self.held |= modifiers;
        self
    }

    /// Let go of every key held down.
    pub fn let_go(&mut self) -> &mut Self {
        self.held = Modifiers::default();
        self
    }
}
