//! ONE COMMAND MEANS ONE TIMELINE NODE. Written from a report.
//!
//! Reported behaviour: two contours were picked into one revolve and cut with it from the part. Then a
//! sketch earlier in the history was changed — and the revolve feature fell apart into 2 features that
//! ADD instead of CUT, and broke: they cannot be edited, only deleted, and deleting them deleted every
//! feature below.
//!
//! IT DID NOT FALL APART FROM THE EDIT — IT WAS CREATED THAT WAY. The revolve command ran a loop over
//! the contours: a `Revolve` node of its own for every contour (and that is a NEW body, that is, ADD)
//! plus a `BodyBoolean` node of its own for the cut. Two contours meant four nodes for one action.
//! Everything else follows: editing a node opened ONE contour and knew nothing of the neighbouring
//! cut, and deleting any link carried off the chain below. The extrude had satisfied this rule ("all
//! the contours in one operation, the boolean INSIDE the node") long ago; the revolve and the sweep
//! had not.
//!
//! What is checked is NOT the arrangement but the consequence: how many nodes were added to the
//! timeline by one application of a command. Such a test will not let the rule fall off any operation
//! over contours.
#[cfg(test)]
mod tests {
    use super::super::{App, Sel};
    use qymcad_core::feature::{FeatureKind as FK, SketchPlane};

    /// A plate 60x60x10 — what the cutting is done out of.
    fn plate(app: &mut App) {
        let si = app.create_sketch_on(SketchPlane::default());
        app.project.add_rect_entity(si, -30.0, -30.0, 30.0, 30.0, qymcad_core::feature::Purpose::Real);
        app.project.regen_sketch(si);
        app.finish_sketch_edit();
        app.sel = Sel::Sketch(si);
        app.feat.op = 0;
        app.start_feat_cmd(1);
        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "height") {
            p.val = 10.0;
            p.txt = "10".into();
        }
        app.apply_feat_cmd();
    }

    /// A sketch with TWO circles on one side of the X axis — two tool contours for the revolve.
    fn two_contours(app: &mut App) -> (usize, Vec<u64>) {
        let si = app.create_sketch_on(SketchPlane::default());
        app.project.add_circle_entity(si, 12.0, 18.0, 3.0, qymcad_core::feature::Purpose::Real);
        app.project.add_circle_entity(si, 22.0, 18.0, 3.0, qymcad_core::feature::Purpose::Real);
        app.project.regen_sketch(si);
        app.finish_sketch_edit();
        let cids = app.sketch_closed_contours(si);
        assert_eq!(cids.len(), 2, "the sketch must hold exactly two closed contours");
        (si, cids)
    }

    /// A REVOLVE CUT OVER TWO CONTOURS IS ONE NODE. Exactly the reported case.
    #[test]
    fn a_two_contour_revolve_cut_adds_exactly_one_node() {
        let mut app = App::default();
        plate(&mut app);
        let (si, cids) = two_contours(&mut app);

        let before = app.project.timeline.len();
        app.sel = Sel::Sketch(si);
        app.start_feat_cmd(3); // revolve
        // THE REPORTED ORDER: the command opens on "add", and "cut" is chosen in the bar afterwards
        app.feat.op = 2;
        app.gsel.profiles.extend(cids.iter().copied());
        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "angle") {
            p.val = 360.0;
            p.txt = "360".into();
        }
        app.apply_feat_cmd();

        // the sketch node plus the operation node makes 2; anything beyond that is breeding
        let added: Vec<&qymcad_core::feature::FeatureNode> = app.project.timeline[before..].iter().collect();
        let ops: Vec<String> = added.iter().filter(|n| !matches!(n.kind, FK::Sketch { .. })).map(|n| format!("{:?}", n.kind).chars().take(24).collect()).collect();
        assert_eq!(ops.len(), 1, "one command must give ONE operation node, and it came out {}: {ops:?}", ops.len());

        let node = added.iter().find(|n| !matches!(n.kind, FK::Sketch { .. })).expect("the operation node");
        match &node.kind {
            FK::Revolve { profiles, src, op, .. } => {
                assert_eq!(profiles.len(), 2, "both contours must lie IN ONE node: {profiles:?}");
                assert_ne!(*src, 0, "a cut must remember the body it cuts from — otherwise it is an ADD");
                assert_eq!(*op, 0, "a cut is boolean 0, not an addition");
            }
            k => panic!("one revolve node was expected, and it came out {k:?}"),
        }
        assert!(app.project.regen_errors.is_empty(), "a revolve cut must build: {:?}", app.project.regen_errors);
    }

    /// AND THE SHAPE IS RIGHT: the cut removed material rather than adding it. A count of nodes
    /// alone will not prove that.
    #[test]
    fn the_two_contour_revolve_cut_removes_material() {
        let mut app = App::default();
        plate(&mut app);
        let vol_before: f64 = {
            let eaten = app.consumed_bodies();
            app.live.shapes.iter().filter(|(b, _)| !eaten.contains(b)).map(|(_, s)| s.volume()).sum()
        };
        assert!(vol_before > 1.0, "the plate did not build: {}", app.status);

        let (si, cids) = two_contours(&mut app);
        app.sel = Sel::Sketch(si);
        app.start_feat_cmd(3);
        app.feat.op = 2; // "cut" in the bar, after the command opens
        app.gsel.profiles.extend(cids.iter().copied());
        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "angle") {
            p.val = 360.0;
            p.txt = "360".into();
        }
        app.apply_feat_cmd();

        let vol_after: f64 = {
            let eaten = app.consumed_bodies();
            app.live.shapes.iter().filter(|(b, _)| !eaten.contains(b)).map(|(_, s)| s.volume()).sum()
        };
        assert!(vol_after < vol_before - 1.0, "a CUT must remove material: it was {vol_before:.1}, it became {vol_after:.1} — it looks as if the operation worked as an addition");
    }

    /// AN EDIT OPENS THE FEATURE WHOLE — both contours, not whichever comes first.
    ///
    /// Reported: they cannot be edited. A node that opens with one contour out of two would silently
    /// reduce the cut to half on "apply" — which is worse than a refusal, because it looks like
    /// success.
    #[test]
    fn editing_the_feature_restores_all_its_contours() {
        let mut app = App::default();
        plate(&mut app);
        let (si, cids) = two_contours(&mut app);
        app.sel = Sel::Sketch(si);
        app.start_feat_cmd(3);
        app.feat.op = 2; // "cut" in the bar, after the command opens
        app.gsel.profiles.extend(cids.iter().copied());
        app.apply_feat_cmd();

        let fid = app.project.timeline.iter().rev().find(|n| matches!(n.kind, FK::Revolve { .. })).map(|n| n.id).expect("the revolve node");
        app.cancel_all_tools();
        app.gsel.profiles.clear();
        app.start_feat_cmd_edit(fid);
        assert_eq!(app.gsel.profiles.len(), 2, "the edit must bring back BOTH contours, and it brought back {:?}", app.gsel.profiles);
        assert_eq!(app.feat.op, 2, "the edit must bring back the CUT rather than an addition");
    }

    /// A GUARD FOR THE FUTURE: no command over contours builds a chain of "operation plus a separate
    /// boolean".
    ///
    /// The source of applying the commands is read: `finish_base_body` with a CUT boolean (0) or an
    /// INTERSECTION (2) is exactly the boolean carried outside, because of which a feature stopped
    /// being one feature. An addition (1) in an empty part is legitimate: there is nothing to boolean
    /// there, and the node simply folds into the single body of the part.
    #[test]
    fn no_command_leaves_its_boolean_outside_the_node() {
        let src = include_str!("commands.rs");
        let code = src.split("#[cfg(test)]").next().expect("the working part");
        for bad in ["finish_base_body(body, 0)", "finish_base_body(body, 2)", "finish_base_body(body, fb_op)", "finish_base_body(last, fb_op)"] {
            assert!(
                !code.contains(bad),
                "\"{bad}\": the boolean is carried out as a SEPARATE node — the operation became two features again. \
                 The target body and the kind of boolean must lie IN THE NODE ITSELF (see `src`/`op` on Revolve/Sweep/Loft)"
            );
        }
    }
}
