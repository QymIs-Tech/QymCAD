//! A PARAMETER EDIT MARKS WHAT COUNTS FROM IT, by the name as it is written.
//!
//! The scheduler compares the values of the parameters with those of the last rebuild and asks the document
//! about every name that changed, by its key in `param_map`. `H` and `h` are two parameters: an edit of `H` raises
//! the extrusion written over `H` and leaves the one written over `h`.
//!
//! Reported behaviour: a parameter H changed from 20 to 40 under an extrusion of height H, the properties said 40,
//! the body stayed 20 high; with the name h the body was rebuilt.
use qymcad_core::geom::Point2;
use qymcad_core::model::{Param, Project};
use qymcad_ui_state::{mark_changed_params_dirty, settle_params_seen};

/// An extrusion of a 40 x 30 profile whose height is the formula `formula`.
fn extrusion(p: &mut Project, formula: &str) -> u64 {
    let sid = p.add_line_sketch("Profile", vec![Point2::new(0.0, 0.0), Point2::new(40.0, 0.0), Point2::new(40.0, 30.0), Point2::new(0.0, 30.0)], true);
    p.add_sketch_node(sid, "Profile");
    let node = p.add_extrude_on(sid, 0, 20.0, qymcad_core::feature::Reach::Forward, 0.0);
    p.set_feat_dim(node, "height", formula.into());
    node
}

#[test]
fn an_edit_of_a_parameter_in_capitals_marks_its_extrusion_alone() {
    let mut p = Project::default();
    p.new_document();
    p.parameters.push(Param { name: "H".into(), expr: "20".into(), value: 20.0 });
    p.parameters.push(Param { name: "h".into(), expr: "30".into(), value: 30.0 });
    let over_capital = extrusion(&mut p, "H");
    let over_small = extrusion(&mut p, "h");
    let mut seen = std::collections::HashMap::new();
    settle_params_seen(&mut seen, &mut p);
    for n in p.timeline.iter_mut() {
        n.dirty = false;
    }

    if let Some(prm) = p.parameters.iter_mut().find(|q| q.name == "H") {
        prm.expr = "40".into();
        prm.value = 40.0;
    }
    mark_changed_params_dirty(&seen, &mut p);
    let dirty = |node: u64| p.timeline.iter().find(|n| n.id == node).is_some_and(|n| n.dirty);
    assert!(dirty(over_capital), "the parameter H was made 40 and the extrusion of height H is not marked to rebuild");
    assert!(!dirty(over_small), "the parameter H was made 40 and the extrusion of height h is marked to rebuild");
}

/// A HINGE WHOSE ANGLE IS A FORMULA COUNTS FROM ITS PARAMETER: a mate is no node to mark, so the scheduler is told the
/// placement has to be solved again - by the edit of that parameter and by no other. Reported behaviour: pa made 45
/// under a hinge of angle pa, and the part stayed turned by 90.
#[test]
fn an_edit_of_the_parameter_of_a_hinge_asks_for_the_placement() {
    use qymcad_core::feature::{AnchorRef, JointKind};
    let mut p = Project::default();
    p.new_document();
    p.parameters.push(Param { name: "pa".into(), expr: "90".into(), value: 90.0 });
    p.parameters.push(Param { name: "h".into(), expr: "30".into(), value: 30.0 });
    let hub = p.add_part("hub");
    let wheel = p.add_part("wheel");
    let a = p.add_connector(hub, AnchorRef::Origin);
    let b = p.add_connector(wheel, AnchorRef::Origin);
    let hinge = p.add_joint(a, b, JointKind::Revolute);
    p.set_feat_dim(hinge, "angle", "pa".into());
    let mut seen = std::collections::HashMap::new();
    settle_params_seen(&mut seen, &mut p);

    let set = |p: &mut Project, name: &str, v: f64| {
        if let Some(prm) = p.parameters.iter_mut().find(|q| q.name == name) {
            prm.expr = v.to_string();
            prm.value = v;
        }
    };
    set(&mut p, "h", 40.0);
    assert!(!mark_changed_params_dirty(&seen, &mut p), "h, which no mate counts from, was made 40 and the placement is asked for");
    set(&mut p, "pa", 45.0);
    assert!(mark_changed_params_dirty(&seen, &mut p), "pa was made 45 under a hinge of angle pa and the placement is not asked for");
}
