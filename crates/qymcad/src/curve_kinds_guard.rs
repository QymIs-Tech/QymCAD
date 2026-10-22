//! Guard on THE KINDS OF CURVE: a kind of curve of a sketch is told apart in the curve module alone - `model/curve.rs`
//! and `model/curve/` - and nowhere else, the code and its checks alike. Zero, with no marks and no list of old places.
//!
//! A tool that matched on the kinds itself took a line, a circle and an arc and left out what came later: a spline and
//! an ellipse were no curve to the trim, the fillet, the snaps, the pick and the drawing, each of them in its own way.
//! Read through the parts of a kind (`as_line` and the like), laid from them (`LineParts { a, b }.into()`) or asked of
//! the curve module (`circle_by`, `point_on`, `Curve::of`), a curve is one geometry everywhere.
//!
//! WHAT IS COUNTED is a variant of the kind named outside the module - the kind, two colons and a capital, in a pattern
//! or a constructor - and an alias that would hide one (the kind renamed by `as`). A function of the kind, its name in
//! small letters (`spline`, `laid`), is the module's own door and is not counted.
//!
//! THE SAME FOR THE GEOMETRY OF A CURVE (`Curve` of `model/curve`) AND THE CURVE A CONSTRAINT REFERS TO (`CurveRef`):
//! their variants are named in the curve module alone. The cut of a curve into pieces of its kind, its offset, its edges
//! of a profile, its arrangement into regions and the curve a constraint of the solver refers to are functions of the
//! module, and the operations call them. Outside it, by name and for a reason: the probes, whose oracles read the kind
//! of what a tool made; `gui/session/space.rs`, where the session hands a curve out in words to a script; and
//! `qymcad-meshfit` with `qymcad-kernel/src/recognise.rs`, whose `Curve` is another type of the same name - the curve
//! fitted to an edge of a mesh.
//!
//! A CHAIN OF THE PARTS OF THE KINDS tells the kinds apart as a match does: the parts of one kind read after `else if`,
//! an arm of a match guarded by whether a curve has the parts of a kind, a `match` on the parts of two kinds read at
//! once. One reading of a kind's parts is a door; a chain of them is a match on the kinds written longhand, and is
//! counted.

#[cfg(test)]
mod tests {
    /// The places in `text` that name a variant of a kind of curve, or alias the kind, as `line: column` pairs read by a
    /// person.
    fn kinds_named(text: &str) -> Vec<String> {
        let mut out = Vec::new();
        for (n, line) in text.lines().enumerate() {
            let mut from = 0;
            while let Some(k) = line[from..].find("EntityKind") {
                let at = from + k;
                let rest = &line[at + "EntityKind".len()..];
                let variant = rest.strip_prefix("::").is_some_and(|v| v.chars().next().is_some_and(|c| c.is_ascii_uppercase()));
                let alias = rest.trim_start().starts_with("as ");
                if variant || alias {
                    out.push(format!("{}:{}", n + 1, at + 1));
                }
                from = at + "EntityKind".len();
            }
        }
        out
    }

    /// The places in `text` that name a variant of the geometry of a curve or of the curve a constraint refers to - the
    /// type, two colons and a capital - as `line: column` pairs. A name that only ends in the type's (`ArrCurve`,
    /// `EdgeCurve`) is another type and is not counted.
    fn curves_named(text: &str) -> Vec<String> {
        let mut out = Vec::new();
        for path in ["Curve::", "CurveRef::"] {
            for (n, line) in text.lines().enumerate() {
                let mut from = 0;
                while let Some(k) = line[from..].find(path) {
                    let at = from + k;
                    let own = line[..at].chars().next_back().is_none_or(|c| !(c.is_alphanumeric() || c == '_'));
                    let variant = line[at + path.len()..].chars().next().is_some_and(|c| c.is_ascii_uppercase());
                    if own && variant {
                        out.push(format!("{}:{}", n + 1, at + 1));
                    }
                    from = at + path.len();
                }
            }
        }
        out
    }

    /// The readings of the parts of a kind: `as_line(` and the others of `model/curve/kinds.rs`.
    const PARTS: [&str; 5] = ["line", "arc", "circle", "ellipse", "spline"];

    /// THE PLACES IN `text` WHERE THE PARTS OF THE KINDS ARE READ IN A CHAIN, as `line: column` pairs: a reading after
    /// `else if`, an arm of a match guarded by a reading asked whether it is there, and a `match` on two readings.
    fn chains_named(text: &str) -> Vec<String> {
        let mut out = Vec::new();
        for (n, line) in text.lines().enumerate() {
            let calls: Vec<String> = PARTS.iter().map(|k| format!(".as_{k}()")).collect();
            let readings: Vec<usize> = calls.iter().flat_map(|c| line.match_indices(&c[..c.len() - 1]).map(|(at, _)| at)).collect();
            let Some(&first) = readings.iter().min() else { continue };
            let after_else = line.find("else if ").is_some_and(|e| e < first);
            // an arm whose guard asks of a reading whether it is there: `if <curve>` + call + `.is_some() =>`
            let asked = |at: usize| {
                calls.iter().any(|c| ["is_some()", "is_none()"].iter().any(|q| line[at..].starts_with(&format!("{c}.{q}")) && line[at + c.len() + q.len() + 1..].trim_start().starts_with("=>")))
            };
            let guard = line.find(" if ").is_some_and(|g| g < first) && readings.iter().any(|&at| asked(at));
            let pair = line.find("match (").is_some_and(|m| m < first) && readings.len() >= 2;
            if after_else || guard || pair {
                out.push(format!("{}:{}", n + 1, first + 1));
            }
        }
        out
    }

    /// Whether `path` may name a variant of the geometry of a curve, outside the curve module: a probe, the session that
    /// hands a curve out in words, or a crate whose `Curve` is another type (see the head of this file).
    fn may_name_curves(path: &std::path::Path) -> bool {
        let p = path.to_string_lossy().replace('\\', "/");
        let probe = p.contains("/tests/") || p.ends_with("tests.rs");
        probe || p.ends_with("qymcad/src/gui/session/space.rs") || p.contains("qymcad-meshfit/") || p.ends_with("qymcad-kernel/src/recognise.rs")
    }

    /// Whether `path` is the curve module, where the kinds are told apart.
    fn the_curve_module(path: &std::path::Path) -> bool {
        let p = path.to_string_lossy().replace('\\', "/");
        p.ends_with("qymcad-core/src/model/curve.rs") || p.contains("qymcad-core/src/model/curve/")
    }

    #[test]
    fn the_signal_catches_a_variant_and_an_alias_and_spares_a_door() {
        let kind = "EntityKind";
        let text = format!("let a = {kind}::Line {{ a, b }};\nuse qymcad_core::model::{kind} as K;\nlet s = {kind}::spline(n, h, e);\nfn f(k: &{kind}) {{}}\n");
        assert_eq!(kinds_named(&text), ["1:9", "2:25"], "a variant and an alias are counted; a function of the kind and the type itself are not");
    }

    #[test]
    fn the_signal_catches_a_variant_of_a_curve_and_spares_another_type() {
        let curve = "Curve";
        let text = format!("let c = {curve}::Line {{ a, b }};\nlet d = Arr{curve}::Line {{ a, b }};\nlet e = Edge{curve}::Line {{ point, dir }};\nlet f = {curve}::of(s, e);\n");
        assert_eq!(curves_named(&text), ["1:9"], "a variant of the curve is counted; another type ending in Curve and a function of the curve are not");
    }

    #[test]
    fn the_signal_catches_a_reference_to_a_curve_and_a_chain_of_parts() {
        let (r, a) = ("CurveRef", "as_");
        let text = format!(
            "let r = {r}::Spline {{ curve }};\nlet q = r.open_ends();\n}} else if let Some(c) = k.{a}circle() {{\nSome(k) if k.{a}arc().is_some() => 1,\nmatch (a.{a}line(), b.{a}spline()) {{\n\
             if let Some(l) = k.{a}line() {{\n[x] => k.and_then(|e| if let Some(l) = e.{a}line() {{ Some(l) }} else {{ None }}),\n"
        );
        assert_eq!(curves_named(&text), ["1:9"], "a variant of the reference is counted, a function of it is not");
        assert_eq!(chains_named(&text), ["3:26", "4:13", "5:9"], "a reading after else, in a guard and two in a match are counted; one reading alone, or one inside an arm, is not");
    }

    #[test]
    fn no_kind_of_curve_is_told_apart_outside_the_curve_module() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().and_then(|p| p.parent()).expect("repository root").join("crates");
        let mut found = Vec::new();
        let mut stack = vec![root];
        while let Some(dir) = stack.pop() {
            for e in std::fs::read_dir(&dir).expect("sources are readable").flatten() {
                let path = e.path();
                if path.is_dir() {
                    if path.file_name().is_some_and(|n| n != "target") {
                        stack.push(path);
                    }
                    continue;
                }
                if path.extension().is_none_or(|x| x != "rs") || the_curve_module(&path) {
                    continue;
                }
                let text = std::fs::read_to_string(&path).unwrap_or_default();
                found.extend(kinds_named(&text).into_iter().map(|at| format!("{}:{at}", path.display())));
                if !may_name_curves(&path) {
                    found.extend(curves_named(&text).into_iter().chain(chains_named(&text)).map(|at| format!("{}:{at}", path.display())));
                }
            }
        }
        assert!(
            found.is_empty(),
            "a kind of curve told apart outside model/curve - read it through the parts of its kind (as_line and the like, \
             model/curve/kinds.rs), lay it from them (LineParts {{ a, b }}.into()), or put what it means into the curve module:\n{}",
            found.join("\n")
        );
    }
}
