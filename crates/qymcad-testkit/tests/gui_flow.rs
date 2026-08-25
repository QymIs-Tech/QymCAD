//! The exact path through the interface: a new document, a new part, a sketch whose node appears at once and
//! whose owner is that part, a rectangle drawn with the tool, finishing the sketch, then extruding into the
//! empty part.
use qymcad_core::model::Project;
#[test]
fn gui_flow_new_part_rect_extrude() {
    let mut p = Project::default();
    p.new_document(); // the first part is active, as it is at start-up
    let part = p.add_part(format!("Part {}", p.components.len()));
    p.set_active_component(Some(part)); // enter_component
    let si = p.new_sketch(format!("Sketch {}", p.sketches.len() + 1));
    let sid = p.sketches[si].id;
    p.add_sketch_node(sid, "Sketch"); // as `create_sketch_on` does
    p.add_rect_entity(si, 0.0, 0.0, 30.0, 30.0, qymcad_core::feature::Purpose::Real); // the rectangle tool
    p.regen_sketch(si); // finish_sketch_edit
    let closed: Vec<u64> = p.sketches[si].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect();
    eprintln!("closed: {} {:?}, owner of the sketch: {:?}, active: {:?}", closed.len(), closed, p.sketch_owner(sid), p.active_component);
    assert!(!closed.is_empty(), "the rectangle gave a closed contour");
    // apply_sketch_cmd: owner → active; part == active_body(ctx)
    if let Some(o) = p.sketch_owner(sid) { p.set_active_component(Some(o)); }
    let ctx = p.current_ctx();
    let cur = p.active_body(ctx);
    eprintln!("ctx={ctx} current_body={cur:?}");
    let body = match cur {
        None => { // an empty part: extrude, then `finish_base_body`
            let e = p.add_extrude_multi(sid, closed.clone(), 10.0, qymcad_core::feature::Reach::Forward, 0.0, vec![]);
            assert!(e != 0, "`add_extrude_multi` returned a node");
            p.finish_base_body(e, 1)
        }
        Some(b) => p.add_combine_multi_op(b, sid, closed.clone(), 10.0, 1, qymcad_core::feature::Extent::default(), 0.0, vec![]),
    };
    let (report, shapes) = qymcad_testkit::regenerate(&mut p);
    for (id,e) in &report.errors { eprintln!("error {id}: {e}"); }
    eprintln!("the body was built: {} (body={body})", shapes.contains_key(&body));
    assert!(shapes.contains_key(&body), "an extrusion in a new, empty part builds a body");
    eprintln!("V={:.1}", shapes.get(&body).unwrap().volume());
}
