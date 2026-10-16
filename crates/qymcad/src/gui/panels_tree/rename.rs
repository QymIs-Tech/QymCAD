//! A ROW OF THE TREE RENAMED in place.

use super::*;

/// If tree node `node` is being renamed right now, draw an input field instead of the label and commit the
/// name into ITS OWN storage on Enter or on a click elsewhere (Escape cancels). Returns true if the field is
/// shown (in which case the label and the row menu are not drawn).
pub(crate) fn rename_node_active(tc: &mut qymcad_ui_state::TreeCtx, ui: &mut egui::Ui, node: RenameNode) -> bool {
    if tc.rename.node != Some(node) {
        return false;
    }
    let resp = qymcad_ui_state::rename_field(ui, &mut tc.rename.buf, 160.0, std::mem::take(&mut tc.rename.focus));
    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        tc.rename.node = None; // cancelled
    } else if resp.lost_focus() {
        let nm = tc.rename.buf.trim().to_string(); // Enter or a click elsewhere commits
        if !nm.is_empty() {
            qymcad_ui_state::begin_edit(&mut *tc.edits, &*tc.project, crate::i18n::tr("status-rename")); // THE EDIT BOUNDARY: the document is changed from a panel
            match node {
                RenameNode::Component(id) => {
                    tc.project.rename_component(id, nm);
                }
                RenameNode::Plane(id) => {
                    tc.project.rename_plane(id, nm);
                }
                RenameNode::DatumPoint(id) => {
                    if let Some(d) = tc.project.datum_points.iter_mut().find(|d| d.id == id) {
                        d.name = nm;
                    }
                }
                RenameNode::DatumAxis(id) => {
                    if let Some(d) = tc.project.datum_axes.iter_mut().find(|d| d.id == id) {
                        d.name = nm;
                    }
                }
                RenameNode::Body(mi) => tc.project.set_mesh_name(mi, nm),
            }
            qymcad_ui_state::commit_edit(&mut tc.rebuild());
        }
        tc.rename.node = None;
    }
    true
}

/// If node `id` is being renamed right now, draw an input field instead of the heading and commit the name
/// on Enter or on a click elsewhere (Escape cancels). Returns true if the field is shown (in which case the
/// heading is not drawn).
pub(crate) fn rename_row_active(tc: &mut qymcad_ui_state::TreeCtx, ui: &mut egui::Ui, id: Id) -> bool {
    if tc.rename.target != Some(id) {
        return false;
    }
    let resp = qymcad_ui_state::rename_field(ui, &mut tc.rename.buf, 170.0, std::mem::take(&mut tc.rename.focus));
    let esc = ui.input(|i| i.key_pressed(egui::Key::Escape));
    if esc {
        tc.rename.target = None; // cancelled without saving
    } else if resp.lost_focus() {
        let nm = tc.rename.buf.trim().to_string(); // committed by Enter or by a click elsewhere
        if !nm.is_empty() {
            qymcad_ui_state::begin_edit(&mut *tc.edits, &*tc.project, crate::i18n::tr("status-rename-op")); // THE EDIT BOUNDARY
            tc.project.rename_node(id, nm);
            qymcad_ui_state::commit_edit(&mut tc.rebuild());
        }
        tc.rename.target = None;
    }
    true
}
