//! DOCUMENT TEMPLATES — FILES IN THE CONFIG DIRECTORY, by the same means as colour schemes and
//! languages: save it, drop it next to the others, share it as one file.
//!
//! A TEMPLATE IS AN ORDINARY DOCUMENT, not a separate entity. That is how grown-up CAD does it, and
//! it is not a shortcut: introducing a "template format" alongside the document format would mean
//! maintaining two formats obliged to do the same thing, which will drift apart on the very first
//! new setting. A template carries exactly what a document carries: properties (tolerance, units,
//! author), starting datums, prepared sketches — whatever a person put into it is what they get.
//!
//! Templates are what make DOCUMENT settings useful in the first place: without them the geometry
//! tolerance and the other properties would have to be set again in every new file.
use std::path::PathBuf;

/// The template directory. `None` — the OS has no config directory (happens in sandboxes).
pub fn dir() -> Option<PathBuf> {
    directories::ProjectDirs::from("tech", "qymis", "qym-cad").map(|d| d.config_dir().join("templates"))
}

/// The file name for a template: letters/digits/hyphen only, everything else becomes an underscore.
///
/// The name is written by a person and may contain anything at all, up to `../`: a path must not be
/// assembled from it. The same means as for colour-scheme names, and for the same reason.
pub fn file_name(title: &str) -> String {
    let mut s: String = title.chars().map(|c| if c.is_alphanumeric() || c == '-' { c } else { '_' }).collect();
    if s.trim_matches('_').is_empty() {
        s = "template".into();
    }
    format!("{s}.qcad")
}

/// The templates lying in the directory: (display name, path). Sorted by name.
///
/// The name is taken from the FILE rather than from the document properties inside: reading every
/// template for the sake of a menu caption would mean unpacking a dozen zip bundles every time the
/// menu opens.
pub fn list() -> Vec<(String, String)> {
    let Some(d) = dir() else { return Vec::new() };
    let Ok(rd) = std::fs::read_dir(&d) else { return Vec::new() };
    let mut out: Vec<(String, String)> = rd
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "qcad"))
        .filter_map(|p| {
            let name = p.file_stem()?.to_string_lossy().into_owned();
            Some((name, p.to_string_lossy().into_owned()))
        })
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// Write the document out as a template. Returns the path.
pub fn save(project: &qymcad_core::model::Project, title: &str) -> Result<String, String> {
    let d = dir().ok_or_else(|| crate::i18n::tr("tpl-no-dir"))?;
    std::fs::create_dir_all(&d).map_err(|e| e.to_string())?;
    let path = d.join(file_name(title));
    // THROUGH THE GUARDED WRITE, like an ordinary save: a template is the same kind of document, and
    // losing it halfway through hurts just as much.
    qymcad_io::save_project_guarded(project, &path.to_string_lossy()).map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().into_owned())
}

/// Delete a template by path (only inside the template directory).
///
/// A FINDING, NOT DEAD CODE: deletion is written and covered by a check, and yet it is NOWHERE in
/// the interface — no button, no line in the language catalogue. A template can be created and
/// cannot be removed except by hand in the file system. The lint is lifted here alone so that the
/// trouble stays visible instead of dissolving into the general silence of a clean build.
#[allow(dead_code)]
pub fn remove(path: &str) -> Result<(), String> {
    let d = dir().ok_or_else(|| crate::i18n::tr("tpl-no-dir"))?;
    let p = PathBuf::from(path);
    // THE DIRECTORY BOUNDARY IS CHECKED rather than assumed: the path comes from the list, but the
    // list is data, and one day it will come from somewhere else.
    if !p.starts_with(&d) {
        return Err(crate::i18n::tr("tpl-outside"));
    }
    std::fs::remove_file(p).map_err(|e| e.to_string())
}
