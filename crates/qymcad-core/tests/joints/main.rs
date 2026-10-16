//! Mates built on connectors: propagation over the tree, grounding, parametric angles, loops and degrees of
//! freedom. The solver is a pure model and needs no geometry kernel.

use qymcad_core::feature::{apply12, mat_mul12, AnchorRef, JointKind};
use qymcad_core::model::{Id, Project};

fn tr(x: f64, y: f64, z: f64) -> [f64; 12] {
    [1.0, 0.0, 0.0, x, 0.0, 1.0, 0.0, y, 0.0, 0.0, 1.0, z]
}

fn set_transform(p: &mut Project, comp: Id, m: [f64; 12]) {
    let i = p.component_index(comp).unwrap();
    p.components[i].transform = m;
}

/// An assembly with N parts as children of the root. Returns their ids.
fn parts(p: &mut Project, n: usize) -> Vec<Id> {
    p.new_document();
    let root = p.root;
    p.set_active_component(Some(root));
    (0..n).map(|i| p.add_part(format!("P{i}"))).collect()
}

/// An anchor a joint is tried on: its name, how to build one on part `c`, and the travel axis a slider is expected
/// to take from it.
pub(crate) struct Anchor {
    name: &'static str,
    make: Box<MakeAnchor>,
    axis: [f64; 3],
}

/// How an anchor is built on part `c`.
pub(crate) type MakeAnchor = dyn Fn(&mut Project, Id) -> AnchorRef;

fn anchor(name: &'static str, make: Box<MakeAnchor>, axis: [f64; 3]) -> Anchor {
    Anchor { name, make, axis }
}

mod basics;
mod slots;
mod anchors;
mod kinds;
mod turned;
