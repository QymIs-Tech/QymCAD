//! THE TREE: renaming, picking rows, deleting what is picked.

use super::*;

/// Start renaming a component, a datum or a body in place: the current name goes into the field, with
/// auto-focus. The other two targets are cleared - one field serves all three, and two of them set at once
/// would rename twice.
pub fn start_rename_node(rename: &mut RenameInput, node: RenameNode, cur: String) {
    *rename = RenameInput { node: Some(node), buf: cur, focus: true, ..Default::default() };
}

/// Start renaming timeline node `id` in place.
pub fn start_rename(project: &qymcad_core::model::Project, rename: &mut RenameInput, id: Id) {
    let cur = project.timeline.iter().find(|n| n.id == id).map(|n| qymcad_i18n::name(&n.name)).unwrap_or_default();
    *rename = RenameInput { target: Some(id), buf: cur, focus: true, ..Default::default() };
}

/// Start renaming sketch `sid` in place.
pub fn start_rename_sketch(rename: &mut RenameInput, sid: Id, cur: String) {
    *rename = RenameInput { sketch: Some(sid), buf: cur, focus: true, ..Default::default() };
}

/// START RENAMING WHATEVER IS SELECTED. Says whether there was anything to rename.
///
/// Reported behaviour: "renaming parts/subassemblies/sketches/features not only by right-click -> Rename
/// but also by pressing F2."
///
/// ONE DOOR, because the tree renames five kinds of node through three different fields - a feature by
/// `target`, a sketch by `sketch`, a component, a body and a datum by `node` - and each menu item started
/// its own. A key that had to repeat that fan-out would be a sixth copy, drifting from the other five at
/// the first kind of node added.
pub fn rename_selected(project: &qymcad_core::model::Project, rename: &mut RenameInput, sel: Sel) -> bool {
    // ALREADY RENAMING THIS VERY NODE: leave what has been typed alone. F2 pressed a second time, or the
    // menu item used while the field is open, must not throw a half-typed name away and start over.
    if renaming_now(project, rename, sel) {
        return true;
    }
    match sel {
        Sel::Feature(fi) => match project.timeline.get(fi) {
            Some(n) => {
                start_rename(project, rename, n.id);
                true
            }
            None => false,
        },
        Sel::Sketch(si) => match project.sketches.get(si) {
            Some(s) => {
                start_rename_sketch(rename, s.id, qymcad_i18n::name(&s.name));
                true
            }
            None => false,
        },
        Sel::Component(ci) => match project.components.get(ci) {
            Some(c) => {
                start_rename_node(rename, RenameNode::Component(c.id), qymcad_i18n::name(&c.name));
                true
            }
            None => false,
        },
        Sel::Mesh(mi) => match project.bodies.get(mi) {
            Some(_) => {
                start_rename_node(rename, RenameNode::Body(mi), qymcad_i18n::name(&project.mesh_name(mi)));
                true
            }
            None => false,
        },
        Sel::Plane(pi) => match project.planes.get(pi) {
            Some(p) => {
                start_rename_node(rename, RenameNode::Plane(p.id), qymcad_i18n::name(&p.name));
                true
            }
            None => false,
        },
        Sel::DatumPoint(i) => match project.datum_points.get(i) {
            Some(d) => {
                start_rename_node(rename, RenameNode::DatumPoint(d.id), qymcad_i18n::name(&d.name));
                true
            }
            None => false,
        },
        Sel::DatumAxis(i) => match project.datum_axes.get(i) {
            Some(d) => {
                start_rename_node(rename, RenameNode::DatumAxis(d.id), qymcad_i18n::name(&d.name));
                true
            }
            None => false,
        },
        // A face, a contour, a mate or nothing at all: none of these carries a name of its own.
        Sel::None | Sel::Face(..) | Sel::Contour(_) | Sel::Joint(_) | Sel::Edge(..) | Sel::Vertex(..) => false,
    }
}

/// WHERE A SEGMENT CROSSES ONE OF THE SKETCH AXES: 0 is the line x = 0, 1 is the line y = 0.
///
/// The axes are infinite, so this is not `seg_seg_intersect` against some long enough stand-in segment -
/// "long enough" is a number pulled out of the air, and at a far enough zoom it stops being long enough.
/// THE MIDDLE OF WHAT IS SELECTED, in sketch coordinates - to tell "it moved" from "it was held".
///
/// The centre of the bounding box rather than an average of points: an average shifts with how many
/// points a shape happens to be built from, and a circle carries one while a rectangle carries four.
pub fn entities_centroid(project: &Project, si: usize, eids: &[Id]) -> Option<Point2> {
    let s = project.sketches.get(si)?;
    let points_by_id: std::collections::HashMap<Id, &qymcad_core::model::SketchPoint> = s.points.iter().map(|p| (p.id, p)).collect(); // a table: these are looked up for every entity or constraint
    let pt = |id: Id| points_by_id.get(&id).copied().map(|p| Point2::new(p.x, p.y));
    let (mut lo, mut hi) = ((f64::MAX, f64::MAX), (f64::MIN, f64::MIN));
    let mut seen = false;
    for e in s.entities.iter().filter(|e| eids.contains(&e.id)) {
        for id in qymcad_core::model::entity_points(e) {
            if let Some(p) = pt(id) {
                lo = (lo.0.min(p.x), lo.1.min(p.y));
                hi = (hi.0.max(p.x), hi.1.max(p.y));
                seen = true;
            }
        }
    }
    seen.then(|| Point2::new((lo.0 + hi.0) / 2.0, (lo.1 + hi.1) / 2.0))
}

/// Is the rename field already open on exactly this selection?
pub(crate) fn renaming_now(project: &qymcad_core::model::Project, rename: &RenameInput, sel: Sel) -> bool {
    match sel {
        Sel::Feature(fi) => project.timeline.get(fi).is_some_and(|n| rename.target == Some(n.id)),
        Sel::Sketch(si) => project.sketches.get(si).is_some_and(|s| rename.sketch == Some(s.id)),
        Sel::Component(ci) => project.components.get(ci).is_some_and(|c| rename.node == Some(RenameNode::Component(c.id))),
        Sel::Mesh(mi) => rename.node == Some(RenameNode::Body(mi)),
        Sel::Plane(pi) => project.planes.get(pi).is_some_and(|p| rename.node == Some(RenameNode::Plane(p.id))),
        Sel::DatumPoint(i) => project.datum_points.get(i).is_some_and(|d| rename.node == Some(RenameNode::DatumPoint(d.id))),
        Sel::DatumAxis(i) => project.datum_axes.get(i).is_some_and(|d| rename.node == Some(RenameNode::DatumAxis(d.id))),
        Sel::None | Sel::Face(..) | Sel::Contour(_) | Sel::Joint(_) | Sel::Edge(..) | Sel::Vertex(..) => false,
    }
}

/// THE SINGLE dispatcher for deleting the selection (the Del key or the buttons): a feature or a body
/// cascades, a sketch cascades. Returns true when something was deleted (otherwise the caller handles
/// its own case — sketch geometry, for instance).
/// A human-readable name of the node being deleted, for the confirmation popup.
/// ASK BEFORE DELETING — THE SINGLE ENTRY, whichever button was pressed.
///
/// Only the tree asked, on Del and on "delete the part"; the very same feature, sketch, plane, axis,
/// point, contour and body were removed SILENTLY by the button in the properties panel. That is,
/// whether one was asked depended on the route taken to the same action — exactly the defect the
/// right-hand panel's editors already had.
///
/// The removal itself is done by `execute_delete`, one for every kind: the dialogue must not know how
/// things are deleted.
pub fn ask_delete(deferred: &mut DeferredUi, sel: Sel) {
    deferred.delete = Some(sel);
    deferred.delete_group.clear();
}

/// ASK TO DELETE WHAT IS CHOSEN, AND THE ROWS OF THE TREE PICKED WITH IT: several rows picked with Shift or Ctrl, the one
/// chosen among them, are asked about and deleted together. Reported behaviour: several parts picked in the tree with
/// Shift, Delete removed the lowest of them alone.
pub fn ask_delete_chosen(deferred: &mut DeferredUi, project: &Project, sel: Sel, tree_sel: &TreeSelection) {
    deferred.delete = Some(sel);
    let in_group = tree_sel.multi.len() > 1 && node_of_sel(project, sel).is_some_and(|id| tree_sel.multi.contains(&id));
    deferred.delete_group = if in_group { tree_sel.multi.clone() } else { Vec::new() };
}

/// THE DELETE KEY OUTSIDE A SKETCH: a node of the tree chosen - a feature, a body, a sketch, a datum, a mate, a
/// component - and the rows picked with it, asked about. Answers whether the key is for the sketch being edited.
pub fn delete_key(ctx: &egui::Context, sketch_ses: &SketchSession, chosen: &Chosen, project: &Project, deferred: &mut DeferredUi) -> bool {
    if ctx.egui_wants_keyboard_input() || !ctx.input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace)) {
        return false;
    }
    if sketch_ses.editing.is_some() {
        return true;
    }
    // any structural node of the tree brings up the yes-or-no question; DEL used to do nothing for datums and sketches
    if matches!(chosen.sel, Sel::Feature(_) | Sel::Mesh(_) | Sel::Sketch(_) | Sel::Plane(_) | Sel::DatumPoint(_) | Sel::DatumAxis(_) | Sel::Joint(_) | Sel::Component(_)) {
        ask_delete_chosen(deferred, project, chosen.sel, &chosen.tree_sel);
    }
    false
}

/// THE ID OF THE ROW OF THE TREE A SELECTION IS: a component's, or the timeline node's of a feature, a sketch or a datum.
pub fn node_of_sel(project: &Project, sel: Sel) -> Option<Id> {
    use qymcad_core::feature::FeatureKind as FK;
    let node = |hit: &dyn Fn(&FK) -> bool| project.timeline.iter().find(|n| hit(&n.kind)).map(|n| n.id);
    match sel {
        Sel::Component(ci) => project.components.get(ci).map(|c| c.id),
        Sel::Feature(ti) => project.timeline.get(ti).map(|n| n.id),
        Sel::Sketch(si) => project.sketches.get(si).map(|s| s.id).and_then(|sid| node(&|k| matches!(k, FK::Sketch { sketch } if *sketch == sid))),
        Sel::Plane(i) => project.planes.get(i).map(|p| p.id).and_then(|pid| node(&|k| matches!(k, FK::Plane { plane } if *plane == pid))),
        Sel::DatumPoint(i) => project.datum_points.get(i).map(|p| p.id).and_then(|pid| node(&|k| matches!(k, FK::DatumPoint { point } if *point == pid))),
        Sel::DatumAxis(i) => project.datum_axes.get(i).map(|a| a.id).and_then(|aid| node(&|k| matches!(k, FK::DatumAxis { axis } if *axis == aid))),
        _ => None,
    }
}

/// THE SELECTION A ROW OF THE TREE STANDS FOR, by its id: a component, or a timeline node as the tree selects it - a
/// sketch, a datum, or a feature. None once it is gone.
pub fn sel_of_node(project: &Project, id: Id) -> Option<Sel> {
    use qymcad_core::feature::FeatureKind as FK;
    if let Some(ci) = project.components.iter().position(|c| c.id == id) {
        return Some(Sel::Component(ci));
    }
    let ti = project.timeline.iter().position(|n| n.id == id)?;
    Some(match project.timeline[ti].kind {
        FK::Sketch { sketch } => Sel::Sketch(project.sketch_index(sketch)?),
        FK::Plane { plane } => Sel::Plane(project.planes.iter().position(|p| p.id == plane)?),
        FK::DatumPoint { point } => Sel::DatumPoint(project.datum_points.iter().position(|p| p.id == point)?),
        FK::DatumAxis { axis } => Sel::DatumAxis(project.datum_axes.iter().position(|a| a.id == axis)?),
        _ => Sel::Feature(ti),
    })
}

/// THE NEXT NODE OF A GROUP DELETE, asked when nothing else is: one a frame, found again by its id; a node a delete
/// before it took along is passed over.
pub fn next_of_a_group_delete(deferred: &mut DeferredUi, project: &Project) -> Option<(Sel, bool)> {
    while deferred.delete.is_none() && !deferred.delete_queue.ids.is_empty() {
        let id = deferred.delete_queue.ids.remove(0);
        if let Some(sel) = sel_of_node(project, id) {
            return Some((sel, deferred.delete_queue.dependents));
        }
    }
    None
}

/// HOW A ROW OF THE TREE WAS CLICKED: alone, with Ctrl (Cmd on a Mac) to add it or take it away, with Shift for the run
/// of rows from the one clicked before.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TreeClick {
    Alone,
    Toggle,
    Run,
}

impl TreeClick {
    pub fn of(m: &egui::Modifiers) -> Self {
        if m.shift {
            TreeClick::Run
        } else if m.ctrl || m.command {
            TreeClick::Toggle
        } else {
            TreeClick::Alone
        }
    }
}

/// PICK THE ROW `id` OF THE TREE among `order`, the rows it stands among, as `click` says: alone, added or taken away,
/// or the run of rows from the anchor to it.
pub fn tree_pick(tree_sel: &mut TreeSelection, order: &[Id], id: Id, click: TreeClick) {
    match click {
        TreeClick::Run => match (tree_sel.anchor.and_then(|a| order.iter().position(|&x| x == a)), order.iter().position(|&x| x == id)) {
            (Some(ia), Some(ib)) => tree_sel.multi = order[ia.min(ib)..=ia.max(ib)].to_vec(),
            _ => {
                tree_sel.multi = vec![id];
                tree_sel.anchor = Some(id);
            }
        },
        TreeClick::Toggle => {
            match tree_sel.multi.iter().position(|&x| x == id) {
                Some(p) => {
                    tree_sel.multi.remove(p);
                }
                None => tree_sel.multi.push(id),
            }
            tree_sel.anchor = Some(id);
        }
        TreeClick::Alone => {
            tree_sel.multi = vec![id];
            tree_sel.anchor = Some(id);
        }
    }
}

/// Whether the row `id` is lit as one of several picked in the tree.
pub fn in_tree_group(tree_sel: &TreeSelection, id: Id) -> bool {
    tree_sel.multi.len() > 1 && tree_sel.multi.contains(&id)
}
