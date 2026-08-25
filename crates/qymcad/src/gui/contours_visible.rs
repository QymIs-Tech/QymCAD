//! SKETCH CONTOURS ARE VISIBLE IN AN ASSEMBLY, NOT ONLY INSIDE A PART.
//!
//! Reported behaviour: no contours appear in 3D however they are clicked. The checkbox had nothing to
//! do with it: the contours were filtered out as "sketches of other components". The rule counted a
//! sketch as its own only when its owner was EXACTLY the current context, and at the root of an
//! assembly the sketches belong to PARTS, so every single one came out foreign.
//!
//! Bodies do not work that way: they use `component_is_within`, which is why bodies of nested parts
//! are visible in an assembly. Sketches now follow the same rule: the context and its descendants are
//! its own, neighbours stay foreign.
#[cfg(test)]
mod tests {
    use super::super::App;

    /// At the root of an assembly the contours of its parts are its own; a neighbour's are foreign.
    #[test]
    fn nested_sketches_are_visible_from_the_assembly_but_siblings_are_not() {
        let mut app = App::default();
        super::super::joint_flow::tests::add_part_at(&mut app, 0.0);
        super::super::joint_flow::tests::add_part_at(&mut app, 100.0);
        app.rebuild_if_dirty();
        assert_eq!(app.project.sketches.len(), 2, "setup: two parts means two sketches");

        // THE ROOT OF THE ASSEMBLY: sketches of nested parts must count as its own
        let root = app.project.root;
        app.enter_component(root);
        assert_eq!(
            app.foreign_contour_ids().len(),
            0,
            "at the root of an assembly the sketches of nested parts are ITS OWN; otherwise the Contours checkbox shows nothing"
        );

        // INSIDE A PART: its own sketch is visible, a neighbour's is not (the isolation is intact)
        let body = app.project.mesh_id(0).expect("the body");
        let owner = app.project.body_owner(body).expect("the owner");
        app.enter_component(owner);
        assert_eq!(
            app.foreign_contour_ids().len(),
            1,
            "inside a part a NEIGHBOURING sketch must stay foreign: the isolation between parts holds"
        );
    }
}
