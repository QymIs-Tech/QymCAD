//! A REGION OF A SKETCH IS PICKED BY A CLICK INSIDE IT AND SHOWN FILLED, in every tool that takes a closed profile.
//!
//! The owner's request: in the tools that take closed geometry of a sketch, the picked region is filled with a colour
//! of its own rather than only outlined, and a region is picked by a click inside it, not only on its line.
use qymcad::{Key, Session};
use qymcad_acceptance::{build, probe};

/// A rectangle 40 x 30 on XY with a circle of R 6 inside it, the sketch left: two regions, a ring and a disk.
fn a_ring_and_a_disk() -> Session {
    let mut s = Session::start();
    build::into_the_first_part(&mut s);
    build::rectangle_on_xy(&mut s);
    build::draw(&mut s, "tb-circle-hint", &[(20.0, 15.0), (26.0, 15.0)]);
    let finish = s.word("wb-finish");
    s.press_word(&finish);
    s
}

/// The colour of the picture at a point of the screen.
fn colour_at(pic: &qymcad::Picture, at: qymcad::Pos2) -> [u8; 3] {
    let (x, y) = (at.x.round() as usize, at.y.round() as usize);
    let o = (y * pic.width + x) * 4;
    [pic.rgba[o], pic.rgba[o + 1], pic.rgba[o + 2]]
}

/// How far apart two colours are, the largest of the three channels.
fn apart(a: [u8; 3], b: [u8; 3]) -> i32 {
    (0..3).map(|k| (a[k] as i32 - b[k] as i32).abs()).max().unwrap_or(0)
}

probe! {
    /// A CLICK INSIDE A REGION PICKS IT AND FILLS IT, a hole of it left open: in the picker of an extrusion over a
    /// rectangle with a circle inside, a click between them fills the ring and leaves the disk as the sheet; a click in
    /// the circle fills the disk as well.
    fn a_click_inside_a_region_picks_it_and_fills_it_its_holes_left_open() {
        let mut s = a_ring_and_a_disk();
        let extrude = s.word("tb-extrude-hint");
        s.press_hint(&extrude);
        // the sheet's own colour, at a corner of the canvas, clear of the sketch
        let corner = s.canvas().min + qymcad::vec2(12.0, 12.0);
        // where a click lands, and where the colour is read - away from the pointer's cross drawn at the click
        let ring = s.on_sheet(5.0, 5.0);
        let disk = s.on_sheet(20.0, 15.0);
        let in_ring = s.on_sheet(35.0, 25.0);
        let in_disk = s.on_sheet(20.0, 18.0);
        let before = s.snapshot();
        let sheet = colour_at(&before, corner);
        assert!(apart(colour_at(&before, in_ring), sheet) < 8, "setup: the ring is filled before any click");
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/look");
        let _ = std::fs::create_dir_all(&dir);

        s.click(ring);
        let after_ring = s.snapshot();
        let _ = std::fs::write(dir.join("region_picker_ring.png"), after_ring.png());
        assert!(apart(colour_at(&after_ring, in_ring), sheet) >= 20, "the ring was clicked and is not filled: {:?} against the sheet {sheet:?}", colour_at(&after_ring, in_ring));
        assert!(apart(colour_at(&after_ring, in_disk), sheet) < 8, "the ring was picked and its hole is filled too: {:?} against the sheet {sheet:?}", colour_at(&after_ring, in_disk));

        // the region under the pointer is lit before the click - which region a click would take
        s.move_to(disk);
        let hovered = s.snapshot();
        let _ = std::fs::write(dir.join("region_picker_hover.png"), hovered.png());
        assert!(apart(colour_at(&hovered, in_disk), sheet) >= 8, "the disk under the pointer is not lit before the click: {:?} against the sheet {sheet:?}", colour_at(&hovered, in_disk));

        s.click(disk);
        let both = s.snapshot();
        let _ = std::fs::write(dir.join("region_picker_both.png"), both.png());
        assert!(apart(colour_at(&both, in_disk), sheet) >= 20, "the disk was clicked and is not filled: {:?}", colour_at(&both, in_disk));
        // on to the size, in 3D: the picked regions lie filled on the sketch's plane under the preview (for the eyes)
        s.key(Key::Enter);
        let _ = std::fs::write(dir.join("region_picker_3d.png"), s.snapshot().png());
        s.key(Key::Escape);
    }
}
