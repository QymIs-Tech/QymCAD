//! A BLEND BUILT A HAIR SHORT ADVISES ONLY A SIZE THAT BUILDS, on the box and the rounding of the report: R10 on the four
//! top edges of a box of 20 is built a hair short and says so; a slightly larger radius is refused - the roundings of
//! opposite edges overlap - and a slightly smaller one builds clean. The words of the warning, in every language of the
//! catalogue, advise the smaller one alone.
//!
//! Reported behaviour: the warning advised "make it slightly smaller or larger", and R10.01 and R10.001 were refused.
use crate::gui::App;
use qymcad_core::errors::CoreError;
use qymcad_core::model::Id;

/// Each language of the catalogue on its line, then its words for "or larger" - the advice the box of the report cannot
/// follow.
const OR_LARGER: &str = include_str!("or_larger.words");

/// A box of 20 with the four edges of its top rounded, rebuilt, and the node of the rounding.
struct Rounded {
    app: App,
    node: Id,
}

/// The box of 20 with its top rounded to `radius`.
fn rounded(radius: f64) -> Rounded {
    let mut app = App { project: qymcad_core::model::Project::default(), ..Default::default() };
    app.project.new_document();
    let cube = app.project.add_box(20.0, 20.0, 20.0);
    crate::gui::io_jobs::regenerate_now(&mut app.rebuild_ctx());
    let top: Vec<u32> = app.project.regen_edges[&cube].iter().filter(|e| (e.a[2] - 20.0).abs() < 1e-9 && (e.b[2] - 20.0).abs() < 1e-9).map(|e| e.id).collect();
    assert_eq!(top.len(), 4, "setup: the top of the box has four edges");
    let node = app.project.add_fillet(cube, radius, top);
    crate::gui::io_jobs::regenerate_now(&mut app.rebuild_ctx());
    Rounded { app, node }
}

#[test]
fn a_blend_built_a_hair_short_advises_only_a_size_that_builds() {
    let exact = rounded(10.0);
    assert_eq!(exact.app.project.regen_warnings.get(&exact.node), Some(&CoreError::BuiltSteppedBack), "setup: R10 on the top of a box of 20 is built a hair short and says so");
    let larger = rounded(10.01);
    assert!(larger.app.project.regen_errors.contains_key(&larger.node), "a slightly larger rounding builds, so the warning may advise it: {:?}", larger.app.project.regen_warnings.get(&larger.node));
    let Rounded { app: smaller, node } = rounded(9.99);
    assert!(
        !smaller.project.regen_errors.contains_key(&node) && !smaller.project.regen_warnings.contains_key(&node),
        "the slightly smaller rounding the warning advises does not build clean: {:?} {:?}",
        smaller.project.regen_errors.get(&node),
        smaller.project.regen_warnings.get(&node)
    );
    let listed: Vec<&str> = OR_LARGER.lines().filter_map(|l| l.split_once(' ').map(|w| w.0)).collect();
    for (lang, _) in qymcad_i18n::available() {
        assert!(listed.contains(&lang.as_str()), "the language {lang} has no line in or_larger.words");
    }
    let was = qymcad_i18n::language();
    let mut bad = Vec::new();
    for line in OR_LARGER.lines() {
        let Some((lang, or_larger)) = line.split_once(' ') else { continue };
        qymcad_i18n::set_language(lang);
        let words = crate::gui::error_words::error_text(&CoreError::BuiltSteppedBack);
        if words.contains(or_larger) {
            bad.push(format!("{lang}: {words}"));
        }
    }
    qymcad_i18n::set_language(&was);
    assert!(bad.is_empty(), "the warning advises a larger size, which is refused:\n{}", bad.join("\n"));
}
