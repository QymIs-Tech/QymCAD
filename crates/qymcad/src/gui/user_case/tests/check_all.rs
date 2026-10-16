use super::*;

/// THE SINGLE "EVERYTHING ADDS UP" CHECK - run after every action rather than as a separate test.
pub(super) fn check_all(app: &mut App, step: &str, problems: &mut Vec<String>) {
    qymcad_ui_state::rebuild_if_dirty(&mut app.rebuild_ctx());
    // 1. THE GEOMETRY: not a single red node.
    // WHAT COUNTS AS RED IS WHAT THE PROGRAM COULD NOT EXPLAIN. A named refusal is a conversation: "the
    // chamfer is larger than the wall", "the fillet has nothing to round". Such a node is legitimate
    // during work - it is seen and reworked. A nameless `OpFailed` is another matter: that is the program
    // staying silent, and it remains a finding at every step.
    let unexplained: std::collections::HashMap<u64, String> =
        app.project.regen_errors.iter().filter(|(_, e)| matches!(e, qymcad_core::errors::CoreError::OpFailed(_))).map(|(id, e)| (*id, format!("{e:?}"))).collect();
    if !unexplained.is_empty() {
        problems.push(format!("[{step}] red nodes WITH NO EXPLANATION: {unexplained:?}"));
    }
    // 1b. THE STRUCTURE OF THE DOCUMENT. A file was opened and showed a part that contained both nested
    // parts and its own build features. No CAD has such a structure, and there was no check for it -
    // which is how it reached a file. A part either CONTAINS a makeup or IS BUILT.
    for c in &app.project.components {
        if !matches!(c.kind, qymcad_core::feature::ComponentKind::Part) {
            continue;
        }
        let nm = crate::i18n::name(&c.name);
        let inside: Vec<String> = app.project.components.iter().filter(|x| x.parent == Some(c.id)).map(|x| crate::i18n::name(&x.name)).collect();
        if !inside.is_empty() {
            problems.push(format!("[{step}] part \"{nm}\" contains other components: {}", inside.join(", ")));
        }
    }
    // 1c. A NODE WITH NO PART. A build feature must belong to a part: there is nowhere to show it in the
    // tree, and it can be neither found nor deleted. It is caught at the step where it appeared rather
    // than at the end of the run.
    for n in &app.project.timeline {
        // IT EITHER BUILT OR EXPLAINED ITSELF - THERE IS NO THIRD OPTION.
        //
        // A node that declares a body must either produce it or leave an error. Silence is the worst
        // outcome: the row is in the tree, nothing is on screen, and there is nothing to take hold of.
        // Found by a full rebuild of AN UNCHANGED timeline: a fillet that built the first time
        // disappeared on the second pass without a single word.
        // ...AND ONLY WHERE THAT IS EXPECTED. Under a rollback of the history a node is legitimately
        // unbuilt and has no reason attached - that is not silence but "we have not reached it yet". The
        // silence of a node whose INPUT is unbuilt is legitimate in the same way: the reason was already
        // given further up the chain.
        // ...AND ONE MORE BOUNDARY, crossed once and rewarded with a false alarm: an input may carry A
        // MESH FROM A PREVIOUS BUILD yet fail to build now - then the reason is named ON IT, and the
        // silence of the consumer is legitimate. The judgement must go by whether the producer of the
        // input explained itself, not by whether a mesh exists.
        let inputs_built = n.kind.inputs().iter().all(|i| {
            let producer = app.project.timeline.iter().find(|x| x.kind.bodies().contains(i));
            match producer {
                Some(p) => app.project.mesh_index(*i).is_some() && !app.project.regen_errors.contains_key(&p.id),
                None => true,
            }
        });
        if !n.suppressed && app.project.rollback.is_none() && inputs_built {
            for b in n.kind.bodies() {
                let built = app.project.mesh_index(b).is_some();
                let explained = app.project.regen_errors.contains_key(&n.id);
                if !built && !explained {
                    problems.push(format!("[{step}] node {} \"{}\" did not build body {b} and NAMED NO REASON - the row is in the tree, the screen is empty", n.id, n.name));
                }
            }
        }
        // THE PARENT MUST EXIST rather than merely be filled in. The old check caught only `None` and
        // missed the worst case: a reference to A DELETED part. There is nowhere to show such a node in
        // the tree (its branch is gone), it can be neither found nor deleted, and a rebuild will honestly
        // keep counting it.
        let parent_alive = n.parent.is_some_and(|p| app.project.components.iter().any(|c| c.id == p));
        if !parent_alive {
            problems.push(format!("[{step}] node {} \"{}\" sits in the timeline with no live part (parent = {:?}) - there is nowhere to show it in the tree", n.id, n.name, n.parent));
        }
    }
    // 1d. THERE MUST BE NO BODIES IN THE ROOT. A body lives in A PART; in the root assembly it has
    // nowhere to come from, and opening the tree shows a bare "Body 36" with no part around it.
    {
        let root = app.project.root;
        // by the body's owner, not the node's parent: a pattern of components stands in the assembly and builds
        // bodies that belong to its copies
        let orphan: Vec<u64> = app.project.timeline.iter().flat_map(|n| n.kind.bodies().into_iter().filter(move |b| n.owner_of(*b) == Some(root))).collect();
        // BY THE SAME SIGN THE TREE USES. In the root it shows THE MESHES that no timeline node produces
        // - the previous check looked for "bodies with no owner" and did not see those.
        let produced: std::collections::HashSet<u64> = app.project.timeline.iter().flat_map(|n| n.kind.bodies()).collect();
        let in_root_tree: Vec<u64> = app.project.bodies.iter().map(|b| b.id).filter(|b| !produced.contains(b)).collect();
        if !in_root_tree.is_empty() {
            problems.push(format!("[{step}] bodies {in_root_tree:?} are shown IN THE ROOT of the tree - no node produces them"));
        }
        let ownerless: Vec<u64> = app
            .project
            .bodies
            .iter()
            .filter(|b| {
                let o = app.project.body_owner(b.id);
                o.is_none() || o == Some(root)
            })
            .map(|b| b.id)
            .collect();
        if !orphan.is_empty() || !ownerless.is_empty() {
            problems.push(format!("[{step}] bodies with no part: in the root {orphan:?}, with no owner {ownerless:?} - a body must live in A PART, otherwise it hangs by itself in the tree"));
        }
    }
    // 1e. THE NAMES OF PARTS DO NOT REPEAT. Two parts with one name are indistinguishable in the tree:
    // one gets picked and another gets edited. The instances of an array are legitimately called
    // "Housing (2)", "(3)" - those are DIFFERENT names and they are fine; what is at issue is exact
    // matches.
    {
        let mut seen: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for c in app.project.components.iter().filter(|c| c.id != app.project.root) {
            *seen.entry(crate::i18n::name(&c.name)).or_default() += 1;
        }
        let dups: Vec<String> = seen.into_iter().filter(|(_, n)| *n > 1).map(|(k, n)| format!("\"{k}\" x{n}")).collect();
        if !dups.is_empty() {
            problems.push(format!("[{step}] parts with IDENTICAL names: {} - they cannot be told apart in the tree", dups.join(", ")));
        }
    }
    // 1f. THE REMAINING SECTIONS OF THE TREE, SEEN BY EYE. Opening a document shows everything at once,
    // and an oddity in any section reads as a fault, however correctly everything is computed.
    {
        // A joint with no part: one whose connector points nowhere hangs in the tree as an orphan.
        let live: std::collections::HashSet<u64> = app.project.components.iter().map(|c| c.id).collect();
        let dangling: Vec<u64> = app
            .project
            .joints
            .iter()
            .filter(|j| {
                let owner = |c: u64| app.project.connectors.iter().find(|k| k.id == c).map(|k| k.owner);
                !owner(j.a).is_some_and(|o| live.contains(&o)) || !owner(j.b).is_some_and(|o| live.contains(&o))
            })
            .map(|j| j.id)
            .collect();
        if !dangling.is_empty() {
            problems.push(format!("[{step}] joints {dangling:?} point at parts that do not exist"));
        }

        // An abandoned sketch: it belongs to no part, and there is nowhere to show it in the tree.
        let orphan_sk: Vec<u64> = app.project.sketches.iter().filter(|sk| app.project.sketch_owner(sk.id).is_none_or(|o| !live.contains(&o))).map(|sk| sk.id).collect();
        if !orphan_sk.is_empty() {
            problems.push(format!("[{step}] sketches {orphan_sk:?} belong to no part"));
        }
    }
    // 1g. WHAT A PERSON SEES ON OPENING A FINISHED FILE. A hidden body in someone else's document looks
    // like a loss: the part is in the tree and not on screen. A part with not a single body is an empty
    // row with nothing behind it. A node with no name is the same thing in the timeline.
    {
        // A HIDDEN BODY DURING WORK IS NORMAL: that is how a neighbouring part is reached. The check for
        // ones left hidden stands at the end, on A FINISHED document.
        // AN EMPTY PART DURING WORK IS NORMAL (the program keeps one from the very start, and building
        // into it is about to begin). It becomes rubbish only in A FINISHED file, so the check stands at
        // the end of the scenario rather than at every step.
        let nameless: Vec<u64> = app.project.timeline.iter().filter(|n| n.name.trim().is_empty()).map(|n| n.id).collect();
        if !nameless.is_empty() {
            problems.push(format!("[{step}] nodes with no name: {nameless:?}"));
        }
    }
    // 2. THE TREE ON THE LEFT: every feature has a row, and that row is words rather than a catalogue key
    for ti in 0..app.project.timeline.len() {
        if matches!(app.project.timeline[ti].kind, qymcad_core::feature::FeatureKind::Sketch { .. }) {
            continue; // sketches live in a branch of their own
        }
        // THE DATUMS LIVE IN THEIR OWN BRANCH of the tree: their row comes not from the timeline but from
        // the plane, the axis or the point itself - and that is where to look, otherwise an honest node
        // appears lost.
        use qymcad_core::feature::FeatureKind as FKd;
        let row = match app.project.timeline[ti].kind {
            FKd::Plane { plane } => app.project.planes.iter().find(|p| p.id == plane).map(|p| format!("· {}", crate::i18n::name(&p.name))).unwrap_or_default(),
            FKd::DatumAxis { axis } => app.project.datum_axes.iter().find(|a| a.id == axis).map(|a| format!("· {}", crate::i18n::name(&a.name))).unwrap_or_default(),
            FKd::DatumPoint { point } => app.project.datum_points.iter().find(|q| q.id == point).map(|q| format!("· {}", crate::i18n::name(&q.name))).unwrap_or_default(),
            _ => crate::gui::panels_tree::feature_row_label(&app.project, ti),
        };
        let words = row.split_whitespace().skip(1).collect::<Vec<_>>().join(" ");
        if words.trim().is_empty() {
            problems.push(format!("[{step}] node {} ({}) has no row in the tree", app.project.timeline[ti].id, crate::gui::feat_default_name(&app.project.timeline[ti].kind)));
        } else if !words.contains(' ') && words.contains('-') && words.chars().all(|c| c.is_ascii_lowercase() || c == '-' || c.is_ascii_digit()) {
            problems.push(format!("[{step}] node {} shows A KEY \"{words}\"", app.project.timeline[ti].id));
        }
    }
    // 3. THE BODIES: "a part is ONE body" is a rule ABOUT A PART, not about the document. An assembly has
    //    as many bodies as it has parts, and counting them together would declare every assembly broken.
    let mut per_part: std::collections::HashMap<u64, usize> = std::collections::HashMap::new();
    for (mi, b) in app.project.bodies.iter().enumerate() {
        if !qymcad_ui_state::body_shown(qymcad_ui_state::body_view_of!(app), mi) || b.sheet {
            continue;
        }
        if let Some(owner) = app.project.body_owner(b.id) {
            *per_part.entry(owner).or_default() += 1;
        }
    }
    for (owner, n) in per_part {
        if n > 1 {
            let name = app.project.components.iter().find(|c| c.id == owner).map(|c| crate::i18n::name(&c.name)).unwrap_or_default();
            let who: Vec<String> = app
                .project
                .bodies
                .iter()
                .enumerate()
                .filter(|(mi, b)| qymcad_ui_state::body_shown(qymcad_ui_state::body_view_of!(app), *mi) && !b.sheet && app.project.body_owner(b.id) == Some(owner))
                .map(|(_, b)| {
                    let node = app.project.timeline.iter().find(|n| n.kind.bodies().contains(&b.id));
                    let red = node.is_some_and(|n| app.project.regen_errors.contains_key(&n.id));
                    format!("{}({}{})", b.id, node.map(|n| n.name.clone()).unwrap_or_else(|| "no node".into()), if red { ", red" } else { "" })
                })
                .collect();
            // A CUT IS A LEGITIMATE EXCEPTION. The "split a body" tool exists precisely so that one body
            // becomes several, and a cut through the body leaves its pieces bodies of the part the same way;
            // after them, several bodies in a part are not a fault but work that was done deliberately. Without
            // this proviso the check would forbid the tools themselves.
            let was_split =
                app.project.timeline.iter().any(|nd| nd.parent == Some(owner) && (matches!(nd.kind, qymcad_core::feature::FeatureKind::SplitBody { .. }) || nd.kind.cut_pieces().len() > 1));
            if !was_split {
                problems.push(format!("[{step}] part \"{name}\" has {n} visible bodies instead of one: {}", who.join(", ")));
            }
        }
    }
    for (mi, b) in app.project.bodies.iter().enumerate() {
        if qymcad_ui_state::body_shown(qymcad_ui_state::body_view_of!(app), mi) && b.mesh.tris.is_empty() {
            problems.push(format!("[{step}] body {} is visible, but there is nothing to draw", b.id));
        }
    }
    // 4. THE STATUS: the program shows no key where a message belongs
    let st = app.status.words.trim();
    if !st.is_empty() && !st.contains(' ') && st.contains('-') && st.chars().all(|c| c.is_ascii_lowercase() || c == '-' || c.is_ascii_digit()) {
        problems.push(format!("[{step}] the status line holds A KEY \"{st}\""));
    }
}
