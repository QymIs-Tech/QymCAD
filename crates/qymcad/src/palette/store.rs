//! CUSTOM SCHEMES — FILES IN THE CONFIG DIRECTORY, by the same means as the languages: copy it, fix
//! it, drop it next to the others. Nothing has to be rebuilt, and a scheme can be shared as one file.
//!
//! The built-in schemes are NOT stored as files and are not overwritten: the dark scheme must stay the
//! same for everyone, otherwise "put it back the way it was" is backed by nothing. A custom scheme
//! carrying the name of a built-in one is a separate scheme, not a replacement: the list shows both,
//! and a person can see there are two.
use super::Palette;
use std::path::PathBuf;

/// The directory with the custom schemes. `None` — the OS has no config directory (that happens in
/// sandboxes).
pub fn dir() -> Option<PathBuf> {
    directories::ProjectDirs::from("tech", "qymis", "qym-cad").map(|d| d.config_dir().join("schemes"))
}

/// The file name for a scheme: letters/digits/hyphen only, everything else becomes an underscore,
/// and runs of underscores collapse. The caption of a scheme is written by a person and may contain
/// anything at all, up to `../` — a path must not be assembled from it.
///
/// THE FILE NAME COMES FROM THE CAPTION, NOT FROM THE IDENTIFIER. The identifier is a machine key
/// (`light-1`), and a file with such a name can neither be recognised in the directory nor sensibly
/// handed to a colleague — and sharing it as one file is exactly what all this was made for. Reported
/// behaviour: a scheme was renamed, Save was pressed, and the file came out named after the
/// identifier left over from the copying, in the language of the original. The link between file and
/// scheme is held not by the name but by the `id` field INSIDE the file (see [`path_for_id`]).
pub fn file_name(title: &str) -> String {
    let mut s = String::new();
    for c in title.chars() {
        if c.is_alphanumeric() || c == '-' {
            s.push(c);
        } else if !s.ends_with('_') {
            s.push('_');
        }
    }
    let s = s.trim_matches('_').to_string();
    if s.is_empty() {
        return "scheme.ron".into();
    }
    format!("{s}.ron")
}

/// The same, but in a GIVEN directory. All the mechanics of the store live in the `*_in` versions,
/// and the `dir()` wrappers over them are thin: otherwise the only way to check it would be replacing
/// HOME for the whole process — and the tests run in parallel, so such a replacement would break the
/// neighbours.
pub fn path_for_id_in(d: &std::path::Path, id: &str) -> Option<PathBuf> {
    let rd = std::fs::read_dir(d).ok()?;
    for e in rd.filter_map(|e| e.ok()) {
        let p = e.path();
        if p.extension().is_none_or(|x| x != "ron") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&p) else { continue };
        if ron::from_str::<Palette>(&text).is_ok_and(|q| q.id == id) {
            return Some(p);
        }
    }
    None
}

/// Read every custom scheme. A broken file is skipped with a message: one typo in one file has no
/// right to deprive a person of the rest of their schemes.
pub fn load_all() -> (Vec<Palette>, Vec<String>) {
    let (mut out, mut errs) = (Vec::new(), Vec::new());
    let Some(d) = dir() else { return (out, errs) };
    let Ok(rd) = std::fs::read_dir(&d) else { return (out, errs) };
    let mut paths: Vec<PathBuf> = rd.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|x| x == "ron")).collect();
    paths.sort();
    for p in paths {
        match std::fs::read_to_string(&p).map_err(|e| e.to_string()).and_then(|s| ron::from_str::<Palette>(&s).map_err(|e| e.to_string())) {
            Ok(mut pal) => {
                // a custom scheme without a caption takes the file name: it must be recognisable in
                // the list
                if pal.name.trim().is_empty() {
                    pal.name = p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "scheme".into());
                }
                if pal.id.trim().is_empty() || is_builtin(&pal.id) {
                    pal.id = pal.name.clone(); // the identifier of a built-in is not given away to another scheme
                }
                out.push(pal);
            }
            Err(e) => errs.push(format!("{}: {e}", p.display())),
        }
    }
    (out, errs)
}

/// Write a custom scheme. Returns the path — it is shown so that the file can be found and handed
/// on.
///
/// A RENAMED SCHEME MOVES INSTEAD OF SPLITTING IN TWO: if the file of this scheme already lies under
/// another name, it is moved. Simply writing into a new file would leave the previous one in the
/// directory — two schemes with one identifier, and which of them loads would be decided by the order
/// the directory is read in.
///
/// A taken name is not taken away from somebody else's file: in that case the write goes where the
/// scheme already lies (and a new one gets a free name with a number). Silently overwriting somebody
/// else's scheme is worse than an ugly file name.
pub fn save(pal: &Palette) -> Result<PathBuf, String> {
    save_in(&dir().ok_or_else(|| crate::i18n::tr("io-no-config-dir"))?, pal)
}

/// The same, but in a GIVEN directory.
pub fn save_in(d: &std::path::Path, pal: &Palette) -> Result<PathBuf, String> {
    std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    let here = path_for_id_in(d, &pal.id);
    let want = d.join(free_name(&d, &file_name(&pal.title()), here.as_deref()));
    let path = match here {
        Some(old) if old != want => {
            // the move comes BEFORE the write: if it fails, the write goes where the scheme already lay
            match std::fs::rename(&old, &want) {
                Ok(()) => want,
                Err(_) => old,
            }
        }
        Some(old) => old,
        None => want,
    };
    let text = ron::ser::to_string_pretty(pal, ron::ser::PrettyConfig::default()).map_err(|e| e.to_string())?;
    std::fs::write(&path, text).map_err(|e| e.to_string())?;
    Ok(path)
}

/// A free file name: `name.ron`, `name-2.ron`, and so on. Somebody else's file is not touched, and
/// our own previous one is not counted as taken (otherwise a scheme would move to a new number on
/// every save).
fn free_name(d: &std::path::Path, want: &str, mine: Option<&std::path::Path>) -> String {
    let taken = |p: &PathBuf| p.exists() && Some(p.as_path()) != mine;
    if !taken(&d.join(want)) {
        return want.to_string();
    }
    let stem = want.trim_end_matches(".ron");
    for n in 2..1000 {
        let cand = format!("{stem}-{n}.ron");
        if !taken(&d.join(&cand)) {
            return cand;
        }
    }
    want.to_string()
}

/// Delete a custom scheme — by identifier, with the same search by content the write uses.
pub fn delete(id: &str) -> Result<(), String> {
    delete_in(&dir().ok_or_else(|| crate::i18n::tr("io-no-config-dir"))?, id)
}

/// The same, but in a GIVEN directory.
pub fn delete_in(d: &std::path::Path, id: &str) -> Result<(), String> {
    let path = path_for_id_in(d, id).ok_or_else(|| crate::i18n::tr1("scheme-file-missing", "name", id))?;
    std::fs::remove_file(path).map_err(|e| e.to_string())
}

/// Is this a built-in scheme — those cannot be edited, only copied.
pub fn is_builtin(id: &str) -> bool {
    super::builtin().iter().any(|p| p.id == id)
}

/// An identifier for a copy that exists neither among the built-in ones nor among the custom ones.
/// Not the caption: a person will fix the caption right away, while the identifier must be free and
/// independent of the language.
pub fn unique_copy_id(base: &str, existing: &[String]) -> String {
    let stem = base.trim_end_matches(|c: char| c.is_ascii_digit() || c == '-').to_string();
    let stem = if stem.trim().is_empty() { "scheme".to_string() } else { stem };
    for n in 1..1000 {
        let candidate = format!("{stem}-{n}");
        if !existing.iter().any(|e| e == &candidate) {
            return candidate;
        }
    }
    format!("{stem}-copy")
}
