//! COMMAND SEARCH.
//!
//! Press, type three letters, apply. It closes both "I do not know where the button is" and "I know,
//! but finding it takes too long" — and there are forty icons here in two scrolling columns.
//!
//! TWO ENTRANCES, AND THAT IS NOT A WHIM. The space bar is convenient (free, under the thumb), but in
//! an input field it must type a space: `60 + 2` is a legitimate expression. So the space bar opens
//! search only while the keyboard is free, and Ctrl+K always, including from within a field.
//!
//! SEARCH HAS NO LIST OF COMMANDS OF ITS OWN. It reads `command_catalog`, and a guard keeps the
//! catalogue and the toolbar together. A search with its own list would fall behind the toolbar on
//! the very first new feature — that has already happened to the help and to joint diagnostics.
use super::App;
use egui_phosphor::regular as ph;
use crate::gui::WinKind;

/// How many rows are shown. Beyond a dozen a person does not read but refines the query.
const MAX_ROWS: usize = 8;

impl App {

    /// The search window: the door that builds the narrow context.
    pub(super) fn command_search_window(&mut self, ctx: &egui::Context) {
        let mut asks = Vec::new();
        command_search_window(&mut self.win_ctx(&mut asks), ctx);
        self.do_win_asks(asks, ctx);
    }
}

/// Open or close the command search.
pub(crate) fn toggle_command_search(win: &mut super::Windows) {
    win.toggle(WinKind::CmdSearch);
    if win.is(WinKind::CmdSearch) {
        win.cmd_search_query.clear();
        win.cmd_search_sel = 0;
        win.cmd_search_focus = true;
    }
}
/// THE COMMANDS FOUND: the current workbench first, then the rest.
///
/// The order is not cosmetic: a person searches for what they are busy with. A command of another
/// workbench is shown lower and with a marker - it is legitimate (it will switch the workbench) but must
/// not push aside one's own.
pub(crate) fn command_search_hits(workbench: qymcad_ui_state::Workbench, query: &str) -> Vec<&'static crate::command_catalog::Command> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Vec::new();
    }
    let wb = crate::gui::workbench_code(&workbench);
    let mut mine: Vec<(usize, &'static crate::command_catalog::Command)> = Vec::new();
    let mut other: Vec<(usize, &'static crate::command_catalog::Command)> = Vec::new();
    for c in crate::command_catalog::COMMANDS {
        // SEARCH IN BOTH LANGUAGES. Someone working in a translated interface often remembers
        // `fillet`: that is how other manuals, videos and forums write it.
        let names = [c.name().to_lowercase(), c.name_in("ru").to_lowercase(), c.name_in("en").to_lowercase()];
        let hit = names.iter().filter(|n| !n.is_empty()).find_map(|n| n.find(&q));
        // and by code: `part.fillet` is found by "fillet" even without a name
        let hit = hit.or_else(|| c.code.to_lowercase().find(&q));
        let Some(at) = hit else { continue };
        // a match AT THE START of a name is more precise than one in the middle, so it ranks higher
        let rank = at;
        if c.workbench == wb {
            mine.push((rank, c));
        } else {
            other.push((rank, c));
        }
    }
    mine.sort_by_key(|(r, c)| (*r, c.code));
    other.sort_by_key(|(r, c)| (*r, c.code));
    mine.into_iter().chain(other).map(|(_, c)| c).take(MAX_ROWS).collect()
}

/// THE SEARCH WINDOW: the field, the list, the arrows, Enter and Esc.
pub(crate) fn command_search_window(wc: &mut qymcad_ui_state::WinCtx, ctx: &egui::Context) {
    if !wc.win.is(WinKind::CmdSearch) {
        return;
    }
    let hits = command_search_hits(wc.workbench, &wc.win.cmd_search_query.clone());
    if !hits.is_empty() {
        wc.win.cmd_search_sel = wc.win.cmd_search_sel.min(hits.len() - 1);
    }
    // THE ARROWS AND ENTER ARE READ BEFORE THE FIELD: `TextEdit` does not use them, but the
    // order matters for Esc - it must close the SEARCH rather than fall through into the general
    // cancel ladder.
    let (up, down, enter, esc) = ctx.input(|i| {
        (
            i.key_pressed(egui::Key::ArrowUp),
            i.key_pressed(egui::Key::ArrowDown),
            i.key_pressed(egui::Key::Enter),
            i.key_pressed(egui::Key::Escape),
        )
    });
    if down && !hits.is_empty() {
        wc.win.cmd_search_sel = (wc.win.cmd_search_sel + 1) % hits.len();
    }
    if up && !hits.is_empty() {
        wc.win.cmd_search_sel = (wc.win.cmd_search_sel + hits.len() - 1) % hits.len();
    }
    let mut run: Option<&'static str> = None;
    let sel = wc.win.cmd_search_sel;
    let wb_code = crate::gui::workbench_code(&wc.workbench);
    egui::Window::new(format!("{} {}", ph::MAGNIFYING_GLASS, crate::i18n::tr("cs-title")))
        .title_bar(false)
        .resizable(false)
        .fixed_size(egui::vec2(460.0, 0.0))
        .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 90.0))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(ph::MAGNIFYING_GLASS);
                let te = ui.add(egui::TextEdit::singleline(&mut wc.win.cmd_search_query).desired_width(f32::INFINITY).hint_text(crate::i18n::tr("cs-hint")));
                if std::mem::take(&mut wc.win.cmd_search_focus) {
                    te.request_focus();
                }
            });
            if hits.is_empty() && !wc.win.cmd_search_query.trim().is_empty() {
                ui.label(egui::RichText::new(crate::i18n::tr1("cs-nothing", "q", &wc.win.cmd_search_query)).weak().small());
            }
            for (i, c) in hits.iter().enumerate() {
                let mine = c.workbench == wb_code;
                let key = qymcad_ui_state::hotkey_key(wc.set, c.code);
                ui.horizontal(|ui| {
                    let mut text = egui::RichText::new(c.name());
                    if i == sel {
                        text = text.strong().color(wc.scheme.pal.active());
                    }
                    if ui.selectable_label(i == sel, text).clicked() {
                        run = Some(c.code);
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if !key.is_empty() {
                            ui.label(egui::RichText::new(&key).weak().small());
                        }
                        // ANOTHER WORKBENCH - SAY SO. Otherwise the command "works wrong": a
                        // person will not understand why the workbench changed.
                        if !mine {
                            ui.label(egui::RichText::new(crate::i18n::tr(&format!("wb-{}", c.workbench))).weak().small());
                        }
                    });
                });
            }
        });
    if enter && !hits.is_empty() {
        run = Some(hits[wc.win.cmd_search_sel].code);
    }
    if esc {
        wc.win.close(WinKind::CmdSearch);
    }
    if let Some(code) = run {
        wc.win.close(WinKind::CmdSearch);
        // THE SHARED DOOR, asked for rather than opened here: launching a command switches the workbench and
        // starts a tool, which is the application's business and cannot happen in the middle of drawing.
        wc.ask.push(qymcad_ui_state::WinAsk::RunCommand(code));
    }
}
