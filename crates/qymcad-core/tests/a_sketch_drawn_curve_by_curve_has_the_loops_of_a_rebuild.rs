//! A SKETCH DRAWN CURVE BY CURVE HAS THE LOOPS OF A REBUILD: every kind of sketch, and a half disc of 48 segments closed
//! by its diameter, laid one curve at a time as a person draws them; after each curve the loops a rebuild of the change
//! makes (`regen_sketch`, the loops round what changed) are those of a rebuild from all the curves.
//!
//! Reported behaviour: the revolve of a half disc drawn segment by segment found no closed profile. The diameter drawn
//! last was laid in a window with only the two segments beside its ends, which hung there clear of the sides of the
//! window; the walk round them was left out, and the half disc with it.
mod sketch_kinds;

use qymcad_core::feature::Purpose;
use qymcad_core::geom::Point2;
use qymcad_core::model::Project;
use sketch_kinds::{build, Kind};

/// The loops of the first sketch as a sorted list of what each is: closed, its entities, its centroid and area to 1e-6.
fn loops(p: &Project) -> Vec<String> {
    let mut out: Vec<String> = p.sketches[0]
        .contour_ids
        .iter()
        .filter_map(|cid| {
            let c = &p.contours[p.contour_index(*cid)?];
            let mut src = c.edge_src.clone();
            src.sort_unstable();
            src.dedup();
            let n = c.points.len().max(1) as f64;
            let (sx, sy) = c.points.iter().fold((0.0, 0.0), |(a, b), q| (a + q.x, b + q.y));
            Some(format!("{} {:?} ({:.6}, {:.6}) {:.6}", c.closed, src, sx / n, sy / n, c.signed_area().abs()))
        })
        .collect();
    out.sort();
    out
}

/// The sketch of `from` laid again one curve at a time - its points first, then each curve with a rebuild - and after
/// each curve the loops held against a rebuild from all the curves.
fn drawn_curve_by_curve(from: &Project, what: &str, failures: &mut Vec<String>) {
    let mut p = Project::default();
    p.new_document();
    let _ = p.new_sketch("S");
    p.sketches[0].points = from.sketches[0].points.clone();
    for (k, e) in from.sketches[0].entities.iter().enumerate() {
        p.sketches[0].entities.push(e.clone());
        p.regen_sketch(0);
        let mut whole = p.clone();
        whole.regen_sketch_whole(0);
        let (made, rebuilt) = (loops(&p), loops(&whole));
        if made != rebuilt {
            failures.push(format!("{what}, curve {k}: {} loops against {} of a rebuild; first apart: {:?}", made.len(), rebuilt.len(), made.iter().zip(&rebuilt).find(|(a, b)| a != b)));
            return;
        }
    }
}

/// A half disc of 48 segments on the X axis, its diameter drawn last.
fn half_disc() -> Project {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("S");
    let (r, n) = (10.0, 48);
    let pts: Vec<Point2> = (0..=n).map(|k| std::f64::consts::PI * k as f64 / n as f64).map(|a| Point2::new(-r * a.cos(), r * a.sin())).collect();
    for w in pts.windows(2) {
        p.add_line_entity(si, w[0].x, w[0].y, w[1].x, w[1].y, Purpose::Real);
    }
    p.add_line_entity(si, r, 0.0, -r, 0.0, Purpose::Real);
    p
}

#[test]
fn every_sketch_drawn_curve_by_curve_has_the_loops_of_a_rebuild() {
    let mut failures = Vec::new();
    for kind in [Kind::Lines, Kind::Arcs, Kind::Rectangles, Kind::Circles, Kind::Hexagons, Kind::Tangents, Kind::Array, Kind::Mixed] {
        drawn_curve_by_curve(&build(kind, 12), &format!("{kind:?}"), &mut failures);
    }
    drawn_curve_by_curve(&half_disc(), "a half disc closed by its diameter", &mut failures);
    let closed = {
        let mut p = half_disc();
        p.regen_sketch(0);
        p.sketches[0].contour_ids.iter().filter_map(|c| p.contour_index(*c)).filter(|&ci| p.contours[ci].closed).count()
    };
    assert!(failures.is_empty(), "the loops of a sketch drawn curve by curve and of a rebuild differ:\n{}", failures.join("\n"));
    assert_eq!(closed, 1, "the half disc drawn segment by segment is one closed loop");
}

#[test]
fn every_sketch_taken_apart_curve_by_curve_has_the_loops_of_a_rebuild() {
    // the curves of every kind of sketch deleted, or made construction and back, one at a time; after each the loops a
    // rebuild of the change makes are those of a rebuild from all the curves. Two sides of a rectangle deleted leave the
    // other two an open chain, which the loops round the change lost
    let mut failures = Vec::new();
    for kind in [Kind::Lines, Kind::Arcs, Kind::Rectangles, Kind::Circles, Kind::Hexagons, Kind::Tangents, Kind::Array, Kind::Mixed] {
        for (how, construction) in [("deleted", false), ("made construction", true)] {
            let mut p = build(kind, 12);
            p.regen_sketch(0);
            let ids: Vec<u64> = p.sketches[0].entities.iter().map(|e| e.id).step_by(3).collect();
            for (k, id) in ids.into_iter().enumerate() {
                if construction {
                    let _ = p.toggle_construction(0, &[id]);
                } else {
                    p.delete_entities(0, &[id]);
                }
                let mut whole = p.clone();
                whole.regen_sketch_whole(0);
                let (made, rebuilt) = (loops(&p), loops(&whole));
                if made != rebuilt {
                    failures.push(format!(
                        "{kind:?}, {how}, curve {k}: {} loops against {} of a rebuild; only in the rebuild: {:?}",
                        made.len(),
                        rebuilt.len(),
                        rebuilt.iter().find(|x| !made.contains(x))
                    ));
                    break;
                }
            }
        }
    }
    assert!(failures.is_empty(), "the loops of a sketch taken apart and of a rebuild differ:\n{}", failures.join("\n"));
}
