//! A 3MF OR AMF NESTED TOO DEEP IS REFUSED, and a tree of elements of any depth is freed without recursion.
//!
//! The reader builds the tree in a loop, but freeing it took one nested call per level: a 3MF of 2,208 bytes nested
//! 43,750 levels deep ran an import thread's 2 MB stack out, and the program died. A 3MF model nests 6 levels and an AMF
//! 7. The checks run on a thread with the stack the program's import threads have.
use std::io::Write;

/// A folder for the files, under `target`: nothing goes to `/tmp`, which lives in memory.
fn file(name: &str) -> String {
    let dir = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/deep-probe"));
    std::fs::create_dir_all(&dir).expect("a folder for the check");
    dir.join(name).to_string_lossy().into_owned()
}

/// Elements nested `depth` levels deep, the outermost named `root`.
fn nested(root: &str, depth: usize) -> String {
    format!("<?xml version=\"1.0\"?><{root}>{}{}</{root}>", "<a>".repeat(depth - 1), "</a>".repeat(depth - 1))
}

/// A 3MF package whose model is nested `depth` levels deep.
fn deep_3mf(name: &str, depth: usize) -> String {
    let p = file(name);
    let mut z = zip::ZipWriter::new(std::fs::File::create(&p).expect("created"));
    z.start_file("3D/3dmodel.model", zip::write::SimpleFileOptions::default()).expect("a part");
    z.write_all(nested("model", depth).as_bytes()).expect("written");
    z.finish().expect("closed");
    p
}

/// `f` on a thread with the 2 MB stack of the program's import threads.
fn on_an_import_thread<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new().stack_size(2 << 20).spawn(f).expect("a thread").join().expect("the thread came back")
}

/// The refusal says the file is nested too deep, not that it is broken.
fn too_deep(err: Option<String>) -> bool {
    err.is_some_and(|e| e.starts_with("io-xml-too-deep#"))
}

#[test]
fn an_amf_nested_fifty_thousand_levels_is_refused() {
    let p = file("deep.amf");
    std::fs::write(&p, nested("amf", 50_000)).expect("written");
    let err = on_an_import_thread(move || qymcad_io::import_amf(&p).err());
    assert!(too_deep(err.clone()), "the AMF was not refused as nested too deep: {err:?}");
}

#[test]
fn a_3mf_nested_fifty_thousand_levels_is_refused() {
    let p = deep_3mf("deep.3mf", 50_000);
    let err = on_an_import_thread(move || qymcad_io::import_3mf(&p).err());
    assert!(too_deep(err.clone()), "the 3MF was not refused as nested too deep: {err:?}");
}

/// THE LIMIT IS THE LAST DEPTH READ: a document exactly `MAX_DEPTH` levels deep is read, one level more is refused.
#[test]
fn the_limit_is_the_last_depth_read() {
    use qymcad_io::xml;
    assert!(xml::parse(&nested("a", xml::MAX_DEPTH)).is_ok(), "a document {} levels deep was refused", xml::MAX_DEPTH);
    assert_eq!(xml::parse(&nested("a", xml::MAX_DEPTH + 1)).err(), Some(xml::XmlError::TooDeep), "one level past the limit was read");
}

/// A TREE OF ANY DEPTH IS FREED IN A LOOP: built by hand, past what the reader would ever build, and dropped on an
/// import thread's stack.
#[test]
fn a_tree_of_any_depth_is_freed_without_recursion() {
    on_an_import_thread(|| {
        let mut tree = qymcad_io::xml::Node::default();
        for _ in 0..100_000 {
            let mut outer = qymcad_io::xml::Node::default();
            outer.children.push(tree);
            tree = outer;
        }
        drop(tree);
    });
}
