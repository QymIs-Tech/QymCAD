//! THE SCHEMES OF COLOURS: their section of the settings, duplicated and edited.

use super::*;

/// Editing a colour is NOT editing the document: the picture is repainted while the project stays clean.
/// THE PICTURE IS PAINTED BY THE SCHEME, so a change of colour throws both caches away.
///
/// A SHARED BORROW IS ENOUGH: the caches keep their contents behind `RefCell` and `Cell`, so dropping
/// them needs no exclusive access. It asked for `&mut` only because it used to be a method on `&mut self`,
/// and that would have forced every caller to lend the whole application out exclusively for nothing.
pub(crate) fn repaint_after_scheme_edit(cache: &super::Caches) {
    *cache.view.borrow_mut() = None; // the CPU raster is painted by the scheme, so it is recomputed
    cache.gpu_scene_key.set(u64::MAX); // the GPU: re-upload the vertex buffer
}

/// WHAT THE SCHEME EDITOR NEEDS, and nothing besides: the schemes themselves, the setting that names the
/// chosen one, and the picture caches it has to drop when a colour changes.
pub(crate) struct SchemeCtx<'a> {
    pub scheme: &'a mut super::SchemeUi,
    pub set: &'a mut super::Settings,
    pub cache: &'a super::Caches,
}

/// WHAT THE EDITOR ASKS THE APPLICATION TO DO AFTERWARDS.
///
/// Re-reading the schemes from disk and applying a theme to `egui` are the application's own doings - it
/// owns the directory and the egui context. The editor says WHAT happened; the doing stays outside. Two
/// flags rather than an enum because they are not exclusive: deleting a scheme asks for both at once.
#[derive(Default)]
pub(crate) struct SchemeAsk {
    pub reload: bool,
    pub apply_theme: bool,
}

/// THE COLOUR SCHEME SECTION: choosing one, making a copy of one's own, the colour editor.
///
/// The edits go in LIVE, with no Apply button: choosing a shade blind - close the window, look, come back - is
/// impossible. Writing to the file is a separate button: until it is written, this is a draft in memory and a
/// restart brings back what was there.
///
/// THE BUILT-IN SCHEMES ARE NOT EDITED. The dark one must stay exactly as it is: someone said they liked the
/// look, and "put it back the way it was" has to be backed by something. Editing a built-in one makes a copy by
/// itself, as material editors in grown-up CAD packages do.
pub(crate) fn section(c: &mut SchemeCtx, ask: &mut SchemeAsk, ui: &mut egui::Ui, ctx: &egui::Context) {
    ui.horizontal_wrapped(|ui| {
        // THE SCHEMES COME FROM THE LIST: there will be more than two of one's own, and "dark/light" cannot express that
        let rows: Vec<(String, String, bool)> = c.scheme.all.iter().map(|p| (p.id.clone(), p.title(), p.light)).collect();
        for (id, title, light) in rows {
            let own = !crate::palette::store::is_builtin(&id);
            let mark = if own {
                ph::PENCIL_SIMPLE
            } else if light {
                ph::SUN
            } else {
                ph::MOON
            };
            if ui.selectable_label(c.set.scheme == id, format!("{mark} {title}")).clicked() {
                c.set.scheme = id.clone();
                ask.apply_theme = true;
            }
        }
    });

    let own = !crate::palette::store::is_builtin(&c.set.scheme);
    ui.horizontal(|ui| {
        let label = crate::i18n::tr(if own { "scheme-edit" } else { "scheme-duplicate" });
        if ui.button(format!("{}  {label}", ph::PALETTE)).clicked() {
            if own {
                c.scheme.edit.open = !c.scheme.edit.open;
            } else {
                duplicate(c, ask);
            }
        }
        if own && ui.button(format!("{}  {}", ph::TRASH, crate::i18n::tr("scheme-delete"))).clicked() {
            let id = c.set.scheme.clone();
            let title = c.scheme.pal.title();
            c.scheme.edit.note = match crate::palette::store::delete(&id) {
                Ok(()) => crate::i18n::tr1("scheme-deleted", "name", &title),
                Err(e) => crate::i18n::tr1("scheme-delete-failed", "error", &e),
            };
            c.set.scheme = crate::palette::dark().id;
            c.scheme.edit.open = false;
            ask.reload = true;
            ask.apply_theme = true;
        }
        if own && ui.button(format!("{}  {}", ph::FLOPPY_DISK, crate::i18n::tr("scheme-save"))).clicked() {
            let pal = c.scheme.pal.clone();
            c.scheme.edit.note = match crate::palette::store::save(&pal) {
                Ok(p) => crate::i18n::tr1("scheme-saved", "path", &p.display().to_string()),
                Err(e) => crate::i18n::tr1("scheme-save-failed", "error", &e),
            };
            ask.reload = true;
        }
    });
    if !c.scheme.edit.note.is_empty() {
        ui.label(egui::RichText::new(&c.scheme.edit.note).small().color(c.scheme.pal.hint()));
    }
    if own && c.scheme.edit.open {
        editor(c, ask, ui, ctx);
    }
}

/// Make a scheme of one's own as a copy of the current one and switch to it at once.
pub(crate) fn duplicate(c: &mut SchemeCtx, ask: &mut SchemeAsk) {
    let existing: Vec<String> = c.scheme.all.iter().map(|p| p.id.clone()).collect();
    let mut copy = c.scheme.pal.clone();
    // THE LABEL IS TAKEN BEFORE THE IDENTIFIER CHANGES: a built-in scheme has no name of its own, and `title()`
    // goes to the language catalogue for it - after the id is replaced it would find something else there.
    let was = copy.title();
    copy.id = crate::palette::store::unique_copy_id(&copy.id, &existing);
    // THE LABEL IS IN WORDS, NOT A MACHINE KEY. It used to read like "Light (light-1)": the identifier is of no
    // use to a person at all, and it exists precisely so that it need not be seen.
    copy.name = crate::i18n::tr1("scheme-copy-of", "name", &was);
    let mut n = 2;
    while c.scheme.all.iter().any(|p| p.title() == copy.name) {
        copy.name = crate::i18n::tr2("scheme-copy-of-n", "name", &was, "n", &n.to_string());
        n += 1;
    }
    c.scheme.edit.note = match crate::palette::store::save(&copy) {
        Ok(p) => crate::i18n::tr2("scheme-created", "name", &copy.name, "path", &p.display().to_string()),
        Err(e) => crate::i18n::tr1("scheme-create-failed", "error", &e),
    };
    c.set.scheme = copy.id.clone();
    c.scheme.edit.rename = copy.name.clone();
    c.scheme.edit.open = true;
    ask.reload = true;
    ask.apply_theme = true;
}

/// THE EDITOR ITSELF: the name, light or dark, the shading fractions and every colour by section.
pub(crate) fn editor(c: &mut SchemeCtx, ask: &mut SchemeAsk, ui: &mut egui::Ui, ctx: &egui::Context) {
    ui.separator();
    ui.horizontal(|ui| {
        ui.label(crate::i18n::tr("scheme-name"));
        if c.scheme.edit.rename.is_empty() {
            c.scheme.edit.rename = c.scheme.pal.title();
        }
        ui.add(egui::TextEdit::singleline(&mut c.scheme.edit.rename).desired_width(180.0));
        if ui.button(crate::i18n::tr("scheme-rename")).clicked() {
            // THE LABEL IS RENAMED while the identifier stays: it lives in the settings and in the file name,
            // and changing it along with the label would mean losing the chosen scheme.
            let new = c.scheme.edit.rename.trim().to_string();
            if new.is_empty() {
                c.scheme.edit.note = crate::i18n::tr("scheme-name-taken");
            } else {
                c.scheme.pal.name = new;
                let pal = c.scheme.pal.clone();
                // A RENAME SAYS WHERE THE FILE MOVED TO. The result used to be swallowed silently (`let _ =`),
                // and the "written" message came only from Save - with the former, by then wrong, file name.
                c.scheme.edit.note = match crate::palette::store::save(&pal) {
                    Ok(p) => crate::i18n::tr1("scheme-saved", "path", &p.display().to_string()),
                    Err(e) => crate::i18n::tr1("scheme-save-failed", "error", &e),
                };
                ask.reload = true;
            }
        }
    });
    // light or dark: egui's own appearance depends on it, not only the canvas colours
    let mut light = c.scheme.pal.light;
    if ui.checkbox(&mut light, crate::i18n::tr("scheme-is-light")).changed() {
        c.scheme.pal.light = light;
        crate::gui::sync_visuals(c.scheme, ctx);
        repaint_after_scheme_edit(c.cache);
    }
    // WHETHER TO PAINT THE INTERFACE ITSELF. Off: the panels and buttons take egui's factory look, exactly as
    // it was before the Interface section existed; on: the twelve colours from it are in force.
    let mut ui_on = c.scheme.pal.ui_on;
    if ui.checkbox(&mut ui_on, crate::i18n::tr("scheme-ui-on")).on_hover_text(crate::i18n::tr("scheme-ui-on-hint")).changed() {
        c.scheme.pal.ui_on = ui_on;
        crate::gui::sync_visuals(c.scheme, ctx);
        repaint_after_scheme_edit(c.cache);
    }

    egui::CollapsingHeader::new(crate::i18n::tr("scheme-shading")).show(ui, |ui| {
        let mut changed = false;
        for (key, v) in [
            ("shade-body", &mut c.scheme.pal.shade_floor_body),
            ("shade-mesh", &mut c.scheme.pal.shade_floor_mesh),
            ("shade-viewcube", &mut c.scheme.pal.shade_floor_viewcube),
            ("body-lighten", &mut c.scheme.pal.body_lighten),
            ("body-saturate", &mut c.scheme.pal.body_saturate),
        ] {
            let label = crate::i18n::tr(&format!("scheme-{key}"));
            let hint = crate::i18n::tr(&format!("scheme-{key}-hint"));
            changed |= ui.add(egui::Slider::new(v, 0.0..=1.0).text(label)).on_hover_text(hint).changed();
        }
        if changed {
            repaint_after_scheme_edit(c.cache);
        }
    });

    for (section, rows) in crate::palette::groups() {
        let title = crate::i18n::tr(&format!("scheme-group-{section}"));
        egui::CollapsingHeader::new(title).show(ui, |ui| {
            egui::Grid::new(section).num_columns(2).spacing([8.0, 2.0]).show(ui, |ui| {
                for key in rows {
                    let label = crate::i18n::tr(&format!("scheme-color-{key}"));
                    let mut rgb = c.scheme.pal.entries().iter().find(|(k, _)| *k == key).map(|(_, v)| *v).unwrap_or([0, 0, 0]);
                    if ui.color_edit_button_srgb(&mut rgb).changed() {
                        // THROUGH `set` BY NAME: a typo in a key is rejected rather than writing a colour into
                        // the wrong place - the same device as in a user scheme's file.
                        if c.scheme.pal.set(key, rgb) {
                            crate::gui::sync_visuals(c.scheme, ctx);
                            repaint_after_scheme_edit(c.cache);
                        }
                    }
                    if section == "icons" {
                        ui.label(format!("{label} (--{})", key.replace('_', "-")));
                    } else {
                        ui.label(label);
                    }
                    ui.end_row();
                }
            });
        });
    }
}

/// THE ONE PLACE THE BORROWS ARE SPLIT for the scheme editor, and the one place its requests are met.
///
/// The editor cannot re-read the directory or hand a palette to `egui` - both belong to the
/// application - so it raises flags and they are honoured here, after the panel has finished drawing.
/// Honouring them inside would mean borrowing the application while its own field is still lent out.
pub(crate) fn scheme_section(wc: &mut qymcad_ui_state::WinCtx, ui: &mut egui::Ui, ctx: &egui::Context) {
    let mut ask = SchemeAsk::default();
    {
        let mut c = SchemeCtx { scheme: &mut *wc.scheme, set: &mut *wc.set, cache: &*wc.cache };
        section(&mut c, &mut ask, ui, ctx);
    }
    if ask.reload {
        crate::gui::reload_schemes(&mut *wc.scheme, &mut *wc.status);
    }
    if ask.apply_theme {
        crate::gui::apply_theme(&mut *wc.scheme, &*wc.set, ctx);
    }
}
