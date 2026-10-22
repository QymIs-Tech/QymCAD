use super::*;

/// A NESTED ASSEMBLY: AN ASSEMBLY INSIDE AN ASSEMBLY.
///
/// A node inside a node is where the document broke twice, and both times it showed: bodies
/// surfaced into the ROOT, or a part ended up inside a part. Here a person makes a subassembly,
/// enters it, builds a part there and leaves - and everything built must stay INSIDE.
pub(super) fn a_nested_assembly(app: &mut App, problems: &mut Vec<String>) {
    let root = app.project.root;
    let bodies_in_root = |app: &App| -> usize { app.project.timeline.iter().filter(|n| n.kind.bodies().iter().any(|b| n.owner_of(*b) == Some(app.project.root))).count() };
    let root_bodies_before = bodies_in_root(app);

    app.set_context_to(root);
    let sub = app.project.add_assembly("Subassembly");
    app.enter_component(sub);
    if qymcad_ui_state::current_ctx_id(&app.active_path, &app.project) != sub {
        problems.push("entering the subassembly did not change the context".into());
    }
    if app.project.components.iter().find(|c| c.id == sub).and_then(|c| c.parent) != Some(root) {
        problems.push("the subassembly was created in a different assembly than the one it was called from".into());
    }

    // build A PART inside it rather than a body: bodies live only in Parts, a subassembly
    // does not hold them. The first edition extruded straight into the subassembly and got a
    // silent refusal, because that is not allowed. The mistake was in the test, but the
    // program's SILENCE is the program's own.
    let inner = app.project.add_part("Part in the subassembly");
    app.enter_component(inner);
    let sk = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
    app.project.add_rect_entity(sk, 0.0, 0.0, 12.0, 12.0, qymcad_core::feature::Purpose::Real);
    app.project.solve_sketch(sk);
    app.project.regen_sketch(sk);
    let sid = app.project.sketches[sk].id;
    app.project.add_sketch_node(sid, "subassembly sketch");
    let body = app.project.add_extrude(sid, 8.0);
    qymcad_ui_state::rebuild_if_dirty(&mut app.rebuild_ctx());

    // EVERYTHING BUILT STAYED INSIDE: neither the node nor the body surfaced into the root
    let owner = app.project.body_owner(body);
    let inside = owner.is_some_and(|o| {
        let mut cur = Some(o);
        while let Some(c) = cur {
            if c == sub {
                return true;
            }
            cur = app.project.components.iter().find(|x| x.id == c).and_then(|x| x.parent);
        }
        false
    });
    if !inside {
        problems.push(format!("the body built in the subassembly ended up outside it: owner {owner:?}"));
    }
    if bodies_in_root(app) != root_bodies_before {
        problems.push(format!("after working in the subassembly the ROOT gained builds: there were {root_bodies_before}, now {}", bodies_in_root(app)));
    }

    // go back up - the context must return and the subassembly must stay in the tree
    app.set_context_to(root);
    if qymcad_ui_state::current_ctx_id(&app.active_path, &app.project) != root {
        problems.push("leaving the subassembly did not return the context to the root".into());
    }
    if !app.project.components.iter().any(|c| c.id == sub) {
        problems.push("the subassembly vanished from the document after leaving it".into());
    }
    check_all(app, "a nested assembly", problems);
}

