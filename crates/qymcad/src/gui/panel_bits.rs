//! PIECES OF THE PANELS: the properties of a point, a face, a mesh, an axis; the names of features; the tree of library parts.

use qymcad_ui_state::Notice;
use super::*;

/// Toggle the parts library window; on the first opening it builds the catalogue tree.
pub(crate) fn toggle_parts_library(parts: &mut PartsLibrary, win: &mut Windows) {
    win.toggle(WinKind::PartsLibrary);
    if win.is(WinKind::PartsLibrary) && parts.tree.is_none() {
        parts.tree = Some(crate::parts_library::load_library_tree());
    }
}

pub(crate) fn tree_drop_intent(rect: egui::Rect, pointer_y: f32) -> TreeDrop {
    let h = rect.height().max(1.0);
    let t = ((pointer_y - rect.top()) / h).clamp(0.0, 1.0);
    if t < 0.25 {
        TreeDrop::Before
    } else if t > 0.75 {
        TreeDrop::After
    } else {
        TreeDrop::Onto
    }
}

/// The height limit of the parameter list: beyond it the list SCROLLS rather than growing the window off
/// the edge of the screen.
pub(crate) const PARAM_ROWS_MAX_H: f32 = 420.0;

/// What drawing the parameter rows returned. The width and the height are REAL, taken from the frame: a
/// test uses them to check that the fields stretch and that a long list scrolls rather than growing without
/// end.
#[derive(Default)]
pub(crate) struct ParamRowsOut {
    pub dirty: bool,
    pub remove: Option<usize>,
    pub name_w: f32,
    /// The height of the scrolling VIEWPORT — this is what the limit caps.
    pub height: f32,
    /// The height of the CONTENT. It grows with the number of rows, and shows that the list really is long
    /// and that the viewport scrolls it rather than showing it whole.
    pub content_h: f32,
    /// The expressions that did not evaluate, as (the parameter's name, the reason in words). They are shown
    /// BELOW the table, across its whole width: the value column is too narrow for a sentence, and a reason
    /// that has to be hunted for by hovering is a reason nobody reads.
    pub errors: Vec<(String, String)>,
}

/// THE FIELD WIDTHS IN THE PARAMETERS WINDOW ARE ELASTIC, NOT FIXED.
///
/// They used to be 90 points for the name and 120 for the formula, and the window could be stretched as far
/// as one liked while the fields stayed as they were. A long variable name was IMPOSSIBLE to type: the text
/// crept past the edge and one typed blind. Now both fields share the window's width, and the former
/// numbers became the lower bound — a narrow window does not collapse them to nothing.
///
/// The proportions are chosen so that the formula is wider than the name (expressions like `w*2+3` live in
/// it), and so that the third column with the value and the button still has room.
pub(crate) fn param_field_widths(avail: f32) -> (f32, f32) {
    const NAME_MIN: f32 = 90.0;
    const EXPR_MIN: f32 = 120.0;
    const NAME_SHARE: f32 = 0.30;
    const EXPR_SHARE: f32 = 0.38;
    ((avail * NAME_SHARE).max(NAME_MIN), (avail * EXPR_SHARE).max(EXPR_MIN))
}

pub(crate) fn normal_label(n: [f64; 3]) -> &'static str {
    if n[2] > 0.85 {
        "normal-top"
    } else if n[2] < -0.85 {
        "normal-bottom"
    } else if n[2].abs() < 0.25 {
        "normal-side"
    } else {
        "normal-tilted"
    }
}

/// Draw a category node of the tree, recursively. `path` holds the indices from the root of the level
/// (mutated by push and pop).
pub(crate) fn parts_tree_node(ui: &mut egui::Ui, node: &crate::parts_library::CatNode, tier: bool, path: &mut Vec<usize>, sel: &mut Option<(bool, Vec<usize>)>) {
    let is_sel = matches!(sel, Some((t, p)) if *t == tier && p == path);
    let title = format!("{}  {} ({})", ph::FOLDER_OPEN, node.title, node.total_parts());
    if node.subcats.is_empty() {
        if ui.selectable_label(is_sel, title).clicked() {
            *sel = Some((tier, path.clone()));
        }
    } else {
        let id = ui.make_persistent_id(("parts_cat", tier, &*path));
        egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, path.is_empty())
            .show_header(ui, |ui| {
                if ui.selectable_label(is_sel, title).clicked() {
                    *sel = Some((tier, path.clone()));
                }
            })
            .body(|ui| {
                for (i, c) in node.subcats.iter().enumerate() {
                    path.push(i);
                    parts_tree_node(ui, c, tier, path, sel);
                    path.pop();
                }
            });
    }
}

/// The category node at a path of indices from the root of the level.
pub(crate) fn cat_at<'a>(root: &'a crate::parts_library::CatNode, path: &[usize]) -> Option<&'a crate::parts_library::CatNode> {
    let mut n = root;
    for &i in path {
        n = n.subcats.get(i)?;
    }
    Some(n)
}

/// Gather every part of the subtree whose name or tags contain `query` (in lower case).
pub(crate) fn collect_matching<'a>(node: &'a crate::parts_library::CatNode, query: &str, out: &mut Vec<&'a crate::parts_library::PartEntry>) {
    for p in &node.parts {
        let name_hit = p.name.to_lowercase().contains(query);
        let tag_hit = p.manifest.as_ref().map(|m| m.tags.iter().any(|t| t.to_lowercase().contains(query))).unwrap_or(false);
        if name_hit || tag_hit {
            out.push(p);
        }
    }
    for c in &node.subcats {
        collect_matching(c, query, out);
    }
}

/// The icon of a feature in the tree (by its kind) — shared with the labels of `tree_feature_row` (which shows a custom name).
pub(crate) fn feat_icon(kind: &qymcad_core::feature::FeatureKind) -> &'static str {
    use qymcad_core::feature::FeatureKind as FK;
    match kind {
        FK::Cylinder { .. } => ph::CYLINDER,
        FK::Sphere { .. } | FK::Torus { .. } | FK::Fillet { .. } | FK::Hole { .. } => ph::CIRCLE,
        FK::Combine { op, .. } => [ph::SCISSORS, ph::CUBE, ph::INTERSECT][(*op as usize).min(2)],
        FK::BodyBoolean { .. } => ph::INTERSECT,
        FK::PushFace { .. } => ph::ARROWS_OUT_LINE_VERTICAL,
        FK::RemoveFace { .. } => ph::ERASER,
        FK::SplitBody { .. } => ph::SQUARE_SPLIT_HORIZONTAL,
        FK::PartInstance { .. } => ph::CUBE_TRANSPARENT, // NOT the assembly icon: there is nothing to enter here (see panels.rs)
        FK::Thicken { .. } => ph::STACK_SIMPLE,
        FK::SplitFace { .. } => ph::GRID_FOUR,
        FK::Chamfer { .. } => ph::TRIANGLE,
        FK::Prism { .. } => ph::HEXAGON,
        FK::Shell { .. } => ph::BOUNDING_BOX,
        FK::LinearArray { .. } | FK::ComponentPattern { kind: qymcad_core::model::CompPatternKind::Linear { .. }, .. } => ph::DOTS_THREE_OUTLINE,
        FK::CircularArray { .. } | FK::ComponentPattern { kind: qymcad_core::model::CompPatternKind::Circular { .. }, .. } => ph::ARROWS_CLOCKWISE,
        FK::Mirror { .. } => ph::FLIP_HORIZONTAL,
        FK::Move { .. } => ph::ARROWS_OUT_CARDINAL,
        FK::Thread { .. } => ph::SPIRAL,
        FK::Import { .. } | FK::MeshPiece { .. } => ph::FILE_ARROW_DOWN,
        _ => ph::CUBE,
    }
}

/// A feature's default name (as the `add_*` methods set it) — used to tell a RENAMED feature from a
/// default one, so that the tree shows the name given by hand.
/// A FEATURE'S DEFAULT NAME IS A CATALOGUE KEY, the same one the core puts in through `add_*`.
///
/// The core writes a KEY into the node rather than a word: the default name is read by a person, and
/// the core knows no languages. The same key is needed here to compare "has the feature been
/// renamed" — what gets compared is KEYS, so a change of language does not affect that decision.
pub(crate) fn feat_default_name(kind: &qymcad_core::feature::FeatureKind) -> String {
    use qymcad_core::feature::FeatureKind as FK;
    match kind {
        // THE SURFACE FEATURES. Without these lines a node counted as RENAMED (its name did not match
        // the "default"), and the tree showed the raw catalogue key instead of a word. Caught by a scenario.
        FK::FaceCopy { .. } => "feat-name-face-copy".into(),
        FK::OffsetSurface { .. } => "feat-name-offset-surface".into(),
        FK::SurfaceReplace { .. } => "feat-name-surface-replace".into(),
        FK::Patch { .. } => "feat-name-patch".into(),
        FK::Stitch { .. } => "feat-name-stitch".into(),
        FK::MeshSolid { .. } => "feat-name-mesh-solid".into(),
        FK::MeshRecognised { .. } => "feat-name-mesh-recognised".into(),
        FK::Trim { .. } => "feat-name-trim".into(),
        FK::Extrude { .. } => "feat-name-extrude".into(),
        FK::Revolve { .. } => "feat-name-revolve".into(),
        FK::Sweep { .. } => "feat-name-sweep".into(),
        FK::PushFace { .. } => "feat-name-push-face".into(),
        FK::RemoveFace { .. } => "feat-name-remove-face".into(),
        FK::SplitBody { .. } => "feat-name-split-body".into(),
        FK::PartInstance { .. } => "feat-name-instance".into(),
        // A MIRRORED PART WITH NO NAME. It had a row in the tree but no default name — so the node
        // looked nameless everywhere the name is taken from the timeline (properties, search, reports).
        FK::MirrorPart { .. } => "feat-mirror-part".into(),
        FK::Thicken { .. } => "feat-name-thicken".into(),
        FK::SplitFace { .. } => "feat-name-split-face".into(),
        FK::Loft { .. } => "feat-name-loft".into(),
        FK::Draft { .. } => "feat-name-draft".into(),
        FK::Box3 { .. } => "feat-name-box".into(),
        FK::Cylinder { .. } => "feat-name-cylinder".into(),
        FK::Sphere { .. } => "feat-name-sphere".into(),
        FK::Cone { .. } => "feat-name-cone".into(),
        FK::Torus { .. } => "feat-name-torus".into(),
        FK::Prism { .. } => "feat-name-prism".into(),
        FK::Combine { op, .. } => ["feat-name-combine-cut", "feat-name-combine-boss", "feat-name-combine-intersect"][(*op as usize).min(2)].into(),
        FK::Fillet { .. } => "feat-name-fillet".into(),
        FK::Chamfer { .. } => "feat-name-chamfer".into(),
        FK::Shell { .. } => "feat-name-shell".into(),
        FK::LinearArray { .. } => "feat-name-linear-array".into(),
        FK::CircularArray { .. } => "feat-name-circular-array".into(),
        FK::ComponentPattern { kind: qymcad_core::model::CompPatternKind::Linear { .. }, .. } => "feat-name-comp-linear-array".into(),
        FK::ComponentPattern { kind: qymcad_core::model::CompPatternKind::Circular { .. }, .. } => "feat-name-comp-circular-array".into(),
        FK::Mirror { .. } => "feat-name-mirror".into(),
        FK::Hole { sketch, .. } => {
            if *sketch != 0 {
                "feat-name-holes-sketch".into()
            } else {
                "feat-name-hole".into()
            }
        }
        FK::Thread { .. } => "feat-name-thread".into(),
        FK::Auger { .. } => "feat-name-auger".into(),
        FK::Move { .. } => "feat-name-move".into(),
        FK::BodyBoolean { op, .. } => ["feat-name-body-cut", "feat-name-body-union", "feat-name-body-intersect"][(*op as usize).min(2)].into(),
        FK::Import { .. } => "name-import".into(),
        FK::MeshPiece { .. } => "name-mesh-piece".into(),
        _ => String::new(),
    }
}

/// The target assembly for an insertion: the active assembly; if a Part is active, its parent; otherwise the root.
pub(crate) fn parts_insert_target(project: &mut qymcad_core::model::Project) -> qymcad_core::model::Id {
    let root = project.ensure_root();
    let ctx = project.current_ctx();
    if ctx != 0 && !project.component_is_part(ctx) {
        return ctx;
    }
    project.components.iter().find(|c| c.id == ctx).and_then(|c| c.parent).unwrap_or(root)
}

/// WHAT WILL GO WITH IT — by name, for the question asked before deleting.
///
/// Deletion in the timeline cascades: with a fillet goes everything built on it. This used to be
/// reported by one general line saying "together with its dependants" — true, but no answer to the
/// question "what am I about to lose". The list comes from the same `Project::dependents_of` as the
/// lineage in the properties card: two places must not answer one question differently.
pub(crate) fn delete_cascade_names(project: &qymcad_core::model::Project, sel: Sel) -> Vec<String> {
    let subject = match sel {
        Sel::Feature(ti) => project.timeline.get(ti).map(|n| n.id),
        Sel::Mesh(mi) => project.mesh_id(mi),
        Sel::Sketch(si) => project.sketches.get(si).map(|s| s.id),
        Sel::Plane(i) => project.planes.get(i).map(|p| p.id),
        Sel::DatumPoint(i) => project.datum_points.get(i).map(|d| d.id),
        Sel::DatumAxis(i) => project.datum_axes.get(i).map(|d| d.id),
        _ => None,
    };
    let Some(id) = subject else { return Vec::new() };
    project.dependents_of(id).into_iter().filter_map(|nid| project.timeline.iter().find(|n| n.id == nid)).map(|n| crate::i18n::name(&n.name)).collect()
}

pub(crate) fn sel_delete_label(project: &qymcad_core::model::Project, sel: Sel) -> String {
    match sel {
        Sel::Feature(_) => crate::i18n::tr("del-feature"),
        Sel::Mesh(mi) => crate::i18n::tr1("del-body-named", "name", &crate::i18n::name(&project.mesh_name(mi))),
        Sel::Sketch(si) => project.sketches.get(si).map(|s| crate::i18n::tr1("del-sketch-named", "name", &crate::i18n::name(&s.name))).unwrap_or_else(|| crate::i18n::tr("del-sketch")),
        Sel::Plane(i) => project.planes.get(i).map(|p| crate::i18n::tr1("del-plane-named", "name", &crate::i18n::name(&p.name))).unwrap_or_else(|| crate::i18n::tr("del-plane")),
        Sel::DatumPoint(i) => project.datum_points.get(i).map(|d| crate::i18n::tr1("del-point-named", "name", &crate::i18n::name(&d.name))).unwrap_or_else(|| crate::i18n::tr("del-point")),
        Sel::DatumAxis(i) => project.datum_axes.get(i).map(|d| crate::i18n::tr1("del-axis-named", "name", &crate::i18n::name(&d.name))).unwrap_or_else(|| crate::i18n::tr("del-axis")),
        Sel::Joint(_) => crate::i18n::tr("del-joint"),
        Sel::Component(ci) => project.components.get(ci).map(|c| crate::i18n::tr1("del-part-named", "name", &crate::i18n::name(&c.name))).unwrap_or_else(|| crate::i18n::tr("del-part")),
        _ => crate::i18n::tr("del-item"),
    }
}

/// The thumbnail texture of a part for the library grid (`thumb.png` is loaded lazily from the
/// `.qpart`, decoded, turned into a GPU texture, and cached by source). None means there is no preview
/// (an older part with no thumbnail), and the grid shows a placeholder icon. The cache is cleared when
/// the catalogue is rebuilt or something is saved.
pub(crate) fn parts_thumb_texture(parts: &mut PartsLibrary, ctx: &egui::Context, src: &crate::parts_library::PartSource) -> Option<egui::TextureHandle> {
    use crate::parts_library::PartSource;
    let key = match src {
        PartSource::Embedded(s) => format!("e:{s}"),
        PartSource::User(p) => format!("u:{}", p.to_string_lossy()),
    };
    if let Some(cached) = parts.thumbs.get(&key) {
        return cached.clone();
    }
    let png = match src {
        PartSource::User(p) => qymcad_io::load_part_thumb(&p.to_string_lossy()),
        PartSource::Embedded(rel) => crate::parts_library::embedded_bytes(rel).and_then(qymcad_io::load_part_thumb_bytes),
    };
    let tex = png.and_then(|bytes| image::load_from_memory(&bytes).ok()).map(|img| {
        let rgba = img.to_rgba8();
        let (w, h) = rgba.dimensions();
        let color = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], rgba.as_raw());
        ctx.load_texture(format!("part_thumb_{key}"), color, egui::TextureOptions::LINEAR)
    });
    parts.thumbs.insert(key, tex.clone());
    tex
}

/// The datum's visibility tick in the tree (a stable Id against `datum_hidden`). Shared by planes, points and axes.
pub(crate) fn datum_vis_checkbox(datum: &mut DatumCommand, ui: &mut egui::Ui, id: Id) {
    let mut vis = !datum.hidden.contains(&id);
    if ui.add(egui::Checkbox::without_text(&mut vis)).on_hover_text(crate::i18n::tr("datum-vis-hint")).changed() {
        if vis {
            datum.hidden.remove(&id);
        } else {
            datum.hidden.insert(id);
        }
    }
}

/// The title of a tree node: the selected one is highlighted. A method rather than a free function:
/// the selection colour belongs to the scheme, and a function without `&self` cannot ask the scheme —
/// exactly the fault that kept the tool bar from being recoloured by the theme.
pub(crate) fn sel_title(scheme: &SchemeUi, text: String, selected: bool) -> egui::RichText {
    let rt = egui::RichText::new(text);
    if selected {
        rt.color(scheme.pal.tree_selected()).strong()
    } else {
        rt
    }
}

/// The properties of a datum POINT: its name, its coordinates and a delete button. Parametric
/// definitions live elsewhere.
pub(crate) fn datum_point_props(datum: &mut DatumCommand, deferred: &mut DeferredUi, project: &mut Project, ui: &mut egui::Ui, i: usize) {
    let lin = crate::gui::props_card::lineage_of(project, Some(project.datum_points[i].id));
    if let Some(n) = props_header(ui, ph::DOT, "datum-point-title", NameSlot::Editable(project.datum_points[i].name.clone()), &lin) {
        project.datum_points[i].name = n;
    }
    let mut changed = false;
    {
        let d = &mut project.datum_points[i];
        ui.label(crate::i18n::tr("datum-point-coords"));
        egui::Grid::new(("dpt", i)).num_columns(2).spacing([6.0, 3.0]).show(ui, |ui| {
            changed |= drag(ui, "X", &mut d.at[0], 1.0, -100000.0..=100000.0);
            changed |= drag(ui, "Y", &mut d.at[1], 1.0, -100000.0..=100000.0);
            changed |= drag(ui, "Z", &mut d.at[2], 1.0, -100000.0..=100000.0);
        });
    }
    if changed {
        datum.regen_pending = true; // the point moved, so an axis through two points and the bodies need rebuilding (debounced)
    }
    ui.separator();
    if ui.button(format!("{} {}", ph::TRASH, crate::i18n::tr("datum-point-delete"))).clicked() {
        ask_delete(deferred, Sel::DatumPoint(i));
    }
}

pub(crate) fn face_props(project: &mut Project, ui: &mut egui::Ui, mi: usize, fi: usize, for_cmd: bool) {
    // INFORMATION ABOUT THE FACE, and nothing else. A face is selected by a click with nothing in hand and under
    // a command that collects faces (`for_cmd`, said in the last line); the older buttons (sketch, plane,
    // contour, projection, Z0) are gone — a sketch on a face is made with the Sketch tool on the toolbar (a
    // click on the face), and a datum by the datum command.
    let Some(face) = project.bodies.get(mi).and_then(|b| b.faces.get(fi)) else { return };
    let n = face.normal;
    // THE LINEAGE IS TAKEN FROM THE BODY: a face has no Id of its own in the document, but "what made
    // the thing this face lies on" is exactly the question the face's properties are opened for.
    let lin = crate::gui::props_card::lineage_of(project, project.mesh_id(mi));
    props_header(ui, ph::SQUARE_HALF, "face-props-title", NameSlot::None, &lin);
    ui.label(crate::i18n::tr1("face-area", "v", &format!("{:.1}", face.area)));
    ui.label(crate::i18n::trn(
        "face-normal",
        &[("n", &format!("[{}, {}, {}]", crate::i18n::num(n[0], 2), crate::i18n::num(n[1], 2), crate::i18n::num(n[2], 2))), ("side", &crate::i18n::tr(normal_label(n)))],
    ));
    ui.label(crate::i18n::tr1("face-center", "v", &format!("{:.1}, {:.1}, {:.1}", face.centroid.x, face.centroid.y, face.centroid.z)));
    if for_cmd {
        ui.label(egui::RichText::new(crate::i18n::tr("face-picked-for-cmd")).weak().small());
    }
}

/// The world (origin, dir) of the axis object caught by a click pick (an edge, a cylindrical face or a
/// datum axis) — WITHOUT creating anything. One resolver for both the preview and the creation (of a
/// pattern and of a datum axis).
pub(crate) fn axis_ref_world(active_path: &[Id], edges: &EdgeCache, live: &LiveGeom, project: &Project, hit: AxisHit) -> Option<([f64; 3], [f64; 3])> {
    use qymcad_core::feature::{apply12, is_identity12};
    let ctx = current_ctx_id(active_path, project);
    match hit {
        AxisHit::Datum(id) => project.datum_axes.iter().find(|d| d.id == id).map(|d| (d.origin(), d.dir())),
        AxisHit::Edge(i) => {
            let (body, _id, poly) = edges.axes.get(i)?;
            if poly.len() < 2 {
                return None;
            }
            let wt = project.body_display_transform(*body, ctx);
            let w = |p: &[f32; 3]| -> [f64; 3] {
                let v = [p[0] as f64, p[1] as f64, p[2] as f64];
                if is_identity12(&wt) {
                    v
                } else {
                    apply12(&wt, v)
                }
            };
            let a = w(&poly[0]);
            let b = w(poly.last().unwrap());
            let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
            let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
            (l > 1e-9).then(|| (a, [d[0] / l, d[1] / l, d[2] / l]))
        }
        AxisHit::Face(body, fid) => {
            let (lo, ld) = live.shapes.get(&body)?.face_axis(fid)?;
            let wt = project.body_display_transform(body, ctx);
            let o = if is_identity12(&wt) { lo } else { apply12(&wt, lo) };
            let z = if is_identity12(&wt) { [0.0; 3] } else { apply12(&wt, [0.0, 0.0, 0.0]) };
            let d = if is_identity12(&wt) { ld } else { [apply12(&wt, ld)[0] - z[0], apply12(&wt, ld)[1] - z[1], apply12(&wt, ld)[2] - z[2]] };
            let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
            (l > 1e-9).then(|| (o, [d[0] / l, d[1] / l, d[2] / l]))
        }
    }
}

pub(crate) fn mesh_props(deferred: &mut DeferredUi, project: &mut Project, regen: &mut Rebuilding, ui: &mut egui::Ui, i: usize) {
    // The name in the header is READABLE rather than a catalogue key, and only a name that was touched
    // gets written back; otherwise merely opening the properties would freeze the automatic name in the
    // current language (that is what `NameSlot::Editable` takes care of).
    let lin = crate::gui::props_card::lineage_of(project, project.mesh_id(i));
    if let Some(n) = props_header(ui, ph::CUBE, "mesh-props-title", NameSlot::Editable(project.mesh_name(i)), &lin) {
        project.set_mesh_name(i, n);
    }
    // the part's colour (so an assembly reads clearly)
    ui.horizontal(|ui| {
        ui.label(crate::i18n::tr("mesh-colour"));
        let mut rgb = project.mesh_color(i);
        if ui.color_edit_button_srgb(&mut rgb).changed() {
            project.set_mesh_color(i, rgb);
            regen.geom_rev = regen.geom_rev.wrapping_add(1); // redraw the 3D view
        }
        if ui.small_button(crate::i18n::tr("mesh-colour-reset")).on_hover_text(crate::i18n::tr("mesh-colour-reset-hint")).clicked() {
            project.reset_mesh_color(i); // drop the manual colour, back to the palette keyed by the lineage root
            regen.geom_rev = regen.geom_rev.wrapping_add(1);
        }
    });
    // INFORMATION, and only what is relevant. A body's size and position change PARAMETRICALLY, through
    // features in the timeline; moving and rotating go through the three-axis gizmo in 3D; booleans and
    // patterns are commands on the toolbar, and they are features too.
    ui.separator();
    if let Some(b) = project.bodies[i].mesh.bounds() {
        ui.label(crate::i18n::tr1("mesh-size", "v", &format!("{:.1} × {:.1} × {:.1}", b.max.x - b.min.x, b.max.y - b.min.y, b.max.z - b.min.z)));
        ui.label(egui::RichText::new(crate::i18n::tr1("mesh-tris-n", "n", &project.bodies[i].mesh.tris.len().to_string())).weak().small());
    } else {
        ui.label(egui::RichText::new(crate::i18n::tr("mesh-empty")).weak());
    }
    ui.separator();
    if ui.button(format!("{} {}", ph::TRASH, crate::i18n::tr("mesh-delete"))).clicked() {
        ask_delete(deferred, Sel::Mesh(i));
    }
}

/// Enter the joint EDITING mode (a double click on its glyph): the top parameter bar plus the popup of
/// anchors A and B. The competing picking and creating modes are cleared and the joint is selected, so
/// its degree-of-freedom gizmo is visible.
pub(crate) fn enter_joint_edit(joint: &mut JointCommand, sel: &mut Sel, status: &mut qymcad_ui_state::Notice, jid: Id) {
    joint.edit = Some(jid);
    joint.edit_repick = None;
    joint.pick_faces = false;
    joint.pick_first = None;
    joint.ground_pick = false;
    *sel = Sel::Joint(jid);
    *status = Notice::info(crate::i18n::tr("g-joint-edit-hint"));
}

/// WHAT THE STATUS LINE SAYS ON ENTERING A SKETCH: its name - or, for a sketch on a face of another part's body,
/// whose face it stands on and that editing that part drives this one. Said here, where the sketch is entered: said at
/// the placement it was overwritten a moment later by the name of the sketch.
pub(crate) fn sketch_entry_status(project: &Project, s: &qymcad_core::model::Sketch) -> String {
    if let qymcad_core::feature::SketchPlane::Face(body, _) = s.plane {
        let (owner, of_sketch) = (project.body_owner(body), project.sketch_owner(s.id));
        if owner.is_some() && owner != of_sketch {
            let src = owner.and_then(|o| project.components.iter().find(|c| c.id == o)).map(|c| crate::i18n::name(&c.name)).unwrap_or_else(|| crate::i18n::tr("g-neighbour"));
            return crate::i18n::tr1("g-sketch-on-foreign-face", "name", &src);
        }
    }
    crate::i18n::tr1("g-editing-sketch", "name", &crate::i18n::name(&s.name))
}
