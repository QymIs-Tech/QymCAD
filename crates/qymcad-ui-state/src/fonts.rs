//! THE FONTS: found, picked, drawn.

use crate::Notice;
use super::*;

/// The bytes of the default system font (used for text); cached.
pub fn any_font_at_all(font_cache: &mut FontCache) -> bool {
    let mut probe = qymcad_core::model::FontRef::default();
    font_cache.for_tool(&mut probe).is_some()
}

/// THE FONTS THE APPLICATION HAS IN HAND.
///
/// `chosen` is what the text tool writes with - the file the person picked last, or the system font it fell
/// back to. `files` holds the bytes by path, so one file is read once.
///
/// Why this is not simply a pile of bytes any more: a text that is re-baked has to be re-baked IN ITS OWN
/// font, and the only way to find that font again is the name and the path recorded with the text. Measured
/// on a label written in Liberation Sans and edited after a reopen: it came back in Cantarell.
#[derive(Default)]
pub struct FontCache {
    /// The list of installed faces and the state of the window that shows it.
    pub picker: FontPicker,
    pub(crate) files: std::collections::HashMap<String, Vec<u8>>,
}

/// THE WINDOW THAT SHOWS THE INSTALLED FONTS: whether it is open, what is being searched for, and the faces
/// found.
///
/// It lives beside the fonts rather than in the application object, which takes nothing new by rule; and the
/// list belongs here anyway, because walking the font tree costs a noticeable moment and is done once.
#[derive(Default)]
pub struct FontPicker {
    pub open: bool,
    /// Pictures of the names, one per face and size - see `text_texture`.
    pub(crate) textures: std::collections::HashMap<String, egui::TextureHandle>,
    /// The choice is being made FOR THE LABEL BEING EDITED, not for the tool.
    ///
    /// The same list serves both, and what is chosen has to land where it was asked for: a font picked while
    /// editing a label changes THAT label, not what the next click will write.
    pub for_label: bool,
    pub search: String,
    /// Every face found on this machine. Empty until the walk that fills it comes back.
    pub faces: Vec<FontFace>,
    /// The walk, while it is still going.
    pub(crate) coming: Option<std::sync::mpsc::Receiver<Vec<FontFace>>>,
}

impl FontPicker {
    /// START THE WALK OVER THE FONT FOLDERS, in a thread of its own, and return at once.
    ///
    /// Measured on a real machine: the walk takes 536 ms for 232 faces - reading the names out of every file
    /// under half a dozen folders. Done inside a frame that is half a second of a frozen window every time
    /// the list is opened for the first time, and a font tree of several thousand files makes it worse.
    pub fn start_scan(&mut self) {
        if !self.faces.is_empty() || self.coming.is_some() {
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(installed_fonts());
        });
        self.coming = Some(rx);
    }

    /// Take the result if it has arrived. Answers whether the list is ready to be shown.
    pub fn poll_scan(&mut self) -> bool {
        if let Some(rx) = &self.coming {
            match rx.try_recv() {
                Ok(faces) => {
                    self.faces = faces;
                    self.coming = None;
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => self.coming = None, // the thread died; nothing to wait for
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
            }
        }
        !self.faces.is_empty()
    }

    /// Is the walk still going.
    pub fn scanning(&self) -> bool {
        self.coming.is_some()
    }
}

impl FontCache {
    /// The bytes of a font that was recorded somewhere - with a text, or as the current choice. `None` when
    /// nothing is recorded or the file is no longer there.
    pub fn bytes(&mut self, font: &qymcad_core::model::FontRef) -> Option<Vec<u8>> {
        if font.path.is_empty() {
            return None;
        }
        if let Some(b) = self.files.get(&font.path) {
            return Some(b.clone());
        }
        let b = std::fs::read(&font.path).ok()?;
        self.files.insert(font.path.clone(), b.clone());
        Some(b)
    }

    /// Read a font file and say what family it holds, keeping the bytes. `None` if the file cannot be read or
    /// is not a font.
    pub fn read(&mut self, path: &str, index: u32) -> Option<qymcad_core::model::FontRef> {
        let bytes = std::fs::read(path).ok()?;
        let family = qymcad_core::text::family_name(&bytes, index)?;
        self.files.insert(path.to_string(), bytes);
        Some(qymcad_core::model::FontRef { family, path: path.to_string(), index })
    }

    /// THE FONT A TOOL WILL WRITE WITH: what it already carries, or the first font of the system that can be
    /// read - which is then written into the tool, so that what is used is what is shown in the bar.
    pub fn for_tool(&mut self, tool_font: &mut qymcad_core::model::FontRef) -> Option<(qymcad_core::model::FontRef, Vec<u8>)> {
        if !tool_font.path.is_empty() {
            if let Some(b) = self.bytes(&tool_font.clone()) {
                return Some((tool_font.clone(), b));
            }
        }
        for path in system_font_paths() {
            if let Some(f) = self.read(&path, 0) {
                let b = self.bytes(&f)?;
                *tool_font = f.clone();
                return Some((f, b));
            }
        }
        None
    }
}

/// THE FACES THAT MATCH WHAT IS BEING SEARCHED FOR - by family or by style, ignoring case.
///
/// A function of its own so it can be measured without a window: what a person types is the only thing
/// standing between a list of six hundred faces and the one they want.
pub fn fonts_matching(faces: &[FontFace], query: &str) -> Vec<FontFace> {
    let q = query.trim().to_lowercase();
    faces.iter().filter(|f| q.is_empty() || f.family.to_lowercase().contains(&q) || f.style.to_lowercase().contains(&q)).cloned().collect()
}

/// EVERY POPUP A TEXT NEEDS, drawn in one call: the editor of a label and the list of fonts.
///
/// One door on purpose - the application object takes no logic by rule, and two calls plus the answer to
/// "the person asked for a file" would be five lines of it. What the list asks for goes into the ordinary
/// queue of requests, the same way every other button of the interface asks the application for a file.
pub fn text_popups(mut ed: Editing, font_cache: &mut FontCache, tc: &mut TextCtx, ctx: &egui::Context, rect: Rect) -> Vec<BarAsk> {
    let editing = tc.inline.text();
    // the preview of the text tool follows the string as it is typed: re-baked only when the string, the height or
    // the font changed, not every frame
    // A NOTE IS DRAWN IN THE INTERFACE'S FONT, so its letters are asked of that font and not of the tool's: a letter
    // the interface cannot draw is a box on the sheet, whatever the tool's font holds
    if tc.armed.draw_kind() == 11 && tc.writes_note {
        let font = egui::FontId::proportional(14.0);
        let kept: String = tc.text.chars().filter(|c| c.is_whitespace() || ctx.fonts_mut(|f| f.has_glyph(&font, *c))).collect();
        if kept != *tc.text {
            *tc.text = kept;
            *ed.status = Notice::refusal(qymcad_i18n::tr("sk-note-no-letters"));
        }
    } else if tc.armed.draw_kind() == 11 {
        let stale = tc.tool.text_ghost.as_ref().is_none_or(|(t, h, f, _)| t != &*tc.text || *h != tc.tool_text_height || f != &*tc.font);
        if stale {
            // a copy: finding the face a font stands for fills the name in, and the bar would then name a font the
            // person never chose - "Font..." must stay until one is
            let mut asked = tc.font.clone();
            let font = font_cache.for_tool(&mut asked).map(|(f, _)| f).unwrap_or_default();
            if let Some(bytes) = font_cache.bytes(&font) {
                // A LETTER THE FONT DOES NOT HAVE IS NOT TAKEN: it would be a box in the field and nothing on the
                // sheet. It is taken out, and the status line names the font that lacks it.
                let kept: String = tc.text.chars().filter(|c| c.is_whitespace() || qymcad_core::text::can_write(&bytes, font.index, &c.to_string())).collect();
                tc.tool.text_refused = kept != *tc.text || (tc.tool.text_refused && tc.text.trim().is_empty());
                if kept != *tc.text {
                    *tc.text = kept;
                    *ed.status = Notice::refusal(qymcad_i18n::tr1("sk-text-no-letters", "name", &font_label(&font, "opt-font")));
                }
                // what the font does write, the field shows in it: the tool's font stands behind the interface's
                // own, so a script the interface has no letters for is drawn rather than boxed (egui adds a font of
                // a name once)
                let mut data = egui::FontData::from_owned(bytes);
                data.index = font.index;
                ctx.add_font(egui::epaint::text::FontInsert::new(
                    &format!("tool-font:{}#{}", font.path, font.index),
                    data,
                    vec![egui::epaint::text::InsertFontFamily { family: egui::FontFamily::Proportional, priority: egui::epaint::text::FontPriority::Lowest }],
                ));
            }
            let glyphs = bake_text_glyphs(font_cache, &font, GlyphPlace { at: Point2::new(0.0, 0.0), height: tc.tool_text_height, angle: 0.0 }, tc.text);
            tc.tool.text_ghost = Some((tc.text.clone(), tc.tool_text_height, tc.font.clone(), glyphs));
        }
    }
    text_obj_editor(ed.reborrow(), font_cache, tc, ctx, rect);
    let mut want_file = false;
    if let Some(f) = font_picker_window(font_cache, ctx, &mut want_file) {
        *ed.status = Notice::info(qymcad_i18n::tr1("pk-font-is", "name", &f.family));
        if font_cache.picker.for_label {
            // THE LABEL BEING EDITED CHANGES ITS FONT, at once and visibly: the glyphs are re-baked in the
            // face just chosen, with the string and the height as they stand in the popup.
            font_cache.picker.for_label = false;
            if let (Sel::Sketch(si), Some(ti)) = (*ed.sel, editing) {
                let at = ed.project.sketches[si].texts.get(ti).map(|t| (t.x, t.y, t.angle));
                if let Some((x, y, angle)) = at {
                    let (txt, h) = (tc.annot.text_buf.clone(), tc.annot.text_h);
                    let glyphs = bake_text_glyphs(font_cache, &f, GlyphPlace { at: Point2::new(x, y), height: h, angle }, &txt);
                    ed.project.set_sketch_text(si, ti, qymcad_core::model::TextSpec { at: Point2::new(x, y), height: h, angle, text: txt, glyphs, font: f });
                    invalidate(ed.regen);
                }
            }
        } else {
            *tc.font = f;
        }
    }
    if want_file {
        vec![BarAsk::PickFontFile]
    } else {
        Vec::new()
    }
}

/// ONE ROW OF THE LIST: the family and the style, written in that very face.
///
/// DRAWN AS OUTLINES, NOT AS A FILLED SHAPE. A glyph is not convex - `o` and `e` have holes, `s` bends back
/// on itself - and asking a painter for a convex polygon per loop gives torn letters with their holes filled
/// in. Measured on the list: every second face came out ragged. The sketch itself draws its text as outlines
/// too, so the row shows exactly what the canvas will.
///
/// A face that cannot write its own name is shown in the interface font and marked instead: an icon font
/// holds thousands of glyphs and not one letter, and chosen by mistake it puts nothing into the sketch at
/// all. Such a row does not answer to a click - `Sense::hover` rather than `click`.
pub fn font_row(ui: &mut egui::Ui, cache: &mut FontCache, f: &FontFace, row: f32) -> egui::Response {
    let label = font_row_text(f);
    let font = qymcad_core::model::FontRef { family: f.family.clone(), path: f.path.clone(), index: f.index };
    let bytes = cache.bytes(&font);
    let writes = bytes.as_deref().is_some_and(|b| qymcad_core::text::can_write(b, f.index, &label));
    let sense = if writes { egui::Sense::click() } else { egui::Sense::hover() };
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), row), sense);
    // THE FILE, ON HOVER. The row shows the name; where the name is shared by two editions the file tells
    // them apart, and a long file name runs off the edge of the row. The whole path is one hover away.
    let resp = resp.on_hover_text(&f.path);
    if resp.hovered() && writes {
        ui.painter().rect_filled(rect, 2.0, ui.visuals().widgets.hovered.bg_fill);
    }
    match (&bytes, writes) {
        (Some(b), true) => {
            // THE NAME IS RASTERISED IN ITS OWN FACE, not drawn as outlines.
            //
            // Reported behaviour, with a screenshot: "the fonts are still drawn wrong". Outlines were the
            // first answer and they are right on the canvas of a sketch, where a line is a line; in a list
            // row they are not. A row is 26 px tall, a window is one pixel to the point, and a letter drawn
            // as a one-pixel outline of its inner and outer edge at that size comes out as a smear. What a
            // person needs here is to see the face, which is what a rasteriser gives.
            let px = (row - 10.0).max(8.0);
            let key = format!("{}#{}@{:.0}", f.path, f.index, px);
            let ink = ui.visuals().strong_text_color();
            match text_texture(ui.ctx(), &mut cache.picker.textures, &key, b, px, &label) {
                Some(tex) => {
                    let size = tex.size_vec2();
                    let at = egui::Rect::from_min_size(rect.left_top() + egui::vec2(8.0, (row - size.y) / 2.0), size);
                    let mut mesh = egui::Mesh::with_texture(tex.id());
                    mesh.add_rect_with_uv(at, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), ink);
                    ui.painter().add(egui::Shape::mesh(mesh));
                }
                None => {
                    ui.painter().text(rect.left_center() + egui::vec2(8.0, 0.0), egui::Align2::LEFT_CENTER, &label, egui::TextStyle::Body.resolve(ui.style()), ink);
                }
            }
        }
        _ => {
            ui.painter().text(
                rect.left_center() + egui::vec2(8.0, 0.0),
                egui::Align2::LEFT_CENTER,
                format!("{} {} - {}", crate::tone_icon(crate::Tone::Warning).unwrap_or_default(), label, qymcad_i18n::tr("font-no-letters")),
                egui::TextStyle::Body.resolve(ui.style()),
                crate::tone_colour_in(ui, crate::Tone::Warning),
            );
        }
    }
    resp
}

/// THE WINDOW OF INSTALLED FONTS: a search field, the faces found, each drawn in its own outlines, and the
/// button that still opens a file of one's own.
///
/// Returns the face a person chose, or `None` while they have not. Sets `want_file` when they ask for the
/// file dialog instead - a Snap sees nothing under `~/.local/share/fonts`, so the button is the only way to
/// that font there, and the portal hands the file over with no permission at all.
///
/// The preview is drawn with OUR OWN outlines, the same ones the text tool will cut into the part: a font
/// shown in the interface font would say nothing about what comes out in the drawing. Only the visible rows
/// are drawn, because parsing a face costs milliseconds and a system holds hundreds of them.
pub fn font_picker_window(cache: &mut FontCache, ctx: &egui::Context, want_file: &mut bool) -> Option<qymcad_core::model::FontRef> {
    if !cache.picker.open {
        return None;
    }
    cache.picker.start_scan();
    let ready = cache.picker.poll_scan();
    let shown = fonts_matching(&cache.picker.faces, &cache.picker.search);

    let mut chosen: Option<qymcad_core::model::FontRef> = None;
    let mut open = true;
    egui::Window::new(qymcad_i18n::tr("font-window")).id(egui::Id::new("win_font")).open(&mut open).default_pos(egui::pos2(40.0, 40.0)).default_width(460.0).resizable(true).show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut cache.picker.search).desired_width(200.0).hint_text(qymcad_i18n::tr("font-search")));
            if ui.button(qymcad_i18n::tr("font-from-file")).on_hover_text(qymcad_i18n::tr("opt-pick-font")).clicked() {
                *want_file = true;
            }
        });
        ui.label(if ready { qymcad_i18n::tr1("font-found", "n", &shown.len().to_string()) } else { qymcad_i18n::tr("font-searching") });
        ui.separator();
        let row = 26.0;
        egui::ScrollArea::vertical().max_height(420.0).show_rows(ui, row, shown.len(), |ui, range| {
            for i in range {
                let f = shown[i].clone();
                if font_row(ui, cache, &f, row).clicked() {
                    chosen = Some(qymcad_core::model::FontRef { family: f.family.clone(), path: f.path.clone(), index: f.index });
                }
            }
        });
    });
    if !open || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        cache.picker.open = false;
    }
    if chosen.is_some() {
        cache.picker.open = false;
    }
    chosen
}

/// CAN THIS FONT WRITE THIS STRING - asked of the font a label is about to be written in.
///
/// A font that cannot be read at all answers no: there is nothing to write with either way.
pub fn font_can_write(cache: &mut FontCache, font: &qymcad_core::model::FontRef, text: &str) -> bool {
    cache.bytes(font).is_some_and(|b| qymcad_core::text::can_write(&b, font.index, text))
}

/// A STRING BAKED INTO A TEXTURE, in the font it is given, and kept in a cache.
///
/// TWO CALLERS WANTED THIS, which is why it lives here: the caption of a face of the view cube (stretched
/// over the face, so it is distorted exactly as the face is), and a row of the list of fonts (drawn in the
/// very face it names). Rasterising a font every frame is thousands of glyphs a second for nothing, so the
/// picture is made once per string, size and font.
///
/// ONLY THE ALPHA IS BAKED: the colour comes from the vertices it is drawn with, so one texture serves a
/// dark caption on a light ground and the other way round alike.
pub fn text_texture(ctx: &egui::Context, cache: &mut std::collections::HashMap<String, egui::TextureHandle>, key: &str, font: &[u8], px: f32, text: &str) -> Option<egui::TextureHandle> {
    if let Some(t) = cache.get(key) {
        return Some(t.clone());
    }
    use ab_glyph::{Font, ScaleFont};
    let parsed = ab_glyph::FontRef::try_from_slice(font).ok()?;
    let scaled = parsed.as_scaled(px);
    // THE WIDTH COMES FROM THE GLYPHS THEMSELVES rather than from the number of letters: strings differ in
    // length, and a texture of fixed width would stretch one and squeeze another.
    let glyphs: Vec<_> = text.chars().map(|c| parsed.glyph_id(c)).collect();
    let advance: f32 = glyphs.iter().map(|g| scaled.h_advance(*g)).sum();
    let pad = px * 0.25;
    let w = (advance + pad * 2.0).ceil().max(1.0) as usize;
    let h = (scaled.height() + pad).ceil().max(1.0) as usize;
    let mut alpha = vec![0u8; w * h];
    let mut pen = pad;
    let baseline = pad * 0.5 + scaled.ascent();
    for g in &glyphs {
        let q = g.with_scale_and_position(px, ab_glyph::point(pen, baseline));
        if let Some(outline) = parsed.outline_glyph(q) {
            let bb = outline.px_bounds();
            outline.draw(|gx, gy, c| {
                let (x, y) = (bb.min.x as i32 + gx as i32, bb.min.y as i32 + gy as i32);
                if x >= 0 && y >= 0 && (x as usize) < w && (y as usize) < h {
                    let i = y as usize * w + x as usize;
                    // THE MAXIMUM is taken rather than the sum: neighbouring glyphs overlap, and adding
                    // gives dirty dark patches at the joins.
                    alpha[i] = alpha[i].max((c * 255.0) as u8);
                }
            });
        }
        pen += scaled.h_advance(*g);
    }
    let pixels: Vec<egui::Color32> = alpha.iter().map(|a| egui::Color32::from_white_alpha(*a)).collect();
    let img = egui::ColorImage { size: [w, h], source_size: egui::Vec2::new(w as f32, h as f32), pixels };
    let tex = ctx.load_texture(key, img, egui::TextureOptions::LINEAR);
    cache.insert(key.to_string(), tex.clone());
    Some(tex)
}

/// HOW A FONT IS NAMED IN THE INTERFACE: its family, or the given word when nothing is recorded.
///
/// One function for the two places that name it - the button of the top bar, which says what the text tool
/// will write with, and the editor of a label, which says what THAT label is written in. Spelled out twice
/// they drift, and both are the answer to the same question a person asks before pressing anything: in which
/// font will this be.
pub fn font_label(font: &qymcad_core::model::FontRef, when_unknown: &str) -> String {
    if font.family.trim().is_empty() {
        qymcad_i18n::tr(when_unknown)
    } else {
        font.family.clone()
    }
}

/// TAKE THE FILE A PERSON CHOSE and say what to put in the status line.
///
/// The name comes out of the FILE, not out of the file name: the label keeps the family it was written in,
/// and that is what has to be shown in the bar and looked for later on another machine.
pub fn choose_font(cache: &mut FontCache, tool_font: &mut qymcad_core::model::FontRef, path: &str) -> crate::Notice {
    match cache.read(path, 0) {
        Some(f) => {
            let said = crate::Notice::info(qymcad_i18n::tr1("pk-font-is", "name", &f.family));
            *tool_font = f;
            said
        }
        None => crate::Notice::error(qymcad_i18n::tr1("pk-font-error", "error", path)),
    }
}

/// A FACE A PERSON CAN WRITE WITH: one entry of the list of installed fonts.
///
/// A file is not a face: a `.ttc` collection holds several, and each of them is its own family and style, so
/// the index travels with the path everywhere - into the list, into the record kept with a label, and into
/// the parsing of the outlines.
#[derive(Clone, Debug, PartialEq)]
pub struct FontFace {
    pub family: String,
    pub style: String,
    pub path: String,
    pub index: u32,
    /// WHAT TELLS THIS FACE FROM ITS NAMESAKE, empty when there is none.
    ///
    /// Two different files calling themselves the same thing are ordinary: measured on a real machine,
    /// `CaskaydiaCove Nerd Font Regular` lies there twice, once as a `.ttf` and once as an `.otf` "Complete"
    /// edition - different formats, different sizes, possibly different coverage. Hiding one of them would be
    /// a lie, and two identical lines tell a person nothing about which they are taking.
    pub note: String,
}

/// The line shown for a face: its name, and what tells it from its namesake when there is one.
pub fn font_row_text(f: &FontFace) -> String {
    if f.note.is_empty() {
        format!("{} {}", f.family, f.style)
    } else {
        format!("{} {} - {}", f.family, f.style, f.note)
    }
}

/// WHERE FONTS LIE, per system and per sandbox.
///
/// The sandboxes matter as much as the systems here. Inside Flatpak the host's fonts are mounted read-only
/// under `/run/host` while `/usr/share/fonts` holds only the runtime's own few, and inside Snap the host's
/// tree appears under `/var/lib/snapd/hostfs`. Neither needs a permission of any kind; what they need is to
/// be looked at, or a person is shown three fonts and none of their own.
///
/// What is NOT here, and cannot be: a Snap sees nothing under `~/.local/share/fonts`, because its `home`
/// permission excludes hidden directories on purpose. That is why the "choose a file" button stays beside
/// the list - the portal hands over whatever file a person picks, permission or not.
pub fn font_directories() -> Vec<std::path::PathBuf> {
    let home = std::env::var("HOME").ok();
    let mut dirs: Vec<String> = Vec::new();
    if cfg!(target_os = "windows") {
        dirs.push("C:/Windows/Fonts".into());
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            dirs.push(format!("{local}/Microsoft/Windows/Fonts"));
        }
    } else if cfg!(target_os = "macos") {
        dirs.push("/System/Library/Fonts".into());
        dirs.push("/Library/Fonts".into());
        if let Some(h) = &home {
            dirs.push(format!("{h}/Library/Fonts"));
        }
    } else {
        dirs.push("/usr/share/fonts".into());
        dirs.push("/usr/local/share/fonts".into());
        if let Some(h) = &home {
            dirs.push(format!("{h}/.local/share/fonts"));
            dirs.push(format!("{h}/.fonts"));
        }
        // the host, as the sandboxes show it
        dirs.push("/run/host/fonts".into());
        dirs.push("/run/host/local-fonts".into());
        dirs.push("/run/host/user-fonts".into());
        dirs.push("/var/lib/snapd/hostfs/usr/share/fonts".into());
    }
    dirs.into_iter().map(std::path::PathBuf::from).collect()
}

/// EVERY FACE FOUND UNDER `dirs`, sorted by family and style.
///
/// The walk reads the names out of each file, because a file name says nothing reliable: `DejaVuSans.ttf`
/// and `n019003l.pfb` are the same kind of thing to a directory listing. Files that are not fonts are
/// skipped in silence - a font directory holds licences, caches and READMEs, and none of that is an error.
pub fn installed_fonts_in(dirs: &[std::path::PathBuf]) -> Vec<FontFace> {
    let mut out: Vec<FontFace> = Vec::new();
    let mut stack: Vec<std::path::PathBuf> = dirs.to_vec();
    let mut seen_dirs: std::collections::HashSet<std::path::PathBuf> = std::collections::HashSet::new();
    let mut same: std::collections::HashSet<(String, String, u32, usize)> = std::collections::HashSet::new();
    while let Some(dir) = stack.pop() {
        // A font tree is full of symlinks; the same directory reached twice would list the same faces twice.
        let key = std::fs::canonicalize(&dir).unwrap_or(dir.clone());
        if !seen_dirs.insert(key) {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for e in entries.flatten() {
            let path = e.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("").to_ascii_lowercase();
            if !matches!(ext.as_str(), "ttf" | "otf" | "ttc" | "otc") {
                continue;
            }
            let Ok(bytes) = std::fs::read(&path) else { continue };
            for index in 0..qymcad_core::text::faces_in(&bytes) {
                if let Some((family, style)) = qymcad_core::text::face_name(&bytes, index) {
                    // THE SAME FONT IN TWO FOLDERS IS ONE ROW. Measured on a real machine: 254 faces held
                    // thirteen pairs saying exactly the same thing - one `.ttc` lying in three font folders
                    // at once, and a family copied into a folder inside its own. Real copies, not symlinks,
                    // so canonicalising the directories does not catch them; the names plus the size of the
                    // file do. A person choosing a font must not be shown three identical lines with no way
                    // to tell them apart, because there is nothing to tell.
                    if same.insert((family.clone(), style.clone(), index, bytes.len())) {
                        out.push(FontFace { family, style, path: path.to_string_lossy().into_owned(), index, note: String::new() });
                    }
                }
            }
        }
    }
    out.sort_by_key(|a| (a.family.to_lowercase(), a.style.to_lowercase()));
    // NAMESAKES ARE GIVEN THEIR FILE NAME. Only namesakes: putting the file beside every row would drown the
    // name a person is actually looking for.
    let mut namesakes: std::collections::HashMap<(String, String), usize> = std::collections::HashMap::new();
    for f in &out {
        *namesakes.entry((f.family.clone(), f.style.clone())).or_default() += 1;
    }
    for f in &mut out {
        if namesakes.get(&(f.family.clone(), f.style.clone())).copied().unwrap_or(0) > 1 {
            f.note = std::path::Path::new(&f.path).file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        }
    }
    out
}

/// Every face installed on this machine, sandboxes included.
pub fn installed_fonts() -> Vec<FontFace> {
    installed_fonts_in(&font_directories())
}

/// WHERE A FONT MIGHT BE FOUND when the person has chosen none - one file per system, not a catalogue. The
/// list of installed fonts is a separate matter; this is only the fallback for writing something at all.
pub(crate) fn system_font_paths() -> Vec<String> {
    let mut paths: Vec<String> = vec![
        "/usr/share/fonts/TTF/DejaVuSans.ttf".into(),
        "/usr/share/fonts/TTF/OpenSans-Regular.ttf".into(),
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf".into(),
        "/usr/share/fonts/liberation/LiberationSans-Regular.ttf".into(),
        "/System/Library/Fonts/Supplemental/Arial.ttf".into(),
        "C:/Windows/Fonts/arial.ttf".into(),
    ];
    if let Ok(o) = std::process::Command::new("fc-match").args(["-f", "%{file}", "sans"]).output() {
        if let Ok(s) = String::from_utf8(o.stdout) {
            let s = s.trim().to_string();
            if !s.is_empty() {
                paths.insert(0, s);
            }
        }
    }
    paths
}
