//! Importing SVG into exact curves. `usvg` resolves the transforms and units and returns flat paths in CSS pixels,
//! turned into millimetres here.
//!
//! SVG curves are Beziers — `usvg` does not keep arcs or circles as primitives but lowers them into cubic
//! Beziers — so the curves are linearised into segments while straight segments stay as they are. Y is flipped,
//! SVG running downwards and CAM upwards. The sketch assembles connected chains from the segments.

use qymcad_core::geom::{Point2, ProfEdge};
use usvg::tiny_skia_path::PathSegment;

use crate::ImportedSketch;

/// Millimetres in a CSS pixel. `usvg` resolves the sheet's units and the viewBox into pixels of 1/96 inch, as the SVG
/// specification defines them, so a sheet `width="42mm"` with a viewBox of 42 comes out 158.74 wide; read as
/// millimetres, every length grew 96/25.4 = 3.78 times (a 40 x 30 rectangle came in as 151.18 x 113.39). A sheet
/// with no unit is in pixels by the same specification.
const MM_PER_PX: f64 = 25.4 / 96.0;

/// THE DEEPEST AN ELEMENT MAY SIT BELOW THE OUTERMOST ONE, as `usvg` counts it: the outermost element at depth 0, one
/// more per level, and a node deeper than this refused (`usvg` 0.48, `parser/svgtree/parse.rs:182`). Fixed when the
/// program is built. `usvg` applies it only to the finished tree, and `roxmltree` builds that tree by recursion: its
/// tokenizer calls itself once per level (`parse_element` -> `parse_content` -> `parse_element`), for the elements
/// of an entity's text where the entity is used as well. 3,466 nested groups (a 24 KB file) ran a 2 MB stack out, the
/// main thread's 8 MB 13,866, and the program died. So the text is scanned for its depth first. An element deeper than
/// this is refused here; one AT this depth with something inside it (even blank text) `usvg` refuses itself, so the
/// two refuse the same files.
pub const SVG_MAX_DEPTH: usize = 1024;

/// How many entity expansions `roxmltree` 0.21 nests inside one another: it refuses the file at the next one.
const ENTITY_LEVELS: usize = 10;

/// Whether an element of the XML text `b` sits deeper than `limit` below the outermost one, as `roxmltree` nests it.
fn deeper_than(b: &[u8], limit: usize) -> bool {
    Depth::default().reach(b, 0, limit + 1) > limit + 1
}

/// A scan of the bytes for their depth, not a parse. It passes over what holds no element - comments, CDATA,
/// processing instructions, quoted attribute values where a `>` may stand - and reads the doctype's internal subset
/// for its entities, as `roxmltree` reads it, since the text of an entity is parsed where the entity is used.
#[derive(Default)]
struct Depth<'a> {
    /// The text of each entity by its name; the first declaration of a name counts, as in `roxmltree`
    entities: std::collections::HashMap<&'a [u8], &'a [u8]>,
    /// The reach of an entity expanded so many levels deep, measured once
    known: std::collections::HashMap<(&'a [u8], usize), usize>,
}

impl<'a> Depth<'a> {
    /// How far the markup `b` reaches below where it stands: 1 for an element, 2 for one with an element inside it,
    /// and an entity used reaches as far as its text does from there. `level` is how many entity expansions `b` sits
    /// in. The scan stops once the reach passes `stop`.
    fn reach(&mut self, b: &'a [u8], level: usize, stop: usize) -> usize {
        let find = |from: &[u8], what: &[u8]| from.windows(what.len()).position(|w| w == what);
        let (mut i, mut open, mut deepest) = (0usize, 0usize, 0usize);
        while i < b.len() && deepest <= stop {
            let rest = &b[i..];
            if rest[0] == b'&' {
                // an entity used in text: its elements nest from here. The name ends at the `;`, looked for only as
                // far as a name goes, so a text of many bare `&` is not read over and over
                let k = 1 + rest[1..].iter().take_while(|&&c| c != b';' && c != b'&' && c != b'<' && !c.is_ascii_whitespace()).count();
                if rest.get(k) == Some(&b';') {
                    deepest = deepest.max(open + self.entity(&rest[1..k], level));
                    i += k + 1;
                } else {
                    i += 1;
                }
                continue;
            }
            if rest[0] != b'<' {
                i += 1;
                continue;
            }
            let skip = |end: &[u8]| find(rest, end).map_or(rest.len(), |k| k + end.len());
            i += if rest.starts_with(b"<!--") {
                skip(b"-->")
            } else if rest.starts_with(b"<![CDATA[") {
                skip(b"]]>")
            } else if rest.starts_with(b"<?") {
                skip(b"?>")
            } else if level == 0 && rest.starts_with(b"<!DOCTYPE") {
                // a doctype stands before the outermost element only; in an entity's text `roxmltree` refuses it
                self.doctype(rest)
            } else if rest.starts_with(b"<!") {
                // nothing else of this kind is legal here; `roxmltree` refuses the file
                skip(b">")
            } else if rest.starts_with(b"</") {
                open = open.saturating_sub(1);
                skip(b">")
            } else {
                // an opening tag, to its `>` past quoted values
                deepest = deepest.max(open + 1);
                let mut quote = 0u8;
                let end = rest.iter().enumerate().skip(1).find(|&(_, &c)| {
                    if quote != 0 {
                        if c == quote {
                            quote = 0;
                        }
                        false
                    } else if c == b'"' || c == b'\'' {
                        quote = c;
                        false
                    } else {
                        c == b'>'
                    }
                });
                match end {
                    Some((k, _)) => {
                        if rest[k - 1] != b'/' {
                            open += 1;
                        }
                        k + 1
                    }
                    None => rest.len(),
                }
            };
        }
        deepest
    }

    /// The reach of the entity `name` used `level` expansions deep. One `roxmltree` would not expand reaches nowhere:
    /// past its nesting of expansions (it refuses the file there), not declared, one of the five XML names it reads as
    /// characters before any declaration (`lt`, `gt`, `amp`, `quot`, `apos`), or a character reference such as
    /// `&#60;` - which `roxmltree` keeps as text even in an entity's text, where the XML specification would make it
    /// markup.
    fn entity(&mut self, name: &'a [u8], level: usize) -> usize {
        if level >= ENTITY_LEVELS || matches!(name, b"lt" | b"gt" | b"amp" | b"quot" | b"apos") {
            return 0;
        }
        let Some(&text) = self.entities.get(name) else { return 0 };
        if let Some(&r) = self.known.get(&(name, level)) {
            return r;
        }
        let r = self.reach(text, level + 1, usize::MAX);
        self.known.insert((name, level), r);
        r
    }

    /// The doctype at the start of `b`; returns its length. Its internal subset is read the way `roxmltree` 0.21 reads
    /// it (`tokenizer.rs`, `parse_doctype`): comments, processing instructions, and ENTITY, ELEMENT, ATTLIST and
    /// NOTATION declarations, a `[` or `]` inside any of them not counting. An entity with a quoted value is kept, a
    /// parameter entity too (`roxmltree` keeps both alike); an external one `usvg` never loads. Anything else ends the
    /// scan of the file: `roxmltree` refuses the file there, before any element is parsed.
    fn doctype(&mut self, b: &'a [u8]) -> usize {
        let spaces = |i: usize| i + b[i..].iter().take_while(|c| c.is_ascii_whitespace()).count();
        let to = |i: usize, what: &[u8]| b[i..].windows(what.len()).position(|w| w == what).map_or(b.len(), |k| i + k + what.len());
        // past a quoted literal from `i`, or nothing
        let quoted = |i: usize| match b.get(i) {
            Some(&q) if q == b'"' || q == b'\'' => b[i + 1..].iter().position(|&c| c == q).map(|k| (&b[i + 1..i + 1 + k], i + k + 2)),
            _ => None,
        };
        // the name and the external identifier, to the subset or the end
        let mut i = b"<!DOCTYPE".len();
        loop {
            match b.get(i) {
                None => return b.len(),
                Some(b'>') => return i + 1,
                Some(b'[') => break,
                _ => i = quoted(i).map_or(i + 1, |(_, after)| after),
            }
        }
        i += 1;
        loop {
            i = spaces(i);
            let rest = &b[i..];
            if rest.starts_with(b"<!ENTITY") {
                let mut k = spaces(i + b"<!ENTITY".len());
                if b.get(k) == Some(&b'%') {
                    k = spaces(k + 1);
                }
                let name_end = k + b[k..].iter().take_while(|&&c| !c.is_ascii_whitespace() && c != b'"' && c != b'\'').count();
                let name = &b[k..name_end];
                k = spaces(name_end);
                if let Some((text, after)) = quoted(k) {
                    self.entities.entry(name).or_insert(text);
                    k = after;
                }
                // to the `>` past any quoted literal of an external identifier
                loop {
                    match b.get(k) {
                        None => return b.len(),
                        Some(b'>') => break,
                        _ => k = quoted(k).map_or(k + 1, |(_, after)| after),
                    }
                }
                i = k + 1;
            } else if rest.starts_with(b"<!--") {
                i = to(i, b"-->");
            } else if rest.starts_with(b"<?") {
                i = to(i, b"?>");
            } else if rest.starts_with(b"<!ELEMENT") || rest.starts_with(b"<!ATTLIST") || rest.starts_with(b"<!NOTATION") {
                i = to(i, b">");
            } else if rest.starts_with(b"]") {
                return match b.get(spaces(i + 1)) {
                    Some(b'>') => spaces(i + 1) + 1,
                    _ => b.len(),
                };
            } else {
                return b.len();
            }
        }
    }
}

pub fn import_svg(path: &str) -> Result<ImportedSketch, String> {
    let data = std::fs::read(path).map_err(|e| format!("SVG open: {e}"))?;
    // a compressed drawing (.svgz) is scanned as `usvg` reads it - decompressed - and handed over decompressed
    let data = if data.starts_with(&[0x1f, 0x8b]) { usvg::decompress_svgz(&data).map_err(|e| format!("SVG parse: {e}"))? } else { data };
    if deeper_than(&data, SVG_MAX_DEPTH) {
        return Err(format!("io-svg-too-deep#{SVG_MAX_DEPTH}"));
    }
    let opt = usvg::Options::default();
    let tree = usvg::Tree::from_data(&data, &opt).map_err(|e| format!("SVG parse: {e}"))?;
    let h = tree.size().height() as f64;
    let mut curves = Vec::new();
    collect(tree.root(), h, &mut curves);
    Ok(ImportedSketch { curves, ..Default::default() })
}

fn collect(group: &usvg::Group, h: f64, out: &mut Vec<ProfEdge>) {
    for node in group.children() {
        match node {
            usvg::Node::Group(g) => collect(g, h, out),
            usvg::Node::Path(p) => path_to_curves(p, h, out),
            _ => {}
        }
    }
}

/// A segment between two points; degenerate ones are skipped.
fn line(a: Point2, b: Point2, out: &mut Vec<ProfEdge>) {
    if a.dist(b) > 1e-9 {
        out.push(ProfEdge::Line { a, b });
    }
}

fn path_to_curves(p: &usvg::Path, h: f64, out: &mut Vec<ProfEdge>) {
    let t = p.abs_transform();
    let map = |x: f32, y: f32| -> Point2 {
        let mut pt = usvg::tiny_skia_path::Point::from_xy(x, y);
        t.map_point(&mut pt);
        Point2::new(pt.x as f64 * MM_PER_PX, (h - pt.y as f64) * MM_PER_PX)
    };

    let mut start = Point2::new(0.0, 0.0); // the start of the current subpath, used by a close command
    let mut last = Point2::new(0.0, 0.0);

    for seg in p.data().segments() {
        match seg {
            PathSegment::MoveTo(pt) => {
                let q = map(pt.x, pt.y);
                start = q;
                last = q;
            }
            PathSegment::LineTo(pt) => {
                let q = map(pt.x, pt.y);
                line(last, q, out);
                last = q;
            }
            PathSegment::QuadTo(c, pt) => {
                let (c, e) = (map(c.x, c.y), map(pt.x, pt.y));
                let mut prev = last;
                for k in 1..=12 {
                    let tt = k as f64 / 12.0;
                    let q = quad(last, c, e, tt);
                    line(prev, q, out);
                    prev = q;
                }
                last = e;
            }
            PathSegment::CubicTo(c1, c2, pt) => {
                let (c1, c2, e) = (map(c1.x, c1.y), map(c2.x, c2.y), map(pt.x, pt.y));
                let mut prev = last;
                for k in 1..=16 {
                    let tt = k as f64 / 16.0;
                    let q = cubic(last, c1, c2, e, tt);
                    line(prev, q, out);
                    prev = q;
                }
                last = e;
            }
            PathSegment::Close => {
                line(last, start, out);
                last = start;
            }
        }
    }
}

fn lerp(a: Point2, b: Point2, t: f64) -> Point2 {
    Point2::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t)
}

fn quad(p0: Point2, p1: Point2, p2: Point2, t: f64) -> Point2 {
    lerp(lerp(p0, p1, t), lerp(p1, p2, t), t)
}

fn cubic(p0: Point2, p1: Point2, p2: Point2, p3: Point2, t: f64) -> Point2 {
    let a = quad(p0, p1, p2, t);
    let b = quad(p1, p2, p3, t);
    lerp(a, b, t)
}
