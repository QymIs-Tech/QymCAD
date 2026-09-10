//! IS THERE A NEWER VERSION, AND WHAT SHOULD BE SAID ABOUT IT.
//!
//! A program that never says a new version exists leaves people on the one they installed. Measured on
//! this project in September 2026: 470 downloads, 300 clones of the repository and 10-20 people in the
//! chat - so the overwhelming majority of those running it are reachable by nothing at all, and every
//! fix made since their download is invisible to them.
//!
//! WHAT THIS MODULE IS NOT. It does not download, unpack or replace anything. Behind self-replacement
//! come resuming a broken transfer, checksums, write permissions, the macOS quarantine flag and an
//! argument with an antivirus on Windows; a browser does all of that better and every person already
//! has one. This asks one question, and a button opens the release page.
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
        install_from(
            std::env::var("APPIMAGE").ok().as_deref(),
            std::path::Path::new("/.flatpak-info").exists(),
            std::env::current_exe().ok().as_deref(),
        )
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
        let full = parse(
            r#"{"latest":"v0.2.0-dev.20260920","url":"https://example.invalid/r","published":"2026-09-20","notes":"a line","notice":"read this","broken_below":"v0.1.0-dev.20260101"}"#,
        )
        .expect("a complete answer reads");
        assert_eq!(full.latest, "v0.2.0-dev.20260920");
        assert_eq!(full.notice.as_deref(), Some("read this"));

        let bare = parse(r#"{"latest":"v0.2.0-dev.20260920","url":"https://example.invalid/r"}"#).expect("the two required fields are enough");
        let nulls = parse(r#"{"latest":"v0.2.0-dev.20260920","url":"https://example.invalid/r","notes":null,"notice":null,"broken_below":null}"#)
            .expect("nulls read");
        let empty = parse(r#"{"latest":"v0.2.0-dev.20260920","url":"https://example.invalid/r","notes":"","notice":"   ","broken_below":""}"#)
            .expect("empty strings read");
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
}
