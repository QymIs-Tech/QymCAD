//! THE PREVIEW OF A SWEEP STANDS WHERE THE BODY WILL: a profile drawn on the table at the foot of an upright path stays
//! where it is drawn, in the preview as in the body, whichever end the path was drawn from.
//!
//! Reported behaviour: the body of a sweep stood elsewhere than its profile and turned over. The kernel came to build
//! the body from the profile where it is drawn, and the preview kept its own copy of the old rule: it carried the
//! profile by its sketch's origin to the start of the path and turned it by the world axes, 10 mm off here.

use qymcad_core::feature::{BasePlane, Purpose, SketchPlane};
use qymcad_core::model::Project;

/// A box around points: its least and its greatest corner.
struct Bounds {
    min: [f64; 3],
    max: [f64; 3],
}

fn bounds(points: &[[f64; 3]]) -> Bounds {
    let mut b = Bounds { min: [f64::INFINITY; 3], max: [f64::NEG_INFINITY; 3] };
    for p in points {
        b.min = [0, 1, 2].map(|c| b.min[c].min(p[c]));
        b.max = [0, 1, 2].map(|c| b.max[c].max(p[c]));
    }
    b
}

/// A path line drawn on the front plane, from `from` to `to` in the sketch's own coordinates.
struct Line {
    from: (f64, f64),
    to: (f64, f64),
}

#[test]
fn a_sweep_preview_stands_where_the_body_will() {
    let mut fails = Vec::new();
    for line in [Line { from: (10.0, 0.0), to: (10.0, 30.0) }, Line { from: (10.0, 30.0), to: (10.0, 0.0) }] {
        let mut p = Project::default();
        p.new_document();
        // the profile: a rectangle 6 x 4 on the table at (10, 0)
        let prof = p.new_sketch("profile");
        p.add_rect_entity(prof, 10.0, 0.0, 16.0, 4.0, Purpose::Real);
        p.regen_sketch(prof);
        // the path: an upright line of 30 on the front plane from the corner (10, 0, 0) of the rectangle
        let path = p.new_sketch("path");
        p.sketches[path].plane = SketchPlane::World(BasePlane::XZ);
        p.add_line_entity(path, line.from.0, line.from.1, line.to.0, line.to.1, Purpose::Real);
        p.regen_sketch(path);
        let sweep = qymcad_ui_state::SweepParams { prof_sid: p.sketches[prof].id, path_sid: p.sketches[path].id, ..Default::default() };
        let Some(preview) = qymcad_ui_state::sweep_preview(&p, sweep) else {
            fails.push(format!("{:?} -> {:?}: no preview", line.from, line.to));
            continue;
        };
        let a = bounds(preview.sections.first().map_or(&[], |s| s.as_slice()));
        let b = bounds(preview.sections.last().map_or(&[], |s| s.as_slice()));
        let at = |b: &Bounds, z: f64| (0..3).all(|c| (b.min[c] - [10.0, 0.0, z][c]).abs() < 1e-6 && (b.max[c] - [16.0, 4.0, z][c]).abs() < 1e-6);
        if !at(&a, 0.0) || !at(&b, 30.0) {
            fails.push(format!(
                "{:?} -> {:?}: the profile drawn at x 10..16, y 0..4 on the table is previewed at {:?}..{:?} and ends at {:?}..{:?}, not risen to z 30 over it",
                line.from, line.to, a.min, a.max, b.min, b.max
            ));
        }
    }
    assert!(fails.is_empty(), "\n{}", fails.join("\n"));
}
