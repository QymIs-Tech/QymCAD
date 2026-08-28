//! WHEN THE WINDOW NEVER OPENS, SOMETHING ELSE HAS TO SPEAK.
//!
//! Reported behaviour: on Windows 11 with an old card and no graphics driver the program starts, blinks
//! and closes. Nothing in the crash folder, nothing on screen, nothing to send.
//!
//! Two reasons for that silence, and both are answered here. A start-up failure comes back as an ordinary
//! error rather than a panic, so the crash hook never saw it (`crash::note_failed_start` now does). And the
//! message went to standard error - which a windowed build on Windows has nowhere to print to, so it went
//! nowhere at all. The system's own message box is the one thing certain to be seen when ours never opened.
use crate::diagnostics::StartFailure;

/// Say, in a window the system draws, that we could not start - and where the details are.
pub fn tell_the_person(failure: &StartFailure) {
    let title = crate::i18n::tr("start-failed-title");
    let mut body = if failure.no_adapter {
        // THE ONE CAUSE WITH ADVICE A PERSON CAN ACT ON. Anything else at this door is ours to fix from
        // the report; this one they can fix themselves in ten minutes.
        crate::i18n::tr("start-failed-no-adapter")
    } else {
        crate::i18n::tr("start-failed-other")
    };
    if let Some(path) = &failure.report {
        body.push_str("\n\n");
        body.push_str(&crate::i18n::tr1("start-failed-report", "path", &path.display().to_string()));
    }
    if !failure.reason.is_empty() {
        body.push_str("\n\n");
        body.push_str(&failure.reason);
    }
    // The blocking call is right HERE and nowhere else: there is no frame loop left to hold up - the
    // window never opened - and the program has nothing to do but say this and stop.
    rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Error)
        .set_title(&title)
        .set_description(&body)
        .set_buttons(rfd::MessageButtons::Ok)
        .show();
}

#[cfg(test)]
mod tests {
    /// THE WORDS EXIST IN BOTH LANGUAGES AND SAY WHAT TO DO. Nobody sees this window while things work, so
    /// nothing but a check will notice the day one of these lines is a catalogue key.
    #[test]
    fn the_door_speaks_in_both_languages() {
        for lang in ["ru", "en"] {
            crate::i18n::set_language(lang);
            for key in ["start-failed-title", "start-failed-no-adapter", "start-failed-other"] {
                let text = crate::i18n::tr(key);
                assert_ne!(text, key, "[{lang}] {key} is missing from the catalogue");
                assert!(text.len() > 4, "[{lang}] {key} says nothing: {text:?}");
            }
            let with_path = crate::i18n::tr1("start-failed-report", "path", "/tmp/crash_1.txt");
            assert!(with_path.contains("crash_1.txt"), "[{lang}] the report line loses the path: {with_path}");
        }
        crate::i18n::set_language("ru");
    }

    /// THE ADVICE MUST DIFFER. Telling somebody with no graphics driver the same thing as somebody whose
    /// start failed for an unknown reason wastes the one case they could have fixed themselves.
    #[test]
    fn a_missing_driver_is_not_told_the_same_as_anything_else() {
        for lang in ["ru", "en"] {
            crate::i18n::set_language(lang);
            assert_ne!(crate::i18n::tr("start-failed-no-adapter"), crate::i18n::tr("start-failed-other"), "[{lang}] both cases say the same thing");
        }
        crate::i18n::set_language("ru");
    }
}
