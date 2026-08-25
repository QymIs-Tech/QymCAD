mod common;

#[test]
fn top_face_edges_only() {
    let mut p = common::testbug();
    let (report, shapes) = qymcad_testkit::regenerate(&mut p);
    let s = shapes.get(&(202u64 as qymcad_core::model::Id)).expect("202");
    // the faces of the body, taken from the report
    let faces = &report.built.iter().find(|(b,_)| *b == 202u64 as qymcad_core::model::Id).expect("the body is in the report").1;
    // the top face: its centroid is at z near 20 and its normal is +Z
    let top = faces.iter().find(|f| (f.centroid.z-20.0).abs()<1.0 && f.normal[2]>0.9).expect("the top face");
    let te = s.face_edge_ids(top.id);
    // every edge of the body, with its id and z
    let (polys, ids, geom) = s.edges_full();
    let (mut arc, mut line, mut anyz_below) = (0,0,0);
    for (i,&id) in ids.iter().enumerate() {
        if te.contains(&id) {
            if geom[i].is_some(){arc+=1}else{line+=1}
            // are there edges of the face that are not at the top, below z = 19?
            if polys[i].iter().any(|pt| (pt[2] as f64) < 19.0) { anyz_below+=1; }
        }
    }
    eprintln!("top face id={}: {} contour edges ({line} straight, {arc} arcs), of which {anyz_below} run downward", top.id, te.len());
    eprintln!("{} edges in the body, so the projection of the face shows {} rather than {}", ids.len(), te.len(), ids.len());
    assert!(te.len() < ids.len(), "a face is a subset of the edges of the body, not the whole body");
    assert!(arc > 0, "the contour of the face is rounded, so it has arcs");
    assert_eq!(anyz_below, 0, "the contour of the top face does not run downward, staying at the top");
}
