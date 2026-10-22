//! A DRAG REMEMBERS WHAT IT DRAGS. A frame of a drag solves the part of the dragged point and rebuilds the loops of
//! what it moved, and nothing of the rest of the sketch changes from one frame to the next - only where the points of
//! that part stand. Found again on every frame, the part, the rectangles, the reference dimensions and the loops that
//! hold each curve were a walk over the whole sketch each: a frame among 70 000 rectangles took 0.5 s in a release
//! build, with a solve of a few points in it. The first frame of a drag finds them and keeps them (`DragSession`),
//! and the frames after it work on the part alone. The session goes when the sketch is rebuilt whole, and a frame
//! that finds the structure of the sketch changed under it makes a new one.
use super::sketch::{entity_box, held_for_size, measure_driven};
use super::*;
use std::collections::{HashMap, HashSet};

/// Where a radius of the dragged part comes from, to give it to the solve and to take its answer back.
#[derive(Clone, Copy, Debug)]
enum RadiusFrom {
    /// a circle, by its place among the entities
    Circle(usize),
    /// an arc, by the places of its centre and of its first end among the points
    Arc { centre: usize, end: usize },
}

#[derive(Clone, Copy, Debug)]
struct PartRadius {
    centre: Id,
    from: RadiusFrom,
}

/// A rectangle the drag may move, by its place, and whether its centre is put on the middle of its diagonal before
/// a solve (drawn by its corners, its centre held by nothing else).
#[derive(Clone, Copy, Debug)]
struct TouchedRect {
    index: usize,
    mid: bool,
    /// whether its centre is held by the sketch, looked at once when the drag begins (`Centre`)
    centre: super::sketch::Centre,
}

/// THE BOXES OF THE CURVES OF A SKETCH on a grid, by their place among the drawn curves; the curves that do not move
/// stand on its cells, the moving ones are looked at one by one.
#[derive(Clone, Debug, Default)]
struct Boxes {
    cell: f64,
    boxes: Vec<[f64; 4]>,
    cells: HashMap<(i64, i64), Vec<usize>>,
    /// a box over more than 64 cells, or at no number: looked at from every box
    wide: Vec<usize>,
}

impl Boxes {
    fn span(&self, b: &[f64; 4]) -> Option<[i64; 4]> {
        cells_of(self.cell, b)
    }

    fn put(&mut self, k: usize) {
        match self.span(&self.boxes[k]) {
            Some([x0, y0, x1, y1]) => {
                for x in x0..=x1 {
                    for y in y0..=y1 {
                        self.cells.entry((x, y)).or_default().push(k);
                    }
                }
            }
            None => self.wide.push(k),
        }
    }

    /// The standing boxes that may meet box `b`.
    fn near(&self, b: &[f64; 4]) -> Vec<usize> {
        match self.span(b) {
            Some([x0, y0, x1, y1]) => (x0..=x1).flat_map(|x| (y0..=y1).map(move |y| (x, y))).filter_map(|c| self.cells.get(&c)).flatten().copied().chain(self.wide.iter().copied()).collect(),
            None => (0..self.boxes.len()).collect(),
        }
    }
}

/// THE CELLS OF SIDE `cell` A BOX COVERS, as the first and the last column and row; `None` for a box over more than 64 of
/// them, at no number, or of no area - the box of a loop with no point is laid out inverted, and its count of cells ran
/// past what a number holds.
fn cells_of(cell: f64, b: &[f64; 4]) -> Option<[i64; 4]> {
    if !b.iter().all(|v| v.is_finite()) || b[0] > b[2] || b[1] > b[3] {
        return None;
    }
    let s = b.map(|v| (v / cell).floor() as i64);
    let across = s[2].saturating_sub(s[0]).saturating_add(1);
    let up = s[3].saturating_sub(s[1]).saturating_add(1);
    (across.saturating_mul(up) <= 64).then_some(s)
}

/// THE BOXES OF THE LOOPS OF A SKETCH on a grid, by loop id: which loops may meet a box, without a look at every loop.
#[derive(Clone, Debug, Default)]
struct LoopGrid {
    cell: f64,
    cells: HashMap<(i64, i64), Vec<Id>>,
    /// a box over more than 64 cells, or at no number: offered for every box
    wide: Vec<Id>,
}

impl LoopGrid {
    fn span(&self, b: &[f64; 4]) -> Option<[i64; 4]> {
        cells_of(self.cell, b)
    }

    fn put(&mut self, id: Id, b: &[f64; 4]) {
        match self.span(b) {
            Some([x0, y0, x1, y1]) => {
                for x in x0..=x1 {
                    for y in y0..=y1 {
                        self.cells.entry((x, y)).or_default().push(id);
                    }
                }
            }
            None => self.wide.push(id),
        }
    }

    fn take(&mut self, id: Id, b: &[f64; 4]) {
        match self.span(b) {
            Some([x0, y0, x1, y1]) => {
                for x in x0..=x1 {
                    for y in y0..=y1 {
                        if let Some(list) = self.cells.get_mut(&(x, y)) {
                            list.retain(|c| *c != id);
                        }
                    }
                }
            }
            None => self.wide.retain(|c| *c != id),
        }
    }

    /// The loops that may meet box `b`; every loop, where `b` is too wide for the grid.
    fn near(&self, b: &[f64; 4]) -> Option<HashSet<Id>> {
        let [x0, y0, x1, y1] = self.span(b)?;
        Some((x0..=x1).flat_map(|x| (y0..=y1).map(move |y| (x, y))).filter_map(|c| self.cells.get(&c)).flatten().chain(self.wide.iter()).copied().collect())
    }
}

fn meet(a: &[f64; 4], b: &[f64; 4]) -> bool {
    a[0] <= b[2] && b[0] <= a[2] && a[1] <= b[3] && b[1] <= a[3] || !a.iter().chain(b).all(|v| v.is_finite())
}

/// WHAT A DRAG OF ONE POINT KEEPS FROM ONE FRAME TO THE NEXT, see the module.
#[derive(Clone, Debug, Default)]
pub struct DragSession {
    /// the structure of the sketch it was made on (`structure_stamp`), where its points that no frame moves stand
    /// (`still_stamp`), and the point dragged
    stamp: u64,
    still: u64,
    dragged: Id,
    /// the place of every point by its id
    place: HashMap<Id, usize>,
    /// the part of the dragged point: its points, radii and constraints (the reference ones left out, the entities'
    /// own in)
    points: Vec<usize>,
    radii: Vec<PartRadius>,
    constraints: Vec<Constraint>,
    /// what else a frame may move: the points carried with the dragged one, the points held still
    carried: Vec<Id>,
    held: HashSet<Id>,
    rects: Vec<TouchedRect>,
    /// the points watched for a move: the part, what is carried, the centres of the rectangles touched
    watched: Vec<usize>,
    /// the reference dimensions measuring any of them, by place
    driven: Vec<usize>,
    /// the drawn curves by their place among the entities, the place of each among them by id, the ones that may
    /// move (places among the drawn), and the boxes of all
    drawn: Vec<usize>,
    drawn_at: HashMap<Id, usize>,
    moving: Vec<usize>,
    boxes: Boxes,
    /// the loops of the sketch: the loops each curve is in, the curves each loop is made of, the box of each
    loops_of: HashMap<Id, Vec<Id>>,
    made_of: HashMap<Id, Vec<Id>>,
    loop_box: HashMap<Id, [f64; 4]>,
    loop_grid: LoopGrid,
}

/// A STAMP OF THE STRUCTURE OF A SKETCH: how many points, entities, constraints, figures, splines and texts it has,
/// the ends of its entities, the kind of each constraint, whether it is a reference and the value of its dimension -
/// what a frame of a drag leaves as it is. Each word turned and multiplied in, nothing laid out: laid out as the lists
/// of the points of every entity and constraint, it was 20 ms of a frame on 70 000 rectangles. Where the points stand
/// is told by `still_stamp`; a constraint moved onto other points with no number changing is not told - nothing does
/// that in the middle of a drag, and the release rebuilds the sketch whole.
pub(crate) fn structure_stamp(s: &Sketch) -> u64 {
    let mut h: u64 = 0;
    let mut mix = |x: u64| h = (h.rotate_left(5) ^ x).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    for n in [s.points.len(), s.entities.len(), s.constraints.len(), s.figures.len(), s.texts.len()] {
        mix(n as u64);
    }
    for e in &s.entities {
        mix(e.id ^ u64::from(e.construction) << 63);
        entity_points(e).into_iter().for_each(&mut mix);
    }
    use std::hash::{Hash, Hasher};
    let mut kinds = std::collections::hash_map::DefaultHasher::new();
    for c in &s.constraints {
        std::mem::discriminant(c).hash(&mut kinds);
        mix(u64::from(c.is_driven()));
        mix(c.dim_value().map_or(0, f64::to_bits));
    }
    mix(kinds.finish());
    h
}

/// A STAMP OF WHERE THE POINTS NO FRAME MOVES STAND - all but `watched`: what the session keeps of them (their boxes)
/// holds only while they stand there.
fn still_stamp(s: &Sketch, watched: &[usize]) -> u64 {
    let mut skip = vec![false; s.points.len()];
    for &i in watched {
        skip[i] = true;
    }
    let mut h: u64 = 0;
    for (p, _) in s.points.iter().zip(&skip).filter(|(_, skip)| !**skip) {
        for x in [p.x.to_bits(), p.y.to_bits()] {
            h = (h.rotate_left(5) ^ x).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
        }
    }
    h
}

/// The box of a loop over its points.
fn loop_box(c: &Contour) -> [f64; 4] {
    c.points.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, p| [b[0].min(p.x), b[1].min(p.y), b[2].max(p.x), b[3].max(p.y)])
}

/// The curves a loop is made of: its provenance and the source of each of its edges.
fn loop_curves(project: &Project, cid: Id) -> Vec<Id> {
    let mut of: Vec<Id> = project.contours.ents_of(cid).cloned().unwrap_or_default();
    if let Some(ci) = project.contour_index(cid) {
        of.extend(project.contours[ci].edge_src.iter().copied());
    }
    of.sort_unstable();
    of.dedup();
    of
}

impl Project {
    /// A FRAME OF A DRAG through the session of the drag: the part of the dragged point solved within the budget of a
    /// frame, the reference dimensions measuring it read again, the loops of what moved made again. `None` where the
    /// sketch takes no session (splines, text, a frame of reference to make first, the origin drawn on) - the frame is
    /// then made as before (`solve_sketch_inner`).
    pub(super) fn drag_frame(&mut self, si: usize, drag: (Id, f64, f64)) -> Option<f64> {
        let (d, tx, ty) = drag;
        let stamp = structure_stamp(self.sketches.get(si)?);
        let mut sess = match self.sketches[si].drag_session.take() {
            Some(sess) if sess.stamp == stamp && sess.dragged == d && sess.still == still_stamp(&self.sketches[si], &sess.watched) => sess,
            _ => Box::new(self.drag_session(si, d, stamp)?),
        };
        let s = &mut self.sketches[si];
        let at = |s: &Sketch, i: usize| (s.points[i].x, s.points[i].y);
        let at_start: Vec<(f64, f64)> = sess.watched.iter().map(|&i| at(s, i)).collect();
        // the centres of the rectangles drawn by their corners on the middles of their diagonals (`solve_sketch_inner`)
        // the rectangles the frame touches read once, and only where it touches one
        let rects = if sess.rects.is_empty() { Vec::new() } else { s.rects() };
        for r in sess.rects.iter().filter(|r| r.mid) {
            let rect = &rects[r.index];
            let (Some(&a), Some(&c), Some(&m)) = (sess.place.get(&rect.corners[0]), sess.place.get(&rect.corners[2]), sess.place.get(&rect.centre)) else { continue };
            (s.points[m].x, s.points[m].y) = ((s.points[a].x + s.points[c].x) / 2.0, (s.points[a].y + s.points[c].y) / 2.0);
        }
        // a shape dragged by its centre goes with it as a whole (`solve_sketch_inner`)
        if !sess.carried.is_empty() {
            if let Some(&di) = sess.place.get(&d) {
                let (dx, dy) = (tx - s.points[di].x, ty - s.points[di].y);
                for id in sess.carried.iter().chain(std::iter::once(&d)).filter(|id| !sess.held.contains(id)) {
                    if let Some(&i) = sess.place.get(id) {
                        s.points[i].x += dx;
                        s.points[i].y += dy;
                    }
                }
            }
        }
        let mut own_points: Vec<SketchPoint> = sess.points.iter().map(|&i| s.points[i]).collect();
        let was: Vec<(f64, f64)> = own_points.iter().map(|p| (p.x, p.y)).collect();
        let radius = |s: &Sketch, r: &PartRadius| match r.from {
            RadiusFrom::Circle(e) => match s.entities[e].kind.as_circle() {
                Some(crate::model::CircleParts { r, .. }) => r,
                _ => 0.001,
            },
            RadiusFrom::Arc { centre, end } => (s.points[end].x - s.points[centre].x).hypot(s.points[end].y - s.points[centre].y).max(0.001),
        };
        let mut own_radii: Vec<crate::solver::RadiusVar> = sess.radii.iter().map(|r| crate::solver::RadiusVar { center: r.centre, value: radius(s, r) }).collect();
        let radii_was: Vec<f64> = own_radii.iter().map(|r| r.value).collect();
        // a rectangle changes its size from where it was drawn (`held_for_size`)
        let anchors = sess.rects.iter().filter_map(|r| held_for_size(&rects[r.index], Some(d), r.centre)).filter(|p| sess.place.contains_key(p)).map(|p| Constraint::Fixed { p });
        let mut constraints: Vec<Constraint> = sess.constraints.iter().cloned().chain(anchors).collect();
        // WHERE CURVES MADE TO TOUCH TOUCH NOW, read before each frame as a full solve reads it (`touchings_follow`): kept
        // from the first frame, a circle made to touch an ellipse came away 10 deg off
        for c in constraints.iter_mut() {
            if let Constraint::TangentCurves { one, other, near, end: None } = c {
                *near = crate::solver::touching_places(&own_points, &own_radii, [&*one, &*other], *near).or(*near);
            }
        }
        let outcome = crate::solver::solve_part_within(&mut own_points, &mut own_radii, &constraints, Some(drag), crate::solver::Budget::FRAME, &s.points);
        s.left_unsolved = outcome.left;
        // a move below rounding is not written (`solve_sketch_inner`)
        let same = |new: f64, old: f64| (new - old).abs() <= 1e-12 * old.abs().max(1.0);
        for ((&i, q), &(x, y)) in sess.points.iter().zip(&own_points).zip(&was) {
            s.points[i] = if same(q.x, x) && same(q.y, y) { SketchPoint { x, y, ..*q } } else { *q };
        }
        let mut resized: Vec<Id> = Vec::new();
        for ((r, solved), &old) in sess.radii.iter().zip(&own_radii).zip(&radii_was) {
            let value = if same(solved.value, old) { old } else { solved.value };
            if let RadiusFrom::Circle(e) = r.from {
                let kind = &mut s.entities[e].kind;
                if let Some(was) = kind.as_circle().map(|c| c.r) {
                    if was.to_bits() != value.to_bits() {
                        resized.push(r.centre);
                    }
                    kind.set_radius(value);
                }
            }
        }
        let moved: HashSet<Id> = sess
            .watched
            .iter()
            .zip(&at_start)
            .filter(|(&i, &(x, y))| s.points[i].x.to_bits() != x.to_bits() || s.points[i].y.to_bits() != y.to_bits())
            .map(|(&i, _)| s.points[i].id)
            .chain(resized)
            .collect();
        // the reference dimensions measuring what moved
        if !sess.driven.is_empty() {
            let mut pos: HashMap<Id, (f64, f64)> = HashMap::new();
            let mut crad: HashMap<Id, f64> = HashMap::new();
            for &ci in &sess.driven {
                for p in s.constraints[ci].points() {
                    if let Some(&i) = sess.place.get(&p) {
                        pos.insert(p, (s.points[i].x, s.points[i].y));
                    }
                }
            }
            // a circle's radius, and an arc's from its first end (`update_driven_dims`)
            let at = |id: Id| sess.place.get(&id).map(|&i| Point2::new(s.points[i].x, s.points[i].y));
            for e in &s.entities {
                if let Some(c) = e.kind.circle_by(&at).filter(|c| pos.contains_key(&c.center)) {
                    crad.insert(c.center, c.r);
                }
            }
            for &ci in &sess.driven {
                measure_driven(&mut s.constraints[ci], &pos, &crad);
            }
        }
        self.frame_loops(si, &mut sess, &moved);
        self.sketches[si].drag_session = Some(sess);
        Some(outcome.residual)
    }

    /// The session of a drag of point `d` in sketch `si` (see the module), `None` where the sketch takes none.
    pub(crate) fn drag_session(&self, si: usize, d: Id, stamp: u64) -> Option<DragSession> {
        let s = self.sketches.get(si)?;
        let origin_drawn = s.origin != 0 && s.entities.iter().any(|e| entity_points(e).contains(&s.origin));
        let frame_missing = (s.origin != 0 || s.axis_pts.iter().any(|g| *g != 0)) && s.frame == 0;
        // a sketch with an offset laid again from its source has its copies laid anew in a frame, and the points the
        // session keeps would not be theirs; text is made whole
        if !s.offsets.is_empty() || !s.texts.is_empty() || origin_drawn || frame_missing {
            return None;
        }
        let place: HashMap<Id, usize> = s.points.iter().enumerate().map(|(i, p)| (p.id, i)).collect();
        // the radii as `entity_radii` lays them out, each with where it comes from
        let mut radii = Vec::new();
        let mut from = Vec::new();
        for (k, e) in s.entities.iter().enumerate() {
            match e.kind.radius_source() {
                Some(RadiusSource::Held(circle)) => {
                    radii.push(crate::solver::RadiusVar { center: circle.center, value: circle.r });
                    from.push(PartRadius { centre: circle.center, from: RadiusFrom::Circle(k) });
                }
                Some(RadiusSource::ToEnd { center, end: a }) => {
                    if let (Some(&c), Some(&end)) = (place.get(&center), place.get(&a)) {
                        let value = (s.points[end].x - s.points[c].x).hypot(s.points[end].y - s.points[c].y).max(0.001);
                        radii.push(crate::solver::RadiusVar { center, value });
                        from.push(PartRadius { centre: center, from: RadiusFrom::Arc { centre: c, end } });
                    }
                }
                None => {}
            }
        }
        let mut active: Vec<Constraint> = s.constraints.iter().filter(|c| s.solves(c)).map(|c| s.as_solved(c)).collect();
        active.extend(self.entity_intrinsics(si));
        let part = crate::solver::part_holding(&s.points, &radii, &active, d)?;
        let carried = s.carried_with(d);
        let mut touched: HashSet<Id> = part.points.iter().map(|&i| s.points[i].id).chain(carried.iter().copied()).collect();
        touched.insert(d);
        // how many constraints name each centre of a rectangle drawn by its corners
        let mut naming: HashMap<Id, usize> = s.rects().iter().map(|r| (r.centre, 0)).collect();
        for c in &s.constraints {
            for p in c.points() {
                if let Some(n) = naming.get_mut(&p) {
                    *n += 1;
                }
            }
        }
        // THE POINTS OF THE PART THAT CAN MOVE, counted once for the drag: a centre no motion of the part moves is held
        let free: HashMap<Id, bool> = {
            let own_points: Vec<SketchPoint> = part.points.iter().map(|&i| s.points[i]).collect();
            let own_radii: Vec<crate::solver::RadiusVar> = part.radii.iter().map(|&j| radii[j]).collect();
            let own: Vec<Constraint> = part.constraints.iter().map(|&ci| active[ci].clone()).collect();
            own_points.iter().map(|q| q.id).zip(crate::solver::free_points(&own_points, &own_radii, &own)).collect()
        };
        let all_rects = s.rects();
        let rects: Vec<TouchedRect> = all_rects
            .iter()
            .enumerate()
            .filter(|(_, r)| touched.contains(&r.centre) || r.corners.iter().any(|k| touched.contains(k)))
            .map(|(index, r)| TouchedRect {
                index,
                mid: matches!(r.anchor, RectAnchor::Corner(_)) && naming.get(&r.centre) == Some(&1),
                centre: if free.get(&r.centre) == Some(&false) { super::sketch::Centre::Held } else { super::sketch::Centre::Free },
            })
            .collect();
        let watched: Vec<usize> =
            touched.iter().chain(rects.iter().map(|r| &all_rects[r.index].centre)).filter_map(|id| place.get(id).copied()).collect::<std::collections::BTreeSet<usize>>().into_iter().collect();
        let watched_ids: HashSet<Id> = watched.iter().map(|&i| s.points[i].id).collect();
        let driven: Vec<usize> = s.constraints.iter().enumerate().filter(|(_, c)| c.is_driven() && c.points().iter().any(|p| watched_ids.contains(p))).map(|(i, _)| i).collect();
        let drawn: Vec<usize> = s.entities.iter().enumerate().filter(|(_, e)| !e.construction).map(|(k, _)| k).collect();
        let drawn_at: HashMap<Id, usize> = drawn.iter().enumerate().map(|(k, &e)| (s.entities[e].id, k)).collect();
        let moving: Vec<usize> = drawn.iter().enumerate().filter(|(_, &e)| entity_points(&s.entities[e]).iter().any(|p| watched_ids.contains(p))).map(|(k, _)| k).collect();
        let pos = |id: Id| place.get(&id).map(|&i| (s.points[i].x, s.points[i].y));
        let all: Vec<[f64; 4]> = drawn.iter().map(|&e| entity_box(&s.entities[e], &pos)).collect();
        let cell = (all.iter().filter(|b| b.iter().all(|v| v.is_finite())).map(|b| (b[2] - b[0]).max(b[3] - b[1])).sum::<f64>() / all.len().max(1) as f64).max(1e-3);
        let mut boxes = Boxes { cell, boxes: all, cells: HashMap::new(), wide: Vec::new() };
        let moving_set: HashSet<usize> = moving.iter().copied().collect();
        for k in (0..drawn.len()).filter(|k| !moving_set.contains(k)) {
            boxes.put(k);
        }
        let mut loops_of: HashMap<Id, Vec<Id>> = HashMap::new();
        let mut made_of: HashMap<Id, Vec<Id>> = HashMap::new();
        let mut loop_boxes: HashMap<Id, [f64; 4]> = HashMap::new();
        for &cid in &s.contour_ids {
            let curves = loop_curves(self, cid);
            for e in &curves {
                loops_of.entry(*e).or_default().push(cid);
            }
            made_of.insert(cid, curves);
            if let Some(ci) = self.contour_index(cid) {
                loop_boxes.insert(cid, loop_box(&self.contours[ci]));
            }
        }
        // a cell as wide as a loop is on the mean: by the curves, a grid of lines 4 000 mm long put its 40 000 cells of
        // 20 mm on one cell, and a frame looked through all of them for each loop it made again - 20 ms of 31
        let finite: Vec<&[f64; 4]> = loop_boxes.values().filter(|b| b.iter().all(|v| v.is_finite())).collect();
        let loop_cell = (finite.iter().map(|b| (b[2] - b[0]).max(b[3] - b[1])).sum::<f64>() / finite.len().max(1) as f64).max(1e-3);
        let mut loop_grid = LoopGrid { cell: loop_cell, ..Default::default() };
        for (cid, b) in &loop_boxes {
            loop_grid.put(*cid, b);
        }
        let still = still_stamp(s, &watched);
        Some(DragSession {
            stamp,
            still,
            dragged: d,
            points: part.points,
            radii: part.radii.iter().map(|&j| from[j]).collect(),
            constraints: part.constraints.iter().map(|&ci| active[ci].clone()).collect(),
            place,
            carried,
            held: s.held_points(),
            rects,
            watched,
            driven,
            drawn,
            drawn_at,
            moving,
            boxes,
            loops_of,
            made_of,
            loop_box: loop_boxes,
            loop_grid,
        })
    }

    /// THE LOOPS OF A FRAME through the session: the regions and the open chains round what moved made again
    /// (`round`), the regions that held it before or met where it swept giving way to them.
    pub(crate) fn frame_loops(&mut self, si: usize, sess: &mut DragSession, moved: &HashSet<Id>) {
        let s = &self.sketches[si];
        let pos = |id: Id| sess.place.get(&id).map(|&i| (s.points[i].x, s.points[i].y));
        // the curves that moved, by place among the drawn, and their boxes where they stood and where they stand
        let mut shifted: Vec<usize> = Vec::new();
        let mut swept: Vec<[f64; 4]> = Vec::new();
        for &k in &sess.moving {
            let was = sess.boxes.boxes[k];
            sess.boxes.boxes[k] = entity_box(&s.entities[sess.drawn[k]], &pos);
            if entity_points(&s.entities[sess.drawn[k]]).iter().any(|p| moved.contains(p)) {
                shifted.push(k);
                swept.extend([was, sess.boxes.boxes[k]]);
            }
        }
        if shifted.is_empty() {
            return; // nothing drawn moved: every loop stands
        }
        let shifted_ids: HashSet<Id> = shifted.iter().map(|&k| s.entities[sess.drawn[k]].id).collect();
        // the loops before the frame that held a moved curve, and the regions that met where it swept: a chain changes
        // only with what is joined to it
        let closed = |cid: &Id| self.contour_index(*cid).is_some_and(|ci| self.contours[ci].closed);
        let mut old: HashSet<Id> = shifted.iter().flat_map(|&k| sess.loops_of.get(&s.entities[sess.drawn[k]].id).into_iter().flatten().copied()).collect();
        for b in &swept {
            match sess.loop_grid.near(b) {
                Some(maybe) => old.extend(maybe.into_iter().filter(|cid| closed(cid) && sess.loop_box.get(cid).is_some_and(|l| meet(l, b)))),
                None => old.extend(sess.loop_box.iter().filter(|(cid, l)| closed(cid) && meet(l, b)).map(|(cid, _)| *cid)),
            }
        }
        let held = old.iter().filter(|cid| closed(cid)).filter_map(|cid| sess.loop_box.get(cid)).copied().reduce(round::union);
        // the chains are made from the moved curves and every curve of a chain one of them was in
        let mut from: Vec<usize> = shifted.clone();
        for cid in old.iter().filter(|cid| !closed(cid)) {
            from.extend(sess.made_of.get(cid).into_iter().flatten().filter_map(|e| sess.drawn_at.get(e).copied()));
        }
        let curve = |k: usize| s.entities[sess.drawn[k]].clone();
        let at = |id: Id| sess.place.get(&id).map(|&i| s.points[i]);
        let near = |b: &[f64; 4]| sess.boxes.near(b).into_iter().chain(sess.moving.iter().copied()).collect();
        let change = round::Change { curve: &curve, boxes: &sess.boxes.boxes, at: &at, near: &near, moved: &shifted, swept: &swept, held };
        let mut pairs = round::regions_round(&change);
        let chains = round::chains_round(&change, &from);
        pairs.extend(chains.made);
        old.extend(chains.through.iter().flat_map(|e| sess.loops_of.get(e).into_iter().flatten().copied()).filter(|cid| !closed(cid)));
        // the loops whose nesting may change: those whose box meets the box of the loops going and coming
        let over = old
            .iter()
            .filter_map(|cid| sess.loop_box.get(cid))
            .copied()
            .chain(pairs.iter().map(|(c, _)| loop_box(c)))
            .fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |u, b| [u[0].min(b[0]), u[1].min(b[1]), u[2].max(b[2]), u[3].max(b[3])]);
        let kept: Vec<Id> = s.contour_ids.iter().copied().filter(|cid| !old.contains(cid)).collect();
        let near: Vec<Id> = match sess.loop_grid.near(&over) {
            Some(maybe) => kept.iter().copied().filter(|cid| maybe.contains(cid) && sess.loop_box.get(cid).is_some_and(|b| meet(b, &over))).collect(),
            None => kept.iter().copied().filter(|cid| sess.loop_box.get(cid).is_some_and(|b| meet(b, &over))).collect(),
        };
        let old_in_order: Vec<Id> = s.contour_ids.iter().copied().filter(|cid| old.contains(cid)).collect();
        let old_in_order_ids = old_in_order.clone();
        let made = self.replace_contours(old_in_order, pairs, &near);
        self.sketches[si].contour_ids = kept.into_iter().chain(made.iter().copied()).collect();
        self.laid_framed(si, &old_in_order_ids, &made, &shifted_ids);
        // the session follows the loops
        for cid in &old {
            for e in sess.made_of.remove(cid).into_iter().flatten() {
                if let Some(list) = sess.loops_of.get_mut(&e) {
                    list.retain(|c| c != cid);
                }
            }
            if let Some(b) = sess.loop_box.remove(cid) {
                sess.loop_grid.take(*cid, &b);
            }
        }
        for &cid in &made {
            let curves = loop_curves(self, cid);
            for e in &curves {
                sess.loops_of.entry(*e).or_default().push(cid);
            }
            sess.made_of.insert(cid, curves);
            if let Some(ci) = self.contour_index(cid) {
                let b = loop_box(&self.contours[ci]);
                sess.loop_grid.put(cid, &b);
                sess.loop_box.insert(cid, b);
            }
        }
    }
}
