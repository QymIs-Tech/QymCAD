//! LOST REFERENCES DO NOT SURVIVE AN EDIT OF THE FEATURE.
//!
//! Reported behaviour: a chamfer on 4 picked edges, then the sketch is rebuilt and the chamfer turns
//! red; a double click, 4 edges picked again (the previous ones are gone and are not highlighted),
//! Enter — and now the feature shows 8 selected edges in the tree.
//!
//! That is the worst kind of trouble: the program did NOT what it was asked to, and said so only by a
//! number in the tree. References that point at nothing stay in the feature, and it will keep trying
//! to resolve them on every rebuild.
//!
//! The check was written BEFORE the fix and had to go red — otherwise a guess would have been fixed.
#[cfg(test)]
mod tests {
    use super::super::{App, Sel};
    use qymcad_core::feature::FeatureKind;
    use qymcad_core::model::Id;

    /// A cube in a part; returns (mesh index, body id).
    fn part_with_cube(app: &mut App) -> (usize, Id) {
        super::super::joint_flow::tests::add_part_at(app, 0.0);
        app.rebuild_if_dirty_for_test();
        let body = app.project.mesh_id(0).expect("the body");
        if let Some(owner) = app.project.body_owner(body) {
            app.enter_component(owner);
        }
        let mi = app.project.mesh_index(body).expect("the mesh");
        app.sel = Sel::Mesh(mi);
        (mi, body)
    }

    /// The live edge descriptors of the body — what a person sees and can pick.
    fn live_edges(app: &App, body: Id) -> Vec<u32> {
        app.project.regen_edges.get(&body).map(|es| es.iter().map(|e| e.id).collect()).unwrap_or_default()
    }

    /// How many references are recorded in the chamfer.
    fn refs_in_chamfer(app: &App) -> usize {
        app.project
            .timeline
            .iter()
            .find_map(|n| match &n.kind {
                FeatureKind::Chamfer { edges, .. } => Some(edges.query.picked_descs().len()),
                _ => None,
            })
            .unwrap_or(0)
    }

    /// A chamfer on four edges. Returns the node id.
    fn chamfer_on_four(app: &mut App, body: Id) -> Id {
        app.refresh_edges();
        let four: Vec<u32> = live_edges(app, body).into_iter().take(4).collect();
        assert_eq!(four.len(), 4, "a cube must have at least four edges");
        app.start_feat_cmd(5);
        app.select_body(body);
        // THE SELECTION IS PUT IN DIRECTLY, as the neighbouring sweep over the tools does
        // (`tool_popup_sweep`): aiming the mouse at four particular edges is not needed here — the
        // trouble is not in landing the click but in what stays in the feature AFTER the edit.
        app.gsel.edges = four.iter().copied().collect();
        app.gsel.faces_body = Some(body);
        app.apply_feat_cmd();
        app.rebuild_if_dirty_for_test();
        app.project
            .timeline
            .iter()
            .find(|n| matches!(n.kind, FeatureKind::Chamfer { .. }))
            .map(|n| n.id)
            .expect("the chamfer in the timeline")
    }

    /// AN EDIT AFTER LOSING EDGES KEEPS EXACTLY WHAT THE PERSON PICKED.
    #[test]
    fn editing_after_losing_edges_keeps_only_what_was_picked() {
        let mut app = App::default();
        let (_mi, body) = part_with_cube(&mut app);
        let fid = chamfer_on_four(&mut app, body);
        assert_eq!(refs_in_chamfer(&app), 4, "setup: the chamfer holds four edges");

        // THE REFERENCES ARE LOST. In the reported case that came from rebuilding the sketch; what
        // matters to the check is the state itself -- the feature holds descriptors that the live
        // geometry does not have -- and not how it came about.
        if let Some(n) = app.project.timeline.iter_mut().find(|n| n.id == fid) {
            if let FeatureKind::Chamfer { edges, .. } = &mut n.kind {
                *edges = qymcad_core::refs::Ref::picks(&[900_001, 900_002, 900_003, 900_004]);
            }
        }
        app.rebuild_if_dirty_for_test();

        // THE PERSON OPENS THE EDIT AND PICKS FOUR LIVE EDGES.
        app.start_feat_cmd_edit(fid);
        assert!(
            app.gsel.edges.is_empty(),
            "the edit raised LOST references into the selection ({} of them) — a person neither sees them nor picked them",
            app.gsel.edges.len()
        );
        app.refresh_edges();
        let four: Vec<u32> = live_edges(&app, body).into_iter().take(4).collect();
        for e in &four {
            app.gsel.edges.insert(*e);
        }
        app.apply_feat_cmd();
        app.rebuild_if_dirty_for_test();

        assert_eq!(
            refs_in_chamfer(&app),
            4,
            "after the edit the chamfer holds {} references instead of four — the lost ones survived the edit",
            refs_in_chamfer(&app)
        );
    }

    /// The live face descriptors of the body.
    fn live_faces(app: &App, body: Id) -> Vec<u32> {
        app.project.regen_faces.get(&body).map(|fs| fs.iter().map(|f| f.id).collect()).unwrap_or_default()
    }

    /// How many references the LAST feature of the timeline holds, whatever kind it is.
    ///
    /// One reader for every kind: the rule is one (`live_picks`), so the check has to be one too, or it
    /// will be written for the chamfer alone again.
    fn refs_in_last(app: &App) -> usize {
        app.project
            .timeline
            .last()
            .map(|n| match &n.kind {
                FeatureKind::Fillet { edges, .. } | FeatureKind::Chamfer { edges, .. } => edges.query.picked_descs().len(),
                FeatureKind::Shell { faces, .. } | FeatureKind::RemoveFace { faces, .. } | FeatureKind::Draft { faces, .. } => faces.query.picked_descs().len(),
                _ => 0,
            })
            .unwrap_or(0)
    }

    /// EVERY FEATURE WITH REFERENCES OBEYS THE SAME RULE.
    ///
    /// The rule lives in one method (`live_picks`), and until now it was checked by gesture on the chamfer
    /// alone: on the rest it was trusted because "it is wired up in one place, so it holds everywhere" —
    /// the very reasoning that has already cost this project twice. Here the same story is played out on
    /// each of them: pick, lose the references, edit, pick as many again.
    #[test]
    fn every_feature_with_references_keeps_only_what_was_picked() {
        // (the command, its name for the message, whether it picks faces rather than edges, how many)
        let cases: &[(u8, &str, bool, usize)] = &[(4, "fillet", false, 4), (5, "chamfer", false, 4), (6, "shell", true, 1), (26, "remove face", true, 1), (23, "draft", true, 1)];
        let mut bad: Vec<String> = Vec::new();
        for (cmd, what, faces, n) in cases {
            let mut app = App::default();
            let (_mi, body) = part_with_cube(&mut app);
            app.refresh_edges();

            let picks: Vec<u32> = if *faces { live_faces(&app, body) } else { live_edges(&app, body) }.into_iter().take(*n).collect();
            if picks.len() != *n {
                bad.push(format!("  {what}: the cube gave {} of the {n} needed — nothing to check", picks.len()));
                continue;
            }
            app.start_feat_cmd(*cmd);
            app.select_body(body);
            if *faces {
                app.gsel.faces = picks.iter().copied().collect();
            } else {
                app.gsel.edges = picks.iter().copied().collect();
            }
            app.gsel.faces_body = Some(body);
            // A DRAFT NEEDS A NEUTRAL FACE BESIDES the ones being tilted — that is why it stood outside this
            // list for a while, and why "the method is shared, so it holds" was all that covered it.
            if *cmd == 23 {
                let other = live_faces(&app, body).into_iter().find(|f| !picks.contains(f));
                let Some(other) = other else {
                    bad.push(format!("  {what}: the cube gave no face for the neutral one"));
                    continue;
                };
                app.draft.neutral = other;
            }
            app.apply_feat_cmd();
            app.rebuild_if_dirty_for_test();
            let Some(fid) = app.project.timeline.last().map(|x| x.id) else {
                bad.push(format!("  {what}: the feature was not created"));
                continue;
            };
            if refs_in_last(&app) != *n {
                bad.push(format!("  {what}: setup — {} references instead of {n}", refs_in_last(&app)));
                continue;
            }

            // THE REFERENCES ARE LOST. What matters is the state — descriptors the live geometry does not
            // have — not how it came about.
            let dead = qymcad_core::refs::Ref::picks(&[900_001, 900_002, 900_003, 900_004][..*n]);
            if let Some(node) = app.project.timeline.iter_mut().find(|x| x.id == fid) {
                match &mut node.kind {
                    FeatureKind::Fillet { edges, .. } | FeatureKind::Chamfer { edges, .. } => *edges = dead,
                    FeatureKind::Shell { faces, .. } | FeatureKind::RemoveFace { faces, .. } | FeatureKind::Draft { faces, .. } => *faces = dead,
                    _ => {}
                }
            }
            app.rebuild_if_dirty_for_test();

            // THE PERSON OPENS THE EDIT: nothing lost may be raised into the selection.
            app.start_feat_cmd_edit(fid);
            let raised = if *faces { app.gsel.faces.len() } else { app.gsel.edges.len() };
            if raised != 0 {
                bad.push(format!("  {what}: the edit raised {raised} LOST references — a person neither sees nor picked them"));
            }

            app.refresh_edges();
            let again: Vec<u32> = if *faces { live_faces(&app, body) } else { live_edges(&app, body) }.into_iter().take(*n).collect();
            for id in &again {
                if *faces {
                    app.gsel.faces.insert(*id);
                } else {
                    app.gsel.edges.insert(*id);
                }
            }
            app.apply_feat_cmd();
            app.rebuild_if_dirty_for_test();
            let got = refs_in_last(&app);
            if got != *n {
                bad.push(format!("  {what}: after the edit {got} references instead of {n} — the lost ones survived"));
            }
        }
        assert!(bad.is_empty(), "the rule does not hold for every feature:\n{}", bad.join("\n"));
    }

    /// AN EDIT WITH NOTHING LOST BREAKS NOTHING: what was picked stays picked.
    #[test]
    fn editing_without_losses_keeps_the_same_edges() {
        let mut app = App::default();
        let (_mi, body) = part_with_cube(&mut app);
        let fid = chamfer_on_four(&mut app, body);

        app.start_feat_cmd_edit(fid);
        assert_eq!(app.gsel.edges.len(), 4, "the edit must raise into the selection exactly the four edges that were there");
        app.apply_feat_cmd();
        app.rebuild_if_dirty_for_test();
        assert_eq!(refs_in_chamfer(&app), 4, "an edit with nothing lost changed the set of references");
    }
}
