//! THE WINDOWS 7 PACKAGE IS REFUSED BEFORE IT REACHES A PERSON, NOT AFTER.
//!
//! Reported behaviour: the program started on Windows 7 and stopped starting after a change of the package, while a
//! run on another system stayed green. The loader of Windows 7 refuses a program for one imported name it does not
//! have, and no other system shows that: msvcp140.dll 14.51 imports CopyFile2, CreateFile2 and
//! GetSystemTimePreciseAsFileTime.
//!
//! `check-win7.py` is fed files built here, byte by byte, so each test states exactly one import.
use std::path::{Path, PathBuf};
use std::process::Command;

fn repo(path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(path)
}

fn read(path: &str) -> String {
    std::fs::read_to_string(repo(path)).unwrap_or_else(|e| panic!("{path} must be readable: {e}"))
}

/// What one imported library gives a file.
struct Import<'a> {
    library: &'a str,
    names: &'a [&'a str],
}

/// The system a file asks for in its header: 6.0 and 6.2, either side of Windows 7, which is 6.1.
#[derive(Clone, Copy)]
enum Asks {
    Vista,
    Windows8,
}

impl Asks {
    fn minor(self) -> u16 {
        match self {
            Asks::Vista => 0,
            Asks::Windows8 => 2,
        }
    }
}

const SECTION_RVA: u32 = 0x1000;
const OPTIONAL_HEADER: usize = 240;
const HEADERS: usize = 0x200;

fn put_u16(file: &mut [u8], at: usize, value: u16) {
    file[at..at + 2].copy_from_slice(&value.to_le_bytes());
}

fn put_u32(file: &mut [u8], at: usize, value: u32) {
    file[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

/// The import section: descriptors, then the lookup table and the names of each library.
fn import_section(imports: &[Import]) -> Vec<u8> {
    let mut section = vec![0u8; 20 * (imports.len() + 1)];
    for (i, import) in imports.iter().enumerate() {
        let lookup = section.len();
        section.resize(lookup + 8 * (import.names.len() + 1), 0);
        for (n, name) in import.names.iter().enumerate() {
            let hint = section.len() as u64 + u64::from(SECTION_RVA);
            section[lookup + 8 * n..lookup + 8 * n + 8].copy_from_slice(&hint.to_le_bytes());
            section.extend_from_slice(&[0, 0]);
            section.extend_from_slice(name.as_bytes());
            section.push(0);
        }
        let library = section.len() as u32 + SECTION_RVA;
        section.extend_from_slice(import.library.as_bytes());
        section.push(0);
        put_u32(&mut section, 20 * i, lookup as u32 + SECTION_RVA);
        put_u32(&mut section, 20 * i + 12, library);
        put_u32(&mut section, 20 * i + 16, lookup as u32 + SECTION_RVA);
    }
    section
}

/// A PE32+ file holding nothing but an import table.
fn pe_file(imports: &[Import], system: Asks) -> Vec<u8> {
    let section = import_section(imports);
    let head = 0x40;
    let optional = head + 24;
    let mut file = vec![0u8; HEADERS];
    file[..2].copy_from_slice(b"MZ");
    put_u32(&mut file, 0x3C, head as u32);
    file[head..head + 4].copy_from_slice(b"PE\0\0");
    put_u16(&mut file, head + 4, 0x8664);
    put_u16(&mut file, head + 6, 1);
    put_u16(&mut file, head + 20, OPTIONAL_HEADER as u16);
    put_u16(&mut file, optional, 0x20B);
    put_u16(&mut file, optional + 40, 6);
    put_u16(&mut file, optional + 42, system.minor());
    put_u16(&mut file, optional + 48, 6);
    put_u16(&mut file, optional + 50, system.minor());
    put_u32(&mut file, optional + 112 + 8, SECTION_RVA);
    put_u32(&mut file, optional + 112 + 12, section.len() as u32);
    let table = optional + OPTIONAL_HEADER;
    put_u32(&mut file, table + 8, section.len() as u32);
    put_u32(&mut file, table + 12, SECTION_RVA);
    put_u32(&mut file, table + 16, section.len() as u32);
    put_u32(&mut file, table + 20, HEADERS as u32);
    file.extend_from_slice(&section);
    file
}

/// What the check says of a folder holding one `qymcad.exe`.
struct Verdict {
    loads: bool,
    said: String,
}

fn check_program(case: &str, imports: &[Import], system: Asks) -> Verdict {
    let folder = std::env::temp_dir().join(format!("qymcad-win7-{}-{case}", std::process::id()));
    std::fs::create_dir_all(&folder).expect("the folder is made");
    std::fs::write(folder.join("qymcad.exe"), pe_file(imports, system)).expect("the program is written");
    let out = Command::new("python3").arg(repo("packaging/win/check-win7.py")).arg(&folder).output().expect("python3 runs");
    std::fs::remove_dir_all(&folder).expect("the folder is removed");
    let said = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    assert!(matches!(out.status.code(), Some(0 | 1)), "the check itself failed:\n{said}");
    Verdict { loads: out.status.success(), said }
}

#[test]
fn a_program_of_functions_windows_7_has_passes() {
    let imports = [Import { library: "KERNEL32.dll", names: &["CreateFileW", "GetTickCount"] }, Import { library: "ole32.dll", names: &["CoTaskMemFree"] }];
    let verdict = check_program("has", &imports, Asks::Vista);
    assert!(verdict.loads, "{}", verdict.said);
}

#[test]
fn each_function_the_runtime_14_51_imports_is_refused() {
    for name in ["CopyFile2", "CreateFile2", "GetSystemTimePreciseAsFileTime"] {
        let verdict = check_program(name, &[Import { library: "KERNEL32.dll", names: &[name] }], Asks::Vista);
        assert!(!verdict.loads && verdict.said.contains(name), "{name} must be refused and named:\n{}", verdict.said);
    }
}

#[test]
fn a_function_kept_behind_a_forward_windows_7_lacks_is_refused() {
    let imports = [Import { library: "api-ms-win-core-synch-l1-2-0.dll", names: &["WaitOnAddress"] }];
    let verdict = check_program("synch", &imports, Asks::Vista);
    assert!(!verdict.loads && verdict.said.contains("WaitOnAddress"), "{}", verdict.said);
}

#[test]
fn a_library_windows_7_does_not_have_is_refused() {
    let imports = [Import { library: "combase.dll", names: &["CoTaskMemFree"] }];
    let verdict = check_program("library", &imports, Asks::Vista);
    assert!(!verdict.loads && verdict.said.contains("combase.dll"), "{}", verdict.said);
}

#[test]
fn a_file_asking_for_a_newer_system_is_refused() {
    let imports = [Import { library: "KERNEL32.dll", names: &["CreateFileW"] }];
    let verdict = check_program("header", &imports, Asks::Windows8);
    assert!(!verdict.loads && verdict.said.contains("6.2"), "{}", verdict.said);
}

/// EVERY FORWARD OF THE combase.dll WE CARRY LEADS TO A FUNCTION OF WINDOWS 7.
#[test]
fn the_forwarder_of_combase_leads_to_what_ole32_has() {
    let exports = read("packaging/win/win7-exports.txt");
    let ole32: Vec<&str> = exports.lines().skip_while(|l| *l != "[ole32.dll]").skip(1).take_while(|l| !l.starts_with('[')).filter_map(|l| l.split(' ').next()).collect();
    let source = read("packaging/win/combase_forwarder.c");
    let forwards: Vec<&str> = source.lines().filter_map(|l| l.split("=ole32.").nth(1)).filter_map(|l| l.split('"').next()).collect();
    assert!(!forwards.is_empty(), "the forwarder forwards nothing");
    let missing: Vec<&&str> = forwards.iter().filter(|f| !ole32.contains(f)).collect();
    assert!(missing.is_empty(), "ole32.dll of Windows 7 lacks {missing:?}");
}

/// The text of one job of the release workflow.
fn job(name: &str) -> String {
    let flow = read(".github/workflows/release.yml");
    let header = format!("\n  {name}:\n");
    let start = flow.find(&header).unwrap_or_else(|| panic!("the release workflow has no job `{name}`")) + header.len();
    let rest = &flow[start..];
    let is_next_job = |l: &str| l.starts_with("  ") && !l.starts_with("   ") && l.trim_end().ends_with(':');
    let end = rest.match_indices('\n').map(|(at, _)| at + 1).find(|at| is_next_job(rest[*at..].lines().next().unwrap_or("")));
    rest[..end.unwrap_or(rest.len())].to_string()
}

fn cache_keys(job: &str) -> Vec<&str> {
    job.lines().filter_map(|l| l.trim().strip_prefix("key: ")).collect()
}

/// `windows-latest` moved to the compiler whose runtime Windows 7 cannot load.
#[test]
fn the_windows_7_job_names_its_compiler_and_keeps_its_own_kernel() {
    let flow = read(".github/workflows/release.yml");
    let legacy = job("windows7");
    let usual = job("windows");
    assert!(legacy.contains("runs-on: windows-2022"), "the image decides the runtime, so it is named");
    assert!(legacy.contains(r#"-G "Visual Studio 17 2022""#), "the kernel must be built by the compiler of the program");
    assert!(legacy.contains("--target x86_64-win7-windows-msvc"), "the usual target needs WaitOnAddress and ProcessPrng");
    let keys = cache_keys(&legacy);
    let usual_keys = cache_keys(&usual);
    let shared: Vec<&&str> = keys.iter().filter(|k| usual_keys.contains(k)).collect();
    assert!(!keys.is_empty() && shared.is_empty(), "a shared key restores a kernel built by the other compiler: {shared:?}");
    let publish = flow.lines().find(|l| l.trim().starts_with("needs: [windows")).expect("the publishing job waits for the builds");
    assert!(publish.contains("windows7"), "the package would be published without waiting for it: {publish}");
}

/// A PACKAGE IS CHECKED BEFORE IT IS PACKED, AND ONLY THE BINARY OF ITS OWN TARGET GOES IN.
///
/// The usual `target/release/qymcad.exe` as a fallback would put a program for Windows 10 into this archive silently.
#[test]
fn the_bundle_checks_and_takes_no_other_binary() {
    let script = read("packaging/win/bundle-win7.py");
    let checked = script.find(r#""check-win7.py""#).expect("the bundle must run the check");
    let packed = script.rfind("pack(").expect("the bundle packs");
    assert!(checked < packed, "the check must run before the archive is made");
    assert!(script.contains(r#""x86_64-win7-windows-msvc", "release", "qymcad.exe""#), "the binary of the Windows 7 target goes in");
    assert_eq!(script.matches("qymcad.exe\"").count(), 1, "one binary path, no fallback");
}
