//! AN ANCHOR IS TAKEN FROM THE PART BEING POINTED AT — AND FROM NO OTHER.
//!
//! An anchor is inferred under the cursor by the rule "the nearest snap point wins". Those points were
//! collected from ALL the bodies whose bounding box is near the cursor — that is, an anchor was stolen
//! from the neighbouring part as soon as its edge happened to be closer to the cursor on screen. On a
//! single part that goes unnoticed; on a machine there is always a neighbour standing next to it.
//!
//! Reported behaviour: point at the start of a face, both faces being horizontal, and the gantry
//! carriage is supposed to travel along the rail — yet the gizmo handle runs along Z. Measured on the
//! reported machine: the cursor stood over body 7 and the program offered `EdgeMid(6, 39)` — an edge
//! of the NEIGHBOURING part. Everything follows from that at once: the axis of travel belongs to
//! somebody else, the joint glyph sits away from the click, and "point at the axis" changes
//! nothing.
#[cfg(test)]
mod tests {
    use super::super::App;
    use qymcad_core::feature::{apply12, AnchorRef};
    use qymcad_core::model::Id;

    fn viewport() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 700.0))
    }

    /// TWO PARTS SIDE BY SIDE — like a rail and a beam on a machine.
    fn two_parts_side_by_side(app: &mut App) -> (Id, Id) {
        let before: Vec<Id> = app.project.bodies.iter().map(|b| b.id).collect();
        super::super::joint_flow::tests::add_part_at(app, 0.0);
        super::super::joint_flow::tests::add_part_at(app, 20.0);
        let root = app.project.root;
        app.enter_component(root);
        app.rebuild_if_dirty();
        app.refresh_edges();
        let mine: Vec<Id> = app.project.bodies.iter().map(|b| b.id).filter(|b| !before.contains(b)).collect();
        assert_eq!(mine.len(), 2, "setup: there should be two bodies of our own, and there are {}", mine.len());
        (mine[0], mine[1])
    }

    /// The snap point of THE BODY nearest to a pixel: face centres, edge midpoints and edge ends.
    fn nearest_snap(app: &App, body: Id, at: egui::Pos2) -> f32 {
        let ctx = app.current_ctx_id_for_test();
        let basis = app.cam.basis();
        let wt = app.project.body_display_transform(body, ctx);
        let mut best = f32::MAX;
        if let Some(faces) = app.project.regen_faces.get(&body) {
            for f in faces {
                let w = apply12(&wt, [f.centroid.x, f.centroid.y, f.centroid.z]);
                best = best.min(app.project3(w, viewport(), &basis).0.distance(at));
            }
        }
        if let Some(edges) = app.project.regen_edges.get(&body) {
            for e in edges {
                for p in [e.mid, e.a, e.b] {
                    let w = apply12(&wt, p);
                    best = best.min(app.project3(w, viewport(), &basis).0.distance(at));
                }
            }
        }
        best
    }

    /// A SCENE IN WHICH THE NEIGHBOUR HAS SOMETHING TO STEAL.
    ///
    /// The neighbouring part is moved BEHIND our own along the ray of sight and shifted so that its
    /// snap point falls exactly on the aim. The projection is parallel, so a shift along the ray does
    /// not change the screen position: the neighbour stays exactly under the cursor on screen but
    /// BEHIND the part — so the part under the cursor is still ours while the nearest snap is the
    /// neighbour's.
    ///
    /// The aim is neither the centre of a face nor an edge: there our own snap would be the nearest by
    /// itself and there would be nothing to steal.
    fn aim_where_the_neighbour_is_closer(app: &mut App, mine: Id, neighbour: Id) -> egui::Pos2 {
        let ctx = app.current_ctx_id_for_test();
        let wt = app.project.body_display_transform(mine, ctx);
        let faces = app.project.regen_faces.get(&mine).cloned().expect("the faces of our own part");
        let top = faces.iter().max_by(|a, b| a.centroid.z.total_cmp(&b.centroid.z)).expect("the top face");
        // between the centre of the face and its edge — away from all of our own snaps
        let spot = apply12(&wt, [top.centroid.x + 4.0, top.centroid.y + 4.0, top.centroid.z]);

        let basis = app.cam.basis();
        let away = basis.2;
        let nwt = app.project.body_display_transform(neighbour, ctx);
        let nfaces = app.project.regen_faces.get(&neighbour).cloned().expect("the faces of the neighbour");
        let ntop = nfaces.iter().max_by(|a, b| a.centroid.z.total_cmp(&b.centroid.z)).expect("the top face of the neighbour");
        let now = apply12(&nwt, [ntop.centroid.x, ntop.centroid.y, ntop.centroid.z]);
        let shift = [spot[0] - now[0] + away[0] * 300.0, spot[1] - now[1] + away[1] * 300.0, spot[2] - now[2] + away[2] * 300.0];
        let owner = app.project.body_owner(neighbour).expect("the neighbour has a part");
        let i = app.project.component_index(owner).expect("the part in the document");
        let t = app.project.components[i].transform;
        app.project.components[i].transform = [t[0], t[1], t[2], t[3] + shift[0], t[4], t[5], t[6], t[7] + shift[1], t[8], t[9], t[10], t[11] + shift[2]];
        app.rebuild_if_dirty();
        app.refresh_edges();
        app.project3(spot, viewport(), &basis).0
    }

    #[test]
    fn a_neighbour_does_not_steal_the_anchor() {
        let mut app = App::default();
        let (mine, neighbour) = two_parts_side_by_side(&mut app);
        app.mode_3d = true;
        let at = aim_where_the_neighbour_is_closer(&mut app, mine, neighbour);

        // TRAP GUARD: the neighbour's snap point really is CLOSER — otherwise the check catches
        // nothing and its green colour means nothing.
        let (d_mine, d_neighbour) = (nearest_snap(&app, mine, at), nearest_snap(&app, neighbour, at));
        assert!(
            d_neighbour < d_mine,
            "GUARD: the neighbour's snap must be closer to the cursor ({d_neighbour:.1} px) than our own ({d_mine:.1} px) — otherwise there is nothing to steal"
        );
        // And the part under the cursor must be OUR OWN, otherwise the aim went elsewhere.
        let under = app.pick_part_face_at(viewport(), at).map(|(b, _)| b);
        assert_eq!(under, Some(mine), "GUARD: the wrong part turned up under the cursor ({under:?}) — the aim missed");

        let got = app.infer_mate_anchor(viewport(), at).expect("there is a part under the cursor, an anchor must be inferred");
        let owner = match &got.1 {
            AnchorRef::FaceCenter(b, _) | AnchorRef::EdgeMid(b, _) | AnchorRef::Vertex(b, _, _) => Some(*b),
            _ => None,
        };
        assert_eq!(
            owner,
            Some(mine),
            "the pointing was at part {mine} and the anchor was inferred on {owner:?} — the neighbouring part took the anchor away: {:?}",
            got.1
        );
    }

    #[test]
    fn the_axis_pick_takes_the_part_under_the_cursor_too() {
        // THE SAME RULE FOR "POINT AT THE AXIS". It has a door of its own, and it took an edge from
        // anywhere in the frame: click the same horizontal rail guide and the axis is drawn along Z.
        // An axis is a direction, and it must belong to whatever is being pointed at.
        let mut app = App::default();
        let (mine, neighbour) = two_parts_side_by_side(&mut app);
        app.mode_3d = true;
        let at = aim_where_the_neighbour_is_closer(&mut app, mine, neighbour);

        let under = app.pick_part_face_at(viewport(), at).map(|(b, _)| b);
        assert_eq!(under, Some(mine), "GUARD: the wrong part turned up under the cursor ({under:?}) — the aim missed");

        let got = app.infer_axis_anchor(viewport(), at).expect("there is a part under the cursor, a direction must be inferred");
        let owner = match &got.1 {
            AnchorRef::FaceCenter(b, _) | AnchorRef::EdgeMid(b, _) | AnchorRef::Vertex(b, _, _) => Some(*b),
            _ => None,
        };
        assert_eq!(owner, Some(mine), "the axis was pointed at on part {mine} and was taken from {owner:?}: {:?}", got.1);
    }
}
