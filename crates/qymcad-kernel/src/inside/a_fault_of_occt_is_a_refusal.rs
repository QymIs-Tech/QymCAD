//! A FAULT OF OCCT NEVER ENDS THE PROGRAM. On a staircase of three steps 10 high - the first chamfered 10 x 10, the second
//! rounded R10, the third chamfered 10 x 30, each eating its faces whole - roundings of the arc of the R10 tried one radius
//! after another on one copy of the body, as a person trying radii does, read memory OCCT does not own inside
//! `BRepFilletAPI` at R9.9999998 and took the process down with SIGSEGV. With the handlers of `OSD` set and a jump point in
//! every `try` of the bridge (`QYM_SIGNALS`) the fault comes back as a refusal in words, and the program goes on.
#[cfg(test)]
mod tests {
    use crate::OcctKernel;
    use qymcad_core::feature::{BlendRoles, ChamferMode, Kernel};
    use qymcad_core::model::ChamferShape;

    /// The id of the edge of `body` whose middle stands at `at`.
    fn edge_at(k: &OcctKernel, body: u64, at: [f64; 3]) -> u32 {
        let es = k.edges(body);
        let d = |m: [f64; 3]| (0..3).map(|i| (m[i] - at[i]).powi(2)).sum::<f64>().sqrt();
        let e = es.iter().min_by(|a, b| d(a.mid).total_cmp(&d(b.mid))).expect("an edge");
        assert!(d(e.mid) < 0.2, "SETUP: no edge of body {body} stands at {at:?}, the nearest at {:?}", e.mid);
        e.id
    }

    /// The staircase of the report, each blend through the kernel as a node builds it; the body before the last rounding.
    /// Across the steps X, up Y, along Z: the steps are 10 high, 50 deep along Z, their fronts at X = 0, 10 and 20.
    fn the_staircase() -> OcctKernel {
        let k = OcctKernel::default();
        let steps = crate::Shape::extrude(&[0.0, 0.0, 50.0, 0.0, 50.0, 30.0, 20.0, 30.0, 20.0, 20.0, 10.0, 20.0, 10.0, 10.0, 0.0, 10.0], 50.0).expect("the steps");
        k.shapes.borrow_mut().insert(1, steps);
        let two_legs = |d2: f64, flip: bool| ChamferShape { mode: ChamferMode::TwoDist, d2, flip, ref_face: 0 };
        let e = edge_at(&k, 1, [0.0, 10.0, 25.0]);
        k.chamfer_ex(2, 1, 10.0, two_legs(10.0, false), &[e], BlendRoles::Unnamed).expect("the first step chamfered 10 x 10");
        let e = edge_at(&k, 2, [10.0, 20.0, 25.0]);
        k.fillet(3, 2, 10.0, &[e], BlendRoles::Unnamed).expect("the second step rounded R10");
        let e = edge_at(&k, 3, [20.0, 30.0, 25.0]);
        let third =
            [false, true].into_iter().find(|flip| k.chamfer_ex(4, 3, 10.0, two_legs(30.0, *flip), &[e], BlendRoles::Unnamed).is_ok_and(|_| (volume(&k, 4) - (volume(&k, 3) - 7500.0)).abs() < 1.0));
        assert!(third.is_some(), "SETUP: the third step chamfered 10 x 30 across its whole top");
        // R2 on the line of the first chamfer on the near side
        let e = edge_at(&k, 4, [5.0, 5.0, 50.0]);
        k.fillet(6, 4, 2.0, &[e], BlendRoles::Unnamed).expect("the line of the first chamfer on the near side rounded R2");
        k
    }

    fn volume(k: &OcctKernel, body: u64) -> f64 {
        k.shapes.borrow().get(&body).map(|s| s.volume()).unwrap_or(f64::NAN)
    }

    /// The edge of `s` whose middle stands at `at`, rounded `radius`: a body inside the box of `s`, or a refusal in words.
    /// `s` is a copy of a body of the staircase (`transformed` by identity: the same faces and names, its geometry copied),
    /// which is where OCCT answers the two cases below.
    fn rounded_inside_or_refused(s: &crate::Shape, at: [f64; 3], radius: f64) {
        let near = |e: &crate::shape::EdgeInfo| {
            let m = e.poly[e.poly.len() / 2];
            (0..3).map(|i| (m[i] as f64 - at[i]).powi(2)).sum::<f64>().sqrt()
        };
        let info = s.edges_info();
        let e = info.iter().min_by(|a, b| near(a).total_cmp(&near(b))).expect("an edge");
        assert!(near(e) < 0.2, "SETUP: no edge of the copy stands at {at:?}");
        let src_box = s.bbox_exact().expect("a box");
        crate::clear_kernel_refusal();
        match s.fillet_edges(radius, &[e.id]) {
            Some(got) => {
                let b = got.bbox_exact().expect("a box");
                let out = (0..3).map(|i| (src_box[i] - b[i]).max(b[i + 3] - src_box[i + 3])).fold(0.0f64, f64::max);
                assert!(out <= radius + 1e-3, "a rounding of R{radius} at {at:?} gave a body {out:.1} mm past the box of its source {src_box:?}: {b:?}");
            }
            None => assert!(crate::last_kernel_refusal().is_some(), "a rounding of R{radius} at {at:?} refused without a word"),
        }
    }

    /// The fault inside OCCT comes back as a refusal, and the program goes on. The roundings of the arc one after another
    /// on one copy, as a person trying radii does: R9.9999998 after them faulted.
    #[test]
    fn a_fault_inside_the_kernel_is_a_refusal() {
        let k = the_staircase();
        let s = k.shapes.borrow().get(&3).and_then(|s| s.transformed(&qymcad_core::feature::PLACE_IDENTITY)).expect("a copy");
        for radius in [0.5, 1.0, 2.0, 2.0 * (1.0 - 2e-8), 3.0, 5.0, 9.9999998] {
            rounded_inside_or_refused(&s, [12.93, 17.07, 0.0], radius);
        }
    }
}
