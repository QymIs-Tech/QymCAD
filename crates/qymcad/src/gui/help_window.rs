//! THE HELP WINDOW: an article from `docs/help`, drawn in our own colours and fonts.
//!
//! The rendering is ours rather than a crate's, and that is visible here: every kind of text takes its
//! colour from THE SCHEME. Someone else's renderer would bring a palette of its own and drift apart from
//! the light theme exactly as the canvas of the viewport once did.
use super::App;
use crate::help::{Block, Span};
use egui_phosphor::regular as ph;

/// THE MARGINS OF AN ARTICLE, in pixels. Here rather than as a number in two places: a guard checks
/// the indent against this value.
// A WHOLE NUMBER since egui 0.35: frame margins are integers there, and half a pixel of padding was
// never worth anything anyway.
pub(super) const HELP_PAD: i8 = 8;

/// HOW LONG A FRAME of an animation is held. Half a second: faster and it flickers, leaving no time to
/// read what changed; slower and the program looks as though it were thinking.
pub(super) const FRAME_SECS: f64 = 0.5;

/// THE TEXTURE OF A HELP IMAGE, with its memory in the context.
///
/// The cache is in `egui::Context` and not in `App`: it lives exactly as long as the textures do and
/// dies with it. Without a cache every redraw of the window would decode the PNG again — and an
/// animation redraws twice a second.
fn help_texture(ctx: &egui::Context, path: &str) -> Option<egui::TextureHandle> {
    let id = egui::Id::new(("help.img", path));
    if let Some(t) = ctx.data(|d| d.get_temp::<egui::TextureHandle>(id)) {
        return Some(t);
    }
    let bytes = crate::help::image(path)?;
    let rgba = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (w, h) = rgba.dimensions();
    let color = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], rgba.as_raw());
    let tex = ctx.load_texture(format!("help:{path}"), color, egui::TextureOptions::LINEAR);
    ctx.data_mut(|d| d.insert_temp(id, tex.clone()));
    Some(tex)
}

/// WHERE TO OPEN THE HELP — the answer apart from the action.
///
/// The same device as in `reveal_command`: a test cannot and must not check whether a browser started —
/// launching a real process inside a test run is not allowed. What it can check is THE DECISION: where
/// we are headed and with what address. The action after that is trivial.
#[derive(PartialEq, Debug)]
pub(crate) enum HelpTarget {
    Window,
    Site(String),
}

impl App {
    /// Where the help for this article will go under the current settings.
    pub(crate) fn help_target(&self, article: &str) -> HelpTarget {
        if self.set.help_external {
            HelpTarget::Site(crate::help::web_url(article))
        } else {
            HelpTarget::Window
        }
    }

    /// Open the help at a particular article — in our own window or in a browser, as chosen.
    ///
    /// THE FORK IS HERE AND NOT AT EVERY CALLER. The help is opened by F1, by the menu, by the "?"
    /// buttons of the commands; have each of them ask about the setting and one is bound to forget, and
    /// it will forget silently.
    pub(crate) fn open_help(&mut self, article: &str) {
        if let HelpTarget::Site(url) = self.help_target(article) {
            self.launch_browser(&url);
            return;
        }
        self.goto_help(article);
        self.win.help = true;
    }

    /// Facades for the tests: a test looks at the same thing the window does.
    #[cfg(test)]
    pub(crate) fn help_open_for_test(&self) -> bool {
        self.win.help
    }

    /// OPEN THE SAME ARTICLE ON THE SITE.
    ///
    /// The address is derived from the path of the article (`help::web_url`) rather than kept as a list:
    /// the window and the site have ONE source — the same `.md` files — so there is nothing to
    /// diverge.
    pub(crate) fn open_help_on_site(&mut self, article: &str) {
        let url = crate::help::web_url(article);
        self.launch_browser(&url);
    }

    /// Launch the browser. Separated from choosing the address, so that the choice is what gets
    /// tested rather than the launch.
    fn launch_browser(&mut self, url: &str) {
        // NO BROWSER IS LAUNCHED IN A TEST RUN, and that is no concession to the test. The very first
        // edition opened a real browser window on a developer machine — the test reached outside the run;
        // on a machine with no graphics it would instead hang on `xdg-open`. The decision of where to go
        // is checked in full through `help_target`, and only the launch is left here, with nothing in it
        // to check.
        if cfg!(test) {
            self.status = crate::i18n::tr1("help-opened-in-browser", "url", url);
            return;
        }
        let (cmd, args) = super::browse_command(egui::os::OperatingSystem::from_target_os(), url);
        match std::process::Command::new(cmd).args(&args).spawn() {
            Ok(_) => self.status = crate::i18n::tr1("help-opened-in-browser", "url", url),
            // IT DID NOT WORK — THE ADDRESS IS SHOWN. There may be no browser at all (a bare server, a
            // stripped-down environment), and then the only useful thing is the link itself, to be
            // carried off by hand.
            Err(e) => self.status = format!("{} {}", crate::i18n::tr1("help-browser-failed", "url", url), e),
        }
    }

    /// Go to an article, remembering where we came from.
    ///
    /// "Back" is no decoration: the help is walked from article to article and out of the search, and it
    /// must be possible to return to where the reading was. A repeat of the same article is not put into
    /// the history — otherwise the back button would stop moving on a second click on the same row.
    fn goto_help(&mut self, article: &str) {
        if self.win.help_article != article {
            let cur = std::mem::replace(&mut self.win.help_article, article.to_string());
            if !cur.is_empty() {
                self.win.help_back.push(cur);
            }
        }
    }

    /// F1: THE ARTICLE ABOUT WHAT IS BEING DONE RIGHT NOW.
    ///
    /// The order of the answer runs from the particular to the general: the active Part command, then the
    /// active Sketch tool, then the section of the workbench, and only then the contents. A help that
    /// always opens at the title page is not help but an extra click at the very minute somebody is
    /// stuck.
    pub(crate) fn help_for_context(&self) -> &'static str {
        if self.cmd.active() {
            if let Some(a) = crate::help_map::part_article(self.cmd.kind) {
                return a;
            }
        }
        // ASSEMBLY MODES: the Assembly tools are not timeline commands, but they are what a person is
        // busy with, and F1 must answer about them.
        //
        // THE SINGLE LIST IS ASKED rather than a couple of conditions written in by hand. Exactly such an
        // enumeration stood here and knew ONE tool out of nine: take the anchor, the group, the width,
        // the tangency, the relation, the grounding, the pointing at an axis or the re-pick, and F1 gave
        // back the contents — that is, the answer "go and look for it yourself".
        for t in self.armed_assembly_tools() {
            if let Some(a) = crate::help_map::assembly_article(t.help_mode()) {
                return a;
            }
        }
        if self.carr.mode != 0 {
            if let Some(a) = crate::help_map::assembly_article("asm.comp-array") {
                return a;
            }
        }
        if self.sketch_ses.editing.is_some() {
            if self.tool.kind > 0 {
                if let Some(a) = crate::help_map::sketch_article("sk", self.tool.kind) {
                    return a;
                }
            }
            if self.dim.kind > 0 {
                if let Some(a) = crate::help_map::sketch_article("dim", self.dim.kind) {
                    return a;
                }
            }
        }
        // TOOLBAR BUTTONS WITH NO COMMAND NUMBER are an occupied hand as well, and F1 must answer about
        // them. There are 36 of the 91: the boolean of bodies, move/copy/rotate, the array in a sketch,
        // measurement, the section.
        //
        // THIS BRANCH WAS MISSING, AND THE COMPILER SAID SO. The table `help_map::TOOLBAR` was created, a
        // guard and five articles were written against it — and it was never wired in here. The warning
        // that `toolbar_article` is never used stood in the build and drowned among 175 others. The same
        // class as the editing of a component array, found once in exactly the same way.
        if let Some(a) = self.armed_toolbar_hint().and_then(crate::help_map::toolbar_article) {
            return a;
        }
        crate::help_map::workbench_article(self.workbench_code())
    }

    /// WHAT OCCUPIES THE HAND among the things with no command number — by the key of its hint.
    ///
    /// The conditions here are THE SAME ones by which a button in the bar is shown as pressed
    /// (`icon_tool(..., active)`). Otherwise F1 and the highlight of the button would diverge: the button
    /// glows and the help is about something else.
    ///
    /// ACTION buttons (create a part, insert a component) are not here and cannot be: they leave no
    /// state, and there is nothing to ask about during them. Their rows in the table hold a different
    /// promise — that the article is written and will be found through the contents.
    pub(crate) fn armed_toolbar_hint(&self) -> Option<&'static str> {
        if self.boolean.pick.is_some() {
            return Some("tb-bool-bodies-hint");
        }
        match self.tool.move_op {
            1 => return Some("tb-move-hint"),
            2 => return Some("tb-copy-hint"),
            3 => return Some("tb-rotate-hint"),
            _ => {}
        }
        match self.pat.op {
            1 => return Some("tb-lin-array-hint"),
            2 => return Some("tb-circ-array-hint"),
            _ => {}
        }
        if self.m3.on {
            return Some("tb-measure3d-hint");
        }
        if self.measure.on {
            return Some("tb-measure-hint");
        }
        if self.section.pick || self.section.plane.is_some() {
            return Some("tb-section-hint");
        }
        None
    }

    pub(super) fn help_window(&mut self, ctx: &egui::Context) {
        if !self.win.help {
            return;
        }
        let mut open = true;
        // A HIDDEN SECTION IS NOT SHOWN EVEN IF THE WINDOW STOOD ON IT. An article opened while the
        // machining module was on would outlive the unticking of the box: the contents no longer show it
        // while the text stayed on screen. It is hidden whole — the window is taken to the contents.
        if !crate::help::visible(&self.win.help_article, self.set.cam_tab_enabled) {
            self.win.help_article = "index".to_string();
        }
        let article = self.win.help_article.clone();
        let mut go: Option<String> = None;
        let mut back = false;
        let mut site = false;
        let mut link: Option<String> = None;
        egui::Window::new(format!("{} {}", ph::BOOK_OPEN, crate::i18n::tr("help-title")))
            .open(&mut open)
            .resizable(true)
            .default_width(860.0)
            .default_height(600.0)
            .show(ctx, |ui| {
                // THE CONTENTS ON THE LEFT — AS A PANEL OF ITS OWN of fixed width: the help is not one
                // page, and without a permanent list there is no knowing what is in it at all.
                egui::Panel::left("help_toc").resizable(false).exact_size(240.0).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(ph::MAGNIFYING_GLASS);
                        ui.add(egui::TextEdit::singleline(&mut self.win.help_query).desired_width(f32::INFINITY).hint_text(crate::i18n::tr("help-search")));
                    });
                    ui.separator();
                    egui::ScrollArea::vertical().id_salt("help_toc_scroll").show(ui, |ui| {
                        let q = self.win.help_query.trim().to_string();
                        if q.is_empty() {
                            for (dir, items) in crate::help::sections(self.set.cam_tab_enabled) {
                                if !dir.is_empty() {
                                    ui.add_space(4.0);
                                    ui.label(egui::RichText::new(crate::i18n::tr(&format!("help-section-{dir}"))).strong());
                                }
                                for a in items {
                                    if ui.selectable_label(a == article, crate::help::title(&a)).clicked() {
                                        go = Some(a);
                                    }
                                }
                            }
                        } else {
                            // THE SEARCH REPLACES THE CONTENTS rather than adding to them: two lists at
                            // once are read as one and clicked in the wrong place.
                            let found = crate::help::search(&q, self.set.cam_tab_enabled);
                            if found.is_empty() {
                                ui.label(egui::RichText::new(crate::i18n::tr1("help-nothing-found", "q", &q)).weak().small());
                            }
                            for a in found {
                                if ui.selectable_label(a == article, crate::help::title(&a)).clicked() {
                                    go = Some(a);
                                }
                            }
                        }
                    });
                });
                ui.horizontal(|ui| {
                    if ui.add_enabled(!self.win.help_back.is_empty(), egui::Button::new(format!("{} {}", ph::ARROW_LINE_UP, crate::i18n::tr("help-back")))).clicked() {
                        back = true;
                    }
                    ui.label(egui::RichText::new(crate::help::title(&article)).weak().small());
                    // "OPEN ON THE SITE" GOES ON THE RIGHT, BY THE ARTICLE ITSELF. A link is wanted for a
                    // particular reason: to show a colleague, to leave open on a second monitor, to put
                    // into a task. The button must lead to THAT SAME article and not to the title page of
                    // the site.
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(format!("{} {}", ph::ARROW_SQUARE_OUT, crate::i18n::tr("help-open-on-site"))).on_hover_text(crate::help::web_url(&article)).clicked() {
                            site = true;
                        }
                    });
                });
                ui.separator();
                egui::ScrollArea::vertical().id_salt("help_body").show(ui, |ui| {
                    // MARGINS AROUND THE TEXT. An article nailed to the very edge of the panel is harder
                    // to read: the eye has nothing to catch on when the line comes back. The indent goes
                    // on both sides and not only on the left — the right edge ran into the scrollbar just
                    // the same.
                    egui::Frame::NONE.inner_margin(egui::Margin { left: HELP_PAD, right: HELP_PAD, top: 0, bottom: 0 }).show(ui, |ui| match crate::help::article(&article) {
                        Some(md) => {
                            if let Some(to) = self.draw_markdown(ui, md) {
                                link = Some(to);
                            }
                        }
                        // THERE IS NO ARTICLE — IT IS SAID IN WORDS. An empty window is read as a
                        // breakage of the program rather than as "this article is not written yet".
                        None => {
                            ui.label(egui::RichText::new(crate::i18n::tr1("help-missing", "what", &article)).color(self.scheme.pal.error_mild()));
                        }
                    });
                });
            });
        if site {
            let a = article.clone();
            self.open_help_on_site(&a);
        }
        // A LINK FROM THE TEXT: an internal one leads to an article, an external one to the browser. They
        // are told apart by the scheme rather than by a list: "what starts with https:// leads out of the
        // program" is a rule that cannot be forgotten to be updated.
        if let Some(to) = link {
            if to.starts_with("https://") || to.starts_with("http://") {
                self.launch_browser(&to);
            } else {
                go = Some(to);
            }
        }
        if let Some(a) = go {
            self.goto_help(&a);
        } else if back {
            if let Some(prev) = self.win.help_back.pop() {
                self.win.help_article = prev; // going back does NOT add to the history, or there would be no way out
            }
        }
        self.win.help = open;
    }

    /// Draw the parsed markdown. The colours come from the scheme, the sizes from the base font of the
    /// interface.
    pub(super) fn draw_markdown(&self, ui: &mut egui::Ui, md: &str) -> Option<String> {
        // AN ARTICLE MAY HOLD SEVERAL TABLES, and `egui::Grid` keeps the state of its column widths
        // under its own name: one name for two tables and the second travels by the widths of the
        // first.
        let mut table_n = 0usize;
        let mut clicked: Option<String> = None;
        for b in crate::help::parse(md) {
            match b {
                Block::Heading(level, spans) => {
                    ui.add_space(if level == 1 { 2.0 } else { 8.0 });
                    let size = match level {
                        1 => 22.0,
                        2 => 17.0,
                        3 => 15.0,
                        _ => 14.0,
                    };
                    self.md_line(ui, &spans, size, true, self.scheme.pal.text_strong(), 0.0, &mut clicked);
                    if level <= 2 {
                        ui.separator();
                    }
                }
                Block::Para(spans) => {
                    ui.add_space(3.0);
                    self.md_line(ui, &spans, 14.0, false, self.scheme.pal.text_strong(), 0.0, &mut clicked);
                }
                Block::Bullet(spans) => self.md_line(ui, &spans, 14.0, false, self.scheme.pal.text_strong(), 14.0, &mut clicked),
                Block::Numbered(n, spans) => {
                    let mut s = vec![Span { text: format!("{n}. "), bold: true, italic: false, code: false, link: None }];
                    s.extend(spans);
                    self.md_line(ui, &s, 14.0, false, self.scheme.pal.text_strong(), 14.0, &mut clicked);
                }
                Block::Quote(spans) => self.md_line(ui, &spans, 14.0, false, self.scheme.pal.text_dim(), 14.0, &mut clicked),
                Block::Code(text) => {
                    ui.add_space(3.0);
                    egui::Frame::NONE.fill(self.scheme.pal.panel_bg()).inner_margin(6.0).corner_radius(4.0).show(ui, |ui| {
                        ui.label(egui::RichText::new(text.trim_end()).monospace().color(self.scheme.pal.text_strong()));
                    });
                }
                Block::Rule => {
                    ui.add_space(6.0);
                    ui.separator();
                }
                Block::Image { path, alt } => {
                    ui.add_space(6.0);
                    // AN ANIMATION IS A FOLDER OF FRAMES rather than a GIF: `egui` does not decode those,
                    // and dragging in a decoder for the sake of the help is the same mistake as somebody
                    // else's markdown renderer. The frame is chosen by time and a redraw is requested for
                    // the next one — otherwise the picture freezes on the first.
                    let frames: Vec<String> = if path.ends_with('/') { crate::help::frames(&path) } else { vec![path.clone()] };
                    let shown = if frames.len() > 1 {
                        let t = ui.input(|i| i.time);
                        let idx = ((t / FRAME_SECS) as usize) % frames.len();
                        ui.ctx().request_repaint_after(std::time::Duration::from_secs_f64(FRAME_SECS));
                        frames[idx].clone()
                    } else {
                        frames.first().cloned().unwrap_or_default()
                    };
                    match help_texture(ui.ctx(), &shown) {
                        Some(tex) => {
                            // TO THE WIDTH OF THE WINDOW BUT NO LARGER THAN ITS OWN SIZE: a stretched
                            // screenshot reads as a blurry mush, and a button is exactly what has to be
                            // made out in it.
                            let native = tex.size_vec2();
                            let w = native.x.min(ui.available_width());
                            ui.add(egui::Image::new(&tex).fit_to_exact_size(egui::vec2(w, native.y * w / native.x.max(1.0))).corner_radius(4.0));
                        }
                        // THERE IS NO IMAGE — THE PATH IS SAID. An empty space is read as a breakage of
                        // the program; a guard would not let that through, but an article may also arrive
                        // from outside.
                        None => {
                            ui.label(egui::RichText::new(format!("[{}]", path)).weak().small().color(self.scheme.pal.error_mild()));
                        }
                    }
                    if !alt.is_empty() {
                        // THE CAPTION ALWAYS, not only on a miss: it explains what to look at, and that
                        // is half the use of an illustration.
                        self.md_line(ui, &alt, 12.5, false, self.scheme.pal.text_dim(), 0.0, &mut clicked);
                    }
                    ui.add_space(6.0);
                }
                Block::Table { head, rows } => {
                    ui.add_space(4.0);
                    let cols = head.iter().map(|h| h.len()).chain(rows.iter().map(|r| r.len())).max().unwrap_or(1);
                    // STRIPED: the table of what each kind of mate leaves free has seven rows, and
                    // without alternation the eye slides onto the neighbouring one.
                    egui::Grid::new(("help_table", table_n)).striped(true).num_columns(cols).spacing([16.0, 4.0]).show(ui, |ui| {
                        if let Some(h) = &head {
                            for c in h {
                                self.md_cell(ui, c, true);
                            }
                            for _ in h.len()..cols {
                                ui.label("");
                            }
                            ui.end_row();
                        }
                        for r in &rows {
                            for c in r {
                                self.md_cell(ui, c, false);
                            }
                            // the row is shorter than the rest — it is padded with empties, otherwise
                            // `Grid` shifts the next row into somebody else's columns
                            for _ in r.len()..cols {
                                ui.label("");
                            }
                            ui.end_row();
                        }
                    });
                    table_n += 1;
                    ui.add_space(4.0);
                }
            }
        }
        clicked
    }

    /// A TABLE CELL. Not `md_line`: that one wraps to the width of the window, while inside a `Grid` the
    /// wrapping must be computed by the width of THE COLUMN — otherwise a cell stretches the table across
    /// all the available width and the second column travels off the edge.
    fn md_cell(&self, ui: &mut egui::Ui, spans: &[Span], head: bool) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            for s in spans {
                let mut t = egui::RichText::new(&s.text).size(14.0).color(self.scheme.pal.text_strong());
                if s.code {
                    t = t.monospace().color(self.scheme.pal.active());
                }
                if s.bold || head {
                    t = t.strong();
                }
                if s.italic {
                    t = t.italics();
                }
                ui.label(t);
            }
        });
    }

    /// One line made of spans with emphasis.
    ///
    /// `horizontal_wrapped` rather than separate `label`s: a paragraph must wrap to the width of the
    /// window while the spans inside it run one after another, with no break at every `**bold**`.
    fn md_line(&self, ui: &mut egui::Ui, spans: &[Span], size: f32, heading: bool, col: egui::Color32, indent: f32, clicked: &mut Option<String>) {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            if indent > 0.0 {
                ui.add_space(indent);
                ui.label(egui::RichText::new("• ").size(size).color(self.scheme.pal.text_dim()));
            }
            for s in spans {
                let mut t = egui::RichText::new(&s.text).size(size).color(col);
                if s.code {
                    t = t.monospace().color(self.scheme.pal.active());
                }
                if s.bold || heading {
                    t = t.strong();
                }
                if s.italic {
                    t = t.italics();
                }
                // A LINK GOES IN THE COLOUR OF AN ACTION AND UNDERLINED: nobody clicks text that differs
                // in no way from the text beside it, and then the tie between articles exists only on
                // paper.
                match &s.link {
                    Some(to) => {
                        if ui.add(egui::Link::new(t.color(self.scheme.pal.active()).underline())).clicked() {
                            *clicked = Some(to.clone());
                        }
                    }
                    None => {
                        ui.label(t);
                    }
                }
            }
        });
    }
}
