//! Splitting an assembly into independent parts.
//!
//! An assembly is almost never a single problem: bodies form groups that are mated inside but not
//! between. Solving everything as one matrix costs a cube of the total body count for a problem that
//! falls apart into dozens of small ones. A real document with 1182 components and a single mate makes
//! this concrete: dense algebra over all of them is unusable, while over the two mated bodies it is
//! instant.
//!
//! The second effect matters more than speed: failure stays local. An inconsistent mate in one node
//! must not disturb the placement of another. A single least-squares problem over the whole document
//! spreads the error of one conflicting mate across every body in it.
//!
//! Connected components come from `petgraph` rather than a hand-written traversal.

use nalgebra::Isometry3;
use petgraph::unionfind::UnionFind;

use super::iterate::{solve, Report};
use super::problem::{Body, Constraint, Problem};

/// Result for the whole assembly: body poses and the reports of each independent part.
#[derive(Clone, Debug, Default)]
pub struct AssemblyReport {
    /// Whether every part converged.
    pub converged: bool,
    /// Largest residual among the parts.
    pub residual: f64,
    /// Total remaining degrees of freedom.
    pub dof: usize,
    /// Redundant constraints, indexed into the original problem list.
    pub redundant: Vec<usize>,
    /// Violated constraints, indexed into the original list, with the size of the violation.
    pub violated: Vec<(usize, f64)>,
    /// How many independent parts the assembly fell into.
    pub parts: usize,
    /// Bodies of the parts that did not converge: those must not be moved, the rest may be.
    ///
    /// The rule "no solution, no movement" has to be per part, not per document. Applied to the whole
    /// document, one sick mate freezes mechanisms that have nothing to do with it: measured on a
    /// five-mate document with one bad mate, a slider travelled 0.000 mm instead of 15.000 and a wheel
    /// 0.000 degrees instead of 40.000, while removing that single mate let the same mechanism run.
    /// Parts are solved separately anyway.
    pub stuck: std::collections::HashSet<usize>,
}

/// Solve an assembly by splitting it into independent parts.
///
/// Returns poses for every body. Bodies that take part in no constraint stay where they are; there is
/// no reason to include them in the problem at all.
pub fn solve_assembly(problem: &Problem) -> (Vec<Isometry3<f64>>, AssemblyReport) {
    let n = problem.bodies.len();
    let mut poses: Vec<Isometry3<f64>> = problem.bodies.iter().map(|b| b.pose).collect();
    let mut report = AssemblyReport { converged: true, ..Default::default() };

    if !problem.references_are_valid() {
        report.converged = false;
        report.residual = f64::INFINITY;
        return (poses, report);
    }

    // Connected components: two bodies belong to one part if any constraint joins them.
    let mut uf = UnionFind::<usize>::new(n);
    for c in &problem.constraints {
        // A relation between degrees joins up to four bodies, and all of them form one part. That
        // is exactly what separates a gear pair from two independent revolutes: the position of one
        // gear determines the position of the other, so they cannot be solved apart.
        let bs = c.bodies();
        for w in bs.windows(2) {
            uf.union(w[0], w[1]);
        }
    }

    // Distribute the constraints over the parts.
    let mut parts: std::collections::HashMap<usize, Vec<usize>> = std::collections::HashMap::new();
    for (i, c) in problem.constraints.iter().enumerate() {
        parts.entry(uf.find(c.bodies()[0])).or_default().push(i);
    }
    report.parts = parts.len();

    // Solve the parts in a predictable order: otherwise the report — and with it the answer to
    // "which constraint is redundant" — would differ between runs on the same document.
    let mut keys: Vec<usize> = parts.keys().copied().collect();
    keys.sort_unstable();

    for key in keys {
        let cons = &parts[&key];
        // Bodies of this part.
        let mut local_of: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
        let mut bodies: Vec<Body> = Vec::new();
        let mut global_of: Vec<usize> = Vec::new();
        for &ci in cons {
            for g in problem.constraints[ci].bodies() {
                if !local_of.contains_key(&g) {
                    local_of.insert(g, bodies.len());
                    global_of.push(g);
                    bodies.push(problem.bodies[g].clone());
                }
            }
        }

        // Constraints with body indices remapped into the part.
        let mut sub = Problem::new(bodies);
        for &ci in cons {
            sub.add(remap(&problem.constraints[ci], &local_of));
        }

        // A part with nothing grounded floats: it carries six spare degrees of freedom for the
        // whole group, so the solution is defined only up to a rigid motion. Ground the first body —
        // it already stands where it was left, and the group assembles around it.
        if !sub.bodies.iter().any(|b| b.grounded) {
            sub.bodies[0].grounded = true;
        }

        let (sub_poses, rep) = solve(&sub);
        // A part that did not converge stays where it was — that part alone, see `stuck`.
        if rep.converged {
            for (l, g) in global_of.iter().enumerate() {
                poses[*g] = sub_poses[l];
            }
        } else {
            report.stuck.extend(global_of.iter().copied());
        }
        merge(&mut report, &rep, cons);
    }

    (poses, report)
}

/// Remap a constraint into the local body indices of a part.
fn remap(c: &Constraint, map: &std::collections::HashMap<usize, usize>) -> Constraint {
    let f = |mut a: super::frame::Anchor| {
        a.body = map[&a.body];
        a
    };
    let fm = |mut m: super::problem::SlotMeasure| {
        m.a = f(m.a);
        m.b = f(m.b);
        m
    };
    match *c {
        Constraint::SlotRatio { m1, m2, ratio, offset, period } => Constraint::SlotRatio { m1: fm(m1), m2: fm(m2), ratio, offset, period },
        Constraint::PointCoincident { a, b } => Constraint::PointCoincident { a: f(a), b: f(b) },
        Constraint::AxisAligned { a, b } => Constraint::AxisAligned { a: f(a), b: f(b) },
        Constraint::OnAxis { a, b } => Constraint::OnAxis { a: f(a), b: f(b) },
        Constraint::OnPlane { a, b, offset } => Constraint::OnPlane { a: f(a), b: f(b), offset },
        Constraint::RollAligned { a, b } => Constraint::RollAligned { a: f(a), b: f(b) },
        Constraint::Angle { a, b, deg } => Constraint::Angle { a: f(a), b: f(b), deg },
        Constraint::AxisDistance { a, b, dist } => Constraint::AxisDistance { a: f(a), b: f(b), dist },
        Constraint::PointDistance { a, b, dist } => Constraint::PointDistance { a: f(a), b: f(b), dist },
        Constraint::Pull { a, to } => Constraint::Pull { a: f(a), to },
    }
}

/// Merge a part report into the overall one, mapping constraint indices back to the original list.
///
/// The mapping is required: the interface says "constraint 7 is redundant", and that has to be the
/// user's seventh constraint, not the seventh inside an arbitrarily cut part.
fn merge(total: &mut AssemblyReport, part: &Report, cons: &[usize]) {
    total.converged &= part.converged;
    total.residual = total.residual.max(part.residual);
    total.dof += part.dof;
    total.redundant.extend(part.redundant.iter().filter_map(|&i| cons.get(i).copied()));
    total.violated.extend(part.violated.iter().filter_map(|&(i, v)| cons.get(i).map(|&g| (g, v))));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asm::frame::Anchor;
    use crate::asm::joint::{Joint, JointKind};
    use nalgebra::{Translation3, UnitQuaternion, Vector3};

    fn at(x: f64, y: f64, z: f64) -> Isometry3<f64> {
        Isometry3::from_parts(Translation3::new(x, y, z), UnitQuaternion::identity())
    }

    #[test]
    fn bodies_without_constraints_are_left_exactly_where_they_are() {
        let mut p = Problem::new(vec![Body::grounded(at(0.0, 0.0, 0.0)), Body::new(at(10.0, 0.0, 0.0)), Body::new(at(77.0, -3.0, 9.0))]);
        p.add(Constraint::PointCoincident { a: Anchor::origin(0), b: Anchor::origin(1) });
        let (poses, rep) = solve_assembly(&p);
        assert!((poses[2].translation.vector - Vector3::new(77.0, -3.0, 9.0)).norm() < 1e-12, "a body with no constraints must stay put, but it sits at {:?}", poses[2].translation.vector);
        assert_eq!(rep.parts, 1, "one constraint means one part");
    }

    #[test]
    fn independent_groups_are_solved_independently() {
        // Two pairs, with no constraint between the pairs.
        let mut p = Problem::new(vec![
            Body::grounded(at(0.0, 0.0, 0.0)),
            Body::new(at(50.0, 0.0, 0.0)),
            Body::grounded(at(0.0, 100.0, 0.0)),
            Body::new(at(50.0, 100.0, 0.0)),
        ]);
        p.add(Constraint::PointCoincident { a: Anchor::origin(0), b: Anchor::origin(1) });
        p.add(Constraint::PointCoincident { a: Anchor::origin(2), b: Anchor::origin(3) });
        let (poses, rep) = solve_assembly(&p);
        assert_eq!(rep.parts, 2, "two unconnected pairs make two independent parts");
        assert!(poses[1].translation.vector.norm() < 1e-6, "the first pair must converge");
        assert!((poses[3].translation.vector - Vector3::new(0.0, 100.0, 0.0)).norm() < 1e-6, "the second pair must converge onto its own grounded body");
    }

    /// Failure stays local: a conflict in one group must not disturb another.
    ///
    /// Solved as a single least-squares problem, one unsatisfiable constraint spreads its error over
    /// the whole assembly.
    #[test]
    fn a_conflict_in_one_group_does_not_disturb_another() {
        let mut p = Problem::new(vec![
            Body::grounded(at(0.0, 0.0, 0.0)),
            Body::new(at(50.0, 0.0, 0.0)),
            Body::grounded(at(0.0, 100.0, 0.0)),
            Body::new(at(50.0, 100.0, 0.0)),
        ]);
        // Group 1: two incompatible constraints pull one body towards two different points.
        p.add(Constraint::PointCoincident { a: Anchor::new(0, at(0.0, 0.0, 0.0)), b: Anchor::origin(1) });
        p.add(Constraint::PointCoincident { a: Anchor::new(0, at(200.0, 0.0, 0.0)), b: Anchor::origin(1) });
        // Group 2: an ordinary satisfiable constraint.
        p.add(Constraint::PointCoincident { a: Anchor::origin(2), b: Anchor::origin(3) });
        let (poses, rep) = solve_assembly(&p);
        assert!(!rep.converged, "the conflict must be visible");
        assert!(
            (poses[3].translation.vector - Vector3::new(0.0, 100.0, 0.0)).norm() < 1e-6,
            "the healthy group must assemble exactly despite the conflict next door: {:?}",
            poses[3].translation.vector
        );
    }

    /// Report indices are the original ones, not those local to a part.
    #[test]
    fn report_indices_refer_to_the_original_constraint_list() {
        let mut p = Problem::new(vec![
            Body::grounded(at(0.0, 0.0, 0.0)),
            Body::new(at(50.0, 0.0, 0.0)),
            Body::grounded(at(0.0, 100.0, 0.0)),
            Body::new(at(50.0, 100.0, 0.0)),
        ]);
        p.add(Constraint::PointCoincident { a: Anchor::origin(0), b: Anchor::origin(1) }); // 0
        p.add(Constraint::PointCoincident { a: Anchor::origin(2), b: Anchor::origin(3) }); // 1
        p.add(Constraint::PointCoincident { a: Anchor::origin(2), b: Anchor::origin(3) }); // 2 - a duplicate
        let (_, rep) = solve_assembly(&p);
        assert!(!rep.redundant.is_empty(), "the duplicate must be found");
        assert!(
            rep.redundant.iter().all(|&i| i == 1 || i == 2),
            "the redundant constraint must be one of the second group (1 or 2), but {:?} was reported: indices were not mapped back",
            rep.redundant
        );
    }

    /// Scale: hundreds of bodies with a single mate solve instantly, because the problem covers two bodies.
    #[test]
    fn hundreds_of_unrelated_bodies_do_not_slow_the_solve() {
        let mut bodies = vec![Body::grounded(at(0.0, 0.0, 0.0)), Body::new(at(80.0, 30.0, 15.0))];
        for i in 0..800 {
            bodies.push(Body::new(at(i as f64, 0.0, 0.0)));
        }
        let a = Anchor::from_axes(0, Vector3::zeros(), Vector3::z(), Vector3::x()).unwrap();
        let b = Anchor::from_axes(1, Vector3::zeros(), Vector3::z(), Vector3::x()).unwrap();
        let p = crate::asm::joint::problem_from(bodies, &[Joint::new(a, b, JointKind::Rigid)]);
        let t = std::time::Instant::now();
        let (poses, rep) = solve_assembly(&p);
        let ms = t.elapsed().as_millis();
        assert!(rep.converged, "must converge: {:.3e}", rep.residual);
        assert!(ms < 200, "802 bodies with one constraint must solve instantly, but took {ms} ms");
        assert!((poses[500].translation.vector - Vector3::new(498.0, 0.0, 0.0)).norm() < 1e-12, "a body outside the problem must stay put");
    }

    /// A group with nothing grounded assembles around its first body instead of drifting away.
    #[test]
    fn a_floating_group_assembles_around_its_first_body() {
        let mut p = Problem::new(vec![Body::new(at(10.0, 20.0, 30.0)), Body::new(at(-99.0, 0.0, 0.0))]);
        p.add(Constraint::PointCoincident { a: Anchor::origin(0), b: Anchor::origin(1) });
        let (poses, rep) = solve_assembly(&p);
        assert!(rep.converged, "an ungrounded group must assemble: {:.3e}", rep.residual);
        assert!((poses[0].translation.vector - Vector3::new(10.0, 20.0, 30.0)).norm() < 1e-9, "the first body of the group must stay put");
        assert!((poses[1].translation.vector - Vector3::new(10.0, 20.0, 30.0)).norm() < 1e-6, "the second must come to it");
    }
}
