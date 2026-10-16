//! THE LOOPS ROUND A CHANGE. When curves of a sketch move, only the regions they bounded or crossed before and the
//! regions they bound or cross now can change; every other region stands. They used to be found as every curve whose
//! box meets a moved one, and every curve whose box meets one of those, and so on - in a grid where every line crosses
//! every line across it that is the whole sketch: a frame of a drag of one line of a grid of 200 by 200 made all its
//! 40 000 cells again, 0.7 s in a release build.
//!
//! Here the regions are made inside a window: the boxes of the moved curves where they stood and where they stand (the
//! swept boxes), and of the regions that held them before. Every curve that meets the window is laid, and four lines
//! are laid round it. A region inside the window and clear of those four lines is a region of the whole sketch: a
//! curve that would cut it meets its box, and so the window, and is laid. A region that meets a swept box and runs
//! into the side of the window may go on past it, and the window grows on that side to the farthest curve laid; a side
//! already past every curve of the sketch has nothing behind it, and what runs into it is the outside.
//!
//! The open chains are made apart, from the curves joined end to end with a moved one: a chain is all the curves so
//! joined, wherever they reach.
use super::tess::{arrangement_faces, tessellate_sketch_multi};
use super::*;
use crate::geom::ProfEdge;
use std::collections::{BTreeSet, HashMap, HashSet};

/// WHAT THE LOOPS ROUND A CHANGE ARE MADE FROM: the curves drawn (not construction), each with its box where it stands,
/// a way to find the curves whose boxes may meet a box, the curves that moved, and where they swept.
pub(super) struct Change<'a> {
    /// the curve drawn at a place, and the box of each where it stands
    pub curve: &'a dyn Fn(usize) -> SketchEntity,
    pub boxes: &'a [[f64; 4]],
    /// the point of an id where it stands
    pub at: &'a dyn Fn(Id) -> Option<SketchPoint>,
    /// the curves whose boxes may meet a box, by place among `curves`; more is harmless, fewer is not
    pub near: &'a dyn Fn(&[f64; 4]) -> Vec<usize>,
    /// the curves that moved, by place
    pub moved: &'a [usize],
    /// the boxes of the moved curves where they stood and where they stand
    pub swept: &'a [[f64; 4]],
    /// the union of the boxes of the regions before the change that held a moved curve or met a swept box
    pub held: Option<[f64; 4]>,
}

/// The regions round a change, each with the curves it is made of, as the regions of the whole sketch would give them:
/// those whose box meets a swept box.
pub(super) fn regions_round(ch: &Change) -> Vec<(Contour, Vec<Id>)> {
    let Some(all) = ch.boxes.iter().copied().filter(finite).reduce(union) else { return Vec::new() };
    let Some(mut w) = ch.swept.iter().copied().chain(ch.held).filter(finite).reduce(union) else { return Vec::new() };
    let moved: HashSet<Id> = ch.moved.iter().map(|&k| (ch.curve)(k).id).collect();
    // an end of a moved curve with nothing else of the sketch at it
    let free_end = |id: Id, p: Point2| -> bool {
        let Some(&k) = ch.moved.iter().find(|&&k| (ch.curve)(k).id == id) else { return false };
        let is_end = match (ch.curve)(k).kind {
            EntityKind::Line { a, b } | EntityKind::Arc { a, b, .. } => [a, b].into_iter().filter_map(ch.at).any(|q| (q.x - p.x).hypot(q.y - p.y) < 1e-6),
            _ => false,
        };
        let spot = [p.x - 1e-3, p.y - 1e-3, p.x + 1e-3, p.y + 1e-3];
        is_end && (ch.near)(&spot).into_iter().all(|j| j == k || !meet(&ch.boxes[j], &spot))
    };
    loop {
        let laid: Vec<usize> = (ch.near)(&w).into_iter().chain(ch.moved.iter().copied()).collect::<BTreeSet<usize>>().into_iter().filter(|&k| meet(&ch.boxes[k], &w)).collect();
        let margin = 1.0 + 0.01 * (w[2] - w[0]).max(w[3] - w[1]);
        let rim = [w[0] - margin, w[1] - margin, w[2] + margin, w[3] + margin];
        let mut ents: Vec<SketchEntity> = laid.iter().map(|&k| (ch.curve)(k)).collect();
        let mut pts: Vec<SketchPoint> = ents.iter().flat_map(entity_points).collect::<BTreeSet<Id>>().into_iter().filter_map(ch.at).collect();
        // the four sides of the window, by ids no sketch gives: a corner a point, a side a line
        let corner = [(rim[0], rim[1]), (rim[2], rim[1]), (rim[2], rim[3]), (rim[0], rim[3])];
        for (k, &(x, y)) in corner.iter().enumerate() {
            pts.push(SketchPoint { id: RIM_POINT - k as Id, x, y });
        }
        for side in 0..4 {
            ents.push(SketchEntity { id: RIM_SIDE - side as Id, kind: EntityKind::Line { a: RIM_POINT - side as Id, b: RIM_POINT - ((side + 1) % 4) as Id }, construction: false });
        }
        let mut out = Vec::new();
        // the sides a region running out of the window was found at: bottom, right, top, left
        let mut grow = [false; 4];
        let mut grew = false;
        let faces = arrangement_faces(&pts, &ents, &[]); // a sketch with splines is never laid round (`regen_sketch`)
                                                         // A PIECE OF THE DRAWING SEEN WHOLE IN THE WINDOW, clear of its sides, is walked round on its outside and not
                                                         // made a region: a moved curve on that walk may close a region with what lies past the window. The window takes
                                                         // the piece in, and the next laying reaches further along it - the half disc whose diameter was drawn last came
                                                         // in a few segments a step
        for walk in &faces.outer {
            let b = contour_box(walk);
            if !inside(&b, &w) && borders_a_moved_curve(walk, &moved, &free_end) {
                w = union(w, b);
                grew = true;
            }
        }
        for (c, prov) in faces.inner {
            let b = contour_box(&c);
            if !ch.swept.iter().any(|s| meet(&b, s)) {
                continue; // a region the change did not reach: the one of the sketch stands
            }
            let sides: Vec<usize> = prov.iter().chain(&c.edge_src).filter(|id| **id <= RIM_SIDE && **id > RIM_SIDE - 4).map(|id| (RIM_SIDE - id) as usize).collect();
            if !sides.is_empty() {
                if borders_a_moved_curve(&c, &moved, &free_end) {
                    for side in sides {
                        grow[side] = true;
                    }
                }
            } else if inside(&b, &w) {
                out.push((c, prov));
            } else if inside(&b, &rim) {
                // between the window and its sides: a curve laid in neither may cut it, so the window takes it in
                w = union(w, b);
                grew = true;
            }
        }
        // a side grows to the farthest curve laid past it, or to the sketch's own box; a side at the sketch's box stays
        let reach = laid.iter().map(|&k| ch.boxes[k]).filter(finite).fold(w, union);
        for (side, at) in [(0, 1), (1, 2), (2, 3), (3, 0)] {
            if !grow[side] {
                continue;
            }
            let (now, past, end) = match at {
                0 => (w[0], reach[0], all[0]),
                1 => (w[1], reach[1], all[1]),
                2 => (w[2], reach[2], all[2]),
                _ => (w[3], reach[3], all[3]),
            };
            let outward = at >= 2;
            let at_end = if outward { now >= end } else { now <= end };
            if at_end {
                continue;
            }
            let further = if outward { past > now } else { past < now };
            w[at] = if further { past } else { end };
            grew = true;
        }
        if !grew {
            return out;
        }
    }
}

/// The open chains round a change, and the curves they were made from.
pub(super) struct Chains {
    pub made: Vec<(Contour, Vec<Id>)>,
    /// every curve reached: the old chains through any of them give way to the ones made
    pub through: HashSet<Id>,
}

/// The open chains of the curves joined end to end with `from` (by place), wherever the joins reach.
pub(super) fn chains_round(ch: &Change, from: &[usize]) -> Chains {
    let ends = |k: usize| -> Vec<(f64, f64)> {
        match (ch.curve)(k).kind {
            EntityKind::Line { a, b } | EntityKind::Arc { a, b, .. } => [a, b].into_iter().filter_map(ch.at).map(|p| (p.x, p.y)).collect(),
            _ => Vec::new(),
        }
    };
    let joined = |a: &[(f64, f64)], b: &[(f64, f64)]| a.iter().any(|p| b.iter().any(|q| (p.0 - q.0).powi(2) + (p.1 - q.1).powi(2) < 1e-6));
    let mut reached: HashSet<usize> = HashSet::new();
    let mut queue: Vec<usize> = Vec::new();
    for &k in from {
        if !ends(k).is_empty() && reached.insert(k) {
            queue.push(k);
        }
    }
    while let Some(k) = queue.pop() {
        let mine = ends(k);
        for j in (ch.near)(&ch.boxes[k]) {
            if !reached.contains(&j) && meet(&ch.boxes[k], &ch.boxes[j]) && joined(&mine, &ends(j)) {
                reached.insert(j);
                queue.push(j);
            }
        }
    }
    let mut order: Vec<usize> = reached.into_iter().collect();
    order.sort_unstable();
    let ents: Vec<SketchEntity> = order.iter().map(|&k| (ch.curve)(k)).collect();
    let pts: Vec<SketchPoint> = ents.iter().flat_map(entity_points).collect::<BTreeSet<Id>>().into_iter().filter_map(ch.at).collect();
    Chains { made: tessellate_sketch_multi(&pts, &ents).into_iter().filter(|c| !c.closed).map(|c| (c, Vec::new())).collect(), through: ents.iter().map(|e| e.id).collect() }
}

/// WHETHER A MOVED CURVE BOUNDS A REGION, so the region may run on past the side of the window and close there: an edge
/// of it with another face across, or an edge walked both ways that does not end at a free end of its curve. In the
/// window a curve is laid without what lies past it, and a curve the window shows hanging may close a loop out of
/// sight: the diameter of a half disc drawn last, laid with only the two segments of the arc beside its ends, was walked
/// both ways, the window did not grow, and the half disc was lost. Only an end standing alone in the whole sketch -
/// a line past the last line across it in a grid - hangs for sure; grown for such ends, the window of a line of a grid
/// of 200 by 200 took the whole grid, 0.25 s a frame.
fn borders_a_moved_curve(c: &Contour, moved: &HashSet<Id>, free_end: &dyn Fn(Id, Point2) -> bool) -> bool {
    let key = |p: Point2| ((p.x * 1e6).round() as i64, (p.y * 1e6).round() as i64);
    let ends = |e: &ProfEdge| match *e {
        ProfEdge::Line { a, b } | ProfEdge::Arc { a, b, .. } => Some((a, b)),
        ProfEdge::Circle { .. } => None,
    };
    let walked: HashSet<((i64, i64), (i64, i64))> = c.edges.iter().filter_map(ends).map(|(a, b)| (key(a), key(b))).collect();
    c.edges.iter().zip(&c.edge_src).filter(|(_, id)| moved.contains(id)).any(|(e, &id)| match ends(e) {
        None => true,
        Some((a, b)) => !walked.contains(&(key(b), key(a))) || !(free_end(id, a) || free_end(id, b)),
    })
}

/// The ids of the four corners and of the four sides of the window: past every id a sketch hands out.
const RIM_POINT: Id = Id::MAX - 16;
const RIM_SIDE: Id = Id::MAX - 32;

pub(super) fn finite(b: &[f64; 4]) -> bool {
    b.iter().all(|v| v.is_finite())
}

pub(super) fn union(a: [f64; 4], b: [f64; 4]) -> [f64; 4] {
    [a[0].min(b[0]), a[1].min(b[1]), a[2].max(b[2]), a[3].max(b[3])]
}

pub(super) fn meet(a: &[f64; 4], b: &[f64; 4]) -> bool {
    a[0] <= b[2] && b[0] <= a[2] && a[1] <= b[3] && b[1] <= a[3] || !finite(a) || !finite(b)
}

fn inside(a: &[f64; 4], b: &[f64; 4]) -> bool {
    a[0] >= b[0] && a[1] >= b[1] && a[2] <= b[2] && a[3] <= b[3]
}

/// The box of a loop over its points.
pub(super) fn contour_box(c: &Contour) -> [f64; 4] {
    c.points.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, p| [b[0].min(p.x), b[1].min(p.y), b[2].max(p.x), b[3].max(p.y)])
}

/// A grid of boxes, to find the boxes that may meet a box without a look at every one.
pub(super) struct BoxGrid {
    cell: f64,
    cells: HashMap<(i64, i64), Vec<usize>>,
    /// a box over more than 64 cells, or at no number: offered for every box
    wide: Vec<usize>,
}

impl BoxGrid {
    pub(super) fn new(boxes: &[[f64; 4]]) -> Self {
        let n = boxes.len().max(1) as f64;
        let cell = (boxes.iter().filter(|b| finite(b)).map(|b| (b[2] - b[0]).max(b[3] - b[1])).sum::<f64>() / n).max(1e-3);
        let mut grid = BoxGrid { cell, cells: HashMap::new(), wide: Vec::new() };
        for (k, b) in boxes.iter().enumerate() {
            match grid.span(b) {
                Some([x0, y0, x1, y1]) => {
                    for x in x0..=x1 {
                        for y in y0..=y1 {
                            grid.cells.entry((x, y)).or_default().push(k);
                        }
                    }
                }
                None => grid.wide.push(k),
            }
        }
        grid
    }

    fn span(&self, b: &[f64; 4]) -> Option<[i64; 4]> {
        if !finite(b) {
            return None;
        }
        let s = [(b[0] / self.cell).floor() as i64, (b[1] / self.cell).floor() as i64, (b[2] / self.cell).floor() as i64, (b[3] / self.cell).floor() as i64];
        ((s[2] - s[0] + 1) * (s[3] - s[1] + 1) <= 64).then_some(s)
    }

    /// The boxes that may meet `b`; every box where `b` is too wide for the grid.
    pub(super) fn near(&self, b: &[f64; 4], all: usize) -> Vec<usize> {
        match self.span(b) {
            Some([x0, y0, x1, y1]) => {
                (x0..=x1).flat_map(|x| (y0..=y1).map(move |y| (x, y))).filter_map(|c| self.cells.get(&c)).flatten().chain(&self.wide).copied().collect::<BTreeSet<usize>>().into_iter().collect()
            }
            None => (0..all).collect(),
        }
    }
}

/// WHAT THE LOOPS OF A SKETCH WERE MADE FROM: a print of each drawn curve where it stood, and a print of the loops made.
/// A rebuild finds by it which curves changed since, and makes again only the loops round them (`regions_round`);
/// nothing changed, the loops stand. Kept by every path that makes loops; where the loops of the sketch are not the
/// ones printed - a loop taken out apart, a sketch copied with its loops cleared - the sketch is rebuilt whole.
/// Measured on a grid of two patterns of 200 lines: a rebuild of the whole grid 0.27 s in a release build.
#[derive(Clone, Default)]
pub struct Laid {
    curves: HashMap<Id, u64>,
    loops: u64,
}

/// Printed without its prints, as `IdPlaces`: what the loops were made from is no fact of the sketch.
impl std::fmt::Debug for Laid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Laid")
    }
}

/// The curves of a sketch changed since its loops were made: drawn and changed or new, and gone.
pub(super) struct Changed {
    pub drawn: HashSet<Id>,
    pub gone: HashSet<Id>,
}

/// A print of a drawn curve: its kind, its points and where they stand, its radius and direction.
fn print_of(e: &SketchEntity, at: &dyn Fn(Id) -> Option<SketchPoint>) -> u64 {
    let mut h: u64 = 0;
    let mut mix = |x: u64| h = (h.rotate_left(5) ^ x).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    let (tag, extra) = match e.kind {
        EntityKind::Line { .. } => (1, 0),
        EntityKind::Arc { ccw, .. } => (2, u64::from(ccw)),
        EntityKind::Circle { r, .. } => (3, r.to_bits()),
        EntityKind::Ellipse { .. } => (4, 0),
    };
    mix(tag);
    mix(extra);
    for id in entity_points(e) {
        mix(id);
        let p = at(id).map_or((f64::NAN, f64::NAN), |p| (p.x, p.y));
        mix(p.0.to_bits());
        mix(p.1.to_bits());
    }
    h
}

/// A print of one loop: its id, mixed.
fn print_of_loop(id: Id) -> u64 {
    (id ^ 0x9e_37_79_b9_7f_4a_7c_15).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95).rotate_left(29)
}

/// A print of the loops of a sketch, `None` where one of them is not in the project: the sum of the prints of the loops,
/// so a frame of a drag follows it by the loops it took out and made (`laid_framed`). Made again from every loop for
/// every frame, it was 2 ms of a frame of 16 among 70 000 rectangles.
fn print_of_loops(project: &Project, ids: &[Id]) -> Option<u64> {
    let mut h: u64 = ids.len() as u64;
    for &id in ids {
        project.contour_index(id)?;
        h = h.wrapping_add(print_of_loop(id));
    }
    Some(h)
}

impl Project {
    /// The print of what the loops of sketch `si` are made from, as they stand now.
    pub(super) fn laid_now(&self, si: usize) -> Option<Laid> {
        let s = self.sketches.get(si)?;
        let at = |id: Id| s.point(id).copied();
        let curves = s.entities.iter().filter(|e| !e.construction).map(|e| (e.id, print_of(e, &at))).collect();
        Some(Laid { curves, loops: print_of_loops(self, &s.contour_ids)? })
    }

    /// The curves of sketch `si` changed since its loops were made, `None` where what they were made from is not known
    /// or the loops are not the ones it printed.
    pub(super) fn changed_since_laid(&self, si: usize) -> Option<Changed> {
        let s = self.sketches.get(si)?;
        let laid = s.laid.as_deref()?;
        if print_of_loops(self, &s.contour_ids)? != laid.loops {
            return None;
        }
        let at = |id: Id| s.point(id).copied();
        let mut drawn = HashSet::new();
        let mut seen = 0;
        for e in s.entities.iter().filter(|e| !e.construction) {
            match laid.curves.get(&e.id) {
                Some(&print) => {
                    seen += 1;
                    if print != print_of(e, &at) {
                        drawn.insert(e.id);
                    }
                }
                None => {
                    drawn.insert(e.id);
                }
            }
        }
        let gone = if seen == laid.curves.len() {
            HashSet::new()
        } else {
            let now: HashSet<Id> = s.entities.iter().filter(|e| !e.construction).map(|e| e.id).collect();
            laid.curves.keys().copied().filter(|id| !now.contains(id)).collect()
        };
        Some(Changed { drawn, gone })
    }

    /// The print followed through a frame of a drag: the loops `gone` taken out of it and `made` put in, the curves
    /// `changed` printed where they stand - with no look at every loop of the sketch. Where the loops of the sketch were
    /// changed in another way the print no longer fits them, and the next rebuild makes them whole.
    pub(super) fn laid_framed(&mut self, si: usize, gone: &[Id], made: &[Id], changed: &HashSet<Id>) {
        let Some(s) = self.sketches.get_mut(si) else { return };
        let Some(mut laid) = s.laid.take() else { return };
        let at = |id: Id| s.point(id).copied();
        laid.loops = laid.loops.wrapping_add(made.len() as u64).wrapping_sub(gone.len() as u64);
        for &id in gone {
            laid.loops = laid.loops.wrapping_sub(print_of_loop(id));
        }
        for &id in made {
            laid.loops = laid.loops.wrapping_add(print_of_loop(id));
        }
        for e in changed.iter().filter_map(|id| s.entity(*id)).filter(|e| !e.construction) {
            laid.curves.insert(e.id, print_of(e, &at));
        }
        s.laid = Some(laid);
    }

    /// The print of sketch `si` taken out before its loops are made again round a change, where it holds for the loops
    /// as they stand; it goes back through `laid_follows`.
    pub(super) fn laid_held(&mut self, si: usize) -> Option<Laid> {
        let laid = self.sketches.get_mut(si)?.laid.take()?;
        (print_of_loops(self, &self.sketches[si].contour_ids)? == laid.loops).then_some(*laid)
    }

    /// The print put back after the loops were made again round the curves `changed` (drawn now) and `gone`.
    pub(super) fn laid_follows(&mut self, si: usize, laid: Option<Laid>, changed: &HashSet<Id>, gone: &HashSet<Id>) {
        let Some(s) = self.sketches.get(si) else { return };
        let next = laid.and_then(|mut laid| {
            let at = |id: Id| s.point(id).copied();
            for id in gone {
                laid.curves.remove(id);
            }
            for e in changed.iter().filter_map(|id| s.entity(*id)).filter(|e| !e.construction) {
                laid.curves.insert(e.id, print_of(e, &at));
            }
            laid.loops = print_of_loops(self, &s.contour_ids)?;
            Some(Box::new(laid))
        });
        self.sketches[si].laid = next;
    }
}
