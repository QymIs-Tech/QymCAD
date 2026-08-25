//! A SURFACE THROUGH SECTIONS: the same loft, not closed into a solid.
//!
//! An outline through sections is the most common design-layer job: a rim, a bonnet, a transition
//! between two outlines. It becomes a solid later — by a thickness or by being stitched to its
//! neighbours — and until then it must be a SHEET.
//!
//! There is no separate tool for this and none is needed: the question is the same as for a loft
//! ("what should come out of it"), and splitting it across two buttons would mean asking a person to
//! choose a tool before they have answered that question.
use qymcad_core::model::Project;

/// Two square sections at different heights: 40x40 at the bottom, 20x20 at the top. Returns
/// (project, sids, cids per section).
fn two_sections() -> (Project, Vec<u64>, Vec<u64>) {
    let mut p = Project::default();
    p.new_document();
    let mut sids = Vec::new();
    let mut cids = Vec::new();
    for (i, (half, z)) in [(20.0_f64, 0.0_f64), (10.0, 30.0)].into_iter().enumerate() {
        let si = p.new_sketch(&format!("section {}", i + 1));
        let sid = p.sketches[si].id;
        if z != 0.0 {
            let pl = p.add_plane(qymcad_core::model::WorkPlane { id: 0, name: format!("z{z}"), origin: [0.0, 0.0, z], normal: [0.0, 0.0, 1.0], rot_deg: 0.0, def: Default::default() });
            p.sketches[si].plane = qymcad_core::feature::SketchPlane::Datum(pl);
        }
        p.add_rect_entity(si, -half, -half, half, half, qymcad_core::feature::Purpose::Real);
        p.regen_sketch(si);
        p.add_sketch_node(sid, &format!("section {}", i + 1));
        let cid = p.sketches[si].contour_ids.iter().copied().find(|c| p.contour_profile_xy(*c).is_some()).expect("the section contour");
        sids.push(sid);
        cids.push(cid);
    }
    (p, sids, cids)
}

/// A SURFACE IS A SHEET AND A SOLID IS A SOLID. The same set of sections; the only difference is the
/// answer to "what should come out of it".
#[test]
fn the_same_sections_give_a_sheet_or_a_solid() {
    let (mut p, sids, cids) = two_sections();
    let sheet = p.add_loft(sids.clone(), cids.clone(), false, 0, 0, true);
    let (rep, _) = qymcad_testkit::regenerate(&mut p);
    assert!(rep.errors.is_empty(), "a surface through sections must build: {:?}", rep.errors);
    let s = p.bodies.iter().find(|b| b.id == sheet).expect("the surface became a body of the document");
    assert!(s.sheet, "the result must be a SHEET — otherwise it is an ordinary loft under another name");
    assert!(!s.mesh.tris.is_empty(), "and it has geometry");

    let (mut p2, sids2, cids2) = two_sections();
    let solid = p2.add_loft(sids2, cids2, false, 0, 0, false);
    let (rep, _) = qymcad_testkit::regenerate(&mut p2);
    assert!(rep.errors.is_empty(), "setup — an ordinary loft: {:?}", rep.errors);
    let v = p2.bodies.iter().find(|b| b.id == solid).expect("the solid").mesh.volume();
    assert!(v > 1.0, "an ordinary loft must stay a SOLID with a volume, and it came out {v:.3}");

    // THEIR SIDES ARE THE SAME: the difference is in the caps, not in the shape of the outline
    let a_sheet: f64 = p.regen_faces[&sheet].iter().map(|f| f.area).sum();
    let a_solid: f64 = p2.regen_faces[&solid].iter().map(|f| f.area).sum();
    let caps = 40.0 * 40.0 + 20.0 * 20.0;
    assert!((a_solid - a_sheet - caps).abs() < 5.0, "the solid differs from the surface by exactly two caps: {a_solid:.1} against {a_sheet:.1} plus {caps:.1}");
}

/// A SURFACE CAN BE GIVEN A THICKNESS — and it becomes an ordinary body like everything else in the
/// timeline.
#[test]
fn the_lofted_surface_can_be_thickened() {
    let (mut p, sids, cids) = two_sections();
    let sheet = p.add_loft(sids, cids, false, 0, 0, true);
    let (rep, _) = qymcad_testkit::regenerate(&mut p);
    assert!(rep.errors.is_empty(), "setup: {:?}", rep.errors);

    let face = p.regen_faces[&sheet].first().map(|f| f.id).expect("a face of the surface");
    let solid = p.add_thicken(sheet, face, 1.0);
    let (rep, _) = qymcad_testkit::regenerate(&mut p);
    assert!(rep.errors.is_empty(), "the outline must thicken: {:?}", rep.errors);
    let out = p.bodies.iter().find(|b| b.id == solid).expect("the solid");
    assert!(!out.sheet, "after a thickness this is a solid");
    assert!(out.mesh.volume() > 1.0, "and it has a volume: {:.2}", out.mesh.volume());
}

/// THE SURFACE FOLLOWS ITS SECTIONS: edit the sketch and the outline changes.
#[test]
fn the_lofted_surface_follows_its_sections() {
    let (mut p, sids, cids) = two_sections();
    let sheet = p.add_loft(sids, cids, false, 0, 0, true);
    let (rep, _) = qymcad_testkit::regenerate(&mut p);
    assert!(rep.errors.is_empty(), "setup: {:?}", rep.errors);
    let before: f64 = p.regen_faces[&sheet].iter().map(|f| f.area).sum();

    let sid = p.sketches[0].id;
    for pt in &mut p.sketches[0].points {
        if pt.x.abs() > 19.0 {
            pt.x *= 2.0;
        }
    }
    p.solve_sketch(0);
    p.regen_sketch(0);
    p.mark_sketch_dirty(sid);
    let (rep, _) = qymcad_testkit::regenerate(&mut p);
    assert!(rep.errors.is_empty(), "after the section edit the outline must rebuild: {:?}", rep.errors);
    let after: f64 = p.regen_faces[&sheet].iter().map(|f| f.area).sum();
    assert!(after > before + 10.0, "the lower section became wider — the outline must grow: {after:.1} against {before:.1}");
}
