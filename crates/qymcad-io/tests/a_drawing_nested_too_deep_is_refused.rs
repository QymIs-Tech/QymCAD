//! AN SVG NESTED DEEPER THAN `usvg` ALLOWS IS REFUSED BEFORE IT IS PARSED.
//!
//! `usvg` refuses a node more than 1024 levels below the outermost element, but only after `roxmltree` has parsed the
//! whole text - its tokenizer recursing once per level, through the text of an entity where it is used as well - and
//! 3,466 nested groups (a 24 KB file) ran an import thread's 2 MB stack out: the program died. The text is now scanned
//! for its depth first, with `usvg`'s own rule and number. The checks run on a thread with the 2 MB stack of the
//! program's import threads.
use qymcad_io::{import_svg, SVG_MAX_DEPTH};

/// A folder for the files, under `target`: nothing goes to `/tmp`, which lives in memory.
fn file(name: &str, bytes: &[u8]) -> String {
    let dir = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/svg-depth-probe"));
    std::fs::create_dir_all(&dir).expect("a folder for the check");
    let p = dir.join(name);
    std::fs::write(&p, bytes).expect("written");
    p.to_string_lossy().into_owned()
}

/// A drawing whose innermost element - a short line - sits `depth` levels below the outermost `<svg>` (which is at 0).
fn nested(depth: usize) -> String {
    let groups = depth - 1;
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"10mm\" height=\"10mm\" viewBox=\"0 0 10 10\">{}<path d=\"M 1 1 L 9 1\" stroke=\"black\"/>{}</svg>",
        "<g>".repeat(groups),
        "</g>".repeat(groups)
    )
}

/// `text` as a gzip stream - what an `.svgz` is. The deflate stream is taken out of a zip archive (the `zip` crate is
/// what the reader already has): compressed for real, so not one `<g>` of the text is left to be seen in the bytes.
fn gzip(text: &[u8]) -> Vec<u8> {
    use std::io::{Read, Write};
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    zip.start_file("drawing.svg", zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated)).expect("an entry");
    zip.write_all(text).expect("compressed");
    let mut archive = zip::ZipArchive::new(zip.finish().expect("an archive")).expect("read back");
    let mut entry = archive.by_index_raw(0).expect("the entry");
    let crc = entry.crc32();
    let mut out = vec![0x1f, 0x8b, 8, 0, 0, 0, 0, 0, 0, 0xff];
    entry.read_to_end(&mut out).expect("the deflate stream");
    assert!(!out.windows(3).any(|w| w == b"<g>"), "the drawing is not compressed: its groups are seen in the bytes");
    out.extend(crc.to_le_bytes());
    out.extend((text.len() as u32).to_le_bytes());
    out
}

/// `f` on a thread with the 2 MB stack of the program's import threads.
fn on_an_import_thread<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new().stack_size(2 << 20).spawn(f).expect("a thread").join().expect("the thread came back")
}

fn too_deep() -> Option<String> {
    Some(format!("io-svg-too-deep#{SVG_MAX_DEPTH}"))
}

#[test]
fn a_drawing_nested_five_thousand_levels_is_refused() {
    let p = file("deep.svg", nested(5_000).as_bytes());
    assert_eq!(on_an_import_thread(move || import_svg(&p).err()), too_deep(), "a drawing 5,000 levels deep was not refused before parsing");
}

#[test]
fn a_compressed_drawing_is_scanned_as_it_is_read() {
    let p = file("deep.svgz", &gzip(nested(5_000).as_bytes()));
    assert_eq!(on_an_import_thread(move || import_svg(&p).err()), too_deep(), "an .svgz 5,000 levels deep was not refused before parsing");
}

/// THE LIMIT IS `usvg`'s: an element at depth 1024 is read, one at 1025 is refused here.
#[test]
fn the_limit_is_usvgs() {
    let at = file("at-limit.svg", nested(SVG_MAX_DEPTH).as_bytes());
    let past = file("past-limit.svg", nested(SVG_MAX_DEPTH + 1).as_bytes());
    let (at, past) = on_an_import_thread(move || (import_svg(&at).map(|s| s.curves.len()), import_svg(&past).err()));
    assert!(at.as_ref().is_ok_and(|&n| n > 0), "a line at depth {SVG_MAX_DEPTH} did not come in: {at:?}");
    assert_eq!(past, too_deep(), "a line at depth {} was not refused", SVG_MAX_DEPTH + 1);
}

/// NOTHING BUT ELEMENTS COUNTS: a drawing whose line sits AT the limit is read with, before its groups, a doctype whose
/// internal subset holds a `>`, a comment, a CDATA section and a quoted value with a `>` in them. Taken for markup, any
/// one of them would leave an element open that never closes, put the line one level past the limit and refuse it.
#[test]
fn comments_cdata_doctype_and_quotes_do_not_count() {
    let decoys = concat!("<!-- a > <g> -->", "<style><![CDATA[ a > <g> ]]></style>", "<path d=\"M 1 2 L 9 2\" stroke=\"black\" data-note=\"a > b\"/>");
    let text =
        |depth: usize| format!("<?xml version=\"1.0\"?><!DOCTYPE svg [ <!ENTITY e \"a > <g>\"> ]>{}", nested(depth).replacen("viewBox=\"0 0 10 10\">", &format!("viewBox=\"0 0 10 10\">{decoys}"), 1));
    let at = file("decoys.svg", text(SVG_MAX_DEPTH).as_bytes());
    let past = file("decoys-past.svg", text(SVG_MAX_DEPTH + 1).as_bytes());
    let (at, past) = on_an_import_thread(move || (import_svg(&at).map(|s| s.curves.len()), import_svg(&past).err()));
    assert!(at.as_ref().is_ok_and(|&n| n >= 2), "a drawing at the limit with comments, CDATA, a doctype and quotes was refused or lost a line: {at:?}");
    // and the other way: a scan that took one of them for more than it is could pass over the elements after it and
    // let a drawing past the limit through
    assert_eq!(past, too_deep(), "a drawing past the limit with comments, CDATA, a doctype and quotes was not refused");
}

/// A doctype whose entities nest `per` groups each, every one using the next, `links` of them; the drawing uses the
/// first, so its line sits `links * per + 1` levels deep, though the body has one element in it.
fn chained(links: usize, per: usize) -> String {
    let mut subset = String::new();
    for k in 0..links {
        let inner = if k + 1 < links { format!("&e{};", k + 1) } else { "<path d='M 1 1 L 9 1' stroke='black'/>".to_string() };
        subset.push_str(&format!("<!ENTITY e{k} \"{}{inner}{}\">", "<g>".repeat(per), "</g>".repeat(per)));
    }
    format!("<?xml version=\"1.0\"?><!DOCTYPE svg [{subset}]><svg xmlns=\"http://www.w3.org/2000/svg\" width=\"10mm\" height=\"10mm\" viewBox=\"0 0 10 10\">&e0;</svg>")
}

/// THE TEXT OF AN ENTITY NESTS WHERE IT IS USED: ten entities of 400 groups each, one using the next, put a line
/// 4,001 levels deep in a body of one element - refused, not parsed into a stack overflow.
#[test]
fn groups_inside_entities_count_where_they_are_used() {
    let p = file("entities.svg", chained(10, 400).as_bytes());
    assert_eq!(on_an_import_thread(move || import_svg(&p).err()), too_deep(), "a drawing nested 4,001 levels through its entities was not refused");
}

/// THE LIMIT HOLDS THROUGH ENTITIES TOO: 3 entities of 341 groups put the line at depth 1024 - read; one group more
/// in the body puts it at 1025 - refused.
#[test]
fn the_limit_through_entities_is_usvgs() {
    let at = file("entities-at.svg", chained(3, 341).as_bytes());
    let past = file("entities-past.svg", chained(3, 341).replace("&e0;", "<g>&e0;</g>").as_bytes());
    let (at, past) = on_an_import_thread(move || (import_svg(&at).map(|s| s.curves.len()), import_svg(&past).err()));
    assert!(at.as_ref().is_ok_and(|&n| n > 0), "a line at depth 1024 through entities did not come in: {at:?}");
    assert_eq!(past, too_deep(), "a line at depth 1025 through entities was not refused");
}

/// A `[` OR `]` INSIDE THE DOCTYPE'S DECLARATIONS DOES NOT END OR PROLONG IT: in an entity's value, a comment and a
/// processing instruction there, then a body 5,000 levels deep - refused, the body scanned.
#[test]
fn brackets_inside_the_doctype_do_not_hide_the_body() {
    let body = nested(5_000);
    for subset in ["<!ENTITY x \"[\">", "<!-- [ -->", "<?pi [ ?>", "<!ENTITY y ']>'>"] {
        let p = file("bracket.svg", format!("<?xml version=\"1.0\"?><!DOCTYPE svg [{subset}]>{body}").as_bytes());
        assert_eq!(on_an_import_thread(move || import_svg(&p).err()), too_deep(), "a deep body after the doctype [{subset}] was not refused");
    }
}

/// THE FIVE NAMES XML READS AS CHARACTERS stay characters: `roxmltree` reads `&lt;` as `<` before it looks at what the
/// doctype declares, so a doctype declaring `lt` as 1,100 nested groups does not nest them where `&lt;` stands - the
/// drawing is read, not refused.
#[test]
fn a_predefined_name_redeclared_stays_a_character() {
    let text = format!(
        "<?xml version=\"1.0\"?><!DOCTYPE svg [<!ENTITY lt \"{}{}\">]><svg xmlns=\"http://www.w3.org/2000/svg\" width=\"10mm\" height=\"10mm\" viewBox=\"0 0 10 10\"><desc>a &lt; b</desc><path d=\"M 1 1 L 9 1\" stroke=\"black\"/></svg>",
        "<g>".repeat(1_100),
        "</g>".repeat(1_100)
    );
    let p = file("predefined.svg", text.as_bytes());
    let got = on_an_import_thread(move || import_svg(&p).map(|s| s.curves.len()));
    assert!(got.as_ref().is_ok_and(|&n| n > 0), "a drawing using `&lt;` as a character was refused or lost its line: {got:?}");
}
