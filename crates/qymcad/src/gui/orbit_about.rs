//! WHAT THE VIEW TURNS ABOUT, by the setting: the point of the model in the middle of the view, or under the pointer
//! where the turn begins (`OrbitAbout`) - `look_at_point::point_under` either way, so over empty space it is a point at
//! the depth of the part seen nearest. The point is taken once, when the turn begins, and held for as long as it lasts -
//! a point taken anew every frame would be a point under a pointer that moves, and the view would swim.
//!
//! THE MIDDLE OF THE VIEW IS A POINT OF THE MODEL TOO. The view used to turn about its centre as it stood, and after a
//! view framed at 4 m was wheeled down to a part the centre stood metres behind or before it: the part swung out of
//! the frame at the first pixel of turning. A point on the line through the middle of the view keeps that line through
//! it as the view turns, so the middle of the view stays the middle.
use egui::{Rect, Response};

/// Where the turn of this frame is about: `None` while the view is not turning.
pub(crate) fn pivot(pn: &qymcad_ui_state::Painting, rect: Rect, ctx: &egui::Context, resp: &Response) -> Option<[f64; 3]> {
    let id = egui::Id::new("orbit-about-pivot");
    let turning = pn.set.mouse_nav.rotates().iter().chain(pn.set.mouse_nav.tilts().iter()).any(|g| g.active(ctx, resp));
    if !turning {
        ctx.data_mut(|d| d.remove::<[f64; 3]>(id));
        return None;
    }
    if let Some(p) = ctx.data(|d| d.get_temp::<[f64; 3]>(id)) {
        return Some(p);
    }
    // where the turn began: the press of a drag, or the pointer where a turn without a button starts moving
    let at = match pn.set.orbit_about {
        qymcad_ui_state::OrbitAbout::ViewCentre => rect.center(),
        qymcad_ui_state::OrbitAbout::Pointer => ctx.input(|i| i.pointer.press_origin()).or_else(|| resp.hover_pos())?,
    };
    let p = super::look_at_point::point_under(pn, rect, at);
    ctx.data_mut(|d| d.insert_temp(id, p));
    Some(p)
}

/// THE VIEW MOVED SIDEWAYS by the gesture `pan` of this frame, the point of the model grabbed where the move began
/// following the pointer.
///
/// The centre is moved in its own plane, which carries along exactly the points at its depth. In perspective a point
/// nearer or further moved by more or by less than the pointer, and after a view wheeled in from far the grabbed part
/// slid away under the hand. So when a move begins the centre is slid along the line of sight to the depth of the point
/// grabbed, which changes nothing on screen (`centre_at_depth_of`).
pub(crate) fn panned(pn: &qymcad_ui_state::Painting, rect: Rect, ctx: &egui::Context, resp: &Response, pan: &qymcad_ui_state::Gesture) -> qymcad_ui_state::Cam3 {
    let mut cam = pn.cam;
    let id = egui::Id::new("pan-grip");
    let began = ctx.input(|i| i.pointer.press_origin());
    if began.is_some() && ctx.data(|d| d.get_temp::<Option<egui::Pos2>>(id)) != Some(began) {
        ctx.data_mut(|d| d.insert_temp(id, began));
        if let Some(at) = began {
            qymcad_ui_state::centre_at_depth_of(&mut cam, pn.set, rect.height() * 0.5, super::look_at_point::point_under(pn, rect, at));
        }
    }
    let d = qymcad_ui_state::nav_delta(ctx, resp, pan);
    let (right, up, _) = cam.basis();
    let k = 1.0 / cam.scale as f64;
    for a in 0..3 {
        cam.target[a] += (-(d.x as f64) * right[a] + d.y as f64 * up[a]) * k;
    }
    cam
}
