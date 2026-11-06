//! THE FILE CARRIES THE PREVIEW A FILE MANAGER SHOWS: a model saved has one, a document with nothing to draw has none.
use qymcad::Session;
use qymcad_acceptance::{build, probe};

/// A scratch file for one check, gone from before the save.
fn scratch(tag: &str) -> String {
    let path = std::env::temp_dir().join(format!("qymcad-preview-{tag}-{}.qcad", std::process::id())).to_string_lossy().into_owned();
    let _ = std::fs::remove_file(&path);
    path
}

probe! {
    /// A BLOCK SAVED CARRIES A PNG.
    fn a_saved_block_carries_a_png_preview() {
        let mut s = Session::start();
        build::block(&mut s);
        let path = scratch("block");
        s.save_copy(&path).expect("the copy is written");
        let png = s.file_preview(&path).expect("the file of a block has no preview");
        assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"), "the preview is not a PNG");
        let _ = std::fs::remove_file(&path);
    }
}

probe! {
    /// NOTHING TO DRAW, NO PREVIEW.
    fn a_sketch_alone_has_no_preview() {
        let mut s = Session::start();
        build::rectangle_on_xy(&mut s);
        let path = scratch("sketch");
        s.save_copy(&path).expect("the copy is written");
        assert!(s.file_preview(&path).is_none(), "a document with nothing to draw has a preview");
        let _ = std::fs::remove_file(&path);
    }
}
