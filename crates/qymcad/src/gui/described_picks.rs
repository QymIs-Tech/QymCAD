//! A REFERENCE REMEMBERS NOT ONLY WHAT WAS PICKED BUT ALSO HOW.
//!
//! The query mechanism was ready, and yet a descriptive reference could not be created from the
//! interface: the commands only laid down manual picks. The benefit stayed potential — the code exists
//! and a person cannot reach it.
//!
//! The answer needs no new buttons: **the gesture already carries the intent**. A click on an edge
//! means that particular edge. A click on a FACE means something else: "round this whole rim", and
//! that must survive an edit after which there are more edges. The first is written as a list, the
//! second as a description.
//!
//! Touch even one edge on its own and the description collapses: "every edge of the face except this
//! one" is not yet expressible, and leaving the description as it is would be a lie. Then it is a list
//! again.
#[cfg(test)]
mod tests {
    use super::super::App;
    use qymcad_core::feature::FeatureKind;
    use qymcad_core::refs::Query;

    /// A plate with a built body; returns (application, body).
    fn plate() -> (App, qymcad_core::model::Id) {
        let mut app = App::default();
        let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        app.project.add_rect_entity(si, 0.0, 0.0, 60.0, 40.0, qymcad_core::feature::Purpose::Real);
        app.project.regen_sketch(si);
        app.finish_sketch_edit();
        app.sel = super::super::Sel::Sketch(si);
        app.start_feat_cmd(1);
        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "height") {
            p.val = 12.0;
            p.txt = "12".into();
        }
        app.apply_feat_cmd();
        app.rebuild_if_dirty();
        let body = app.project.timeline.iter().rev().find_map(|n| n.kind.body()).expect("the body is built");
        (app, body)
    }

    /// A CLICK ON A FACE IS RECORDED AS A DESCRIPTION rather than a snapshot of a list of edges.
    #[test]
    fn picking_a_face_records_the_intent_not_the_snapshot() {
        let (mut app, body) = plate();
        let face = app.project.regen_faces[&body].iter().max_by(|a, b| a.centroid.z.total_cmp(&b.centroid.z)).expect("the top face").id;

        app.select_body(body);
        app.start_feat_cmd(4); // fillet
        app.gsel.edges = [11, 12, 13, 14].into_iter().collect(); // as if the edges of the face had been collected
        app.gsel.describe_edges_of_face(face);
        app.apply_feat_cmd();

        let edges = app
            .project
            .timeline
            .iter()
            .find_map(|n| match n.kind {
                FeatureKind::Fillet { ref edges, .. } => Some(edges.clone()),
                _ => None,
            })
            .expect("the fillet in the timeline");
        match &edges.query {
            Query::Adjacent(inner) => assert!(matches!(**inner, Query::Id(f) if f == face), "the description must refer to that very face"),
            other => panic!("a click on a face must be recorded as a description, and it came out {other:?}"),
        }
        assert!(edges.query.picked_descs().iter().all(|d| *d == face), "a description must hold no snapshot of edges");
    }

    /// AND A CLICK ON AN EDGE IS A LIST. The other half: without it "everything is described" would
    /// be untrue.
    #[test]
    fn picking_edges_one_by_one_stays_a_list() {
        let (mut app, body) = plate();
        app.select_body(body);
        app.start_feat_cmd(4);
        app.gsel.edges = [21, 22].into_iter().collect();
        app.gsel.described = None; // the edges were picked one by one
        app.apply_feat_cmd();

        let edges = app
            .project
            .timeline
            .iter()
            .find_map(|n| match n.kind {
                FeatureKind::Fillet { ref edges, .. } => Some(edges.clone()),
                _ => None,
            })
            .expect("the fillet in the timeline");
        let mut picked = edges.query.picked_descs();
        picked.sort_unstable();
        assert_eq!(picked, vec![21, 22], "a manual selection must stay a list: {:?}", edges.query);
        assert!(!matches!(edges.query, Query::Adjacent(_)), "there must be no description here");
    }

    /// A SINGLE EDGE AFTER A FACE COLLAPSES THE DESCRIPTION — and rightly so.
    ///
    /// "Every edge of the face except this one" is not yet expressible. Leaving the description as it
    /// is would mean writing into the document an intent nobody has.
    #[test]
    fn touching_one_edge_after_a_face_drops_the_description() {
        let source = include_str!("pick.rs");
        assert!(
            source.contains("self.gsel.described = None;"),
            "a single click on an edge must clear the description, otherwise the reference lies"
        );
        // and the other way round: a click on a face must RECORD the description
        assert!(source.contains("self.gsel.describe_edges_of_face(fid);"), "a click on a face must record a description");
    }

    /// A VARIABLE FILLET IS NOW SET BY A DESCRIPTION TOO.
    ///
    /// The opposite guard used to stand here: a variable radius must stay on an explicit list, because
    /// the r-to-r2 law needs the start and the end of an edge and a description has neither. It was
    /// true of THAT way of setting it and left along with it: the radius moved from the ends of the
    /// edge into a VERTEX, and a vertex has no direction. So it does not require a list either — the
    /// description lives on.
    ///
    /// This is the case where a red test means a lifted restriction rather than a regression; so it
    /// was not "fixed by value" but rewritten whole under the new rule.
    #[test]
    fn a_variable_radius_no_longer_forces_an_explicit_list() {
        let (mut app, body) = plate();
        let face = app.project.regen_faces[&body].iter().max_by(|a, b| a.centroid.z.total_cmp(&b.centroid.z)).expect("the top face").id;

        app.select_body(body);
        app.start_feat_cmd(4);
        app.gsel.edges = [11, 12].into_iter().collect();
        app.gsel.describe_edges_of_face(face); // the face is picked, so a description suggests itself
        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "radius") {
            p.val = 3.0;
            p.txt = "3".into();
        }
        // a radius at one of the corners, as a vertex table
        let corner = app.project.vertex_spots(body).into_iter().map(|(pt, ids)| (pt, ids)).next().expect("the body has a corner");
        let desc = app.project.names.intern_vertex(qymcad_core::names::VertexName::new(corner.1));
        app.cmd.params.push(crate::gui::CmdParam::new("f-radius-at-vertex", &format!("at{desc}"), 5.0, 0.0, 1000.0).at(corner.0));
        app.apply_feat_cmd();

        let (edges, table) = app
            .project
            .timeline
            .iter()
            .find_map(|n| match n.kind {
                FeatureKind::Fillet { ref edges, ref at_vertices, .. } if !at_vertices.is_empty() => Some((edges.clone(), at_vertices.clone())),
                _ => None,
            })
            .expect("the variable fillet in the timeline");
        assert_eq!(table.len(), 1, "the vertex table must reach the timeline");
        assert!(
            matches!(edges.query, Query::Adjacent(_)),
            "a variable radius no longer drops the description down to a list of picks: a radius at a vertex has no direction and needs no list ({:?})",
            edges.query
        );
    }
}
