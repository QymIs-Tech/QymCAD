//! THE PART WORKBENCH, CHECKED WITHOUT THE APPLICATION.
//!
//! Until now every check of this crate was a check of the application: the workbench takes a context, a
//! context is a bundle of borrows, and only `App` owned the records the borrows point at. So verifying
//! three lines of arithmetic meant building a window and running frames, and the crate could not be
//! verified on its own at all - which is the one thing splitting it out was supposed to buy.
//!
//! `Bench` owns the same records and ties the same context. Nothing here is a mock: the document,
//! the command, the selection are the real ones.

use qymcad_ui_state::{Armed, Bench};

/// OPENING A COMMAND AND CANCELLING IT LEAVES NOTHING IN HAND.
///
/// The rule the whole `Armed` rework is about, checked at the level it lives on rather than through a
/// window: the command is one field, and cancelling puts it back to nothing.
#[test]
fn cancelling_a_command_leaves_the_hand_empty() {
    let mut b = Bench::default();
    b.cmd.open(&mut b.armed, 1, false); // extrude
    assert_eq!(b.armed, Armed::Command(1), "opening a command must put it in hand");

    qymcad_ui_state::cancel_feat_cmd(&mut b.part_ctx());
    assert_eq!(b.armed, Armed::None, "cancelling must leave nothing in hand");
}

/// THE COMMAND'S NAME COMES FROM WHAT IS IN HAND, not from a field of its own.
///
/// Checked by swapping ONLY the hand: `feat` is untouched between the two readings, so a name that
/// changes can have come from nowhere else. Asking instead what an EMPTY hand is called would prove
/// nothing - `feat_cmd_name` has one caller, `apply_feat_cmd`, which names the undo step of a command
/// already in hand, so an empty hand never reaches it and its `f-operation` fallback stands for an
/// unknown KIND rather than for no command at all.
#[test]
fn the_command_names_itself_from_the_hand() {
    let mut b = Bench::default();

    b.cmd.open(&mut b.armed, 1, false); // extrude
    let extrude = qymcad_part::feat_cmd_name(&b.armed, b.feat);

    b.cmd.open(&mut b.armed, 4, false); // fillet
    let fillet = qymcad_part::feat_cmd_name(&b.armed, b.feat);

    assert!(!extrude.is_empty() && !fillet.is_empty(), "a command in hand must have a name to show");
    assert_ne!(extrude, fillet, "the name must follow the hand: same `feat`, different command, same name means it does not");
}

/// TOOLBAR BUTTONS WITH STRONGLY-TYPED ICON IDS RENDER PROPERLY.
#[test]
fn icon_tool_with_typed_ids_renders_in_toolbar() {
    let ctx = egui::Context::default();
    egui_extras::install_image_loaders(&ctx);
    let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
        // Line tool
        let _ = qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::SketchLine, "Line", false);
        // Circle 3pt (migrated from procedural Gly::Circle3)
        let _ = qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::SketchCircle3Pt, "Circle 3pt", false);
        // Ellipse (migrated from procedural Gly::Ellipse)
        let _ = qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::SketchEllipse, "Ellipse", false);
        // Part Extrude
        let _ = qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::PartExtrude, "Extrude", true);
    });

    let tested_icons = [qymcad_ui_state::IconId::SketchLine, qymcad_ui_state::IconId::SketchCircle3Pt, qymcad_ui_state::IconId::SketchEllipse, qymcad_ui_state::IconId::PartExtrude];
    for icon in tested_icons {
        let uri = icon.uri();
        let poll = ctx.try_load_image(uri, egui::load::SizeHint::default()).expect("toolbar icon must be loaded by image loader");
        match poll {
            egui::load::ImagePoll::Ready { image } => {
                assert!(image.size[0] > 0 && image.size[1] > 0, "rasterized icon must have positive dimensions");
                assert!(image.pixels.iter().any(|p| p.a() > 0), "rasterized icon must contain visible pixels");
            }
            egui::load::ImagePoll::Pending { .. } => panic!("in-memory SVG bytes must load synchronously into Ready state"),
        }
    }
}
