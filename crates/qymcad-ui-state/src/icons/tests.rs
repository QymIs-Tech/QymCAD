//! Unit tests for the icon theme system.

use std::collections::HashMap;

use super::bundle::*;
use super::id::*;
use super::manager::*;
use super::manifest::*;
use super::pack::*;
use super::runtime::forget_all_cad_icons;
use super::sha256::TrailerCheck;

#[test]
fn every_icon_id_has_relative_path() {
    for id in ALL_ICONS {
        let path = id.relative_path();
        assert!(!path.is_empty(), "icon {:?} has empty relative path", id);
        assert!(!path.ends_with(".svg"), "relative path should not contain extension: {}", path);

        let roundtrip = IconId::from_id_str(path);
        assert_eq!(roundtrip, Some(*id), "from_id_str failed for {}", path);
    }
}

#[test]
fn generated_correct_break_path() {
    assert_eq!(IconId::SketchBreak.relative_path(), "sketch/break");
}

mod svg;
mod bundles;
mod packs;
mod watch;
