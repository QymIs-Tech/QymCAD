//! A BODY CARRIES ITS NAME THROUGH THE FEATURES THAT CHANGE IT. A fillet, a shell, a cut put their result under a new id,
//! and the record of that id is made when the rebuild lands its mesh; to the person the body is the same one, under the
//! name they gave it.
//!
//! Reported behaviour: a body renamed "Housing" read "Body 2" after a fillet, and a shell turned "Body 1" into "Body 2".
use qymcad_core::geom::Mesh;
use qymcad_core::model::{Id, Project};

/// The name of body `id` as the document keeps it.
fn name_of(p: &Project, id: Id) -> String {
    p.mesh_index(id).map(|i| p.mesh_name(i)).unwrap_or_default()
}

/// The body the last node of the timeline gives, its mesh landed as a rebuild lands it.
fn landed(p: &mut Project) -> Id {
    let body = p.timeline.last().and_then(|n| n.kind.body()).expect("the node gives a body");
    p.set_body_mesh(body, Mesh::default());
    body
}

#[test]
fn a_body_carries_its_name_through_the_features_that_change_it() {
    let mut p = Project::default();
    p.new_document();
    p.add_box(20.0, 20.0, 20.0);
    let cube = landed(&mut p);
    let generated = name_of(&p, cube);
    p.add_shell_mode(cube, 2.0, Vec::new(), qymcad_core::feature::ShellSide::Inward);
    let hollow = landed(&mut p);
    let mut bad = Vec::new();
    if name_of(&p, hollow) != generated {
        bad.push(format!("a shell renamed {generated:?} to {:?}", name_of(&p, hollow)));
    }
    let i = p.mesh_index(hollow).expect("the shell's body");
    p.set_mesh_name(i, "Housing".into());
    p.add_fillet(hollow, 2.0, vec![1]);
    let rounded = landed(&mut p);
    if name_of(&p, rounded) != "Housing" {
        bad.push(format!("a fillet renamed \"Housing\" to {:?}", name_of(&p, rounded)));
    }
    // A BODY OF ITS OWN takes a name of its own: the cylinder consumes nothing
    p.add_cylinder(5.0, 10.0);
    let other = landed(&mut p);
    if ["Housing".to_string(), generated.clone()].contains(&name_of(&p, other)) {
        bad.push(format!("a body standing on nothing took the name {:?}", name_of(&p, other)));
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}
