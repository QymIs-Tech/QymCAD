//! A SKETCH FILLET IS GIVEN BY ITS RADIUS, ITS CHORD OR ITS ARC LENGTH, and keeps the size as it was given: a chord as
//! the distance between the two points of touching, an arc length as the length of the arc, so a dimension changed
//! afterwards moves the fillet. A size the corner cannot take is refused.
//!
//! Reported (issue #36): the sketch fillet took a radius only; a rounding given by its chord or its arc length had its
//! radius worked out by hand.
use qymcad_core::feature::Purpose;
use qymcad_core::model::{Constraint, FilletBy, FilletSize, Project};

/// The square corner (30, 0) -> (0, 0) -> (0, 30), held as a drawing holds it - the one line horizontal, the other
/// upright, the far ends pinned; (project, sketch, the corner point).
fn corner() -> (Project, usize, u64) {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("S");
    let l1 = p.add_line_entity(si, 30.0, 0.0, 0.0, 0.0, Purpose::Real);
    let l2 = p.add_line_entity(si, 0.0, 0.0, 0.0, 30.0, Purpose::Real);
    let ends = |p: &Project, id| match p.sketches[si].entities.iter().find(|e| e.id == id).and_then(|e| e.kind.as_line()) {
        Some(qymcad_core::model::LineParts { a, b }) => (a, b),
        _ => panic!("a line"),
    };
    let ((a1, b1), (a2, b2)) = (ends(&p, l1), ends(&p, l2));
    p.sketches[si].constraints.extend([Constraint::Horizontal { a: a1, b: b1 }, Constraint::Vertical { a: a2, b: b2 }, Constraint::Fixed { p: a1 }, Constraint::Fixed { p: b2 }]);
    p.solve_sketch(si);
    (p, si, b1)
}

/// What the fillet arc measures: its radius, its chord and the length of its arc.
struct Measured {
    radius: f64,
    chord: f64,
    arc: f64,
}

fn measure(p: &Project, si: usize) -> Option<Measured> {
    let s = &p.sketches[si];
    let at = |id| s.points.iter().find(|q| q.id == id).map(|q| (q.x, q.y));
    let (c, a, b) = s.entities.iter().find_map(|e| match e.kind.as_arc() {
        Some(qymcad_core::model::ArcParts { center, a, b, .. }) => Some((at(center)?, at(a)?, at(b)?)),
        _ => None,
    })?;
    let radius = (a.0 - c.0).hypot(a.1 - c.1);
    let chord = (b.0 - a.0).hypot(b.1 - a.1);
    let sweep = 2.0 * (chord / (2.0 * radius)).clamp(-1.0, 1.0).asin(); // the arcs here turn less than half a circle
    Some(Measured { radius, chord, arc: radius * sweep })
}

/// Set the value of the dimension the fillet keeps (a radius, a chord or an arc length) and solve.
fn set_size(p: &mut Project, si: usize, v: f64) {
    let ci =
        p.sketches[si].constraints.iter().rposition(|c| matches!(c, Constraint::Diameter { .. } | Constraint::ArcLength { .. } | Constraint::Distance { .. })).expect("the fillet keeps a dimension");
    match &mut p.sketches[si].constraints[ci] {
        Constraint::Diameter { d, .. } | Constraint::Distance { d, .. } => *d = v,
        Constraint::ArcLength { len, .. } => *len = v,
        _ => unreachable!(),
    }
    let resid = p.solve_sketch(si);
    assert!(resid < 1e-6, "the size changed to {v} left the sketch unsolved: residual {resid:e}");
}

/// Equal within what the solver leaves: a radius worked back from a solved chord is off by 1.7e-6.
fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-5
}

#[test]
fn a_fillet_by_its_radius_its_chord_or_its_arc_length() {
    let mut sins = Vec::new();
    let pi = std::f64::consts::PI;
    // radius 5, chord 5 (radius 5 / sqrt 2 = 3.54), arc 5 (radius 10 / pi = 3.18); each edited afterwards to 6
    let cases = [(FilletBy::Radius, 5.0, 6.0), (FilletBy::Chord, 5.0 / 2f64.sqrt(), 6.0 / 2f64.sqrt()), (FilletBy::ArcLength, 10.0 / pi, 12.0 / pi)];
    for (by, r, r_after) in cases {
        let (mut p, si, pc) = corner();
        if !p.fillet_at_vertex_by(si, pc, FilletSize { by, value: 5.0 }) {
            sins.push(format!("{by:?} 5 refused on a square corner"));
            continue;
        }
        match measure(&p, si) {
            Some(m) if near(m.radius, r) => {}
            m => sins.push(format!("{by:?} 5: the radius is {:?}, not {r:.4}", m.map(|m| m.radius))),
        }
        set_size(&mut p, si, 6.0);
        match measure(&p, si) {
            Some(m) if near(m.radius, r_after) && near([m.radius, m.chord, m.arc][by as usize], 6.0) => {}
            m => sins.push(format!("{by:?} changed to 6: radius / chord / arc {:?}, not a radius of {r_after:.4}", m.map(|m| (m.radius, m.chord, m.arc)))),
        }
    }

    // what the corner cannot take: a chord as long as the lines, an arc longer than a quarter circle of radius 30
    for (by, v) in [(FilletBy::Chord, 45.0), (FilletBy::ArcLength, 50.0)] {
        let (mut p, si, pc) = corner();
        if p.fillet_at_vertex_by(si, pc, FilletSize { by, value: v }) {
            sins.push(format!("{by:?} {v} on lines of 30 must be refused"));
        }
    }

    // two picked lines rounded by an arc of 5, as the editing tool does it
    let (mut p, si, _) = corner();
    let lines: Vec<u64> = p.sketches[si].entities.iter().map(|e| e.id).collect();
    let picked = p.corner_of_pair(si, lines[0], lines[1]).map(|pc| qymcad_core::model::CornerAt::of(pc, [lines[0], lines[1]]));
    if !picked.is_some_and(|c| p.cut_corner_set(si, &[c], qymcad_core::model::CornerCut::Fillet(FilletSize { by: FilletBy::ArcLength, value: 5.0 }), None) == 1) {
        sins.push("two picked lines by an arc of 5 refused".to_string());
    } else if !measure(&p, si).is_some_and(|m| near(m.arc, 5.0) && near(m.radius, 10.0 / pi)) {
        sins.push(format!("two picked lines by an arc of 5: radius / chord / arc {:?}", measure(&p, si).map(|m| (m.radius, m.chord, m.arc))));
    }

    // every corner of a rectangle by a chord of 4: four arcs, each with its ends 4 apart
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("R");
    p.add_rect_entity(si, 0.0, 0.0, 40.0, 30.0, Purpose::Real);
    p.solve_sketch(si);
    let n = p.fillet_all_corners_by(si, FilletSize { by: FilletBy::Chord, value: 4.0 }, None);
    let s = &p.sketches[si];
    let at = |id| s.points.iter().find(|q| q.id == id).map(|q| (q.x, q.y));
    let chords: Vec<f64> = s
        .entities
        .iter()
        .filter_map(|e| match e.kind.as_arc() {
            Some(qymcad_core::model::ArcParts { a, b, .. }) => Some((at(a)?, at(b)?)),
            _ => None,
        })
        .map(|(a, b)| (b.0 - a.0).hypot(b.1 - a.1))
        .collect();
    if n != 4 || chords.len() != 4 || chords.iter().any(|c| !near(*c, 4.0)) {
        sins.push(format!("every corner of a rectangle by a chord of 4: {n} rounded, chords {chords:?}"));
    }
    assert!(sins.is_empty(), "{}", sins.join("\n"));
}
