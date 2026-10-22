//! The hand at the text of a sketch: the font chosen from a file or from the list, a text written, its font and its words changed.

use super::*;

impl Hand<'_> {
    /// CHOOSE A FONT FROM A FILE: the list of fonts is opened by the button of the top bar that names the font of
    /// the text tool, the file chooser behind "From a file..." answers with `path`, and the list is closed with
    /// Esc, as a person closes it having got the font.
    ///
    /// The button "From a file..." itself is not pressed. It puts up the file chooser of the system, and a check
    /// must not put a window on somebody's screen. What the chooser answers arrives through `arm_file_ask`, and
    /// the hand gives the path there - as the checks of importing do - to the same answer the button leads to.
    pub fn pick_font_file(&mut self, path: &str) -> &mut Self {
        if !self.app.font_cache.picker.open {
            if self.app.tools.armed.drawing() != Some(qymcad_ui_state::SketchShape::Text) {
                self.sk_tool(qymcad_ui_state::SketchShape::Text);
            }
            let named = qymcad_ui_state::font_label(&self.app.tool_prefs.font, "opt-font");
            assert!(self.press_word(&named, egui::Pos2::new(700.0, 0.0)), "the top bar named no font {named:?} to open the list with");
        }
        let (tx, rx) = std::sync::mpsc::channel();
        self.app.arm_file_ask(rx, crate::gui::pick::font_answer);
        tx.send(Some(std::path::PathBuf::from(path))).expect("the chooser's channel is open");
        self.frame(Vec::new());
        self.key(egui::Key::Escape);
        self.close_window()
    }

    /// OPEN THE LIST OF FONTS, search for `query` in it, and click the first face it shows. Returns whether
    /// anything was chosen.
    ///
    /// All of it in whole frames. A list that is not open yet is opened by the button of the top bar that names
    /// the font of the text tool - with the text tool taken first, since the bar shows that button only then.
    /// The search field is clicked where its hint is written and the query typed into it.
    pub fn pick_font_from_list(&mut self, query: &str) -> bool {
        self.app.viewing.mode_3d = false;
        if !self.app.font_cache.picker.open {
            if self.app.tools.armed.drawing() != Some(qymcad_ui_state::SketchShape::Text) {
                self.sk_tool(qymcad_ui_state::SketchShape::Text); // not twice: the button of a tool in hand puts the tool down
            }
            let named = qymcad_ui_state::font_label(&self.app.tool_prefs.font, "opt-font");
            if !self.press_word(&named, egui::Pos2::ZERO) {
                return false; // the bar named no font: there is nothing to open the list with
            }
        }
        // THE LIST ARRIVES FROM A THREAD, so the hand waits for it exactly as a person waits for "Looking for
        // fonts..." to turn into the list. Measured: the walk takes about half a second on a machine with 232
        // faces, longer with a cold file cache.
        crate::gui::session::waiting::until("the list of the fonts of the system", || {
            if !self.app.font_cache.picker.faces.is_empty() {
                return true;
            }
            self.frame(Vec::new());
            false
        });
        if !self.press_word(&qymcad_i18n::tr("font-search"), egui::Pos2::ZERO) {
            return false; // no search field in sight: the window did not open
        }
        self.type_text(query);
        // WHERE THE ROWS ARE: under the button "from a file", the count and the separator. The hand works down
        // the list a row at a time until a click lands on one, which is what a person does after a miss; the
        // rows are 26 px apart. It never presses the button itself, which opens the file dialog of the system.
        self.frame(Vec::new());
        let Some(below) = self.win.drawn.iter().find(|(t, _)| *t == qymcad_i18n::tr("font-from-file")).map(|(_, r)| r).copied() else { return false };
        let first = egui::pos2(below.left(), below.bottom() + 26.0 + 13.0);
        for step in 0..12 {
            let at = first + egui::vec2(0.0, 26.0 * step as f32);
            self.frame(vec![egui::Event::PointerMoved(at)]);
            for pressed in [true, false] {
                self.frame(vec![egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() }]);
            }
            if !self.app.font_cache.picker.open {
                self.close_window();
                return true; // the window closed: a face was taken
            }
        }
        self.close_window();
        false
    }

    /// TYPE THE TEXT into the top bar and ITS HEIGHT into the box of the tool, with the text tool taken by its button
    /// first - unless it is in hand already, where its button would put it down. The click that follows places the text.
    ///
    /// Each field is clicked just right of the word that names it, its text is selected with Ctrl+A and typed
    /// over. Tab leaves the height, so the next click on the canvas is not a click out of a field.
    pub fn sk_text(&mut self, text: &str, height: f64) -> &mut Self {
        if self.app.tools.armed.drawing() != Some(qymcad_ui_state::SketchShape::Text) {
            self.sk_tool(qymcad_ui_state::SketchShape::Text);
        }
        self.frame(Vec::new());
        let word =
            |hand: &Self, w: &str, near: egui::Pos2| hand.win.drawn.iter().filter(|(t, _)| t == w).map(|(_, r)| *r).min_by(|a, b| a.center().distance(near).total_cmp(&b.center().distance(near)));
        let named = word(self, &qymcad_i18n::tr("tool-text"), egui::Pos2::new(700.0, 0.0)).expect("the top bar of the text tool names no string field");
        self.press_screen(egui::pos2(named.right() + 24.0, named.center().y));
        self.chord(egui::Modifiers::COMMAND, egui::Key::A);
        self.type_text(text);
        // the height is a field of the box of the tool, found right of its caption there
        assert!(self.type_after_word(&qymcad_i18n::tr("opt-height-short"), &format!("{height}")), "the box of the text tool names no height");
        self.key(egui::Key::Tab);
        self.close_window()
    }

    /// CHANGE THE FONT OF THE LABEL LYING AT `at`: open its editor, press the font it is written in, and take
    /// the first face of the list that matches `query`.
    ///
    /// Returns whether a face was taken. The list serves the tool and the label alike, and what is chosen
    /// must land on the label - that is what this drives. The editor names the font on a button, and the top
    /// bar names the font of the text tool on another: the one pressed is the one next to the label.
    pub fn sk_change_label_font(&mut self, at: (f64, f64), query: &str) -> bool {
        if !self.sk_open_text_edit(at) {
            return false;
        }
        let Some(si) = qymcad_ui_state::edit_si(&self.app.project, &self.app.sketch_ses) else { return false };
        let Some(ti) = self.app.tools.inline.text() else { return false };
        let font = self.app.project.sketches[si].texts[ti].font.clone();
        let named = qymcad_ui_state::font_label(&font, "sk-text-font-unknown-short");
        if !self.press_word(&named, self.screen2d(at)) {
            return false;
        }
        self.pick_font_from_list(query)
    }

    /// OPEN THE EDITOR OF THE LABEL LYING AT `at` - a double click on it, and nothing more. Answers whether an
    /// editor opened.
    ///
    /// Separate from applying the edit, because the tool is taken up when the editor opens and put down when
    /// the edit is applied: the two states are checked apart.
    pub fn sk_open_text_edit(&mut self, at: (f64, f64)) -> bool {
        self.double_click2d(at.0, at.1);
        self.app.tools.inline.text().is_some()
    }

    /// EDIT THE TEXT LYING AT `at` and apply the change: the double click, the string typed, Tab on to the
    /// height, the height typed, Shift+Tab back to the string and Enter - as a person does it from the keyboard.
    ///
    /// Each field selects its text as it takes the focus, so typing replaces what was there; the height is
    /// taken as its field lets the focus go.
    pub fn sk_edit_text(&mut self, at: (f64, f64), text: &str, height: f64) -> &mut Self {
        if !self.sk_open_text_edit(at) {
            return self;
        }
        self.frame(Vec::new()); // the string field takes the focus
        self.type_text(text);
        self.key(egui::Key::Tab);
        self.type_text(&format!("{height}"));
        self.chord(egui::Modifiers::SHIFT, egui::Key::Tab);
        self.key(egui::Key::Enter);
        self.close_window()
    }
}
