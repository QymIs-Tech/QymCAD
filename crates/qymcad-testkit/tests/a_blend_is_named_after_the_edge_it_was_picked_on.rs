//! A BLEND IS NAMED AFTER THE EDGE IT WAS PICKED ON, as the node holds the pick: an edit above that renames the edge -
//! a face of its pair merged into another - but moves neither the edge nor the blend leaves the surface of the blend
//! its name. Reported behaviour: on the case of the examples a dimension of a sketch by +10% renamed eight faces of a
//! chamfer that did not move, and everything standing on them with them.
use qymcad_core::model::{DimTarget, Id, Project};

/// A FACE OF A BLEND by its name and where it stands.
struct Stand {
    name: String,
    at: [f64; 3],
    area: f64,
}

/// The faces node `node` made on its own body - a blend's body is the node - by where they stand.
fn blends(p: &Project, node: Id) -> Vec<Stand> {
    p.regen_faces[&node]
        .iter()
        .filter(|f| p.names.get(f.id).is_some_and(|n| n.feature == node))
        .map(|f| Stand { name: p.names.describe(f.id), at: [f.centroid.x, f.centroid.y, f.centroid.z], area: f.area })
        .collect()
}

/// AN EDIT OF A DIMENSION ON AN EXAMPLE: the faces of blend `node` that did not move, renamed. The example `name` is
/// opened, the dimension `refs` of sketch `sketch` set to `value`, and what was renamed is told.
struct DimensionEdit {
    name: &'static str,
    sketch: u64,
    refs: &'static [u64],
    value: f64,
    node: Id,
}

fn renamed_by(edit: DimensionEdit) -> Vec<String> {
    let path = format!("{}/../../examples/{}", env!("CARGO_MANIFEST_DIR"), edit.name);
    let mut p = qymcad_io::load_project(&path).expect("the example opens");
    let shapes = qymcad_testkit::restore_import_shapes(&p);
    let (_, shapes) = qymcad_testkit::regenerate_with_shapes(&mut p, shapes);
    let before = blends(&p, edit.node);
    assert!(!before.is_empty(), "GUARD: the blend {} of {} has faces of its own", edit.node, edit.name);
    p.set_dim_target_value(&DimTarget::Sketch { sketch: edit.sketch, refs: edit.refs.to_vec() }, edit.value);
    let si = p.sketch_index(edit.sketch).expect("the sketch");
    p.solve_sketch(si);
    p.mark_sketch_dirty(edit.sketch);
    let _ = qymcad_testkit::regenerate_dirty_with_shapes(&mut p, shapes);
    let after = blends(&p, edit.node);
    let still = |a: &Stand, b: &Stand| (0..3).all(|k| (a.at[k] - b.at[k]).abs() < 1e-6) && (a.area - b.area).abs() < 1e-6 * a.area.max(1.0);
    after.iter().filter_map(|a| before.iter().find(|b| still(a, b)).filter(|b| b.name != a.name).map(|b| format!("{} -> {}", b.name, a.name))).collect()
}

#[test]
fn the_faces_of_a_chamfer_that_did_not_move_keep_their_names() {
    let renamed = renamed_by(DimensionEdit { name: "QymBoxPc-Case.qcad", sketch: 5182, refs: &[5328, 5329], value: 68.2, node: 5341 });
    assert!(renamed.is_empty(), "faces of the chamfer that did not move were renamed:\n  {}", renamed.join("\n  "));
}

/// TWO PICKS ON ONE EDGE: on the filter of the examples a dimension by -10% merges walls of one cut into the walls of
/// another, and the edge a rounding picked first is translated onto the edge it picked thirtieth. The surface on that
/// edge, which did not move, keeps the name of the pick that holds the edge by its name.
#[test]
fn a_rounding_of_two_picks_on_one_edge_keeps_the_name_of_the_pick_that_holds_it() {
    let renamed = renamed_by(DimensionEdit { name: "Filter-v2.qcad", sketch: 28, refs: &[32], value: 30.6, node: 269 });
    assert!(renamed.is_empty(), "faces of the rounding that did not move were renamed:\n  {}", renamed.join("\n  "));
}
