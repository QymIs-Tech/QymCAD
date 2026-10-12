//! A DRAWING SHEET FOLLOWS THE MODEL: views of a part stay live while the part stands, a projected
//! view hangs off its parent, and a dimension over the project parameters reads the edited value with no
//! rebuild of the sheet. Deleting nothing here needs the kernel: expressions evaluate headless.
use qymcad_core::drawing::{DrawingDimSource, SheetFormat, SheetPos, ViewDir};
use qymcad_core::geom::Point2;
use qymcad_core::model::{Param, Project};

fn part_with_block() -> (Project, u64) {
    let mut p = Project::default();
    let part = p.new_document();
    let sid = p.add_line_sketch("s", vec![Point2::new(0.0, 0.0), Point2::new(40.0, 0.0), Point2::new(40.0, 30.0), Point2::new(0.0, 30.0)], true);
    p.add_sketch_node(sid, "s");
    p.add_extrude(sid, 20.0);
    (p, part)
}

#[test]
fn a_sheet_views_its_part_and_a_dim_follows_the_parameter() {
    let (mut p, part) = part_with_block();
    p.parameters.push(Param { name: "h".into(), expr: "20".into(), value: 20.0 });

    let sheet = p.add_drawing_sheet("sheet", SheetFormat::A4Landscape);
    let size = p.drawing_sheet(sheet).expect("the sheet is there").size;
    assert!((size.width - 297.0).abs() < 1e-9 && (size.height - 210.0).abs() < 1e-9, "A4 landscape is 297x210: {size:?}");

    let base = p.sheet_add_base_view(sheet, part, ViewDir::Front, 1.0, SheetPos { x: 100.0, y: 100.0 }).expect("a view of the part");
    let proj = p.sheet_add_projected_view(sheet, base, ViewDir::Top, 1.0, SheetPos { x: 100.0, y: 200.0 }).expect("a view off the base");
    assert!(p.drawing_view_is_live(sheet, base), "the base view is live while its part stands");
    assert!(p.drawing_view_is_live(sheet, proj), "the projected view is live off its live parent");

    let dim = p.sheet_add_dim(sheet, base, DrawingDimSource::Expression { expr: "h".into() }).expect("a dimension of the view");
    assert_eq!(p.drawing_dim_text(sheet, dim).as_deref(), Some("20"), "the dimension reads the parameter");

    // An edit of the parameter moves the dimension with it: the text is derived on every reading.
    p.parameters.iter_mut().find(|q| q.name == "h").expect("h").expr = "30".into();
    p.eval_parameters();
    assert_eq!(p.drawing_dim_text(sheet, dim).as_deref(), Some("30"), "the dimension followed the edit with no rebuild of the sheet");
}

#[test]
fn a_view_of_nothing_is_not_made() {
    let (mut p, _part) = part_with_block();
    let sheet = p.add_drawing_sheet("sheet", SheetFormat::A3Landscape);
    assert!(p.sheet_add_base_view(sheet, 999999, ViewDir::Front, 1.0, SheetPos { x: 10.0, y: 10.0 }).is_none(), "no component 999999 stands");
    assert!(p.sheet_add_projected_view(sheet, 888888, ViewDir::Top, 1.0, SheetPos { x: 10.0, y: 10.0 }).is_none(), "no parent view 888888 stands");
    let size = p.drawing_sheet(sheet).expect("sheet").size;
    assert!((size.width - 420.0).abs() < 1e-9, "A3 landscape is 420 wide: {size:?}");
    assert!(!p.drawing_view_is_live(sheet, 777777), "no view 777777 is live");
    assert!(p.drawing_dim_text(sheet, 666666).is_none(), "no dimension 666666 has text");
}

#[test]
fn an_empty_dimension_is_refused() {
    let (mut p, part) = part_with_block();
    let sheet = p.add_drawing_sheet("sheet", SheetFormat::A4Portrait);
    let base = p.sheet_add_base_view(sheet, part, ViewDir::Iso, 2.0, SheetPos { x: 50.0, y: 50.0 }).expect("a view");
    assert!(p.sheet_add_dim(sheet, base, DrawingDimSource::Expression { expr: "  ".into() }).is_none(), "a dimension showing nothing is not made");
}

#[test]
fn a_hole_distance_is_grounded_while_both_bodies_stand() {
    let mut p = Project::default();
    let _part = p.new_document();
    let mut bodies = Vec::new();
    for (name, x) in [("s1", 0.0), ("s2", 60.0)] {
        let sid = p.add_line_sketch(name, vec![Point2::new(x, 0.0), Point2::new(x + 10.0, 0.0), Point2::new(x + 10.0, 10.0), Point2::new(x, 10.0)], true);
        p.add_sketch_node(sid, name);
        bodies.push(p.add_extrude(sid, 5.0));
    }
    let sheet = p.add_drawing_sheet("sheet", SheetFormat::A4Landscape);
    let part = p.components.iter().find(|c| matches!(c.kind, qymcad_core::feature::ComponentKind::Part)).expect("a part").id;
    let base = p.sheet_add_base_view(sheet, part, ViewDir::Front, 1.0, SheetPos { x: 100.0, y: 100.0 }).expect("a view");
    let dim = p.sheet_add_dim(sheet, base, DrawingDimSource::HoleDistance { first: bodies[0], second: bodies[1] }).expect("a hole-distance dimension");
    assert!(p.drawing_dim_is_grounded(sheet, dim), "both bodies stand, so the workbench has geometry to measure");
    assert!(p.drawing_dim_text(sheet, dim).is_none(), "the core stores the pair and measures nothing: the text comes from live geometry at draw time");

    // Fusing the part into one body consumes the first: the pair is no longer measurable.
    p.finish_base_body(bodies[1], 1);
    assert!(!p.drawing_dim_is_grounded(sheet, dim), "the first body is consumed, so the distance has nothing to measure");
}
