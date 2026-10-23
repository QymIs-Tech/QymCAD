//! THE FORMATS A FILE CAN BE BROUGHT IN FROM, in one table.
//!
//! Every format used to have a door of its own - a menu item, a chooser with its own filter, a path of its own
//! into the document - and every new format would have added one more. The one door reads this table: the
//! chooser's filters are built from it, the reader is picked from it, and a format missing here cannot be
//! opened at all. The failure this rules out is a format that is read but not listed: nobody finds it.

/// What a file becomes once it has been read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lands {
    /// Exact geometry: bodies with faces and edges.
    Solid,
    /// Triangles: a mesh body.
    Mesh,
    /// Flat curves: an editable sketch, laid on a plane the person picks.
    Drawing,
}

/// A format the program reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Step,
    Iges,
    F3d,
    Stl,
    Obj,
    Ply,
    Gltf,
    ThreeMf,
    Amf,
    Dxf,
    Svg,
}

impl Format {
    /// Every format the one door opens, in the order the chooser lists them.
    pub const ALL: [Format; 11] = [Format::Step, Format::Iges, Format::F3d, Format::Stl, Format::Obj, Format::Ply, Format::Gltf, Format::ThreeMf, Format::Amf, Format::Dxf, Format::Svg];

    /// The name a person knows the format by. It is a name in no language, so it is not in the catalogue.
    pub fn name(self) -> &'static str {
        match self {
            Format::Step => "STEP",
            Format::Iges => "IGES",
            Format::F3d => "F3D",
            Format::Stl => "STL",
            Format::Obj => "OBJ",
            Format::Ply => "PLY",
            Format::Gltf => "glTF",
            Format::ThreeMf => "3MF",
            Format::Amf => "AMF",
            Format::Dxf => "DXF",
            Format::Svg => "SVG",
        }
    }

    /// The extensions, in lower case. A file's own extension is compared without regard to case: `PART.STP`
    /// written by an old program is as much a STEP file as `part.step`.
    pub fn extensions(self) -> &'static [&'static str] {
        match self {
            Format::Step => &["step", "stp"],
            Format::Iges => &["igs", "iges"],
            Format::F3d => &["f3d"],
            Format::Stl => &["stl"],
            Format::Obj => &["obj"],
            Format::Ply => &["ply"],
            // the single binary file first: it is what is written
            Format::Gltf => &["glb", "gltf"],
            Format::ThreeMf => &["3mf"],
            Format::Amf => &["amf"],
            Format::Dxf => &["dxf"],
            Format::Svg => &["svg"],
        }
    }

    pub fn lands(self) -> Lands {
        match self {
            Format::Step | Format::Iges | Format::F3d => Lands::Solid,
            Format::Stl | Format::Obj | Format::Ply | Format::Gltf | Format::ThreeMf | Format::Amf => Lands::Mesh,
            Format::Dxf | Format::Svg => Lands::Drawing,
        }
    }

    /// The format of a file, told by its extension; `None` for a file the program does not read.
    pub fn of_path(path: &str) -> Option<Format> {
        let ext = std::path::Path::new(path).extension()?.to_str()?.to_ascii_lowercase();
        Format::ALL.into_iter().find(|f| f.extensions().contains(&ext.as_str()))
    }

    /// The names of the formats in one line - what the answer to a file of an unknown kind lists.
    pub fn names_of(formats: &[Format]) -> String {
        formats.iter().map(|f| f.name()).collect::<Vec<_>>().join(", ")
    }
}
