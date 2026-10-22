//! A CUT SKETCHED ON A DATUM PLANE FOLLOWS THE PARAMETER THAT PLACES THE PLANE. Reported behaviour: a 40 x 40 plate of
//! thickness `t`, a plane from XY at the distance `t` (its top), a 10 x 10 rectangle on it cut 3 mm down; `t` from 6 to
//! 10 - the plate and the plane follow, the floor of the pocket stays at z = 3, deeper inside the thicker plate. No
//! error, no warning.
//!
//! The edit is applied the way the Parameters window applies it (`eval_parameters`, `mark_param_dependents_dirty_for`,
//! a rebuild of what is dirty on the bodies of the cache), on every core but one - the program's default - and on one
//! thread, one after the other in one test: the number of threads is the process's. Live OCCT, headless.
use qymcad_core::feature::{BasePlane, Extent, Purpose, SketchPlane};
use qymcad_core::model::{CombineSpan, Param, PlaneDef, Project, WorkPlane};

/// Where the floor of the pocket stands after `t` goes from 6 to 10, the kernel divided into `threads` (0: every core
/// but one), or what went wrong on the way.
fn floor_after_the_edit(threads: i32) -> Result<Vec<f64>, String> {
    qymcad_kernel::set_parallel(threads != 1, threads);
    let mut p = Project::default();
    p.new_document();
    p.parameters.push(Param { name: "t".into(), expr: "6".into(), value: 6.0 });
    let stock = p.add_box(40.0, 40.0, 6.0);
    p.set_feat_dim(stock, "dz", "t".into());
    let plane = p.add_plane(WorkPlane { origin: [0.0, 0.0, 6.0], def: PlaneDef::OffsetBase { base: BasePlane::XY, dist: 6.0 }, ..Default::default() });
    p.set_feat_dim(plane, "dist", "t".into());
    let si = p.new_sketch("pocket");
    p.sketches[si].plane = SketchPlane::Datum(plane);
    let sid = p.sketches[si].id;
    p.add_sketch_node(sid, "pocket");
    p.add_rect_entity(si, -5.0, -5.0, 5.0, 5.0, Purpose::Real);
    p.regen_sketch(si);
    let contour = p.sketches[si].contour_ids.iter().copied().find(|id| p.contour_profile_xy(*id).is_some()).ok_or("setup: the rectangle closes no contour")?;
    let cut = p.add_combine_multi_op(stock, sid, vec![contour], CombineSpan { height: 0.0, down: 3.0, extent: Extent::default(), fill: &[] }, 0);
    let (report, shapes) = qymcad_testkit::regenerate(&mut p);
    if !report.errors.is_empty() {
        return Err(format!("setup: the plate is not built: {:?}", report.errors));
    }
    let floor = |p: &Project| -> Vec<f64> {
        p.regen_faces.get(&cut).map(|faces| faces.iter().filter(|f| f.normal[2] > 0.99 && (f.area - 100.0).abs() < 1e-6).map(|f| f.centroid.z).collect()).unwrap_or_default()
    };
    if floor(&p) != [3.0] {
        return Err(format!("setup: the floor of the pocket stands at {:?}, not at z = 6 - 3", floor(&p)));
    }
    p.parameters[0].expr = "10".into();
    let refused = p.eval_parameters();
    if !refused.is_empty() {
        return Err(format!("the parameter did not take 10: {refused:?}"));
    }
    p.mark_param_dependents_dirty_for("t");
    let (report, _) = qymcad_testkit::regenerate_dirty_with_shapes(&mut p, shapes);
    if !report.errors.is_empty() {
        return Err(format!("the rebuild after the edit refused: {:?}", report.errors));
    }
    let datum = p.planes.iter().find(|e| e.id == plane).map_or(f64::NAN, |e| e.origin[2]);
    if (datum - 10.0).abs() > 1e-6 {
        return Err(format!("the plane stands at z = {datum}, not at 10"));
    }
    Ok(floor(&p))
}

#[test]
fn a_cut_on_a_datum_follows_the_parameter_on_many_threads_and_on_one() {
    let mut fails = Vec::new();
    for threads in [0, 1] {
        match floor_after_the_edit(threads) {
            Ok(z) if z == [7.0] => {}
            Ok(z) => fails.push(format!("{threads} threads (0: every core but one): the floor of the pocket stands at {z:?}, not at z = 10 - 3")),
            Err(e) => fails.push(format!("{threads} threads: {e}")),
        }
    }
    qymcad_kernel::set_parallel(true, 0);
    assert!(fails.is_empty(), "{}", fails.join("\n"));
}
