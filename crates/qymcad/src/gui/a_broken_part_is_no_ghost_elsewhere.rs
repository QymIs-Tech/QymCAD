//! A PART WHOSE LAST OPERATION FAILED IS SHOWN WHERE A PART IS SHOWN, AND ONLY THERE.
//!
//! Reported behaviour: with "In context" off, bodies still showed as ghosts; a part with a broken feature was seen as
//! a ghost from every other component, even with its tick off in the tree.
//!
//! When the node that consumed a body fails, the body before it stays on screen - otherwise the failure wipes the
//! part out. That exception answered "shown" before any other rule was asked: the body's own tick, its component's,
//! the context, the in-context switch. And outside the context a body is drawn as a ghost.
#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use super::super::App;
    use qymcad_core::feature::FeatureKind;

    /// Two parts, the first with a fillet that failed to build. Returns the first part's body before the fillet,
    /// that part and the second one.
    fn a_broken_part(app: &mut App) -> (u64, u64, u64) {
        super::super::joint_flow::tests::add_part_at(app, 0.0);
        qymcad_ui_state::rebuild_if_dirty(&mut app.rebuild_ctx());
        let body = app.project.mesh_id(0).expect("the body");
        let part = app.project.body_owner(body).expect("its part");
        app.enter_component(part);
        let edge = app.project.regen_edges[&body].iter().filter(|e| (e.a[2] - e.b[2]).abs() < 1e-6).max_by(|x, y| x.mid[2].total_cmp(&y.mid[2])).cloned().expect("the top edge");
        Hand::new(app).look_at([10.0, 10.0, 5.0], 9.0).tool(qymcad_ui_state::PartTool::Fillet).click(edge.mid).enter();
        let fillet = app.project.timeline.iter().find(|n| matches!(n.kind, FeatureKind::Fillet { .. })).map(|n| n.id).expect("a fillet was made");
        app.exit_context();
        super::super::joint_flow::tests::add_part_at(app, 60.0);
        qymcad_ui_state::rebuild_if_dirty(&mut app.rebuild_ctx());
        app.exit_context(); // the helper steps into the part it makes; the checks start at the top
        let other = app.project.components.iter().rev().find(|c| c.kind == qymcad_core::feature::ComponentKind::Part && c.id != part).map(|c| c.id).expect("the second part");
        // THE FILLET FAILS, as one does when an edit above it takes its edge away; last, so no rebuild clears it
        app.project.regen_errors.insert(fillet, qymcad_core::errors::CoreError::SourceBodyNotBuilt);
        (body, part, other)
    }

    fn shown(app: &App, body: u64) -> bool {
        let mi = app.project.mesh_index(body).expect("the body");
        qymcad_ui_state::body_shown(app.painting().body_view(), mi)
    }

    fn ghost(app: &App, body: u64) -> bool {
        let mi = app.project.mesh_index(body).expect("the body");
        qymcad_ui_state::visible_mesh_items(&app.painting()).iter().any(|m| m.index == mi && m.ghost)
    }

    #[test]
    fn a_broken_part_is_not_seen_from_inside_another_with_in_context_off() {
        let mut app = App::default();
        let (body, _, other) = a_broken_part(&mut app);
        assert!(shown(&app, body), "at the top, the part before its failed fillet must stay on screen");
        app.enter_component(other);
        app.win.context = false;
        assert!(!shown(&app, body), "inside another part with In context off, the broken part is still drawn (a ghost: {})", ghost(&app, body));
    }

    #[test]
    fn a_hidden_broken_part_stays_hidden() {
        let mut app = App::default();
        let (body, part, _) = a_broken_part(&mut app);
        assert!(app.project.set_component_visible(part, false), "the part's tick");
        assert!(!shown(&app, body), "its tick is off, and the broken part is still drawn");
    }

    #[test]
    fn with_in_context_on_a_broken_neighbour_is_a_ghost_like_any_other() {
        let mut app = App::default();
        let (body, _, other) = a_broken_part(&mut app);
        app.enter_component(other);
        app.win.context = true;
        assert!(shown(&app, body) && ghost(&app, body), "in context, a neighbour - broken or not - is a ghost to refer to");
    }
}
