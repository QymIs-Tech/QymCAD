//! THE REVOLVE NAMES ITS AXES BY WHAT THEY ARE IN THE WORLD.
//!
//! Reported behaviour: "in Part -> Revolve there are two axes, X and Y, but there is no Z axis to revolve
//! the sketch about."
//!
//! WHY THERE IS NO THIRD BUTTON, and why that is not the defect. The profile is flat and lives in the
//! sketch's own frame; the kernel revolves it about the sketch's local X or Y and only then places it on
//! the plane. The sketch's local Z is its NORMAL, and a flat profile turned about its own normal sweeps no
//! volume at all - it stays flat. An axis of revolution has to lie IN the plane of the profile, and a plane
//! has exactly two of its own.
//!
//! WHAT IS ACTUALLY MISSING is the name. The two buttons were labelled X and Y - the sketch's local axes,
//! wearing the world's letters. On the front plane the sketch's local Y IS the world Z (`BasePlane::XZ`
//! gives y = [0,0,1]), and on the side plane too. So the axis a person is looking for was already there,
//! under the wrong letter, and pressing "Y" revolved about Z.
//!
//! Naming them by the world direction they point along answers the report where it is true and does not
//! invent a button that could only build nothing.

use qymcad_ui_state::Bench;

/// Everything the frame drew.
fn texts(shapes: &[egui::epaint::ClippedShape]) -> Vec<String> {
    fn walk(s: &egui::epaint::Shape, out: &mut Vec<String>) {
        match s {
            egui::epaint::Shape::Text(t) => out.push(t.galley.text().to_string()),
            egui::epaint::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for cs in shapes {
        walk(&cs.shape, &mut out);
    }
    out
}

/// The revolve command open over a sketch on `plane`, and what its bar drew.
fn the_revolve_bar_on(plane: qymcad_core::feature::BasePlane) -> Vec<String> {
    let mut b = Bench::default();
    b.project.new_document();
    let si = b.project.new_sketch("S");
    b.project.sketches[si].plane = qymcad_core::feature::SketchPlane::World(plane);
    b.project.add_rect_entity(si, 10.0, 10.0, 40.0, 30.0, qymcad_core::feature::Purpose::Real);
    b.project.regen_sketch(si);
    b.mode_3d = true;
    b.cmd.open(&mut b.armed, 3, true); // revolve
    b.cmd.sketch = Some(si);

    let ctx = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(2000.0, 400.0));
    let full = ctx.run_ui(egui::RawInput { screen_rect: Some(screen), ..Default::default() }, |ui| {
        qymcad_part::feat_command_bar(&mut b.part_ctx(), ui);
    });
    texts(&full.shapes)
}

/// ON THE FRONT PLANE ONE OF THE TWO AXES IS Z, and it says so.
#[test]
fn on_the_front_plane_the_upright_axis_is_called_z() {
    let said = the_revolve_bar_on(qymcad_core::feature::BasePlane::XZ);
    assert!(
        said.iter().any(|t| t == "Z"),
        "the sketch stands on the front plane, its second axis points along the world Z, and the bar offers no Z at all: {said:?}"
    );
    assert!(
        !said.iter().any(|t| t == "Y"),
        "and it must not go on calling that same axis Y - there is no Y in this plane: {said:?}"
    );
}

/// ON THE SIDE PLANE THE PAIR IS Y AND Z.
#[test]
fn on_the_side_plane_the_axes_are_y_and_z() {
    let said = the_revolve_bar_on(qymcad_core::feature::BasePlane::YZ);
    let missing: Vec<_> = ["Y", "Z"].iter().filter(|w| !said.iter().any(|t| t == *w)).collect();
    assert!(missing.is_empty(), "the side plane is spanned by the world Y and Z, and the bar does not name {missing:?}: {said:?}");
    assert!(!said.iter().any(|t| t == "X"), "and nothing in this plane points along X: {said:?}");
}

/// ON THE TOP PLANE NOTHING CHANGES: it really is X and Y there.
///
/// Without this the fix could be "rename the second button to Z always", which would be a new lie in place
/// of the old one.
#[test]
fn on_the_top_plane_the_axes_are_still_x_and_y() {
    let said = the_revolve_bar_on(qymcad_core::feature::BasePlane::XY);
    let missing: Vec<_> = ["X", "Y"].iter().filter(|w| !said.iter().any(|t| t == *w)).collect();
    assert!(missing.is_empty(), "the top plane is spanned by the world X and Y, and the bar does not name {missing:?}: {said:?}");
    assert!(!said.iter().any(|t| t == "Z"), "and nothing in this plane points along Z: {said:?}");
}
