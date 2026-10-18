//! MAKE A PIECE A PART: the right button on one of the bodies a part shows - a piece a cut through it or a split left -
//! offers to make that body a part of its own. Only by hand: a cut leaves the pieces bodies of the one part, as the
//! professional systems do, and a part that came and went with every edit of the cut would take its mates and
//! references with it.
//! The item asks for the new part's name beside the cursor, the name of the part it came from with "piece" put
//! after it; Enter makes the part, Esc lets it go.
use qymcad_ui_state::Notice;
use egui::{Pos2, Rect, Response};
use qymcad_core::model::{Id, Project};

/// The mark of what the right button was pressed on, in egui's memory.
struct Under;

/// What the item was pressed on, while its name is being typed.
#[derive(Clone)]
struct Asking {
    body: Id,
    name: String,
    pos: Pos2,
}

/// The piece and the name typed, once Enter confirms. Only with no command in hand (under a command the right button
/// widens its selection) and only on a body its part shows beside another (see `Project::may_be_made_a_part`).
pub(crate) fn menu(pn: &qymcad_ui_state::Painting, resp: &Response, rect: Rect) -> Option<(Id, String)> {
    let under_id = resp.id.with(std::any::TypeId::of::<Under>());
    let ask_id = ask_id();
    if pn.armed.cmd_kind() == 0 && resp.secondary_clicked() {
        let pos = resp.interact_pointer_pos().filter(|p| rect.contains(*p));
        let hit = pos.and_then(|p| qymcad_pick::pick_face_ray(pn, rect, p));
        let under = hit.filter(|(b, _, _)| pn.project.may_be_made_a_part(*b)).map(|(b, _, _)| (b, pos.unwrap_or_default()));
        resp.ctx.data_mut(|d| d.insert_temp(under_id, under));
    }
    // the command taken from the search with nothing picked: a click on a piece takes it, Esc lets the command go
    if resp.ctx.data(|d| d.get_temp::<bool>(waiting_id())).unwrap_or(false) {
        if resp.ctx.input(|i| i.key_pressed(egui::Key::Escape)) || pn.armed.cmd_kind() != 0 {
            resp.ctx.data_mut(|d| d.remove::<bool>(waiting_id()));
        } else if resp.clicked() {
            let pos = resp.interact_pointer_pos().filter(|p| rect.contains(*p));
            if let Some((body, _, _)) = pos.and_then(|p| qymcad_pick::pick_face_ray(pn, rect, p)).filter(|(b, _, _)| pn.project.may_be_made_a_part(*b)) {
                resp.ctx.data_mut(|d| d.remove::<bool>(waiting_id()));
                ask(&resp.ctx, pn.project, body, pos.unwrap_or_default());
            }
        }
    }
    let under: Option<(Id, Pos2)> = resp.ctx.data(|d| d.get_temp(under_id)).flatten();
    if let Some((body, pos)) = under.filter(|_| pn.armed.cmd_kind() == 0) {
        resp.context_menu(|ui| {
            if ui.button(format!("{} {}", egui_phosphor::regular::CUBE, crate::i18n::tr("act-piece-to-part"))).clicked() {
                ask(&resp.ctx, pn.project, body, pos);
                ui.close();
            }
        });
    }
    let mut asking: Option<Asking> = resp.ctx.data(|d| d.get_temp(ask_id)).flatten();
    let a = asking.as_mut()?;
    let mut done = None;
    let mut gone = false;
    egui::Area::new(ask_id).order(qymcad_ui_state::AT_GEOMETRY).fixed_pos(a.pos).show(&resp.ctx, |ui| {
        qymcad_ui_state::at_geometry_ui(ui);
        egui::Frame::popup(ui.style()).show(ui, |ui| {
            // the caption beside the field, on its line, as every field of the program stands
            let field = ui
                .horizontal(|ui| {
                    ui.label(crate::i18n::tr("piece-part-name"));
                    ui.add(egui::TextEdit::singleline(&mut a.name).desired_width(220.0))
                })
                .inner;
            field.request_focus();
            // an empty name goes on too, to be refused in words - the field stays open for another
            if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                done = Some((a.body, a.name.trim().to_string()));
            } else if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                gone = true;
            }
            ui.label(egui::RichText::new(crate::i18n::tr("piece-part-name-hint")).weak());
        });
    });
    let given = done.as_ref().is_some_and(|(_, n)| !n.is_empty());
    let keep = if given || gone { None } else { asking };
    resp.ctx.data_mut(|d| d.insert_temp(ask_id, keep));
    done
}

/// Where it is kept that the command, taken from the search with nothing picked, waits for a click on a piece.
fn waiting_id() -> egui::Id {
    egui::Id::new("piece_part_waiting")
}

/// THE COMMAND TAKEN FROM THE SEARCH: a body picked before it - one of the several its part shows - is taken at once
/// and its name asked; with none picked the command waits for a click on such a body, and the status line says so.
pub(crate) fn from_search(ctx: &egui::Context, pc: &mut qymcad_ui_state::PartCtx, action: &str) {
    if action != "piece" {
        return;
    }
    let picked = qymcad_ui_state::selected_body(pc.project, pc.sel).filter(|b| pc.project.may_be_made_a_part(*b));
    match picked {
        Some(body) => {
            let at = ctx.pointer_latest_pos().unwrap_or_else(|| ctx.content_rect().center());
            ask(ctx, pc.project, body, at);
        }
        None => {
            ctx.data_mut(|d| d.insert_temp(waiting_id(), true));
            *pc.status = Notice::info(crate::i18n::tr("piece-part-pick"));
        }
    }
}

/// Where the name being asked is kept: one place, whether the item was pressed on the canvas or on a row of the tree.
fn ask_id() -> egui::Id {
    egui::Id::new(std::any::TypeId::of::<Asking>())
}

/// ASK THE NAME OF THE PART `body` IS TO BECOME, beside `pos` - from the item on the canvas or on the body's row of the
/// tree alike. The field is drawn with the canvas, by [`menu`], which also answers once Enter confirms.
pub(crate) fn ask(ctx: &egui::Context, project: &Project, body: Id, pos: Pos2) {
    let name = piece_name(project, body);
    ctx.data_mut(|d| d.insert_temp(ask_id(), Some(Asking { body, name, pos })));
}

/// The name offered: the part the piece came from with "piece" after it, numbered when a part reads so already.
fn piece_name(project: &Project, body: Id) -> String {
    let from = project.body_owner(body).and_then(|p| project.components.iter().find(|c| c.id == p)).map(|c| crate::i18n::name(&c.name)).unwrap_or_default();
    let offered = crate::i18n::tr1("piece-part-name-offered", "name", &from);
    project.free_component_name_shown(&offered, 0, &|n| crate::i18n::name(n))
}
