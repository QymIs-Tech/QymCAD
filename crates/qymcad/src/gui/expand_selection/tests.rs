//! "Expand the selection" is checked by an item CHANGING THE RECORD IN THE TIMELINE, not by it being
//! drawn.
use super::super::App;
use super::*;
use qymcad_core::feature::FeatureKind;

/// A plate with a built body — INSIDE A PART.
///
/// It used to be built in the root, that is, in an ASSEMBLY, and that was no cosmetic detail: the
/// expand-the-selection menu must not open there at all (it was reported that it could be summoned in
/// assemblies too). While the tests lived in an assembly they could not catch that.
fn plate() -> (App, qymcad_core::model::Id) {
    let mut app = App::default();
    let part = app.project.add_part("part");
    app.enter_component(part);
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
    let body = app.project.timeline.iter().rev().find_map(|n| n.kind.body()).expect("the body");
    (app, body)
}

/// Open a command that WILL ACCEPT a description — otherwise the menu (rightly) stays silent.
fn in_command(app: &mut App, kind: u8, body: qymcad_core::model::Id) {
    app.select_body(body);
    app.start_feat_cmd(kind);
}

/// Select the top face of a body the way a click does it.
fn pick_top_face(app: &mut App, body: qymcad_core::model::Id) -> u32 {
    let f = app.project.regen_faces[&body].iter().max_by(|a, b| a.centroid.z.total_cmp(&b.centroid.z)).expect("the top face");
    let id = f.id;
    app.gsel.faces.insert(id);
    app.gsel.faces_body = Some(body);
    id
}

/// EVERY MENU ITEM IS TRANSLATED — otherwise a person will see a catalogue key.
#[test]
fn every_menu_item_speaks_the_users_language() {
    let prev = crate::i18n::language();
    for (code, _) in crate::i18n::available() {
        crate::i18n::set_language(&code);
        for e in EXPANSIONS {
            let s = crate::i18n::tr(e.key);
            assert!(!s.is_empty() && s != e.key, "in language {code} the item \"{}\" has no translation", e.key);
        }
        let applied = crate::i18n::tr1("expand-applied", "what", "X");
        assert!(applied.contains('X'), "in language {code} the applied message does not substitute the item: {applied}");
    }
    crate::i18n::set_language(&prev);
}

/// ITEMS ARE SHOWN ONLY WHEN THERE IS SOMETHING TO EXPAND — AND SOMEONE TO HAND IT TO.
#[test]
fn the_menu_is_empty_until_something_is_picked() {
    let (mut app, body) = plate();
    in_command(&mut app, 6, body);
    assert!(app.expansion_menu_items().is_empty(), "nothing is selected — there is nothing to offer");
    pick_top_face(&mut app, body);
    assert!(!app.expansion_menu_items().is_empty(), "a face is selected — the items must appear");
}

/// WITHOUT A COMMAND THE MENU DOES NOT OPEN AT ALL.
///
/// It was reported that the menu could be summoned outside any feature simply by right-clicking any
/// face. A description is a way of TELLING A COMMAND what to take; outside a command there is nowhere
/// to record it, and the menu becomes a button leading nowhere: it is pressed, nothing happens, and
/// why is not clear.
#[test]
fn outside_a_command_there_is_no_menu() {
    let (mut app, body) = plate();
    pick_top_face(&mut app, body);
    app.gsel.last_face = Some((body as u32, body)); // and what was pointed at is there too
    assert!(app.cmd.kind == 0 || !app.cmd.active(), "setup: there is no command");
    assert!(app.expansion_menu_items().is_empty(), "outside a command a description has nowhere to go — there is nothing to open the menu on");
}

/// IN AN ASSEMBLY IT DOES NOT OPEN EITHER.
///
/// It was reported that the menu could be summoned outside parts, in assemblies for instance. In an
/// Assembly faces and edges belong to nobody: components are moved and mates are placed there, and
/// there is no one to describe geometry for.
#[test]
fn in_an_assembly_there_is_no_menu() {
    let (mut app, body) = plate();
    in_command(&mut app, 6, body);
    pick_top_face(&mut app, body);
    assert!(!app.expansion_menu_items().is_empty(), "setup: inside a Part the menu is there");

    app.workbench = super::super::Workbench::Assembly;
    assert!(app.expansion_menu_items().is_empty(), "in an Assembly there is nothing to open the menu for");
}

/// EVERY TOOL GETS ONLY WHAT IT WILL ACCEPT.
///
/// It was reported that the menu dumped the entire list regardless of whether it suited the context.
/// The check goes over three breeds of command at once: the fillet takes EDGES, the shell takes A SET
/// OF FACES, push-face takes EXACTLY ONE face.
#[test]
fn each_tool_is_offered_only_what_it_can_take() {
    // THE FILLET — only items that yield edges
    let (mut app, body) = plate();
    in_command(&mut app, 4, body);
    pick_top_face(&mut app, body);
    let keys: Vec<&str> = app.expansion_menu_items().iter().map(|(k, _)| *k).collect();
    assert!(keys.contains(&"expand-face-edges"), "the fillet wants the edges of a face: {keys:?}");
    assert!(!keys.contains(&"expand-parallel"), "there is no way to hand \"all parallel FACES\" to the fillet: {keys:?}");
    assert!(!keys.contains(&"expand-feature-faces"), "\"every face of the feature\" is faces as well: {keys:?}");

    // THE SHELL — only items that yield faces
    let (mut app, body) = plate();
    in_command(&mut app, 6, body);
    pick_top_face(&mut app, body);
    let keys: Vec<&str> = app.expansion_menu_items().iter().map(|(k, _)| *k).collect();
    assert!(keys.contains(&"expand-parallel") && keys.contains(&"expand-feature-faces"), "the shell wants faces: {keys:?}");
    assert!(!keys.contains(&"expand-face-edges"), "the shell has no use for edges: {keys:?}");
    assert!(!keys.contains(&"expand-between"), "a junction yields edges — it does not suit the shell: {keys:?}");

    // PUSH FACE — only items that yield ONE face
    let (mut app, body) = plate();
    in_command(&mut app, 25, body);
    pick_top_face(&mut app, body);
    let keys: Vec<&str> = app.expansion_menu_items().iter().map(|(k, _)| *k).collect();
    assert!(keys.contains(&"expand-topmost") && keys.contains(&"expand-largest"), "push-face can be told \"the topmost one\": {keys:?}");
    assert!(!keys.contains(&"expand-parallel"), "push-face takes ONE face — a set of parallel ones cannot be handed to it: {keys:?}");
    assert!(!keys.contains(&"expand-feature-faces"), "\"every face of the feature\" is a set as well: {keys:?}");
}

/// "EVERY FACE OF THIS FEATURE" IS BUILT FROM THE NAME, NOT FROM THE TABLE OF ITEMS.
///
/// Which feature gave birth to a face is known to the document. The table of items cannot know that,
/// and had it built the query itself it would have substituted the number of the face for the number
/// of the feature — silently and wrongly.
#[test]
fn the_feature_query_is_built_from_the_name_not_from_the_table() {
    let (mut app, body) = plate();
    in_command(&mut app, 6, body); // the shell — takes A SET OF FACES
    let face = pick_top_face(&mut app, body);
    let name = app.project.names.get(face).expect("the face has a structural name");
    let items = app.expansion_menu_items();
    let (_, q) = items.iter().find(|(k, _)| *k == "expand-feature-faces").expect("the item about the feature");
    match q {
        Query::OfFeature { feature, .. } => assert_eq!(*feature, name.feature, "the query must point at the feature that GAVE BIRTH to it"),
        other => panic!("a by-feature query was expected, and out came {other:?}"),
    }
}

/// A MENU ITEM REACHES THE TIMELINE: the feature records A DESCRIPTION rather than a list of numbers.
///
/// That is the whole check of the idea. An item that looks pretty and changes nothing in the document
/// is worse than a missing one.
#[test]
fn choosing_an_item_makes_the_feature_store_a_description() {
    let (mut app, body) = plate();
    in_command(&mut app, 4, body); // the fillet — takes A SET OF EDGES
    let face = pick_top_face(&mut app, body);
    let items = app.expansion_menu_items();
    let (key, q) = items.into_iter().find(|(k, _)| *k == "expand-face-edges").expect("the item about the edges of a face");
    app.apply_expansion(key, q);
    app.gsel.edges = [11, 12].into_iter().collect(); // the highlight of the set stays
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
        Query::Adjacent(inner) => assert!(matches!(**inner, Query::Id(f) if f == face), "the description must refer to the selected face"),
        other => panic!("what got recorded into the timeline is not a description: {other:?}"),
    }
}

/// "ALL PARALLEL" TAKES THE NORMAL OF THE FACE THAT WAS SELECTED rather than just any.
#[test]
fn parallel_uses_the_normal_of_the_face_that_was_picked() {
    let (mut app, body) = plate();
    in_command(&mut app, 6, body); // the shell — takes A SET OF FACES
    pick_top_face(&mut app, body);
    let items = app.expansion_menu_items();
    let (_, q) = items.iter().find(|(k, _)| *k == "expand-parallel").expect("the item about parallel ones");
    match q {
        Query::Oriented { dir, tol_deg } => {
            assert!(dir[2] > 0.9, "the normal of the top face looks upwards, and what was taken is {dir:?}");
            assert!(*tol_deg > 0.0, "a zero tolerance will not find even the face itself: bodies are tessellated");
        }
        other => panic!("a by-direction query was expected, and out came {other:?}"),
    }
}

/// THE MENU IS REALLY DRAWN IN THE VIEWPORT, not merely computed.
///
/// The logic may be right while the item is absent from the screen — that has already happened with
/// the datums, which the help guard missed entirely. The source is checked for the menu being wired to
/// the response of the scene.
#[test]
fn the_menu_is_actually_wired_to_the_viewport() {
    let render = crate::gui::render_source::RENDER;
    assert!(render.contains("resp.context_menu(|ui|"), "the menu must hang on the response of the viewport");
    assert!(render.contains("self.expansion_menu_items()"), "and take the items from the shared table");
    assert!(render.contains("self.apply_expansion(key, q)"), "and apply the chosen item");
}

/// THE COMMAND BAR DOES NOT LIE WITH A NUMBER when the selection is described.
///
/// "Edges: 4" is a snapshot of today. The description "all the edges of this face" will take a fifth
/// one tomorrow, and the number in the bar will become untrue at exactly the moment the description
/// does its job.
#[test]
fn the_command_bar_says_words_when_the_pick_is_described() {
    let panels = crate::gui::panels_source::PANELS;
    let at = panels.find("cmd-edges-n").expect("the caption of the edge set is in place");
    let around = &panels[at.saturating_sub(600)..at + 200];
    assert!(around.contains("self.gsel.described"), "the bar must tell a description from a list");
    assert!(around.contains("expand-described"), "and write it in words rather than as a number");
}

/// THE MENU OPENS EVEN WHERE THE FACE DOES NOT LAND IN THE SET OF FACES.
///
/// Reported behaviour: the right button is pressed and nothing happens. The cause: in the fillet a
/// click on a face puts its EDGES into the set, and the face itself was left nowhere. The menu asked
/// `gsel.faces`, saw emptiness and did not open at all — that is, it worked in exactly the commands
/// where it was not needed and stayed silent in the one it was made for.
#[test]
fn the_menu_opens_in_the_fillet_command_where_only_edges_are_selected() {
    let (mut app, body) = plate();
    in_command(&mut app, 4, body);
    let face = app.project.regen_faces[&body].iter().max_by(|a, b| a.centroid.z.total_cmp(&b.centroid.z)).expect("the top face").id;

    // as after a click on a face in the fillet: EDGES are collected, the set of faces is empty
    app.gsel.edges = [11, 12, 13, 14].into_iter().collect();
    app.gsel.described = None;
    app.gsel.last_face = Some((face, body));
    assert!(app.gsel.faces.is_empty(), "the scene is that very one: there are no faces in the set");

    let items = app.expansion_menu_items();
    assert!(!items.is_empty(), "the menu must open on the last face that was clicked");
    assert!(items.iter().any(|(k, _)| *k == "expand-face-edges"), "and offer \"all the edges of this face\"");
}

/// AN ITEM ALREADY CHOSEN STAYS IN THE MENU — SO THAT THERE IS SOMEWHERE TO MARK IT.
///
/// At first the item that would change nothing was hidden, on the grounds that it is of no use once
/// the face is already selected. Hiding turned out worse: the item is there, then it is not, and WHAT
/// IS RECORDED RIGHT NOW is visible nowhere. A menu must show state, not only offer actions.
#[test]
fn the_chosen_item_stays_in_the_menu_so_it_can_be_marked() {
    let (mut app, body) = plate();
    in_command(&mut app, 4, body);
    let face = pick_top_face(&mut app, body);
    app.gsel.describe_edges_of_face(face);

    let items = app.expansion_menu_items();
    let chosen = items.iter().find(|(k, _)| *k == "expand-face-edges").expect("the chosen item must stay in the list");
    assert_eq!(app.gsel.described.as_ref(), Some(&chosen.1), "and match what is recorded — the mark is drawn from it");
}

/// AND THE MENU REALLY DOES MARK IT.
#[test]
fn the_menu_marks_the_chosen_item() {
    let render = crate::gui::render_source::RENDER;
    assert!(render.contains("ui.selectable_label(on, crate::i18n::tr(key))"), "the item must be drawn with a mark of its state");
}

    /// REPRODUCTION: a shell, then a fillet on its face — a segfault in the program.
    #[test]
    fn shell_then_fillet_on_its_face_does_not_crash() {
        let mut app = super::super::App::default();
        let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        app.project.add_rect_entity(si, 0.0, 0.0, 40.0, 30.0, qymcad_core::feature::Purpose::Real);
        app.project.regen_sketch(si);
        app.finish_sketch_edit();
        app.sel = super::super::Sel::Sketch(si);
        app.start_feat_cmd(1);
        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "height") {
            p.val = 20.0;
            p.txt = "20".into();
        }
        app.apply_feat_cmd();
        app.rebuild_if_dirty();
        let base = app.project.timeline.iter().rev().find_map(|n| n.kind.body()).expect("the body");

        // THE SHELL through the command
        let top = app.project.regen_faces[&base].iter().max_by(|a, b| a.centroid.z.total_cmp(&b.centroid.z)).expect("the top").id;
        app.select_body(base);
        app.start_feat_cmd(6);
        app.gsel.faces.insert(top);
        app.gsel.faces_body = Some(base);
        app.apply_feat_cmd();
        app.rebuild_if_dirty();
        let shell = app.project.timeline.iter().rev().find_map(|n| n.kind.body()).expect("the shell");
        eprintln!("the shell is built: {}", app.project.regen_faces.contains_key(&shell));

        // THE FILLET on a face of the shell — through the command, refreshing the edges as the program does
        app.select_body(shell);
        app.start_feat_cmd(4);
        app.refresh_edges();
        let f = app.project.regen_faces[&shell].iter().max_by(|a, b| a.area.total_cmp(&b.area)).expect("the face").id;
        let eids: Vec<u32> = app
            .project
            .regen_edges
            .get(&shell)
            .map(|es| es.iter().filter(|e| app.project.names.edge(e.id).is_some_and(|n| n.faces.contains(&f))).map(|e| e.id).collect())
            .unwrap_or_default();
        eprintln!("edges of the face: {}", eids.len());
        app.gsel.edges = eids.into_iter().collect();
        app.gsel.describe_edges_of_face(f);
        app.apply_feat_cmd();
        app.rebuild_if_dirty();
        eprintln!("the fillet is applied, nodes {}", app.project.timeline.len());
    }

/// THE TREE SHOWS HOW THE SET WAS DEFINED — in words rather than as a number.
///
/// It was reported that the right-hand panel does not show it, leaving no way to understand what was
/// clicked together. The number "edges: 4" describes a manual set and lies about a description:
/// tomorrow there will be five.
#[test]
fn the_tree_says_how_the_set_was_defined() {
    use qymcad_core::refs::Ref;
    let app = App::default();
    let prev = crate::i18n::language();
    crate::i18n::set_language("ru");

    let by_list = Ref::picks(&[11, 12, 13]);
    let s = app.ref_summary(&by_list);
    assert!(s.contains('3'), "a manual set is described by a number: {s}");

    let by_face = Ref::many(Query::Adjacent(Box::new(Query::Id(7))));
    let s = app.ref_summary(&by_face);
    assert_eq!(s, crate::i18n::tr("expand-face-edges"), "a description must name ITSELF rather than a number");
    assert!(!s.chars().any(|c| c.is_ascii_digit()), "and not show numbers that will change tomorrow: {s}");

    let by_feature = Ref::many(Query::OfFeature { feature: 5, role: None });
    assert_eq!(app.ref_summary(&by_feature), crate::i18n::tr("expand-feature-faces"));
    crate::i18n::set_language(&prev);
}

// -- ITEMS FOR EDGES ------------------------------------------------------------------------------

/// A plate with the fillet command open and live edges; returns (the application, the body).
fn plate_in_fillet() -> (App, qymcad_core::model::Id) {
    let (mut app, body) = plate();
    app.select_body(body);
    app.start_feat_cmd(4);
    app.refresh_edges();
    (app, body)
}

/// HOVERING AN EDGE MAKES THE MENU OFFER A CHAIN RATHER THAN ITEMS ABOUT A FACE.
///
/// Before this step the menu honestly had nothing to offer in edge commands: a click on a face already
/// records "all its edges", and the rest cannot be said with a gesture.
#[test]
fn hovering_an_edge_offers_the_tangent_chain() {
    let (mut app, body) = plate_in_fillet();
    let edge = app.project.regen_edges[&body].first().expect("an edge").id;
    app.gsel.last_edge = Some((edge, body));

    let items = app.expansion_menu_items();
    let keys: Vec<&str> = items.iter().map(|(k, _)| *k).collect();
    assert!(keys.contains(&"expand-tangent-chain"), "on an edge there must be an item about the chain: {keys:?}");
    assert!(!keys.contains(&"expand-parallel"), "there is no point offering items about a FACE on an edge: {keys:?}");

    let q = items.iter().find(|(k, _)| *k == "expand-tangent-chain").map(|(_, q)| q.clone()).expect("the query of the item");
    match q {
        qymcad_core::refs::Query::TangentChain { ref seed, tol_deg } => {
            assert!(matches!(**seed, qymcad_core::refs::Query::Id(e) if e == edge), "the chain must grow from THAT edge");
            assert!(tol_deg > 0.0, "the tolerance must be non-zero: strict collinearity would break the chain on noise");
        }
        other => panic!("a chain was expected, and out came {other:?}"),
    }
}

/// THE CHAIN IS WRITTEN INTO THE TIMELINE AS A DESCRIPTION rather than a snapshot of today's edges.
#[test]
fn the_chain_is_written_as_a_description() {
    let (mut app, body) = plate_in_fillet();
    let edge = app.project.regen_edges[&body].first().expect("an edge").id;
    app.gsel.last_edge = Some((edge, body));
    let (key, q) = app.expansion_menu_items().into_iter().find(|(k, _)| *k == "expand-tangent-chain").expect("the item");
    app.apply_expansion(key, q);
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
    assert!(matches!(edges.query, qymcad_core::refs::Query::TangentChain { .. }), "a description must land in the timeline: {:?}", edges.query);
    assert!(!edges.query.is_pick_list(), "this is not a list of picks");
}

/// THE JUNCTION WAITS FOR THE SECOND FACE rather than creating a reference from one.
#[test]
fn the_seam_asks_for_the_second_face() {
    let (mut app, body) = plate_in_fillet();
    let top = pick_top_face(&mut app, body);
    app.gsel.last_face = Some((top, body));

    let (key, q) = app.expansion_menu_items().into_iter().find(|(k, _)| *k == "expand-between").expect("the junction item");
    app.apply_expansion(key, q);
    assert_eq!(app.gsel.between_first, Some(top), "the first side of the junction must be remembered");
    assert!(app.gsel.described.is_none(), "a reference cannot be built from ONE face — a junction has two sides");
    assert_eq!(app.status, crate::i18n::tr("expand-between-pick-second"), "a person must be told what is expected of them");

    // the second pick — now the reference does get assembled
    let side = app.project.regen_faces[&body].iter().find(|f| f.normal[2].abs() < 0.1).expect("a side face").id;
    let q2 = qymcad_core::refs::Query::Between(
        Box::new(qymcad_core::refs::Query::Id(top)),
        Box::new(qymcad_core::refs::Query::Id(side)),
    );
    app.gsel.between_first = None;
    app.apply_expansion("expand-between-done", q2);
    match app.gsel.described {
        Some(qymcad_core::refs::Query::Between(ref a, ref b)) => {
            assert!(matches!(**a, qymcad_core::refs::Query::Id(f) if f == top), "the first side is the one pointed at first");
            assert!(matches!(**b, qymcad_core::refs::Query::Id(f) if f == side), "the second is the one pointed at second");
        }
        ref other => panic!("after the second pick there must be a junction, and out came {other:?}"),
    }
}

/// AN UNFINISHED WAIT DOES NOT OUTLIVE THE COMMAND.
///
/// Otherwise the next command would silently eat the very first click on a face, taking it for the
/// second side of the junction — and a person would not understand why the selection did not work.
#[test]
fn a_half_finished_seam_does_not_leak_into_the_next_command() {
    let (mut app, body) = plate_in_fillet();
    let top = pick_top_face(&mut app, body);
    app.gsel.last_face = Some((top, body));
    let (key, q) = app.expansion_menu_items().into_iter().find(|(k, _)| *k == "expand-between").expect("the item");
    app.apply_expansion(key, q);
    assert!(app.gsel.between_first.is_some(), "setup: the wait is switched on");

    app.cancel_all_tools();
    assert!(app.gsel.between_first.is_none(), "the wait for a second face must end together with the command");
    assert!(app.gsel.last_edge.is_none() && app.gsel.last_face.is_none(), "and so must the memory of what was pointed at");
}


/// THE WHOLE POINT, END TO END: describe "all the edges of the top face" -> edit the sketch -> the
/// fillet picks up the new edge.
///
/// This was required as a step of its own ("a description outlives an edit after which there are more
/// elements — as in `refs/tests.rs`, but THROUGH THE INTERFACE"), and there was no guard for it: the
/// kernel was checked on a synthetic pool, the interface on the shape of the query. Between "a query
/// of the right shape" and "the fillet really took the fifth edge" fits the whole of associativity,
/// which is what all of this is for.
///
/// The edit is deliberately of the kind a person makes: one side of the contour is split by a point in
/// the middle (two collinear segments are merged by the kernel into ONE edge), the point is moved
/// sideways — and there are more edges without a single new feature.
#[test]
fn a_description_picks_up_an_edge_that_appeared_after_a_sketch_edit() {
    let mut app = App::default();
    let part = app.project.add_part("part");
    app.enter_component(part);
    let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
    // a square whose bottom side is split by a point in the middle (while collinear there is one edge)
    app.project.add_line_entity(si, 0.0, 0.0, 30.0, 0.0, qymcad_core::feature::Purpose::Real);
    app.project.add_line_entity(si, 30.0, 0.0, 60.0, 0.0, qymcad_core::feature::Purpose::Real);
    app.project.add_line_entity(si, 60.0, 0.0, 60.0, 40.0, qymcad_core::feature::Purpose::Real);
    app.project.add_line_entity(si, 60.0, 40.0, 0.0, 40.0, qymcad_core::feature::Purpose::Real);
    app.project.add_line_entity(si, 0.0, 40.0, 0.0, 0.0, qymcad_core::feature::Purpose::Real);
    app.project.regen_sketch(si);
    app.finish_sketch_edit();
    app.sel = super::super::Sel::Sketch(si);
    app.start_feat_cmd(1);
    if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "height") {
        p.val = 10.0;
        p.txt = "10".into();
    }
    app.apply_feat_cmd();
    app.rebuild_if_dirty();
    let body = app.project.timeline.iter().rev().find_map(|n| n.kind.body()).expect("the body");

    // A FILLET BY DESCRIPTION: the top face was clicked -> "all the edges of this face"
    in_command(&mut app, 4, body);
    let top = app.project.regen_faces[&body].iter().filter(|f| f.normal[2] > 0.9).max_by(|a, b| a.centroid.z.total_cmp(&b.centroid.z)).expect("the top face").id;
    app.gsel.last_face = Some((top, body));
    let (key, q) = app.expansion_menu_items().into_iter().find(|(k, _)| *k == "expand-face-edges").expect("the item \"all the edges of this face\"");
    app.apply_expansion(key, q);
    if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "radius") {
        p.val = 1.0;
        p.txt = "1".into();
    }
    app.apply_feat_cmd();
    app.rebuild_if_dirty();

    let fillet = app
        .project
        .timeline
        .iter()
        .find_map(|n| match n.kind {
            FeatureKind::Fillet { src, ref edges, .. } => Some((src, edges.clone())),
            _ => None,
        })
        .expect("the fillet in the timeline");
    let before = app.project.resolve_edge_refs(fillet.0, &fillet.1, "ref-what-fillet-edge").expect("the description must resolve");
    assert_eq!(before.len(), 4, "a rectangular top face has four edges, and out came {}: {before:?}", before.len());

    // THE SKETCH EDIT: the middle point of the bottom side is moved sideways — collinearity is over
    for pt in &mut app.project.sketches[si].points {
        if (pt.x - 30.0).abs() < 1e-9 && pt.y.abs() < 1e-9 {
            pt.y = -8.0;
        }
    }
    app.project.solve_sketch(si);
    app.project.regen_sketch(si);
    app.project.mark_sketch_dirty(app.project.sketches[si].id);
    app.rebuild_if_dirty();

    let fillet = app
        .project
        .timeline
        .iter()
        .find_map(|n| match n.kind {
            FeatureKind::Fillet { src, ref edges, .. } => Some((src, edges.clone())),
            _ => None,
        })
        .expect("the fillet is in place");
    let after = app.project.resolve_edge_refs(fillet.0, &fillet.1, "ref-what-fillet-edge").expect("the description must resolve after the edit too");
    assert!(
        after.len() > before.len(),
        "there are more edges now — the description must PICK THEM UP, while a snapshot would not: there were {}, there are {}",
        before.len(),
        after.len()
    );
    assert!(matches!(fillet.1.query, Query::Adjacent(_)), "the timeline must hold A DESCRIPTION rather than a list: {:?}", fillet.1.query);
    assert!(!app.project.regen_errors.values().any(|_| true), "the part must stay built: {:?}", app.project.regen_errors);
}

/// ORBITING THE CAMERA WITH THE RIGHT BUTTON IS NOT BROKEN BY THE MENU.
///
/// This needed sorting out: the right button in the scene is taken by the orbit, and the menu must
/// live on a SHORT click without a drag. The sorting is done by egui itself — `context_menu` fires on
/// a click and the orbit on `dragged()` — but it rests on the two handlers being separated onto
/// different events. The guard pins down exactly that separation: if one day the menu is hung on the
/// press, the orbit will die silently, and only a person with a mouse will catch it.
#[test]
fn the_right_button_still_orbits_the_camera() {
    let render = crate::gui::render_source::RENDER;
    assert!(render.contains("resp.context_menu(|ui|"), "the menu must hang on the CLICK (context_menu) rather than on the press");
    assert!(render.contains("} else if resp.dragged() {"), "the orbit must live on the DRAG");
    assert!(render.contains("self.cam.yaw -= d.x as f64 * 0.01;"), "and really turn the camera");
    // and the menu must not dare open in response to a drag
    assert!(!render.contains("resp.dragged() && self.expansion_accepts()"), "a menu on the drag would take the camera rotation away");
}
