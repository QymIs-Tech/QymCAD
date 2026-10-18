//! THE TEXT OF A SKETCH AND OF A NOTE: edited and baked into glyphs.

use crate::Notice;
use super::*;

pub fn note_editor(annot: &mut AnnotEdit, inline: &mut InlineEdit, project: &mut Project, sel: Sel, view: View2d, ctx: &egui::Context, rect: Rect) {
    let Sel::Sketch(si) = sel else {
        inline.clear();
        return;
    };
    let Some(ni) = inline.note() else { return };
    let Some(at) = project.sketches.get(si).and_then(|s| s.notes.get(ni)).map(|n| (Sheet { view, rect }).at(Point2::new(n.x, n.y))) else {
        inline.clear();
        return;
    };
    let (mut apply, mut close) = (false, false);
    egui::Area::new(egui::Id::new(("noteedit", si, ni))).fixed_pos(clamp_popup(at, rect) + egui::vec2(0.0, -28.0)).order(crate::AT_GEOMETRY).show(ctx, |ui| {
        crate::at_geometry_ui(ui);
        egui::Frame::popup(ui.style()).show(ui, |ui| {
            ui.horizontal(|ui| {
                let r = ui.add(egui::TextEdit::singleline(&mut annot.note_buf).desired_width(160.0));
                if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    apply = true;
                }
                if ui.button(egui_phosphor::regular::CHECK).clicked() {
                    apply = true;
                }
            });
        });
    });
    if apply {
        project.set_note_text(si, ni, annot.note_buf.clone());
        close = true;
    }
    if close || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        inline.clear(); // a note is not a label: no tool is in hand for it
    }
}

/// CLOSE THE EDITING OF A LABEL and put the text tool down.
///
/// Reported behaviour: "after the tick or Enter the tool stays in hand with the parameters of the edit, and
/// clicking the sketch spams more of them". Taking the tool up when the editor opens is right - the bar is
/// where the string, the height and the font are named - and keeping it after the edit is applied is not:
/// the work is finished and nothing is waiting to be placed.
pub fn end_text_edit(tc: &mut TextCtx) {
    // the string of the label edited is not offered to the next label (found checking issue #32)
    tc.text.clear();
    tc.inline.clear();
    tc.annot.text = None;
    tc.annot.text_focus = false;
    *tc.armed = Armed::None;
    tc.tool.pts.clear();
}

/// In-place editing of a note's text (a double click).
/// Bake the glyph polylines of a text through the active font (world coordinates, baseline point x, y).
/// WHERE A TEXT'S GLYPHS ARE LAID: the point the line of text starts at, the height of its letters, and the angle it is
/// turned by, in degrees.
#[derive(Clone, Copy, Debug)]
pub struct GlyphPlace {
    pub at: Point2,
    pub height: f64,
    pub angle: f64,
}

/// The glyphs of `text` in `font`, laid at `place`. TURNED BY ITS ANGLE: baked level, a turned text edited went back to
/// standing level (found with issue #32).
pub fn bake_text_glyphs(font_cache: &mut FontCache, font: &qymcad_core::model::FontRef, place: GlyphPlace, text: &str) -> Vec<Vec<Point2>> {
    let Some(bytes) = font_cache.bytes(font) else { return Vec::new() };
    let GlyphPlace { at, height, angle } = place;
    let (sn, cs) = angle.to_radians().sin_cos();
    qymcad_core::text::text_outline_contours(&bytes, font.index, text, height, at.x, at.y)
        .into_iter()
        .map(|c| c.points.into_iter().map(|p| Point2::new(at.x + (p.x - at.x) * cs - (p.y - at.y) * sn, at.y + (p.x - at.x) * sn + (p.y - at.y) * cs)).collect())
        .collect()
}

/// The popup for editing a text object: the string plus the height. On apply the glyphs are re-baked and updated.
/// WHAT THE POPUPS OF A TEXT WORK ON: the buffers being typed into, the tool in hand, and the font the tool
/// writes with.
///
/// A record rather than five arguments: the editor of a label has to PUT THE TOOL DOWN when the edit is
/// applied, and that is two more things to reach - past the limit a signature is allowed.
pub struct TextCtx<'a> {
    pub annot: &'a mut AnnotEdit,
    pub inline: &'a mut InlineEdit,
    pub armed: &'a mut Armed,
    pub tool: &'a mut SketchTool,
    /// The font the TEXT TOOL writes with (not the one a label is written in).
    pub font: &'a mut qymcad_core::model::FontRef,
    /// the string and the height the text tool writes, for its preview; a letter its font cannot write is taken out
    pub text: &'a mut String,
    pub tool_text_height: f64,
    /// the text tool writes a note - not geometry, drawn in the interface's own font
    pub writes_note: bool,
}

/// OPEN THE EDITING OF A TEXT OBJECT: what a double click on it does.
///
/// One door for the two callers - the double click in the sketch and the hand of a test. Spelled out in both
/// places it drifts: the buffers are what the popup edits, and a caller that forgets one of them opens the
/// popup over the previous text.
pub fn begin_text_edit(project: &Project, tools: &mut Tools, prefs: &mut SketchToolPrefs, si: usize, ti: usize) {
    let Some(t) = project.sketches.get(si).and_then(|s| s.texts.get(ti)) else { return };
    tools.annot.text_buf = t.text.clone();
    tools.annot.text_h = t.height;
    tools.annot.text = Some(ti);
    tools.annot.text_focus = true; // the caret lands in the field, so typing and Enter work straight away
    *tools.inline = InlineEdit::Text(ti);
    // THE TEXT TOOL IS TAKEN UP, and its options in the top bar are set to what is being edited.
    //
    // Reported behaviour, with a screenshot: "while editing, the tool stays Select". Editing a label IS
    // working with text, and the bar is where its string, height and font are named: a person editing a
    // label looked at the options of the arrow. What follows from taking the tool is the ordinary rule of
    // every tool of the sketch - the next click on the canvas places another label.
    prefs.text = t.text.clone();
    prefs.text_h = t.height;
    prefs.font = t.font.clone();
    // Not `select`, which TOGGLES: pressing the text tool twice puts it down, and opening an editor must
    // always end with the tool in hand.
    *tools.armed = Armed::Draw(11);
    tools.tool.pts.clear();
}

pub fn text_obj_editor(ed: Editing, font_cache: &mut FontCache, tc: &mut TextCtx, ctx: &egui::Context, rect: Rect) {
    // NO LABEL BEING EDITED, NOTHING TO END. This runs on every flat frame, and ending a text edit puts the tool in hand
    // down: asked first whether the sketch was selected, it put down whatever was in hand whenever it was not.
    // Reported behaviour: an extrusion opened for editing, "Pick contours" pressed - the view went flat with nothing to
    // pick and no bar, the extrusion gone from the hand (its node, not its sketch, was selected).
    let Some(ti) = tc.inline.text() else { return };
    let Sel::Sketch(si) = *ed.sel else {
        end_text_edit(tc);
        return;
    };
    let Some((bx, _by, _, maxy)) = ed.project.sketch_text_bbox(si, ti) else {
        end_text_edit(tc);
        return;
    };
    let at = (Sheet { view: *ed.view, rect }).at(Point2::new(bx, maxy));
    let (mut apply, mut close) = (false, false);
    egui::Area::new(egui::Id::new(("textedit", si, ti))).fixed_pos(clamp_popup(at, rect) + egui::vec2(0.0, -34.0)).order(crate::AT_GEOMETRY).show(ctx, |ui| {
        crate::at_geometry_ui(ui);
        egui::Frame::popup(ui.style()).show(ui, |ui| {
            ui.horizontal(|ui| {
                let want_focus = tc.annot.text_focus;
                let r = focus_edit(ui, &mut tc.annot.text_buf, 160.0, "", want_focus);
                if r.has_focus() {
                    tc.annot.text_focus = false;
                }
                ui.label(qymcad_i18n::tr("g-height-short"));
                ui.add(egui::DragValue::new(&mut tc.annot.text_h).speed(0.2).range(1.0..=1000.0).suffix(qymcad_i18n::tr("unit-mm-suffix")));
                // WHICH FONT THIS LABEL IS IN. It is re-baked in that one, so it is the one to show; a label
                // made before fonts were recorded says so instead, because its edit will be refused.
                let of_label = ed.project.sketches[si].texts.get(ti).map(|t| t.font.clone()).unwrap_or_default();
                if ui.button(font_label(&of_label, "sk-text-font-unknown-short")).on_hover_text(qymcad_i18n::tr("opt-pick-font")).clicked() {
                    font_cache.picker.open = true;
                    font_cache.picker.for_label = true; // what is chosen changes THIS label
                }
                if (r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))) || ui.button(egui_phosphor::regular::CHECK).clicked() {
                    apply = true;
                }
            });
        });
    });
    if apply {
        // THE LABEL IS RE-BAKED IN ITS OWN FONT, not in whatever the application has in hand. A text that was
        // written in one typeface and edited in another is a change nobody asked for and nobody is told
        // about: measured, a label written in Liberation Sans came back in Cantarell after a reopen.
        let (x, y, angle, font) = {
            let t = &ed.project.sketches[si].texts[ti];
            (t.x, t.y, t.angle, t.font.clone())
        };
        let (txt, h) = (tc.annot.text_buf.clone(), tc.annot.text_h);
        // A label made before the font was recorded, or one whose font file is gone: the drawing stays as it
        // is and the person is told why, rather than being handed a different typeface silently.
        match font_cache.bytes(&font) {
            Some(_) => {
                let glyphs = bake_text_glyphs(font_cache, &font, GlyphPlace { at: Point2::new(x, y), height: h, angle }, &txt);
                ed.project.set_sketch_text(si, ti, qymcad_core::model::TextSpec { at: qymcad_core::geom::Point2::new(x, y), height: h, angle, text: txt, glyphs, font });
                invalidate(ed.regen);
            }
            None => {
                *ed.status = Notice::warning(if font.family.is_empty() { qymcad_i18n::tr("sk-text-font-unknown") } else { qymcad_i18n::tr1("sk-text-font-gone", "name", &font.family) });
            }
        }
        close = true;
    }
    if close || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        end_text_edit(tc);
    }
}
