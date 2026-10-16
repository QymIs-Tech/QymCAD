//! THE WINDOW OF FAULTS: every error and warning of the document in one list, each row leading to its place.

use super::*;

/// THE WINDOWS OF THE DOCUMENT's numbers and faults, drawn in one pass of the frame: the parameters and the faults.
/// The window of faults leads to the parameters' table for a parameter, and both carry out their requests together.
pub(crate) fn doc_windows(wc: &mut qymcad_ui_state::WinCtx, ctx: &egui::Context) {
    params_window(wc, ctx);
    faults_window(wc, ctx);
}

/// THE ERRORS AND WARNINGS, opened from the status line when there are more than one: a row is the icon of its tone,
/// what is wrong and where - the part, the sketch, the operation, the assembly and its mate, or the parameter - and a
/// click on it goes there. The faults of the context being worked in come first.
pub(crate) fn faults_window(wc: &mut qymcad_ui_state::WinCtx, ctx: &egui::Context) {
    if !wc.win.is(WinKind::Faults) {
        return;
    }
    let faults = qymcad_ui_state::document_faults(wc.cache, wc.project);
    let here = qymcad_ui_state::current_ctx_id(wc.active_path, wc.project);
    let rows = qymcad_ui_state::faults_in_order(&faults, wc.project, here);
    let mut open = true;
    let mut chosen: Option<usize> = None;
    egui::Window::new(format!("{} {}", ph::LIST_CHECKS, crate::i18n::tr("win-faults"))).id(egui::Id::new("win_faults")).open(&mut open).resizable(true).default_width(420.0).show(ctx, |ui| {
        if rows.is_empty() {
            ui.label(egui::RichText::new(crate::i18n::tr("faults-none")).weak());
            return;
        }
        egui::ScrollArea::vertical().max_height(360.0).auto_shrink([false, true]).show(ui, |ui| {
            for (i, f) in rows.iter().enumerate() {
                // THE ROW IS ONE TARGET: the words and the path both lead there, as the path of a driver does in the
                // window of parameters
                let words = qymcad_ui_state::notice_text(&wc.scheme.pal, &qymcad_ui_state::Notice { tone: f.tone, words: f.words.clone() }, qymcad_ui_state::NoticeSize::Line);
                let said = ui.add(egui::Label::new(words).wrap().sense(egui::Sense::click())).on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text(crate::i18n::tr("faults-row-hint"));
                let path = ui
                    .add(egui::Label::new(egui::RichText::new(&f.path).small().weak()).sense(egui::Sense::click()))
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .on_hover_text(crate::i18n::tr("faults-row-hint"));
                if said.clicked() || path.clicked() {
                    chosen = Some(i);
                }
                ui.separator();
            }
        });
    });
    if let Some(f) = chosen.and_then(|i| rows.get(i)) {
        // the words are about the document: the next rebuild replaces them as its own line (see `StatusCtx::rebuild_line`)
        *wc.status = qymcad_ui_state::lead_to(f, wc.project, wc.win, wc.par_search, wc.ask);
        wc.regen.line = wc.status.clone();
    }
    if !open {
        wc.win.close(WinKind::Faults);
    }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod voice_tests;
