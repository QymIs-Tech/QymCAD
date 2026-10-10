//! DEVELOPER THEME PACKAGER DIALOG.
//!
//! Modal dialog for inspecting icon packs and packaging directory themes
//! into compressed `.qicons` bundles.

use egui::Color32;
use egui_phosphor::regular as ph;
use qymcad_ui_state::icons::{inspect_pack_directory, package_bundle, IconManifest, IconPack, PackageType, ValidationReport};
use std::path::{Path, PathBuf};

use super::discovery::invalidate_theme_discovery_cache;

/// Developer Packager modal state kept in UI context.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PackagerDialogState {
    pub is_open: bool,
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub license: String,
    pub description: String,
    pub source_dir: String,
    pub output_file: String,
    pub message: Option<String>,
    pub is_error: bool,
    pub report: Option<ValidationReport>,
}

impl Default for PackagerDialogState {
    fn default() -> Self {
        Self {
            is_open: false,
            id: "my-cad-theme".into(),
            name: "My CAD Theme".into(),
            version: "1.0.0".into(),
            author: "".into(),
            license: "LGPL-2.1-or-later".into(),
            description: "Custom CAD vector icons".into(),
            source_dir: "".into(),
            output_file: "".into(),
            message: None,
            is_error: false,
            report: None,
        }
    }
}

pub(crate) fn open_packager_for_directory(ctx: &egui::Context, pack: &IconPack, source: &Path) {
    ctx.data_mut(|data| {
        let state = data.get_temp_mut_or_default::<PackagerDialogState>(egui::Id::new("icon_packager_dialog"));
        state.is_open = true;
        state.id = pack.manifest.id.clone();
        state.name = pack.manifest.name.clone();
        state.version = pack.manifest.version.clone();
        state.author = pack.manifest.author.clone();
        state.license = pack.manifest.license.clone();
        state.description = pack.manifest.description.clone();
        state.source_dir = source.display().to_string();
        state.output_file = source.with_extension("qicons").display().to_string();
        state.message = None;
        state.report = None;
        state.is_error = false;
    });
}

/// In-app developer modal dialog for packaging an icon bundle into `.qicons`.
pub(crate) fn draw_packager_modal(ctx: &egui::Context, state: &mut PackagerDialogState) {
    let mut open = state.is_open;
    egui::Window::new(crate::i18n::tr("icontheme-packager-title")).id(egui::Id::new("icon_packager_dialog")).open(&mut open).collapsible(false).resizable(true).default_width(450.0).show(ctx, |ui| {
        ui.label(egui::RichText::new(crate::i18n::tr("icontheme-packager-desc")).small().weak());
        ui.add_space(4.0);

        egui::Grid::new("packager_grid").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
            ui.label(crate::i18n::tr("icontheme-packager-field-id"));
            ui.text_edit_singleline(&mut state.id);
            ui.end_row();

            ui.label(crate::i18n::tr("icontheme-packager-field-name"));
            ui.text_edit_singleline(&mut state.name);
            ui.end_row();

            ui.label(crate::i18n::tr("icontheme-packager-field-version"));
            ui.text_edit_singleline(&mut state.version);
            ui.end_row();

            ui.label(crate::i18n::tr("icontheme-packager-field-author"));
            ui.text_edit_singleline(&mut state.author);
            ui.end_row();

            ui.label(crate::i18n::tr("icontheme-packager-field-license"));
            ui.text_edit_singleline(&mut state.license);
            ui.end_row();

            ui.label(crate::i18n::tr("icontheme-packager-field-desc"));
            ui.text_edit_singleline(&mut state.description);
            ui.end_row();

            ui.label(crate::i18n::tr("icontheme-packager-field-source"));
            ui.text_edit_singleline(&mut state.source_dir);
            ui.end_row();

            ui.label(crate::i18n::tr("icontheme-packager-field-output"));
            ui.text_edit_singleline(&mut state.output_file);
            ui.end_row();
        });

        ui.add_space(8.0);

        if let Some(msg) = &state.message {
            let color = if state.is_error { Color32::RED } else { Color32::GREEN };
            ui.label(egui::RichText::new(msg).color(color));
            ui.add_space(4.0);
        }

        ui.horizontal(|ui| {
            if ui.button(crate::i18n::tr("icontheme-packager-inspect-btn")).clicked() {
                let source_path = PathBuf::from(state.source_dir.trim());
                if !source_path.exists() {
                    let path_str = source_path.display().to_string();
                    state.message = Some(crate::i18n::tr1("icontheme-packager-source-not-found", "path", &path_str));
                    state.is_error = true;
                    state.report = None;
                } else {
                    match inspect_pack_directory(&source_path) {
                        Ok(rep) => {
                            if rep.has_issues() {
                                let total_str = (rep.rejected.len() + rep.extraneous.len()).to_string();
                                let rej_str = rep.rejected.len().to_string();
                                let ext_str = rep.extraneous.len().to_string();
                                state.message = Some(crate::i18n::trn("icontheme-packager-validation-issues", &[("total", &total_str), ("rejected", &rej_str), ("extraneous", &ext_str)]));
                                state.is_error = !rep.rejected.is_empty();
                            } else {
                                state.message = Some(crate::i18n::tr("icontheme-packager-all-valid"));
                                state.is_error = false;
                            }
                            state.report = Some(rep);
                        }
                        Err(e) => {
                            let err_str = e.to_string();
                            state.message = Some(crate::i18n::tr1("icontheme-packager-inspection-failed", "error", &err_str));
                            state.is_error = true;
                            state.report = None;
                        }
                    }
                }
            }

            if ui.button(crate::i18n::tr("icontheme-packager-build-btn")).clicked() {
                let source_path = PathBuf::from(state.source_dir.trim());
                let output_path = PathBuf::from(state.output_file.trim());

                if !source_path.exists() {
                    let path_str = source_path.display().to_string();
                    state.message = Some(crate::i18n::tr1("icontheme-packager-source-not-found", "path", &path_str));
                    state.is_error = true;
                } else if state.output_file.trim().is_empty() {
                    state.message = Some(crate::i18n::tr("icontheme-packager-output-empty"));
                    state.is_error = true;
                } else {
                    let result = (|| {
                        let translations = if source_path.join("manifest.ron").exists() { IconPack::from_directory(&source_path)?.manifest.translations } else { Default::default() };
                        let manifest = IconManifest {
                            package_type: PackageType::IconTheme,
                            id: state.id.trim().to_string(),
                            name: state.name.trim().to_string(),
                            version: state.version.trim().to_string(),
                            author: state.author.trim().to_string(),
                            license: state.license.trim().to_string(),
                            description: state.description.trim().to_string(),
                            translations,
                            verified: true,
                        };
                        manifest.validate()?;
                        package_bundle(&source_path, &manifest, &output_path)
                    })();

                    match result {
                        Ok(rep) => {
                            if rep.included.is_empty() {
                                state.message = Some(crate::i18n::tr("icontheme-packager-no-icons"));
                                state.is_error = true;
                            } else {
                                invalidate_theme_discovery_cache();
                                let cov_str = rep.included.len().to_string();
                                let path_str = output_path.display().to_string();
                                state.message = Some(crate::i18n::trn("icontheme-packager-success", &[("count", &cov_str), ("path", &path_str)]));
                                state.is_error = false;
                            }
                            state.report = Some(rep);
                        }
                        Err(e) => {
                            let err_str = e.to_string();
                            state.message = Some(crate::i18n::tr1("icontheme-packager-packaging-failed", "error", &err_str));
                            state.is_error = true;
                        }
                    }
                }
            }

            if ui.button(crate::i18n::tr("nav-cancel")).clicked() {
                state.is_open = false;
            }
        });

        // Diagnostic validation report section
        if let Some(rep) = &state.report {
            ui.separator();
            let cov = rep.coverage();
            let pct = rep.coverage_percent();
            let cov_str = cov.present.to_string();
            let total_str = cov.total.to_string();
            let pct_str = pct.to_string();

            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(crate::i18n::trn("icontheme-packager-coverage", &[("included", &cov_str), ("total", &total_str), ("percent", &pct_str)])).strong());
            });

            // Category badges
            ui.horizontal_wrapped(|ui| {
                for cat in rep.category_breakdown() {
                    let text = format!("{}: {}/{}", cat.category, cat.present, cat.total);
                    ui.label(egui::RichText::new(text).small().weak());
                }
            });

            egui::ScrollArea::vertical().max_height(240.0).show(ui, |ui| {
                // Rejected files (critical errors)
                if !rep.rejected.is_empty() {
                    ui.add_space(4.0);
                    let title = crate::i18n::tr1("icontheme-packager-rejected-title", "count", &rep.rejected.len().to_string());
                    ui.label(egui::RichText::new(title).strong().color(Color32::RED));
                    ui.label(egui::RichText::new(crate::i18n::tr("icontheme-packager-rejected-desc")).small().weak());
                    for rej in &rep.rejected {
                        ui.label(egui::RichText::new(format!("  - {}: {}", rej.path, rej.reason)).color(ui.visuals().error_fg_color).small());
                    }
                }

                // Extraneous files (warnings)
                if !rep.extraneous.is_empty() {
                    ui.add_space(4.0);
                    let title = crate::i18n::tr1("icontheme-packager-extraneous-title", "count", &rep.extraneous.len().to_string());
                    ui.label(egui::RichText::new(title).strong().color(ui.visuals().warn_fg_color));
                    ui.label(egui::RichText::new(crate::i18n::tr("icontheme-packager-extraneous-desc")).small().weak());
                    for file in &rep.extraneous {
                        ui.label(egui::RichText::new(format!("  - {file}")).color(ui.visuals().warn_fg_color).small());
                    }
                }

                // Included icons list (collapsible)
                if !rep.included.is_empty() {
                    ui.add_space(4.0);
                    let inc_title = crate::i18n::tr1("icontheme-packager-included-title", "count", &cov_str);
                    egui::CollapsingHeader::new(egui::RichText::new(inc_title).color(Color32::GREEN)).default_open(rep.rejected.is_empty() && rep.extraneous.is_empty()).show(ui, |ui| {
                        for id in &rep.included {
                            ui.label(egui::RichText::new(format!("  {} {}", ph::CHECK, id.relative_path())).small());
                        }
                    });
                }

                // Missing icons list (collapsible)
                if !rep.missing.is_empty() {
                    ui.add_space(4.0);
                    let miss_title = crate::i18n::tr1("icontheme-packager-missing-title", "count", &rep.missing.len().to_string());
                    egui::CollapsingHeader::new(egui::RichText::new(miss_title).weak()).default_open(false).show(ui, |ui| {
                        ui.label(egui::RichText::new(crate::i18n::tr("icontheme-packager-missing-desc")).small().weak());
                        for id in &rep.missing {
                            ui.label(egui::RichText::new(format!("  - {}", id.relative_path())).weak().small());
                        }
                    });
                }
            });
        }
    });
    state.is_open = open && state.is_open;
}
