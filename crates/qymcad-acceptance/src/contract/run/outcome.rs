//! Whether what the tool left behind is what it should be, and the words a failed point says.

use super::*;

/// IS WHAT THE TOOL LEFT BEHIND WHAT IT SHOULD BE - the body of the part, or the geometry of the sketch?
pub(crate) fn matches(s: &mut Session, want: &Outcome) -> Result<(), String> {
    let doc = s.document();
    match *want {
        Outcome::Body { volume, faces, edges, min, max } => {
            let b = the_body(&doc)?;
            let close = |a: f64, b: f64| (a - b).abs() <= 1e-5 * b.abs().max(1.0);
            // the box of a body is read off its mesh, whose points on a curve lie inside the true surface by up to
            // the tessellation deflection - 0.0015 of the body's diagonal at the ordinary quality. A sphere of
            // radius 20 measures 19.953 across the mesh, one of radius 1000 measures 999.5: a fixed allowance
            // would have to be wrong at one size or the other, so the allowance follows the body.
            let diagonal = (0..3).map(|i| (max[i] - min[i]).powi(2)).sum::<f64>().sqrt();
            let slack = (0.0015 * diagonal).max(0.05);
            let box_close = |a: [f64; 3], b: [f64; 3]| a.iter().zip(b.iter()).all(|(x, y)| (x - y).abs() <= slack);
            let got = (b.volume, b.faces, b.edges.unwrap_or(usize::MAX), b.min, b.max);
            if close(got.0, volume) && got.1 == faces && got.2 == edges && box_close(got.3, min) && box_close(got.4, max) {
                Ok(())
            } else {
                Err(format!("the body is {got:?}, it should be {:?}", (volume, faces, edges, min, max)))
            }
        }
        Outcome::Pieces { pieces, volume } => {
            let (volume_got, at) = {
                let b = the_body(&doc)?;
                (b.volume, doc.bodies.iter().position(|x| x.name == b.name).unwrap_or(0))
            };
            let solids = s.inspect(at).map(|i| i.solids).unwrap_or(0);
            let close = |a: f64, b: f64| (a - b).abs() <= 1e-5 * b.abs().max(1.0);
            if solids == pieces && close(volume_got, volume) {
                Ok(())
            } else {
                Err(format!("the body holds {solids} separate pieces of {volume_got} mm^3 together, it should hold {pieces} of {volume}"))
            }
        }
        Outcome::Volume { volume, tol, min, max } => {
            let b = the_body(&doc)?;
            let diagonal = (0..3).map(|i| (max[i] - min[i]).powi(2)).sum::<f64>().sqrt();
            let slack = (0.0015 * diagonal).max(0.05);
            let in_box = b.min.iter().zip(min.iter()).chain(b.max.iter().zip(max.iter())).all(|(g, w)| (g - w).abs() <= slack);
            if (b.volume - volume).abs() <= tol && in_box {
                Ok(())
            } else {
                Err(format!("the body holds {} mm^3 in {:?}..{:?}; it should hold {volume} to within {tol} in {min:?}..{max:?}", b.volume, b.min, b.max))
            }
        }
        Outcome::Construction { count, lines } => {
            let sk = doc.sketches.last().ok_or_else(|| "the document holds no sketch".to_string())?;
            if sk.construction == count && sk.lines == lines {
                Ok(())
            } else {
                Err(format!("the sketch holds {} lines, {} of its geometry construction; it should hold {lines}, {count} construction", sk.lines, sk.construction))
            }
        }
        Outcome::Picked { count } => {
            let sk = doc.sketches.last().ok_or_else(|| "the document holds no sketch".to_string())?;
            if sk.picked == count {
                Ok(())
            } else {
                Err(format!("{} entities of the sketch are selected, it should be {count}", sk.picked))
            }
        }
        Outcome::Mates { kinds } => {
            if doc.mates.iter().map(String::as_str).eq(kinds.iter().copied()) {
                Ok(())
            } else {
                Err(format!("the assembly is tied by the mates {:?}, it should be by {kinds:?}", doc.mates))
            }
        }
        Outcome::Turned { part, degrees } => {
            let Some(p) = doc.parts.get(part) else { return Err(format!("the assembly holds {} parts, not {}", doc.parts.len(), part + 1)) };
            // the angle of the turn from the trace of its matrix: cos = (trace - 1) / 2
            let trace = p.axes[0][0] + p.axes[1][1] + p.axes[2][2];
            let turned = ((trace - 1.0) / 2.0).clamp(-1.0, 1.0).acos().to_degrees();
            let want = degrees.rem_euclid(360.0);
            let want = want.min(360.0 - want);
            if (turned - want).abs() < 0.5 {
                Ok(())
            } else {
                Err(format!("{:?} is turned by {turned:.2} deg from how it was built, it should be by {want:.2}; its axes look {:?}", p.name, p.axes))
            }
        }
        Outcome::TreeRows { shown, hidden } => {
            let canvas = s.canvas();
            let rows: Vec<String> = s.words_at().into_iter().filter(|(_, r)| r.max.x < canvas.min.x && r.min.y > canvas.min.y).map(|(w, _)| w).collect();
            let missing: Vec<&str> = shown.iter().copied().filter(|w| !rows.iter().any(|r| r.starts_with(w))).collect();
            let left_in: Vec<&str> = hidden.iter().copied().filter(|w| rows.iter().any(|r| r.starts_with(w))).collect();
            if missing.is_empty() && left_in.is_empty() {
                Ok(())
            } else {
                Err(format!("the tree should show {missing:?} and does not, should not show {left_in:?} and does; it shows {rows:?}"))
            }
        }
        Outcome::Fitted => {
            let canvas = s.canvas();
            let mut outside = Vec::new();
            for b in doc.bodies.iter().filter(|b| b.visible && !b.consumed) {
                for i in 0..8 {
                    let corner = [if i & 1 == 0 { b.min[0] } else { b.max[0] }, if i & 2 == 0 { b.min[1] } else { b.max[1] }, if i & 4 == 0 { b.min[2] } else { b.max[2] }];
                    if !canvas.contains(s.projected(corner)) {
                        outside.push(corner);
                    }
                }
            }
            if doc.bodies.is_empty() || outside.is_empty() {
                Ok(())
            } else {
                Err(format!("the corners {outside:?} fall outside the canvas {canvas:?}"))
            }
        }
        Outcome::OnScreen { word } => {
            let want = s.word(word);
            if s.shows(&want) {
                Ok(())
            } else {
                Err(format!("{want:?} is not on screen"))
            }
        }
        Outcome::Says { text } => {
            let said = s.status();
            if said.contains(text) {
                Ok(())
            } else {
                Err(format!("the status line says {said:?}; it should say {text:?}"))
            }
        }
        Outcome::Section { on } => {
            if doc.section == on {
                Ok(())
            } else {
                Err(format!("the view is {}cut by a section; it should {}be", if doc.section { "" } else { "not " }, if on { "" } else { "not " }))
            }
        }
        Outcome::Shade { dark } => {
            let pic = s.snapshot();
            let (mut sum, mut n) = (0u64, 0u64);
            for px in pic.rgba.as_chunks::<4>().0 {
                sum += (u64::from(px[0]) * 299 + u64::from(px[1]) * 587 + u64::from(px[2]) * 114) / 1000;
                n += 1;
            }
            let mean = sum.checked_div(n).unwrap_or(0);
            if (mean < 128) == dark {
                Ok(())
            } else {
                Err(format!("the window is drawn with a mean brightness of {mean} of 255; it should be {}", if dark { "dark" } else { "light" }))
            }
        }
        Outcome::Chooser { save } => {
            let want = if save { qymcad::Chooser::Save } else { qymcad::Chooser::Open };
            match s.chooser() {
                Some(k) if k == want => Ok(()),
                other => Err(format!("the chooser up is {other:?}, it should be {want:?}")),
            }
        }
        Outcome::Window { title } => {
            let want = s.word(title);
            let open = s.windows();
            if open.iter().any(|w| w.contains(&want)) {
                Ok(())
            } else {
                Err(format!("no window {want:?} is open; windows open: {open:?}"))
            }
        }
        Outcome::Parts { parts, grounded, joints, assemblies } => {
            let own: Vec<&qymcad::Part> = doc.parts.iter().filter(|p| !p.assembly).collect();
            let subs = doc.parts.iter().filter(|p| p.assembly).count();
            let fixed = own.iter().filter(|p| p.grounded).count();
            let held = doc.joints.iter().filter(|j| !j.violated).count();
            if own.len() == parts && fixed == grounded && doc.joints.len() == joints && held == joints && subs == assemblies {
                Ok(())
            } else {
                Err(format!("the assembly holds {} parts, {fixed} of them grounded, {} joints ({held} held), {subs} sub-assemblies; it should hold {parts}, {grounded} grounded, {joints} joints held, {assemblies} sub-assemblies", own.len(), doc.joints.len()))
            }
        }
        Outcome::Row { parts, step } => {
            let at: Vec<[f64; 3]> = doc.parts.iter().filter(|p| !p.assembly).map(|p| p.at).collect();
            let stepped = at.windows(2).all(|w| (0..3).all(|k| (w[1][k] - w[0][k] - step[k]).abs() < 1e-3));
            if at.len() == parts && stepped {
                Ok(())
            } else {
                Err(format!("the parts stand at {at:?}; there should be {parts} of them, each {step:?} on from the one before"))
            }
        }
        Outcome::Bodies { count, volume, largest } => {
            let got: Vec<f64> = doc.bodies.iter().filter(|b| b.visible && !b.consumed).map(|b| b.volume).collect();
            let whole: f64 = got.iter().sum();
            let biggest = got.iter().copied().fold(0.0, f64::max);
            let close = |a: f64, b: f64| (a - b).abs() <= 1e-5 * b.abs().max(1.0);
            if got.len() == count && close(whole, volume) && close(biggest, largest) {
                Ok(())
            } else {
                Err(format!("the part holds {} bodies of {whole} mm^3, the largest {biggest}; it should hold {count} of {volume}, the largest {largest}", got.len()))
            }
        }
        Outcome::Sheet { area, min, max } => {
            let near = |a: [f64; 3], b: [f64; 3]| (0..3).all(|k| (a[k] - b[k]).abs() < 1e-3);
            let sheets: Vec<&Solid> = doc.bodies.iter().filter(|b| b.visible && !b.consumed && b.sheet).collect();
            if sheets.iter().any(|b| (b.area - area).abs() <= 1e-3 * area.max(1.0) && near(b.min, min) && near(b.max, max)) {
                Ok(())
            } else {
                Err(format!("no sheet of {area} mm^2 from {min:?} to {max:?}; the sheets shown are {:?}", sheets.iter().map(|b| (b.area, b.min, b.max)).collect::<Vec<_>>()))
            }
        }
        Outcome::Sketch { points, lines, arcs, circles, ellipses, splines, texts, notes, constraints, dof, box_of, size_of, under } => {
            let sk = doc.sketches.last().ok_or_else(|| "the document holds no sketch".to_string())?.clone();
            let got = (sk.points, sk.lines, sk.arcs, sk.circles, sk.ellipses, sk.splines, sk.texts, sk.notes);
            if got != (points, lines, arcs, circles, ellipses, splines, texts, notes) {
                return Err(format!(
                    "the sketch holds {got:?} (points, lines, arcs, circles, ellipses, splines, texts, notes), it should hold {:?}",
                    (points, lines, arcs, circles, ellipses, splines, texts, notes)
                ));
            }
            if constraints.is_some_and(|want| sk.constraints != want) {
                return Err(format!("the sketch holds {} constraints and dimensions, it should hold {:?}", sk.constraints, constraints));
            }
            if dof.is_some_and(|want| sk.dof != want) {
                return Err(format!("the sketch has {} degrees of freedom left, it should have {:?}", sk.dof, dof));
            }
            if let Some(size) = size_of {
                let got = [sk.max[0] - sk.min[0], sk.max[1] - sk.min[1]];
                if got.iter().zip(size.iter()).any(|(a, b)| (a - b).abs() > 1e-3) {
                    return Err(format!("what is drawn is {got:?} across, it should be {size:?}"));
                }
            }
            if let Some((min, max)) = box_of {
                let close = |a: [f64; 2], b: [f64; 2]| a.iter().zip(b.iter()).all(|(x, y)| (x - y).abs() <= 1e-3);
                if !close(sk.min, min) || !close(sk.max, max) {
                    return Err(format!("what is drawn stands in the box {:?}..{:?}, it should stand in {min:?}..{max:?}", sk.min, sk.max));
                }
            }
            let mut wrong = Vec::new();
            for (x, y, what) in under {
                let there = s.sketch_under(*x, *y);
                let is = matches!(
                    (&there, what),
                    (None, Under::Nothing)
                        | (Some(qymcad::SketchPick::Point { .. }), Under::Point)
                        | (Some(qymcad::SketchPick::Line { .. }), Under::Line)
                        | (Some(qymcad::SketchPick::Arc { .. }), Under::Arc)
                        | (Some(qymcad::SketchPick::Circle { .. }), Under::Circle)
                        | (Some(qymcad::SketchPick::Ellipse { .. }), Under::Ellipse)
                        | (Some(qymcad::SketchPick::Spline { .. }), Under::Spline)
                );
                if !is {
                    wrong.push(format!("at ({x}, {y}) lies {there:?}, it should be {what:?}"));
                }
            }
            if wrong.is_empty() {
                Ok(())
            } else {
                Err(wrong.join("; "))
            }
        }
    }
}

/// What a check compares of a document: its timeline, its bodies and their numbers, its sketches.
#[derive(Debug, PartialEq)]
pub(crate) struct Counted {
    pub(crate) nodes: Vec<crate::oracles::NodeShape>,
    pub(crate) bodies: Vec<BodyCount>,
    pub(crate) sketches: Vec<crate::oracles::SketchShape>,
}

/// A body by its name, its volume in thousandths of a mm^3, its faces and its edges.
#[derive(Debug, PartialEq)]
pub(crate) struct BodyCount {
    pub(crate) name: String,
    pub(crate) volume: u64,
    pub(crate) faces: usize,
    pub(crate) edges: Option<usize>,
}

pub(crate) fn shape_of(doc: &Document) -> Counted {
    Counted {
        nodes: crate::oracles::node_shapes(doc),
        bodies: doc.bodies.iter().map(|b| BodyCount { name: b.name.clone(), volume: (b.volume * 1e3).round() as u64, faces: b.faces, edges: b.edges }).collect(),
        sketches: crate::oracles::sketch_shapes(doc),
    }
}

/// A place on the canvas with nothing drawn on it: a corner of the canvas.
pub(crate) fn empty_space(s: &mut Session) -> Pos2 {
    let c = s.canvas();
    c.min + (c.max - c.min) * 0.04
}

/// A PICTURE OF THE WINDOW WITH THE POINTER PUT ASIDE, so what is under it is not lit up by the hover alone.
pub(crate) fn look(s: &mut Session) -> qymcad::Picture {
    let aside = empty_space(s);
    s.move_to(aside);
    s.snapshot()
}

/// Where the fields for typing stand: a click elsewhere takes the keyboard from them and redraws them, which is not
/// something drawn on the canvas.
pub(crate) fn fields_at(s: &mut Session) -> Vec<qymcad::Rect> {
    s.widgets().into_iter().filter(|w| matches!(w.kind, qymcad::Kind::TextField | qymcad::Kind::Number)).map(|w| w.rect).collect()
}
