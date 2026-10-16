//! THE TREE BUILT: the panel, the bar of the rollback, the walk of the document.

use qymcad_ui_state::Notice;
use super::*;

pub(crate) fn tree_panel(tc: &mut qymcad_ui_state::TreeCtx, ui: &mut egui::Ui) {
    if *tc.sel == Sel::None {
        tc.tree.shown = None; // nothing selected: the next selection shows its row again, even the same one
    }
    // the upper section: the machine and the geometry (it scrolls)
    egui::ScrollArea::vertical().id_salt("geomscroll").show(ui, |ui| {
        ui.add_space(4.0);

        // ONE build tree (the history in order, grouped by component)
        build_tree(tc, ui);

        // the embedded originals of the imports (dxf/svg/stl)
        if !tc.project.sources.is_empty() {
            ui.separator();
            egui::CollapsingHeader::new(format!("{} {}", ph::FILE, crate::i18n::tr1("tree-import-sources", "n", &tc.project.sources.len().to_string()))).id_salt("sources").show(ui, |ui| {
                let (mut del, mut again): (Option<usize>, Option<usize>) = (None, None);
                for si in 0..tc.project.sources.len() {
                    let s = &tc.project.sources[si];
                    ui.horizontal(|ui| {
                        // the right button on the row opens what can be done with the file, as on any row of the tree
                        ui.add(
                            egui::Label::new(crate::i18n::tr2("tree-source-size", "name", &crate::i18n::name(&s.name), "kb", &crate::i18n::num(s.data.len() as f64 / 1024.0, 1)))
                                .sense(egui::Sense::click()),
                        )
                        .context_menu(|ui| {
                            if ui.button(format!("{} {}", ph::ARROW_CLOCKWISE, crate::i18n::tr("tree-source-again"))).clicked() {
                                again = Some(si);
                                ui.close();
                            }
                            if ui.button(format!("{} {}", ph::TRASH, crate::i18n::tr("tree-source-delete"))).clicked() {
                                del = Some(si);
                                ui.close();
                            }
                        });
                        if ui.small_button(ph::ARROW_CLOCKWISE).on_hover_text(crate::i18n::tr("tree-source-again")).clicked() {
                            again = Some(si);
                        }
                        if ui.small_button(ph::TRASH).clicked() {
                            del = Some(si);
                        }
                    });
                }
                if let Some(i) = again {
                    bring_in_again(tc, i);
                }
                if let Some(i) = del {
                    tc.project.sources.remove(i);
                }
            });
        }
        ui.separator();
    });
}

/// The rollback bar: a horizontal line in the feature list that is dragged up and down. Everything
/// BELOW the line is suppressed (not built), everything above it is active. `active_k` is the current
/// number of active features (the position of the line). Dragging changes `Project::rollback` in steps
/// of one row. `rollback = None` means everything is active (the line sits at the bottom);
/// `Some(feat_tis[k])` means the first k features are active.
pub(crate) fn rollback_bar(tc: &mut qymcad_ui_state::TreeCtx, ui: &mut egui::Ui, feat_tis: &[usize], active_k: usize) {
    let n = feat_tis.len();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 14.0), egui::Sense::hover());
    // A STABLE id lets the drag survive a reflow of the row (the line visually follows the cursor between frames)
    let resp = ui.interact(rect, egui::Id::new("rollback_bar_drag"), egui::Sense::drag());
    let hot = resp.hovered() || resp.dragged();
    let col = if hot { tc.scheme.pal.selected() } else { tc.scheme.pal.rollback() };
    let y = rect.center().y;
    {
        let painter = ui.painter();
        painter.hline((rect.left() + 16.0)..=(rect.right() - 4.0), y, egui::Stroke::new(2.0, col));
        // the "drag me vertically" grip: two triangles drawn BY THE PAINTER, without a font (phosphor or
        // raw unicode inside painter.text comes out as "tofu" boxes).
        let gx = rect.left() + 8.0;
        let tri = |a: egui::Pos2, b: egui::Pos2, c: egui::Pos2| egui::Shape::convex_polygon(vec![a, b, c], col, egui::Stroke::NONE);
        painter.add(tri(egui::pos2(gx, y - 6.0), egui::pos2(gx - 4.0, y - 1.5), egui::pos2(gx + 4.0, y - 1.5)));
        painter.add(tri(egui::pos2(gx, y + 6.0), egui::pos2(gx - 4.0, y + 1.5), egui::pos2(gx + 4.0, y + 1.5)));
    }
    let tip = if active_k >= n {
        crate::i18n::tr1("rollback-all", "n", &n.to_string())
    } else {
        crate::i18n::tr2("rollback-part", "k", &active_k.to_string(), "n", &n.to_string())
    };
    let resp = resp.on_hover_text(tip).on_hover_cursor(egui::CursorIcon::ResizeVertical);
    if resp.dragged() {
        tc.rollback.accum += resp.drag_delta().y;
        let step = 20.0; // roughly the height of a feature row
        let mut k = active_k as i32;
        while tc.rollback.accum >= step {
            k += 1;
            tc.rollback.accum -= step;
        }
        while tc.rollback.accum <= -step {
            k -= 1;
            tc.rollback.accum += step;
        }
        let k = k.clamp(0, n as i32) as usize;
        if k != active_k {
            // WHILE DRAGGING only the line moves (the tree preview follows instantly) plus a repaint
            // request; the heavy resync (a full 3D regeneration) happens ONCE on release, not on every step.
            tc.project.set_rollback(if k >= n { None } else { Some(feat_tis[k]) });
            tc.rollback.pending = true;
            ui.ctx().request_repaint();
        }
    }
    if resp.drag_stopped() {
        tc.rollback.accum = 0.0;
        if std::mem::take(&mut tc.rollback.pending) {
            tc.ask.push(qymcad_ui_state::TreeAsk::Resync); // the final 3D regeneration at the line's resulting position
        }
    }
}

pub(crate) fn build_tree(tc: &mut qymcad_ui_state::TreeCtx, ui: &mut egui::Ui) {
    // Nodes are created from the Create panel on the left; here there is only the tree itself, with no duplicated buttons.
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(crate::i18n::tr("tree-title")).strong());
    });
    // THE TREE SEARCH: on a part of fifty features the right one cannot be found by eye.
    // The search goes by THE SAME label a person sees in the row (`feature_row_label`) - otherwise it would
    // stop finding what is displayed.
    //
    // THE FIELD'S WIDTH MUST NOT COME FROM THE PANEL'S WIDTH. This used to be `available_width()`, that is,
    // the width of LAST frame's panel, and it made a feedback loop: the field demands that width, the content
    // is wider by an icon and a button, the panel grows, and on the next frame the field asks for more still.
    // What that looked like: type into the search, and the panel slides smoothly to the right, squeezing
    // everything out of the window.
    //
    // The layout goes RIGHT TO LEFT: the button takes its place first and the field gets WHAT IS LEFT of the
    // row. That is why `desired_width(INFINITY)` is safe here - it means "whatever remains", not "whatever
    // the panel had". A zero width must not be asked for: the field would collapse and there would be nowhere
    // to type - which is what the first attempt did, leaving a search field impossible to reach.
    ui.horizontal(|ui| {
        ui.label(ph::MAGNIFYING_GLASS);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if !tc.tree.search.is_empty() && ui.small_button(ph::X).on_hover_text(crate::i18n::tr("tree-search-clear")).clicked() {
                tc.tree.search.clear();
            }
            ui.add(egui::TextEdit::singleline(&mut tc.tree.search).id(egui::Id::new("tree_search_field")).hint_text(crate::i18n::tr("tree-search")).desired_width(f32::INFINITY));
        });
    });
    // The mates appear only in an Assembly or a subassembly. ONLY the active assembly's joints are shown:
    // the joints of nested subassemblies are hidden so that they do not get in the way (filtered by
    // joint_home == the active context).
    if matches!(tc.workbench, Workbench::Assembly) {
        // THE SHARED contours toggle belongs here only. Inside a Part a sketch's visibility is governed by
        // its own checkbox in the tree, and a second, shared one would be superfluous there.
        ui.checkbox(&mut tc.set.show_contours, format!("{}  {}", ph::POLYGON, crate::i18n::tr("tree-contours-toggle"))).on_hover_text(crate::i18n::tr("tree-contours-hint"));
        ui.checkbox(&mut tc.set.show_joints, crate::i18n::tr("tree-show-joints")).on_hover_text(crate::i18n::tr("tree-joints-hint"));
        if ui.checkbox(&mut tc.set.show_interference, crate::i18n::tr("tree-interference")).on_hover_text(crate::i18n::tr("tree-interference-hint")).changed() {
            tc.interference.rev = u64::MAX; // force a recompute when it is toggled
        }
        if tc.set.show_interference && !tc.interference.pairs.is_empty() {
            qymcad_ui_state::say(
                ui,
                &tc.scheme.pal,
                &qymcad_ui_state::Notice::error(crate::i18n::tr1("tree-interference-n", "n", &tc.interference.pairs.len().to_string())),
                qymcad_ui_state::NoticeSize::Line,
            );
        }
    }
    ui.separator();
    // THE CONTEXTUAL tree: the folders of the current context. The breadcrumbs live in the top panel.
    use qymcad_core::feature::{ComponentKind, FeatureKind as FK};
    let ctx = qymcad_ui_state::current_ctx_id(tc.active_path, &*tc.project);
    let ctx_nodes: Vec<(Id, FK)> = tc.project.timeline.iter().filter(|n| n.parent == Some(ctx)).map(|n| (n.id, n.kind.clone())).collect();

    // The origin: the reference base planes (each component has a frame of its own)
    egui::CollapsingHeader::new(format!("{} {}", ph::SELECTION_ALL, crate::i18n::tr("tree-origin-node"))).id_salt(("origin", ctx)).default_open(false).show(ui, |ui| {
        use qymcad_core::feature::BasePlane;
        // a component's base planes are selectable DATUMS: a click creates a sketch on that plane
        let mut new_on: Option<BasePlane> = None;
        for (nm, bp) in [(&crate::i18n::tr("plane-xy"), BasePlane::XY), (&crate::i18n::tr("plane-xz"), BasePlane::XZ), (&crate::i18n::tr("plane-yz"), BasePlane::YZ)] {
            if ui.selectable_label(false, format!("{} {nm}", ph::DOT)).on_hover_text(crate::i18n::tr("tree-base-plane-hint")).clicked() {
                new_on = Some(bp);
            }
        }
        ui.label(egui::RichText::new(format!("{} {}", ph::DOT, crate::i18n::tr("tree-origin"))).weak());
        if let Some(b) = new_on {
            tc.ask.push(qymcad_ui_state::TreeAsk::SketchOnBasePlane(b));
        }
    });

    // THE BODIES OF A PART, a row each: the pieces a split or a cut through left are told apart here, shown, hidden,
    // picked and made parts of their own, as the solid bodies of a part are listed in the professional systems
    //
    // THE LIST HOLDS STILL while the timeline is rebuilt: a body waiting for the step after it stays listed, as it stays
    // on screen (`qymcad_ui_state::consumed_bodies`), and a body made below the rollback bar is not listed while its
    // mesh is still on hand. Reported behaviour: the bar dragged up or down, the section came and went for as long as
    // the rebuild ran, and everything under it - the bar held by the cursor included - jumped by a row or two.
    if tc.project.ctx_holds_bodies(ctx) {
        let consumed = qymcad_ui_state::consumed_bodies(&*tc.project);
        let rolled_back: std::collections::HashSet<Id> = tc.project.timeline.iter().skip(tc.project.rollback_place()).flat_map(|n| n.kind.bodies()).collect();
        let rows: Vec<usize> = tc
            .project
            .component_bodies(ctx)
            .into_iter()
            .filter(|b| !consumed.contains(b) && !rolled_back.contains(b))
            .filter_map(|b| tc.project.mesh_index(b))
            .filter(|&mi| !tc.project.bodies[mi].sheet && tree_text_matches(&*tc.tree, &crate::i18n::name(&tc.project.mesh_name(mi))))
            .collect();
        if !rows.is_empty() {
            egui::CollapsingHeader::new(format!("{} {}", ph::CUBE, crate::i18n::tr1("tree-part-bodies", "n", &rows.len().to_string()))).id_salt(("part-bodies", ctx)).default_open(true).show(
                ui,
                |ui| {
                    for mi in rows {
                        tree_body_row(tc, ui, mi);
                    }
                },
            );
        }
    }

    // The sketches
    let sketches: Vec<Id> = ctx_nodes
        .iter()
        .filter(|(_, k)| matches!(k, FK::Sketch { .. }))
        .map(|(id, _)| *id)
        // THE SEARCH RUNS THROUGH EVERYTHING: a section is filtered by the same text its row displays
        .filter(|id| tc.project.sketch_index(*id).is_some_and(|si| tree_text_matches(&*tc.tree, &crate::i18n::name(&tc.project.sketches[si].name))))
        .collect();
    if !sketches.is_empty() {
        egui::CollapsingHeader::new(format!("{} {}", ph::POLYGON, crate::i18n::tr("tree-sketches"))).id_salt(("sketches", ctx)).default_open(true).show(ui, |ui| {
            for sid in sketches {
                tree_sketch_row(tc, ui, sid);
            }
        });
    }

    // The datums (user planes, points and axes)
    let datums: Vec<(Id, FK)> = ctx_nodes
        .iter()
        .filter(|(_, k)| matches!(k, FK::Plane { .. } | FK::DatumPoint { .. } | FK::DatumAxis { .. }))
        .filter(|(_, k)| tree_text_matches(&*tc.tree, &datum_row_name(&*tc.project, k)))
        .cloned()
        .collect();
    if !datums.is_empty() {
        egui::CollapsingHeader::new(format!("{} {}", ph::SELECTION_PLUS, crate::i18n::tr("tree-datums"))).id_salt(("datums", ctx)).default_open(true).show(ui, |ui| {
            for (id, k) in datums {
                tree_datum_row(tc, ui, id, &k);
            }
        });
    }

    // Bodies and history (the features that produce a body), in build order; plus the imported bodies in the root
    let feat_tis: Vec<usize> = (0..tc.project.timeline.len()).filter(|&ti| tc.project.timeline[ti].parent == Some(ctx) && tc.project.timeline[ti].kind.body().is_some()).collect();
    let imported: Vec<usize> = if ctx == tc.project.root {
        // ALL of a node's bodies, not the first one. Splitting a body yields several; only the first was
        // recognised, and the remaining pieces surfaced in the ROOT assembly as separate numbered rows - so
        // the tree showed bodies nobody had made.
        let produced: std::collections::HashSet<Id> = tc.project.timeline.iter().flat_map(|n| n.kind.bodies()).collect();
        (0..tc.project.bodies.len())
            .filter(|&mi| tc.project.mesh_id(mi).is_none_or(|b| !produced.contains(&b)))
            .filter(|&mi| tree_text_matches(&*tc.tree, &crate::i18n::name(&tc.project.mesh_name(mi))))
            .collect()
    } else {
        Vec::new()
    };
    if !feat_tis.is_empty() || !imported.is_empty() {
        egui::CollapsingHeader::new(format!("{} {}", ph::CUBE, crate::i18n::tr("tree-bodies"))).id_salt(("bodies", ctx)).default_open(true).show(ui, |ui| {
            // The rollback bar: it is dragged up and down the feature list. Everything BELOW the line is
            // suppressed (it does not build), everything above it is active. active_k is the number of active
            // features (those above the line).
            let rb = tc.project.rollback;
            let active_k = feat_tis.iter().filter(|&&ti| rb.is_none_or(|r| ti < r)).count();
            // ONE OPERATION IS ONE ROW: a multi-contour extrude or cut used to breed a node per contour
            // (plus BodyBoolean joins). An operation span is collapsed into one row (its first node); the
            // span's other nodes are not duplicated in the tree. It is edited in the shared half-sketcher
            // (by a double click).
            let mut shown: std::collections::HashSet<qymcad_core::model::Id> = std::collections::HashSet::new();
            // ITERATE BY Id, NOT BY INDEX IN THE TIMELINE. A tree row can delete a feature and swap it with
            // its neighbour - after which the indexes taken before the loop point somewhere else, and the last
            // one past the end of the timeline altogether. That surfaced as a crash: "len is 14 but the index
            // is 15". An Id survives both deletion and reordering; a node that has vanished is simply skipped -
            // there is nobody left to draw its row in this frame.
            let feat_ids: Vec<qymcad_core::model::Id> = feat_tis.iter().map(|&ti| tc.project.timeline[ti].id).collect();
            let searching = !tc.tree.search.trim().is_empty();
            for (idx, &id) in feat_ids.iter().enumerate() {
                if idx == active_k && !searching {
                    // THE ROLLBACK BAR IS NOT DRAWN WHILE SEARCHING: half the rows are hidden, and a line
                    // meaning "everything below is suppressed" would land in the middle of a filtered list,
                    // where it separates nothing and only lies.
                    rollback_bar(tc, ui, &feat_tis, active_k);
                }
                let Some(ti) = tc.project.timeline_index(id) else { continue };
                if shown.contains(&id) {
                    continue; // part of an operation already shown - the row is not duplicated
                }
                if !tree_row_matches(&*tc.project, &*tc.tree, ti) {
                    continue;
                }
                for s in tc.project.feature_op_span(id) {
                    shown.insert(s);
                }
                tree_feature_row(tc, ui, ti);
            }
            if active_k >= feat_tis.len() && !feat_tis.is_empty() {
                rollback_bar(tc, ui, &feat_tis, active_k); // everything is active, so the line sits at the very bottom
            }
            for mi in imported {
                tree_body_row(tc, ui, mi); // the imported bodies (STL and the like, with no source feature)
            }
        });
    }

    // The child components: a double click enters them
    let children: Vec<(usize, Id, String)> = tc
        .project
        .components
        .iter()
        .enumerate()
        .filter(|(_, c)| c.parent == Some(ctx))
        .map(|(ci, c)| (ci, c.id, crate::i18n::name(&c.name)))
        .filter(|(_, _, name)| tree_text_matches(&*tc.tree, name))
        .collect();
    // WHAT WAS DROPPED IS DECIDED AFTER THE WALK. `components` must not be changed in the middle of drawing
    // the list: the indexes would shift under the feet of that very loop.
    let mut drop_act: Option<(Id, Id, super::TreeDrop)> = None;
    let mut rows_geom: Vec<(Id, egui::Rect)> = Vec::new();
    // A PATTERN OF COMPONENTS IS ONE ROW, right under its source, and its copies stand under it, indented: the
    // pattern is one operation, and the copies are its product rather than parts placed one by one
    let patterns: Vec<(usize, Id, Vec<Id>)> = (0..tc.project.timeline.len())
        .filter(|&ti| tc.project.timeline[ti].parent == Some(ctx))
        .filter_map(|ti| match &tc.project.timeline[ti].kind {
            FK::ComponentPattern { src, copies, .. } => Some((ti, *src, copies.clone())),
            _ => None,
        })
        .collect();
    let mut rows: Vec<TreeCompRow> = Vec::new();
    for (ci, cid, name) in &children {
        if patterns.iter().any(|(_, _, copies)| copies.contains(cid)) {
            continue; // drawn under its pattern
        }
        rows.push(TreeCompRow::Comp(*ci, *cid, name.clone(), false));
        for (ti, _, copies) in patterns.iter().filter(|(_, src, _)| src == cid) {
            rows.push(TreeCompRow::Pattern(*ti));
            rows.extend(children.iter().filter(|(_, c, _)| copies.contains(c)).map(|(ci, c, n)| TreeCompRow::Comp(*ci, *c, n.clone(), true)));
        }
    }
    if !children.is_empty() {
        let mut fold = false;
        let listed: Vec<(Id, bool)> = rows
            .iter()
            .filter_map(|row| match row {
                TreeCompRow::Comp(ci, cid, _, _) => tc.project.components.get(*ci).map(|c| (*cid, c.visible)),
                _ => None,
            })
            .collect();

        let listed_count = listed.len();
        let shown_count = listed.iter().filter(|(_, vis)| *vis).count();

        let mut all_checked = shown_count == listed_count;
        let mixed = shown_count != 0 && !all_checked;

        let state = egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), ui.make_persistent_id(("comps", ctx)), true);

        let mut header = state.show_header(ui, |ui| {
            // the hover fill goes under the words, so its place in the paint order is kept before they are drawn
            let fill = ui.painter().add(egui::Shape::Noop);
            let toggle_all_checkbox = egui::Checkbox::without_text(&mut all_checked).indeterminate(mixed);
            let toggle_all = ui.add(toggle_all_checkbox).on_hover_text(crate::i18n::tr("tree-components-all-hint"));

            if toggle_all.changed() {
                for &(cid, _) in &listed {
                    crate::gui::set_component_visible(&mut *tc.project, &mut *tc.regen, cid, all_checked);
                }
            }

            let after_tick = toggle_all.rect.max.x + ui.spacing().item_spacing.x * 0.5;
            // in the colour of the other branches' headings (a widget's text), not the plainer colour of a label: the
            // heading of the components stood greyer than Origin beside it
            let heading_text =
                egui::RichText::new(format!("{} {}", ph::STACK, crate::i18n::tr("tree-components"))).text_style(egui::TextStyle::Button).color(ui.visuals().widgets.inactive.text_color());
            ui.add(egui::Label::new(heading_text).selectable(false));

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.weak(crate::i18n::tr2("tree-components-count", "shown", &shown_count.to_string(), "n", &listed_count.to_string()));
            });
            // THE WHOLE LINE PAST THE TICK FOLDS THE BRANCH, lit under the pointer, as every other heading of the tree:
            // the word alone answered a click, and one between the word and the count did nothing
            let line = egui::Rect::from_min_max(egui::pos2(after_tick, ui.min_rect().min.y), egui::pos2(ui.max_rect().max.x, ui.min_rect().max.y));
            let row = ui.interact(line, ui.id().with("comps-fold"), egui::Sense::click());
            if row.hovered() {
                let look = ui.visuals().widgets.hovered;
                ui.painter().set(fill, egui::Shape::rect_filled(line, look.corner_radius, look.weak_bg_fill));
            }
            fold = row.clicked();
        });

        if fold {
            header.toggle();
        }

        header.body(|ui| {
            for row in rows {
                let (ci, cid, name, indented) = match row {
                    TreeCompRow::Pattern(ti) => {
                        tree_feature_row(tc, ui, ti);
                        continue;
                    }
                    TreeCompRow::Comp(ci, cid, name, indented) => (ci, cid, name, indented),
                };
                let is_asm = tc.project.component_kind(cid) == Some(ComponentKind::Assembly);
                // A CLONE SAYS SO: its own icon, and the word beside the name - an icon alone reads as a part
                let origin = tc.project.instance_origin(cid);
                let icon = if is_asm {
                    ph::STACK
                } else if origin != cid {
                    ph::CUBE_TRANSPARENT
                } else {
                    ph::CUBE
                };
                ui.horizontal(|ui| {
                    if indented {
                        ui.add_space(ui.spacing().indent); // a copy of the pattern above
                    }
                    let mut vis = tc.project.components.iter().find(|c| c.id == cid).map(|c| c.visible).unwrap_or(true);
                    if ui.add(egui::Checkbox::without_text(&mut vis)).on_hover_text(crate::i18n::tr("tree-component-visible-hint")).changed() {
                        crate::gui::set_component_visible(&mut *tc.project, &mut *tc.regen, cid, vis);
                    }
                    if rename_node_active(tc, ui, RenameNode::Component(cid)) {
                        return; // inline renaming: a field instead of the label
                    }
                    let in_multi = crate::gui::is_multi(&*tc.project, *tc.sel, &*tc.tree_sel) && tc.tree_sel.multi.contains(&cid);
                    // THE ROW: A CLICK SELECTS, A DOUBLE CLICK ENTERS, PRESS AND DRAG MOVES IT.
                    //
                    // This used to be `dnd_drag_source`, and it took the press for itself: a click stopped
                    // selecting and a double click stopped entering the part. Reported behaviour: a selection
                    // could not be made by a click at all, the row was immediately grabbed for dragging, and
                    // no other program works that way. Quite so. The dragging is assembled by hand on top of
                    // an ordinary row: `click_and_drag` leaves the click and the double click in place, and
                    // dragging only begins once the cursor has moved with the button held.
                    //
                    // THE GRABBED ROW TRAVELS WITH THE CURSOR - that same row, not a text in a popup.
                    // Reported behaviour: while holding, there was no sign at all that an item had been picked
                    // up and was being moved; and a popup with a text instead of the row was rejected
                    // separately. What is wanted is THE ITEM ITSELF travelling under the cursor. An overlay
                    // area paints the same row at the cursor while the original row keeps click and double-click.
                    let carried = tc.tree.drag.is_some_and(|src| src == cid || (tc.tree_sel.multi.contains(&src) && tc.tree_sel.multi.contains(&cid)));
                    let picked = *tc.sel == Sel::Component(ci) || in_multi;
                    let row = |ui: &mut egui::Ui| ui.selectable_label(picked, format!("{icon} {name}")).interact(egui::Sense::click_and_drag()).on_hover_text(crate::i18n::tr("tree-component-hint"));
                    let resp = row(ui);
                    if carried {
                        if let Some(at) = ui.ctx().pointer_interact_pos() {
                            let position = at - egui::vec2(0.0, resp.rect.height() / 2.0);
                            egui::Area::new(ui.id().with(("carry", cid))).order(egui::Order::Tooltip).fixed_pos(position).interactable(false).fade_in(false).show(ui.ctx(), |ui| {
                                let _ = ui.selectable_label(picked, format!("{icon} {name}"));
                            });
                        }
                    }
                    if origin != cid {
                        let of = tc.project.components.iter().find(|c| c.id == origin).map(|c| crate::i18n::name(&c.name)).unwrap_or_default();
                        ui.weak(crate::i18n::tr("tree-clone-mark")).on_hover_text(crate::i18n::tr1("tree-clone-hint", "name", &of));
                    }
                    if resp.double_clicked() {
                        tc.ask.push(qymcad_ui_state::TreeAsk::EnterComponent(cid));
                    } else if resp.clicked() {
                        let click = qymcad_ui_state::TreeClick::of(&ui.input(|i| i.modifiers));
                        tree_select_component(&*tc.project, &mut *tc.sel, &mut *tc.tree_sel, ci, cid, click);
                    }
                    if resp.drag_started() {
                        tc.tree.drag = Some(cid);
                    }
                    // A HIT IS COMPUTED FROM THE CURSOR'S COORDINATE, NOT FROM `hovered()`.
                    //
                    // While a row is being dragged, egui gives the hover to IT - the neighbouring rows never
                    // get `hovered()` at all. That is why none of it worked: neither the highlight nor the drop.
                    // The row rectangles are gathered here and the decision is taken after the walk.
                    rows_geom.push((cid, resp.rect));
                    let mut act: Option<u8> = None; // 1 copy, 2 cut, 3 paste, 6 rename, 7 save as a part, 8 delete
                    let mut export: Option<crate::gui::export_menu::ExportChoice> = None;
                    let multi_n = if in_multi { tc.tree_sel.multi.len() } else { 0 };
                    resp.context_menu(|ui| {
                        // EDITING A COMPONENT PATTERN starts here, as editing a feature in the timeline does.
                        // The edit function had been written and covered by a test but was NOT CONNECTED to the
                        // interface: the test called it directly and nobody else could reach it. It surfaced
                        // through the compiler's "never used" warning - exactly the case where such a warning
                        // must not be silenced.
                        if let Some(pid) = tc.project.comp_pattern_of(cid).map(|p| p.id) {
                            if ui.button(format!("{} {}", ph::DOTS_THREE_OUTLINE, crate::i18n::tr("act-edit-array"))).clicked() {
                                tc.ask.push(qymcad_ui_state::TreeAsk::EditCompArray(pid));
                                ui.close();
                            }
                            ui.separator();
                        }
                        // THE ROOT IS NOT RENAMED: its name is a catalogue key rather than the document's text
                        // (see `Project::migrate_root`). Offering the edit and silently reverting it on the next
                        // load would be worse than not offering it at all.
                        if cid != tc.project.root && ui.button(format!("{} {}", ph::TEXT_T, crate::i18n::tr("act-rename"))).clicked() {
                            act = Some(6);
                            ui.close();
                        }
                        ui.separator();
                        let (cl, xl) = if multi_n > 1 {
                            (
                                format!("{} {}", ph::COPY, crate::i18n::tr1("act-copy-selected", "n", &multi_n.to_string())),
                                format!("{} {}", ph::SCISSORS, crate::i18n::tr1("act-cut-selected", "n", &multi_n.to_string())),
                            )
                        } else {
                            (format!("{} {}", ph::COPY, crate::i18n::tr("act-copy-ctrl-c")), format!("{} {}", ph::SCISSORS, crate::i18n::tr("act-cut-ctrl-x")))
                        };
                        if ui.button(cl).clicked() {
                            act = Some(1);
                            ui.close();
                        }
                        if ui.button(xl).clicked() {
                            act = Some(2);
                            ui.close();
                        }
                        if (tc.clip.tree.is_some() || tc.clip.tree_multi.is_some()) && ui.button(format!("{} {}", ph::CLIPBOARD, crate::i18n::tr("act-paste-here"))).clicked() {
                            act = Some(3);
                            ui.close();
                        }
                        if !is_asm && ui.button(format!("{} {}", ph::CUBE_TRANSPARENT, crate::i18n::tr("act-clone-part"))).on_hover_text(crate::i18n::tr("act-clone-part-hint")).clicked() {
                            act = Some(9);
                            ui.close();
                        }
                        ui.separator();
                        if let Some(choice) = crate::gui::export_menu::export_submenu(ui, crate::gui::export_menu::ExportFrom::Component) {
                            export = Some(choice);
                            ui.close();
                        }
                        ui.separator();
                        if ui.button(format!("{} {}", ph::PACKAGE, crate::i18n::tr("act-save-as-part"))).on_hover_text(crate::i18n::tr("tree-save-as-part-hint")).clicked() {
                            act = Some(7);
                            ui.close();
                        }
                        // THE WAY OUT, BY THE SAME QUESTION AS EVERYWHERE ELSE. Reported behaviour: the menu
                        // offers Cut and Copy and no Delete, so the one destructive action of the three had
                        // to be reached another way - select the row, find the Del key. The item goes
                        // through `ask_delete`, not through the kernel: a second route to one action must
                        // decide nothing of its own. The root is left out, as with renaming: it cannot be
                        // deleted, and an item that does nothing is worse than none.
                        if cid != tc.project.root {
                            ui.separator();
                            if ui.button(format!("{} {}", ph::TRASH, crate::i18n::tr("act-delete-part"))).clicked() {
                                act = Some(8);
                                ui.close();
                            }
                        }
                    });
                    match export {
                        Some(crate::gui::export_menu::ExportChoice::Exact(f)) => tc.ask.push(qymcad_ui_state::TreeAsk::ExportExact(f, cid)),
                        Some(crate::gui::export_menu::ExportChoice::Mesh(f)) => *tc.mesh_export = Some((f, ExportTarget::Component(cid))),
                        None => {}
                    }
                    match act {
                        Some(1) => {
                            *tc.sel = Sel::Component(ci);
                            tc.ask.push(qymcad_ui_state::TreeAsk::Clipboard { cut: false });
                        }
                        Some(2) => {
                            *tc.sel = Sel::Component(ci);
                            tc.ask.push(qymcad_ui_state::TreeAsk::Clipboard { cut: true });
                        }
                        Some(3) => {
                            *tc.sel = Sel::Component(ci);
                            tc.ask.push(qymcad_ui_state::TreeAsk::Paste);
                        }
                        Some(6) => qymcad_ui_state::start_rename_node(&mut *tc.rename, RenameNode::Component(cid), name.clone()),
                        Some(7) => tc.ask.push(qymcad_ui_state::TreeAsk::SavePart(cid)),
                        Some(9) => {
                            // a clone beside its original, in the same assembly, as one step of undo
                            let parent = tc.project.components.iter().find(|c| c.id == cid).and_then(|c| c.parent).unwrap_or(tc.project.root);
                            let made = qymcad_ui_state::edit_over(tc.rebuild(), crate::i18n::tr("act-clone-part")).project().clone_part(cid, parent);
                            if let Some(ci) = made.and_then(|cl| tc.project.components.iter().position(|c| c.id == cl)) {
                                *tc.sel = Sel::Component(ci);
                                tc.ask.push(qymcad_ui_state::TreeAsk::Resync);
                            }
                        }
                        Some(8) => {
                            *tc.sel = Sel::Component(ci);
                            qymcad_ui_state::ask_delete(&mut *tc.deferred, Sel::Component(ci));
                        }
                        _ => {}
                    }
                });
            }
        });
    }
    // THE TARGET IS FOUND BY THE CURSOR, A HINT IS DRAWN, AND ON RELEASE THE ROW IS DROPPED.
    //
    // AUTO-SCROLLING AT THE EDGE. The list is longer than the window, so the drop target may be beyond its
    // edge; a person brings the cursor there and expects the list to move on its own. The speed is constant:
    // "the closer to the edge, the faster" would only make it harder to hit here.
    if tc.tree.drag.is_some() {
        if let Some(at) = ui.ctx().input(|i| i.pointer.interact_pos().or(i.pointer.hover_pos())) {
            const EDGE: f32 = 24.0; // the band at the edge in which scrolling starts
            const SPEED: f32 = 8.0; // points per frame
            let clip = ui.clip_rect();
            if at.x >= clip.left() && at.x <= clip.right() {
                // The sign: a positive delta moves THE CONTENT downwards, that is, reveals what is above.
                // At the bottom edge the opposite is wanted.
                let d = if at.y > clip.bottom() - EDGE {
                    -SPEED
                } else if at.y < clip.top() + EDGE {
                    SPEED
                } else {
                    0.0
                };
                if d != 0.0 {
                    ui.scroll_with_delta(egui::vec2(0.0, d));
                    ui.ctx().request_repaint(); // the scroll runs frame by frame rather than in one jerk
                }
            }
        }
    }
    // ESCAPE CANCELS THE DRAG, as in any tree. Without it a grabbed row has nowhere to go but to be dropped
    // somewhere.
    if tc.tree.drag.is_some() && ui.ctx().input(|i| i.key_pressed(egui::Key::Escape)) {
        tc.tree.drag = None;
        *tc.status = Notice::info(crate::i18n::tr("tree-drag-cancelled"));
    }
    if let Some(src) = tc.tree.drag {
        if let Some(at) = ui.ctx().input(|i| i.pointer.interact_pos().or(i.pointer.hover_pos())) {
            if let Some((cid, rect)) = rows_geom.iter().copied().find(|(cid, r)| *cid != src && at.y >= r.top() && at.y <= r.bottom()) {
                let how = super::tree_drop_intent(rect, at.y);
                // AN INVALID TARGET GETS NEITHER A LINE NOR A HIGHLIGHT. Otherwise a person aims at a place
                // where nothing will happen and finds out only after releasing.
                let moving: Vec<Id> = if tc.tree_sel.multi.contains(&src) { tc.tree_sel.multi.clone() } else { vec![src] };
                if !tc.project.tree_drop_allowed(&moving, cid, how == super::TreeDrop::Onto) {
                    tc.tree.row_rects = rows_geom;
                    if ui.ctx().input(|i| i.pointer.any_released()) {
                        tc.tree.drag = None;
                    }
                    return;
                }
                // BETWEEN ITEMS there is a full-width coloured insertion line; OVER AN ITEM the item itself
                // is highlighted. The ordinary behaviour of a tree.
                let full = egui::Rect::from_min_max(egui::pos2(ui.min_rect().left(), rect.top()), egui::pos2(ui.min_rect().right(), rect.bottom()));
                let col = ui.visuals().selection.bg_fill;
                let pnt = ui.painter();
                match how {
                    super::TreeDrop::Before | super::TreeDrop::After => {
                        let y = if how == super::TreeDrop::Before { full.top() } else { full.bottom() };
                        pnt.line_segment([egui::pos2(full.left(), y), egui::pos2(full.right(), y)], egui::Stroke::new(3.0, col));
                    }
                    super::TreeDrop::Onto => {
                        pnt.rect_filled(full.expand(1.0), 3.0, col.linear_multiply(0.25));
                        pnt.rect_stroke(full.expand(1.0), 3.0, egui::Stroke::new(2.0, col), egui::StrokeKind::Middle);
                    }
                }
                if ui.input(|i| i.pointer.any_released()) {
                    drop_act = Some((src, cid, how));
                }
            }
        }
    }
    // THE ROW RECTANGLES ARE FOR THE TESTS: a test moves the mouse over the REAL coordinates of the rows.
    tc.tree.row_rects = rows_geom;
    // THE DROP IS APPLIED AFTER THE WALK - otherwise the reorder would shift the indexes under that same loop.
    if let Some((src, dst, how)) = drop_act {
        tc.ask.push(qymcad_ui_state::TreeAsk::Drop { dragged: src, target: dst, how });
    }
    // THE BUTTON WAS RELEASED, SO THE DRAG IS OVER, wherever that happened. Otherwise a grabbed row would
    // stay grabbed forever and the next click in the tree would act as a drop.
    if ui.input(|i| i.pointer.any_released()) {
        tc.tree.drag = None;
    }
}
