//! WHAT A TOOL OF THE WINDOW ASKS THE BRIDGE BEFORE IT LAYS A NODE: how many pieces a plane cuts a body into, whether it
//! divides a face at all, whether a sheet closes what it replaces, whether a boolean gives a body. The window holds the
//! live bodies but builds nothing on them: every question is asked under the lock of the kernel and of copies. OCCT is
//! not safe beside a rebuild running on another thread, and a boolean may nudge its arguments - a question asked of the
//! live body would move the part it is asked about.

use crate::{BoolVerdict, Shape};
use qymcad_core::feature::{PlaneAt, PLACE_IDENTITY};

/// A copy of `body` to ask a question of, its names with it. A body that cannot be copied leaves the question to the
/// rebuild: the answers below that would refuse say nothing then.
fn copy(body: &Shape) -> Option<Shape> {
    body.transformed(&PLACE_IDENTITY)
}

/// How many pieces the plane `at` cuts `body` into; `None` when it does not cut it.
pub fn split_pieces(body: &Shape, at: PlaneAt) -> Option<usize> {
    let _gate = crate::kernel::kernel_gate();
    copy(body)?.split_by_plane(at.origin, at.normal, 0).map(|pieces| pieces.len())
}

/// Whether the plane `at` divides any face of `body`.
pub fn splits_faces(body: &Shape, at: PlaneAt) -> bool {
    let _gate = crate::kernel::kernel_gate();
    copy(body).and_then(|c| c.split_faces(at.origin, at.normal)).is_some()
}

/// Whether `sheet` closes the faces `faces` of `body` it is to replace; the number of the edges it leaves unpaired when
/// it does not.
pub fn sheet_closes(body: &Shape, faces: &[u32], sheet: &Shape) -> Result<(), u32> {
    let _gate = crate::kernel::kernel_gate();
    let (Some(b), Some(s)) = (copy(body), copy(sheet)) else { return Ok(()) };
    b.replace_faces(faces, &s, 1e-4).map(|_| ())
}

/// Whether the boolean `op` (0 subtracts, 1 unites, 2 intersects) of `a` with `b` gives a body; why not when it does not.
pub fn boolean_gives_body(a: &Shape, b: &Shape, op: u8) -> Result<(), BoolVerdict> {
    let _gate = crate::kernel::kernel_gate();
    let (Some(a), Some(b)) = (copy(a), copy(b)) else { return Ok(()) };
    a.boolean_checked(&b, op).map(|_| ())
}

/// THE TRIANGLES OF A BODY TO WRITE INTO A FILE, at `deflection` mm, its faces as one mesh. Asked under the lock of the
/// kernel: the mesher writes its triangles into the body, and a rebuild beside it may be reading the same geometry.
/// `None` when the body does not tessellate.
pub fn export_mesh(body: &Shape, deflection: f64) -> Option<qymcad_core::geom::Built> {
    let _gate = crate::kernel::kernel_gate();
    body.tessellate_merged(deflection)
}

/// A BODY PLACED FOR A QUESTION OF CLASH: which body, the part it belongs to, its live shape and where it stands.
pub struct PlacedBody<'a> {
    pub body: qymcad_core::model::Id,
    pub part: qymcad_core::model::Id,
    pub shape: &'a Shape,
    pub place: [f64; 12],
}

/// TWO BODIES OF DIFFERENT PARTS THAT GO INTO EACH OTHER, or that could not be measured.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Clash {
    pub a: qymcad_core::model::Id,
    pub b: qymcad_core::model::Id,
}

/// WHETHER A QUESTION WAITS FOR THE KERNEL.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wait {
    /// Only when nobody holds it: a window rebuilding its document asks again on a later frame instead of freezing for
    /// the rebuild.
    IfFree,
    /// Until it is free: nothing of the asker's own holds it, and whoever does is quick.
    Always,
}

/// THE PAIRS OF BODIES OF DIFFERENT PARTS THAT GO INTO EACH OTHER by more than 1e-3 mm^3. Bodies of one part overlap as
/// they are made, and are not asked about. A pair the kernel cannot measure is told too: "could not measure" is not
/// "clear of each other", and taking it for the second is how a part ends up inside another with nothing said. Each
/// body is placed on a copy, and the common part is asked under the lock of the kernel; with `Wait::IfFree` there is no
/// answer (`None`) while someone holds it.
pub fn clashes(bodies: &[PlacedBody<'_>], wait: Wait) -> Option<Vec<Clash>> {
    let _gate = match wait {
        Wait::IfFree => crate::kernel::try_kernel_gate()?,
        Wait::Always => crate::kernel::kernel_gate(),
    };
    let placed: Vec<(&PlacedBody<'_>, Shape)> = bodies.iter().filter_map(|b| b.shape.transformed(&b.place).map(|s| (b, s))).collect();
    let mut out = Vec::new();
    for i in 0..placed.len() {
        for j in (i + 1)..placed.len() {
            if placed[i].0.part == placed[j].0.part {
                continue;
            }
            if placed[i].1.interference_volume(&placed[j].1).is_none_or(|v| v > 1e-3) {
                out.push(Clash { a: placed[i].0.body, b: placed[j].0.body });
            }
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 10 x 10 x 10 block with its lower corner at x = `x`.
    fn block(x: f64) -> Shape {
        Shape::extrude(&[0.0, 0.0, 10.0, 0.0, 10.0, 10.0, 0.0, 10.0], 10.0)
            .expect("the block")
            .transformed(&[1.0, 0.0, 0.0, x, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0])
            .expect("the block moved into place")
    }

    /// A CLASH ASKED WHILE A REBUILD HOLDS THE KERNEL is not waited for: there is no answer, and the window asks again on a
    /// later frame. With the kernel free it is answered: two overlapping blocks of two parts clash.
    #[test]
    fn a_clash_is_not_asked_beside_a_rebuild() {
        let (a, b) = (block(0.0), block(5.0));
        let bodies = [PlacedBody { body: 1, part: 10, shape: &a, place: PLACE_IDENTITY }, PlacedBody { body: 2, part: 20, shape: &b, place: PLACE_IDENTITY }];
        {
            let _rebuild = crate::kernel::kernel_gate();
            assert_eq!(clashes(&bodies, Wait::IfFree), None, "a clash was asked while a rebuild held the kernel");
        }
        assert_eq!(clashes(&bodies, Wait::IfFree), Some(vec![Clash { a: 1, b: 2 }]), "two overlapping blocks of two parts do not clash");
    }

    /// THE ANSWERS ARE THOSE OF THE OPERATIONS, and the body asked about is left as it was, byte for byte.
    #[test]
    fn a_question_answers_as_the_operation_and_leaves_the_body_alone() {
        let a = block(0.0);
        let before = a.to_brep_bytes();
        let middle = PlaneAt { origin: [5.0, 0.0, 0.0], normal: [1.0, 0.0, 0.0] };
        let beside = PlaneAt { origin: [50.0, 0.0, 0.0], normal: [1.0, 0.0, 0.0] };
        assert_eq!(split_pieces(&a, middle), Some(2), "a plane through the middle cuts the block in two");
        assert_eq!(split_pieces(&a, beside), None, "a plane beside the block cuts nothing");
        assert!(splits_faces(&a, middle), "a plane through the middle divides faces");
        assert!(!splits_faces(&a, beside), "a plane beside the block divides none");
        assert_eq!(boolean_gives_body(&a, &block(10.0), 2), Err(BoolVerdict::NothingInCommon), "touching blocks have nothing in common");
        assert_eq!(boolean_gives_body(&a, &block(5.0), 0), Ok(()), "an overlapping cut removes something");
        assert_eq!(a.to_brep_bytes(), before, "a question moved the body it was asked about");
    }
}
