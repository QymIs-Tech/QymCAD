//! A 4×4 square with its corners filleted one after another, solving after each, as the interface does.
//!
//! At r = 2, half the side, the trimmed edges collapse. A degenerate tangency used to conflict and deflate the
//! radii to about 1.33. Zero-length edges are now removed and a true circle comes out.
use qymcad_core::model::Project;
const PI: f64 = std::f64::consts::PI;
#[test]
fn gui_like_sequential_fillet_with_solve() {
    for r in [1.0_f64, 1.8, 2.0] {
        let mut p = Project::default();
        let si = p.new_sketch("s");
        p.add_rect_entity(si, 0.0, 0.0, 4.0, 4.0, qymcad_core::feature::Purpose::Real);
        p.solve_sketch(si);
        // as the interface does it: one corner at a time, solving the sketch after each
        let mut done = 0;
        for _ in 0..4 {
            let corner = {
                let s = &p.sketches[si];
                let mut cnt: std::collections::HashMap<u64, usize> = Default::default();
                for e in &s.entities {
                    if let qymcad_core::model::EntityKind::Line { a, b } = e.kind { *cnt.entry(a).or_default() += 1; *cnt.entry(b).or_default() += 1; }
                }
                cnt.into_iter().filter(|&(_, c)| c == 2).map(|(id, _)| id).next()
            };
            let Some(pid) = corner else { break };
            if p.fillet_at_vertex(si, pid, r) { done += 1; }
            p.solve_sketch(si);

        }
        p.regen_sketch(si);
        let area: f64 = p.sketches[si].contour_ids.iter().filter_map(|&c| p.contour_index(c)).map(|ci| p.contours[ci].area()).fold(0.0, f64::max);
        let exp = 16.0 - (4.0 - PI) * r * r;
        eprintln!("r={r}: filleted {done}/4, area={area:.3} (expecting {exp:.3})");
        assert_eq!(done, 4, "r={r}: only {done} of 4 corners filleted");
        assert!((area - exp).abs() / exp < 0.03, "r={r}: area={area:.2} ≠ {exp:.2}");
    }
}
