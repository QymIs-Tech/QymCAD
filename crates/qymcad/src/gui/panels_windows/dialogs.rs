//! THE DIALOGS: a crash, the updates, about, a template saved, the document, the mouse, the mesh quality, a delete confirmed.

use super::*;

/// "THE LAST RUN ENDED IN AN ERROR" - shown once, when reports from earlier runs are found.
///
/// Without this the reports pile up in a directory nobody has heard of. The window says where the
/// file is and hands the path over, because the next thing asked of the person is to attach it.
///
/// IT TAKES ALL OF THEM AT ONCE, not the newest one. Reported behaviour: "however many times you close
/// the window at startup, it comes up again every time". The start used to pick up a single report, so
/// three of them meant three starts and three windows - and a window that returns every start is a
/// window people learn to dismiss without reading, which is the whole point of showing it once.
///
/// The path shown is the newest report's: that is the run the person remembers. Closing answers for
/// every one of them.
pub(crate) fn crash_notice(crash_report: &mut Vec<std::path::PathBuf>, ctx: &egui::Context) {
    let Some(path) = crash_report.first().cloned() else { return };
    let mut open = true;
    let mut dismiss = false;
    egui::Window::new(format!("{} {}", qymcad_ui_state::tone_icon(qymcad_ui_state::Tone::Warning).unwrap_or_default(), crate::i18n::tr("crash-title"))).id(egui::Id::new("win_crash"))
        // OVER EVERY WINDOW, the start screen too: it stood on the same layer and took the click meant for "Close"
        .order(egui::Order::Foreground)
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
        for p in crash_report.iter() {
            crate::crash::mark_seen(p);
        }
        crash_report.clear();
    }
}

/// The About window (Help -> About).
/// WHAT THE CHECK FOR A NEWER VERSION CAME TO (Help -> Check for updates).
///
/// ONE BUTTON, AND IT OPENS A BROWSER. The program downloads nothing: behind self-replacement come
/// resuming a broken transfer, checksums, write permissions, the macOS quarantine flag and an argument
/// with an antivirus on Windows. A browser does all of that better and every person already has one -
/// and whoever installed from a package manager updates with that manager, which they know.
pub(crate) fn updates_dialog(win: &mut super::Windows, scheme: &super::SchemeUi, ctx: &egui::Context) {
    if !win.is(WinKind::Updates) {
        return;
    }
    use qymcad_update::Outcome;
    let outcome = crate::gui::update_ui::outcome();
    let mut open = true;
    egui::Window::new(format!("{} {}", ph::ARROW_CIRCLE_UP, crate::i18n::tr("help-check-updates")))
        .id(egui::Id::new("win_check_updates"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(460.0)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(crate::i18n::tr("update-yours"));
                ui.label(egui::RichText::new(crate::gui::update_ui::ours()).monospace());
            });
            ui.add_space(6.0);
            match outcome {
                Outcome::Idle | Outcome::Asking => {
                    ui.label(crate::i18n::tr("update-asking"));
                }
                Outcome::UpToDate => {
                    ui.label(crate::i18n::tr("update-none"));
                }
                Outcome::Unreachable => {
                    // Said plainly rather than dressed up as good news: somebody on a version six months
                    // old must not be told they are current because a request did not go through.
                    qymcad_ui_state::say(ui, &scheme.pal, &qymcad_ui_state::Notice::warning(crate::i18n::tr("update-unreachable")), qymcad_ui_state::NoticeSize::Line);
                }
                Outcome::Found(latest) => {
                    ui.horizontal(|ui| {
                        ui.label(crate::i18n::tr("update-newest"));
                        ui.label(egui::RichText::new(&latest.latest).monospace().strong());
                        if let Some(d) = &latest.published {
                            ui.label(egui::RichText::new(d).weak());
                        }
                    });
                    // A VERSION DECLARED UNFIT IS SAID FIRST AND LOUDEST. It is the one message here
                    // that is worth interrupting somebody for.
                    if qymcad_update::is_unfit(&crate::gui::update_ui::ours(), latest.broken_below.as_deref()) {
                        ui.add_space(6.0);
                        qymcad_ui_state::say(ui, &scheme.pal, &qymcad_ui_state::Notice::warning(crate::i18n::tr("update-unfit")), qymcad_ui_state::NoticeSize::Line);
                    }
                    if let Some(notice) = &latest.notice {
                        ui.add_space(6.0);
                        ui.colored_label(scheme.pal.hint_action(), notice);
                    }
                    if let Some(notes) = &latest.notes {
                        ui.add_space(8.0);
                        ui.separator();
                        ui.add_space(4.0);
                        // The notes are written by a person and can be any length; the window must not
                        // grow past the screen because a release had a lot to say.
                        egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                            ui.label(egui::RichText::new(notes).small());
                        });
                    }
                    ui.add_space(10.0);
                    ui.separator();
                    ui.add_space(6.0);
                    if ui.button(format!("{} {}", ph::ARROW_SQUARE_OUT, crate::i18n::tr("update-open-page"))).clicked() {
                        ui.ctx().open_url(egui::OpenUrl::new_tab(&latest.url));
                    }
                }
            }
            ui.add_space(4.0);
        });
    if !open {
        win.close(WinKind::Updates);
    }
}

pub(crate) fn about_dialog(win: &mut super::Windows, scheme: &super::SchemeUi, ctx: &egui::Context) {
    if !win.is(WinKind::About) {
        return;
    }
    let mut open = true;
    egui::Window::new(format!("{} {}", ph::INFO, crate::i18n::tr("win-about")))
        .id(egui::Id::new("win_about"))
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
                if ui.small_button(ph::COPY).on_hover_text(crate::i18n::tr("about-copy-hint")).clicked() {
                    ui.output_mut(|o| o.commands.push(egui::OutputCommand::CopyText(crate::diagnostics::block())));
                }
            });
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(8.0);
            ui.label(crate::i18n::tr("about-what"));
            ui.add_space(10.0);
            qymcad_ui_state::say(ui, &scheme.pal, &qymcad_ui_state::Notice::warning(crate::i18n::tr("about-develop-warning")), qymcad_ui_state::NoticeSize::Line);
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
        .id(egui::Id::new("win_save_as_template"))
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
    egui::Window::new(format!("{} {}", ph::FILE_TEXT, crate::i18n::tr("doc-props-title"))).id(egui::Id::new("win_doc_props")).open(&mut open).resizable(true).default_width(420.0).show(ctx, |ui| {
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
        ui.label(egui::RichText::new(if created.is_empty() { crate::i18n::tr("doc-props-not-saved-yet") } else { crate::i18n::tr1("doc-props-created", "when", &created) }).small().weak());
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
                                       // THE QUESTION HOLDS THE WINDOW: a backdrop over the whole window takes every click, and the question stands above
                                       // it - as `egui::Modal` does it, but with a window of its own, which names itself to assistive technology and to
                                       // a check where a modal names itself to nobody. Reported behaviour: a click on "Edit the sketch" behind the
                                       // question opened the sketch.
                                       // the backdrop is the splash ground of the scheme, let through at 40 percent
    let backdrop = wc.scheme.pal.splash_bg().gamma_multiply(0.4);
    egui::Area::new(egui::Id::new("unsaved_backdrop")).order(egui::Order::Foreground).fixed_pos(egui::pos2(0.0, 0.0)).show(ctx, |ui| {
        let all = ui.ctx().content_rect();
        ui.allocate_rect(all, egui::Sense::click_and_drag());
        ui.painter().rect_filled(all, 0.0, backdrop);
    });
    egui::Window::new(format!("{}  {}", qymcad_ui_state::tone_icon(qymcad_ui_state::Tone::Warning).unwrap_or_default(), crate::i18n::tr("win-unsaved")))
        .id(egui::Id::new("win_unsaved"))
        .order(egui::Order::Tooltip)
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
    if choice.is_none() && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        choice = Some(2); // Esc answers as Cancel does
    }
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

/// The modal dialogue for choosing the mesh quality (the deflection) before an export.
pub(crate) fn mesh_quality_dialog(wc: &mut qymcad_ui_state::WinCtx, ctx: &egui::Context) {
    let Some((format, target)) = *wc.mesh_export else { return };
    let mut choice: Option<Option<f64>> = None; // None = close; Some(Some(defl)) = export; Some(None) = cancel
    egui::Window::new(crate::i18n::tr1("mesh-title", "format", crate::gui::mesh_entry(format).name()))
        .id(egui::Id::new("win_mesh_quality"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.label(crate::i18n::tr("stl-detail"));
            ui.add_space(4.0);
            // (the label, the deflection in mm)
            let presets: [(&str, f64); 4] =
                [(&crate::i18n::tr("stl-draft"), 0.2), (&crate::i18n::tr("stl-standard"), 0.05), (&crate::i18n::tr("stl-high"), 0.02), (&crate::i18n::tr("stl-max"), 0.005)];
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
            *wc.mesh_export = None;
            wc.ask.push(qymcad_ui_state::WinAsk::ExportMesh(format, target, defl));
        }
        Some(None) => *wc.mesh_export = None,
        None => {}
    }
}

/// The modal confirmation popup for deleting a tree node. Enter or Yes deletes, Esc or No cancels.
pub(crate) fn confirm_delete_popup(wc: &mut qymcad_ui_state::WinCtx, ctx: &egui::Context) {
    // a group answered yes goes on, one node a frame
    if let Some((sel, dependents)) = qymcad_ui_state::next_of_a_group_delete(&mut *wc.deferred, &*wc.project) {
        wc.ask.push(qymcad_ui_state::WinAsk::Delete(sel, dependents));
        ctx.request_repaint();
        return;
    }
    // the group is done: its one step of undo is closed
    if wc.deferred.delete_queue.step.take().is_some() {
        qymcad_ui_state::commit_edit(&mut wc.rebuild());
    }
    let Some(sel) = wc.deferred.delete else { return };
    // SEVERAL ROWS PICKED: the question names every one of them
    let group: Vec<qymcad_ui_state::Sel> = wc.deferred.delete_group.iter().filter_map(|id| qymcad_ui_state::sel_of_node(&*wc.project, *id)).collect();
    let what = if group.len() > 1 {
        group.iter().map(|s| crate::gui::sel_delete_label(&*wc.project, *s)).collect::<Vec<_>>().join(", ")
    } else {
        crate::gui::sel_delete_label(&*wc.project, sel)
    };
    let cascade = crate::gui::delete_cascade_names(&*wc.project, sel);
    let mut do_del = ctx.input(|i| i.key_pressed(egui::Key::Enter)); // Yes is the default
    let mut cancel = ctx.input(|i| i.key_pressed(egui::Key::Escape));
    egui::Window::new(format!("{} {}", ph::TRASH, crate::i18n::tr("win-delete-q")))
        .id(egui::Id::new("win_delete_question"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.label(crate::i18n::tr1("confirm-delete-what", "what", &what));
            // WHAT STANDS ON IT, BY NAME, and a tick that decides its fate: by default it stays in the timeline,
            // red with the reason, for the person to repair; ticked, it goes too. A general line saying "along with
            // its dependants" is true but does not answer "what am I about to lose"; a list does, and it comes
            // from the same core query as the lineage in the properties card.
            if cascade.is_empty() {
                ui.label(egui::RichText::new(crate::i18n::tr("confirm-cascade")).weak().small());
            } else {
                ui.checkbox(&mut wc.deferred.delete_dependents, crate::i18n::tr("confirm-with-dependents"));
                if !wc.deferred.delete_dependents {
                    ui.label(egui::RichText::new(crate::i18n::tr("confirm-dependents-stay")).weak().small());
                }
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
        let dependents = std::mem::take(&mut wc.deferred.delete_dependents);
        // the one chosen now, the rest of the group after it, one a frame
        let chosen = qymcad_ui_state::node_of_sel(&*wc.project, sel);
        let rest: Vec<qymcad_core::model::Id> = std::mem::take(&mut wc.deferred.delete_group).into_iter().filter(|id| Some(*id) != chosen).collect();
        // SEVERAL NODES ARE ONE STEP OF UNDO: opened here, closed when the last of them is gone
        let step = (!rest.is_empty()).then(|| crate::i18n::tr("sk-delete"));
        if let Some(name) = &step {
            qymcad_ui_state::begin_edit(&mut *wc.edits, &*wc.project, name.clone());
        }
        wc.deferred.delete_queue = qymcad_ui_state::DeleteQueue { ids: rest, dependents, step };
        wc.ask.push(qymcad_ui_state::WinAsk::Delete(sel, dependents));
    } else if cancel {
        wc.deferred.delete = None;
        wc.deferred.delete_dependents = false;
        wc.deferred.delete_group.clear();
    }
}
