//! GEOMETRY DRAWN AT ZERO MUST STAY THE PERSON'S GEOMETRY, and not become the origin itself.
//!
//! Reported behaviour: "I cannot move a circle away from the origin with the move tool". The centre of a
//! circle drawn at (0,0) was the origin point: `merge_close_points` glues whatever lands at zero INTO the
//! origin and keeps the origin's id, and `immovable_points` holds the system points - so the drawing was
//! nailed down by the frame of reference it merely touched.
//!
//! The repair that used to cover this, `detach_geometry_from_origin`, only fires once the origin has
//! already wandered off zero; `pin_frame` now keeps it at zero, so it never fires.
use qymcad_core::model::{SketchAxis, Project};

fn new_sketch() -> (Project, usize) {
    let mut p = Project::default();
    p.new_document();
    let sid = p.add_sketch("t", vec![], None);
    p.add_sketch_node(sid, "Sketch");
    let si = p.sketch_index(sid).unwrap();
    (p, si)
}

/// The endpoints of the first line of the sketch.
fn line_points(p: &Project, si: usize) -> (u64, u64) {
    p.sketches[si]
        .entities
        .iter()
        .find_map(|e| match e.kind.as_line() {
            Some(qymcad_core::model::LineParts { a, b }) => Some((a, b)),
            _ => None,
        })
        .expect("the sketch holds a line")
}

#[test]
fn geometry_drawn_at_the_origin_can_still_be_moved() {
    let (mut p, si) = new_sketch();
    // The frame of reference exists, as it does in any sketch a person has touched: a dimension or a
    // constraint against an axis creates it.
    let origin = p.ensure_origin(si);
    let _ = p.ensure_axis(si, SketchAxis::X);
    let _ = p.ensure_axis(si, SketchAxis::Y);

    // A CIRCLE IS ALREADY SAFE: `merge_close_points` refuses to glue the centre of a radius curve, so it
    // keeps a point of its own - and a line started from that centre inherits the protection. A line drawn
    // from zero on its own has none, which is why nothing else is drawn here.
    let _el = p.add_line_entity(si, 0.0, 0.0, 30.0, 0.0, qymcad_core::feature::Purpose::Real);
    // The drawing tools stitch coincident points afterwards; at the default zoom the tolerance reaches 2.0.
    p.merge_close_points(si, 2.0);
    p.regen_sketch(si);

    let (a, b) = line_points(&p, si);
    let sys = p.sketches[si].system_ids();
    let immovable = p.sketches[si].immovable_points();
    eprintln!("origin={origin} line ends={a},{b} system={sys:?}");

    assert!(!sys.contains(&a) && !sys.contains(&b), "an end of the line became a system point of the frame");
    assert!(!immovable.contains(&a) && !immovable.contains(&b), "an end of the line cannot be dragged: it is held as immovable");
    assert!(p.sketches[si].points.iter().any(|q| q.id == origin), "the origin itself has to survive");
    let o = p.sketches[si].points.iter().find(|q| q.id == origin).unwrap();
    assert_eq!((o.x, o.y), (0.0, 0.0), "the origin stays at zero");
}

/// A DOCUMENT SAVED BEFORE THIS RULE IS REPAIRED WHEN IT OPENS.
///
/// Files already exist in which a line, drawn from zero, holds the origin's own id as its end - that is how
/// the merging used to work. Such a corner cannot be dragged and the shape around it cannot be moved. The
/// repair gives the drawing a point of its own, in the same place, and leaves the origin to the frame.
#[test]
fn a_saved_sketch_whose_line_ends_on_the_origin_is_repaired_on_rebuild() {
    let (mut p, si) = new_sketch();
    let origin = p.ensure_origin(si);
    let _ = p.ensure_axis(si, SketchAxis::X);
    let _ = p.ensure_axis(si, SketchAxis::Y);

    // The state such a file is in: the far end is the line's own point, the near end IS the origin.
    let far = p.alloc_id();
    let eid = p.alloc_id();
    let s = &mut p.sketches[si];
    s.points.push(qymcad_core::model::SketchPoint { id: far, x: 30.0, y: 0.0 });
    s.entities.push(qymcad_core::model::SketchEntity { id: eid, kind: qymcad_core::model::LineParts { a: origin, b: far }.into(), construction: false });

    p.regen_sketch(si);

    let (a, b) = line_points(&p, si);
    let sys = p.sketches[si].system_ids();
    eprintln!("origin={origin} line ends={a},{b} system={sys:?}");
    assert!(!sys.contains(&a) && !sys.contains(&b), "the repair left the line standing on the frame");

    // The drawing does not move: the freed end stays where it was.
    let pt = |id: u64| p.sketches[si].points.iter().find(|q| q.id == id).map(|q| (q.x, q.y)).expect("the point exists");
    assert_eq!(pt(a), (0.0, 0.0), "the line still starts at zero");
    assert_eq!(pt(b), (30.0, 0.0), "the far end is untouched");
    assert_eq!(pt(origin), (0.0, 0.0), "the origin stays at zero");
}

/// THE WHOLE CLASS AT ONCE, one tool per row.
///
/// The line was the tool the defect was found through, and a fix aimed at one tool is a fix that comes back
/// through the next one. Every tool that can be started at zero is drawn there, stitched the way the drawing
/// tools stitch (the tolerance reaches 2.0 at the ordinary zoom) and rebuilt, and then two things have to
/// hold: not one point of the drawing is a point of the frame, and the origin is still at zero.
///
/// The failures are collected and reported together - one run says which tools are broken, rather than the
/// first of them.
#[test]
fn no_tool_started_at_zero_welds_its_drawing_to_the_frame() {
    let mut sins: Vec<String> = Vec::new();
    for name in ["line", "rectangle", "circle", "arc"] {
        let (mut p, si) = new_sketch();
        let origin = p.ensure_origin(si);
        let _ = p.ensure_axis(si, SketchAxis::X);
        let _ = p.ensure_axis(si, SketchAxis::Y);
        match name {
            "line" => {
                p.add_line_entity(si, 0.0, 0.0, 30.0, 0.0, qymcad_core::feature::Purpose::Real);
            }
            "rectangle" => {
                p.add_rect_entity(si, 0.0, 0.0, 30.0, 20.0, qymcad_core::feature::Purpose::Real);
            }
            "circle" => {
                p.add_circle_entity(si, 0.0, 0.0, 10.0, qymcad_core::feature::Purpose::Real);
            }
            "arc" => p.add_arc_entity(
                si,
                qymcad_core::geom::Point2::new(0.0, 0.0),
                qymcad_core::geom::Point2::new(10.0, 0.0),
                qymcad_core::geom::Point2::new(0.0, 10.0),
                qymcad_core::feature::Winding::Ccw,
                qymcad_core::feature::Purpose::Real,
            ),
            _ => unreachable!(),
        }
        p.merge_close_points(si, 2.0);
        p.regen_sketch(si);

        let sys = p.sketches[si].system_ids();
        let mine: Vec<u64> = p.sketches[si].entities.iter().flat_map(qymcad_core::model::entity_points).collect();
        for id in &mine {
            if sys.contains(id) {
                sins.push(format!("{name}: the drawing stands on the point {id} of the frame - it cannot be dragged"));
            }
        }
        match p.sketches[si].points.iter().find(|q| q.id == origin) {
            Some(o) if o.x == 0.0 && o.y == 0.0 => {}
            Some(o) => sins.push(format!("{name}: the origin wandered to ({}, {})", o.x, o.y)),
            None => sins.push(format!("{name}: the origin is gone")),
        }
    }
    assert!(sins.is_empty(), "geometry welded to the frame of reference:\n{}", sins.join("\n"));
}
