//! A FIELD THAT EXISTS IN ONE BUILD AND NOT THE OTHER.
//!
//! Reported behaviour: `cargo run --release -p qymcad` refused to build with five copies of
//! "no field `stl_export` on type `&mut gui::App`", while the whole test run was green.
//!
//! The cause was a move, not a decision. `App` carried a debug-only field, and its
//! `#[cfg(debug_assertions)]` stood a line above its own doc comment. When the field moved into a
//! bundle, the field line went and the switch stayed, landing on the doc comment of the NEXT field.
//! From then on `stl_export` existed only in the debug build, while the five places that use it were
//! not gated at all.
//!
//! Nothing caught it: `cargo test` builds with `debug_assertions` on, so every test run compiled the
//! very build in which the field exists. The release build is a second shape of the same code, and
//! the guards below check what a debug-only test run cannot see.
#[cfg(test)]
mod tests {
    /// The body of a struct, from its opening brace to the closing brace at the start of a line.
    fn struct_body<'a>(source: &'a str, header: &str) -> &'a str {
        let at = source.find(header).unwrap_or_else(|| panic!("`{header}` is no longer declared in gui.rs"));
        let body = &source[at + header.len()..];
        let end = body.find("\n}").expect("the struct has to be closed by a brace at the start of a line");
        &body[..end]
    }

    /// No field of `App` may exist in one build and not in the other.
    ///
    /// `App` is reached from everywhere, and the places that reach it are not gated. A field of it
    /// that a build switch can remove takes every one of those places with it, and only in the build
    /// nobody tests.
    #[test]
    fn no_field_of_app_belongs_to_one_build_only() {
        let body = struct_body(include_str!("../gui.rs"), "pub(crate) struct App {");
        let gated: Vec<&str> = body.lines().map(str::trim).filter(|l| l.starts_with("#[cfg(")).collect();
        assert!(
            gated.is_empty(),
            "a field of `App` is behind a build switch ({gated:?}) - every place that uses it has to be behind \
             the same switch, and none of them is; the debug-only test run cannot see the refusal"
        );
    }

    /// A build switch stays glued to the item it belongs to.
    ///
    /// A doc comment between `#[cfg(...)]` and the item is how the switch survived the move of its
    /// own field: it read as an attribute of the next field instead. The conventional order - doc
    /// first, then attributes - leaves nothing to detach.
    #[test]
    fn a_build_switch_is_never_separated_from_its_item_by_a_doc_comment() {
        let source = include_str!("../gui.rs");
        let lines: Vec<&str> = source.lines().map(str::trim).collect();
        let stray: Vec<usize> = (0..lines.len().saturating_sub(1))
            .filter(|&i| lines[i].starts_with("#[cfg(debug_assertions)]") && lines[i + 1].starts_with("///"))
            .map(|i| i + 1)
            .collect();
        assert!(
            stray.is_empty(),
            "gui.rs line(s) {stray:?}: `#[cfg(debug_assertions)]` stands above a doc comment, so it reads as the \
             switch of the item BELOW that comment; put the doc first and the switch directly on its own item"
        );
    }
}
