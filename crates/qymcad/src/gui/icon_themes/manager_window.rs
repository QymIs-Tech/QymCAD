//! ICON THEME MANAGER WINDOW AND SIDEBAR.
//!
//! Main management interface for activating, reordering priority cascades,
//! inspecting theme readmes, and handling duplicate theme conflicts.

use egui_phosphor::regular as ph;
use qymcad_ui_state::icons::{clean_directory_icon, clean_directory_icons, load_default_pack, BundleFormat, CleanIconResult, PackSource, ALL_ICONS, DEFAULT_THEME_ID};
use qymcad_ui_state::WinCtx;
use std::path::PathBuf;

use super::appearance_section::draw_bundle_format_badge;
pub(crate) use super::appearance_section::apply_icon_themes;
use super::conflicts::draw_duplicate_conflict_view;
use super::discovery::{all_theme_dirs, discover_all_theme_packs, ensure_discovery_worker, stop_discovery_worker};
use super::gallery::{
    draw_gallery_icon_row, draw_pack_icon, draw_pack_icon_bytes, forget_pack_gallery_textures, manager_archive_preview, manager_directory_preview, manager_embedded_preview, GalleryRowParams,
    ManagerPreviewImage,
};
use super::packager_dialog::{draw_packager_modal, open_packager_for_directory, PackagerDialogState};
use super::sidebar::{draw_icon_manager_actions, draw_manager_card_title, manager_sidebar_shell, manager_theme_card, IndexSwap};

/// Active tab in the Icon Theme Manager window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum IconManagerTab {
    Readme,
    Gallery,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CopiedPathNotice {
    pub path: String,
    pub timestamp: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct IconManagerState {
    pub is_open: bool,
    pub selected_pack_id: String,
    pub active_tab: IconManagerTab,
    pub search_query: String,
    pub category_filter: String,
    pub(crate) clean_notice: Option<CleanNotice>,
    pub(crate) copied_path: Option<CopiedPathNotice>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CleanNotice {
    pub pack_id: String,
    pub text: String,
    pub is_error: bool,
    pub details: Vec<String>,
}

impl Default for IconManagerState {
    fn default() -> Self {
        Self { is_open: false, selected_pack_id: String::new(), active_tab: IconManagerTab::Readme, search_query: String::new(), category_filter: "all".into(), clean_notice: None, copied_path: None }
    }
}

/// Request to open the Icon Theme Manager window.
pub(crate) fn open_icon_manager(ctx: &egui::Context) {
    ctx.data_mut(|d| {
        let state = d.get_temp_mut_or_default::<IconManagerState>(egui::Id::new("icon_manager_window"));
        state.is_open = true;
    });
}

/// Poll watched theme packs during the frame.
pub(crate) fn poll_icon_themes_frame(ctx: &egui::Context, palette: &qymcad_scheme::Palette) {
    if qymcad_ui_state::icons::has_watched_icon_packs(ctx) {
        qymcad_ui_state::icons::ensure_watcher_thread(ctx, palette);
    }
}

/// Draw the dedicated Icon Theme Manager window.
pub(crate) fn draw_icon_manager_window(ctx: &egui::Context, wc: &mut WinCtx) {
    poll_icon_themes_frame(ctx, &wc.scheme.pal);
    draw_icon_manager_window_in_dirs(ctx, wc, &all_theme_dirs());
}

pub(crate) fn draw_icon_manager_window_in_dirs(ctx: &egui::Context, wc: &mut WinCtx, dirs: &[PathBuf]) {
    let locale = crate::i18n::language();
    let mut state = ctx.data_mut(|d| d.get_temp_mut_or_default::<IconManagerState>(egui::Id::new("icon_manager_window")).clone());

    let mut packager_state = ctx.data_mut(|d| d.get_temp_mut_or_default::<PackagerDialogState>(egui::Id::new("icon_packager_dialog")).clone());

    if packager_state.is_open {
        draw_packager_modal(ctx, &mut packager_state);
    }

    ctx.data_mut(|d| {
        d.insert_temp(egui::Id::new("icon_packager_dialog"), packager_state);
    });

    if !state.is_open {
        stop_discovery_worker(ctx);
        return;
    }
    ensure_discovery_worker(ctx, dirs);

    let initial_selected_pack_id = state.selected_pack_id.clone();
    let mut open = state.is_open;
    let mut changed = false;

    // Ensure the built-in default pack is never placed in the user's active or inactive cascade lists
    if wc.set.active_icon_packs.iter().any(|id| id == DEFAULT_THEME_ID) {
        wc.set.active_icon_packs.retain(|id| id != DEFAULT_THEME_ID);
        changed = true;
    }
    if wc.set.inactive_icon_packs.iter().any(|id| id == DEFAULT_THEME_ID) {
        wc.set.inactive_icon_packs.retain(|id| id != DEFAULT_THEME_ID);
        changed = true;
    }

    let discovered = discover_all_theme_packs(dirs);
    let mut all_packs = discovered.packs;

    // Retain only directory packs in watched list
    wc.set.watched_icon_packs.retain(|id| all_packs.iter().find(|p| &p.manifest.id == id).is_some_and(|p| p.is_directory()));

    // Ensure the built-in default pack is always represented if discovered or from embedded
    if !all_packs.iter().any(|p| p.manifest.id == DEFAULT_THEME_ID && !p.has_id_conflict()) {
        if let Some(default_pack) = load_default_pack() {
            all_packs.push(default_pack);
        }
    }

    // Default selection if current is invalid
    if !all_packs.iter().any(|p| p.selection_key() == state.selected_pack_id || p.manifest.id == state.selected_pack_id) {
        if let Some(first) = all_packs.first() {
            state.selected_pack_id = first.selection_key();
        }
    }

    egui::Window::new(format!("{} {}", ph::PALETTE, crate::i18n::tr("icontheme-mgr-title")))
        .id(egui::Id::new("icon_manager_window"))
        .open(&mut open)
        .default_size(egui::vec2(1020.0, 700.0))
        .min_size(egui::vec2(760.0, 500.0))
        .resizable(true)
        .show(ctx, |ui| {
            ui.label(egui::RichText::new(crate::i18n::tr("icontheme-mgr-desc")).weak());
            ui.add_space(8.0);
            ui.separator();

            egui::Panel::left("icon_manager_sidebar").resizable(true).default_size(330.0).size_range(300.0..=420.0).show(ui, |ui| {
                manager_sidebar_shell(ui, |ui| {
                    draw_icon_manager_actions(ui);
                })
                .show(ui, |ui| {
                    ui.label(egui::RichText::new(crate::i18n::tr("icontheme-mgr-active-cascade")).strong());
                    ui.label(egui::RichText::new(crate::i18n::tr("settings-icon-themes-priority-hint")).small().weak());
                    ui.add_space(6.0);

                    let mut to_swap: Option<IndexSwap> = None;
                    let mut to_remove = None;

                    for (idx, id) in wc.set.active_icon_packs.iter().enumerate() {
                        let pack_opt = all_packs.iter().find(|p| &p.manifest.id == id);
                        let is_selected = state.selected_pack_id == *id;

                        let card = manager_theme_card(ui, ("active_theme", idx, id.as_str()), is_selected, |ui| {
                            let text_width = (ui.available_width() - 150.0).max(96.0);
                            ui.horizontal(|ui| {
                                if let Some(pack) = pack_opt {
                                    draw_pack_icon(ui, pack, &wc.scheme.pal, 36.0);
                                } else {
                                    ui.add_sized([36.0, 36.0], egui::Label::new(ph::PACKAGE));
                                }
                                ui.vertical(|ui| {
                                    let name = pack_opt.map(|p| p.manifest.name_for_locale(&locale)).unwrap_or(id.as_str());
                                    let title = format!("{:02}  {name}", idx + 1);
                                    draw_manager_card_title(ui, text_width, &title, name);
                                    if let Some(p) = pack_opt {
                                        ui.horizontal(|ui| {
                                            draw_bundle_format_badge(ui, p.format(), p.is_tampered);
                                            if p.is_directory() && wc.set.watched_icon_packs.contains(id) {
                                                ui.label(egui::RichText::new(ph::EYE).small().color(ui.visuals().warn_fg_color)).on_hover_text(crate::i18n::tr("icontheme-mgr-watch-this-pack"));
                                            }
                                        });
                                    }
                                });
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if ui.add(egui::Button::new(ph::MINUS).small()).on_hover_text(crate::i18n::tr("icontheme-mgr-deactivate-btn")).clicked() {
                                        to_remove = Some(idx);
                                    }
                                    if ui
                                        .add_enabled(idx + 1 < wc.set.active_icon_packs.len(), egui::Button::new(ph::ARROW_DOWN).small())
                                        .on_hover_text(crate::i18n::tr("icontheme-mgr-move-down"))
                                        .clicked()
                                    {
                                        to_swap = Some(IndexSwap { from: idx, to: idx + 1 });
                                    }
                                    let leftmost_action = ui.add_enabled(idx > 0, egui::Button::new(ph::ARROW_UP).small()).on_hover_text(crate::i18n::tr("icontheme-mgr-move-up"));
                                    if leftmost_action.clicked() {
                                        to_swap = Some(IndexSwap { from: idx, to: idx - 1 });
                                    }
                                    Some(leftmost_action.rect)
                                })
                                .inner
                            })
                            .inner
                        });
                        if card.clicked {
                            state.selected_pack_id = id.clone();
                        }
                        ui.add_space(4.0);
                    }

                    // Base fallback
                    let base_card = manager_theme_card(ui, ("base_fallback_theme", DEFAULT_THEME_ID), state.selected_pack_id == DEFAULT_THEME_ID, |ui| {
                        let text_width = (ui.available_width() - 44.0).max(110.0);
                        let base_pack = all_packs.iter().find(|pack| pack.manifest.id == DEFAULT_THEME_ID && !pack.has_id_conflict());
                        ui.horizontal(|ui| {
                            if let Some(pack) = base_pack {
                                draw_pack_icon(ui, pack, &wc.scheme.pal, 36.0);
                            }
                            ui.vertical(|ui| {
                                let name = base_pack.map(|pack| pack.manifest.name_for_locale(&locale).to_string()).unwrap_or_else(|| crate::i18n::tr("settings-icon-themes-base"));
                                let description = base_pack.map(|pack| pack.manifest.description_for_locale(&locale).to_string()).unwrap_or_else(|| crate::i18n::tr("settings-icon-themes-base-desc"));
                                draw_manager_card_title(ui, text_width, &name, &description);
                                draw_bundle_format_badge(ui, BundleFormat::Embedded, false);
                            });
                        });
                        None
                    });
                    if base_card.clicked {
                        state.selected_pack_id = DEFAULT_THEME_ID.into();
                    }

                    if let Some(swap) = to_swap {
                        wc.set.active_icon_packs.swap(swap.from, swap.to);
                        changed = true;
                    }

                    ui.add_space(10.0);
                    ui.separator();
                    ui.label(egui::RichText::new(crate::i18n::tr("icontheme-mgr-available-themes")).strong());
                    ui.add_space(6.0);

                    // Available (inactive) themes
                    let mut to_activate = None;
                    for p in &all_packs {
                        if p.manifest.id == DEFAULT_THEME_ID && !p.has_id_conflict() {
                            continue;
                        }
                        if !p.has_id_conflict() && wc.set.active_icon_packs.contains(&p.manifest.id) {
                            continue;
                        }
                        let p_key = p.selection_key();
                        let is_selected = state.selected_pack_id == p_key || (!p.has_id_conflict() && state.selected_pack_id == p.manifest.id);
                        let card = manager_theme_card(ui, ("available_theme", p_key.as_str()), is_selected, |ui| {
                            let text_width = (ui.available_width() - 168.0).max(90.0);
                            ui.horizontal(|ui| {
                                if p.has_id_conflict() {
                                    let (rect, _) = ui.allocate_exact_size(egui::vec2(36.0, 36.0), egui::Sense::hover());
                                    ui.painter().rect_filled(rect, 4.0, ui.visuals().error_fg_color.linear_multiply(0.18));
                                    ui.painter().rect_stroke(rect, 4.0, egui::Stroke::new(1.0, ui.visuals().error_fg_color), egui::StrokeKind::Inside);
                                    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, ph::WARNING, egui::FontId::proportional(22.0), ui.visuals().error_fg_color);
                                } else {
                                    draw_pack_icon(ui, p, &wc.scheme.pal, 36.0);
                                }
                                ui.vertical(|ui| {
                                    let name = p.manifest.name_for_locale(&locale);
                                    draw_manager_card_title(ui, text_width, name, name);
                                    ui.horizontal(|ui| {
                                        draw_bundle_format_badge(ui, p.format(), p.is_tampered);
                                        if p.has_id_conflict() {
                                            egui::Frame::NONE.fill(ui.visuals().error_fg_color.linear_multiply(0.20)).corner_radius(3.0).inner_margin(egui::Margin::symmetric(5, 2)).show(ui, |ui| {
                                                ui.horizontal(|ui| {
                                                    ui.spacing_mut().item_spacing.x = 3.0;
                                                    ui.label(egui::RichText::new(ph::WARNING).color(ui.visuals().error_fg_color).small());
                                                    ui.label(egui::RichText::new(crate::i18n::tr("icontheme-mgr-conflict-badge")).color(ui.visuals().error_fg_color).small().strong());
                                                });
                                            });
                                        }
                                        if p.is_directory() && wc.set.watched_icon_packs.contains(&p.manifest.id) {
                                            ui.label(egui::RichText::new(ph::EYE).small().color(ui.visuals().warn_fg_color)).on_hover_text(crate::i18n::tr("icontheme-mgr-watch-this-pack"));
                                        }
                                    });
                                });
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if p.has_id_conflict() {
                                        let label = format!("{} {}", ph::WARNING, crate::i18n::tr("icontheme-mgr-conflict-badge"));
                                        let resp = ui.label(egui::RichText::new(label).color(ui.visuals().error_fg_color).small().strong());
                                        Some(resp.rect)
                                    } else {
                                        let label = format!("{} {}", ph::PLUS, crate::i18n::tr("icontheme-mgr-activate-btn"));
                                        let action = ui
                                            .add_sized(
                                                [112.0, 36.0],
                                                egui::Button::new(egui::RichText::new(label).strong().color(ui.visuals().selection.stroke.color)).fill(ui.visuals().selection.bg_fill).truncate(),
                                            )
                                            .on_hover_text(crate::i18n::tr("icontheme-mgr-activate-btn"));
                                        if action.clicked() {
                                            to_activate = Some(p.manifest.id.clone());
                                        }
                                        Some(action.rect)
                                    }
                                })
                                .inner
                            })
                            .inner
                        });
                        if card.clicked {
                            state.selected_pack_id = p_key;
                        }
                        ui.add_space(4.0);
                    }
                    if all_packs.iter().all(|p| !p.has_id_conflict() && (p.manifest.id == DEFAULT_THEME_ID || wc.set.active_icon_packs.contains(&p.manifest.id))) {
                        ui.label(egui::RichText::new(crate::i18n::tr("settings-icon-themes-none-available")).weak());
                    }

                    if !discovered.errors.is_empty() {
                        ui.add_space(10.0);
                        ui.separator();
                        ui.label(egui::RichText::new(format!("{} {}", ph::WARNING, crate::i18n::tr("icontheme-mgr-rejected-title"))).strong().color(ui.visuals().error_fg_color));
                        ui.add_space(4.0);
                        for err in &discovered.errors {
                            let file_name = err.path.file_name().and_then(|f| f.to_str()).unwrap_or("unknown");
                            egui::Frame::NONE.fill(ui.visuals().error_fg_color.linear_multiply(0.12)).corner_radius(4.0).inner_margin(egui::Margin::symmetric(6, 4)).show(ui, |ui| {
                                ui.vertical(|ui| {
                                    ui.label(egui::RichText::new(file_name).strong());
                                    ui.label(egui::RichText::new(&err.reason).small().weak());
                                });
                            });
                            ui.add_space(3.0);
                        }
                    }

                    if let Some(idx) = to_remove {
                        let removed = wc.set.active_icon_packs.remove(idx);
                        if !wc.set.inactive_icon_packs.contains(&removed) {
                            wc.set.inactive_icon_packs.push(removed);
                        }
                        changed = true;
                    }

                    if let Some(act) = to_activate {
                        wc.set.active_icon_packs.push(act.clone());
                        wc.set.inactive_icon_packs.retain(|x| x != &act);
                        state.selected_pack_id = act;
                        changed = true;
                    }
                });
            });

            egui::CentralPanel::default().show(ui, |ui| {
                let pack_opt = all_packs.iter().find(|p| p.selection_key() == state.selected_pack_id).or_else(|| all_packs.iter().find(|p| p.manifest.id == state.selected_pack_id));
                let Some(pack) = pack_opt else {
                    ui.label(crate::i18n::tr("icontheme-mgr-no-pack-selected"));
                    return;
                };
                if let Some(conflict) = &pack.duplicate_conflict {
                    draw_duplicate_conflict_view(ui, pack, conflict, &locale);
                    return;
                }
                let pack_preview = manager_embedded_preview(ctx, pack).or_else(|| manager_archive_preview(ctx, pack)).or_else(|| manager_directory_preview(ctx, pack));
                let folder_source = pack.is_directory();

                egui::Frame::group(ui.style()).inner_margin(12).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        if let Some(preview) = pack_preview.as_ref() {
                            draw_pack_icon_bytes(ui, pack, &wc.scheme.pal, 48.0, preview.pack_icon.clone(), preview.image_generation);
                        } else {
                            draw_pack_icon(ui, pack, &wc.scheme.pal, 48.0);
                        }
                        ui.vertical(|ui| {
                            ui.add(egui::Label::new(egui::RichText::new(pack.manifest.name_for_locale(&locale)).heading().strong()).truncate());
                            let description = pack.manifest.description_for_locale(&locale);
                            if !description.is_empty() {
                                ui.add(egui::Label::new(egui::RichText::new(description).small().weak()).wrap());
                            }
                            ui.horizontal(|ui| {
                                draw_bundle_format_badge(ui, pack.format(), pack.is_tampered);
                                if let PackSource::Directory(source) = &pack.source {
                                    if wc.set.active_icon_packs.contains(&pack.manifest.id) && ui.button(format!("{} {}", ph::PACKAGE, crate::i18n::tr("settings-icon-package-btn"))).clicked() {
                                        open_packager_for_directory(ctx, pack, source);
                                    }
                                }
                                ui.label(egui::RichText::new(format!("v{}", pack.manifest.version)).small().weak());
                            });
                        });
                    });

                    ui.horizontal_wrapped(|ui| {
                        ui.label(egui::RichText::new(crate::i18n::tr1("icontheme-mgr-meta-id", "value", &pack.manifest.id)).small().monospace().weak());
                        ui.separator();
                        ui.label(egui::RichText::new(crate::i18n::tr1("icontheme-mgr-meta-license", "value", &pack.manifest.license)).small().weak());
                        if !pack.manifest.author.is_empty() {
                            ui.separator();
                            ui.label(egui::RichText::new(crate::i18n::tr1("icontheme-mgr-meta-author", "value", &pack.manifest.author)).small().weak());
                        }
                    });

                    let cov = pack_preview.as_ref().map_or_else(|| pack.coverage(), |preview| qymcad_ui_state::icons::CoverageCount { present: preview.coverage, total: ALL_ICONS.len() });
                    let pct = (cov.present * 100).checked_div(cov.total).unwrap_or(0);
                    let cov_msg = crate::i18n::trn("icontheme-mgr-total-icons", &[("count", &cov.present.to_string()), ("total", &cov.total.to_string()), ("percent", &pct.to_string())]);
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(cov_msg).small().strong());
                    });
                    ui.add(egui::ProgressBar::new(pct as f32 / 100.0).desired_width(ui.available_width()));

                    // Verified bundle / Tampered status and hygiene checks
                    if pack.is_tampered {
                        ui.add_space(2.0);
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(format!("{} {}", ph::WARNING, crate::i18n::tr("icontheme-mgr-tampered-desc"))).color(ui.visuals().error_fg_color).small().strong());
                        });
                    } else if pack.format() == BundleFormat::Package {
                        ui.add_space(2.0);
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(format!("{} {}", ph::PACKAGE, crate::i18n::tr("icontheme-mgr-package-desc"))).color(ui.visuals().selection.bg_fill).small().strong());
                        });
                    }

                    // For packs requiring runtime validation (Directory or tampered bundle), scan for SVG hygiene issues
                    if folder_source || pack.is_tampered {
                        let invalid_count = pack_preview.as_ref().map_or_else(|| ALL_ICONS.iter().filter(|id| pack.inspect_svg_for_id(**id).is_err()).count(), |preview| preview.invalid_icons);

                        ui.add_space(2.0);
                        if invalid_count > 0 {
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new(format!("{} {}", ph::WARNING, crate::i18n::trn("icontheme-mgr-hygiene-warning", &[("count", &invalid_count.to_string())])))
                                        .color(ui.visuals().warn_fg_color)
                                        .small()
                                        .strong(),
                                );
                            });
                        } else {
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(format!("{} {}", ph::CHECK_CIRCLE, crate::i18n::tr("icontheme-mgr-hygiene-clean"))).color(ui.visuals().selection.bg_fill).small());
                            });
                        }
                        if folder_source
                            && pack_preview.as_ref().is_some_and(|preview| preview.has_cleanable_icons)
                            && ui.button(format!("{} {}", ph::BROOM, crate::i18n::tr("icontheme-mgr-clean-all"))).clicked()
                        {
                            match clean_directory_icons(pack) {
                                Ok(report) => {
                                    let fixed = report.cleaned.len();
                                    let failed = report.failed.len();
                                    let text = if fixed == 0 && failed == 0 {
                                        crate::i18n::tr("icontheme-mgr-clean-none")
                                    } else {
                                        crate::i18n::trn("icontheme-mgr-clean-summary", &[("count", &fixed.to_string()), ("failed", &failed.to_string())])
                                    };
                                    let details = report.failed.iter().map(|failure| format!("{}: {}", failure.path.display(), failure.reason)).collect();
                                    state.clean_notice = Some(CleanNotice { pack_id: pack.manifest.id.clone(), text, is_error: failed > 0, details });
                                    if fixed > 0 {
                                        changed = true;
                                        ctx.request_repaint();
                                    }
                                }
                                Err(reason) => {
                                    state.clean_notice = Some(CleanNotice {
                                        pack_id: pack.manifest.id.clone(),
                                        text: crate::i18n::tr1("icontheme-mgr-clean-failed", "reason", &reason),
                                        is_error: true,
                                        details: Vec::new(),
                                    });
                                }
                            }
                        }
                    }

                    ui.add_space(4.0);
                    ui.separator();
                    if pack.is_directory() {
                        ui.horizontal(|ui| {
                            let mut is_watched = wc.set.watched_icon_packs.contains(&pack.manifest.id);
                            if ui.checkbox(&mut is_watched, crate::i18n::tr("icontheme-mgr-watch-this-pack")).changed() {
                                if is_watched {
                                    if !wc.set.watched_icon_packs.contains(&pack.manifest.id) {
                                        wc.set.watched_icon_packs.push(pack.manifest.id.clone());
                                    }
                                    qymcad_ui_state::icons::sync_watched_packs(ctx, &wc.set.watched_icon_packs);
                                    qymcad_ui_state::icons::ensure_watcher_thread(ctx, &wc.scheme.pal);
                                } else {
                                    wc.set.watched_icon_packs.retain(|id| id != &pack.manifest.id);
                                    qymcad_ui_state::icons::sync_watched_packs(ctx, &wc.set.watched_icon_packs);
                                    if !qymcad_ui_state::icons::has_watched_icon_packs(ctx) {
                                        qymcad_ui_state::icons::stop_watcher_thread(ctx);
                                    }
                                }
                                changed = true;
                            }

                            if ui.button(format!("{} {}", ph::ARROW_CLOCKWISE, crate::i18n::tr("settings-icon-reload-now"))).clicked() {
                                qymcad_ui_state::icons::bump_icon_revision(ctx);
                                apply_icon_themes(wc.set, ctx, &wc.scheme.pal);
                                ctx.request_repaint();
                                *wc.status = crate::i18n::tr("icontheme-mgr-reloaded");
                            }
                        });
                    } else if pack.format() != BundleFormat::Embedded {
                        ui.label(egui::RichText::new(crate::i18n::tr("settings-icon-watch-folder-only")).small().weak());
                    }
                });

                ui.add_space(10.0);

                ui.horizontal_wrapped(|ui| {
                    ui.selectable_value(&mut state.active_tab, IconManagerTab::Readme, crate::i18n::tr("icontheme-mgr-tab-readme"));
                    ui.selectable_value(&mut state.active_tab, IconManagerTab::Gallery, crate::i18n::tr("icontheme-mgr-tab-gallery"));
                });
                ui.separator();
                if let Some(notice) = state.clean_notice.as_ref().filter(|notice| notice.pack_id == pack.manifest.id) {
                    let color = if notice.is_error { ui.visuals().warn_fg_color } else { ui.visuals().text_color() };
                    ui.label(egui::RichText::new(&notice.text).small().color(color));
                    if !notice.details.is_empty() {
                        egui::CollapsingHeader::new(crate::i18n::tr("icontheme-mgr-clean-details")).show(ui, |ui| {
                            egui::ScrollArea::vertical().max_height(120.0).show(ui, |ui| {
                                for detail in &notice.details {
                                    ui.add(egui::Label::new(egui::RichText::new(detail).small().monospace()).wrap());
                                }
                            });
                        });
                    }
                }

                match state.active_tab {
                    IconManagerTab::Readme => {
                        egui::ScrollArea::vertical().id_salt("mgr_readme_scroll").auto_shrink([false, false]).show(ui, |ui| {
                            struct PreviewDisplay {
                                image: Option<ManagerPreviewImage>,
                                generation: u64,
                            }
                            let preview_display = if let Some(preview) = pack_preview.as_ref() {
                                PreviewDisplay { image: preview.preview_image.clone(), generation: preview.image_generation }
                            } else {
                                PreviewDisplay { image: pack.get_preview_image().map(|asset| ManagerPreviewImage { bytes: asset.data.into(), extension: asset.extension }), generation: 0 }
                            };
                            let id_key = egui::Id::new("preview_image_prev_uri").with(&pack.manifest.id);
                            if let Some(image) = preview_display.image {
                                let generation = preview_display.generation;
                                let uri =
                                    format!("bytes://preview/{}/r{}-g{generation}/{}.{}", pack.manifest.id, qymcad_ui_state::icons::get_icon_revision(ui.ctx()), image.extension, image.extension);
                                let changed = ui.data_mut(|d| {
                                    let prev = d.get_temp::<String>(id_key);
                                    if prev.as_ref() != Some(&uri) {
                                        d.insert_temp(id_key, uri.clone());
                                        Some(prev)
                                    } else {
                                        None
                                    }
                                });
                                if let Some(prev) = changed {
                                    if let Some(prev_uri) = prev {
                                        ui.ctx().forget_image(&prev_uri);
                                    }
                                    ui.ctx().include_bytes(uri.clone(), image.bytes);
                                }
                                let img = egui::Image::new(uri).max_width(ui.available_width());
                                ui.add(img);
                                ui.add_space(8.0);
                                ui.separator();
                            } else {
                                let to_forget = ui.data_mut(|d| d.remove_temp::<String>(id_key));
                                if let Some(prev) = to_forget {
                                    ui.ctx().forget_image(&prev);
                                }
                            }

                            let readme_md = pack_preview.as_ref().and_then(|preview| preview.readmes.get(&locale)).cloned().unwrap_or_else(|| pack.get_readme_for_locale(&locale));
                            crate::gui::help_window::markdown(&wc.scheme.pal, ui, &readme_md);
                        });
                    }
                    IconManagerTab::Gallery => {
                        // Filter & Search bar
                        ui.horizontal(|ui| {
                            ui.label(ph::MAGNIFYING_GLASS);
                            ui.add(egui::TextEdit::singleline(&mut state.search_query).hint_text(crate::i18n::tr("icontheme-mgr-search-icons")).desired_width((ui.available_width() - 8.0).max(120.0)));
                        });

                        ui.horizontal_wrapped(|ui| {
                            let cats = [
                                ("all", "icontheme-mgr-filter-all"),
                                ("sketch", "icontheme-mgr-filter-sketch"),
                                ("constraint", "icontheme-mgr-filter-constraint"),
                                ("part", "icontheme-mgr-filter-part"),
                                ("assembly", "icontheme-mgr-filter-assembly"),
                                ("datum", "icontheme-mgr-filter-datum"),
                            ];
                            for (val, key) in cats {
                                ui.selectable_value(&mut state.category_filter, val.to_string(), crate::i18n::tr(key));
                            }
                        });

                        ui.add_space(4.0);

                        let q = state.search_query.trim().to_lowercase();
                        let cat_filter = state.category_filter.as_str();

                        let now = ui.input(|i| i.time);
                        if let Some(notice) = &state.copied_path {
                            if now - notice.timestamp >= 2.0 {
                                state.copied_path = None;
                            } else {
                                ctx.request_repaint_after(std::time::Duration::from_millis(50));
                            }
                        }
                        let active_copied_path = state.copied_path.as_ref().map(|notice| notice.path.clone());

                        egui::ScrollArea::vertical().id_salt("mgr_gallery_scroll").auto_shrink([false, false]).show(ui, |ui| {
                            let mut shown = 0;
                            for &id in ALL_ICONS {
                                let relative_path = id.relative_path();
                                if cat_filter != "all" && !relative_path.starts_with(cat_filter) {
                                    continue;
                                }
                                if !q.is_empty() && !relative_path.to_lowercase().contains(&q) {
                                    continue;
                                }
                                shown += 1;
                                let row = if let Some(icon) = pack_preview.as_ref().and_then(|preview| preview.icons.get(&id)) {
                                    draw_gallery_icon_row(
                                        ui,
                                        &GalleryRowParams {
                                            pack,
                                            id,
                                            icon,
                                            palette: &wc.scheme.pal,
                                            cleanable: folder_source,
                                            generation: pack_preview.as_ref().map_or(0, |preview| preview.image_generation),
                                            copied_path: active_copied_path.as_deref(),
                                        },
                                    )
                                } else {
                                    let icon = pack.inspect_svg_for_id(id).map(|data| data.map(egui::load::Bytes::from));
                                    draw_gallery_icon_row(
                                        ui,
                                        &GalleryRowParams { pack, id, icon: &icon, palette: &wc.scheme.pal, cleanable: folder_source, generation: 0, copied_path: active_copied_path.as_deref() },
                                    )
                                };
                                if row.clean_clicked {
                                    ui.scroll_to_rect(row.rect, Some(egui::Align::Center));
                                    match clean_directory_icon(pack, id) {
                                        Ok(CleanIconResult::Cleaned) => {
                                            state.clean_notice =
                                                Some(CleanNotice { pack_id: pack.manifest.id.clone(), text: crate::i18n::tr("icontheme-mgr-cleaned-one"), is_error: false, details: Vec::new() });
                                            changed = true;
                                            ctx.request_repaint();
                                        }
                                        Ok(CleanIconResult::Unchanged | CleanIconResult::Missing) => {
                                            state.clean_notice =
                                                Some(CleanNotice { pack_id: pack.manifest.id.clone(), text: crate::i18n::tr("icontheme-mgr-clean-none"), is_error: false, details: Vec::new() });
                                        }
                                        Err(reason) => {
                                            state.clean_notice = Some(CleanNotice {
                                                pack_id: pack.manifest.id.clone(),
                                                text: crate::i18n::tr1("icontheme-mgr-clean-failed", "reason", &reason),
                                                is_error: true,
                                                details: Vec::new(),
                                            });
                                        }
                                    }
                                }
                                if row.path_copied {
                                    state.copied_path = Some(CopiedPathNotice { path: format!("icons/{}.svg", id.relative_path()), timestamp: now });
                                    ctx.request_repaint();
                                }
                                ui.add_space(4.0);
                            }
                            if shown == 0 {
                                ui.label(egui::RichText::new(crate::i18n::tr("icontheme-mgr-no-icons-found")).weak());
                            }
                        });
                    }
                }
            });
        });

    if changed {
        qymcad_ui_state::icons::bump_icon_revision(ctx);
        apply_icon_themes(wc.set, ctx, &wc.scheme.pal);
        ctx.request_repaint();
    }

    if initial_selected_pack_id != state.selected_pack_id {
        forget_pack_gallery_textures(ctx, &initial_selected_pack_id);
    }
    if !open {
        stop_discovery_worker(ctx);
        forget_pack_gallery_textures(ctx, &state.selected_pack_id);
    }

    state.is_open = open;
    ctx.data_mut(|d| {
        d.insert_temp(egui::Id::new("icon_manager_window"), state);
    });
}
