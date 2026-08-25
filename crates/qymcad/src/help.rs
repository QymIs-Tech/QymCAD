//! THE HELP: `.md` articles baked into the binary, and their rendering.
//!
//! ONE SOURCE, TWO SHOP WINDOWS. The same files `docs/help/{ru,en}/**.md` go both into this window and
//! onto the site. Two sets of texts about one thing would diverge at the very first edit, and would
//! diverge silently.
//!
//! THE RENDERING IS OURS, NOT A CRATE'S. `egui_commonmark` drags in pulldown-cmark and a way of its own
//! with images, while what is needed is a predictable look: headings, paragraphs, lists, code, emphasis,
//! links, a quote, a rule. Our own parser is a hundred and fifty lines, and in exchange the colours come
//! from THE SCHEME and the fonts are ours; somebody else's renderer would bring its own and drift apart
//! from the light theme exactly as the canvas once did.
use include_dir::{include_dir, Dir};

/// The tree of articles baked into the binary. An edit to a `.md` is picked up by a rebuild (see
/// `build.rs`).
static HELP: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../../docs/help");

/// THE CHOSEN LANGUAGE OF THE HELP. Empty means whatever the interface uses.
///
/// As global state of its own rather than a parameter on every call: the language of the interface
/// already lives exactly that way (`i18n::set_language`), and the article, the title, the contents and
/// the search must all speak ONE language. A second way of setting it would diverge from the first.
static PICKED: std::sync::RwLock<String> = std::sync::RwLock::new(String::new());

/// Set the language of the help (`""` follows the interface). Called from `apply_language`.
/// Which language of the help is chosen right now (empty means "as in the interface").
///
/// ONLY A CHECK READS IT, and that is its lawful business: the language of the help is shared across the
/// process, and a test must put it back, otherwise the neighbouring checks turn red from THE ORDER of
/// the run.
#[cfg(test)]
pub fn picked_lang() -> String {
    PICKED.read().map(|s| s.clone()).unwrap_or_default()
}

pub fn set_lang(code: &str) {
    if let Ok(mut w) = PICKED.write() {
        code.clone_into(&mut w);
    }
}

/// The language of the help: the chosen one; failing that the language of the interface, if there are
/// articles for it; failing that English.
///
/// Falling back to English rather than showing emptiness: an incomplete translation of the help is the
/// norm for an outside contribution, and it has no right to leave anybody without text.
pub fn lang() -> &'static str {
    let picked = PICKED.read().map(|p| p.clone()).unwrap_or_default();
    if !picked.is_empty() {
        if let Some(d) = HELP.get_dir(&picked).and_then(|d| d.path().to_str()) {
            return d;
        }
    }
    let cur = crate::i18n::language();
    if HELP.get_dir(&cur).is_some() {
        // the string lives as long as the directory does: its name is what gets returned
        return HELP.get_dir(&cur).and_then(|d| d.path().to_str()).unwrap_or("en");
    }
    "en"
}

/// The languages the help exists for at all.
///
/// A LANGUAGE IS A FOLDER WITH AN `index.md`, not every folder in the tree. Beside the languages lies
/// `img` — the images are shared across all of them (a drawing is not translated, and a copy per
/// language would double the binary and drift apart at the first redraw). Without this condition `img`
/// would become a "language": it would land in the choice of help language and in the guard that the
/// sets of articles match.
pub fn languages() -> Vec<String> {
    let mut v: Vec<String> = HELP.dirs().filter(|d| d.get_file(d.path().join("index.md")).is_some()).filter_map(|d| d.path().to_str().map(|s| s.to_string())).collect();
    v.sort();
    v
}

/// THE BYTES OF AN IMAGE from the shared folder (`img/...`). Images are not translated, so the language
/// takes no part in the path.
pub fn image(path: &str) -> Option<&'static [u8]> {
    HELP.get_file(path).map(|f| f.contents())
}

/// THE FRAMES OF AN ANIMATION: a path ending in `/` is a folder of frames `00.png`, `01.png` and so on.
///
/// There is no GIF format here and none is needed: `egui` does not decode them, and dragging in a decoder
/// for the sake of the help is the same mistake as somebody else's markdown renderer. The frames are
/// shown as separate images by the renderer itself, and the same files serve the site should a real GIF
/// be wanted there.
pub fn frames(dir: &str) -> Vec<String> {
    let Some(d) = HELP.get_dir(dir.trim_end_matches('/')) else { return Vec::new() };
    let mut v: Vec<String> = d.files().filter(|f| f.path().extension().is_some_and(|x| x == "png")).filter_map(|f| f.path().to_str().map(|s| s.to_string())).collect();
    v.sort(); // the order of the frames is given by their names: 00, 01, 02 and so on
    v
}

/// The text of an article by its path inside a language (`"index"`, `"sketch/01-lines"`).
///
/// A missing article is taken from English: a translation may lag behind, and whoever pressed F1 must
/// get text. If there is none at all, `None`, and the window will say so in words.
pub fn article(path: &str) -> Option<&'static str> {
    let try_lang = |l: &str| HELP.get_file(format!("{l}/{path}.md")).and_then(|f| f.contents_utf8());
    try_lang(lang()).or_else(|| try_lang("en"))
}

/// Every article of a language: paths without the extension, sorted.
pub fn articles(l: &str) -> Vec<String> {
    fn walk(d: &Dir<'_>, out: &mut Vec<String>) {
        for f in d.files() {
            if f.path().extension().is_some_and(|x| x == "md") {
                if let Some(s) = f.path().with_extension("").to_str() {
                    out.push(s.to_string());
                }
            }
        }
        for sub in d.dirs() {
            walk(sub, out);
        }
    }
    let mut out = Vec::new();
    if let Some(d) = HELP.get_dir(l) {
        walk(d, &mut out);
    }
    // the path inside the language: the prefix of the language folder is trimmed off
    let mut v: Vec<String> = out.into_iter().filter_map(|s| s.strip_prefix(&format!("{l}/")).map(|x| x.to_string())).collect();
    v.sort();
    v
}

/// THE TITLE OF AN ARTICLE is its first `# ...` line.
///
/// Taken FROM THE ARTICLE ITSELF rather than from a separate list of names: such a list would have to be
/// kept as a second place of truth and translated apart, and it would diverge from the files at the very
/// first edit. With no heading the path is shown: a silent empty row in the contents is worse than an
/// ugly one.
pub fn title(path: &str) -> String {
    article(path)
        .and_then(|md| md.lines().find_map(|l| l.trim().strip_prefix("# ").map(|t| t.trim().to_string())))
        .unwrap_or_else(|| path.to_string())
}

/// THE TITLE OF AN ARTICLE IN A PARTICULAR LANGUAGE — past the current choice.
///
/// The command search needs it: it searches in the other language too (people remember `fillet` from
/// other manuals), and the name of a command comes from the title of its article. Through the global
/// language this would be done by switching there and back — a race and a mess.
pub fn title_in(lang: &str, path: &str) -> Option<String> {
    let md = HELP.get_file(format!("{lang}/{path}.md")).and_then(|f| f.contents_utf8())?;
    md.lines().find_map(|l| l.trim().strip_prefix("# ").map(|t| t.trim().to_string()))
}

/// THE ADDRESS OF THE SAME ARTICLE ON THE SITE.
///
/// The same path and the same language as in the program, because the source is ONE: the same files
/// `docs/help/{language}/{path}.md` go into the window and onto the site. So the address need be kept as
/// a list nowhere; it is derived from the path of the article, and there is nothing to diverge.
pub fn web_url(article: &str) -> String {
    format!("https://cad.qymis.tech/help/{}/{}", lang(), article)
}

/// WHETHER AN ARTICLE IS VISIBLE in this state of the machining module.
///
/// CAM is an addition that is switched on deliberately, and while the box is unticked none of its innards
/// must be ANYWHERE, the contents of the help and the search over it included. The section is hidden
/// whole by the prefix of the path: a list of articles kept apart would diverge from the files at the
/// very first edit.
pub fn visible(article: &str, cam_on: bool) -> bool {
    cam_on || !article.starts_with("cam/")
}

/// THE CONTENTS: the sections and the articles in them, in the order they are shown.
///
/// Built FROM THE FILES rather than from a list in the code. Then "the article exists and is not in the
/// contents" is an inexpressible state rather than something to watch for: drop in a file and it appears.
/// The order is given by the numeric prefixes in the names (`01-...`), so sorting by name is enough.
///
/// First comes the section with no folder (root articles such as `index`); its name is empty.
pub fn sections(cam_on: bool) -> Vec<(String, Vec<String>)> {
    let mut root: Vec<String> = Vec::new();
    let mut by_dir: std::collections::BTreeMap<String, Vec<String>> = std::collections::BTreeMap::new();
    for a in articles(lang()).into_iter().filter(|a| visible(a, cam_on)) {
        match a.split_once('/') {
            Some((dir, _)) => by_dir.entry(dir.to_string()).or_default().push(a.clone()),
            None => root.push(a.clone()),
        }
    }
    let mut out: Vec<(String, Vec<String>)> = Vec::new();
    if !root.is_empty() {
        out.push((String::new(), root));
    }
    // THE ORDER OF THE SECTIONS GOES BY MEANING, NOT BY ALPHABET.
    //
    // `BTreeMap` handed them out in the alphabetical order of the folder names, which came out exactly
    // opposite to the order they are read in. The lessons stood below the reference on assemblies though
    // they are needed first, and that is what was noticed. The order of learning: lessons -> sketch ->
    // part -> assembly -> machining -> general.
    //
    // An unknown folder does not vanish but goes to the end — and a guard complains about it: a section
    // with no place in the order would appear at the very bottom silently.
    for want in SECTION_ORDER {
        if let Some(mut items) = by_dir.remove(*want) {
            // THE OVERVIEW OF A SECTION COMES FIRST. By the name `index` it landed at the end of the
            // list (sorting by numeric prefixes puts `01-...` earlier), that is, the entry article lay
            // AFTER everything it introduces.
            if let Some(i) = items.iter().position(|a| a.ends_with("/index")) {
                let overview = items.remove(i);
                items.insert(0, overview);
            }
            out.push((want.to_string(), items));
        }
    }
    out.extend(by_dir);
    out
}

/// THE SECTIONS IN READING ORDER. The name of the folder and not the caption: a caption is translated,
/// an order is not.
pub const SECTION_ORDER: &[&str] = &["start", "sketch", "part", "assembly", "cam", "general"];

/// THE SEARCH OVER THE HELP: the paths of the articles the query was met in — in the title or in the
/// text.
///
/// The TEXT is searched too, not the titles alone: what people remember is a word from an article, not
/// its heading. A match in the title ranks higher — it is more precise.
pub fn search(q: &str, cam_on: bool) -> Vec<String> {
    let q = q.trim().to_lowercase();
    if q.is_empty() {
        return Vec::new();
    }
    let (mut head, mut body) = (Vec::new(), Vec::new());
    for a in articles(lang()).into_iter().filter(|a| visible(a, cam_on)) {
        if title(&a).to_lowercase().contains(&q) {
            head.push(a);
        } else if article(&a).is_some_and(|t| t.to_lowercase().contains(&q)) {
            body.push(a);
        }
    }
    head.extend(body);
    head
}

/// A span of a line with its emphasis flags — the result of parsing `**bold**`, `*italic*` and
/// `` `code` ``.
pub struct Span {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub code: bool,
    /// WHERE IT LEADS, if this is a link: the path of an article (`part/08-hole`) or an `https://...`
    /// address.
    ///
    /// A help with no cross-references is a stack of loose sheets. Somebody who has finished reading about
    /// extrusion wants to go straight to the fillet rather than look for it in the contents from
    /// memory.
    pub link: Option<String>,
}

/// A line of an article, parsed into what it is.
pub enum Block {
    Heading(u8, Vec<Span>),
    Para(Vec<Span>),
    Bullet(Vec<Span>),
    Numbered(usize, Vec<Span>),
    Quote(Vec<Span>),
    Code(String),
    Rule,
    /// A TABLE: the header (if it was separated by a `|---|---|` line) and the rows of data.
    ///
    /// The parser understood tables exactly as long as nobody wrote one: the `| ... | ... |` lines were
    /// glued into ONE paragraph through spaces, because there are no empty lines between them. The
    /// article about mates lived that way — seven kinds and what each leaves free read as a solid mush of
    /// bars.
    Table {
        head: Option<Vec<Vec<Span>>>,
        rows: Vec<Vec<Vec<Span>>>,
    },
    /// AN IMAGE or AN ANIMATION (a path ending in `/` is a folder of frames). `alt` is the caption under
    /// it.
    ///
    /// The caption is shown ALWAYS and not only when the image was not found: it explains what to look at,
    /// and that is half the use of an illustration.
    Image {
        path: String,
        alt: Vec<Span>,
    },
}

/// Parse a line of the form `![caption](img/something.png)`.
fn image_line(t: &str) -> Option<(String, String)> {
    let rest = t.strip_prefix("![")?;
    let (alt, rest) = rest.split_once("](")?;
    let path = rest.strip_suffix(')')?;
    if path.is_empty() {
        return None;
    }
    Some((alt.to_string(), path.to_string()))
}

/// The line looks like a row of a table.
fn is_table_line(t: &str) -> bool {
    t.starts_with('|') && t.len() > 1
}

/// The separator line of a header: `|---|:--:|`.
fn is_table_sep(t: &str) -> bool {
    t.trim_matches('|').split('|').all(|c| {
        let c = c.trim();
        !c.is_empty() && c.chars().all(|ch| ch == '-' || ch == ':')
    })
}

/// Assemble the collected lines into a table.
fn table_block(lines: &[String]) -> Block {
    let cells = |l: &str| -> Vec<Vec<Span>> { l.trim().trim_start_matches('|').trim_end_matches('|').split('|').map(|c| spans(c.trim())).collect() };
    let mut head = None;
    let mut rows: Vec<Vec<Vec<Span>>> = Vec::new();
    for l in lines {
        if is_table_sep(l) {
            // the separator makes the previous line THE HEADER; out of place it is simply skipped —
            // drawing bars as data is worse than not drawing them at all
            if head.is_none() && !rows.is_empty() {
                head = Some(rows.remove(0));
            }
            continue;
        }
        rows.push(cells(l));
    }
    Block::Table { head, rows }
}

/// WHAT IS BEING COLLECTED RIGHT NOW — the kind of the unclosed multi-line block.
///
/// Only a paragraph used to be collected, while a list marker was closed by its own line. But the
/// articles are wrapped by width, and the continuation of an item travelled into a SEPARATE paragraph
/// with no indent and no bullet: half the item as an item and the rest as a paragraph beneath it.
#[derive(Clone, Copy)]
enum Pend {
    Para,
    Bullet,
    Numbered(usize),
    Quote,
}

/// Parse an article into blocks. The parsing is deliberately LINE-BASED: the markdown of the articles is
/// headings, paragraphs, lists and code, not arbitrary HTML. Anything more complicated is not written in
/// the articles.
pub fn parse(md: &str) -> Vec<Block> {
    let mut out = Vec::new();
    let mut para: Vec<String> = Vec::new();
    let mut pend = Pend::Para;
    let mut code: Option<String> = None;
    let mut tbl: Vec<String> = Vec::new();
    /// Close what has been collected with the block it began as.
    fn close(para: &mut Vec<String>, pend: &mut Pend, out: &mut Vec<Block>) {
        if !para.is_empty() {
            let s = spans(&para.join(" "));
            out.push(match *pend {
                Pend::Para => Block::Para(s),
                Pend::Bullet => Block::Bullet(s),
                Pend::Numbered(n) => Block::Numbered(n, s),
                Pend::Quote => Block::Quote(s),
            });
            para.clear();
        }
        *pend = Pend::Para;
    }
    let flush = close;
    let flush_tbl = |tbl: &mut Vec<String>, out: &mut Vec<Block>| {
        if !tbl.is_empty() {
            out.push(table_block(tbl));
            tbl.clear();
        }
    };
    for line in md.lines() {
        // a code block ```
        if let Some(buf) = code.as_mut() {
            if line.trim_start().starts_with("```") {
                out.push(Block::Code(std::mem::take(buf)));
                code = None;
            } else {
                buf.push_str(line);
                buf.push('\n');
            }
            continue;
        }
        if line.trim_start().starts_with("```") {
            flush(&mut para, &mut pend, &mut out);
            flush_tbl(&mut tbl, &mut out);
            code = Some(String::new());
            continue;
        }
        let t = line.trim();
        // A TABLE IS COLLECTED WHOLE: its lines run one after another with no empty ones between them,
        // and they cannot be parsed one by one — the header is separated from the data by THE NEXT
        // line.
        if is_table_line(t) {
            flush(&mut para, &mut pend, &mut out);
            tbl.push(t.to_string());
            continue;
        }
        flush_tbl(&mut tbl, &mut out);
        if t.is_empty() {
            flush(&mut para, &mut pend, &mut out);
            continue;
        }
        // AN IMAGE GOES ON A LINE OF ITS OWN rather than inside a paragraph. The parsing is line-based
        // and this is its rule: what stands between paragraphs reads more easily than what is squeezed
        // into them.
        if let Some((alt, path)) = image_line(t) {
            flush(&mut para, &mut pend, &mut out);
            out.push(Block::Image { path, alt: spans(&alt) });
            continue;
        }
        if t.starts_with("---") && t.chars().all(|c| c == '-') {
            flush(&mut para, &mut pend, &mut out);
            out.push(Block::Rule);
            continue;
        }
        if let Some(rest) = t.strip_prefix("> ") {
            // A QUOTE OF SEVERAL LINES IS ONE quote, unlike two list items in a row: there each bar
            // starts an item of its own, here each continues the same inset.
            if !matches!(pend, Pend::Quote) {
                flush(&mut para, &mut pend, &mut out);
                pend = Pend::Quote;
            }
            para.push(rest.to_string());
            continue;
        }
        let hashes = t.chars().take_while(|c| *c == '#').count();
        if hashes > 0 && t.chars().nth(hashes) == Some(' ') {
            flush(&mut para, &mut pend, &mut out);
            out.push(Block::Heading(hashes.min(4) as u8, spans(t[hashes + 1..].trim())));
            continue;
        }
        if let Some(rest) = t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")) {
            flush(&mut para, &mut pend, &mut out);
            pend = Pend::Bullet;
            para.push(rest.to_string());
            continue;
        }
        if let Some((num, rest)) = t.split_once(". ") {
            if let Ok(n) = num.parse::<usize>() {
                flush(&mut para, &mut pend, &mut out);
                pend = Pend::Numbered(n);
                para.push(rest.to_string());
                continue;
            }
        }
        // THE CONTINUATION OF WHAT WAS BEGUN, whatever it was: a paragraph, a list item, a quote. Line
        // breaks inside a block carry no meaning — the articles are wrapped by the width of the text
        // rather than by sense.
        para.push(t.to_string());
    }
    flush(&mut para, &mut pend, &mut out);
    flush_tbl(&mut tbl, &mut out); // a table at the very end of an article is a table too
    if let Some(buf) = code {
        out.push(Block::Code(buf)); // an unclosed code block is shown anyway rather than swallowed
    }
    out
}

/// Parse a link starting at position `i`: `[text](target)` gives (the text, the target, how many
/// characters).
///
/// As a function of its own rather than a line inside the parser: there must be no nested brackets in the
/// text of a link, and saying so explicitly is easier than later chasing an article that had half a line
/// eaten.
fn link_at(ch: &[char], i: usize) -> Option<(String, String, usize)> {
    let close = ch[i + 1..].iter().position(|c| *c == ']')? + i + 1;
    if ch.get(close + 1) != Some(&'(') {
        return None;
    }
    let end = ch[close + 2..].iter().position(|c| *c == ')')? + close + 2;
    let text: String = ch[i + 1..close].iter().collect();
    let target: String = ch[close + 2..end].iter().collect();
    if text.is_empty() || target.is_empty() || text.contains('[') {
        return None;
    }
    Some((text, target, end + 1 - i))
}

/// Parse the emphasis inside a line: `**bold**`, `*italic*`, `` `code` ``, `[a link](target)`.
pub fn spans(s: &str) -> Vec<Span> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let (mut bold, mut italic, mut code) = (false, false, false);
    let ch: Vec<char> = s.chars().collect();
    let mut i = 0;
    let push = |cur: &mut String, out: &mut Vec<Span>, b: bool, it: bool, c: bool| {
        if !cur.is_empty() {
            out.push(Span { text: std::mem::take(cur), bold: b, italic: it, code: c, link: None });
        }
    };
    while i < ch.len() {
        // A LINK `[text](target)` only OUTSIDE code: in articles about formulas square brackets occur
        // inside `code` as they stand, and turning them into a link is not allowed.
        if !code && ch[i] == '[' {
            if let Some((text, target, len)) = link_at(&ch, i) {
                push(&mut cur, &mut out, bold, italic, code);
                out.push(Span { text, bold, italic, code: false, link: Some(target) });
                i += len;
                continue;
            }
        }
        // code is not parsed inside: `**` within code is asterisks, not emphasis
        if ch[i] == '`' {
            push(&mut cur, &mut out, bold, italic, code);
            code = !code;
            i += 1;
            continue;
        }
        if !code && ch[i] == '*' {
            if i + 1 < ch.len() && ch[i + 1] == '*' {
                push(&mut cur, &mut out, bold, italic, code);
                bold = !bold;
                i += 2;
                continue;
            }
            push(&mut cur, &mut out, bold, italic, code);
            italic = !italic;
            i += 1;
            continue;
        }
        cur.push(ch[i]);
        i += 1;
    }
    push(&mut cur, &mut out, bold, italic, code);
    out
}

/// THE LANGUAGE OF THE HELP IS SHARED ACROSS THE PROCESS, AND IN A TEST RUN THAT IS A RACE.
///
/// Tests change it back and forth while their neighbours read the address of the help — and, landing in
/// somebody else's window, compare `/ru/...` with `/en/...`. The gate flickered about once every five
/// full runs: no defect and a red build, and a guard like that is worth nothing. So both those who CHANGE
/// the language and those who READ it take this lock.
#[cfg(test)]
pub(crate) fn lang_guard() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}
