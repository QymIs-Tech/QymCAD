//! Tests of 3D machining over a Z map.

use qymcad_core::geom::{Mesh, Point2, Point3};
use qymcad_core::heightmap::Heightmap;
use qymcad_core::ir::Move;
use qymcad_core::model::{OpKind, OperationDef, Project};
use qymcad_core::ops::params::Feeds;
use qymcad_core::ops::{Heights, Passes};
use qymcad_core::tool::{Tool, ToolType};

/// An inclined plane z = x/2 over a 10×10 square.
fn ramp_mesh() -> Mesh {
    Mesh {
        verts: vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(10.0, 0.0, 5.0),
            Point3::new(10.0, 10.0, 5.0),
            Point3::new(0.0, 10.0, 0.0),
        ],
        tris: vec![[0, 1, 2], [0, 2, 3]],
    }
}

#[test]
fn heightmap_follows_slope() {
    let hm = Heightmap::from_mesh(&ramp_mesh(), 0.5).unwrap();
    // the height at (1,5) is about 0.5 and at (9,5) about 4.5
    let lo = hm.drop_tool(1.0, 5.0, 0.1, false);
    let hi = hm.drop_tool(9.0, 5.0, 0.1, false);
    assert!(lo < 1.0 && hi > 4.0, "lo={lo} hi={hi}");
    assert!(hi > lo + 3.0, "the slope has to rise");
}

#[test]
fn surface_op_toolpath_tracks_surface() {
    let mut p = Project::default();
    let mid = p.add_mesh(ramp_mesh());
    p.tools = vec![Tool {
        number: 1,
        name: "Ball 2".into(),
        kind: ToolType::BallNose,
        diameter: 2.0,
        corner_radius: 1.0,
        flutes: 2,
        v_angle: None,
    }];
    let mut op = OperationDef::new("Finish", 1, OpKind::Surface3D { mesh: mid });
    op.heights = Heights { clearance: 10.0, retract: 8.0, top: 5.0, bottom: -1.0 };
    op.passes = Passes { stepdown: 1.0, stepover: 2.0, stock_to_leave: 0.0 };
    op.feeds = Feeds::default();
    p.operations.push(op);

    let tp = &p.build_program("3d").toolpaths[0];
    let zs: Vec<f64> = tp.moves.iter().filter_map(|m| match m {
        Move::Linear { to, .. } => Some(to.z),
        _ => None,
    }).collect();
    assert!(!zs.is_empty(), "there have to be cutting moves");
    let zmax = zs.iter().cloned().fold(f64::MIN, f64::max);
    let zmin = zs.iter().cloned().fold(f64::MAX, f64::min);
    // the toolpath runs from the bottom of the slope to the top
    assert!(zmax > 3.5, "the top of the slope is about 5, zmax={zmax}");
    assert!(zmin < 1.5, "the bottom of the slope is about 0, zmin={zmin}");
}

#[test]
fn rough3d_clears_in_layers() {
    let mut p = Project::default();
    let mid = p.add_mesh(ramp_mesh());
    p.tools = vec![Tool {
        number: 1,
        name: "Flat 4".into(),
        kind: ToolType::FlatEnd,
        diameter: 4.0,
        corner_radius: 0.0,
        flutes: 3,
        v_angle: None,
    }];
    let mut op = OperationDef::new("Rough", 1, OpKind::Rough3D { mesh: mid });
    op.heights = Heights { clearance: 10.0, retract: 8.0, top: 5.0, bottom: 0.0 };
    op.passes = Passes { stepdown: 1.0, stepover: 2.0, stock_to_leave: 0.5 };
    p.operations.push(op);

    let tp = &p.build_program("r").toolpaths[0];
    // the cutting moves have to span several distinct Z levels, one per layer
    let mut levels: Vec<i64> = tp
        .moves
        .iter()
        .filter_map(|m| match m {
            Move::Linear { to, .. } => Some((to.z * 10.0).round() as i64),
            _ => None,
        })
        .collect();
    levels.sort_unstable();
    levels.dedup();
    assert!(levels.len() >= 4, "expected several Z layers, found {}", levels.len());
}

/// A 20×20×10 cube: flat 3D machining has to clear the top face at about z = 10.
fn box_mesh() -> Mesh {
    let v = vec![
        Point3::new(0.0, 0.0, 0.0), Point3::new(20.0, 0.0, 0.0), Point3::new(20.0, 20.0, 0.0), Point3::new(0.0, 20.0, 0.0),
        Point3::new(0.0, 0.0, 10.0), Point3::new(20.0, 0.0, 10.0), Point3::new(20.0, 20.0, 10.0), Point3::new(0.0, 20.0, 10.0),
    ];
    let tris = vec![
        [0, 2, 1], [0, 3, 2], [4, 5, 6], [4, 6, 7], [0, 1, 5], [0, 5, 4],
        [3, 2, 6], [3, 6, 7], [0, 4, 7], [0, 7, 3], [1, 2, 6], [1, 6, 5],
    ];
    Mesh { verts: v, tris }
}

/// A 3D operation cuts exactly the selected bodies in `op.bodies` rather than everything at once.
#[test]
fn surface3d_targets_selected_body() {
    let mut p = Project::default();
    let a = p.add_mesh(box_mesh()); // x∈[0,20]
    let mut bm = box_mesh();
    bm.translate(100.0, 0.0, 0.0); // x∈[100,120]
    let b = p.add_mesh(bm);
    p.tools = vec![Tool { number: 1, name: "B2".into(), kind: ToolType::BallNose, diameter: 2.0, corner_radius: 1.0, flutes: 2, v_angle: None }];

    // `kind.mesh` names A while `bodies` holds B, so only body B may be machined
    let mut op = OperationDef::new("Finish", 1, OpKind::Surface3D { mesh: a });
    op.bodies = vec![b];
    op.heights = Heights { clearance: 15.0, retract: 12.0, top: 10.0, bottom: 0.0 };
    op.passes = Passes { stepdown: 1.0, stepover: 3.0, stock_to_leave: 0.0 };
    p.operations.push(op);

    let tp = &p.build_program("3d").toolpaths[0];
    let xs: Vec<f64> = tp.moves.iter().filter_map(|m| match m {
        Move::Linear { to, .. } => Some(to.x),
        _ => None,
    }).collect();
    assert!(!xs.is_empty(), "there have to be cutting moves");
    assert!(xs.iter().all(|&x| x > 50.0), "only the selected body B is machined, at x > 50, and not A");
}

#[test]
fn flat3d_machines_top_plateau() {
    let mut p = Project::default();
    let mid = p.add_mesh(box_mesh());
    p.tools = vec![Tool { number: 1, name: "EM4".into(), kind: ToolType::FlatEnd, diameter: 4.0, corner_radius: 0.0, flutes: 2, v_angle: None }];
    let mut op = OperationDef::new("Flat", 1, OpKind::Flat3D { mesh: mid });
    op.heights = Heights { clearance: 15.0, retract: 12.0, top: 10.0, bottom: 0.0 };
    op.passes = Passes { stepdown: 1.0, stepover: 3.0, stock_to_leave: 0.0 };
    p.operations.push(op);

    let tp = &p.build_program("f").toolpaths[0];
    let cut_at_top = tp.moves.iter().filter(|m| matches!(m, Move::Linear { to, .. } if (to.z - 10.0).abs() < 1e-6)).count();
    assert!(cut_at_top > 3, "the top face at z = 10 has to be cleared, moves: {cut_at_top}");
}

#[test]
fn project_engraves_along_surface() {
    use qymcad_core::geom::Contour;
    let mut p = Project::default();
    let mid = p.add_mesh(ramp_mesh()); // z = x/2 over a 10×10 square
    p.tools = vec![Tool { number: 1, name: "V1".into(), kind: ToolType::BallNose, diameter: 1.0, corner_radius: 0.5, flutes: 2, v_angle: None }];
    // a line across the slope
    p.set_contours(vec![Contour::open(vec![Point2::new(1.0, 5.0), Point2::new(9.0, 5.0)])]);
    let mut op = OperationDef::new("Engrave3D", 1, OpKind::Project3D { mesh: mid });
    op.heights = Heights { clearance: 10.0, retract: 8.0, top: 5.0, bottom: 4.5 }; // engraving 0.5 mm deep
    op.passes = Passes { stepdown: 1.0, stepover: 1.0, stock_to_leave: 0.0 };
    p.operations.push(op);

    let tp = &p.build_program("e").toolpaths[0];
    let zs: Vec<f64> = tp.moves.iter().filter_map(|m| match m {
        Move::Linear { to, .. } => Some(to.z),
        _ => None,
    }).collect();
    assert!(!zs.is_empty(), "there has to be engraving");
    let zmax = zs.iter().cloned().fold(f64::MIN, f64::max);
    let zmin = zs.iter().cloned().fold(f64::MAX, f64::min);
    // Z follows the slope, x/2 − 0.5: from about 0 at x = 1 to about 4 at x = 9
    assert!(zmax - zmin > 3.0, "the engraving follows the surface, Z spread = {}", zmax - zmin);
}

#[test]
fn surface_respects_boundary_region() {
    use qymcad_core::geom::Contour;
    let mut p = Project::default();
    let mid = p.add_mesh(ramp_mesh()); // a 10×10 slope
    p.tools = vec![Tool { number: 1, name: "B2".into(), kind: ToolType::BallNose, diameter: 2.0, corner_radius: 1.0, flutes: 2, v_angle: None }];
    // the boundary 3..7 by 3..7
    let cid = p.add_contour(Contour::closed(vec![
        Point2::new(3.0, 3.0),
        Point2::new(7.0, 3.0),
        Point2::new(7.0, 7.0),
        Point2::new(3.0, 7.0),
    ]));
    let mut op = OperationDef::new("Finish", 1, OpKind::Surface3D { mesh: mid });
    op.selection = vec![cid]; // the contour is the boundary
    op.heights = Heights { clearance: 10.0, retract: 8.0, top: 5.0, bottom: -1.0 };
    op.passes = Passes { stepdown: 1.0, stepover: 1.0, stock_to_leave: 0.0 };
    p.operations.push(op);

    let tp = &p.build_program("b").toolpaths[0];
    // every cutting point lies inside the region, within one map step
    for m in &tp.moves {
        if let Move::Linear { to, .. } = m {
            assert!(to.x >= 2.0 && to.x <= 8.0 && to.y >= 2.0 && to.y <= 8.0, "a point outside the region: {to:?}");
        }
    }
}

/// A pyramid with its apex at the centre over a 20×20 base: the iso-contours at different Z values give closed
/// rings of different sizes, so a waterline operation has to cut level by level.
fn pyramid_mesh() -> Mesh {
    // base at z = 0, apex at (10,10,5)
    let apex = Point3::new(10.0, 10.0, 5.0);
    let c = [
        Point3::new(0.0, 0.0, 0.0),
        Point3::new(20.0, 0.0, 0.0),
        Point3::new(20.0, 20.0, 0.0),
        Point3::new(0.0, 20.0, 0.0),
    ];
    Mesh {
        verts: vec![c[0], c[1], c[2], c[3], apex],
        tris: vec![[0, 1, 4], [1, 2, 4], [2, 3, 4], [3, 0, 4]],
    }
}

#[test]
fn waterline_cuts_at_multiple_levels() {
    let mut p = Project::default();
    let mid = p.add_mesh(pyramid_mesh());
    p.tools = vec![Tool {
        number: 1,
        name: "Ball 2".into(),
        kind: ToolType::BallNose,
        diameter: 2.0,
        corner_radius: 1.0,
        flutes: 2,
        v_angle: None,
    }];
    let mut op = OperationDef::new("WL", 1, OpKind::Waterline3D { mesh: mid });
    op.heights = Heights { clearance: 10.0, retract: 8.0, top: 5.0, bottom: 0.5 };
    op.passes = Passes { stepdown: 1.0, stepover: 1.0, stock_to_leave: 0.0 };
    p.operations.push(op);

    let tp = &p.build_program("wl").toolpaths[0];
    let levels: std::collections::BTreeSet<i64> = tp
        .moves
        .iter()
        .filter_map(|m| match m {
            Move::Linear { to, .. } => Some((to.z * 10.0).round() as i64),
            _ => None,
        })
        .collect();
    assert!(levels.len() >= 3, "a waterline has to cut at several levels, found {}", levels.len());
}
