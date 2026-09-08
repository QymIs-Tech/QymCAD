//! THE WINDOWS AND DIALOGUES: the parts library, the properties of a component and of a datum, the
//! parameters, the machine, the settings, the tools, the modal confirmations.

use super::*;

/// THE STEP THE INTERFACE SCALE SNAPS TO. Five hundredths: a scale of 1.37 is of no use to anybody, and
/// with a drag speed of 0.01 per point every fifth point of the drag lands on a round number, which is what
/// makes the drag aimable at all.
const UI_SCALE_STEP: f32 = 0.05;

impl App {












}

/// WHICH SECTIONS ARE SHOWN AT ALL: CAM only when its module is enabled, and while searching, only those where
/// something was found. ONE function for the window and for the tests alike: separate them, and a test starts
/// checking something other than what a person sees.
pub(crate) fn settings_sections_visible(scheme: &super::SchemeUi) -> Vec<super::settings_sections::SettingsSection> {
    use super::settings_sections::SettingsSection as Sec;
    let q = scheme.search.clone();
    let searching = !q.trim().is_empty();
    Sec::all().iter().copied().filter(|s| !searching || s.has_match(&q, &|k: &str| crate::i18n::tr(k))).collect()
}

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



/// "THE LAST RUN ENDED IN AN ERROR" - shown once, when a report from an earlier run is found.
///
/// Without this the reports pile up in a directory nobody has heard of. The window says where the
/// file is and hands the path over, because the next thing asked of the person is to attach it.
pub(crate) fn crash_notice(crash_report: &mut Option<std::path::PathBuf>, ctx: &egui::Context) {
    let Some(path) = crash_report.clone() else { return };
    let mut open = true;
    let mut dismiss = false;
    egui::Window::new(format!("{} {}", ph::WARNING, crate::i18n::tr("crash-title")))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(460.0)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.add_space(4.0);
            ui.label(crate::i18n::tr("crash-what"));
            ui.add_space(8.0);
            ui.label(egui::RichText::new(crate::crash::without_home(&path.to_string_lossy())).monospace().small());
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui.button(format!("{} {}", ph::COPY, crate::i18n::tr("crash-copy-path"))).clicked() {
                    ui.output_mut(|o| o.commands.push(egui::OutputCommand::CopyText(path.to_string_lossy().into_owned())));
                }
                if ui.button(crate::i18n::tr("close")).clicked() {
                    dismiss = true;
                }
            });
        });
    if dismiss || !open {
        // Renamed rather than deleted: the person may still want to attach it to a report.
        crate::crash::mark_seen(&path);
        *crash_report = None;
    }
}

/// The About window (Help -> About).
pub(crate) fn about_dialog(win: &mut super::Windows, scheme: &super::SchemeUi, ctx: &egui::Context) {
    if !win.is(WinKind::About) {
        return;
    }
    let mut open = true;
    egui::Window::new(format!("{} {}", ph::INFO, crate::i18n::tr("win-about")))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(420.0)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.add_space(4.0);
            ui.label(egui::RichText::new("QymCAD").size(22.0).strong());
            ui.label(egui::RichText::new(crate::i18n::tr("about-tagline")).italics());
            // WHICH BUILD IS THIS - the first question asked of any complaint. With a build a day the
            // version number alone names a whole week of binaries, so the commit stands beside it, and
            // the button hands the whole line over ready to paste: nobody retypes a hash by eye.
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label(crate::i18n::tr("about-build"));
                ui.label(egui::RichText::new(crate::build_info::line()).monospace());
                if ui
                    .small_button(ph::COPY)
                    .on_hover_text(crate::i18n::tr("about-copy-hint"))
                    .clicked()
                {
                    ui.output_mut(|o| o.commands.push(egui::OutputCommand::CopyText(crate::diagnostics::block())));
                }
            });
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(8.0);
            ui.label(crate::i18n::tr("about-what"));
            ui.add_space(10.0);
            ui.colored_label(
                scheme.pal.warning(),
                format!("{} {}", ph::WARNING, crate::i18n::tr("about-develop-warning")),
            );
            ui.add_space(6.0);
            // THE CAM TAB IS IN THE SETTINGS AND DOES NOT WORK. Whoever finds the checkbox is owed the
            // reason here rather than after an hour of trying: it is groundwork, not a finished tool.
            ui.label(egui::RichText::new(crate::i18n::tr("about-cam")).weak().small());
            ui.add_space(10.0);
            ui.separator();
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label(crate::i18n::tr("about-site"));
                ui.hyperlink_to("cad.qymis.tech", "https://cad.qymis.tech");
            });
            ui.horizontal(|ui| {
                ui.label(crate::i18n::tr("about-author"));
                ui.label(egui::RichText::new(crate::i18n::tr("about-author-name")).strong());
            });
            // THE PERSON MUST SEE ON WHAT TERMS THEY GOT THE PROGRAM. A copyleft licence is worth nothing
            // to whoever does not know they hold it: the rights to study, change and pass the program on
            // come with it, and the only place a desktop program can say so is here.
            ui.horizontal(|ui| {
                ui.label(crate::i18n::tr("about-license"));
                ui.hyperlink_to("AGPL-3.0-or-later", "https://www.gnu.org/licenses/agpl-3.0.html");
            });
            ui.label(egui::RichText::new(crate::i18n::tr("about-no-warranty")).weak().small());
            ui.add_space(8.0);
            ui.vertical_centered(|ui| {
                if ui.button(crate::i18n::tr("close")).clicked() {
                    win.close(WinKind::About);
                }
            });
        });
    if !open {
        win.close(WinKind::About);
    }
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
            let mark = if own { ph::PENCIL_SIMPLE } else if light { ph::SUN } else { ph::MOON };
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
                            repaint_after_scheme_edit(c.cache);
                        }
                    }
                    ui.label(label);
                    ui.end_row();
                }
            });
        });
    }
}

/// The parts library window: a category tree on the left, a grid of products on the right, insertion into the project.
pub(crate) fn parts_library_window(wc: &mut qymcad_ui_state::WinCtx, ctx: &egui::Context) {
    if !wc.win.is(WinKind::PartsLibrary) {
        return;
    }
    // The tree is taken out of self for a moment, so that it can be read while the selection and the search are mutated.
    let tree = wc.parts.tree.take().unwrap_or_else(crate::parts_library::load_library_tree);
    let mut open = wc.win.is(WinKind::PartsLibrary);
    let mut to_insert: Option<crate::parts_library::PartSource> = None;
    let mut rescan = false;
    egui::Window::new(format!("{} {}", ph::PACKAGE, crate::i18n::tr("win-parts-library")))
        .open(&mut open)
        .default_width(640.0).default_height(440.0)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui
                    .button(format!("{} {}", ph::ARROWS_CLOCKWISE, crate::i18n::tr("win-refresh")))
                    .on_hover_text(crate::i18n::tr("pl-rescan"))
                    .clicked()
                {
                    rescan = true;
                }
                ui.separator();
                ui.label(ph::MAGNIFYING_GLASS.to_string());
                ui.add(
                    egui::TextEdit::singleline(&mut wc.parts.search)
                        .hint_text(crate::i18n::tr("pl-search"))
                        .desired_width(200.0),
                );
                if !wc.parts.search.is_empty() && ui.small_button(ph::X).clicked() {
                    wc.parts.search.clear();
                }
            });
            ui.separator();
            let query = wc.parts.search.trim().to_lowercase();
            egui::Panel::left("parts_lib_tree")
                .resizable(true)
                .default_size(230.0)
                .show(ui, |ui| {
                    egui::ScrollArea::vertical().id_salt("parts_lib_tree_scroll").show(ui, |ui| {
                        let mut path = Vec::new();
                        crate::gui::parts_tree_node(ui, &tree.embedded, true, &mut path, &mut wc.parts.sel);
                        crate::gui::parts_tree_node(ui, &tree.user, false, &mut path, &mut wc.parts.sel);
                    });
                });
            egui::CentralPanel::default().show(ui, |ui| {
                // The set on the right: while searching, every match in the catalogue; otherwise the direct products of the chosen category.
                let mut entries: Vec<&crate::parts_library::PartEntry> = Vec::new();
                if query.is_empty() {
                    match &wc.parts.sel {
                        Some((true, p)) => {
                            if let Some(n) = crate::gui::cat_at(&tree.embedded, p) {
                                entries.extend(n.parts.iter());
                            }
                        }
                        Some((false, p)) => {
                            if let Some(n) = crate::gui::cat_at(&tree.user, p) {
                                entries.extend(n.parts.iter());
                            }
                        }
                        None => {
                            ui.weak(crate::i18n::tr("pl-pick-category"));
                        }
                    }
                } else {
                    crate::gui::collect_matching(&tree.embedded, &query, &mut entries);
                    crate::gui::collect_matching(&tree.user, &query, &mut entries);
                    ui.weak(crate::i18n::tr1("pl-found-n", "n", &entries.len().to_string()));
                    ui.add_space(2.0);
                }
                if (query.is_empty() && wc.parts.sel.is_some() || !query.is_empty()) && entries.is_empty() {
                    ui.weak(crate::i18n::tr("pl-empty"));
                }
                // the thumbnails (lazily loaded and cached) are prepared IN ADVANCE, so that no &mut self is held inside the drawing loop
                let thumbs: Vec<Option<egui::TextureHandle>> = entries.iter().map(|e| crate::gui::parts_thumb_texture(&mut *wc.parts, ui.ctx(), &e.source)).collect();
                egui::ScrollArea::vertical().id_salt("parts_lib_grid").show(ui, |ui| {
                    for (e, thumb) in entries.iter().zip(thumbs.iter()) {
                        ui.group(|ui| {
                            ui.horizontal(|ui| {
                                // a preview thumbnail of the body (or a placeholder icon when the product has no thumb.png)
                                match thumb {
                                    Some(t) => {
                                        ui.add(egui::Image::from_texture(egui::load::SizedTexture::new(t.id(), egui::vec2(52.0, 52.0))).corner_radius(3.0));
                                    }
                                    None => {
                                        ui.add_sized([52.0, 52.0], egui::Label::new(egui::RichText::new(ph::CUBE).size(24.0).weak()));
                                    }
                                }
                                ui.vertical(|ui| {
                                    ui.label(egui::RichText::new(&e.name).strong());
                                    if let Some(m) = &e.manifest {
                                        if !m.description.is_empty() {
                                            ui.label(egui::RichText::new(&m.description).weak().small());
                                        }
                                        if !m.tags.is_empty() {
                                            ui.label(egui::RichText::new(m.tags.join(" · ")).weak().small());
                                        }
                                    }
                                });
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if ui.button(format!("{} {}", ph::ARROW_SQUARE_IN, crate::i18n::tr("win-insert"))).clicked() {
                                        to_insert = Some(e.source.clone());
                                    }
                                });
                            });
                        });
                    }
                });
            });
        });
    // The tree is put back (or rebuilt when Refresh was pressed).
    if rescan {
        wc.parts.tree = Some(crate::parts_library::load_library_tree());
        // the thumbnails are re-read (they may have changed on disk) and the old textures are retired (never dropped mid-frame)
        wc.tex_graveyard.extend(wc.parts.thumbs.drain().filter_map(|(_, v)| v));
        wc.parts.sel = None;
    } else {
        wc.parts.tree = Some(tree);
    }
    if let Some(src) = to_insert {
        wc.ask.push(qymcad_ui_state::WinAsk::InsertPart(src));
    }
    wc.win.set(WinKind::PartsLibrary, open);
}

/// THE ROWS OF THE PARAMETER TABLE. They were lifted out of the window into a METHOD OF THEIR OWN, because a
/// field's width has to be checked in a REAL frame rather than against a number some function computed. The
/// first attempt at making the fields elastic was checked by a test on the function and by a test that the
/// call appeared in the source - both green, while the fields in the window stayed narrow. Now the test draws
/// THIS method into a ui of a known width and measures how much the frame grew.
///
/// Returns (whether anything was edited, what to delete).
pub(crate) fn params_rows_ui(wc: &mut qymcad_ui_state::WinCtx, ui: &mut egui::Ui) -> super::ParamRowsOut {
    let mut out = super::ParamRowsOut::default();
    let (dirty, remove) = (&mut out.dirty, &mut out.remove);
    // THE FIELD WIDTHS ARE ELASTIC. They used to be a hard 90 and 120 points: the window stretched and the
    // fields did not, so a long variable name COULD NOT BE TYPED - the text crawled under the edge and one had
    // to type blind. The width comes from the ui being drawn into and is handed to EVERY field.
    let (w_name, w_expr) = super::param_field_widths(ui.available_width());
    let mut name_w = 0.0_f32;
    // THE LIST SCROLLS. Asked for: with some 150 variables and drivers, is there vertical scrolling? There
    // was not: the table simply grew, and at a hundred and fifty parameters the window ran off the edge of the
    // screen along with the Add button. The height is now capped and the contents scroll; when there is less
    // room than the cap, whatever there is gets used.
    let max_h = ui.available_height().clamp(120.0, super::PARAM_ROWS_MAX_H);
    // WHAT WAS ASKED FOR. The edits are gathered and applied AFTER the drawing: during it the document is lent
    // to the driver list (which reads the whole project), and changing the model mid-frame is the very thing
    // being moved away from.
    enum Act {
        Rename(usize, String),
        SetExpr(usize, String),
        /// A driving dimension's value (or a feature's parameter) is edited straight from the table.
        SetDriver(usize, f64),
        DropDriver(usize),
        /// Go to whatever was named: the dimension's sketch, or the feature.
        GoTo(usize),
    }
    let mut acts: Vec<Act> = Vec::new();
    // WHERE THE PATHS WERE DRAWN, for the tests: they move the mouse over the REAL coordinates.
    let mut paths: Vec<(usize, egui::Rect)> = Vec::new();
    // WHO HOLDS A NAME IS SAID IN WORDS, UNDER THE TABLE. What was asked for: either forbid identical names,
    // or make it clear which sketch, body or assembly each of them comes from.
    let mut refusal: Option<String> = None;
    let names: Vec<String> = wc.project.parameters.iter().map(|p| p.name.clone()).collect();
    let exprs: Vec<String> = wc.project.parameters.iter().map(|p| p.expr.clone()).collect();
    // A SEARCH ABOVE THE TABLE. At a hundred and fifty names the list cannot be read by eye. It searches both
    // the name and the path: a person remembers either what they called it or where it sits.
    ui.horizontal(|ui| {
        ui.label(ph::MAGNIFYING_GLASS);
        ui.add(egui::TextEdit::singleline(&mut *wc.par_search).desired_width(160.0).hint_text(crate::i18n::tr("par-search")));
    });
    let q = wc.par_search.trim().to_lowercase();
    let hit = |name: &str, path: &str| q.is_empty() || name.to_lowercase().contains(&q) || path.to_lowercase().contains(&q);
    let scrolled = egui::ScrollArea::vertical().max_height(max_h).auto_shrink([false, true]).show(ui, |ui| {
    egui::Grid::new("params_grid").num_columns(4).spacing([8.0, 4.0]).striped(true).show(ui, |ui| {
        ui.label(egui::RichText::new(crate::i18n::tr("pp-name")).strong());
        ui.label(egui::RichText::new(crate::i18n::tr("par-expression")).strong());
        ui.label(egui::RichText::new(crate::i18n::tr("par-value")).strong());
        ui.label("");
        ui.end_row();
        for i in 0..names.len() {
            if !hit(&names[i], "") {
                continue;
            }
            // THE NAME. It is edited in A BUFFER and goes into the document on Enter - it used to be written
            // into the model on every letter, and every formula referring to it broke on the very first one.
            let own = names[i].clone();
            let taken = |nm: &str| {
                !nm.eq_ignore_ascii_case(&own) && wc.project.name_owner(nm).is_some()
            };
            let ok = |nm: &str| qymcad_core::drivers::check_ident(nm).is_ok() && !taken(nm);
            let id = egui::Id::new(("par_name", i));
            let r = super::expr_field::name_field(ui, &*wc.project, id, &names[i], w_name, "w", &ok);
            name_w = name_w.max(r.resp.rect.width());
            if r.committed && r.text.trim() != names[i] {
                acts.push(Act::Rename(i, r.text.trim().to_string()));
            }
            // THE EXPLANATION STAYS FOR AS LONG AS THE NAME IS BAD rather than flashing for one frame on
            // refusal. Measured: the first edition showed the line only in the frame where Enter was pressed -
            // that is, never. It is computed from the CURRENT text.
            let nm = r.text.trim().to_string();
            if !nm.is_empty() && !ok(&nm) {
                refusal = Some(match wc.project.name_owner(&nm) {
                    Some(o) if !nm.eq_ignore_ascii_case(&own) => {
                        // A GLOBAL PARAMETER HAS NO PATH, and giving it a "where this dimension sits" would be
                        // a lie: it is not a dimension and it sits nowhere.
                        let where_ = if o.path.is_empty() { crate::i18n::tr("par-owner-project") } else { o.path.clone() };
                        crate::i18n::tr2("par-name-taken", "name", &nm, "where", &where_)
                    }
                    _ => crate::i18n::tr1("par-name-bad", "name", &nm),
                });
            }

            let id = egui::Id::new(("par_expr", i));
            let r = super::expr_field::expr_field(ui, &*wc.project, id, &exprs[i], w_expr, &crate::i18n::tr("par-example"));
            if r.committed && r.text != exprs[i] {
                acts.push(Act::SetExpr(i, r.text.clone()));
            }

            // THE VALUE IS COMPUTED FROM WHAT IS IN THE FIELD RIGHT NOW rather than from what was recorded: the
            // answer is visible while typing, and it costs the document nothing.
            match wc.project.eval_expr(&r.text) {
                Ok(v) => {
                    ui.label(format!("{v:.3}"));
                }
                Err(e) => {
                    // THE REASON IN WORDS RATHER THAN AN ICON (see the history in expr_errors.rs).
                    //
                    // IN THE CELL ONLY A MARK: the value column is the narrowest of the four, and the reason
                    // does not fit it. Written into the cell as a plain label it ran off the edge of the
                    // window and was cut mid-word ("...a number or a na"); wrapped, it broke every second
                    // word onto its own line. The words themselves go below the table, across its whole
                    // width, where they can be read at a glance - see `params_window`.
                    out.errors.push((nm.clone(), crate::gui::error_words::expr_error_text(&e)));
                    ui.label(egui::RichText::new("!").color(wc.scheme.pal.error_mild()).small())
                        .on_hover_text(crate::gui::error_words::expr_error_text(&e));
                }
            }
            if ui.button(ph::TRASH).clicked() {
                *remove = Some(i);
            }
            ui.end_row();
        }
        // THE DRIVERS LIVE IN THE SAME TABLE, AND THEIR VALUES ARE EDITABLE THERE.
        //
        // They used to hang in a separate look-but-do-not-touch list: a name, a path, a number. This table is
        // the one place where the project's WHOLE set of numbers is both visible and editable.
        let drv: Vec<(usize, String, String, Option<f64>, bool)> = wc
            .project
            .named_dims
            .iter()
            .enumerate()
            .map(|(k, n)| {
                let dup = wc.project.named_dims.iter().filter(|m| m.name.eq_ignore_ascii_case(&n.name)).count() > 1;
                (k, n.name.clone(), wc.project.driver_path(&n.target), wc.project.named_dim_value(n), dup)
            })
            .filter(|(_, nm, path, _, _)| hit(nm, path))
            .collect();
        for (k, nm, path, val, dup) in drv {
            // THE NAME AND THE PATH. The path answers "which of the identically named ones"; for namesakes it
            // also carries the warning colour - a bare name in a formula is ambiguous for them.
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(&nm).strong());
                // A PATH IS A ROAD, NOT A CAPTION. A click leads to that very sketch or feature: the question
                // "which sketch is this dimension from" gets answered by arriving there, rather than by hunting
                // through the tree by hand.
                let mut t = egui::RichText::new(&path).small();
                t = if dup { t.color(ui.visuals().warn_fg_color) } else { t.weak() };
                let r = ui
                    .add(egui::Label::new(t).sense(egui::Sense::click()))
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .on_hover_text(crate::i18n::tr(if dup { "par-driver-ambiguous" } else { "par-driver-goto" }));
                paths.push((k, r.rect));
                if r.clicked() {
                    acts.push(Act::GoTo(k));
                }
            });
            match val {
                Some(v) => {
                    let id = egui::Id::new(("drv_val", k));
                    let shown = qymcad_core::expr::fmt_num(v);
                    let r = super::expr_field::expr_field(ui, &*wc.project, id, &shown, w_expr, &crate::i18n::tr("par-example"));
                    if r.committed && r.text.trim() != shown {
                        if let Ok(nv) = wc.project.eval_expr(&r.text) {
                            acts.push(Act::SetDriver(k, nv));
                        }
                    }
                    ui.label(format!("{v:.3}"));
                }
                None => {
                    // THE DIMENSION IS GONE (its sketch or its constraint was deleted) - that is said out loud
                    // rather than shown as a blank: the driver is still in the formulas and has no value.
                    ui.label(egui::RichText::new(crate::i18n::tr1("par-dim-missing", "name", &nm)).weak());
                    ui.label("");
                }
            }
            if ui.button(ph::TRASH).on_hover_text(crate::i18n::tr("par-drop-driver")).clicked() {
                acts.push(Act::DropDriver(k));
            }
            ui.end_row();
        }

        // A FEATURE'S UNNAMED NUMBERS DO NOT GET IN HERE.
        //
        // They were all shown at first, following the convention where a parameter table also holds the
        // model's parameters. What was asked for: remove from the global parameters the empty entries where no
        // driver name was given. The parameter list is what a person HAS NAMED; a hundred foreign rows with
        // empty names turn it into a dump. There is a place to name a feature's number: the feature's
        // properties in the right panel.
    });
    });
    wc.tree.drv_path_rects = paths;
    if let Some(msg) = &refusal {
        ui.label(egui::RichText::new(msg).color(wc.scheme.pal.warning()).small());
    }
    for a in acts {
        match a {
            Act::Rename(i, nm) => {
                let old = wc.project.parameters[i].name.clone();
                // ONE OPERATION, ONE UNDO STEP, AND THE REFERENCES FOLLOW THE NAME.
                let mut ed = qymcad_ui_state::edit_over(wc.rebuild(), crate::i18n::tr("par-rename-step"));
                let done = ed.project().rename_driver(&old, &nm);
                drop(ed);
                if done.is_err() {
                    *wc.status = crate::i18n::tr1("par-name-bad", "name", &nm);
                }
            }
            Act::SetExpr(i, e) => {
                let mut ed = qymcad_ui_state::edit_over(wc.rebuild(), crate::i18n::tr("par-edit-step"));
                ed.project().parameters[i].expr = e;
                drop(ed);
                *dirty = true;
            }
            Act::SetDriver(k, v) => {
                let Some(target) = wc.project.named_dims.get(k).map(|n| n.target.clone()) else { continue };
                let mut ed = qymcad_ui_state::edit_over(wc.rebuild(), crate::i18n::tr("par-edit-step"));
                let done = ed.project().set_dim_target_value(&target, v);
                // THE SKETCH IS RESOLVED EXACTLY ONCE - here, on commit - AND THE BODIES STANDING ON IT ARE
                // MARKED FOR REBUILD.
                //
                // Without that second step came exactly the trouble that was reported: a driver was changed
                // from 90 to 300, the green circle grew, and the part behind it never rebuilt. The general
                // parameter pass (`apply_param_edit`) touches only the sketches that contain EXPRESSIONS - and
                // a driving dimension is an ordinary NUMBER, so its turn never came. A silent answer: 300 in
                // the table, 90 on the screen.
                if let qymcad_core::model::DimTarget::Sketch { sketch, .. } = &target {
                    if let Some(si) = ed.project().sketch_index(*sketch) {
                        ed.project().solve_sketch(si);
                    }
                    ed.project().mark_sketch_dirty(*sketch);
                }
                drop(ed);
                if done {
                    *dirty = true;
                }
            }
            Act::GoTo(k) => {
                // NAVIGATING IS NOT EDITING THE DOCUMENT: there must be no undo step here.
                let Some(target) = wc.project.named_dims.get(k).map(|n| n.target.clone()) else { continue };
                // THE ORDER MATTERS AND IT IS ONE REQUEST, NOT TWO. Stepping into a component clears the
                // selection, so a selection made here and a step deferred to the end of the frame would
                // undo each other: the jump landed nowhere.
                match target {
                    qymcad_core::model::DimTarget::Sketch { sketch, .. } => {
                        let owner = wc.project.sketch_owner(sketch);
                        if let Some(si) = wc.project.sketch_index(sketch) {
                            wc.ask.push(qymcad_ui_state::WinAsk::GoTo { owner, sel: Sel::Sketch(si) });
                        }
                    }
                    qymcad_core::model::DimTarget::Feature { node, .. } => {
                        let owner = wc.project.timeline.iter().find(|n| n.id == node).and_then(|n| n.parent);
                        if let Some(ti) = wc.project.timeline.iter().position(|n| n.id == node) {
                            wc.ask.push(qymcad_ui_state::WinAsk::GoTo { owner, sel: Sel::Feature(ti) });
                        }
                    }
                }
            }
            Act::DropDriver(k) => {
                if k < wc.project.named_dims.len() {
                    let mut ed = qymcad_ui_state::edit_over(wc.rebuild(), crate::i18n::tr("par-edit-step"));
                    ed.project().named_dims.remove(k);
                    drop(ed);
                    *dirty = true;
                }
            }
        }
    }
    out.name_w = name_w;
    out.height = scrolled.inner_rect.height();
    out.content_h = scrolled.content_size.y;
    out
}

pub(crate) fn params_window(wc: &mut qymcad_ui_state::WinCtx, ctx: &egui::Context) {
    use qymcad_core::model::Param;
    if !wc.win.is(WinKind::Params) {
        return;
    }
    let mut open = true;
    let mut dirty = false;
    let mut remove: Option<usize> = None;
    egui::Window::new(format!("{} {}", ph::FUNCTION, crate::i18n::tr("win-params"))).open(&mut open).resizable(true).default_width(360.0).show(ctx, |ui| {
        ui.label(egui::RichText::new(crate::i18n::tr("par-hint")).weak().small());
        ui.separator();
        // THE FIELD WIDTHS ARE ELASTIC. They used to be a hard 90 and 120 points: the window stretched and the
        // fields did not, so a long variable name COULD NOT BE TYPED - the text crawled under the edge and one
        // had to type blind. The fields now share the window's width, and the hard numbers became a lower bound.
        let rows = params_rows_ui(wc, ui);
        dirty |= rows.dirty;
        remove = rows.remove;
        // THE REASONS, BELOW THE TABLE AND ACROSS ITS WHOLE WIDTH. A cause that has to be hunted for by
        // hovering is a cause nobody reads, and the narrowest column of the table is no place for a sentence.
        for (name, msg) in &rows.errors {
            ui.add(egui::Label::new(egui::RichText::new(format!("{name}: {msg}")).color(wc.scheme.pal.error_mild()).small()).wrap());
        }
        ui.separator();
        if ui.button(format!("{} {}", ph::PLUS, crate::i18n::tr("win-add-param"))).clicked() {
            wc.project.parameters.push(Param { name: String::new(), expr: String::new(), value: 0.0 });
            dirty = true;
        }
        // THE DRIVERS LIVE IN THE TABLE ITSELF (see `params_rows_ui`) rather than in a separate list below.
        //
        // That separate list was look-but-do-not-touch: a name, a path, a number, and nothing more. This table
        // is the one place where the project's WHOLE set of numbers is visible and editable; two different
        // lists with different rules have no business here.
    });
    if let Some(i) = remove {
        wc.project.parameters.remove(i);
        dirty = true;
    }
    if dirty {
        apply_param_edit(wc);
    }
    if !open {
        wc.win.close(WinKind::Params);
    }
}

/// A GLOBAL PARAMETER EDIT HAS BEEN APPLIED. A method of its own, because this path has to be TESTABLE: the
/// parameter window draws itself, and a test must pull the same handle rather than a similar one of its own.
pub(crate) fn apply_param_edit(wc: &mut qymcad_ui_state::WinCtx) {
    wc.project.eval_parameters();
    // THE REBUILD GRAPH: editing a parameter touches only the sketches that actually mention it, not every one
    // of them. The features with expressions over that name are marked by `mark_changed_params_dirty` (by
    // comparing against a snapshot of the values), so there is nothing to duplicate here.
    for si in 0..wc.project.sketches.len() {
        let uses = wc.project.sketches[si].constraints.iter().any(|c| c.expr().is_some());
        if uses {
            wc.project.solve_sketch(si);
            let sid = wc.project.sketches[si].id;
            wc.project.mark_sketch_dirty(sid);
        }
    }
    wc.ask.push(qymcad_ui_state::WinAsk::RegenerateAll); // the bodies rebuild associatively from the new parameters
}

/// THE SETTINGS WINDOW: the sections on the left, the search at the top, a per-section reset at the bottom.
///
/// It used to be one flat scroll where the sections were bold labels. Such a list does not scale: the settings
/// will treble, and a bedsheet can be neither searched nor reset in parts. The sections, their rows and their
/// resets are declared IN ONE place (`settings_sections.rs`), otherwise a new setting would reach the window
/// and not be found by the search - silently.
///
/// THE "UNITS: MILLIMETRES" LABEL WAS REMOVED. It stood among the settings and looked like one, yet it
/// switched nothing: inches were left to a separate piece of work later. An interface pretending to do what it
/// cannot is worse than a missing item.
pub(crate) fn settings_window(wc: &mut qymcad_ui_state::WinCtx, ctx: &egui::Context) {
    if !wc.win.is(WinKind::Settings) {
        return;
    }
    let mut open = wc.win.is(WinKind::Settings);
    egui::Window::new(format!("{} {}", ph::GEAR, crate::i18n::tr("win-settings"))).open(&mut open).default_width(620.0).default_height(460.0).show(ctx, |ui| {
        let mut q = std::mem::take(&mut wc.scheme.search);
        // the field's width does NOT come from the window's width - otherwise the window swells as text is typed (see the tree)
        ui.horizontal(|ui| {
            ui.label(ph::MAGNIFYING_GLASS);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut q)
                        .id(egui::Id::new("settings_search_field"))
                        .hint_text(crate::i18n::tr("settings-search"))
                        .desired_width(f32::INFINITY),
                );
            });
        });
        wc.scheme.search = q;
        let query = wc.scheme.search.clone();
        let searching = !query.trim().is_empty();
        ui.separator();

        let visible = settings_sections_visible(&*wc.scheme);
        if visible.is_empty() {
            ui.label(egui::RichText::new(crate::i18n::tr("settings-search-empty")).weak());
            return; // closing the window is handled by `open` OUTSIDE the closure

        }
        if !visible.contains(&wc.scheme.section) {
            wc.scheme.section = visible[0];
        }

        if searching {
            // THE SEARCH RUNS ACROSS THE SECTIONS: a person searches for a setting, not for a section, and is
            // under no obligation to know where it was put.
            egui::ScrollArea::vertical().show(ui, |ui| {
                for sec in visible {
                    ui.label(egui::RichText::new(crate::i18n::tr(sec.key())).strong());
                    settings_section_body(wc, ui, ctx, sec, &query);
                    ui.separator();
                }
            });
        } else {
            let cur = wc.scheme.section;
            egui::Panel::left("settings_sections").resizable(false).exact_size(168.0).show(ui, |ui| {
                for sec in &visible {
                    if ui.selectable_label(cur == *sec, crate::i18n::tr(sec.key())).clicked() {
                        wc.scheme.section = *sec;
                        wc.scheme.note.clear();
                    }
                }
                ui.separator();
                // WHERE THE CONFIG LIVES - otherwise support turns into guesswork
                if let Some(dir) = crate::gui::settings_dir() {
                    ui.label(egui::RichText::new(crate::i18n::tr1("settings-config-path", "path", &dir.display().to_string())).small().weak());
                    // OPEN THE FOLDER in the system file manager. No separate crate is added for one button:
                    // this is a single OS command, and it reads plainly as one.
                    if ui.small_button(format!("{}  {}", ph::FOLDER_OPEN, crate::i18n::tr("settings-open-folder"))).clicked() {
                        let (bin, args) = crate::gui::reveal_command(ui.ctx().os(), &dir);
                        if let Err(e) = std::process::Command::new(bin).args(&args).spawn() {
                            wc.scheme.note = crate::i18n::tr1("settings-open-folder-failed", "error", &e.to_string());
                        }
                    }
                }
            });
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.label(egui::RichText::new(crate::i18n::tr(cur.key())).strong());
                ui.separator();
                settings_section_body(wc, ui, ctx, cur, "");
                ui.separator();
                if ui.button(format!("{}  {}", ph::ARROW_COUNTER_CLOCKWISE, crate::i18n::tr("settings-reset-section"))).clicked() {
                    cur.reset(&mut *wc.set);
                    // the language and the scheme are not merely values: they have to be APPLIED, otherwise a
                    // reset shows only after a restart
                    crate::gui::apply_language(&*wc.set);
                    crate::gui::apply_theme(&mut *wc.scheme, &*wc.set, ctx);
                    wc.scheme.note = crate::i18n::tr1("settings-reset-done", "name", &crate::i18n::tr(cur.key()));
                }
                if !wc.scheme.note.is_empty() {
                    ui.label(egui::RichText::new(&wc.scheme.note).small().color(wc.scheme.pal.hint()));
                }
            });
        }
    });
    wc.win.set(WinKind::Settings, open);
}

/// A SECTION'S CONTENTS. An empty `query` shows everything; otherwise only the matching rows.
///
/// Every row goes through `row`, and its key must appear in its section's `row_keys`: a guard checks that in
/// both directions, which makes "present in the window but not searchable" inexpressible.
pub(crate) fn settings_section_body(wc: &mut qymcad_ui_state::WinCtx, ui: &mut egui::Ui, ctx: &egui::Context, sec: super::settings_sections::SettingsSection, query: &str) {
    use super::settings_sections::SettingsSection as Sec;
    let show = |k: &str| Sec::row_matches(k, query, &|s: &str| crate::i18n::tr(s));
    match sec {
        Sec::General => {
            // THE INTERFACE LANGUAGE. The list is built FROM THE `i18n/` CATALOGUE - drop a folder in and the
            // language appears; its name is shown in that language itself, so that it is recognised by whoever
            // does not read the current one.
            if show("settings-language") {
                ui.label(crate::i18n::tr("settings-language"));
                ui.horizontal_wrapped(|ui| {
                    let cur = crate::i18n::language();
                    for (code, name) in crate::i18n::available() {
                        if ui.selectable_label(cur == code, &name).clicked() {
                            wc.set.language = code.clone();
                            crate::i18n::set_language(&code);
                        }
                    }
                });
            }
            // THE HELP'S LANGUAGE IS SEPARATE FROM THE INTERFACE'S. CAD terminology is English, and someone
            // working in a translated interface may well want to read `sweep` and `loft` as they are written in
            // the manuals. The first choice is "as in the interface": that is both the default and the way back,
            // with no guessing which code was the native one.
            if show("settings-help-lang") {
                ui.label(crate::i18n::tr("settings-help-lang"));
                ui.horizontal_wrapped(|ui| {
                    if ui.selectable_label(wc.set.help_lang.is_empty(), crate::i18n::tr("settings-help-lang-follow")).clicked() {
                        wc.set.help_lang.clear();
                        crate::help::set_lang("");
                    }
                    for code in crate::help::languages() {
                        let name = crate::i18n::available().into_iter().find(|(c, _)| *c == code).map(|(_, n)| n).unwrap_or_else(|| code.clone());
                        if ui.selectable_label(wc.set.help_lang == code, &name).clicked() {
                            wc.set.help_lang = code.clone();
                            crate::help::set_lang(&code);
                        }
                    }
                });
            }
            if show("settings-help-open") {
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::tr("settings-help-open"));
                    ui.selectable_value(&mut wc.set.help_external, false, crate::i18n::tr("settings-help-open-window"));
                    ui.selectable_value(&mut wc.set.help_external, true, crate::i18n::tr("settings-help-open-browser"));
                });
                ui.label(egui::RichText::new(crate::i18n::tr("settings-help-open-hint")).weak().small());
            }
            // WHAT THE PROGRAM OPENS WITH: two independent answers, drawn as two ticks rather than as one
            // list of three states. The person asked for exactly this pair, and the pair says plainly that
            // "which document" and "am I greeted" are separate questions.
            if show("settings-open-last") {
                ui.checkbox(&mut wc.set.open_last, crate::i18n::tr("settings-open-last")).on_hover_text(crate::i18n::tr("settings-open-last-hint"));
            }
            if show("settings-show-start") {
                ui.checkbox(&mut wc.set.show_start_screen, crate::i18n::tr("settings-show-start")).on_hover_text(crate::i18n::tr("settings-show-start-hint"));
            }
            if show("settings-autosave") {
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::tr("settings-autosave"));
                    ui.add(egui::DragValue::new(&mut wc.set.autosave_secs).range(0..=3600).suffix(crate::i18n::tr("unit-seconds")));
                });
                ui.label(egui::RichText::new(crate::i18n::tr("settings-autosave-hint")).weak().small());
            }
            if show("settings-undo-cap") {
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::tr("settings-undo-cap"));
                    ui.add(egui::DragValue::new(&mut wc.set.undo_cap).range(1..=500));
                });
                ui.label(egui::RichText::new(crate::i18n::tr("settings-undo-cap-hint")).weak().small());
            }
            if show("settings-profile") {
                ui.add_space(4.0);
                ui.label(egui::RichText::new(crate::i18n::tr("settings-profile")).strong());
                ui.horizontal(|ui| {
                    if ui.button(format!("{}  {}", ph::EXPORT, crate::i18n::tr("settings-profile-export"))).clicked() {
                        wc.ask.push(qymcad_ui_state::WinAsk::ExportSettings);
                    }
                    if ui.button(format!("{}  {}", ph::FOLDER_OPEN, crate::i18n::tr("settings-profile-import"))).clicked() {
                        wc.ask.push(qymcad_ui_state::WinAsk::ImportSettings);
                    }
                });
                ui.label(egui::RichText::new(crate::i18n::tr("settings-profile-hint")).weak().small());
            }
            if show("settings-recent-limit") {
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::tr("settings-recent-limit"));
                    let before = wc.set.recent_limit;
                    ui.add(egui::DragValue::new(&mut wc.set.recent_limit).range(1..=50));
                    if wc.set.recent_limit < before {
                        // the list was shortened, so the excess is trimmed AT ONCE rather than at some next
                        // opening of the file: a setting must take effect where it is made
                        let n = wc.set.recent_limit.max(1);
                        wc.set.recent.truncate(n);
                    }
                });
            }
        }
        Sec::Appearance => {
            // THE INTERFACE SCALE is applied LIVE - choosing a size blind, with the window closed, is
            // impossible.
            //
            // NO DRAGGING HERE, and that is the point of the control rather than an omission.
            //
            // Reported behaviour, in two rounds. First: "egui lets you hold the left button and drag to
            // raise and lower the value; with us that breaks straight away - the value jumps to a whole
            // number instead of, say, 0.05." That much was arithmetic: `speed` is how far the value moves
            // per POINT dragged and stood at 0.05 over a range of 0.5 to 3.0 - the whole range in fifty
            // points. Measured with a real pointer: a twenty-point twitch took the scale from 1.00 to
            // 1.70. Slowing it to 0.01 fixed the arithmetic and NOT the control.
            //
            // Then: "it still glitches - you move the mouse, the scale changes, the window under the
            // mouse moves away, and the scale runs off further. Can this drag be turned off altogether?"
            // That is a loop, not a speed: this setting is applied LIVE, so every value the drag produces
            // resizes the very field being dragged. The pointer stays put while the widget travels out
            // from under it, and the drag keeps feeding on its own output. No speed makes that aimable.
            //
            // So the value is stepped by two buttons, 0.05 at a time. A click is one discrete step - the
            // window may relayout after it, and nothing has run away.
            if show("settings-ui-scale") {
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::tr("settings-ui-scale"));
                    let before = wc.set.ui_scale;
                    // THE MINUS AND PLUS STAND BEFORE THE NUMBER, so that changing it does not move them:
                    // the number is two characters wide at 1.00 and three at 1.05 -> 10.00, and a button
                    // that shifts under the finger cannot be clicked twice in a row.
                    if ui.small_button("-").clicked() {
                        wc.set.ui_scale -= UI_SCALE_STEP;
                    }
                    if ui.small_button("+").clicked() {
                        wc.set.ui_scale += UI_SCALE_STEP;
                    }
                    wc.set.ui_scale = ((wc.set.ui_scale / UI_SCALE_STEP).round() * UI_SCALE_STEP).clamp(0.5, 3.0);
                    ui.label(format!("{:.2}", wc.set.ui_scale));
                    if (wc.set.ui_scale - before).abs() > 1e-6 {
                        crate::gui::apply_ui_scale(&*wc.set, ctx);
                    }
                    if ui.small_button(crate::i18n::tr("settings-ui-scale-reset")).clicked() {
                        wc.set.ui_scale = 1.0;
                        crate::gui::apply_ui_scale(&*wc.set, ctx);
                    }
                });
            }
            if show("settings-scheme") {
                ui.label(crate::i18n::tr("settings-scheme"));
                scheme_section(wc, ui, ctx);
            }
        }
        Sec::Viewport => {
            // THE ORDER GOES FROM THE MAIN THING TO THE PARTICULAR, not however it happened to accumulate.
            //
            // It used to be: antialiasing, ghosts, field of view, projection, shading, the cube, precision.
            // First came what is touched once in a lifetime, while the projection - switched daily - hid in
            // the middle. The blocks were MOVED WHOLE: they carry side effects inside (clearing the raster
            // cache, re-uploading the vertex buffer), and rewriting them would have lost something silently.
            if show("settings-engine") {
                if *wc.gpu_ok {
                    // the 3D viewport's engine. The GPU (wgpu, a depth buffer) is faster and free of
                    // visibility artefacts; the CPU raster is the compatible fallback.
                    ui.horizontal(|ui| {
                        ui.label(crate::i18n::tr("settings-engine"));
                        let prev = wc.set.gpu_viewport;
                        ui.selectable_value(&mut wc.set.gpu_viewport, true, crate::i18n::tr("settings-engine-gpu"));
                        ui.selectable_value(&mut wc.set.gpu_viewport, false, crate::i18n::tr("settings-engine-cpu"));
                        if prev != wc.set.gpu_viewport {
                            *wc.cache.view.borrow_mut() = None; // force a redraw when it is switched
                            wc.cache.gpu_scene_key.set(u64::MAX); // force the GPU buffer to be re-uploaded
                        }
                    });
                } else {
                    ui.label(egui::RichText::new(format!("{} {}", ph::WARNING, crate::i18n::tr("settings-gpu-unavailable"))).weak().small());
                }
            }
            // The projection: one formula in `Screen::at`, so it works on BOTH paths (GPU and CPU raster) and the
            // overlays (edges, dimensions, gizmos) stay glued to the bodies.
            if show("settings-projection") {
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::tr("settings-projection"));
                    let prev = wc.set.projection;
                    ui.selectable_value(&mut wc.set.projection, Projection::Ortho, crate::i18n::tr("settings-projection-ortho"));
                    ui.selectable_value(&mut wc.set.projection, Projection::Perspective, crate::i18n::tr("settings-projection-persp"));
                    if prev != wc.set.projection {
                        *wc.cache.view.borrow_mut() = None; // the CPU raster: the projection changed
                    }
                });
            }
            // Shading: smooth (Gouraud, from the smoothed normals) or flat (from the face).
            if show("settings-shading") {
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::tr("settings-shading"));
                    let prev = wc.set.shading;
                    ui.selectable_value(&mut wc.set.shading, Shading::Smooth, crate::i18n::tr("settings-shading-smooth"));
                    ui.selectable_value(&mut wc.set.shading, Shading::Flat, crate::i18n::tr("settings-shading-flat"));
                    if prev != wc.set.shading {
                        *wc.cache.view.borrow_mut() = None;
                        wc.cache.gpu_scene_key.set(u64::MAX); // the GPU: re-upload the vertex buffer
                    }
                });
            }
            // THE NAVIGATION CUBE'S SIZE: on a 4K screen the old one is unreadable, on a small screen a large one gets in the way
            if show("settings-viewcube") {
                ui.label(crate::i18n::tr("settings-viewcube"));
                ui.horizontal(|ui| {
                    for (v, key) in [(0u8, "settings-viewcube-small"), (1, "settings-viewcube-medium"), (2, "settings-viewcube-large")] {
                        if ui.selectable_label(wc.set.viewcube_size == v, crate::i18n::tr(key)).clicked() {
                            wc.set.viewcube_size = v;
                        }
                    }
                });
            }
            // POINTING PRECISION: it scales the grab radii of every role at once (see `grab.rs`). On a 4K or a
            // touch screen the pixel radii are small; with a mouse, large ones make it hard to aim in tight
            // geometry - that is a person's choice, not ours.
            // WHICH BUTTON MOVES THE VIEW. Walked over `MouseNav::ALL` rather than listed here: a set added
            // to the type appears in the window by itself, and a list written out beside a full one falls
            // behind on the first addition (D19).
            if show("settings-mouse-nav") {
                // A DROPPING LIST, not a row of buttons: eleven layouts laid out side by side run off the
                // edge of the window and stop being readable at about the fourth.
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::tr("settings-mouse-nav"));
                    egui::ComboBox::from_id_salt("mouse-nav")
                        .selected_text(crate::i18n::tr(&wc.set.mouse_nav.key()))
                        .show_ui(ui, |ui| {
                            for nav in qymcad_ui_state::MouseNav::ALL {
                                ui.selectable_value(&mut wc.set.mouse_nav, nav, crate::i18n::tr(&nav.key()))
                                    .on_hover_text(crate::i18n::tr(&nav.hint_key()));
                            }
                        });
                });
                // WHAT THE CHOSEN ONE DOES, under the list rather than in a tooltip: a person picking a
                // layout is choosing between habits, and a habit cannot be compared with one hidden behind
                // a hover.
                ui.label(egui::RichText::new(crate::i18n::tr(&wc.set.mouse_nav.hint_key())).weak().small());
            }
            // WHERE THE VIEW ZOOMS FROM. Walked over `ZoomAt::ALL` rather than listed here, so a variant
            // added to the type appears in the window by itself (D19).
            if show("settings-zoom-at") {
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::tr("settings-zoom-at"));
                    for z in qymcad_ui_state::ZoomAt::ALL {
                        ui.selectable_value(&mut wc.set.zoom_at, z, crate::i18n::tr(z.key()));
                    }
                });
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::tr("settings-zoom-editing"));
                    for z in qymcad_ui_state::ZoomWhileEditing::ALL {
                        ui.selectable_value(&mut wc.set.zoom_editing, z, crate::i18n::tr(z.key()));
                    }
                });
                ui.label(egui::RichText::new(crate::i18n::tr("settings-zoom-editing-hint")).weak());
            }
            if show("settings-pick-precision") {
                ui.label(crate::i18n::tr("settings-pick-precision"));
                ui.horizontal(|ui| {
                    for (v, key) in [(0u8, "settings-pick-fine"), (1, "settings-pick-normal"), (2, "settings-pick-coarse")] {
                        if ui.selectable_label(wc.set.pick_precision == v, crate::i18n::tr(key)).clicked() {
                            wc.set.pick_precision = v;
                        }
                    }
                });
            }
            if show("settings-ghost-alpha") {
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::tr("settings-ghost-alpha"));
                    if ui.add(egui::Slider::new(&mut wc.set.ghost_alpha, 20..=255)).changed() {
                        qymcad_ui_state::invalidate(&mut *wc.regen); // the ghost is drawn from the raster cache, so the edit would not be seen otherwise
                    }
                });
            }
            if show("settings-fov") {
                // AN INAPPLICABLE SETTING GOES GREY AND SAYS WHY.
                //
                // A field of view exists only in perspective. Under an orthographic projection the slider
                // looked alive, moved and changed nothing: a setting that pretends to work is worse than a
                // missing one - a person turns it and concludes the program is broken. It is greyed out and the
                // reason written beside it, not only in a tooltip: a tooltip has to be summoned first.
                let persp = wc.set.projection == Projection::Perspective;
                ui.add_enabled_ui(persp, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(crate::i18n::tr("settings-fov"));
                        if ui.add(egui::Slider::new(&mut wc.set.persp_fov_deg, 10.0..=90.0).suffix("°")).changed() {
                            qymcad_ui_state::invalidate(&mut *wc.regen);
                        }
                    });
                })
                .response
                .on_disabled_hover_text(crate::i18n::tr("settings-fov-needs-persp"));
                if !persp {
                    ui.label(egui::RichText::new(crate::i18n::tr("settings-fov-needs-persp")).weak().small());
                }
            }
            if show("settings-msaa") {
                // ANTIALIASING BELONGS TO THE GPU VIEWPORT ONLY: the software raster draws its own way, and
                // the wgpu sample count does not affect it at all. On the CPU path the row looked alive.
                let gpu = *wc.gpu_ok && wc.set.gpu_viewport;
                ui.add_enabled_ui(gpu, |ui| {
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::tr("settings-msaa"));
                    // THE LIST COMES FROM THE DEVICE, not from us. The specification guarantees only 1 and 4;
                    // 8x was once offered without asking, and on a real graphics card the program CRASHED ON
                    // START - the setting made it unlaunchable. What the hardware cannot do is not shown.
                    for n in crate::viewport_gpu::supported_msaa() {
                        let label = if n == 1 { crate::i18n::tr("settings-msaa-off") } else { format!("{n}×") };
                        if ui.selectable_label(wc.set.msaa == n, label).clicked() {
                            wc.set.msaa = n;
                        }
                    }
                });
                })
                .response
                .on_disabled_hover_text(crate::i18n::tr("settings-msaa-needs-gpu"));
                // THE SETTING'S PRICE IS SAID OUT LOUD. The sample count is baked into the wgpu pipelines at
                // start-up; silently not applying it would be a lie, so it is stated plainly. And on the CPU
                // raster what is said is not "restart" but that the setting has nothing to do with it.
                ui.label(egui::RichText::new(if gpu { crate::i18n::tr("settings-msaa-restart") } else { crate::i18n::tr("settings-msaa-needs-gpu") }).weak().small());
            }
        }
        Sec::Sketch => {
            if show("settings-snap-on") {
                ui.checkbox(&mut wc.set.snap.on, crate::i18n::tr("settings-snap-on"));
            }
            if show("settings-grid-step") {
                ui.add_enabled(
                    wc.set.snap.on,
                    egui::DragValue::new(&mut wc.set.snap.grid).speed(0.5).range(0.1..=100.0).prefix(crate::i18n::tr("settings-grid-step")).suffix(crate::i18n::tr("unit-mm-suffix")),
                );
            }
            if show("settings-rot-step") {
                ui.add_enabled(wc.set.snap.on, egui::DragValue::new(&mut wc.set.snap.rot_deg).speed(1.0).range(0.5..=90.0).prefix(crate::i18n::tr("settings-rot-step")).suffix("°"));
            }
            if show("settings-auto-constrain") {
                ui.checkbox(&mut wc.set.auto_constrain, crate::i18n::tr("settings-auto-constrain")).on_hover_text(crate::i18n::tr("settings-auto-constrain-hint"));
            }
        }
        Sec::Part => {
            egui::Grid::new("settings_def").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
                if show("settings-default-extrude") {
                    ui.label(crate::i18n::tr("settings-default-extrude"));
                    ui.add(egui::DragValue::new(&mut wc.set.defaults.extrude_h).speed(0.5).range(0.1..=5000.0).suffix(crate::i18n::tr("unit-mm-suffix")));
                    ui.end_row();
                }
                if show("settings-default-offset") {
                    ui.label(crate::i18n::tr("settings-default-offset"));
                    ui.add(egui::DragValue::new(&mut wc.set.defaults.offset_2d).speed(0.2).range(0.1..=500.0).suffix(crate::i18n::tr("unit-mm-suffix")));
                    ui.end_row();
                }
            });
        }
        Sec::Assembly => {
            // The shared contours toggle is about the ASSEMBLY only: inside a Part every sketch has a
            // visibility checkbox of its own, and a shared one would duplicate it.
            if show("settings-show-contours") {
                ui.checkbox(&mut wc.set.show_contours, crate::i18n::tr("settings-show-contours"));
            }
            if show("settings-show-joints") {
                ui.checkbox(&mut wc.set.show_joints, crate::i18n::tr("settings-show-joints"));
            }
            if show("settings-show-interference") {
                ui.checkbox(&mut wc.set.show_interference, crate::i18n::tr("settings-show-interference"));
            }
        }
        Sec::Layout => {
            // WHERE THE PANELS STAND. The list is the shell's own: this code names no panel and no
            // workbench, it walks what was registered. A place added by anyone appears here by itself.
            let shell = crate::gui::shell(&*wc.set);
            if show("settings-layout-place") {
                ui.label(crate::i18n::tr("settings-layout-place"));
                let mut moves: Vec<(String, qymcad_shell::Slot)> = Vec::new();
                for key in shell.keys() {
                    let now = shell.slot_of(key).unwrap_or(qymcad_shell::Slot::Centre);
                    ui.horizontal(|ui| {
                        ui.label(crate::i18n::tr(&format!("panel-{key}")));
                        for slot in qymcad_shell::Slot::ORDER {
                            if ui.selectable_label(now == slot, crate::i18n::tr(slot.key())).clicked() && now != slot {
                                moves.push((key.to_string(), slot));
                            }
                        }
                    });
                }
                for (k, slot) in moves {
                    let mut sh = crate::gui::shell(&*wc.set);
                    sh.move_to(&k, slot);
                    wc.set.layout = sh.saved();
                }
            }
            if show("settings-layout-reset") && ui.button(crate::i18n::tr("settings-layout-reset")).clicked() {
                let mut sh = crate::gui::shell(&*wc.set);
                sh.reset();
                wc.set.layout = sh.saved();
            }
        }
    }
}

/// THE DOCUMENT PROPERTIES WINDOW. The document used to be a nameless heap of geometry.
///
/// The fields are free text on purpose: one person's version is `1.2` and another's is `rev. B`, and an imposed
/// format would only be worked around in a comment. The program does not interpret them - it stores and shows them.
///
/// The edits go STRAIGHT INTO THE DOCUMENT, as everything else does: the project becomes dirty and the "not
/// saved?" question is asked on exit. That is right in substance - the properties travel WITH THE FILE.
pub(crate) fn save_template_dialog(wc: &mut qymcad_ui_state::WinCtx, ctx: &egui::Context) {
    if !wc.win.is(WinKind::SaveTemplate) {
        return;
    }
    let mut done = false;
    let mut cancel = ctx.input(|i| i.key_pressed(egui::Key::Escape));
    egui::Window::new(format!("{} {}", ph::PACKAGE, crate::i18n::tr("file-save-as-template")))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.label(crate::i18n::tr("tpl-name"));
            let r = ui.text_edit_singleline(&mut wc.win.tpl_name);
            r.request_focus();
            if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                done = true;
            }
            ui.label(egui::RichText::new(crate::i18n::tr("tpl-hint")).weak().small());
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.add_enabled(!wc.win.tpl_name.trim().is_empty(), egui::Button::new(crate::i18n::tr("confirm-yes"))).clicked() {
                    done = true;
                }
                if ui.button(crate::i18n::tr("confirm-no")).clicked() {
                    cancel = true;
                }
            });
        });
    if done && !wc.win.tpl_name.trim().is_empty() {
        let name = wc.win.tpl_name.trim().to_string();
        crate::gui::save_as_template(&*wc.project, &mut *wc.status, &name);
        wc.win.close(WinKind::SaveTemplate);
    } else if cancel {
        wc.win.close(WinKind::SaveTemplate);
    }
}

pub(crate) fn doc_props_window(wc: &mut qymcad_ui_state::WinCtx, ctx: &egui::Context) {
    if !wc.win.is(WinKind::DocProps) {
        return;
    }
    let mut open = wc.win.is(WinKind::DocProps);
    egui::Window::new(format!("{} {}", ph::FILE_TEXT, crate::i18n::tr("doc-props-title")))
        .open(&mut open)
        .resizable(true)
        .default_width(420.0)
        .show(ctx, |ui| {
            egui::Grid::new("doc_props").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
                ui.label(crate::i18n::tr("doc-props-name"));
                ui.text_edit_singleline(&mut wc.project.meta.title);
                ui.end_row();
                ui.label(crate::i18n::tr("doc-props-author"));
                ui.text_edit_singleline(&mut wc.project.meta.author);
                ui.end_row();
                ui.label(crate::i18n::tr("doc-props-version"));
                ui.text_edit_singleline(&mut wc.project.meta.version);
                ui.end_row();
            });
            ui.label(crate::i18n::tr("doc-props-comment"));
            ui.add(egui::TextEdit::multiline(&mut wc.project.meta.comment).desired_rows(4).desired_width(f32::INFINITY));
            ui.separator();
            // THE GEOMETRY TOLERANCE LIVES HERE, NOT IN THE PROGRAM'S SETTINGS. Both the look on screen and the
            // contents of an STL depend on it: were it a program setting, one file would export differently for
            // two people, and both would be sure the program was lying.
            ui.label(egui::RichText::new(crate::i18n::tr("doc-props-quality")).strong());
            ui.horizontal_wrapped(|ui| {
                for q in qymcad_core::model::GeomQuality::all() {
                    if ui.selectable_label(wc.project.geom_quality == q, crate::i18n::tr(q.label_key())).clicked() && wc.project.geom_quality != q {
                        qymcad_ui_state::begin_edit(&mut *wc.edits, &*wc.project, crate::i18n::tr("doc-props-quality")); // THE EDIT BOUNDARY: this changes the document and can be undone
                        wc.project.geom_quality = q;
                        // the geometry must be recomputed: the tolerance changes THE MESH, not just a number
                        wc.project.mark_bodies_dirty();
                        qymcad_ui_state::commit_edit(&mut wc.rebuild());
                        qymcad_ui_state::mark_dirty_for_rebuild(&mut wc.rebuild());
                    }
                }
            });
            ui.label(egui::RichText::new(crate::i18n::tr("doc-props-quality-hint")).weak().small());
            ui.separator();
            // THE FACTS THE PROGRAM KNOWS BY ITSELF - they are read, not edited
            let created = wc.project.meta.created.clone();
            ui.label(
                egui::RichText::new(if created.is_empty() {
                    crate::i18n::tr("doc-props-not-saved-yet")
                } else {
                    crate::i18n::tr1("doc-props-created", "when", &created)
                })
                .small()
                .weak(),
            );
            let counts = crate::i18n::tr2("doc-props-counts", "parts", &wc.project.components.len().to_string(), "bodies", &wc.project.bodies.len().to_string());
            ui.label(egui::RichText::new(counts).small().weak());
            if let Some(p) = wc.project_path.clone() {
                ui.label(egui::RichText::new(crate::i18n::tr1("doc-props-path", "path", &p)).small().weak());
            }
            // WHICH BUILD WROTE THIS FILE. Shown, not editable: it is the program's word about itself,
            // and the person's own version lives in the field above. Empty for a document that has
            // never been saved, and then there is nothing to say.
            if !wc.project.meta.saved_by.is_empty() {
                let by = wc.project.meta.saved_by.clone();
                ui.label(egui::RichText::new(crate::i18n::tr1("doc-props-saved-by", "build", &by)).small().weak());
            }
        });
    wc.win.set(WinKind::DocProps, open);
}

/// The modal "save the changes?" dialogue - drawn for as long as a navigation is pending.
pub(crate) fn nav_dialog(wc: &mut qymcad_ui_state::WinCtx, ctx: &egui::Context) {
    // THE FLOOR IS CHECKED BEFORE THE NAVIGATION, not after it. The write finishing is what sets
    // `pending_nav`, and performing it at once takes the whole frame away - card and all - so a card put
    // up a moment earlier would blink out exactly as it did before. The navigation waits the few
    // milliseconds it takes for what is on screen to be readable.
    if let Some(shown) = wc.waiting.save_shown {
        if std::time::Instant::now().duration_since(shown) < super::SAVE_WAIT_MIN {
            crate::gui::render::draw_splash(&*wc.logo_tex, &*wc.scheme, ctx, &crate::i18n::tr("io-saving"));
            ctx.request_repaint();
            return;
        }
        wc.waiting.save_since = None;
        wc.waiting.save_shown = None;
    }
    // A NAVIGATION THAT WAITED FOR THE WRITE IS PERFORMED HERE: this is where `ctx` exists, which the
    // background task's handler does not have.
    if let Some(nav) = wc.pending_nav.take() {
        wc.ask.push(qymcad_ui_state::WinAsk::Nav(nav));
        return;
    }
    // WHILE THE WRITE IS RUNNING THERE IS A WAITING CARD, NOT SILENCE. The "save?" question has been answered
    // and the navigation is being waited for; the window is alive and says what is going on.
    if wc.deferred.nav_after_save {
        let now = std::time::Instant::now();
        // THE NAME IS STILL BEING CHOSEN. Save As puts the chooser up and returns at once - it no
        // longer holds the frame thread - so no write has started yet. Nothing is drawn over the
        // window (the person is looking at the system chooser), but the navigation keeps waiting:
        // dropping it here would save the file and never open the document that was asked for.
        if wc.file_ask_open {
            return;
        }
        if crate::gui::io_jobs::saving_now(&*wc.regen) {
            // THE CARD DOES NOT BLINK. A small document is written faster than an eye can catch, and a
            // card flashing for one frame reads as a glitch rather than an answer: nothing is shown
            // before `SAVE_WAIT_GRACE`.
            let since = *wc.waiting.save_since.get_or_insert(now);
            if now.duration_since(since) >= super::SAVE_WAIT_GRACE {
                wc.waiting.save_shown.get_or_insert(now);
                crate::gui::render::draw_splash(&*wc.logo_tex, &*wc.scheme, ctx, &crate::i18n::tr("io-saving"));
            }
            ctx.request_repaint();
            return;
        }
        // NO WRITE EVER HAPPENED, so the navigation goes no further: the chooser was closed without a
        // name, or the request never reached a write. The person is left where they were, with their
        // edits - which is what walking away from "where shall I put it?" means. The question is NOT
        // asked a second time; that reads as a loop rather than an answer.
        wc.waiting.save_since = None;
        wc.waiting.save_shown = None;
        wc.deferred.nav_after_save = false;
        wc.deferred.nav = None;
    }
    if wc.deferred.nav.is_none() {
        return;
    }
    let mut choice: Option<u8> = None; // 0 save, 1 do not save, 2 cancel
    egui::Window::new(format!("{}  {}", ph::WARNING, crate::i18n::tr("win-unsaved")))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            ui.label(crate::i18n::tr("nav-unsaved-text"));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button(format!("{}  {}", ph::FLOPPY_DISK, crate::i18n::tr("io-save"))).clicked() {
                    choice = Some(0);
                }
                if ui.button(crate::i18n::tr("nav-dont-save")).clicked() {
                    choice = Some(1);
                }
                if ui.button(crate::i18n::tr("nav-cancel")).clicked() {
                    choice = Some(2);
                }
            });
        });
    match choice {
        Some(0) => {
            wc.ask.push(qymcad_ui_state::WinAsk::Save);
            // THE WRITE IS WAITED FOR WITHOUT FREEZING THE WINDOW.
            //
            // There used to be a blocking `wait_bg()` here: no frame was drawn at all while the file went to
            // disk. To a person that is indistinguishable from a hung program. Now the navigation simply waits
            // its turn, and a waiting card is drawn for the duration of the write.
            //
            // The request to save may never have reached a write (Save As was cancelled) - then there is no
            // background task and the navigation is cancelled at once, as before. A chooser still open
            // counts as a write on its way: the name has been asked for and not yet given.
            if crate::gui::io_jobs::saving_now(&*wc.regen) || wc.file_ask_open {
                wc.deferred.nav_after_save = true;
            } else if !qymcad_ui_state::is_dirty(&mut wc.rebuild()) {
                if let Some(nav) = wc.deferred.nav.take() {
                    wc.ask.push(qymcad_ui_state::WinAsk::Nav(nav));
                }
            } else {
                wc.deferred.nav = None;
            }
        }
        Some(1) => {
            if let Some(nav) = wc.deferred.nav.take() {
                wc.ask.push(qymcad_ui_state::WinAsk::Nav(nav));
            }
        }
        Some(_) => wc.deferred.nav = None, // Cancel: stay here and drop the navigation
        None => {}
    }
}

/// The modal dialogue for choosing the STL quality (the deflection) before an export.
pub(crate) fn stl_quality_dialog(wc: &mut qymcad_ui_state::WinCtx, ctx: &egui::Context) {
    let Some(target) = *wc.stl_export else { return };
    let mut choice: Option<Option<f64>> = None; // None = close; Some(Some(defl)) = export; Some(None) = cancel
    egui::Window::new(crate::i18n::tr("stl-title"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.label(crate::i18n::tr("stl-detail"));
            ui.add_space(4.0);
            // (the label, the deflection in mm)
            let presets: [(&str, f64); 4] = [
                (&crate::i18n::tr("stl-draft"), 0.2),
                (&crate::i18n::tr("stl-standard"), 0.05),
                (&crate::i18n::tr("stl-high"), 0.02),
                (&crate::i18n::tr("stl-max"), 0.005),
            ];
            for (lbl, defl) in presets {
                if ui.add_sized([260.0, 24.0], egui::Button::new(lbl)).clicked() {
                    choice = Some(Some(defl));
                }
            }
            ui.add_space(6.0);
            if ui.button(crate::i18n::tr("nav-cancel")).clicked() {
                choice = Some(None);
            }
        });
    match choice {
        Some(Some(defl)) => {
            *wc.stl_export = None;
            wc.ask.push(qymcad_ui_state::WinAsk::ExportStl(target, defl));
        }
        Some(None) => *wc.stl_export = None,
        None => {}
    }
}

/// The modal confirmation popup for deleting a tree node. Enter or Yes deletes, Esc or No cancels.
pub(crate) fn confirm_delete_popup(wc: &mut qymcad_ui_state::WinCtx, ctx: &egui::Context) {
    let Some(sel) = wc.deferred.delete else { return };
    let what = crate::gui::sel_delete_label(&*wc.project, sel);
    let cascade = crate::gui::delete_cascade_names(&*wc.project, sel);
    let mut do_del = ctx.input(|i| i.key_pressed(egui::Key::Enter)); // Yes is the default
    let mut cancel = ctx.input(|i| i.key_pressed(egui::Key::Escape));
    egui::Window::new(format!("{} {}", ph::TRASH, crate::i18n::tr("win-delete-q")))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.label(crate::i18n::tr1("confirm-delete-what", "what", &what));
            // WHAT WILL GO WITH IT, BY NAME. A general line saying "along with its dependants" is true but does
            // not answer "what am I about to lose"; a list does, and it comes from the same core query as the
            // lineage in the properties card.
            if cascade.is_empty() {
                ui.label(egui::RichText::new(crate::i18n::tr("confirm-cascade")).weak().small());
            } else {
                ui.label(egui::RichText::new(crate::i18n::tr1("confirm-cascade-n", "n", &cascade.len().to_string())).weak().small());
                for n in cascade.iter().take(8) {
                    ui.label(egui::RichText::new(format!("  · {n}")).weak().small());
                }
                if cascade.len() > 8 {
                    ui.label(egui::RichText::new(crate::i18n::tr1("confirm-cascade-more", "n", &(cascade.len() - 8).to_string())).weak().small());
                }
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.add(egui::Button::new(crate::i18n::tr("confirm-yes")).min_size(egui::vec2(64.0, 0.0))).clicked() {
                    do_del = true;
                }
                if ui.add(egui::Button::new(crate::i18n::tr("confirm-no")).min_size(egui::vec2(64.0, 0.0))).clicked() {
                    cancel = true;
                }
                ui.label(egui::RichText::new(crate::i18n::tr("confirm-keys")).weak().small());
            });
        });
    if do_del {
        wc.deferred.delete = None;
        wc.ask.push(qymcad_ui_state::WinAsk::Delete(sel));
    } else if cancel {
        wc.deferred.delete = None;
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

