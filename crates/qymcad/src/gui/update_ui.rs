//! WHETHER A NEWER VERSION EXISTS, AS THE INTERFACE SEES IT.
//!
//! THE STATE LIVES HERE AND NOT ON `App`. There is one program, one window and one check, so the answer
//! is genuinely process-wide; putting it on the application object would add a twenty-first field to a
//! record that two weeks of work went into shrinking. A menu item, the status line and the window all
//! read the same answer through these functions, and none of them needs the application at all.
//!
//! Nothing here waits for the network: `ask` starts a thread, `outcome` reads whatever has arrived.
use qymcad_update::{Checker, Install, Outcome};

/// The one check of the session.
static CHECKER: std::sync::Mutex<Checker> = std::sync::Mutex::new(Checker::new());

// A RELEASE TAG PRETENDED FOR THE DURATION OF A CHECK, and nothing else.
//
// Nothing built on this machine is a release, and by the rule above a build that is not a release shows
// none of this - so the menu item, the badge and the window cannot be reached by hand here at all. That
// left the whole of the interface asked about only through pure functions, which is exactly the kind of
// green that says nothing about what a person can do.
// PER THREAD, NOT PER PROCESS, and that is not a detail. Tests run beside each other, each on its own
// thread; a single pretence shared between them is switched off by whichever finishes first, and the
// others then quietly ask the real build - which is not a release, so they see nothing and pass or fail
// for reasons that have nothing to do with what they are asking. Measured: two of the three failed that
// way, showing "checking..." where the answer was supposed to be.
//
// Compiled only under `cfg(test)`: the program itself has no way to pretend anything.
#[cfg(test)]
thread_local! {
    static PRETEND: std::cell::RefCell<Option<(String, Outcome)>> = const { std::cell::RefCell::new(None) };
}

/// What this build calls itself, as far as the check is concerned.
fn release() -> Option<String> {
    #[cfg(test)]
    if let Some(tag) = PRETEND.with_borrow(|p| p.as_ref().map(|(tag, _)| tag.clone())) {
        return Some(tag);
    }
    crate::build_info::release().map(str::to_string)
}

/// WHAT TO SHOW AS "YOURS". The same string the comparison used, so the window cannot say one version
/// while the answer was worked out from another.
pub(crate) fn ours() -> String {
    release().unwrap_or_else(|| crate::build_info::head().to_string())
}

/// Stand in for a release and an answer, so a frame can be drawn and clicked.
#[cfg(test)]
pub(crate) fn pretend(tag: &str, outcome: Outcome) {
    PRETEND.with_borrow_mut(|p| *p = Some((tag.to_string(), outcome)));
}

/// Stop pretending. Called at the end of a test so the next one starts clean.
#[cfg(test)]
pub(crate) fn stop_pretending() {
    PRETEND.with_borrow_mut(|p| *p = None);
}

/// Seconds since the epoch. A clock before 1970 gives zero, which reads as "never asked".
fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// CAN THIS COPY ASK AT ALL, and therefore: should any of this be on screen.
///
/// Two answers say no, and both mean the same thing - do not show what cannot work:
///
/// * INSIDE FLATPAK there is no network. The store updates the package and tells the person itself.
/// * A BUILD WITH NO RELEASE TAG has nothing to compare. Somebody's own compilation carries the manifest
///   number, which is the same for every build of the week; asking would be a request whose answer could
///   not be used. Whoever built it themselves follows releases another way.
pub(crate) fn available() -> bool {
    release().is_some() && Install::from_env().may_ask()
}

/// ASK NOW, because a person pressed the item. Goes even when the automatic check is switched off:
/// pressing it IS the asking.
pub(crate) fn ask(set: &mut qymcad_ui_state::Settings) {
    let Some(release) = release() else { return };
    let install = Install::from_env();
    if !install.may_ask() {
        return;
    }
    set.update_last_checked = now();
    if let Ok(mut c) = CHECKER.lock() {
        c.start(release, install);
    }
}

/// ASK IF IT IS TIME, at a start. Called from the frame, so it must be cheap and it must not repeat.
pub(crate) fn ask_if_due(set: &mut qymcad_ui_state::Settings) {
    if !available() || !matches!(outcome(), Outcome::Idle) {
        return; // already asked this session, or nothing to ask with
    }
    if qymcad_ui_state::update_check_due(set.update_check, set.update_last_checked, now()) {
        ask(set);
    }
}

/// WHAT THE CHECK CAME TO. Never blocks; a poisoned lock reads as "nobody has asked", which is the
/// quietest of the wrong answers.
pub(crate) fn outcome() -> Outcome {
    #[cfg(test)]
    if let Some(o) = PRETEND.with_borrow(|p| p.as_ref().map(|(_, o)| o.clone())) {
        return o;
    }
    let Ok(mut c) = CHECKER.lock() else { return Outcome::Idle };
    if let Some(release) = release() {
        c.poll(&release);
    }
    c.outcome().clone()
}

#[cfg(test)]
mod tests {
    /// THE MENU ITEM AND THE SETTING ARE ABSENT WHERE THEY CANNOT WORK.
    ///
    /// Not "present and quietly doing nothing". A person who presses "check for updates" and sees the
    /// line say nothing at all learns that the program is broken, which is worse than never having
    /// offered it.
    #[test]
    fn what_cannot_work_is_not_offered() {
        // this build has no release tag, so nothing of it should be on screen
        assert!(crate::build_info::release().is_none(), "a build made here is not a release, and this check assumes it");
        assert!(!super::available(), "an ordinary build offers a check it has nothing to compare against");
    }

    /// AND ASKING ON SUCH A BUILD CHANGES NOTHING, including the time of the last check.
    ///
    /// Writing the time down would be worse than doing nothing: after a person upgraded to a real
    /// release, "checked a moment ago" would hold the first real check back for a whole day.
    #[test]
    fn asking_on_a_build_that_cannot_ask_leaves_no_trace() {
        let mut set = qymcad_ui_state::Settings::default();
        set.update_last_checked = 0;
        super::ask(&mut set);
        assert_eq!(set.update_last_checked, 0, "a build with nothing to compare wrote down that it had checked");
        super::ask_if_due(&mut set);
        assert_eq!(set.update_last_checked, 0);
    }
}
