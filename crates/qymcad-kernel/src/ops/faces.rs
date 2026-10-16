//! THE NAMES OF THE FACES THE OPERATIONS ON FACES MAKE: a copy or an offset of a face, a patch, the inner walls of a
//! shell, the plate and walls of a thickening, the side faces of a draft. The model says which node; the rule is here.

use crate::door::Roles;
use crate::ImageOf;
use qymcad_core::model::Id;
use qymcad_core::names::{GeoName, NameTable, Role};

/// The face a copy or an offset makes of each of `faces`, named after the face it was taken from.
pub fn instances(feature: Id, faces: &[u32], roles: &mut Roles) -> Vec<u32> {
    faces.iter().map(|f| roles.give(GeoName::new(feature, Role::Instance, *f as Id))).collect()
}

/// The one face of a patch, named after the node: there is one patch a node and its surface has no other source.
pub fn patch(feature: Id, roles: &mut Roles) -> u32 {
    roles.give(GeoName::new(feature, Role::Patch, 0))
}

/// THE INNER WALL A SHELL MAKES FROM EVERY FACE OF THE BODY, named after that face. Without its own name an inner wall
/// took the name of its outer face and could not be told from it. Only a face named by recipe gives a wall a name: a
/// positional number is nothing to derive one from.
pub fn shell_walls(feature: Id, faces: &[u32], roles: &mut Roles) -> Vec<ImageOf> {
    faces.iter().filter(|f| NameTable::is_named(**f)).map(|f| ImageOf { source: *f, name: roles.give(GeoName::new(feature, Role::ShellWall, *f as Id)) }).collect()
}

/// THE NAMES OF A THICKENED PLATE, as the C++ side reads them - pairs laid flat: the offset side of every face after
/// that face, and the wall every edge sweeps after that edge.
pub struct Thickened {
    pub faces: Vec<u32>,
    pub edges: Vec<u32>,
}

pub fn thickened(feature: Id, faces: &[u32], edges: &[u32], roles: &mut Roles) -> Thickened {
    let mut out = Thickened { faces: Vec::with_capacity(faces.len() * 2), edges: Vec::with_capacity(edges.len() * 2) };
    for f in faces {
        out.faces.push(*f);
        out.faces.push(roles.give(GeoName::new(feature, Role::Thickened, *f as Id)));
    }
    for e in edges {
        out.edges.push(*e);
        out.edges.push(roles.give(GeoName::new(feature, Role::ThickenWall, *e as Id)));
    }
    out
}

/// THE SIDE FACE A DRAFT MAKES next to every face it tilts, named after that face, as pairs laid flat: without a name
/// of its own it took a positional number.
pub fn draft_sides(feature: Id, faces: &[u32], roles: &mut Roles) -> Vec<u32> {
    faces.iter().filter(|f| NameTable::is_named(**f)).flat_map(|f| [*f, roles.give(GeoName::new(feature, Role::DraftSide, *f as Id))]).collect()
}

/// THE RADIUS A ROUND FACE WOULD BE MOVED PAST: the first of `faces` - a cylinder - that an offset of `dist` along its
/// normal brings to its axis or beyond it, its radius. Moved in by its radius a cylinder closes on its axis, past it it
/// turns inside out, and the offset of the kernel answers only "no result". Which way the face moves is told by its
/// normal at its place: away from the axis for the outside of a shaft, towards it for the wall of a hole. A sphere, a
/// cone and a torus are refused in the general words.
pub fn past_radius(body: &crate::Shape, faces: &[u32], dist: f64) -> Option<f64> {
    let places = body.face_places();
    faces.iter().find_map(|f| {
        let (origin, dir, r) = body.face_cylinder(*f)?;
        let at = places.iter().find(|p| p.id == *f)?;
        let v = [at.at[0] - origin[0], at.at[1] - origin[1], at.at[2] - origin[2]];
        let along = v[0] * dir[0] + v[1] * dir[1] + v[2] * dir[2];
        let radial = [v[0] - dir[0] * along, v[1] - dir[1] * along, v[2] - dir[2] * along];
        let outward = at.normal[0] * radial[0] + at.normal[1] * radial[1] + at.normal[2] * radial[2] > 0.0;
        let towards_axis = if outward { -dist } else { dist };
        (towards_axis >= r - 1e-9).then_some(r)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// THE WAY A ROUND FACE MOVES IS ITS OWN: the outside of a shaft of radius 10 goes in at a distance below zero, the
    /// wall of a hole of radius 5 at a distance above it; each is refused at its radius and taken short of it.
    #[test]
    fn a_round_face_is_refused_moved_past_its_radius_whichever_way_it_faces() {
        let shaft = crate::Shape::cylinder(10.0, 10.0).expect("a shaft");
        let side = |s: &crate::Shape| s.face_places().into_iter().find(|p| s.face_cylinder(p.id).is_some()).map(|p| p.id).expect("a round face");
        let outside = side(&shaft);
        assert_eq!(past_radius(&shaft, &[outside], -10.0), Some(10.0), "the shaft moved in by its radius");
        assert_eq!(past_radius(&shaft, &[outside], -9.9), None, "the shaft moved in short of its radius");
        assert_eq!(past_radius(&shaft, &[outside], 30.0), None, "the shaft moved out");
        let block = crate::Shape::extrude(&[-20.0, -20.0, 20.0, -20.0, 20.0, 20.0, -20.0, 20.0], 10.0).expect("a block");
        let pin = crate::Shape::cylinder(5.0, 10.0).expect("a pin");
        let holed = block.boolean(&pin, 0).expect("a block with a hole");
        let wall = side(&holed);
        assert_eq!(past_radius(&holed, &[wall], 5.0), Some(5.0), "the wall of the hole moved towards its axis by its radius");
        assert_eq!(past_radius(&holed, &[wall], -30.0), None, "the wall of the hole moved away from its axis");
    }
}
