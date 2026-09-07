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

    /// Open the help at a particular article — in our own window or in a browser, as chosen.
    ///
    /// THE FORK IS HERE AND NOT AT EVERY CALLER. The help is opened by F1, by the menu, by the "?"
    /// buttons of the commands; have each of them ask about the setting and one is bound to forget, and
    /// it will forget silently.
    pub(crate) fn open_help(&mut self, article: &str) {
        open(&self.set, &mut self.win.help, &self.scheme.pal, &mut self.status, article);
    }

    /// F1: THE ARTICLE ABOUT WHAT IS BEING DONE RIGHT NOW.
    ///
    /// The order of the answer runs from the particular to the general: the active Part command, then the
    /// active Sketch tool, then the section of the workbench, and only then the contents. A help that
    /// always opens at the title page is not help but an extra click at the very minute somebody is
    /// stuck.
    pub(crate) fn help_for_context(&self) -> &'static str {
        if self.tools.armed.commanding() {
            if let Some(a) = crate::help_map::part_article(self.tools.armed.cmd_kind()) {
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
        if self.side.carr.mode != 0 {
            if let Some(a) = crate::help_map::assembly_article("asm.comp-array") {
                return a;
            }
        }
        if self.sketch_ses.editing.is_some() {
            if self.tools.armed.draw_kind() > 0 {
                if let Some(a) = crate::help_map::sketch_article("sk", self.tools.armed.draw_kind()) {
                    return a;
                }
            }
            if self.tools.armed.dim_kind() > 0 {
                if let Some(a) = crate::help_map::sketch_article("dim", self.tools.armed.dim_kind()) {
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
        crate::help_map::workbench_article(crate::gui::workbench_code(&self.workbench))
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
        if self.params.boolean.pick.is_some() {
            return Some("tb-bool-bodies-hint");
        }
        match self.tools.armed.move_op() {
            1 => return Some("tb-move-hint"),
            2 => return Some("tb-copy-hint"),
            3 => return Some("tb-rotate-hint"),
            _ => {}
        }
        match self.tools.armed.pat_op() {
            1 => return Some("tb-lin-array-hint"),
            2 => return Some("tb-circ-array-hint"),
            _ => {}
        }
        if self.side.m3.on {
            return Some("tb-measure3d-hint");
        }
        if self.tools.armed.measuring() {
            return Some("tb-measure-hint");
        }
        if self.side.section.pick || self.side.section.plane.is_some() {
            return Some("tb-section-hint");
        }
        None
    }





    /// THE ONE PLACE THE BORROWS ARE SPLIT for the help window.
    ///
    /// The application owns the pieces; here they are handed out separately - the window's own state, the
    /// palette to read, the status line to write. Rust allows disjoint borrows of fields inside one
    /// function, and that is the whole trick: the split happens once, and everything below it says what it
    /// needs instead of taking the application entire.
    pub(super) fn help_window(&mut self, ui_ctx: &egui::Context) {
        let mut ctx = HelpCtx { win: &mut self.win.help, pal: &self.scheme.pal, say: &mut self.status };
        window(&mut ctx, ui_ctx);
    }
}


/// WHAT DRAWING THE HELP NEEDS, and nothing besides.
///
/// This is the first panel taken off `App`, and it is here to show the shape. It used to be four methods
/// on the application, which meant they could see the document, the camera, the selection and everything
/// else - and, being methods, could never move to a crate of their own: a method belongs to the crate that
/// declares its type. As free functions over this context they say what they touch and can travel.
///
/// MEASURED, not guessed: the drawing reads four fields of its own state, five colours of the palette and
/// writes one line of status. That is the whole of it.
pub(crate) struct HelpCtx<'a> {
pub win: &'a mut qymcad_ui_state::HelpWin,
pub pal: &'a crate::palette::Palette,
/// The status line - the one place the program speaks in passing.
pub say: &'a mut String,
}

/// Go to an article, remembering where we came from.
/// OPEN THE HELP AT AN ARTICLE - in our own window or in a browser, as chosen.
///
/// THE FORK IS HERE AND NOT AT EVERY CALLER. The help is opened by F1, by the menu, by the "?" buttons of
/// the commands and by the start screen; have each of them ask about the setting and one is bound to forget,
/// and it will forget silently.
pub(crate) fn open(set: &super::Settings, win: &mut qymcad_ui_state::HelpWin, pal: &crate::palette::Palette, say: &mut String, article: &str) {
    if let HelpTarget::Site(url) = help_target(set, article) {
        let mut ctx = HelpCtx { win, pal, say };
        browse(&mut ctx, &url);
        return;
    }
    goto(win, article);
    win.open = true;
}

pub(crate) fn goto(win: &mut qymcad_ui_state::HelpWin, article: &str) {
if win.article != article {
    let cur = std::mem::replace(&mut win.article, article.to_string());
    if !cur.is_empty() {
        win.back.push(cur);
    }
}
}

/// tested rather than the launch.
pub(crate) fn browse(ctx: &mut HelpCtx, url: &str) {
    // NO BROWSER IS LAUNCHED IN A TEST RUN, and that is no concession to the test. The very first
    // edition opened a real browser window on a developer machine — the test reached outside the run;
    // on a machine with no graphics it would instead hang on `xdg-open`. The decision of where to go
    // is checked in full through `help_target`, and only the launch is left here, with nothing in it
    // to check.
    if cfg!(test) {
        *ctx.say = crate::i18n::tr1("help-opened-in-browser", "url", url);
        return;
    }
    let (cmd, args) = super::browse_command(egui::os::OperatingSystem::from_target_os(), url);
    match std::process::Command::new(cmd).args(&args).spawn() {
        Ok(_) => *ctx.say = crate::i18n::tr1("help-opened-in-browser", "url", url),
        // IT DID NOT WORK — THE ADDRESS IS SHOWN. There may be no browser at all (a bare server, a
        // stripped-down environment), and then the only useful thing is the link itself, to be
        // carried off by hand.
        Err(e) => *ctx.say = format!("{} {}", crate::i18n::tr1("help-browser-failed", "url", url), e),
    }
}

///
/// The address is derived from the path of the article (`help::web_url`) rather than kept as a list:
/// the window and the site have ONE source — the same `.md` files — so there is nothing to
/// diverge.
pub(crate) fn on_site(ctx: &mut HelpCtx, article: &str) {
    let url = crate::help::web_url(article);
    browse(ctx, &url);
}

pub(crate) fn window(ctx: &mut HelpCtx, ui_ctx: &egui::Context) {
    if !ctx.win.open {
        return;
    }
    let mut open = true;
    // A HIDDEN SECTION IS NOT SHOWN EVEN IF THE WINDOW STOOD ON IT. An article opened while the
    // machining module was on would outlive the unticking of the box: the contents no longer show it
    // while the text stayed on screen. It is hidden whole — the window is taken to the contents.
    if !crate::help::visible(&ctx.win.article) {
        ctx.win.article = "index".to_string();
    }
    let article = ctx.win.article.clone();
    let mut go: Option<String> = None;
    let mut back = false;
    let mut site = false;
    let mut link: Option<String> = None;
    egui::Window::new(format!("{} {}", ph::BOOK_OPEN, crate::i18n::tr("help-title")))
        .open(&mut open)
        .resizable(true)
        .default_width(860.0)
        .default_height(600.0)
        .show(ui_ctx, |ui| {
            // THE CONTENTS ON THE LEFT — AS A PANEL OF ITS OWN of fixed width: the help is not one
            // page, and without a permanent list there is no knowing what is in it at all.
            egui::Panel::left("help_toc").resizable(false).exact_size(240.0).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(ph::MAGNIFYING_GLASS);
                    ui.add(egui::TextEdit::singleline(&mut ctx.win.query).desired_width(f32::INFINITY).hint_text(crate::i18n::tr("help-search")));
                });
                ui.separator();
                egui::ScrollArea::vertical().id_salt("help_toc_scroll").show(ui, |ui| {
                    let q = ctx.win.query.trim().to_string();
                    if q.is_empty() {
                        for (dir, items) in crate::help::sections() {
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
                        let found = crate::help::search(&q);
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
                if ui.add_enabled(!ctx.win.back.is_empty(), egui::Button::new(format!("{} {}", ph::ARROW_LINE_UP, crate::i18n::tr("help-back")))).clicked() {
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
                        if let Some(to) = markdown(ctx.pal, ui, md) {
                            link = Some(to);
                        }
                    }
                    // THERE IS NO ARTICLE — IT IS SAID IN WORDS. An empty window is read as a
                    // breakage of the program rather than as "this article is not written yet".
                    None => {
                        ui.label(egui::RichText::new(crate::i18n::tr1("help-missing", "what", &article)).color(ctx.pal.error_mild()));
                    }
                });
            });
        });
    if site {
        let a = article.clone();
        on_site(ctx, &a);
    }
    // A LINK FROM THE TEXT: an internal one leads to an article, an external one to the browser. They
    // are told apart by the scheme rather than by a list: "what starts with https:// leads out of the
    // program" is a rule that cannot be forgotten to be updated.
    if let Some(to) = link {
        if to.starts_with("https://") || to.starts_with("http://") {
            browse(ctx, &to);
        } else {
            go = Some(to);
        }
    }
    if let Some(a) = go {
        goto(ctx.win, &a);
    } else if back {
        if let Some(prev) = ctx.win.back.pop() {
            ctx.win.article = prev; // going back does NOT add to the history, or there would be no way out
        }
    }
    ctx.win.open = open;
}

/// interface.
pub(crate) fn markdown(pal: &crate::palette::Palette, ui: &mut egui::Ui, md: &str) -> Option<String> {
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
                line(pal, ui, &spans, LineStyle { size: size, heading: true, col: pal.text_strong(), indent: 0.0 }, &mut clicked);
                if level <= 2 {
                    ui.separator();
                }
            }
            Block::Para(spans) => {
                ui.add_space(3.0);
                line(pal, ui, &spans, LineStyle { size: 14.0, heading: false, col: pal.text_strong(), indent: 0.0 }, &mut clicked);
            }
            Block::Bullet(spans) => line(pal, ui, &spans, LineStyle { size: 14.0, heading: false, col: pal.text_strong(), indent: 14.0 }, &mut clicked),
            Block::Numbered(n, spans) => {
                let mut s = vec![Span { text: format!("{n}. "), bold: true, italic: false, code: false, link: None }];
                s.extend(spans);
                line(pal, ui, &s, LineStyle { size: 14.0, heading: false, col: pal.text_strong(), indent: 14.0 }, &mut clicked);
            }
            Block::Quote(spans) => line(pal, ui, &spans, LineStyle { size: 14.0, heading: false, col: pal.text_dim(), indent: 14.0 }, &mut clicked),
            Block::Code(text) => {
                ui.add_space(3.0);
                egui::Frame::NONE.fill(pal.panel_bg()).inner_margin(6.0).corner_radius(4.0).show(ui, |ui| {
                    ui.label(egui::RichText::new(text.trim_end()).monospace().color(pal.text_strong()));
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
                        ui.label(egui::RichText::new(format!("[{}]", path)).weak().small().color(pal.error_mild()));
                    }
                }
                if !alt.is_empty() {
                    // THE CAPTION ALWAYS, not only on a miss: it explains what to look at, and that
                    // is half the use of an illustration.
                    line(pal, ui, &alt, LineStyle { size: 12.5, heading: false, col: pal.text_dim(), indent: 0.0 }, &mut clicked);
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
                            cell(pal, ui, c, true);
                        }
                        for _ in h.len()..cols {
                            ui.label("");
                        }
                        ui.end_row();
                    }
                    for r in &rows {
                        for c in r {
                            cell(pal, ui, c, false);
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

/// wrapping must be computed by the width of THE COLUMN — otherwise a cell stretches the table across
/// all the available width and the second column travels off the edge.
fn cell(pal: &crate::palette::Palette, ui: &mut egui::Ui, spans: &[Span], head: bool) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        for s in spans {
            let mut t = egui::RichText::new(&s.text).size(14.0).color(pal.text_strong());
            if s.code {
                t = t.monospace().color(pal.active());
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

///
/// `horizontal_wrapped` rather than separate `label`s: a paragraph must wrap to the width of the
/// window while the spans inside it run one after another, with no break at every `**bold**`.
/// HOW ONE LINE OF HELP IS SET: its size, whether it is a heading, its colour, and how far it is
/// indented. Four things about looks that travelled beside the words they describe.
#[derive(Clone, Copy)]
struct LineStyle {
    size: f32,
    heading: bool,
    col: egui::Color32,
    indent: f32,
}

fn line(pal: &crate::palette::Palette, ui: &mut egui::Ui, spans: &[Span], st: LineStyle, clicked: &mut Option<String>) {
    let LineStyle { size, heading, col, indent } = st;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        if indent > 0.0 {
            ui.add_space(indent);
            ui.label(egui::RichText::new("• ").size(size).color(pal.text_dim()));
        }
        for s in spans {
            let mut t = egui::RichText::new(&s.text).size(size).color(col);
            if s.code {
                t = t.monospace().color(pal.active());
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
                    if ui.add(egui::Link::new(t.color(pal.active()).underline())).clicked() {
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

/// Where the help for this article will go under the current settings.
pub(crate) fn help_target(set: &super::Settings, article: &str) -> HelpTarget {
    if set.help_external {
        HelpTarget::Site(crate::help::web_url(article))
    } else {
        HelpTarget::Window
    }
}
