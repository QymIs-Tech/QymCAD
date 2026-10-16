//! A SHEET HOLDS NO VOLUME: a surface stitched from faces that do not close is a surface, and its volume is zero.
//!
//! Reported behaviour: two faces copied off a block and stitched along their shared edge weighed 2000 mm^3 - the volume
//! of an open shell was taken as if it were closed, and the part with the block counted 14000 instead of 12000.
use crate::Shape;

#[test]
fn two_faces_stitched_along_an_edge_hold_no_volume() {
    let block = Shape::extrude(&[0.0, 0.0, 40.0, 0.0, 40.0, 30.0, 0.0, 30.0], 10.0).expect("the block");
    assert!((block.volume() - 12000.0).abs() < 1e-6, "the block itself holds 12000, it holds {}", block.volume());
    let n = 6u32; // a box
    let mut found = false;
    'pairs: for i in 0..n {
        for j in i + 1..n {
            let (Some(a), Some(b)) = (block.copy_faces(&[i], &[1]), block.copy_faces(&[j], &[2])) else { continue };
            let Ok(sheet) = Shape::stitch(&[&a, &b], 1e-6) else { continue }; // two faces that do not meet join nothing
            assert!(sheet.is_sheet(), "two faces stitched are a sheet");
            assert!(sheet.volume().abs() < 1e-9, "faces {i} and {j} stitched hold {} mm^3", sheet.volume());
            found = true;
            break 'pairs;
        }
    }
    assert!(found, "setup: no two faces of the block stitched");
}
