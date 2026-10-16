//! THE NAMES OF THE COPIES OF A BODY: the instances of a pattern, the image of a mirror, and the section a split
//! leaves on its pieces. The model says which node; the rule is here.

use crate::door::Roles;
use qymcad_core::feature::{ImageNames, Rename};
use qymcad_core::model::Id;
use qymcad_core::names::{GeoName, Role};

/// FACE NAMES PER PATTERN INSTANCE: `[k]` holds pairs of "source face to its name in copy k".
///
/// An instance is not the same face but the image of a face under the k-th transform, so the name comes from the
/// recipe: `Role::Instance` with `src` set to the descriptor of the source face and `split` to the copy number. Without
/// this every copy carries the names of the original (measured: 18 faces sharing 6 names) and a reference to a face of
/// the second copy silently resolves to the first, putting a feature in the wrong place.
///
/// Copy zero keeps the original names: it is the source in its own place, and references made before the pattern
/// existed have to keep working. The pairs are laid before the union: after it every copy carries the numbers of the
/// original, unique within one copy, not across all of them.
pub fn instances(feature: Id, faces: &[u32], count: usize, roles: &mut Roles) -> Vec<Vec<Rename>> {
    if faces.is_empty() || count < 2 {
        return Vec::new();
    }
    (0..count)
        .map(|k| {
            if k == 0 {
                return Vec::new();
            }
            faces.iter().map(|&f| Rename { was: f, now: roles.give(GeoName { feature, role: Role::Instance, src: f as Id, split: k as u16 }) }).collect()
        })
        .collect()
}

/// THE IMAGE OF A MIRROR named as the second instance of the node, or left with the names it reflects. An image kept
/// beside the original is not the original face, and without names of its own the two halves are indistinguishable.
pub fn image(names: ImageNames, faces: &[u32], roles: &mut Roles) -> Vec<Rename> {
    match names {
        ImageNames::Source => Vec::new(),
        ImageNames::Instance { feature } => instances(feature, faces, 2, roles).pop().unwrap_or_default(),
    }
}

/// THE SECTION A SPLIT LEAVES on each of its pieces, named after the node: one plane, one name, in every piece.
pub fn section(feature: Id, roles: &mut Roles) -> u32 {
    roles.give(GeoName::new(feature, Role::CutSection, 0))
}
