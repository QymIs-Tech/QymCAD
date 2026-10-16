//! ONE DOOR FOR BRINGING A FILE IN.
//!
//! There were four - "Import DXF", "Import STL", "Import STEP", "Import SVG" - each with a chooser showing
//! its own extension and nothing else, and every new format would have added a fifth. Now there is one: the
//! chooser lists everything the table `qymcad_io::Format` knows, and the file's extension decides what it
//! becomes - a solid, a mesh or a sketch. A file of a kind the program does not read gets an answer naming the
//! kinds it does, instead of a reader's error about a format the file was never in.
use qymcad_ui_state::Notice;
use std::path::PathBuf;

use qymcad_io::{Format, Lands};
use qymcad_ui_state::Want;

use qymcad_kernel::ExactFormat;

use qymcad_ui_state::MeshFormat;

use super::{open_exact, open_mesh, open_svg, App, Rebuilding};

/// Whether a file that becomes `lands` may come in at a place that wants `want`.
fn takes(want: Want, lands: Lands) -> bool {
    match want {
        Want::Anything => true,
        Want::Part => lands != Lands::Drawing,
    }
}

/// The formats the door offers at a place.
fn formats_for(want: Want) -> Vec<Format> {
    Format::ALL.into_iter().filter(|f| takes(want, f.lands())).collect()
}

/// The chooser's filters: everything together first, then each format on its own.
///
/// Both cases of every extension: the chooser matches case-sensitively on some systems, and a `PART.STP`
/// would then simply not be shown.
pub(crate) fn import_filters(want: Want) -> Vec<(String, Vec<String>)> {
    let formats = formats_for(want);
    let both = |f: &Format| f.extensions().iter().flat_map(|e| [e.to_string(), e.to_ascii_uppercase()]).collect::<Vec<_>>();
    let mut out = vec![(crate::i18n::tr("pk-import-all"), formats.iter().flat_map(both).collect())];
    out.extend(formats.iter().map(|f| (f.name().to_string(), both(f))));
    out
}

/// The chooser, with the filters above.
pub(crate) fn import_dialog(want: Want) -> rfd::AsyncFileDialog {
    import_filters(want).into_iter().fold(rfd::AsyncFileDialog::new(), |d, (name, exts)| d.add_filter(name, &exts))
}

/// What a picked file turned into.
pub(crate) enum Landing {
    /// A solid or a mesh is being read in the background; the job puts it into the document when it is done.
    Started,
    /// The curves of a drawing, waiting for the plane they are laid on.
    /// A drawing read into curves, with what of it did not come in as the file has it.
    Drawing(qymcad_io::ImportedSketch),
    /// Nothing came in, and this says why.
    Refused(String),
}

/// Read `path` the way its extension says, for a place that wants `want`.
pub(crate) fn land_import(regen: &mut Rebuilding, path: &str, want: Want) -> Landing {
    let Some(format) = Format::of_path(path) else {
        return Landing::Refused(crate::i18n::tr2("import-unknown", "file", &super::file_name(path), "formats", &Format::names_of(&formats_for(want))));
    };
    if !takes(want, format.lands()) {
        return Landing::Refused(crate::i18n::tr1("import-not-a-part", "format", format.name()));
    }
    let drawing = |read: Result<qymcad_io::ImportedSketch, String>| match read {
        Ok(sketch) => Landing::Drawing(sketch),
        Err(why) => Landing::Refused(why),
    };
    // no wildcard: a format added to the table does not compile until it has a reader here
    match format {
        Format::Step => {
            open_exact(regen, path.to_string(), ExactFormat::Step);
            Landing::Started
        }
        Format::Iges => {
            open_exact(regen, path.to_string(), ExactFormat::Iges);
            Landing::Started
        }
        Format::Stl => {
            open_mesh(regen, path.to_string(), MeshFormat::Stl);
            Landing::Started
        }
        Format::Obj => {
            open_mesh(regen, path.to_string(), MeshFormat::Obj);
            Landing::Started
        }
        Format::Ply => {
            open_mesh(regen, path.to_string(), MeshFormat::Ply);
            Landing::Started
        }
        Format::Gltf => {
            open_mesh(regen, path.to_string(), MeshFormat::Glb);
            Landing::Started
        }
        Format::ThreeMf => {
            open_mesh(regen, path.to_string(), MeshFormat::ThreeMf);
            Landing::Started
        }
        Format::Amf => {
            open_mesh(regen, path.to_string(), MeshFormat::Amf);
            Landing::Started
        }
        Format::Dxf => drawing(qymcad_io::import_dxf(path).map_err(|e| crate::i18n::tr1("g-dxf-error", "error", &e.to_string()))),
        Format::Svg => {
            open_svg(regen, path.to_string());
            Landing::Started
        }
    }
}

/// What the door does with the chooser's answer. The checks hand the same function a path of their own.
pub(crate) fn import_answer(want: Want) -> impl FnOnce(&mut App, PathBuf) + 'static {
    move |app, p| {
        let path = p.to_string_lossy().into_owned();
        match land_import(&mut app.regen, &path, want) {
            Landing::Started => {}
            Landing::Drawing(sketch) => {
                app.arm_sketch_import(sketch.curves, &path);
                // WHAT DID NOT COME IN AS THE FILE HAS IT IS SAID, beside what the door asks next: a text or a hatch
                // silently lost is found missing only later, by the person, on the drawing; a spline drawn through its
                // fit points because its control data is invalid is not quite the curve the file meant. Each entity is
                // named once, under what happened to it.
                for (key, kinds) in [("import-redrawn", &sketch.redrawn), ("import-invalid", &sketch.invalid), ("import-not-read", &sketch.skipped)] {
                    if !kinds.is_empty() {
                        let list = kinds.iter().map(|(kind, n)| format!("{kind} {n}")).collect::<Vec<_>>().join(", ");
                        app.status.words = format!("{} · {}", app.status, crate::i18n::tr1(key, "list", &list));
                    }
                }
            }
            Landing::Refused(why) => app.status = Notice::refusal(why),
        }
    }
}

#[cfg(test)]
pub(crate) mod tests;
