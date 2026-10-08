//! Ratchet for the size of `App`: counters that may only go down.
//!
//! `App` is the application's one object, and the panels are not functions that TAKE it - they are METHODS
//! ON it. That is the shape of the trouble, and it is worth spelling out because the first attempt to
//! measure it looked at the wrong thing: counting `&mut App` gave 245, of which 207 were test scaffolding
//! and the rest mostly test helpers. Panels never take `&mut App`; they take `&mut self`.
//!
//! While a panel is a method on `App` it CANNOT move to a crate of its own - a method belongs to the crate
//! that declares the type. And it does not have to say what it needs: `self` is everything at once, so the
//! reader cannot tell whether a panel touches the document, the camera or the status line without reading
//! its whole body.
//!
//! Three numbers say how far the untangling has got, all read from the source rather than declared:
//!
//! * `IMPL_APP_CEILING` - how many `impl App` blocks exist. Each one is a file reaching into the god object.
//! * `APP_METHODS_CEILING` - how many methods hang on `App`.
//! * `APP_SIZE_CEILING` - how big those blocks are, in word characters - a formatter does not move it.
//! * `APP_FIELDS_CEILING` - how many fields `App` carries.
//!
//! WHY EQUALITY AND NOT "NO MORE THAN". A pair of `<=` and `>=` was tried in the other ratchets and half of
//! it turned out to be dead: at a mark of zero nothing can fall below, so the `>=` could never fire while
//! looking exactly like a guard. `assert_eq!` holds both directions at any mark: growth is a regression, and
//! a drop asks for the mark to be lowered in the same commit - otherwise slack piles up silently.
#[cfg(test)]
mod tests {
    /// Blocks of `impl App`.
    ///
    /// The target is one: the application's own frame loop. Every other block is a panel or a tool that has
    /// not yet been given a narrow context to work through.
    const IMPL_APP_CEILING: usize = 36;

    /// Methods hanging on `App` THAT THE PROGRAM ITSELF USES.
    ///
    /// The target is the few that are genuinely the application's own: start, the frame, the split into a
    /// context. A panel's drawing is not the application's business and should not be reachable from it.
    const APP_METHODS_CEILING: usize = 167;

    /// The size of what lives inside `impl App` blocks, comments included, counted in WORD CHARACTERS (`word_chars`)
    /// rather than in lines. Lines were the measure until the tree was first formatted: rustfmt took the same code
    /// from 6056 lines to 5298 without a word of it changing, and a mark that a formatter moves says nothing about
    /// the god object.
    ///
    /// WHY A SECOND COUNTER NEXT TO THE ONE ABOVE. The method count cannot see the commonest move there is:
    /// a 218-line panel split into four free functions with a four-line wrapper left behind. That is exactly
    /// the work wanted - 214 lines leave the god object - and the method count does not move at all, because
    /// one method went in and one came out. Measured on `place_input_popup`, which is four independent
    /// popups (corner, ellipse, rectangle, polygon) welded into one method.
    ///
    /// It also cannot be gamed the other way. Splitting a method in two adds a line and no more; the only
    /// way this number falls is code leaving `impl App`. That is the thing that has to reach zero before the
    /// interface can live in a crate of its own, since a method belongs to the crate declaring the type.
    const APP_SIZE_CEILING: usize = 177_996;

    /// Methods that exist ONLY so a check can reach inside - `*_for_test` and `*_pub`.
    ///
    /// COUNTED APART, and that is the point. They are compiled only under `cfg(test)`, so they weigh
    /// nothing on the program itself; mixed into the same total they would hide movement, and worse, they
    /// would reward the wrong work - deleting a facade is easy and buys nothing.
    ///
    /// A facade exists because the real function is a METHOD and sees everything; a check has no other way
    /// in. When the function becomes free and takes what it needs, the facade stops being needed - the
    /// check calls it directly. So this number is expected to fall BY ITSELF as the one above falls, and if
    /// it does not, the wrong things are being moved.
    const APP_TEST_FACADES_CEILING: usize = 14;

    /// Fields of `App`.
    ///
    /// The target is the few that are genuinely OWNED state. Anything computable from the document is a
    /// query with a memory, not a field - a stored answer nobody remembers to invalidate is where the bags
    /// of `bool` come from.
    const APP_FIELDS_CEILING: usize = 41;

    /// Every `.rs` of the workspace, build output and the git store aside.
    fn sources() -> Vec<(String, String)> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().and_then(|p| p.parent()).expect("repository root").to_path_buf();
        let mut out = Vec::new();
        let mut stack = vec![root];
        while let Some(dir) = stack.pop() {
            for e in std::fs::read_dir(&dir).expect("sources are readable").flatten() {
                let p = e.path();
                if p.is_dir() {
                    if !p.file_name().and_then(|n| n.to_str()).is_some_and(|n| matches!(n, "target" | ".git")) {
                        stack.push(p);
                    }
                    continue;
                }
                if p.extension().and_then(|x| x.to_str()) == Some("rs") {
                    out.push((p.display().to_string(), std::fs::read_to_string(&p).expect("file is readable")));
                }
            }
        }
        out
    }

    /// THE WORD CHARACTERS OF A LINE: letters, digits and `_` - the names, the numbers, the words of a string or a
    /// comment. What a formatter moves - spaces, line breaks, trailing commas, a pair of braces round a match arm -
    /// is not among them, so the size of the god object reads the same before and after the tree is formatted.
    fn word_chars(l: &str) -> usize {
        l.chars().filter(|c| c.is_alphanumeric() || *c == '_').count()
    }

    /// What `impl App` comes to: its blocks, the methods inside them (working ones and test facades apart), their size
    /// in word characters, and the files holding most of it.
    struct ImplApp {
        blocks: usize,
        methods: usize,
        facades: usize,
        size: usize,
        worst: Vec<FileSize>,
    }

    /// The word characters of `impl App` one file holds.
    struct FileSize {
        file: String,
        size: usize,
    }

    fn impl_app() -> ImplApp {
        let (mut blocks, mut methods, mut facades, mut lines_in, mut per) = (0usize, 0usize, 0usize, 0usize, Vec::new());
        for (name, text) in sources() {
            // The file DEFINING the counter is skipped: the words above would count themselves.
            if name.ends_with("god_object_ratchet.rs") {
                continue;
            }
            let lines: Vec<&str> = text.lines().collect();
            let mut here_lines = 0usize;
            for (i, l) in lines.iter().enumerate() {
                if !(l.starts_with("impl App {") || l.starts_with("impl crate::gui::App {")) {
                    continue;
                }
                blocks += 1;
                let (mut depth, mut seen, mut j) = (0i32, false, i);
                while j < lines.len() {
                    depth += lines[j].matches('{').count() as i32 - lines[j].matches('}').count() as i32;
                    if lines[j].contains('{') {
                        seen = true;
                    }
                    // A METHOD IS A LINE AT ONE LEVEL OF INDENTATION starting with `fn` or a visibility.
                    let t = lines[j];
                    if t.starts_with("    fn ") || t.starts_with("    pub fn ") || t.starts_with("    pub(crate) fn ") || t.starts_with("    pub(super) fn ") {
                        let name = t.split("fn ").nth(1).unwrap_or("").split(['(', '<']).next().unwrap_or("");
                        if name.ends_with("_for_test") || name.ends_with("_pub") {
                            facades += 1;
                        } else {
                            methods += 1;
                        }
                    }
                    if seen && depth <= 0 {
                        break;
                    }
                    j += 1;
                }
                // the block from its `impl` line to its closing brace, by its word characters
                here_lines += lines[i..=j.min(lines.len() - 1)].iter().map(|l| word_chars(l)).sum::<usize>();
            }
            lines_in += here_lines;
            if here_lines > 0 {
                per.push(FileSize { file: name, size: here_lines });
            }
        }
        per.sort_by_key(|a| std::cmp::Reverse(a.size));
        per.truncate(10);
        ImplApp { blocks, methods, facades, size: lines_in, worst: per }
    }

    /// How many fields the `App` struct carries.
    fn app_fields() -> usize {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/gui.rs");
        let text = std::fs::read_to_string(path).expect("gui.rs is readable");
        let head = text.find("struct App {").expect("the App struct is in gui.rs");
        let body = &text[head..];
        let end = body.find("\n}").expect("the struct closes");
        body[..end]
            .lines()
            .skip(1)
            .filter(|l| {
                let t = l.trim_start();
                let indent = l.len() - t.len();
                indent == 4 && t.ends_with(',') && !t.starts_with('/') && !t.starts_with('#') && t.contains(':')
            })
            .count()
    }

    #[test]
    fn fewer_and_fewer_places_hang_methods_on_the_application() {
        let ImplApp { blocks, methods, facades, size: lines_in, worst: per } = impl_app();
        let worst: Vec<String> = per.iter().map(|FileSize { file: f, size: n }| format!("  {n:7} word characters  {f}")).collect();
        assert_eq!(
            blocks, IMPL_APP_CEILING,
            "the count of `impl App` blocks has moved off its mark of {IMPL_APP_CEILING}: now {blocks}.\n\
             MORE means another file reached into the god object instead of asking for what it needs.\n\
             FEWER means progress: lower the mark in the same commit."
        );
        assert_eq!(
            methods, APP_METHODS_CEILING,
            "the count of methods on `App` has moved off its mark of {APP_METHODS_CEILING}: now {methods}.\n\
             A method on `App` sees everything at once and cannot move to a crate of its own."
        );
        assert_eq!(
            lines_in,
            APP_SIZE_CEILING,
            "the size of `impl App` in word characters has moved off the mark of {APP_SIZE_CEILING}: now {lines_in}.\n\
             MORE means drawing or logic was written into the god object rather than beside it.\n\
             FEWER means progress: lower the mark in the same commit.\n\
             Where most of them are:\n{}",
            worst.join("\n")
        );
        assert_eq!(
            facades, APP_TEST_FACADES_CEILING,
            "the count of test facades on `App` has moved off its mark of {APP_TEST_FACADES_CEILING}: now {facades}.\n\
             A facade is needed only while the real function is a method. MORE means a new one was added\n\
             instead of making the function free; FEWER means one stopped being needed - lower the mark."
        );
    }

    #[test]
    fn the_application_object_carries_no_more_state() {
        let n = app_fields();
        assert_eq!(
            n, APP_FIELDS_CEILING,
            "the field count of `App` has moved off its mark of {APP_FIELDS_CEILING}: now {n}.\n\
             MORE means state was added to the one object instead of being computed on demand.\n\
             FEWER means progress: lower the mark to {n} in the same commit."
        );
    }
}
