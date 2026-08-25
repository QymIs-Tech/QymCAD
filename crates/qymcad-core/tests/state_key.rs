//! The state key of the model: whether anything worth saving has changed.
//!
//! The application used to compute it by serialising half the project on every frame, and even so the key did
//! not see the structure: an empty part, a rename, a component moved, a joint — none of these made the project
//! count as dirty, and closing the window asked nothing about unsaved work.
use qymcad_core::feature::{JointKind, PLACE_IDENTITY};
use qymcad_core::geom::Point2;
use qymcad_core::model::{Project, WorkPlane};

fn scene() -> Project {
    let mut p = Project::default();
    p.new_document();
    let sid = p.add_line_sketch("s", vec![Point2::new(0.0, 0.0), Point2::new(10.0, 0.0), Point2::new(10.0, 10.0), Point2::new(0.0, 10.0)], true);
    p.add_sketch_node(sid, "Sketch");
    let b = p.add_extrude(sid, 5.0);
    p.finish_base_body(b, 1);
    p
}

/// Every edit has to change the key. Failures accumulate, so all the gaps are visible at once.
#[test]
fn every_user_edit_changes_the_key() {
    let mut bad: Vec<String> = Vec::new();
    let mut check = |label: &str, p: &mut Project, f: &dyn Fn(&mut Project)| {
        let before = p.state_key();
        f(p);
        if p.state_key() == before {
            bad.push(format!("{label}: the key did not change, so the edit is invisible and the project stays clean"));
        }
    };
    let mut p = scene();
    check("an empty part was created", &mut p, &|p| {
        p.add_part("Part X");
    });
    check("a component was renamed", &mut p, &|p| {
        if let Some(c) = p.components.last_mut() {
            c.name = "Another name".into();
        }
    });
    check("a component was moved", &mut p, &|p| {
        let id = p.components.last().map(|c| c.id).unwrap();
        let mut m = PLACE_IDENTITY;
        m[3] = 25.0;
        p.set_component_transform(id, m);
    });
    check("a component was hidden", &mut p, &|p| {
        if let Some(c) = p.components.last_mut() {
            c.visible = false;
        }
    });
    check("a timeline node was suppressed", &mut p, &|p| {
        if let Some(n) = p.timeline.last_mut() {
            n.suppressed = true;
        }
    });
    check("a timeline node was renamed", &mut p, &|p| {
        if let Some(n) = p.timeline.last_mut() {
            n.name = "Extrude 2".into();
        }
    });
    check("a sketch point was moved", &mut p, &|p| {
        if let Some(pt) = p.sketches[0].points.first_mut() {
            pt.x += 1.0;
        }
    });
    check("a datum plane was added", &mut p, &|p| {
        p.add_plane(WorkPlane { name: "Datum".into(), origin: [0.0, 0.0, 7.0], normal: [0.0, 0.0, 1.0], ..Default::default() });
    });
    check("a datum was moved", &mut p, &|p| {
        if let Some(pl) = p.planes.last_mut() {
            pl.origin[2] = 9.0;
        }
    });
    check("an expression of a feature dimension was edited", &mut p, &|p| {
        let id = p.timeline.last().map(|n| n.id).unwrap();
        p.feat_dims.entry(id).or_default().insert("height".into(), "h*2".into());
    });
    check("a joint was added", &mut p, &|p| {
        let (a, b) = (p.components[0].id, p.components[1].id);
        p.add_joint(a, b, JointKind::Rigid);
    });
    check("the angle of a joint was edited", &mut p, &|p| {
        if let Some(j) = p.joints.last_mut() {
            j.angle = 30.0;
        }
    });
    check("a parameter was added", &mut p, &|p| {
        p.parameters.push(qymcad_core::model::Param { name: "h".into(), expr: "10".into(), ..Default::default() });
    });
    check("a component was deleted", &mut p, &|p| {
        let id = p.components.last().map(|c| c.id).unwrap();
        p.components.retain(|c| c.id != id);
    });
    assert!(bad.is_empty(), "the state key is blind to edits:\n{}", bad.join("\n"));
}

/// Derived data stays out of the key: recomputing the regeneration caches must not make the project dirty, or
/// every open and every rebuild would ask to save.
#[test]
fn derived_caches_do_not_change_the_key() {
    let mut p = scene();
    let before = p.state_key();
    let body = p.timeline.iter().find_map(|n| n.kind.body()).expect("the body of the feature");
    p.regen_faces.insert(body, vec![qymcad_core::geom::MeshFace { triangles: vec![0], normal: [0.0, 0.0, 1.0], centroid: qymcad_core::geom::Point3::new(0.0, 0.0, 0.0), area: 1.0, id: 1 }]);
    p.regen_errors.insert(body, qymcad_core::errors::CoreError::SourceBodyNotBuilt);
    assert_eq!(p.state_key(), before, "regeneration caches are derived and have no place in the key");
}

/// The key has to be cheap, being computed on every frame, and what it must never become again is a
/// serialisation of the document.
///
/// MEASURED AGAINST THE DOCUMENT ITSELF, not against the clock. A budget in milliseconds is a property of the
/// machine that runs it: this one used to say "a generous 2 ms" while the real cost was 0.9 to 1.3 ms with
/// spikes to 2.2, so the check went red about once in five runs and said nothing true when it did. Copying
/// the whole document costs about 2.15 ms here, and serialising it costs far more than that - so "the key is
/// cheaper than a copy of what it describes" is the same guard without the machine in it.
#[test]
fn state_key_is_cheap_on_big_project() {
    let mut p = Project::default();
    p.new_document();
    for i in 0..1000 {
        let c = p.add_part(format!("Part {i}"));
        p.set_active_component(Some(c));
        let sid = p.add_line_sketch("s", vec![Point2::new(0.0, 0.0), Point2::new(5.0, 0.0), Point2::new(5.0, 5.0)], true);
        p.add_sketch_node(sid, "Sketch");
    }
    let t = std::time::Instant::now();
    let mut acc = 0u64;
    for _ in 0..10 {
        acc = acc.wrapping_add(p.state_key()); // summed rather than XORed, so identical keys do not cancel out
    }
    let per_call = t.elapsed() / 10;
    let t = std::time::Instant::now();
    for _ in 0..10 {
        std::hint::black_box(p.clone());
    }
    let per_copy = t.elapsed() / 10;
    eprintln!("key {per_call:?} per call, whole-document copy {per_copy:?}");
    assert!(acc != 0, "the key was computed");
    assert!(
        per_call < per_copy,
        "the state key has to be cheaper than copying the document it describes: key {per_call:?}, copy {per_copy:?}"
    );
}

/// The drive of a joint, the "hold as built" flag and the limits are part of the document, not derived from it.
///
/// Neither `drive` nor `as_built` nor the limits entered the key. While a drive moves something the problem is
/// invisible, since the placement changes and that is in the key. But a joint whose drive moved nobody — the
/// same value, the part already there — along with cleared limits and a declaration to hold as built, all
/// passed silently: the document counted as saved and closing it asked nothing. The edit exists and the file
/// does not know about it.
///
/// The mating side (`flip` and `flip_decided`) is deliberately left out: the solver writes it itself, and its
/// presence in the key would turn every solve into an edit outside the boundary of an operation.
#[test]
fn a_mate_value_a_limit_and_as_built_are_part_of_the_document() {
    use qymcad_core::feature::AnchorRef;
    let mut p = Project::default();
    p.new_document();
    let root = p.root;
    p.set_active_component(Some(root));
    let (a, b) = (p.add_part("A"), p.add_part("B"));
    let ca = p.add_connector(a, AnchorRef::Origin);
    let cb = p.add_connector(b, AnchorRef::Origin);
    let jid = p.add_joint(ca, cb, JointKind::Slider);

    let base = p.state_key();
    p.joints.iter_mut().find(|x| x.id == jid).unwrap().drive[1] = Some(7.0);
    let with_drive = p.state_key();
    assert_ne!(with_drive, base, "a travel was driven, so the document has to count as changed");

    p.joints.iter_mut().find(|x| x.id == jid).unwrap().limit_max[1] = Some(50.0);
    let with_limit = p.state_key();
    assert_ne!(with_limit, with_drive, "a limit was set, so the document has to count as changed");

    p.set_joint_as_built(jid);
    assert_ne!(p.state_key(), with_limit, "hold-as-built was declared, so the document has to count as changed");
}
