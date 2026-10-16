//! THE TEXT OF A DIMENSION turned with it, an angle drawn, the overlay of dimensions.

use super::*;

/// The intersection of two INFINITE lines (screen points a -> b and c -> d). None means they are parallel.
/// The arc of an angular dimension around `center`, from direction `u` to direction `v` the SHORT way (the
/// side the dimension is on, through the bisector), of radius `r` in screen pixels, with small arrowheads at
/// its ends. It shows which angle exactly the degree label refers to.
/// A TEXT CENTRED AT `center` AND TURNED BY `angle` radians about its middle.
pub fn text_turned(painter: &egui::Painter, center: Pos2, txt: String, font: egui::FontId, col: Color32, angle: f32) {
    if angle.abs() < 1e-4 {
        painter.text(center, egui::Align2::CENTER_CENTER, txt, font, col);
        return;
    }
    let galley = painter.layout_no_wrap(txt, font, col);
    let half = galley.size() / 2.0;
    let (c, s) = (angle.cos(), angle.sin());
    // the text turns about its top-left corner, so that corner is put where the turned middle lands on `center`
    let corner = center - egui::vec2(half.x * c - half.y * s, half.x * s + half.y * c);
    painter.add(egui::epaint::TextShape::new(corner, galley, col).with_angle(angle));
}

/// AN ANGULAR DIMENSION: the arc at its radius - run on past a side to a label placed beyond it - with arrowheads where
/// it meets the sides, the extension lines carrying a side to the arc, and the label outside the arc.
pub fn draw_angle_dim(painter: &egui::Painter, g: &qymcad_ui_state::AngleDim, txt: String, font: egui::FontId, col: Color32) {
    let at = |a: f32| g.center + egui::vec2(a.cos(), a.sin()) * g.r;
    let (from, to) = g.arc;
    let n = ((to - from).abs() / 0.05).ceil().clamp(8.0, 96.0) as usize;
    let pts: Vec<Pos2> = (0..=n).map(|i| at(from + (to - from) * (i as f32 / n as f32))).collect();
    painter.add(egui::Shape::line(pts, Stroke::new(1.1, col)));
    for e in &g.ext {
        painter.line_segment(*e, Stroke::new(0.7, col));
    }
    // the arrowheads stand on the sides and point at them from inside the angle
    let head = |a: f32, dir: f32| {
        let p = at(a);
        let tan = egui::vec2(-a.sin(), a.cos()) * dir;
        let back = (g.center - p).normalized();
        for s in [-1.0, 1.0] {
            painter.line_segment([p, p + (tan + back * s) * 5.0], Stroke::new(1.0, col));
        }
    };
    if g.sweep.abs() > 0.05 {
        head(g.a0, g.sweep.signum());
        head(g.a0 + g.sweep, -g.sweep.signum());
    }
    painter.text(g.label, egui::Align2::CENTER_CENTER, txt, font, col);
}

/// The same, but with a COUNTER: `(done, total)` timeline nodes.
///
/// A spinner on its own says one thing - "busy". On an assembly that takes seconds to compute a person
/// needs something else: how much is left and whether they can change their mind. Both answers come from
/// one place (the rebuild loop in the core, see `RegenWatch`), which is why they are shown together.
pub fn draw_dim_overlay_with(scheme: &SchemeUi, ctx: &egui::Context, label: &str, progress: Option<NodeCount>, live: egui::Rect) -> bool {
    modal_input_barrier_except(ctx, live);
    paint_dim_overlay(scheme, ctx, label, progress, live)
}

/// HOW DARK THE DIMMING OF A LONG REBUILD IS, out of 255.
pub const DIM_ALPHA: u8 = 120;

/// THE DIMMING AND THE CARD OF A LONG REBUILD, with no barrier: the barrier goes up from the first frame of the rebuild,
/// the paint only once it has run long enough to be worth it (`qymcad_ui_state::rebuild_sign`). Returns `true` if the
/// cancel button was pressed.
pub fn paint_dim_overlay(scheme: &SchemeUi, ctx: &egui::Context, label: &str, progress: Option<NodeCount>, live: egui::Rect) -> bool {
    let screen = ctx.viewport_rect();
    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("regen_dim")));
    // WHAT IS TO BE SHOWN IS MEASURED FIRST, and the card is then made to fit it. It used to be a
    // fixed 280 px wide with the label painted centred inside: a long line ("Rebuilding 28 nodes, one
    // of them a thread") ran out past both edges of the card and stood on the dimmed viewport.
    let prog_text = progress.map(|n| qymcad_i18n::tr2("io-rebuild-progress", "done", &n.done.to_string(), "total", &n.total.to_string()));
    let wrap = (screen.width() - 96.0).clamp(200.0, 520.0); // wraps rather than growing off a narrow window
    let title = painter.layout(label.to_owned(), egui::FontId::proportional(15.0), scheme.pal.text_strong(), wrap);
    let prog = prog_text.map(|t| painter.layout(t, egui::FontId::proportional(13.0), scheme.pal.text_dim(), wrap));
    let size = regen_card_size(title.size(), prog.as_ref().map(|g| g.size()), progress.is_some());

    // THE VIEWPORT STAYS LIVE AND UNDIMMED when it is given (`live`): the model is visible, the view
    // orbits, selection works. What gets dimmed and muted is exactly where the document is changed from.
    let card = if live.is_positive() {
        for r in [
            egui::Rect::from_min_max(screen.min, egui::pos2(screen.max.x, live.min.y)),
            egui::Rect::from_min_max(egui::pos2(screen.min.x, live.max.y), screen.max),
            egui::Rect::from_min_max(egui::pos2(screen.min.x, live.min.y), egui::pos2(live.min.x, live.max.y)),
            egui::Rect::from_min_max(egui::pos2(live.max.x, live.min.y), egui::pos2(screen.max.x, live.max.y)),
        ] {
            if r.is_positive() {
                painter.rect_filled(r, 0.0, qymcad_scheme::a(scheme.pal.scrim(), DIM_ALPHA));
            }
        }
        // the card sits AT THE BOTTOM of the viewport rather than in its centre: in the centre it would
        // cover exactly what the live window was kept for - the model itself
        egui::Rect::from_center_size(egui::pos2(live.center().x, live.max.y - size.y / 2.0 - 16.0), size)
    } else {
        painter.rect_filled(screen, 0.0, qymcad_scheme::a(scheme.pal.scrim(), DIM_ALPHA));
        egui::Rect::from_center_size(screen.center(), size)
    };
    let places = regen_card_places(card, title.size(), prog.as_ref().map(|g| g.size()), progress.is_some());
    painter.rect_filled(card, 10.0, scheme.pal.panel_bg());
    painter.rect_stroke(card, 10.0, egui::Stroke::new(1.0, scheme.pal.panel_border()), egui::StrokeKind::Middle);
    painter.galley(places.title.min, title, scheme.pal.text_strong());
    let mut cancelled = false;
    // A NODE COUNT rather than a percentage: nodes are the timeline's unit of work, and a fraction of it
    // would lie - a thread takes seconds, a datum is instant.
    if let (Some(g), Some(at)) = (prog, places.progress) {
        painter.galley(at.min, g, scheme.pal.text_dim());
    }
    // THE BUTTON IS A REAL WIDGET AND SITS ABOVE THE BARRIER. The barrier mutes input across the whole
    // screen, so the button's area is created AFTER it: otherwise the click would go to the barrier and
    // the button would stay silent.
    if let Some(bt) = places.button {
        egui::Area::new(egui::Id::new("regen_cancel")).order(egui::Order::Tooltip).fixed_pos(bt.min).show(ctx, |ui| {
            ui.set_max_size(bt.size());
            if ui.add_sized(bt.size(), egui::Button::new(format!("{} {}", ph::X, qymcad_i18n::tr("io-rebuild-cancel")))).clicked() {
                cancelled = true;
            }
        });
    }
    // the "spinner" is drawn by hand: dots around a circle with a running brightness - no widget, over everything
    let t = ctx.input(|i| i.time) as f32;
    for k in 0..8 {
        let a = std::f32::consts::TAU * k as f32 / 8.0;
        let phase = (t * 2.0 - k as f32 / 8.0).fract();
        let alpha = (40.0 + 215.0 * (1.0 - phase)) as u8;
        painter.circle_filled(places.spinner + egui::vec2(a.cos(), a.sin()) * 13.0, 2.6, qymcad_scheme::a(scheme.pal.text_strong(), alpha));
    }
    ctx.request_repaint();
    cancelled
}

/// The overlay for long work. Returns `true` if the cancel button was pressed.
///
/// Only the tests call it - the program draws the overlay with the counter (`_with`). It is kept because
/// a test must draw with THE SAME code as the program rather than with a copy of its own.
/// The overlay with no progress and no live rectangle - the shape the checks use.
pub fn draw_dim_overlay(scheme: &SchemeUi, ctx: &egui::Context, label: &str) -> bool {
    draw_dim_overlay_with(scheme, ctx, label, None, egui::Rect::NOTHING)
}
