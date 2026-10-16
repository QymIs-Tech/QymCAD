//! EVERY OPERATION OF THE KERNEL ENDS AT THE ONE DOOR (`OcctKernel::run` and `finish`, `src/door/`): the result is cleaned of
//! pinched faces and cracks, checked for being empty, broken or of no volume, repaired where it can be, and only then
//! cached as a body. An operation that put its result into the cache itself would hand the document a body none of
//! that was asked of - and a new operation would start with none of the fixes the old ones were given.
//!
//! The check reads the sources of the kernel: every method of `impl Kernel for OcctKernel` that builds a body ends in
//! `self.run(` (or its short ways) or hands over to another such method, and nothing but the door puts a body into the cache.

/// The source of a file of the kernel, its comments taken out.
fn source(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join(name);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{} does not read: {e}", path.display()));
    text.lines().map(|l| l.find("//").map_or(l, |at| &l[..at])).collect::<Vec<_>>().join("\n")
}

/// A method of the `Kernel` implementation: its name, whether it returns a built body, and its body.
struct Method {
    name: String,
    builds: bool,
    body: String,
}

/// The index just past the brace that closes the one opened right before `from`.
fn closing(text: &[u8], from: usize) -> usize {
    let mut depth = 1usize;
    for (i, c) in text.iter().enumerate().skip(from) {
        match c {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return i + 1;
                }
            }
            _ => {}
        }
    }
    text.len()
}

/// The methods of `impl qymcad_core::feature::Kernel for OcctKernel`.
fn kernel_methods() -> Vec<Method> {
    let text = source("kernel.rs");
    let start = text.find("impl qymcad_core::feature::Kernel for OcctKernel {").expect("the Kernel implementation is in kernel.rs");
    let bytes = text.as_bytes();
    let end = closing(bytes, start + text[start..].find('{').expect("its brace") + 1);
    let block = &text[start..end];
    let mut out: Vec<Method> = Vec::new();
    let mut at = 0usize;
    while let Some(found) = block[at..].find("\n    fn ") {
        let head_start = at + found + "\n    fn ".len();
        let name: String = block[head_start..].chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
        let open = head_start + block[head_start..].find('{').expect("a method has a body");
        let close = closing(block.as_bytes(), open + 1);
        let head = &block[head_start..open];
        let builds = head.contains("Result<Built") || head.contains("Result<qymcad_core::geom::Built") || head.contains("Result<Vec<Built>");
        out.push(Method { name, builds, body: block[open..close].to_string() });
        at = close;
    }
    out
}

/// THE DOOR AND ITS SHORT WAYS: `run`, `plain` for an operation with no size (with the roles of its faces or without),
/// and `land_piece` for each body of an operation that makes several.
const DOOR: [&str; 4] = ["self.run(", "self.plain(", "self.plain_named(", "self.land_piece("];

#[test]
fn every_operation_that_builds_ends_at_the_door() {
    let methods = kernel_methods();
    let building: Vec<&str> = methods.iter().filter(|m| m.builds).map(|m| m.name.as_str()).collect();
    // 50 methods build a body on 12.10.2026; a parse that found a handful would pass all
    assert!(building.len() >= 45, "the parse found {} building methods of the kernel, fewer than it has - it is reading them wrong", building.len());
    let mut caught: Vec<String> = Vec::new();
    for m in methods.iter().filter(|m| m.builds) {
        // through the door - `run`, or its short ways - or handed over to an operation of the bridge (`ops/`), which the
        // check below holds to the door; `finish` itself is private to the door, which the compiler holds
        let at_the_door = DOOR.iter().any(|w| m.body.contains(w)) || (m.body.contains("self.") && m.body.contains("_op("));
        let hands_over = building.iter().any(|other| *other != m.name && m.body.contains(&format!("self.{other}(")));
        if !(at_the_door || hands_over) {
            caught.push(m.name.clone());
        }
    }
    assert!(caught.is_empty(), "an operation of the kernel builds a body that does not go through the door `finish`:\n{}", caught.join("\n"));
}

/// THE DOORS OF THE CACHE: what may put a body into it. The door itself; and the two methods that hand finished bodies
/// between the kernel and the half of it a node is built on in parallel (`split_off`, `absorb`) - they move bodies
/// that have been through the door already, they make none.
const MOVERS: [&str; 2] = ["split_off", "absorb"];

#[test]
fn nothing_but_the_door_puts_a_body_into_the_cache() {
    let mut caught: Vec<String> = Vec::new();
    for m in kernel_methods() {
        if m.body.contains("shapes.borrow_mut()") && !MOVERS.contains(&m.name.as_str()) {
            caught.push(format!("kernel.rs: {}", m.name));
        }
    }
    let kernel = source("kernel.rs");
    let outside_impl = kernel.split("impl qymcad_core::feature::Kernel for OcctKernel {").next().unwrap_or_default();
    if outside_impl.contains("shapes.borrow_mut()") {
        caught.push("kernel.rs: outside the Kernel implementation".into());
    }
    let door = source("door/land.rs");
    assert!(door.contains("self.shapes.borrow_mut().insert(body, shape)"), "the door no longer caches the body it lets through");
    assert!(caught.is_empty(), "a body is put into the cache past the door:\n{}", caught.join("\n"));
}

/// AN OPERATION OF THE BRIDGE GOES THROUGH THE DOOR: every `*_op` of `src/ops/` runs its work by the door (or hands
/// over to another operation that does).
#[test]
fn every_operation_of_the_bridge_runs_through_the_door() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join("ops");
    let mut caught: Vec<String> = Vec::new();
    let mut seen = 0usize;
    for e in std::fs::read_dir(&dir).expect("the operations of the bridge").flatten() {
        let name = format!("ops/{}", e.file_name().to_string_lossy());
        if name == "ops/mod.rs" {
            continue;
        }
        let text = source(&name);
        let bytes = text.as_bytes();
        let mut at = 0usize;
        while let Some(found) = text[at..].find("pub(crate) fn ") {
            let head = at + found + "pub(crate) fn ".len();
            let fname: String = text[head..].chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
            let open = head + text[head..].find('{').expect("a body");
            let close = closing(bytes, open + 1);
            if fname.ends_with("_op") {
                seen += 1;
                let body = &text[open..close];
                let hands_over = body.contains("self.") && body.matches("_op(").count() > 0;
                if !(DOOR.iter().any(|w| body.contains(w)) || hands_over) {
                    caught.push(format!("{name}: {fname}"));
                }
            }
            at = close;
        }
    }
    assert!(seen >= 1, "no operation of the bridge was found - the check reads the wrong place");
    assert!(caught.is_empty(), "an operation of the bridge does not go through the door:\n{}", caught.join("\n"));
}
