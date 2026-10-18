//! THE NAMES OF THE FACES A PROFILE OPERATION MAKES - an extrusion, a cut or a boss, a revolve, a sweep, a loft: a side
//! face after the piece of the sketch entity its edge swept, the caps after the node. The model hands the sources of
//! the sides (`Sides`); the rule is here.

use crate::door::Roles;
use qymcad_core::feature::Sides;
use qymcad_core::geom::EDGE_FIELDS;
use qymcad_core::model::Id;
use qymcad_core::names::{GeoName, Role};

/// PROFILES NAMED: the profiles with the numbers of their side roles in the slots of the names of their edges, the
/// caps as the C++ side reads them (triples of [region key, start, end]), and the roles.
pub struct NamedSides {
    pub profiles: Vec<Vec<f64>>,
    pub caps: Vec<u32>,
    pub roles: Roles,
}

/// THE SIDES AND CAPS OF A PROFILE OPERATION. A side face is `role` of the node from the source of its edge (`Wall`
/// for an extrusion, `Revolved`, `Swept`, `Lofted`). One region needs no key: its caps are the start and the end of
/// the node. Several regions make several faces, and one cap name for all would be two faces under one name; so the
/// caps of a region are named after the profile it is drawn from - the contour the person picked (`Sides::regions`).
/// The profiles picked merge into faces before the sweep, and a side two of them share is gone from the face they
/// make; so every wall offers the caps of its profile (`[wall, start, end]` for each), and the C++ side takes those of
/// the lowest wall the face still has. The walls are numbered by their profile first, then by their recipe - the sketch
/// entity, the piece - so a face merged from several profiles takes the caps of the lowest of them, whatever entities
/// bound it. Measured with the caps named after the lowest wall: a dimension that cut a side of the sketch in three
/// pieces made it the lowest wall of three regions, and their caps, which did not move, were renamed. A cap name a face
/// does not take is not written into the table.
pub fn name_sides(profiles: &[Vec<f64>], sides: Sides<'_>, role: Role) -> NamedSides {
    let mut roles = Roles::default();
    // the profile a source stands in, the lowest by its contour where a side is shared
    let region_of = |i: usize| sides.regions.get(i).copied().unwrap_or(i as Id + 1);
    let mut owner: Vec<Id> = vec![Id::MAX; sides.sources.len()];
    for (i, prof) in profiles.iter().enumerate() {
        for place in loop_places(prof) {
            if let Some(o) = place.checked_sub(1).and_then(|k| owner.get_mut(k)) {
                *o = (*o).min(region_of(i));
            }
        }
    }
    let mut order: Vec<usize> = (0..sides.sources.len()).collect();
    order.sort_by_key(|&k| (owner[k], sides.sources[k].src, sides.sources[k].split));
    let mut numbers: Vec<u32> = vec![0; sides.sources.len()];
    for k in order {
        let s = sides.sources[k];
        numbers[k] = roles.give(GeoName { feature: sides.feature, role, src: s.src, split: s.split });
    }
    let mut rewritten: Vec<Vec<f64>> = profiles.to_vec();
    let mut walls: Vec<u32> = Vec::new();
    for prof in rewritten.iter_mut() {
        let loops = prof.first().copied().unwrap_or(0.0) as usize;
        walls.extend(rename_loops(prof, 1, loops, &numbers));
    }
    let mut caps: Vec<u32> = Vec::new();
    if profiles.len() == 1 {
        caps.extend([0, roles.give(GeoName::new(sides.feature, Role::CapStart, 0)), roles.give(GeoName::new(sides.feature, Role::CapEnd, 0))]);
    } else {
        walls.sort_unstable();
        walls.dedup();
        for wall in walls {
            let Some(k) = numbers.iter().position(|n| *n == wall) else { continue };
            let start = roles.spare(GeoName::new(sides.feature, Role::CapStart, owner[k]));
            let end = roles.spare(GeoName::new(sides.feature, Role::CapEnd, owner[k]));
            caps.extend([wall, start, end]);
        }
    }
    NamedSides { profiles: rewritten, caps, roles }
}

/// The places of the sources in the slots of the edges of a profile - `[loops, then loop blocks of [n, n edges of
/// EDGE_FIELDS]]` - counted from 1, 0 for an edge with no source.
fn loop_places(data: &[f64]) -> Vec<usize> {
    let mut out = Vec::new();
    let mut i = 1;
    for _ in 0..data.first().copied().unwrap_or(0.0) as usize {
        let n = data.get(i).copied().unwrap_or(0.0) as usize;
        i += 1;
        for k in 0..n {
            out.push(data.get(i + k * EDGE_FIELDS + EDGE_FIELDS - 1).copied().unwrap_or(0.0) as usize);
        }
        i += n * EDGE_FIELDS;
    }
    out
}

/// THE SECTIONS OF A LOFT NAMED: the faces between sections after the edges of the first, the caps after the node.
pub struct NamedSections {
    pub data: Vec<f64>,
    pub caps: [u32; 2],
    pub roles: Roles,
}

/// The loop blocks of the sections one after another (`offsets` marks where each starts), their slots renamed.
pub fn name_sections(data: &[f64], offsets: &[usize], sides: Sides<'_>) -> NamedSections {
    let mut roles = Roles::default();
    let numbers: Vec<u32> = sides.sources.iter().map(|s| roles.give(GeoName { feature: sides.feature, role: Role::Lofted, src: s.src, split: s.split })).collect();
    let mut rewritten = data.to_vec();
    for start in offsets.iter().take(offsets.len().saturating_sub(1)) {
        rename_loops(&mut rewritten, *start, 1, &numbers);
    }
    let caps = [roles.give(GeoName::new(sides.feature, Role::CapStart, 0)), roles.give(GeoName::new(sides.feature, Role::CapEnd, 0))];
    NamedSections { data: rewritten, caps, roles }
}

/// Rename the slots of `loops` loop blocks starting at `at` - `[n, n edges of EDGE_FIELDS]` each - from the place of a
/// source (from 1) to the number of its role, giving the numbers the edges took.
fn rename_loops(data: &mut [f64], at: usize, loops: usize, numbers: &[u32]) -> Vec<u32> {
    let mut taken: Vec<u32> = Vec::new();
    let mut i = at;
    for _ in 0..loops {
        let n = data.get(i).copied().unwrap_or(0.0) as usize;
        i += 1;
        for k in 0..n {
            let slot = i + k * EDGE_FIELDS + EDGE_FIELDS - 1;
            let Some(v) = data.get_mut(slot) else { continue };
            let place = *v as usize;
            let number = if place == 0 { 0 } else { numbers.get(place - 1).copied().unwrap_or(0) };
            *v = number as f64;
            if number != 0 {
                taken.push(number);
            }
        }
        i += n * EDGE_FIELDS;
    }
    taken
}

/// WHERE A TOOL REACHES once its ends are met with the body it cuts: from `start` for `length` along the normal of its
/// sketch.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reach {
    pub start: f64,
    pub length: f64,
}

/// THE ENDS OF A ONE-SIDED CUT MET WITH THE BODY IT CUTS. An end of the tool coincident with a face of the body leaves
/// a cap behind and the hole comes out closed - OCCT does not cut through a face it only touches. So an end that almost
/// reaches the far side of the body (to a thousandth of its span, at least 1e-4) is carried past it by 1 % of the depth
/// (at least 0.05): a cut exactly as deep as the wall becomes a through cut, while an honest pocket keeps its depth. And
/// an end on the sketch plane - which usually lies on a face of the body - is set back by a micrometre: 0.1 mm would
/// shave a visible step where there is material behind the plane, 1e-3 removes the coplanarity and is not seen.
pub fn meet_ends(span: qymcad_core::feature::ToolSpan, body: Option<&crate::Shape>, place: &[f64; 12]) -> Reach {
    use qymcad_core::feature::ToolEnds;
    let (mut lo, mut hi) = (span.start, span.start + span.length);
    if span.ends == ToolEnds::OneSidedCut {
        let depth = span.length.abs();
        let eps = (depth * 0.01).max(0.05);
        if let Some(along) = body.and_then(|b| span_along(b, place)) {
            let touch = ((along.max - along.min).abs() * 1e-3).max(1e-4);
            if hi > 1e-9 && (hi - along.max).abs() <= touch {
                hi = along.max + eps;
            }
            if lo < -1e-9 && (lo - along.min).abs() <= touch {
                lo = along.min - eps;
            }
        }
        let seam = 1.0e-3;
        if lo.abs() < 1e-9 {
            lo -= seam;
        }
        if hi.abs() < 1e-9 {
            hi += seam;
        }
    }
    Reach { start: lo, length: hi - lo }
}

/// How far a body reaches along the normal of a placement, from its origin.
struct Along {
    min: f64,
    max: f64,
}

fn span_along(body: &crate::Shape, place: &[f64; 12]) -> Option<Along> {
    let b = body.bbox()?;
    let (n, o) = ([place[2], place[6], place[10]], [place[3], place[7], place[11]]);
    let corners = (0..8).map(|k| [if k & 1 == 0 { b[0] } else { b[3] }, if k & 2 == 0 { b[1] } else { b[4] }, if k & 4 == 0 { b[2] } else { b[5] }]);
    let along: Vec<f64> = corners.map(|c| (c[0] - o[0]) * n[0] + (c[1] - o[1]) * n[1] + (c[2] - o[2]) * n[2]).collect();
    Some(Along { min: along.iter().copied().fold(f64::MAX, f64::min), max: along.iter().copied().fold(f64::MIN, f64::max) })
}

/// The placement moved by `by` along its normal.
pub fn shifted(place: [f64; 12], by: f64) -> [f64; 12] {
    let mut pl = place;
    if by.abs() > 1e-12 {
        pl[3] += pl[2] * by;
        pl[7] += pl[6] * by;
        pl[11] += pl[10] * by;
    }
    pl
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::door::RoleRecipe;
    use qymcad_core::feature::SideSource;

    /// A loop block of `n` edges whose slots hold `places`.
    fn block(places: &[u32]) -> Vec<f64> {
        let mut v = vec![places.len() as f64];
        for p in places {
            let mut e = vec![0.0; EDGE_FIELDS];
            e[EDGE_FIELDS - 1] = *p as f64;
            v.extend(e);
        }
        v
    }

    /// A ONE-SIDED CUT MEETS THE BODY: its end on the sketch plane is set back a micrometre, an end a hair from the far
    /// side of the body is carried past it; a cut kept as drawn is left alone.
    #[test]
    fn a_one_sided_cut_meets_the_body() {
        use qymcad_core::feature::{ToolEnds, ToolSpan};
        let block = crate::Shape::extrude(&[0.0, 0.0, 10.0, 0.0, 10.0, 10.0, 0.0, 10.0], 5.0).expect("a block 5 tall");
        let up = qymcad_core::feature::PLACE_IDENTITY;
        let through = meet_ends(ToolSpan { start: 0.0, length: 5.0, ends: ToolEnds::OneSidedCut }, Some(&block), &up);
        assert!(through.start < 0.0 && through.start > -2e-3, "the end on the sketch plane is not set back a micrometre: {through:?}");
        assert!(through.start + through.length > 5.0 + 1e-3, "a cut as deep as the block is not carried through it: {through:?}");
        let pocket = meet_ends(ToolSpan { start: 0.0, length: 3.0, ends: ToolEnds::OneSidedCut }, Some(&block), &up);
        assert!((pocket.start + pocket.length - 3.0).abs() < 1e-9, "an honest pocket lost its depth: {pocket:?}");
        let kept = meet_ends(ToolSpan { start: 0.0, length: 5.0, ends: ToolEnds::Kept }, Some(&block), &up);
        assert_eq!(kept, Reach { start: 0.0, length: 5.0 }, "a tool kept as drawn was moved");
    }

    /// Two regions: every wall offers the caps of its profile, named after the contour the profile is drawn from, the
    /// walls numbered by their profile and then by their recipe; one region gets the plain caps of the node.
    #[test]
    fn caps_of_several_regions_are_named_after_their_profiles() {
        let sources = [SideSource { src: 13, split: 0 }, SideSource { src: 11, split: 1 }, SideSource { src: 11, split: 0 }];
        let one = name_sides(&[[vec![1.0], block(&[1, 2])].concat()], Sides { feature: 5, sources: &sources, regions: &[40] }, Role::Wall);
        assert_eq!(one.caps[0], 0, "one region has no key");
        // the profile of contour 40 holds sources 1 and 2 (13/0, 11/1), the profile of contour 30 holds source 3 (11/0)
        let two = name_sides(&[[vec![1.0], block(&[1, 2])].concat(), [vec![1.0], block(&[3])].concat()], Sides { feature: 5, sources: &sources, regions: &[40, 30] }, Role::Wall);
        let walls_of = |p: &Vec<f64>| -> Vec<u32> { (0..p[1] as usize).map(|k| p[2 + k * EDGE_FIELDS + EDGE_FIELDS - 1] as u32).collect() };
        let [first, second] = [&two.profiles[0], &two.profiles[1]].map(walls_of);
        assert!(second[0] < first[1] && first[1] < first[0], "the walls are not numbered by profile, then by recipe (30: 11/0; 40: 11/1, 13/0): {first:?} {second:?}");
        let named = |number: u32| two.roles.given.iter().find(|r| r.number == number).map(|r| r.recipe.clone());
        let cap_of = |wall: u32| two.caps.chunks(3).find(|c| c[0] == wall).map(|c| c[1]).and_then(named);
        assert_eq!(cap_of(second[0]), Some(RoleRecipe::Spare(GeoName::new(5, Role::CapStart, 30))), "the cap of a wall is not named after its profile");
        assert_eq!(cap_of(first[0]), cap_of(first[1]), "two walls of one profile offer different caps");
    }
}
