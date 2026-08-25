//! THE INTERFERENCE CHECK BETWEEN PARTS.
//!
//! The checkbox was dead: the computation only produced anything in the Assembly workbench, while it
//! was called from a single place, the drawing of the FLAT view. Those conditions are incompatible,
//! since assemblies are built in 3D. The logic inside was real (the volume of the common part through
//! the kernel), so it was not thrown away but wired up.
#[cfg(test)]
mod tests {
    use super::super::{App, Workbench};

    /// Two parts that certainly overlap are found; separated ones are not.
    #[test]
    fn overlapping_parts_are_found_and_separated_ones_are_not() {
        let mut app = App::default();
        super::super::joint_flow::tests::add_part_at(&mut app, 0.0);
        super::super::joint_flow::tests::add_part_at(&mut app, 5.0); // overlaps the first one (width 20)
        let root = app.project.root;
        app.enter_component(root);
        app.workbench = Workbench::Assembly;
        app.rebuild_if_dirty();
        app.set.show_interference = true;
        app.interference.rev = u64::MAX; // the cache is stale, so compute
        app.refresh_interference();
        assert!(
            !app.interference.pairs.is_empty(),
            "the parts overlap and no interference was found: the checkbox does nothing again"
        );

        // separate them: the second part moves far away
        let bodies: Vec<_> = (0..app.project.bodies.len()).filter_map(|mi| app.project.mesh_id(mi)).collect();
        let owner = app.project.body_owner(bodies[1]).expect("the owner");
        app.project.set_component_transform(owner, [1.0, 0.0, 0.0, 500.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0]);
        app.interference.rev = u64::MAX;
        app.refresh_interference();
        assert!(app.interference.pairs.is_empty(), "separated parts do not overlap, yet a pair is still listed");
    }

    /// A switched-off checkbox does not start the expensive computation.
    #[test]
    fn the_check_costs_nothing_while_switched_off() {
        let mut app = App::default();
        super::super::joint_flow::tests::add_part_at(&mut app, 0.0);
        super::super::joint_flow::tests::add_part_at(&mut app, 5.0);
        let root = app.project.root;
        app.enter_component(root);
        app.workbench = Workbench::Assembly;
        app.rebuild_if_dirty();
        app.set.show_interference = false;
        app.interference.rev = u64::MAX;
        app.refresh_interference();
        assert!(app.interference.pairs.is_empty(), "with the checkbox off the computation must not run");
    }
}
