//! The hand pressing with keys held - Shift or Ctrl adding to what is picked - in the scene and in the panels, and
//! the program taken in hand as it stands.

use super::*;

impl<'a> Hand<'a> {
    /// ENTER APPLIES THE COMMAND, as `enter` does; answers why it did not apply, if it did not.
    pub fn enter_answered(&mut self) -> Option<qymcad_ui_state::NotApplied> {
        let why = self.app.apply_feat_cmd();
        qymcad_ui_state::rebuild_if_dirty(&mut self.app.rebuild_ctx());
        why
    }

    /// WHAT THE LAST FRAME KEPT IN SIGHT: the places on the screen its handles and gizmos took, which no box covers.
    pub fn kept_in_sight(&self) -> Vec<egui::Rect> {
        qymcad_ui_state::in_sight(&self.win.ctx)
    }

    /// WHERE THE TEXT FIELDS INSIDE `area` STAND, the last frame drawn, from the top down.
    pub fn fields_in(&self, area: egui::Rect) -> Vec<egui::Rect> {
        let mut out: Vec<egui::Rect> = self.win.widgets.iter().filter(|w| w.kind == super::super::window::Kind::TextField && area.contains_rect(w.rect)).map(|w| w.rect).collect();
        out.sort_by(|a, b| a.top().total_cmp(&b.top()).then(a.left().total_cmp(&b.left())));
        out
    }

    /// WHAT THE TOOL IN HAND SAYS on its plate in the corner of the view, the last frame drawn: each sentence with its
    /// tone - the answer to an Enter it refused among them.
    pub fn tool_says(&mut self) -> Vec<qymcad_ui_state::Notice> {
        self.frame(Vec::new());
        qymcad_ui_state::words_laid(&self.win.ctx)
    }

    /// THE SAME PRESS with `modifiers` held - Shift or Ctrl adding a row to what is picked.
    pub fn press_word_holding(&mut self, word: &str, near: egui::Pos2, modifiers: egui::Modifiers) -> bool {
        self.frame(Vec::new());
        let Some(at) = self.win.drawn.iter().filter(|(t, _)| t == word).map(|(_, r)| r.center()).min_by(|a, b| a.distance(near).total_cmp(&b.distance(near))) else {
            return false;
        };
        self.press_holding(at, modifiers);
        true
    }

    /// A press and a release of the left button at `at` with `modifiers` held, each in a frame of its own, the pointer
    /// come to rest there first - a second later than the press before, so two of them are not one double click.
    pub(super) fn press_holding(&mut self, at: egui::Pos2, modifiers: egui::Modifiers) {
        self.win.clock += 1.0;
        self.frame_holding(modifiers, vec![egui::Event::PointerMoved(at)]);
        let button = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers };
        self.frame_holding(modifiers, vec![button(true)]);
        self.frame_holding(modifiers, vec![button(false)]);
    }

    /// WHERE A PLACE OF THE SCENE, given in world coordinates, stands on screen in the view the window laid out.
    pub fn on_screen3d(&mut self, world: [f64; 3]) -> egui::Pos2 {
        self.frame(Vec::new()); // the window lays the canvas out, where the point is then seen
        let basis = self.app.viewing.cam.basis();
        qymcad_ui_state::Screen { cam: &self.app.viewing.cam, set: &self.app.set, rect: self.app.viewing.view_rect, basis: &basis }.at(world).0
    }

    /// A CLICK ON A PLACE OF THE SCENE in whole frames, `modifiers` held: the click the window reads - the tool in hand
    /// takes it, or with nothing in hand it picks, Shift adding to what is picked - not a call of its handler.
    pub fn press3d(&mut self, world: [f64; 3], modifiers: egui::Modifiers) -> &mut Self {
        let at = self.on_screen3d(world);
        self.press_holding(at, modifiers);
        self.close_window()
    }

    /// TAKE THE PROGRAM IN HAND AS IT STANDS: the view it is in - flat or in space - left as the program has it.
    pub fn as_it_stands(app: &'a mut super::super::App) -> Self {
        let flat = !app.viewing.mode_3d;
        let hand = Self::new(app);
        hand.app.viewing.mode_3d = !flat;
        hand
    }
}

/// PRESS Esc ONCE over the program as it stands, in whole frames of the window: a field holding the keyboard gives it
/// up, a tool in hand steps back, and with nothing in hand the ladder of the window answers.
pub(in crate::gui) fn escape(app: &mut super::super::App) {
    Hand::as_it_stands(app).key(egui::Key::Escape);
}
