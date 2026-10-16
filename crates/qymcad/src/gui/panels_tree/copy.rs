//! THE TREE'S CLIPBOARD: a node copied, an import brought in again.

use qymcad_ui_state::Notice;
use super::*;

/// Ctrl+C OUTSIDE A SKETCH: what is chosen goes into the clipboard - several components, a sketch node, a component, or a
/// node of the timeline (its tool is opened again with its values on Ctrl+V).
pub(crate) fn tree_copy(chosen: &qymcad_ui_state::Chosen, project: &Project, clip: &mut qymcad_ui_state::Clipboard, status: &mut qymcad_ui_state::Notice, cut: bool) {
    // With several components selected, the whole set goes into the bulk clipboard (the root excepted).
    if crate::gui::is_multi(project, chosen.sel, &chosen.tree_sel) {
        let root = project.root;
        let ids: Vec<Id> = chosen.tree_sel.multi.iter().copied().filter(|&id| id != root).collect();
        if ids.is_empty() {
            *status = Notice::refusal(crate::i18n::tr("tree-root-not-copyable"));
            return;
        }
        let n = ids.len();
        clip.tree = None;
        clip.tree_multi = Some((ids, cut));
        clip.os_ping = true; // a marker into the OS clipboard, so that Ctrl+V (Event::Paste) starts working
        *status = Notice::info(crate::i18n::tr2("clip-components", "n", &n.to_string(), "how", &if cut { crate::i18n::tr("action-cut") } else { crate::i18n::tr("action-copy") }));
        return;
    }
    clip.tree_multi = None;
    match chosen.sel {
        Sel::Sketch(si) => {
            if let Some(s) = project.sketches.get(si) {
                let sid = s.id;
                clip.tree = Some(TreeClip::Sketch { sid, cut });
                clip.os_ping = true; // a marker into the OS clipboard, so that Ctrl+V (Event::Paste) starts working
                *status = Notice::info(crate::i18n::tr1("clip-sketch", "how", &if cut { crate::i18n::tr("action-cut") } else { crate::i18n::tr("action-copy") }));
            }
        }
        Sel::Component(ci) => {
            if let Some(c) = project.components.get(ci) {
                let id = c.id;
                if id == project.root {
                    *status = Notice::refusal(crate::i18n::tr("tree-root-not-copyable"));
                    return;
                }
                clip.tree = Some(TreeClip::Component { id, cut });
                clip.os_ping = true; // a marker into the OS clipboard, so that Ctrl+V (Event::Paste) starts working
                let what = if project.component_kind(id) == Some(qymcad_core::feature::ComponentKind::Assembly) {
                    crate::i18n::tr("node-subassembly")
                } else {
                    crate::i18n::tr("node-part")
                };
                *status = Notice::info(crate::i18n::tr2("clip-node", "what", &what, "how", &if cut { crate::i18n::tr("action-cut") } else { crate::i18n::tr("action-copy") }));
            }
        }
        Sel::Feature(fi) => {
            if let Some(n) = project.timeline.get(fi) {
                clip.tree = Some(TreeClip::Feature { nid: n.id });
                clip.os_ping = true; // a marker into the OS clipboard, so that Ctrl+V (Event::Paste) starts working
                *status = Notice::info(crate::i18n::tr2("clip-node", "what", &crate::i18n::name(&n.name), "how", &crate::i18n::tr("action-copy")));
            }
        }
        _ => *status = Notice::info(crate::i18n::tr("tree-copy-pick-first")),
    }
}

/// A KEPT FILE BROUGHT IN AGAIN - what it is kept for: its bytes written out as the file they were, and that file taken
/// through the door File -> Import takes, a drawing asking there for the plane its curves go on. Reported behaviour: a
/// drawing could not be brought in again from its row, only through File -> Import.
pub(crate) fn bring_in_again(tc: &mut qymcad_ui_state::TreeCtx, i: usize) {
    let Some(src) = tc.project.sources.get(i) else { return };
    let dir = std::env::temp_dir().join("qymcad-again");
    let path = dir.join(&src.name);
    if std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(&path, &src.data)).is_err() {
        *tc.status = Notice::error(crate::i18n::tr1("tree-source-again-failed", "name", &src.name));
        return;
    }
    tc.ask.push(qymcad_ui_state::TreeAsk::ImportFile(path));
}
