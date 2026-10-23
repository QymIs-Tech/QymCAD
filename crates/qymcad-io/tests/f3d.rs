//! THE F3D CONVERTER: looked up on the search path, run once per file, and a failure of its own is said with
//! what the converter itself said. The checks run against a stand-in program, so the machine needs no
//! converter installed.

use std::path::PathBuf;
use qymcad_io::{convert_with, find_in, InOut, CONVERTER};

/// WHAT THE STAND-IN DOES WHEN RUN: it writes the output it is named and leaves, or it says a line and leaves
/// with a code of its own.
enum Answer {
    Writes,
    Refuses { code: i32, line: &'static str },
}

/// A DIRECTORY WITH A PROGRAM IN IT that answers for the converter: on the search path it is found, and when
/// run it does what its `answer` says. Unix only: a program a search path can run is made executable, and a
/// stand-in on Windows would need a shell around it.
#[cfg(unix)]
struct StandIn {
    dir: PathBuf,
}

#[cfg(unix)]
impl StandIn {
    fn standing(answer: Answer) -> StandIn {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("qym-f3d-test-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // the call is `convert <input> -o <output> --force`: the output stands fourth
        let script = match answer {
            Answer::Writes => "#!/bin/sh\nout=\"$4\"\nprintf step > \"$out\"\nexit 0\n".to_string(),
            Answer::Refuses { code, line } => format!("#!/bin/sh\necho '{line}' >&2\nexit {code}\n"),
        };
        let program = dir.join(CONVERTER);
        std::fs::write(&program, script).unwrap();
        let mut perms = std::fs::metadata(&program).unwrap().permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o755);
        std::fs::set_permissions(&program, perms).unwrap();
        StandIn { dir }
    }

    /// The stand-in as a conversion names it: the converter to run, and where its output goes.
    fn files(&self, output: &str) -> InOut {
        InOut { input: "whatever.f3d".to_string(), output: output.to_string() }
    }
}

#[cfg(unix)]
impl Drop for StandIn {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// THE CONVERTER IS FOUND IN THE FIRST DIRECTORY OF THE SEARCH PATH that holds it.
#[cfg(unix)]
#[test]
fn the_converter_is_found_on_the_search_path() {
    let stand = StandIn::standing(Answer::Writes);
    let found = find_in(&std::env::join_paths([&stand.dir]).unwrap());
    assert_eq!(found, Some(stand.dir.join(CONVERTER)), "the program in the search path was not found");
}

/// AN EMPTY SEARCH PATH HOLDS NO CONVERTER: the refusal comes before any file is read.
#[test]
fn an_empty_search_path_holds_no_converter() {
    assert_eq!(find_in(&std::env::join_paths(Vec::<PathBuf>::new()).unwrap()), None, "an empty search path gave a program");
}

/// A DIRECTORY WITHOUT THE PROGRAM IS PASSED OVER, not taken for the answer.
#[cfg(unix)]
#[test]
fn a_directory_without_the_program_is_passed_over() {
    let empty = std::env::temp_dir().join(format!("qym-f3d-empty-{}", std::process::id()));
    std::fs::create_dir_all(&empty).unwrap();
    let stand = StandIn::standing(Answer::Writes);
    let search = std::env::join_paths([&empty, &stand.dir]).unwrap();
    assert_eq!(find_in(&search), Some(stand.dir.join(CONVERTER)), "the second directory was not looked into");
    std::fs::remove_dir(&empty).unwrap();
}

/// A CONVERTED FILE IS WRITTEN where the call names it.
#[cfg(unix)]
#[test]
fn a_conversion_writes_its_output() {
    let stand = StandIn::standing(Answer::Writes);
    let out = std::env::temp_dir().join(format!("qym-f3d-out-{}.step", std::process::id()));
    let _ = std::fs::remove_file(&out);
    convert_with(&stand.dir.join(CONVERTER), &stand.files(&out.to_string_lossy())).unwrap();
    assert_eq!(std::fs::read(&out).unwrap(), b"step", "the output was not written by the converter");
    std::fs::remove_file(&out).unwrap();
}

/// A FAILING CONVERTER IS SAID WITH ITS OWN WORDS: the exit code and the last line it printed, apart by the
/// unit separator the names reader splits into `$v` and `$w`.
#[cfg(unix)]
#[test]
fn a_failing_converter_says_its_own_words() {
    let stand = StandIn::standing(Answer::Refuses { code: 3, line: "error: the body carries no geometry" });
    let said = convert_with(&stand.dir.join(CONVERTER), &stand.files("/nowhere/out.step")).unwrap_err();
    let (key, rest) = said.split_once('#').unwrap();
    assert_eq!(key, "io-f3d-convert-failed");
    let (code, line) = rest.split_once('\u{1f}').unwrap();
    assert_eq!(code, "exit status: 3");
    assert_eq!(line, "error: the body carries no geometry", "the converter's own words were not carried");
}
