//! A VERTEX IS NAMED BY THE BRIDGE, by the topology: the vertex an end of an edge stands on, named by the edges that
//! meet at it. The document only takes the name into its table when a vertex is referenced.
use qymcad_core::model::Project;

#[test]
fn every_vertex_is_named_by_the_edges_meeting_at_it() {
    let mut fails = Vec::new();
    for (what, build, want) in [
        ("a box", (|p: &mut Project| p.add_box(40.0, 30.0, 10.0)) as fn(&mut Project) -> u64, 8usize),
        ("a cylinder", |p| p.add_cylinder(10.0, 10.0), 2),
        ("a hexagonal prism", |p| p.add_prism(10.0, 6, 10.0), 12),
    ] {
        let mut p = Project::default();
        p.new_document();
        let body = build(&mut p);
        let (report, _) = qymcad_testkit::regenerate(&mut p);
        assert!(report.errors.is_empty(), "setup: {what} did not build");
        let spots = p.vertex_spots(body);
        if spots.len() != want {
            fails.push(format!("{what}: {} vertices, {want} expected", spots.len()));
        }
        let edges = &p.regen_edges[&body];
        for v in &spots {
            // the edges with an end on the vertex are the ones its name holds (up to three)
            let near = |q: [f64; 3]| (0..3).all(|i| (q[i] - v.at[i]).abs() < 1e-6);
            let mut meeting: Vec<u32> = edges.iter().filter(|e| near(e.a) || near(e.b)).map(|e| e.id).collect();
            meeting.sort_unstable();
            meeting.dedup();
            let named: Vec<u32> = v.name.edges.iter().copied().filter(|d| *d != 0).collect();
            if !named.iter().all(|d| meeting.contains(d)) || named.len() < meeting.len().min(3) {
                fails.push(format!("{what}: the vertex at {:?} is named {named:?}, the edges meeting there are {meeting:?}", v.at));
            }
        }
        // every end of every edge stands on a named vertex - the edge itself need not be among the three smallest of
        // its vertex, the end is found by the topology
        for e in edges {
            if e.ends.iter().any(|v| v.edges[0] == 0) {
                fails.push(format!("{what}: an end of edge {} stands on no named vertex", e.id));
            }
        }
        // and the names go into the table as references take them, each once
        let pool = p.vertex_pool(body);
        let mut descs: Vec<u32> = pool.iter().map(|c| c.desc).collect();
        descs.sort_unstable();
        descs.dedup();
        if descs.len() != want {
            fails.push(format!("{what}: {} names in the table for {want} vertices", descs.len()));
        }
    }
    assert!(fails.is_empty(), "the vertices are not named by the bridge:\n{}", fails.join("\n"));
}
