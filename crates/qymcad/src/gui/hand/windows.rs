//! The hand at the windows of the program: folding one by its title bar.

use super::*;

impl<'a> Hand<'a> {
    /// FOLD THE WINDOW TITLED `title` with the arrow at the left end of its title bar, as a person folds a window kept for
    /// later, and let the folding run out. Answers whether such a window with an arrow was on screen. The arrow is the
    /// framework's own button, told to a screen reader as "Hide".
    pub fn fold_titled(&mut self, title: &str) -> bool {
        let Some(window) = self.window_titled(title) else { return false };
        let arrow = self
            .win
            .widgets
            .iter()
            .find(|w| w.kind == super::super::window::Kind::Button && w.label == "Hide" && window.contains(w.rect.center()) && w.rect.center().y < window.min.y + 40.0)
            .map(|w| w.rect.center());
        let Some(at) = arrow else { return false };
        self.press_screen(at);
        // the window folds in an animation of some frames
        for _ in 0..10 {
            self.frame(Vec::new());
        }
        true
    }
}
