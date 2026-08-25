//! SUPPRESSING A FEATURE REBUILDS EVERYTHING BUILT ON IT.
//!
//! Reported behaviour: pressing Suppress on a feature made the CAD flicker with a rebuild window;
//! switching it back on changed nothing; only Edit -> Rebuild Everything helped.
//!
//! The cause was exactly one line: ONE node was marked dirty. It rebuilt, while the features standing
//! on its body kept the previous shape — that is, the model on screen stopped being the result of its
//! own timeline. A history rollback has long marked everything; suppression is the same kind of
//! structural edit to the timeline.
#[cfg(test)]
mod tests {
    use qymcad_core::geom::Point2;
    use qymcad_core::model::Project;

    /// Plate then fillet: a linear chain where the second stands on the first.
    fn chain() -> (Project, usize, usize) {
        let mut p = Project::default();
        p.new_document();
        let sid = p.add_line_sketch(
            "sq",
            vec![Point2::new(0.0, 0.0), Point2::new(30.0, 0.0), Point2::new(30.0, 30.0), Point2::new(0.0, 30.0)],
            true,
        );
        let si = p.sketch_index(sid).unwrap();
        p.regen_sketch(si);
        if let Some(o) = p.sketch_owner(sid) {
            p.set_active_component(Some(o));
        }
        p.add_sketch_node(sid, "sq");
        let closed: Vec<u64> = p.sketches[si].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect();
        let base = p.add_extrude_multi(sid, closed, 10.0, qymcad_core::feature::Reach::Forward, 0.0, vec![]);
        let fillet = p.add_fillet(base, 2.0, vec![]);
        let (bi, fi) = (
            p.timeline.iter().position(|n| n.kind.body() == Some(base)).expect("the extrude is in the timeline"),
            p.timeline.iter().position(|n| n.kind.body() == Some(fillet)).expect("the fillet is in the timeline"),
        );
        for n in &mut p.timeline {
            n.dirty = false;
        }
        (p, bi, fi)
    }

    /// SUPPRESS THE LOWER FEATURE AND EVERYTHING ON IT BECOMES DIRTY.
    #[test]
    fn suppressing_a_feature_dirties_everything_built_on_it() {
        let (mut p, base, fillet) = chain();
        p.set_feature_suppressed(base, true);
        assert!(p.timeline[base].dirty, "the node itself must be dirty");
        assert!(p.timeline[fillet].dirty, "the fillet stands on that body, so it must rebuild");
    }

    /// AND SWITCHING IT BACK ON DOES THE SAME. Exactly the reported "nothing changed".
    #[test]
    fn un_suppressing_dirties_the_dependents_as_well() {
        let (mut p, base, fillet) = chain();
        p.set_feature_suppressed(base, true);
        for n in &mut p.timeline {
            n.dirty = false;
        }
        p.set_feature_suppressed(base, false);
        assert!(p.timeline[fillet].dirty, "it was switched back on and the dependent kept the previous shape: that is the reported behaviour");
    }

    /// AND NOTHING ABOVE THE NODE IN THE TIMELINE BECOMES DIRTY.
    ///
    /// Marking everything would be simpler, but the timeline is ordered: what stands above cannot
    /// stand on the suppressed feature by construction. A surplus rebuild on a large part is seconds
    /// wasted.
    #[test]
    fn nothing_above_the_node_is_touched() {
        let (mut p, base, fillet) = chain();
        p.set_feature_suppressed(fillet, true);
        assert!(!p.timeline[base].dirty, "the extrude stands ABOVE the fillet and does not have to rebuild");
    }
}
