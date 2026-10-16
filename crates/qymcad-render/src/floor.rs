//! THE FLOOR IN THE SOFTWARE RASTER: the plane Z = 0 under its grid and the world axes, worked out row by row
//! (`qymcad_ui_state::FloorRows`) into a picture of its own, kept while the camera, the frame and the colours stay, and
//! laid only where no solid body is - the floor lies behind every body, seen from above or from below. It comes after
//! the solid bodies and before the ghosts, as on the card.

use super::*;
use std::hash::{Hash, Hasher};

/// The floor of one frame: its rays, its look, and the size of the picture in pixels.
pub struct FloorFrame<'a> {
    pub rays: &'a ViewRays,
    pub look: &'a FloorLook,
    pub size: [usize; 2],
}

impl FloorFrame<'_> {
    /// WHAT THE PICTURE OF THE FLOOR DEPENDS ON, as one number: the camera and its projection, the frame, the colours.
    /// The model is not in it.
    pub fn key(&self) -> u64 {
        let (r, l) = (self.rays, self.look);
        let mut h = std::collections::hash_map::DefaultHasher::new();
        for v in [r.target, r.right, r.up, r.fwd].iter().flatten().chain(&[r.scale, r.ppp, r.proj.inv_d_eye, r.proj.z_near, l.step, l.reach, l.centre[0], l.centre[1]]) {
            v.to_bits().hash(&mut h);
        }
        for v in [r.half.x, r.half.y, r.pixel.x, r.pixel.y] {
            v.to_bits().hash(&mut h);
        }
        (self.size, l.minor, l.major, l.axis_x, l.axis_y, l.axis_z).hash(&mut h);
        h.finish()
    }

    /// The picture of the floor, kept in `cache` under its key: worked out again only when no picture of that key is
    /// kept, by bands of rows on up to eight threads, as the bodies are. The latest `FLOOR_KEPT` are kept.
    pub fn picture<'c>(&self, cache: &'c std::cell::RefCell<Vec<qymcad_ui_state::Cached<Vec<Color32>>>>) -> std::cell::Ref<'c, Vec<Color32>> {
        let key = self.key();
        let [w, h] = self.size;
        {
            let mut kept = cache.borrow_mut();
            match kept.iter().position(|c| c.rev == key && c.value.len() == w * h) {
                Some(at) => {
                    let found = kept.remove(at);
                    kept.insert(0, found);
                }
                None => {
                    let mut value = vec![Color32::TRANSPARENT; w * h];
                    let rows = FloorRows::new(self.rays, self.look);
                    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1).clamp(1, 8);
                    let band = h.div_ceil(threads).max(1);
                    std::thread::scope(|s| {
                        for (k, chunk) in value.chunks_mut(band * w.max(1)).enumerate() {
                            let rows = &rows;
                            s.spawn(move || {
                                for (i, row) in chunk.chunks_mut(w.max(1)).enumerate() {
                                    rows.row(k * band + i, row);
                                }
                            });
                        }
                    });
                    kept.insert(0, qymcad_ui_state::Cached { rev: key, value });
                    kept.truncate(FLOOR_KEPT);
                }
            }
        }
        std::cell::Ref::map(cache.borrow(), |kept| &kept[0].value)
    }
}

/// How many pictures of the floor are kept: the sizes the canvas takes as panels and windows open and close.
const FLOOR_KEPT: usize = 4;

/// THE FLOOR LAID UNDER THE BODIES in a band of the picture: where the depth buffer `zbuf` is still empty, `floor` - the
/// same band of the picture of the floor - goes over what is there.
pub fn lay_floor(color: &mut [Color32], zbuf: &[f32], floor: &[Color32]) {
    for ((px, z), f) in color.iter_mut().zip(zbuf).zip(floor) {
        if *z != f32::INFINITY || f.a() == 0 {
            continue; // a body is drawn here, or the floor shows nothing
        }
        let (top, under) = (f.to_array(), px.to_array());
        let k = 255 - top[3] as u32;
        let ch = |c: usize| (top[c] as u32 + (under[c] as u32 * k + 127) / 255).min(255) as u8;
        *px = Color32::from_rgba_premultiplied(ch(0), ch(1), ch(2), ch(3));
    }
}
