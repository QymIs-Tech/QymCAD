//! ICON GALLERY AND PACK PREVIEW INSPECTION.
//!
//! Renders icon grids, SVG status indicators, hygiene repair actions,
//! and caches pack preview snapshots.

use egui_phosphor::regular as ph;
use qymcad_ui_state::icons::{directory_has_cleanable_icons, FileSignature, IconId, IconPack, PackSource, ALL_ICONS};
use std::path::PathBuf;

pub(crate) fn draw_pack_icon(ui: &mut egui::Ui, pack: &IconPack, palette: &qymcad_scheme::Palette, size: f32) {
    draw_pack_icon_role(ui, pack, palette, size, None, 0, "sidebar");
}

pub(crate) fn draw_pack_icon_bytes(ui: &mut egui::Ui, pack: &IconPack, palette: &qymcad_scheme::Palette, size: f32, bytes: egui::load::Bytes, generation: u64) {
    draw_pack_icon_role(ui, pack, palette, size, Some(bytes), generation, "detail");
}

pub(crate) fn draw_pack_icon_role(ui: &mut egui::Ui, pack: &IconPack, palette: &qymcad_scheme::Palette, size: f32, custom_bytes: Option<egui::load::Bytes>, generation: u64, role: &str) {
    let uri =
        format!("bytes://pack-icon/{}/{}/p{:016x}_{role}-r{}-g{generation}.svg", pack.manifest.id, palette.identifier(), palette.fingerprint(), qymcad_ui_state::icons::get_icon_revision(ui.ctx()));
    let id_key = egui::Id::new("pack_icon_prev_uri").with((&pack.manifest.id, role));
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
        let resolved = if let Some(bytes) = custom_bytes { qymcad_ui_state::icons::resolve_icon_tokens(bytes.as_ref(), palette) } else { pack.get_pack_icon_svg_resolved(palette) };
        ui.ctx().include_bytes(uri.clone(), resolved);
    }
    ui.add(egui::Image::new(uri).fit_to_exact_size(egui::vec2(size, size)));
}

pub(crate) type ManagerIconPreview = Result<Option<egui::load::Bytes>, String>;

pub(crate) fn forget_pack_gallery_textures(ctx: &egui::Context, pack_id: &str) {
    let to_forget: Vec<String> = ctx.data_mut(|d| {
        let mut uris = Vec::new();
        for &id in ALL_ICONS {
            let id_key = egui::Id::new("gallery_icon_prev_uri").with((pack_id, id));
            if let Some(uri) = d.remove_temp::<String>(id_key) {
                uris.push(uri);
            }
        }
        let preview_key = egui::Id::new("preview_image_prev_uri").with(pack_id);
        if let Some(uri) = d.remove_temp::<String>(preview_key) {
            uris.push(uri);
        }
        uris
    });
    for uri in to_forget {
        ctx.forget_image(&uri);
    }
}

pub(crate) struct GalleryRowResponse {
    pub rect: egui::Rect,
    pub clean_clicked: bool,
    pub path_copied: bool,
}

pub(crate) struct GalleryRowParams<'a> {
    pub pack: &'a IconPack,
    pub id: IconId,
    pub icon: &'a ManagerIconPreview,
    pub palette: &'a qymcad_scheme::Palette,
    pub cleanable: bool,
    pub generation: u64,
    pub copied_path: Option<&'a str>,
}

pub(crate) fn draw_gallery_icon_row(ui: &mut egui::Ui, p: &GalleryRowParams<'_>) -> GalleryRowResponse {
    let relative_path = p.id.relative_path();
    let archive_path = format!("icons/{relative_path}.svg");
    let name = relative_path.rsplit('/').next().unwrap_or(relative_path);
    let row_width = ui.available_width();
    let mut clean_clicked = false;
    let mut path_copied = false;
    let is_copied = p.copied_path == Some(archive_path.as_str());
    let rect = egui::Frame::NONE
        .fill(ui.visuals().faint_bg_color)
        .corner_radius(6.0)
        .inner_margin(egui::Margin::symmetric(10, 8))
        .show(ui, |ui| {
            let inner_width = (row_width - 20.0).max(0.0);
            ui.set_width(inner_width);
            ui.horizontal(|ui| {
                let (preview, _) = ui.allocate_exact_size(egui::vec2(56.0, 56.0), egui::Sense::hover());
                ui.painter().rect_filled(preview, 4.0, ui.visuals().extreme_bg_color);
                ui.painter().rect_stroke(preview, 4.0, egui::Stroke::new(1.0, ui.visuals().weak_text_color()), egui::StrokeKind::Inside);
                match &p.icon {
                    Ok(Some(raw_bytes)) => {
                        let uri = format!(
                            "bytes://mgr/{}/{}/p{:016x}_r{}-g{}/{}.svg",
                            p.pack.manifest.id,
                            p.palette.identifier(),
                            p.palette.fingerprint(),
                            qymcad_ui_state::icons::get_icon_revision(ui.ctx()),
                            p.generation,
                            relative_path
                        );
                        let id_key = egui::Id::new("gallery_icon_prev_uri").with((&p.pack.manifest.id, p.id));
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
                            let prepared_bytes = qymcad_ui_state::icons::resolve_icon_tokens(raw_bytes.as_ref(), p.palette);
                            ui.ctx().include_bytes(uri.clone(), prepared_bytes);
                        }
                        let image = egui::Image::new(uri).fit_to_exact_size(egui::vec2(48.0, 48.0));
                        ui.put(preview.shrink(4.0), image);
                    }
                    Err(_) => {
                        ui.painter().text(preview.center(), egui::Align2::CENTER_CENTER, ph::WARNING, egui::FontId::proportional(22.0), ui.visuals().warn_fg_color);
                    }
                    Ok(None) => {}
                }

                let text_width = (inner_width - 56.0 - ui.spacing().item_spacing.x).max(0.0);
                ui.vertical(|ui| {
                    ui.set_max_width(text_width);
                    ui.label(egui::RichText::new(name).strong());

                    let path_text = egui::RichText::new(&archive_path).monospace().small();
                    let path_label = if is_copied { path_text.color(ui.visuals().hyperlink_color) } else { path_text.weak() };
                    ui.horizontal_wrapped(|ui| {
                        let resp = ui.add(egui::Label::new(path_label).sense(egui::Sense::click())).on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text(if is_copied {
                            crate::i18n::tr("icontheme-mgr-path-copied")
                        } else {
                            crate::i18n::tr("icontheme-mgr-copy-path")
                        });

                        if resp.clicked() {
                            ui.output_mut(|output| output.commands.push(egui::OutputCommand::CopyText(archive_path.clone())));
                            path_copied = true;
                        }

                        if is_copied {
                            egui::Frame::NONE
                                .fill(ui.visuals().window_fill())
                                .stroke(egui::Stroke::new(1.0, ui.visuals().hyperlink_color))
                                .corner_radius(4.0)
                                .inner_margin(egui::Margin::symmetric(6, 2))
                                .show(ui, |ui| {
                                    ui.label(egui::RichText::new(format!("{} {}", ph::CHECK, crate::i18n::tr("icontheme-mgr-path-copied"))).small().color(ui.visuals().hyperlink_color));
                                });
                        }
                    });
                    match &p.icon {
                        Ok(Some(_)) => {
                            ui.label(egui::RichText::new(crate::i18n::tr("icontheme-mgr-gallery-present")).small().weak());
                        }
                        Ok(None) => {
                            ui.label(egui::RichText::new(crate::i18n::tr("icontheme-mgr-gallery-missing")).small().weak());
                        }
                        Err(reason) => {
                            let error = format!("{}: {reason}", crate::i18n::tr("icontheme-mgr-gallery-invalid"));
                            ui.add(egui::Label::new(egui::RichText::new(error).small().color(ui.visuals().warn_fg_color)).wrap());
                            if p.cleanable && ui.button(format!("{} {}", ph::BROOM, crate::i18n::tr("icontheme-mgr-clean-icon"))).clicked() {
                                clean_clicked = true;
                            }
                        }
                    }
                });
            });
        })
        .response
        .rect;
    GalleryRowResponse { rect, clean_clicked, path_copied }
}

#[derive(Clone)]
pub(crate) struct ManagerArchiveCache {
    pub path: PathBuf,
    pub file_size: u64,
    pub modified: Option<std::time::SystemTime>,
    pub revision: u64,
    pub preview: std::sync::Arc<ManagerPackPreview>,
}

#[derive(Clone)]
pub(crate) struct ManagerDirectoryCache {
    pub path: PathBuf,
    pub snapshot: std::sync::Arc<std::collections::HashMap<PathBuf, FileSignature>>,
    pub revision: u64,
    pub preview: std::sync::Arc<ManagerPackPreview>,
}

#[derive(Clone)]
pub(crate) struct ManagerPreviewImage {
    pub bytes: egui::load::Bytes,
    pub extension: &'static str,
}

pub(crate) struct ManagerPackPreview {
    pub coverage: usize,
    pub invalid_icons: usize,
    pub has_cleanable_icons: bool,
    pub image_generation: u64,
    pub readmes: std::collections::HashMap<String, String>,
    pub preview_image: Option<ManagerPreviewImage>,
    pub pack_icon: egui::load::Bytes,
    pub icons: std::collections::HashMap<IconId, ManagerIconPreview>,
}

pub(crate) fn manager_embedded_preview(ctx: &egui::Context, pack: &IconPack) -> Option<std::sync::Arc<ManagerPackPreview>> {
    let PackSource::Embedded(_) = &pack.source else {
        return None;
    };
    let cache_id = egui::Id::new("icon_manager_embedded_cache").with(&pack.manifest.id);
    if let Some(cached) = ctx.data(|data| data.get_temp::<std::sync::Arc<ManagerPackPreview>>(cache_id)) {
        return Some(cached);
    }
    let mut coverage = 0;
    let mut invalid_icons = 0;
    let mut icons = std::collections::HashMap::new();
    for &id in ALL_ICONS {
        let available = pack.get_svg_for_id(id).is_some();
        if available {
            coverage += 1;
        }
        let inspected = pack.inspect_svg_for_id(id).map(|data| data.map(egui::load::Bytes::from));
        if inspected.is_err() {
            invalid_icons += 1;
        }
        icons.insert(id, inspected);
    }
    let preview = std::sync::Arc::new(ManagerPackPreview {
        coverage,
        invalid_icons,
        has_cleanable_icons: false,
        image_generation: 0,
        readmes: crate::i18n::available().into_iter().map(|(locale, _)| (locale.clone(), pack.get_readme_for_locale(&locale))).collect(),
        preview_image: pack.get_preview_image().map(|asset| ManagerPreviewImage { bytes: asset.data.into(), extension: asset.extension }),
        pack_icon: pack.get_pack_icon_svg().into(),
        icons,
    });
    ctx.data_mut(|data| {
        data.insert_temp(cache_id, preview.clone());
    });
    Some(preview)
}

pub(crate) fn manager_archive_preview(ctx: &egui::Context, pack: &IconPack) -> Option<std::sync::Arc<ManagerPackPreview>> {
    let PackSource::Archive(path) = &pack.source else {
        return None;
    };
    let metadata = std::fs::metadata(path).ok()?;
    let modified = metadata.modified().ok();
    let revision = qymcad_ui_state::icons::get_icon_revision(ctx);
    let cache_id = egui::Id::new("icon_manager_archive_cache").with(&pack.manifest.id);
    if let Some(cached) = ctx.data(|data| data.get_temp::<ManagerArchiveCache>(cache_id)) {
        if cached.path == *path && cached.file_size == metadata.len() && cached.modified == modified && cached.revision == revision {
            return Some(cached.preview);
        }
    }
    let snapshot = pack.archive_snapshot().ok()?;
    let mut coverage = 0;
    let mut invalid_icons = 0;
    let mut icons = std::collections::HashMap::new();
    for &id in ALL_ICONS {
        let available = snapshot.get_svg_for_id(id).is_some();
        if available {
            coverage += 1;
        }
        let inspected = snapshot.inspect_svg_for_id(id).map(|data| data.map(egui::load::Bytes::from));
        if inspected.is_err() {
            invalid_icons += 1;
        }
        icons.insert(id, inspected);
    }
    let image_generation = ctx.data(|data| data.get_temp::<ManagerArchiveCache>(cache_id).map_or(1, |cache| cache.preview.image_generation.wrapping_add(1)));
    let preview = std::sync::Arc::new(ManagerPackPreview {
        coverage,
        invalid_icons,
        has_cleanable_icons: false,
        image_generation,
        readmes: crate::i18n::available().into_iter().map(|(locale, _)| (locale.clone(), snapshot.get_readme_for_locale(&locale))).collect(),
        preview_image: snapshot.get_preview_image().map(|asset| ManagerPreviewImage { bytes: asset.data.into(), extension: asset.extension }),
        pack_icon: snapshot.get_pack_icon_svg().into(),
        icons,
    });
    ctx.data_mut(|data| {
        data.insert_temp(cache_id, ManagerArchiveCache { path: path.clone(), file_size: metadata.len(), modified, revision, preview: preview.clone() });
    });
    Some(preview)
}

pub(crate) fn manager_directory_preview(ctx: &egui::Context, pack: &IconPack) -> Option<std::sync::Arc<ManagerPackPreview>> {
    let PackSource::Directory(path) = &pack.source else {
        return None;
    };
    let mut snapshot = pack.directory_snapshot()?;
    for name in ["README.md", "readme.md", "README.txt", "description.md", "preview.svg", "preview.png", "preview.webp"] {
        let file = path.join(name);
        if let Ok(metadata) = std::fs::metadata(file) {
            snapshot.insert(PathBuf::from(name), FileSignature { modified: metadata.modified().unwrap_or(std::time::UNIX_EPOCH), len: metadata.len() });
        }
    }
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with("README.") && name.ends_with(".md") {
                if let Ok(metadata) = entry.metadata() {
                    snapshot.insert(PathBuf::from(name.as_ref()), FileSignature { modified: metadata.modified().unwrap_or(std::time::UNIX_EPOCH), len: metadata.len() });
                }
            }
        }
    }
    let revision = qymcad_ui_state::icons::get_icon_revision(ctx);
    let cache_id = egui::Id::new("icon_manager_directory_cache").with(&pack.manifest.id);
    let cached = ctx.data(|data| data.get_temp::<ManagerDirectoryCache>(cache_id));
    if let Some(cache) = &cached {
        if cache.path == *path && *cache.snapshot == snapshot && cache.revision == revision {
            return Some(cache.preview.clone());
        }
    }

    let mut coverage = 0;
    let mut invalid_icons = 0;
    let mut icons = std::collections::HashMap::new();
    for &id in ALL_ICONS {
        let icon_path = PathBuf::from(format!("icons/{}.svg", id.relative_path()));
        if snapshot.get(&icon_path).is_some_and(|sig| sig.len > 0) {
            coverage += 1;
        }
        let inspected = pack.inspect_svg_for_id(id).map(|data| data.map(egui::load::Bytes::from));
        if inspected.is_err() {
            invalid_icons += 1;
        }
        icons.insert(id, inspected);
    }
    let image_generation = cached.map_or(1, |cache| cache.preview.image_generation.wrapping_add(1));
    let has_cleanable_icons = directory_has_cleanable_icons(pack).unwrap_or(false);
    let preview = std::sync::Arc::new(ManagerPackPreview {
        coverage,
        invalid_icons,
        has_cleanable_icons,
        image_generation,
        readmes: crate::i18n::available().into_iter().map(|(locale, _)| (locale.clone(), pack.get_readme_for_locale(&locale))).collect(),
        preview_image: pack.get_preview_image().map(|asset| ManagerPreviewImage { bytes: asset.data.into(), extension: asset.extension }),
        pack_icon: pack.get_pack_icon_svg().into(),
        icons,
    });
    ctx.data_mut(|data| {
        data.insert_temp(cache_id, ManagerDirectoryCache { path: path.clone(), snapshot: std::sync::Arc::new(snapshot), revision, preview: preview.clone() });
    });
    Some(preview)
}
