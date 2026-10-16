//! EVERY BLEND NAMES ITS SURFACE AFTER ITS EDGE: a fillet of one radius, a fillet with radii at its vertices, a chamfer
//! by one leg, by two legs, by a leg and an angle. Where the size changes along an edge, or the two legs differ,
//! nothing changes of where the face comes from; a reference to it - a sketch on the bevel, a mate on the round - holds
//! the name of the edge it was made from, through an edit above.
use qymcad_core::feature::ChamferMode;
use qymcad_core::model::{ChamferShape, Project};
use qymcad_core::names::Role;
use qymcad_core::refs::Ref;

/// A block 40 x 30 x 10 and the upright edge at its corner (0, 0) - the body and the edge.
fn block(p: &mut Project) -> (u64, u32) {
    let b = p.add_box(40.0, 30.0, 10.0);
    qymcad_testkit::regenerate(p);
    let e = p.regen_edges[&b].iter().find(|e| (e.mid[0] + 20.0).abs() < 1e-3 && (e.mid[1] + 15.0).abs() < 1e-3).map(|e| e.id).expect("the upright edge at the corner");
    (b, e)
}

/// A WAY TO BLEND THE EDGE: what it is, and how its node is laid on body `b` and edge `e`.
struct Way {
    name: &'static str,
    lay: fn(&mut Project, u64, u32) -> u64,
}

const WAYS: [Way; 5] = [
    Way { name: "a fillet of one radius", lay: |p, b, e| p.add_fillet(b, 2.0, vec![e]) },
    Way {
        name: "a fillet with a radius at a vertex",
        lay: |p, b, e| {
            let at = p
                .vertex_pool(b)
                .into_iter()
                .find(|c| (c.centroid[0] + 20.0).abs() < 1e-3 && (c.centroid[1] + 15.0).abs() < 1e-3 && c.centroid[2] > 9.0)
                .map(|c| c.desc)
                .expect("the top end of the edge");
            p.add_fillet_at_vertices(b, 2.0, Ref::picks(&[e]), vec![(at, 3.0)])
        },
    },
    Way { name: "a chamfer by one leg", lay: |p, b, e| p.add_chamfer(b, 2.0, vec![e]) },
    Way { name: "a chamfer by two legs", lay: |p, b, e| p.add_chamfer_ex(b, 2.0, ChamferShape { mode: ChamferMode::TwoDist, d2: 3.0, flip: false, ref_face: 0 }, vec![e]) },
    Way { name: "a chamfer by a leg and an angle", lay: |p, b, e| p.add_chamfer_ex(b, 2.0, ChamferShape { mode: ChamferMode::DistAngle, d2: 30.0, flip: false, ref_face: 0 }, vec![e]) },
];

#[test]
fn the_surface_of_a_blend_is_named_after_its_edge() {
    let mut fails = Vec::new();
    for way in &WAYS {
        let mut p = Project::default();
        p.new_document();
        let (b, e) = block(&mut p);
        let node = (way.lay)(&mut p, b, e);
        let (report, _) = qymcad_testkit::regenerate(&mut p);
        if let Some((_, err)) = report.errors.iter().find(|(id, _)| *id == node) {
            fails.push(format!("{}: did not build - {err:?}", way.name));
            continue;
        }
        // the new face at the corner: the only one standing within 3 of it in plan - the sides and the ends of the box
        // stand in their middles
        let Some(face) = p.regen_faces[&node].iter().find(|f| (f.centroid.x + 20.0).abs() < 3.0 && (f.centroid.y + 15.0).abs() < 3.0).map(|f| f.id) else {
            fails.push(format!("{}: no face at the corner", way.name));
            continue;
        };
        match p.names.get(face) {
            Some(n) if n.role == Role::Blend && n.src == e as u64 => {}
            other => fails.push(format!("{}: the surface is named {other:?}, after edge {e} as a blend expected", way.name)),
        }
    }
    assert!(fails.is_empty(), "a blend does not name its surface after its edge:\n{}", fails.join("\n"));
}
