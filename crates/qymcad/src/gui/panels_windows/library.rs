//! THE WINDOW OF THE PARTS LIBRARY.

use super::*;

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
    egui::Window::new(format!("{} {}", ph::PACKAGE, crate::i18n::tr("win-parts-library"))).id(egui::Id::new("win_parts_library")).open(&mut open).default_width(640.0).default_height(440.0).show(
        ctx,
        |ui| {
            ui.horizontal(|ui| {
                if ui.button(format!("{} {}", ph::ARROWS_CLOCKWISE, crate::i18n::tr("win-refresh"))).on_hover_text(crate::i18n::tr("pl-rescan")).clicked() {
                    rescan = true;
                }
                ui.separator();
                ui.label(ph::MAGNIFYING_GLASS.to_string());
                ui.add(egui::TextEdit::singleline(&mut wc.parts.search).hint_text(crate::i18n::tr("pl-search")).desired_width(200.0));
                if !wc.parts.search.is_empty() && ui.small_button(ph::X).clicked() {
                    wc.parts.search.clear();
                }
            });
            ui.separator();
            let query = wc.parts.search.trim().to_lowercase();
            egui::Panel::left("parts_lib_tree").resizable(true).default_size(230.0).show(ui, |ui| {
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
        },
    );
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
