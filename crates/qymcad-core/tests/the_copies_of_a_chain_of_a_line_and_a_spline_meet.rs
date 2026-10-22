//! THE COPIES OF A CHAIN OF A LINE AND A SPLINE MEET: a line and a spline going on from its end, offset 2 together - the
//! copy of the line and the copy of the spline end at one point where the spline goes on smoothly, an arc about the joint
//! closes the gap where they part, and where they cross both end at the crossing. A node of the spline dragged, the copies
//! meet still. As in the sketchers a person comes from, an offset of a chain is one chain.
//!
//! Reported behaviour: each copy lay by itself, and in the corner of the chain the copies did not meet.
use qymcad_core::feature::{Ends, Purpose};
use qymcad_core::geom::Point2;
use qymcad_core::model::curve::Curve;
use qymcad_core::model::{Id, Project};

/// A JOINT OF THE CHAIN: what it is, and the nodes of the spline going on from the end of the line at (10, 0).
struct Joint {
    name: &'static str,
    spline: [[f64; 2]; 3],
}

const JOINTS: [Joint; 3] = [
    Joint { name: "smooth", spline: [[10.0, 0.0], [20.0, 0.0], [28.0, 6.0]] },
    // turning right: the copies to the left of the chain part
    Joint { name: "turning away from the copy", spline: [[10.0, 0.0], [16.0, -8.0], [20.0, -18.0]] },
    // turning left: the copies to the left of the chain cross
    Joint { name: "turning towards the copy", spline: [[10.0, 0.0], [16.0, 8.0], [20.0, 18.0]] },
];

/// The ends of every curve of sketch `si` but those of the source, as places.
fn copy_ends(p: &Project, si: usize, source: &[Id]) -> Vec<Point2> {
    let s = &p.sketches[si];
    s.entities.iter().filter(|e| !source.contains(&e.id)).filter_map(|e| Curve::of(s, e)).filter(|c| !c.closed()).flat_map(|c| c.ends()).collect()
}

/// The ends of the copies that meet no end of another copy: the two free ends of the chain, and any end left apart.
fn loose(ends: &[Point2]) -> usize {
    ends.iter().enumerate().filter(|(k, q)| !ends.iter().enumerate().any(|(j, r)| j != *k && q.dist(*r) < 1e-6)).count()
}

#[test]
fn the_copies_of_a_chain_of_a_line_and_a_spline_meet() {
    let mut failures = Vec::new();
    for Joint { name, spline } in JOINTS {
        let mut p = Project::default();
        p.new_document();
        let si = p.new_sketch(name);
        let line = p.add_line_entity(si, 0.0, 0.0, 10.0, 0.0, Purpose::Real);
        let curve = p.add_spline(si, spline.iter().map(|q| Point2::new(q[0], q[1])).collect(), Ends::Open, Purpose::Real).expect("the spline");
        p.merge_close_points(si, 1e-9);
        p.solve_sketch(si);
        if p.offset_entities(si, &[line, curve], 2.0) == 0 {
            failures.push(format!("{name}: not offset"));
            continue;
        }
        let ends = copy_ends(&p, si, &[line, curve]);
        if loose(&ends) != 2 {
            failures.push(format!("{name}: the copies meet nowhere but at the two free ends of the chain - {} loose ends of {}", loose(&ends), ends.len()));
            continue;
        }
        // the last node of the spline dragged 2 up: the copies meet still
        let node = p.sketches[si].entity(curve).and_then(|e| e.kind.as_spline()).map(|sp| sp.nodes[2]).expect("the node");
        let q = *p.sketches[si].point(node).expect("its place");
        p.solve_sketch_drag(si, Some((node, q.x, q.y + 2.0)));
        p.solve_sketch(si);
        let ends = copy_ends(&p, si, &[line, curve]);
        if loose(&ends) != 2 {
            failures.push(format!("{name}: a node of the spline dragged, {} loose ends of {}", loose(&ends), ends.len()));
        }
        // the joint holds without a constraint against another
        let conflicts = p.sketch_conflicts(si);
        if !conflicts.is_empty() {
            failures.push(format!("{name}: constraints in conflict at the joint: {:?}", conflicts.iter().map(|ci| &p.sketches[si].constraints[*ci]).collect::<Vec<_>>()));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
