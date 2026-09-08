//! THE THREAD BAR DESCRIBES THE THREAD THAT WILL ACTUALLY BE BUILT.
//!
//! Reported behaviour: "a custom thread is broken when trying to make a bolt and a nut for YouTube - that
//! is, when you set the profile and the angle yourself."
//!
//! WHAT THE BAR IS FOR. It prints the geometry of the chosen size and, next to it, WHAT THE MATING PART
//! NEEDS: the diameter of the shaft to turn and of the hole to drill. Those numbers are the whole point of
//! making a pair - a person reads the hole diameter off the bar and bores the nut to it.
//!
//! The bar built a `ThreadSpec` OF ITS OWN to compute them, and filled the rest of the fields from
//! `Default`. For the five standards that costs nothing: their profile comes from a table, and the fields
//! left out are not part of it. `Custom` is the one standard whose depth and angle come from the person -
//! so those two were dropped exactly where they were the only thing that mattered.
//!
//! MEASURED at Ø20 x 5 with a depth of 2.5 typed in: the hole is 15.00 mm, and the bar said 14.00 - the
//! depth it used was `Default`'s 0.6 of the pitch, not the 2.5 asked for. A nut bored to what the bar says
//! is a millimetre undersize and does not go on. The crest and root radii were dropped the same way.

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

/// Set one of the command's fields, as typing into it does.
fn typed(cmd: &mut qymcad_ui_state::FeatCommand, key: &str, v: f64) {
    cmd.params.iter_mut().find(|p| p.key == key).unwrap_or_else(|| panic!("the command has no field {key}")).val = v;
}

/// The thread command in hand, cut to a profile of one's own: Ø20 x 5, 40 degrees, 2.5 deep.
///
/// The angle is a shallow one deliberately: at 90 degrees a groove 2.5 deep does not fit a pitch of 5, and
/// the program says so before building - that case belongs to the message, not to this measurement.
fn a_thread_of_ones_own() -> (Bench, qymcad_core::thread::ThreadSpec) {
    let mut b = Bench::default();
    b.mode_3d = true;
    b.thread.radius = 10.0; // the cylinder the thread sits on: Ø20
    b.thread.form = 5; // a profile of one's own
    b.cmd.open(&mut b.armed, 24, true);
    qymcad_ui_state::set_thread_params(&mut b.cmd, b.thread);
    typed(&mut b.cmd, "pitch", 5.0);
    typed(&mut b.cmd, "angle", 40.0);
    typed(&mut b.cmd, "depth", 2.5);
    typed(&mut b.cmd, "fit", 0.0);

    // WHAT WILL BE BUILT, stated here from the numbers a person typed rather than taken from the code under
    // test: the same fields, read straight off the command.
    let spec = qymcad_core::thread::ThreadSpec {
        standard: qymcad_core::thread::ThreadStandard::Custom,
        nominal_d: 20.0,
        pitch: 5.0,
        custom_angle: 40.0,
        custom_depth: 2.5,
        ..Default::default()
    };
    (b, spec)
}

/// What the bar drew, as one string.
fn what_the_bar_says(b: &mut Bench) -> String {
    let ctx = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(2000.0, 400.0));
    let full = ctx.run_ui(egui::RawInput { screen_rect: Some(screen), ..Default::default() }, |ui| {
        qymcad_part::feat_command_bar(&mut b.part_ctx(), ui);
    });
    texts(&full.shapes).join(" | ")
}

/// THE HOLE THE BAR NAMES IS THE HOLE THE NUT NEEDS.
#[test]
fn the_bar_names_the_hole_the_mating_part_actually_needs() {
    let (mut b, spec) = a_thread_of_ones_own();
    let (own, mate) = spec.blank_diameters();
    let said = what_the_bar_says(&mut b);

    assert!(
        said.contains(&qymcad_i18n::num(mate, 2)),
        "a person bores the nut to the diameter the bar names, and it names one for a thread nobody asked for: the hole is {:.2} mm and the bar says\n{said}",
        mate
    );
    assert!(said.contains(&qymcad_i18n::num(own, 2)), "and the shaft is {own:.2} mm, which the bar does not name either:\n{said}");
}

/// AND SO IS THE GEOMETRY IT PRINTS.
///
/// The depth and the pitch diameter beside it come from the same spec, so if that spec is not the one being
/// built they are wrong together - and it is the depth a person checks the profile by.
#[test]
fn the_bar_prints_the_geometry_of_the_profile_that_was_asked_for() {
    let (mut b, spec) = a_thread_of_ones_own();
    let g = spec.geometry();
    let said = what_the_bar_says(&mut b);

    assert!(
        said.contains(&qymcad_i18n::num(g.depth, 2)),
        "the profile is {:.2} deep and the bar prints a different depth:\n{said}",
        g.depth
    );
    assert!(said.contains(&qymcad_i18n::num(g.minor_d, 2)), "and the minor diameter is {:.2}:\n{said}", g.minor_d);
}

/// THE DEPTH FIELD OPENS ON THE DEPTH THAT WILL BE CUT.
///
/// Zero in that field does not mean a groove of no depth: the core reads it as "nothing was typed" and cuts
/// 0.6 of the pitch. So the field opened at 0.00 over a groove 1.50 deep, and a person setting a profile of
/// their own started from a number that described nothing - which is the same lie as the bar's, one field
/// further along.
#[test]
fn the_depth_field_opens_on_the_depth_that_will_be_cut() {
    let mut b = Bench::default();
    b.thread.radius = 10.0; // Ø20
    b.thread.form = 5; // a profile of one's own
    b.cmd.open(&mut b.armed, 24, true);
    qymcad_ui_state::set_thread_params(&mut b.cmd, b.thread);
    // The pitch is left at zero - "the standard coarse pitch for this size" - which is what a person gets
    // without touching it.
    let shown = qymcad_ui_state::cmd_val(&b.cmd, "depth");
    let cut = qymcad_part::thread_spec(&b.cmd, b.thread).geometry().depth;

    assert!(
        (shown - cut).abs() < 1e-6,
        "the field says the groove is {shown:.2} deep and the groove that gets cut is {cut:.2}: a field that does not describe its own feature"
    );
}
