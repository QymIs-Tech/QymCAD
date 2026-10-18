//! THE SHELL DRAWN: the strips of each edge as egui panels, the panels of a strip one after another with a border
//! that is dragged, and the panels standing over the view.

use super::*;

/// The gap between two panels of one strip; the border is dragged in it.
const GAP: f32 = 7.0;

/// THE THICKNESS OF A STRIP AT A SIDE when none of its panels says one: a bar made for the top, put at a side, is as
/// wide as its row of fields otherwise, and the view is squeezed to nothing.
const SIDE: f32 = 240.0;

/// The least length a panel of a strip is squeezed to, so that a border dragged too far still leaves it a handle.
const LEAST: f32 = 40.0;

/// WHERE A PANEL WAS LAID OUT in the last frame that laid out panels, kept for whoever looks at the window from
/// outside.
#[derive(Clone)]
struct Laid {
    key: &'static str,
    rect: egui::Rect,
    pass: u64,
}

/// WHERE THE PANEL `key` STANDS, as the last frame laid it out; `None` when that frame did not draw it.
pub fn laid_out(ctx: &egui::Context, key: &str) -> Option<egui::Rect> {
    let laid: Vec<Laid> = ctx.data(|d| d.get_temp(laid_id())).unwrap_or_default();
    let last = laid.iter().map(|l| l.pass).max()?;
    laid.iter().find(|l| l.key == key && l.pass == last).map(|l| l.rect)
}

/// The last pass that laid out any panel.
pub(crate) fn last_pass(ctx: &egui::Context) -> Option<u64> {
    let laid: Vec<Laid> = ctx.data(|d| d.get_temp(laid_id())).unwrap_or_default();
    laid.iter().map(|l| l.pass).max()
}

fn laid_id() -> egui::Id {
    egui::Id::new("shell-laid-out")
}

/// Write down where a panel was laid out in this pass, dropping what older passes wrote.
fn note(ctx: &egui::Context, key: &'static str, rect: egui::Rect) {
    let pass = ctx.cumulative_pass_nr();
    ctx.data_mut(|d| {
        let laid: &mut Vec<Laid> = d.get_temp_mut_or_default(laid_id());
        laid.retain(|l| l.pass == pass && l.key != key);
        laid.push(Laid { key, rect, pass });
    });
}

/// A panel laid over the view at one of its edges, as the last frame laid it.
#[derive(Clone, Copy)]
struct LaidOver {
    edge: Edge,
    rect: egui::Rect,
}

/// Where the panels laid over the view are kept from one frame to the next.
fn over_id() -> egui::Id {
    egui::Id::new("the panels laid over the view")
}

/// THE VIEW CLEAR OF WHAT LIES OVER IT: `view` less the panels the last frame laid over it at its edges - a bar at the
/// top takes the top off, one at the right the right side. What is drawn in a corner of the view, as the cube of the
/// view is, stands there and not under a bar seen through.
pub fn clear_of_overs(ctx: &egui::Context, view: egui::Rect) -> egui::Rect {
    let laid: Vec<LaidOver> = ctx.data(|d| d.get_temp(over_id())).unwrap_or_default();
    laid.iter().filter(|o| o.rect.intersects(view)).fold(view, |v, o| match o.edge {
        Edge::Top => egui::Rect::from_min_max(egui::pos2(v.min.x, v.min.y.max(o.rect.max.y)), v.max),
        Edge::Bottom => egui::Rect::from_min_max(v.min, egui::pos2(v.max.x, v.max.y.min(o.rect.min.y))),
        Edge::Left => egui::Rect::from_min_max(egui::pos2(v.min.x.max(o.rect.max.x), v.min.y), v.max),
        Edge::Right => egui::Rect::from_min_max(v.min, egui::pos2(v.max.x.min(o.rect.min.x), v.max.y)),
    })
}

/// WHAT A PLACE DRAWS, UNDER THE ID OF ITS OWN KEY: the ids of all it draws come from `id` alone.
///
/// A scope seeded the usual way (`push_id`, `UiBuilder::id_salt`) mixes the salt with the parent's id for what it
/// keeps, but gives its widgets ids that also carry the count of what the parent drew before it (`Ui::unique_id`).
/// Whatever stands in a strip, a stack, a list of tiles or a line whose other parts come and go is drawn after a
/// different count from one frame to the next, and egui marks its widgets with a red frame - "the rectangle changed
/// its id". An explicit id (`UiBuilder::id`) is what egui gives for "moved in the tree of the window without losing
/// its state"; `id` must then be drawn once a frame.
pub fn keyed<R>(ui: &mut egui::Ui, id: impl egui::AsId, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    ui.scope_builder(egui::UiBuilder::new().id(egui::Id::new(id)), add).inner
}

pub(crate) fn run_slot(shell: &Shell, slot: Slot, ui: &mut egui::Ui, host: &mut dyn Fills) {
    match slot {
        Slot::Menu => {
            for p in shell.places.iter().filter(|p| p.slot == Slot::Menu) {
                if host.live(p.key) {
                    let r = egui::Panel::top(p.key).show(ui, |ui| host.fill(p.key, ui, None));
                    note(ui.ctx(), p.key, r.response.rect);
                }
            }
        }
        Slot::Centre => {
            for p in shell.places.iter().filter(|p| p.slot == Slot::Centre) {
                if host.live(p.key) {
                    let r = egui::CentralPanel::default().show(ui, |ui| host.fill(p.key, ui, None));
                    note(ui.ctx(), p.key, r.response.rect);
                    // over the view itself, not over the margin of the middle round it: a bar laid on that margin
                    // shows a dark band under it, not the model
                    let margin = egui::Frame::central_panel(ui.style()).total_margin();
                    let inside = egui::Rect::from_min_max(r.response.rect.min + margin.left_top(), r.response.rect.max - margin.right_bottom());
                    over_the_view(shell, inside, ui, host);
                }
            }
        }
        _ => {
            let Some(edge) = slot.edge() else { return };
            // EVERY STRIP TAKES ITS NUMBER, drawn or not, and every edge the same count of them: egui numbers a panel by
            // the panels opened before it, and a bar of a command coming up at the top, or the tree giving way in a
            // sketch, renumbered every panel after it. egui then marks each with a red frame - "the widget changed its
            // id" - in the frames of the change. Measured on the status line on entering a sketch: 17 frames with
            // another id. egui does the same for a panel it hides (`skip_ahead_auto_ids`).
            let mut taken = 0;
            for (index, strip) in shell.strips(edge).into_iter().enumerate() {
                let members: Vec<&'static str> = strip.into_iter().filter(|k| drawn_beside(shell, k, host)).collect();
                if members.is_empty() {
                    ui.skip_ahead_auto_ids(1);
                } else {
                    one_strip(Strip { edge, index }, shell, &members, ui, host);
                }
                taken += 1;
            }
            let all = shell.places.iter().filter(|p| p.slot.edge().is_some()).count();
            ui.skip_ahead_auto_ids(all.saturating_sub(taken));
        }
    }
}

/// Is the panel drawn in a strip this frame: wanted, shown and not over the view?
fn drawn_beside(shell: &Shell, key: &'static str, host: &dyn Fills) -> bool {
    shell.place_of(key).is_some_and(|p| p.seen == Seen::Shown && !stands_over(p, key, shell)) && host.live(key)
}

/// DOES THE PANEL STAND OVER THE VIEW: asked to, or seen through. A translucent panel beside the view would show the
/// empty window under it, not the model; laid over the view it shows the model, which is what the transparency is for.
fn stands_over(p: PanelPlace, key: &'static str, shell: &Shell) -> bool {
    p.over == Over::Over || shell.registered(key).is_some_and(|r| r.opacity < 1.0)
}

/// THE FRAME OF A PLACE: the host's bar frame or the plain one of a panel, with the place's own margin.
fn frame_of(regs: &[&Place], host: &dyn Fills, style: &egui::Style) -> egui::Frame {
    let framed = regs.iter().all(|p| p.framed);
    let frame = if framed { host.bar_frame() } else { egui::Frame::side_top_panel(style) };
    match regs.iter().find_map(|p| p.margin) {
        Some(m) => frame.inner_margin(egui::Margin::same(m.round() as i8)),
        None => frame,
    }
}

/// THE GROUND OF A PLACE with the opacity the host asks for.
fn ground(frame: egui::Frame, opacity: f32) -> egui::Frame {
    if opacity >= 1.0 {
        return frame;
    }
    frame.fill(frame.fill.gamma_multiply(opacity.clamp(0.0, 1.0)))
}

/// A strip of an edge: the edge and its number from the edge inward.
#[derive(Clone, Copy)]
struct Strip {
    edge: Edge,
    index: usize,
}

/// ONE STRIP: an egui panel at the edge, and in it the panels of the strip.
fn one_strip(strip: Strip, shell: &Shell, members: &[&'static str], ui: &mut egui::Ui, host: &mut dyn Fills) {
    let Strip { edge, index } = strip;
    let regs: Vec<&Place> = members.iter().filter_map(|k| shell.registered(k)).collect();
    let first = members[0];
    // THE PANEL IS THE STRIP, the n-th from the edge, whatever stands in it: a panel joining it or leaving it does not
    // make another panel of the place - which egui would mark with a red frame, and which would forget its width
    let id = egui::Id::new(("shell-strip", edge, index));
    let kept = members.iter().find_map(|k| shell.place_of(k).and_then(|p| p.across));
    let registered = regs.iter().filter_map(|p| p.size).reduce(f32::max);
    let resizable = regs.iter().any(|p| p.resizable);
    let mut panel = match edge {
        Edge::Left => egui::Panel::left(id),
        Edge::Right => egui::Panel::right(id),
        Edge::Top => egui::Panel::top(id),
        Edge::Bottom => egui::Panel::bottom(id),
    };
    let frame = frame_of(&regs, host, ui.style());
    let margin = frame.total_margin().sum();
    let content = regs.iter().filter_map(|p| p.content).map(|c| if edge.upright() { c.side + margin.x } else { c.flat + margin.y }).reduce(f32::max);
    let registered = registered.or(content);
    let side_default = registered.is_none() && edge.upright();
    let resizable = resizable || side_default;
    let size = kept.or(registered).or(side_default.then_some(SIDE));
    if let Some(w) = size {
        panel = if resizable { panel.default_size(w).resizable(true) } else { panel.exact_size(w).resizable(false) };
    }
    panel = panel.frame(ground(frame, regs.iter().map(|p| p.opacity).fold(1.0, f32::min)));
    if regs.iter().all(|p| !p.separator) {
        panel = panel.show_separator_line(false);
    }
    let r = panel.show(ui, |ui| {
        if let [only] = members {
            // under the key of the panel, as in a stack: a panel taken into a stack or out of one keeps the ids of what
            // it draws, and egui sees no widget change its id where it stands
            keyed(ui, ("shell-member", *only), |ui| host.fill(only, ui, Some(edge)));
            Vec::new()
        } else {
            stack(shell, edge, members, ui, host)
        }
    });
    let outer = r.response.rect;
    if let [only] = members {
        note(ui.ctx(), only, outer);
    } else {
        // a panel of a stack reaches across to the edges of the strip, and along to the middle of the border on
        // either side of it: that is where a person sees it end and where the border is taken
        let spans: Vec<egui::Rect> = r.inner;
        for (i, (k, span)) in members.iter().zip(&spans).enumerate() {
            let (lo, hi) = if edge.upright() { (outer.min.y, outer.max.y) } else { (outer.min.x, outer.max.x) };
            let start = if i == 0 { lo } else { (if edge.upright() { span.min.y } else { span.min.x }) - GAP / 2.0 };
            let end = if i + 1 == members.len() { hi } else { (if edge.upright() { span.max.y } else { span.max.x }) + GAP / 2.0 };
            let rect = if edge.upright() { egui::Rect::from_x_y_ranges(outer.x_range(), start..=end) } else { egui::Rect::from_x_y_ranges(start..=end, outer.y_range()) };
            note(ui.ctx(), k, rect);
        }
    }
    // A BORDER DRAGGED is kept in the settings; the size egui settles on by itself is not, or every start would write
    // a record of a layout nobody changed
    if resizable && ui.ctx().input(|i| i.pointer.primary_down()) {
        let now = if edge.upright() { outer.width() } else { outer.height() };
        if size.is_none_or(|s| (s - now).abs() > 0.5) {
            host.resized(first, Resize::Across(now));
        }
    }
}

/// The length a panel takes along its strip.
#[derive(Clone, Copy)]
enum Length {
    /// What its content needs, as the last frame measured it; not yet measured on the first frame.
    Content(Option<f32>),
    /// The length the person dragged it to.
    Kept(f32),
    /// A share of what is left.
    Share,
}

/// THE PANELS OF A STRIP one after another along the edge, with a dragged border between them; where each was laid.
fn stack(shell: &Shell, edge: Edge, members: &[&'static str], ui: &mut egui::Ui, host: &mut dyn Fills) -> Vec<egui::Rect> {
    let full = ui.available_rect_before_wrap();
    let upright = edge.upright();
    let total = if upright { full.height() } else { full.width() };
    let room = (total - GAP * (members.len() - 1) as f32).max(0.0);
    let natural_id = |k: &str| egui::Id::new(("shell-natural", k));
    let wants: Vec<Length> = members
        .iter()
        .map(|k| {
            let reg = shell.registered(k).is_some_and(|p| p.resizable);
            match shell.place_of(k).and_then(|p| p.along) {
                Some(v) => Length::Kept(v),
                None if reg => Length::Share,
                None => Length::Content(ui.ctx().data(|d| d.get_temp::<f32>(natural_id(k)))),
            }
        })
        .collect();
    let lengths = share_out(&wants, room);
    let mut at = if upright { full.min.y } else { full.min.x };
    let mut rects = Vec::new();
    for (k, len) in members.iter().zip(&lengths) {
        let rect = if upright {
            egui::Rect::from_min_size(egui::pos2(full.min.x, at), egui::vec2(full.width(), *len))
        } else {
            egui::Rect::from_min_size(egui::pos2(at, full.min.y), egui::vec2(*len, full.height()))
        };
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(*ui.layout()).id(egui::Id::new(("shell-member", *k))));
        child.set_clip_rect(rect.intersect(ui.clip_rect()));
        host.fill(k, &mut child, Some(edge));
        let used = child.min_rect();
        ui.ctx().data_mut(|d| d.insert_temp(natural_id(k), if upright { used.height() } else { used.width() }));
        rects.push(rect);
        at += len + GAP;
    }
    for (i, before) in rects.iter().enumerate().take(members.len().saturating_sub(1)) {
        border(edge, members, &lengths, i, *before, ui, host);
    }
    // the content of a strip of bars is as tall as its tallest bar; a strip of resizable panels fills what it was given
    let thick = rects.iter().map(|r| if upright { r.width() } else { r.height() }).fold(0.0, f32::max);
    let claim = if upright { egui::Rect::from_min_size(full.min, egui::vec2(thick, total)) } else { egui::Rect::from_min_size(full.min, egui::vec2(total, thick)) };
    ui.allocate_rect(claim, egui::Sense::hover());
    rects
}

/// SHARE THE ROOM OF A STRIP: what was kept and what the content measured first, the rest equally among the panels
/// that share, and the last panel takes whatever is left so the strip is filled to its end.
fn share_out(wants: &[Length], room: f32) -> Vec<f32> {
    let n = wants.len();
    let sharing = wants.iter().filter(|w| matches!(w, Length::Share | Length::Content(None))).count();
    let fixed: f32 = wants
        .iter()
        .map(|w| match w {
            Length::Kept(v) | Length::Content(Some(v)) => *v,
            _ => 0.0,
        })
        .sum();
    let each = if sharing > 0 { ((room - fixed) / sharing as f32).max(LEAST) } else { 0.0 };
    let mut out: Vec<f32> = wants
        .iter()
        .map(|w| match w {
            Length::Kept(v) | Length::Content(Some(v)) => v.max(LEAST),
            _ => each,
        })
        .collect();
    // no panel is pushed out of the strip: what is over is taken from the longest, never below the least
    let mut over = out.iter().sum::<f32>() - room;
    while over > 0.5 {
        let Some(i) = (0..n).max_by(|a, b| out[*a].total_cmp(&out[*b])) else { break };
        let cut = over.min(out[i] - LEAST);
        if cut <= 0.0 {
            break;
        }
        out[i] -= cut;
        over -= cut;
    }
    // the last panel reaches the end of the strip
    let short = room - out.iter().sum::<f32>();
    if short > 0.0 {
        if let Some(last) = out.last_mut() {
            *last += short;
        }
    }
    out
}

/// THE BORDER after the panel `i` of a strip: a hairline, and a handle the person drags to change the lengths.
fn border(edge: Edge, members: &[&'static str], lengths: &[f32], i: usize, before: egui::Rect, ui: &mut egui::Ui, host: &mut dyn Fills) {
    let upright = edge.upright();
    let gap = if upright {
        egui::Rect::from_min_max(egui::pos2(before.min.x, before.max.y), egui::pos2(before.max.x, before.max.y + GAP))
    } else {
        egui::Rect::from_min_max(egui::pos2(before.max.x, before.min.y), egui::pos2(before.max.x + GAP, before.max.y))
    };
    let r = ui.interact(gap, egui::Id::new(("shell-border", members[i])), egui::Sense::drag());
    let stroke = if r.hovered() || r.dragged() { ui.visuals().widgets.active.bg_stroke } else { ui.visuals().widgets.noninteractive.bg_stroke };
    if upright {
        ui.painter().hline(gap.x_range(), gap.center().y, stroke);
    } else {
        ui.painter().vline(gap.center().x, gap.y_range(), stroke);
    }
    if r.hovered() || r.dragged() {
        ui.ctx().set_cursor_icon(if upright { egui::CursorIcon::ResizeVertical } else { egui::CursorIcon::ResizeHorizontal });
    }
    if r.dragged() {
        let d = if upright { r.drag_delta().y } else { r.drag_delta().x };
        if d != 0.0 {
            host.resized(members[i], Resize::Along((lengths[i] + d).max(LEAST)));
        }
    }
}

/// THE PANELS STANDING OVER THE VIEW: at its top edge going down, at its bottom edge going up, at its sides going inward;
/// the view under them is not narrowed, and their ground lets it show through as much as the host says.
fn over_the_view(shell: &Shell, view: egui::Rect, ui: &mut egui::Ui, host: &mut dyn Fills) {
    let mut laid_over = Vec::new();
    for edge in Edge::ALL {
        let mut room = view;
        for k in shell.strips(edge).into_iter().flatten() {
            let wanted = shell.place_of(k).is_some_and(|p| p.seen == Seen::Shown && stands_over(p, k, shell));
            if !wanted || !host.live(k) {
                continue;
            }
            let Some(reg) = shell.registered(k) else { continue };
            let frame = ground(frame_of(&[reg], host, ui.style()), reg.opacity);
            let margin = frame.total_margin().sum();
            // across the edge: what the content says it takes, or what was registered, or the content's own
            let across = reg.content.map(|c| if edge.upright() { c.side + margin.x } else { c.flat + margin.y }).or(reg.size);
            let (pivot, pos) = match edge {
                Edge::Top => (egui::Align2::LEFT_TOP, room.left_top()),
                Edge::Bottom => (egui::Align2::LEFT_BOTTOM, room.left_bottom()),
                Edge::Left => (egui::Align2::LEFT_TOP, room.left_top()),
                Edge::Right => (egui::Align2::RIGHT_TOP, room.right_top()),
            };
            let r = egui::Area::new(egui::Id::new(("shell-over", k))).order(egui::Order::Middle).pivot(pivot).fixed_pos(pos).constrain(false).show(ui.ctx(), |ui| {
                let inner = if edge.upright() {
                    egui::vec2(across.unwrap_or(SIDE) - margin.x, room.height() - margin.y)
                } else {
                    egui::vec2(room.width() - margin.x, across.map_or(0.0, |a| a - margin.y))
                };
                frame.show(ui, |ui| {
                    ui.set_min_size(inner);
                    if edge.upright() {
                        ui.set_max_size(inner);
                    } else {
                        ui.set_max_width(inner.x);
                    }
                    host.fill(k, ui, Some(edge));
                });
            });
            let rect = r.response.rect;
            note(ui.ctx(), k, rect);
            laid_over.push(LaidOver { edge, rect });
            room = match edge {
                Edge::Top => egui::Rect::from_min_max(egui::pos2(room.min.x, rect.max.y), room.max),
                Edge::Bottom => egui::Rect::from_min_max(room.min, egui::pos2(room.max.x, rect.min.y)),
                Edge::Left => egui::Rect::from_min_max(egui::pos2(rect.max.x, room.min.y), room.max),
                Edge::Right => egui::Rect::from_min_max(room.min, egui::pos2(rect.min.x, room.max.y)),
            };
        }
    }
    ui.ctx().data_mut(|d| d.insert_temp(over_id(), laid_over));
}
