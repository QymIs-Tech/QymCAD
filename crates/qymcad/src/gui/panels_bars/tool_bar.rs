//! THE BAR OF THE TOOLS OF A WORKBENCH, drawn from the catalogue: a button a row of the bench open, in the order and
//! with the "More" list the person set, laid by the shell for the edge it stands at, lit while its command is in hand,
//! pressed through the same door as the search.

use super::*;
use crate::command_catalog::Named;

/// The side of an icon against the side of its button: 22 points on the button of 34 the bar had when it was fixed.
const ICON_SHARE: f32 = 22.0 / 34.0;

/// THE MARGIN ROUND THE BUTTONS OF THE BAR, the same on every side. Reported behaviour: at the top the buttons touched
/// the upper edge of the bar and stood off the lower one, at a side they touched the left edge and stood off the right.
pub(crate) const BAR_MARGIN: f32 = 3.0;

/// The size of the words of the caption of a group: a little larger than the 8.5 points of the captions the bar had before.
const CAPTION_SIZE: f32 = 11.0;

/// THE BAR OF THE WORKBENCH OPEN at `edge` (none: as at a side).
pub(crate) fn wb_toolbar(bc: &mut qymcad_ui_state::BarCtx, ui: &mut egui::Ui, edge: Option<qymcad_shell::Edge>) {
    let edge = edge.unwrap_or(qymcad_shell::Edge::Left);
    let bench = bc.workbench.code();
    // the bar of each workbench under its own key, whatever was drawn before it
    qymcad_shell::keyed(ui, ("the bar of", bench), |ui| bar_of(bc, ui, edge, bench));
}

/// THE BAR OF `bench`, laid and pressed.
fn bar_of(bc: &mut qymcad_ui_state::BarCtx, ui: &mut egui::Ui, edge: qymcad_shell::Edge, bench: &'static str) {
    use qymcad_ui_state::catalogue::{arrange, hint, pressed, Command};
    let rows = crate::command_catalog::of_bench(bench);
    let arranged = arrange(&rows, bc.set.bar_sets.get(bench));
    let look = bc.set.bar.look();
    // a group is numbered by its changes, so a group the person split by moving a button stands apart twice
    let mut group = 0;
    let items: Vec<qymcad_shell::BarItem> = arranged
        .on_bar
        .iter()
        .enumerate()
        .map(|(i, c)| {
            if i > 0 && arranged.on_bar[i - 1].group != c.group {
                group += 1;
            }
            qymcad_shell::BarItem { group }
        })
        .collect();
    let slot = if arranged.in_more.is_empty() { qymcad_shell::MoreSlot::WhenFull } else { qymcad_shell::MoreSlot::Always };
    // THE BUTTONS IN THE MIDDLE OF THE STRIP ACROSS IT: a strip shared with a wider panel leaves room on one side only
    let mut room = ui.available_rect_before_wrap();
    let spare = if edge.upright() { room.width() - look.across(edge) } else { room.height() - look.across(edge) };
    if spare > 0.0 {
        room = if edge.upright() { room.shrink2(egui::vec2(spare / 2.0, 0.0)) } else { room.shrink2(egui::vec2(0.0, spare / 2.0)) };
    }
    let plan = qymcad_shell::plan_bar(edge, &look, room, &items, slot);
    ui.allocate_rect(plan.used, egui::Sense::hover());
    for caption in &plan.captions {
        let text = crate::i18n::tr(arranged.on_bar[caption.item].group);
        let font = egui::FontId::proportional(CAPTION_SIZE);
        let colour = ui.visuals().weak_text_color();
        let galley = ui.painter().layout_no_wrap(text, font, colour);
        let at = egui::pos2(caption.rect.min.x + 1.0, caption.rect.center().y - galley.size().y / 2.0);
        ui.painter().with_clip_rect(caption.rect).galley(at, galley, colour);
    }
    let mut chosen: Option<&'static Command> = None;
    for p in &plan.on_bar {
        let c = arranged.on_bar[p.item];
        let img = qymcad_ui_state::icon_image(ui, c.icon, look.size * ICON_SHARE);
        let button = egui::Button::image(img).selected(pressed(c.launch, bc));
        let r = ui.scope_builder(place_of(bench, p.rect), |ui| ui.add_sized(p.rect.size(), button)).inner.on_hover_text(hint(c, bc));
        qymcad_shell::note_button(ui.ctx(), p.rect, qymcad_shell::Button::Plain);
        if r.clicked() {
            chosen = Some(c);
        }
    }
    let stroke = ui.visuals().widgets.noninteractive.bg_stroke;
    for line in &plan.lines {
        ui.painter().line_segment([line.min, line.max], stroke);
    }
    if let Some(at) = plan.more {
        let rest: Vec<&'static Command> = plan.in_more.iter().map(|i| arranged.on_bar[*i]).chain(arranged.in_more.iter().copied()).collect();
        let lit = rest.iter().any(|c| pressed(c.launch, bc));
        let more = egui::Button::new(egui::RichText::new(ph::DOTS_THREE).size(look.size * ICON_SHARE)).selected(lit);
        let r = ui.scope_builder(place_of(bench, at), |ui| ui.put(at, more)).inner.on_hover_text(crate::i18n::tr("bar-more-hint"));
        qymcad_shell::note_button(ui.ctx(), at, qymcad_shell::Button::More);
        egui::Popup::menu(&r).show(|ui| {
            for c in rest {
                let img = qymcad_ui_state::icon_image(ui, c.icon, 18.0);
                let row = ui.add(egui::Button::image_and_text(img, c.name()).selected(pressed(c.launch, bc))).on_hover_text(hint(c, bc));
                qymcad_shell::note_button(ui.ctx(), row.rect, qymcad_shell::Button::Plain);
                if row.clicked() {
                    chosen = Some(c);
                }
            }
        });
    }
    if let Some(c) = chosen {
        crate::command_catalog::press(c, bc);
    }
}

/// A PLACE ON THE BAR, KNOWN BY WHERE IT STANDS, for a button or for "More": a button keeps nothing between frames, which
/// is when egui says the place may be its identity. A button put into "More" leaves its place to the next one, or to
/// "More" itself; under the code of its command that would be one widget gone and another come at the same rectangle,
/// which egui marks with a red frame.
fn place_of(bench: &str, rect: egui::Rect) -> egui::UiBuilder {
    let corner = [rect.min.x.round() as i32, rect.min.y.round() as i32];
    egui::UiBuilder::new().max_rect(rect).id(egui::Id::new(("a place of the bar", bench, corner)))
}
