//! Fusing two coaxial blocks that share a face has to give a monolith: the coplanar faces merge and the
//! collinear edges become one edge.
//!
//! Without unifying the same domain the seam remains: the vertical edges are doubled, the body has two edges
//! where it should have one, and chamfers and fillets stop working.
use qymcad_kernel::Shape;

// the profile of a 10×10 square, closed and counter-clockwise
fn square10() -> Vec<f64> {
    vec![0.0, 0.0, 10.0, 0.0, 10.0, 10.0, 0.0, 10.0]
}

#[test]
fn fused_stacked_boxes_are_monolithic() {
    let b1 = Shape::extrude(&square10(), 10.0).expect("block 1"); // z 0..10
    let b2 = Shape::extrude(&square10(), 10.0).expect("block 2");
    // raise the second block by 10 so they share the face at z = 10; a 3×4 row-major matrix with the
    // translation along z at index 11
    let up = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 10.0];
    let b2 = b2.transformed(&up).expect("moving block 2");
    let fused = b1.boolean(&b2, 1).expect("the union");
    let (_polys, ids) = fused.edges_with_ids();
    let n = ids.len();
    eprintln!("edges of the fused blocks: {n}; a 10×10×20 monolith has 12, and about 20 with a seam");
    // A monolithic 10×10×20 block has exactly twelve edges. If the seam is not merged the verticals are
    // doubled, giving about twenty.
    assert_eq!(n, 12, "fusing two blocks has to give a monolith of twelve edges rather than a seam ({n})");
    // the volume is preserved at 2000 mm³
    assert!((fused.volume() - 2000.0).abs() < 1.0, "the volume of the monolith is 2000 mm³");
}

// The case: a base extruded upwards, with a rectangle on its side face set below the top and extruded
// sideways as a second block. The front-left vertical edge of the base is split by that block into two
// segments, above and below it. Those are different edges of the monolith and each has to carry its own id, so
// that selecting the upper one does not catch the lower. The defect: a fuse that propagates ids gives both
// pieces one id.
#[test]
fn split_edge_segments_get_distinct_ids() {
    let a = Shape::extrude(&[0.0, 0.0, 10.0, 0.0, 10.0, 10.0, 0.0, 10.0], 20.0).unwrap(); // the base, a 10 mm square extruded 20 up
    // the second block, translated so that it sits against the face at y = 0, flush at x = 0 and set below
    // the top
    let b = Shape::extrude(&[0.0, 0.0, 6.0, 0.0, 6.0, 8.0, 0.0, 8.0], 12.0).unwrap();
    let mv = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, -8.0, 0.0, 0.0, 1.0, 2.0];
    let b = b.transformed(&mv).unwrap();
    let fused = a.boolean(&b, 1).expect("the fusion");
    let (polys, ids) = fused.edges_with_ids();
    // the vertical edges at the corner near x = 0, y = 0: the front-left edge of the base, split by the second
    // block
    let mut vert: Vec<(u32, f32, f32)> = Vec::new();
    for (i, p) in polys.iter().enumerate() {
        if p.len() < 2 {
            continue;
        }
        let (f, l) = (p[0], *p.last().unwrap());
        let vertical = (f[0] - l[0]).abs() < 0.01 && (f[1] - l[1]).abs() < 0.01 && (f[2] - l[2]).abs() > 0.5;
        if vertical && f[0].abs() < 0.01 && f[1].abs() < 0.01 {
            let zmin = p.iter().map(|q| q[2]).fold(f32::MAX, f32::min);
            let zmax = p.iter().map(|q| q[2]).fold(f32::MIN, f32::max);
            vert.push((ids[i], zmin, zmax));
        }
    }
    eprintln!("vertical edges at (0,0): {vert:?}");
    assert!(vert.len() >= 2, "the front-left edge is split into at least two segments, found {}", vert.len());
    let uniq: std::collections::HashSet<u32> = vert.iter().map(|(id, _, _)| *id).collect();
    assert_eq!(uniq.len(), vert.len(), "each segment of the vertical edge carries its own id and does not catch its neighbour: {vert:?}");
}

// A stress test of the unification, hunting for a crash: operations after a union, plus a compound of
// disjoint bodies.
#[test]
fn unify_survives_downstream_ops_and_disjoint() {
    let up = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 10.0];
    let far = [1.0, 0.0, 0.0, 100.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0];
    // 1) fusing touching bodies, then chamfering and filleting every edge afterwards, which must not crash
    let a = Shape::extrude(&square10(), 10.0).unwrap();
    let b = Shape::extrude(&square10(), 10.0).unwrap().transformed(&up).unwrap();
    let fused = a.boolean(&b, 1).expect("the fusion");
    assert!(fused.fillet_all(1.0).is_some(), "filleting after the unification");
    assert!(fused.chamfer_all(1.0).is_some(), "chamfering after the unification");
    // 2) a cut out of the union: another boolean and unification on top
    let c = Shape::extrude(&square10(), 30.0).unwrap();
    assert!(fused.boolean(&c, 0).is_some(), "a cut out of the monolith");
    // 3) fusing disjoint bodies gives a compound of two solids, and the unification must not crash
    let d = Shape::extrude(&square10(), 10.0).unwrap();
    let e = Shape::extrude(&square10(), 10.0).unwrap().transformed(&far).unwrap();
    let comp = d.boolean(&e, 1).expect("fusing disjoint bodies into a compound");
    assert!((comp.volume() - 2000.0).abs() < 1.0, "the volume of the compound is 2×1000");
    // 4) filleting the compound, unifying it and going on from there
    assert!(comp.fillet_all(0.5).is_some(), "filleting the compound after the unification");
}
