//! THE REPORTED CASE STRIPPED TO THE MODEL LEVEL: a body, a cut by a profile, a moved sketch point.
//!
//! Measuring on the real file narrowed the chain to its essence: the edit erases 6 FACE names, those
//! take 18 edge names with them, and those take the fillet's references. The body was produced by a
//! cut. Here is the same cut in pure form — to investigate in seconds rather than in a
//! ninety-second scenario.
//!
//! IMPORTANT: the probe must be AT THE MODEL LEVEL. On the raw kernel the names are not recipe-based
//! at all (the model layer assigns them), and the case does not reproduce there — the first attempt
//! failed for exactly that reason.
use qymcad_core::model::Project;

fn names_of(p: &Project, body: u64) -> (Vec<u32>, usize) {
    let ids: Vec<u32> = p.regen_faces.get(&body).map(|f| f.iter().map(|x| x.id).collect()).unwrap_or_default();
    let named = ids.iter().filter(|x| qymcad_core::names::NameTable::is_named(**x)).count();
    (ids, named)
}

#[test]
fn moving_the_cut_sketch_keeps_the_face_names() {
    let mut p = Project::default();
    p.ensure_document();
    let part = p.add_part("Part");
    p.set_active_component(Some(part));

    // THE BODY IS A CYLINDER, as in the real Tube part: a thread needs a round edge.
    let body = p.add_cylinder(20.0, 20.0);

    // THE THREAD is the suspect named by the measurement on the real part: its chain holds two
    // threads, and they are what produce positional faces (32 out of 68), which a plain cut does not
    // produce at all.
    let (_r00, sh00) = qymcad_testkit::regenerate(&mut p);
    let round = p
        .regen_edges
        .get(&body)
        .and_then(|es| es.iter().find(|e| e.radius > 1e-6).map(|e| e.id))
        .unwrap_or(0);
    let body = if round != 0 {
        let t = p.add_thread(body, round, qymcad_core::thread::ThreadSpec::default(), 10.0, 0.0, 0.0);
        eprintln!("SETUP: a thread on edge {round}, body {t}");
        t
    } else {
        eprintln!("SETUP: there is no round edge — nothing to put a thread on");
        body
    };
    let _ = sh00;

    // the cut: a rectangle from 10 to 20 along both axes
    let s2 = p.add_sketch("cut", Vec::new(), None);
    let si2 = p.sketch_index(s2).expect("the cut sketch");
    // TWO PROFILES IN ONE SKETCH — the multi-region path of the cut, the one where the prism used to
    // be built twice. The real part also holds several cuts.
    p.add_rect_entity(si2, 10.0, 10.0, 20.0, 20.0, qymcad_core::feature::Purpose::Real);
    p.add_rect_entity(si2, 25.0, 25.0, 35.0, 35.0, qymcad_core::feature::Purpose::Real);
    p.solve_sketch(si2);
    p.regen_sketch(si2);
    p.add_sketch_node(s2, "cut");
    let profs: Vec<u64> = p.sketches[si2].contour_ids.clone();
    eprintln!("SETUP: {} profiles in the cut sketch", profs.len());
    // ALL profiles at once — the multi-region path of the cut, the one where the prism was built twice.
    let cut = p.add_combine_multi_op(body, s2, profs.clone(), 30.0, 0, qymcad_core::feature::Extent::default(), 0.0, Vec::new());

    // A FILLET ON TOP OF THE CUT — the next step towards the real part: there the fillet stands on the
    // body the cut produced, and its references were the ones falling off.
    let (_r0, sh0) = qymcad_testkit::regenerate(&mut p);
    // THE SHARE OF RECIPE NAMES STEP BY STEP: without it only the total is visible and it is unclear
    // which step loses them.
    for (label, b) in [("thread", body), ("cut", cut)] {
        let (ids, named) = names_of(&p, b);
        eprintln!("NAME SHARE after \"{label}\": from the recipe {named} of {}", ids.len());
    }
    let edges: Vec<u32> = p.regen_edges.get(&cut).map(|e| e.iter().map(|x| x.id).take(4).collect()).unwrap_or_default();
    let fillet = p.add_fillet(cut, 1.0, edges.clone());
    let _ = sh0;
    let (_r, shapes) = qymcad_testkit::regenerate(&mut p);
    let watch = if p.mesh_index(fillet).is_some() { fillet } else { cut };
    let (before, named_before) = names_of(&p, watch);
    eprintln!("SETUP: a fillet on {} edges, watching body {watch}", edges.len());
    assert!(!before.is_empty(), "setup: the body with the cut must have faces");

    // MOVING A POINT OF THE CUT SKETCH — exactly the reported action
    let pi = (0..p.sketches[si2].points.len())
        .max_by(|&a, &b| {
            let (pa, pb) = (&p.sketches[si2].points[a], &p.sketches[si2].points[b]);
            (pa.x * pa.x + pa.y * pa.y).total_cmp(&(pb.x * pb.x + pb.y * pb.y))
        })
        .expect("a point");
    p.sketches[si2].points[pi].x += 1.5;
    p.sketches[si2].points[pi].y += 1.5;
    p.solve_sketch(si2);
    p.regen_sketch(si2);
    p.mark_node_dirty(s2);
    let (_r2, _s2) = qymcad_testkit::regenerate_dirty_with_shapes(&mut p, shapes);

    let (after, named_after) = names_of(&p, watch);
    let survived = before.iter().filter(|x| after.contains(x)).count();
    eprintln!(
        "MEASURED: faces {} -> {}, survived {survived}; from the recipe it was {named_before}, now {named_after}",
        before.len(),
        after.len()
    );
    let lost_named = before
        .iter()
        .filter(|x| qymcad_core::names::NameTable::is_named(**x) && !after.contains(x))
        .count();
    eprintln!("     of which RECIPE names lost: {lost_named}");
    assert_eq!(lost_named, 0, "moving a sketch point erased {lost_named} recipe-based face names");
}
