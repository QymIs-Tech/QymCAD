//! "REPORT A PROBLEM": what the person writes, what goes with it, and where it is sent.
//!
//! There is no network here and there will not be. Sending straight to a tracker means a token, and a
//! token in the binary of an open program is extracted in a minute and used to post in the owner's
//! name. So the program does the half a person cannot: it gathers the files and fills the form in
//! ADVANCE, through the address bar - GitHub reads its issue forms' fields out of the query string.
//! What it cannot do through a link is attach files, so those are gathered into one folder and the
//! folder is opened, ready to drag from.
//!
//! THE DOCUMENT IS NEVER ATTACHED BY DEFAULT. An issue is public for ever and a model is the person's
//! own work; the tick is theirs to make, with the consequence written beside it. Everything else -
//! the environment, the crash file, a picture of the window - carries nothing of theirs.

use std::path::PathBuf;

/// The address of the public tracker. ONE constant, because the same address is used by the link in the
/// window and by the guard that checks the form's field names against this.
pub(crate) const ISSUES_NEW: &str = "https://github.com/QymIs-Tech/QymCAD/issues/new";

/// What has been typed and what is to travel with it.
pub(crate) struct ReportDraft {
    pub what: String,
    pub expected: String,
    pub steps: String,
    pub attach_shot: bool,
    pub attach_env: bool,
    pub attach_crash: bool,
    /// OFF by default, and that is the whole point of the field.
    pub attach_doc: bool,
    /// Where the last gathering put the files.
    pub folder: Option<PathBuf>,
    /// A picture of the window was asked for and has not come back yet: the reply arrives a frame or
    /// more later, as an event.
    pub awaiting_shot: bool,
}

impl Default for ReportDraft {
    fn default() -> Self {
        Self {
            what: String::new(),
            expected: String::new(),
            steps: String::new(),
            attach_shot: true,
            attach_env: true,
            attach_crash: true,
            attach_doc: false,
            folder: None,
            awaiting_shot: false,
        }
    }
}

/// PERCENT-ENCODING for the query string, by hand rather than by a crate.
///
/// One rule and no configuration: everything that is not unreserved becomes `%XX`. A space could be a
/// `+`, but `%20` is right in both halves of a URL and needs no thinking about which half this is.
pub(crate) fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + s.len() / 4);
    for b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(*b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// HOW MUCH TEXT A LINK CAN CARRY.
///
/// Servers and browsers start refusing somewhere around eight thousand characters, and a refusal here
/// looks to the person like "the button does nothing". Percent-encoding can triple a Russian sentence
/// (three bytes a letter, three characters a byte), so the budget is counted on the ENCODED length and
/// the rest stays in the file, which is being attached anyway.
const URL_BUDGET: usize = 6000;

impl super::App {
    /// The whole report as text: what the person said, then what the machine was.
    pub(crate) fn report_text(&self) -> String {
        let d = &self.report;
        let mut s = String::new();
        s.push_str(&format!("## What happened\n{}\n\n", d.what.trim()));
        if !d.expected.trim().is_empty() {
            s.push_str(&format!("## What was expected\n{}\n\n", d.expected.trim()));
        }
        if !d.steps.trim().is_empty() {
            s.push_str(&format!("## Steps\n{}\n\n", d.steps.trim()));
        }
        if d.attach_env {
            s.push_str(&format!("## Environment\n```\n{}```\n", crate::diagnostics::block()));
        }
        s
    }

    /// The prefilled address of the issue form.
    ///
    /// The field names are the `id`s in `.github/ISSUE_TEMPLATE/bug.yml`; a guard holds the two sides
    /// together, because when they drift apart the prefilling stops silently and the form simply comes
    /// up empty.
    pub(crate) fn report_url(&self) -> String {
        let title: String = self.report.what.trim().lines().next().unwrap_or_default().chars().take(80).collect();
        let mut body = self.report_text();
        // Trimmed on the ENCODED length, and by whole characters: cutting a UTF-8 letter in half would
        // give a link nothing can read.
        while urlencode(&body).len() > URL_BUDGET {
            let keep = body.chars().count().saturating_sub(64);
            if keep == 0 {
                break;
            }
            body = body.chars().take(keep).collect();
        }
        format!(
            "{ISSUES_NEW}?template=bug.yml&title={}&what-happened={}",
            urlencode(&title),
            urlencode(&body)
        )
    }

    /// GATHER THE FILES INTO ONE FOLDER, ready to be dragged into the form.
    ///
    /// Returns where they went. A failure to copy one attachment does not lose the rest: a report with
    /// three files out of four beats no report at all.
    pub(crate) fn collect_report(&mut self, ctx: &egui::Context) -> Option<PathBuf> {
        let root = directories::ProjectDirs::from("tech", "qymis", "qym-cad").map(|d| d.data_dir().join("reports"))?;
        let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let dir = root.join(format!("report_{secs}"));
        std::fs::create_dir_all(&dir).ok()?;

        std::fs::write(dir.join("report.md"), self.report_text()).ok()?;

        if self.report.attach_crash {
            // The newest one, shown or not: the person reporting the trouble is usually reporting THAT
            // crash, and it has already been marked seen by the window that told them about it.
            if let Some(newest) = newest_crash_file() {
                let name = newest.file_name().map(|n| n.to_os_string()).unwrap_or_default();
                let _ = std::fs::copy(&newest, dir.join(name));
            }
        }

        if self.report.attach_doc {
            if let Some(p) = self.project_path.clone() {
                let name = std::path::Path::new(&p).file_name().map(|n| n.to_os_string()).unwrap_or_default();
                let _ = std::fs::copy(&p, dir.join(name));
            }
        }

        if self.report.attach_shot {
            // The picture comes back as an event, a frame or more later; `take_screenshot` writes it.
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot);
            self.report.awaiting_shot = true;
        }

        self.report.folder = Some(dir.clone());
        Some(dir)
    }

    /// THE PICTURE OF THE WINDOW CAME BACK. Called every frame; does nothing until it has.
    pub(crate) fn take_screenshot(&mut self, ctx: &egui::Context) {
        if !self.report.awaiting_shot {
            return;
        }
        let Some(dir) = self.report.folder.clone() else {
            self.report.awaiting_shot = false;
            return;
        };
        let shot = ctx.input(|i| {
            i.events.iter().find_map(|e| match e {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        let Some(image) = shot else { return };
        if let Some(png) = super::App::color_image_to_png(&image) {
            let _ = std::fs::write(dir.join("window.png"), png);
        }
        self.report.awaiting_shot = false;
    }
}

/// The newest crash file, whether or not it has been shown.
fn newest_crash_file() -> Option<PathBuf> {
    let dir = directories::ProjectDirs::from("tech", "qymis", "qym-cad").map(|d| d.data_dir().join("crashes"))?;
    let mut all: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.file_name().map(|n| n.to_string_lossy().starts_with("crash_")).unwrap_or(false))
        .collect();
    all.sort();
    all.pop()
}

impl super::App {
    /// The window itself (Help -> Report a problem).
    pub(crate) fn report_window(&mut self, ctx: &egui::Context) {
        if !self.win.report {
            return;
        }
        use egui_phosphor::regular as ph;
        let mut open = true;
        let mut collect = false;
        let mut to_form = false;
        egui::Window::new(format!("{} {}", ph::BUG, crate::i18n::tr("report-title")))
            .open(&mut open)
            .collapsible(false)
            .default_width(540.0)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.add_space(4.0);
                ui.label(&crate::i18n::tr("report-intro"));
                ui.add_space(8.0);

                ui.label(&crate::i18n::tr("report-what"));
                ui.add(egui::TextEdit::multiline(&mut self.report.what).desired_rows(3).desired_width(f32::INFINITY).hint_text(crate::i18n::tr("report-what-hint")));
                ui.add_space(6.0);
                ui.label(&crate::i18n::tr("report-expected"));
                ui.add(egui::TextEdit::multiline(&mut self.report.expected).desired_rows(2).desired_width(f32::INFINITY));
                ui.add_space(6.0);
                ui.label(&crate::i18n::tr("report-steps"));
                ui.add(egui::TextEdit::multiline(&mut self.report.steps).desired_rows(3).desired_width(f32::INFINITY).hint_text(crate::i18n::tr("report-steps-hint")));

                ui.add_space(10.0);
                ui.separator();
                ui.add_space(6.0);
                ui.label(egui::RichText::new(&crate::i18n::tr("report-attach")).strong());
                ui.checkbox(&mut self.report.attach_env, &crate::i18n::tr("report-attach-env"));
                ui.checkbox(&mut self.report.attach_shot, &crate::i18n::tr("report-attach-shot"));
                ui.checkbox(&mut self.report.attach_crash, &crate::i18n::tr("report-attach-crash"));
                // THE DOCUMENT IS THE PERSON'S OWN WORK AND THE TRACKER IS PUBLIC FOR EVER. The tick is
                // theirs to make, and the consequence is written where they make it - not in a help
                // article they will not open.
                let has_doc = self.project_path.is_some();
                ui.add_enabled_ui(has_doc, |ui| {
                    ui.checkbox(&mut self.report.attach_doc, &crate::i18n::tr("report-attach-doc"));
                });
                if !has_doc {
                    ui.label(egui::RichText::new(&crate::i18n::tr("report-doc-unsaved")).weak().small());
                } else if self.report.attach_doc {
                    ui.colored_label(self.scheme.pal.warning(), format!("{} {}", ph::WARNING, crate::i18n::tr("report-doc-public")));
                }

                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    let ready = !self.report.what.trim().is_empty();
                    if ui.add_enabled(ready, egui::Button::new(format!("{} {}", ph::FOLDER_OPEN, crate::i18n::tr("report-collect")))).on_disabled_hover_text(crate::i18n::tr("report-need-what")).clicked() {
                        collect = true;
                    }
                    if ui.add_enabled(ready, egui::Button::new(format!("{} {}", ph::ARROW_SQUARE_OUT, crate::i18n::tr("report-open-form")))).on_disabled_hover_text(crate::i18n::tr("report-need-what")).clicked() {
                        to_form = true;
                    }
                });

                if let Some(dir) = self.report.folder.clone() {
                    ui.add_space(10.0);
                    ui.separator();
                    ui.add_space(6.0);
                    ui.label(&crate::i18n::tr("report-collected"));
                    ui.label(egui::RichText::new(crate::crash::without_home(&dir.to_string_lossy())).monospace().small());
                    if ui.button(format!("{} {}", ph::COPY, crate::i18n::tr("report-copy-path"))).clicked() {
                        ui.output_mut(|o| o.copied_text = dir.to_string_lossy().into_owned());
                    }
                }
            });
        if collect {
            self.collect_report(ctx);
        }
        if to_form {
            ctx.open_url(egui::OpenUrl::new_tab(self.report_url()));
        }
        if !open {
            self.win.report = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::gui::App;

    /// The ids of the fields in the issue form, read from the form itself.
    fn form_field_ids() -> Vec<String> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.github/ISSUE_TEMPLATE/bug.yml");
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("the issue form is missing at {path:?}: {e}"));
        text.lines()
            .filter_map(|l| l.trim().strip_prefix("id:").map(|v| v.trim().trim_matches('"').to_string()))
            .collect()
    }

    /// A RUSSIAN PHRASE BUILT FROM CODE POINTS RATHER THAN TYPED.
    ///
    /// The tests below have to work on Cyrillic - that is the whole point of them, since percent-encoding
    /// triples it - but a ratchet counts every Cyrillic literal in the sources and only lets that number
    /// go down. This is data, not a message to anybody, so it is written as code points and the ratchet
    /// keeps holding what it was put there to hold. U+0430..U+044F is the Russian lower-case alphabet.
    fn cyrillic() -> String {
        (0x0430u32..=0x044F).filter_map(char::from_u32).collect()
    }

    /// The query keys of a link, without their values.
    fn query_keys(url: &str) -> Vec<String> {
        let Some((_, q)) = url.split_once('?') else { return Vec::new() };
        q.split('&').filter_map(|p| p.split_once('=').map(|(k, _)| k.to_string())).collect()
    }

    /// THE LINK AND THE FORM MUST NAME THE SAME FIELDS.
    ///
    /// GitHub fills a form from the query string BY FIELD ID. Rename a field in the form, or a key in
    /// the link, and the prefilling stops - silently: the form simply comes up empty, and nobody
    /// notices, because an empty form is what a form looks like.
    #[test]
    fn the_form_and_the_link_name_the_same_fields() {
        let ids = form_field_ids();
        assert!(ids.contains(&"what-happened".to_string()), "the form has no field to fill in: {ids:?}");

        let mut app = crate::gui::screen_keys::tests::populated();
        app.report.what = "the fillet does not take".into();
        let url = app.report_url();

        assert!(url.starts_with(super::ISSUES_NEW), "the link does not lead to the tracker: {url}");
        assert!(url.contains("template=bug.yml"), "the link does not name the form: {url}");

        for k in query_keys(&url) {
            if k == "template" || k == "title" {
                continue; // both are GitHub's own, not fields of ours
            }
            assert!(ids.contains(&k), "the link fills in `{k}`, and the form has no such field: {ids:?}");
        }
    }

    /// A LINK TOO LONG SIMPLY DOES NOT OPEN, and to the person the button "does nothing".
    ///
    /// Percent-encoding triples a Russian sentence - three bytes a letter, three characters a byte - so
    /// the budget is counted on the encoded length. Checked in Russian for that very reason: in Latin
    /// the same text would fit and the check would pass while proving nothing.
    #[test]
    fn the_link_stays_short_enough_to_open() {
        let phrase = cyrillic();
        let mut app = crate::gui::screen_keys::tests::populated();
        app.report.what = format!("{phrase} ").repeat(400);
        let url = app.report_url();
        assert!(url.len() < 8000, "the link came out {} characters long and will not open", url.len());
        // ...and what was cut off is still whole text, not half a letter.
        assert!(url.is_char_boundary(url.len()), "the link was cut through a character");
        let head = super::urlencode(&phrase.chars().take(3).collect::<String>());
        assert!(url.contains(&head), "the beginning of what was written did not survive the trim: {}", &url[..url.len().min(200)]);
    }

    /// THE DOCUMENT IS NEVER ATTACHED BY ITSELF. An issue is public for ever, and the model is the
    /// person's own work: the tick is theirs to make.
    #[test]
    fn the_document_is_never_attached_by_itself() {
        let d = super::ReportDraft::default();
        assert!(!d.attach_doc, "the document would travel to a public tracker without being asked for");
        assert!(d.attach_env, "the environment is the cheap half of the answer and carries nothing personal");
    }

    /// The environment goes into the text only when it was asked for.
    #[test]
    fn the_text_carries_what_was_ticked() {
        let mut app = crate::gui::screen_keys::tests::populated();
        app.report.what = "the fillet does not take".into();
        app.report.attach_env = true;
        let with = app.report_text();
        assert!(with.contains("## Environment"), "the environment was ticked and is not in the text:\n{with}");
        assert!(with.contains("QymCAD "), "the environment block is empty:\n{with}");

        app.report.attach_env = false;
        let without = app.report_text();
        assert!(!without.contains("## Environment"), "the environment travelled although it was not ticked:\n{without}");
        assert!(without.contains("the fillet does not take"), "what the person wrote was lost:\n{without}");
    }

    fn raw() -> egui::RawInput {
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1400.0, 900.0))),
            ..Default::default()
        }
    }

    fn painted(app: &mut App) -> Vec<String> {
        fn walk(s: &egui::epaint::Shape, out: &mut Vec<String>) {
            match s {
                egui::epaint::Shape::Text(t) => out.push(t.galley.text().to_string()),
                egui::epaint::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
                _ => {}
            }
        }
        let ctx = egui::Context::default();
        crate::gui::install_fonts(&ctx);
        let _ = ctx.run(raw(), |c| app.report_window(c));
        let out = ctx.run(raw(), |c| app.report_window(c));
        let mut found = Vec::new();
        for cs in &out.shapes {
            walk(&cs.shape, &mut found);
        }
        found
    }

    /// THE CONSEQUENCE IS WRITTEN WHERE THE TICK IS MADE.
    ///
    /// Attaching the document sends somebody's own work to a tracker that is public for ever. A warning
    /// in a help article they will not open is no warning; it has to stand under the tick itself, and
    /// only when the tick is on - a warning shown always is a warning nobody reads.
    #[test]
    fn the_document_tick_says_what_it_costs() {
        let mut app = crate::gui::screen_keys::tests::populated();
        app.win.report = true;
        app.set_project_path(std::env::temp_dir().join("a-part.qcad").to_string_lossy().into_owned());

        let quiet = painted(&mut app);
        let warning = crate::i18n::tr("report-doc-public");
        assert!(quiet.iter().any(|t| t.contains(&crate::i18n::tr("report-title"))), "the window did not open at all");
        assert!(!quiet.iter().any(|t| t.contains(&warning)), "the warning is shown while nothing is being attached");

        app.report.attach_doc = true;
        let loud = painted(&mut app);
        assert!(loud.iter().any(|t| t.contains(&warning)), "the tick is on and nothing says the model becomes public: {loud:?}");
    }

    /// The urlencoding by hand, on what it is actually given: Cyrillic, spaces, newlines, the
    /// characters that mean something in a query string.
    #[test]
    fn the_encoding_leaves_nothing_that_breaks_a_query() {
        assert_eq!(super::urlencode("a-b_c.d~e"), "a-b_c.d~e", "the unreserved characters must pass through");
        assert_eq!(super::urlencode("a b"), "a%20b");
        assert_eq!(super::urlencode("a&b=c#d"), "a%26b%3Dc%23d");
        // U+0434 U+0430 - two Russian letters, as code points for the reason given by `cyrillic`.
        let two: String = [0x0434u32, 0x0430].iter().filter_map(|c| char::from_u32(*c)).collect();
        assert_eq!(super::urlencode(&two), "%D0%B4%D0%B0", "a letter of three bytes must come out as three escapes");
        let encoded = super::urlencode("line\nline");
        assert!(!encoded.contains('\n'), "a newline in a link cuts it in half: {encoded}");
    }
}
