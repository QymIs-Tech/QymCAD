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

    /// TYPE `text` into the focused field, and answer how many passes egui ran for it: one, unless something drawn
    /// for the first time asked to throw its pass away and draw again (`request_discard`).
    pub fn passes_to_type(&mut self, text: &str) -> u64 {
        let before = self.win.ctx.cumulative_pass_nr();
        self.frame(vec![egui::Event::Text(text.to_string())]);
        self.win.ctx.cumulative_pass_nr() - before
    }
}
