//! A CUT "THROUGH ALL" KEEPS ITS DIRECTION. Reported behaviour: the direction of a cut is stored, but "through all" cut
//! the body both ways whatever it held; a sketch lying inside a part, cut one way, took metal off the other side too.
//! On an end face the difference does not show: the body lies on one side of it.
//!
//! A block 40 x 40 x 20, a plane through its middle (z = 10), a square 10 x 10 on it cut through all: along the normal
//! the cut takes the 10 x 10 x 10 above the plane, against it the one below, both ways - as "symmetric" says - both.
//! Live OCCT, headless.
use qymcad_core::feature::{BasePlane, Extent, Purpose, Reach, SketchPlane};
use qymcad_core::model::{CombineSpan, PlaneDef, Project, WorkPlane};

/// WHAT THE CUT LEFT: the volume the block keeps, and which way the face 10 x 10 the cut leaves on the plane looks along
/// Z (up for the floor of a cut above it, down for the ceiling of a cut below it, none for a cut both ways).
struct Left {
    volume: f64,
    floor_faces: Option<f64>,
}

/// What the block keeps after the square is cut through all in the direction `reach`.
fn kept_after(reach: Reach) -> Result<Left, String> {
    let mut p = Project::default();
    p.new_document();
    let block = p.add_box(40.0, 40.0, 20.0);
    let plane = p.add_plane(WorkPlane { origin: [0.0, 0.0, 10.0], def: PlaneDef::OffsetBase { base: BasePlane::XY, dist: 10.0 }, ..Default::default() });
    let si = p.new_sketch("square");
    p.sketches[si].plane = SketchPlane::Datum(plane);
    let sid = p.sketches[si].id;
    p.add_sketch_node(sid, "square");
    p.add_rect_entity(si, -5.0, -5.0, 5.0, 5.0, Purpose::Real);
    p.regen_sketch(si);
    let contour = p.sketches[si].contour_ids.iter().copied().find(|id| p.contour_profile_xy(*id).is_some()).ok_or("setup: the square closes no contour")?;
    let cut = p.add_combine_multi_op(block, sid, vec![contour], CombineSpan { height: 10.0, down: 0.0, extent: Extent { through: true, reach }, fill: &[] }, 0);
    let (report, shapes) = qymcad_testkit::regenerate(&mut p);
    if let Some((_, e)) = report.errors.iter().find(|(n, _)| *n == cut) {
        return Err(format!("the cut refused: {e:?}"));
    }
    if !report.errors.is_empty() {
        return Err(format!("setup: the block is not built: {:?}", report.errors));
    }
    let volume = shapes.get(&cut).map(|s| s.volume()).ok_or("the cut left no body")?;
    // the end of a cut one way on the sketch plane is set back by a micrometre from the plane (`meet_ends`)
    let floor_faces = p.regen_faces.get(&cut).and_then(|faces| faces.iter().find(|f| (f.centroid.z - 10.0).abs() < 1e-2 && (f.area - 100.0).abs() < 1e-6).map(|f| f.normal[2]));
    Ok(Left { volume, floor_faces })
}

/// A DIRECTION AND WHAT A CUT THROUGH ALL IN IT TAKES: the volume, and which way its face on the plane looks along Z.
struct Case {
    reach: Reach,
    taken: f64,
    faces: Option<f64>,
}

#[test]
fn a_cut_through_all_takes_metal_only_on_its_side() {
    let whole = 40.0 * 40.0 * 20.0;
    let cases = [
        Case { reach: Reach::Forward, taken: 1000.0, faces: Some(1.0) },
        Case { reach: Reach::Backward, taken: 1000.0, faces: Some(-1.0) },
        Case { reach: Reach::BothWays, taken: 2000.0, faces: None },
    ];
    let mut fails = Vec::new();
    for Case { reach, taken, faces } in cases {
        match kept_after(reach) {
            // a micrometre set back on 100 mm^2 is 0.1 mm^3; a cut both ways would take 1000 more
            Ok(left) if (whole - left.volume - taken).abs() < 0.2 && left.floor_faces.map(f64::signum) == faces => {}
            Ok(left) => fails.push(format!("{reach:?}: the cut took {:.3} mm^3 (not {taken}), its face on the plane looks {:?} along Z (not {faces:?})", whole - left.volume, left.floor_faces)),
            Err(e) => fails.push(format!("{reach:?}: {e}")),
        }
    }
    assert!(fails.is_empty(), "{}", fails.join("\n"));
}
