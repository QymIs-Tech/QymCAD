//! A pattern of components.
//!
//! Patterns of bodies existed; patterns of components did not, so bolts around a circle were inserted by hand
//! and each one had to be moved separately. The pattern is ONE node of the assembly's timeline
//! (`FeatureKind::ComponentPattern`): it builds the body of every copy, repeating the active body of the source, while
//! the copies stay parts of their own that the pattern places. Edit the source part and every copy follows; move
//! the source and the row follows; delete the node and the copies go while the source stays. Reported behaviour:
//! a pattern of three laid a node per copy, and could not be reopened or deleted as one operation.
//!
//! The same approach as a mirrored part, and the same boundary: the active body of the source is what gets
//! copied, a part being one body in this project. A part with several bodies is copied by its first active one,
//! and that is stated here rather than left silent.
use super::{Id, Project};
use super::tess::rot_about_axis;
use crate::feature::{mat_mul12, FeatureKind, FeatureNode, PLACE_IDENTITY};
use serde::{Deserialize, Serialize};

/// How the copies are laid out.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum CompPatternKind {
    /// Linear: `count` instances spaced by `step` along `dir`, in the local frame of the parent assembly; `more` holds
    /// a second and a third direction with their own step and count, a grid as a pattern of bodies makes one (a count
    /// of one leaves a direction unused).
    Linear {
        dir: [f64; 3],
        step: f64,
        count: u32,
        #[serde(default = "one_way")]
        more: [([f64; 3], f64, u32); 2],
    },
    /// Circular: `count` instances filling `angle` degrees about the axis given by `origin` and `dir`; a non-zero
    /// `axis` is a datum axis the numbers are read off at every rebuild - picked on an edge, a cylindrical face or
    /// placed by hand - so the pattern turns about it and follows it.
    Circular {
        origin: [f64; 3],
        dir: [f64; 3],
        angle: f64,
        count: u32,
        #[serde(default)]
        axis: Id,
    },
}

/// The second and third directions of a linear pattern that has only its first.
pub fn one_way() -> [([f64; 3], f64, u32); 2] {
    [([0.0, 1.0, 0.0], 0.0, 1), ([0.0, 0.0, 1.0], 0.0, 1)]
}

impl CompPatternKind {
    /// A linear pattern along one direction.
    pub fn linear(dir: [f64; 3], step: f64, count: u32) -> Self {
        CompPatternKind::Linear { dir, step, count, more: one_way() }
    }

    /// How many instances there are in total, including the source, which is the first: along a grid, the product of
    /// the counts of its directions. Zero when that is past `MAX_PATTERN_INSTANCES`: such a layout is refused, and a
    /// zero makes no copies wherever the count is used.
    pub fn count(&self) -> u32 {
        crate::feature::pattern_instances(&self.counts()).unwrap_or(0)
    }

    /// The count along each direction, as the layout states it.
    pub fn counts(&self) -> [u32; 3] {
        match *self {
            CompPatternKind::Linear { count, more, .. } => [count, more[0].2, more[1].2],
            CompPatternKind::Circular { count, .. } => [count, 1, 1],
        }
    }

    /// The transform of instance i in the local frame of the parent; i = 0 is the source itself, the identity.
    pub fn step_transform(&self, i: u32) -> [f64; 12] {
        match *self {
            CompPatternKind::Linear { dir, step, count, more } => {
                // instance i of the grid: along the first direction first, then the second, then the third
                // in 64 bits: the product of two counts no longer wraps round to zero and divides by it
                let (c1, c2, i) = (count.max(1) as u64, more[0].2.max(1) as u64, i as u64);
                let (a, b, c) = (i % c1, (i / c1) % c2, i / (c1 * c2));
                let mut m = PLACE_IDENTITY;
                for (d, s, k) in [(dir, step, a), (more[0].0, more[0].1, b), (more[1].0, more[1].1, c)] {
                    let t = k as f64 * s;
                    m[3] += d[0] * t;
                    m[7] += d[1] * t;
                    m[11] += d[2] * t;
                }
                m
            }
            CompPatternKind::Circular { origin, dir, angle, count, .. } => {
                // A full circle is divided by `count`, and so is a partial one — the same convention as for a
                // pattern of bodies. Otherwise the same 360° would lay out bodies and components differently.
                let c = count.max(1) as f64;
                let step = if angle.abs() >= 359.9 { 360.0 / c } else { angle / c };
                rot_about_axis(origin, dir, i as f64 * step)
            }
        }
    }
}

/// A component pattern as its node holds it: the node's id, the source, the layout and the copies (the source not
/// among them), in instance order from 1.
#[derive(Clone, Debug, PartialEq)]
pub struct CompPattern {
    pub id: Id,
    pub src: Id,
    pub kind: CompPatternKind,
    pub copies: Vec<Id>,
}

impl Project {
    /// Create a component pattern: one node in the assembly the source stands in, and its copies. Returns the id of
    /// the node; zero means the source is unsuitable, having no body or being the root of the document, or the layout
    /// makes no copy - a pattern of one instance is the source alone, and a node for it would change nothing.
    pub fn add_comp_pattern(&mut self, src: Id, kind: CompPatternKind) -> Id {
        if src == self.root || !self.components.iter().any(|c| c.id == src) || kind.count() < 2 {
            return 0;
        }
        if self.active_body(src).is_none() {
            return 0; // there is nothing to copy, and a pattern of empty components is of no use
        }
        let parent = self.components.iter().find(|c| c.id == src).and_then(|c| c.parent).or(Some(self.root));
        let id = self.alloc_id();
        let name = match kind {
            CompPatternKind::Linear { .. } => "feat-name-comp-linear-array",
            CompPatternKind::Circular { .. } => "feat-name-comp-circular-array",
        };
        self.push_timeline(FeatureNode { id, name: name.into(), kind: FeatureKind::ComponentPattern { src, kind, copies: Vec::new(), bodies: Vec::new() }, parent, dirty: true, suppressed: false });
        self.grow_copies(id);
        self.resolve_comp_patterns();
        id
    }

    /// Change the layout of a pattern: count, step or angle. Copies are added or removed to match the count, the
    /// ones kept keep their ids: joints may rest on them, and recreating everything would tear those apart on every
    /// change of the count.
    pub fn set_comp_pattern(&mut self, id: Id, kind: CompPatternKind) -> bool {
        if kind.count() == 0 {
            return false; // past `MAX_PATTERN_INSTANCES`: the pattern keeps the layout it had
        }
        let Some(i) = self.timeline.iter().position(|n| n.id == id && matches!(n.kind, FeatureKind::ComponentPattern { .. })) else { return false };
        let want = kind.count().saturating_sub(1) as usize;
        let mut gone = Vec::new();
        if let FeatureKind::ComponentPattern { kind: k, copies, bodies, .. } = &mut self.timeline[i].kind {
            *k = kind;
            while copies.len() > want {
                gone.extend(copies.pop());
                bodies.pop();
            }
        }
        self.timeline[i].dirty = true;
        for c in gone {
            self.delete_component(c);
        }
        self.drop_orphan_bodies();
        self.grow_copies(id);
        self.resolve_comp_patterns();
        true
    }

    /// Delete a pattern: its node and the copies go, the source stays, being a part in its own right rather than a
    /// product of the pattern.
    pub fn delete_comp_pattern(&mut self, id: Id) -> bool {
        let Some(pat) = self.comp_pattern(id) else { return false };
        self.timeline.retain(|n| n.id != id);
        self.regen_errors.remove(&id);
        for c in pat.copies {
            self.delete_component(c);
        }
        self.drop_orphan_bodies();
        true
    }

    /// The pattern whose node is `id`.
    pub fn comp_pattern(&self, id: Id) -> Option<CompPattern> {
        self.comp_patterns().into_iter().find(|p| p.id == id)
    }

    /// Every pattern of components of the document, in timeline order.
    pub fn comp_patterns(&self) -> Vec<CompPattern> {
        self.timeline
            .iter()
            .filter_map(|n| match &n.kind {
                FeatureKind::ComponentPattern { src, kind, copies, .. } => Some(CompPattern { id: n.id, src: *src, kind: *kind, copies: copies.clone() }),
                _ => None,
            })
            .collect()
    }

    /// The pattern a component belongs to, whether as source or as copy; used by the tree and by deletion.
    pub fn comp_pattern_of(&self, comp: Id) -> Option<CompPattern> {
        self.comp_patterns().into_iter().find(|p| p.src == comp || p.copies.contains(&comp))
    }

    /// Lay the copies out. Called from the rebuild: a pattern is parametric and has to follow its source, so
    /// moving the part moves the whole row with it.
    pub fn resolve_comp_patterns(&mut self) {
        let plans: Vec<(Vec<Id>, Vec<[f64; 12]>)> = self
            .comp_patterns()
            .iter()
            .map(|p| {
                let base = self.components.iter().find(|c| c.id == p.src).map(|c| c.transform).unwrap_or(PLACE_IDENTITY);
                let mut kind = p.kind;
                if let CompPatternKind::Circular { origin, dir, axis, .. } = &mut kind {
                    let frame = self.components.iter().find(|c| c.id == p.src).and_then(|c| c.parent).unwrap_or(self.root);
                    if let Some((o, d)) = (*axis != 0).then(|| self.pattern_axis(*axis, frame)).flatten() {
                        (*origin, *dir) = (o, d);
                    }
                }
                let ts = (1..=p.copies.len() as u32).map(|i| mat_mul12(&kind.step_transform(i), &base)).collect();
                (p.copies.clone(), ts)
            })
            .collect();
        for (copies, ts) in plans {
            for (c, t) in copies.into_iter().zip(ts) {
                self.set_component_transform(c, t);
            }
        }
    }

    /// THE DATUM AXIS `axis` IN THE FRAME OF COMPONENT `frame`: an axis read off an edge or a face lives in the frame
    /// of that body's part, any other in the frame of the component it was made in; the pattern turns in the frame of
    /// the assembly it stands in.
    fn pattern_axis(&self, axis: Id, frame: Id) -> Option<([f64; 3], [f64; 3])> {
        use crate::model::AxisDef;
        let a = self.datum_axes.iter().find(|d| d.id == axis)?;
        // points standing on a vertex of a part live in that part's frame, as an edge's axis does
        let on_vertex = |pt: Id| {
            self.datum_points.iter().find(|d| d.id == pt).and_then(|d| match d.def {
                crate::model::PointDef::AtVertex { body, .. } => self.body_owner(body),
                _ => None,
            })
        };
        let own = match a.def {
            AxisDef::FromEdge { body, .. } | AxisDef::FromFace { body, .. } => self.body_owner(body),
            AxisDef::TwoPoints { a: pa, .. } if on_vertex(pa).is_some() => on_vertex(pa),
            _ => self.timeline.iter().find(|n| matches!(n.kind, FeatureKind::DatumAxis { axis: x } if x == axis)).and_then(|n| n.parent),
        }
        .unwrap_or(self.root);
        let m = self.relative_transform(own, frame);
        Some((crate::feature::apply12(&m, a.origin()), crate::feature::apply12_dir(&m, a.dir())))
    }

    /// Bring the copies of pattern `id` up to `count - 1`, each a part of its own with a body the node builds.
    fn grow_copies(&mut self, id: Id) {
        let Some(i) = self.timeline.iter().position(|n| n.id == id) else { return };
        let FeatureKind::ComponentPattern { src, kind, copies, .. } = &self.timeline[i].kind else { return };
        let (src, want, have) = (*src, kind.count().saturating_sub(1) as usize, copies.len());
        let (name, parent) = self.components.iter().find(|c| c.id == src).map(|c| (c.name.clone(), c.parent)).unwrap_or_else(|| ("name-part".into(), Some(self.root)));
        for n in have..want {
            let saved = self.active_component;
            self.active_component = parent;
            let comp = self.add_part(format!("{name} ({})", n + 2));
            self.active_component = saved;
            let body = self.alloc_id();
            if let FeatureKind::ComponentPattern { copies, bodies, .. } = &mut self.timeline[i].kind {
                copies.push(comp);
                bodies.push(body);
            }
        }
        self.timeline[i].dirty = true;
    }
}
