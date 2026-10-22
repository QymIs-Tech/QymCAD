//! THE BAR OF THE PATH OF THE DOCUMENT: the steps from the root down to the component in hand and the sketch open in
//! it, the button that leaves, the parameters and "in context" that stand in every workbench, and the switches each
//! workbench offers - laid out by the shell.

use crate::catalogue::Offer;
use crate::BarCtx;
use egui_phosphor::regular as ph;
use qymcad_shell::{Crumb, Drew, PathAsk, Step, Switches};

/// LAY THE BAR OF THE PATH with the switches of `offers`, and push what the person asked of it.
pub fn path_bar(bc: &mut BarCtx, ui: &mut egui::Ui, offers: &[&Offer]) {
    let crumbs = crumbs(bc);
    let leave = (bc.sketch_ses.editing.is_some() || bc.active_path.len() > 1).then(|| qymcad_shell::Leave { word: qymcad_i18n::tr("wb-finish"), hint: qymcad_i18n::tr("wb-finish-hint") });
    let bar = qymcad_shell::PathBar { crumbs: &crumbs, leave, open_colour: bc.scheme.pal.hint() };
    let asked = qymcad_shell::path_bar(ui, bar, |row| {
        row.group(|ui| params_switch(bc, ui));
        for offer in offers {
            (offer.switches)(bc, row);
        }
        row.group(|ui| in_context_switch(bc, ui));
    });
    match asked {
        Some(PathAsk::GoTo(i)) => bc.ask.push(crate::BarAsk::GotoPath(i)),
        Some(PathAsk::Leave) => bc.ask.push(crate::BarAsk::ExitContext),
        None => {}
    }
}

/// THE STEPS OF THE PATH: the components from the root down to the one in hand - an assembly and the root wear the
/// same icon, both hold other things rather than being a shape - and the sketch open in the last one.
fn crumbs(bc: &mut BarCtx) -> Vec<Crumb> {
    crate::ensure_active_path(&mut *bc.active_path, &mut *bc.project);
    let editing = bc.sketch_ses.editing;
    let last = bc.active_path.len();
    let mut crumbs: Vec<Crumb> = bc
        .active_path
        .iter()
        .enumerate()
        .map(|(i, cid)| {
            let name = bc.project.components.iter().find(|c| c.id == *cid).map(|c| qymcad_i18n::name(&c.name)).unwrap_or_else(|| qymcad_i18n::tr("wb-assembly"));
            let assembly = i == 0 || bc.project.component_kind(*cid) == Some(qymcad_core::feature::ComponentKind::Assembly);
            let step = if i + 1 == last && editing.is_none() { Step::Here } else { Step::Above };
            Crumb { icon: if assembly { ph::STACK } else { ph::CUBE }, name, step }
        })
        .collect();
    if let Some(sid) = editing {
        let name = bc.project.sketches.iter().find(|s| s.id == sid).map(|s| qymcad_i18n::name(&s.name)).unwrap_or_else(|| qymcad_i18n::tr("wb-sketch"));
        crumbs.push(Crumb { icon: ph::PENCIL, name, step: Step::Open });
    }
    crumbs
}

/// THE GLOBAL PARAMETERS, reachable from any workbench.
fn params_switch(bc: &mut BarCtx, ui: &mut egui::Ui) -> Drew {
    if ui.selectable_label(bc.win.is(crate::WinKind::Params), qymcad_i18n::tr("wb-params")).on_hover_text(qymcad_i18n::tr("wb-params-hint")).clicked() {
        bc.win.toggle(crate::WinKind::Params);
    }
    Drew::Something
}

/// "IN CONTEXT" applies inside ANY component, a part or a subassembly, not at the root: top-down references to the
/// neighbours and the datums of the parent shown. The root has no ancestors, so it is not offered there.
fn in_context_switch(bc: &mut BarCtx, ui: &mut egui::Ui) -> Drew {
    if crate::current_ctx_id(&*bc.active_path, &*bc.project) == bc.project.root {
        return Drew::Nothing;
    }
    ui.toggle_value(&mut bc.win.context, format!("{} {}", ph::STACK, qymcad_i18n::tr("wb-in-context"))).on_hover_text(qymcad_i18n::tr("wb-in-context-hint"));
    Drew::Something
}

/// THE SNAP AND ITS GRID: on and off, and the step of the grid while it is on - the cursor of a sketch snaps to it, and
/// the gizmo in 3D.
pub fn snap_and_grid(set: &mut crate::Settings, ui: &mut egui::Ui) {
    ui.toggle_value(&mut set.snap.on, format!("{} {}", ph::MAGNET, qymcad_i18n::tr("wb-snap"))).on_hover_text(qymcad_i18n::tr("wb-snap-hint"));
    if set.snap.on {
        ui.add(egui::DragValue::new(&mut set.snap.grid).speed(0.5).range(0.1..=100.0).prefix(qymcad_i18n::tr("wb-grid")).suffix(qymcad_i18n::tr("unit-mm-suffix")));
    }
}

/// THE SNAP OF THE GIZMO in 3D inside workbench `home`: the snap, its grid and the step of a rotation, which a sketch
/// has no use for. Nothing while another workbench is in hand, in 2D, or while a sketch is open.
pub fn gizmo_snap_switches(bc: &mut BarCtx, row: &mut Switches, home: crate::Workbench) {
    if *bc.workbench != home || !*bc.mode_3d || bc.sketch_ses.editing.is_some() {
        return;
    }
    row.group(|ui| {
        snap_and_grid(bc.set, ui);
        if bc.set.snap.on {
            ui.add(egui::DragValue::new(&mut bc.set.snap.rot_deg).speed(1.0).range(0.5..=90.0).prefix(qymcad_i18n::tr("wb-rotation")).suffix(qymcad_i18n::tr("unit-deg-suffix")));
        }
        Drew::Something
    });
}
