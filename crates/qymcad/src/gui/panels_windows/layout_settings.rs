//! THE SETTINGS OF THE LAYOUT AND OF THE BARS: the diagram of the window with its panels, the look of the bars of
//! tools, the buttons of the bar of a workbench, and the panels of the menu "View".
//!
//! Everything here is read from the shell and the catalogue: the places from what the workbenches registered, the
//! buttons from the rows a workbench gives. This file names no panel and no workbench, so a workbench added later is
//! set up here without a line of it changed - checked by a test over this file.

use super::*;
use crate::command_catalog::Named;

/// A PANEL CARRIED ON THE DIAGRAM, by its key.
struct Carried {
    key: &'static str,
}

/// A BUTTON CARRIED IN THE LIST OF A BAR, by its place in the list.
struct CarriedButton {
    index: usize,
}

/// The thickness of a strip on the diagram: at a side, and at the top or the bottom.
const SIDE_STRIP: f32 = 104.0;
const FLAT_STRIP: f32 = 30.0;

/// The height of the diagram: room for two strips at the top, one at the bottom and a stack of two at a side, every name
/// whole.
const DIAGRAM_HEIGHT: f32 = 300.0;

/// The side of a switch on a tile: the eye, the place over the view.
const SWITCH: f32 = 16.0;

/// The words on a tile, small enough for the longest name to stand in two lines of a strip at a side.
const TILE_TEXT: f32 = 11.0;

/// The band of the menu at the top of the diagram.
const MENU_BAND: f32 = 18.0;

/// The seam a panel is dropped on to stand in a strip of its own.
const SEAM: f32 = 8.0;

/// An edge's strips on the diagram, from the edge inward: the rectangles of the strips.
fn strip_rects(edge: qymcad_shell::Edge, count: usize, room: egui::Rect) -> Vec<egui::Rect> {
    (0..count)
        .map(|i| {
            let k = i as f32;
            match edge {
                qymcad_shell::Edge::Top => egui::Rect::from_min_size(egui::pos2(room.min.x, room.min.y + k * FLAT_STRIP), egui::vec2(room.width(), FLAT_STRIP)),
                qymcad_shell::Edge::Bottom => egui::Rect::from_min_size(egui::pos2(room.min.x, room.max.y - (k + 1.0) * FLAT_STRIP), egui::vec2(room.width(), FLAT_STRIP)),
                qymcad_shell::Edge::Left => egui::Rect::from_min_size(egui::pos2(room.min.x + k * SIDE_STRIP, room.min.y), egui::vec2(SIDE_STRIP, room.height())),
                qymcad_shell::Edge::Right => egui::Rect::from_min_size(egui::pos2(room.max.x - (k + 1.0) * SIDE_STRIP, room.min.y), egui::vec2(SIDE_STRIP, room.height())),
            }
        })
        .collect()
}

/// What is left inside an edge's strips.
fn inside_of(edge: qymcad_shell::Edge, count: usize, room: egui::Rect) -> egui::Rect {
    let k = count as f32;
    match edge {
        qymcad_shell::Edge::Top => egui::Rect::from_min_max(egui::pos2(room.min.x, room.min.y + k * FLAT_STRIP), room.max),
        qymcad_shell::Edge::Bottom => egui::Rect::from_min_max(room.min, egui::pos2(room.max.x, room.max.y - k * FLAT_STRIP)),
        qymcad_shell::Edge::Left => egui::Rect::from_min_max(egui::pos2(room.min.x + k * SIDE_STRIP, room.min.y), room.max),
        qymcad_shell::Edge::Right => egui::Rect::from_min_max(room.min, egui::pos2(room.max.x - k * SIDE_STRIP, room.max.y)),
    }
}

/// THE SEAM BEFORE THE STRIP `before` of an edge: a panel dropped there takes a strip of its own in that place.
fn seam(edge: qymcad_shell::Edge, strips: &[egui::Rect], before: usize, room: egui::Rect) -> egui::Rect {
    let half = SEAM / 2.0;
    // the line the seam lies on: the outer side of the strip, or the inner side of the last one
    let line = match (strips.get(before), strips.last()) {
        (Some(s), _) => match edge {
            qymcad_shell::Edge::Top => s.min.y,
            qymcad_shell::Edge::Bottom => s.max.y,
            qymcad_shell::Edge::Left => s.min.x,
            qymcad_shell::Edge::Right => s.max.x,
        },
        (None, Some(s)) => match edge {
            qymcad_shell::Edge::Top => s.max.y,
            qymcad_shell::Edge::Bottom => s.min.y,
            qymcad_shell::Edge::Left => s.max.x,
            qymcad_shell::Edge::Right => s.min.x,
        },
        (None, None) => match edge {
            qymcad_shell::Edge::Top => room.min.y + half,
            qymcad_shell::Edge::Bottom => room.max.y - half,
            qymcad_shell::Edge::Left => room.min.x + half,
            qymcad_shell::Edge::Right => room.max.x - half,
        },
    };
    if edge.upright() {
        egui::Rect::from_min_max(egui::pos2(line - half, room.min.y), egui::pos2(line + half, room.max.y))
    } else {
        egui::Rect::from_min_max(egui::pos2(room.min.x, line - half), egui::pos2(room.max.x, line + half))
    }
}

/// THE DIAGRAM OF THE WINDOW: the menu above, the view in the middle, and at each edge its strips with the panels in
/// them. A panel is dragged onto another to stand beside it in its strip, onto a seam to take a strip of its own; its
/// eye hides it; a panel at the top or the bottom may stand over the view. What changes here is seen in the window
/// at once.
pub(crate) fn layout_diagram(set: &mut qymcad_ui_state::Settings, ui: &mut egui::Ui) {
    let mut shell = crate::gui::shell(set);
    ui.label(egui::RichText::new(crate::i18n::tr("settings-layout-drag-hint")).weak().small());
    let width = ui.available_width().clamp(360.0, 640.0);
    // A BLOCK OF ITS OWN: the diagram is claimed whole first, and everything on it is laid in a child bounded by it, so
    // nothing after it can stand over it. Reported behaviour: the sliders of the bar lay over the tiles of the diagram.
    let (whole, _) = ui.allocate_exact_size(egui::vec2(width, DIAGRAM_HEIGHT), egui::Sense::hover());
    let mut block = ui.new_child(egui::UiBuilder::new().max_rect(whole).id_salt("layout-diagram"));
    block.set_clip_rect(whole.intersect(ui.clip_rect()));
    let ui = &mut block;
    let painter = ui.painter_at(whole);
    let vis = ui.visuals().clone();
    painter.rect_filled(whole, 4.0, vis.extreme_bg_color);
    let menu = egui::Rect::from_min_size(whole.min, egui::vec2(whole.width(), MENU_BAND));
    painter.rect_filled(menu, 0.0, vis.faint_bg_color);
    painter.text(
        menu.left_center() + egui::vec2(6.0, 0.0),
        egui::Align2::LEFT_CENTER,
        format!("{}  {}", ph::LOCK_SIMPLE, crate::i18n::tr("settings-layout-menu")),
        egui::FontId::proportional(TILE_TEXT),
        vis.weak_text_color(),
    );
    // the strips: the top and the bottom across the whole width, the sides between them, as the window lays them
    let mut room = egui::Rect::from_min_max(egui::pos2(whole.min.x, menu.max.y), whole.max);
    let mut target: Option<qymcad_shell::Target> = None;
    let mut seen: Option<qymcad_shell::Seen> = None;
    let mut over: Option<qymcad_shell::Over> = None;
    let mut touched: Option<&'static str> = None;
    for edge in [qymcad_shell::Edge::Top, qymcad_shell::Edge::Bottom, qymcad_shell::Edge::Left, qymcad_shell::Edge::Right] {
        let strips = shell.strips(edge);
        let rects = strip_rects(edge, strips.len(), room);
        for (si, (members, band)) in strips.iter().zip(&rects).enumerate() {
            let n = members.len().max(1) as f32;
            for (j, key) in members.iter().enumerate() {
                let k = j as f32;
                let tile = if edge.upright() {
                    egui::Rect::from_min_size(egui::pos2(band.min.x, band.min.y + k * band.height() / n), egui::vec2(band.width(), band.height() / n))
                } else {
                    egui::Rect::from_min_size(egui::pos2(band.min.x + k * band.width() / n, band.min.y), egui::vec2(band.width() / n, band.height()))
                }
                .shrink(2.0);
                let pick = tile_ui(&shell, key, tile, ui);
                if pick.seen.is_some() || pick.over.is_some() {
                    seen = pick.seen;
                    over = pick.over;
                    touched = Some(key);
                }
                // dropped on a panel: into its strip, before it or after it by the half the pointer is in; nothing joins
                // the strip of a pinned one
                if shell.pinned(key) {
                    continue;
                }
                let zone = ui.interact(tile, egui::Id::new(("layout-tile-drop", *key)), egui::Sense::hover());
                if let Some(c) = zone.dnd_release_payload::<Carried>() {
                    let at = ui.ctx().pointer_interact_pos().unwrap_or(tile.center());
                    let after = if edge.upright() { at.y > tile.center().y } else { at.x > tile.center().x };
                    target = Some(qymcad_shell::Target::Into { edge, strip: si, before: j + usize::from(after) });
                    touched = Some(c.key);
                }
                if zone.dnd_hover_payload::<Carried>().is_some() {
                    painter.rect_stroke(tile, 3.0, vis.selection.stroke, egui::StrokeKind::Inside);
                }
            }
        }
        // the seams: before each strip, and inside the last - a strip of its own there
        for before in 0..=strips.len() {
            let s = seam(edge, &rects, before, room).intersect(room);
            let zone = ui.interact(s, egui::Id::new(("layout-seam", edge, before)), egui::Sense::hover());
            if zone.dnd_hover_payload::<Carried>().is_some() {
                painter.rect_filled(s, 0.0, vis.selection.bg_fill);
            }
            if let Some(c) = zone.dnd_release_payload::<Carried>() {
                target = Some(qymcad_shell::Target::NewStrip { edge, before });
                touched = Some(c.key);
            }
        }
        room = inside_of(edge, strips.len(), room);
    }
    painter.text(room.center(), egui::Align2::CENTER_CENTER, format!("{}  {}", ph::LOCK_SIMPLE, crate::i18n::tr("settings-layout-view")), egui::FontId::proportional(12.0), vis.weak_text_color());
    if let Some(key) = touched {
        if let Some(t) = target {
            shell.move_to(key, t);
        }
        if let Some(s) = seen {
            shell.set_seen(key, s);
        }
        if let Some(o) = over {
            shell.set_over(key, o);
        }
        set.layout = shell.saved();
    }
}

/// What a tile of the diagram asked for: to be shown or hidden, to stand over the view or beside it.
struct TilePick {
    seen: Option<qymcad_shell::Seen>,
    over: Option<qymcad_shell::Over>,
}

/// ONE PANEL ON THE DIAGRAM: its whole name, wrapped where the tile is narrow, to carry it by; its eye; at the top or the
/// bottom its place over the view. A pinned panel wears a lock and is not carried.
fn tile_ui(shell: &qymcad_shell::Shell, key: &'static str, tile: egui::Rect, ui: &mut egui::Ui) -> TilePick {
    let mut pick = TilePick { seen: None, over: None };
    let Some(place) = shell.place_of(key) else { return pick };
    let name = crate::i18n::tr(&format!("panel-{key}"));
    let hidden = place.seen == qymcad_shell::Seen::Hidden;
    let pinned = shell.pinned(key);
    let vis = ui.visuals().clone();
    let mut switches = Vec::new();
    if shell.hideable(key) {
        switches.push(Switch::Eye);
    }
    if !place.edge.upright() && !pinned {
        switches.push(Switch::Over);
    }
    // the switches in a row at the top right corner; the name takes the rest of the tile
    let row = SWITCH * switches.len() as f32;
    let body = if place.edge.upright() {
        egui::Rect::from_min_max(egui::pos2(tile.min.x, tile.min.y + SWITCH), tile.max)
    } else {
        egui::Rect::from_min_max(tile.min, egui::pos2(tile.max.x - row, tile.max.y))
    };
    // A PLACE THAT NEITHER MOVES NOR HIDES is drawn as the menu is: a band in the colour of the menu with a lock
    if pinned && !shell.hideable(key) {
        ui.painter().rect_filled(tile, 0.0, vis.faint_bg_color);
        let words = format!("{}  {name}", ph::LOCK_SIMPLE);
        ui.painter().text(tile.left_center() + egui::vec2(6.0, 0.0), egui::Align2::LEFT_CENTER, words, egui::FontId::proportional(TILE_TEXT), vis.weak_text_color());
        return pick;
    }
    let carried = egui::Id::new(("layout-tile", key));
    let lifted = ui.ctx().is_being_dragged(carried);
    let fill = if hidden || lifted { vis.faint_bg_color } else { vis.widgets.inactive.weak_bg_fill };
    ui.painter().rect_filled(tile, 3.0, fill);
    let text = if pinned { format!("{} {name}", ph::LOCK_SIMPLE) } else { name.clone() };
    let words = egui::RichText::new(text).size(TILE_TEXT);
    let label = egui::Label::new(if hidden { words.weak() } else { words }).wrap().selectable(false);
    // THE NAME STAYS ON THE TILE, and what is carried is the plate under the pointer: a drag source carries its content
    // along see-through, so the source holds nothing to see
    // every part of a tile under the key of its panel: a tile laid where another stood is a new widget, not an old one
    // whose id changed
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(body.shrink(3.0)).layout(egui::Layout::left_to_right(egui::Align::Center)).id(egui::Id::new(("tile-name", key))));
    child.add(label);
    if pinned {
        ui.interact(body, carried, egui::Sense::hover()).on_hover_text(crate::i18n::tr1("settings-layout-pinned", "name", &name));
    } else {
        let mut handle = ui.new_child(egui::UiBuilder::new().max_rect(body).id(egui::Id::new(("tile-handle", key))));
        handle.dnd_drag_source(carried, Carried { key }, |ui| ui.allocate_space(body.size())).response.on_hover_text(&name);
    }
    if lifted {
        carried_plate(ui.ctx(), &name);
    }
    for (i, sw) in switches.iter().enumerate() {
        let at = egui::Rect::from_min_size(egui::pos2(tile.max.x - row + i as f32 * SWITCH, tile.min.y + 1.0), egui::vec2(SWITCH, SWITCH));
        match sw {
            Switch::Eye => {
                let glyph = if hidden { ph::EYE_SLASH } else { ph::EYE };
                let hint = if hidden { "settings-layout-show" } else { "settings-layout-hide" };
                let eye = ui.scope_builder(egui::UiBuilder::new().max_rect(at).id(egui::Id::new(("tile-eye", key))), |ui| ui.add_sized(at.size(), egui::Button::new(glyph).small().frame(false))).inner;
                if eye.on_hover_text(crate::i18n::tr1(hint, "name", &name)).clicked() {
                    pick.seen = Some(if hidden { qymcad_shell::Seen::Shown } else { qymcad_shell::Seen::Hidden });
                }
            }
            Switch::Over => {
                let on = place.over == qymcad_shell::Over::Over;
                let hint = if on { "settings-layout-beside" } else { "settings-layout-over" };
                let button = egui::Button::new(ph::STACK_SIMPLE).small().frame(false).selected(on);
                let over = ui.scope_builder(egui::UiBuilder::new().max_rect(at).id(egui::Id::new(("tile-over", key))), |ui| ui.add_sized(at.size(), button)).inner;
                if over.on_hover_text(crate::i18n::tr1(hint, "name", &name)).clicked() {
                    pick.over = Some(if on { qymcad_shell::Over::Beside } else { qymcad_shell::Over::Over });
                }
            }
        }
    }
    pick
}

/// THE PANEL IN HAND while it is carried: a small solid plate under the pointer with the name of the panel, in the
/// colours of the theme and with a shadow, kept inside the window. Reported behaviour: a see-through copy of the long
/// tile was carried, its words running out of the window.
fn carried_plate(ctx: &egui::Context, name: &str) {
    let Some(at) = ctx.pointer_interact_pos() else { return };
    let screen = ctx.content_rect();
    let pos = egui::pos2((at.x + 12.0).min(screen.max.x - 180.0).max(screen.min.x), (at.y + 12.0).min(screen.max.y - 30.0).max(screen.min.y));
    egui::Area::new(egui::Id::new("layout-carried")).order(egui::Order::Tooltip).fixed_pos(pos).interactable(false).show(ctx, |ui| {
        let frame = egui::Frame::popup(ui.style()).fill(ui.visuals().widgets.active.bg_fill.to_opaque());
        frame.show(ui, |ui| {
            ui.set_max_width(160.0);
            ui.add(egui::Label::new(egui::RichText::new(name).size(TILE_TEXT).color(ui.visuals().strong_text_color())).wrap());
        });
    });
}

/// A switch on a tile of the diagram.
enum Switch {
    Eye,
    Over,
}

/// THE LOOK OF THE BARS OF TOOLS, a group of its own under the diagram: each row a caption on the left and its field on
/// the right, in one grid.
pub(crate) fn bar_look(set: &mut qymcad_ui_state::Settings, ui: &mut egui::Ui, show: &dyn Fn(&str) -> bool) {
    ui.add_space(8.0);
    ui.label(egui::RichText::new(crate::i18n::tr("settings-layout-bar")).strong());
    egui::Grid::new("settings_bar_look").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
        if show("settings-bar-size") {
            ui.label(crate::i18n::tr("settings-bar-size"));
            let range = qymcad_ui_state::BAR_SIZES;
            ui.add(egui::Slider::new(&mut set.bar.size, range).step_by(2.0).suffix(crate::i18n::tr("unit-pt-suffix")));
            ui.end_row();
        }
        if show("settings-bar-lines") {
            ui.label(crate::i18n::tr("settings-bar-lines"));
            ui.horizontal(|ui| {
                for lines in [qymcad_shell::BarLines::One, qymcad_shell::BarLines::Two] {
                    if ui.selectable_label(set.bar.lines == lines, crate::i18n::tr(lines_word(lines))).clicked() {
                        set.bar.lines = lines;
                    }
                }
            });
            ui.end_row();
        }
        if show("settings-bar-align") {
            ui.label(crate::i18n::tr("settings-bar-align"));
            ui.horizontal(|ui| {
                for align in [qymcad_shell::BarAlign::Start, qymcad_shell::BarAlign::Centre, qymcad_shell::BarAlign::End] {
                    if ui.selectable_label(set.bar.align == align, crate::i18n::tr(align_word(align))).clicked() {
                        set.bar.align = align;
                    }
                }
            });
            ui.end_row();
        }
        if show("settings-bar-opacity") {
            ui.label(crate::i18n::tr("settings-bar-opacity"));
            ui.add(egui::Slider::new(&mut set.bar.opacity, qymcad_ui_state::BAR_OPACITY).suffix(" %"));
            ui.end_row();
        }
        if show("settings-bar-group-names") {
            ui.label(crate::i18n::tr("settings-bar-group-names"));
            let mut shown = set.bar.captions == qymcad_shell::Captions::Shown;
            if ui.checkbox(&mut shown, "").changed() {
                set.bar.captions = if shown { qymcad_shell::Captions::Shown } else { qymcad_shell::Captions::Hidden };
            }
            ui.end_row();
        }
    });
}

/// The word of a count of lines of buttons.
fn lines_word(lines: qymcad_shell::BarLines) -> &'static str {
    match lines {
        qymcad_shell::BarLines::One => "bar-lines-one",
        qymcad_shell::BarLines::Two => "bar-lines-two",
    }
}

/// The word of where the buttons stand along a bar.
fn align_word(align: qymcad_shell::BarAlign) -> &'static str {
    match align {
        qymcad_shell::BarAlign::Start => "bar-align-start",
        qymcad_shell::BarAlign::Centre => "bar-align-centre",
        qymcad_shell::BarAlign::End => "bar-align-end",
    }
}

/// THE BUTTONS OF THE BAR OF `bench`: in their order, each on the bar or in "More". A row is dragged to change the
/// order; the standard set comes back with one button.
pub(crate) fn bar_buttons(set: &mut qymcad_ui_state::Settings, ui: &mut egui::Ui, bench: &str) {
    // UNDER A FOLD OF ITS OWN, as the colours of a scheme are: the section of a workbench holds more than its bar
    egui::CollapsingHeader::new(crate::i18n::tr("settings-bar-buttons")).id_salt(("buttons of the bar", bench)).default_open(false).show(ui, |ui| bar_buttons_list(set, ui, bench));
}

/// The list of the buttons of the bar of `bench`, inside its fold.
fn bar_buttons_list(set: &mut qymcad_ui_state::Settings, ui: &mut egui::Ui, bench: &str) {
    let rows = crate::command_catalog::of_bench(bench);
    let kept = set.bar_sets.get(bench).cloned();
    let order = qymcad_ui_state::catalogue::ordered(&rows, kept.as_ref());
    let mut in_more: Vec<String> = kept.map(|k| k.in_more).unwrap_or_default();
    ui.label(egui::RichText::new(crate::i18n::tr("settings-bar-drag-hint")).weak().small());
    let mut moved: Option<Move> = None;
    let mut changed = false;
    for (i, c) in order.iter().enumerate() {
        let row = ui.horizontal(|ui| {
            ui.dnd_drag_source(egui::Id::new(("bar button", c.code)), CarriedButton { index: i }, |ui| {
                ui.label(ph::DOTS_SIX_VERTICAL);
                ui.add(qymcad_ui_state::icon_image(ui, c.icon, 18.0));
                ui.label(c.name());
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let more = in_more.iter().any(|m| m == c.code);
                if ui.selectable_label(more, crate::i18n::tr("settings-bar-in-more")).clicked() && !more {
                    in_more.push(c.code.to_string());
                    changed = true;
                }
                if ui.selectable_label(!more, crate::i18n::tr("settings-bar-on-bar")).clicked() && more {
                    in_more.retain(|m| m != c.code);
                    changed = true;
                }
            });
        });
        let r = row.response;
        if r.dnd_hover_payload::<CarriedButton>().is_some() {
            let below = ui.ctx().pointer_interact_pos().is_some_and(|p| p.y > r.rect.center().y);
            let y = if below { r.rect.max.y } else { r.rect.min.y };
            ui.painter().hline(r.rect.x_range(), y, ui.visuals().selection.stroke);
        }
        if let Some(c) = r.dnd_release_payload::<CarriedButton>() {
            let below = ui.ctx().pointer_interact_pos().is_some_and(|p| p.y > r.rect.center().y);
            moved = Some(Move { from: c.index, to: i + usize::from(below) });
        }
    }
    let mut codes: Vec<String> = order.iter().map(|c| c.code.to_string()).collect();
    if let Some(Move { from, to }) = moved {
        let code = codes.remove(from);
        codes.insert(if to > from { to - 1 } else { to }, code);
        changed = true;
    }
    if changed {
        set.bar_sets.insert(bench.to_string(), qymcad_ui_state::BarSet { order: codes, in_more });
    }
    if ui.button(format!("{}  {}", ph::ARROW_COUNTER_CLOCKWISE, crate::i18n::tr("settings-bar-reset"))).clicked() {
        set.bar_sets.remove(bench);
    }
}

/// A button of a bar carried from one place of the list to another.
struct Move {
    from: usize,
    to: usize,
}

/// THE PANELS OF THE MENU "VIEW": a tick a panel that may be hidden. The menu and the bar of the command in hand are
/// not among them.
pub(crate) fn panels_menu(set: &mut qymcad_ui_state::Settings, ui: &mut egui::Ui) {
    let mut shell = crate::gui::shell(set);
    let mut changed = false;
    for key in shell.movable() {
        if !shell.hideable(key) {
            continue;
        }
        let Some(place) = shell.place_of(key) else { continue };
        let mut shown = place.seen == qymcad_shell::Seen::Shown;
        if ui.checkbox(&mut shown, crate::i18n::tr(&format!("panel-{key}"))).changed() {
            shell.set_seen(key, if shown { qymcad_shell::Seen::Shown } else { qymcad_shell::Seen::Hidden });
            changed = true;
        }
    }
    if changed {
        set.layout = shell.saved();
    }
}

#[cfg(test)]
mod tests {
    use crate::gui::hand::Hand;

    /// The words a person reads for `key`.
    fn tr(key: &str) -> String {
        crate::i18n::tr(key)
    }

    /// A PANEL CARRIED ON THE DIAGRAM IS A SOLID PLATE UNDER THE POINTER: the frames of a real drag - the button held,
    /// the pointer led in steps - show the name of the panel near the pointer on a small opaque ground inside the window.
    /// Reported behaviour: a see-through copy of the long tile was carried, its words running out of the window.
    #[test]
    fn a_panel_carried_on_the_diagram_is_a_solid_plate_under_the_pointer() {
        let (mut app, _ctx) = crate::gui::import_door::tests::running();
        let mut hand = Hand::new(&mut app);
        let origin = egui::pos2(0.0, 0.0);
        assert!(hand.press_word(&tr("menu-windows"), origin) && hand.press_word(&tr("win-settings"), origin), "the settings do not open");
        assert!(hand.press_word(&tr("settings-sec-layout"), egui::pos2(700.0, 200.0)), "the section of the layout is not in the settings");
        let corner = hand.written_at(&tr("settings-layout-menu")).expect("the diagram is drawn").center();
        let name = tr("panel-props");
        let from = hand.written_near(&name, corner).expect("the properties stand on the diagram").center();
        let to = from + egui::vec2(-240.0, 60.0);
        let button = |pos, pressed| egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        hand.frame(vec![egui::Event::PointerMoved(from)]);
        hand.frame(vec![button(from, true)]);
        for k in 1..=20 {
            hand.frame(vec![egui::Event::PointerMoved(from + (to - from) * (k as f32 / 20.0))]);
        }
        let plate = hand.ground_of(&name, to).expect("the name of the panel is not written near the pointer while it is carried");
        hand.frame(vec![button(to, false)]);
        let screen = egui::Rect::from_min_size(origin, hand.screen());
        assert_eq!(plate.fill.a(), 255, "the plate under the pointer is see-through: {:?}", plate.fill);
        assert!(screen.contains_rect(plate.rect), "the plate {:?} runs out of the window {screen:?}", plate.rect);
        assert!(plate.rect.width() <= 200.0 && plate.rect.center().distance(to) < 120.0, "the plate {:?} is not a small one beside the pointer at {to:?}", plate.rect);
    }

    /// THE SETTINGS OF THE LAYOUT AND THE BAR OF TOOLS NAME NO PANEL AND NO WORKBENCH: they walk what the shell and the
    /// catalogue hold. A key or a code written here is a workbench that cannot be taken out, or a panel a new one would
    /// not be set up beside.
    #[test]
    fn the_layout_settings_and_the_bar_name_no_panel_and_no_workbench() {
        /// A file read and its text.
        struct Source {
            file: &'static str,
            text: &'static str,
        }
        for Source { file, text } in [
            Source { file: "layout_settings.rs", text: include_str!("layout_settings.rs") },
            Source { file: "tool_bar.rs", text: include_str!("../panels_bars/tool_bar.rs") },
        ] {
            let code = text.split("\n#[cfg(test)]").next().expect("the working part is above the tests");
            // the words written in quotes, a literal at a time
            let literals: Vec<&str> = code.split('"').skip(1).step_by(2).collect();
            for name in ["tree", "props", "wbtools", "status", "toolbar", "command_bar", "menubar", "viewport", "sketch", "part", "assembly"] {
                let named = literals.iter().any(|l| *l == name || l.starts_with(&format!("{name}.")) || l.ends_with(&format!("-{name}")));
                assert!(!named, "{file} names `{name}`: what stands in the layout and on the bars comes from the shell and the catalogue");
            }
        }
    }
}
