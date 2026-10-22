//! THE VALUES OF THE ITEMS NO DIMENSION HOLDS - the sides and the turn of a polygon, the axes and the turn of an
//! ellipse, the counts and the steps of a pattern of a sketch, the height of a text, the ratio of a relation of mates.
//! Each is kept as a node of the timeline keeps its numbers: a formula under the item's own id in `feat_dims`, named by
//! `DimTarget::Feature`, its number read by `kept_number`. A formula is imposed on the item as the parameters are
//! evaluated (`eval_parameters`); with none the item holds what it holds, and its name reads that - a reference.

use super::*;

/// The keys of the values of a pattern of a sketch.
const PATTERN_KEYS: [&str; 7] = ["count", "dx", "dy", "count2", "dx2", "dy2", "angle"];

/// A FORMULA OF A VALUE OF AN ITEM THAT CANNOT BE WORKED OUT: the formula, and why.
pub(crate) struct ItemFault {
    pub formula: String,
    pub error: crate::errors::ExprError,
}

/// What a value of an item is imposed with: the item, the key, and the number.
struct Imposed<'a> {
    id: Id,
    key: &'a str,
    value: f64,
}

impl Project {
    /// THE VALUE `key` OF ITEM `id` AS IT STANDS: of a relation of mates its ratio; of a pattern of a sketch its counts,
    /// steps and angle; of a text its height; of the centre of a polygon its sides and turn (degrees); of the centre of an
    /// ellipse its full axes and turn. None for an id that is none of these, or a key the item has not.
    pub(crate) fn item_value(&self, id: Id, key: &str) -> Option<f64> {
        if let Some(r) = self.relations.iter().find(|r| r.id == id) {
            return (key == "ratio").then_some(r.value);
        }
        // the limits of the free degrees of a mate: `min0`..`max2`, a limit switched off holds no value
        if let Some(j) = self.joints.iter().find(|j| j.id == id) {
            let slot: usize = key.get(3..)?.parse().ok().filter(|s| *s < 3)?;
            return match key.get(..3)? {
                "min" => j.limit_min[slot],
                "max" => j.limit_max[slot],
                _ => None,
            };
        }
        // the turn and the offset of the anchor of a mate
        if let Some(c) = self.connectors.iter().find(|c| c.id == id) {
            return match key {
                "turn" => Some(c.rot_deg),
                "x" | "y" | "z" => Some(c.offset_xyz[["x", "y", "z"].iter().position(|k| *k == key)?]),
                _ => None,
            };
        }
        for (si, s) in self.sketches.iter().enumerate() {
            if let Some(p) = s.patterns.iter().find(|p| p.id == id) {
                return pattern_value(&p.kind, key);
            }
            if let Some(t) = s.texts.iter().find(|t| t.id == id) {
                return (key == "height").then_some(t.height);
            }
            if s.points.iter().any(|q| q.id == id) {
                return self.polygon_value(si, id, key).or_else(|| self.ellipse_value(si, id, key));
            }
        }
        None
    }

    /// THE VALUE `key` OF THE DATUM NODE `kind` STANDS FOR: of a plane its offset, tilts and turn; of a point placed by
    /// hand its coordinates; of an axis placed by hand its origin and direction. They are read from the datum itself, as
    /// the rebuild resolves it, since the node holds no number of its own.
    pub(crate) fn datum_value(&self, kind: &crate::feature::FeatureKind, key: &str) -> Option<f64> {
        use crate::feature::FeatureKind;
        match *kind {
            FeatureKind::Plane { plane } => {
                let pl = self.planes.iter().find(|p| p.id == plane)?;
                match key {
                    "dist" => match pl.def {
                        PlaneDef::OffsetBase { dist, .. } | PlaneDef::OffsetFace { dist, .. } | PlaneDef::OffsetPlane { dist, .. } => Some(dist),
                        _ => None,
                    },
                    "tilt_x" => Some(pl.tilt.about_x),
                    "tilt_y" => Some(pl.tilt.about_y),
                    "turn" => Some(pl.rot_deg),
                    _ => None,
                }
            }
            FeatureKind::DatumPoint { point } => {
                let at = self.datum_points.iter().find(|p| p.id == point)?.at;
                ["x", "y", "z"].iter().position(|k| *k == key).map(|i| at[i])
            }
            FeatureKind::DatumAxis { axis } => {
                let AxisDef::Manual { origin, dir } = self.datum_axes.iter().find(|a| a.id == axis)?.def else { return None };
                let i = ["ox", "oy", "oz", "dx", "dy", "dz"].iter().position(|k| *k == key)?;
                Some(if i < 3 { origin[i] } else { dir[i - 3] })
            }
            _ => None,
        }
    }

    /// The sides or the turn of the polygon about centre `c`.
    fn polygon_value(&self, si: usize, c: Id, key: &str) -> Option<f64> {
        let sides = self.polygon_vertices(si, c).len();
        if sides < 3 {
            return None;
        }
        match key {
            "sides" => Some(sides as f64),
            "turn" => self.polygon_angle(si, c).map(f64::to_degrees),
            _ => None,
        }
    }

    /// The full axes or the turn of the ellipse about centre `c`.
    fn ellipse_value(&self, si: usize, c: Id, key: &str) -> Option<f64> {
        let (rx, ry) = self.ellipse_axes(si, c)?;
        match key {
            "w" => Some(2.0 * rx),
            "h" => Some(2.0 * ry),
            "turn" => self.ellipse_turn(si, c).map(f64::to_degrees),
            _ => None,
        }
    }

    /// The turn of the ellipse about centre `c`, in radians: the direction of its major axis.
    fn ellipse_turn(&self, si: usize, c: Id) -> Option<f64> {
        let s = self.sketches.get(si)?;
        let ma = s.entities.iter().find_map(|e| e.kind.as_ellipse().filter(|el| el.c == c).map(|el| el.ma))?;
        let o = s.points.iter().find(|q| q.id == c)?;
        let m = s.points.iter().find(|q| q.id == ma)?;
        Some((m.y - o.y).atan2(m.x - o.x))
    }

    /// The items of sketch `si` that hold a formula of a value of theirs.
    pub(crate) fn sketch_item_ids(&self, si: usize) -> Vec<Id> {
        let Some(s) = self.sketches.get(si) else { return Vec::new() };
        s.patterns.iter().map(|p| p.id).chain(s.texts.iter().map(|t| t.id)).chain(s.points.iter().map(|q| q.id)).filter(|id| self.feat_dims.contains_key(id)).collect()
    }

    /// THE FORMULAS OF THE ITEMS IMPOSED: each value with a formula takes what it comes to; a value already standing at it
    /// is left alone, so a pattern is laid anew and a polygon rebuilt only when its count changed. The words of a formula
    /// that cannot be worked out go into `errs`.
    pub(crate) fn impose_item_values(&mut self, vars: &std::collections::HashMap<String, f64>) -> Vec<ItemFault> {
        let mut faults = Vec::new();
        // the nodes of the timeline read their own formulas as they are built; a mate reads those of its slots as it is
        // solved, and `item_value` answers only its limits
        let own: std::collections::HashSet<Id> = self.timeline.iter().map(|n| n.id).collect();
        let held: Vec<Id> = self.feat_dims.keys().filter(|id| !own.contains(id)).copied().collect();
        for id in held {
            let Some(dims) = self.feat_dims.get(&id).cloned() else { continue };
            for (key, expr) in dims.iter().filter(|(_, e)| !e.trim().is_empty()) {
                let Some(now) = self.item_value(id, key) else { continue };
                match crate::expr::eval(expr, vars) {
                    Ok(v) if (v - now).abs() > 1e-9 => self.impose(&Imposed { id, key, value: v }),
                    Ok(_) => {}
                    Err(error) => faults.push(ItemFault { formula: expr.clone(), error }),
                }
            }
        }
        faults
    }

    /// Item value `i` set to its number, the item laid anew where its shape depends on it.
    fn impose(&mut self, i: &Imposed) {
        if let Some(r) = self.relations.iter_mut().find(|r| r.id == i.id) {
            r.value = i.value;
            return;
        }
        if let Some(j) = self.joints.iter_mut().find(|j| j.id == i.id) {
            let slot: Option<usize> = i.key.get(3..).and_then(|s| s.parse().ok()).filter(|s: &usize| *s < 3);
            match (i.key.get(..3), slot) {
                (Some("min"), Some(s)) => j.limit_min[s] = Some(i.value),
                (Some("max"), Some(s)) => j.limit_max[s] = Some(i.value),
                _ => {}
            }
            return;
        }
        if let Some(c) = self.connectors.iter().find(|c| c.id == i.id).cloned() {
            let mut offset = c.offset_xyz;
            let mut rot = c.rot_deg;
            match i.key {
                "turn" => rot = i.value,
                k => {
                    if let Some(at) = ["x", "y", "z"].iter().position(|x| *x == k) {
                        offset[at] = i.value;
                    }
                }
            }
            self.set_connector_placement(c.id, c.point, rot, offset);
            return;
        }
        let Some(si) = self.sketches.iter().position(|s| s.patterns.iter().any(|p| p.id == i.id) || s.texts.iter().any(|t| t.id == i.id) || s.points.iter().any(|q| q.id == i.id)) else { return };
        if let Some(pi) = self.sketches[si].patterns.iter().position(|p| p.id == i.id) {
            let mut kind = self.sketches[si].patterns[pi].kind;
            set_pattern_value(&mut kind, i.key, i.value);
            self.update_pattern(si, pi, kind);
            return;
        }
        if let Some(t) = self.sketches[si].texts.iter_mut().find(|t| t.id == i.id) {
            // the glyphs are the outline of the letters drawn at the height: they scale about the point the line starts at
            let k = i.value.max(0.01) / t.height.max(1e-9);
            let (x, y) = (t.x, t.y);
            for g in &mut t.glyphs {
                for p in g.iter_mut() {
                    *p = Point2::new(x + (p.x - x) * k, y + (p.y - y) * k);
                }
            }
            t.height = i.value.max(0.01);
            self.regen_sketch(si);
            return;
        }
        match i.key {
            "sides" => {
                self.set_polygon_sides(si, i.id, i.value.round().max(3.0) as u32);
            }
            "turn" if self.polygon_value(si, i.id, "sides").is_some() => self.turn_polygon_points(si, i.id, i.value.to_radians()),
            "turn" => self.turn_ellipse_points(si, i.id, i.value.to_radians()),
            "w" | "h" => {
                if let Some((rx, ry)) = self.ellipse_axes(si, i.id) {
                    let (rx, ry) = if i.key == "w" { (i.value / 2.0, ry) } else { (rx, i.value / 2.0) };
                    self.ellipse_axes_points(si, i.id, [rx, ry]);
                }
            }
            _ => {}
        }
    }

    /// The vertices of the polygon about centre `c` turned to angle `a` (radians), at its radius, with no solve: the
    /// solve that follows the evaluation holds them.
    fn turn_polygon_points(&mut self, si: usize, c: Id, a: f64) {
        let verts = self.polygon_vertices(si, c);
        let (Some((cx, cy, r)), Some(cur)) = (self.polygon_circle(si, c), self.polygon_angle(si, c)) else { return };
        for vid in verts {
            if let Some(q) = self.sketches[si].points.iter_mut().find(|q| q.id == vid) {
                let ang = (q.y - cy).atan2(q.x - cx) + a - cur;
                (q.x, q.y) = (cx + r * ang.cos(), cy + r * ang.sin());
            }
        }
    }

    /// THE ELLIPSE ABOUT CENTRE `c` TURNED to angle `a` (radians) - its major axis along it - and the sketch solved.
    /// Answers whether there is such an ellipse.
    pub fn set_ellipse_turn(&mut self, si: usize, c: Id, a: f64) -> bool {
        if self.ellipse_value(si, c, "turn").is_none() {
            return false;
        }
        self.turn_ellipse_points(si, c, a);
        self.solve_sketch(si);
        true
    }

    /// The axes of the ellipse about centre `c` turned to angle `a` (radians), with no solve.
    fn turn_ellipse_points(&mut self, si: usize, c: Id, a: f64) {
        let Some((rx, ry)) = self.ellipse_axes(si, c) else { return };
        let Some((ma, mi)) = self.sketches[si].entities.iter().find_map(|e| e.kind.as_ellipse().filter(|el| el.c == c).map(|el| (el.ma, el.mi))) else { return };
        let Some((cx, cy)) = self.sketches[si].points.iter().find(|q| q.id == c).map(|q| (q.x, q.y)) else { return };
        let half = std::f64::consts::FRAC_PI_2;
        let mut put = |pid: Id, r: f64, ang: f64| {
            if let Some(q) = self.sketches[si].points.iter_mut().find(|q| q.id == pid) {
                q.x = cx + r * ang.cos();
                q.y = cy + r * ang.sin();
            }
        };
        put(ma, rx, a);
        put(mi, ry, a + half);
    }

    /// THE SIDES OF THE POLYGON ABOUT CENTRE `c` MADE `n`, ITS OWN POINTS AND LINES KEPT: the vertices it has and the
    /// sides between them stay, set round its circle at its radius and its turn; the ones it gains are added after them,
    /// the ones it loses go with what stood on them; one side is turned to close it. Only the ties of the figure itself
    /// are laid anew. Laid anew whole, a polygon whose count was typed took new ids each time, and what held a vertex
    /// went with it. Answers whether the polygon has `n` sides now.
    pub fn set_polygon_sides(&mut self, si: usize, c: Id, n: u32) -> bool {
        let n = n.max(3) as usize;
        let (Some((cx, cy, r)), Some(a0)) = (self.polygon_circle(si, c), self.polygon_angle(si, c)) else { return false };
        let Some(s) = self.sketches.get(si) else { return false };
        let Some(circ) = s.entities.iter().find(|e| e.kind.as_circle().is_some_and(|k| k.center == c)).map(|e| e.id) else { return false };
        let Some(fi) = s.figures.iter().position(|f| f.curves.contains(&circ)) else { return false };
        let ends = |id: Id| s.entity(id).and_then(|e| e.kind.as_line()).map(|l| [l.a, l.b]);
        let lines: Vec<Id> = s.figures[fi].curves.iter().copied().filter(|&id| ends(id).is_some()).collect();
        // THE VERTICES IN ORDER ROUND IT, walked along its sides from the first
        let Some([v0, mut next]) = lines.first().and_then(|&l| ends(l)) else { return false };
        let mut verts = vec![v0];
        let mut sides = vec![lines[0]];
        while next != v0 && verts.len() <= lines.len() {
            verts.push(next);
            let Some(&l) = lines.iter().find(|&&l| !sides.contains(&l) && ends(l).is_some_and(|e| e.contains(&next))) else { return false };
            sides.push(l);
            next = ends(l).and_then(|e| e.into_iter().find(|&p| p != next)).unwrap_or(v0);
        }
        let m = verts.len();
        if m == n {
            return true;
        }
        let construction = s.entity(sides[0]).is_some_and(|e| e.construction);
        // the figure is taken out while its curves change - a curve of it deleted takes the figure with it - and put back
        // under its own id
        let figure = self.sketches[si].figures.remove(fi);
        // side k runs from vertex k to vertex k + 1; the last one closes the polygon on vertex 0
        let mut pids: Vec<Id> = verts.iter().copied().take(n).collect();
        let gone: Vec<Id> = verts.iter().copied().skip(n).collect();
        while pids.len() < n {
            let id = self.alloc_id();
            self.sketches[si].points.push(SketchPoint { id, x: cx, y: cy });
            pids.push(id);
        }
        let set_ends = |p: &mut Project, line: Id, a: Id, b: Id| {
            if let Some(e) = p.sketches[si].entities.iter_mut().find(|e| e.id == line) {
                e.kind = crate::model::LineParts { a, b }.into();
            }
        };
        let mut eids: Vec<Id> = sides.iter().copied().take(n.min(m) - 1).collect();
        if n > m {
            // the side that closed it goes on from the last old vertex, and the new ones follow to close it again
            set_ends(self, sides[m - 1], pids[m - 1], pids[m]);
            eids.push(sides[m - 1]);
            for k in m..n {
                let id = self.alloc_id();
                self.sketches[si].entities.push(SketchEntity { id, kind: crate::model::LineParts { a: pids[k], b: pids[(k + 1) % n] }.into(), construction });
                eids.push(id);
            }
        } else {
            set_ends(self, sides[n - 1], pids[n - 1], pids[0]);
            eids.push(sides[n - 1]);
            let extra: Vec<Id> = sides.iter().copied().skip(n).collect();
            self.delete_entities(si, &extra);
        }
        let sk = &mut self.sketches[si];
        // the ties of the figure itself: each vertex on the circle, the sides equal
        let own_tie = |k: &Constraint| match *k {
            Constraint::PointOnCircle { p, c: cc } => cc == c && verts.contains(&p),
            Constraint::Equal { a, b, c: cc, d } => [a, b, cc, d].iter().all(|p| verts.contains(p)),
            _ => false,
        };
        sk.constraints.retain(|k| !own_tie(k) && !k.points().iter().any(|p| gone.contains(p)));
        sk.points.retain(|q| !gone.contains(&q.id));
        for (k, &p) in pids.iter().enumerate() {
            let a = a0 + std::f64::consts::TAU * k as f64 / n as f64;
            if let Some(q) = sk.points.iter_mut().find(|q| q.id == p) {
                (q.x, q.y) = (cx + r * a.cos(), cy + r * a.sin());
            }
        }
        let mut own: Vec<Constraint> = pids.iter().map(|&p| Constraint::PointOnCircle { p, c }).collect();
        own.extend((1..n).map(|k| Constraint::Equal { a: pids[0], b: pids[1], c: pids[k], d: pids[(k + 1) % n] }));
        sk.constraints.extend(own.iter().cloned());
        sk.figures.push(SketchFigure { curves: eids.into_iter().chain([circ]).collect(), own, ..figure });
        self.regen_sketch(si);
        true
    }
}

/// Value `key` of a pattern of a sketch.
fn pattern_value(kind: &PatternKind, key: &str) -> Option<f64> {
    if !PATTERN_KEYS.contains(&key) {
        return None;
    }
    match *kind {
        PatternKind::Linear { dx, dy, count, dx2, dy2, count2 } => match key {
            "count" => Some(f64::from(count)),
            "dx" => Some(dx),
            "dy" => Some(dy),
            "count2" => Some(f64::from(count2)),
            "dx2" => Some(dx2),
            "dy2" => Some(dy2),
            _ => None,
        },
        PatternKind::Circular { count, total_deg, .. } => match key {
            "count" => Some(f64::from(count)),
            "angle" => Some(total_deg),
            _ => None,
        },
    }
}

/// Value `key` of a pattern of a sketch set to `v`; a count is whole and at least one.
fn set_pattern_value(kind: &mut PatternKind, key: &str, v: f64) {
    let whole = v.round().max(1.0) as u32;
    match kind {
        PatternKind::Linear { dx, dy, count, dx2, dy2, count2 } => match key {
            "count" => *count = whole,
            "dx" => *dx = v,
            "dy" => *dy = v,
            "count2" => *count2 = whole,
            "dx2" => *dx2 = v,
            "dy2" => *dy2 = v,
            _ => {}
        },
        PatternKind::Circular { count, total_deg, .. } => match key {
            "count" => *count = whole,
            "angle" => *total_deg = v,
            _ => {}
        },
    }
}
