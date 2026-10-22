//! A PIECE OF A CUT FACE KEEPS ITS NAME WHEN ANOTHER PIECE GOES. A face cut in pieces named its pieces by their place
//! in an order, and an edit that took one piece away renumbered the others: measured over the matrix of edits of the
//! examples, a piece that did not move was renamed after its neighbour went (the filter: the start cap of a
//! revolution cut in three, a gap closed). A piece is named after what cut it off and on which side
//! (`crates/qymcad-kernel/NAMING.md`, "A face split in pieces").
//!
//! A block of width `w` (x from -w/2 to w/2), 20 deep and 10 high; one cut of three slots across its top, at x
//! -25..-20, -5..0 and 15..20, 4 deep: the top is cut in four. `w` from 80 to 50 brings the left slot to the end of the
//! block and the left piece goes; the two pieces between the slots do not move and keep their names - the one between
//! the second and the third slot came after the one that went, and took its number. Live OCCT, headless.
use qymcad_core::feature::{Extent, Purpose, SketchPlane};
use qymcad_core::model::{CombineSpan, Id, Param, PlaneDef, Project, WorkPlane};

/// A PIECE OF THE TOP: where its middle is along X, and how wide it is (all are 20 deep).
struct Spot {
    x: f64,
    width: f64,
}

/// The name of the piece of the top at `spot`, on z = 10.
fn piece_of_the_top(p: &Project, body: Id, spot: &Spot) -> Option<u32> {
    p.regen_faces
        .get(&body)?
        .iter()
        .find(|f| f.normal[2] > 0.99 && (f.centroid.z - 10.0).abs() < 1e-6 && (f.centroid.x - spot.x).abs() < 1e-6 && (f.area - 20.0 * spot.width).abs() < 1e-6)
        .map(|f| f.id)
}

/// How many pieces the top of `body` is in.
fn pieces_of_the_top(p: &Project, body: Id) -> usize {
    p.regen_faces.get(&body).map_or(0, |faces| faces.iter().filter(|f| f.normal[2] > 0.99 && (f.centroid.z - 10.0).abs() < 1e-6).count())
}

#[test]
fn the_pieces_between_the_slots_keep_their_names_when_the_piece_beside_them_goes() {
    let mut p = Project::default();
    p.new_document();
    p.parameters.push(Param { name: "w".into(), expr: "80".into(), value: 80.0 });
    let block = p.add_box(80.0, 20.0, 10.0);
    p.set_feat_dim(block, "dx", "w".into());
    let plane = p.add_plane(WorkPlane { origin: [0.0, 0.0, 10.0], def: PlaneDef::OffsetBase { base: qymcad_core::feature::BasePlane::XY, dist: 10.0 }, ..Default::default() });
    let si = p.new_sketch("slots");
    p.sketches[si].plane = SketchPlane::Datum(plane);
    let sid = p.sketches[si].id;
    p.add_sketch_node(sid, "slots");
    for x in [-25.0, -5.0, 15.0] {
        p.add_rect_entity(si, x, -15.0, x + 5.0, 15.0, Purpose::Real);
    }
    p.regen_sketch(si);
    let slots: Vec<Id> = p.sketches[si].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect();
    assert_eq!(slots.len(), 3, "setup: the three slots are not three contours");
    let cut = p.add_combine_multi_op(block, sid, slots, CombineSpan { height: 0.0, down: 4.0, extent: Extent::default(), fill: &[] }, 0);
    let (report, shapes) = qymcad_testkit::regenerate(&mut p);
    assert!(report.errors.is_empty(), "setup: the block is not built: {:?}", report.errors);
    assert_eq!(pieces_of_the_top(&p, cut), 4, "setup: the top is not cut in four");
    let between = [Spot { x: -12.5, width: 15.0 }, Spot { x: 7.5, width: 15.0 }];
    let before: Vec<u32> = between.iter().map(|spot| piece_of_the_top(&p, cut, spot).expect("setup: a piece between the slots")).collect();

    p.parameters[0].expr = "50".into();
    assert!(p.eval_parameters().is_empty(), "the parameter did not take 50");
    p.mark_param_dependents_dirty_for("w");
    let (report, _) = qymcad_testkit::regenerate_dirty_with_shapes(&mut p, shapes);
    assert!(report.errors.is_empty(), "the rebuild after the edit refused: {:?}", report.errors);
    assert_eq!(pieces_of_the_top(&p, cut), 3, "setup: the left piece of the top did not go");
    let mut renamed = Vec::new();
    for (spot, was) in between.iter().zip(&before) {
        let x = spot.x;
        match piece_of_the_top(&p, cut, spot) {
            Some(now) if now == *was => {}
            Some(now) => renamed.push(format!("the piece at x = {x} was {}, is {}", p.names.describe(*was), p.names.describe(now))),
            None => renamed.push(format!("the piece at x = {x} is gone")),
        }
    }
    assert!(renamed.is_empty(), "pieces that did not move were renamed:\n  {}", renamed.join("\n  "));
}
