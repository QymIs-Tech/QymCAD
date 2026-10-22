//! An integration test of the DXF import.

use qymcad_io::import_dxf;
use std::io::Write;

/// The fixture: a closed 10×10 polyline square, a circle of r = 5, and two lines forming an open chain from
/// (0,0) through (5,0) to (5,5).
const FIXTURE: &str = "\
0
SECTION
2
ENTITIES
0
LWPOLYLINE
8
0
90
4
70
1
10
0.0
20
0.0
10
10.0
20
0.0
10
10.0
20
10.0
10
0.0
20
10.0
0
CIRCLE
8
0
10
20.0
20
20.0
40
5.0
0
LINE
8
0
10
0.0
20
0.0
11
5.0
21
0.0
0
LINE
8
0
10
5.0
20
0.0
11
5.0
21
5.0
0
ENDSEC
0
EOF
";

fn write_fixture() -> std::path::PathBuf {
    let path = std::env::temp_dir().join("qymcad_dxf_test.dxf");
    let mut f = std::fs::File::create(&path).unwrap();
    f.write_all(FIXTURE.as_bytes()).unwrap();
    path
}

#[test]
fn imports_exact_curves_square_circle_lines() {
    use qymcad_core::geom::ProfEdge;
    let path = write_fixture();
    let sketch = import_dxf(path.to_str().unwrap()).expect("import ok");

    // Exact curves: the polyline square gives four segments and is closed, the circle gives one primitive, and
    // the two lines give two segments. Six segments plus one circle, rather than a tessellation into hundreds
    // of segments.
    let circles: Vec<f64> = sketch.curves.iter().filter_map(|e| if let ProfEdge::Circle { r, .. } = e { Some(*r) } else { None }).collect();
    assert_eq!(circles.len(), 1, "the circle stayed a circle primitive");
    assert!((circles[0] - 5.0).abs() < 1e-9, "the radius of the circle is 5");

    let lines = sketch.curves.iter().filter(|e| matches!(e, ProfEdge::Line { .. })).count();
    assert_eq!(lines, 6, "the square gives four and the two separate lines give two, so six segments");

    assert_eq!(sketch.curves.len(), 7, "seven exact curves in total, with no tessellation of arcs or circles");
}

/// A FOLDER for the files written by these checks, under `target`.
fn written(name: &str) -> String {
    let dir = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/dxf-probe"));
    std::fs::create_dir_all(&dir).expect("a folder for the check");
    dir.join(name).to_string_lossy().into_owned()
}

/// SPLINES, ELLIPSES AND BLOCKS ARE READ, and what is not read is named. A drawing from another CAD - the kind a DWG
/// converts into - holds a full ellipse about (50, 0) of half-axes 20 and 10, a cubic spline with the control points of
/// a Bezier from (0, 0) to (40, 0), a block of one line from (0, 0) to (10, 0) inserted at (100, 100) turned 90 deg and
/// scaled twice, and a text. The ellipse comes in as that ellipse and the spline as that cubic - through its middle at
/// (20, 0) - the line of the block from (100, 100) to (100, 120), and the text is named as not read rather than silently
/// lost.
#[test]
fn splines_ellipses_and_blocks_are_read() {
    use dxf::entities::{Ellipse, Entity, EntityType, Insert, Line, Spline, Text};
    use dxf::{Block, Drawing, Point, Vector};
    let mut d = Drawing::new();
    d.header.version = dxf::enums::AcadVersion::R2013; // ellipses and splines came with R13; the default writes R12
    d.add_entity(Entity::new(EntityType::Ellipse(Ellipse {
        center: Point::new(50.0, 0.0, 0.0),
        major_axis: Vector::new(20.0, 0.0, 0.0),
        minor_axis_ratio: 0.5,
        start_parameter: 0.0,
        end_parameter: std::f64::consts::TAU,
        ..Default::default()
    })));
    let mut spline = Spline { degree_of_curve: 3, knot_values: vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0], ..Default::default() };
    spline.control_points = vec![Point::new(0.0, 0.0, 0.0), Point::new(10.0, 20.0, 0.0), Point::new(30.0, -20.0, 0.0), Point::new(40.0, 0.0, 0.0)];
    d.add_entity(Entity::new(EntityType::Spline(spline)));
    let mut block = Block { name: "B".into(), ..Default::default() };
    block.entities.push(Entity::new(EntityType::Line(Line::new(Point::new(0.0, 0.0, 0.0), Point::new(10.0, 0.0, 0.0)))));
    d.add_block(block);
    d.add_entity(Entity::new(EntityType::Insert(Insert { name: "B".into(), location: Point::new(100.0, 100.0, 0.0), x_scale_factor: 2.0, y_scale_factor: 2.0, rotation: 90.0, ..Default::default() })));
    d.add_entity(Entity::new(EntityType::Text(Text { value: "note".into(), ..Default::default() })));
    let p = written("curves.dxf");
    d.save_file(&p).expect("written");

    let got = import_dxf(&p).expect("read");
    use qymcad_core::geom::ProfEdge;
    let ellipse = got.curves.iter().find(|c| matches!(c, ProfEdge::Oval { .. }));
    let on_ellipse = ellipse.is_some_and(|e| (0..=64).filter_map(|k| e.curve_at(k as f64 / 64.0)).all(|q| (((q.x - 50.0) / 20.0).powi(2) + (q.y / 10.0).powi(2) - 1.0).abs() < 1e-9));
    assert!(on_ellipse && ellipse.is_some_and(|e| e.ends().is_none()), "the ellipse did not come in as the whole ellipse about (50, 0) of 20 and 10: {ellipse:?}");
    let spline = got.curves.iter().find(|c| matches!(c, ProfEdge::Spline(_)));
    let middle = spline.and_then(|s| s.curve_at(0.5));
    assert!(middle.is_some_and(|m| (m.x - 20.0).abs() < 1e-9 && m.y.abs() < 1e-9), "the spline does not run through its middle at (20, 0): {spline:?}");
    let ends = spline.and_then(|s| s.ends());
    assert!(ends.is_some_and(|[a, b]| a.x.abs() < 1e-12 && a.y.abs() < 1e-12 && (b.x - 40.0).abs() < 1e-12 && b.y.abs() < 1e-12), "the spline does not end at its end points: {ends:?}");
    let inserted = got
        .curves
        .iter()
        .any(|c| matches!(c, qymcad_core::geom::ProfEdge::Line { a, b } if (a.x - 100.0).abs() < 1e-9 && (a.y - 100.0).abs() < 1e-9 && (b.x - 100.0).abs() < 1e-9 && (b.y - 120.0).abs() < 1e-9));
    assert!(inserted, "the block's line is not where its insert puts it");
    assert_eq!(got.skipped, vec![("TEXT".to_string(), 1)], "what was not read is not named");
}

/// WHAT REAL DRAWINGS BRING IN: every `.dxf` in the folder `QYM_DXF` read, and its curves counted by kind, with what was
/// not read. Ignored: it reads files that are not in the tree.
#[test]
#[ignore = "reads the files under QYM_DXF"]
fn real_drawings_are_measured() {
    let Ok(dir) = std::env::var("QYM_DXF") else { return };
    let mut files: Vec<_> = std::fs::read_dir(&dir).expect("the folder").flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("dxf"))).collect();
    files.sort();
    for f in files {
        match import_dxf(&f.to_string_lossy()) {
            Ok(s) => {
                let (mut lines, mut arcs, mut circles) = (0, 0, 0);
                let mut curves = 0;
                for c in &s.curves {
                    match c {
                        qymcad_core::geom::ProfEdge::Line { .. } => lines += 1,
                        qymcad_core::geom::ProfEdge::Arc { .. } => arcs += 1,
                        qymcad_core::geom::ProfEdge::Circle { .. } => circles += 1,
                        qymcad_core::geom::ProfEdge::Oval { .. } | qymcad_core::geom::ProfEdge::Spline(_) => curves += 1,
                    }
                }
                println!("DRAWING {}: lines {lines}, arcs {arcs}, circles {circles}, ellipses and splines {curves}; not read {:?}", f.display(), s.skipped);
            }
            Err(e) => println!("DRAWING {}: refused - {e}", f.display()),
        }
    }
}

/// A DRAWING IS NOT REFUSED FOR AN OBJECT IT DOES NOT NEED. A drawing of a modern CAD holds, besides its entities, an
/// OBJECTS section of styles; the reader took a VISUALSTYLE object (as AutoCAD 2018 writes it, kept in
/// `data/visualstyle-object.dxf.part`) for a broken file and refused the whole drawing - both drawings converted from the
/// owner's DWG files were refused so, a line of 229 and 98 arcs lost with them. The line of this drawing comes in.
#[test]
fn an_unneeded_object_does_not_refuse_the_drawing() {
    let object = include_str!("data/visualstyle-object.dxf.part");
    let text = format!("0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1032\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n0\nLINE\n8\n0\n10\n0.0\n20\n0.0\n30\n0.0\n11\n10.0\n21\n0.0\n31\n0.0\n0\nENDSEC\n0\nSECTION\n2\nOBJECTS\n{object}0\nENDSEC\n0\nEOF\n");
    let p = written("objects.dxf");
    std::fs::write(&p, text).expect("written");
    let got = import_dxf(&p).expect("the drawing is read, not refused for its styles");
    assert_eq!(got.curves.len(), 1, "the line of the drawing did not come in: {:?}", got.curves);
}

/// A file of `tests/data`.
fn data(name: &str) -> String {
    format!("{}/tests/data/{name}", env!("CARGO_MANIFEST_DIR"))
}

/// Whether every end of `curves` meets exactly one other end: the curves make closed contours.
fn closed(curves: &[qymcad_core::geom::ProfEdge]) -> bool {
    let ends: Vec<qymcad_core::geom::Point2> = curves.iter().filter_map(|c| c.ends()).flatten().collect();
    ends.iter().all(|p| ends.iter().filter(|q| q.dist(*p) < 1e-9).count() == 2)
}

fn near(p: Option<qymcad_core::geom::Point2>, (x, y): (f64, f64)) -> bool {
    p.is_some_and(|p| (p.x - x).abs() < 1e-9 && (p.y - y).abs() < 1e-9)
}

/// AN ENTITY IS READ IN ITS OWN PLANE: the centre of an ARC or a CIRCLE, the vertices of a polyline and the place of an
/// INSERT are given in the entity's object coordinate system, whose X axis the arbitrary axis algorithm takes from the
/// extrusion direction (210/220/230) - for (0,0,-1) it is -X of the drawing, and a turn counter-clockwise in it runs
/// clockwise on the drawing. `slot_ocs.dxf`: a 40 x 20 slot whose right arc is written that way, centre (-40,10), 90 to
/// 270 deg. Reported behaviour: the arc came in about (-40,10), the slot open in four contours. `slot_plain.dxf` is the
/// same slot with the arc written in the drawing's own axes.
#[test]
fn an_entity_is_read_in_its_own_plane() {
    use dxf::entities::{Arc, Circle, Ellipse, Entity, EntityType, Insert, Line, LwPolyline};
    use dxf::{Block, LwPolylineVertex, Point, Vector};
    use qymcad_core::geom::ProfEdge;
    let got = import_dxf(&data("slot_ocs.dxf")).expect("read").curves;
    let right = got.iter().find(|c| matches!(c, ProfEdge::Arc { center, .. } if center.x.abs() > 20.0));
    assert!(
        matches!(right, Some(ProfEdge::Arc { center, .. }) if near(Some(*center), (40.0, 10.0)))
            && near(right.and_then(|c| c.ends()).map(|e| e[0]), (40.0, 20.0))
            && near(right.and_then(|c| c.ends()).map(|e| e[1]), (40.0, 0.0)),
        "the right arc is not the one about (40,10) from (40,20) to (40,0): {right:?}"
    );
    assert!(matches!(right, Some(ProfEdge::Arc { ccw: false, .. })), "the right arc does not run clockwise, through (50,10): {right:?}");
    assert!(closed(&got), "the slot is not closed: {got:?}");
    assert!(closed(&import_dxf(&data("slot_plain.dxf")).expect("read").curves), "the control slot is not closed");

    // a circle, a polyline with a bulge and an insert, each written with the extrusion (0,0,-1)
    let down = Vector::new(0.0, 0.0, -1.0);
    let mut d = dxf::Drawing::new();
    d.header.version = dxf::enums::AcadVersion::R2013; // the lightweight polyline came with R14; the default writes R12
    d.add_entity(Entity::new(EntityType::Circle(Circle { center: Point::new(-40.0, 10.0, 0.0), radius: 5.0, normal: down.clone(), ..Default::default() })));
    let mut poly = LwPolyline { extrusion_direction: down.clone(), ..Default::default() };
    poly.vertices = vec![LwPolylineVertex { x: 0.0, y: 50.0, bulge: 1.0, ..Default::default() }, LwPolylineVertex { x: -10.0, y: 50.0, ..Default::default() }];
    d.add_entity(Entity::new(EntityType::LwPolyline(poly)));
    let mut block = Block { name: "B".into(), ..Default::default() };
    block.entities.push(Entity::new(EntityType::Line(Line::new(Point::new(0.0, 0.0, 0.0), Point::new(10.0, 0.0, 0.0)))));
    d.add_block(block);
    d.add_entity(Entity::new(EntityType::Insert(Insert { name: "B".into(), location: Point::new(-100.0, 0.0, 0.0), extrusion_direction: down.clone(), ..Default::default() })));
    d.add_entity(Entity::new(EntityType::Ellipse(Ellipse {
        center: Point::new(50.0, -50.0, 0.0),
        major_axis: Vector::new(20.0, 0.0, 0.0),
        minor_axis_ratio: 0.5,
        start_parameter: 0.0,
        end_parameter: std::f64::consts::FRAC_PI_2,
        normal: down.clone(),
    })));
    let p = written("ocs.dxf");
    d.save_file(&p).expect("written");
    let got = import_dxf(&p).expect("read").curves;
    assert!(got.iter().any(|c| matches!(c, ProfEdge::Circle { center, r } if near(Some(*center), (40.0, 10.0)) && (r - 5.0).abs() < 1e-9)), "the circle is not about (40,10): {got:?}");
    let bulge = got.iter().find(|c| matches!(c, ProfEdge::Arc { .. }));
    assert!(
        matches!(bulge, Some(ProfEdge::Arc { a, b, center, ccw: false }) if near(Some(*a), (0.0, 50.0)) && near(Some(*b), (10.0, 50.0)) && near(Some(*center), (5.0, 50.0))),
        "the bulge does not run clockwise about (5,50) from (0,50) over (5,55) to (10,50): {bulge:?}"
    );
    // the ellipse's centre and major axis are the drawing's; its minor axis is (0,0,-1) x (20,0,0) / 2 = (0,-10)
    let ellipse = got.iter().find(|c| matches!(c, ProfEdge::Oval { .. }));
    assert!(near(ellipse.and_then(|c| c.curve_at(1.0)), (50.0, -60.0)), "the quarter ellipse does not end at (50,-60): {ellipse:?}");
    assert!(got.iter().any(|c| matches!(c, ProfEdge::Line { a, b } if near(Some(*a), (100.0, 0.0)) && near(Some(*b), (90.0, 0.0)))), "the block's line is not at (100,0)-(90,0): {got:?}");

    // an arc standing across the drawing plane is not drawn flat: it is named as not read
    let mut d = dxf::Drawing::new();
    d.add_entity(Entity::new(EntityType::Arc(Arc { center: Point::new(0.0, 0.0, 7.0), radius: 5.0, start_angle: 0.0, end_angle: 180.0, normal: Vector::new(1.0, 0.0, 0.0), ..Default::default() })));
    let p = written("across.dxf");
    d.save_file(&p).expect("written");
    let got = import_dxf(&p).expect("read");
    assert!(got.curves.is_empty(), "an arc across the drawing plane was drawn: {:?}", got.curves);
    assert_eq!(got.skipped, vec![("ARC".to_string(), 1)], "the arc across the plane is not named");
}
