use super::*;

impl<'a> Hand<'a> {
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
        for _ in 0..10 {
            self.frame(Vec::new());
        }
        true
    }
}
