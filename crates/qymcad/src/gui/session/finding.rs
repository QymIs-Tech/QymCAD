//! WHAT A SESSION FINDS ON THE SCREEN: words, buttons, menus, hints, fields, ticks.

use super::*;

impl Session {
    /// RESIZE THE WINDOW to `size` points.
    pub fn resize(&mut self, size: Vec2) -> &mut Self {
        self.win.screen = size;
        self.settle();
        self
    }

    /// The size of the window, in points.
    pub fn screen(&self) -> Vec2 {
        self.win.across() // in points, which the interface scale changes
    }

    /// WHERE `text` IS WRITTEN ON SCREEN, whole - the place nearest to `near` when it is written in several.
    pub fn find(&mut self, text: &str, near: Pos2) -> Option<Rect> {
        self.settle();
        self.win.drawn.iter().filter(|(t, _)| t == text).map(|(_, r)| *r).min_by(|a, b| a.center().distance(near).total_cmp(&b.center().distance(near)))
    }

    /// PRESS WHERE `text` IS WRITTEN - a menu item, a tree row, a button with a caption.
    ///
    /// # Panics
    /// When `text` is not on screen, or is written in more than one place: then [`Session::press_word_near`] says
    /// which one.
    pub fn press_word(&mut self, text: &str) -> &mut Self {
        let places: Vec<Rect> = self.places_of(text);
        match places.as_slice() {
            [one] => self.click(one.center()),
            [] => panic!("{text:?} is not on screen; on screen: {:?}", self.words()),
            many => panic!("{text:?} is written in {} places, {many:?}: say which with press_word_near", many.len()),
        }
    }

    /// PRESS WHERE `text` IS WRITTEN, the place nearest to `near` when it is written in several.
    ///
    /// # Panics
    /// When `text` is not on screen.
    pub fn press_word_near(&mut self, text: &str, near: Pos2) -> &mut Self {
        match self.find(text, near) {
            Some(r) => self.click(r.center()),
            None => panic!("{text:?} is not on screen; on screen: {:?}", self.words()),
        }
    }

    /// WHERE THE PANEL `id` STANDS, as the last frame laid it out: a panel of the window by its key, or a side panel of
    /// a window by its id - its outer rectangle, the divider at its edge. `None` when no such panel was drawn.
    pub fn panel(&mut self, id: &str) -> Option<Rect> {
        self.settle();
        qymcad_shell::laid_out(&self.win.ctx, id).or_else(|| egui::PanelState::load(&self.win.ctx, egui::Id::new(id)).map(|p| p.outer_rect))
    }

    /// THE WORDS A WINDOW OF THE PROGRAM SHOWS: the runs of words the last frame drew clipped to `area` - the content of a
    /// window, and not what lies under it.
    pub fn words_within(&mut self, area: Rect) -> Vec<WordAt> {
        fn collect(s: &egui::Shape, out: &mut Vec<WordAt>) {
            match s {
                egui::Shape::Text(t) => out.push(WordAt { text: t.galley.text().to_string(), rect: Rect::from_min_size(t.pos, t.galley.size()) }),
                egui::Shape::Vec(v) => v.iter().for_each(|x| collect(x, out)),
                _ => {}
            }
        }
        self.settle();
        let mut out = Vec::new();
        for cs in self.win.shapes.iter().filter(|cs| area.expand(1.0).contains_rect(cs.clip_rect)) {
            collect(&cs.shape, &mut out);
        }
        out.retain(|w| !w.text.trim().is_empty());
        out
    }

    /// EGUI'S COMPLAINTS OF ONE ID TAKEN BY TWO WIDGETS, painted in any frame since they were last asked for: they flash
    /// in the frames of a change, so the screen at rest does not show them.
    pub fn id_clashes(&mut self) -> Vec<String> {
        std::mem::take(&mut self.win.clashes)
    }

    /// THE BUTTONS OF THE BARS OF TOOLS the last frame drew, and the rows of an open "More" list: where each stands.
    pub fn bar_buttons(&mut self) -> Vec<Rect> {
        self.settle();
        qymcad_shell::noted_buttons(&self.win.ctx)
    }

    /// THE "MORE" BUTTONS OF THE BARS the last frame drew.
    pub fn more_buttons(&mut self) -> Vec<Rect> {
        self.settle();
        qymcad_shell::noted_more(&self.win.ctx)
    }

    /// WHAT THE LAST FRAME PAINTED that a person reads as content: every run of words, and every filled background of
    /// one row's height - a highlight, a button - named "a filled background". The ground of a whole area is left out.
    pub fn painted(&mut self) -> Vec<(Rect, String)> {
        fn collect(s: &egui::Shape, out: &mut Vec<(Rect, String)>) {
            match s {
                egui::Shape::Text(t) => out.push((Rect::from_min_size(t.pos, t.galley.size()), t.galley.text().to_string())),
                egui::Shape::Rect(r) if r.fill.a() > 0 && r.rect.height() <= 40.0 => out.push((r.rect, "a filled background".to_string())),
                egui::Shape::Vec(v) => v.iter().for_each(|x| collect(x, out)),
                _ => {}
            }
        }
        self.settle();
        let mut out = Vec::new();
        for cs in &self.win.shapes {
            collect(&cs.shape, &mut out);
        }
        out
    }

    /// GO DOWN A MENU: press each of `path` in turn, each nearest to the one pressed before it.
    ///
    /// # Panics
    /// When an item of the path is not on screen.
    pub fn menu(&mut self, path: &[&str]) -> &mut Self {
        let mut near = pos2(0.0, 0.0);
        for item in path {
            // an item that names what it acts on reads "Undo: Extrusion" - it is still the item "Undo"
            let found = match self.find(item, near) {
                Some(r) => Some(r),
                None => {
                    let lead = format!("{item}:");
                    self.win.drawn.iter().filter(|(t, _)| t.starts_with(&lead)).map(|(_, r)| *r).min_by(|a, b| a.center().distance(near).total_cmp(&b.center().distance(near)))
                }
            };
            let Some(r) = found else { panic!("{item:?} of the menu path {path:?} is not on screen; on screen: {:?}", self.words()) };
            self.click(r.center());
            near = r.center();
        }
        self
    }

    /// PRESS THE BUTTON WHOSE HINT IS `hint`, found as a person finds an icon they do not know: the pointer goes
    /// over the buttons and rests until the hint comes up; a panel longer than the window is scrolled with the
    /// wheel.
    ///
    /// # Panics
    /// When no button of the window has that hint.
    pub fn press_hint(&mut self, hint: &str) -> &mut Self {
        match self.find_hint(hint) {
            Some(at) => self.click(at),
            None => panic!("no button of the window has the hint {hint:?}"),
        }
    }

    /// WHERE THE BUTTON WITH THE HINT `hint` IS, looked for as [`Session::press_hint`] looks; `None` when no
    /// button has it.
    pub fn find_hint(&mut self, hint: &str) -> Option<Pos2> {
        if let Some(at) = self.look_for_hint(hint) {
            return Some(at);
        }
        // A PANEL LEFT SCROLLED DOWN by a button pressed before hides what stands above: a person scrolls back up and
        // looks again
        for _ in 0..12 {
            let Some(over) = self.win.plates.first().copied() else { break };
            let before = self.win.plates.clone();
            self.wheel(over, vec2(0.0, 200.0), Modifiers::default());
            self.settle();
            if self.win.plates == before {
                break;
            }
        }
        // the wheel was turned over the first plate, and the pointer left resting there raises its hint before it
        // is read: a hint already up reads as none. Measured on the part with a block: the sketch pencil, first of the
        // panel, went unread under the oracles
        self.pointer_off_the_plates();
        if let Some(at) = self.look_for_hint(hint) {
            return Some(at);
        }
        // A COMMAND IN "MORE": a person opens the list at the end of the bar and looks there; the list is left open
        // on the row found, for the press that follows
        if let Some(at) = self.hint_in_more(hint) {
            return Some(at);
        }
        // AN ICON BUTTON OUTSIDE THE TOOL PLATES - the cross beside a line of the properties: a person rests the
        // pointer over the small buttons in turn, as over the plates
        let small = self.small_buttons();
        small.into_iter().find(|&at| self.hint_comes_up(at, hint))
    }

    /// THE ROW OF A "MORE" LIST WITH THE HINT `hint`: each "More" of the bars opened in turn and its rows rested over;
    /// a list without the row is closed again by its button.
    fn hint_in_more(&mut self, hint: &str) -> Option<Pos2> {
        for more in self.win.mores.clone() {
            let bar: Vec<Pos2> = self.win.plates.clone();
            self.click(more);
            let rows: Vec<Pos2> = self.win.plates.iter().copied().filter(|p| !bar.contains(p)).collect();
            // the pointer rests where nothing is, or the hint of "More" stays up over the first row and egui opens no
            // second one
            let aside = self.canvas();
            self.rest_over(pos2(aside.max.x - 8.0, 10.0));
            if let Some(at) = rows.into_iter().find(|&at| self.hint_comes_up(at, hint)) {
                return Some(at);
            }
            self.click(more);
        }
        None
    }

    /// THE HINTS OF THE ROWS OF EVERY "MORE" LIST, each list opened by its button and closed again.
    fn hints_in_more(&mut self) -> Vec<String> {
        let mut hints = Vec::new();
        for more in self.win.mores.clone() {
            let bar: Vec<Pos2> = self.win.plates.clone();
            self.click(more);
            let rows: Vec<Pos2> = self.win.plates.iter().copied().filter(|p| !bar.contains(p)).collect();
            let aside = self.canvas();
            self.rest_over(pos2(aside.max.x - 8.0, 10.0));
            for at in rows {
                hints.push(self.hint_at(at).join(" "));
            }
            self.click(more);
        }
        hints
    }

    /// Look for the button with the hint `hint` from where the panel stands now, scrolling downwards.
    pub(crate) fn look_for_hint(&mut self, hint: &str) -> Option<Pos2> {
        self.settle();
        let mut scrolled = 0;
        loop {
            for at in self.win.plates.clone() {
                if self.hint_comes_up(at, hint) {
                    return Some(at);
                }
            }
            // down the panel the buttons stand in, a notch of the wheel at a time
            let over = self.win.plates.first().copied()?;
            let before = self.win.plates.clone();
            self.wheel(over, vec2(0.0, -200.0), Modifiers::default());
            self.settle();
            scrolled += 1;
            if self.win.plates == before || scrolled > 10 {
                return None; // the panel went no further: there is no such button
            }
        }
    }

    /// THE HINTS OF THE SMALL ICON BUTTONS OUTSIDE THE TOOL PLATES - the cross beside a line of the properties - read
    /// as the plates are: the pointer rests over each in turn. A button without a hint is left out.
    pub fn icon_hints(&mut self) -> Vec<String> {
        let small = self.small_buttons();
        let mut hints: Vec<String> = Vec::new();
        for at in small {
            let words = self.hint_at(at).join(" ");
            if !words.is_empty() && !hints.contains(&words) {
                hints.push(words);
            }
        }
        hints
    }

    /// Take the pointer off the buttons so no hint is up when they are read: to the empty right end of the menu bar,
    /// not onto the canvas - there the readout of the coordinates changes as the pointer leaves it again, and its words
    /// would be read as part of the next hint.
    pub(crate) fn pointer_off_the_plates(&mut self) {
        let c = self.canvas();
        self.move_to(pos2(c.max.x - 8.0, 10.0));
    }

    /// Where the small buttons of the window stand: an icon, not a tool plate and not a word.
    pub(crate) fn small_buttons(&mut self) -> Vec<Pos2> {
        self.widgets().into_iter().filter(|w| matches!(w.kind, crate::gui::window::Kind::Button) && w.rect.width() <= 32.0 && w.rect.height() <= 32.0).map(|w| w.rect.center()).collect()
    }

    /// THE HINT OF EVERY TOOL BUTTON OF THE WINDOW, read as a person reads them: the pointer rests over each button in
    /// turn, a panel longer than the window is scrolled with the wheel, and a "More" list is opened and read. A button
    /// without a hint is given as empty.
    pub fn button_hints(&mut self) -> Vec<String> {
        self.settle();
        // a panel left scrolled down by what was done before hides its first buttons: back to the top first, as
        // `find_hint` goes
        for _ in 0..12 {
            let Some(over) = self.win.plates.first().copied() else { break };
            let before = self.win.plates.clone();
            self.wheel(over, vec2(0.0, 200.0), Modifiers::default());
            self.settle();
            if self.win.plates == before {
                break;
            }
        }
        // the wheel was turned over the first plate, and the pointer left resting there raises its hint before it
        // is read: a hint already up reads as none. Measured on the part with a block: the sketch pencil, first of the
        // panel, went unread under the oracles
        self.pointer_off_the_plates();
        let mut hints: Vec<String> = Vec::new();
        let mut scrolled = 0;
        loop {
            for at in self.win.plates.clone() {
                let words = self.hint_at(at).join(" ");
                if !hints.contains(&words) {
                    hints.push(words);
                }
            }
            let Some(over) = self.win.plates.first().copied() else { break };
            let before = self.win.plates.clone();
            self.wheel(over, vec2(0.0, -200.0), Modifiers::default());
            self.settle();
            scrolled += 1;
            if self.win.plates == before || scrolled > 10 {
                break;
            }
        }
        for words in self.hints_in_more() {
            if !hints.contains(&words) {
                hints.push(words);
            }
        }
        hints
    }

    /// THE HINT OF WHAT IS UNDER `at`, read as it comes up after the pointer rests there; `None` when nothing
    /// new comes up.
    pub fn hint_at(&mut self, at: Pos2) -> Vec<String> {
        self.settle();
        let before: Vec<String> = self.win.drawn.iter().map(|(t, _)| t.clone()).collect();
        // COUNTED, not looked for: the name of a command in a "More" list may be the very words of its hint, and the
        // hint coming up then writes them a second time
        let rested = self.rest_over(at);
        let mut came: Vec<String> = Vec::new();
        for t in rested.risen_over(&before) {
            if !t.is_empty() && !came.contains(&t) {
                came.push(t);
            }
        }
        came
    }

    /// EVERY WIDGET ON SCREEN once it is at rest, as the window describes them to a screen reader.
    pub fn widgets(&mut self) -> Vec<Widget> {
        self.settle();
        self.win.widgets.clone()
    }

    /// THE FIELD A PERSON FILLS UNDER THE CAPTION `label`: the text field, spin box, list or checkbox that stands
    /// nearest after the caption - on its row to the right first, below it otherwise.
    ///
    /// # Panics
    /// When `label` is not on screen or nothing to fill stands after it.
    pub fn field(&mut self, label: &str) -> Widget {
        self.settle();
        let captions = self.places_of(label);
        let inputs: Vec<Widget> = self.win.widgets.iter().filter(|w| matches!(w.kind, Kind::TextField | Kind::Number | Kind::ComboBox | Kind::Slider)).cloned().collect();
        let after = |caption: &Rect, w: &Widget| {
            let same_row = w.rect.min.y < caption.max.y && w.rect.max.y > caption.min.y && w.rect.min.x >= caption.max.x - 1.0;
            let below = w.rect.min.y >= caption.max.y - 1.0 && w.rect.min.x < caption.max.x;
            if same_row {
                Some(w.rect.min.x - caption.max.x)
            } else if below {
                Some(1000.0 + w.rect.min.y - caption.max.y)
            } else {
                None
            }
        };
        let best = captions.iter().flat_map(|c| inputs.iter().filter_map(move |w| after(c, w).map(|d| (d, w)))).min_by(|a, b| a.0.total_cmp(&b.0));
        match best {
            Some((_, w)) => w.clone(),
            None => panic!("nothing to fill stands after {label:?} (at {captions:?}); fields on screen: {inputs:?}"),
        }
    }

    /// FILL THE FIELD under the caption `label` with `text`: click into it, select what is there, type over it.
    /// Nothing is pressed after - Enter and Tab are the check's to press.
    pub fn fill(&mut self, label: &str, text: &str) -> &mut Self {
        let field = self.field(label);
        self.click(field.rect.center()).chord(Modifiers::COMMAND, Key::A).type_over(text)
    }

    /// TYPE `text` OVER WHAT IS SELECTED - nothing typed clears it, as Backspace does: typing no letters over a
    /// selection leaves it standing, which is not what a person emptying a field does.
    pub(crate) fn type_over(&mut self, text: &str) -> &mut Self {
        if text.is_empty() {
            self.key(Key::Backspace)
        } else {
            self.type_text(text)
        }
    }

    /// FILL THE EMPTY FIELD that shows `placeholder` in grey - the name of a new row, say - with `text`.
    ///
    /// # Panics
    /// When no empty field shows it, or more than one does.
    pub fn fill_empty(&mut self, placeholder: &str, text: &str) -> &mut Self {
        self.settle();
        let fields: Vec<Widget> = self.win.widgets.iter().filter(|w| w.kind == Kind::TextField && w.placeholder == placeholder && w.value.is_empty()).cloned().collect();
        match fields.as_slice() {
            [one] => self.click(one.rect.center()).chord(Modifiers::COMMAND, Key::A).type_over(text),
            [] => panic!("no empty field shows {placeholder:?}"),
            many => panic!("{} empty fields show {placeholder:?}: {many:?}", many.len()),
        }
    }

    /// FILL THE FIELD THAT SHOWS `placeholder` in grey - a field of a dimension holds a number and still shows what it
    /// would take - with `text`, over what it holds.
    ///
    /// # Panics
    /// When no field shows it, or more than one does.
    pub fn fill_hinted(&mut self, placeholder: &str, text: &str) -> &mut Self {
        self.settle();
        let fields: Vec<Widget> = self.win.widgets.iter().filter(|w| w.kind == Kind::TextField && w.placeholder == placeholder).cloned().collect();
        match fields.as_slice() {
            [one] => self.click(one.rect.center()).chord(Modifiers::COMMAND, Key::A).type_over(text),
            [] => panic!("no field shows {placeholder:?}"),
            many => panic!("{} fields show {placeholder:?}: {many:?}", many.len()),
        }
    }

    /// FILL THE FIRST FIELD, in reading order, THAT SHOWS `placeholder` in grey with `text`, over what it holds: the
    /// first value of a bar that holds two fields of the same grey words - the size of a chamfer and its second leg.
    ///
    /// # Panics
    /// When no field shows it.
    pub fn fill_first_hinted(&mut self, placeholder: &str, text: &str) -> &mut Self {
        self.settle();
        let first = self
            .win
            .widgets
            .iter()
            .filter(|w| w.kind == Kind::TextField && w.placeholder == placeholder)
            .min_by(|a, b| (a.rect.min.y, a.rect.min.x).partial_cmp(&(b.rect.min.y, b.rect.min.x)).unwrap_or(std::cmp::Ordering::Equal))
            .cloned();
        match first {
            Some(one) => self.click(one.rect.center()).chord(Modifiers::COMMAND, Key::A).type_over(text),
            None => panic!("no field shows {placeholder:?}"),
        }
    }

    /// TICK OR UNTICK THE CHECKBOX whose words are `label`.
    ///
    /// # Panics
    /// When no checkbox has those words.
    pub fn toggle(&mut self, label: &str) -> &mut Self {
        self.settle();
        let boxes: Vec<Widget> = self.win.widgets.iter().filter(|w| w.kind == Kind::CheckBox && w.label == label).cloned().collect();
        match boxes.as_slice() {
            [one] => self.click(one.rect.center()),
            [] => panic!("no checkbox says {label:?}; checkboxes on screen: {:?}", self.win.widgets.iter().filter(|w| w.kind == Kind::CheckBox).map(|w| &w.label).collect::<Vec<_>>()),
            many => panic!("{} checkboxes say {label:?}: {many:?}", many.len()),
        }
    }
}
