//! A SCHEME OF ONE'S OWN — THE WHOLE PATH: start it, correct it, write it, read it back, delete it.
//!
//! What is checked is the store, not the screen: the screen is thin wiring over these same calls, while
//! all the machinery that can be broken silently (a file name built from text somebody typed, a broken
//! file, a clash of names) lives here.
use super::*;

/// Every test gets a folder of its own: they run in parallel and through a shared folder would get in
/// each other's way.
fn temp_dir(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("qym_scheme_{tag}"));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("the folder is created");
    d
}

/// THE FILE NAME IS BUILT FROM THE NAME OF THE SCHEME, AND A PERSON TYPES THAT.
///
/// Anything at all can end up in the name, `../../` included, and building a path from it as it stands
/// means allowing a file to be written anywhere. The check is that no trace of a path separator
/// survives.
#[test]
fn a_scheme_name_can_never_escape_its_folder() {
    for evil in ["../../etc/passwd", "..", "/", "a/b\\c", "   ", "…"] {
        let f = store::file_name(evil);
        assert!(!f.contains('/') && !f.contains('\\'), "the name \"{evil}\" gave a path: {f}");
        assert!(f.ends_with(".ron") && f.len() > 4, "the name \"{evil}\" gave an empty file name: {f}");
    }
    assert_eq!(store::file_name("my-scheme-2"), "my-scheme-2.ron", "an ordinary identifier stays recognisable");
}

/// WRITTEN AND READ BACK: the colours and the shading fractions come back the same.
#[test]
fn a_scheme_survives_the_trip_through_a_file() {
    let mut p = light();
    p.id = "probe".into();
    p.name = "Probe".into();
    p.sketch_line = [1, 2, 3];
    p.shade_floor_body = 0.61;
    p.body_saturate = 0.25;
    let text = ron::ser::to_string_pretty(&p, ron::ser::PrettyConfig::default()).expect("written");
    let back: Palette = ron::from_str(&text).expect("read");
    assert_eq!(back.entries(), p.entries(), "not a single colour was lost");
    assert_eq!(back.shade_floor_body, 0.61);
    assert_eq!(back.body_saturate, 0.25);
    assert!(back.light, "the light one stayed light");
}

/// A BROKEN FILE DOES NOT TAKE THE OTHERS WITH IT. One typo in one file has no right to deprive a
/// person of all their schemes — it is reported and the rest are read.
#[test]
fn one_broken_file_does_not_take_the_others_with_it() {
    let d = temp_dir("broken");
    let mut good = dark();
    good.id = "good".into();
    good.name = "Good".into();
    std::fs::write(d.join("good.ron"), ron::ser::to_string(&good).unwrap()).unwrap();
    std::fs::write(d.join("bad.ron"), "(this is not a scheme").unwrap();

    // read with the same code as the store, but over a folder of our own
    let mut ok = 0;
    let mut errs = 0;
    for e in std::fs::read_dir(&d).unwrap().flatten() {
        match std::fs::read_to_string(e.path()).map_err(|x| x.to_string()).and_then(|s| ron::from_str::<Palette>(&s).map_err(|x| x.to_string())) {
            Ok(_) => ok += 1,
            Err(_) => errs += 1,
        }
    }
    assert_eq!((ok, errs), (1, 1), "the good scheme was read and the broken one counted separately");
}

/// AN INCOMPLETE FILE (a scheme from a future version with more fields) still reads: what is missing
/// is taken from the dark one.
#[test]
fn a_partial_file_still_loads() {
    let back: Palette = ron::from_str("(name:\"Stub\",sketch_line:(1,2,3))").expect("it reads");
    assert_eq!(back.sketch_line, [1, 2, 3]);
    assert_eq!(back.viewport_bg, dark().viewport_bg, "what is missing comes from the dark one");
    assert_eq!(back.shade_floor_body, dark().shade_floor_body, "the shading fractions as well");
}

/// BUILT-IN SCHEMES ARE RECOGNISED BY NAME — they are not edited by the editor, a copy of them is.
#[test]
fn builtin_schemes_are_recognised_and_copies_get_a_free_name() {
    assert!(store::is_builtin("dark") && store::is_builtin("light"));
    assert!(!store::is_builtin("my-scheme"), "a scheme of one's own does not count as built in");

    let existing = vec!["dark".to_string(), "dark-1".to_string()];
    let n = store::unique_copy_id("dark", &existing);
    assert!(!existing.contains(&n), "a copy gets an identifier that does not exist yet: {n}");
    assert!(n.starts_with("dark"), "and stays recognisable: {n}");
}

/// A SCHEME OF ONE'S OWN CARRYING THE NAME OF A BUILT-IN ONE DOES NOT REPLACE IT.
///
/// Otherwise a request to put things back the way they were is backed by nothing: the current look was
/// said to be liked, and the dark scheme must stay the same one for everybody.
#[test]
fn a_user_scheme_never_replaces_a_builtin_one() {
    let mut impostor = light();
    impostor.id = "dark".into();
    let mut all = builtin();
    all.push(impostor);
    let first = all.iter().find(|p| p.id == "dark").expect("the built-in one is in place");
    assert_eq!(first.viewport_bg, dark().viewport_bg, "`dark` finds the BUILT-IN one and not the impostor");
    assert_eq!(all.iter().filter(|p| p.id == "dark").count(), 2, "both are visible in the list, so it is plain there are two");
}

/// THE FILE NAME IS READABLE AND COMES FROM THE TITLE.
///
/// Reported behaviour: a scheme was renamed to "Main Light FIX", Save was pressed, and out came a file
/// named after the identifier left over from copying. A file that cannot be recognised in a folder
/// cannot be handed to a colleague either, and that is exactly what schemes lie in files for.
///
/// The non-ASCII title is deliberate test data: the sanitiser must clean the separators and leave the
/// letters alone, whatever alphabet they are written in.
#[test]
fn the_file_is_named_after_the_title_a_human_typed() {
    assert_eq!(store::file_name("Main Light FIX"), "Main_Light_FIX.ron");
    assert_eq!(store::file_name("Светлая (копия)"), "Светлая_копия.ron", "runs of underscores collapse and trailing ones are dropped");
    assert_eq!(store::file_name("   "), "scheme.ron", "an empty name still yields a file");
}

/// A RENAMED SCHEME MOVES RATHER THAN DOUBLING ITSELF.
///
/// This is the heart of the matter. Writing under a new name without removing the former file would put
/// TWO schemes with one identifier into the folder — and which of them loaded would be decided by the
/// order the folder is read in.
///
/// The non-ASCII title here is deliberate test data as well: the round trip must survive it.
#[test]
fn renaming_moves_the_file_instead_of_leaving_a_twin() {
    let d = temp_dir("rename");
    let mut p = light();
    p.id = "light-1".into();
    p.name = "Светлая (копия)".into();
    let first = store::save_in(&d, &p).expect("written");
    assert_eq!(first.file_name().unwrap(), "Светлая_копия.ron");

    p.name = "Main Light FIX".into();
    let second = store::save_in(&d, &p).expect("renamed");
    assert_eq!(second.file_name().unwrap(), "Main_Light_FIX.ron", "the file must move along with the title");
    assert!(!first.exists(), "the former file stayed — two schemes with one identifier are in the folder");
    let files: Vec<_> = std::fs::read_dir(&d).unwrap().flatten().map(|e| e.file_name()).collect();
    assert_eq!(files.len(), 1, "ONE file must be left in the folder, and there are {}: {files:?}", files.len());
}

/// A SCHEME IS FOUND BY ITS IDENTIFIER RATHER THAN BY THE FILE NAME — and deleted the same way.
///
/// Both writing and deleting used to build the file name from a string. The moment the name diverged
/// from the contents (and after a rename it always diverges), Delete found nothing.
#[test]
fn a_scheme_is_found_and_deleted_by_its_id() {
    let d = temp_dir("byid");
    let mut p = dark();
    p.id = "dark-7".into();
    p.name = "Night".into();
    store::save_in(&d, &p).expect("written");
    // the file is renamed by hand on purpose — that happens when a scheme arrives by mail
    std::fs::rename(d.join("Night.ron"), d.join("from-a-colleague.ron")).expect("renamed");

    assert_eq!(store::path_for_id_in(&d, "dark-7").unwrap().file_name().unwrap(), "from-a-colleague.ron", "the scheme is searched for by its contents");
    assert!(store::path_for_id_in(&d, "no-such").is_none(), "what is not there is not there");
    store::delete_in(&d, "dark-7").expect("deleted");
    assert!(!d.join("from-a-colleague.ron").exists(), "the file deleted is the very one the scheme lay in");
    assert!(store::delete_in(&d, "dark-7").is_err(), "a second deletion honestly answers with a refusal");
}

/// SOMEBODY ELSE'S FILE IS NEVER OVERWRITTEN BECAUSE THE TITLES CLASH.
///
/// Two schemes have every right to be called the same (both were named "Mine"), but each has a file of
/// its own. Silently overwriting another scheme is worse than an ugly file name.
#[test]
fn a_clashing_title_never_overwrites_someone_elses_file() {
    let d = temp_dir("clash");
    let mut a = dark();
    a.id = "one".into();
    a.name = "Mine".into();
    let pa = store::save_in(&d, &a).expect("the first one is written");

    let mut b = light();
    b.id = "two".into();
    b.name = "Mine".into();
    let pb = store::save_in(&d, &b).expect("the second one is written");

    assert_ne!(pa, pb, "the second scheme was given a file of its own: {pa:?} / {pb:?}");
    assert_eq!(store::path_for_id_in(&d, "one").unwrap(), pa, "the first one is in place and intact");
    assert_eq!(store::path_for_id_in(&d, "two").unwrap(), pb);
    // and writing the same scheme again does NOT chase it through numbered names
    assert_eq!(store::save_in(&d, &b).expect("rewritten"), pb, "a scheme of one's own does not move on every save");
}
