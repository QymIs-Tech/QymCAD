//! Tests of the project model: the operation tree, automatic side selection, serialisation and tabs.

use qymcad_core::geom::{circle_contour, Contour, Point2};
use qymcad_core::ir::{DrillKind, Move};
use qymcad_core::model::{from_ron, to_ron, OpKind, OperationDef, Project, SideMode};
use qymcad_core::ops::{Heights, Passes, Ramp, Tabs};
use qymcad_core::tool::{Tool, ToolType};

fn endmill(n: u32, d: f64) -> Tool {
    Tool { number: n, name: format!("EM{d}"), kind: ToolType::FlatEnd, diameter: d, corner_radius: 0.0, flutes: 2, v_angle: None }
}

fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Contour {
    Contour::closed(vec![
        Point2::new(x0, y0),
        Point2::new(x1, y0),
        Point2::new(x1, y1),
        Point2::new(x0, y1),
    ])
}

/// A sketch groups contours; removing the sketch removes its contours and clears the operation references to
/// them, leaving contours owned by anything else alone.
#[test]
fn sketch_groups_and_removes_contours() {
    let mut p = Project::default();
    let loose = p.add_contour(circle_contour(0.0, 0.0, 3.0, 0.1));
    let sk = p.add_sketch("rama.dxf", vec![rect(0.0, 0.0, 50.0, 30.0), circle_contour(25.0, 15.0, 5.0, 0.1)], None);

    assert_eq!(p.sketches.len(), 1);
    assert_eq!(p.contours.len(), 3);
    let si = p.sketch_index(sk).unwrap();
    assert_eq!(p.sketches[si].contour_ids.len(), 2);
    assert_eq!(p.loose_contour_ids(), vec![loose]);
    assert_eq!(p.sketch_of_contour(p.sketches[si].contour_ids[0]), Some(sk));

    // an operation references a contour of the sketch
    let cref = p.sketches[si].contour_ids[1];
    let mut op = OperationDef::new("Drill", 1, OpKind::Drill { cycle: DrillKind::Drill, peck: None, dwell: None });
    op.selection = vec![cref];
    p.operations.push(op);

    // remove the sketch: its contours and the reference are gone, the loose contour survives
    p.remove_sketch(si);
    assert_eq!(p.sketches.len(), 0);
    assert_eq!(p.contours.len(), 1, "only the loose contour is left");
    assert_eq!(p.contour_index(loose).map(|i| i), Some(0));
    assert!(p.operations[0].selection.is_empty(), "the reference to the removed contour is cleared");
}

/// Setups: operations are ordered by setup and are tagged with the work coordinate system.
#[test]
fn setups_order_and_tag_wcs() {
    use qymcad_core::model::{Setup, Wcs};
    let mut p = Project::default();
    p.set_contours(vec![rect(0.0, 0.0, 40.0, 40.0)]);
    p.tools = vec![endmill(1, 6.0)];
    p.setups = vec![Setup { name: "A".into(), wcs: Wcs::G54 }, Setup { name: "B".into(), wcs: Wcs::G55 }];

    // op0 belongs to setup B (G55) and op1 to setup A (G54); the output order is A then B
    let mut o0 = OperationDef::new("P0", 1, OpKind::Engrave);
    o0.setup = 1;
    o0.heights.bottom = -1.0;
    let mut o1 = OperationDef::new("P1", 1, OpKind::Engrave);
    o1.setup = 0;
    o1.heights.bottom = -1.0;
    p.operations = vec![o0, o1];

    let prog = p.build_program("x");
    assert_eq!(prog.toolpaths.len(), 2, "two toolpaths");
    assert_eq!(prog.toolpaths[0].meta.wcs, 54, "setup A (G54) comes out first");
    assert_eq!(prog.toolpaths[1].meta.wcs, 55, "setup B (G55) comes second");
}

/// A typed sketch: editing a point re-tessellates the contour while the contour id stays the same.
#[test]
fn typed_sketch_point_edit_regens() {
    let mut p = Project::default();
    let sid = p.add_line_sketch("L", vec![Point2::new(0.0, 0.0), Point2::new(10.0, 0.0), Point2::new(10.0, 10.0)], false);
    let si = p.sketch_index(sid).unwrap();
    assert!(p.is_typed_sketch(si));
    let cid = p.sketches[si].contour_ids[0];
    assert_eq!(p.contours[p.contour_index(cid).unwrap()].points.len(), 3, "three points in the contour");

    let mut op = OperationDef::new("Eng", 1, OpKind::Engrave);
    op.selection = vec![cid];
    p.operations.push(op);

    p.sketches[si].points[1].x = 20.0; // move the second point
    p.regen_sketch(si);
    let c = &p.contours[p.contour_index(cid).unwrap()];
    assert!((c.points[1].x - 20.0).abs() < 1e-9, "the point is updated in the contour");
    assert_eq!(p.operations[0].selection, vec![cid], "associativity: the contour id is preserved");
}

/// Importing DXF or SVG produces an editable sketch built from exact curves: a timeline node in the active
/// context rather than an orphan, a circle kept as a circle entity rather than a polygon, segments sharing
/// deduplicated endpoints so the contour closes, the placement plane preserved, and the contours rebuilt from
/// the entities.
#[test]
fn import_sketch_keeps_circle_and_dedups_shared_corners() {
    use qymcad_core::feature::{BasePlane, SketchPlane};
    use qymcad_core::geom::{Point2, ProfEdge};
    use qymcad_core::model::EntityKind;
    let mut p = Project::default();
    p.new_document();
    let root = p.root;
    p.set_active_component(Some(root));
    let part = p.add_part("part");
    p.set_active_component(Some(part)); // importing while inside a part drops the sketch into it

    // a square made of four segments sharing their corners, plus a circle primitive
    let sq = |x: f64, y: f64| Point2::new(x, y);
    let curves = vec![
        ProfEdge::Line { a: sq(0.0, 0.0), b: sq(40.0, 0.0) },
        ProfEdge::Line { a: sq(40.0, 0.0), b: sq(40.0, 20.0) },
        ProfEdge::Line { a: sq(40.0, 20.0), b: sq(0.0, 20.0) },
        ProfEdge::Line { a: sq(0.0, 20.0), b: sq(0.0, 0.0) },
        ProfEdge::Circle { center: sq(20.0, 10.0), r: 4.0 },
    ];
    let si = p.import_sketch("plan.dxf", curves, None, SketchPlane::World(BasePlane::XZ));
    let sid = p.sketches[si].id;

    // 1) the timeline node exists and belongs to the active context, the part, rather than being an orphan
    let node = p.timeline.iter().find(|n| n.id == sid).expect("sketch node in the timeline");
    assert_eq!(node.parent, Some(part), "the imported sketch belongs to the active part");

    // 2) the placement plane is preserved
    assert_eq!(p.sketches[si].plane, SketchPlane::World(BasePlane::XZ));

    // 3) the circle stays a circle entity and is not turned into a polygon
    let circles = p.sketches[si].entities.iter().filter(|e| matches!(e.kind, EntityKind::Circle { .. })).count();
    assert_eq!(circles, 1, "the circle is imported as a circle entity");

    // 4) the four segments share their corners, giving four contour points plus the circle centre: five in
    // total, so the deduplication worked
    let lines = p.sketches[si].entities.iter().filter(|e| matches!(e.kind, EntityKind::Line { .. })).count();
    assert_eq!(lines, 4, "the square is four segments");
    assert_eq!(p.sketches[si].points.len(), 5, "four shared corners plus the circle centre, endpoints deduplicated");
    assert!(p.is_typed_sketch(si), "the sketch is typed, points plus entities, so it can be edited in the sketcher");

    // 5) the contours are rebuilt: a closed square of area ~800 and a circle of area ~50
    let closed_areas: Vec<f64> = p.sketches[si].contour_ids.iter().filter_map(|cid| p.contour_index(*cid)).map(|ci| p.contours[ci].area()).collect();
    assert!(closed_areas.iter().any(|a| (a - 800.0).abs() < 1.0), "closed square of area ~800: {closed_areas:?}");
    assert!(closed_areas.iter().any(|a| (a - std::f64::consts::PI * 16.0).abs() < 1.0), "circle of area ~50: {closed_areas:?}");
}

/// A circle entity: changing the radius through a dimension and the solver regenerates the contour, while the
/// contour id is preserved, so the operation reference survives.
#[test]
fn entity_circle_regen_keeps_contour_id() {
    use qymcad_core::model::{Constraint, EntityKind};
    let mut p = Project::default();
    let si = p.drawing_sketch();
    p.add_circle_entity(si, 0.0, 0.0, 5.0, qymcad_core::feature::Purpose::Real);
    let center = p.sketches[si].entities.iter().find_map(|e| match e.kind { EntityKind::Circle { center, .. } => Some(center), _ => None }).unwrap();
    let cid = p.sketches[si].contour_ids[0];
    let mut op = OperationDef::new("Bore", 1, OpKind::Bore);
    op.selection = vec![cid];
    p.operations.push(op);

    let r0 = p.contours[p.contour_index(cid).unwrap()].as_circle().unwrap().1;
    assert!((r0 - 5.0).abs() < 0.2, "the starting radius is ~5, got {r0}");

    // a Ø24 dimension means r = 12, and the solver rebuilds the contour
    p.sketches[si].constraints.push(Constraint::Diameter { c: center, d: 24.0, off: 0.0, expr: String::new(), driven: false, diam: true });
    p.solve_sketch(si);

    let r1 = p.contours[p.contour_index(cid).unwrap()].as_circle().unwrap().1;
    assert!((r1 - 12.0).abs() < 0.3, "the radius is updated to ~12, got {r1}");
    assert_eq!(p.operations[0].selection, vec![cid], "associativity: the contour id is preserved");
}

/// A slot cuts along the centreline of an open contour, one Z layer at a time.
#[test]
fn slot_cuts_centerline_in_layers() {
    let mut p = Project::default();
    p.set_contours(vec![Contour::open(vec![Point2::new(0.0, 0.0), Point2::new(50.0, 0.0)])]);
    p.tools = vec![endmill(1, 6.0)];
    let mut op = OperationDef::new("Slot", 1, OpKind::Slot);
    op.heights = Heights { clearance: 5.0, retract: 2.0, top: 0.0, bottom: -3.0 };
    op.passes = Passes { stepdown: 1.0, stepover: 3.0, stock_to_leave: 0.0 };
    p.operations.push(op);

    let tp = &p.build_program("s").toolpaths[0];
    let levels: std::collections::BTreeSet<i64> = tp.moves.iter().filter_map(|m| match m {
        Move::Linear { to, .. } => Some((to.z * 10.0).round() as i64),
        _ => None,
    }).collect();
    assert!(levels.len() >= 3, "several Z layers, found {}", levels.len());
    let on_line = tp.moves.iter().any(|m| matches!(m, Move::Linear { to, .. } if to.y.abs() < 1e-6 && (to.x - 50.0).abs() < 1e-6));
    assert!(on_line, "cuts along the centreline towards (50,0)");
}

/// A finish pass adds a clean-up lap on size, so there are more moves than without it.
#[test]
fn finish_pass_adds_clean_pass() {
    let make = |finish: bool| {
        let mut p = Project::default();
        p.set_contours(vec![rect(0.0, 0.0, 40.0, 40.0)]);
        p.tools = vec![endmill(1, 6.0)];
        let mut op = OperationDef::new("Profile", 1, OpKind::Contour { side: SideMode::Outside, tabs: Tabs::default(), ramp: Ramp::default(), climb: true, finish });
        op.heights = Heights { clearance: 5.0, retract: 2.0, top: 0.0, bottom: -2.0 };
        op.passes = Passes { stepdown: 1.0, stepover: 3.0, stock_to_leave: 0.5 };
        p.operations.push(op);
        let prog = p.build_program("x");
        prog.toolpaths[0].moves.iter().filter(|m| matches!(m, Move::Linear { .. })).count()
    };
    let no_finish = make(false);
    let with_finish = make(true);
    assert!(with_finish > no_finish, "the finish pass adds moves: {with_finish} > {no_finish}");
}

/// An outer frame with a hole inside it: `Auto` has to cut the frame from the outside and the hole from the
/// inside. Checked through `build_program` with two operations.
#[test]
fn auto_side_by_nesting_and_multi_op() {
    let mut p = Project::default();
    p.set_contours(vec![rect(0.0, 0.0, 100.0, 60.0), rect(40.0, 25.0, 60.0, 35.0)]);
    let hole_id = p.contour_id(1).unwrap();
    p.tools = vec![endmill(1, 6.0), endmill(2, 5.0)];

    // operation 1: contour both, with the side left on `Auto`
    let mut contour = OperationDef::new("Profile", 1, OpKind::Contour { side: SideMode::Auto, tabs: Tabs::default(), ramp: Ramp::default() , climb: true, finish: false });
    contour.heights.bottom = -3.0;
    p.operations.push(contour);

    // operation 2: drilling at the centre of the hole, with contour #1 selected
    let mut drill = OperationDef::new("Drill", 2, OpKind::Drill { cycle: DrillKind::Drill, peck: None, dwell: None });
    drill.selection = vec![hole_id];
    drill.heights.bottom = -8.0;
    p.operations.push(drill);

    let prog = p.build_program("part");
    assert_eq!(prog.toolpaths.len(), 2, "two operations give two toolpaths");

    // the drilling has a cycle over a single point, the hole centre at about (50,30)
    let drill_tp = &prog.toolpaths[1];
    let pt = drill_tp.moves.iter().find_map(|m| match m {
        Move::DrillCycle { points, .. } => points.first().copied(),
        _ => None,
    });
    let pt = pt.expect("drill point");
    assert!((pt.x - 50.0).abs() < 1.0 && (pt.y - 30.0).abs() < 1.0, "hole centre");
}

#[test]
fn bore_spirals_circular_hole() {
    // a circle of r = 5 bored with a d4 endmill gives a path radius of 5 − 2 = 3
    let circle = circle_contour(20.0, 20.0, 5.0, 0.05);
    assert!(circle.as_circle().is_some(), "the circle has to be recognised");
    assert!(rect(0.0, 0.0, 10.0, 10.0).as_circle().is_none(), "a square is not a circle");

    let mut p = Project::default();
    p.set_contours(vec![circle]);
    p.tools = vec![endmill(1, 4.0)];
    let mut op = OperationDef::new("Bore", 1, OpKind::Bore);
    op.heights = Heights { clearance: 5.0, retract: 2.0, top: 0.0, bottom: -6.0 };
    op.passes = Passes { stepdown: 1.0, stepover: 2.0, stock_to_leave: 0.0 };
    p.operations.push(op);

    let tp = &p.build_program("b").toolpaths[0];
    let zs: Vec<f64> = tp.moves.iter().filter_map(|m| match m {
        Move::Linear { to, .. } => Some(to.z),
        _ => None,
    }).collect();
    assert!(!zs.is_empty(), "there has to be a spiral");
    let zmin = zs.iter().cloned().fold(f64::MAX, f64::min);
    assert!((zmin + 6.0).abs() < 0.2, "the spiral reaches the bottom at -6, zmin={zmin}");
    // the path runs at radius 3 around (20,20), so points sit about 3 away from the centre
    if let Some(Move::Linear { to, .. }) = tp.moves.iter().find(|m| matches!(m, Move::Linear { .. })) {
        let r = ((to.x - 20.0).powi(2) + (to.y - 20.0).powi(2)).sqrt();
        assert!((r - 3.0).abs() < 0.5, "path radius ~3, r={r}");
    }
}

#[test]
fn simulate_lowers_stock_where_machined() {
    // a pocket in a plate: the simulation has to lower the material in the middle
    let mut p = Project::default();
    p.set_contours(vec![rect(0.0, 0.0, 40.0, 40.0)]);
    p.tools = vec![endmill(1, 6.0)];
    let mut op = OperationDef::new("Pocket", 1, OpKind::Pocket { dogbone: false });
    op.heights = Heights { clearance: 5.0, retract: 2.0, top: 0.0, bottom: -3.0 };
    op.passes = Passes { stepdown: 1.5, stepover: 3.0, stock_to_leave: 0.0 };
    p.operations.push(op);

    let mesh = p.simulate("sim", 1.0).expect("geometry present");
    // in the middle of the pocket the height drops to about -3 where material was removed, and stays 0 at the top
    let zmin = mesh.verts.iter().map(|v| v.z).fold(f64::MAX, f64::min);
    let zmax = mesh.verts.iter().map(|v| v.z).fold(f64::MIN, f64::max);
    assert!(zmin < -2.0, "material removed down to ~-3, zmin={zmin}");
    assert!((zmax - 0.0).abs() < 1e-6, "the top of the stock stays at 0, zmax={zmax}");
}

#[test]
fn disabled_op_is_skipped() {
    let mut p = Project::default();
    p.set_contours(vec![rect(0.0, 0.0, 50.0, 50.0)]);
    p.tools = vec![endmill(1, 6.0)];
    let mut op = OperationDef::new("Profile", 1, OpKind::Contour { side: SideMode::Outside, tabs: Tabs::default(), ramp: Ramp::default() , climb: true, finish: false });
    op.enabled = false;
    p.operations.push(op);
    assert_eq!(p.build_program("x").toolpaths.len(), 0);
}

#[test]
fn project_roundtrips_through_ron() {
    let mut p = Project::default();
    p.set_contours(vec![circle_contour(10.0, 10.0, 5.0, 0.05)]);
    p.tools = vec![endmill(1, 3.0)];
    p.operations.push(OperationDef::new("Engrave", 1, OpKind::Engrave));

    let ron = to_ron(&p).expect("serialize");
    let back = from_ron(&ron).expect("deserialize");
    assert_eq!(back.contours.len(), 1);
    assert_eq!(back.tools.len(), 1);
    assert_eq!(back.operations.len(), 1);
    assert!(matches!(back.operations[0].kind, OpKind::Engrave));
}

#[test]
fn ramp_descends_gradually_without_plunge() {
    let mut p = Project::default();
    p.set_contours(vec![rect(0.0, 0.0, 60.0, 40.0)]);
    p.tools = vec![endmill(1, 6.0)];
    let ramp = Ramp { enabled: true, angle_deg: 3.0 };
    let mut op = OperationDef::new("Profile", 1, OpKind::Contour { side: SideMode::Outside, tabs: Tabs::default(), ramp, climb: true, finish: false });
    op.heights.bottom = -2.0;
    op.passes.stepdown = 1.0;
    p.operations.push(op);

    let tp = &p.build_program("x").toolpaths[0];
    // ramped entry, with no vertical plunge
    assert!(!tp.moves.iter().any(|m| matches!(m, Move::Plunge { .. })), "a ramp must not plunge");
    // there have to be cutting moves at intermediate depths, between 0 and -2
    let has_mid = tp.moves.iter().any(|m| matches!(m, Move::Linear { to, .. } if to.z < -0.05 && to.z > -1.95));
    assert!(has_mid, "expected a gradual descent through intermediate Z values");
}

#[test]
fn tabs_lift_bottom_pass() {
    // A through-cut contour with tabs: the bottom pass has to contain points above the floor, at `tab_top`.
    let mut p = Project::default();
    p.set_contours(vec![rect(0.0, 0.0, 80.0, 50.0)]);
    p.tools = vec![endmill(1, 6.0)];
    let tabs = Tabs { enabled: true, count: 4, width: 6.0, height: 1.5 };
    let mut op = OperationDef::new("Profile", 1, OpKind::Contour { side: SideMode::Outside, tabs, ramp: Ramp::default() , climb: true, finish: false });
    op.heights.bottom = -3.0;
    op.passes.stepdown = 3.0; // a single pass to the bottom
    p.operations.push(op);

    let prog = p.build_program("part");
    let tp = &prog.toolpaths[0];
    // the floor is at -3 and `tab_top` at -1.5, so there have to be linear points with Z ≈ -1.5
    let has_tab = tp.moves.iter().any(|m| matches!(m, Move::Linear { to, .. } if (to.z + 1.5).abs() < 1e-6));
    assert!(has_tab, "expected a lift to tab_top (-1.5) over the tabs");
}

#[test]
fn p1_root_assembly_and_component_kinds() {
    // The root assembly exists, and parts and sub-assemblies are created with the right kind in the active
    // context.
    use qymcad_core::feature::ComponentKind;
    let mut p = Project::default();
    let root = p.ensure_root();
    assert!(root != 0, "the root is created");
    assert_eq!(p.ensure_root(), root, "ensure_root is idempotent");
    assert_eq!(p.component_kind(root), Some(ComponentKind::Assembly), "the root is an assembly");
    assert!(!p.ctx_holds_bodies(root), "an assembly does not hold bodies directly");
    // a part in the root
    let part = p.add_part("Part 1");
    assert_eq!(p.component_kind(part), Some(ComponentKind::Part));
    assert!(p.ctx_holds_bodies(part), "a part holds bodies");
    assert_eq!(p.components.iter().find(|c| c.id == part).unwrap().parent, Some(root), "the part is nested in the root");
    // a sub-assembly
    let sub = p.add_assembly("Sub-assembly");
    assert_eq!(p.component_kind(sub), Some(ComponentKind::Assembly));
}

#[test]
fn p1_active_context_and_datums() {
    // New nodes drop into the active context; datum points and axes are created both as timeline nodes and in
    // their pools.
    use qymcad_core::model::{DatumAxis, DatumPoint};
    let mut p = Project::default();
    let root = p.ensure_root();
    let part = p.add_part("Part");
    p.set_active_component(Some(part));
    assert_eq!(p.active_ctx(), part, "the active context is the part");
    // the sketch and the datums land in the part
    let si = p.new_sketch("Sketch");
    let sid = p.sketches[si].id;
    p.add_sketch_node(sid, "Sketch");
    let dp = p.add_datum_point(DatumPoint { id: 0, name: "P1".into(), at: [1.0, 2.0, 3.0], def: Default::default() });
    let da = p.add_datum_axis(DatumAxis::manual("A1", [0.0; 3], [0.0, 0.0, 1.0]));
    assert_eq!(p.datum_points.len(), 1);
    assert_eq!(p.datum_axes.len(), 1);
    let owner = |id: u64| p.timeline.iter().find(|n| n.id == id).unwrap().parent;
    assert_eq!(owner(sid), Some(part), "the sketch is in the part");
    assert_eq!(owner(dp), Some(part), "the datum point is in the part");
    assert_eq!(owner(da), Some(part), "the datum axis is in the part");
    let _ = root;
}

#[test]
fn p1_migrate_reparents_floating_nodes() {
    // Migrating an older project: floating nodes, those with `parent = None`, move under the root.
    use qymcad_core::feature::{FeatureKind, FeatureNode};
    let mut p = Project::default();
    // imitate an older project: a node without a parent, and no root yet
    let sid = p.alloc_id();
    p.timeline.push(FeatureNode { id: sid, name: "Old sketch".into(), kind: FeatureKind::Sketch { sketch: sid }, parent: None, dirty: false, suppressed: false });
    p.migrate_root();
    let root = p.root;
    assert!(root != 0, "the migration creates the root");
    assert_eq!(p.timeline.iter().find(|n| n.id == sid).unwrap().parent, Some(root), "the floating node is reparented to the root");
    // the only component with `parent == None` is the root component itself
    assert_eq!(p.components.iter().filter(|c| c.parent.is_none()).count(), 1);
}

#[test]
fn copy_paste_sketch_geometry_between_sketches() {
    // Copy geometry out of a sketch into a clipboard together with a base point, then paste it into another
    // sketch offset from that base point to the target: new ids, references remapped, contour reproduced.
    use qymcad_core::model::{EntityKind, Project};
    let mut p = Project::default();
    p.new_document();

    // source: a 10×10 rectangle from the corner (0,0) plus a circle of r = 2 at (5,5)
    let src = p.new_sketch("Source");
    let rect_ids = p.add_rect_entity(src, 0.0, 0.0, 10.0, 10.0, qymcad_core::feature::Purpose::Real);
    let circ = p.add_circle_entity(src, 5.0, 5.0, 2.0, qymcad_core::feature::Purpose::Real);
    let mut sel = rect_ids.clone();
    sel.push(circ);

    // copy with the base point at the corner (0,0)
    let clip = p.copy_sketch_geometry(src, &sel, 0.0, 0.0);
    assert_eq!(clip.entities.len(), 5, "four segments plus the circle");
    assert_eq!(clip.points.len(), 5, "four corners plus the circle centre, corners deduplicated");

    // target: an empty sketch; paste so that the base point lands at (100,100)
    let dst = p.new_sketch("Target");
    let before_pts = p.sketches[dst].points.len();
    let new_ids = p.paste_sketch_geometry(dst, &clip, 100.0, 100.0);
    assert_eq!(new_ids.len(), 5, "all five entities are pasted");
    assert_eq!(p.sketches[dst].points.len(), before_pts + 5, "five new points");

    // the source is untouched
    assert_eq!(p.sketches[src].entities.len(), 5, "the source is intact");

    // the circle in the target moved to (105,105) with r = 2
    let csel: Vec<_> = p.sketches[dst]
        .entities
        .iter()
        .filter_map(|e| if let EntityKind::Circle { center, r } = e.kind { Some((center, r)) } else { None })
        .collect();
    assert_eq!(csel.len(), 1);
    let (cc, cr) = csel[0];
    let cp = p.sketches[dst].points.iter().find(|q| q.id == cc).unwrap();
    assert!((cp.x - 105.0).abs() < 1e-9 && (cp.y - 105.0).abs() < 1e-9, "centre at (105,105): ({},{})", cp.x, cp.y);
    assert!((cr - 2.0).abs() < 1e-9);

    // the closed rectangular contour of area ~100 is reproduced in the target
    let areas: Vec<f64> = p.sketches[dst].contour_ids.iter().filter_map(|cid| p.contour_index(*cid)).map(|ci| p.contours[ci].area()).collect();
    assert!(areas.iter().any(|a| (a - 100.0).abs() < 1.0), "rectangle of ~100: {areas:?}");
}

#[test]
fn clone_component_deep_reparents_and_remaps() {
    // A deep clone of a part, sketch and extrude together, under a different component: new ids, references
    // rewired to the clones, and the original untouched.
    use qymcad_core::feature::{FeatureKind, SketchPlane, BasePlane};
    use qymcad_core::model::Project;
    let mut p = Project::default();
    let root = p.ensure_root();
    let part = p.add_part("Part");
    p.set_active_component(Some(part));
    let si = p.new_sketch("Sketch");
    let sid = p.sketches[si].id;
    p.sketches[si].plane = SketchPlane::World(BasePlane::XY);
    p.add_sketch_node(sid, "Sketch");
    p.add_rect_entity(si, 0.0, 0.0, 10.0, 10.0, qymcad_core::feature::Purpose::Real);
    let body = p.add_extrude(sid, 5.0);

    p.set_active_component(Some(root)); // the sub-assembly goes in the root, not inside the part
    let sub = p.add_assembly("Sub-assembly");

    let n_comp = p.components.len();
    let n_sk = p.sketches.len();
    let clone = p.clone_component(part, sub).expect("clone succeeded");

    assert_ne!(clone, part, "a new id");
    assert_eq!(p.components.len(), n_comp + 1, "one more component");
    assert_eq!(p.sketches.len(), n_sk + 1, "one more sketch");
    // the clone is nested in the sub-assembly
    assert_eq!(p.components.iter().find(|c| c.id == clone).unwrap().parent, Some(sub));
    // the original is intact, still in the root through `part`
    assert_eq!(p.components.iter().find(|c| c.id == part).unwrap().parent, Some(root));

    // the clone has its own sketch node and its own extrude node, and the bodies differ
    let clone_nodes: Vec<&FeatureKind> = p.timeline.iter().filter(|n| n.parent == Some(clone)).map(|n| &n.kind).collect();
    assert_eq!(clone_nodes.len(), 2, "a sketch and an extrude on the clone");
    let clone_body = clone_nodes.iter().find_map(|k| k.body());
    assert!(clone_body.is_some() && clone_body != Some(body), "the body of the clone has a new id");
    // the extrude of the clone references the sketch of the clone, not the original `sid`
    let clone_extr = clone_nodes.iter().find_map(|k| if let FeatureKind::Extrude { sketch, .. } = k { Some(*sketch) } else { None }).unwrap();
    assert_ne!(clone_extr, sid, "the sketch reference is rewired to the clone");
    assert!(p.sketches.iter().any(|s| s.id == clone_extr), "the cloned sketch exists");
}

#[test]
fn clone_and_move_sketch_node_between_components() {
    // Cloning a single sketch into another part, and reparenting the node, which is cut and paste.
    use qymcad_core::model::Project;
    let mut p = Project::default();
    p.ensure_root();
    let a = p.add_part("A");
    p.set_active_component(Some(a));
    let si = p.new_sketch("S");
    let sid = p.sketches[si].id;
    p.add_sketch_node(sid, "S");
    p.add_circle_entity(si, 0.0, 0.0, 3.0, qymcad_core::feature::Purpose::Real);
    let b = p.add_part("B");

    // a copy in B
    let clone = p.clone_sketch_node(sid, b).expect("cloned sketch");
    assert_ne!(clone, sid);
    assert_eq!(p.timeline.iter().find(|n| n.id == clone).unwrap().parent, Some(b), "the clone sits under B");
    assert_eq!(p.timeline.iter().find(|n| n.id == sid).unwrap().parent, Some(a), "the original stays under A");

    // reparent the original into B, i.e. cut and paste
    assert!(p.move_sketch_node(sid, b));
    assert_eq!(p.timeline.iter().find(|n| n.id == sid).unwrap().parent, Some(b), "the original is reparented into B");
}

#[test]
fn clone_sketch_detaches_foreign_datum_plane_to_xy() {
    // A copy of a sketch whose plane rested on a datum or a face belonging to a different part used to hang
    // with `sketch_frame = None`, leaving nothing to extrude. The clone has to detach onto XY and resolve
    // again.
    use qymcad_core::feature::{BasePlane, SketchPlane};
    use qymcad_core::model::{Project, WorkPlane};
    let mut p = Project::default();
    p.ensure_root();
    let a = p.add_part("A");
    p.set_active_component(Some(a));
    // a datum plane owned by A, shifted so that it does not coincide with the world plane
    let pid = p.add_plane(WorkPlane { name: "D".into(), origin: [0.0, 0.0, 25.0], normal: [0.0, 0.0, 1.0], ..Default::default() });
    let si = p.new_sketch("S");
    let sid = p.sketches[si].id;
    p.add_sketch_node(sid, "S");
    p.sketches[si].plane = SketchPlane::Datum(pid);
    p.add_circle_entity(si, 0.0, 0.0, 4.0, qymcad_core::feature::Purpose::Real);
    let b = p.add_part("B");

    let clone = p.clone_sketch_node(sid, b).expect("cloned sketch");
    let ci = p.sketch_index(clone).unwrap();
    assert!(matches!(p.sketches[ci].plane, SketchPlane::World(BasePlane::XY)), "a datum of another part falls back to XY: {:?}", p.sketches[ci].plane);
    assert!(p.sketch_frame(ci).is_some(), "the clone resolves again and can be extruded");
    // the original is untouched and stays on its own datum
    let oi = p.sketch_index(sid).unwrap();
    assert!(matches!(p.sketches[oi].plane, SketchPlane::Datum(_)), "the original is on the datum");
}

#[test]
fn reparent_keeps_world_position_and_inner_refs() {
    // Reparenting a part into a transformed sub-assembly: the world position is preserved, so the part does
    // not jump, and the internal references, sketch and extrude, keep their ids, so a rebuild produces the same
    // body.
    use qymcad_core::feature::{apply12, BasePlane, FeatureKind, SketchPlane};
    use qymcad_core::model::Project;
    let mut p = Project::default();
    let root = p.ensure_root();
    let part = p.add_part("Part");
    p.set_active_component(Some(part));
    let si = p.new_sketch("Sketch");
    let sid = p.sketches[si].id;
    p.sketches[si].plane = SketchPlane::World(BasePlane::XY);
    p.add_sketch_node(sid, "Sketch");
    p.add_rect_entity(si, 0.0, 0.0, 10.0, 10.0, qymcad_core::feature::Purpose::Real);
    let body = p.add_extrude(sid, 5.0);
    // the part sits in the world with a shift of its own
    let pi = p.component_index(part).unwrap();
    p.components[pi].transform = [1.0, 0.0, 0.0, 7.0, 0.0, 1.0, 0.0, 3.0, 0.0, 0.0, 1.0, 0.0];
    let world_before = apply12(&p.world_transform(part), [0.0, 0.0, 0.0]);

    // a sub-assembly with a shift of its own
    p.set_active_component(Some(root));
    let sub = p.add_assembly("Sub-assembly");
    let sii = p.component_index(sub).unwrap();
    p.components[sii].transform = [1.0, 0.0, 0.0, 100.0, 0.0, 1.0, 0.0, -40.0, 0.0, 0.0, 1.0, 20.0];

    // a snapshot of the nodes of the part before the move, plus an extractor for the extrude references
    let node_count = |p: &Project| p.timeline.iter().filter(|n| n.parent == Some(part)).count();
    let extr_ref = |p: &Project| {
        p.timeline
            .iter()
            .filter(|n| n.parent == Some(part))
            .find_map(|n| match n.kind {
                FeatureKind::Extrude { sketch, body, .. } => Some((sketch, body)),
                _ => None,
            })
            .unwrap()
    };
    let count_before = node_count(&p);
    let extr_before = extr_ref(&p);

    assert!(p.reparent_component(part, sub));
    assert_eq!(p.components.iter().find(|c| c.id == part).unwrap().parent, Some(sub), "reparented into the sub-assembly");

    // 1) the world position is preserved and the part did not jump
    let world_after = apply12(&p.world_transform(part), [0.0, 0.0, 0.0]);
    for k in 0..3 {
        assert!((world_before[k] - world_after[k]).abs() < 1e-9, "world position preserved along axis {k}: {world_before:?} vs {world_after:?}");
    }
    // 2) the internal nodes, sketch and extrude, keep their ids and their references
    assert_eq!(node_count(&p), count_before, "the node count of the part is unchanged");
    assert_eq!(extr_before, extr_ref(&p), "the extrude references the same sketch and body, so a rebuild produces the same result");
    assert_eq!(extr_ref(&p), (sid, body), "the references point at the original sketch and body of the part");
}

#[test]
fn reparent_component_rejects_cycles() {
    use qymcad_core::model::Project;
    let mut p = Project::default();
    let root = p.ensure_root();
    let a = p.add_assembly("A");
    p.set_active_component(Some(a));
    let b = p.add_assembly("B"); // b lives inside a
    // a cannot be nested into its own descendant b
    assert!(!p.reparent_component(a, b), "a cycle is rejected");
    // the root cannot be moved
    assert!(!p.reparent_component(root, a));
    // b can be reparented into the root
    assert!(p.reparent_component(b, root));
    assert_eq!(p.components.iter().find(|c| c.id == b).unwrap().parent, Some(root));
}

#[test]
fn step_import_single_solid_makes_one_part() {
    // A single STEP solid becomes one part with an imported base body that can be built upon.
    use qymcad_core::feature::ComponentKind;
    use qymcad_core::geom::Mesh;
    use qymcad_core::model::Project;
    let mut p = Project::default();
    let root = p.ensure_root();
    let src = p.add_source("cube.step", vec![1, 2, 3]);
    let body = p.add_mesh(Mesh::default());
    let created = p.import_bodies_as_parts(vec![(body, "Cube".into(), src, 0)], "cube").expect("created");

    assert_eq!(p.component_kind(created), Some(ComponentKind::Part), "a single solid becomes a part");
    assert_eq!(p.components.iter().find(|c| c.id == created).unwrap().parent, Some(root), "in the root");
    assert_eq!(p.component_bodies(created), vec![body], "the imported body belongs to the part");
    // the import node is present under the part
    assert!(p.timeline.iter().any(|n| n.parent == Some(created) && matches!(n.kind, qymcad_core::feature::FeatureKind::Import { body: b, .. } if b == body)));
}

#[test]
fn step_import_multi_solid_makes_subassembly() {
    // Several solids become a sub-assembly holding one part per solid.
    use qymcad_core::feature::ComponentKind;
    use qymcad_core::geom::Mesh;
    use qymcad_core::model::Project;
    let mut p = Project::default();
    let root = p.ensure_root();
    let src = p.add_source("asm.step", vec![9]);
    let b0 = p.add_mesh(Mesh::default());
    let b1 = p.add_mesh(Mesh::default());
    let b2 = p.add_mesh(Mesh::default());
    let solids = vec![(b0, "A".into(), src, 0), (b1, "B".into(), src, 1), (b2, "C".into(), src, 2)];
    let asm = p.import_bodies_as_parts(solids, "asm").expect("created");

    assert_eq!(p.component_kind(asm), Some(ComponentKind::Assembly), "several solids become a sub-assembly");
    assert_eq!(p.components.iter().find(|c| c.id == asm).unwrap().parent, Some(root));
    // three parts inside the sub-assembly, each with one imported body
    let parts: Vec<_> = p.components.iter().filter(|c| c.parent == Some(asm) && c.kind == ComponentKind::Part).map(|c| c.id).collect();
    assert_eq!(parts.len(), 3, "one part per solid");
    for (part, body) in parts.iter().zip([b0, b1, b2]) {
        assert_eq!(p.component_bodies(*part), vec![body], "the part holds its own solid");
    }
    // the active context is restored and did not leak into the sub-assembly
    assert_ne!(p.current_ctx(), asm, "active_component did not stick to the sub-assembly");
}

#[test]
fn component_into_part_is_forbidden() {
    // A part inside a part is forbidden: a component may only be placed into an assembly.
    use qymcad_core::model::Project;
    let mut p = Project::default();
    p.ensure_root();
    let part = p.add_part("Part");
    let sub = p.add_assembly("Sub-assembly");
    // neither reparenting nor cloning a sub-assembly into a part is allowed
    assert!(!p.reparent_component(sub, part), "reparenting a component into a part is rejected");
    assert!(p.clone_component(sub, part).is_none(), "cloning a component into a part is rejected");
    // into an assembly it is allowed
    let root = p.root;
    assert!(p.reparent_component(sub, root));
    assert!(p.clone_component(sub, root).is_some());
}
