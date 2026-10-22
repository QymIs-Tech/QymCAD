//! WHERE A TOOL SPEAKS: one plate in the lower corner of the view, each sentence in its tone, said by the voice of the
//! program - what the tool tells, what it warns of, what will not build, and the answer to an Enter it refused.
//!
//! The sentences used to lie in three places at once: under the controls of the bar, under the buttons of the box at
//! the geometry, and in the status line for a refused Enter.

use super::*;

/// THE ANSWER TO A REFUSED ENTER, kept with the gate that gave it: said until the gate changes - a pick made, a value
/// typed - so it stays in sight rather than for one frame.
#[derive(Clone)]
struct Remembered {
    gate: Gate,
    words: Notice,
}

/// Where the answer to a refused Enter is kept between frames.
const REFUSAL: &str = "the refusal of the tool in hand";

/// The words of the tool laid in the last frame, and the frame, for a reader of the screen.
#[derive(Clone)]
struct Laid {
    words: Vec<Notice>,
    frame: u64,
}

/// THE PLATE THE WORDS OF THE TOOL IN HAND LIE ON: the id of its area.
pub const TOOL_WORDS: &str = "the words of the tool in hand";

/// Keep the answer to a refused Enter with the gate it was given by.
pub(crate) fn remember_refusal(ctx: &egui::Context, gate: &Gate, words: &Notice) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(REFUSAL), Remembered { gate: gate.clone(), words: words.clone() }));
}

/// The answer to a refused Enter, while the gate is the one it was given by; let go once the gate changes.
pub(crate) fn remembered_refusal(ctx: &egui::Context, gate: &Gate) -> Option<Notice> {
    let id = egui::Id::new(REFUSAL);
    let kept: Option<Remembered> = ctx.data(|d| d.get_temp(id));
    match kept {
        Some(r) if r.gate == *gate => Some(r.words),
        Some(_) => {
            ctx.data_mut(|d| d.remove::<Remembered>(id));
            None
        }
        None => None,
    }
}

/// LAY THE WORDS OF THE TOOL IN HAND on their plate in the lower left corner of the view `canvas`, over the status line
/// where the program speaks and clear of the model a person aims at, each sentence on its own line in its tone. They
/// lie over the view rather than in the bar, which keeps its height (`command_bar_row`) - three lines of them in the
/// bar moved the view by 43 points as the tool said them and went quiet. Nothing to say, nothing is laid.
pub fn lay_words(ctx: &egui::Context, canvas: egui::Rect, pal: &qymcad_scheme::Palette, words: &[Notice]) {
    let frame = ctx.cumulative_frame_nr();
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(TOOL_WORDS), Laid { words: words.to_vec(), frame }));
    // a view not laid out yet - a bar drawn alone, before its window - lends the plate the whole screen
    let canvas = view_or_window(ctx, canvas);
    if words.is_empty() {
        return;
    }
    egui::Area::new(egui::Id::new(TOOL_WORDS)).pivot(egui::Align2::LEFT_BOTTOM).fixed_pos(canvas.left_bottom() + egui::vec2(4.0, -4.0)).order(AT_GEOMETRY).show(ctx, |ui| {
        egui::Frame::popup(ui.style()).show(ui, |ui| {
            ui.set_max_width((canvas.width() * 0.6).max(240.0));
            for w in words {
                say(ui, pal, w, NoticeSize::Line);
            }
        });
    });
}

/// THE WORDS THE TOOL IN HAND LAID IN THE LAST FRAME - what a person reads on its plate; none when no tool spoke.
pub fn words_laid(ctx: &egui::Context) -> Vec<Notice> {
    let laid: Option<Laid> = ctx.data(|d| d.get_temp(egui::Id::new(TOOL_WORDS)));
    laid.filter(|l| ctx.cumulative_frame_nr() <= l.frame + 1).map(|l| l.words).unwrap_or_default()
}

/// THE VIEW A TOOL STANDS IN: `canvas`, or the window while no view is laid yet - the first frames, a check drawing a bar
/// alone.
pub(crate) fn view_or_window(ctx: &egui::Context, canvas: egui::Rect) -> egui::Rect {
    if canvas.is_finite() && canvas.is_positive() {
        canvas
    } else {
        ctx.content_rect()
    }
}
