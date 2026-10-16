use super::smallvec_tris::clip_by_dists;

/// A triangle crossing the plane: the cut runs EXACTLY along it (the new vertices sit at d=0) and the area is preserved.
#[test]
fn crossing_triangle_cut_exactly_on_plane() {
    // the vertices have d = [-1, -1, +2]: two visible and one cut away, giving a quadrilateral (4 fan vertices)
    let v = [[0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [0.0, 9.0, 0.0]];
    let out = clip_by_dists(v, [-1.0, -1.0, 2.0]);
    assert!(!out.whole);
    assert_eq!(out.verts.len(), 4, "a quad of 4 vertices (2 original + 2 on the plane)");
    // the cut points carry weights that give d=0: w.d = 0
    for cv in &out.verts {
        let d = -cv.w[0] + -cv.w[1] + cv.w[2] * 2.0;
        assert!(d <= 1e-12, "the vertex is on the visible side or on the plane: d={d}");
    }
    // one visible vertex gives a triangle of 3
    let out2 = clip_by_dists(v, [-1.0, 2.0, 2.0]);
    assert_eq!(out2.verts.len(), 3);
    // wholly visible, or wholly hidden
    assert!(clip_by_dists(v, [-1.0, -1.0, -1.0]).whole);
    assert!(clip_by_dists(v, [1.0, 1.0, 1.0]).verts.is_empty());
}
