//! A pattern of components.
//!
//! Patterns of bodies existed; patterns of components did not, so bolts around a circle were inserted by hand
//! and each one had to be moved separately. Here a copy is not a part inserted once more but an instance: its
//! body associatively repeats the active body of the source (`FeatureKind::PartInstance`) while the pattern
//! itself drives the placement. Edit the source part and every copy follows; move the pattern and every copy
//! follows; delete the pattern and the copies go while the source stays.
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
    /// Linear: `count` instances spaced by `step` along `dir`, in the local frame of the parent assembly.
    Linear { dir: [f64; 3], step: f64, count: u32 },
    /// Circular: `count` instances filling `angle` degrees about the axis given by `origin` and `dir`.
    Circular { origin: [f64; 3], dir: [f64; 3], angle: f64, count: u32 },
}

impl CompPatternKind {
    /// How many instances there are in total, including the source, which is the first.
    pub fn count(&self) -> u32 {
        match *self {
            CompPatternKind::Linear { count, .. } | CompPatternKind::Circular { count, .. } => count.max(1),
        }
    }

    /// The transform of instance i in the local frame of the parent; i = 0 is the source itself, the identity.
    pub fn step_transform(&self, i: u32) -> [f64; 12] {
        match *self {
            CompPatternKind::Linear { dir, step, .. } => {
                let k = i as f64 * step;
                let mut m = PLACE_IDENTITY;
                m[3] = dir[0] * k;
                m[7] = dir[1] * k;
                m[11] = dir[2] * k;
                m
            }
            CompPatternKind::Circular { origin, dir, angle, count } => {
                // A full circle is divided by `count`, and so is a partial one — the same convention as for a
                // pattern of bodies. Otherwise the same 360° would lay out bodies and components differently.
                let c = count.max(1) as f64;
                let step = if angle.abs() >= 359.9 { 360.0 / c } else { angle / c };
                rot_about_axis(origin, dir, i as f64 * step)
            }
        }
    }
}

/// A component pattern: the source, the layout and the ids of the copies, excluding the source.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompPattern {
    pub id: Id,
    /// The source component; the first instance is the source itself.
    pub src: Id,
    pub kind: CompPatternKind,
    /// The copies, in instance order from 1 to count.
    #[serde(default)]
    pub copies: Vec<Id>,
}

impl Project {
    /// Create a component pattern. The copies are created immediately and `resolve_comp_patterns` computes
    /// their placement.
    ///
    /// Returns the id of the pattern; zero means the source is unsuitable, having no body or being the root of
    /// the document.
    pub fn add_comp_pattern(&mut self, src: Id, kind: CompPatternKind) -> Id {
        if src == self.root || !self.components.iter().any(|c| c.id == src) {
            return 0;
        }
        if self.active_body(src).is_none() {
            return 0; // there is nothing to copy, and a pattern of empty components is of no use
        }
        let id = self.alloc_id();
        let mut pat = CompPattern { id, src, kind, copies: Vec::new() };
        self.grow_copies(&mut pat);
        self.comp_patterns.push(pat);
        self.resolve_comp_patterns();
        id
    }

    /// Change the layout of a pattern: count, step or angle. Copies are added or removed to match the count.
    pub fn set_comp_pattern(&mut self, id: Id, kind: CompPatternKind) -> bool {
        let Some(k) = self.comp_patterns.iter().position(|p| p.id == id) else { return false };
        let mut pat = self.comp_patterns.remove(k);
        pat.kind = kind;
        // Surplus copies are removed and missing ones created in place, without recreating the pattern: the
        // copies have ids of their own, joints may rest on them, and recreating everything would tear those
        // apart on every change of the count.
        let want = kind.count().saturating_sub(1) as usize;
        while pat.copies.len() > want {
            if let Some(c) = pat.copies.pop() {
                self.delete_component(c);
            }
        }
        self.grow_copies(&mut pat);
        self.comp_patterns.insert(k, pat);
        self.resolve_comp_patterns();
        true
    }

    /// Delete a pattern: the copies go and the source stays, being a part in its own right rather than a
    /// product of the pattern.
    pub fn delete_comp_pattern(&mut self, id: Id) -> bool {
        let Some(k) = self.comp_patterns.iter().position(|p| p.id == id) else { return false };
        let pat = self.comp_patterns.remove(k);
        for c in pat.copies {
            self.delete_component(c);
        }
        true
    }

    /// The pattern a component belongs to, whether as source or as copy; used by the tree and by deletion.
    pub fn comp_pattern_of(&self, comp: Id) -> Option<&CompPattern> {
        self.comp_patterns.iter().find(|p| p.src == comp || p.copies.contains(&comp))
    }

    /// Lay the copies out. Called from the rebuild: a pattern is parametric and has to follow its source, so
    /// moving the part moves the whole row with it.
    pub fn resolve_comp_patterns(&mut self) {
        let plans: Vec<(Vec<Id>, Vec<[f64; 12]>)> = self
            .comp_patterns
            .iter()
            .map(|p| {
                let base = self.components.iter().find(|c| c.id == p.src).map(|c| c.transform).unwrap_or(PLACE_IDENTITY);
                let ts = (1..=p.copies.len() as u32).map(|i| mat_mul12(&p.kind.step_transform(i), &base)).collect();
                (p.copies.clone(), ts)
            })
            .collect();
        for (copies, ts) in plans {
            for (c, t) in copies.into_iter().zip(ts) {
                self.set_component_transform(c, t);
            }
        }
    }

    /// Bring the number of copies up to `count - 1`, creating the missing ones.
    fn grow_copies(&mut self, pat: &mut CompPattern) {
        let want = pat.kind.count().saturating_sub(1) as usize;
        let (name, parent) = self
            .components
            .iter()
            .find(|c| c.id == pat.src)
            .map(|c| (c.name.clone(), c.parent))
            .unwrap_or_else(|| ("name-part".into(), Some(self.root)));
        while pat.copies.len() < want {
            let n = pat.copies.len() + 1;
            let saved = self.active_component;
            self.active_component = parent;
            let comp = self.add_part(format!("{name} ({})", n + 1));
            self.active_component = saved;
            // the body of a copy is an associative instance of the source: the rebuild drives its shape and
            // the pattern its placement
            let body = self.alloc_id();
            self.push_timeline(FeatureNode {
                id: body,
                name: "name-instance".into(),
                kind: FeatureKind::PartInstance { src_comp: pat.src, body },
                parent: Some(comp),
                dirty: true,
                suppressed: false,
            });
            pat.copies.push(comp);
        }
    }
}
