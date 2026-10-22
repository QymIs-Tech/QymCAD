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

    a_link_leads_to_the_place_in_the_tree(app, body, problems);

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

/// A LINK LEADS TO THE PLACE IN THE TREE AND "FINISH" CLIMBS IT: the height of `body` named in the parameters, the
/// person at the root clicks the path of that driver in the window of parameters and lands in the part of the body -
/// the path above the canvas its chain of ancestors - and "Finish" pressed once per level comes back to the root.
/// Reported behaviour: each jump by a link was appended to the path it started from, and "Finish" walked back along the
/// jumps.
fn a_link_leads_to_the_place_in_the_tree(app: &mut App, body: qymcad_core::model::Id, problems: &mut Vec<String>) {
    let step = "a link of a driver into the part of a subassembly";
    let node = app.project.timeline.iter().find(|n| n.kind.body() == Some(body)).map(|n| n.id);
    let Some(part) = app.project.body_owner(body).filter(|_| node.is_some()) else {
        problems.push(format!("[{step}] the body built in the subassembly has no node or no part"));
        return;
    };
    let node = node.unwrap_or_default();
    let root = app.project.root;
    let driver = "inner_h".to_string();
    app.project.add_named_feat_dim(driver.clone(), node, "height");
    app.set_context_to(root);
    let mut hand = Hand::new(app);
    hand.press_word(&crate::i18n::tr("wb-params"), egui::pos2(0.0, 0.0));
    let path = hand.app.project.named_dims.iter().find(|d| d.name == driver).map(|d| qymcad_ui_state::path_in_words(&hand.app.project.driver_path(&d.target))).unwrap_or_default();
    hand.frame(Vec::new());
    let near = hand.window_at("win_params").map_or(egui::pos2(0.0, 0.0), |w| w.center());
    if !hand.press_word(&path, near) {
        problems.push(format!("[{step}] the window of parameters shows no path {path:?}"));
    }
    let tree = qymcad_ui_state::context_path_to(&hand.app.project, part);
    if hand.app.active_path != tree {
        problems.push(format!("[{step}] the path is {:?}, the place of the part in the tree is {tree:?}", hand.app.active_path));
    }
    hand.close_titled(&crate::i18n::tr("win-params"));
    for _ in 1..tree.len() {
        hand.press_word(&crate::i18n::tr("wb-finish"), egui::pos2(0.0, 0.0));
    }
    hand.close_window();
    if hand.app.active_path != vec![root] {
        problems.push(format!("[{step}] \"Finish\" pressed {} times left the path {:?}, not the root", tree.len() - 1, hand.app.active_path));
    }
    check_all(app, step, problems);
}
