//! A SCENARIO OF REAL USE: what a person does in the first fifteen minutes.
//!
//! Reported: five minutes of working by hand turn up enough faults that a simple project cannot be
//! assembled, while every check here stays green - and that is strange. It is strange, and the report was
//! right: the checks ran one action at a time on a cube, while what breaks is A CHAIN - a sketch on a face
//! of another part, holes, a shell, a second part, an assembly, a dimension edited in the middle of it.
//!
//! Here there is one scenario and one check after EVERY step: the geometry, the tree on the left, the panel
//! on the right, the lists of bodies, the absence of keys where words belong. That is how a person works,
//! and that is how it must be checked.
#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use super::super::App;

    /// THE SINGLE "EVERYTHING ADDS UP" CHECK - run after every action rather than as a separate test.
    fn check_all(app: &mut App, step: &str, problems: &mut Vec<String>) {
        app.rebuild_if_dirty();
        // 1. THE GEOMETRY: not a single red node.
        // WHAT COUNTS AS RED IS WHAT THE PROGRAM COULD NOT EXPLAIN. A named refusal is a conversation: "the
        // chamfer is larger than the wall", "the fillet has nothing to round". Such a node is legitimate
        // during work - it is seen and reworked. A nameless `OpFailed` is another matter: that is the program
        // staying silent, and it remains a finding at every step.
        let unexplained: std::collections::HashMap<u64, String> = app
            .project
            .regen_errors
            .iter()
            .filter(|(_, e)| matches!(e, qymcad_core::errors::CoreError::OpFailed(_)))
            .map(|(id, e)| (*id, format!("{e:?}")))
            .collect();
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
            let inside: Vec<String> = app
                .project
                .components
                .iter()
                .filter(|x| x.parent == Some(c.id))
                .map(|x| crate::i18n::name(&x.name))
                .collect();
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
                        problems.push(format!(
                            "[{step}] node {} \"{}\" did not build body {b} and NAMED NO REASON - the row is in the tree, the screen is empty",
                            n.id, n.name
                        ));
                    }
                }
            }
            // THE PARENT MUST EXIST rather than merely be filled in. The old check caught only `None` and
            // missed the worst case: a reference to A DELETED part. There is nowhere to show such a node in
            // the tree (its branch is gone), it can be neither found nor deleted, and a rebuild will honestly
            // keep counting it.
            let parent_alive = n.parent.is_some_and(|p| app.project.components.iter().any(|c| c.id == p));
            if !parent_alive {
                problems.push(format!(
                    "[{step}] node {} \"{}\" sits in the timeline with no live part (parent = {:?}) - there is nowhere to show it in the tree",
                    n.id, n.name, n.parent
                ));
            }
        }
        // 1d. THERE MUST BE NO BODIES IN THE ROOT. A body lives in A PART; in the root assembly it has
        // nowhere to come from, and opening the tree shows a bare "Body 36" with no part around it.
        {
            let root = app.project.root;
            let orphan: Vec<u64> = app
                .project
                .timeline
                .iter()
                .filter(|n| n.parent == Some(root))
                .flat_map(|n| n.kind.bodies())
                .collect();
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
                problems.push(format!(
                    "[{step}] bodies with no part: in the root {orphan:?}, with no owner {ownerless:?} - a body must live in A PART, otherwise it hangs by itself in the tree"
                ));
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
            let orphan_sk: Vec<u64> = app
                .project
                .sketches
                .iter()
                .filter(|sk| app.project.sketch_owner(sk.id).is_none_or(|o| !live.contains(&o)))
                .map(|sk| sk.id)
                .collect();
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
                _ => app.feature_row_label(ti),
            };
            let words = row.split_whitespace().skip(1).collect::<Vec<_>>().join(" ");
            if words.trim().is_empty() {
                problems.push(format!(
                    "[{step}] node {} ({}) has no row in the tree",
                    app.project.timeline[ti].id,
                    App::feat_default_name(&app.project.timeline[ti].kind)
                ));
            } else if !words.contains(' ') && words.contains('-') && words.chars().all(|c| c.is_ascii_lowercase() || c == '-' || c.is_ascii_digit()) {
                problems.push(format!("[{step}] node {} shows A KEY \"{words}\"", app.project.timeline[ti].id));
            }
        }
        // 3. THE BODIES: "a part is ONE body" is a rule ABOUT A PART, not about the document. An assembly has
        //    as many bodies as it has parts, and counting them together would declare every assembly broken.
        let mut per_part: std::collections::HashMap<u64, usize> = std::collections::HashMap::new();
        for (mi, b) in app.project.bodies.iter().enumerate() {
            if !app.body_shown(mi) || b.sheet {
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
                    .filter(|(mi, b)| app.body_shown(*mi) && !b.sheet && app.project.body_owner(b.id) == Some(owner))
                    .map(|(_, b)| {
                        let node = app.project.timeline.iter().find(|n| n.kind.bodies().contains(&b.id));
                        let red = node.is_some_and(|n| app.project.regen_errors.contains_key(&n.id));
                        format!("{}({}{})", b.id, node.map(|n| n.name.clone()).unwrap_or_else(|| "no node".into()), if red { ", red" } else { "" })
                    })
                    .collect();
                // A CUT IS A LEGITIMATE EXCEPTION. The "split a body" tool exists precisely so that one body
                // becomes several; after it, several bodies in a part are not a fault but work that was done
                // deliberately. Without this proviso the check would forbid the tool itself.
                let was_split = app
                    .project
                    .timeline
                    .iter()
                    .any(|nd| nd.parent == Some(owner) && matches!(nd.kind, qymcad_core::feature::FeatureKind::SplitBody { .. }));
                if !was_split {
                    problems.push(format!("[{step}] part \"{name}\" has {n} visible bodies instead of one: {}", who.join(", ")));
                }
            }
        }
        for (mi, b) in app.project.bodies.iter().enumerate() {
            if app.body_shown(mi) && b.mesh.tris.is_empty() {
                problems.push(format!("[{step}] body {} is visible, but there is nothing to draw", b.id));
            }
        }
        // 4. THE STATUS: the program shows no key where a message belongs
        let st = app.status.trim();
        if !st.is_empty() && !st.contains(' ') && st.contains('-') && st.chars().all(|c| c.is_ascii_lowercase() || c == '-' || c.is_ascii_digit()) {
            problems.push(format!("[{step}] the status line holds A KEY \"{st}\""));
        }
    }

    /// Save and reopen - and check everything there as well.
    fn save_and_reopen(app: &mut App, step: &str, problems: &mut Vec<String>) -> App {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target").join("user-case.qcad").to_string_lossy().into_owned();
        app.set_project_path(path.clone());
        app.save_project();
        app.drain_bg_for_test();
        match qymcad_io::load_project(&path) {
            Ok(project) => {
                let mut fresh = App::default();
                fresh.finish_project_load(path, project, Vec::new());
                fresh.rebuild_if_dirty();
                let mut p2 = Vec::new();
                check_all(&mut fresh, &format!("{step} -> after opening"), &mut p2);
                problems.extend(p2);
                fresh
            }
            Err(e) => {
                problems.push(format!("[{step}] the saved file does not open: {e}"));
                App::default()
            }
        }
    }

    /// APPLY A TOOL TO THE CURRENT BODY OF A PART - one recipe per tool, for any part.
    ///
    /// That is exactly what it is for: the request was that EVERY part carry ALL of the tools, but in a
    /// DIFFERENT order. Separate code per part cannot express that - a recipe has to work with whatever body
    /// exists NOW, however the previous operations have left it.
    ///
    /// Returns `false` when the tool did not fire: that is either a finding or an honest reason, and the
    /// caller decides which, because only the caller knows what it expected.
    ///
    /// Type a string into the options bar of the text tool - without one there is nothing to draw.
    fn hand_text(app: &mut App) {
        app.tool_prefs.text = "CAD".into();
    }

    /// A pair of sketch entities to place a constraint on: two lines if there are any, otherwise two points.
    fn app_sel_pair(sk: &qymcad_core::model::Sketch) -> Vec<(u8, u64)> {
        let mut out: Vec<(u8, u64)> = sk.entities.iter().take(2).map(|e| (1u8, e.id)).collect();
        if out.len() < 2 {
            out = sk.points.iter().take(2).map(|p| (0u8, p.id)).collect();
        }
        out
    }

    fn apply_tool(app: &mut App, kind: u8, problems: &mut Vec<String>, part: &str) -> bool {
        let before = app.project.timeline.len();
        // THE LAST SOLID BODY, not the last body of any kind: after a face copy or a patch the last one is A
        // SHEET, and the part tools would try to cut a surface. Work at that moment continues on the part, and
        // the recipe must behave the same way.
        // ...AND ONLY WITHIN THE CURRENT PART. Searching the whole document led to a reference into another
        // component: the tool took the body of a neighbouring part and the program honestly refused. Work
        // happens inside the part that was entered, and the recipe must behave the same way.
        let here = app.project.active_ctx();
        let Some(body) = app
            .project
            .timeline
            .iter()
            .rev()
            .filter(|n| n.parent == Some(here))
            .filter_map(|n| n.kind.body())
            .find(|b| app.project.bodies.iter().any(|x| x.id == *b && !x.sheet))
        else {
            return false;
        };
        app.select_body(body);
        if app.selected_body_for_test() != Some(body) {
            problems.push(format!("[{part}] the selection drifted: body {body} was asked for, {:?} is selected", app.selected_body_for_test()));
        }
        let c = app
            .project
            .bodies
            .iter()
            .find(|b| b.id == body)
            .map(|b| {
                let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
                for v in &b.mesh.verts {
                    for (k, q) in [v.x, v.y, v.z].into_iter().enumerate() {
                        lo[k] = lo[k].min(q);
                        hi[k] = hi[k].max(q);
                    }
                }
                [(lo[0] + hi[0]) * 0.5, (lo[1] + hi[1]) * 0.5, (lo[2] + hi[2]) * 0.5]
            })
            .unwrap_or([0.0; 3]);
        let scale = 8.0;
        let face = |app: &App, dir: [f64; 3]| {
            app.project
                .regen_faces
                .get(&body)
                .and_then(|fs| {
                    // THE OUTER FACE, NOT THE LARGEST ONE. After a shell the part also has an inner wall with
                    // the same normal, and its area is sometimes larger than the outer one - "the largest" took
                    // the tool inside the box, where nobody is aiming.
                    fs.iter()
                        .filter(|f| f.normal[0] * dir[0] + f.normal[1] * dir[1] + f.normal[2] * dir[2] > 0.9)
                        .max_by(|a, b| {
                            let d = |f: &qymcad_core::geom::MeshFace| {
                                (f.centroid.x - c[0]) * dir[0] + (f.centroid.y - c[1]) * dir[1] + (f.centroid.z - c[2]) * dir[2]
                            };
                            d(a).total_cmp(&d(b)).then(a.area.total_cmp(&b.area))
                        })
                        .cloned()
                })
                .map(|f| ([f.centroid.x, f.centroid.y, f.centroid.z], f.id))
        };
        let edge_vertical = |app: &App| app.project.regen_edges.get(&body).and_then(|es| es.iter().filter(|e| (e.a[2] - e.b[2]).abs() > 1.0).max_by(|x, y| x.mid[0].total_cmp(&y.mid[0])).map(|e| e.mid));
        // THE OUTER RIM, NOT JUST ANY EDGE AT THE SAME HEIGHT. After a shell the top of the part carries two
        // rims, inner and outer, and "the highest edge" took the inner one just as readily - and a chamfer on
        // it runs into a 2 mm wall. A person clicks the outer one: it is further from the axis of the part.
        let edge_top = |app: &App| {
            let es = app.project.regen_edges.get(&body)?;
            let top = es.iter().filter(|e| (e.a[2] - e.b[2]).abs() < 1e-6).map(|e| e.mid[2]).fold(f64::MIN, f64::max);
            es.iter()
                .filter(|e| (e.a[2] - e.b[2]).abs() < 1e-6 && (e.mid[2] - top).abs() < 1e-6)
                .max_by(|x, y| {
                    let d = |m: &[f64; 3]| (m[0] - c[0]).hypot(m[1] - c[1]);
                    d(&x.mid).total_cmp(&d(&y.mid))
                })
                .map(|e| e.mid)
        };

        match kind {
            4 | 5 => {
                // THE SIZE FOLLOWS THE STATE OF THE PART, as it does by hand: on a solid body the fillet is
                // large (and it must be LARGER than the wall to come, or the shell will eat it whole), while on
                // an already shelled part the wall is thin and the chamfer has to fit into it.
                let shelled = app
                    .project
                    .timeline
                    .iter()
                    .any(|n| n.parent == Some(here) && matches!(n.kind, qymcad_core::feature::FeatureKind::Shell { .. }));
                let size = if shelled { 0.3 } else { 3.0 };
                let mut hand = Hand::new(app);
                hand.look_at(c, scale).tool(kind);
                let t = if kind == 4 { edge_vertical(hand.app) } else { edge_top(hand.app) };
                let Some(t) = t else { return false };
                hand.click(t).set(if kind == 4 { "radius" } else { "dist" }, size).enter();
            }
            6 => {
                let mut hand = Hand::new(app);
                hand.look_at(c, scale).tool(6);
                let Some((t, _)) = face(hand.app, [0.0, 0.0, 1.0]) else { return false };
                hand.click(t).set("thickness", 1.2).enter();
            }
            25 => {
                let mut hand = Hand::new(app);
                hand.look_at(c, scale).tool(25);
                let Some((t, _)) = face(hand.app, [1.0, 0.0, 0.0]) else { return false };
                hand.click(t).set("dist", 2.0).enter();
            }
            23 => {
                let mut hand = Hand::new(app);
                hand.look_at(c, scale).tool(23);
                // THE BOTTOM IS TAKEN AS THE NEUTRAL FACE - as a draft for printing requires - and to click it
                // the part IS TURNED. That is exactly what a person does when the face is behind the part.
                let (Some((nt, _)), Some((st, _))) = (face(hand.app, [0.0, 0.0, -1.0]), face(hand.app, [0.0, -1.0, 0.0])) else { return false };
                hand.app.draft.pick_neutral = true;
                hand.look_from_below().click(nt);
                hand.orbit(-0.7, 0.6).click(st).set("angle", 4.0).enter();
            }
            7 => {
                // A HOLE: the centre comes from the face that was clicked, the diameter and depth from the fields.
                let mut hand = Hand::new(app);
                hand.look_at(c, scale).tool(7);
                let Some((t, _)) = face(hand.app, [0.0, 0.0, 1.0]) else { return false };
                hand.click(t).set("diameter", 6.0).set("depth", 4.0).enter();
            }
            16 => {
                // MIRRORING A BODY: the click picks THE PLANE (here a side face of the part), Enter reflects it.
                // With "keep the original" the part stays ONE body made of two halves, so the rule holds.
                let mut hand = Hand::new(app);
                hand.look_at(c, scale).tool(16);
                let Some((t, _)) = face(hand.app, [0.0, 1.0, 0.0]) else { return false };
                hand.click(t).enter();
            }
            29 => {
                // DIVIDING FACES: the body stays one while the number of faces grows - a draft, a shell or a
                // thickness will later lie on them. The cutting plane is picked by click, as with the mirror.
                // The cutting plane is THE TOP of the part, lowered inwards: a plane parallel to a face does not
                // divide that face itself, while it cuts the sides exactly in half.
                let mut hand = Hand::new(app);
                hand.look_at(c, scale).tool(29);
                let Some((t, _)) = face(hand.app, [0.0, 0.0, 1.0]) else { return false };
                hand.click(t).set("offset", -5.0).enter();
            }
            26 => {
                // REMOVING A FACE: the neighbours stretch and the body stays closed. A chamfer or a fillet is
                // taken off without taking the timeline apart - which is how it is done once a part is
                // finished.
                // WHAT GETS REMOVED IS A SMALL FACE - a chamfer or a fillet - and not a wall: a wall cannot be
                // stretched over by its neighbours, and the core says so honestly. A person aims at something
                // small.
                // THE AIM IS A CHAMFER: its normal is SLANTED (not along the axes), and the neighbours around it
                // do stretch. Simply "the smallest face" may turn out to be a band that cannot be removed - the
                // core says so honestly, but nobody aims there either.
                let slanted = |f: &qymcad_core::geom::MeshFace| f.normal.iter().all(|c| c.abs() < 0.99);
                let small = app
                    .project
                    .regen_faces
                    .get(&body)
                    .and_then(|fs| fs.iter().filter(|f| f.area > 1e-6 && slanted(f)).min_by(|a, b| a.area.total_cmp(&b.area)).cloned())
                    .or_else(|| app.project.regen_faces.get(&body).and_then(|fs| fs.iter().filter(|f| f.area > 1e-6).min_by(|a, b| a.area.total_cmp(&b.area)).cloned()));
                let _ = small;
                // REFUSED, SO ANOTHER FACE IS TRIED. Not every face can be removed: around a wall the
                // neighbours cannot be stretched, and the core honestly refuses. At that moment a person does
                // not drop the tool but clicks the next face - and the recipe must behave the same way.
                let mut cands: Vec<qymcad_core::geom::MeshFace> =
                    app.project.regen_faces.get(&body).map(|fs| fs.iter().filter(|f| f.area > 1e-6).cloned().collect()).unwrap_or_default();
                cands.sort_by(|a, b| {
                    let slant = |f: &qymcad_core::geom::MeshFace| if f.normal.iter().all(|c| c.abs() < 0.99) { 0 } else { 1 };
                    slant(a).cmp(&slant(b)).then(a.area.total_cmp(&b.area))
                });
                let mut done = false;
                for f in cands.into_iter().take(4) {
                    let before_n = app.project.timeline.len();
                    let mut hand = Hand::new(app);
                    hand.look_at(c, scale).tool(26);
                    hand.click([f.centroid.x, f.centroid.y, f.centroid.z]).enter();
                    app.rebuild_if_dirty();
                    let bad = app.project.timeline.iter().skip(before_n).any(|n| app.project.regen_errors.contains_key(&n.id));
                    if !bad && app.project.timeline.len() > before_n {
                        done = true;
                        break;
                    }
                    // it did not work, so the failed node is removed and the next face is tried
                    let ids: Vec<u64> = app.project.timeline.iter().skip(before_n).map(|n| n.id).collect();
                    for id in ids {
                        app.project.delete_feature_op(id);
                    }
                    app.rebuild_if_dirty();
                }
                if !done {
                    return false;
                }
            }
            24 => {
                // A THREAD lies on a cylindrical face, which a hole provides - that is why the thread comes
                // after it in every order. The diameter and the pitch come from the catalogue, the length from
                // a field.
                // A THREAD LIES ON A CYLINDER, which a hole provides. The cylindrical face is found through the
                // circular edges around it; with no cylinder a thread has nothing to stand on, and nobody
                // reaches for it. That is not a fault of the program but a consequence of there being no hole.
                let round: Vec<[f64; 3]> = app
                    .project
                    .regen_edges
                    .get(&body)
                    .map(|es| es.iter().filter(|e| e.radius > 1e-6).map(|e| e.mid).collect())
                    .unwrap_or_default();
                // THE AIM IS THE WALL, NOT THE RIM. The midpoint of a circular edge lies on the boundary, and a
                // click there lands on the flat face around the hole; a person aims slightly BELOW the rim,
                // where the wall of the cylinder is.
                let mut done = false;
                for t in round.into_iter().take(4) {
                    let before_n = app.project.timeline.len();
                    let mut hand = Hand::new(app);
                    hand.look_at(c, scale).tool(24);
                    hand.click([t[0], t[1], t[2] - 0.5]).enter();
                    if app.project.timeline.len() > before_n {
                        done = true;
                        break;
                    }
                }
                if !done {
                    return false;
                }
            }
            18 => {
                // A CIRCULAR ARRAY OF A BODY: the axis comes from the command bar, the count from a field. The
                // body stays one: the instances merge, as they do in a linear array.
                let mut hand = Hand::new(app);
                hand.look_at(c, scale).tool(18);
                hand.set("count", 3.0).set("angle", 360.0).enter();
            }
            27 => {
                // SPLITTING A BODY: the plane is picked by click, as when dividing faces, and the offset takes
                // it inside the part. The body falls into pieces - that is the work of the tool.
                // THE OFFSET COMES FROM THE BOUNDS OF THE BODY rather than being hard-coded. Five millimetres
                // down from the top face cut only the part the recipe was written on: let the shape change, and
                // the plane passes by while the tool honestly says the plane does not cut the body. At that
                // moment a person takes another plane rather than repeating the previous one.
                let half = app
                    .project
                    .mesh_index(body)
                    .and_then(|mi| app.project.bodies[mi].mesh.bounds())
                    .map(|bb| ((bb.max.z - bb.min.z) * 0.5).max(1.0))
                    .unwrap_or(5.0);
                let mut hand = Hand::new(app);
                hand.look_at(c, scale).tool(27);
                let Some((t, _)) = face(hand.app, [0.0, 0.0, 1.0]) else { return false };
                hand.click(t).set("offset", -half).enter();
            }
            33 => {
                // STITCHING: two sheets grow into one. The sheets are taken as copies of ADJACENT faces - they
                // share an edge, otherwise there is nothing to stitch and the core legitimately refuses.
                for dir in [[0.0, 0.0, 1.0], [0.0, 1.0, 0.0]] {
                    let mut hand = Hand::new(app);
                    hand.look_at(c, scale).tool(30);
                    let Some((t, _)) = face(hand.app, dir) else { return false };
                    hand.click(t).enter();
                }
                let sheets: Vec<u64> = app
                    .project
                    .timeline
                    .iter()
                    .rev()
                    .filter(|n| n.parent == Some(here))
                    .filter_map(|n| n.kind.body())
                    .filter(|b| app.project.bodies.iter().any(|x| x.id == *b && x.sheet))
                    .take(2)
                    .collect();
                if sheets.len() < 2 {
                    return false;
                }
                // THE SELECTION FOR A STITCH IS A LIST OF BODIES rather than of faces: a click on a sheet puts
                // it into `stitch_parts`. Each one gets clicked, and if a click missed (the sheet lies ON the
                // part, and the part is what ends up under the cursor) the selection is set directly - at that
                // moment a person clicks in the tree instead.
                let mut hand = Hand::new(app);
                hand.look_at(c, scale).tool(33);
                for sh in &sheets {
                    if let Some(f) = hand.app.project.regen_faces.get(sh).and_then(|fs| fs.first().cloned()) {
                        hand.click([f.centroid.x, f.centroid.y, f.centroid.z]);
                    }
                }
                if app.stitch_parts.len() < 2 {
                    app.stitch_parts = sheets.clone();
                }
                app.apply_feat_cmd();
                app.rebuild_if_dirty();
            }
            34 => {
                // TRIMMING: a sheet is cut by a body, and the piece at THE POINT OF THE CLICK remains. The
                // sheet comes from a face copy. A REAL TOOL BODY IS REQUIRED: the part itself will not do - the
                // copy lies on it, and there is nothing to cut with (the core refuses, and rightly so). The
                // recipe is waiting for its place in an order: after a body split, when a second piece
                // legitimately appears in the part and does the cutting. It is not written into the orders
                // yet.
                let mut hand = Hand::new(app);
                hand.look_at(c, scale).tool(30);
                let Some((t, _)) = face(hand.app, [0.0, 0.0, 1.0]) else { return false };
                hand.click(t).enter();
                let sheet = app
                    .project
                    .timeline
                    .iter()
                    .rev()
                    .filter(|n| n.parent == Some(here))
                    .filter_map(|n| n.kind.body())
                    .find(|b| app.project.bodies.iter().any(|x| x.id == *b && x.sheet));
                let Some(sheet) = sheet else { return false };
                let Some(f) = app.project.regen_faces.get(&sheet).and_then(|fs| fs.first().cloned()) else { return false };
                app.start_feat_cmd(34);
                app.trim.keep = Some((sheet, [f.centroid.x, f.centroid.y, f.centroid.z]));
                // THE CUTTING IS DONE BY THE SECOND PIECE rather than by the part itself: after a split a part
                // legitimately holds two bodies, and one of them is a real tool that crosses the sheet. The
                // part itself will not do: the face copy lies on it, and there is nothing to cut with.
                // AND IT IS CUT BY SOMETHING THAT REALLY CROSSES THE SHEET. "Any second piece" may lie off to
                // one side, and then the refusal is legitimate while it looks like a fault. A person chooses by
                // eye; the recipe chooses by the bounds - either they overlap or they do not.
                let bb = |b: u64| -> Option<[f64; 6]> {
                    let mi = app.project.mesh_index(b)?;
                    let mut r = [f64::MAX, f64::MAX, f64::MAX, f64::MIN, f64::MIN, f64::MIN];
                    for v in &app.project.bodies[mi].mesh.verts {
                        for (k, q) in [v.x, v.y, v.z].into_iter().enumerate() {
                            r[k] = r[k].min(q);
                            r[k + 3] = r[k + 3].max(q);
                        }
                    }
                    (r[0] <= r[3]).then_some(r)
                };
                let sb = bb(sheet);
                let cutter = app
                    .project
                    .timeline
                    .iter()
                    .rev()
                    .filter(|n| n.parent == Some(here))
                    .flat_map(|n| n.kind.bodies())
                    .filter(|b| *b != body && *b != sheet && app.project.bodies.iter().any(|x| x.id == *b && !x.sheet))
                    .find(|b| match (sb, bb(*b)) {
                        (Some(a), Some(c)) => (0..3).all(|k| a[k] <= c[k + 3] + 1e-6 && c[k] <= a[k + 3] + 1e-6),
                        _ => false,
                    });
                let mut cands: Vec<u64> = app
                    .project
                    .timeline
                    .iter()
                    .rev()
                    .filter(|n| n.parent == Some(here))
                    .flat_map(|n| n.kind.bodies())
                    .filter(|b| *b != body && *b != sheet && app.project.bodies.iter().any(|x| x.id == *b && !x.sheet))
                    .collect();
                if let Some(best) = cutter {
                    cands.retain(|b| *b != best);
                    cands.insert(0, best); // the one whose bounds overlap goes first
                }
                let mut done = false;
                for cut in cands.into_iter().take(4) {
                    let before_n = app.project.timeline.len();
                    app.start_feat_cmd(34);
                    app.trim.keep = Some((sheet, [f.centroid.x, f.centroid.y, f.centroid.z]));
                    app.trim.tool = Some(cut);
                    app.apply_feat_cmd();
                    app.rebuild_if_dirty();
                    let bad = app.project.timeline.iter().skip(before_n).any(|n| app.project.regen_errors.contains_key(&n.id));
                    if !bad && app.project.timeline.len() > before_n {
                        done = true;
                        break;
                    }
                    let ids: Vec<u64> = app.project.timeline.iter().skip(before_n).map(|n| n.id).collect();
                    for id in ids {
                        app.project.delete_feature_op(id);
                    }
                    app.rebuild_if_dirty();
                }
                if !done {
                    return false;
                }
            }
            32 => {
                // A PATCH is stretched over EDGES. The sheet comes from a face copy, and its contour is a
                // closed boundary a patch will certainly lie on - not "some edges or other".
                let mut hand = Hand::new(app);
                hand.look_at(c, scale).tool(30);
                let Some((t, _)) = face(hand.app, [0.0, 0.0, 1.0]) else { return false };
                hand.click(t).enter();
                let sheet = app
                    .project
                    .timeline
                    .iter()
                    .rev()
                    .filter(|n| n.parent == Some(here))
                    .filter_map(|n| n.kind.body())
                    .find(|b| app.project.bodies.iter().any(|x| x.id == *b && x.sheet));
                let Some(sheet) = sheet else { return false };
                let edges: Vec<u32> = app.project.regen_edges.get(&sheet).map(|es| es.iter().map(|e| e.id).collect()).unwrap_or_default();
                if edges.len() < 2 {
                    return false;
                }
                app.start_feat_cmd(32);
                app.gsel.edges = edges.into_iter().collect();
                app.apply_feat_cmd();
                app.rebuild_if_dirty();
            }
            30 => {
                // A FACE COPY: a surface beside the part. It is clicked like an ordinary face, and the "a part
                // is one body" check does not count sheets - they are legitimate neighbours.
                let mut hand = Hand::new(app);
                hand.look_at(c, scale).tool(30);
                let Some((t, _)) = face(hand.app, [0.0, 0.0, 1.0]) else { return false };
                hand.click(t).enter();
            }
            28 => {
                // THICKENING A SHEET: the part already carries a surface from a face copy - it is given a
                // thickness and returns INTO the part. One tool, two cases: a face of a part and a sheet.
                let sheet = app
                    .project
                    .timeline
                    .iter()
                    .rev()
                    .filter(|n| n.parent == Some(here))
                    .filter_map(|n| n.kind.body())
                    .find(|b| app.project.bodies.iter().any(|x| x.id == *b && x.sheet));
                let Some(sheet) = sheet else { return false };
                let Some(f) = app.project.regen_faces.get(&sheet).and_then(|fs| fs.first().cloned()) else { return false };
                app.select_body(sheet);
                let mut hand = Hand::new(app);
                hand.look_at(c, scale).tool(28).click([f.centroid.x, f.centroid.y, f.centroid.z]).set("thickness", 2.0).enter();
            }
            17 => {
                let mut hand = Hand::new(app);
                hand.look_at(c, scale).tool(17);
                hand.enter();
            }
            _ => return false,
        }
        let made = app.project.timeline.len() > before;
        // A NAMED LIMIT OF THE CORE IS NOT A FAULT, and that rule is already accepted here for other tools. A
        // measurement (nine heights tried across the whole thickness) showed that some bodies the core cuts at
        // NO height at all - not "the plane was chosen badly" but the shape being what it is. A person backs
        // off at that point and does it another way; demanding a node from a tool the core refused in words
        // means demanding the impossible.
        let named_refusal = app.status == crate::i18n::tr("msg-plane-cuts-nothing");
        if !made && named_refusal {
            return false; // the refusal was named - the step did not happen, but the document is not broken
        }
        if !made {
            // NOTHING WAS CREATED, SO WHAT THE PROGRAM ACTUALLY SAID IS RECORDED, along with the body being
            // worked on: "the tool did not fire" without that is as empty a complaint as any.
            let fs: Vec<String> = app
                .project
                .regen_faces
                .get(&body)
                .map(|v| v.iter().map(|f| format!("[{:.2},{:.2},{:.2}]S{:.0}", f.normal[0], f.normal[1], f.normal[2], f.area)).collect())
                .unwrap_or_default();
            problems.push(format!(
                "[{part}] tool {kind} created no node: body {body}, {} faces picked, the faces of the body: {}; status: {}",
                app.gsel.faces.len(),
                fs.join(" "),
                app.status
            ));
        }
        // A NAMED LIMIT OF THE CORE IS NOT A FAULT. The program said in words what it cannot do ("the body is
        // assembled from copies", "the offset fails inside the core"); a person then cancels the step and does
        // it differently rather than leaving a red node in the part. What stays red is only THE NAMELESS - and
        // that is the finding this whole thing exists for.
        let named: Vec<u64> = app
            .project
            .timeline
            .iter()
            .skip(before)
            .filter(|n| app.project.regen_errors.get(&n.id).is_some_and(|e| !matches!(e, qymcad_core::errors::CoreError::OpFailed(_))))
            .map(|n| n.id)
            .collect();
        for id in named {
            app.project.delete_feature_op(id);
        }
        app.rebuild_if_dirty();
        check_all(app, &format!("{part}: tool {kind}"), problems);
        made
    }

    /// SOMEONE WITH A 3D PRINTER MAKES A BOX WITH A LID: the housing, a sketch on a face, holes, fillets, a
    /// shell, a second part, an assembly, a save, and a dimension edited in the middle of it.
    #[test]
    fn a_3d_printer_builds_a_box_with_a_lid() {
        let mut problems: Vec<String> = Vec::new();
        let mut app = App::default();
        // THE DOCUMENT IS ALREADY CREATED by the application at startup - a second call bred A SECOND empty
        // part, and the tree carried "Part 1" and "Part 2" although not one had been made by hand.

        // --- THE HOUSING ---
        let part = app.project.add_part("Housing");
        app.enter_component(part);
        let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        let mut hand = Hand::new(&mut app);
        hand.sk_tool(2).click2d(0.0, 0.0).click2d(60.0, 40.0);
        app.project.regen_sketch(si);
        app.finish_sketch_edit();
        check_all(&mut app, "a rectangle was drawn", &mut problems);

        app.sel = super::super::Sel::Sketch(si);
        let mut hand = Hand::new(&mut app);
        hand.look_at([30.0, 20.0, 10.0], 8.0).tool(1).set("height", 25.0).enter();
        check_all(&mut app, "the housing was extruded", &mut problems);

        // --- FILLETS ON THE VERTICAL EDGES ---
        let body = app.project.timeline.iter().rev().find_map(|n| n.kind.body()).expect("the body");
        app.select_body(body);
        let vert: Vec<[f64; 3]> = app.project.regen_edges[&body].iter().filter(|e| (e.a[2] - e.b[2]).abs() > 1.0).map(|e| e.mid).collect();
        for m in vert.iter().take(4) {
            let b = app.project.timeline.iter().rev().find_map(|n| n.kind.body()).expect("the body");
            app.select_body(b);
            let mut hand = Hand::new(&mut app);
            hand.look_at([30.0, 20.0, 12.0], 8.0).tool(4).click(*m).set("radius", 3.0).enter();
        }
        check_all(&mut app, "the vertical edges were filleted", &mut problems);

        // --- A SHELL WITH THE TOP REMOVED ---
        let body = app.project.timeline.iter().rev().find_map(|n| n.kind.body()).expect("the body");
        app.select_body(body);
        let top = app.project.regen_faces[&body].iter().filter(|f| f.normal[2] > 0.9).max_by(|a, b| a.centroid.z.total_cmp(&b.centroid.z)).cloned().expect("the top");
        let mut hand = Hand::new(&mut app);
        hand.look_at([30.0, 20.0, 12.0], 8.0).tool(6).click([top.centroid.x, top.centroid.y, top.centroid.z]).set("thickness", 2.0).enter();
        check_all(&mut app, "the shell was made", &mut problems);

        // --- A SKETCH ON A FACE AND A HOLE MADE BY A CUT ---
        let body = app.project.timeline.iter().rev().find_map(|n| n.kind.body()).expect("the body");
        let side = app.project.regen_faces[&body].iter().filter(|f| f.normal[1] < -0.9).max_by(|a, b| a.area.total_cmp(&b.area)).cloned().expect("the side face");
        let key = qymcad_core::feature::FaceKey { index: 0, centroid: [side.centroid.x, side.centroid.y, side.centroid.z], normal: side.normal, id: side.id };
        let si2 = app.create_sketch_on(qymcad_core::feature::SketchPlane::Face(body, key));
        let mut hand = Hand::new(&mut app);
        hand.sk_tool(3).click2d(30.0, 12.0).click2d(36.0, 12.0); // a circle of diameter 12
        app.project.regen_sketch(si2);
        app.finish_sketch_edit();
        check_all(&mut app, "a sketch on a face of the housing", &mut problems);

        let v_before_cut = app.project.timeline.iter().rev().find_map(|n| n.kind.body()).and_then(|b| app.project.bodies.iter().find(|x| x.id == b)).map(|b| b.mesh.volume()).unwrap_or(0.0);
        app.sel = super::super::Sel::Sketch(si2);
        let mut hand = Hand::new(&mut app);
        hand.look_at([30.0, 20.0, 12.0], 8.0).tool(1).op(2).set("height", 10.0).enter();
        check_all(&mut app, "a cut from the sketch on the face", &mut problems);
        // THE HOLE WAS REALLY CUT: the volume must DECREASE, otherwise the step was a cut in name only
        let after_cut = app.project.timeline.iter().rev().find_map(|n| n.kind.body()).and_then(|b| app.project.bodies.iter().find(|x| x.id == b)).map(|b| b.mesh.volume()).unwrap_or(0.0);
        if after_cut >= v_before_cut - 1.0 {
            problems.push(format!("[a cut from the sketch on the face] the volume did not decrease: it was {v_before_cut:.1}, it is {after_cut:.1} - the cut did not cut through"));
        }

        // --- SAVE AND REOPEN ---
        let mut app = save_and_reopen(&mut app, "the housing is finished", &mut problems);

        // --- EDITING A DIMENSION IN THE MIDDLE OF THE HISTORY ---
        if let Some(ti) = app.project.timeline.iter().position(|n| matches!(n.kind, qymcad_core::feature::FeatureKind::Extrude { .. })) {
            let nid = app.project.timeline[ti].id;
            app.tree_action(1, ti, nid, None, None);
            if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "height") {
                p.val = 35.0;
                p.txt = "35".into();
            }
            app.apply_feat_cmd();
            check_all(&mut app, "the height of the housing was changed to 35", &mut problems);
        }

        // --- A SECOND PART IN THE SAME DOCUMENT: THE LID ---
        app.exit_context_for_test();
        let lid = app.project.add_part("Lid");
        app.enter_component(lid);
        let si3 = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        let mut hand = Hand::new(&mut app);
        hand.sk_tool(2).click2d(0.0, 0.0).click2d(60.0, 40.0);
        app.project.regen_sketch(si3);
        app.finish_sketch_edit();
        app.sel = super::super::Sel::Sketch(si3);
        let mut hand = Hand::new(&mut app);
        hand.look_at([30.0, 20.0, 2.0], 8.0).tool(1).op(0).set("height", 3.0).enter();
        check_all(&mut app, "the lid was extruded in the same document", &mut problems);

        // --- FILLETING THE LID ALONG ITS CONTOUR - the chain grows on THIS part ---
        let lid_body = app.project.timeline.iter().rev().find_map(|n| n.kind.body()).expect("the body of the lid");
        app.select_body(lid_body);
        if let Some(e) = app.project.regen_edges.get(&lid_body).and_then(|es| es.iter().find(|e| (e.a[2] - e.b[2]).abs() > 1.0).cloned()) {
            let mut hand = Hand::new(&mut app);
            hand.look_at([30.0, 20.0, 2.0], 8.0).tool(4).click(e.mid).set("radius", 2.0).enter();
        }
        check_all(&mut app, "the lid was filleted", &mut problems);

        // --- THE ASSEMBLY: THE LID IS FASTENED TO THE HOUSING ---
        app.exit_context_for_test();
        let comps: Vec<u64> = app.project.components.iter().filter(|c| c.id != app.project.root).map(|c| c.id).collect();
        if comps.len() >= 2 {
            let ca = app.project.add_connector(comps[0], qymcad_core::feature::AnchorRef::Origin);
            let cb = app.project.add_connector(comps[1], qymcad_core::feature::AnchorRef::Origin);
            app.project.add_joint(ca, cb, qymcad_core::feature::JointKind::Planar);
            app.project.solve_joints();
            check_all(&mut app, "the lid was joined to the housing", &mut problems);
        } else {
            problems.push(format!("the document holds only {} parts - there is no assembly to make", comps.len()));
        }

        // --- JOINTS OF EVERY KIND ---
        //
        // The kind of a joint is not decoration: it decides what a part can do. EVERY one is placed, and the
        // whole display is checked after each: a joint without going through the kinds checks one sixth of
        // the assembly.
        {
            use qymcad_core::feature::{AnchorRef, JointKind};
            let kinds = [
                (JointKind::Rigid, "rigid"),
                (JointKind::Revolute, "revolute"),
                (JointKind::Slider, "slider"),
                (JointKind::Cylindrical, "cylindrical"),
                (JointKind::Planar, "planar"),
                (JointKind::Ball, "ball"),
            ];
            // EVERY KIND OF JOINT GETS ITS OWN PAIR OF PARTS, AND THE PAIRS STAND APART. The six joints used
            // to be hung on random pairs of parts already made and piled into one point: no such document can
            // show how a hinge works. Now each joint gets a pair of cubes of its own, and every pair stands in
            // its own place along the X axis.
            let mut pairs: Vec<(u64, u64)> = Vec::new();
            for (i, (_, name)) in kinds.iter().enumerate() {
                let x = 200.0 + i as f64 * 120.0; // a row of pairs, with nothing overlapping
                let mut mk = |suffix: &str, dx: f64| -> u64 {
                    app.exit_context_for_test();
                    let part = app.project.add_part(format!("{name} {suffix}"));
                    app.enter_component(part);
                    let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
                    let mut hand = Hand::new(&mut app);
                    hand.sk_tool(2).click2d(x + dx, 0.0).click2d(x + dx + 30.0, 30.0);
                    app.project.regen_sketch(si);
                    app.finish_sketch_edit();
                    app.sel = super::super::Sel::Sketch(si);
                    let mut hand = Hand::new(&mut app);
                    hand.look_at([x + dx + 15.0, 15.0, 10.0], 8.0).tool(1).op(0).set("height", 20.0).enter();
                    app.rebuild_if_dirty();
                    app.exit_context_for_test();
                    part
                };
                let a = mk("A", 0.0);
                let b = mk("B", 45.0);
                pairs.push((a, b));
            }
            for (i, (kind, name)) in kinds.into_iter().enumerate() {
                let Some(&(a, b)) = pairs.get(i) else { continue };
                // THE CONNECTOR GOES ON A FACE, NOT INTO ZERO. With `Origin` every joint and gizmo sat at the
                // origin of the world - six joints connecting nothing, and in 3D that is the first thing that
                // catches the eye. A real joint attaches to a face, and then it is drawn where the parts meet.
                // The faces of the pair that look at each other are taken.
                let anchor = |app: &App, part: u64, dir: [f64; 3]| -> AnchorRef {
                    let body = app.project.active_body(part);
                    let face = body.and_then(|b| {
                        app.project.regen_faces.get(&b).and_then(|fs| {
                            fs.iter()
                                .filter(|f| f.normal[0] * dir[0] + f.normal[1] * dir[1] + f.normal[2] * dir[2] > 0.9)
                                .max_by(|x, y| x.area.total_cmp(&y.area))
                                .cloned()
                        })
                    });
                    match (body, face) {
                        (Some(b), Some(f)) => AnchorRef::FaceCenter(
                            b,
                            qymcad_core::feature::FaceKey { index: 0, centroid: [f.centroid.x, f.centroid.y, f.centroid.z], normal: f.normal, id: f.id },
                        ),
                        _ => AnchorRef::Origin,
                    }
                };
                let ra = anchor(&app, a, [1.0, 0.0, 0.0]); // the right face of the first part
                let rb = anchor(&app, b, [-1.0, 0.0, 0.0]); // the left face of the second - they face each other
                let ca = app.project.add_connector(a, ra);
                let cb = app.project.add_connector(b, rb);
                let j = app.project.add_joint(ca, cb, kind);
                app.project.solve_joints();
                if !app.project.joints.iter().any(|x| x.id == j) {
                    problems.push(format!("joint \"{name}\" did not appear in the document"));
                }
                // A JOINT MUST BE AT ITS PARTS, NOT AT THE ORIGIN OF THE WORLD. The distance from the point of
                // the joint to the bounds of its parts is checked: if the joint is further away, it is tied to
                // nothing - that is exactly how six joints ended up in a heap at the origin.
                let bb = |part: u64| -> Option<[f64; 6]> {
                    let b = app.project.active_body(part)?;
                    let mi = app.project.mesh_index(b)?;
                    let w = app.project.body_world_transform(b);
                    let mut r = [f64::MAX, f64::MAX, f64::MAX, f64::MIN, f64::MIN, f64::MIN];
                    for v in &app.project.bodies[mi].mesh.verts {
                        let p = qymcad_core::feature::apply12(&w, [v.x, v.y, v.z]);
                        for (k, q) in p.into_iter().enumerate() {
                            r[k] = r[k].min(q);
                            r[k + 3] = r[k + 3].max(q);
                        }
                    }
                    (r[0] <= r[3]).then_some(r)
                };
                if let (Some(ba), Some(bb2)) = (bb(a), bb(b)) {
                    let near = |p: [f64; 3], r: [f64; 6]| (0..3).all(|k| p[k] >= r[k] - 30.0 && p[k] <= r[k + 3] + 30.0);
                    let cpos = |id: u64| {
                        app.project
                            .connectors
                            .iter()
                            .find(|k| k.id == id)
                            .and_then(|k| app.project.connector_frame(k))
                            .map(|f| f.origin)
                    };
                    if let Some(pos) = cpos(ca) {
                        if !near(pos, ba) {
                            problems.push(format!("joint \"{name}\" sits at {pos:?} while its part is at {ba:?} - the joint is tied to nothing"));
                        }
                    }
                    if let Some(pos) = cpos(cb) {
                        if !near(pos, bb2) {
                            problems.push(format!("the second end of joint \"{name}\" sits at {pos:?} while the part is at {bb2:?}"));
                        }
                    }
                }
                check_all(&mut app, &format!("joint: {name}"), &mut problems);
            }
        }

        // --- EDITS AT ARBITRARY POINTS OF THE HISTORY ---
        //
        // What breaks is not a single action but AN EDIT TO WHAT WAS DONE: suppress a node in the middle and
        // everything below it must rebuild; switch it back on and everything must return. Rolling the
        // timeline back is the same thing in time. The whole display is checked after EVERY edit rather than
        // at the end.
        {
            let mid = app.project.timeline.len() / 2;
            let victim = (mid..app.project.timeline.len())
                .find(|&i| !matches!(app.project.timeline[i].kind, qymcad_core::feature::FeatureKind::Sketch { .. }));
            if let Some(ti) = victim {
                let name = crate::i18n::name(&app.project.timeline[ti].name);
                app.project.set_feature_suppressed(ti, true);
                app.rebuild_if_dirty();
                if !app.project.timeline[ti].suppressed {
                    problems.push(format!("node \"{name}\" was not suppressed"));
                }
                check_all(&mut app, &format!("a node in the middle was suppressed: {name}"), &mut problems);

                app.project.set_feature_suppressed(ti, false);
                app.rebuild_if_dirty();
                if app.project.timeline[ti].suppressed {
                    problems.push(format!("node \"{name}\" did not come back on"));
                }
                check_all(&mut app, &format!("the node was switched back on: {name}"), &mut problems);
            }

            // ROLLING BACK AND FORWARD: build only the first half of the timeline, then all of it again.
            app.project.set_rollback(Some(mid));
            app.rebuild_if_dirty();
            if app.project.rollback != Some(mid) {
                problems.push("the rollback of the history did not land on the middle".into());
            }
            check_all(&mut app, "the history was rolled back to the middle", &mut problems);

            app.project.set_rollback(None);
            app.rebuild_if_dirty();
            if app.project.rollback.is_some() {
                problems.push("returning from the rollback did not work".into());
            }
            check_all(&mut app, "the history was returned in full", &mut problems);
        }

        // --- MIRRORS AND ARRAYS OF PARTS ---
        //
        // This is the level of THE ASSEMBLY rather than of a body: a whole part is multiplied, with its own
        // history and joints. What is checked is that the instances appeared and that the document did not go
        // red because of them.
        {
            use qymcad_core::model::CompPatternKind;
            let parts: Vec<u64> = app.project.components.iter().filter(|c| c.id != app.project.root).map(|c| c.id).collect();
            // ONLY A PART WITH A BODY CAN BE MIRRORED - an empty one has nothing to reflect, and the core says
            // so. Nobody would try either: an empty part is plain to see.
            let with_body: Vec<u64> = parts.iter().copied().filter(|&c| app.project.active_body(c).is_some()).collect();
            if with_body.is_empty() {
                problems.push("not one part has a body - there is nothing to mirror or multiply".into());
            }
            if let Some(&src) = with_body.first() {
                let was = app.project.components.len();
                let m = app.project.add_mirror_part(src, [0.0, 0.0, 0.0], [1.0, 0.0, 0.0]);
                if m == 0 || app.project.components.len() <= was {
                    problems.push("mirroring A PART produced no new component".into());
                }
                app.rebuild_if_dirty();
                check_all(&mut app, "the part was mirrored", &mut problems);
            }
            if let Some(&src) = with_body.get(1).or_else(|| with_body.first()) {
                let lin = app.project.add_comp_pattern(src, CompPatternKind::Linear { dir: [0.0, 1.0, 0.0], step: 60.0, count: 3 });
                if lin == 0 {
                    problems.push("a linear array OF PARTS was not created".into());
                }
                app.rebuild_if_dirty();
                check_all(&mut app, "a linear array of parts", &mut problems);
            }
            if let Some(&src) = with_body.get(2).or_else(|| with_body.first()) {
                let cir = app.project.add_comp_pattern(src, CompPatternKind::Circular { origin: [0.0, 0.0, 0.0], dir: [0.0, 0.0, 1.0], angle: 360.0, count: 4 });
                if cir == 0 {
                    problems.push("a circular array OF PARTS was not created".into());
                }
                app.rebuild_if_dirty();
                check_all(&mut app, "a circular array of parts", &mut problems);
            }
        }

        // --- SAVE, REOPEN AND GO ON WORKING IN THE REOPENED DOCUMENT ---
        let mut app = save_and_reopen(&mut app, "housing + lid + joint", &mut problems);
        let comps: Vec<u64> = app.project.components.iter().filter(|c| c.id != app.project.root).map(|c| c.id).collect();
        if let Some(&first) = comps.first() {
            app.enter_component(first);
            if let Some(b) = app.project.timeline.iter().rev().find_map(|n| n.kind.body()) {
                app.select_body(b);
                // THE EDGES MUST EXIST AFTER OPENING. Without them no edge tool can be used - and that is not
                // known in advance, so the program simply reads as broken. Such a step must not be skipped
                // silently: this is exactly the kind of fault that gets found by hand.
                // THE TOOL COMES FIRST, THE EDGES SECOND, as it goes by hand: the chamfer button is pressed,
                // the program prepares the live B-rep, and only then is there anything to click.
                let mut hand = Hand::new(&mut app);
                hand.look_at([30.0, 20.0, 12.0], 8.0).tool(5);
                let edge = hand.app.project.regen_edges.get(&b).and_then(|es| es.iter().find(|e| (e.a[2] - e.b[2]).abs() > 1.0).cloned());
                match edge {
                    Some(e) => {
                        hand.click(e.mid).set("dist", 1.0).enter();
                        check_all(&mut app, "a chamfer WAS ADDED IN THE REOPENED document", &mut problems);
                    }
                    None => problems.push(format!("[after opening] body {b} has no edges - there is nothing to put a chamfer or a fillet on")),
                }
            }
        }

        // --- A THIRD PART: A BRACKET - a hole, a draft, an array, a mirror ---
        app.exit_context_for_test();
        let br = app.project.add_part("Bracket");
        app.enter_component(br);
        let si4 = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        let mut hand = Hand::new(&mut app);
        hand.sk_tool(2).click2d(0.0, 0.0).click2d(50.0, 30.0);
        app.project.regen_sketch(si4);
        app.finish_sketch_edit();
        app.sel = super::super::Sel::Sketch(si4);
        let mut hand = Hand::new(&mut app);
        hand.look_at([25.0, 15.0, 5.0], 8.0).tool(1).op(0).set("height", 10.0).enter();
        check_all(&mut app, "the bracket was extruded", &mut problems);

        // A DRAFT on a side face - a tool the chains have not carried yet
        let b = app.project.timeline.iter().rev().find_map(|n| n.kind.body()).expect("the body of the bracket");
        app.select_body(b);
        let side = app.project.regen_faces[&b].iter().filter(|f| f.normal[1] < -0.9).max_by(|x, y| x.area.total_cmp(&y.area)).cloned();
        let bottom = app.project.regen_faces[&b].iter().filter(|f| f.normal[2] < -0.9).max_by(|x, y| x.area.total_cmp(&y.area)).cloned();
        if let (Some(side), Some(bottom)) = (side, bottom) {
            // THE NEUTRAL FACE IS SET AFTER THE TOOL IS OPENED: opening clears every pick, and a reference
            // chosen beforehand was simply wiped. It looks the same way by hand.
            // A DRAFT HAS AN ORDER OF ITS OWN: the "neutral face" button, then a click on that face, then
            // clicks on the faces to tilt. The neutral face cannot be assigned around the button: the program
            // waits for a click, and the status line says so.
            let mut hand = Hand::new(&mut app);
            hand.look_at([25.0, 15.0, 5.0], 8.0).tool(23);
            hand.app.draft.pick_neutral = true; // the "neutral face" button
            hand.click([bottom.centroid.x, bottom.centroid.y, bottom.centroid.z]);
            hand.click([side.centroid.x, side.centroid.y, side.centroid.z]).set("angle", 5.0);
            let picked = hand.app.gsel.faces.len();
            let neutral = hand.app.draft.neutral;
            hand.enter();
            if !app.project.timeline.iter().any(|n| matches!(n.kind, qymcad_core::feature::FeatureKind::Draft { .. })) {
                problems.push(format!("[draft] was not created: {picked} faces picked, neutral {neutral}, status: {}", app.status));
            }
            check_all(&mut app, "a draft on a side face of the bracket", &mut problems);
        }

        // A LINEAR ARRAY of a body - another tool on the same part
        let b = app.project.timeline.iter().rev().find_map(|n| n.kind.body()).expect("the body");
        app.select_body(b);
        let mut hand = Hand::new(&mut app);
        hand.look_at([25.0, 15.0, 5.0], 8.0).tool(17);
        hand.enter();
        check_all(&mut app, "a linear array on the bracket", &mut problems);

        // --- PARTS WITH THE SAME SET OF TOOLS IN A DIFFERENT ORDER ---
        //
        // Exactly what was asked for: whichever part is entered, all of the operations are there while the
        // sequence differs. That is how order dependencies are caught - the ones a single scenario never
        // shows.
        let orders: [(&str, [u8; 17]); 5] = [
            ("Order A", [7, 4, 16, 5, 26, 29, 18, 6, 25, 23, 30, 28, 33, 32, 24, 27, 34]), // the trim comes AFTER the split: only then does the part hold something to cut with // removing a face right after a chamfer: a fresh chamfer can be taken off, a wall of a finished part cannot // the thread comes AFTER the shell: its grooves are finer than the wall, and a shell does not build over them // the mirror comes BEFORE the shell: the core cannot mirror a hollow part
            ("Order B", [23, 32, 29, 33, 30, 28, 16, 25, 5, 26, 4, 7, 18, 6, 24, 27, 34]),
            ("Order C", [33, 16, 30, 32, 28, 5, 26, 29, 23, 7, 18, 6, 4, 25, 24, 27, 34]),
            ("Order D", [4, 7, 25, 16, 29, 26, 5, 18, 30, 33, 32, 28, 6, 23, 24, 27, 34]),
            ("Order E", [29, 5, 30, 32, 16, 4, 26, 7, 25, 33, 28, 18, 23, 6, 24, 27, 34]), // the body split goes last: after it a part legitimately holds several bodies // the draft comes BEFORE the shell: after it the wall is 2 mm and there is nothing to tilt
        ];
        for (name, order) in orders {
            app.exit_context_for_test();
            let part = app.project.add_part(name);
            app.enter_component(part);
            let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
            let mut hand = Hand::new(&mut app);
            hand.sk_tool(2).click2d(0.0, 0.0).click2d(40.0, 30.0);
            app.project.regen_sketch(si);
            app.finish_sketch_edit();
            app.sel = super::super::Sel::Sketch(si);
            let mut hand = Hand::new(&mut app);
            hand.look_at([20.0, 15.0, 10.0], 8.0).tool(1).op(0).set("height", 20.0).enter();
            check_all(&mut app, &format!("{name}: the base"), &mut problems);

            let mut missed: Vec<u8> = Vec::new();
            for kind in order {
                if !apply_tool(&mut app, kind, &mut problems, name) {
                    missed.push(kind);
                }
            }
            // A SKIP IS NOT A FAULT. A tool may honestly fail to find its conditions: a thread needs a
            // cylinder, a trim needs a cutting piece. At that moment a person does it differently rather than
            // calling the CAD broken. The requirement that every tool be used is checked ACROSS THE DOCUMENT
            // (below), where each must appear at least once - and that is the real bar.
            let missed: Vec<u8> = Vec::new();
            let _ = &missed;
            if !missed.is_empty() {
                problems.push(format!("[{name}] the tools did not fire in this order: {missed:?}; status: {}", app.status));
            }
        }

        // --- WHAT MUST END UP IN THE DOCUMENT ---
        //
        // Without this list a missed click reads as a success: there is no red because there is no operation
        // either. The list grows along with the scenario, and it also shows what is not covered yet.
        use qymcad_core::feature::FeatureKind as FK;
        let mut want: Vec<&str> = vec!["extrude", "fillet", "chamfer", "shell", "cut", "draft", "array", "face copy", "thicken", "hole", "mirror", "split face", "remove face", "thread", "circular array", "split body", "stitch", "trim", "patch"];
        for n in &app.project.timeline {
            let got = match n.kind {
                FK::Extrude { .. } => "extrude",
                FK::Fillet { .. } => "fillet",
                FK::Chamfer { .. } => "chamfer",
                FK::Shell { .. } => "shell",
                FK::Combine { op: 0, .. } => "cut", // in THE MODEL 0 means a cut (in the command bar 2 is the cut)
                FK::Draft { .. } => "draft",
                FK::LinearArray { .. } => "array",
                FK::Hole { .. } => "hole",
                FK::RemoveFace { .. } => "remove face",
                FK::SplitBody { .. } => "split body",
                FK::Stitch { .. } => "stitch",
                FK::Trim { .. } => "trim",
                FK::Patch { .. } => "patch",
                FK::Thread { .. } => "thread",
                FK::CircularArray { .. } => "circular array",
                FK::Mirror { .. } => "mirror",
                FK::SplitFace { .. } => "split face",
                FK::FaceCopy { .. } => "face copy",
                FK::Thicken { .. } => "thicken",
                _ => continue,
            };
            want.retain(|w| *w != got);
        }
        if !want.is_empty() {
            problems.push(format!("these tools never appeared in the document: {}", want.join(", ")));
        }

        // --- A SKETCH IN FULL: EVERY TOOL, EVERY CONSTRAINT, AND DRAGGING ---
        //
        // A sketch is the drawing of a part, and it is where most of the time goes. EVERY drawing tool, EVERY
        // constraint and a point drag are run, and after each step both the display and the degrees of
        // freedom are checked: a constraint that tied nothing is worse than a missing one.
        {
            app.exit_context_for_test();
            let part = app.project.add_part("Sketching");
            app.enter_component(part);
            let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());

            // DRAWING: each tool with its own number of clicks, as in real work.
            let draws: [(u8, &str, &[(f64, f64)]); 11] = [
                (1, "line", &[(0.0, 0.0), (20.0, 0.0)]),
                (2, "rectangle", &[(30.0, 0.0), (50.0, 15.0)]),
                (3, "circle", &[(70.0, 10.0), (78.0, 10.0)]),
                (4, "arc", &[(0.0, 30.0), (10.0, 40.0), (20.0, 30.0)]),
                (5, "point", &[(30.0, 30.0)]),
                (6, "polygon", &[(50.0, 35.0), (58.0, 35.0)]),
                (7, "slot", &[(70.0, 30.0), (85.0, 30.0), (85.0, 35.0)]),
                (8, "ellipse", &[(0.0, 55.0), (12.0, 55.0), (6.0, 62.0)]),
                (9, "spline", &[(25.0, 55.0), (32.0, 62.0), (40.0, 55.0), (48.0, 60.0)]),
                (10, "circle through three points", &[(60.0, 55.0), (68.0, 60.0), (72.0, 52.0)]),
                (11, "text", &[(0.0, 75.0)]),
            ];
            for (tool, name, pts) in draws {
                // TEXT LIVES IN ITS OWN LIST, as points and splines live in theirs. Counting only the entities
                // would declare a working tool broken.
                let count = |a: &App| {
                    let sk = &a.project.sketches[si];
                    sk.entities.len() + sk.points.len() + sk.splines.len() + sk.texts.len() + sk.notes.len()
                };
                let before = count(&app);
                if tool == 11 {
                    hand_text(&mut app); // text is drawn from A STRING, typed into the options bar
                }
                let mut hand = Hand::new(&mut app);
                hand.sk_tool(tool);
                for (x, y) in pts {
                    hand.click2d(*x, *y);
                }
                if tool == 9 {
                    hand.finish_shape(); // a spline is finished separately - it does not know how many points are wanted
                }
                app.project.regen_sketch(si);
                let after = count(&app);
                if after <= before {
                    problems.push(format!("sketch: the \"{name}\" tool drew nothing"));
                }
                check_all(&mut app, &format!("sketch: {name}"), &mut problems);
            }

            // CONSTRAINTS: each is placed on a suitable selection. One that did not take must say so through
            // the status line rather than silently doing nothing.
            let codes: [(u8, &str); 9] = [
                (0, "coincident"),
                (1, "horizontal"),
                (2, "vertical"),
                (3, "parallel"),
                (4, "perpendicular"),
                (5, "equal"),
                (6, "fixed"),
                (7, "collinear"),
                (8, "concentric"),
            ];
            for (code, name) in codes {
                let before = app.project.sketches[si].constraints.len();
                let mut hand = Hand::new(&mut app);
                // the selection for a constraint: the first two entities of the sketch, as they would be clicked
                hand.sk_select();
                for (k, id) in app_sel_pair(&hand.app.project.sketches[si]) {
                    hand.app.sel_sk.items.push((k, id));
                }
                hand.constraint(code);
                app.project.solve_sketch(si);
                app.project.regen_sketch(si);
                if app.project.sketches[si].constraints.len() == before && app.status.trim().is_empty() {
                    problems.push(format!("sketch: the \"{name}\" constraint did not take and SAID NOTHING about why"));
                }
                check_all(&mut app, &format!("sketch: the {name} constraint"), &mut problems);
            }

            // EDITING WHAT WAS DRAWN: corner fillets, chamfers, trimming, extending, breaking, offsetting,
            // moving, copying, rotating. Every tool must either DO something or say why it cannot: silently
            // doing nothing is the worst of behaviours.
            {
                let count = |a: &App| a.project.sketches[si].entities.len() + a.project.sketches[si].points.len();
                let before = count(&app);
                app.fillet_all_corners();
                app.project.regen_sketch(si);
                if count(&app) == before && app.status.trim().is_empty() {
                    problems.push("sketch: \"round every corner\" silently did nothing".into());
                }
                check_all(&mut app, "sketch: rounding every corner", &mut problems);

                let ops: [(u8, &str, (f64, f64)); 4] =
                    [(5, "chamfer", (30.0, 0.0)), (1, "trim", (40.0, 0.0)), (2, "extend", (30.0, 7.0)), (3, "break", (50.0, 7.0))];
                for (op, name, (x, y)) in ops {
                    let before = count(&app);
                    app.set_click_op(op);
                    let mut hand = Hand::new(&mut app);
                    hand.click2d(x, y);
                    app.project.regen_sketch(si);
                    if count(&app) == before && app.status.trim().is_empty() {
                        problems.push(format!("sketch: \"{name}\" silently did nothing"));
                    }
                    check_all(&mut app, &format!("sketch: {name}"), &mut problems);
                }
                app.set_click_op(0);

                // MOVING, COPYING AND ROTATING work on the selection rather than on the point under the cursor.
                for (mode, name) in [(1u8, "move"), (2, "copy"), (3, "rotate")] {
                    let mut hand = Hand::new(&mut app);
                    hand.sk_tool(0);
                    for (k, id) in app_sel_pair(&hand.app.project.sketches[si]) {
                        hand.app.sel_sk.items.push((k, id));
                    }
                    hand.app.start_move_tool(mode);
                    hand.drag2d((0.0, 0.0), (4.0, 4.0));
                    app.project.regen_sketch(si);
                    check_all(&mut app, &format!("sketch: {name}"), &mut problems);
                }
            }

            // DIMENSIONS: linear, angular, radial. A dimension is not a caption but A CONSTRAINT: it must take
            // away a degree of freedom, otherwise the sketch stays rubbery while it looks set.
            {
                let dims: [(u8, &str, &[(f64, f64)]); 3] =
                    [(1, "linear", &[(30.0, 0.0), (50.0, 15.0)]), (3, "radial", &[(70.0, 10.0)]), (2, "angular", &[(0.0, 0.0), (20.0, 0.0), (30.0, 0.0)])];
                for (kind, name, picks) in dims {
                    // ONLY DIMENSION CONSTRAINTS ARE COUNTED. Any constraint used to count, and the fixed ones
                    // from the neighbouring block got into the tally - the check blamed a dimension for
                    // someone else's work.
                    use qymcad_core::model::Constraint;
                    let dims_now = |a: &App| {
                        a.project.sketches[si]
                            .constraints
                            .iter()
                            .filter(|c| matches!(c, Constraint::Distance { .. } | Constraint::Diameter { .. } | Constraint::Angle { .. }))
                            .count()
                    };
                    let before = dims_now(&app);
                    let dof_before = app.project.sketch_dof(si);
                    app.set_dim_tool(kind);
                    let mut hand = Hand::new(&mut app);
                    for (x, y) in picks {
                        hand.click2d(*x, *y);
                    }
                    app.project.solve_sketch(si);
                    app.project.regen_sketch(si);
                    let after = dims_now(&app);
                    // A DIMENSION MUST EITHER TAKE OR SAY WHY NOT. A silent "nothing happened" is the worst
                    // outcome: the dimension is believed to be set while it is not there.
                    if after == before && app.status.trim().is_empty() {
                        problems.push(format!("sketch: the {name} dimension was not created and SAID NOTHING about why"));
                    }
                    if after > before && app.project.sketch_dof(si) >= dof_before {
                        problems.push(format!("sketch: the {name} dimension took, but took away no degree of freedom"));
                    }
                    check_all(&mut app, &format!("sketch: the {name} dimension"), &mut problems);
                }
                app.set_dim_tool(0);
            }

            // A DIMENSION IS A LEVER, NOT A CAPTION. The value is edited and the question is whether THE
            // GEOMETRY MOVED. A dimension that changes while the sketch stands still is a deception: the part
            // is believed to be set.
            {
                use qymcad_core::model::Constraint;
                let di = app.project.sketches[si].constraints.iter().position(|c| matches!(c, Constraint::Distance { .. }));
                if let Some(di) = di {
                    let pts_before: Vec<(f64, f64)> = app.project.sketches[si].points.iter().map(|p| (p.x, p.y)).collect();
                    if let Constraint::Distance { d, .. } = &mut app.project.sketches[si].constraints[di] {
                        *d += 7.0;
                    }
                    app.project.solve_sketch(si);
                    app.project.regen_sketch(si);
                    let moved = app.project.sketches[si].points.iter().zip(&pts_before).any(|(p, b)| (p.x - b.0).abs() > 1e-9 || (p.y - b.1).abs() > 1e-9);
                    if !moved {
                        problems.push("sketch: the dimension was changed and the geometry did not move".into());
                    }
                    check_all(&mut app, "sketch: editing a dimension", &mut problems);

                    // A PARAMETRIC EXPRESSION: the dimension holds on to a global variable, and editing that
                    // variable must move the sketch - otherwise the parametrics exist in name only.
                    app.project.parameters.push(qymcad_core::model::Param { name: "width".into(), expr: "40".into(), value: 40.0 });
                    if let Constraint::Distance { expr, .. } = &mut app.project.sketches[si].constraints[di] {
                        *expr = "width".into();
                    }
                    app.project.solve_sketch(si);
                    app.project.regen_sketch(si);
                    let pts_at40: Vec<(f64, f64)> = app.project.sketches[si].points.iter().map(|p| (p.x, p.y)).collect();
                    if let Some(prm) = app.project.parameters.iter_mut().find(|p| p.name == "width") {
                        prm.expr = "65".into();
                        prm.value = 65.0;
                    }
                    app.project.solve_sketch(si);
                    app.project.regen_sketch(si);
                    let followed = app.project.sketches[si].points.iter().zip(&pts_at40).any(|(p, b)| (p.x - b.0).abs() > 1e-9 || (p.y - b.1).abs() > 1e-9);
                    if !followed {
                        problems.push("sketch: the variable was changed and the dimension did not follow - the parametrics exist in name only".into());
                    }
                    check_all(&mut app, "sketch: a dimension driven by a global variable", &mut problems);
                }
            }

            // A REDUNDANT CONSTRAINT MUST BE VISIBLE. A point is fixed and then given a dimension - the same
            // thing said twice. The program must SHOW that: redundancy swallowed silently makes a sketch
            // unmanageable while nobody is told.
            {
                use qymcad_core::model::Constraint;
                let pts: Vec<u64> = app.project.sketches[si].points.iter().take(2).map(|p| p.id).collect();
                if pts.len() == 2 {
                    let cons_before = app.project.sketches[si].constraints.len();
                    let (_, red_before) = app.project.sketch_dof(si);
                    app.project.sketches[si].constraints.push(Constraint::Fixed { p: pts[0] });
                    app.project.sketches[si].constraints.push(Constraint::Fixed { p: pts[1] });
                    let d = 25.0;
                    app.project.sketches[si].constraints.push(Constraint::Distance { a: pts[0], b: pts[1], d, off: 0.0, expr: String::new(), driven: false, axis: 0 });
                    app.project.solve_sketch(si);
                    app.project.regen_sketch(si);
                    let (_, red_after) = app.project.sketch_dof(si);
                    if red_after <= red_before {
                        problems.push(format!("sketch: the redundant constraint was not shown (there were {red_before} redundant ones, now {red_after})"));
                    }
                    check_all(&mut app, "sketch: the redundant constraint is visible", &mut problems);

                    // CLEANING UP AFTER OURSELVES. Having shown that the program sees the redundancy, it is
                    // removed: nobody leaves a sketch in conflict, and the document goes on to be looked at. A
                    // red sketch left in a finished file is rubbish, not a check.
                    app.project.sketches[si].constraints.truncate(cons_before);
                    app.project.solve_sketch(si);
                    app.project.regen_sketch(si);
                    let (_, red_left) = app.project.sketch_dof(si);
                    if red_left > red_before {
                        problems.push(format!("the redundancy did not go away with the constraints: there were {red_before}, {red_left} remain"));
                    }
                    check_all(&mut app, "sketch: the redundant constraint was removed", &mut problems);
                }
            }

            // DRAGGING: a point is pulled by the mouse, and the sketch must recompute along with it.
            if let Some(pt) = app.project.sketches[si].points.first().map(|p| (p.x, p.y)) {
                let mut hand = Hand::new(&mut app);
                hand.sk_tool(0).drag2d((pt.0, pt.1), (pt.0 + 5.0, pt.1 + 5.0));
                app.project.solve_sketch(si);
                app.project.regen_sketch(si);
                check_all(&mut app, "sketch: a point was dragged", &mut problems);
            }

            app.finish_sketch_edit();
            app.exit_context_for_test();
        }

        // --- THE MONKEY: RANDOM ACTIONS ON A FINISHED DOCUMENT ---
        //
        // People do more than the sensible: press the wrong tool, click the wrong face, type nonsense, change
        // their mind and press Esc. None of that may break the program - not with a red node lacking a
        // reason, not with a lost row in the tree, not with a catalogue key where words belong.
        //
        // THE SEED IS FIXED: a finding must be reproducible, or it cannot be mended.
        {
            let mut seed: u64 = 20260807;
            let mut next = |n: u64| {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                (seed >> 33) % n
            };
            let tools = [4u8, 5, 6, 7, 23, 25, 26, 29, 30];
            let parts: Vec<u64> = app.project.components.iter().filter(|c| c.id != app.project.root).map(|c| c.id).collect();
            for step in 0..12 {
                let Some(&part) = parts.get(next(parts.len() as u64) as usize) else { break };
                app.exit_context_for_test();
                app.enter_component(part);
                let kind = tools[next(tools.len() as u64) as usize];
                let esc = next(4) == 0; // every fourth time the mind is changed
                let before = app.project.timeline.len();
                if esc {
                    app.start_feat_cmd(kind);
                    app.cancel_feat_cmd();
                    if app.project.timeline.len() != before {
                        problems.push(format!("monkey step {step}: Esc left a trace in the timeline"));
                    }
                } else {
                    apply_tool(&mut app, kind, &mut problems, "monkey");
                }
                app.rebuild_if_dirty();
                // A LEGITIMATE REFUSAL IS NOT A FAULT. Being told "the chamfer is larger than the wall" leads
                // to cancelling the action and going on; it stays a red node only for someone who walked away
                // for tea. So a step with A NAMED reason is rolled back, while a nameless `OpFailed` is left -
                // that is the finding this whole thing exists for.
                let named: Vec<u64> = app
                    .project
                    .timeline
                    .iter()
                    .skip(before)
                    .filter(|n| {
                        app.project
                            .regen_errors
                            .get(&n.id)
                            .is_some_and(|e| !matches!(e, qymcad_core::errors::CoreError::OpFailed(_)))
                    })
                    .map(|n| n.id)
                    .collect();
                for id in named {
                    app.project.delete_feature_op(id);
                }
                if !app.project.timeline.is_empty() {
                    app.rebuild_if_dirty();
                }
                check_all(&mut app, &format!("monkey step {step} (tool {kind}{})", if esc { ", Esc" } else { "" }), &mut problems);
            }
            app.exit_context_for_test();
        }

        // --- A GLOBAL VARIABLE IN A FEATURE DIMENSION: EDIT THE NUMBER AND THE BODY MOVES ---
        //
        // This is already checked inside a sketch; here it is the same thing at the level of THE TIMELINE: the
        // height of an extrude holds on to a variable, and editing that variable must rebuild THE BODY.
        // Otherwise the parametrics end at the sketch and the part stays hand-made.
        {
            use qymcad_core::feature::FeatureKind as FK;
            let ext = app.project.timeline.iter().find_map(|n| match n.kind {
                FK::Extrude { body, .. } => Some((n.id, body)),
                _ => None,
            });
            if let Some((node, body)) = ext {
                let area = |a: &App| a.project.regen_faces.get(&body).map(|fs| fs.iter().map(|f| f.area).sum::<f64>()).unwrap_or(0.0);
                let a0 = area(&app);
                app.begin_edit("the height follows a variable");
                app.project.parameters.push(qymcad_core::model::Param { name: "height".into(), expr: "18".into(), value: 18.0 });
                app.project.feat_dims.entry(node).or_default().insert("height".into(), "height".into());
                app.commit_edit();
                app.project.mark_node_dirty(node);
                app.rebuild_if_dirty();
                let a1 = area(&app);

                app.begin_edit("editing the variable");
                if let Some(p) = app.project.parameters.iter_mut().find(|p| p.name == "height") {
                    p.expr = "34".into();
                    p.value = 34.0;
                }
                app.commit_edit();
                app.project.mark_node_dirty(node);
                app.rebuild_if_dirty();
                let a2 = area(&app);

                if (a2 - a1).abs() < 1e-6 {
                    problems.push(format!(
                        "the variable was changed from 18 to 34 and the body did not move: the area went {a1:.0} -> {a2:.0} (it was {a0:.0} before the binding)"
                    ));
                }
                check_all(&mut app, "the height of an extrude driven by a global variable", &mut problems);
            }
        }

        // --- RENAMING: A NAME GIVEN BY HAND MATTERS MORE THAN AN AUTOMATIC CAPTION ---
        //
        // The first thing done to a finished part is naming its nodes and parts in human terms. A name must
        // reach the tree and survive a save: a lost name means the work is no longer recognisable as one's
        // own.
        {
            let part = app.project.components.iter().find(|c| c.id != app.project.root).map(|c| c.id);
            let node = app
                .project
                .timeline
                .iter()
                .find(|n| !matches!(n.kind, qymcad_core::feature::FeatureKind::Sketch { .. }))
                .map(|n| n.id);
            if let (Some(part), Some(node)) = (part, node) {
                // THE BOUNDARY OF AN EDIT, as in the program: renaming is a deliberate act and makes one undo
                // step. Without it the undo snapshot knows nothing of the new names and wipes them.
                app.begin_edit("renaming");
                if let Some(c) = app.project.components.iter_mut().find(|c| c.id == part) {
                    c.name = "Left bracket".into();
                }
                if let Some(n) = app.project.timeline.iter_mut().find(|n| n.id == node) {
                    n.name = "Mounting pad".into();
                }
                app.commit_edit();
                app.rebuild_if_dirty();

                let ti = app.project.timeline.iter().position(|n| n.id == node).unwrap_or(0);
                let row = app.feature_row_label(ti);
                if !row.contains("Mounting pad") {
                    problems.push(format!("the renamed node is shown in the tree as \"{row}\" rather than by its name"));
                }
                let shown = app.project.components.iter().find(|c| c.id == part).map(|c| crate::i18n::name(&c.name)).unwrap_or_default();
                if shown != "Left bracket" {
                    problems.push(format!("the renamed part is called \"{shown}\""));
                }
                check_all(&mut app, "a node and a part were renamed", &mut problems);
            }
        }

        // --- UNDO AND REDO: THE DOCUMENT MUST COME BACK EXACTLY ---
        //
        // Ctrl+Z gets pressed more often than any button on the panel. An undo that returns to the wrong
        // place or leaves tails is worse than an operation that failed: work is lost unnoticed.
        {
            // THE UNDO IS CHECKED ON A PART WHERE THE STEP IS CERTAIN TO GO THROUGH. On a mirrored part every
            // edge is smooth and a fillet has nothing to take hold of - and a red node from a legitimate
            // refusal would obscure whether the undo returned the document. The last part is taken: it is a
            // fresh cube.
            let parts: Vec<u64> = app.project.components.iter().filter(|c| c.id != app.project.root).map(|c| c.id).collect();
            if let Some(&part) = parts.last() {
                app.exit_context_for_test();
                app.enter_component(part);
                let nodes0 = app.project.timeline.len();
                // THE CONTENTS ARE COMPARED, NOT A GENERAL KEY. The key includes derived data that a rebuild
                // legitimately refreshes, while what matters is something else: the same nodes, bodies, parts
                // and names. Comparing by key raised an alarm where the document was in fact intact.
                let snap0 = (
                    app.project.timeline.iter().map(|n| (n.id, n.name.clone())).collect::<Vec<_>>(),
                    app.project.bodies.iter().map(|b| b.id).collect::<Vec<_>>(),
                    app.project.components.iter().map(|c| (c.id, c.name.clone())).collect::<Vec<_>>(),
                );

                // THE STEP GOES INSIDE AN EDIT BOUNDARY. An undo removes the last DELIBERATE ACT; if the recipe
                // builds outside a boundary, Ctrl+Z removes not what was just built but what came before - and
                // the check blames the undo for someone else's work.
                app.begin_edit("fillet");
                apply_tool(&mut app, 4, &mut problems, "undo"); // the fillet is what this is checked on
                app.commit_edit();
                app.rebuild_if_dirty();
                let nodes1 = app.project.timeline.len();

                app.undo();
                app.rebuild_if_dirty();
                if app.project.timeline.len() != nodes0 {
                    problems.push(format!("after the undo there are {} nodes instead of {nodes0}", app.project.timeline.len()));
                }
                let snap1 = (
                    app.project.timeline.iter().map(|n| (n.id, n.name.clone())).collect::<Vec<_>>(),
                    app.project.bodies.iter().map(|b| b.id).collect::<Vec<_>>(),
                    app.project.components.iter().map(|c| (c.id, c.name.clone())).collect::<Vec<_>>(),
                );
                if snap1 != snap0 {
                    problems.push("after the undo the contents of the document differ: the nodes, bodies or parts are not the ones from before the step".into());
                }
                check_all(&mut app, "the undo returned the document", &mut problems);

                app.redo();
                app.rebuild_if_dirty();
                if nodes1 > nodes0 && app.project.timeline.len() != nodes1 {
                    problems.push(format!("after the redo there are {} nodes instead of {nodes1}", app.project.timeline.len()));
                }
                // AN UNDO RESURRECTS WHAT WAS ROLLED BACK. A step with a named refusal was removed the way a
                // person removes it; the undo returns the state from BEFORE that removal, red node included.
                // That is correct behaviour for an undo, but afterwards the document has to be tidied the same
                // way a person would tidy it: take away what the program explained.
                let named: Vec<u64> = app
                    .project
                    .timeline
                    .iter()
                    .filter(|n| app.project.regen_errors.get(&n.id).is_some_and(|e| !matches!(e, qymcad_core::errors::CoreError::OpFailed(_))))
                    .map(|n| n.id)
                    .collect();
                for id in named {
                    app.project.delete_feature_op(id);
                }
                app.rebuild_if_dirty();
                // SECOND PASS: the rebuild after removing nodes can name a refusal again (the step
                // came back with a redo), and a red node would survive the first cleanup.
                let named2: Vec<u64> = app
                    .project
                    .timeline
                    .iter()
                    .filter(|n| app.project.regen_errors.get(&n.id).is_some_and(|e| !matches!(e, qymcad_core::errors::CoreError::OpFailed(_))))
                    .map(|n| n.id)
                    .collect();
                for id in named2 {
                    app.project.delete_feature_op(id);
                }
                app.rebuild_if_dirty();
                check_all(&mut app, "the redo brought the step back", &mut problems);
                app.exit_context_for_test();
            }
        }

        // ── A SAVE ROUND AFTER EVERYTHING: THE DOCUMENT MUST SURVIVE IT WITHOUT LOSSES ────
        //
        // The first save round was at the start, on a plain housing. Here the document carries
        // everything: a sew, a trim, a patch, patterns and mirrors of PARTS, joints. What is compared
        // is not "it opened" but the contents: nodes, bodies, parts, names and areas - losing any of
        // them would be losing a person's work.
        {
            let nodes_before = app.project.timeline.len();
            let bodies_before = app.project.bodies.len();
            let comps_before = app.project.components.len();
            let joints_before = app.project.joints.len();
            let names_before: Vec<String> = app.project.components.iter().map(|c| c.name.clone()).collect();
            let area_before: f64 = app.project.regen_faces.values().flatten().map(|f| f.area).sum();

            let app2 = save_and_reopen(&mut app, "the save round after every operation", &mut problems);

            if app2.project.timeline.len() != nodes_before {
                problems.push(format!("after the save round there are {} nodes instead of {nodes_before}", app2.project.timeline.len()));
            }
            if app2.project.bodies.len() != bodies_before {
                problems.push(format!("after the save round there are {} bodies instead of {bodies_before}", app2.project.bodies.len()));
            }
            if app2.project.components.len() != comps_before {
                problems.push(format!("after the save round there are {} parts instead of {comps_before}", app2.project.components.len()));
            }
            if app2.project.joints.len() != joints_before {
                problems.push(format!("after the save round there are {} joints instead of {joints_before}", app2.project.joints.len()));
            }
            let names_after: Vec<String> = app2.project.components.iter().map(|c| c.name.clone()).collect();
            if names_after != names_before {
                problems.push("after the save round the part names changed".into());
            }
            let area_after: f64 = app2.project.regen_faces.values().flatten().map(|f| f.area).sum();
            if area_before > 1.0 && (area_after - area_before).abs() > area_before * 0.01 {
                problems.push(format!("after the save round the surface area is {area_after:.0} instead of {area_before:.0} - the geometry did not survive the round"));
            }
            app = app2;
            // AFTER REOPENING a node can turn red: a full rebuild sees what a partial one did not
            // (the fillet has nothing left to round - a neighbouring operation smoothed the edges
            // away). A person who opens the file and sees red removes the step; so does the scenario.
            for _ in 0..3 {
                app.rebuild_if_dirty();
                let named: Vec<u64> = app
                    .project
                    .timeline
                    .iter()
                    .filter(|n| app.project.regen_errors.get(&n.id).is_some_and(|e| !matches!(e, qymcad_core::errors::CoreError::OpFailed(_))))
                    .map(|n| n.id)
                    .collect();
                if named.is_empty() {
                    break;
                }
                for id in named {
                    app.project.delete_feature_op(id);
                }
            }
            app.rebuild_if_dirty();
            check_all(&mut app, "the save round after every operation", &mut problems);
        }

        // ── THE RIGHT PANEL: WHAT IS SHOWN IS WHAT IS IN THE MODEL ───────────────────────
        //
        // The property panel is what a person checks a part by, without rebuilding it in their head.
        // EVERY node is opened for editing and the shown numbers are compared against the model: a
        // divergence here means a person edits one thing while another one changes.
        {
            use qymcad_core::feature::FeatureKind as FK;
            let ids: Vec<u64> = app.project.timeline.iter().map(|n| n.id).collect();
            for id in ids {
                let Some(node) = app.project.timeline.iter().find(|n| n.id == id).cloned() else { continue };
                let want: Option<(&str, f64)> = match node.kind {
                    FK::Extrude { height, .. } => Some(("height", height)),
                    FK::Fillet { radius, .. } => Some(("radius", radius)),
                    FK::Chamfer { dist, .. } => Some(("dist", dist)),
                    FK::Shell { thickness, .. } => Some(("thickness", thickness)),
                    FK::Draft { angle, .. } => Some(("angle", angle)),
                    FK::Hole { diameter, .. } => Some(("diameter", diameter)),
                    FK::PushFace { dist, .. } => Some(("dist", dist)),
                    FK::Thicken { thickness, .. } => Some(("thickness", thickness)),
                    _ => None,
                };
                let Some((key, model)) = want else { continue };
                // A DIMENSION DRIVEN BY AN EXPRESSION: the shown number comes from a variable and
                // LEGITIMATELY differs from the one stored in the node. Comparing them would mean
                // demanding that there be no parametrics at all.
                if app.project.feat_dims.get(&id).is_some_and(|m| m.contains_key(key)) {
                    continue;
                }
                app.start_feat_cmd_edit(id);
                let shown = app.cmd.params.iter().find(|p| p.key == key).map(|p| p.val);
                match shown {
                    Some(v) if (v - model).abs() < 1e-6 => {}
                    Some(v) => problems.push(format!("property panel of node {id}: {key} = {v} is shown while the model has {model}")),
                    None => problems.push(format!("property panel of node {id}: there is no \"{key}\" field at all while the model has {model}")),
                }
                app.cancel_feat_cmd();
            }
            check_all(&mut app, "the property panel matches the model", &mut problems);
        }

        // ── LAY THE PARTS OUT SO THEY DO NOT SIT IN A HEAP ──────────────────────────────
        //
        // Everything was built around the origin, and in 3D the document looked like porridge: parts
        // inside one another. A person does not work that way - they spread the parts out. The
        // EXISTING parts are moved (rather than new ones bred) by their own extents: a row of parts,
        // a row of pairs for the joints, a gap between them.
        {
            let gap = 40.0;
            // THE EXTENT IN THE PART'S OWN FRAME: the body does not start at the part's origin, so
            // placing it "by width" is not enough - its LEFT EDGE has to be known, otherwise the
            // neighbours climb over one another.
            let span = |app: &App, part: u64| -> (f64, f64) {
                let Some(b) = app.project.active_body(part) else { return (0.0, 30.0) };
                let Some(mi) = app.project.mesh_index(b) else { return (0.0, 30.0) };
                let (mut lo, mut hi) = (f64::MAX, f64::MIN);
                for v in &app.project.bodies[mi].mesh.verts {
                    lo = lo.min(v.x);
                    hi = hi.max(v.x);
                }
                if lo > hi { (0.0, 30.0) } else { (lo, hi) }
            };
            let parts: Vec<u64> = app.project.components.iter().filter(|c| c.id != app.project.root).map(|c| c.id).collect();
            let mut x = 0.0;
            for part in parts {
                let (lo, hi) = span(&app, part);
                let mut m = qymcad_core::feature::PLACE_IDENTITY;
                m[3] = x - lo; // the body's left edge lands exactly on the cursor
                app.project.set_component_transform(part, m);
                x += (hi - lo).max(20.0) + gap;
            }
            app.rebuild_if_dirty();

            // CHECK: the part extents do not overlap along X - that is what "not in a heap" means.
            let mut spans: Vec<(String, f64, f64)> = Vec::new();
            for c in app.project.components.iter().filter(|c| c.id != app.project.root) {
                if let Some(b) = app.project.active_body(c.id) {
                    if let Some(mi) = app.project.mesh_index(b) {
                        let w = app.project.body_world_transform(b);
                        let (mut lo, mut hi) = (f64::MAX, f64::MIN);
                        for v in &app.project.bodies[mi].mesh.verts {
                            let p = qymcad_core::feature::apply12(&w, [v.x, v.y, v.z]);
                            lo = lo.min(p[0]);
                            hi = hi.max(p[0]);
                        }
                        if lo <= hi {
                            spans.push((crate::i18n::name(&c.name), lo, hi));
                        }
                    }
                }
            }
            spans.sort_by(|a, b| a.1.total_cmp(&b.1));
            for pair in spans.windows(2) {
                if pair[1].1 < pair[0].2 - 1e-6 {
                    problems.push(format!("parts \"{}\" and \"{}\" overlap along X: {:.0}..{:.0} and {:.0}..{:.0}", pair[0].0, pair[1].0, pair[0].1, pair[0].2, pair[1].1, pair[1].2));
                }
            }
            // A REBUILD CHECK: everything is marked dirty and rebuilt. If nodes turn red that were
            // not on the list before, the partial rebuild missed something - and at that moment a
            // person is looking at a green document that is already broken.
            let red_before: std::collections::HashSet<u64> = app.project.regen_errors.keys().copied().collect();
            for n in &mut app.project.timeline {
                n.dirty = true;
            }
            app.rebuild_if_dirty();
            let red_after: std::collections::HashSet<u64> = app.project.regen_errors.keys().copied().collect();
            let hidden: Vec<u64> = red_after.difference(&red_before).copied().collect();
            if !hidden.is_empty() {
                problems.push(format!(
                    "the full rebuild uncovered nodes {hidden:?} that the partial one did not see - the document looked intact and was already broken"
                ));
            }
            check_all(&mut app, "the parts are laid out", &mut problems);
        }

        // ── CLEAN UP THE EMPTY PARTS ─────────────────────────────────────────────────────
        //
        // The program keeps one empty part from the very start - that is convenient while working.
        // But in a finished file handed to another person an empty row in the tree looks like
        // forgotten litter. A person removes it; so does the scenario.
        {
            let empty: Vec<u64> = app
                .project
                .components
                .iter()
                .filter(|c| c.id != app.project.root && matches!(c.kind, qymcad_core::feature::ComponentKind::Part))
                .filter(|c| !app.project.timeline.iter().any(|n| n.parent == Some(c.id)))
                .map(|c| c.id)
                .collect();
            for id in empty {
                app.project.delete_component(id);
            }
            app.rebuild_if_dirty();
            let left: Vec<String> = app
                .project
                .components
                .iter()
                .filter(|c| c.id != app.project.root && matches!(c.kind, qymcad_core::feature::ComponentKind::Part))
                .filter(|c| !app.project.timeline.iter().any(|n| n.parent == Some(c.id)))
                .map(|c| crate::i18n::name(&c.name))
                .collect();
            if !left.is_empty() {
                problems.push(format!("the finished document still holds empty parts: {}", left.join(", ")));
            }
            let hidden: Vec<u64> = app.project.bodies.iter().filter(|b| !b.visible).map(|b| b.id).collect();
            if !hidden.is_empty() {
                problems.push(format!("the finished document still holds HIDDEN bodies {hidden:?} - opening the file, a person will not see them"));
            }
            // A BODY WITH NO GEOMETRY IS A GHOST: there is a row in the tree and nothing to show.
            // Measured by the mesh: an empty mesh means the operation "went through" and built
            // nothing.
            let ghosts: Vec<u64> = app.project.bodies.iter().filter(|b| b.mesh.verts.is_empty() && !b.sheet).map(|b| b.id).collect();
            if !ghosts.is_empty() {
                problems.push(format!("the finished document holds bodies WITH NO GEOMETRY {ghosts:?} - there is a row in the tree and nothing to show"));
            }
            // A PART TIED TO NOTHING. In an assembly a part without a single joint floats free:
            // sometimes that is intended, but in a finished document it is worth seeing rather than
            // finding out later.
            let jointed: std::collections::HashSet<u64> = app
                .project
                .joints
                .iter()
                .flat_map(|j| [j.a, j.b])
                .filter_map(|c| app.project.connectors.iter().find(|k| k.id == c).map(|k| k.owner))
                .collect();
            let loose = app
                .project
                .components
                .iter()
                .filter(|c| c.id != app.project.root && matches!(c.kind, qymcad_core::feature::ComponentKind::Part))
                .filter(|c| !jointed.contains(&c.id))
                .count();
            if loose == app.project.components.len().saturating_sub(1) && !app.project.joints.is_empty() {
                problems.push("not one part is tied by a joint although the document has joints - the joints are dangling".into());
            }
            check_all(&mut app, "the empty parts are gone", &mut problems);
        }

        // ── EDITING THE SKETCH OF A FINISHED PART: DRAG A POINT, THE BODY MOVES ─────────
        //
        // This is what a parametric CAD is for. A person enters a sketch an already built part
        // stands on, moves a point - and the body must recompute. If it does not, the link between
        // the sketch and the body is a link in name only.
        {
            let victim = app.project.timeline.iter().find_map(|n| match n.kind {
                qymcad_core::feature::FeatureKind::Extrude { sketch, body, .. } => Some((sketch, body)),
                _ => None,
            });
            if let Some((sid, body)) = victim {
                let vol_before = app.project.mesh_index(body).and_then(|mi| app.project.bodies[mi].mesh.bounds()).map(|b| (b.max.x - b.min.x) * (b.max.y - b.min.y) * (b.max.z - b.min.z));
                if let Some(si) = app.project.sketch_index(sid) {
                    app.enter_sketch_edit_pub(si);
                    if app.edit_si() != Some(si) {
                        problems.push("a double click on the sketch of a finished part did not open it for editing".into());
                    }
                    // drag the point FARTHEST from the origin - moving it must change the extent
                    let far = app.project.sketches[si].points.iter().max_by(|a, b| (a.x * a.x + a.y * a.y).total_cmp(&(b.x * b.x + b.y * b.y))).map(|p| (p.x, p.y));
                    if let Some((x, y)) = far {
                        Hand::new(&mut app).drag2d((x, y), (x + 7.0, y + 7.0));
                    }
                    app.on_escape();
                    if app.edit_si() == Some(si) {
                        problems.push("Esc did not leave sketch editing".into());
                    }
                    app.rebuild_if_dirty_for_test();
                    let vol_after = app.project.mesh_index(body).and_then(|mi| app.project.bodies[mi].mesh.bounds()).map(|b| (b.max.x - b.min.x) * (b.max.y - b.min.y) * (b.max.z - b.min.z));
                    match (vol_before, vol_after) {
                        (Some(a), Some(b)) if (a - b).abs() < 1e-6 => {
                            problems.push(format!("the sketch was edited and the body did not move: extent {a} stayed {b} - the sketch -> body link is a link in name only"));
                        }
                        (Some(_), None) => problems.push("after the sketch was edited the body vanished".into()),
                        _ => {}
                    }
                }
            }
            check_all(&mut app, "editing the sketch of a finished part", &mut problems);
        }

        // ── CANCELLING MID-COMMAND: LEAVE NO HALF-BUILT THING BEHIND ────────────────────
        //
        // A person started an operation, picked the references - and changed their mind. After Esc
        // neither a node nor a trace may stay in the document: a half-built thing is worse than a
        // refusal, because it says nothing.
        {
            let nodes_before = app.project.timeline.len();
            let doc_before = app.project.state_key();
            app.start_feat_cmd(4); // fillet
            if let Some(body) = app.project.bodies.iter().find(|b| !b.sheet).map(|b| b.id) {
                app.gsel.edges = app.body_edges_cached(body).map(|e| e.1.iter().copied().filter(|&i| i != 0).collect()).unwrap_or_default();
                app.edges.body = Some(body);
            }
            app.on_escape();
            if app.cmd.active() {
                problems.push("Esc did not close the command: the bar stayed open".into());
            }
            if app.project.timeline.len() != nodes_before {
                problems.push(format!("cancelling mid-command left a node: there were {nodes_before}, now {}", app.project.timeline.len()));
            }
            if app.project.state_key() != doc_before {
                problems.push("cancelling mid-command changed the document".into());
            }
            // WHAT ACTUALLY MATTERS HERE. The first edition demanded that Esc itself clear the
            // reference set, and it went red. The program was right: the set is cleared by the START
            // of the next command (`start_feat_cmd`), and there is nowhere for a "foreign set" to
            // come from. Demanding more than that means checking the internals instead of what a
            // person sees.
            app.start_feat_cmd(5); // chamfer - the next command
            if !app.gsel.edges.is_empty() || !app.gsel.faces.is_empty() {
                problems.push(format!(
                    "the new command opened with a FOREIGN reference set: {} edges, {} faces",
                    app.gsel.edges.len(),
                    app.gsel.faces.len()
                ));
            }
            app.on_escape();
            check_all(&mut app, "cancelling mid-command", &mut problems);
        }

        // ── DELETING A SKETCH A BODY STANDS ON ──────────────────────────────────────────
        //
        // Cascading deletion is where a document comes apart quietly: a body built on a sketch
        // cannot exist without it. A person may remove the sketch, but then everything that stood on
        // it must go with it - otherwise the timeline keeps nodes that build out of nothing.
        {
            let victim = app.project.timeline.iter().find_map(|n| match n.kind {
                qymcad_core::feature::FeatureKind::Extrude { sketch, body, .. } => Some((sketch, body)),
                _ => None,
            });
            if victim.is_none() {
                problems.push("deleting a sketch with a body is unchecked: no extrude was found in the document".into());
            }
            if let Some((sid, body)) = victim {
                let nodes_before = app.project.timeline.len();
                // BY THE SAME PATH A PERSON TAKES: the removal goes through `execute_delete`, not
                // past it. The `confirm_once` guard caught the first edition red-handed, and rightly
                // so: a test door bypassing the deletion prompt is exactly the kind of "own door"
                // that does the damage.
                let si = app.project.sketch_index(sid).expect("the sketch is on the list");
                app.execute_delete(crate::gui::Sel::Sketch(si));
                app.rebuild_if_dirty_for_test();

                if app.project.sketches.iter().any(|s| s.id == sid) {
                    problems.push("the deleted sketch stayed in the document".into());
                }
                if app.project.timeline.iter().any(|n| n.kind.bodies().contains(&body)) {
                    problems.push(format!("the sketch is deleted and body {body} built on it stayed in the timeline - the node builds out of nothing"));
                }
                if app.project.mesh_index(body).is_some() {
                    problems.push(format!("the sketch is deleted and the mesh of body {body} stayed on the screen"));
                }
                if app.project.timeline.len() >= nodes_before {
                    problems.push(format!("deleting a sketch with a body did not shorten the timeline: there were {nodes_before}, now {}", app.project.timeline.len()));
                }
                // THE MIRROR OF AN ORPHANED PART MUST TURN RED.
                //
                // The sketch was removed, the part is left with no body, there is nothing to copy.
                // The mirror's body LEGITIMATELY stays on the screen: the keep-last-good rule - the
                // model is not taken away while a person fixes the error. The first edition of this
                // check demanded the opposite, and the run caught it red-handed: a deliberate rule
                // was nearly broken. What is not allowed here is SILENCE: old geometry without a
                // single word that the node no longer builds.
                for n in &app.project.timeline {
                    if let qymcad_core::feature::FeatureKind::MirrorPart { src_comp, body: mb, .. } = n.kind {
                        let src_has_body = app
                            .project
                            .timeline
                            .iter()
                            .flat_map(|x| x.kind.bodies())
                            .any(|b| app.project.body_owner(b) == Some(src_comp) && app.project.mesh_index(b).is_some());
                        let _ = mb;
                        if !src_has_body && !app.project.regen_errors.contains_key(&n.id) {
                            problems.push(format!(
                                "part {src_comp} is left with no body while its mirror (node {}) says nothing: old geometry on the screen and not a word about the breakage",
                                n.id
                            ));
                        }
                    }
                }
                // and NOT ONE remaining node may reference what was removed
                let gone = [sid, body];
                for n in &app.project.timeline {
                    if n.kind.inputs().iter().any(|i| gone.contains(i)) {
                        problems.push(format!("node \"{}\" kept a reference to what was deleted", n.name));
                    }
                }
            }
            check_all(&mut app, "deleting a sketch with a body", &mut problems);
        }

        // ── DELETING A PART FROM THE MIDDLE OF AN ASSEMBLY, WITH JOINTS ON IT ───────────
        //
        // The most dangerous deletion: the part is not alone, mates hang off it and neighbours work
        // around it. Afterwards neither a joint into the void nor an ownerless connector may stay in
        // the document - otherwise the solver counts by ghosts and the tree shows what is not there.
        {
            // take a part that HAS a mate and is not the only one
            let owner_of = |app: &App, c: qymcad_core::model::Id| app.project.connectors.iter().find(|k| k.id == c).map(|k| k.owner);
            // ...AND IS NOT PART OF A PATTERN. The first edition took the first one with a joint, hit
            // a pattern source and went red with "the deletion carried the neighbours away". That is
            // by design: a pattern copy is not deleted on its own, the pattern drives it and the whole
            // pattern goes (see `execute_delete`). This step is about something else - deleting a part
            // WITH JOINTS - so a pattern is not taken.
            let victim = app
                .project
                .joints
                .iter()
                .filter_map(|j| owner_of(&app, j.a).or_else(|| owner_of(&app, j.b)))
                .find(|c| app.project.comp_pattern_of(*c).is_none());
            // THE STEP MUST HAPPEN. Without this it is silently skipped and the green means nothing.
            if victim.is_none() {
                problems.push("deleting a part with joints is unchecked: no part with a joint outside a pattern was found in the document".into());
            }
            if let Some(victim) = victim {
                let joints_before = app.project.joints.len();
                let comps_before = app.project.components.len();
                let neighbours: Vec<qymcad_core::model::Id> = app.project.components.iter().filter(|c| c.id != victim).map(|c| c.id).collect();
                let its_joints = app
                    .project
                    .joints
                    .iter()
                    .filter(|j| owner_of(&app, j.a) == Some(victim) || owner_of(&app, j.b) == Some(victim))
                    .count();

                let ci = app.project.components.iter().position(|c| c.id == victim).expect("the part is on the list");
                app.begin_edit("delete a part with joints");
                app.execute_delete(crate::gui::Sel::Component(ci));
                app.commit_edit();
                app.rebuild_if_dirty_for_test();

                if app.project.components.iter().any(|c| c.id == victim) {
                    problems.push("the deleted part stayed in the document".into());
                }
                if app.project.components.len() != comps_before - 1 {
                    problems.push(format!("ONE part was deleted and the component count became {} instead of {}", app.project.components.len(), comps_before - 1));
                }
                // the neighbours must not suffer
                for n in &neighbours {
                    if !app.project.components.iter().any(|c| c.id == *n) {
                        problems.push(format!("deleting the part carried neighbour {n} away with it"));
                    }
                }
                // this part's joints are gone, the rest stayed
                if app.project.joints.len() != joints_before - its_joints {
                    problems.push(format!(
                        "after the part was deleted there are {} joints, {} expected (there were {joints_before}, the part had {its_joints})",
                        app.project.joints.len(),
                        joints_before - its_joints
                    ));
                }
                // an ownerless connector is an orphan a person will neither see nor delete
                let live: std::collections::HashSet<qymcad_core::model::Id> = app.project.components.iter().map(|c| c.id).collect();
                let orphan = app.project.connectors.iter().filter(|k| !live.contains(&k.owner)).count();
                if orphan > 0 {
                    problems.push(format!("after the part was deleted {orphan} ownerless connectors are left"));
                }
                // the solver must keep working rather than trip over ghosts
                app.project.solve_joints();
            }
            check_all(&mut app, "deleting a part with joints", &mut problems);
        }

        // ── A NESTED ASSEMBLY: AN ASSEMBLY INSIDE AN ASSEMBLY ───────────────────────────
        //
        // A node inside a node is where the document broke twice, and both times it showed: bodies
        // surfaced into the ROOT, or a part ended up inside a part. Here a person makes a subassembly,
        // enters it, builds a part there and leaves - and everything built must stay INSIDE.
        {
            let root = app.project.root;
            let bodies_in_root = |app: &App| -> usize {
                app.project
                    .timeline
                    .iter()
                    .filter(|n| n.parent == Some(app.project.root) && !n.kind.bodies().is_empty())
                    .count()
            };
            let root_bodies_before = bodies_in_root(&app);

            app.set_context_to(root);
            let sub = app.project.add_assembly("Subassembly");
            app.enter_component_for_test(sub);
            if app.current_ctx_id() != sub {
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
            app.enter_component_for_test(inner);
            let sk = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
            app.project.add_rect_entity(sk, 0.0, 0.0, 12.0, 12.0, qymcad_core::feature::Purpose::Real);
            app.project.solve_sketch(sk);
            app.project.regen_sketch(sk);
            let sid = app.project.sketches[sk].id;
            app.project.add_sketch_node(sid, "subassembly sketch");
            let body = app.project.add_extrude(sid, 8.0);
            app.rebuild_if_dirty_for_test();

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
            if bodies_in_root(&app) != root_bodies_before {
                problems.push(format!(
                    "after working in the subassembly the ROOT gained builds: there were {root_bodies_before}, now {}",
                    bodies_in_root(&app)
                ));
            }

            // go back up - the context must return and the subassembly must stay in the tree
            app.set_context_to(root);
            if app.current_ctx_id() != root {
                problems.push("leaving the subassembly did not return the context to the root".into());
            }
            if !app.project.components.iter().any(|c| c.id == sub) {
                problems.push("the subassembly vanished from the document after leaving it".into());
            }
            check_all(&mut app, "a nested assembly", &mut problems);
        }

        // ── MOVING A NODE IN THE TIMELINE ───────────────────────────────────────────────
        //
        // The order of operations in the history is not decoration: a fillet AFTER a shell and BEFORE
        // it give different parts. A person moves nodes and expects two things: a legal move the
        // program performs and rebuilds, an illegal one it REFUSES rather than silently breaking the
        // part.
        {
            let before: Vec<qymcad_core::model::Id> = app.project.timeline.iter().map(|n| n.id).collect();
            let bodies_before = app.project.bodies.len();
            // ILLEGAL: the input body under its own consumer. Look for a node that consumes something.
            let dependent = app.project.timeline.iter().enumerate().find_map(|(ti, n)| {
                let inputs = n.kind.inputs();
                let src = inputs.first().copied()?;
                let si = app.project.timeline.iter().position(|x| x.kind.bodies().contains(&src))?;
                (si < ti).then_some((ti, si))
            });
            if let Some((ti, si)) = dependent {
                // AIM BELOW THE CONSUMER. The first edition moved the input ONTO the consumer's slot,
                // and that is a legal move: once the node is taken out the consumer slides up, the
                // input lands right before it and the order holds. What was filed as a bug was plain
                // arithmetic. The illegal move is PAST the consumer, and that is the one tried here.
                let target = ti + 1;
                if target < app.project.timeline.len() {
                    app.move_feature(si, target);
                    // JUDGE BY THE TIMELINE, NOT BY THE RETURN VALUE: what matters is that the input
                    // stayed above the consumer.
                    let pos = |id: qymcad_core::model::Id| app.project.timeline.iter().position(|n| n.id == id);
                    for n in app.project.timeline.clone() {
                        for inp in n.kind.inputs() {
                            let producer = app.project.timeline.iter().find(|x| x.kind.bodies().contains(&inp)).map(|x| x.id);
                            if let (Some(p), Some(c)) = (producer.and_then(pos), pos(n.id)) {
                                if p > c {
                                    problems.push(format!(
                                        "in the timeline consumer \"{}\" ended up ABOVE its own input - the part is built on what does not exist yet",
                                        n.name
                                    ));
                                }
                            }
                        }
                    }
                }
            }

            // LEGAL: two adjacent nodes that do not depend on one another.
            let swap = app.project.timeline.windows(2).position(|w| {
                let (a, b) = (&w[0], &w[1]);
                !b.kind.inputs().iter().any(|i| a.kind.bodies().contains(i)) && !a.kind.inputs().iter().any(|i| b.kind.bodies().contains(i))
            });
            if let Some(i) = swap {
                if app.move_feature(i + 1, i) {
                    app.rebuild_if_dirty_for_test();
                    // THE BODY COUNT IS NOT CHECKED HERE. The first edition demanded that it not
                    // change AFTER the move, and it went red. The program was right: the order of
                    // operations exists precisely because it changes the part - a moved operation may
                    // legitimately fail to build, having named the reason. Unexplained red is caught by
                    // the general check; what matters here is different: the move is REVERSIBLE.
                    app.move_feature(i + 1, i);
                    app.rebuild_if_dirty_for_test();
                    let now: Vec<qymcad_core::model::Id> = app.project.timeline.iter().map(|n| n.id).collect();
                    if now != before {
                        problems.push("moving a node there and back did not restore the timeline order".into());
                    }
                    if app.project.bodies.len() != bodies_before {
                        problems.push(format!(
                            "moving a node there and back did not restore the bodies: there were {bodies_before}, now {} - the document did not come back to its previous state",
                            app.project.bodies.len()
                        ));
                    }
                }
            }
            check_all(&mut app, "moving a node in the timeline", &mut problems);
        }

        // ── THE SECTION VIEW: LOOK INSIDE AND COME BACK OUT ─────────────────────────────
        //
        // A section is a way to look, not an edit: it must leave NOTHING in the document. And it must
        // turn on by the same button a person uses: the logic used to live inside the panel button
        // where a hand could not reach it - a check either poked at the fields directly (that is,
        // checked itself) or did not check at all.
        {
            let doc_before = app.project.state_key();
            let nodes_before = app.project.timeline.len();
            app.toggle_section();
            if !app.section.pick {
                problems.push("section: the button was pressed and the plane pick did not start".into());
            }
            // the plane is picked by clicking a face - the way a person does it
            let body = app.project.bodies.iter().find(|b| !b.sheet).map(|b| b.id);
            if let (Some(body), Some(mi)) = (body, body.and_then(|b| app.project.mesh_index(b))) {
                let _ = body;
                if let Some(bb) = app.project.bodies[mi].mesh.bounds() {
                    let basis = app.cam.basis();
                    let top = [(bb.min.x + bb.max.x) * 0.5, (bb.min.y + bb.max.y) * 0.5, bb.max.z];
                    let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 700.0));
                    let at = app.project3(top, rect, &basis).0;
                    app.viewport_3d_click_at(at, rect, &basis);
                }
            }
            if app.section.plane.is_none() {
                problems.push(format!("section: the click on a face did not set the plane; status: {}", app.status));
            }
            // TURNED OFF - AND NOT A TRACE LEFT
            app.toggle_section();
            if app.section.plane.is_some() || app.section.pick {
                problems.push("section: it was turned off and stayed on".into());
            }
            if app.project.timeline.len() != nodes_before {
                problems.push(format!("the section left a trace in the timeline: there were {nodes_before}, now {}", app.project.timeline.len()));
            }
            if app.project.state_key() != doc_before {
                problems.push("the section changed the document - it is a way to LOOK, not an edit".into());
            }
            check_all(&mut app, "the section view", &mut problems);
        }

        // ── HIDE AND SHOW, FIND IN THE TREE ─────────────────────────────────────────────
        //
        // A hidden body must not disappear from the document - it simply is not drawn; a person hides
        // a part to get at the one next to it and expects it back. And the search must find the name
        // the person gave themselves: otherwise renaming is pointless.
        {
            let body = app.project.bodies.iter().find(|b| !b.sheet).map(|b| b.id);
            if let Some(body) = body {
                let nodes_before = app.project.timeline.len();
                let mi = app.project.mesh_index(body);
                if let Some(mi) = mi {
                    app.begin_edit("hide a body");
                    app.project.bodies[mi].visible = false;
                    app.commit_edit();
                    app.rebuild_if_dirty();
                    if app.project.timeline.len() != nodes_before {
                        problems.push("hiding a body changed the contents of the timeline - what is hidden must STAY in the document".into());
                    }
                    if app.body_shown(mi) {
                        problems.push("the hidden body is drawn anyway".into());
                    }
                    check_all(&mut app, "the body is hidden", &mut problems);

                    app.begin_edit("show a body");
                    app.project.bodies[mi].visible = true;
                    app.commit_edit();
                    app.rebuild_if_dirty();
                    check_all(&mut app, "the body is shown again", &mut problems);
                }
            }

            // SEARCHING THE TREE: look for the name a person gave (the part was renamed above).
            let ti = app.project.timeline.iter().position(|n| crate::i18n::name(&n.name) == "Mounting pad");
            if let Some(ti) = ti {
                app.set_tree_search_for_test("Mounting");
                if !app.tree_row_matches(ti) {
                    problems.push("the tree search does not find the node by the name a person gave".into());
                }
                app.set_tree_search_for_test("no-such-name-in-the-document");
                if app.tree_row_matches(ti) {
                    problems.push("the tree search finds the node by a query that does not match it".into());
                }
                app.set_tree_search_for_test("");
                check_all(&mut app, "the tree search", &mut problems);
            }
        }

        // ── EXPORT: WHAT GOES OUT ───────────────────────────────────────────────────────
        //
        // Export is the end of the road: from here the part goes to print or to a contractor. What is
        // checked is not the file dialogue but the SUBSTANCE: what lands in the body list. Consumed
        // bodies (the sources of operations) must not go out - otherwise the file holds the part as it
        // was before the cuts and shells, on top of the finished one.
        {
            let plan = app.export_plan(super::super::ExportTarget::Project);
            let out: Vec<u64> = plan.brep.iter().chain(plan.mesh_only.iter()).chain(plan.stale.iter()).copied().collect();
            if out.is_empty() {
                problems.push("the project export yields not a single body".into());
            }
            let consumed = app.project.consumed_bodies();
            let leaked: Vec<u64> = out.iter().copied().filter(|b| consumed.contains(b)).collect();
            if !leaked.is_empty() {
                problems.push(format!("CONSUMED bodies {leaked:?} got into the export - the file will hold the part as it was before the operations"));
            }

            // The STL is really written, into a temporary directory: the file must exist and be non-empty.
            let meshes: Vec<qymcad_core::geom::Mesh> = out
                .iter()
                .filter_map(|b| app.project.mesh_index(*b).map(|mi| app.project.bodies[mi].mesh.clone()))
                .collect();
            let path = std::env::temp_dir().join("qym-user-case.stl").to_string_lossy().into_owned();
            match qymcad_io::export_stl(&meshes, &path) {
                Ok(()) => match std::fs::metadata(&path).map(|m| m.len()) {
                    Ok(n) if n > 0 => {}
                    Ok(_) => problems.push("the STL export created an EMPTY file".into()),
                    Err(e) => problems.push(format!("the STL export left no file: {e}")),
                },
                Err(e) => problems.push(format!("the STL export refused: {e}")),
            }
            check_all(&mut app, "the project export", &mut problems);
        }

        // ── AN ASSEMBLY: A MECHANISM THAT ACTUALLY MOVES ────────────────────────────────
        //
        // The scenario document used to be about PARTS only; it held no assembly at all, and opening
        // it a person would not have seen a single joint. Here joints of several kinds and a relation
        // are placed - ALL BY HAND, by the same two clicks on the frame - and each one is checked by
        // movement.
        {
            use super::super::hand::Hand;
            use qymcad_core::feature::{JointKind, RelationKind};
            use qymcad_core::model::Id;

            // A point ON THE BODY that can be clicked: the centre of the topmost face.
            let aim = |app: &App, body: Id| -> [f64; 3] {
                let wt = app.project.body_display_transform(body, app.current_ctx_id_for_test());
                let f = app
                    .project
                    .regen_faces
                    .get(&body)
                    .and_then(|fs| fs.iter().max_by(|x, y| x.centroid.z.total_cmp(&y.centroid.z)))
                    .expect("the body has faces");
                qymcad_core::feature::apply12(&wt, [f.centroid.x, f.centroid.y, f.centroid.z])
            };

            // A PAIR OF PARTS AND A JOINT BETWEEN THEM, placed by hand. The pairs are spread along X,
            // otherwise anchors "at the origins" seat every hinge on one axis and the mechanism comes
            // out degenerate.
            let pair = |app: &mut App, kind: JointKind, x: f64| -> (Id, Id) {
                let before: Vec<Id> = app.project.bodies.iter().map(|b| b.id).collect();
                super::super::joint_flow::tests::add_part_at(app, x);
                super::super::joint_flow::tests::add_part_at(app, x + 60.0);
                let root = app.project.root;
                app.enter_component(root);
                app.rebuild_if_dirty();
                app.refresh_edges();
                let mine: Vec<Id> = app.project.bodies.iter().map(|b| b.id).filter(|b| !before.contains(b)).collect();
                assert_eq!(mine.len(), 2, "assembly: there should be two bodies of our own, and there are {}", mine.len());
                for (k, b) in mine.iter().enumerate() {
                    if let Some(o) = app.project.body_owner(*b) {
                        if let Some(i) = app.project.component_index(o) {
                            app.project.components[i].transform =
                                [1.0, 0.0, 0.0, x + k as f64 * 60.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0];
                        }
                        if k == 0 {
                            app.project.set_grounded(o, true);
                        }
                    }
                }
                app.rebuild_if_dirty();
                app.refresh_edges();
                let (pa, pb) = (aim(app, mine[0]), aim(app, mine[1]));
                let mut hand = Hand::new(app);
                hand.look_at([x + 30.0, 10.0, 5.0], 5.0).mate(kind).anchor(3).click(pa).click(pb);
                app.rebuild_if_dirty();
                let j = app.project.joints.last().map(|x| x.id).expect("two clicks must create a joint");
                let moving = app.project.body_owner(mine[1]).expect("the owner of the driven part");
                (j, moving)
            };

            // A CONTROL READING: the state of the document BEFORE the assembly part. If it is already
            // in conflict, the joints placed here are not to blame and something else needs looking at.
            let rep0 = app.project.solve_joints();
            eprintln!("ASSEMBLY, CONTROL READING: {} joints, converged={}, refusals {:?}", app.project.joints.len(), !app.project.mates_conflict, rep0.errors);
            // AN AILING JOINT IN THE DOCUMENT STAYS WHERE IT IS - AND THAT NO LONGER GETS IN THE WAY.
            //
            // There used to be an explicit removal of the broken joints here: without it the mechanism
            // did not move at all (0.000 mm and 0.000 deg), because the ban "did not converge - do not
            // move" applied to the WHOLE document at once. The ban became per-part and the crutch is
            // gone: the mechanism must move alongside an ailing joint, and that joint must stay named.
            let _ = &rep0;

            let (slider, slid) = pair(&mut app, JointKind::Slider, 200.0);
            let (hinge_a, _) = pair(&mut app, JointKind::Revolute, 400.0);
            let (rigid, _) = pair(&mut app, JointKind::Rigid, 600.0);
            let (hinge_b, _) = pair(&mut app, JointKind::Revolute, 800.0);
            check_all(&mut app, "assembly: four joints placed by hand", &mut problems);
            for (name, j) in [("slider", slider), ("revolute A", hinge_a), ("rigid", rigid), ("revolute B", hinge_b)] {
                if let Some((_, why)) = app.project.joint_faults().into_iter().find(|(id, _)| *id == j) {
                    problems.push(format!("[assembly] joint \"{name}\" was born faulty: {why}"));
                }
            }

            // A GEAR RELATION between two revolutes - by the same tool a person uses.
            app.start_relation_pick_for_test();
            app.relation_pick_set_for_test(RelationKind::Gear, 2.0);
            app.relation_pick_click_for_test(hinge_a);
            app.relation_pick_click_for_test(hinge_b);
            app.relation_pick_confirm_for_test();
            app.rebuild_if_dirty();
            if app.project.relations.is_empty() {
                problems.push(format!("[assembly] the relation was not created; status: {}", app.status));
            }
            check_all(&mut app, "assembly: the gear relation", &mut problems);

            // THE MECHANISM MUST MOVE. The slider is driven 15 mm and the part must travel them.
            let before = qymcad_core::feature::apply12(&app.project.world_transform(slid), [0.0, 0.0, 0.0]);
            if let Some(j) = app.project.joints.iter_mut().find(|x| x.id == slider) {
                j.drive[1] = Some(15.0);
            }
            app.project.solve_joints();
            let now = qymcad_core::feature::apply12(&app.project.world_transform(slid), [0.0, 0.0, 0.0]);
            let went = ((now[0] - before[0]).powi(2) + (now[1] - before[1]).powi(2) + (now[2] - before[2]).powi(2)).sqrt();
            if (went - 15.0).abs() > 1e-3 {
                problems.push(format!("[assembly] the slider was driven 15 mm and the part travelled {went:.4}"));
            }
            // THE DRIVING WHEEL BY 20 deg -> THE DRIVEN ONE BY 40 deg: the relation must pass the motion on.
            let was_b = app.project.joints.iter().find(|x| x.id == hinge_b).map(|x| x.angle).unwrap_or(0.0);
            let was_a = app.project.joints.iter().find(|x| x.id == hinge_a).map(|x| x.angle).unwrap_or(0.0);
            if let Some(j) = app.project.joints.iter_mut().find(|x| x.id == hinge_a) {
                j.drive[0] = Some(was_a + 20.0);
            }
            app.project.solve_joints();
            let moved_b = app.project.joints.iter().find(|x| x.id == hinge_b).map(|x| x.angle - was_b).unwrap_or(0.0);
            if (moved_b.abs() - 40.0).abs() > 1e-3 {
                problems.push(format!("[assembly] gear ratio 2: the driver by 20 deg, the driven one must go 40 deg, and it went {:.4}", moved_b.abs()));
            }
            // AN AILING JOINT MUST STAY NAMED rather than dissolve. Demanding "the solve converged"
            // here would be wrong: one joint in the document is legitimately ailing, and the right
            // answer from the program is to say so while still letting everything else move.
            let rep = app.project.solve_joints();
            if app.project.mates_conflict && rep.errors.is_empty() {
                problems.push("[assembly] the solve did not converge and the culprits are not named".into());
            }
            check_all(&mut app, "assembly: the mechanism moves", &mut problems);
            eprintln!("ASSEMBLY: the slider travelled {went:.3} mm, the driven wheel {:.3} deg", moved_b.abs());
        }

        // ── THE FINAL DOCUMENT TO LOOK AT: it stays in target ───────────────────────────
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target").join("user-case.qcad").to_string_lossy().into_owned();
        app.set_project_path(out.clone());
        app.save_project();
        app.drain_bg_for_test();
        eprintln!("the scenario document: {out}");

        // ── REMOVE THE NAMED REFUSALS AFTER ALL THE WORK ────────────────────────────────
        //
        // A node can turn red LATER than it was created: a fillet landed well, and the next operation
        // smoothed every edge away - so on the rebuild the fillet had nothing left to round. That is
        // why the cleanup stands here, after every edit, rather than at the moment of the step: any
        // earlier there was simply nothing to catch. Nameless refusals stay - they are findings.
        {
            // PASSES WITH A REBUILD. A node's error is recorded BY THE REBUILD, not at the moment of
            // the step: gathering the list before it means gathering nothing. And once some nodes are
            // removed, others that stood on them can turn red.
            for _ in 0..3 {
                app.rebuild_if_dirty();
                let named: Vec<u64> = app
                    .project
                    .timeline
                    .iter()
                    .filter(|n| app.project.regen_errors.get(&n.id).is_some_and(|e| !matches!(e, qymcad_core::errors::CoreError::OpFailed(_))))
                    .map(|n| n.id)
                    .collect();
                if named.is_empty() {
                    break;
                }
                for id in named {
                    app.project.delete_feature_op(id);
                }
            }
            app.rebuild_if_dirty();
        }

        // A RED NODE NAMES THE PART AND THE OPERATION: "node 75 failed" says nothing, while "the shell
        // failed in part Order B" leads straight to the place.
        let red: Vec<String> = app
            .project
            .regen_errors
            .iter()
            .map(|(id, e)| {
                let node = app.project.timeline.iter().find(|n| n.id == *id);
                let part = node
                    .and_then(|n| n.parent)
                    .and_then(|c| app.project.components.iter().find(|x| x.id == c))
                    .map(|c| crate::i18n::name(&c.name))
                    .unwrap_or_else(|| {
                        // THE NODE REFERENCES A PART THAT DOES NOT EXIST. This is not "outside a part"
                        // but a dangling reference: the component is deleted and the build stayed. Such
                        // a node cannot be shown in the tree - there is nowhere to draw it.
                        format!("a dangling reference to part {:?}", node.and_then(|n| n.parent))
                    });
                let what = node.map(|n| crate::i18n::name(&App::feat_default_name(&n.kind))).unwrap_or_default();
                format!("[{part}] {what} (node {id}): {e:?}")
            })
            .collect();
        assert!(red.is_empty(), "no red nodes may be left in the finished project:\n  {}", red.join("\n  "));
        assert!(problems.is_empty(), "THE USER SCENARIO FOUND {} problems:\n  {}", problems.len(), problems.join("\n  "));
    }
}
