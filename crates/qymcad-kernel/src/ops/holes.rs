//! THE NAMES OF THE FACES A HOLE AND A HELIX MAKE: the wall of a hole after the sketch point that placed it, the
//! faces of the drill tip and countersink after the node, the faces of a thread groove or an auger flight after the
//! edge of the profile and the start that swept them. The model says which node; the rule is here.

use crate::door::Roles;
use qymcad_core::model::Id;
use qymcad_core::names::{GeoName, Role};

/// The wall of a hole, named after the sketch point that placed it (`site`, 0 for a hole placed on a face): adding a
/// point does not rename the neighbouring holes.
pub fn bore(feature: Id, site: Id, roles: &mut Roles) -> u32 {
    roles.give(GeoName::new(feature, Role::Hole, site))
}

/// The remaining faces of the drill: the tip, the countersink and its bottom.
pub fn tool_faces(feature: Id, roles: &mut Roles) -> Vec<u32> {
    (0..3).map(|k| roles.give(GeoName::new(feature, Role::HoleTool, k as Id))).collect()
}

/// THE NAMES OF THE TURNS OF A HELIX, as the C++ side reads them: a groove face for every edge of the profile and
/// every start (start-major), and the sixteen faces of the rings at the ends - `[((tool * 2 + end) * 4) + surface]`,
/// tool 0 the relief and 1 the lead-in, end 0 the start and 1 the finish, surface 0 the near plane, 1 the far plane,
/// 2 the cylinder, 3 the cone. Without them each ring added three unnamed faces, the lead-in chamfer a person puts a
/// fillet on among them.
pub struct Turns {
    pub grooves: Vec<u32>,
    pub reliefs: Vec<u32>,
}

/// The turns of a helix swept from `profile` (an encoded profile of one loop, `[1, n, n edges...]`) `starts` times.
/// The turn faces are named before the sweep: the recipe supplies the names (a profile edge plus the lead). Without
/// them a probe of "cylinder, thread, cut, fillet" had 6 faces of 78 named by recipe, and moving one sketch point
/// disturbed the rest, the edges between them and the fillet references. The groove profile comes from the thread
/// standard with the same edges (flanks, crest, root) at any diameter, pitch and length, so the edge number is a
/// recipe; the start number keeps the copies of a multi-start groove apart.
pub fn turns(feature: Id, profile: &[f64], starts: u32, roles: &mut Roles) -> Turns {
    let edges = profile.get(1).copied().unwrap_or(0.0) as usize;
    let mut grooves = Vec::with_capacity(edges * starts.max(1) as usize);
    for start in 0..starts.max(1) {
        for e in 0..edges {
            grooves.push(roles.give(GeoName { feature, role: Role::ThreadGroove, src: e as Id, split: start as u16 }));
        }
    }
    let mut reliefs = Vec::with_capacity(16);
    for ring in 0..4u32 {
        for surface in 0..4u32 {
            reliefs.push(roles.give(GeoName { feature, role: Role::ThreadRelief, src: ring as Id, split: surface as u16 }));
        }
    }
    Turns { grooves, reliefs }
}
