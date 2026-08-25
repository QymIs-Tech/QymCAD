//! LOCALISATION: what is checked is what it is built this way for.
//!
//! The main requirement is "adding a language = dropping in a catalogue". It is checkable, and here it is
//! checked: the registry is built from the catalogue, the translation comes from the same place, and an
//! incomplete language does not break the interface.
use super::*;

/// THE LANGUAGES COME FROM THE CATALOGUE rather than from a list in the code.
///
/// That is exactly what makes an outside contribution possible: drop in a folder and the language
/// appears. Were the list to live in the code, every outside translation would require an edit to the
/// code, and none would be sent.
#[test]
fn languages_come_from_the_catalogue_not_from_code() {
    let list = available();
    assert!(list.len() >= 2, "the catalogue must hold at least ru and en, and out came {list:?}");
    let codes: Vec<&str> = list.iter().map(|(c, _)| c.as_str()).collect();
    assert!(codes.contains(&"ru") && codes.contains(&"en"), "the language codes: {codes:?}");

    // THE NAME OF A LANGUAGE IS IN THAT LANGUAGE ITSELF: that is how it is recognised by somebody who
    // does not read the other one. The expected values are deliberate test data — a Latin string here
    // would stop checking the very thing the assertion is about.
    let ru = list.iter().find(|(c, _)| c == "ru").expect("ru");
    let en = list.iter().find(|(c, _)| c == "en").expect("en");
    assert_eq!(ru.1, "Русский", "the Russian language must be named in Russian");
    assert_eq!(en.1, "English", "and English in English");
}

/// SWITCHING THE LANGUAGE CHANGES THE STRING. Not "the function returned a key" but the text itself.
///
/// THE SECOND LANGUAGE IS ASKED FOR A WORD OF ITS OWN rather than compared with a literal: a literal
/// would pin the check to one particular translation and would have to be edited along with it.
#[test]
fn switching_the_language_changes_the_text() {
    set_language("en");
    assert_eq!(tr("menu-file"), "File");
    set_language("ru");
    let other = tr("menu-file");
    assert_ne!(other, "File", "the other language must carry a word of its own");
    assert_ne!(other, "menu-file", "and not the key itself");
    let saved_as = tr("file-save-as");
    assert_ne!(saved_as, "file-save-as", "and every key of the menu is translated, not only the first");
}

/// A MISSING KEY LEAVES NO EMPTINESS.
///
/// An empty button looks like a breakage of the program; a visible key says at once what is missing, and
/// it shows up in a screenshot attached to a report.
#[test]
fn a_missing_key_shows_the_key_not_emptiness() {
    set_language("ru");
    assert_eq!(tr("no-such-key-at-all"), "no-such-key-at-all");
}

/// AN INCOMPLETE TRANSLATION DOES NOT BREAK THE INTERFACE: what a language lacks comes from the
/// reference.
///
/// Without that, the very first translation sent in at 60% coverage would leave the interface full of
/// holes and contributions would become frightening to accept. It is checked on a language assembled
/// right here out of ONE line.
#[test]
fn a_partial_language_falls_back_for_what_it_lacks() {
    let only_one = "language-name = Partial\nmenu-file = Fichier\n";
    let missing = check_partial(only_one, "menu-help");
    assert_eq!(missing, tr_in(FALLBACK, "menu-help").expect("the reference has the key"), "what is missing comes from the reference");
    let present = check_partial(only_one, "menu-file");
    assert_eq!(present, "Fichier", "and what is translated stays its own");
}

/// AN UNKNOWN LANGUAGE CODE does not leave the interface without strings.
///
/// A setting from a future version or from somebody else's build is an ordinary thing; an interface with
/// no text is not.
#[test]
fn an_unknown_language_code_falls_back_instead_of_breaking() {
    set_language("kl-XX");
    assert_eq!(language(), FALLBACK, "an unknown code is brought to the reference");
    assert_eq!(tr("menu-file"), "File", "and the strings are in place through it");
}

/// SUBSTITUTIONS DO NOT ACQUIRE INVISIBLE CHARACTERS.
///
/// fluent wraps them in U+2068/U+2069 (right for bidirectional text, but in an egui button they are extra
/// glyphs, and in a test an unexplainable mismatch of strings).
#[test]
fn no_invisible_isolate_characters_leak_into_the_ui() {
    set_language("ru");
    for key in ["menu-file", "file-save", "help-about"] {
        let s = tr(key);
        assert!(!s.contains('\u{2068}') && !s.contains('\u{2069}'), "the string {key} carries invisible characters: {s:?}");
    }
}

/// QUOTES IN THE CATALOGUE ARE TEXT, NOT MARKUP.
///
/// In Fluent the value `key = "word "` contains the quotes LITERALLY: what is seen on screen is `"word "`.
/// Quotes are meaningful only inside a placeable, and that is how edge spaces are preserved:
/// `key = { "word " }`. The difference is almost invisible to the eye in an editor — and that is exactly
/// why it lived unnoticed until the first screenshot of a window where the tool bar read `"rotate "15""`.
///
/// Exactly that shape is caught: a value wrapped in quotes AS A WHOLE. A quote inside a phrase is
/// lawful.
#[test]
fn a_catalogue_value_is_never_wrapped_in_bare_quotes() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().and_then(|p| p.parent()).expect("the root of the repository").join("i18n");
    let mut bad = Vec::new();
    for lang in std::fs::read_dir(&dir).expect("the catalogue of languages").filter_map(|e| e.ok()) {
        if !lang.path().is_dir() {
            continue; // ordinary files lie beside the languages too
        }
        for file in std::fs::read_dir(lang.path()).expect("the files of the language").filter_map(|e| e.ok()) {
            let text = std::fs::read_to_string(file.path()).unwrap_or_default();
            for (n, line) in text.lines().enumerate() {
                let Some((key, val)) = line.split_once(" = ") else { continue };
                if key.starts_with(['#', ' ']) {
                    continue;
                }
                if val.starts_with('"') && val.ends_with('"') && val.len() > 1 {
                    bad.push(format!("{}:{}: {key}", file.path().display(), n + 1));
                }
            }
        }
    }
    assert!(
        bad.is_empty(),
        "a value of the catalogue is wrapped in quotes and they will reach the screen literally ({}):\n{}\nedge spaces are written as {{ \" ... \" }}",
        bad.len(),
        bad.join("\n")
    );
}

/// THE COVERAGE OF EVERY LANGUAGE — both a report and a check.
///
/// It is printed so that an outside translator can see what is left and we can see what to accept. It
/// fails only when a language holds a key the reference does NOT: that is either a typo in the key or a
/// string forgotten in the reference — and both are mended rather than tolerated.
#[test]
fn every_language_is_measured_against_the_reference() {
    let reference = reference_keys();
    assert!(!reference.is_empty(), "the reference language must exist and hold keys");
    for (code, name) in available() {
        let keys = keys_of(&code);
        let have = keys.iter().filter(|k| reference.contains(k)).count();
        println!("[i18n] {code} ({name}): {have}/{}", reference.len());
        let extra: Vec<&String> = keys.iter().filter(|k| !reference.contains(k)).collect();
        assert!(extra.is_empty(), "language {code} holds keys the reference {FALLBACK} does not: {extra:?}");
    }
}

/// THE REFERENCE IS COMPLETE BY DEFINITION: every key of it has a non-empty value.
#[test]
fn the_reference_language_has_no_empty_strings() {
    for k in reference_keys() {
        let v = tr_in(FALLBACK, &k).unwrap_or_default();
        assert!(!v.trim().is_empty(), "an empty string in the reference: {k}");
    }
}

/// THE SETS OF SUBSTITUTIONS MATCH THE REFERENCE.
///
/// A string where a substitution was lost or renamed breaks AT RUNTIME rather than at build time — so it
/// is checked here, while that is cheap.
#[test]
fn placeholder_sets_match_the_reference() {
    for (code, _) in available() {
        if code == FALLBACK {
            continue;
        }
        for k in keys_of(&code) {
            let (Some(a), Some(b)) = (tr_in(FALLBACK, &k), tr_in(&code, &k)) else { continue };
            assert_eq!(placeholders(&a), placeholders(&b), "in {code}/{k} the set of substitutions diverged from the reference");
        }
    }
}

/// The names of the substitutions in a formatted string. It works off the RESULT of formatting with no
/// arguments: fluent puts `{$name}` there as it stands, and the names are visible.
fn placeholders(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = s;
    while let Some(p) = rest.find("{$") {
        rest = &rest[p + 2..];
        let end = rest.find('}').unwrap_or(rest.len());
        out.push(rest[..end].trim().to_string());
        rest = &rest[end.min(rest.len())..];
    }
    out.sort();
    out.dedup();
    out
}

/// Assemble a language out of text right here in the test and ask it for a key WITH A FALLBACK to the
/// reference — that is how the behaviour of an incomplete translation is checked, without slipping files
/// into the catalogue.
fn check_partial(ftl: &str, key: &str) -> String {
    use fluent_bundle::{FluentBundle, FluentResource};
    let li: unic_langid::LanguageIdentifier = "fr".parse().expect("the language tag");
    let mut b = FluentBundle::new(vec![li]);
    b.set_use_isolating(false);
    b.add_resource(FluentResource::try_new(ftl.to_string()).expect("the parsing")).expect("the adding");
    let own = b.get_message(key).and_then(|m| m.value().map(|p| {
        let mut e = Vec::new();
        b.format_pattern(p, None, &mut e).to_string()
    }));
    own.unwrap_or_else(|| tr_in(FALLBACK, key).unwrap_or_else(|| key.to_string()))
}

/// THE LANGUAGE REACHED THE LIVE INTERFACE rather than staying a mechanism for the future.
///
/// A frame wired to nothing is the appearance of work. What is checked here is that the menu takes its
/// names from the localisation and that the language setting is applied at startup.
#[test]
fn the_menu_bar_actually_uses_the_catalogue() {
    let panels = crate::gui::panels_source::PANELS;
    for key in ["menu-file", "menu-edit", "menu-view", "menu-windows", "menu-help", "file-save", "file-quit", "help-about"] {
        assert!(panels.contains(&format!(r#"tr("{key}")"#)), "the menu must take \"{key}\" from the localisation rather than from a literal");
    }
    // AND THE OTHER WAY ROUND, BY THE MECHANISM RATHER THAN BY FORMER WORDS: no menu is opened with a
    // caption typed straight into the code. A guard keyed on the old literals would stop meaning anything
    // the moment somebody wrote them back in another language.
    let bare: Vec<&str> = panels
        .match_indices("menu_button(\"")
        .map(|(i, _)| panels[i..].lines().next().unwrap_or(""))
        .collect();
    assert!(bare.is_empty(), "the caption of a menu must come from the localisation:\n{}", bare.join("\n"));
}

/// THE LANGUAGE SWITCH IS IN THE SETTINGS and is built from the catalogue rather than from a list in the
/// code.
#[test]
fn the_settings_window_offers_the_catalogue() {
    let panels = crate::gui::panels_source::PANELS;
    assert!(panels.contains("crate::i18n::available()"), "the list of languages must be built from the catalogue");
    assert!(panels.contains("self.set.language = code"), "the choice must be saved into the settings");
}

/// THE LANGUAGE IS APPLIED AT STARTUP and survives a restart.
#[test]
fn the_language_setting_survives_a_restart_and_is_applied() {
    let gui = include_str!("../gui.rs");
    // THROUGH THE SHARED HANDLE: the settings take effect by one path (`adopt_settings`), and the startup
    // goes by it too. Checking for a literal call at startup here would mean forbidding that handle to
    // exist.
    assert!(gui.contains("app.adopt_settings("), "the startup must adopt the settings through the shared handle");
    let door = gui.split("fn adopt_settings").nth(1).expect("the shared handle of the settings is in place");
    let body = door.split("\n    ///").next().unwrap_or(door);
    assert!(body.contains("self.apply_language();"), "the language must be applied when the settings are adopted (and so at startup too)");
    assert!(gui.contains("pub language: String"), "the language must live in the settings (and so be saved)");

    // an empty setting means "nothing was chosen", so the system decides rather than a silent English
    let mut app = crate::gui::App::default();
    assert!(app.settings_language_is_empty(), "by default there is no choice");
    app.set_language_for_test("ru");
    let other = tr("menu-file");
    assert_ne!(other, "File", "the chosen language is applied");
    app.set_language_for_test("en");
    assert_eq!(tr("menu-file"), "File");
}

/// THE BUILD WATCHES THE CATALOGUE OF LANGUAGES.
///
/// Found while checking extensibility: a `de/` folder was dropped in, the build was run — and NOTHING.
/// The catalogue is baked in by `include_dir!` at compile time, and cargo did not count it as an input and
/// simply did not rebuild the binary. A silent refusal at exactly the point where somebody tries to
/// contribute FOR THE FIRST TIME: they did everything right and the program pretended the language was not
/// there.
///
/// The test guards a line in `build.rs`: without it, extensibility stays a promise rather than a
/// property.
#[test]
fn the_build_watches_the_language_catalogue() {
    let build = include_str!("../../build.rs");
    assert!(
        build.contains("cargo:rerun-if-changed=../../i18n"),
        "without watching the catalogue an added language is not picked up until a forced rebuild"
    );
}

/// EVERY KERNEL ERROR HAS WORDS — in both languages.
///
/// The kernel gives back a code; if there is no string for the code, the key itself is what gets seen.
/// That is not "ugly", it is the loss of diagnostics at exactly the moment they are needed most — when a
/// feature failed to build.
#[test]
fn every_core_error_has_words_in_every_language() {
    use qymcad_core::errors::{CoreError as E, ExprError as X, Op};
    let mut samples: Vec<E> = vec![
        E::SourceBodyNotBuilt,
        E::SourcePartHasNoBody,
        E::BodyANotBuilt,
        E::BodyBNotBuilt,
        E::FaceNotFound,
        E::FacesNotFound,
        E::CutPlaneDeleted,
        E::SplitPlaneDeleted,
        E::MirrorPlaneUnset,
        E::ZeroNormal,
        E::ZeroThickness,
        E::ZeroPushDistance,
        E::SplitPieceCount { got: 3, want: 2 },
        E::LoftNeedsTwoSections,
        E::DraftNeedsFaces,
        E::SweepProfileMissing,
        E::SweepPathMissing,
        E::NoIsolatedPointsForHoles,
        E::NoPointsForHoles,
        E::ThreadRimNotFound,
        E::AugerRimNotFound,
        E::AugerBadPitchOrLength,
        E::AugerOuterNotBigger { outer: 20.0, shaft: 20.0 },
        E::ThreadRemovedNothing { before: 100.0, after: 100.0 },
        E::AugerAddedNothing { before: 100.0, after: 100.0 },
        E::ProfileNotFound,
        E::RevolveProfileCrossesAxis,
        E::ThreadLengthUnset,
        E::ThreadPitchTooSmall { pitch: 0.01 },
        E::ThreadTooManyTurns { turns: 900.0 },
        E::ThreadDepthTooDeep { depth: 5.0, radius: 4.0, dia: 8.0, pitch: 2.0 },
        E::ThreadFailed,
        E::BodyOnlyInPart,
        E::CrossComponentInput { input: 42 },
        E::SketchOnForeignFace { input: 42 },
        E::SketchFaceRefLost { sketch: 7, body: 9 },
        E::NoContours,
        E::AllEdgesSmooth,
        E::FilletRadiusTooBig { radius: 3.0, issues: vec![qymcad_core::errors::FilletEdgeIssue { edge: 6101, takes_up_to: None }], smooth_skipped: 2 },
        E::FilletEdgesOneByOne { radius: 3.0 },
        E::ChamferTooBig { dist: 4.0 },
        E::SurfaceDoesNotClose { free: 4 },
        E::PushFaceOnSheet,
        E::NeedsSolidNotSheet,
        E::DraftFailed { angle: 4.0 },
        E::AugerFlightFailed,
        E::ArrayEmpty,
        E::EmptyResult,
        E::RemoveFacesFailed { why: "x".into() },
        E::JointUnsatisfied { residual: 0.5 },
        E::Kernel("occt says no".into()),
    ];
    for x in [
        X::UnknownChar("@".into()),
        X::UnknownFn("foo".into()),
        X::NeedsOneArg("sin".into()),
        X::NeedsTwoArgs("max".into()),
        X::ExpectedParen,
        X::ExpectedParenAfterArgs,
        X::UnexpectedToken("]".into()),
        X::TrailingInput("xx".into()),
        X::NotANumber,
    ] {
        samples.push(E::Expr(x));
    }
    for op in Op::all() {
        samples.push(E::OpFailed(*op));
        samples.push(E::KernelRequired(*op));
    }

    for (code, name) in available() {
        set_language(&code);
        for e in &samples {
            let key = e.key();
            let text = error_text(e);
            assert_ne!(text, key, "language {code} ({name}) has no text for the error {key}");
            assert!(!text.trim().is_empty(), "an empty text of the error {key} in language {code}");
            // a substitution left unsubstituted is the same defect as a missing string
            assert!(!text.contains("{$"), "in {code}/{key} the substitution was not filled in: {text}");
        }
    }
    set_language("ru");
}

/// NUMBERS IN ERRORS USE A DOT rather than a comma by locale.
///
/// A diameter written as 12,5 in the middle of technical text reads as a typo: in drawings and in input
/// fields there is a dot everywhere.
#[test]
fn numbers_in_errors_use_a_dot() {
    use qymcad_core::errors::CoreError as E;
    for code in ["ru", "en"] {
        set_language(code);
        let t = error_text(&E::AugerOuterNotBigger { outer: 12.5, shaft: 20.25 });
        assert!(t.contains("12.5"), "in {code} a number must carry a dot: {t}");
    }
    set_language("ru");
}

/// A KERNEL ERROR IS TRANSLATED WHOLE: in an English build there is no Cyrillic in it.
///
/// This is the check of what all of it was started for: half the message used to come from the kernel and
/// stayed in one language whatever the language of the interface.
#[test]
fn in_english_a_core_error_has_no_cyrillic() {
    use qymcad_core::errors::CoreError as E;
    set_language("en");
    for e in [E::CutPlaneDeleted, E::SplitPieceCount { got: 3, want: 2 }, E::OpFailed(qymcad_core::errors::Op::Fillet), E::ThreadRimNotFound] {
        let t = error_text(&e);
        assert!(!t.chars().any(|c| ('а'..='я').contains(&c) || ('А'..='Я').contains(&c)), "an English build and the text carries Cyrillic: {t}");
    }
    set_language("ru");
}

/// EVERY KEY THE CODE ASKS FOR EXISTS IN EVERY LANGUAGE.
///
/// The translation goes area by area and is far from finished, so checking that the catalogue holds
/// nothing spare is premature. The other way round is essential: `tr("tree-title")` with a missing string
/// shows `tree-title` instead of words, and the person using the program notices it rather than we do. The
/// keys are collected FROM THE SOURCES rather than from a list kept by hand: a list would have to be
/// topped up, and the very first forgotten call would slip past.
#[test]
fn every_key_the_code_asks_for_exists_in_every_language() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut asked: Vec<String> = Vec::new();
    let mut stack = vec![root];
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir).expect("the sources read").flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            if p.extension().is_none_or(|x| x != "rs") {
                continue;
            }
            let text = std::fs::read_to_string(&p).expect("the file reads");
            // literal keys only: the ones assembled by format! are checked by their own areas.
            // `trn` WAS NOT on the list, and `face-normal` went through that gap: the properties of a
            // face showed the service name instead of words. The neighbouring keys of the family were
            // all in place — the one lost was exactly the one called through another function.
            for m in text
                .match_indices("i18n::tr(\"")
                .chain(text.match_indices("i18n::tr1(\""))
                .chain(text.match_indices("i18n::tr2(\""))
                .chain(text.match_indices("i18n::trn(\""))
            {
                let rest = &text[m.0 + m.1.len()..];
                if let Some(end) = rest.find('"') {
                    let k = rest[..end].to_string();
                    if !k.is_empty() && !asked.contains(&k) {
                        asked.push(k);
                    }
                }
            }
        }
    }
    assert!(asked.len() > 100, "suspiciously few keys collected: {}", asked.len());

    let prev = language();
    let mut holes: Vec<String> = Vec::new();
    for (code, _) in available() {
        set_language(&code);
        for k in &asked {
            let t = tr(k);
            if &t == k || t.trim().is_empty() {
                holes.push(format!("{code}: {k}"));
            }
        }
    }
    set_language(&prev);
    assert!(holes.is_empty(), "the interface would show keys instead of words ({}):\n{}", holes.len(), holes.join("\n"));
}

/// A FEATURE LABEL COMES OUT WHOLE IN EVERY LANGUAGE.
///
/// These strings carry substitutions (`{ $h }`, `{ $angle }`), and a forgotten argument shows up not as
/// emptiness but as `{$h}` in the middle of the label — that is, the timeline displays rubbish. Every
/// label is checked with real arguments.
#[test]
fn every_feature_label_is_complete_in_every_language() {
    let n = num(12.345, 1);
    let cases: &[(&str, &[(&str, &str)])] = &[
        ("feat-extrude", &[("h", &n)]),
        ("feat-revolve", &[("angle", "45")]),
        ("feat-sweep", &[]),
        ("feat-loft", &[("n", "3")]),
        ("feat-push-face", &[("d", "+1.0")]),
        ("feat-remove-face", &[("n", "2")]),
        ("feat-split-face", &[]),
        ("feat-thicken", &[("d", "+1.0")]),
        ("feat-part-instance", &[("name", "Housing")]),
        ("feat-split-body", &[("n", "2")]),
        ("feat-draft", &[("angle", "5"), ("n", "3")]),
        ("feat-box", &[("x", "10"), ("y", "20"), ("z", "30")]),
        ("feat-cylinder", &[("d", "10"), ("h", "20")]),
        ("feat-sphere", &[("d", "10")]),
        ("feat-fillet", &[("r", "1.0"), ("which", "all")]),
        ("feat-chamfer", &[("size", "1.0"), ("which", "all")]),
        ("feat-chamfer-dist-angle", &[("d", "1.0"), ("angle", "45")]),
        ("feat-cone", &[("d1", "10"), ("d2", "5"), ("h", "20")]),
        ("feat-torus", &[("r", "10"), ("r2", "2")]),
        ("feat-prism", &[("n", "6"), ("d", "10"), ("h", "20")]),
        ("feat-shell", &[("t", "1.0"), ("n", "2")]),
        ("feat-linear-array", &[("n", "x3")]),
        ("feat-circular-array", &[("n", "6"), ("angle", "360")]),
        ("feat-mirror", &[("plane", "XY")]),
        ("feat-holes", &[("n", "4"), ("d", "5.0"), ("h", "10.0")]),
        ("feat-hole", &[("d", "5.0"), ("h", "10.0")]),
        ("feat-move", &[]),
        ("feat-mirror-part", &[]),
        ("feat-thread", &[("name", "M8x1.25"), ("side", "inner"), ("len", "20")]),
        ("feat-auger", &[("d", "40"), ("pitch", "10"), ("len", "200")]),
        ("count-edges", &[("n", "4")]),
        ("count-starts", &[("n", "2")]),
    ];
    let prev = language();
    let mut bad: Vec<String> = Vec::new();
    for (code, _) in available() {
        set_language(&code);
        for (key, args) in cases {
            let t = trn(key, args);
            if &t == key || t.trim().is_empty() {
                bad.push(format!("{code}: no string for {key}"));
            } else if t.contains("{$") || t.contains("{ $") {
                bad.push(format!("{code}/{key}: a substitution stayed unfilled - {t}"));
            }
        }
    }
    set_language(&prev);
    assert!(bad.is_empty(), "the timeline would show rubbish:\n{}", bad.join("\n"));
}

/// THE PROGRAM DOES NOT DECIDE ANYTHING BY TEXT WRITTEN FOR A PERSON.
///
/// `apply_feat_cmd` used to search the status line for a substring meaning "did not succeed": that is
/// how it decided whether to commit the undo step or roll it back. While the interface was in one
/// language it worked; once translated, the status would no longer match — and a FAILED operation
/// would land in the history as a successful one. Ctrl+Z then rolls back the wrong thing.
///
/// The guard is not on that one phrase but on the technique: the status line is text for a person, and
/// nothing anywhere may branch on what it says.
#[test]
fn no_code_branches_on_the_text_of_the_status_line() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut found: Vec<String> = Vec::new();
    let mut stack = vec![root.clone()];
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir).expect("the sources read").flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            if p.extension().is_none_or(|x| x != "rs") {
                continue;
            }
            let text = std::fs::read_to_string(&p).expect("the file reads");
            let code = text.split("#[cfg(test)]").next().unwrap_or(&text);
            for (i, line) in code.lines().enumerate() {
                if line.trim_start().starts_with("//") {
                    continue;
                }
                if line.contains("status.contains(") || line.contains("status.starts_with(") {
                    let rel = p.strip_prefix(&root).unwrap_or(&p).display();
                    found.push(format!("{rel}:{}: {}", i + 1, line.trim()));
                }
            }
        }
    }
    assert!(found.is_empty(), "the program branches on text written for a person - translation breaks that silently:\n{}", found.join("\n"));
}

/// ERROR CODES DO NOT SIT IDLE.
///
/// `ExprError` was declared, translated into both languages — and RETURNED BY NOBODY: the expression
/// parser went on handing out ready-made strings of one language. The machinery was built, the old path
/// was never removed, and an English build showed text of the other language. The guard demands that the
/// core actually RETURN the code.
#[test]
fn the_expression_parser_returns_codes_not_words() {
    let src = include_str!("../../../qymcad-core/src/expr.rs");
    assert!(src.contains("ExprError"), "the expression parser must return codes");
    let code = src.split("#[cfg(test)]").next().unwrap_or(src);
    let cyr: Vec<&str> = code
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .filter(|l| l.contains('"') && l.chars().any(|c| ('а'..='я').contains(&c) || ('А'..='Я').contains(&c)))
        .collect();
    assert!(cyr.is_empty(), "words have crept back into the expression parser:\n{}", cyr.join("\n"));

    // and the code reaches text in EVERY language
    let prev = language();
    for (lang, _) in available() {
        set_language(&lang);
        let t = error_text(&qymcad_core::errors::CoreError::Expr(qymcad_core::errors::ExprError::UnknownFn("foo".into())));
        assert!(t.contains("foo"), "in language {lang} what the person typed must survive as it is: {t}");
        assert!(!t.contains("error-expr"), "in language {lang} a key is shown instead of words: {t}");
    }
    set_language(&prev);
}


/// EVERY CODE THE CORE AND THE BRIDGES EMIT HAS WORDS IN EVERY LANGUAGE.
///
/// Written after a report: a drop-down list showed `joint-kind-rigid`, `joint-kind-revolute` — codes
/// instead of words. The neighbouring check did not catch them: it collects keys from literal
/// translation calls IN THE APPLICATION, while these codes are born IN THE CORE, where there is no
/// catalogue at all.
///
/// The libraries have no language of their own — that is a rule of this project, and also its weak
/// spot: the code reaches the window intact, and if no words were entered for it, the person reads a
/// service name. So the codes are looked for WHERE THEY ARE BORN — in every crate but the application —
/// by the prefixes that mark them.
#[test]
fn every_code_the_libraries_emit_has_words_in_every_language() {
    // code prefixes: node names, rebinding, joint kinds, thread standards, materials, connectors,
    // feature labels, fasteners, the OCCT bridge, the file layer, the post-processor, program checking
    const PREFIXES: [&str; 13] =
        ["name-", "rebind-", "joint-", "thread-", "material-", "conn-", "f-", "m3-", "cad-", "io-", "post-", "verify-", "error-"];

    let crates_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("the crates directory").to_path_buf();
    let mut codes: Vec<(String, String)> = Vec::new(); // (code, where it was found)
    let mut stack = vec![crates_dir.clone()];
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir).expect("the sources read").flatten() {
            let p = e.path();
            if p.is_dir() {
                // the application is skipped: its keys are already checked by the neighbouring test
                if p.file_name().is_some_and(|n| n == "qymcad" || n == "target" || n == "tests") {
                    continue;
                }
                stack.push(p);
                continue;
            }
            if p.extension().is_none_or(|x| x != "rs") || !p.components().any(|c| c.as_os_str() == "src") {
                continue;
            }
            let text = std::fs::read_to_string(&p).expect("the file reads");
            let where_ = p.strip_prefix(&crates_dir).unwrap_or(&p).display().to_string();
            for line in text.lines() {
                let t = line.trim_start();
                if t.starts_with("//") || t.starts_with("///") {
                    continue;
                }
                let mut rest = line;
                while let Some(a) = rest.find('"') {
                    let after = &rest[a + 1..];
                    let Some(b) = after.find('"') else { break };
                    let lit = &after[..b];
                    // a code is the WHOLE literal (`name-plane`) or its head up to `#` (`name-body#{n}`)
                    let key = lit.split('#').next().unwrap_or(lit);
                    let looks_like_code = PREFIXES.iter().any(|p| key.starts_with(p))
                        && key.len() > 2
                        && key.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
                    if looks_like_code && !codes.iter().any(|(k, _)| k == key) {
                        codes.push((key.to_string(), where_.clone()));
                    }
                    rest = &after[b + 1..];
                }
            }
        }
    }
    assert!(codes.len() > 30, "suspiciously few codes collected: {} - have the prefixes drifted away from the code?", codes.len());

    let prev = language();
    let mut holes: Vec<String> = Vec::new();
    for (code, _) in available() {
        set_language(&code);
        for (k, where_) in &codes {
            let t = tr(k);
            if &t == k || t.trim().is_empty() {
                holes.push(format!("{code}: {k}  ({where_})"));
            }
        }
    }
    set_language(&prev);
    assert!(holes.is_empty(), "a library emits a code and there are no words for it - the person would read a service name ({}):\n{}", holes.len(), holes.join("\n"));
}

/// A DEFAULT NODE NAME REACHES THE WINDOW AS WORDS, A GIVEN NAME UNTOUCHED.
///
/// Default names are stored in the document AS KEYS (`name-sketch`); otherwise a project started in a
/// build of one language would stay in that language forever. But the same field also holds the name a
/// person typed — and that must never be translated under any circumstances.
#[test]
fn default_names_become_words_and_given_names_stay_as_typed() {
    let prev = language();
    for code in ["ru", "en"] {
        set_language(code);
        for k in ["name-sketch", "name-plane", "name-assembly", "name-part", "name-instance", "name-datum-point", "name-datum-axis"] {
            let t = name(k);
            assert_ne!(t, k, "{code}: the node name is shown as a key");
            assert!(!t.contains('-') || t.contains(' '), "{code}: {k} -> {t} - the key seems to have passed straight through");
        }
        // with a substitution: "Body 3"
        let body = name("name-body#3");
        assert!(body.contains('3'), "{code}: the body number was lost: {body}");
        assert!(!body.contains("name-body"), "{code}: the key passed straight through: {body}");
        // a name a person typed - byte for byte, even when it is spelled like a key. One name is
        // deliberately in another alphabet: a name must survive whatever letters it is written in.
        for given in ["Крышка", "Top Case", "bracket-left", "M8", "name-part-of-mine"] {
            assert_eq!(name(given), given, "{code}: a name given by a person was translated - {given}");
        }
    }
    set_language(&prev);
}

/// ONE KEY, ONE STRING. A twin in the catalogue is lost silently.
///
/// Fluent takes the FIRST definition and discards the second — with no crash and no trace in the window.
/// The catalogue held exactly that: `con-coincident` was both a toolbar hint ("Coincident: 2 points will
/// meet - ...") and the caption in the list of constraints ("Coincident"). The hint won, and the list of
/// constraints carried a sentence a line long instead of a word. Ten such pairs — one per constraint.
///
/// The test reads the catalogue files DIRECTLY rather than through the loaded set: after loading, the
/// twin is no longer visible — by then it is already lost.
#[test]
fn no_key_is_defined_twice_in_a_catalogue() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().and_then(|p| p.parent()).expect("the repository root").join("i18n");
    let mut dupes: Vec<String> = Vec::new();
    let mut stack = vec![root];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            if p.extension().is_none_or(|x| x != "ftl") {
                continue;
            }
            let text = std::fs::read_to_string(&p).expect("the catalogue reads");
            let mut seen: Vec<&str> = Vec::new();
            for line in text.lines() {
                let Some((k, _)) = line.split_once(" = ") else { continue };
                if k.is_empty() || !k.starts_with(|c: char| c.is_ascii_lowercase()) || k.contains(' ') {
                    continue; // a line continuation or a comment is not a definition
                }
                if seen.contains(&k) {
                    dupes.push(format!("{}: {k}", p.file_name().unwrap_or_default().to_string_lossy()));
                } else {
                    seen.push(k);
                }
            }
        }
    }
    assert!(dupes.is_empty(), "a key is defined twice - the second definition is lost silently ({}):\n{}", dupes.len(), dupes.join("\n"));
}

/// A KEY HANDED TO SOMETHING OTHER THAN `tr` IS STILL A KEY.
///
/// Written after a screenshot: the thread popup showed `f-nominal-d`, `f-pitch-std`, `f-length` — service
/// names instead of captions. The neighbouring test could not see them by construction: it collects only
/// literal translation calls, while these keys travelled as a parameter into the constructor of a command
/// field and were translated inside it. Draw the field around the translation, and a key goes to the
/// screen, silently.
///
/// Hence the rule: **a key is recognised by its SHAPE, not by who received it.** The prefixes are taken
/// FROM THE CATALOGUE rather than from a hand-kept list: such a list would have to be topped up for every
/// new family, and the very first forgotten one would slip past. It also cuts off strings that merely look
/// alike (`qymcad`, `g-code`): no such prefixes exist in the catalogue.
#[test]
fn a_key_handed_to_anything_at_all_still_needs_words() {
    // prefixes from the catalogue: everything up to the first hyphen of every real key
    let cat = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().and_then(|p| p.parent()).expect("the repository root").join("i18n");
    let mut prefixes: Vec<String> = Vec::new();
    let mut stack = vec![cat];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            if p.extension().is_none_or(|x| x != "ftl") {
                continue;
            }
            for line in std::fs::read_to_string(&p).expect("the catalogue reads").lines() {
                let Some((k, _)) = line.split_once(" = ") else { continue };
                if !k.starts_with(|c: char| c.is_ascii_lowercase()) || k.contains(' ') {
                    continue;
                }
                if let Some((head, _)) = k.split_once('-') {
                    if !prefixes.iter().any(|x| x == head) {
                        prefixes.push(head.to_string());
                    }
                }
            }
        }
    }
    assert!(prefixes.len() > 10, "suspiciously few prefixes collected from the catalogue: {}", prefixes.len());

    // literals shaped like a key across ALL working code of the application - whoever they are handed to
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut asked: Vec<(String, String)> = Vec::new();
    let mut stack = vec![src.clone()];
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir).expect("the sources read").flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            let n = p.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            if p.extension().is_none_or(|x| x != "rs") || n == "tests.rs" || n.ends_with("_tests.rs") {
                continue;
            }
            let text = std::fs::read_to_string(&p).expect("the file reads");
            let where_ = p.strip_prefix(&src).unwrap_or(&p).display().to_string();
            for line in super::ratchet::tests::working_part(&text).lines() {
                if line.trim_start().starts_with("//") {
                    continue;
                }
                let mut rest = line;
                while let Some(a) = rest.find('"') {
                    let after = &rest[a + 1..];
                    let Some(b) = after.find('"') else { break };
                    let lit = &after[..b];
                    rest = &after[b + 1..];
                    let key = lit.split('#').next().unwrap_or(lit);
                    let shaped = key.len() > 2
                        && key.starts_with(|c: char| c.is_ascii_lowercase())
                        && key.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
                        && key.split_once('-').is_some_and(|(h, _)| prefixes.iter().any(|x| x == h));
                    if shaped && !asked.iter().any(|(k, _)| k == key) {
                        asked.push((key.to_string(), where_.clone()));
                    }
                }
            }
        }
    }
    assert!(asked.len() > 200, "suspiciously few keys collected: {} - has the shape of a key drifted away from the code?", asked.len());

    let prev = language();
    let mut holes: Vec<String> = Vec::new();
    for (code, _) in available() {
        set_language(&code);
        for (k, where_) in &asked {
            let t = tr(k);
            if &t == k || t.trim().is_empty() {
                holes.push(format!("{code}: {k}  ({where_})"));
            }
        }
    }
    set_language(&prev);
    assert!(holes.is_empty(), "the interface would show a service name instead of words ({}):\n{}", holes.len(), holes.join("\n"));
}

/// THE GEOMETRIC FALLBACK ANNOUNCES ITSELF IN WORDS, NOT AS A KEY.
///
/// The event is rare and unpleasant: no name was found, and the element was identified BY PLACE, by
/// resemblance. It used to be visible only as a counter in the tests, that is, not at all to a person.
/// The guard here is that the string must not degenerate into a key (a scenario run separately catches
/// keys in the status line).
#[test]
fn the_fallback_speaks_words_not_a_key() {
    for lang in ["ru", "en"] {
        set_language(lang);
        let s = tr1("io-rebound-by-place", "n", "3");
        assert_ne!(s, "io-rebound-by-place", "{lang}: the string about the fallback degenerated into a key");
        assert!(s.contains('3'), "{lang}: the number of occurrences was not substituted: {s}");
    }
    set_language("ru");
}

/// THE CATALOGUE SPEAKS LIKE A PROGRAM, NOT LIKE CORRESPONDENCE (i18n/README.md, section "Tone").
///
/// The guard exists because the interface read as if it were chatting with the person rather than naming
/// things the way a serious program does. The measurement confirmed it: 221 strings addressed the person
/// familiarly ("pick a contour", "hit it", "poke the face") against 20 that did not.
///
/// WHY A GUARD AND NOT A ONE-OFF PROOFREAD. Tone is not a property of a string but of the CATALOGUE: one
/// new familiar hint among two and a half thousand checked ones reads like a slip of the program, and no
/// eye will catch it during the next edit. A rule is either held by a check or it is not held at all.
///
/// WHAT THE GUARD DOES NOT TOUCH: command names — those are infinitives, not a form of address, and that
/// is how commands are captioned everywhere. Only address TO A PERSON is caught.
///
/// The word list below stays in the language of the catalogue it guards: the words ARE the subject of
/// the check.
#[test]
fn the_catalogue_speaks_like_a_program() {
    // the familiar imperative - the very thing that reads as chatting with the person
    const FAMILIAR: &[&str] = &[
        "выбери", "кликни", "нажми", "укажи", "поставь", "возьми", "потяни", "открой", "закрой", "введи", "наведи", "щёлкни", "щелкни", "перетащи", "отпусти", "начни", "проверь", "задай",
        "жми", "ткни", "двигай", "тяни", "смотри", "сделай", "изволь",
    ];
    let mut bad: Vec<String> = Vec::new();
    for lang in ["ru", "en"] {
        for (key, value) in catalogue(lang) {
            let low = value.to_lowercase();
            for w in FAMILIAR {
                // word boundaries: the bare imperative is caught, its polite form is not
                let found = low.split(|c: char| !c.is_alphabetic()).any(|t| t == *w);
                if found {
                    bad.push(format!("{lang}/{key}: familiar address - \"{w}\" in \"{value}\""));
                }
            }
            // ARROWS ARE NOT JUDGED HERE, AND THAT IS NOT AN OVERSIGHT. The first edition of this guard
            // demanded a unicode arrow instead of `->` and WAS WRONG: raw unicode in the catalogue is
            // drawn as a box, because the icon font intercepts it. The rule and its guard already exist -
            // `gui/font_coverage.rs`, raised after a report of a box in a category caption. It caught
            // that edit, on 233 strings.
        }
    }
    assert!(bad.is_empty(), "the catalogue chats with the person instead of naming things ({} strings):\n{}", bad.len(), bad.join("\n"));
}

/// Every `key = value` pair of one language, straight from the catalogue files.
fn catalogue(lang: &str) -> Vec<(String, String)> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../i18n").join(lang);
    let mut out = Vec::new();
    for e in std::fs::read_dir(&dir).expect("the language directory") {
        let p = e.expect("the file").path();
        if p.extension().and_then(|s| s.to_str()) != Some("ftl") {
            continue;
        }
        let text = std::fs::read_to_string(&p).expect("the .ftl reads");
        let mut cur: Option<(String, String)> = None;
        for line in text.lines() {
            let is_cont = line.starts_with("    ") || line.starts_with('\t');
            match line.split_once('=') {
                Some((k, v)) if !is_cont && k.trim().chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') && !k.trim().is_empty() => {
                    if let Some(kv) = cur.take() {
                        out.push(kv);
                    }
                    cur = Some((k.trim().to_string(), v.trim().to_string()));
                }
                _ => {
                    if let (Some(kv), true) = (cur.as_mut(), is_cont && !line.trim().is_empty() && !line.trim().starts_with('.')) {
                        kv.1.push(' ');
                        kv.1.push_str(line.trim());
                    }
                }
            }
        }
        if let Some(kv) = cur {
            out.push(kv);
        }
    }
    out
}
