//! THE TABLE OF FORMATS says what the one door opens, so it has to be whole and unambiguous.
use qymcad_io::{Format, Lands};

/// EVERY FORMAT IS FOUND BY EACH OF ITS EXTENSIONS, whatever their case.
#[test]
fn every_format_is_found_by_each_of_its_extensions_in_any_case() {
    for f in Format::ALL {
        assert!(!f.extensions().is_empty(), "{} has no extension, so no file can ever be taken for it", f.name());
        for ext in f.extensions() {
            assert_eq!(*ext, ext.to_ascii_lowercase(), "{}: the table keeps extensions in lower case", f.name());
            for name in [format!("part.{ext}"), format!("PART.{}", ext.to_ascii_uppercase()), format!("/some/dir.v2/part.{ext}")] {
                assert_eq!(Format::of_path(&name), Some(f), "{name} was not taken for {}", f.name());
            }
        }
    }
}

/// NO EXTENSION BELONGS TO TWO FORMATS: the door would open the file with whichever came first.
#[test]
fn no_extension_belongs_to_two_formats() {
    let mut seen: Vec<(&str, &str)> = Vec::new();
    for f in Format::ALL {
        for ext in f.extensions() {
            if let Some((_, other)) = seen.iter().find(|(e, _)| e == ext) {
                panic!(".{ext} is claimed by both {other} and {}", f.name());
            }
            seen.push((ext, f.name()));
        }
    }
}

/// A FILE OF AN UNKNOWN KIND IS NOT TAKEN FOR ANY FORMAT - it gets an answer instead, from the door.
#[test]
fn a_file_of_an_unknown_kind_is_taken_for_nothing() {
    for name in ["drawing.cdw", "model.x_t", "readme", "archive.step.zip", ".stl"] {
        assert_eq!(Format::of_path(name), None, "{name} was taken for a format the program reads");
    }
}

/// EVERY FORMAT STANDS IN THE LIST THE DOOR READS.
///
/// A variant added to `Format` but left out of `Format::ALL` would be readable by code and invisible to a
/// person. The match below has no wildcard, so a new variant stops this file compiling until it is given a
/// place here - and the count then asks for its place in the list.
#[test]
fn every_format_stands_in_the_list_the_door_reads() {
    fn place(f: Format) -> usize {
        match f {
            Format::Step => 0,
            Format::Iges => 1,
            Format::F3d => 2,
            Format::Stl => 3,
            Format::Obj => 4,
            Format::Ply => 5,
            Format::Gltf => 6,
            Format::ThreeMf => 7,
            Format::Amf => 8,
            Format::Dxf => 9,
            Format::Svg => 10,
        }
    }
    let mut places: Vec<usize> = Format::ALL.iter().map(|f| place(*f)).collect();
    places.sort();
    assert_eq!(places, (0..11).collect::<Vec<_>>(), "the list the door reads is missing a format or holds one twice");
}

/// WHAT A FILE BECOMES follows from its format: exact geometry is a solid, triangles a mesh, curves a sketch.
#[test]
fn what_a_file_becomes_follows_from_its_format() {
    assert_eq!(Format::Step.lands(), Lands::Solid);
    assert_eq!(Format::Iges.lands(), Lands::Solid);
    assert_eq!(Format::F3d.lands(), Lands::Solid);
    assert_eq!(Format::Stl.lands(), Lands::Mesh);
    assert_eq!(Format::Obj.lands(), Lands::Mesh);
    assert_eq!(Format::Ply.lands(), Lands::Mesh);
    assert_eq!(Format::Gltf.lands(), Lands::Mesh);
    assert_eq!(Format::ThreeMf.lands(), Lands::Mesh);
    assert_eq!(Format::Amf.lands(), Lands::Mesh);
    assert_eq!(Format::Dxf.lands(), Lands::Drawing);
    assert_eq!(Format::Svg.lands(), Lands::Drawing);
    assert_eq!(Format::names_of(&Format::ALL), "STEP, IGES, F3D, STL, OBJ, PLY, glTF, 3MF, AMF, DXF, SVG");
}
