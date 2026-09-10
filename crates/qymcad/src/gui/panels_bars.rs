//! THE BARS ACROSS THE TOP: the menu, the tool bars, the options of the active tool, the command bars.
//!
//! They used to sit inside the frame's body, where "what the frame does" and "what it draws" could not be told
//! apart and editing one command touched the whole life cycle of the frame.

pub(crate) use qymcad_part::*;
use super::*;

impl App {
    /// The command bar of the Part workbench, through its own doorway.
    pub(crate) fn feat_command_bar(&mut self, ui: &mut egui::Ui) {
        crate::gui::panels_bars::feat_command_bar(&mut self.part_ctx(), ui);
    }








}

pub(crate) fn menu_bar(bc: &mut qymcad_ui_state::BarCtx, ui: &mut egui::Ui) {
    // The panel lives inside a `Ui` now; the context is still wanted for windows,
    // input and viewport commands, and it comes from the same place.
    let ctx = &ui.ctx().clone();
    // The menu items belonging to CAM (the machine, the tools, the G-code, the setup, the rapids) appear only
    // when the machining module is enabled. That module is under development and hidden by default.
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
    egui::MenuBar::new().ui(ui, |ui| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
        ui.menu_button(qymcad_i18n::tr("menu-file"), |ui| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
            if ui.button(format!("{}  {}", ph::FILE, qymcad_i18n::tr("file-new"))).clicked() {
                bc.ask.push(qymcad_ui_state::BarAsk::Nav(qymcad_ui_state::Nav::New));
                ui.close();
            }
            // TEMPLATES: the item is disabled rather than hidden, as the recent files are. An empty
            // submenu explains itself, while a vanishing item leaves one guessing whether it ever existed.
            let tpls = crate::templates::list();
            ui.add_enabled_ui(!tpls.is_empty(), |ui| {
                ui.menu_button(format!("{}  {}", ph::FILE_TEXT, qymcad_i18n::tr("file-new-from-template")), |ui| {
                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                    for (name, path) in &tpls {
                        if ui.button(name).on_hover_text(path).clicked() {
                            bc.ask.push(qymcad_ui_state::BarAsk::Nav(qymcad_ui_state::Nav::NewFromTemplate(path.clone())));
                            ui.close();
                        }
                    }
                });
            });
            if ui.button(format!("{}  {}", ph::PACKAGE, qymcad_i18n::tr("file-save-as-template"))).on_hover_text(qymcad_i18n::tr("file-save-as-template-hint")).clicked() {
                bc.win.tpl_name = bc.project.meta.title.clone();
                bc.win.open(WinKind::SaveTemplate);
                ui.close();
            }
            ui.separator();
            if ui.button(format!("{}  {}", ph::FOLDER_OPEN, qymcad_i18n::tr("file-open"))).clicked() {
                bc.ask.push(qymcad_ui_state::BarAsk::Nav(qymcad_ui_state::Nav::OpenDialog));
                ui.close();
            }
            // RECENT FILES: a basic expectation of any program that has files. The item is disabled
            // rather than hidden: an empty submenu explains itself, while a vanishing item leaves one
            // guessing whether it ever existed.
            let recent = bc.set.recent.clone();
            ui.add_enabled_ui(!recent.is_empty(), |ui| {
                ui.menu_button(format!("{}  {}", ph::CLOCK_COUNTER_CLOCKWISE, qymcad_i18n::tr("file-recent")), |ui| {
                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                    for path in &recent {
                        // the row shows THE FILE NAME with the full path in the tooltip: paths are longer than the menu
                        let name = std::path::Path::new(path).file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| path.clone());
                        if ui.button(name).on_hover_text(path).clicked() {
                            bc.ask.push(qymcad_ui_state::BarAsk::Nav(qymcad_ui_state::Nav::OpenPath(path.clone())));
                            ui.close();
                        }
                    }
                    ui.separator();
                    if ui.button(format!("{}  {}", ph::TRASH, qymcad_i18n::tr("file-recent-clear"))).clicked() {
                        bc.set.recent.clear();
                        ui.close();
                    }
                });
            });
            if ui.add(egui::Button::new(format!("{}  {}", ph::FLOPPY_DISK, qymcad_i18n::tr("file-save"))).shortcut_text("Ctrl+S")).clicked() {
                bc.ask.push(qymcad_ui_state::BarAsk::Save);
                ui.close();
            }
            if ui.button(format!("{}  {}", ph::FILE_TEXT, qymcad_i18n::tr("file-doc-props"))).clicked() {
                bc.win.open(WinKind::DocProps);
                ui.close();
            }
            if ui.add(egui::Button::new(format!("{}  {}", ph::FLOPPY_DISK, qymcad_i18n::tr("file-save-as"))).shortcut_text("Ctrl+Shift+S")).clicked() {
                bc.ask.push(qymcad_ui_state::BarAsk::SaveAs);
                ui.close();
            }
            ui.separator();
            if ui.button(format!("{}  {}", ph::FILE, qymcad_i18n::tr("file-import-dxf"))).clicked() {
                bc.ask.push(qymcad_ui_state::BarAsk::PickDxf);
                ui.close();
            }
            if ui.button(format!("{}  {}", ph::CUBE, qymcad_i18n::tr("file-import-stl"))).clicked() {
                bc.ask.push(qymcad_ui_state::BarAsk::PickStl);
                ui.close();
            }
            if ui.button(format!("{}  {}", ph::CUBE, qymcad_i18n::tr("file-import-step"))).clicked() {
                bc.ask.push(qymcad_ui_state::BarAsk::PickStep);
                ui.close();
            }
            if ui.button(format!("{}  {}", ph::POLYGON, qymcad_i18n::tr("file-import-svg"))).clicked() {
                bc.ask.push(qymcad_ui_state::BarAsk::PickSvg);
                ui.close();
            }
            ui.separator();
            if ui.button(format!("{}  {}", ph::EXPORT, qymcad_i18n::tr("file-export-step"))).on_hover_text(qymcad_i18n::tr("menu-export-step-hint")).clicked() {
                bc.ask.push(qymcad_ui_state::BarAsk::ExportStep(qymcad_ui_state::ExportTarget::Project));
                ui.close();
            }
            if ui.button(format!("{}  {}", ph::EXPORT, qymcad_i18n::tr("file-export-stl"))).on_hover_text(qymcad_i18n::tr("menu-export-stl-hint")).clicked() {
                *bc.stl_export = Some(qymcad_ui_state::ExportTarget::Project);
                ui.close();
            }
            ui.separator();
            if ui.button(format!("{}  {}", ph::SIGN_OUT, qymcad_i18n::tr("file-quit"))).clicked() {
                bc.ask.push(qymcad_ui_state::BarAsk::Nav(qymcad_ui_state::Nav::Exit));
            }
        });
        ui.menu_button(qymcad_i18n::tr("menu-edit"), |ui| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
            // THE OPERATION'S NAME IN THE MENU: what exactly will be undone is visible - a step knows its
            // own name, because it was created by a command rather than by the frame.
            let undo_label = match bc.edits.undo.last() {
                Some(s) => format!("{}  {}", ph::ARROW_COUNTER_CLOCKWISE, qymcad_i18n::tr1("menu-undo-named", "what", &s.name)),
                None => format!("{}  {}", ph::ARROW_COUNTER_CLOCKWISE, qymcad_i18n::tr("menu-undo")),
            };
            if ui.add_enabled(!bc.edits.undo.is_empty(), egui::Button::new(undo_label).shortcut_text("Ctrl+Z")).clicked() {
                bc.ask.push(qymcad_ui_state::BarAsk::Undo);
                ui.close();
            }
            let redo_label = match bc.edits.redo.last() {
                Some(s) => format!("{}  {}", ph::ARROW_CLOCKWISE, qymcad_i18n::tr1("menu-redo-named", "what", &s.name)),
                None => format!("{}  {}", ph::ARROW_CLOCKWISE, qymcad_i18n::tr("menu-redo")),
            };
            if ui.add_enabled(!bc.edits.redo.is_empty(), egui::Button::new(redo_label).shortcut_text("Ctrl+Shift+Z")).clicked() {
                bc.ask.push(qymcad_ui_state::BarAsk::Redo);
                ui.close();
            }
            ui.separator();
            // The clipboard: sketches, parts and subassemblies in the tree, or geometry in the sketch editor.
            let can_copy = qymcad_ui_state::clipboard_can_copy(&*bc.project, *bc.sel, &*bc.sel_sk, *bc.sketch_ses);
            let can_paste = bc.clip.tree.is_some() || bc.clip.geom.is_some();
            if ui.add_enabled(can_copy, egui::Button::new(format!("{}  {}", ph::COPY, qymcad_i18n::tr("menu-copy"))).shortcut_text("Ctrl+C")).clicked() {
                bc.ask.push(qymcad_ui_state::BarAsk::Clipboard { cut: false });
                ui.close();
            }
            if ui.add_enabled(can_copy, egui::Button::new(format!("{}  {}", ph::SCISSORS, qymcad_i18n::tr("menu-cut"))).shortcut_text("Ctrl+X")).clicked() {
                bc.ask.push(qymcad_ui_state::BarAsk::Clipboard { cut: true });
                ui.close();
            }
            if ui.add_enabled(can_paste, egui::Button::new(format!("{}  {}", ph::CLIPBOARD, qymcad_i18n::tr("win-insert"))).shortcut_text("Ctrl+V")).clicked() {
                bc.ask.push(qymcad_ui_state::BarAsk::Paste);
                ui.close();
            }
            ui.separator();
            // REBUILD EVERYTHING. The file stores finished meshes and computes nothing anew on opening -
            // that is fast, but it means a part built by an older version of the kernel stays as it was.
            // Reported behaviour: a thread profile was fixed, the CAD restarted, and the same ragged part
            // appeared - nobody had recomputed its mesh. Without this command, fixing that meant poking
            // every feature by hand.
            if ui
                .add_enabled(!bc.project.timeline.is_empty(), egui::Button::new(format!("{}  {}", ph::ARROWS_CLOCKWISE, qymcad_i18n::tr("menu-rebuild"))))
                .on_hover_text(qymcad_i18n::tr("menu-rebuild-hint"))
                .clicked()
            {
                bc.ask.push(qymcad_ui_state::BarAsk::RebuildEverything);
                ui.close();
            }
        });
        ui.menu_button(qymcad_i18n::tr("menu-view"), |ui| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
            ui.checkbox(&mut *bc.mode_3d, format!("{}  {}", ph::CUBE, qymcad_i18n::tr("menu-orbit3d")));
            if ui.button(format!("{}  {}", ph::CORNERS_OUT, qymcad_i18n::tr("menu-fit-view"))).clicked() {
                bc.view.initialized = false;
                bc.cam.init = false;
                ui.close();
            }
            ui.separator();
            ui.label(qymcad_i18n::tr("settings-scheme"));
            // THE SCHEMES COME FROM THE LIVE LIST (the built-in ones and the user's), and the label from
            // `title()`: a built-in scheme has NO name of its own, it comes from the language catalogue.
            // This used to be `p.name`, and after the identifier and the label were separated the menu
            // items were left as bare icons with no words.
            let rows: Vec<(String, String, bool)> = bc.scheme.all.iter().map(|p| (p.id.clone(), p.title(), p.light)).collect();
            for (id, title, light) in rows {
                let icon = if qymcad_scheme::store::is_builtin(&id) {
                    if light { ph::SUN } else { ph::MOON }
                } else {
                    ph::PENCIL_SIMPLE
                };
                if ui.button(format!("{icon}  {title}")).clicked() {
                    bc.set.scheme = id.clone();
                    qymcad_ui_state::apply_theme(&mut *bc.scheme, &*bc.set, ctx);
                    ui.close();
                }
            }
        });
        ui.menu_button(qymcad_i18n::tr("menu-windows"), |ui| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
            if ui.button(format!("{}  {}", ph::GEAR, qymcad_i18n::tr("win-settings"))).clicked() {
                bc.win.toggle(WinKind::Settings);
                ui.close();
            }
            if ui.button(format!("{}  {}", ph::PACKAGE, qymcad_i18n::tr("win-parts-library"))).clicked() {
                bc.ask.push(qymcad_ui_state::BarAsk::ToggleLibrary);
                ui.close();
            }
            if ui.button(format!("{}  {}", ph::HOUSE, qymcad_i18n::tr("win-start"))).clicked() {
                bc.win.start_asked = true; // it was ASKED for rather than raising itself - see `start_screen_visible`
                ui.close();
            }
        });
        ui.menu_button(qymcad_i18n::tr("menu-help"), |ui| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
            if ui.button(format!("{} {}", ph::BOOK_OPEN, qymcad_i18n::tr("help-title"))).clicked() {
                bc.ask.push(qymcad_ui_state::BarAsk::Help("index".to_string()));
                ui.close();
            }
            ui.separator();
            if ui.button(format!("{} {}", ph::KEYBOARD, qymcad_i18n::tr("help-hotkeys"))).clicked() {
                bc.win.open(WinKind::Hotkeys);
                ui.close();
            }
            // CHECK FOR UPDATES. Pressed by hand it asks ALWAYS - even with the automatic check switched
            // off, because pressing it IS the asking. Absent where it cannot work: inside Flatpak there
            // is no network, and a build with no release tag has nothing to compare against.
            if crate::gui::update_ui::available() && ui.button(format!("{} {}", ph::ARROW_CLOCKWISE, qymcad_i18n::tr("help-check-updates"))).clicked() {
                crate::gui::update_ui::ask(bc.set);
                bc.win.open(WinKind::Updates);
                ui.close();
            }
            if ui.button(format!("{} {}", ph::BUG, qymcad_i18n::tr("help-report"))).clicked() {
                bc.win.open(WinKind::Report);
                ui.close();
            }
            if ui.button(format!("{} {}", ph::INFO, qymcad_i18n::tr("help-about"))).clicked() {
                bc.win.open(WinKind::About);
                ui.close();
            }
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
            if let Some(p) = &*bc.dxf_path {
                let fname = std::path::Path::new(p).file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                ui.label(egui::RichText::new(format!("{} {fname}", ph::FILE)).weak());
            }
        });
    });
}

/// WHAT THE CHECK FOR A NEWER VERSION HAS TO SAY, in the status line.
///
/// Silent while nothing has been asked and while nothing was found: a line that says "no updates" for
/// ever is a line people stop reading, and the one time it matters they would not read it either.
///
/// A FAILED CHECK IS SILENT TOO, unless a person asked for it themselves. Somebody who pressed the menu
/// item is owed the answer, even the disappointing one; somebody who did not press anything has no
/// business being told that a request they never made did not go through.
fn update_note(scheme: &qymcad_ui_state::SchemeUi, win: &mut qymcad_ui_state::Windows, ui: &mut egui::Ui) {
    use qymcad_update::Outcome;
    let asked_by_hand = win.is(qymcad_ui_state::WinKind::Updates);
    match crate::gui::update_ui::outcome() {
        Outcome::Idle => {}
        Outcome::Asking => {
            ui.separator();
            ui.label(egui::RichText::new(crate::i18n::tr("update-asking")).weak());
        }
        Outcome::UpToDate if asked_by_hand => {
            ui.separator();
            ui.label(egui::RichText::new(crate::i18n::tr("update-none")).weak());
        }
        Outcome::Unreachable if asked_by_hand => {
            ui.separator();
            ui.label(egui::RichText::new(crate::i18n::tr("update-unreachable")).weak());
        }
        Outcome::UpToDate | Outcome::Unreachable => {}
        Outcome::Found(latest) => {
            ui.separator();
            // The badge leads where the menu item leads: seeing the news and being unable to act on it
            // from the same place is what makes people hunt through menus.
            let text = format!("{} {} {}", ph::ARROW_CIRCLE_UP, crate::i18n::tr("update-found"), latest.latest);
            if ui.button(egui::RichText::new(text).color(scheme.pal.hint_action())).clicked() {
                win.open(qymcad_ui_state::WinKind::Updates);
            }
        }
    }
}

/// THE STATUS LINE: what was just said, how defined the sketch is, the units and the cursor.
///
/// Takes what it reads and nothing else - six borrows rather than the whole application. THE SKETCH'S
/// DEFINEDNESS is the sketcher's main state, and in CAD it belongs in the status line: one looks at the
/// drawing rather than hunting for it in a panel off to the side.
pub(crate) fn status_bar(sc: &mut qymcad_ui_state::StatusCtx, ui: &mut egui::Ui) {
    let qymcad_ui_state::StatusCtx { cache, cursor, project, scheme, set, sketch_ses, status, win } = sc;
    let (cursor, status) = (*cursor, *status);
    // THE CHECK FOR A NEWER VERSION IS STARTED HERE, and shown here, so that the two cannot drift apart.
    //
    // A frame is where it belongs: nothing else in the program knows that a start happened, and this
    // costs one comparison of two numbers when there is nothing to do. It never waits for the network.
    crate::gui::update_ui::ask_if_due(set);
    ui.horizontal(|ui| {
        ui.label(status);
        update_note(scheme, win, ui);
        if let Some(sid) = sketch_ses.editing {
            if let Some(si) = project.sketches.iter().position(|s| s.id == sid) {
                let (line, col) = crate::gui::sketching::sketch_dof_line(cache, project, scheme, si);
                ui.separator();
                ui.label(egui::RichText::new(line).color(col));
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(crate::i18n::tr("bar-units-mm"));
            ui.separator();
            match cursor {
                Some(c) => ui.monospace(format!("X {:.2}  Y {:.2}", c.x, c.y)),
                None => ui.monospace("X —  Y —"),
            };
        });
    });
}
