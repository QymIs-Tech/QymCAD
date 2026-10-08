//! The state key of the model: whether anything worth saving has changed.
//!
//! The application used to compute it by serialising half the project on every frame, and even so the key did
//! not see the structure: an empty part, a rename, a component moved, a joint — none of these made the project
//! count as dirty, and closing the window asked nothing about unsaved work.
use qymcad_core::feature::{JointKind, PLACE_IDENTITY};
use qymcad_core::geom::Point2;
use qymcad_core::model::{to_ron, Project, WorkPlane};

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
    check("a label was placed in a sketch", &mut p, &|p| {
        p.sketches[0].texts.push(qymcad_core::model::SketchText {
            id: 900,
            x: 0.0,
            y: 0.0,
            height: 5.0,
            angle: 0.0,
            text: "A".into(),
            construction: false,
            glyphs: Vec::new(),
            font: Default::default(),
        });
    });
    check("a label was retyped", &mut p, &|p| {
        if let Some(t) = p.sketches[0].texts.last_mut() {
            t.text = "B".into();
        }
    });
    check("a line was turned into construction", &mut p, &|p| {
        p.sketches[0].entities[0].construction = true;
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
    check("a relation was made between two mates", &mut p, &|p| {
        let j = p.joints.last().map(|j| j.id).unwrap();
        p.add_relation(qymcad_core::feature::RelationKind::Gear, j, 0, j, 0, 2.0);
    });
    check("the number of a relation was edited", &mut p, &|p| {
        if let Some(r) = p.relations.last_mut() {
            r.value = 3.0;
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
/// machine that runs it: the first edition said "a generous 2 ms" while the real cost was 0.9 to 1.3 ms with
/// spikes to 2.2, so it went red about once in five runs and said nothing true when it did.
///
/// AGAINST SERIALISING IT, NOT AGAINST COPYING IT, and the reason is a measurement rather than taste. The
/// second edition compared the key with `clone`, which then cost about 2.15 ms - a margin of two. Copying has
/// since become cheaper: measured 05.09.2026 on a thousand parts, key 1.06 ms against copy 1.25 ms. A margin
/// of a fifth is not a check, it is a coin flip, and it duly came up tails during a full workspace run while
/// passing five times in a row on its own. Nothing about the code had changed.
///
/// Serialising the same document costs 24.9 ms - twenty-three times the key, and it stays that way because
/// the two do different amounts of work by nature. That is the sentence at the top of this comment, measured.
/// The copy is still timed and still reported, because a key that grew past it would be worth looking at -
/// but it is written into the message, not into the assertion.
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
    // THE FASTEST OF TEN, NOT THE AVERAGE OF TEN. Scheduling noise only ever makes a measurement SLOWER,
    // so the minimum is the closest thing to the true cost and the only figure that does not depend on what
    // else the machine is doing.
    //
    // As an average this check was a race. Measured on a quiet machine: key 1.08-1.20 ms against copy
    // 1.41-1.55 ms - a margin of about a quarter, which the rest of the workspace running in parallel
    // swallows whole. It went red during a full run and green on its own five times in a row, which says
    // nothing about the code and is exactly what a check must not do.
    // THE FASTEST OF TEN, NOT THE AVERAGE. Scheduling noise only ever makes a measurement SLOWER, so the
    // minimum is the closest thing to the true cost and the only figure that does not depend on what else the
    // machine happens to be running.
    let mut acc = 0u64;
    let mut per_call = std::time::Duration::MAX;
    for _ in 0..10 {
        let t = std::time::Instant::now();
        acc = acc.wrapping_add(p.state_key()); // summed rather than XORed, so identical keys do not cancel out
        per_call = per_call.min(t.elapsed());
    }
    let mut per_copy = std::time::Duration::MAX;
    for _ in 0..10 {
        let t = std::time::Instant::now();
        std::hint::black_box(p.clone());
        per_copy = per_copy.min(t.elapsed());
    }
    let t = std::time::Instant::now();
    let text = to_ron(&p).expect("the document serialises");
    let per_write = t.elapsed();
    eprintln!("key {per_call:?}, whole-document copy {per_copy:?}, serialisation {per_write:?} ({} bytes)", text.len());
    assert!(acc != 0, "the key was computed");
    // A FIFTH of the serialisation, and the real figure is a twenty-third. The threshold is set where a
    // regression would have to be a change of kind - the key having started to walk the document the way a
    // writer does - rather than a change of a few per cent that says nothing.
    assert!(
        per_call * 5 < per_write,
        "the state key has come within a fifth of serialising the document it describes, which is what it must never become: \
         key {per_call:?}, serialisation {per_write:?} (copy, for reference, {per_copy:?})"
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

/// Reported behaviour (issue #101): these edits change what is saved and left the key as it was, so the document
/// still looked saved, closing it asked nothing and the edit was lost; autosave skipped it and it made no undo step.
/// Failures accumulate, as above.
#[test]
fn edits_the_key_used_to_miss_change_it() {
    use qymcad_core::feature::{AnchorRef, FaceKey, Purpose, SketchPlane};
    use qymcad_core::model::{Constraint, Note, PlaneDef, Spline};
    let mut bad: Vec<String> = Vec::new();
    let mut check = |label: &str, p: &mut Project, f: &dyn Fn(&mut Project)| {
        let before = p.state_key();
        f(p);
        if p.state_key() == before {
            bad.push(format!("{label}: the key did not change, so the edit is invisible and the project stays clean"));
        }
    };
    let mut p = scene();
    let (a, b) = (p.add_part("A"), p.add_part("B"));
    let body = p.timeline.iter().find_map(|n| n.kind.body()).expect("the body of the feature");
    check("the title was edited", &mut p, &|p| p.meta.title = "Bracket".into());
    check("the author was edited", &mut p, &|p| p.meta.author = "Somebody".into());
    check("the comment was edited", &mut p, &|p| p.meta.comment = "for the left side".into());
    check("a part was coloured", &mut p, &|p| {
        p.part_colors.insert(a, [200, 30, 30]);
    });
    check("a face was coloured", &mut p, &|p| {
        p.face_colors.insert(body, vec![(1, [30, 200, 30])]);
    });
    check("triangles were coloured", &mut p, &|p| {
        p.tri_colors.insert(body, (vec![[30, 30, 200]], vec![0]));
    });
    check("the mesh quality was changed", &mut p, &|p| p.geom_quality = qymcad_core::model::GeomQuality::Draft);
    check("a circle was drawn", &mut p, &|p| {
        p.add_circle_entity(0, 50.0, 50.0, 5.0, Purpose::Real);
    });
    check("the radius of a circle was changed", &mut p, &|p| {
        for e in &mut p.sketches[0].entities {
            if let qymcad_core::model::EntityKind::Circle { r, .. } = &mut e.kind {
                *r = 8.0;
            }
        }
    });
    check("a note was written in a sketch", &mut p, &|p| p.sketches[0].notes.push(Note { x: 1.0, y: 2.0, text: "check".into() }));
    check("a note was retyped", &mut p, &|p| p.sketches[0].notes[0].text = "checked".into());
    check("a spline was drawn", &mut p, &|p| {
        let pts: Vec<_> = p.sketches[0].points.iter().take(3).map(|q| q.id).collect();
        p.sketches[0].splines.push(Spline { points: pts, tangents: vec![None; 3], closed: false, construction: false });
    });
    check("a tangent of a spline was set", &mut p, &|p| p.sketches[0].splines[0].tangents[1] = Some([1.0, 0.0]));
    check("two points were made level", &mut p, &|p| {
        let (pa, pb) = (p.sketches[0].points[0].id, p.sketches[0].points[1].id);
        p.sketches[0].constraints.push(Constraint::Horizontal { a: pa, b: pb });
    });
    check("a level constraint was turned upright", &mut p, &|p| {
        if let Some(Constraint::Horizontal { a, b }) = p.sketches[0].constraints.last().cloned() {
            *p.sketches[0].constraints.last_mut().unwrap() = Constraint::Vertical { a, b };
        }
    });
    check("a sketch was put on a face", &mut p, &|p| {
        p.sketches[0].plane = SketchPlane::Face(body, FaceKey { index: 1, centroid: [0.0, 0.0, 5.0], normal: [0.0, 0.0, 1.0], id: 0 });
    });
    check("the sketch was moved to another face of the same body", &mut p, &|p| {
        p.sketches[0].plane = SketchPlane::Face(body, FaceKey { index: 2, centroid: [5.0, 0.0, 2.5], normal: [1.0, 0.0, 0.0], id: 0 });
    });
    check("a datum plane was added", &mut p, &|p| {
        p.add_plane(WorkPlane { name: "Datum".into(), origin: [0.0, 0.0, 7.0], normal: [0.0, 0.0, 1.0], ..Default::default() });
    });
    check("the offset of a datum plane was retyped", &mut p, &|p| {
        p.planes.last_mut().unwrap().def = PlaneDef::OffsetBase { base: Default::default(), dist: 12.0 };
    });
    let ca = p.add_connector(a, AnchorRef::Origin);
    p.add_connector(b, AnchorRef::Origin);
    check("a connector was shifted on its part", &mut p, &|p| p.connectors.iter_mut().find(|c| c.id == ca).unwrap().offset_xyz = [0.0, 0.0, 3.0]);
    check("a connector was flipped", &mut p, &|p| p.connectors.iter_mut().find(|c| c.id == ca).unwrap().flip = true);
    check("a connector was renamed", &mut p, &|p| p.connectors.iter_mut().find(|c| c.id == ca).unwrap().name = "Bore".into());
    check("two parts were grouped", &mut p, &|p| {
        p.add_group(&[a, b]);
    });
    check("the group was deleted", &mut p, &|p| {
        let g = p.mate_constraints.last().map(|g| g.id).expect("the group");
        p.delete_group(g);
    });
    assert!(bad.is_empty(), "the state key is blind to edits:\n{}", bad.join("\n"));
}

/// What changes the document and builds nothing - its properties, colours, notes, a connector shifted - asks to save
/// and rebuilds nothing: it is in the state key and not in the rebuild key. (The mesh quality is rebuilt: the bodies
/// are meshed to it.)
#[test]
fn what_builds_nothing_is_saved_and_not_rebuilt() {
    use qymcad_core::feature::AnchorRef;
    use qymcad_core::model::Note;
    let mut p = scene();
    let a = p.add_part("A");
    let ca = p.add_connector(a, AnchorRef::Origin);
    let check = |label: &str, p: &mut Project, edit: &dyn Fn(&mut Project)| {
        let (saved, built) = (p.state_key(), p.rebuild_key());
        edit(p);
        assert_ne!(p.state_key(), saved, "{label}: the document has to count as changed");
        assert_eq!(p.rebuild_key(), built, "{label}: nothing that is built changed, so nothing may be rebuilt");
    };
    check("the title was edited", &mut p, &|p| p.meta.title = "Bracket".into());
    check("a part was coloured", &mut p, &|p| {
        p.part_colors.insert(a, [200, 30, 30]);
    });
    check("a note was written in a sketch", &mut p, &|p| p.sketches[0].notes.push(Note { x: 1.0, y: 2.0, text: "check".into() }));
    check("a connector was shifted on its part", &mut p, &|p| p.connectors.iter_mut().find(|c| c.id == ca).unwrap().offset_xyz = [0.0, 0.0, 3.0]);
}

/// What the program writes itself - the rebuild, the solver - is not an edit: none of it may make the document look
/// changed, or opening one and touching nothing would ask to save.
#[test]
fn what_the_program_writes_does_not_change_the_key() {
    use qymcad_core::feature::{AnchorRef, FaceKey, SketchPlane};
    let mut p = scene();
    let body = p.timeline.iter().find_map(|n| n.kind.body()).expect("the body of the feature");
    p.sketches[0].plane = SketchPlane::Face(body, FaceKey { index: 1, centroid: [0.0, 0.0, 5.0], normal: [0.0, 0.0, 1.0], id: 7 });
    let (a, b) = (p.add_part("A"), p.add_part("B"));
    let (ca, cb) = (p.add_connector(a, AnchorRef::Origin), p.add_connector(b, AnchorRef::Origin));
    p.add_joint(ca, cb, JointKind::Rigid);
    let (pa, pb) = (p.sketches[0].points[0].id, p.sketches[0].points[1].id);
    let axis = p.add_axis_two_points(pa, pb);
    p.sketches[0].texts.push(qymcad_core::model::SketchText { id: 900, x: 0.0, y: 0.0, height: 5.0, angle: 0.0, text: "A".into(), construction: false, glyphs: Vec::new(), font: Default::default() });
    let (saved, built) = (p.state_key(), p.rebuild_key());
    // what opening a document runs on it, and what solving and rebuilding a sketch does
    p.settle_loaded();
    p.solve_sketch(0);
    p.regen_sketch(0);
    // the first save stamps when the document was started, after the saved key is taken (`spawn_save`); the file's
    // copy is stamped with the build that wrote it
    p.meta.created = "2026-10-08T09:00:00Z".into();
    p.meta.saved_by = "QymCAD 0.1".into();
    // the rebuild resolves an axis on points, marks nodes for rebuilding, lays out a label's glyphs; the solver
    // decides which way a mate faces
    p.datum_axes.iter_mut().find(|x| x.id == axis).expect("the axis").set_resolved_for_test([0.0, 0.0, 0.0], [1.0, 0.0, 0.0]);
    p.timeline[0].dirty = !p.timeline[0].dirty;
    p.sketches[0].texts[0].glyphs.push(vec![qymcad_core::geom::Point2::new(0.0, 0.0)]);
    let j = p.joints.last_mut().expect("the joint");
    (j.flip, j.roll_flip, j.flip_decided) = (!j.flip, !j.roll_flip, !j.flip_decided);
    p.dead_bodies.push(body);
    p.edge_refs.insert(body, Vec::new());
    p.face_refs.insert(body, Vec::new());
    p.mates_violated.push(body);
    // the sketch rebound to its face under the face's new persistent id, as `rebind_lost_face_refs` does
    if let SketchPlane::Face(_, key) = &mut p.sketches[0].plane {
        key.id = 8;
    }
    assert_eq!(p.state_key(), saved, "what the rebuild and the solver write is not an edit");
    assert_eq!(p.rebuild_key(), built, "what the rebuild and the solver write is not an edit");
}

/// Two equal documents have one key, whatever order their maps iterate in. A map rebuilt entry by entry - read back
/// from a file, say - gets a hash seed of its own and with it an order of its own; counting that order would call an
/// untouched document changed.
#[test]
fn equal_documents_have_one_key_whatever_their_maps_order() {
    let mut p = scene();
    for i in 0..64u8 {
        p.part_colors.insert(1000 + i as u64, [i, 255 - i, 7]);
        p.face_colors.insert(2000 + i as u64, vec![(i as u32, [i, i, i])]);
    }
    let mut q = p.clone();
    let mut part: Vec<_> = p.part_colors.iter().map(|(k, v)| (*k, *v)).collect();
    part.reverse();
    q.part_colors = part.into_iter().collect();
    let mut face: Vec<_> = p.face_colors.iter().map(|(k, v)| (*k, v.clone())).collect();
    face.reverse();
    q.face_colors = face.into_iter().collect();
    assert_ne!(p.part_colors.keys().collect::<Vec<_>>(), q.part_colors.keys().collect::<Vec<_>>(), "the copy was meant to iterate in another order; with 64 entries and a fresh seed it does");
    assert_eq!(p.state_key(), q.state_key(), "the same document read in another order is the same document");
}
