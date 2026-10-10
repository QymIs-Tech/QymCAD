//! A RECTANGLE DRAWN AS CONSTRUCTION IS A RECTANGLE: by two corners, from its centre or by three points, it is laid as
//! the same rectangle is laid as ordinary geometry - its record, its own constraints, its freedoms - only dashed.
//!
//! Reported behaviour: "a construction rectangle is just four lines: its size cannot be changed and it cannot be dragged
//! by its corners".
use qymcad_core::feature::Purpose;
use qymcad_core::geom::Point2;
use qymcad_core::model::Project;

/// HOW A RECTANGLE IS DRAWN.
#[derive(Clone, Copy, Debug)]
enum Drawn {
    Corners,
    Centre,
    ThreePoints,
}

/// WHAT A RECTANGLE IS LAID AS: its records, its constraints, its freedoms, its curves drawn as construction.
#[derive(Debug, PartialEq)]
struct Laid {
    rects: usize,
    constraints: usize,
    dof: (i32, i32),
    construction: usize,
}

fn laid(drawn: Drawn, purpose: Purpose) -> Laid {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("S");
    match drawn {
        Drawn::Corners => {
            p.add_rect_entity(si, 0.0, 0.0, 40.0, 20.0, purpose);
        }
        Drawn::Centre => {
            p.add_rect_from_centre(si, Point2::new(20.0, 10.0), Point2::new(40.0, 20.0), purpose);
        }
        Drawn::ThreePoints => {
            p.add_rect3_entity(si, Point2::new(0.0, 0.0), Point2::new(30.0, 10.0), Point2::new(25.0, 25.0), purpose);
        }
    }
    let s = &p.sketches[si];
    // the sides drawn dashed: the four lines between the corners, whatever else (the diagonals of a rectangle drawn
    // from its centre) is construction
    let construction = s.entities.iter().filter(|e| e.construction && !s.rects.iter().any(|r| r.diagonals.is_some_and(|d| d.contains(&e.id)))).count();
    Laid { rects: s.rects.len(), constraints: s.constraints.len(), dof: p.sketch_dof(si), construction }
}

#[test]
fn a_construction_rectangle_is_laid_as_an_ordinary_one() {
    let mut failures = Vec::new();
    for drawn in [Drawn::Corners, Drawn::Centre, Drawn::ThreePoints] {
        let (real, dashed) = (laid(drawn, Purpose::Real), laid(drawn, Purpose::Construction));
        let want = Laid { construction: 4, ..laid(drawn, Purpose::Real) };
        // the diagonals of a rectangle drawn from its centre are construction either way; without a record they are
        // not told apart from the sides, so a construction rectangle laid as four lines counts its sides as 4 too

        if dashed != want || real.rects != 1 {
            failures.push(format!("{drawn:?}: as ordinary {real:?}, as construction {dashed:?}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
