//! THE ONE VOICE OF THE PROGRAM: a message carries its tone, and one place says what a tone looks like.
//!
//! Before it, every place that said "this failed" picked its colour and its icon itself: measured on 2026-10-15,
//! the theme's error colour in 13 places, its milder error colour in 23, its warning colour in 10, egui's copies of
//! the same two colours in 34, and a hard-coded red and green in one dialog. The status line drew everything in
//! the plain colour, so a rebuild that left a cut red said so in the same tone as "Ready". Here the tone is a
//! thing of its own: the status line, the panels, the popups and the windows hand a [`Notice`] or a [`Tone`] to
//! these functions, and only these functions turn a tone into a colour and an icon.
//!
//! What each tone takes from the theme:
//!
//! | tone      | colour                   | icon            | meaning                                              |
//! |-----------|--------------------------|-----------------|------------------------------------------------------|
//! | `Info`    | the plain text colour    | none            | what happened, what to do next                       |
//! | `Warning` | `warning` (yellow)       | `ph::WARNING`   | done, but not all of it; something to look at        |
//! | `Refusal` | `error_mild` (soft red)  | `ph::X`         | a value or a pick was not taken; nothing broke       |
//! | `Error`   | `error` (red)            | `ph::X_CIRCLE`  | something did not build or does not hold             |
//!
//! `Refusal` is the soft red the theme keeps for "this is not right" said at a field: a formula that does not
//! evaluate, a name that is taken. It is not counted among the faults of the document - the document is unchanged.

use egui_phosphor::regular as ph;

/// HOW A MESSAGE IS MEANT. An enum rather than a guess from the words: a message that names no tone does not compile.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Tone {
    #[default]
    Info,
    Warning,
    Refusal,
    Error,
}

/// A THING THE PROGRAM SAYS: its words and how they are meant.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Notice {
    pub tone: Tone,
    pub words: String,
}

impl Notice {
    pub fn info(words: impl Into<String>) -> Self {
        Self { tone: Tone::Info, words: words.into() }
    }

    pub fn warning(words: impl Into<String>) -> Self {
        Self { tone: Tone::Warning, words: words.into() }
    }

    pub fn refusal(words: impl Into<String>) -> Self {
        Self { tone: Tone::Refusal, words: words.into() }
    }

    pub fn error(words: impl Into<String>) -> Self {
        Self { tone: Tone::Error, words: words.into() }
    }

    /// Nothing is said.
    pub fn is_empty(&self) -> bool {
        self.words.is_empty()
    }
}

/// THE WORDS ALONE, for a line of a log or a check's message: what is drawn goes through [`say`].
impl std::fmt::Display for Notice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.words)
    }
}

/// HOW MUCH ROOM the words of a message take.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoticeSize {
    /// A line of its own in the body text: the status line, a dialog.
    Line,
    /// Small text under a field or in a panel.
    Small,
}

/// THE COLOUR OF A TONE in the current theme - for what is not a label: a row of the tree, a frame round a card, a
/// line drawn by the painter. A light and a dark theme give different colours; no constant stands in for them.
pub fn tone_colour(pal: &qymcad_scheme::Palette, tone: Tone) -> egui::Color32 {
    match tone {
        Tone::Info => pal.ui_text(),
        Tone::Warning => pal.warning(),
        Tone::Refusal => pal.error_mild(),
        Tone::Error => pal.error(),
    }
}

/// THE COLOUR OF A TONE WHERE ONLY THE `Ui` IS AT HAND - a list deep inside a field, a row of a list that is not
/// handed the palette. The theme writes its warning and error colours into egui's (`qymcad_scheme`, where the visuals
/// are made), so these are the theme's own; a refusal has no slot of egui's and takes the error colour there.
pub fn tone_colour_in(ui: &egui::Ui, tone: Tone) -> egui::Color32 {
    match tone {
        Tone::Info => ui.visuals().text_color(),
        Tone::Warning => ui.visuals().warn_fg_color,
        Tone::Refusal | Tone::Error => ui.visuals().error_fg_color,
    }
}

/// THE ICON OF A TONE, or none for information.
pub fn tone_icon(tone: Tone) -> Option<&'static str> {
    match tone {
        Tone::Info => None,
        Tone::Warning => Some(ph::WARNING),
        Tone::Refusal => Some(ph::X),
        Tone::Error => Some(ph::X_CIRCLE),
    }
}

/// THE WORDS OF A MESSAGE as text to put anywhere a text goes - a label, a wrapped label, a header: its icon in
/// front and the colour of its tone. Information keeps the colour of the place it is put in.
pub fn notice_text(pal: &qymcad_scheme::Palette, notice: &Notice, size: NoticeSize) -> egui::RichText {
    let words = match tone_icon(notice.tone) {
        Some(icon) => format!("{icon} {}", notice.words),
        None => notice.words.clone(),
    };
    let text = egui::RichText::new(words);
    let text = if size == NoticeSize::Small { text.small() } else { text };
    if notice.tone == Tone::Info {
        text
    } else {
        text.color(tone_colour(pal, notice.tone))
    }
}

/// SAY A MESSAGE: its icon and its words, in the colour of its tone. The response carries the hint a caller adds.
pub fn say(ui: &mut egui::Ui, pal: &qymcad_scheme::Palette, notice: &Notice, size: NoticeSize) -> egui::Response {
    ui.label(notice_text(pal, notice, size))
}

/// THE MARK OF A TONE ALONE, where the words do not fit - a narrow cell of a table, the end of a field. The words go
/// into the hint the caller adds to the response.
pub fn tone_mark(ui: &mut egui::Ui, pal: &qymcad_scheme::Palette, tone: Tone) -> egui::Response {
    ui.label(egui::RichText::new(tone_icon(tone).unwrap_or(ph::INFO)).color(tone_colour(pal, tone)))
}

/// THE MARK OF A TONE where only the `Ui` is at hand (see `tone_colour_in`).
pub fn tone_mark_in(ui: &mut egui::Ui, tone: Tone) -> egui::Response {
    let colour = tone_colour_in(ui, tone);
    ui.label(egui::RichText::new(tone_icon(tone).unwrap_or(ph::INFO)).color(colour))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// THE COLOUR OF A TONE FOLLOWS THE THEME: a light and a dark theme give different colours for a warning and an
    /// error, and each is the theme's own.
    #[test]
    fn a_tone_takes_its_colour_from_the_theme() {
        let (dark, light) = (&qymcad_scheme::dark(), &qymcad_scheme::light());
        for tone in [Tone::Warning, Tone::Refusal, Tone::Error] {
            assert_ne!(tone_colour(dark, tone), tone_colour(light, tone), "{tone:?} is the same colour in a light and a dark theme");
        }
        assert_eq!(tone_colour(dark, Tone::Error), dark.error());
        assert_eq!(tone_colour(light, Tone::Warning), light.warning());
    }

    /// EVERY TONE BUT INFORMATION CARRIES AN ICON, and no two the same one.
    #[test]
    fn a_fault_carries_its_own_icon() {
        let icons: Vec<_> = [Tone::Warning, Tone::Refusal, Tone::Error].into_iter().map(|t| tone_icon(t).expect("an icon")).collect();
        assert!(icons.iter().enumerate().all(|(i, a)| icons.iter().skip(i + 1).all(|b| a != b)), "{icons:?}");
        assert_eq!(tone_icon(Tone::Info), None);
    }
}
