//! EVERY EDGE OF A BODY HAS A NAME OF ITS OWN.
//!
//! A reference to an edge - a fillet, a chamfer, a dimension, a joint, the piece lit under the cursor - holds its
//! persistent id and nothing else. Two edges under one id are indistinguishable: a click on one lights the other,
//! a fillet made on one may land on the other after a rebuild.
//!
//! Reported behaviour, on a project of a rectangle and a slanted quadrilateral crossing its corner, three regions
//! that touch only at points extruded by one node - three islands in one body: the cursor on an edge of the slanted
//! bar lit a diagonal edge elsewhere. Measured on that project: 36 edges, 15 distinct ids, twelve ids held by two or
//! three edges.
use qymcad_core::feature::{Purpose, Reach};
use qymcad_core::model::Project;

/// The sketch of the report: a 44 x 48 rectangle and a slanted quadrilateral over its upper right corner. Extruded
/// 10 by one node are the regions through `inside`; the band where the two overlap is left out, as in the report, so
/// the pieces meet only at points. Answers the project, the body and the live shapes.
fn crossing_contours(inside: &[(f64, f64)]) -> (Project, u64, std::collections::HashMap<u64, qymcad_kernel::Shape>) {
    let mut p = Project::default();
    p.new_document();
    let sid = p.add_sketch("base", vec![], None);
    p.add_sketch_node(sid, "Sketch");
    let si = p.sketch_index(sid).unwrap();
    p.add_rect_entity(si, 0.0, 0.0, 44.0, 48.0, Purpose::Real);
    let quad = [(31.165893844147107, 52.19050239242788), (49.97684327849272, 35.80949760757212), (49.97684327849272, 28.13731885082651), (23.880419604494435, 50.86268114917349)];
    for k in 0..4 {
        let (a, b) = (quad[k], quad[(k + 1) % 4]);
        p.add_line_entity(si, a.0, a.1, b.0, b.1, Purpose::Real);
    }
    p.regen_sketch(si);
    let holds = |pts: &[qymcad_core::geom::Point2], (x, y): (f64, f64)| {
        let mut inside = false;
        for k in 0..pts.len() {
            let (a, b) = (pts[k], pts[(k + 1) % pts.len()]);
            if (a.y > y) != (b.y > y) && x < a.x + (y - a.y) * (b.x - a.x) / (b.y - a.y) {
                inside = !inside;
            }
        }
        inside
    };
    let profiles: Vec<u64> = inside
        .iter()
        .map(|&at| {
            let k = p.contours.iter().position(|c| c.closed && holds(&c.points, at)).unwrap_or_else(|| panic!("setup: no region holds {at:?}"));
            p.contours.ids()[k]
        })
        .collect();
    let body = p.add_extrude_multi(sid, profiles, 10.0, Reach::Forward, 0.0, Vec::new());
    for n in p.timeline.iter_mut() {
        n.dirty = true;
    }
    let (rep, shapes) = qymcad_testkit::regenerate(&mut p);
    assert!(rep.errors.is_empty(), "setup: {:?}", rep.errors);
    (p, body, shapes)
}

/// The ids held by more than one edge, with how many edges hold each.
fn twins(ids: &[u32]) -> Vec<(u32, usize)> {
    let mut count = std::collections::BTreeMap::<u32, usize>::new();
    for id in ids {
        *count.entry(*id).or_default() += 1;
    }
    count.into_iter().filter(|(_, n)| *n > 1).collect()
}

#[test]
fn the_edges_of_crossing_contours_extruded_have_names_of_their_own() {
    // the large part of the rectangle, its corner beyond the band, and the piece of the bar outside the rectangle
    let (p, body, shapes) = crossing_contours(&[(10.0, 10.0), (43.0, 46.0), (47.0, 33.0)]);
    let recorded: Vec<u32> = p.regen_edges.get(&body).map(|es| es.iter().map(|e| e.id).collect()).unwrap_or_default();
    assert!(recorded.len() > 12, "setup: the extrusion of crossing contours has more edges than a block, got {}", recorded.len());
    // THE LIVE SHAPE TOO: the window picks and lights edges by the ids the kernel's shape carries, not by the record
    let live: Vec<u32> = shapes.get(&body).map(|s| s.edges_full_smooth().1).expect("the live shape of the body");
    let faces: Vec<u32> = p.regen_faces.get(&body).map(|fs| fs.iter().map(|f| f.id).collect()).unwrap_or_default();
    // THE FACES FIRST: an edge is named by the pair of its faces, and twin faces hand their twin names on
    assert!(twins(&faces).is_empty(), "{} faces, and these ids are held by more than one face each: {:?}", faces.len(), twins(&faces));
    assert!(twins(&recorded).is_empty(), "{} edges recorded, and these ids are held by more than one edge each: {:?}", recorded.len(), twins(&recorded));
    assert!(twins(&live).is_empty(), "{} edges on the live shape, and these ids are held by more than one edge each - a reference to one means them all: {:?}", live.len(), twins(&live));
}
