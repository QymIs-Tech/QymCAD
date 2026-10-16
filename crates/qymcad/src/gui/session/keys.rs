//! THE KEYBOARD AND THE CLIPBOARD OF A SESSION, and the chooser of files.

use super::*;

impl Session {
    /// PRESS AND RELEASE `key`, with whatever is held.
    pub fn key(&mut self, key: Key) -> &mut Self {
        self.chord(Modifiers::default(), key)
    }

    /// PRESS AND RELEASE `key` with `modifiers` down: Ctrl+K is `chord(Modifiers::COMMAND, Key::K)`.
    pub fn chord(&mut self, modifiers: Modifiers, key: Key) -> &mut Self {
        self.settle();
        let before = self.held;
        self.held = before | modifiers;
        let event = |pressed| egui::Event::Key { key, physical_key: None, pressed, repeat: false, modifiers: before | modifiers };
        self.frame(vec![event(true)]);
        self.frame(vec![event(false)]);
        self.held = before;
        self.stepped(&format!("the key {key:?}"));
        self
    }

    /// TYPE `text` on the keyboard, into whatever holds the keyboard.
    pub fn type_text(&mut self, text: &str) -> &mut Self {
        self.settle();
        self.frame(vec![egui::Event::Text(text.to_string())]);
        self.stepped(&format!("typing {text:?}"));
        self
    }

    /// PASTE `text` from the clipboard, as Ctrl+V does.
    pub fn paste(&mut self, text: &str) -> &mut Self {
        self.settle();
        self.frame(vec![egui::Event::Paste(text.to_string())]);
        self.stepped("a paste");
        self
    }

    /// COPY, as Ctrl+C does; what reached the clipboard is in [`Session::clipboard`].
    pub fn copy(&mut self) -> &mut Self {
        self.settle();
        self.frame(vec![egui::Event::Copy]);
        self.stepped("a copy");
        self
    }

    /// CUT, as Ctrl+X does.
    pub fn cut(&mut self) -> &mut Self {
        self.settle();
        self.frame(vec![egui::Event::Cut]);
        self.stepped("a cut");
        self
    }

    /// WHICH CHOOSER THE PROGRAM HAS PUT UP and waits on, if any.
    pub fn chooser(&mut self) -> Option<Chooser> {
        self.settle();
        crate::system::with_stand_in(|s| s.asked.as_ref().map(|a| a.kind)).flatten()
    }

    /// THE FOLDER THE CHOOSER THAT IS UP OPENS IN; `None` when no chooser is up or it leaves the folder to the
    /// system.
    pub fn chooser_folder(&mut self) -> Option<std::path::PathBuf> {
        self.settle();
        crate::system::with_stand_in(|s| s.asked.as_ref().and_then(|a| a.folder.clone())).flatten()
    }

    /// ANSWER THE CHOOSER with `path`, as a person picks a file in it.
    ///
    /// # Panics
    /// When no chooser is up.
    pub fn answer_file(&mut self, path: impl AsRef<std::path::Path>) -> &mut Self {
        self.answer(Some(path.as_ref().to_path_buf()))
    }

    /// CLOSE THE CHOOSER without choosing.
    ///
    /// # Panics
    /// When no chooser is up.
    pub fn cancel_file(&mut self) -> &mut Self {
        self.answer(None)
    }

    /// WHAT THE PROGRAM ASKED THE SYSTEM TO START - a file manager, a browser - with its arguments, in order.
    pub fn started(&self) -> Vec<(String, Vec<String>)> {
        crate::system::with_stand_in(|s| s.started.clone()).unwrap_or_default()
    }

    /// WHAT REACHED THE CLIPBOARD, oldest first.
    pub fn clipboard(&self) -> &[String] {
        &self.win.copied
    }

    /// The pages the program asked the system to open, oldest first.
    pub fn opened_pages(&self) -> &[String] {
        &self.win.urls
    }
}
