//! THE NAMES OF THE FACES OF A PRIMITIVE: a cylinder, a sphere, a cone, a torus. A box and a prism are extrusions of a
//! polygon and are named as one (`ops::profile`).

use crate::door::Roles;
use qymcad_core::model::Id;
use qymcad_core::names::{GeoName, Role};

/// The end face at -z, the end face at +z and the side surface, named after the node. A primitive has no sketch, so
/// the source of a role is the role itself (`src` = 0): the recipe defines the whole topology. A sphere and a torus use
/// the side alone.
pub fn ends_and_side(feature: Id, roles: &mut Roles) -> [u32; 3] {
    [Role::CapStart, Role::CapEnd, Role::Side].map(|role| roles.give(GeoName::new(feature, role, 0)))
}
