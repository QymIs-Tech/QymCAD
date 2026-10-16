//! THE CARDS OVER THE VIEW: a rebuild, an import, the splash, the spinner, the barrier to input.

use super::*;

pub const CARD_BUTTON: egui::Vec2 = egui::vec2(150.0, 28.0);

pub const CARD_PAD: f32 = 16.0;
pub const CARD_SPINNER: f32 = 34.0; // the ring of dots: radius 13 plus the dots themselves
pub const CARD_GAP: f32 = 8.0;
pub const CARD_MIN_W: f32 = 280.0;

/// Where each piece of the rebuild card goes, once the card itself has been placed.
pub struct RegenCard {
    /// the centre of the ring of dots
    pub spinner: egui::Pos2,
    pub title: egui::Rect,
    pub progress: Option<egui::Rect>,
    pub button: Option<egui::Rect>,
}

/// How big the card has to be to hold what is going into it.
///
/// It used to be a fixed 280x96 (150 with a counter) with the label painted centred inside, which held
/// only as long as the label was short. Reported behaviour: the text runs out past the edges of the
/// rebuild window - the line naming the node count and the thread is a good half wider than 280 px.
pub fn regen_card_size(title: egui::Vec2, progress: Option<egui::Vec2>, button: bool) -> egui::Vec2 {
    let mut inner = egui::vec2(title.x, CARD_SPINNER + CARD_GAP + title.y);
    if let Some(p) = progress {
        inner.x = inner.x.max(p.x);
        inner.y += CARD_GAP * 0.75 + p.y;
    }
    if button {
        inner.x = inner.x.max(CARD_BUTTON.x);
        inner.y += CARD_GAP + CARD_BUTTON.y;
    }
    egui::vec2((inner.x + CARD_PAD * 2.0).max(CARD_MIN_W), inner.y + CARD_PAD * 2.0)
}

/// The pieces laid out top-down inside `card`, each centred across it. Reads the same measurements
/// [`regen_card_size`] reserved room for, so nothing can land outside.
pub fn regen_card_places(card: egui::Rect, title: egui::Vec2, progress: Option<egui::Vec2>, button: bool) -> RegenCard {
    let mid = card.center().x;
    let mut y = card.min.y + CARD_PAD;
    let spinner = egui::pos2(mid, y + CARD_SPINNER / 2.0);
    y += CARD_SPINNER + CARD_GAP;
    let title_rect = egui::Rect::from_min_size(egui::pos2(mid - title.x / 2.0, y), title);
    y = title_rect.max.y;
    let progress = progress.map(|p| {
        y += CARD_GAP * 0.75;
        let r = egui::Rect::from_min_size(egui::pos2(mid - p.x / 2.0, y), p);
        y = r.max.y;
        r
    });
    let button = button.then(|| {
        y += CARD_GAP;
        egui::Rect::from_min_size(egui::pos2(mid - CARD_BUTTON.x / 2.0, y), CARD_BUTTON)
    });
    RegenCard { spinner, title: title_rect, progress, button }
}

/// A DIMMING + a spinner over the live interface. The model stays on the screen, so it is visible WHAT
/// is being rebuilt; unlike the start-up splash, the window does not collapse into a black screen.
/// AN INPUT BARRIER UNDER THE MODAL: a layer over everything that EATS clicks and drags.
///
/// Reported behaviour: the dimming worked but did not block mouse clicks on the interface itself - the
/// buttons and menus could still be pressed while it was up. The dimming really was only DRAWN, and the
/// input was muted by a `ctx.input_mut(|i| i.events.clear())` line that came too late: `egui` gathers the
/// input state at the start of the pass, before that line is reached. Clearing the events at that point
/// decides nothing any more.
///
/// This only works "from above": an interactive area in the upper layer takes the hit for itself, and the
/// widgets below never get it.
pub fn modal_input_barrier(ctx: &egui::Context, salt: &'static str) {
    let screen = ctx.viewport_rect();
    egui::Area::new(egui::Id::new(salt)).order(egui::Order::Foreground).fixed_pos(screen.min).interactable(true).show(ctx, |ui| {
        ui.allocate_response(screen.size(), egui::Sense::click_and_drag());
    });
}

/// A BARRIER WITH A WINDOW: it mutes input everywhere EXCEPT `hole`.
///
/// A rebuild forbids changing the document, but it need not forbid LOOKING. Orbiting, zooming and
/// selecting do not touch the document (the `regen_doc_stamp` fingerprint is taken from the model, the
/// camera is not part of it), so there is no reason to lock them away: a person used to sit in front of
/// a dimmed screen and wait. The barrier stays where the buttons and panels live - an edit from there
/// really would land on a stale copy.
pub fn modal_input_barrier_except(ctx: &egui::Context, hole: egui::Rect) {
    let s = ctx.viewport_rect();
    if !hole.is_positive() {
        modal_input_barrier(ctx, "regen_modal_barrier");
        return;
    }
    let strips = [
        egui::Rect::from_min_max(s.min, egui::pos2(s.max.x, hole.min.y)),                              // above
        egui::Rect::from_min_max(egui::pos2(s.min.x, hole.max.y), s.max),                              // below
        egui::Rect::from_min_max(egui::pos2(s.min.x, hole.min.y), egui::pos2(hole.min.x, hole.max.y)), // left
        egui::Rect::from_min_max(egui::pos2(hole.max.x, hole.min.y), egui::pos2(s.max.x, hole.max.y)), // right
    ];
    for (k, r) in strips.iter().enumerate() {
        if !r.is_positive() {
            continue;
        }
        egui::Area::new(egui::Id::new(("regen_barrier_strip", k))).order(egui::Order::Foreground).fixed_pos(r.min).interactable(true).show(ctx, |ui| {
            ui.allocate_response(r.size(), egui::Sense::click_and_drag());
        });
    }
}

/// HOW DARK THE VEIL OF A QUIET REBUILD IS, out of 255: faint, the part goes slightly grey.
pub const QUIET_VEIL_ALPHA: u8 = 44;

/// THE QUIET-REBUILD SPINNER - in the centre of the canvas, with no text and no backing.
///
/// Its only message: the body on show is STALE, a recompute is under way. No window, no barrier - the
/// view orbits, selection works, edits are not blocked.
pub fn draw_quiet_spinner(scheme: &SchemeUi, view_rect: egui::Rect, ctx: &egui::Context) {
    let rect = if view_rect.is_positive() { view_rect } else { ctx.viewport_rect() };
    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("quiet_spinner")));
    // THE PART GOES SLIGHTLY GREY - that is the main sign, and the spinner only confirms it.
    //
    // What was asked for: an unobtrusive mark that a tool is being applied right now and that the
    // geometry on screen will be rebuilt once the effect fades. The veil is faint and it is PAINT, not a
    // barrier: the view and the selection work straight through it. Its disappearance, on the other
    // hand, reads at once - grey means counting, clear means here is the new geometry.
    painter.rect_filled(rect, 0.0, qymcad_scheme::a(scheme.pal.scrim(), QUIET_VEIL_ALPHA));
    let t = ctx.input(|i| i.time) as f32;
    let c = rect.center();
    for k in 0..8 {
        let a = std::f32::consts::TAU * k as f32 / 8.0;
        let phase = (t * 2.0 - k as f32 / 8.0).fract();
        let alpha = (40.0 + 215.0 * (1.0 - phase)) as u8;
        painter.circle_filled(c + egui::vec2(a.cos(), a.sin()) * 16.0, 3.0, qymcad_scheme::a(scheme.pal.text_strong(), alpha));
    }
    ctx.request_repaint();
}

/// THE SPLASH IS A SMALL WINDOW IN THE CENTRE, NOT THE WHOLE SCREEN.
///
/// What was asked for: a small centred window with an icon, a name, a spinner and a description of what
/// is happening. A full-screen splash says no more than a card does, yet it hides the whole program, and
/// on a small project it only manages to blink.
pub fn draw_splash(logo_tex: &Option<egui::TextureHandle>, scheme: &SchemeUi, ctx: &egui::Context, label: &str) {
    let screen = ctx.viewport_rect();
    modal_input_barrier(ctx, "splash_modal_barrier");
    // THE BACKGROUND GOES IN THE LOWER LAYER, THE CARD IN THE UPPER ONE. Once the fill was put into
    // `Foreground` while the card stayed an ordinary window (`Order::Middle`), and the fill painted over
    // the card: what appeared was an empty white rectangle filling the window. The layer decides who is
    // on top of whom, and "drawn later" does not mean "visible" here.
    ctx.layer_painter(egui::LayerId::new(egui::Order::Background, egui::Id::new("splash_dim"))).rect_filled(screen, 0.0, scheme.pal.splash_bg());
    egui::Area::new(egui::Id::new("splash")).order(egui::Order::Foreground).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
        egui::Frame::popup(ui.style()).show(ui, |ui| {
            ui.set_width(300.0);
            ui.vertical_centered(|ui| {
                ui.add_space(10.0);
                if let Some(tex) = &logo_tex {
                    ui.add(egui::Image::new(egui::load::SizedTexture::new(tex.id(), egui::vec2(64.0, 64.0))));
                }
                ui.add_space(8.0);
                ui.heading(egui::RichText::new("QymCAD").color(scheme.pal.text_strong()));
                ui.add_space(10.0);
                ui.add(egui::Spinner::new().size(22.0));
                ui.add_space(8.0);
                ui.label(egui::RichText::new(label).size(13.0).color(scheme.pal.text_dim()));
                ui.add_space(6.0);
            });
        });
    });
}

/// THE CARD OF A LONG IMPORT: the same card as `draw_splash`, with the time it has taken and a way out.
///
/// Reported behaviour: a big IGES assembly kept the card up with a spinner and nothing else - no sign of how long
/// it had gone, no way to stop - and it looked like a hang. Reading a file cannot be stopped half-way (the kernel
/// either reads it or does not), but the program can stop waiting for it: `cancel` gives the window back and the
/// reading, when it ends, is thrown away. Returns whether that was asked for.
pub fn draw_import_card(logo_tex: &Option<egui::TextureHandle>, scheme: &SchemeUi, ctx: &egui::Context, label: &str, elapsed: std::time::Duration, cancel: &str) -> bool {
    let screen = ctx.viewport_rect();
    modal_input_barrier(ctx, "splash_modal_barrier");
    ctx.layer_painter(egui::LayerId::new(egui::Order::Background, egui::Id::new("splash_dim"))).rect_filled(screen, 0.0, scheme.pal.splash_bg());
    let mut asked = false;
    egui::Area::new(egui::Id::new("splash")).order(egui::Order::Foreground).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
        egui::Frame::popup(ui.style()).show(ui, |ui| {
            ui.set_width(300.0);
            ui.vertical_centered(|ui| {
                ui.add_space(10.0);
                if let Some(tex) = &logo_tex {
                    ui.add(egui::Image::new(egui::load::SizedTexture::new(tex.id(), egui::vec2(64.0, 64.0))));
                }
                ui.add_space(8.0);
                ui.heading(egui::RichText::new("QymCAD").color(scheme.pal.text_strong()));
                ui.add_space(10.0);
                ui.add(egui::Spinner::new().size(22.0));
                ui.add_space(8.0);
                ui.label(egui::RichText::new(label).size(13.0).color(scheme.pal.text_dim()));
                let s = elapsed.as_secs();
                ui.label(egui::RichText::new(format!("{}:{:02}", s / 60, s % 60)).size(13.0).monospace().color(scheme.pal.text_dim()));
                ui.add_space(6.0);
                asked = ui.button(cancel).clicked();
                ui.add_space(6.0);
            });
        });
    });
    asked
}
