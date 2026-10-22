//! A FIGURE COPIED IS A FIGURE: Copy, Mirror and the patterns lay a rectangle, a polygon or a slot taken whole as one -
//! a record of its own on the copied curves, a rectangle with its centre and its diagonals, held by the constraints the
//! figure holds itself by. Copied as plain curves, a mirrored rectangle fell apart under a dragged corner and a copied
//! polygon had nothing holding its corners on its circle.
use super::*;

/// THE SHAPES A COPY TAKES WHOLE: the rectangles and the figures all of whose curves it takes.
#[derive(Clone, Debug, Default)]
pub(crate) struct Whole {
    rects: Vec<SketchRect>,
    figures: Vec<SketchFigure>,
}

impl Whole {
    /// What a copy of the whole shapes carries besides the curves taken: the diagonals and the centre of a rectangle.
    pub(crate) fn carried(&self) -> impl Iterator<Item = Id> + '_ {
        self.rects.iter().flat_map(|r| r.diagonals.into_iter().flatten().chain([r.centre]))
    }
}

/// HOW A COPY WAS LAID: the old point and curve ids to the new ones, the map of a direction (the linear part of the map
/// the points went through), and where the old points stand.
pub(crate) struct CopyMaps<'a> {
    pub(crate) points: &'a std::collections::HashMap<Id, Id>,
    pub(crate) curves: &'a std::collections::HashMap<Id, Id>,
    pub(crate) turn: &'a dyn Fn([f64; 2]) -> [f64; 2],
    pub(crate) was: &'a dyn Fn(Id) -> Option<Point2>,
}

impl CopyMaps<'_> {
    /// Whether the map turns the sheet over, as a mirror about a line does.
    fn flips(&self) -> bool {
        let ([xx, xy], [yx, yy]) = ((self.turn)([1.0, 0.0]), (self.turn)([0.0, 1.0]));
        xx * yy - xy * yx < 0.0
    }
}

/// A CONSTRAINT AS IT HOLDS THE COPY: on the new points, and a turn of a line taken through the map - a mirror or a
/// turn of the copy turns its sides, and a side held at 30 deg mirrored about the Y axis stands at 150 deg. A dimension
/// stands where the map takes it (`placed_through`); `None` for a dimension the copy cannot keep.
pub(crate) fn copied_constraint(c: &Constraint, maps: &CopyMaps) -> Option<Constraint> {
    let mut nc = remap_constraint_via(c, maps.points);
    nc.each_curve_mut(|on| on.rename_curve(&|id| maps.curves.get(&id).copied()));
    if let Constraint::Orientation { deg, .. } = &mut nc {
        let (s, k) = deg.to_radians().sin_cos();
        let [x, y] = (maps.turn)([k, s]);
        *deg = y.atan2(x).to_degrees();
    }
    placed_through(nc, c, maps)
}

/// A DIMENSION STANDING WHERE THE MAP TAKES IT: laid outside the source, it stands outside the copy. The side a
/// dimension is laid on is read on the screen, its Y down: an aligned one across the way from its first point to its
/// second, which a mirror turns over; a level one up or down and an upright one left or right, through the map - a
/// turn of a quarter makes a level one upright, a turn of any other angle an aligned one where its points stood level
/// (or upright), and leaves out one that measured across a slant; a radius or a diameter by the angle of its leader.
/// Reported behaviour: the width and height of a mirrored rectangle stood inside it where those of the source stood
/// outside.
fn placed_through(mut nc: Constraint, was: &Constraint, maps: &CopyMaps) -> Option<Constraint> {
    let m = |v: [f64; 2]| (maps.turn)(v);
    let flip = if maps.flips() { -1.0 } else { 1.0 };
    match &mut nc {
        Constraint::Distance { off, axis, .. } if *axis == 1 || *axis == 2 => {
            // the side as a vector of the sheet, through the map
            let v = if *axis == 1 { [0.0, -*off] } else { [*off, 0.0] };
            let [vx, vy] = m(v);
            let [xx, xy] = m([1.0, 0.0]);
            if xy.abs() < 1e-9 {
                *off = if *axis == 1 { -vy } else { vx };
            } else if xx.abs() < 1e-9 {
                *axis = 3 - *axis;
                *off = if *axis == 1 { -vy } else { vx };
            } else {
                let Constraint::Distance { a, b, .. } = *was else { return None };
                let (qa, qb) = ((maps.was)(a)?, (maps.was)(b)?);
                let (dx, dy) = (qb.x - qa.x, qb.y - qa.y);
                let len = dx.hypot(dy).max(1e-12);
                let along = if *axis == 1 { dy.abs() } else { dx.abs() };
                if along > 1e-9 * len {
                    return None;
                }
                let [ux, uy] = m([dx / len, dy / len]);
                *axis = 0;
                *off = vx * uy - vy * ux;
            }
        }
        Constraint::Distance { off, .. } | Constraint::EdgeDistance { off, .. } | Constraint::CurveDistance { off, .. } => *off *= flip,
        Constraint::Diameter { off, .. } => {
            let [x, y] = m([off.cos(), -off.sin()]);
            *off = (-y).atan2(x);
        }
        _ => {}
    }
    Some(nc)
}

impl Project {
    /// The rectangles and the figures of sketch `si` all of whose curves are among `eids`.
    pub(crate) fn wholes_among(&self, si: usize, eids: &[Id]) -> Whole {
        let Some(s) = self.sketches.get(si) else { return Whole { rects: Vec::new(), figures: Vec::new() } };
        Whole {
            rects: s.rects().iter().filter(|r| r.sides.iter().all(|e| eids.contains(e))).cloned().collect(),
            figures: s.shapes_not_rects().filter(|f| f.curves.iter().all(|e| eids.contains(e))).cloned().collect(),
        }
    }

    /// THE CONSTRAINTS THE COPIES OF THE SHAPES TAKEN WHOLE HOLD THEMSELVES BY, and the sizes of what is copied: a copy
    /// that carries no constraints of the set (a mirror, a pattern) still carries these, or it is no figure - and a figure
    /// or a curve of the size the source has. A dimension goes with the copy when every point it measures is copied: the
    /// radius of a polygon, the length and the width of a slot, the sides of a rectangle, the diameter of a circle, the
    /// radius of an arc, the length of a line, the half axis of an ellipse. Carried by the copy alone, a mirrored polygon
    /// kept its shape and lost its size, and a mirrored or patterned circle its diameter.
    pub(crate) fn copied_own_constraints(&self, si: usize, whole: &Whole, maps: &CopyMaps) -> Vec<Constraint> {
        let Some(s) = self.sketches.get(si) else { return Vec::new() };
        let rects = whole.rects.iter().flat_map(|r| s.constraints.iter().filter(move |c| r.holds(c)));
        let own = rects.chain(whole.figures.iter().flat_map(|f| f.own.iter()));
        // the points of each shape taken whole: its curves' and, for a rectangle, its centre and diagonals'
        let shapes: Vec<std::collections::HashSet<Id>> = whole
            .rects
            .iter()
            .map(|r| r.sides.iter().chain(r.diagonals.iter().flatten()).filter_map(|e| s.entity(*e)).flat_map(entity_points).chain([r.centre]).collect())
            .chain(whole.figures.iter().map(|f| f.curves.iter().filter_map(|e| s.entity(*e)).flat_map(entity_points).collect()))
            .collect();
        // a dimension every point of which is copied, of a figure or of a curve by itself: what has a size on the source
        // has it on the copy, what has none has none. An angle of three points is left out of a mirror, which turns its
        // sense.
        let flips = maps.flips();
        let sized = |c: &Constraint| {
            let ps = constraint_point_ids(c);
            c.dim_value().is_some()
                && !(flips && matches!(c, Constraint::Angle { .. }))
                && !ps.is_empty()
                && (shapes.iter().any(|shape| ps.iter().all(|p| shape.contains(p))) || ps.iter().all(|p| maps.points.contains_key(p)))
        };
        let sizes = s.constraints.iter().filter(|c| sized(c));
        own.chain(sizes).filter_map(|c| copied_constraint(c, maps)).collect()
    }

    /// LAY THE RECORDS OF THE COPIES of the shapes taken whole, on the copied curves and points.
    pub(crate) fn lay_copied_records(&mut self, si: usize, whole: &Whole, maps: &CopyMaps) {
        let p = |id: Id| maps.points.get(&id).copied().unwrap_or(id);
        let e = |id: Id| maps.curves.get(&id).copied().unwrap_or(id);
        for r in &whole.rects {
            let anchor = match r.anchor {
                RectAnchor::Corner(c) => RectAnchor::Corner(p(c)),
                RectAnchor::Centre => RectAnchor::Centre,
            };
            let id = self.alloc_id();
            let copy = SketchRect { id, corners: r.corners.map(p), sides: r.sides.map(e), centre: p(r.centre), diagonals: r.diagonals.map(|d| d.map(e)), anchor };
            self.sketches[si].add_rect(&copy);
        }
        for f in &whole.figures {
            let own: Vec<Constraint> = f.own.iter().filter_map(|c| copied_constraint(c, maps)).collect();
            let id = self.alloc_id();
            self.sketches[si].figures.push(SketchFigure { id, shape: f.shape, curves: f.curves.iter().map(|c| e(*c)).collect(), own });
        }
    }
}
