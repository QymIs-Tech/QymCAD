//! A FRAME DRAWN IN THE 3D VIEW takes what it encloses: the parts in an assembly, the bodies in a part, and with an
//! extrusion or a revolution in hand the contours of its sketch, as the box of a sketch takes curves - drawn left to
//! right the ones wholly inside, right to left the ones it touches; with Shift at the release they are added. Under our
//! layout a left drag that starts on the model turns the view as before; one that starts where there is nothing - no
//! face, no handle of a gizmo - draws the frame. Under the layouts of other programs the frame is drawn with that
//! program's gesture (`MouseNav::frames`), or not at all. The right-button menu over the view then copies or cuts what it took.
//!
//! Reported behaviour: a frame drawn round two parts took nothing, the drag turned the view.
//!
//! Where the frame started lives in egui's memory for the drag, not in the application: it is the state of one
//! gesture, gone when the button goes up.
use egui::{Pos2, Rect, Response};
use qymcad_core::model::Id;
use qymcad_ui_state::{Painting, Sel, Workbench};

/// What the frame did this frame.
pub(crate) enum Frame {
    /// It is being drawn: the view must not turn under it.
    Drawing,
    /// The button went up: these parts (in an assembly) or bodies (in a part) are inside.
    Took(Vec<Id>),
    /// The button went up with a tool taking profiles in hand: these contours of its sketch are taken.
    Profiles(TakenProfiles),
}

/// THE CONTOURS A FRAME TOOK for the tool in hand, and whether they are added to what it gathered.
pub(crate) struct TakenProfiles {
    took: Vec<Id>,
    adding: qymcad_ui_state::Adding,
}

fn memory() -> egui::Id {
    egui::Id::new("frame_in_space")
}

/// May a frame start at `pos`: nothing in hand but a tool taking profiles, and nothing there to grab - no face of the
/// model, no handle of the gizmo of what is chosen, no arrow or handle of the command.
fn may_start(pn: &Painting, rect: Rect, pos: Pos2, on_the_model: bool) -> bool {
    let profiles = qymcad_ui_state::profile_sketch(pn.armed, pn.cmd).is_some();
    if !pn.mode_3d || !matches!(pn.workbench, Workbench::Part | Workbench::Assembly) || (pn.armed.commanding() && !profiles) || qymcad_assembly::joint_picking(pn.joint) || pn.m3.on || pn.section.pick
    {
        return false;
    }
    if !on_the_model && qymcad_pick::face_under_cursor(pn, rect, pos).is_some() {
        return false;
    }
    let basis = pn.cam.basis();
    let scr = qymcad_ui_state::Screen { cam: &pn.cam, set: pn.set, rect, basis: &basis };
    if profiles
        && (qymcad_ui_state::cmd_arrow_at(&qymcad_ui_state::feat_cmd_arrows(pn.cmd, pn.gsel, pn.project, pn.feat.flip), &scr, pos).is_some() || qymcad_ui_state::handle_at(pn, &scr, pos).is_some())
    {
        return false;
    }
    let dc = qymcad_ui_state::DrawCtx { cam: &pn.cam, set: pn.set, scheme: pn.scheme, project: pn.project, active_path: pn.active_path };
    let handle = |o: [f64; 3], l: f64| crate::gui::gizmo_axis_hit_at(&dc, o, l, rect, &basis, pos).is_some() || crate::gui::gizmo_ring_hit_at(&dc, o, l, rect, &basis, pos).is_some();
    if let Some(comp) = qymcad_ui_state::gizmo_component(pn.active_path, pn.project, pn.sel, pn.workbench) {
        let (o, l) = qymcad_ui_state::gizmo_geometry(pn.cam, pn.comp_giz, pn.project, comp);
        if handle(o, l) {
            return false;
        }
    }
    if let Some((_, mi)) = qymcad_ui_state::body_gizmo_target(pn.body_view(), pn.sel) {
        let (o, l) = qymcad_ui_state::body_gizmo_geometry(pn.body_giz, pn.cam, pn.project, pn.set, mi);
        if handle(o, l) {
            return false;
        }
    }
    true
}

/// Drive the frame over `resp`: start it from empty space, draw it while the button is held, and answer what it took
/// when the button goes up. `None` when no frame is being drawn.
pub(crate) fn frame(pn: &Painting, ctx: &egui::Context, resp: &Response, painter: &egui::Painter, rect: Rect) -> Option<Frame> {
    // THE LAYOUT'S OWN GESTURE OF THE FRAME, its modifiers exactly: a chord or another modified drag belongs to the
    // layout's movements
    let (gesture, on_the_model) = pn.set.mouse_nav.frames()?;
    let held = ctx.input(|i| {
        i.modifiers.shift == gesture.shift && (i.modifiers.ctrl || i.modifiers.command) == gesture.ctrl && i.modifiers.alt == gesture.alt && !i.pointer.middle_down() && !i.pointer.secondary_down()
    });
    if resp.drag_started_by(egui::PointerButton::Primary) && held {
        if let Some(pos) = ctx.input(|i| i.pointer.press_origin()).filter(|p| may_start(pn, rect, *p, on_the_model)) {
            ctx.data_mut(|d| d.insert_temp(memory(), pos));
        }
    }
    let start: Pos2 = ctx.data(|d| d.get_temp(memory()))?;
    let now = resp.interact_pointer_pos().or(ctx.input(|i| i.pointer.latest_pos())).unwrap_or(start);
    let area = Rect::from_two_pos(start, now);
    let profiles = qymcad_ui_state::profile_sketch(pn.armed, pn.cmd);
    if resp.drag_stopped() || !ctx.input(|i| i.pointer.primary_down()) {
        ctx.data_mut(|d| d.remove::<Pos2>(memory()));
        let Some(si) = profiles else { return Some(Frame::Took(enclosed(pn, rect, area))) };
        let basis = pn.cam.basis();
        let scr = qymcad_ui_state::Screen { cam: &pn.cam, set: pn.set, rect, basis: &basis };
        let took = qymcad_ui_state::contours_in_box_3d(pn.project, &scr, qymcad_ui_state::ScreenBox { from: start, to: now }, si);
        // Shift adds where it is not the layout's own key of the frame
        let adding = if ctx.input(|i| i.modifiers.shift) && !gesture.shift { qymcad_ui_state::Adding::Yes } else { qymcad_ui_state::Adding::No };
        return Some(Frame::Profiles(TakenProfiles { took, adding }));
    }
    // the parts and bodies are taken wholly inside whichever way the frame is drawn, and it is drawn as a window
    let drawn = if profiles.is_some() { qymcad_ui_state::ScreenBox { from: start, to: now } } else { qymcad_ui_state::ScreenBox { from: area.min, to: area.max } };
    paint_box(painter, &pn.scheme.pal, drawn);
    Some(Frame::Drawing)
}

/// THE BOX OF A SELECTION AS IT IS DRAWN, in a sketch and in the 3D view alike: a window (left to right) solid in the
/// colour of a window, a crossing (right to left) dashed in the colour of a crossing, each over a faint fill of its colour.
pub(crate) fn paint_box(painter: &egui::Painter, pal: &qymcad_scheme::Palette, bx: qymcad_ui_state::ScreenBox) {
    let r = bx.rect();
    match bx.rule() {
        qymcad_ui_state::BoxRule::Crossing => {
            let st = egui::Stroke::new(1.2, pal.select_cross());
            for seg in [[r.left_top(), r.right_top()], [r.right_top(), r.right_bottom()], [r.right_bottom(), r.left_bottom()], [r.left_bottom(), r.left_top()]] {
                painter.add(egui::Shape::dashed_line(&seg, st, 5.0, 4.0));
            }
            painter.rect_filled(r, 0.0, qymcad_scheme::a(pal.select_cross(), 20));
        }
        qymcad_ui_state::BoxRule::Window => {
            painter.rect_stroke(r, 0.0, egui::Stroke::new(1.2, pal.select_window()), egui::StrokeKind::Middle);
            painter.rect_filled(r, 0.0, qymcad_scheme::a(pal.select_window(), 20));
        }
    }
}

/// What lies wholly inside `area` on screen: every corner of a body's box is in it. In an assembly the part standing
/// in the assembly being worked in, in a part the body.
fn enclosed(pn: &Painting, rect: Rect, area: Rect) -> Vec<Id> {
    let basis = pn.cam.basis();
    let scr = qymcad_ui_state::Screen { cam: &pn.cam, set: pn.set, rect, basis: &basis };
    let ctx = qymcad_ui_state::current_ctx_id(pn.active_path, pn.project);
    let mut took: Vec<Id> = Vec::new();
    for (mi, body) in qymcad_ui_state::shown_bodies(pn) {
        let Some(mesh) = pn.project.bodies.get(mi).map(|b| &b.mesh) else { continue };
        if mesh.verts.is_empty() {
            continue;
        }
        let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
        for v in &mesh.verts {
            for (i, c) in [v.x, v.y, v.z].into_iter().enumerate() {
                lo[i] = lo[i].min(c);
                hi[i] = hi[i].max(c);
            }
        }
        let wt = pn.project.body_display_transform(body, ctx);
        let inside = (0..8).all(|k| {
            let c = [if k & 1 == 0 { lo[0] } else { hi[0] }, if k & 2 == 0 { lo[1] } else { hi[1] }, if k & 4 == 0 { lo[2] } else { hi[2] }];
            area.contains(scr.at(qymcad_core::feature::apply12(&wt, c)).0)
        });
        if !inside {
            continue;
        }
        let what = if matches!(pn.workbench, Workbench::Assembly) { pn.project.body_owner(body).and_then(|o| pn.project.ancestor_child_of(ctx, o)) } else { Some(body) };
        if let Some(id) = what.filter(|id| !took.contains(id)) {
            took.push(id);
        }
    }
    took
}

/// WHERE WHAT A FRAME TOOK GOES: the selection, or the profiles of the tool in hand and the line that counts them.
pub(crate) struct TakenInto<'a> {
    pub chosen: &'a mut qymcad_ui_state::Chosen,
    pub gsel: &'a mut qymcad_ui_state::GeomSelection,
    pub status: &'a mut qymcad_ui_state::Notice,
}

/// What the frame did, put where it goes: `true` when there was no frame (the drag is the view's or a handle's), and
/// what a finished frame took goes into the selection, or into the tool in hand.
pub(crate) fn none_or_take(framed: Option<Frame>, at: TakenInto, project: &qymcad_core::model::Project, workbench: Workbench) -> bool {
    match framed {
        None => true,
        Some(Frame::Drawing) => false,
        Some(Frame::Took(took)) => {
            take(took, &mut at.chosen.sel, &mut at.chosen.tree_sel.multi, project, workbench);
            false
        }
        Some(Frame::Profiles(p)) => {
            qymcad_ui_state::take_profiles(at.gsel, at.status, p.took, p.adding);
            false
        }
    }
}

/// Put what the frame took into the selection: the first as the one chosen, all of them as the several chosen.
fn take(took: Vec<Id>, sel: &mut Sel, multi: &mut Vec<Id>, project: &qymcad_core::model::Project, workbench: Workbench) {
    *sel = match took.first() {
        None => Sel::None,
        Some(first) if matches!(workbench, Workbench::Assembly) => project.components.iter().position(|c| c.id == *first).map_or(Sel::None, Sel::Component),
        Some(first) => project.mesh_index(*first).map_or(Sel::None, Sel::Mesh),
    };
    *multi = took;
}

/// THE RIGHT-BUTTON MENU OVER THE VIEW for several parts taken: copy them or cut them, as the tree offers. `Some(cut)`
/// when one was pressed.
pub(crate) fn menu(resp: &Response, multi: &[Id]) -> Option<bool> {
    if multi.len() < 2 {
        return None;
    }
    let n = multi.len().to_string();
    let mut pressed = None;
    resp.context_menu(|ui| {
        if ui.button(format!("{} {}", egui_phosphor::regular::COPY, crate::i18n::tr1("act-copy-selected", "n", &n))).clicked() {
            pressed = Some(false);
            ui.close();
        }
        if ui.button(format!("{} {}", egui_phosphor::regular::SCISSORS, crate::i18n::tr1("act-cut-selected", "n", &n))).clicked() {
            pressed = Some(true);
            ui.close();
        }
    });
    pressed
}
