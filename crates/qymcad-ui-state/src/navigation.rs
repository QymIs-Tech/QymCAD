//! MOVING THE VIEW: zoom, pan, orbit, the latches of the wheel.

use super::*;

/// WHERE THE VIEW ZOOMS FROM.
///
/// Reported behaviour: "zooming pans from the origin rather than from the cursor's coordinates - both in
/// sketches and in the 3D viewport. Two variants are needed, in the settings."
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum ZoomAt {
    /// The point under the cursor stays put - the point one is looking at is the point one aims at.
    Cursor,
    /// The middle of the viewport stays put, whatever the cursor is over.
    ViewCentre,
}

impl ZoomAt {
    pub const ALL: [ZoomAt; 2] = [ZoomAt::Cursor, ZoomAt::ViewCentre];

    pub fn key(self) -> &'static str {
        match self {
            ZoomAt::Cursor => "settings-zoom-at-cursor",
            ZoomAt::ViewCentre => "settings-zoom-at-centre",
        }
    }
}

/// WHAT THE VIEW TURNS ABOUT.
///
/// The middle of the view is the program's own way and stays the default. Turning about the point of the model under
/// the pointer is the other habit, asked for in a pull request: the place one is looking at stays where it is while
/// the rest turns around it. A list rather than a pair of buttons, so another centre - the model's, a chosen element -
/// is one more variant.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum OrbitAbout {
    /// The middle of the view, whatever the pointer is over.
    ViewCentre,
    /// The point under the pointer where the turn begins: the face hit there, else the plane through the middle of the
    /// view square to it. Held for as long as the turn lasts.
    Pointer,
}

impl OrbitAbout {
    pub const ALL: [OrbitAbout; 2] = [OrbitAbout::ViewCentre, OrbitAbout::Pointer];

    pub fn key(self) -> &'static str {
        match self {
            OrbitAbout::ViewCentre => "settings-orbit-about-centre",
            OrbitAbout::Pointer => "settings-orbit-about-pointer",
        }
    }

    /// What the variant does, said under the list.
    pub fn hint_key(self) -> &'static str {
        match self {
            OrbitAbout::ViewCentre => "settings-orbit-about-centre-hint",
            OrbitAbout::Pointer => "settings-orbit-about-pointer-hint",
        }
    }
}

/// HOW OFTEN THE PROGRAM ASKS WHETHER A NEWER VERSION EXISTS.
///
/// A program that never says so leaves people on the version they installed. Measured in September 2026:
/// 470 downloads against 10-20 people in the chat - so almost everybody running it is reachable by
/// nothing, and every fix made since their download is invisible to them.
///
/// The other side is a program that goes to the network without being asked, and that is why this is a
/// setting with `Never` in it rather than a fact of life.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum UpdateCheck {
    /// At every start.
    AtStart,
    /// At a start, but no oftener than once a day. The default: a release may appear any day, and one
    /// request a day is nothing to anybody.
    Daily,
    /// At a start, no oftener than once a week.
    Weekly,
    /// At a start, no oftener than once a month.
    Monthly,
    /// Never on its own. The menu item still works: a person who presses it is asking.
    Never,
}

impl UpdateCheck {
    pub const ALL: [UpdateCheck; 5] = [UpdateCheck::AtStart, UpdateCheck::Daily, UpdateCheck::Weekly, UpdateCheck::Monthly, UpdateCheck::Never];

    pub fn key(self) -> &'static str {
        match self {
            UpdateCheck::AtStart => "settings-updates-at-start",
            UpdateCheck::Daily => "settings-updates-daily",
            UpdateCheck::Weekly => "settings-updates-weekly",
            UpdateCheck::Monthly => "settings-updates-monthly",
            UpdateCheck::Never => "settings-updates-never",
        }
    }

    /// How long to wait between checks, in seconds. `None` means never on its own.
    pub(crate) fn every(self) -> Option<u64> {
        match self {
            UpdateCheck::AtStart => Some(0),
            UpdateCheck::Daily => Some(24 * 60 * 60),
            UpdateCheck::Weekly => Some(7 * 24 * 60 * 60),
            UpdateCheck::Monthly => Some(30 * 24 * 60 * 60),
            UpdateCheck::Never => None,
        }
    }
}

/// IS IT TIME TO ASK, given when it was last asked. Both times are seconds since the epoch.
///
/// A CLOCK THAT WENT BACKWARDS ASKS AGAIN. The last time is written by the program itself, and a machine
/// whose clock was wrong, or that came back from suspend across a change of timezone, can hold a "last
/// checked" in the future. Waiting for it to come round would mean a copy that never checks again and
/// never says why, which is the worst of the outcomes here - so an impossible answer asks now.
pub fn update_check_due(how_often: UpdateCheck, last_checked: u64, now: u64) -> bool {
    let Some(every) = how_often.every() else { return false };
    if last_checked == 0 || last_checked > now {
        return true; // never asked, or a clock that cannot be believed
    }
    now - last_checked >= every
}

/// WHERE THE VIEW ZOOMS FROM WHILE A COMMAND IS OPEN.
///
/// Reported behaviour: "and with a tool's popup open, panning should go from the centre of the part (put
/// that in the settings too)."
///
/// A command's fields stand AT THE GEOMETRY, and zooming to the cursor drags that geometry - and the
/// fields with it - out from under the hand that is typing into them. Holding the part still keeps the
/// popup where it was put.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum ZoomWhileEditing {
    /// The middle of the part being edited stays put.
    PartCentre,
    /// No exception: the ordinary rule applies while a command is open too.
    AsUsual,
}

impl ZoomWhileEditing {
    pub const ALL: [ZoomWhileEditing; 2] = [ZoomWhileEditing::PartCentre, ZoomWhileEditing::AsUsual];

    pub fn key(self) -> &'static str {
        match self {
            ZoomWhileEditing::PartCentre => "settings-zoom-editing-part",
            ZoomWhileEditing::AsUsual => "settings-zoom-editing-usual",
        }
    }
}

/// ZOOM THE FLAT VIEW, KEEPING `anchor` WHERE IT IS ON SCREEN.
///
/// The scale alone was changed, which keeps the MIDDLE of the viewport still: whatever a person is looking
/// at slides away from the cursor as the view grows, and to reach a corner one zooms and then pans it back.
/// Keeping the anchor still is one arithmetic step: the world point under it must read the same after the
/// change as before, and `to_world` says what that costs.
pub fn zoom_view_2d(view: &mut View2d, rect: Rect, anchor: Pos2, factor: f32, lo: f32, hi: f32) {
    let before = view.scale;
    view.scale = (view.scale * factor).clamp(lo, hi);
    if (view.scale - before).abs() < f32::EPSILON {
        return; // at the limit: nothing moved, so nothing has to be compensated
    }
    let c = rect.center();
    let k = 1.0 / before - 1.0 / view.scale;
    view.center.x += (anchor.x - c.x) * k;
    view.center.y -= (anchor.y - c.y) * k;
}

/// ZOOM THE 3D VIEW, KEEPING `anchor` WHERE IT IS ON SCREEN.
///
/// The world point held still is the one lying in the plane through the camera's target, which is the plane
/// a CAD orbit turns about; under perspective a point off that plane drifts by the difference in
/// foreshortening, and that is invisible next to the zoom itself.
pub fn zoom_cam_3d(cam: &mut Cam3, rect: Rect, anchor: Pos2, factor: f32, lo: f32, hi: f32) {
    let before = cam.scale;
    cam.scale = (cam.scale * factor).clamp(lo, hi);
    if (cam.scale - before).abs() < f32::EPSILON {
        return;
    }
    let c = rect.center();
    let (bx, by, _) = cam.basis();
    let k = (1.0 / before - 1.0 / cam.scale) as f64;
    let (du, dv) = (((anchor.x - c.x) as f64) * k, (-(anchor.y - c.y) as f64) * k);
    for a in 0..3 {
        cam.target[a] += bx[a] * du + by[a] * dv;
    }
}

/// WHERE THE VIEW MUST HOLD STILL WHILE IT IS ZOOMED: the cursor, the middle of the part being edited, or
/// the middle of the viewport.
///
/// One place decides it for both viewports - the flat one and the 3D one - because it is one rule and a
/// second copy of it would drift on the first change.
pub fn zoom_anchor(set: &Settings, rect: Rect, cursor: Option<Pos2>, part_centre: Option<Pos2>) -> Pos2 {
    if let (ZoomWhileEditing::PartCentre, Some(p)) = (set.zoom_editing, part_centre) {
        return p; // a command is open and its fields stand at the geometry: hold the geometry still
    }
    match set.zoom_at {
        ZoomAt::Cursor => cursor.unwrap_or_else(|| rect.center()),
        ZoomAt::ViewCentre => rect.center(),
    }
}

/// THE MOVEMENT OF A GESTURE THIS FRAME. The raw pointer delta is for the buttonless gesture alone: a touchpad layout
/// moves the view with no button held, so egui reports no drag and there is nothing else to ask. Reaching for it
/// whenever `drag_delta` came back zero was wrong: dragging an open window by its title bar moved the pointer, and the
/// camera turned along with the window.
pub fn nav_delta(ctx: &egui::Context, resp: &egui::Response, g: &Gesture) -> egui::Vec2 {
    if g.buttons.is_empty() && !g.any_button {
        ctx.input(|i| i.pointer.delta())
    } else {
        resp.drag_delta()
    }
}

/// THE GESTURE OF THE LAYOUT MOVING THE VIEW SIDEWAYS THIS FRAME, if one is made: one of its own, or the left button
/// held still and then led under Gesture (its program pans so after a long press) - and none while the middle button's
/// zoom is latched under CAD.
pub fn pan_now(nav: MouseNav, ctx: &egui::Context, resp: &egui::Response) -> Option<&'static Gesture> {
    if zoom_latched(ctx) {
        return None;
    }
    if nav == MouseNav::Gesture && hold_latched(ctx) && ctx.input(|i| i.pointer.primary_down()) && resp.dragged() {
        return Some(const { &Gesture::of(&[LEFT]) });
    }
    nav.pans().iter().find(|g| g.active(ctx, resp))
}

/// TURN THE VIEW by the layout's gestures this frame: a tilt about the line of sight first, where the layout has one,
/// then a turn - about the middle of the view, or about the world point `about` when one is given.
///
/// ABOUT A POINT THE CENTRE OF THE VIEW TURNS WITH THE CAMERA. The camera turns about its centre as ever, and then the
/// centre is carried by the same turn about the point: the point keeps its place in the camera's own frame, so it
/// stays on its spot of the screen at its depth, in perspective as well, and turning back returns the view to where it
/// was. Sliding the centre in the plane of the screen until the point came back to its spot - the first way this was
/// tried - kept the spot but not the depth, and the centre wandered off a little with every frame: after a few turns
/// at a corner of a long block the model swung round a point hundreds of millimetres away.
pub fn turn_view(about: Option<[f64; 3]>, cam: &mut Cam3, nav: MouseNav, ctx: &egui::Context, resp: &egui::Response) {
    let (before, target) = (cam.basis(), cam.target);
    if let Some(tilt) = nav.tilts().iter().find(|g| g.active(ctx, resp)) {
        cam.roll += nav_delta(ctx, resp, tilt).x as f64 * 0.01;
    } else if let Some(turn) = nav.rotates().iter().find(|g| g.active(ctx, resp)) {
        let d = nav_delta(ctx, resp, turn);
        cam.yaw -= d.x as f64 * 0.01;
        cam.pitch = (cam.pitch + d.y as f64 * 0.01).clamp(-1.5, 1.5);
    }
    if let Some(p) = about {
        cam.target = carried_about(p, target, before, cam.basis());
    }
}

/// THE CENTRE `target` CARRIED BY A TURN OF THE CAMERA ABOUT `p`: the point keeps its coordinates in the camera's frame
/// as the frame goes from `before` to `after`.
pub fn carried_about(p: [f64; 3], target: [f64; 3], before: ([f64; 3], [f64; 3], [f64; 3]), after: ([f64; 3], [f64; 3], [f64; 3])) -> [f64; 3] {
    let rel = v_sub(p, target);
    let (c0, c1, c2) = (v_dot(rel, before.0), v_dot(rel, before.1), v_dot(rel, before.2));
    std::array::from_fn(|a| p[a] - (c0 * after.0[a] + c1 * after.1[a] + c2 * after.2[a]))
}

/// Where the long press of the Gesture layout is kept in the frame's memory: when and where the left button went down,
/// and whether the press has become a hold.
pub(crate) fn hold_latch_id() -> egui::Id {
    egui::Id::new("nav_hold_latch")
}

/// THE LONG PRESS OF THE GESTURE LAYOUT: the left button held still (6 px) for 0.63 s - its program's hold, 0.9 of the
/// system's 0.7 s - and led from then on moves the view instead of turning it, until the button goes up. Called once a
/// frame; answers whether the press is a hold.
pub fn latch_hold(ctx: &egui::Context, nav: MouseNav) -> bool {
    if nav != MouseNav::Gesture {
        return false;
    }
    let id = hold_latch_id();
    let (mut since, mut on): (Option<(f64, egui::Pos2)>, bool) = ctx.data(|d| d.get_temp(id)).unwrap_or((None, false));
    let (down, now, at, pressed) = ctx.input(|i| {
        let pressed = i.events.iter().find_map(|e| match e {
            egui::Event::PointerButton { button: egui::PointerButton::Primary, pressed: true, pos, .. } => Some(*pos),
            _ => None,
        });
        (i.pointer.primary_down(), i.time, i.pointer.latest_pos(), pressed)
    });
    if let Some(p) = pressed {
        since = Some((now, p));
        on = false;
    }
    if !down {
        since = None;
        on = false;
    } else if let (Some((t, p)), false) = (since, on) {
        if at.is_some_and(|a| a.distance(p) > 6.0) {
            since = None; // led before it was held: a turn, as ever
        } else if now - t >= 0.63 {
            on = true;
        }
    }
    ctx.data_mut(|d| d.insert_temp(id, (since, on)));
    on
}

/// Has the left button of the Gesture layout been held long enough to move the view?
pub fn hold_latched(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<(Option<(f64, egui::Pos2)>, bool)>(hold_latch_id())).is_some_and(|(_, on)| on)
}

/// HOW MUCH THE VIEW IS ASKED TO SCALE THIS FRAME, in the wheel's units: the wheel where the layout zooms with it, and
/// the layout's own zoom gesture where it has one - a movement up counts as the wheel turned forward. The touchpad has
/// no wheel and puts the zoom on Ctrl + Shift and a movement; nothing asked for that gesture, and the scale stood at
/// 323.44 before and after. One wheel unit a pixel: 120 px of movement scale the view 1.27 times. The zoom holds the
/// cursor still, and the cursor travels with this gesture: at two units a pixel a framed block's near corner went off
/// the bottom of the canvas (616 -> 780 of 770) on the same 120 px.
/// On the sheet of a sketch (`sheet`) a gesture holding the left button zooms only as a chord of two buttons: the left
/// alone, with Ctrl or Shift, is the sketch's own - it draws, grabs and adds to the selection.
pub fn view_scroll(ctx: &egui::Context, resp: &egui::Response, nav: MouseNav, sheet: bool) -> f32 {
    let wheel = if nav.wheel_zooms() { ctx.input(|i| i.smooth_scroll_delta.y) } else { 0.0 };
    // over the canvas rather than hovered: the click of the latch takes the hover away while the middle button is held
    let _ = latch_hold(ctx, nav); // the long press, looked at once a frame as the latch of the zoom is
    let latched = latch_zoom(ctx, nav) && resp.contains_pointer();
    let usable = |g: &&Gesture| !sheet || g.sheet_may_take();
    let moved = ctx.input(|i| i.pointer.delta());
    let by = -moved.y + if nav.zooms_sideways() { moved.x } else { 0.0 };
    let gesture = if latched || nav.zooms().iter().filter(usable).any(|g| g.active(ctx, resp)) { by } else { 0.0 };
    wheel + gesture
}

/// Where the zoom latched by the middle button is kept in the frame's memory: the time a left or right press began
/// while the middle button was held, and whether the latch is on.
pub(crate) fn zoom_latch_id() -> egui::Id {
    egui::Id::new("nav_zoom_latch")
}

/// THE ZOOM LATCHED BY THE MIDDLE BUTTON, as the CAD layout's program has it: the middle button held (it moves the
/// view), a short click of the left or right button - quicker than a double click - and from then on, until the middle
/// button is let go, moving zooms. Held longer, the left or right button with the middle one turns the model instead.
/// Answers whether the latch is on this frame; called once a frame.
pub fn latch_zoom(ctx: &egui::Context, nav: MouseNav) -> bool {
    if nav != MouseNav::Cad {
        return false;
    }
    let id = zoom_latch_id();
    let (mut pressed_at, mut on): (Option<f64>, bool) = ctx.data(|d| d.get_temp(id)).unwrap_or((None, false));
    let (middle, now, events) = ctx.input(|i| (i.pointer.middle_down(), i.time, i.events.clone()));
    if !middle {
        pressed_at = None;
        on = false;
    } else {
        for e in events {
            if let egui::Event::PointerButton { button: egui::PointerButton::Primary | egui::PointerButton::Secondary, pressed, .. } = e {
                match (pressed, pressed_at) {
                    (true, _) => pressed_at = Some(now),
                    (false, Some(t)) if now - t < 0.5 => {
                        on = true;
                        pressed_at = None;
                    }
                    _ => pressed_at = None,
                }
            }
        }
    }
    ctx.data_mut(|d| d.insert_temp(id, (pressed_at, on)));
    on
}

/// Is the zoom latched by the middle button on now - so that the middle button, which moves the view, zooms instead?
pub fn zoom_latched(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<(Option<f64>, bool)>(zoom_latch_id())).is_some_and(|(_, on)| on)
}

/// WHAT A ZOOM HOLDS STILL: the place on screen, and the point of the model seen there.
#[derive(Clone, Copy, Debug)]
pub struct Held {
    pub anchor: Pos2,
    pub point: [f64; 3],
}

/// THE WHEEL OVER THE 3D VIEWPORT: zoom towards whatever the rule says to hold still (`zoom_anchor`), holding the point of
/// the model seen there.
///
/// The centre of the view is slid along the line of sight to the depth of that point first, which changes nothing on
/// screen, and the zoom then holds the plane through the centre still. Held in the plane of the centre as it stood, a
/// view framed at 4 m and wheeled down to 100 mm at a part kept its centre metres away from the part: in orthographic
/// every turn afterwards swung the part out of the frame, and in perspective the eye, 120 mm from that centre, ended
/// up beyond the part, which went behind it. Reported behaviour: "everything flies off into space, it turns fast and
/// touchy".
pub fn wheel_zoom_3d(cam: &mut Cam3, set: &Settings, rect: Rect, held: Held, scroll: f32) {
    centre_at_depth_of(cam, set, rect.height() * 0.5, held.point);
    // the limits are from where the view was framed, and never tighter than where the view already is: sliding the
    // centre in perspective changes the scale, and a limit it now stands past would make the next notch jump
    let (lo, hi) = zoom_limits(cam.fit);
    zoom_cam_3d(cam, rect, held.anchor, (scroll * 0.002).exp(), lo.min(cam.scale), hi.max(cam.scale));
}

/// THE CENTRE OF THE VIEW SLID ALONG THE LINE OF SIGHT to the depth of `p`, the picture left as it is.
///
/// In orthographic the depth of the centre changes nothing on screen. In perspective the eye stands where it stood and
/// the scale follows the centre: it is points per mm in the plane of the centre, and that plane is nearer or further.
/// A point behind the eye (or at it) leaves the view alone.
pub fn centre_at_depth_of(cam: &mut Cam3, set: &Settings, half_h: f32, p: [f64; 3]) {
    let (_, _, fwd) = cam.basis();
    let rel = v_dot(v_sub(p, cam.target), fwd);
    let inv_d = persp_inv_d_eye(cam, set, half_h);
    if inv_d > 0.0 {
        let d_eye = 1.0 / inv_d;
        let d_new = d_eye + rel;
        if d_new <= d_eye * 1e-3 {
            return;
        }
        cam.scale = (cam.scale as f64 * d_eye / d_new) as f32;
    }
    for (t, f) in cam.target.iter_mut().zip(fwd) {
        *t += f * rel;
    }
}

/// THE POINT AT `depth` ALONG THE RAY OF SIGHT through screen `at`: `depth` measured along the line of sight from the
/// plane of the centre of the view.
pub fn on_ray_at_depth(cam: &Cam3, set: &Settings, rect: Rect, at: Pos2, depth: f64) -> [f64; 3] {
    let (right, up, fwd) = cam.basis();
    let c = rect.center();
    let (sx, sy) = (((at.x - c.x) / cam.scale) as f64, (-(at.y - c.y) / cam.scale) as f64);
    let inv_d = persp_inv_d_eye(cam, set, rect.height() * 0.5);
    // in perspective the ray spreads from the eye: its offset from the axis grows with the distance from it
    let spread = if inv_d > 0.0 { 1.0 + depth * inv_d } else { 1.0 };
    std::array::from_fn(|a| cam.target[a] + fwd[a] * depth + (right[a] * sx + up[a] * sy) * spread)
}

/// HOW FAR THE WHEEL GOES: two hundred times either way from where the view was last framed, and never tighter than
/// 0.05 - 400 px/mm. Held at 0.05 - 400 it kept a 38 m model (framed at 0.0087 px/mm) and a 0.1 mm one out of
/// reach: the first notch threw the one out of the frame, and the other stopped at 48 px across.
pub fn zoom_limits(fit: f32) -> (f32, f32) {
    ((fit / 200.0).min(0.05), (fit * 200.0).max(400.0))
}

/// The same over the flat sheet of a sketch.
pub fn wheel_zoom_2d(view: &mut View2d, set: &Settings, rect: Rect, cursor: Option<Pos2>, scroll: f32) {
    // from the scale the sheet was fitted at, as the 3D view zooms. Measured with fixed 0.02 - 800 px/mm: a 40 m sketch
    // is framed at 0.017 px/mm, under that floor, and the wheel could take it out no further than 0.02
    let (lo, hi) = zoom_limits(view.fit);
    zoom_view_2d(view, rect, zoom_anchor(set, rect, cursor, None), (scroll * 0.002).exp(), lo, hi);
}

/// PANNING THE SHEET: with the layout's own gestures of moving the view - the middle button of CAD, the right one of
/// Gesture and OpenSCAD, Shift and the middle one of Blender, the left and right together of Blender and Revit, Shift
/// and a movement on a touchpad - and, in ours, with the middle button as well. A flat sheet has nothing to turn, so
/// the gestures that turn the model do nothing here. The left button alone is the sketch's own (`sheet_may_take`). Reported behaviour: those layouts moved the space and not the sheet; and every layout panned on the
/// middle button, which in Blender turns the model and pans only with Shift.
pub fn pan_sheet_2d(view: &mut View2d, ctx: &egui::Context, resp: &egui::Response, nav: MouseNav) {
    if zoom_latched(ctx) {
        return; // the middle button zooms while the latch is on
    }
    // THE LEFT BUTTON ALONE IS THE SKETCH'S OWN, whatever the gesture names: a gesture of any button with Shift (ours)
    // names no left button of its own, and passed `sheet_may_take`, so a left drag with Shift drew the selection box and
    // moved the sheet under it at once. Held with the right or the middle one it is a chord, and moves the sheet.
    let left_alone = ctx.input(|i| i.pointer.primary_down() && !i.pointer.secondary_down() && !i.pointer.middle_down());
    let by_layout = !left_alone && nav.pans().iter().any(|g| g.sheet_may_take() && g.active(ctx, resp));
    let ours = nav == MouseNav::QymCad && ctx.input(|i| i.pointer.middle_down());
    if !ours && !by_layout {
        return;
    }
    let d = ctx.input(|i| i.pointer.delta());
    view.center.x -= d.x / view.scale;
    view.center.y += d.y / view.scale;
}

/// WHAT A SKETCH'S OWN AXIS IS CALLED IN THE WORLD.
///
/// Reported behaviour: "in Part -> Revolve there are two axes, X and Y, but there is no Z axis to revolve
/// the sketch about."
///
/// A flat profile can only be turned about an axis lying IN its plane, and a plane has two of them - the
/// sketch's own X and Y. Its local Z is the normal, and a flat profile turned about its own normal sweeps
/// nothing. So there is no third axis to add. What was wrong is the NAME: the two buttons wore the world's
/// letters over the sketch's local axes, and on the front plane (`BasePlane::XZ`, whose y is [0,0,1]) the
/// button labelled Y revolved about the world Z. The axis a person was looking for was there, under someone
/// else's letter.
///
/// Named by the world direction the axis points along whenever it points along one; a sketch on an
/// arbitrary face points along nothing in particular and keeps its own letter.
pub fn sketch_axis_name(project: &qymcad_core::model::Project, si: Option<usize>, axis: u8) -> &'static str {
    let own = if axis == 0 { "X" } else { "Y" };
    let Some(f) = si.and_then(|si| project.sketch_frame(si)) else { return own };
    let d = if axis == 0 { f.x } else { f.y };
    let n = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    if n < 1e-9 {
        return own;
    }
    // Along a world axis to within a millionth: anything else is a direction of its own and gets no letter.
    ["X", "Y", "Z"].into_iter().enumerate().find(|(i, _)| d[*i].abs() / n > 1.0 - 1e-6).map(|(_, w)| w).unwrap_or(own)
}

/// The depth a profile of one's own starts at: the one the core would cut with nothing typed.
pub(crate) fn custom_depth_to_start_from(cmd: &FeatCommand, thread: ThreadParams) -> f64 {
    qymcad_core::thread::ThreadSpec {
        standard: qymcad_core::thread::ThreadStandard::Custom,
        nominal_d: if cmd_val(cmd, "nominal") > 0.0 { cmd_val(cmd, "nominal") } else { thread.radius * 2.0 },
        pitch: cmd_val(cmd, "pitch"),
        ..Default::default()
    }
    .geometry()
    .depth
}

/// The candidate planes for a sketch: the world XY, XZ and YZ plus the datum planes, with their frames.
pub fn sketch_plane_candidates(pn: &Painting) -> Vec<(qymcad_core::feature::SketchPlane, qymcad_core::feature::PlaneFrame)> {
    use qymcad_core::feature::{BasePlane, PlaneFrame, SketchPlane};
    let mut v = vec![
        (SketchPlane::World(BasePlane::XY), BasePlane::XY.frame()),
        (SketchPlane::World(BasePlane::XZ), BasePlane::XZ.frame()),
        (SketchPlane::World(BasePlane::YZ), BasePlane::YZ.frame()),
    ];
    for p in &pn.project.planes {
        // only the datums visible in the current context (not hidden by a tick box, not belonging to
        // another component) - otherwise an invisible datum of a part could be picked as the plane of a
        // sketch from inside an assembly. The same holds for picking a mirror or a section plane: what
        // gets drawn are the datums of THE assembly currently open, with no exceptions for direct
        // children - only the datums OF the current context (plus those of its ancestors while working in
        // context, as usual). The frame is carried by the transform of the owner: a datum travels with
        // its part.
        if let Some(wt) = datum_render_transform(pn, p.id) {
            let fr = PlaneFrame::from_origin_normal(p.origin, p.normal, p.rot_deg);
            let fr = if qymcad_core::feature::is_identity12(&wt) { fr } else { fr.transformed(&wt) };
            v.push((SketchPlane::Datum(p.id), fr));
        }
    }
    v
}
