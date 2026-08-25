//! Reproducing defects against a real project. It loads the project, rebuilds it with the real kernel and
//! checks particular defects, so that they are fixed on the facts rather than by guesswork.
mod common;

// a body before its chamfer: the topology and the limit of that chamfer
#[test]
fn bug4_chamfer_and_edge_topology() {
    let mut p = common::testbug();
    let (report, shapes) = qymcad_testkit::regenerate(&mut p);
    // The ladder of step-backs from 2e-8 to 1e-5 in the kernel closed this: a chamfer whose distance equals
    // the length of the face exactly now builds, where it used to fail and only 3.99 worked. The chamfer is
    // told apart by the operation code, since matching a substring would go blind on any edit of the text.
    let chamfer_failed = report.errors.iter().any(|(_, e)| {
        matches!(
            e,
            qymcad_core::errors::CoreError::OpFailed(qymcad_core::errors::Op::Chamfer | qymcad_core::errors::Op::ChamferAsym)
                | qymcad_core::errors::CoreError::ChamferTooBig { .. }
        )
    });
    assert!(!chamfer_failed, "the chamfer of 4.0 from the file builds: {:?}", report.errors);
    let s = shapes.get(&(293u64 as qymcad_core::model::Id)).expect("the body");
    // The raw limit of the kernel has not gone anywhere, which is what the ladder in the kernel is for.
    //
    // The edge is found by its geometry rather than by number. There used to be a positional number here,
    // which lived only while the document was frozen on the old naming scheme. That freeze was removed — it
    // protected files that no longer exist — the body received structural names, and the number ceased to
    // exist. The number was an accident anyway: this check is about the limit of the kernel, not about a name.
    let (polys, ids, _) = s.edges_full();
    let vertical_at = |x: f64, y: f64| -> Option<u32> {
        ids.iter().enumerate().find_map(|(i, &id)| {
            let (a, b) = (polys[i].first()?, polys[i].last()?);
            let on = |p: &[f32; 3]| (p[0] as f64 - x).abs() < 0.6 && (p[1] as f64 - y).abs() < 0.6;
            (on(a) && on(b) && (a[2] - b[2]).abs() > 1.0).then_some(id)
        })
    };
    let e = vertical_at(29.0, 16.0).expect("the vertical edge at x = 29, y = 16");
    assert!(s.chamfer_edges(3.99, &[e], &[], &[], &[]).is_some(), "a chamfer of 3.99 on this edge builds");
    assert!(s.chamfer_edges(4.0, &[e], &[], &[], &[]).is_none(), "the raw kernel fails at 4.0, which equals the length of the face; the ladder in the kernel compensates");
    // on the line at x = 12, y = 40 there are doubled and overlapping edges, one spanning z 5 to 14 over
    // another spanning z 9 to 14, plus a degenerate one
    let (polys, ids, _) = s.edges_full();
    let mut vert = vec![];
    for (i, &id) in ids.iter().enumerate() {
        let (a, b) = (polys[i].first().unwrap(), polys[i].last().unwrap());
        if (a[0]-12.0).abs()<0.6 && (a[1]-40.0).abs()<0.6 && (b[0]-12.0).abs()<0.6 && (b[1]-40.0).abs()<0.6 {
            vert.push((id, (a[2].min(b[2])*10.0).round()/10.0, (a[2].max(b[2])*10.0).round()/10.0));
        }
    }
    eprintln!("vertical edges at x = 12, y = 40: {vert:?}");
    // the defect: the spans 5..14 and 9..14 overlap over z 9..14
    let has63 = vert.iter().any(|&(_,lo,hi)| (lo-5.0).abs()<0.2 && (hi-14.0).abs()<0.2);
    let has28 = vert.iter().any(|&(_,lo,hi)| (lo-9.0).abs()<0.2 && (hi-14.0).abs()<0.2);
    assert!(has63 && has28, "the defect reproduces: doubled, overlapping edges");
}
