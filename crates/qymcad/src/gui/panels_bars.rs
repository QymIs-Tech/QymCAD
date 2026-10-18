//! THE BARS ACROSS THE TOP: the menu, the tool bars, the options of the active tool, the command bars.
//!
//! They used to sit inside the frame's body, where "what the frame does" and "what it draws" could not be told
//! apart and editing one command touched the whole life cycle of the frame.

pub(crate) use qymcad_part::*;
use super::*;

// the bar of the tools of a workbench, drawn from the catalogue
mod tool_bar;
pub(crate) use tool_bar::{wb_toolbar, BAR_MARGIN};

impl App {
    /// The command bar of the Part workbench, through its own doorway.
    pub(crate) fn feat_command_bar(&mut self, ui: &mut egui::Ui) {
        crate::gui::panels_bars::feat_command_bar(&mut self.part_ctx(), ui);
    }
}

pub(crate) fn menu_bar(bc: &mut qymcad_ui_state::BarCtx, ui: &mut egui::Ui) {
    use crate::gui::bar_menu::BarMenu as _;
    // The panel lives inside a `Ui` now; the context is still wanted for windows,
    // input and viewport commands, and it comes from the same place.
    let ctx = &ui.ctx().clone();
    // The menu items belonging to CAM (the machine, the tools, the G-code, the setup, the rapids) appear only
    // when the machining module is enabled. That module is under development and hidden by default.
    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
    egui::MenuBar::new().ui(ui, |ui| {
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
        ui.bar_menu_button(qymcad_i18n::tr("menu-file"), |ui| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
            // A NEW PROJECT IS AN EMPTY ASSEMBLY: a part is made by "New part" of the start screen or of the assembly,
            // by the person's own intent (decided 29.09) - a project that came with a part to delete made them clean up
            if ui.button(format!("{}  {}", ph::FILE, qymcad_i18n::tr("file-new"))).clicked() {
                bc.ask.push(qymcad_ui_state::BarAsk::Nav(qymcad_ui_state::Nav::NewAssembly));
                ui.close();
            }
            // TEMPLATES ARE CHOSEN IN THE CHOOSER, opened in the folder of templates, as a new document from a template is
            // chosen in the professional systems. A submenu of saved templates stood here, disabled and silent while
            // there were none: the item did nothing and said nothing.
            if ui.button(format!("{}  {}", ph::FILE_TEXT, qymcad_i18n::tr("file-new-from-template"))).clicked() {
                bc.ask.push(qymcad_ui_state::BarAsk::Nav(qymcad_ui_state::Nav::NewFromTemplate));
                ui.close();
            }
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
            // RECENT FILES: a basic expectation of any program that has files. The submenu always opens: empty, it says
            // so in words, and "Clear the list" stands in it disabled. A disabled item said nothing at all on a clean start.
            let recent = bc.set.recent.clone();
            {
                ui.menu_button(format!("{}  {}", ph::CLOCK_COUNTER_CLOCKWISE, qymcad_i18n::tr("file-recent")), |ui| {
                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                    if recent.is_empty() {
                        ui.label(egui::RichText::new(qymcad_i18n::tr("file-recent-empty")).weak());
                    }
                    for path in &recent {
                        // the row shows THE FILE NAME with the full path in the tooltip: paths are longer than the menu
                        let name = std::path::Path::new(path).file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| path.clone());
                        if ui.button(name).on_hover_text(path).clicked() {
                            bc.ask.push(qymcad_ui_state::BarAsk::Nav(qymcad_ui_state::Nav::OpenPath(path.clone())));
                            ui.close();
                        }
                    }
                    ui.separator();
                    if ui.add_enabled(!recent.is_empty(), egui::Button::new(format!("{}  {}", ph::TRASH, qymcad_i18n::tr("file-recent-clear")))).clicked() {
                        bc.set.recent.clear();
                        ui.close();
                    }
                });
            }
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
            // ONE DOOR FOR EVERY FORMAT: the file's extension decides what it becomes (see `import_door`)
            let formats = qymcad_io::Format::names_of(&qymcad_io::Format::ALL);
            if ui.button(format!("{}  {}", ph::FILE_ARROW_DOWN, qymcad_i18n::tr("file-import"))).on_hover_text(qymcad_i18n::tr1("file-import-hint", "formats", &formats)).clicked() {
                bc.ask.push(qymcad_ui_state::BarAsk::Import(qymcad_ui_state::Want::Anything));
                ui.close();
            }
            ui.separator();
            // ONE ITEM, THE FORMATS INSIDE IT (see `export_menu`)
            match crate::gui::export_menu::export_submenu(ui, crate::gui::export_menu::ExportFrom::Project) {
                Some(crate::gui::export_menu::ExportChoice::Exact(f)) => {
                    bc.ask.push(qymcad_ui_state::BarAsk::ExportExact(f, qymcad_ui_state::ExportTarget::Project));
                    ui.close();
                }
                Some(crate::gui::export_menu::ExportChoice::Mesh(f)) => {
                    *bc.mesh_export = Some((f, qymcad_ui_state::ExportTarget::Project));
                    ui.close();
                }
                None => {}
            }
            ui.separator();
            if ui.button(format!("{}  {}", ph::SIGN_OUT, qymcad_i18n::tr("file-quit"))).clicked() {
                bc.ask.push(qymcad_ui_state::BarAsk::Nav(qymcad_ui_state::Nav::Exit));
            }
        });
        ui.bar_menu_button(qymcad_i18n::tr("menu-edit"), |ui| {
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
        ui.bar_menu_button(qymcad_i18n::tr("menu-view"), |ui| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
            ui.checkbox(&mut *bc.mode_3d, format!("{}  {}", ph::CUBE, qymcad_i18n::tr("menu-orbit3d")));
            if ui.button(format!("{}  {}", ph::CORNERS_OUT, qymcad_i18n::tr("menu-fit-view"))).clicked() {
                bc.view.initialized = false;
                bc.cam.init = false;
                ui.close();
            }
            // THE PANELS: a tick for each one a person may hide; hidden, it takes no room
            ui.menu_button(format!("{}  {}", ph::SIDEBAR, qymcad_i18n::tr("menu-panels")), |ui| crate::gui::panels_windows::panels_menu(&mut *bc.set, ui));
            ui.separator();
            ui.label(qymcad_i18n::tr("settings-scheme"));
            // THE SCHEMES COME FROM THE LIVE LIST (the built-in ones and the user's), and the label from
            // `title()`: a built-in scheme has NO name of its own, it comes from the language catalogue.
            // This used to be `p.name`, and after the identifier and the label were separated the menu
            // items were left as bare icons with no words.
            let rows: Vec<(String, String, bool)> = bc.scheme.all.iter().map(|p| (p.id.clone(), p.title(), p.light)).collect();
            for (id, title, light) in rows {
                let icon = if qymcad_scheme::store::is_builtin(&id) {
                    if light {
                        ph::SUN
                    } else {
                        ph::MOON
                    }
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
        ui.bar_menu_button(qymcad_i18n::tr("menu-windows"), |ui| {
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
        ui.bar_menu_button(qymcad_i18n::tr("menu-help"), |ui| {
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

/// THE FAULTS OF THE DOCUMENT AS TWO BADGES - the errors red, the warnings yellow, each with its count; a kind with
/// none has no badge. They count what the document holds now, whatever the line last said: a red cut in another part
/// is counted while one works here. True when either was clicked.
fn fault_badges(ui: &mut egui::Ui, pal: &crate::palette::Palette, faults: &[qymcad_ui_state::Fault]) -> bool {
    let count = qymcad_ui_state::fault_count(faults);
    let errors = fault_badge(ui, pal, qymcad_ui_state::Notice::error(count.errors.to_string()), "faults-errors-hint");
    let warnings = fault_badge(ui, pal, qymcad_ui_state::Notice::warning(count.warnings.to_string()), "faults-warnings-hint");
    errors || warnings
}

/// One badge: the icon and the number in the colour of the tone; nothing for a count of zero.
fn fault_badge(ui: &mut egui::Ui, pal: &crate::palette::Palette, count: qymcad_ui_state::Notice, hint: &str) -> bool {
    if count.words == "0" {
        return false;
    }
    ui.separator();
    let text = qymcad_ui_state::notice_text(pal, &count, qymcad_ui_state::NoticeSize::Line);
    ui.add(egui::Label::new(text).sense(egui::Sense::click())).on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text(crate::i18n::tr1(hint, "n", &count.words)).clicked()
}

/// THE STATUS LINE: what was just said, how defined the sketch is, the units and the cursor.
///
/// Takes what it reads and nothing else - six borrows rather than the whole application. THE SKETCH'S
/// DEFINEDNESS is the sketcher's main state, and in CAD it belongs in the status line: one looks at the
/// drawing rather than hunting for it in a panel off to the side.
pub(crate) fn status_bar(sc: &mut qymcad_ui_state::StatusCtx, ui: &mut egui::Ui) {
    let qymcad_ui_state::StatusCtx { cache, cursor, project, scheme, set, sketch_ses, line, win, par_search, ask } = sc;
    let status = &mut *line.said;
    let cursor = *cursor;
    // THE CHECK FOR A NEWER VERSION IS STARTED HERE, and shown here, so that the two cannot drift apart.
    //
    // A frame is where it belongs: nothing else in the program knows that a start happened, and this
    // costs one comparison of two numbers when there is nothing to do. It never waits for the network.
    crate::gui::update_ui::ask_if_due(set);
    ui.horizontal(|ui| {
        // a warning yellow with its icon, an error red with its: the line is where a person looks after an action
        let faults = qymcad_ui_state::document_faults(cache, project);
        // WHAT COMES AND GOES ON THE LINE, each part in a scope of its own that is there whether it shows anything or not:
        // egui numbers what follows by what came before, and the readout of the pointer, renumbered on entering a sketch,
        // was marked with egui's red frame of a widget that changed its id
        qymcad_shell::keyed(ui, ("the status line", "said"), |ui| {
            let said = qymcad_ui_state::notice_text(&scheme.pal, status, qymcad_ui_state::NoticeSize::Line);
            let fault_said = matches!(status.tone, qymcad_ui_state::Tone::Warning | qymcad_ui_state::Tone::Error) && !faults.is_empty();
            // the words of a fault lead where the badges lead: it is the line a person reads first
            let words_clicked = if fault_said {
                ui.add(egui::Label::new(said).sense(egui::Sense::click())).on_hover_cursor(egui::CursorIcon::PointingHand).clicked()
            } else {
                ui.label(said);
                false
            };
            let badge_clicked = fault_badges(ui, &scheme.pal, &faults);
            let lead = words_clicked || badge_clicked;
            if lead {
                if let Some(said) = qymcad_ui_state::lead_to_faults(&faults, project, win, par_search, ask) {
                    *line.rebuild = said.clone();
                    *status = said;
                }
            }
        });
        qymcad_shell::keyed(ui, ("the status line", "update"), |ui| update_note(scheme, win, ui));
        qymcad_shell::keyed(ui, ("the status line", "sketch"), |ui| {
            if let Some(sid) = sketch_ses.editing {
                if let Some(si) = project.sketches.iter().position(|s| s.id == sid) {
                    let (line, col) = crate::gui::sketching::sketch_dof_line(cache, project, scheme, si);
                    ui.separator();
                    ui.label(egui::RichText::new(line).color(col));
                }
            }
        });
        qymcad_shell::keyed(ui, ("the status line", "readout"), |ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(crate::i18n::tr("bar-units-mm"));
                ui.separator();
                match cursor {
                    Some(c) => ui.monospace(format!("X {:.2}  Y {:.2}", c.x, c.y)),
                    None => ui.monospace("X —  Y —"),
                };
            });
        });
    });
}
