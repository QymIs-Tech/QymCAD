//! LOCALISATION: the strings of the interface come from the `i18n/` catalogue built into the binary.
//!
//! The design obeys one requirement: **adding a language = dropping in a catalogue**, without a single
//! edit in the code of the interface. Otherwise outside translations would have to be accepted together
//! with edits to the code, which means they will not be sent.
//!
//! Hence three decisions, each of which closes off a way of spoiling everything:
//!
//! 1. **The list of languages IS BUILT FROM THE CATALOGUE** rather than written by hand. Forgetting to
//!    add a language to the menu is impossible: the menu is the catalogue.
//! 2. **The name of a language lives IN THAT LANGUAGE ITSELF** (the key `language-name`) — "Deutsch",
//!    not "German". Otherwise a table of names would have to be kept in the code, and it would diverge
//!    from the catalogue.
//! 3. **A missing key falls back to English** rather than showing emptiness or the key itself. Without
//!    that, the very first translation at 60% coverage would leave the interface full of holes — and
//!    contributions would become frightening to accept.
//!
//! English is THE REFERENCE: coverage is counted against it and the fallback goes to it. Not because it
//! outranks the rest, but because exactly one reference is needed, otherwise "how complete the
//! translation is" says nothing.
use fluent_bundle::{FluentArgs, FluentBundle, FluentResource};
use include_dir::{include_dir, Dir};
use std::cell::RefCell;
use std::collections::BTreeMap;
use unic_langid::LanguageIdentifier;

/// The localisation catalogue is built into the binary: the program must work without external files.
static I18N: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../../i18n");

/// THE REFERENCE language: coverage is counted against it and a missing key falls back to it.
pub const FALLBACK: &str = "en";

/// One language: the code, the name in that language itself, and the parsed bundles of strings.
pub struct Lang {
    /// the BCP-47 code, which is also the name of the folder (`ru`, `en`, `zh-CN`)
    pub code: String,
    /// the name of the language IN ITSELF (from the key `language-name`)
    pub name: String,
    bundle: FluentBundle<FluentResource>,
    #[cfg_attr(not(test), allow(dead_code))]
    /// The keys of this language. Collected on loading: `FluentBundle` cannot enumerate its messages,
    /// and coverage has to be counted — otherwise an outside translator has nothing to tell them what is
    /// left.
    keys: Vec<String>,
}

impl Lang {
    /// WHETHER SUCH A MESSAGE EXISTS — without assembling the string. See [`has_key`].
    fn has(&self, key: &str) -> bool {
        self.bundle.get_message(key).and_then(|m| m.value()).is_some()
    }

    /// The string for a key; `None` means this language has no such key (and then the caller goes to
    /// the reference).
    fn get(&self, key: &str, args: Option<&FluentArgs>) -> Option<String> {
        let msg = self.bundle.get_message(key)?;
        let pattern = msg.value()?;
        let mut errs = Vec::new();
        let out = self.bundle.format_pattern(pattern, args, &mut errs);
        // FORMATTING ERRORS ARE NOT SWALLOWED SILENTLY: a lost substitution is a defect of the
        // translation and must be visible. In a debug build it goes to the log; in a release the string
        // is shown anyway.
        #[cfg(debug_assertions)]
        for e in &errs {
            eprintln!("[i18n] {}/{key}: {e}", self.code);
        }
        Some(strip_isolates(&out))
    }

    /// All the keys of this language — for counting coverage and checking against the reference.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn keys(&self) -> Vec<String> {
        self.keys.clone()
    }
}

/// FLUENT WRAPS SUBSTITUTIONS IN INVISIBLE CHARACTERS (U+2068/U+2069) — they are right for
/// bidirectional text, but in an egui button they are extra glyphs, and in a test an unexplainable
/// mismatch of strings. They are stripped.
fn strip_isolates(s: &str) -> String {
    s.chars().filter(|c| *c != '\u{2068}' && *c != '\u{2069}').collect()
}

/// The message names of a resource — `FluentBundle` does not enumerate them, so they are taken from
/// the parsed AST.
fn collect_keys(res: &FluentResource, out: &mut Vec<String>) {
    for e in res.entries() {
        if let fluent_syntax::ast::Entry::Message(m) = e {
            out.push(m.id.name.to_string());
        }
    }
}

/// Load every language from the built-in catalogue. The key is the language code, the order is by code
/// (and is stable).
fn load_all() -> BTreeMap<String, Lang> {
    let mut out = BTreeMap::new();
    for dir in I18N.dirs() {
        let code = match dir.path().file_name().and_then(|s| s.to_str()) {
            Some(c) => c.to_string(),
            None => continue,
        };
        let li: LanguageIdentifier = match code.parse() {
            Ok(li) => li,
            // a folder with an unintelligible name is not a language; it is skipped silently rather
            // than crashed on
            Err(_) => continue,
        };
        let mut bundle = FluentBundle::new(vec![li]);
        let mut keys: Vec<String> = Vec::new();
        // THE SPACES AROUND SUBSTITUTIONS: fluent puts non-breaking ones in by default, and in the
        // interface they give ragged gaps inside buttons. They are switched off — the texts here are
        // short and typography is not needed.
        bundle.set_use_isolating(false);
        let mut any = false;
        for f in dir.files() {
            if f.path().extension().and_then(|s| s.to_str()) != Some("ftl") {
                continue;
            }
            let Some(text) = f.contents_utf8() else { continue };
            match FluentResource::try_new(text.to_string()) {
                Ok(res) => {
                    collect_keys(&res, &mut keys);
                    if bundle.add_resource(res).is_ok() {
                        any = true;
                    }
                }
                // A BROKEN TRANSLATION FILE MUST NOT BRING THE PROGRAM DOWN: the file is skipped, the
                // language keeps what did parse, and the rest is fetched from the reference.
                Err((res, errs)) => {
                    #[cfg(debug_assertions)]
                    eprintln!("[i18n] {code}/{}: {} parse errors", f.path().display(), errs.len());
                    let _ = errs;
                    collect_keys(&res, &mut keys);
                    if bundle.add_resource(res).is_ok() {
                        any = true;
                    }
                }
            }
        }
        if !any {
            continue;
        }
        let name = {
            let mut errs = Vec::new();
            bundle
                .get_message("language-name")
                .and_then(|m| m.value().map(|p| strip_isolates(&bundle.format_pattern(p, None, &mut errs))))
                // a language without a name of its own should not exist, but if it happens the code is
                // shown rather than the language hidden altogether
                .unwrap_or_else(|| code.clone())
        };
        keys.sort();
        keys.dedup();
        out.insert(code.clone(), Lang { code, name, bundle, keys });
    }
    out
}

thread_local! {
    /// The parsed languages. `thread_local`, because `FluentBundle` is not `Sync` while the interface
    /// lives in one thread; background tasks need no localisation — they talk to nobody.
    static LANGS: BTreeMap<String, Lang> = load_all();
    /// The current language. Changed by a setting; by default resolved from the system locale.
    static CURRENT: RefCell<String> = RefCell::new(String::new());
}

/// The list of available languages: (code, the name in that language). Built from the catalogue — see
/// the module comment: that is exactly why a language cannot be "forgotten in the menu".
pub fn available() -> Vec<(String, String)> {
    LANGS.with(|m| m.values().map(|l| (l.code.clone(), l.name.clone())).collect())
}

/// RESOLVING THE LANGUAGE ON THE FIRST RUN: the system locale if we have such a language, otherwise the
/// reference.
///
/// The comparison goes by the FIRST part of the tag: the system says `ru-RU` while the folder is called
/// `ru`. Demanding an exact match would mean ignoring regional variants — and a system set to a language
/// we have would get an English interface anyway.
pub fn system_default() -> String {
    let sys = sys_locale::get_locale().unwrap_or_default().to_lowercase();
    let base = sys.split(['-', '_']).next().unwrap_or("").to_string();
    LANGS.with(|m| {
        if m.contains_key(&sys) {
            return sys.clone();
        }
        if m.contains_key(&base) {
            return base;
        }
        FALLBACK.to_string()
    })
}

/// Set the current language. An unknown code is silently brought to the reference: a setting from a
/// future version (or from somebody else's build) must not leave the interface without strings.
pub fn set_language(code: &str) {
    let ok = LANGS.with(|m| m.contains_key(code));
    CURRENT.with(|c| *c.borrow_mut() = if ok { code.to_string() } else { FALLBACK.to_string() });
}

/// The current language (its code). Empty means none has been chosen yet, so we go by the system.
pub fn language() -> String {
    CURRENT.with(|c| {
        let cur = c.borrow().clone();
        if cur.is_empty() {
            drop(cur);
            let sys = system_default();
            *c.borrow_mut() = sys.clone();
            sys
        } else {
            cur
        }
    })
}

/// THE STRING FOR A KEY. Absent from the current language, it is taken from the reference; absent there
/// too, the key itself comes back.
///
/// The key instead of emptiness is deliberate: an empty button looks like a breakage of the program,
/// while a visible key says at once what is missing, and it shows up in a screenshot attached to a
/// report.
pub fn tr(key: &str) -> String {
    tr_args(key, None)
}

/// A string with substitutions: `tr_args("bodies-count", Some(&args))`.
pub fn tr_args(key: &str, args: Option<&FluentArgs>) -> String {
    let cur = language();
    LANGS.with(|m| {
        if let Some(s) = m.get(&cur).and_then(|l| l.get(key, args)) {
            return s;
        }
        if let Some(s) = m.get(FALLBACK).and_then(|l| l.get(key, args)) {
            return s;
        }
        key.to_string()
    })
}

/// A NUMBER FOR A CAPTION. Fluent knows no formats like `{:.1}`, and a number in a caption must look
/// the same in every language: this is an engineering document, not prose. The separator is a dot, as in
/// the dimension input field; changing it by locale would mean diverging from what people type
/// themselves.
pub fn num(v: f64, digits: usize) -> String {
    format!("{v:.digits$}")
}

/// A signed number (for offsets: which way it goes matters).
pub fn num_signed(v: f64, digits: usize) -> String {
    format!("{v:+.digits$}")
}

/// A string with one substitution — so that `FluentArgs` need not be assembled by hand at the call
/// site.
pub fn tr1(key: &str, name: &str, value: &str) -> String {
    let mut a = FluentArgs::new();
    a.set(name.to_string(), value.to_string());
    tr_args(key, Some(&a))
}

/// A string with two substitutions.
pub fn tr2(key: &str, n1: &str, v1: &str, n2: &str, v2: &str) -> String {
    trn(key, &[(n1, v1), (n2, v2)])
}

/// A string with any number of substitutions — the captions of features run to three or four values.
pub fn trn(key: &str, args: &[(&str, &str)]) -> String {
    let mut a = FluentArgs::new();
    for (n, v) in args {
        a.set(n.to_string(), v.to_string());
    }
    tr_args(key, Some(&a))
}

/// THE START OF A MESSAGE UP TO THE SUBSTITUTION — for tests that check "was it said" rather than "how".
///
/// A status cannot be compared with a finished phrase when a value is glued into it (a path, a counter):
/// that changes from run to run. Nor can a substring of one particular language be searched for — that is
/// exactly what is being moved away from. The text BEFORE the first substitution is taken: it belongs to
/// the catalogue and changes with it.
#[cfg(test)]
pub fn tr_prefix(key: &str, name: &str) -> String {
    const MARK: &str = "\u{1}";
    let full = tr1(key, name, MARK);
    full.split(MARK).next().unwrap_or_default().to_string()
}

/// THE NAME OF A NODE IN THE LANGUAGE OF THE PERSON.
///
/// The name of a node is AT ONCE data of the document and text on the screen. On creation the kernel puts
/// a KEY there (`name-plane`, `name-body#3` — what follows `#` is a substitution), and it may be renamed
/// to anything at all. The two are told apart by fact: if a recognisable key stands before the `#` AND it
/// exists in the catalogue, it is translated; otherwise it is somebody's own word and must not be
/// touched.
///
/// That way a project named in one language stays in that language in any build — those are the names of
/// ITS parts, not of the interface. And what is newly created gets a name in the language being worked
/// in.
pub fn name(stored: &str) -> String {
    let (key, arg) = match stored.split_once('#') {
        Some((k, v)) => (k, Some(v)),
        None => (stored, None),
    };
    let looks_like_key = !key.is_empty() && key.starts_with(|c: char| c.is_ascii_lowercase()) && key.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if !looks_like_key || !has_key(key) {
        // A NAME OF SEVERAL WORDS IS RESOLVED WORD BY WORD.
        //
        // One space stops the whole string from being a key — and a service name goes out to the screen
        // as it stands. A screenshot arrived showing "joint-kind-slider 5" in the list of joints. The key
        // `joint-kind-slider` IS in the catalogue and translates fine, but the name is stored with a
        // number after a space, was judged whole and was not recognised.
        //
        // The words that ARE in the catalogue get translated and the rest are left alone: somebody's own
        // word stays their word, and a service name is never shown to anybody.
        return stored.split(' ').map(|w| if w == stored { w.to_string() } else { name(w) }).collect::<Vec<_>>().join(" ");
    }
    match arg {
        // A SUBSTITUTION MAY ITSELF BE A KEY. A mirrored part is called `name-mirror-of#<the name of the
        // source>`, and the name of the source is a key too (`name-part-n#1`). Without unwrapping it, what
        // was seen was "name-part-n#1 (mirror)": half the name translated, half a raw catalogue key.
        Some(v) => tr1(key, "v", &name(v)),
        None => tr(key),
    }
}

/// Whether such a key exists in any catalogue at all — otherwise the name belongs to a person.
fn has_key(key: &str) -> bool {
    // EXISTENCE IS ASKED ABOUT, NOTHING IS FORMATTED. `get` used to be called here, that is, the message
    // WAS ASSEMBLED with no arguments — and every key with a substitution (`name-part-n = Part { $v }`)
    // poured "Unknown variable: $v" into the log on every frame. A check for existence has no right to
    // give birth to translation errors.
    LANGS.with(|m| m.get(&language()).is_some_and(|l| l.has(key)) || m.get(FALLBACK).is_some_and(|l| l.has(key)))
}

/// A KERNEL ERROR -> TEXT IN THE LANGUAGE OF THE PERSON.
///
/// The kernel names A FACT (`SplitPieceCount { got, want }`), and the words are chosen here. The numbers
/// are passed AS ARGUMENTS rather than glued into the string in advance: in different languages they land
/// in different places in the phrase.
pub fn error_text(e: &qymcad_core::errors::CoreError) -> String {
    use qymcad_core::errors::{CoreError as E, ExprError as X};
    let mut args = FluentArgs::new();
    match e {
        E::SplitPieceCount { got, want } => {
            args.set("got", *got as i64);
            args.set("want", *want as i64);
        }
        E::AugerOuterNotBigger { outer, shaft } => {
            args.set("outer", fmt1(*outer));
            args.set("shaft", fmt1(*shaft));
        }
        E::ThreadRemovedNothing { before, after } | E::AugerAddedNothing { before, after } => {
            args.set("before", fmt0(*before));
            args.set("after", fmt0(*after));
        }
        E::EdgesNotFound { asked } => args.set("asked", *asked as i64),
        E::ThreadPitchTooSmall { pitch } => args.set("pitch", fmt2(*pitch)),
        E::ThreadTooManyTurns { turns } => args.set("turns", fmt0(*turns)),
        E::ThreadDepthTooDeep { depth, radius, dia, pitch } => {
            args.set("depth", fmt2(*depth));
            args.set("radius", fmt2(*radius));
            args.set("dia", fmt1(*dia));
            args.set("pitch", fmt2(*pitch));
        }
        E::FilletRadiusTooBig { radius, issues, smooth_skipped } => {
            args.set("radius", fmt2(*radius));
            // THE BREAKDOWN BY EDGES is assembled HERE, out of translated pieces: each edge says whether
            // it takes any radius at all. That is the most useful part of the message — it answers "what
            // to do" rather than only "it did not work".
            let parts: Vec<String> = issues
                .iter()
                .map(|i| {
                    let mut a = FluentArgs::new();
                    a.set("edge", i.edge as i64);
                    match i.takes_up_to {
                        Some(m) => {
                            a.set("max", fmt2(m));
                            tr_args("error-fillet-edge-takes-up-to", Some(&a))
                        }
                        None => tr_args("error-fillet-edge-takes-none", Some(&a)),
                    }
                })
                .collect();
            args.set("issues", parts.join(", "));
            args.set(
                "smooth",
                if *smooth_skipped == 0 {
                    String::new()
                } else {
                    let mut a = FluentArgs::new();
                    a.set("n", *smooth_skipped as i64);
                    tr_args("error-fillet-smooth-skipped", Some(&a))
                },
            );
        }
        E::FilletEdgesOneByOne { radius } => args.set("radius", fmt2(*radius)),
        E::ChamferTooBig { dist } => args.set("dist", fmt2(*dist)),
        E::SurfaceDoesNotClose { free } => args.set("n", *free as i64),
        E::OperationSplitBody { pieces } => args.set("n", *pieces as i64),
        E::ShellOfMultiShellBody { shells } => args.set("n", *shells as i64),
        E::ShellThicknessOverRound { thickness, limit } => {
            args.set("t", format!("{thickness:.2}"));
            args.set("r", format!("{limit:.2}"));
        }
        E::DraftFailed { angle } => args.set("angle", fmt2(*angle)),
        E::JointUnsatisfied { residual } => args.set("residual", fmt2(*residual)),
        E::CrossComponentInput { input } | E::SketchOnForeignFace { input } => args.set("input", *input as i64),
        E::SketchFaceRefLost { sketch, body } => {
            args.set("sketch", *sketch as i64);
            args.set("body", *body as i64);
        }
        E::RemoveFacesFailed { why } => args.set("why", why.clone()),
        // A MESSAGE FROM THE BRIDGE TO OCCT IS A CODE: the bridge has no language and hands back a
        // catalogue key. Foreign text (the error number of OCCT itself) passes through as it stands —
        // `name` translates known keys only.
        E::Kernel(msg) => args.set("message", name(msg)),
        E::Expr(x) => match x {
            X::UnknownChar(w) | X::UnknownFn(w) | X::UnknownName(w) | X::NeedsOneArg(w) | X::NeedsTwoArgs(w) | X::UnexpectedToken(w) | X::TrailingInput(w) => {
                args.set("what", w.clone())
            }
            _ => {}
        },
        _ => {}
    }
    tr_args(&e.key(), Some(&args))
}

/// AN EXPRESSION ERROR -> TEXT IN THE LANGUAGE OF THE PERSON.
///
/// A door of its own, because it is called from places where the error is NOT a `CoreError`: the
/// parameter field, the dimension field, the gizmo field. Each of them used to get by on its own — and
/// all three differently: the parameters window printed "(!)" with no reason, the dimension popup showed
/// `Display`, that is, English text in a non-English interface, and the gizmo field said nothing at all.
///
/// The expression parser ALWAYS knew the reason (`ExprError` lists nine kinds, and all of them are
/// translated). It was lost at the last step — in the interface.
pub fn expr_error_text(e: &qymcad_core::errors::ExprError) -> String {
    error_text(&qymcad_core::errors::CoreError::Expr(e.clone()))
}

/// NUMBERS IN MESSAGES ARE FORMATTED HERE rather than handed to fluent: it has typography of its own
/// (digit separators by locale), and in a diameter like 12.5 it gives 12,5 in the middle of technical
/// text where a dot is expected.
fn fmt0(v: f64) -> String {
    format!("{v:.0}")
}
fn fmt1(v: f64) -> String {
    format!("{v:.1}")
}
fn fmt2(v: f64) -> String {
    format!("{v:.2}")
}

/// The keys of the reference language — what the completeness of the rest is measured against.
#[cfg_attr(not(test), allow(dead_code))]
pub fn reference_keys() -> Vec<String> {
    LANGS.with(|m| m.get(FALLBACK).map(|l| l.keys()).unwrap_or_default())
}

/// The keys of a particular language.
#[cfg_attr(not(test), allow(dead_code))]
pub fn keys_of(code: &str) -> Vec<String> {
    LANGS.with(|m| m.get(code).map(|l| l.keys()).unwrap_or_default())
}

/// The string FROM THIS LANGUAGE ONLY, with no fallback to the reference — for the tests and the
/// coverage report.
#[cfg_attr(not(test), allow(dead_code))]
pub fn tr_in(code: &str, key: &str) -> Option<String> {
    LANGS.with(|m| m.get(code).and_then(|l| l.get(key, None)))
}

/// Short form: `t!("menu-file")`. A macro of its own, so that a place where translation happens reads
/// off the code by eye.
#[macro_export]
macro_rules! t {
    ($key:expr) => {
        $crate::i18n::tr($key)
    };
}

#[cfg(test)]
pub(crate) mod ratchet;
#[cfg(test)]
mod tests;
