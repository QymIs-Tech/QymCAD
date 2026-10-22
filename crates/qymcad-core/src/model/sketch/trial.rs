//! A VALUE OF A DIMENSION TRIED ON ITS SKETCH, figure by figure as it is typed: the part of the sketch the dimension
//! holds is found once, with how it stood, and every figure takes back that part alone and solves it alone. Taken back
//! from a copy of the whole document and solved whole, every figure typed among 10 000 lines cost 79-104 ms in a test
//! build.

use super::*;

/// A constraint of the sketch in the part, by its place among the constraints, as it stood.
#[derive(Clone)]
struct ConstraintWas {
    index: usize,
    was: Constraint,
}

/// A circle of the part, by its place among the entities, and the radius it stood at.
#[derive(Clone)]
struct CircleWas {
    entity: usize,
    r: f64,
}

/// The centre of a rectangle drawn by its corners that nothing but its corners holds, by places among the points: put on
/// the middle of the diagonal before the solve, as a solve of the whole sketch puts it.
#[derive(Clone)]
struct MidOf {
    centre: usize,
    a: usize,
    c: usize,
}

/// A DIMENSION OF A SKETCH: the sketch, and the place of the dimension among its constraints.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DimOf {
    pub si: usize,
    pub ci: usize,
}

/// A VALUE TRIED ON A SKETCH: the dimension, the part of the sketch it holds and how that part stood when the trial began.
#[derive(Clone)]
pub struct ValueTrial {
    si: usize,
    /// the constraints of the sketch in the part as they stood: the values tried and what else a box sets
    constraints_was: Vec<ConstraintWas>,
    /// the places of the points of the part, and the points as they stood
    points: Vec<usize>,
    points_was: Vec<SketchPoint>,
    /// the radius unknowns of the part as they stood, and the circles that store them
    radii: Vec<crate::solver::RadiusVar>,
    circles: Vec<CircleWas>,
    /// the constraints of the part as solved, each with its place among the constraints of the sketch - none for what a
    /// curve holds of itself
    constraints: Vec<Constraint>,
    origin: Vec<Option<usize>>,
    /// what holds each rectangle of the part where it was drawn from (`held_for_size`)
    anchors: Vec<Constraint>,
    mids: Vec<MidOf>,
    /// the reference dimensions measuring a point of the part
    driven: Vec<usize>,
    /// the loops of the sketch and the boxes of its curves, made once, so each figure makes again only the loops round
    /// what it moved, as a frame of a drag does (`frame_loops`); none for a sketch a drag takes no session of
    loops: Option<Box<crate::model::drag::DragSession>>,
    /// how many constraints the sketch held when the trial began: what a box lays after it - the dimensions of a shape
    /// sized by its box - is solved with the part
    laid_from: usize,
}

impl Project {
    /// THE TRIAL OF A VALUE OF DIMENSION `of`: the part of the sketch it holds, as it stands now. None for a dimension the
    /// sketch does not solve with - a reference - or that names no point.
    pub fn value_trial(&self, of: DimOf) -> Option<ValueTrial> {
        let s = self.sketches.get(of.si)?;
        let dim = s.constraints.get(of.ci)?;
        if !s.solves(dim) {
            return None;
        }
        self.trial_round(of.si, *dim.points().first()?)
    }

    /// THE TRIAL OF THE VALUES OF A SHAPE AROUND POINT `seed` OF SKETCH `si`: the part of the sketch the point stands in,
    /// as it stands now - for a box that lays the dimensions of its shape as the first figure is typed, which are solved
    /// with the part (`solve_trial`). None for a point not in the sketch.
    pub fn trial_round(&self, si: usize, seed: Id) -> Option<ValueTrial> {
        let radii = self.entity_radii(si);
        let intrinsics = self.entity_intrinsics(si);
        let s = self.sketches.get(si)?;
        // the constraints as the solve takes them, each with the place of its constraint in the sketch
        let mut active: Vec<Constraint> = Vec::new();
        let mut origin: Vec<Option<usize>> = Vec::new();
        for (i, c) in s.constraints.iter().enumerate().filter(|(_, c)| s.solves(c)) {
            active.push(s.as_solved(c));
            origin.push(Some(i));
        }
        origin.extend(intrinsics.iter().map(|_| None));
        active.extend(intrinsics);
        let part = crate::solver::part_holding(&s.points, &radii, &active, seed)?;
        let ids: std::collections::HashSet<Id> = part.points.iter().map(|&i| s.points[i].id).collect();
        let place: std::collections::HashMap<Id, usize> = part.points.iter().map(|&i| (s.points[i].id, i)).collect();
        let part_radii: Vec<crate::solver::RadiusVar> = part.radii.iter().map(|&j| radii[j]).collect();
        let centres: std::collections::HashSet<Id> = part_radii.iter().map(|r| r.center).collect();
        let circles = s.entities.iter().enumerate().filter_map(|(entity, e)| e.kind.as_circle().filter(|k| centres.contains(&k.center)).map(|k| CircleWas { entity, r: k.r })).collect();
        // the rectangles of the part: held where they were drawn from, their centres put on the middle
        let rects: Vec<crate::model::SketchRect> = s.rects().into_iter().filter(|r| ids.contains(&r.centre) || r.corners.iter().any(|k| ids.contains(k))).collect();
        let anchors = rects.iter().filter_map(|r| held_for_size(r, None, Centre::Free)).filter(|p| ids.contains(p)).map(|p| Constraint::Fixed { p }).collect();
        let naming = |centre: Id| s.constraints.iter().filter(|c| c.points().contains(&centre)).count();
        let mids = rects
            .iter()
            .filter(|r| matches!(r.anchor, crate::model::RectAnchor::Corner(_)) && naming(r.centre) == 1)
            .filter_map(|r| Some(MidOf { centre: *place.get(&r.centre)?, a: *place.get(&r.corners[0])?, c: *place.get(&r.corners[2])? }))
            .collect();
        let driven = s.constraints.iter().enumerate().filter(|(_, c)| c.is_driven() && c.points().iter().any(|p| ids.contains(p))).map(|(i, _)| i).collect();
        let loops = self.drag_session(si, seed, crate::model::drag::structure_stamp(s)).map(Box::new);
        let own_origin: Vec<Option<usize>> = part.constraints.iter().map(|&k| origin[k]).collect();
        Some(ValueTrial {
            si,
            constraints_was: own_origin.iter().flatten().map(|&index| ConstraintWas { index, was: s.constraints[index].clone() }).collect(),
            points_was: part.points.iter().map(|&i| s.points[i]).collect(),
            points: part.points.clone(),
            radii: part_radii,
            circles,
            constraints: part.constraints.iter().map(|&k| active[k].clone()).collect(),
            origin: own_origin,
            anchors,
            mids,
            driven,
            loops,
            laid_from: s.constraints.len(),
        })
    }

    /// THE PART OF THE TRIAL PUT BACK as it stood when the trial began - its points, its circles and the dimension - with
    /// its loops left as they are: the solve that follows makes them again (`solve_trial`).
    pub fn trial_restore(&mut self, t: &ValueTrial) {
        let Some(s) = self.sketches.get_mut(t.si) else { return };
        for (&i, was) in t.points.iter().zip(&t.points_was) {
            if let Some(p) = s.points.get_mut(i) {
                *p = *was;
            }
        }
        for c in &t.circles {
            if let Some(e) = s.entities.get_mut(c.entity) {
                e.kind.set_radius(c.r);
            }
        }
        for c in &t.constraints_was {
            if let Some(k) = s.constraints.get_mut(c.index) {
                *k = c.was.clone();
            }
        }
    }

    /// THE PART OF THE TRIAL TAKEN BACK as it stood when the trial began, and the loops round it made again.
    pub fn trial_back(&mut self, t: &mut ValueTrial) {
        self.trial_restore(t);
        self.trial_loops(t, std::collections::HashSet::new());
    }

    /// The loops round the part of the trial made again - every point of the part, and `more` moved besides.
    fn trial_loops(&mut self, t: &mut ValueTrial, more: std::collections::HashSet<Id>) {
        let s = &self.sketches[t.si];
        let moved: std::collections::HashSet<Id> =
            t.points.iter().filter_map(|&i| s.points.get(i)).map(|p| p.id).chain(t.circles.iter().filter_map(|c| s.entities.get(c.entity)?.kind.as_circle().map(|k| k.center))).chain(more).collect();
        match t.loops.as_deref_mut() {
            Some(sess) => self.frame_loops(t.si, sess, &moved),
            None => self.regen_sketch_moved(t.si, &moved),
        }
    }

    /// THE PART OF THE TRIAL SOLVED ALONE with the dimension as it stands now - set by the caller over the part taken back
    /// (`trial_back`) - as a solve of the whole sketch solves that part: the rectangles held where they were drawn from,
    /// let go when the part does not solve with them. Only what it moved is rebuilt. Answers the residual of the part.
    pub fn solve_trial(&mut self, t: &mut ValueTrial) -> f64 {
        let Some(s) = self.sketches.get_mut(t.si) else { return 0.0 };
        for m in &t.mids {
            let (a, c) = (s.points[m.a], s.points[m.c]);
            (s.points[m.centre].x, s.points[m.centre].y) = ((a.x + c.x) / 2.0, (a.y + c.y) / 2.0);
        }
        let mut own: Vec<SketchPoint> = t.points.iter().map(|&i| s.points[i]).collect();
        let at_start = own.clone();
        let mut radii = t.radii.clone();
        // the constraints of the part as they stand now: what the box set over the part taken back
        let mut constraints = t.constraints.clone();
        for (c, o) in constraints.iter_mut().zip(&t.origin) {
            if let Some(now) = o.and_then(|i| s.constraints.get(i)) {
                *c = s.as_solved(now);
            }
        }
        // and what a box laid on the part since the trial began
        let ids: std::collections::HashSet<Id> = own.iter().map(|p| p.id).collect();
        constraints.extend(s.constraints.iter().skip(t.laid_from).filter(|c| s.solves(c) && c.points().iter().all(|p| ids.contains(p))).map(|c| s.as_solved(c)));
        // where curves made to touch touch now, read before the solve as a solve of the whole sketch reads it
        for c in constraints.iter_mut() {
            if let Constraint::TangentCurves { one, other, near, end: None } = c {
                *near = crate::solver::touching_places(&own, &radii, [&*one, &*other], *near).or(*near);
            }
        }
        let budget = crate::solver::Budget::FULL;
        let residual = if t.anchors.is_empty() {
            crate::solver::solve_part_within(&mut own, &mut radii, &constraints, None, budget, &s.points).residual
        } else {
            let (mut held, mut held_radii) = (own.clone(), radii.clone());
            let with: Vec<Constraint> = constraints.iter().cloned().chain(t.anchors.iter().cloned()).collect();
            crate::solver::solve_part_within(&mut held, &mut held_radii, &with, None, budget, &s.points);
            let r = crate::solver::residual_per_constraint(&held, &held_radii, &constraints).iter().map(|v| v * v).sum::<f64>().sqrt();
            if r <= 1e-9 {
                (own, radii) = (held, held_radii);
                r
            } else {
                crate::solver::solve_part_within(&mut own, &mut radii, &constraints, None, budget, &s.points).residual
            }
        };
        // a move below rounding is not written (`solve_sketch_inner`)
        let same = |now_was: [f64; 2]| (now_was[0] - now_was[1]).abs() <= 1e-12 * now_was[1].abs().max(1.0);
        for ((&i, q), was) in t.points.iter().zip(&own).zip(&at_start) {
            s.points[i] = if same([q.x, was.x]) && same([q.y, was.y]) { *was } else { *q };
        }
        for c in &t.circles {
            let centre = s.entities[c.entity].kind.as_circle().map(|k| k.center);
            if let Some(v) = radii.iter().find(|r| Some(r.center) == centre).map(|r| r.value) {
                s.entities[c.entity].kind.set_radius(v);
            }
        }
        // the reference dimensions on the part measure what it moved
        let ends: std::collections::HashSet<Id> = t.driven.iter().filter_map(|&i| s.constraints.get(i)).flat_map(|c| c.points()).collect();
        let pos: std::collections::HashMap<Id, (f64, f64)> = s.points.iter().filter(|p| ends.contains(&p.id)).map(|p| (p.id, (p.x, p.y))).collect();
        // a circle's radius, and an arc's from its first end, as `update_driven_dims` reads them: the points of the part
        let near: std::collections::HashMap<Id, Point2> = t.points.iter().map(|&i| (s.points[i].id, Point2::new(s.points[i].x, s.points[i].y))).collect();
        let crad: std::collections::HashMap<Id, f64> =
            s.entities.iter().filter_map(|e| e.kind.circle_by(&|id| near.get(&id).copied())).filter(|k| ends.contains(&k.center)).map(|k| (k.center, k.r)).collect();
        for &i in &t.driven {
            if let Some(c) = s.constraints.get_mut(i) {
                measure_driven(c, &pos, &crad);
            }
        }
        let followed = self.offsets_follow(t.si);
        if followed.relaid {
            self.regen_sketch(t.si);
            t.loops = None;
        } else {
            self.trial_loops(t, followed.moved.into_iter().collect());
        }
        residual
    }
}
