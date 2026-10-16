//! A SHELL AND A THICKENING through the bridge: the inner walls named after the faces they come from, the plate and its
//! walls after the faces and edges it was raised from, and the refusals said for what they are.

use crate::door::Roles;
use crate::kernel::OcctKernel;
use crate::ops::faces::Thickened;
use crate::ImageOf;
use qymcad_core::geom::Built;
use qymcad_core::model::Id;

/// A SHELL AS ASKED: the body it makes, the body it hollows, the thickness and which way, the faces it opens.
pub struct ShellAsk<'a> {
    pub body: Id,
    pub src: Id,
    pub thickness: f64,
    pub outward: bool,
    pub faces: &'a [u32],
}

/// A THICKENING AS ASKED: the body it makes, the body the face is taken from, the face, the thickness, and the body the
/// plate joins (0 - none).
pub struct ThickenAsk {
    pub body: Id,
    pub src: Id,
    pub face: u32,
    pub thickness: f64,
    pub join: Id,
}

impl OcctKernel {
    /// A SHELL: the faces opened, walls of the thickness left, each inner wall the image of its outer face.
    pub(crate) fn shell_op(&self, ask: ShellAsk<'_>, walls: &[ImageOf], roles: Roles) -> Result<Built, qymcad_core::errors::CoreError> {
        let ShellAsk { body, src, thickness, outward, faces } = ask;
        self.plain_named(qymcad_core::errors::Op::Shell, body, roles, |b| {
            let s = b.get(src)?;
            // the sign is the direction
            let offset = if outward { thickness.abs() } else { -thickness.abs() };
            // There is nothing to remove: the face reference matched none, so the kernel is never reached, yet an
            // unnamed "the operation failed" was reported. That is not a broken shell but a lost reference, and it
            // has to be named as one.
            if faces.is_empty() {
                return Err(qymcad_core::errors::CoreError::FacesNotFound);
            }
            // A refusal has to name its cause. An offset thicker than the smallest fillet eats it whole and
            // the kernel will not build such a shell; that is what has to be said, not "it failed".
            // A shell that hollowed nothing is not a shell. Measured on a 30x18x12 box: at a thickness of
            // 12 the kernel returned the source body - six faces, area 2232.00, exactly the blank - and the
            // node stood green, while at a thickness of 9 the same thing arrived with an honest
            // `BrokenSolid`. The report said the shell was built and the part stayed solid. So the volume is
            // asked for: a hollow part has strictly less of it than a solid one.
            let vol_was = s.volume();
            Ok(match s.shell(offset, faces, walls) {
                Some(v) if !outward && vol_was > 1e-9 && v.volume() >= vol_was - 1e-6 => {
                    let r = s.min_round_radius();
                    let t = thickness.abs();
                    return Err(if r > 1e-9 && t >= r - 1e-9 {
                        qymcad_core::errors::CoreError::ShellThicknessOverRound { thickness: t, limit: r }
                    } else {
                        qymcad_core::errors::CoreError::ShellNotBuiltHere
                    });
                }
                Some(v) => v,
                None => {
                    // Try first, then judge. The check for a body of several shells used to stand before
                    // the attempt and kept the kernel from even starting, though it can shell each body
                    // separately. A refusal is named only after a real failure.
                    let shells = s.shell_count();
                    let r = s.min_round_radius();
                    let t = thickness.abs();
                    return Err(if r > 1e-9 && t >= r - 1e-9 {
                        qymcad_core::errors::CoreError::ShellThicknessOverRound { thickness: t, limit: r }
                    } else if shells > 1 {
                        qymcad_core::errors::CoreError::ShellOfMultiShellBody { shells }
                    } else {
                        qymcad_core::errors::CoreError::ShellNotBuiltHere
                    });
                }
            })
        })
    }

    /// A THICKENING: a plate raised from the face, joined to the part it was taken from.
    pub(crate) fn thicken_op(&self, ask: ThickenAsk, names: &Thickened, roles: Roles) -> Result<Built, qymcad_core::errors::CoreError> {
        let ThickenAsk { body, src, face, thickness, join } = ask;
        self.plain_named(qymcad_core::errors::Op::Thicken, body, roles, |b| {
            let s = b.get(src)?;
            // A refusal names itself. Both possible failures used to look the same, as "the operation
            // failed", leaving no way to tell an excessive thickness from a plate that did not join.
            let plate = s.thicken_face_join(face, thickness, &names.faces, &names.edges).ok_or(qymcad_core::errors::CoreError::ThickenFaceRefused)?;
            // The plate returns into the part the surface was taken from. Otherwise a second body stays in
            // the part - a differently coloured piece on screen - breaking the rule that a part is one body.
            let res = match join {
                0 => plate,
                j => {
                    let base = b.get(j)?;
                    base.boolean(&plate, 1).ok_or(qymcad_core::errors::CoreError::ThickenPlateNotJoined)?
                }
            };
            // a plate grown into the body adds nothing: the top of a block thickened -2 stood green, the body as it
            // was
            if !s.is_sheet() && res.volume() <= s.volume() + 1e-6 {
                return Err(qymcad_core::errors::CoreError::ThickenAddedNothing);
            }
            Ok(res)
        })
    }
}
