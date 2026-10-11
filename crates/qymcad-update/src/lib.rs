//! IS THERE A NEWER VERSION, AND WHAT SHOULD BE SAID ABOUT IT.
//!
//! A program that never says a new version exists leaves people on the one they installed. Measured on
//! this project in September 2026: 470 downloads, 300 clones of the repository and 10-20 people in the
//! chat - so the overwhelming majority of those running it are reachable by nothing at all, and every
//! fix made since their download is invisible to them.
//!
//! WHAT THIS MODULE IS NOT. It does not unpack or replace anything it does not own. Behind
//! self-replacement of a copy owned by somebody else come resuming a broken transfer, checksums,
//! write permissions, the macOS quarantine flag and an argument with an antivirus on Windows; a
//! browser does all of that better and every person already has one. This asks one question, and
//! where the copy cannot replace itself a button opens the release page.
//!
//! WHAT IT DOES BESIDES ASKING. A copy that CAN replace itself - a single AppImage file, or an
//! unpacked archive standing outside every system folder - downloads the release file for its own
//! platform in the background and puts it where it stands, then says that a restart applies it.
//! Anything owned by an installer, a package manager, a store or a sandbox (`Msi`, `System`, `App`,
//! `Flatpak`) is left alone: the release page button stays the whole story there.
//!
//! WHY OUR OWN ADDRESS AND NOT THE PLATFORM'S API. `ENDPOINT` freezes into every binary ever handed
//! out - it cannot be renamed later, because copies installed today will keep knocking at the old one
//! for years. While it is ours, where the answer comes FROM can move without stranding them.
//!
//! It is answered by the site, from a record filled in by hand, and not by the release run. So a release
//! nobody has announced does not exist for this module: when to tell people about a version is a
//! person's decision, not a build's.

/// WHERE THE ANSWER COMES FROM. Written once and never renamed - see the note above.
pub const ENDPOINT: &str = "https://cad.qymis.tech/latest.json";

/// HOW THE COPY WAS INSTALLED, because the answer decides whether to ask at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Install {
    /// A single AppImage file the person keeps wherever they like.
    AppImage,
    /// A package of the system: AUR, or anything else that put us under `/usr` or `/opt`.
    System,
    /// The Windows installer, which is also what `winget` installs.
    Msi,
    /// A macOS `.app` bundle.
    App,
    /// An unpacked archive, or a build somebody made themselves.
    Portable,
    /// Inside a Flatpak sandbox, where there is no network at all.
    Flatpak,
}

impl Install {
    /// EVERY KIND, so a walk over them cannot silently miss one added later.
    pub const ALL: [Install; 6] = [Install::AppImage, Install::System, Install::Msi, Install::App, Install::Portable, Install::Flatpak];

    /// The word that goes into the request signature. ASCII, lower case, one word.
    pub fn word(self) -> &'static str {
        match self {
            Install::AppImage => "appimage",
            Install::System => "system",
            Install::Msi => "msi",
            Install::App => "app",
            Install::Portable => "portable",
            Install::Flatpak => "flatpak",
        }
    }

    /// CAN THIS COPY ASK AT ALL.
    ///
    /// Inside Flatpak it cannot: the manifest carries no `--share=network`, and it will not - the store
    /// updates the package and tells the person itself, while asking for the network permission is a
    /// question the reviewers put separately. The request would simply never leave, so the menu item,
    /// the setting and the badge are absent there rather than present and dead.
    pub fn may_ask(self) -> bool {
        self != Install::Flatpak
    }

    /// WHICH OF THEM THIS IS, decided from what the environment already says.
    ///
    /// Nothing here is a guess dressed up as a fact: each answer comes from something the installer or
    /// the runtime put there. What cannot be told apart honestly falls through to `Portable`, which asks
    /// the question anyway and points at the release page - the safe end of being wrong.
    pub fn from_env() -> Install {
        install_from(std::env::var("APPIMAGE").ok().as_deref(), std::path::Path::new("/.flatpak-info").exists(), std::env::current_exe().ok().as_deref())
    }
}

/// The decision itself, taken from given facts so that all six cases can be asked about on one machine.
pub fn install_from(appimage: Option<&str>, in_flatpak: bool, exe: Option<&std::path::Path>) -> Install {
    // The sandbox first: inside it the other marks may also be present, and it outranks them all.
    if in_flatpak {
        return Install::Flatpak;
    }
    // `APPIMAGE` is set by the AppImage runtime itself, not by us.
    if appimage.is_some_and(|p| !p.is_empty()) {
        return Install::AppImage;
    }
    let Some(exe) = exe else { return Install::Portable };
    let path = exe.to_string_lossy();
    if cfg!(target_os = "macos") && path.contains(".app/Contents/") {
        return Install::App;
    }
    // Windows has no marker of its own without reading the registry, and reading it would cost another
    // dependency for one boolean. The installer is the only thing that writes under Program Files - an
    // unpacked archive lands in Downloads, on a stick, next to a project - so the folder answers it.
    if cfg!(target_os = "windows") {
        let low = path.to_lowercase();
        return if low.contains("\\program files") { Install::Msi } else { Install::Portable };
    }
    if path.starts_with("/usr/") || path.starts_with("/opt/") {
        return Install::System;
    }
    Install::Portable
}

/// THE SIGNATURE OF THE REQUEST: `QymCAD/<version> (<os>-<arch>; <install>)`.
///
/// The request goes out anyway; this only decides whether it is signed. Signed, the site's ordinary log
/// answers a question nothing else here answers - how many copies are alive, of which versions, on which
/// systems - and it answers it without a tracker, an identifier or a third party.
///
/// WHAT IS NOT IN IT: no name, no machine, no number issued by us, nothing that tells one copy from
/// another. Three facts, and all three are already written on the download page. The honest other side
/// is that a log like this counts REQUESTS, not people.
pub fn user_agent(version: &str, install: Install) -> String {
    format!("QymCAD/{version} ({}-{}; {})", std::env::consts::OS, std::env::consts::ARCH, install.word())
}

/// WHAT THE SITE ANSWERED. Every field but the first two may be missing, and missing is the normal case.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Latest {
    /// The release tag, e.g. `v0.2.0-dev.20260920`.
    pub latest: String,
    /// The page a person is sent to.
    pub url: String,
    /// `YYYY-MM-DD`, shown beside the version.
    pub published: Option<String>,
    /// The first lines of what changed.
    pub notes: Option<String>,
    /// A free line for the one day something has to be said besides a version number.
    pub notice: Option<String>,
    /// "Anything below this tag is unfit" - for the day a release turns out bad.
    pub broken_below: Option<String>,
}

/// READING THE ANSWER, and refusing it without a word when it is not one.
///
/// The site is an ordinary web application, and an ordinary web application answers an error with a page
/// of HTML. That is NOT a reason to disturb anybody: nothing understood means nothing shown, until the
/// next check. The same goes for JSON that parses but carries no tag.
pub fn parse(body: &str) -> Option<Latest> {
    let v: serde_json::Value = serde_json::from_str(body).ok()?;
    let text = |key: &str| -> Option<String> {
        // absent, `null` and "" are ONE case: a field left empty in the form must not reach the window
        // as a blank line. Whitespace counts as empty for the same reason.
        let s = v.get(key)?.as_str()?.trim();
        (!s.is_empty()).then(|| s.to_string())
    };
    let latest = text("latest")?;
    let url = text("url")?;
    Some(Latest { latest, url, published: text("published"), notes: text("notes"), notice: text("notice"), broken_below: text("broken_below") })
}

/// THE DATE INSIDE A TAG, which is the whole of version comparison here.
///
/// Tags are ours to name and they carry the day: `v0.1.0-dev.20260828`. That number rises whatever the
/// channel is called - `dev` today, `alpha` or `beta` later - so comparing versions is comparing one
/// integer, and no reading of semantic-version rules is needed.
///
/// The day we move to plain numbers (`0.2.0`), the rule is written into the LAST release before the
/// move: it still has a date, the next one does not, and "the one with a date is older" is settled
/// there once.
pub fn date_of(tag: &str) -> Option<u32> {
    // the last run of exactly eight digits, so that `0.1.0` alone gives nothing
    let bytes = tag.as_bytes();
    let mut found = None;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_digit() {
            let start = i;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
            if i - start == 8 {
                found = tag[start..i].parse::<u32>().ok();
            }
        } else {
            i += 1;
        }
    }
    found
}

/// IS `theirs` NEWER THAN `ours`.
///
/// Unreadable on either side means NO. A build made from a checkout has no date to compare, and a tag of
/// a shape we have not seen is not something to announce as an update: the program must never call an
/// update what it did not understand.
pub fn is_newer(theirs: &str, ours: &str) -> bool {
    match (date_of(theirs), date_of(ours)) {
        (Some(t), Some(o)) => t > o,
        _ => false,
    }
}

/// IS THIS COPY ONE OF THE VERSIONS DECLARED UNFIT.
///
/// Empty or unreadable is not unfit - the same rule as everywhere here: what is not understood is not
/// acted upon.
pub fn is_unfit(ours: &str, broken_below: Option<&str>) -> bool {
    match (broken_below.and_then(date_of), date_of(ours)) {
        (Some(b), Some(o)) => o < b,
        _ => false,
    }
}

/// HOW LONG THE ANSWER MAY BE. The record is a few hundred bytes; anything larger is not it.
///
/// Not a formality: without a cap, an address that answers with something endless - a captive network's
/// portal, a proxy gone wrong, a mistaken route - would be read into memory until it ran out.
const BODY_CAP: u64 = 64 * 1024;

/// ASKING, and coming back with nothing rather than with a complaint.
///
/// EVERY failure is the same failure here: no network, a name that does not resolve, a timeout, an
/// error page, an answer that is not our record. None of them is the person's business and none is
/// worth a word on screen - the check simply found nothing, and the next one will ask again.
///
/// THIS BLOCKS. It is meant to be called on a thread of its own, the way the file jobs already are:
/// the launch must not wait for the network for a single moment.
pub fn fetch(version: &str, install: Install) -> Option<Latest> {
    if !install.may_ask() {
        return None; // inside a sandbox with no network the request would never leave
    }
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(10)))
        .user_agent(user_agent(version, install))
        // The site may answer with a move to another address (http -> https, a trailing slash), and
        // one hop is all that is legitimate here.
        .max_redirects(2)
        .build();
    let agent: ureq::Agent = config.into();
    let mut response = agent.get(ENDPOINT).call().ok()?;
    let body = response.body_mut().with_config().limit(BODY_CAP).read_to_string().ok()?;
    parse(&body)
}

/// WHAT THE CHECK CAME TO, as far as anything on screen is concerned.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Outcome {
    /// Nobody has asked yet.
    #[default]
    Idle,
    /// A thread is asking right now.
    Asking,
    /// Asked, and this build is the current one.
    UpToDate,
    /// Asked, and there is something newer.
    Found(Latest),
    /// Asked and got nowhere: no network, no answer, an answer that was not ours.
    ///
    /// KEPT APART FROM `UpToDate` although both mean "nothing to show". A person who pressed "check for
    /// updates" is owed the difference: "no updates" and "could not reach the site" are two different
    /// pieces of news, and telling the second as the first is a lie the program tells about itself.
    Unreachable,
}

/// THE CHECK, RUNNING BESIDE THE PROGRAM RATHER THAN IN FRONT OF IT.
///
/// The whole of it is one small GET, and it still must not happen on the drawing thread: a name that
/// does not resolve takes as long as the system's resolver decides to take, and a window frozen at
/// start-up for that is a far worse fault than never mentioning a new version at all.
///
/// So it is the same shape the file jobs already have - a thread, a channel, and a frame that asks
/// whether anything arrived without ever waiting.
#[derive(Default)]
pub struct Checker {
    waiting: Option<std::sync::mpsc::Receiver<Option<Latest>>>,
    outcome: Outcome,
}

impl Checker {
    pub const fn new() -> Self {
        Checker { waiting: None, outcome: Outcome::Idle }
    }

    /// What to show. Never blocks, never asks the network.
    pub fn outcome(&self) -> &Outcome {
        &self.outcome
    }

    /// GO AND ASK, on a thread of its own.
    ///
    /// Asking again while an answer is still on its way does nothing: a person leaning on the menu item
    /// would otherwise start a thread per press.
    pub fn start(&mut self, version: String, install: Install) {
        if self.waiting.is_some() {
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            // the answer is sent whatever it is; a receiver that has gone away is not our business
            let _ = tx.send(fetch(&version, install));
        });
        self.waiting = Some(rx);
        self.outcome = Outcome::Asking;
    }

    /// HAS ANYTHING ARRIVED. Returns whether the outcome changed, so a frame knows to repaint.
    ///
    /// `try_recv` and not `recv`: this is called from the drawing thread, and the answer usually is not
    /// there yet.
    pub fn poll(&mut self, ours: &str) -> bool {
        let Some(rx) = &self.waiting else { return false };
        match rx.try_recv() {
            Ok(answer) => {
                self.waiting = None;
                self.outcome = match answer {
                    Some(latest) if is_newer(&latest.latest, ours) => Outcome::Found(latest),
                    Some(_) => Outcome::UpToDate,
                    None => Outcome::Unreachable,
                };
                true
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => false,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                // the thread died without sending - the same news as not reaching the site
                self.waiting = None;
                self.outcome = Outcome::Unreachable;
                true
            }
        }
    }

    /// Feed an answer in without a network, so the whole path can be asked about in a test.
    #[cfg(test)]
    fn answered(&mut self, answer: Option<Latest>, ours: &str) {
        let (tx, rx) = std::sync::mpsc::channel();
        tx.send(answer).expect("the channel takes it");
        self.waiting = Some(rx);
        self.poll(ours);
    }
}

/// WHERE INSTALLABLE FILES LIVE: the release host keeps one page per tag, and the files on it
/// are named by that tag (`qymcad-0.1.0-dev.20261001-x86_64.AppImage`). The address is derived, not
/// announced: the check still goes to ENDPOINT, and only a newer tag sends anybody here.
pub const RELEASES: &str = "https://github.com/QymIs-Tech/QymCAD/releases/download";

/// WHAT THIS COPY IS, AS FAR AS A DOWNLOAD IS CONCERNED: the system, its word size, and how it was
/// installed. One record rather than three values travelling side by side.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub os: String,
    pub arch: String,
    pub install: Install,
}

impl Target {
    /// The copy that is running.
    pub fn current() -> Target {
        Target { os: std::env::consts::OS.to_string(), arch: std::env::consts::ARCH.to_string(), install: Install::from_env() }
    }

    /// CAN A NEW VERSION REPLACE THIS COPY WITHOUT ANYBODY'S HELP.
    ///
    /// Decided by the plan below, so the two cannot drift apart: whatever the plan calls
    /// `Impossible` is not attempted.
    pub fn can_self_install(&self) -> bool {
        !matches!(install_plan(self), InstallPlan::Impossible)
    }
}

/// THE FILE A RELEASE CARRIES FOR THIS COPY, named by the tag with its leading `v` dropped - the
/// release run names packages that way (`qymcad-0.1.0-dev.20261001-x86_64.AppImage`).
///
/// `None` where no file is built for the combination (Linux on anything but `x86_64`) or where
/// the kind names no file of its own. A tag that cannot appear
/// in a file name - carrying a slash, a backslash or whitespace - is refused: the tag is filled in
/// by hand on the site, and a hand slips.
pub fn asset_name(tag: &str, target: &Target) -> Option<String> {
    let base = tag.strip_prefix('v').unwrap_or(tag);
    if base.is_empty() || base.bytes().any(|b| b == b'/' || b == b'\\' || b.is_ascii_whitespace()) {
        return None;
    }
    let name = match (target.os.as_str(), target.arch.as_str(), target.install) {
        ("linux", "x86_64", Install::AppImage | Install::Portable | Install::System) => format!("qymcad-{base}-x86_64.AppImage"),
        ("windows", "x86_64", Install::Portable) => format!("qymcad-{base}-win64.zip"),
        ("windows", "x86_64", Install::Msi) => format!("qymcad-{base}-x64.msi"),
        // The archive names the processor the way `uname -m` does (`arm64`, not `aarch64`) -
        // `bundle.sh` takes it from `MACOS_ARCH` or from there, and refuses any other.
        ("macos", "aarch64", Install::App | Install::Portable) => format!("qymcad-{base}-macos-arm64.zip"),
        ("macos", "x86_64", Install::App | Install::Portable) => format!("qymcad-{base}-macos-x86_64.zip"),
        _ => return None,
    };
    Some(name)
}

/// THE ADDRESS OF THAT FILE, on the release host under the page of its tag.
pub fn download_url(tag: &str, target: &Target) -> Option<String> {
    asset_name(tag, target).map(|file| format!("{RELEASES}/{tag}/{file}"))
}

/// HOW THE NEW VERSION REACHES THIS COPY, decided before anything is downloaded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallPlan {
    /// The asset IS the program: download it and put it where this copy stands.
    ReplaceFile,
    /// The asset is an archive of the folder this copy stands in: download it now, unpack it over
    /// that folder when the restart is pressed.
    UnpackZip,
    /// This copy is owned by something else and no background work may touch it: the installer copy
    /// needs elevation and is locked while it runs, the system one belongs to a package manager,
    /// the macOS bundle carries a quarantine flag and a signature, and the sandbox has no network
    /// at all. A macOS `Portable` copy joins them: its release file holds a whole `.app` bundle,
    /// not a file to stand in for the executable. The release page button stays the whole story.
    Impossible,
}

/// THE PLAN FOR THIS COPY, asked of given facts so every combination can be covered by a test on
/// one machine.
pub fn install_plan(target: &Target) -> InstallPlan {
    match target.install {
        Install::AppImage => InstallPlan::ReplaceFile,
        Install::Portable if target.os == "windows" => InstallPlan::UnpackZip,
        Install::Portable if target.os == "macos" => InstallPlan::Impossible,
        Install::Portable => InstallPlan::ReplaceFile,
        Install::System | Install::Msi | Install::App | Install::Flatpak => InstallPlan::Impossible,
    }
}

/// THE FILE A RESTART LAUNCHES, AND THE FILE AN UPDATE REPLACES: the AppImage itself when running
/// as one (the executable then lies inside its mount, and replacing that would change nothing),
/// otherwise the executable that is running.
pub fn program_target(appimage: Option<&str>, exe: Option<&std::path::Path>) -> Option<std::path::PathBuf> {
    match appimage {
        Some(path) if !path.is_empty() => Some(std::path::PathBuf::from(path)),
        _ => exe.map(|p| p.to_path_buf()),
    }
}

/// The same, read from the environment.
pub fn current_program_target() -> Option<std::path::PathBuf> {
    program_target(std::env::var("APPIMAGE").ok().as_deref(), std::env::current_exe().ok().as_deref())
}

/// HOW LARGE A PACKAGE MAY BE. The AppImage measures around 40 MB and the Windows archive around
/// 80; anything past 256 MB is not one of ours - a captive portal's endless answer, a proxy gone
/// wrong.
const PACKAGE_CAP: u64 = 256 * 1024 * 1024;

/// FETCH A PACKAGE INTO A FILE. Blocks; it runs on the install thread, never on the drawing one.
/// Ten minutes, not ten seconds like the version check: a package is tens of megabytes, and a slow
/// line that answers at last beats a fast refusal.
///
/// The signature carries no version: the check announced this copy seconds earlier, and the release
/// host counts downloads by file anyway. What travels here is bytes, not statistics.
pub fn download_to(url: &str, dest: &std::path::Path) -> std::io::Result<()> {
    let config = ureq::Agent::config_builder().timeout_global(Some(std::time::Duration::from_secs(600))).user_agent("QymCAD/background").max_redirects(2).build();
    let agent: ureq::Agent = config.into();
    let mut response = agent.get(url).call().map_err(|e| std::io::Error::other(e.to_string()))?;
    if !(200..300).contains(&response.status().as_u16()) {
        return Err(std::io::Error::other(format!("the release host answered {}", response.status())));
    }
    let mut out = std::fs::File::create(dest)?;
    let mut body = response.body_mut().with_config().limit(PACKAGE_CAP).reader();
    let bytes = std::io::copy(&mut body, &mut out)?;
    if bytes == 0 {
        return Err(std::io::Error::other("the release host answered nothing"));
    }
    Ok(())
}

/// THE SUFFIX OF WHAT THE LAST UPDATE MOVED ASIDE. A replaced file that does not start leaves no
/// way back otherwise; the next start sweeps these away (`cleanup_backups`).
const BACKUP_SUFFIX: &str = ".qymcad-old";

/// The path beside the given one carrying the backup suffix.
fn backup_of(path: &std::path::Path) -> std::path::PathBuf {
    let mut kept = path.as_os_str().to_owned();
    kept.push(BACKUP_SUFFIX);
    std::path::PathBuf::from(kept)
}

/// PUT THE DOWNLOADED FILE WHERE THIS COPY STANDS. The new file lands beside the target first and
/// is renamed over it, so a half-written download never stands where the program is. On Unix the
/// executable bit is set: a file that arrived without it does not start.
pub fn replace_file(staged: &std::path::Path, target: &std::path::Path) -> std::io::Result<()> {
    let mut fresh = target.as_os_str().to_owned();
    fresh.push(".qymcad-new");
    let fresh = std::path::PathBuf::from(fresh);
    std::fs::copy(staged, &fresh)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&fresh, std::fs::Permissions::from_mode(0o755))?;
    }
    if target.exists() {
        let backup = backup_of(target);
        let _ = std::fs::remove_file(&backup); // a leftover of an older update must not block this one
        std::fs::rename(target, &backup)?;
    }
    let done = std::fs::rename(&fresh, target);
    if done.is_err() {
        let _ = std::fs::remove_file(&fresh);
    }
    done
}

/// SWEEP WHAT THE LAST UPDATE MOVED ASIDE: every `*.qymcad-old` file directly inside the folder.
/// Returns how many went. Runs once per start, before anything else touches the folder.
pub fn cleanup_backups(dir: &std::path::Path) -> usize {
    let Ok(entries) = std::fs::read_dir(dir) else { return 0 };
    let mut swept = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        let is_backup = path.is_file() && path.extension().is_some_and(|e| e == BACKUP_SUFFIX.trim_start_matches('.'));
        if is_backup && std::fs::remove_file(&path).is_ok() {
            swept += 1;
        }
    }
    swept
}

/// One entry of a staged archive: where it stands inside the archive and where it lands.
struct EntrySpot {
    index: usize,
    path: std::path::PathBuf,
}

/// UNPACK A STAGED RELEASE ARCHIVE OVER THE FOLDER THE PROGRAM RUNS FROM. Every file the archive
/// brings that already exists is moved aside first - the files of a running program are locked
/// against overwriting on Windows, but not against renaming - so the new ones land whole. An entry
/// that would escape the folder is refused outright. On any failure the moved files go back, the
/// half-written new ones go away, and the error answers: the copy on disk is the old one, entire.
pub fn apply_staged_zip(zip_path: &std::path::Path, exe_path: &std::path::Path) -> std::io::Result<()> {
    let dir = exe_path.parent().ok_or_else(|| std::io::Error::other("the program stands nowhere to unpack beside"))?;
    let file = std::fs::File::open(zip_path)?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| std::io::Error::other(e.to_string()))?;
    let mut spots = Vec::new();
    for index in 0..zip.len() {
        let entry = zip.by_index(index).map_err(|e| std::io::Error::other(e.to_string()))?;
        if entry.is_dir() {
            continue;
        }
        let Some(path) = entry.enclosed_name() else {
            return Err(std::io::Error::other("an entry that escapes its folder"));
        };
        spots.push(EntrySpot { index, path });
    }
    if spots.is_empty() {
        return Err(std::io::Error::other("an archive with no files in it"));
    }
    let mut asides: Vec<std::path::PathBuf> = Vec::new(); // homes whose file was moved aside, for the way back
    for spot in &spots {
        let home = dir.join(&spot.path);
        if home.is_file() {
            let aside = backup_of(&home);
            let _ = std::fs::remove_file(&aside);
            if std::fs::rename(&home, &aside).is_err() {
                rewind_zip_apply(dir, &asides, &[]);
                return Err(std::io::Error::other(format!("{} would not move aside", home.display())));
            }
            asides.push(home);
        }
    }
    let mut created: Vec<std::path::PathBuf> = Vec::new();
    for spot in &spots {
        let home = dir.join(&spot.path);
        let outcome = (|| {
            if let Some(parent) = home.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let fresh = !home.exists();
            let mut entry = zip.by_index(spot.index).map_err(|e| std::io::Error::other(e.to_string()))?;
            let mut out = std::fs::File::create(&home)?;
            std::io::copy(&mut entry, &mut out)?;
            Ok(fresh)
        })();
        match outcome {
            Ok(fresh) => {
                if fresh {
                    created.push(home);
                }
            }
            Err(fault) => {
                rewind_zip_apply(dir, &asides, &created);
                return Err(fault);
            }
        }
    }
    Ok(())
}

/// THE WAY BACK for an unpacking that failed halfway: the moved files return home, the half-written
/// new ones go away. Best effort throughout - it runs while something is already wrong, and a second
/// error must not bury the first.
fn rewind_zip_apply(_dir: &std::path::Path, asides: &[std::path::PathBuf], created: &[std::path::PathBuf]) {
    for made in created {
        let _ = std::fs::remove_file(made);
    }
    for home in asides.iter().rev() {
        let aside = backup_of(home);
        let _ = std::fs::rename(&aside, home);
    }
}

/// HOW THE NEW COPY IS LAUNCHED: the same file with the same arguments this one was given. Built
/// apart so a test can ask what a restart would run without running it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestartCmd {
    pub program: std::path::PathBuf,
    pub args: Vec<String>,
}

/// The launch for the given program file.
pub fn restart_cmd(program: &std::path::Path) -> RestartCmd {
    RestartCmd { program: program.to_path_buf(), args: std::env::args().skip(1).collect() }
}

/// LEAVE AND LET THE NEW COPY CARRY ON. The child is spawned detached - its parent going away is
/// the plan, not an accident - and this process ends however the spawn came to: staying would run
/// the old copy beside the new one.
pub fn restart_now(cmd: &RestartCmd) -> ! {
    let _ = std::process::Command::new(&cmd.program).args(&cmd.args).spawn();
    std::process::exit(0);
}

/// FETCHING ONE FILE: its address and where the download waits.
#[derive(Debug, Clone)]
pub struct FetchJob {
    pub url: String,
    pub dest: std::path::PathBuf,
}

/// APPLYING IT: how, and to which standing copy.
#[derive(Debug, Clone)]
pub struct ApplyJob {
    pub plan: InstallPlan,
    pub program: std::path::PathBuf,
}

/// THE JOB ITSELF, decided up front so the thread never has to ask the interface anything.
#[derive(Debug, Clone)]
pub struct StageJobs {
    pub fetch: FetchJob,
    pub apply: ApplyJob,
}

/// EVERYTHING THE BACKGROUND THREAD NEEDS, gathered on the drawing thread before it starts.
#[derive(Debug, Clone)]
pub struct StageEnv {
    pub target: Target,
    pub program: Option<std::path::PathBuf>,
    pub workdir: std::path::PathBuf,
}

impl StageEnv {
    /// The copy that is running, and the folder its download waits in.
    pub fn current() -> StageEnv {
        StageEnv { target: Target::current(), program: current_program_target(), workdir: std::env::temp_dir().join("qymcad-update") }
    }
}

/// THE JOB FOR THIS TAG ON THIS COPY, or nothing when there is no file for it or no standing copy
/// to put it into. Nothing here touches the disk or the network; the thread does that.
pub fn stage_jobs(tag: &str, env: &StageEnv) -> Option<StageJobs> {
    let plan = install_plan(&env.target);
    if matches!(plan, InstallPlan::Impossible) {
        return None;
    }
    let file = asset_name(tag, &env.target)?;
    let program = env.program.clone()?;
    let download = env.workdir.join(format!("pending-{file}"));
    Some(StageJobs { fetch: FetchJob { url: format!("{RELEASES}/{tag}/{file}"), dest: download }, apply: ApplyJob { plan, program } })
}

/// WHAT THE BACKGROUND THREAD CAME BACK WITH.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FetchDone {
    /// The standing copy is replaced; only a restart still stands between the person and it.
    Applied,
    /// The archive waits on disk; unpacking it over the folder is the restart button's job.
    ZipStaged(std::path::PathBuf),
    /// Nothing was fetched or replaced - no file for this copy, no standing copy found, the
    /// network gave nothing, or the disk refused. The release page button stays the whole story.
    Skipped,
}

/// THE ANSWER, tagged so the frame knows which version it belongs to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageReport {
    pub tag: String,
    pub done: FetchDone,
}

/// FETCH, THEN STAGE: the whole of the background thread. Whatever goes wrong on the way answers
/// `Skipped` rather than complaining - nobody pressed anything, so there is nobody to complain to.
pub fn fetch_and_stage(jobs: &StageJobs, tag: &str) -> StageReport {
    let done = (|| -> std::io::Result<FetchDone> {
        if matches!(jobs.apply.plan, InstallPlan::Impossible) {
            return Ok(FetchDone::Skipped);
        }
        if let Some(parent) = jobs.fetch.dest.parent() {
            std::fs::create_dir_all(parent).map_err(|e| std::io::Error::other(e.to_string()))?;
        }
        download_to(&jobs.fetch.url, &jobs.fetch.dest)?;
        match jobs.apply.plan {
            InstallPlan::ReplaceFile => {
                let applied = replace_file(&jobs.fetch.dest, &jobs.apply.program);
                let _ = std::fs::remove_file(&jobs.fetch.dest); // the download has no further use either way
                applied.map_err(|e| std::io::Error::other(e.to_string()))?;
                Ok(FetchDone::Applied)
            }
            InstallPlan::UnpackZip => Ok(FetchDone::ZipStaged(jobs.fetch.dest.clone())),
            InstallPlan::Impossible => Ok(FetchDone::Skipped),
        }
    })();
    match done {
        Ok(done) => StageReport { tag: tag.to_string(), done },
        Err(_) => {
            let _ = std::fs::remove_file(&jobs.fetch.dest); // a half-fetched package must not wait for the restart
            StageReport { tag: tag.to_string(), done: FetchDone::Skipped }
        }
    }
}

/// WHAT THE BACKGROUND UPDATE CAME TO, as far as anything on screen is concerned.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum InstallState {
    /// Nothing fetched, nothing waiting.
    #[default]
    Idle,
    /// A thread is fetching the package right now.
    Downloading,
    /// The new version waits: replaced already, or staged for the restart button.
    Ready(ReadyInfo),
}

/// THE WAITING VERSION: its tag, and the staged archive when unpacking is the restart's job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadyInfo {
    pub tag: String,
    pub staged_zip: Option<std::path::PathBuf>,
}

/// THE BACKGROUND UPDATE, RUNNING BESIDE THE PROGRAM RATHER THAN IN FRONT OF IT.
///
/// The check (`Checker`) only learns that a newer tag exists; this fetches the file for it, then
/// either stands replaced (`ReplaceFile`) or holds a staged archive (`UnpackZip`) until the restart
/// is pressed. One attempt per tag per session: a copy that could not be touched - or a network
/// that gave nothing - falls back to the release page button and is not asked twice.
#[derive(Default)]
pub struct Installer {
    waiting: Option<std::sync::mpsc::Receiver<StageReport>>,
    state: InstallState,
    attempted: Option<String>,
}

impl Installer {
    pub const fn new() -> Self {
        Installer { waiting: None, state: InstallState::Idle, attempted: None }
    }

    /// What to show. Never blocks, never touches the network.
    pub fn state(&self) -> &InstallState {
        &self.state
    }

    /// GO AND FETCH, on a thread of its own. Fetching again while a fetch is on its way - or for a
    /// tag already attempted this session - does nothing. Deciding there is no job (no file for
    /// this copy, no standing copy to put it into) counts as an attempt too: the environment does
    /// not change mid-session, and recomputing the refusal every frame would be work for nothing.
    pub fn start(&mut self, latest: &Latest, env: &StageEnv) {
        if self.waiting.is_some() || self.attempted.as_deref() == Some(latest.latest.as_str()) {
            return;
        }
        self.attempted = Some(latest.latest.clone());
        let Some(jobs) = stage_jobs(&latest.latest, env) else { return };
        let tag = latest.latest.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            // the answer is sent whatever it is; a receiver that has gone away is not our business
            let _ = tx.send(fetch_and_stage(&jobs, &tag));
        });
        self.waiting = Some(rx);
        self.state = InstallState::Downloading;
    }

    /// HAS ANYTHING ARRIVED. Returns whether the state changed, so a frame knows to repaint.
    ///
    /// `try_recv` and not `recv`: this is called from the drawing thread, and the answer usually
    /// is not there yet. A report for a tag nobody waits for anymore is still honoured: tags only
    /// move forward, and an answer about the newest known one is news whatever started it.
    pub fn poll(&mut self) -> bool {
        let Some(rx) = &self.waiting else { return false };
        match rx.try_recv() {
            Ok(report) => {
                self.waiting = None;
                self.state = match report.done {
                    FetchDone::Applied => InstallState::Ready(ReadyInfo { tag: report.tag, staged_zip: None }),
                    FetchDone::ZipStaged(zip) => InstallState::Ready(ReadyInfo { tag: report.tag, staged_zip: Some(zip) }),
                    FetchDone::Skipped => InstallState::Idle,
                };
                true
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => false,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                // the thread died without answering - the same news as a fetch that gave nothing
                self.waiting = None;
                self.state = InstallState::Idle;
                true
            }
        }
    }

    /// Feed an answer in without a network, so the whole path can be asked about in a test.
    #[cfg(test)]
    fn answered(&mut self, report: StageReport) {
        let (tx, rx) = std::sync::mpsc::channel();
        tx.send(report).expect("the channel takes it");
        self.waiting = Some(rx);
        self.poll();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A FIELD LEFT EMPTY IN THE ADMIN FORM MUST NOT REACH THE WINDOW AS A BLANK LINE.
    ///
    /// The record is filled in by hand, so most of the fields are empty most of the time, and a database
    /// stores an untouched field as `NULL` while a form may send `""` and an API may omit the key
    /// altogether. All three mean the same thing and the program has to agree.
    #[test]
    fn empty_and_null_and_absent_are_one_and_the_same() {
        let full = parse(r#"{"latest":"v0.2.0-dev.20260920","url":"https://example.invalid/r","published":"2026-09-20","notes":"a line","notice":"read this","broken_below":"v0.1.0-dev.20260101"}"#)
            .expect("a complete answer reads");
        assert_eq!(full.latest, "v0.2.0-dev.20260920");
        assert_eq!(full.notice.as_deref(), Some("read this"));

        let bare = parse(r#"{"latest":"v0.2.0-dev.20260920","url":"https://example.invalid/r"}"#).expect("the two required fields are enough");
        let nulls = parse(r#"{"latest":"v0.2.0-dev.20260920","url":"https://example.invalid/r","notes":null,"notice":null,"broken_below":null}"#).expect("nulls read");
        let empty = parse(r#"{"latest":"v0.2.0-dev.20260920","url":"https://example.invalid/r","notes":"","notice":"   ","broken_below":""}"#).expect("empty strings read");
        assert_eq!(bare, nulls, "an absent field and a null one came out different");
        assert_eq!(bare, empty, "an absent field and an empty one came out different");
        assert_eq!(bare.notes, None);
    }

    /// AN ANSWER THAT IS NOT AN ANSWER IS SILENCE, NOT AN ERROR SHOWN TO SOMEBODY.
    ///
    /// The site is an ordinary web application: on a bad day it replies with a page of HTML, a proxy
    /// replies with its own, and a captive network replies with a login form. None of that is the
    /// person's business.
    #[test]
    fn nonsense_is_refused_without_a_word() {
        assert_eq!(parse("<!doctype html><title>500</title>"), None, "an HTML error page was taken for an answer");
        assert_eq!(parse(""), None);
        assert_eq!(parse("[1,2,3]"), None, "a JSON array is not our record");
        assert_eq!(parse(r#"{"url":"https://example.invalid/r"}"#), None, "an answer with no tag was accepted");
        assert_eq!(parse(r#"{"latest":"v0.2.0-dev.20260920"}"#), None, "an answer with no page to open was accepted");
        assert_eq!(parse(r#"{"latest":"","url":"https://example.invalid/r"}"#), None, "an empty tag was accepted");
    }

    /// COMPARING VERSIONS IS COMPARING THE DATE, AND NOTHING ELSE.
    #[test]
    fn the_date_in_the_tag_decides() {
        assert!(is_newer("v0.1.0-dev.20260909", "v0.1.0-dev.20260828"));
        assert!(!is_newer("v0.1.0-dev.20260828", "v0.1.0-dev.20260909"));
        assert!(!is_newer("v0.1.0-dev.20260828", "v0.1.0-dev.20260828"), "the same build was called an update");

        // the channel may be renamed and the date still rules
        assert!(is_newer("v0.2.0-beta.20261001", "v0.1.0-dev.20260909"));
        assert!(is_newer("v0.2.0-alpha.20260910", "v0.1.0-dev.20260909"));

        // WHAT IS NOT UNDERSTOOD IS NEVER ANNOUNCED. A build from a checkout has no date at all.
        assert!(!is_newer("v0.2.0", "v0.1.0-dev.20260909"), "a tag with no date was called an update");
        assert!(!is_newer("v0.1.0-dev.20260909", "0.1.0"), "a build with no date of its own accepted an update");
        assert!(!is_newer("nonsense", "v0.1.0-dev.20260909"));

        // a number that is not a date must not be mistaken for one
        assert_eq!(date_of("v0.1.0"), None);
        assert_eq!(date_of("v0.1.0-dev.2026092"), None, "seven digits were read as a date");
        assert_eq!(date_of("v0.1.0-dev.20260920"), Some(20260920));
    }

    /// AND A BAD RELEASE CAN BE CALLED BAD, but only when it is understood.
    #[test]
    fn a_release_can_be_declared_unfit() {
        assert!(is_unfit("v0.1.0-dev.20260828", Some("v0.1.0-dev.20260901")));
        assert!(!is_unfit("v0.1.0-dev.20260902", Some("v0.1.0-dev.20260901")));
        assert!(!is_unfit("v0.1.0-dev.20260828", None), "an empty field marked a build unfit");
        assert!(!is_unfit("v0.1.0-dev.20260828", Some("")), "an empty field marked a build unfit");
        assert!(!is_unfit("0.1.0", Some("v0.1.0-dev.20260901")), "a build with no date was marked unfit");
    }

    /// THE SIGNATURE HAS A FIXED SHAPE, because a log is parsed by a pattern.
    #[test]
    fn the_signature_reads_the_same_every_time() {
        for kind in Install::ALL {
            let ua = user_agent("v0.2.0-dev.20260920", kind);
            assert!(ua.starts_with("QymCAD/v0.2.0-dev.20260920 ("), "the signature does not begin with the program and its version: {ua}");
            assert!(ua.ends_with(&format!("; {})", kind.word())), "the signature does not end with how it was installed: {ua}");
            assert!(ua.is_ascii(), "the signature is not ASCII, and a log is read by a pattern: {ua}");
            assert!(!ua.contains("  "), "the signature carries an empty field: {ua}");
        }
        // Flatpak never signs anything, because it never asks - see `may_ask`.
        assert!(!Install::Flatpak.may_ask());
        assert!(Install::ALL.iter().filter(|k| k.may_ask()).count() == 5, "somebody changed who may ask");
    }

    /// THE LIVE ADDRESS ANSWERS WHAT THE CONTRACT SAYS IT ANSWERS.
    ///
    /// IGNORED ON PURPOSE, and this is the only test here that touches the network. Everything else is
    /// asked of a recorded answer, because a test that goes online is red on a train and in a plane and
    /// gets silenced first. This one is run by hand when the site changes:
    ///
    /// ```text
    /// cargo test -p qymcad-update -- --ignored --nocapture
    /// ```
    ///
    /// What it is for: the record is filled in by a person through a form, and the field names are a
    /// contract between that form and this parser. A field renamed there is not a compilation error
    /// here - it is a program that stops noticing releases, and notices nothing about noticing nothing.
    #[test]
    #[ignore = "goes to cad.qymis.tech; run by hand"]
    fn the_site_answers_what_the_contract_promises() {
        let got = fetch("v0.0.0-dev.20200101", Install::Portable).expect("the site did not answer with our record");
        println!("latest={} url={} published={:?}", got.latest, got.url, got.published);
        assert!(date_of(&got.latest).is_some(), "the tag the site returned carries no date: {}", got.latest);
        assert!(got.url.starts_with("https://"), "the page to open is not an https address: {}", got.url);
        assert!(is_newer(&got.latest, "v0.0.0-dev.20200101"), "a release from the site was not seen as newer than an ancient build");
    }

    /// "NO UPDATES" AND "COULD NOT REACH THE SITE" ARE TWO DIFFERENT PIECES OF NEWS.
    ///
    /// A person who pressed the menu item is owed the difference. Reporting a failed request as "you are
    /// up to date" is the program lying about itself, and the lie is invisible: someone on a version six
    /// months old would be told they are current, for months.
    #[test]
    fn a_check_that_got_nowhere_does_not_pass_for_good_news() {
        let ours = "v0.1.0-dev.20260828";
        let newer = Latest { latest: "v0.2.0-dev.20260920".into(), url: "https://example.invalid/r".into(), ..Default::default() };

        let mut c = Checker::new();
        assert_eq!(*c.outcome(), Outcome::Idle, "a checker that has asked nothing claims to know something");

        c.answered(Some(newer.clone()), ours);
        assert_eq!(*c.outcome(), Outcome::Found(newer.clone()));

        let mut c = Checker::new();
        c.answered(Some(Latest { latest: ours.into(), url: "https://example.invalid/r".into(), ..Default::default() }), ours);
        assert_eq!(*c.outcome(), Outcome::UpToDate, "the same version as ours was taken for an update");

        let mut c = Checker::new();
        c.answered(None, ours);
        assert_eq!(*c.outcome(), Outcome::Unreachable, "a request that got nowhere was reported as good news");

        // an OLDER release on the site is not an update either - and not a failure
        let mut c = Checker::new();
        c.answered(Some(Latest { latest: "v0.1.0-dev.20260101".into(), url: "https://example.invalid/r".into(), ..Default::default() }), ours);
        assert_eq!(*c.outcome(), Outcome::UpToDate);
    }

    /// ASKING TWICE OVER DOES NOT START TWO THREADS.
    ///
    /// The menu item can be pressed as fast as a hand moves, and every press would otherwise be a thread
    /// and a request. The site's own limit is 60 an hour.
    #[test]
    fn leaning_on_the_menu_item_asks_once() {
        let mut c = Checker::new();
        c.waiting = Some(std::sync::mpsc::channel().1); // as if a request were on its way
        c.outcome = Outcome::Asking;
        c.start("v0.1.0-dev.20260828".into(), Install::Portable);
        assert_eq!(*c.outcome(), Outcome::Asking, "a second request was started while the first was still out");
    }

    /// A FRAME ASKS AND CARRIES ON. It must never wait for the answer.
    #[test]
    fn a_frame_that_asks_is_not_held_up() {
        let mut c = Checker::new();
        let (tx, rx) = std::sync::mpsc::channel();
        c.waiting = Some(rx);
        assert!(!c.poll("v0.1.0-dev.20260828"), "an empty channel reported news");
        assert_eq!(*c.outcome(), Outcome::Idle);

        // the thread went away without answering: the same news as not reaching the site
        drop(tx);
        assert!(c.poll("v0.1.0-dev.20260828"));
        assert_eq!(*c.outcome(), Outcome::Unreachable);
    }

    /// HOW IT WAS INSTALLED IS READ FROM THE ENVIRONMENT, not guessed.
    #[test]
    fn the_way_it_was_installed_is_told_from_what_is_there() {
        use std::path::Path;
        // the sandbox outranks everything, even a stray APPIMAGE left in the environment
        assert_eq!(install_from(Some("/x/QymCAD.AppImage"), true, Some(Path::new("/app/bin/qymcad"))), Install::Flatpak);
        assert_eq!(install_from(None, true, None), Install::Flatpak);

        // the paths are deliberately NOT under a home directory: what the answer turns on is only that
        // they lie outside /usr and /opt, and a made-up `/home/<name>` in a published tree reads like
        // somebody's real one - the publishing guard says so, and it is right to.
        assert_eq!(install_from(Some("/media/apps/QymCAD.AppImage"), false, Some(Path::new("/tmp/.mount_x/usr/bin/qymcad"))), Install::AppImage);
        assert_eq!(install_from(Some(""), false, Some(Path::new("/media/apps/qymcad"))), Install::Portable, "an empty APPIMAGE was taken for an AppImage");

        if cfg!(unix) && !cfg!(target_os = "macos") {
            assert_eq!(install_from(None, false, Some(Path::new("/usr/bin/qymcad"))), Install::System);
            assert_eq!(install_from(None, false, Some(Path::new("/opt/qymcad/qymcad"))), Install::System);
            assert_eq!(install_from(None, false, Some(Path::new("/media/apps/Downloads/qymcad"))), Install::Portable);
        }
        // nothing known at all still asks, and points at the release page: the safe end of being wrong
        assert_eq!(install_from(None, false, None), Install::Portable);
    }

    /// A scratch folder for the file tests, wiped first so a previous run cannot answer for this one.
    /// The name travels in so tests running side by side never share a folder.
    #[cfg(test)]
    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("qymcad-update-test-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("the scratch folder appears");
        dir
    }

    /// EVERY RELEASE FILE IS NAMED BY ITS TAG, and the name says the platform.
    ///
    /// The release run names packages this way (`bundle.ps1`, `build-appimage.sh`, `bundle.sh` and
    /// `build-msi.ps1` all drop the leading `v` and add their own tail), so the names below are a
    /// contract with those scripts, not an invention. A script renamed is a red test here first.
    #[test]
    fn every_release_file_is_named_by_its_tag() {
        let named = |os: &str, arch: &str, install: Install| -> Option<String> { asset_name("v0.1.0-dev.20261001", &Target { os: os.to_string(), arch: arch.to_string(), install }) };
        assert_eq!(named("linux", "x86_64", Install::AppImage).as_deref(), Some("qymcad-0.1.0-dev.20261001-x86_64.AppImage"));
        assert_eq!(named("linux", "x86_64", Install::Portable).as_deref(), Some("qymcad-0.1.0-dev.20261001-x86_64.AppImage"));
        assert_eq!(named("linux", "x86_64", Install::System).as_deref(), Some("qymcad-0.1.0-dev.20261001-x86_64.AppImage"));
        assert_eq!(named("windows", "x86_64", Install::Portable).as_deref(), Some("qymcad-0.1.0-dev.20261001-win64.zip"));
        assert_eq!(named("windows", "x86_64", Install::Msi).as_deref(), Some("qymcad-0.1.0-dev.20261001-x64.msi"));
        assert_eq!(named("macos", "aarch64", Install::App).as_deref(), Some("qymcad-0.1.0-dev.20261001-macos-arm64.zip"));
        assert_eq!(named("macos", "x86_64", Install::App).as_deref(), Some("qymcad-0.1.0-dev.20261001-macos-x86_64.zip"));

        // the leading `v` is the tag's, not the file's; a tag without one names the same file
        let plain = asset_name("0.1.0-dev.20261001", &Target { os: "linux".into(), arch: "x86_64".into(), install: Install::AppImage });
        assert_eq!(plain.as_deref(), Some("qymcad-0.1.0-dev.20261001-x86_64.AppImage"));

        // no file is built there: Linux on ARM, a system copy on Windows
        assert_eq!(named("linux", "aarch64", Install::AppImage), None, "no Linux ARM package is built");
        assert_eq!(named("windows", "x86_64", Install::System), None);

        // the address is the name under the page of its tag
        let target = Target { os: "linux".into(), arch: "x86_64".into(), install: Install::AppImage };
        assert_eq!(
            download_url("v0.1.0-dev.20261001", &target).as_deref(),
            Some("https://github.com/QymIs-Tech/QymCAD/releases/download/v0.1.0-dev.20261001/qymcad-0.1.0-dev.20261001-x86_64.AppImage")
        );
        assert_eq!(download_url("v0.1.0-dev.20261001", &Target { os: "linux".into(), arch: "aarch64".into(), install: Install::AppImage }), None);
    }

    /// A TAG THAT CANNOT BE A FILE NAME NAMES NO FILE.
    ///
    /// The tag is filled in by hand on the site, and a hand slips. Whatever it carries ends up in a
    /// file name and an address, so a slash, a backslash or whitespace anywhere in it is a refusal,
    /// not a download from somewhere unintended.
    #[test]
    fn a_tag_that_cannot_be_a_file_name_names_no_file() {
        let target = Target { os: "linux".into(), arch: "x86_64".into(), install: Install::AppImage };
        for bad in ["", "v", "v0.1.0-dev.20261001/../../x", "v0.1.0\\x", "v0.1.0 dev", "a/b"] {
            assert_eq!(asset_name(bad, &target), None, "the tag {bad:?} named a file");
            assert_eq!(download_url(bad, &target), None, "the tag {bad:?} named an address");
        }
    }

    /// ONLY A COPY NOBODY OWNS IS REPLACED, and the predicate and the plan agree everywhere.
    ///
    /// An installer copy needs elevation and is locked while it runs, a system one belongs to a
    /// package manager, a macOS bundle carries a quarantine flag and a signature, and the sandbox
    /// has no network at all. Touching any of them in the background would trade a working program
    /// for a broken one - or for a surprise elevation prompt nobody pressed anything to get.
    #[test]
    fn only_a_copy_nobody_owns_is_replaced() {
        let plan = |os: &str, arch: &str, install: Install| install_plan(&Target { os: os.to_string(), arch: arch.to_string(), install });
        assert_eq!(plan("linux", "x86_64", Install::AppImage), InstallPlan::ReplaceFile);
        assert_eq!(plan("linux", "x86_64", Install::Portable), InstallPlan::ReplaceFile);
        assert_eq!(plan("windows", "x86_64", Install::Portable), InstallPlan::UnpackZip);
        assert_eq!(plan("macos", "aarch64", Install::App), InstallPlan::Impossible, "a quarantined bundle is not replaced");
        assert_eq!(plan("macos", "aarch64", Install::Portable), InstallPlan::Impossible, "the macOS asset holds a bundle, not a file");
        assert_eq!(plan("windows", "x86_64", Install::Msi), InstallPlan::Impossible, "an installer copy needs elevation");
        assert_eq!(plan("linux", "x86_64", Install::System), InstallPlan::Impossible, "a system copy belongs to its manager");
        assert_eq!(plan("linux", "x86_64", Install::Flatpak), InstallPlan::Impossible, "the sandbox has no network");

        // the predicate says what the plan says, on every combination, so the two cannot drift apart
        for os in ["linux", "windows", "macos"] {
            for arch in ["x86_64", "aarch64"] {
                for install in Install::ALL {
                    let target = Target { os: os.to_string(), arch: arch.to_string(), install };
                    assert_eq!(target.can_self_install(), !matches!(install_plan(&target), InstallPlan::Impossible), "predicate and plan disagree on {os}-{arch} {install:?}");
                }
            }
        }
    }

    /// THE RESTART LAUNCHES THE BUNDLE, NOT ITS MOUNT.
    ///
    /// A running AppImage's executable lies inside the mount (`/tmp/.mount_xxx/usr/bin/qymcad`):
    /// replacing or relaunching THAT changes nothing once the mount is gone. The file that matters
    /// is the one the runtime names in `APPIMAGE`.
    #[test]
    fn the_restart_launches_the_bundle_not_its_mount() {
        use std::path::Path;
        let mounted = Path::new("/tmp/.mount_x/usr/bin/qymcad");
        assert_eq!(program_target(Some("/media/apps/QymCAD.AppImage"), Some(mounted)), Some(std::path::PathBuf::from("/media/apps/QymCAD.AppImage")));
        assert_eq!(program_target(Some(""), Some(mounted)), Some(mounted.to_path_buf()), "an empty APPIMAGE was taken for an AppImage");
        assert_eq!(program_target(None, Some(mounted)), Some(mounted.to_path_buf()));
        assert_eq!(program_target(None, None), None);

        // the launch carries the same arguments this copy was given
        let cmd = restart_cmd(Path::new("/media/apps/qymcad"));
        assert_eq!(cmd.program, std::path::PathBuf::from("/media/apps/qymcad"));
        assert_eq!(cmd.args, std::env::args().skip(1).collect::<Vec<_>>());
    }

    /// A REPLACED FILE STANDS WHOLE, AND THE OLD ONE WAITS BESIDE IT.
    ///
    /// The new file lands beside the target first and is renamed over it, so a failure halfway
    /// leaves the old copy standing rather than a half-written program. The old one waits under
    /// the backup suffix until the next start sweeps it away.
    #[test]
    fn a_replaced_file_stands_whole_and_the_old_one_waits_beside_it() {
        let dir = scratch("replace");
        let target = dir.join("qymcad.AppImage");
        std::fs::write(&target, "old copy").expect("the old copy stands");
        let staged = dir.join("downloaded.AppImage");
        std::fs::write(&staged, "new copy").expect("the download lands");

        replace_file(&staged, &target).expect("the replacement goes through");
        assert_eq!(std::fs::read_to_string(&target).expect("the target reads"), "new copy");
        assert_eq!(std::fs::read_to_string(dir.join("qymcad.AppImage.qymcad-old")).expect("the backup reads"), "old copy");

        // the next start sweeps exactly the backups away, and nothing else
        assert_eq!(cleanup_backups(&dir), 1);
        assert!(target.exists(), "the sweep took the program with it");
        assert!(staged.exists(), "the sweep took a file it had no business taking");
        assert_eq!(cleanup_backups(&dir), 0, "a second sweep found something");
        assert_eq!(cleanup_backups(&dir.join("absent")), 0, "a missing folder is not an error");

        // replacing where nothing stood needs no backup and makes no noise
        let fresh = dir.join("first.AppImage");
        replace_file(&staged, &fresh).expect("the first copy lands");
        assert_eq!(std::fs::read_to_string(&fresh).expect("the copy reads"), "new copy");
        assert_eq!(cleanup_backups(&dir), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Pack a test archive: the names and bytes the test hands in, stored plainly.
    #[cfg(test)]
    fn pack(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut buf = std::io::Cursor::new(Vec::new());
        let mut zip = zip::ZipWriter::new(&mut buf);
        for (name, data) in files {
            let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
            zip.start_file(*name, opts).expect("the test archive takes it");
            use std::io::Write;
            zip.write_all(data).expect("the test archive takes it");
        }
        zip.finish().expect("the test archive closes");
        buf.into_inner()
    }

    /// AN UNPACKED ARCHIVE REPLACES THE FOLDER WHOLE, AND A FAILED ONE LEAVES IT ENTIRE.
    ///
    /// The release run packs the portable copy as a zip of its own folder, so an update of such a
    /// copy is an unpacking over that folder. Files that already exist move aside first - the files
    /// of a running program are locked against overwriting on Windows, but not against renaming.
    #[test]
    fn an_unpacked_archive_replaces_the_folder_whole() {
        let dir = scratch("unpack");
        std::fs::write(dir.join("qymcad.exe"), "old program").expect("the old program stands");
        std::fs::write(dir.join("old.dll"), "old library").expect("the old library stands");
        std::fs::write(dir.join("keep.txt"), "untouched").expect("the bystander stands");
        let zip_path = dir.join("release.zip");
        std::fs::write(&zip_path, pack(&[("qymcad.exe", b"new program"), ("new.dll", b"new library")])).expect("the archive lands");

        apply_staged_zip(&zip_path, &dir.join("qymcad.exe")).expect("the unpacking goes through");
        assert_eq!(std::fs::read_to_string(dir.join("qymcad.exe")).expect("the program reads"), "new program");
        assert_eq!(std::fs::read_to_string(dir.join("new.dll")).expect("the library reads"), "new library");
        // only what the archive brought moved aside: the bystander and the unmentioned library stand as they were
        assert_eq!(std::fs::read_to_string(dir.join("qymcad.exe.qymcad-old")).expect("the backup reads"), "old program");
        assert_eq!(std::fs::read_to_string(dir.join("old.dll")).expect("the unmentioned library reads"), "old library");
        assert_eq!(std::fs::read_to_string(dir.join("keep.txt")).expect("the bystander reads"), "untouched");
        assert_eq!(cleanup_backups(&dir), 1, "the one moved file is swept at the next start");

        // a broken archive leaves the folder entire: the old files are back, the new ones are gone
        std::fs::write(&zip_path, b"this is no archive").expect("the broken archive lands");
        assert!(apply_staged_zip(&zip_path, &dir.join("qymcad.exe")).is_err(), "a broken archive was applied");
        assert_eq!(std::fs::read_to_string(dir.join("qymcad.exe")).expect("the program reads"), "new program");
        assert_eq!(std::fs::read_to_string(dir.join("keep.txt")).expect("the bystander reads"), "untouched");

        // an entry escaping the folder is refused outright
        std::fs::write(&zip_path, pack(&[("../outside.txt", b"out")])).expect("the hostile archive lands");
        assert!(apply_staged_zip(&zip_path, &dir.join("qymcad.exe")).is_err(), "an escaping entry was unpacked");
        assert!(!dir.join("outside.txt").exists() && !dir.parent().expect("the scratch has a parent").join("outside.txt").exists(), "the escape landed somewhere");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// THERE IS NO JOB WHERE THERE IS NOTHING TO PUT IT INTO.
    ///
    /// No file for the combination, no standing copy found, a copy owned by somebody else: all one
    /// case, and the answer is nothing to fetch. The release page button stays the whole story, and
    /// the frame does not recompute the refusal every time it draws.
    #[test]
    fn there_is_no_job_where_there_is_nothing_to_put_it_into() {
        let tag = "v0.1.0-dev.20261001";
        let workdir = scratch("no-job").join("waiting");
        let env = |os: &str, arch: &str, install: Install, program: Option<&str>| StageEnv {
            target: Target { os: os.to_string(), arch: arch.to_string(), install },
            program: program.map(std::path::PathBuf::from),
            workdir: workdir.clone(),
        };
        // a managed copy: the page button stays the whole story
        assert!(stage_jobs(tag, &env("linux", "x86_64", Install::System, Some("/usr/bin/qymcad"))).is_none());
        // no file is built there
        assert!(stage_jobs(tag, &env("linux", "aarch64", Install::AppImage, Some("/media/a.AppImage"))).is_none());
        // no standing copy found
        assert!(stage_jobs(tag, &env("linux", "x86_64", Install::AppImage, None)).is_none());

        // and where there is one, the job names the file, its address and where it waits
        let jobs = stage_jobs(tag, &env("linux", "x86_64", Install::AppImage, Some("/media/apps/qymcad.AppImage"))).expect("an AppImage has a job");
        assert_eq!(jobs.apply.plan, InstallPlan::ReplaceFile);
        assert_eq!(jobs.apply.program, std::path::PathBuf::from("/media/apps/qymcad.AppImage"));
        assert_eq!(jobs.fetch.url.as_str(), "https://github.com/QymIs-Tech/QymCAD/releases/download/v0.1.0-dev.20261001/qymcad-0.1.0-dev.20261001-x86_64.AppImage");
        assert_eq!(jobs.fetch.dest, workdir.join("pending-qymcad-0.1.0-dev.20261001-x86_64.AppImage"));
        let _ = std::fs::remove_dir_all(workdir.clone());
    }

    /// A FETCH THAT CANNOT START IS AN ERROR, NOT A HANG.
    ///
    /// Asked of an address that is not one, without touching the network: whatever the network
    /// would refuse must already be refused here.
    #[test]
    fn a_fetch_that_cannot_start_is_an_error_not_a_hang() {
        let dir = scratch("bad-fetch");
        assert!(download_to("not an address at all", &dir.join("nothing.bin")).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// THE BACKGROUND UPDATE ASKS ONCE PER TAG, AND REPORTS WHAT CAME BACK.
    ///
    /// A second start for the same tag - or while a fetch is on its way - starts no second thread.
    /// Whatever the thread came back with becomes the state: a replaced copy waits for a restart, a
    /// staged archive waits for the restart button, and a fetch that gave nothing is silence.
    #[test]
    fn the_background_update_asks_once_per_tag_and_reports_what_came_back() {
        let tag = "v9.9.9-dev.20991231";
        let report = |done| StageReport { tag: tag.to_string(), done };

        let mut installed = Installer::new();
        installed.answered(report(FetchDone::Applied));
        assert_eq!(*installed.state(), InstallState::Ready(ReadyInfo { tag: tag.to_string(), staged_zip: None }));

        let mut staged = Installer::new();
        staged.answered(report(FetchDone::ZipStaged(std::path::PathBuf::from("/tmp/pending.zip"))));
        assert!(matches!(*staged.state(), InstallState::Ready(ref info) if info.tag == tag && info.staged_zip.is_some()), "the staged archive did not wait for the restart");

        let mut skipped = Installer::new();
        skipped.answered(report(FetchDone::Skipped));
        assert_eq!(*skipped.state(), InstallState::Idle, "a fetch that gave nothing left something on screen");

        // the thread went away without answering: the same news as a fetch that gave nothing
        let mut gone = Installer::new();
        let (tx, rx) = std::sync::mpsc::channel();
        gone.waiting = Some(rx);
        drop(tx);
        assert!(gone.poll(), "a dead thread reported nothing");
        assert_eq!(*gone.state(), InstallState::Idle);

        // a tag with no job counts as attempted: the refusal is not recomputed every frame
        let mut refused = Installer::new();
        let env = StageEnv {
            target: Target { os: "linux".into(), arch: "x86_64".into(), install: Install::System },
            program: Some(std::path::PathBuf::from("/usr/bin/qymcad")),
            workdir: std::env::temp_dir(),
        };
        let latest = Latest { latest: tag.to_string(), url: "https://example.invalid/r".into(), ..Default::default() };
        refused.start(&latest, &env);
        assert_eq!(*refused.state(), InstallState::Idle);
        assert!(refused.waiting.is_none(), "a refusal started a thread");
        refused.start(&latest, &env);
        assert!(refused.waiting.is_none(), "a second start for the same tag started a thread");
    }
}
