//! THE START SCREEN.
//!
//! The program used to open straight into an empty document: the first thing a person saw told them
//! nothing — not what to open, not where to begin, not where the help is. Now it is a screen with the
//! recent files, two kinds of new document, opening, and the parts library.
//!
//! IT IS SHOWN ONLY ON A BLANK SLATE. A screen over somebody's work is a modal that gets closed
//! without being read; so it goes out as soon as anything appears in the document, and does not come
//! back by itself. It can be brought back from a menu item — but it will never stand between a person
//! and their geometry.
//!
//! THERE IS NO "EXAMPLES" SECTION HERE, AND THAT IS HONEST. An examples section was asked for, but
//! there is not one example project in the distribution: drawing an empty section would mean lying on
//! the very first screen. In its place is the parts library, which the distribution DOES have.
use super::{App, Nav};
use egui_phosphor::regular as ph;

impl App {
    /// Is the start screen visible right now.
    ///
    /// The condition is more than a flag: a document that already holds something NEVER shows the
    /// screen, even if the flag stayed raised. That way "over somebody's work" becomes an
    /// inexpressible state rather than a promise to be careful.
    pub(crate) fn start_screen_visible(&self) -> bool {
        // ASKED FOR MEANS SHOWN. The rule below is about a screen that raises ITSELF.
        self.win.start_asked || (self.win.start && self.project.timeline.is_empty() && self.project_path.is_none())
    }

    /// The start screen: the recent files on the left, the actions on the right.
    pub(crate) fn start_screen(&mut self, ctx: &egui::Context) {
        if !self.start_screen_visible() {
            return;
        }
        let mut close = false;
        // THE SIZE IS FIXED RATHER THAN "BY CONTENT". A window with `default_width` grows to the
        // demands of its innards anyway: a separator and a button in a vertical column ask for ALL the
        // available width, and what is available inside a horizontal row is the width of the screen.
        // The reported result was a screen swollen to the whole window and hanging over its edges.
        //
        // The columns are given their width from above as well (`set_width` rather than
        // `set_min_width`): a minimum grows, an exact size does not.
        const COL_L: f32 = 300.0;
        const COL_R: f32 = 240.0;
        egui::Window::new(format!("{} {}", ph::HOUSE, crate::i18n::tr("start-title")))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .fixed_size([COL_L + COL_R + 40.0, 320.0])
            .show(ctx, |ui| {
                ui.horizontal_top(|ui| {
                    // ON THE LEFT, THE RECENT FILES. The first thing people come here for: carrying
                    // on with yesterday.
                    ui.vertical(|ui| {
                        ui.set_width(COL_L);
                        ui.label(egui::RichText::new(crate::i18n::tr("start-recent")).strong());
                        ui.separator();
                        let recent = self.set.recent.clone();
                        if recent.is_empty() {
                            ui.label(egui::RichText::new(crate::i18n::tr("start-recent-empty")).weak().small());
                        }
                        for path in &recent {
                            let name = std::path::Path::new(path).file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| path.clone());
                            // THE NAME IS TRIMMED: a path to a project can be longer than any
                            // reasonable window, and a button grows to fit its text. The full path
                            // stays in the tooltip.
                            let short: String = if name.chars().count() > 34 { format!("{}…", name.chars().take(33).collect::<String>()) } else { name.clone() };
                            if ui.button(format!("{}  {short}", ph::FILE)).on_hover_text(path).clicked() {
                                self.request_nav(Nav::OpenPath(path.clone()), ctx);
                                close = true;
                            }
                        }
                    });
                    ui.separator();
                    // ON THE RIGHT, WHERE TO BEGIN.
                    ui.vertical(|ui| {
                        ui.set_width(COL_R);
                        ui.label(egui::RichText::new(crate::i18n::tr("start-begin")).strong());
                        ui.separator();
                        if ui.button(format!("{}  {}", ph::CUBE, crate::i18n::tr("start-new-part"))).clicked() {
                            self.request_nav(Nav::New, ctx);
                            close = true;
                        }
                        if ui.button(format!("{}  {}", ph::PACKAGE, crate::i18n::tr("start-new-assembly"))).clicked() {
                            self.request_nav(Nav::NewAssembly, ctx);
                            close = true;
                        }
                        if ui.button(format!("{}  {}", ph::FOLDER_OPEN, crate::i18n::tr("start-open"))).clicked() {
                            self.request_nav(Nav::OpenDialog, ctx);
                            close = true;
                        }
                        if ui.button(format!("{}  {}", ph::PACKAGE, crate::i18n::tr("start-library"))).clicked() {
                            self.toggle_parts_library();
                            close = true;
                        }
                        ui.separator();
                        // LEARNING STARTS HERE. The start screen used to be about files: create,
                        // open, recent. Somebody opening the CAD for the first time needs to go not
                        // into files but into the first lesson — and it now exists
                        // (`start/01-first-part`).
                        //
                        // The lesson stands above the help DELIBERATELY: the help answers "how does
                        // this work", and the lesson answers "what do I do first", and it is the
                        // latter that is needed first.
                        if ui.button(format!("{}  {}", ph::GRADUATION_CAP, crate::i18n::tr("start-first-lesson"))).on_hover_text(&crate::i18n::tr("start-first-lesson-hint")).clicked() {
                            self.open_help("start/01-first-part");
                            close = true;
                        }
                        if ui.button(format!("{}  {}", ph::BOOK_OPEN, crate::i18n::tr("start-help"))).clicked() {
                            self.open_help("index");
                            close = true;
                        }
                        if ui.button(format!("{}  {}", ph::KEYBOARD, crate::i18n::tr("start-hotkeys"))).clicked() {
                            self.win.hotkeys = true;
                            close = true;
                        }
                        ui.hyperlink_to(format!("{}  {}", ph::BOOK_OPEN, crate::i18n::tr("start-help-site")), "https://cad.qymis.tech");
                    });
                });
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button(crate::i18n::tr("start-close")).clicked() {
                        close = true;
                    }
                    ui.label(egui::RichText::new(crate::i18n::tr("start-hint")).weak().small());
                });
            });
        if close || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.win.start = false;
            self.win.start_asked = false;
        }
    }
}

/// THE START SCREEN LEADS TO LEARNING.
///
/// It was entirely about files: create, open, recent, a link to the site. Somebody opening the CAD for
/// the first time needs to go to the first lesson — and it was not on the screen at all, even though
/// the lesson itself was already written.
///
/// A hint in the empty viewport was decided against as too much — the start screen is what serves that
/// purpose.
#[cfg(test)]
mod tests {
    use super::super::App;

    /// THE FIRST LESSON OPENS AS THE LESSON ITSELF rather than as a table of contents.
    ///
    /// A table of contents is one more choice for somebody who does not yet know what there is to
    /// choose from.
    #[test]
    fn the_first_lesson_opens_the_lesson_itself() {
        let mut app = App::default();
        app.open_help("start/01-first-part");
        assert!(app.help_open_for_test(), "the help did not open");
        assert_eq!(app.help_article_for_test(), "start/01-first-part", "the wrong article opened");
        assert!(crate::help::article("start/01-first-part").is_some(), "there is no article for the first lesson — the button would lead into emptiness");
    }

    /// AND BOTH BUTTONS ARE ON THE SCREEN.
    #[test]
    fn the_start_screen_offers_the_lesson_and_the_help() {
        let prev = crate::i18n::language();
        crate::i18n::set_language("ru");
        let mut app = App::default();
        app.set_show_start_for_test(true);
        let texts = super::super::screen_keys::tests::frame_text(&mut app, |a, c| a.start_screen(c));
        let (lesson, help) = (crate::i18n::tr("start-first-lesson"), crate::i18n::tr("start-help"));
        crate::i18n::set_language(&prev);
        assert!(texts.iter().any(|t| t.contains(&lesson)), "the start screen has no \"{lesson}\": {texts:?}");
        assert!(texts.iter().any(|t| t.contains(&help)), "the start screen has no \"{help}\"");
    }

    /// THE LESSON STANDS ABOVE THE HELP. The help answers "how does this work", the lesson answers
    /// "what do I do first"; it is the latter that is needed first, and the order of the buttons must
    /// say so.
    #[test]
    fn the_lesson_comes_before_the_help() {
        let src = include_str!("start_screen.rs");
        let lesson = src.find("start-first-lesson").expect("the lesson button");
        let help = src.find("\"start-help\"").expect("the help button");
        assert!(lesson < help, "the help stands above the lesson — a newcomer will go to the reference instead of the lesson");
    }
}
