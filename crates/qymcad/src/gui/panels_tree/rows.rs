//! THE ROWS OF THE TREE: the labels of features and datums, the rows of sketches, features, bodies and datums, what a search matches.

use super::*;

/// WHAT A FEATURE'S SET IS DEFINED BY - IN WORDS, not as a number.
///
/// Reported behaviour: the right panel did not show this at all, leaving no way to tell what had been
/// clicked together. A count like "edges: 4" describes a manual pick and lies about a description: tomorrow
/// there will be five. So a description shows THE DESCRIPTION ITSELF - that is what the document records.
pub(crate) fn ref_summary(r: &qymcad_core::refs::Ref) -> String {
    use qymcad_core::refs::Query;
    let n = r.query.picked_descs().len();
    match &r.query {
        Query::Id(0) => crate::i18n::tr("all-of-them"),
        q if q.is_pick_list() => {
            if n == 0 {
                crate::i18n::tr("all-of-them")
            } else {
                crate::i18n::tr1("count-edges", "n", &n.to_string())
            }
        }
        Query::Adjacent(_) => crate::i18n::tr("expand-face-edges"),
        Query::Between(_, _) => crate::i18n::tr("expand-between-done"),
        Query::TangentChain { .. } => crate::i18n::tr("expand-tangent-chain"),
        Query::Oriented { .. } => crate::i18n::tr("expand-parallel"),
        Query::Extreme { .. } => crate::i18n::tr("expand-topmost"),
        Query::Largest => crate::i18n::tr("expand-largest"),
        Query::OfFeature { .. } => crate::i18n::tr("expand-feature-faces"),
        _ => crate::i18n::tr("expand-described-short"),
    }
}

/// WHETHER THE DISPLAYED TEXT MATCHES THE SEARCH. One matcher for ALL the sections of the tree.
///
/// The first version filtered only the bodies-and-history section, and that was fairly called a stub: in an
/// assembly the search found neither parts nor subassemblies - that is, exactly what one searches an assembly
/// for. A search that looks in one section out of five is worse than none: it looks like it works.
pub(crate) fn tree_text_matches(tree: &super::TreeUi, text: &str) -> bool {
    let q = tree.search.trim().to_lowercase();
    q.is_empty() || text.to_lowercase().contains(&q)
}

/// A FEATURE'S LABEL IN THE TREE - one label for everyone who shows it or searches by it.
///
/// It was lifted out of the tree row for the sake of the search: a person searches by what they SEE, and if
/// the search assembled the label its own way it would stop finding what is displayed. The same class of
/// A row of the components' list: a component (indented when it is a copy of the pattern above it), or the row of a
/// pattern of components, its timeline node.
pub(crate) enum TreeCompRow {
    Comp(usize, Id, String, bool),
    Pattern(usize),
}

/// divergence already caught on the localisation keys and on the settings table: two places knowing one thing.
pub(crate) fn feature_row_label(project: &qymcad_core::model::Project, ti: usize) -> String {
    use qymcad_core::feature::FeatureKind;
    let Some(node) = project.timeline.get(ti) else { return String::new() };
    let kind = node.kind.clone();
    // THE SIZES THE REBUILD USED. A size given by a formula is counted from it; the number kept in the node is what
    // was typed last and does not follow a parameter. Reported behaviour: a parameter h changed from 20 to 40, the
    // body was 40 high and the row still said h=20.0.
    let vars = project.param_map();
    let at = |key: &str, stored: f64| project.feat_dim_value(node.id, key, stored, &vars);
    let mut lbl = match kind {
        FeatureKind::Extrude { height, .. } => format!("{} {}", ph::CUBE, crate::i18n::tr1("feat-extrude", "h", &crate::i18n::num(at("height", height), 1))),
        FeatureKind::Revolve { angle, .. } => format!("{} {}", ph::CUBE, crate::i18n::tr1("feat-revolve", "angle", &crate::i18n::num(at("angle", angle), 0))),
        FeatureKind::Sweep { .. } => format!("{} {}", ph::CUBE, crate::i18n::tr("feat-sweep")),
        FeatureKind::Loft { ref sketches, src, op, surface, .. } => {
            // A SURFACE IS VISIBLE IN THE TREE: a node with caps and a node without them are different
            // things, and one shared row on both would force the feature open just to learn what was built.
            let suf = if surface {
                crate::i18n::tr("feat-suffix-surface")
            } else if src == 0 {
                String::new()
            } else {
                [crate::i18n::tr("feat-suffix-cut"), crate::i18n::tr("feat-suffix-union"), crate::i18n::tr("feat-suffix-intersect")].get(op as usize).cloned().unwrap_or_default()
            };
            format!("{} {}{}", ph::STACK, crate::i18n::tr1("feat-loft", "n", &sketches.len().to_string()), suf)
        }
        FeatureKind::PushFace { dist, .. } => format!("{} {}", ph::ARROWS_OUT_LINE_VERTICAL, crate::i18n::tr1("feat-push-face", "d", &crate::i18n::num_signed(at("dist", dist), 1))),
        FeatureKind::Trim { .. } => format!("{} {}", ph::SCISSORS, crate::i18n::tr("feat-trim")),
        FeatureKind::Stitch { ref parts, .. } => format!("{} {}", ph::INTERSECT_SQUARE, crate::i18n::tr1("feat-stitch", "n", &parts.len().to_string())),
        // A BODY MADE OF A MESH THAT DID NOT CLOSE says so in its own row; the viewport shows where
        FeatureKind::MeshSolid { body, .. } => {
            format!("{} {}", ph::CUBE, crate::i18n::tr(if project.mesh_index(body).is_some_and(|mi| project.bodies[mi].sheet) { "feat-mesh-solid-open" } else { "feat-mesh-solid" }))
        }
        FeatureKind::MeshRecognised { body, .. } => {
            format!("{} {}", ph::CUBE, crate::i18n::tr(if project.mesh_index(body).is_some_and(|mi| project.bodies[mi].sheet) { "feat-mesh-recognised-open" } else { "feat-mesh-recognised" }))
        }
        FeatureKind::Patch { ref edges, .. } => format!("{} {}", ph::BANDAIDS, crate::i18n::tr1("feat-patch", "n", &edges.query.picked_descs().len().to_string())),
        FeatureKind::SurfaceReplace { ref faces, .. } => format!("{} {}", ph::SWAP, crate::i18n::tr1("feat-surface-replace", "n", &faces.query.picked_descs().len().to_string())),
        FeatureKind::FaceCopy { ref faces, .. } => format!("{} {}", ph::COPY_SIMPLE, crate::i18n::tr1("feat-face-copy", "n", &faces.query.picked_descs().len().to_string())),
        FeatureKind::OffsetSurface { dist, .. } => format!("{} {}", ph::SELECTION_FOREGROUND, crate::i18n::tr1("feat-offset-surface", "d", &crate::i18n::num(at("dist", dist), 1))),
        FeatureKind::RemoveFace { ref faces, .. } => format!("{} {}", ph::ERASER, crate::i18n::tr1("feat-remove-face", "n", &faces.query.picked_descs().len().to_string())),
        FeatureKind::SplitFace { offset, .. } => {
            let offset = at("offset", offset);
            let off = if offset.abs() < 1e-9 { String::new() } else { format!(" {offset:+.1}") };
            format!("{} {}{off}", ph::GRID_FOUR, crate::i18n::tr("feat-split-face"))
        }
        FeatureKind::Thicken { thickness, .. } => format!("{} {}", ph::STACK_SIMPLE, crate::i18n::tr1("feat-thicken", "d", &crate::i18n::num_signed(at("thickness", thickness), 1))),
        FeatureKind::PartInstance { src_comp, .. } => {
            let name = project.components.iter().find(|c| c.id == src_comp).map(|c| crate::i18n::name(&c.name)).unwrap_or_default();
            // NOT AN ASSEMBLY ICON. This used to be `ph::STACK` - the very icon that marks ASSEMBLIES in the
            // tree and labels the "new subassembly" button. Reported behaviour: an Instance inside carrying an
            // assembly icon that cannot be entered. Fairly so: the icon promised a node one enters, while this
            // is a copied body with nothing to enter.
            format!("{} {}", ph::CUBE_TRANSPARENT, crate::i18n::tr1("feat-part-instance", "name", &name))
        }
        // A PATTERN OF COMPONENTS: its name and how many instances it lays, the source among them
        FeatureKind::ComponentPattern { ref kind, .. } => {
            format!("{} {}", crate::gui::feat_icon(&node.kind), crate::i18n::tr2("feat-comp-pattern", "name", &crate::i18n::name(&node.name), "n", &kind.count().to_string()))
        }
        FeatureKind::SplitBody { ref bodies, offset, .. } => {
            let offset = at("offset", offset);
            let off = if offset.abs() < 1e-9 { String::new() } else { format!(" {offset:+.1}") };
            format!("{} {}{}", ph::SQUARE_SPLIT_HORIZONTAL, crate::i18n::tr1("feat-split-body", "n", &bodies.len().to_string()), off)
        }
        FeatureKind::Draft { ref faces, angle, .. } => {
            format!("{} {}", ph::ANGLE, crate::i18n::trn("feat-draft", &[("angle", &crate::i18n::num(at("angle", angle), 0)), ("n", &faces.query.picked_descs().len().to_string())]))
        }
        FeatureKind::Box3 { dx, dy, dz, .. } => {
            format!("{} {}", ph::CUBE, crate::i18n::trn("feat-box", &[("x", &crate::i18n::num(at("dx", dx), 0)), ("y", &crate::i18n::num(at("dy", dy), 0)), ("z", &crate::i18n::num(at("dz", dz), 0))]))
        }
        FeatureKind::Cylinder { r, h, .. } => {
            format!("{} {}", ph::CYLINDER, crate::i18n::trn("feat-cylinder", &[("d", &crate::i18n::num(2.0 * at("r", r), 0)), ("h", &crate::i18n::num(at("h", h), 0))]))
        }
        FeatureKind::Sphere { r, .. } => format!("{} {}", ph::CIRCLE, crate::i18n::tr1("feat-sphere", "d", &crate::i18n::num(2.0 * at("r", r), 0))),
        FeatureKind::Combine { op, height, .. } => {
            // the icon follows THE OPERATION: scissors only for a cut; a boss ADDS material (a cube);
            // an intersection has its own (everything used to be scissors, and a boss looked like a cut)
            let ic = [ph::SCISSORS, ph::CUBE, ph::INTERSECT][(op as usize).min(2)];
            format!(
                "{} {} h={}",
                ic,
                [crate::i18n::tr("bool-cut"), crate::i18n::tr("bool-boss"), crate::i18n::tr("bool-intersect-short")][(op as usize).min(2)],
                crate::i18n::num(at("height", height), 1)
            )
        }
        FeatureKind::Fillet { radius, ref edges, .. } => {
            format!("{} {}", ph::CIRCLE, crate::i18n::trn("feat-fillet", &[("r", &crate::i18n::num(at("radius", radius), 1)), ("which", &ref_summary(edges))]))
        }
        FeatureKind::Chamfer { dist, ref edges, mode, d2, .. } => {
            use qymcad_core::feature::ChamferMode;
            let dist = at("dist", dist);
            let d2 = at("d2", d2);
            let size = match mode {
                ChamferMode::TwoDist => format!("{dist:.1}×{d2:.1}"),
                ChamferMode::DistAngle => crate::i18n::trn("feat-chamfer-dist-angle", &[("d", &crate::i18n::num(dist, 1)), ("angle", &crate::i18n::num(d2, 0))]),
                ChamferMode::Symmetric => format!("{dist:.1}"),
            };
            format!("{} {}", ph::TRIANGLE, crate::i18n::trn("feat-chamfer", &[("size", &size), ("which", &ref_summary(edges))]))
        }
        FeatureKind::Cone { r1, r2, h, .. } => {
            format!(
                "{} {}",
                ph::CUBE,
                crate::i18n::trn("feat-cone", &[("d1", &crate::i18n::num(2.0 * at("r1", r1), 0)), ("d2", &crate::i18n::num(2.0 * at("r2", r2), 0)), ("h", &crate::i18n::num(at("h", h), 0))])
            )
        }
        FeatureKind::Torus { major, minor, .. } => {
            format!("{} {}", ph::CIRCLE, crate::i18n::trn("feat-torus", &[("r", &crate::i18n::num(at("major", major), 0)), ("r2", &crate::i18n::num(at("minor", minor), 0))]))
        }
        FeatureKind::Prism { r, n, h, .. } => {
            format!("{} {}", ph::HEXAGON, crate::i18n::trn("feat-prism", &[("n", &n.to_string()), ("d", &crate::i18n::num(2.0 * at("r", r), 0)), ("h", &crate::i18n::num(at("h", h), 0))]))
        }
        FeatureKind::Shell { thickness, ref faces, .. } => {
            format!("{} {}", ph::BOUNDING_BOX, crate::i18n::trn("feat-shell", &[("t", &crate::i18n::num(at("thickness", thickness), 1)), ("n", &faces.query.picked_descs().len().to_string())]))
        }
        FeatureKind::LinearArray { count, count2, .. } => {
            format!("{} {}", ph::DOTS_THREE_OUTLINE, crate::i18n::tr1("feat-linear-array", "n", &if count2 > 1 { format!("×{count}×{count2}") } else { format!("×{count}") }))
        }
        FeatureKind::CircularArray { count, angle, .. } => {
            format!("{} {}", ph::ARROWS_CLOCKWISE, crate::i18n::trn("feat-circular-array", &[("n", &count.to_string()), ("angle", &crate::i18n::num(at("angle", angle), 0))]))
        }
        FeatureKind::Mirror { plane, datum, .. } => format!(
            "{} {}",
            ph::FLIP_HORIZONTAL,
            crate::i18n::tr1("feat-mirror", "plane", &if datum != 0 { crate::i18n::tr("ref-datum") } else { ["XY", "XZ", "YZ"][(plane as usize).min(2)].to_string() })
        ),
        FeatureKind::Hole { diameter, depth, sketch, .. } => {
            if sketch != 0 {
                let n = project.sketch_isolated_points(sketch).len();
                format!(
                    "{} {}",
                    ph::CIRCLE,
                    crate::i18n::trn("feat-holes", &[("n", &n.to_string()), ("d", &crate::i18n::num(at("diameter", diameter), 1)), ("h", &crate::i18n::num(at("depth", depth), 1))])
                )
            } else {
                format!("{} {}", ph::CIRCLE, crate::i18n::trn("feat-hole", &[("d", &crate::i18n::num(at("diameter", diameter), 1)), ("h", &crate::i18n::num(at("depth", depth), 1))]))
            }
        }
        FeatureKind::BodyBoolean { op, .. } => {
            format!("{} {}", ph::INTERSECT, [crate::i18n::tr("feat-body-cut"), crate::i18n::tr("feat-body-union"), crate::i18n::tr("feat-body-intersect")][(op as usize).min(2)])
        }
        FeatureKind::Move { .. } => format!("{} {}", ph::ARROWS_OUT_CARDINAL, crate::i18n::tr("feat-move")),
        // a piece made a part of its own: the part it came from, whose cut still shapes it
        FeatureKind::Piece { src, .. } => {
            let from = project.body_owner(src).and_then(|p| project.components.iter().find(|c| c.id == p)).map(|c| crate::i18n::name(&c.name)).unwrap_or_default();
            format!("{} {}", ph::CUBE, crate::i18n::tr1("feat-piece", "part", &from))
        }
        FeatureKind::MirrorPart { .. } => format!("{} {}", ph::FLIP_HORIZONTAL, crate::i18n::tr("feat-mirror-part")),
        FeatureKind::Thread { spec, length, .. } => {
            let g = spec.geometry();
            let name = match spec.standard {
                qymcad_core::thread::ThreadStandard::MetricIso => format!("M{:.0}×{:.2}", g.major_d, g.pitch),
                qymcad_core::thread::ThreadStandard::TrapezoidalTr => format!("Tr{:.0}×{:.1}", g.major_d, g.pitch),
                qymcad_core::thread::ThreadStandard::RoundRd => format!("Rd{:.0}×{:.1}", g.major_d, g.pitch),
                _ => format!("Ø{:.1}×{:.2}", g.major_d, g.pitch),
            };
            format!(
                "{} {}{}",
                ph::SPIRAL,
                crate::i18n::trn(
                    "feat-thread",
                    &[
                        ("name", &name),
                        ("side", &if spec.internal { crate::i18n::tr("thread-internal") } else { crate::i18n::tr("thread-external") }),
                        ("len", &crate::i18n::num(at("length", length), 0))
                    ]
                ),
                if spec.starts > 1 { crate::i18n::tr1("count-starts", "n", &spec.starts.to_string()) } else { String::new() }
            )
        }
        FeatureKind::Auger { spec, length, .. } => format!(
            "{} {}",
            ph::SPIRAL,
            crate::i18n::trn("feat-auger", &[("d", &crate::i18n::num(spec.outer_d, 0)), ("pitch", &crate::i18n::num(spec.pitch, 0)), ("len", &crate::i18n::num(at("length", length), 0))])
        ),
        // an import's row names the file it came from: editing it asks again about the file's units and scale
        FeatureKind::Import { source, .. } => {
            format!("{} {}", ph::FILE_ARROW_DOWN, crate::i18n::tr1("feat-import", "file", &project.sources.iter().find(|s| s.id == source).map(|s| s.name.clone()).unwrap_or_default()))
        }
        FeatureKind::MeshPiece { source, .. } => {
            format!("{} {}", ph::FILE_ARROW_DOWN, crate::i18n::tr1("feat-mesh-piece", "file", &project.sources.iter().find(|s| s.id == source).map(|s| s.name.clone()).unwrap_or_default()))
        }
        // a kind of feature that has no row of its own here (sketches and datums have their own rows)
        _ => return String::new(),
    };
    // a RENAMED feature (its name differs from the default) shows its name; otherwise the automatic label with the sizes
    let custom_name = project.timeline[ti].name != crate::gui::feat_default_name(&kind);
    if custom_name {
        lbl = format!("{} {}", crate::gui::feat_icon(&kind), crate::i18n::name(&project.timeline[ti].name));
    }
    lbl
}

/// THE DATUM'S NAME AS THE ROW SHOWS IT - for the search. One place shared with the row itself: were it
/// assembled separately, the search would stop finding what is displayed (this class of fault has been
/// caught four times).
pub(crate) fn datum_row_name(project: &qymcad_core::model::Project, kind: &qymcad_core::feature::FeatureKind) -> String {
    use qymcad_core::feature::FeatureKind as FK;
    // WRITTEN OUT RATHER THAN THROUGH A HELPER. There used to be a `by` closure taking `&dyn Fn(&Self, Id)`,
    // and it existed only to save repeating `.unwrap_or_default()` three times. Once the application was no
    // longer passed about, the helper cost more than it saved.
    match *kind {
        FK::Plane { plane } => project.planes.iter().find(|p| p.id == plane).map(|p| crate::i18n::name(&p.name)).unwrap_or_default(),
        FK::DatumPoint { point } => project.datum_points.iter().find(|p| p.id == point).map(|p| crate::i18n::name(&p.name)).unwrap_or_default(),
        FK::DatumAxis { axis } => project.datum_axes.iter().find(|a| a.id == axis).map(|a| crate::i18n::name(&a.name)).unwrap_or_default(),
        _ => String::new(),
    }
}

/// WHETHER A TIMELINE ROW MATCHES THE SEARCH. An empty query matches everything.
///
/// Both THE LABEL AND THE NODE'S NAME are compared: a person searches for "extrude" as readily as for "lid" -
/// the first is the automatic label from the feature's kind, the second is the name they gave it themselves.
/// Case does not matter: nobody types a capital letter for the sake of a search.
pub(crate) fn tree_row_matches(project: &Project, tree: &TreeUi, ti: usize) -> bool {
    tree_text_matches(tree, &feature_row_label(project, ti)) || project.timeline.get(ti).is_some_and(|n| tree_text_matches(tree, &crate::i18n::name(&n.name)))
}

pub(crate) fn tree_select_component(project: &Project, sel: &mut Sel, tree_sel: &mut TreeSelection, ci: usize, cid: Id, click: qymcad_ui_state::TreeClick) {
    let par = project.components.get(ci).and_then(|c| c.parent);
    let sibs: Vec<Id> = project.components.iter().filter(|c| c.parent == par).map(|c| c.id).collect();
    qymcad_ui_state::tree_pick(tree_sel, &sibs, cid, click);
    *sel = Sel::Component(ci);
}

/// PICK THE ROW OF TIMELINE NODE `id` as the click says, among the rows of the timeline, and choose `sel` - several
/// sketches, features and datums are picked with Shift or Ctrl and deleted together.
pub(crate) fn tree_pick_node(tc: &mut qymcad_ui_state::TreeCtx, ui: &egui::Ui, id: Id, sel: Sel) {
    let order: Vec<Id> = tc.project.timeline.iter().map(|n| n.id).collect();
    qymcad_ui_state::tree_pick(&mut *tc.tree_sel, &order, id, qymcad_ui_state::TreeClick::of(&ui.input(|i| i.modifiers)));
    *tc.sel = sel;
}

pub(crate) fn tree_datum_row(tc: &mut qymcad_ui_state::TreeCtx, ui: &mut egui::Ui, id: Id, kind: &qymcad_core::feature::FeatureKind) {
    use qymcad_core::feature::FeatureKind as FK;
    match *kind {
        FK::Plane { plane } => {
            if let Some(pi) = tc.project.planes.iter().position(|p| p.id == plane) {
                let nm = crate::i18n::name(&tc.project.planes[pi].name);
                ui.horizontal(|ui| {
                    crate::gui::datum_vis_checkbox(&mut *tc.datum, ui, plane);
                    if rename_node_active(tc, ui, RenameNode::Plane(plane)) {
                        return; // inline renaming
                    }
                    let r = ui
                        .selectable_label(*tc.sel == Sel::Plane(pi) || qymcad_ui_state::in_tree_group(&*tc.tree_sel, id), format!("{} {nm}", ph::SELECTION_ALL))
                        .on_hover_text(crate::i18n::tr("tree-datum-hint"));
                    if r.clicked() {
                        tree_pick_node(tc, ui, id, Sel::Plane(pi));
                    }
                    if r.double_clicked() {
                        tc.ask.push(qymcad_ui_state::TreeAsk::EditFeature(plane));
                        // reopen the plane command
                    }
                    let mut ren = false;
                    r.context_menu(|ui| {
                        if ui.button(format!("{} {}", ph::TEXT_T, crate::i18n::tr("act-rename"))).clicked() {
                            ren = true;
                            ui.close();
                        }
                    });
                    if ren {
                        qymcad_ui_state::start_rename_node(&mut *tc.rename, RenameNode::Plane(plane), nm.clone());
                    }
                });
            }
        }
        FK::DatumPoint { point } => {
            if let Some(pi) = tc.project.datum_points.iter().position(|d| d.id == point) {
                let nm = crate::i18n::name(&tc.project.datum_points[pi].name);
                ui.horizontal(|ui| {
                    crate::gui::datum_vis_checkbox(&mut *tc.datum, ui, point);
                    if rename_node_active(tc, ui, RenameNode::DatumPoint(point)) {
                        return; // inline renaming
                    }
                    let r = ui
                        .selectable_label(*tc.sel == Sel::DatumPoint(pi) || qymcad_ui_state::in_tree_group(&*tc.tree_sel, id), format!("{} {nm}", ph::DOT))
                        .on_hover_text(crate::i18n::tr("tree-datum-hint"));
                    if r.clicked() {
                        tree_pick_node(tc, ui, id, Sel::DatumPoint(pi)); // a datum point is selectable (in sync with 3D)
                    }
                    if r.double_clicked() {
                        tc.ask.push(qymcad_ui_state::TreeAsk::EditFeature(point));
                        // reopen the point command
                    }
                    let mut ren = false;
                    r.context_menu(|ui| {
                        if ui.button(format!("{} {}", ph::TEXT_T, crate::i18n::tr("act-rename"))).clicked() {
                            ren = true;
                            ui.close();
                        }
                    });
                    if ren {
                        qymcad_ui_state::start_rename_node(&mut *tc.rename, RenameNode::DatumPoint(point), nm.clone());
                    }
                });
            }
        }
        FK::DatumAxis { axis } => {
            if let Some(ai) = tc.project.datum_axes.iter().position(|d| d.id == axis) {
                let nm = crate::i18n::name(&tc.project.datum_axes[ai].name);
                ui.horizontal(|ui| {
                    crate::gui::datum_vis_checkbox(&mut *tc.datum, ui, axis);
                    if rename_node_active(tc, ui, RenameNode::DatumAxis(axis)) {
                        return; // inline renaming
                    }
                    let r = ui
                        .selectable_label(*tc.sel == Sel::DatumAxis(ai) || qymcad_ui_state::in_tree_group(&*tc.tree_sel, id), format!("{} {nm}", ph::LINE_SEGMENT))
                        .on_hover_text(crate::i18n::tr("tree-datum-hint"));
                    if r.clicked() {
                        tree_pick_node(tc, ui, id, Sel::DatumAxis(ai)); // a datum axis is selectable
                    }
                    if r.double_clicked() {
                        tc.ask.push(qymcad_ui_state::TreeAsk::EditFeature(axis));
                        // reopen the axis command
                    }
                    let mut ren = false;
                    r.context_menu(|ui| {
                        if ui.button(format!("{} {}", ph::TEXT_T, crate::i18n::tr("act-rename"))).clicked() {
                            ren = true;
                            ui.close();
                        }
                    });
                    if ren {
                        qymcad_ui_state::start_rename_node(&mut *tc.rename, RenameNode::DatumAxis(axis), nm.clone());
                    }
                });
            }
        }
        _ => {}
    }
}

/// A sketch row: the visibility checkbox + expansion into contours; editing and finishing; a click selects.
pub(crate) fn tree_sketch_row(tc: &mut qymcad_ui_state::TreeCtx, ui: &mut egui::Ui, sid: Id) {
    let Some(si) = tc.project.sketch_index(sid) else { return };
    let editing = tc.sketch_ses.editing == Some(sid);
    let name = crate::i18n::name(&tc.project.sketches[si].name);
    let icon = if editing { ph::PENCIL_SIMPLE } else { ph::POLYGON };
    let raw = format!("{icon} {name}"); // the icon (a pencil while editing) already tells the mode apart
                                        // A PLAIN selectable row (with no expansion into contours). Editing starts on a double click; the right
                                        // button offers rename (inline) and delete. Only the visibility checkbox and the name.
    let mut act: Option<u8> = None; // 1 rename-start, 2 delete
    ui.horizontal(|ui| {
        let mut vis = !tc.sketch_hidden.contains(&sid);
        if ui.add(egui::Checkbox::without_text(&mut vis)).on_hover_text(crate::i18n::tr("tree-sketch-visible-hint")).changed() {
            if vis {
                tc.sketch_hidden.remove(&sid);
            } else {
                tc.sketch_hidden.insert(sid);
            }
        }
        // INLINE renaming of a sketch (as for features): a field instead of the label
        if tc.rename.sketch == Some(sid) {
            let r = qymcad_ui_state::rename_field(ui, &mut tc.rename.buf, 160.0, std::mem::take(&mut tc.rename.focus));
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                tc.rename.sketch = None; // cancelled
            } else if r.lost_focus() {
                let nm = tc.rename.buf.trim().to_string(); // Enter or a click elsewhere commits
                if !nm.is_empty() {
                    qymcad_ui_state::begin_edit(&mut *tc.edits, &*tc.project, crate::i18n::tr("status-rename")); // THE EDIT BOUNDARY, as for every other row
                    tc.project.sketches[si].name = nm;
                    qymcad_ui_state::commit_edit(&mut tc.rebuild());
                }
                tc.rename.sketch = None;
            }
            return;
        }
        let nid = qymcad_ui_state::node_of_sel(&*tc.project, Sel::Sketch(si)).unwrap_or(sid);
        let lit = *tc.sel == Sel::Sketch(si) || qymcad_ui_state::in_tree_group(&*tc.tree_sel, nid);
        let resp = ui.selectable_label(lit, crate::gui::sel_title(&*tc.scheme, raw, lit)).on_hover_text(crate::i18n::tr("tree-sketch-hint"));
        if *tc.sel == Sel::Sketch(si) {
            show_in_view(&mut *tc.tree, ui, &resp, nid); // a sketch selected from elsewhere is shown in the tree
        }
        if resp.double_clicked() {
            tc.ask.push(qymcad_ui_state::TreeAsk::EnterSketch(si));
        } else if resp.clicked() {
            let alone = qymcad_ui_state::TreeClick::of(&ui.input(|i| i.modifiers)) == qymcad_ui_state::TreeClick::Alone;
            if alone && *tc.sel == Sel::Sketch(si) {
                tc.ask.push(qymcad_ui_state::TreeAsk::SketchAgain(si));
            }
            tree_pick_node(tc, ui, nid, Sel::Sketch(si));
        }
        resp.context_menu(|ui| {
            if ui.button(format!("{} {}", ph::TEXT_T, crate::i18n::tr("act-rename"))).clicked() {
                act = Some(1);
                ui.close();
            }
            if ui.button(format!("{} {}", ph::ARROWS_OUT_CARDINAL, crate::i18n::tr("act-move-sketch"))).on_hover_text(crate::i18n::tr("tree-sketch-move-hint")).clicked() {
                act = Some(7);
                ui.close();
            }
            if ui.button(format!("{} {}", ph::COPY, crate::i18n::tr("act-copy-ctrl-c"))).clicked() {
                act = Some(3);
                ui.close();
            }
            if ui.button(format!("{} {}", ph::SCISSORS, crate::i18n::tr("act-cut-ctrl-x"))).clicked() {
                act = Some(4);
                ui.close();
            }
            if ui.button(format!("{} {}", ph::TRASH, crate::i18n::tr("act-delete-sketch"))).clicked() {
                act = Some(2);
                ui.close();
            }
            ui.separator();
            if ui.button(format!("{} {}", ph::EXPORT, crate::i18n::tr("act-export-svg"))).on_hover_text(crate::i18n::tr("tree-export-svg-hint")).clicked() {
                act = Some(5);
                ui.close();
            }
            if ui.button(format!("{} {}", ph::EXPORT, crate::i18n::tr("act-export-dxf"))).on_hover_text(crate::i18n::tr("tree-export-dxf-hint")).clicked() {
                act = Some(6);
                ui.close();
            }
        });
    });
    match act {
        Some(1) => qymcad_ui_state::start_rename_sketch(&mut *tc.rename, sid, name),
        Some(2) => qymcad_ui_state::ask_delete(&mut *tc.deferred, Sel::Sketch(si)),
        Some(3) => {
            *tc.sel = Sel::Sketch(si);
            tc.ask.push(qymcad_ui_state::TreeAsk::Clipboard { cut: false });
        }
        Some(4) => {
            *tc.sel = Sel::Sketch(si);
            tc.ask.push(qymcad_ui_state::TreeAsk::Clipboard { cut: true });
        }
        Some(5) => tc.ask.push(qymcad_ui_state::TreeAsk::ExportSketch { si, flat: false }),
        Some(6) => tc.ask.push(qymcad_ui_state::TreeAsk::ExportSketch { si, flat: true }),
        Some(7) => tc.ask.push(qymcad_ui_state::TreeAsk::ReplaceSketchPlane(si)),
        _ => {}
    }
}

/// THE ROW OF THE SELECTION SHOWS ITSELF IN THE TREE: selected from elsewhere - the viewport, the window of faults, a
/// path in the window of parameters - it is scrolled into view: selected out of sight is not shown. It asks
/// until it stands in the visible part of the tree, then stays where the person scrolls it: in the frame a section of
/// the tree opens, its rows are still folded under the header, and one request scrolled 69 points of the 1200 needed.
fn show_in_view(tree: &mut qymcad_ui_state::TreeUi, ui: &egui::Ui, row: &egui::Response, nid: Id) {
    if tree.shown == Some(nid) {
        return;
    }
    if ui.clip_rect().contains_rect(row.rect) {
        tree.shown = Some(nid);
    } else {
        row.scroll_to_me(None);
    }
}

/// A feature row: it expands into the resulting body; a click selects the feature (for editing or deleting).
pub(crate) fn tree_feature_row(tc: &mut qymcad_ui_state::TreeCtx, ui: &mut egui::Ui, ti: usize) {
    // a guard against a STALE index: deleting a feature within this same pass of the loop (right button ->
    // Delete) shortens the timeline, so the following rows get an index past the end (immediate mode). On the
    // next frame feat_tis is recomputed; until then, simply skip.
    if ti >= tc.project.timeline.len() {
        return;
    }
    let lbl = feature_row_label(&*tc.project, ti);
    if lbl.is_empty() {
        return; // this kind of node has no row of its own in the body list
    }
    let nid = tc.project.timeline[ti].id;
    let n = tc.project.timeline.len();
    // suppressed either by the rollback bar OR individually - shown grey and italic
    let suppressed = tc.project.rollback.is_some_and(|rb| ti >= rb) || tc.project.timeline[ti].suppressed;
    let node_suppressed = tc.project.timeline[ti].suppressed;
    let mut act: Option<u8> = None; // 1 edit,2 rollback,3 clear-rb,4 up,5 down,6 delete,7 suppress-toggle,8 rename
                                    // the neighbouring FEATURES of the same parent: Up/Down reorder among FEATURES (not among sketches or
                                    // datums), and go grey when the move would break the dependencies (a linear fillet-over-extrude chain).
    let parent = tc.project.timeline[ti].parent;
    let prev_feat = (0..ti).rev().find(|&j| tc.project.timeline[j].parent == parent && tc.project.timeline[j].kind.body().is_some());
    let next_feat = (ti + 1..n).find(|&j| tc.project.timeline[j].parent == parent && tc.project.timeline[j].kind.body().is_some());
    let can_up = prev_feat.is_some_and(|pf| tc.project.can_reorder_feature(ti, pf));
    let can_down = next_feat.is_some_and(|nf| tc.project.can_reorder_feature(nf, ti));
    // the feature did NOT build (it fell back to a pass-through, a copy of the source), so it is marked in
    // the tree. There is now ONE row per operation span, so an error on ANY node of the span (a hidden one
    // included) must turn the row red - otherwise the feature silently fails to apply with no marker at all.
    // A KERNEL ERROR IS A CODE; the text in the reader's language is assembled here (see i18n::error_text)
    let node_err = tc.project.feature_op_span(nid).iter().find_map(|id| tc.project.regen_errors.get(id)).map(crate::gui::error_words::error_text);
    // BUILT, BUT NOT ALL OF IT: a rounding that could not take some edges did the rest - yellow, with the words
    let node_warn = if node_err.is_none() { tc.project.regen_warnings.get(&nid).map(crate::gui::error_words::error_text) } else { None };
    ui.horizontal(|ui| {
        // A feature is suppressed either by THE ROLLBACK LINE (the tail of the list) or individually from the
        // right-button menu. The suppress checkbox was removed: it confused, because in a linear chain it hid
        // everything from the current one downwards. An individually suppressed feature carries a mark.
        // inline renaming: a field instead of the label
        if rename_row_active(tc, ui, nid) {
            return;
        }
        let base = if node_suppressed { format!("{lbl}  {}", ph::PROHIBIT) } else { lbl };
        // the tone of the node, said with the icon and the colour of its tone as every other place says it
        let tone = if node_err.is_some() { Some(qymcad_ui_state::Tone::Error) } else { node_warn.as_ref().map(|_| qymcad_ui_state::Tone::Warning) };
        let icon = tone.and_then(qymcad_ui_state::tone_icon);
        let base = if let Some(icon) = icon { format!("{icon} {base}") } else { base };
        // the node's reference was REBOUND by its fingerprint - that is visible rather than silent.
        let rebound = tc.regen.rebinds.iter().find(|r| r.node == nid).map(|r| r.what.clone());
        let base = if rebound.is_some() { format!("{} {base}", ph::LINK_BREAK) } else { base };
        let mut txt = egui::RichText::new(base);
        if suppressed {
            txt = txt.weak().italics(); // rolled back or suppressed - it is visible that it does not build
        } else if let Some(t) = tone {
            txt = txt.color(qymcad_ui_state::tone_colour(&tc.scheme.pal, t)); // red: it did not apply; yellow: not all of it
        }
        let hover = match (&node_err, &rebound) {
            (Some(e), _) => format!("{} {}", icon.unwrap_or_default(), crate::i18n::tr1("tree-feature-failed", "error", e)),
            (None, _) if node_warn.is_some() => format!("{} {}", icon.unwrap_or_default(), node_warn.clone().unwrap_or_default()),
            (None, Some(w)) => format!("{} {}", ph::LINK_BREAK, crate::i18n::tr1("tree-feature-rebound", "what", w)),
            _ => crate::i18n::tr("tree-feature-hint"),
        };
        let nid = tc.project.timeline.get(ti).map(|n| n.id).unwrap_or_default();
        let lit = *tc.sel == Sel::Feature(ti) || qymcad_ui_state::in_tree_group(&*tc.tree_sel, nid);
        let resp = ui.selectable_label(lit, txt).on_hover_text(hover);
        if *tc.sel == Sel::Feature(ti) {
            show_in_view(&mut *tc.tree, ui, &resp, nid); // a node selected from elsewhere - a fault led to, a path followed
        }
        if resp.clicked() {
            tree_pick_node(tc, ui, nid, Sel::Feature(ti));
        }
        if resp.double_clicked() {
            act = Some(1);
        }
        resp.context_menu(|ui| {
            if ui.button(format!("{} {}", ph::PENCIL_SIMPLE, crate::i18n::tr("act-edit"))).clicked() {
                act = Some(1);
                ui.close();
            }
            if ui.button(format!("{} {}", ph::TEXT_T, crate::i18n::tr("act-rename"))).clicked() {
                act = Some(8);
                ui.close();
            }
            let supp_label = if node_suppressed { format!("{} {}", ph::CHECK, crate::i18n::tr("act-unsuppress")) } else { format!("{} {}", ph::PROHIBIT, crate::i18n::tr("act-suppress")) };
            if ui.button(supp_label).on_hover_text(crate::i18n::tr("tree-suppress-hint")).clicked() {
                act = Some(7);
                ui.close();
            }
            ui.separator();
            if ui.button(format!("{} {}", ph::ARROW_LINE_UP, crate::i18n::tr("act-rollback-here"))).on_hover_text(crate::i18n::tr("tree-rollback-hint")).clicked() {
                act = Some(2);
                ui.close();
            }
            if tc.project.rollback.is_some() && ui.button(format!("{} {}", ph::ARROW_LINE_DOWN, crate::i18n::tr("act-clear-rollback"))).clicked() {
                act = Some(3);
                ui.close();
            }
            ui.separator();
            if ui.add_enabled(can_up, egui::Button::new(format!("{} {}", ph::ARROW_UP, crate::i18n::tr("act-move-up")))).on_hover_text(crate::i18n::tr("tree-move-up-hint")).clicked() {
                act = Some(4);
                ui.close();
            }
            if ui.add_enabled(can_down, egui::Button::new(format!("{} {}", ph::ARROW_DOWN, crate::i18n::tr("act-move-down")))).on_hover_text(crate::i18n::tr("tree-move-down-hint")).clicked() {
                act = Some(5);
                ui.close();
            }
            ui.separator();
            if ui.button(format!("{} {}", ph::TRASH, crate::i18n::tr("act-delete-feature"))).clicked() {
                act = Some(6);
                ui.close();
            }
        });
    });
    if let Some(a) = act {
        // an import has no command of its own: editing it is asking again about its units and scale; a pattern of
        // components reopens the pattern's own command
        let pattern = matches!(tc.project.timeline[ti].kind, qymcad_core::feature::FeatureKind::ComponentPattern { .. });
        tc.ask.push(if a == 1 && tc.project.timeline[ti].kind.is_import() {
            qymcad_ui_state::TreeAsk::RescaleImport(nid)
        } else if a == 1 && pattern {
            qymcad_ui_state::TreeAsk::EditCompArray(nid)
        } else {
            qymcad_ui_state::TreeAsk::Action { act: a, ti, nid, prev_feat, next_feat }
        });
    }
}

/// A body row - one of the bodies a part shows, or an imported one with no source feature: visibility, selection,
/// renaming, and making it a part of its own when its part shows another. WITHOUT nesting the faces - faces are
/// picked by a click in 3D rather than in the tree (the tree is a plain list).
pub(crate) fn tree_body_row(tc: &mut qymcad_ui_state::TreeCtx, ui: &mut egui::Ui, mi: usize) {
    let name = crate::i18n::name(&tc.project.mesh_name(mi));
    ui.horizontal(|ui| {
        let mut vis = tc.project.bodies.get(mi).is_none_or(|b| b.visible);
        if ui.add(egui::Checkbox::without_text(&mut vis)).changed() {
            tc.project.bodies[mi].visible = vis;
            crate::gui::visibility_changed(&mut *tc.regen); // otherwise the "what is shown" cache gives yesterday's answer
        }
        if rename_node_active(tc, ui, RenameNode::Body(mi)) {
            return; // inline renaming of the body
        }
        let r = ui.selectable_label(*tc.sel == Sel::Mesh(mi), format!("{} {}", ph::CUBE, name)).on_hover_text(crate::i18n::tr("tree-body-hint"));
        if r.clicked() {
            *tc.sel = Sel::Mesh(mi);
        }
        let mut ren = false;
        let body = tc.project.mesh_id(mi);
        let own_part = body.is_some_and(|b| tc.project.may_be_made_a_part(b));
        r.context_menu(|ui| {
            if ui.button(format!("{} {}", ph::TEXT_T, crate::i18n::tr("act-rename"))).clicked() {
                ren = true;
                ui.close();
            }
            // one of the several bodies a part shows is made a part of its own from its row as from the canvas
            if let Some(b) = body.filter(|_| own_part) {
                if ui.button(format!("{} {}", ph::CUBE, crate::i18n::tr("act-piece-to-part"))).clicked() {
                    let at = ui.ctx().pointer_latest_pos().unwrap_or(r.rect.right_top());
                    crate::gui::piece_part::ask(ui.ctx(), &*tc.project, b, at);
                    ui.close();
                }
            }
        });
        if ren {
            qymcad_ui_state::start_rename_node(&mut *tc.rename, RenameNode::Body(mi), name.clone());
        }
    });
}
