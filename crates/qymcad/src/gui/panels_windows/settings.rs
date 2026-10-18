//! THE SETTINGS WINDOW and the body of each of its sections.

use qymcad_ui_state::Notice;
use super::*;

/// THE STEP THE INTERFACE SCALE SNAPS TO. Five hundredths: a scale of 1.37 is of no use to anybody, and
/// with a drag speed of 0.01 per point every fifth point of the drag lands on a round number, which is what
/// makes the drag aimable at all.
pub(crate) const UI_SCALE_STEP: f32 = 0.05;

/// WHICH SECTIONS ARE SHOWN AT ALL: CAM only when its module is enabled, and while searching, only those where
/// something was found. ONE function for the window and for the tests alike: separate them, and a test starts
/// checking something other than what a person sees.
pub(crate) fn settings_sections_visible(scheme: &super::SchemeUi) -> Vec<super::settings_sections::SettingsSection> {
    use super::settings_sections::SettingsSection as Sec;
    let q = scheme.search.clone();
    let searching = !q.trim().is_empty();
    Sec::all().iter().copied().filter(|s| !searching || s.has_match(&q, &|k: &str| crate::i18n::tr(k))).collect()
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
    crate::gui::icon_themes::draw_icon_manager_window(ctx, wc);
    if !wc.win.is(WinKind::Settings) {
        return;
    }
    let mut open = wc.win.is(WinKind::Settings);
    egui::Window::new(format!("{} {}", ph::GEAR, crate::i18n::tr("win-settings"))).id(egui::Id::new("win_settings")).open(&mut open).default_width(620.0).default_height(460.0).show(ctx, |ui| {
        let mut q = std::mem::take(&mut wc.scheme.search);
        // the field's width does NOT come from the window's width - otherwise the window swells as text is typed (see the tree)
        ui.horizontal(|ui| {
            ui.label(ph::MAGNIFYING_GLASS);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add(egui::TextEdit::singleline(&mut q).id(egui::Id::new("settings_search_field")).hint_text(crate::i18n::tr("settings-search")).desired_width(f32::INFINITY));
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
                        wc.scheme.note.words.clear();
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
                        if let Err(e) = crate::system::start(bin, &args) {
                            wc.scheme.note = Notice::error(crate::i18n::tr1("settings-open-folder-failed", "error", &e.to_string()));
                        }
                    }
                }
            });
            // THE SECTION GOES INTO A PANEL OF ITS OWN, as in the parts library. A panel inside a window leaves
            // the cursor exactly on its divider, and a bare scroll after it pressed the text against the line
            // (issue #16); the central panel brings the margin that the side panel keeps on its own side.
            egui::CentralPanel::default().show(ui, |ui| {
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
                        wc.scheme.note = Notice::info(crate::i18n::tr1("settings-reset-done", "name", &crate::i18n::tr(cur.key())));
                    }
                    if !wc.scheme.note.is_empty() {
                        let note = qymcad_ui_state::notice_text(&wc.scheme.pal, &wc.scheme.note, qymcad_ui_state::NoticeSize::Small);
                        ui.label(if wc.scheme.note.tone == qymcad_ui_state::Tone::Info { note.color(wc.scheme.pal.hint()) } else { note });
                    }
                });
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
            if show("settings-import-ask") {
                ui.checkbox(&mut wc.set.import_ask_always, crate::i18n::tr("settings-import-ask")).on_hover_text(crate::i18n::tr("settings-import-ask-hint"));
            }
            if show("settings-autosave") {
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::tr("settings-autosave"));
                    ui.add(egui::DragValue::new(&mut wc.set.autosave_secs).range(0..=3600).suffix(crate::i18n::tr("unit-seconds")));
                });
                ui.label(egui::RichText::new(crate::i18n::tr("settings-autosave-hint")).weak().small());
            }
            if show("settings-kernel-threads") {
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::tr("settings-kernel-threads"));
                    // zero is not a number here but a word: "all but one". The word lives in the catalogue.
                    ui.add(egui::DragValue::new(&mut wc.set.kernel_threads).range(0..=64).custom_formatter(
                        |n, _| {
                            if n <= 0.0 {
                                crate::i18n::tr("settings-kernel-threads-auto")
                            } else {
                                format!("{n:.0}")
                            }
                        },
                    ));
                });
                ui.label(egui::RichText::new(crate::i18n::tr("settings-kernel-threads-hint")).weak().small());
            }
            if show("settings-undo-cap") {
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::tr("settings-undo-cap"));
                    ui.add(egui::DragValue::new(&mut wc.set.undo_cap).range(1..=500));
                });
                ui.label(egui::RichText::new(crate::i18n::tr("settings-undo-cap-hint")).weak().small());
            }
            // HOW OFTEN TO ASK ABOUT A NEWER VERSION. Walked over `UpdateCheck::ALL` rather than listed
            // here, so a variant added to the type appears in the window by itself (D19).
            //
            // ABSENT INSIDE FLATPAK, not merely dead: there is no network in that sandbox, the store
            // updates the package and says so itself, and a setting that cannot do anything is worse
            // than one that is not offered.
            if crate::gui::update_ui::available() && show("settings-updates") {
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::tr("settings-updates"));
                    for u in qymcad_ui_state::UpdateCheck::ALL {
                        ui.selectable_value(&mut wc.set.update_check, u, crate::i18n::tr(u.key()));
                    }
                });
                ui.label(egui::RichText::new(crate::i18n::tr("settings-updates-hint")).weak());
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
            if show("settings-icon-themes") {
                ui.separator();
                crate::gui::icon_themes::icon_theme_section(wc, ui, ctx);
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
                    qymcad_ui_state::say(ui, &wc.scheme.pal, &qymcad_ui_state::Notice::warning(crate::i18n::tr("settings-gpu-unavailable")), qymcad_ui_state::NoticeSize::Small);
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
                    egui::ComboBox::from_id_salt("mouse-nav").selected_text(crate::i18n::tr(&wc.set.mouse_nav.key())).show_ui(ui, |ui| {
                        for nav in qymcad_ui_state::MouseNav::ALL {
                            ui.selectable_value(&mut wc.set.mouse_nav, nav, crate::i18n::tr(&nav.key())).on_hover_text(crate::i18n::tr(&nav.hint_key()));
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
            // WHAT THE VIEW TURNS ABOUT. A dropping list walked over `OrbitAbout::ALL`, so a centre added to the type
            // appears here by itself; what each one does is its hint - a line under the list pushed the rows below it
            // off the visible part of the section.
            if show("settings-orbit-about") {
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::tr("settings-orbit-about"));
                    egui::ComboBox::from_id_salt("orbit-about")
                        .selected_text(crate::i18n::tr(wc.set.orbit_about.key()))
                        .show_ui(ui, |ui| {
                            for o in qymcad_ui_state::OrbitAbout::ALL {
                                ui.selectable_value(&mut wc.set.orbit_about, o, crate::i18n::tr(o.key())).on_hover_text(crate::i18n::tr(o.hint_key()));
                            }
                        })
                        .response
                        .on_hover_text(crate::i18n::tr(wc.set.orbit_about.hint_key()));
                });
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
            if show("settings-graphics-adapter") {
                super::graphics_settings::rows(ui);
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
            if show("settings-point-numbers") {
                ui.checkbox(&mut wc.set.show_point_numbers, crate::i18n::tr("settings-point-numbers")).on_hover_text(crate::i18n::tr("settings-point-numbers-hint"));
            }
            if show("settings-dim-name") {
                ui.checkbox(&mut wc.set.dim_show_name, crate::i18n::tr("settings-dim-name")).on_hover_text(crate::i18n::tr("settings-dim-name-hint"));
            }
            if show("settings-dim-formula") {
                ui.checkbox(&mut wc.set.dim_show_formula, crate::i18n::tr("settings-dim-formula")).on_hover_text(crate::i18n::tr("settings-dim-formula-hint"));
            }
            if show("settings-dim-font") {
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::tr("settings-dim-font"));
                    wc.set.dim_font = qymcad_ui_state::step_buttons(
                        ui,
                        wc.set.dim_font,
                        1.0,
                        qymcad_ui_state::DIM_FONT_RANGE,
                        &crate::i18n::tr("settings-dim-font-smaller"),
                        &crate::i18n::tr("settings-dim-font-larger"),
                    );
                });
            }
            if show("settings-dim-text") {
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::tr("settings-dim-text"));
                    ui.selectable_value(&mut wc.set.dim_text, qymcad_ui_state::DimTextTurn::AlongLine, crate::i18n::tr("settings-dim-text-along"));
                    ui.selectable_value(&mut wc.set.dim_text, qymcad_ui_state::DimTextTurn::Horizontal, crate::i18n::tr("settings-dim-text-level"));
                });
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
            // WHERE THE PANELS STAND, as a diagram of the window: the shell's own places, so this code names no
            // panel and no workbench, and a place added by anyone appears here by itself
            if show("settings-layout-place") {
                ui.label(crate::i18n::tr("settings-layout-place"));
                super::layout_diagram(&mut *wc.set, ui);
            }
            super::bar_look(&mut *wc.set, ui, &show);
            if show("settings-layout-reset") && ui.button(crate::i18n::tr("settings-layout-reset")).clicked() {
                let mut sh = crate::gui::shell(&*wc.set);
                sh.reset();
                wc.set.layout = sh.saved();
                wc.set.bar = qymcad_ui_state::BarSetting::default();
            }
        }
    }
    // THE BUTTONS OF THE BAR of the section's workbench, whichever workbench it is
    if let Some(bench) = sec.bench() {
        if show("settings-bar-buttons") {
            ui.separator();
            super::bar_buttons(&mut *wc.set, ui, bench);
        }
    }
}
