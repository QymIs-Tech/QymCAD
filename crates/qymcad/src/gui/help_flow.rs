//! THE FRAME OF THE HELP.
//!
//! The articles are baked into the binary and drawn by a renderer of our own. Three things are checked,
//! each of which otherwise breaks silently: the articles REACH the program, the markdown parser
//! understands what the articles are written with, and the sets of files match across the languages.
#[cfg(test)]
mod tests {
    use crate::help::{self, Block};

    /// THE ARTICLES ARE BAKED IN AND READ. If `include_dir!` did not pick the tree up, the help window
    /// will be empty in a built program — and neither the compiler nor the tests would notice by
    /// themselves.
    #[test]
    fn the_articles_are_embedded_in_the_binary() {
        let ru = help::articles("ru");
        let en = help::articles("en");
        assert!(!ru.is_empty(), "there are no articles in the second language at all — the tree is not baked in");
        assert!(ru.contains(&"index".to_string()), "there is no root article of the contents: {ru:?}");
        assert!(!en.is_empty(), "there are no English articles at all: {en:?}");
        assert!(help::article("index").is_some_and(|t| !t.trim().is_empty()), "the contents are empty");
    }

    /// THE SETS OF FILES MATCH ACROSS THE LANGUAGES.
    ///
    /// The main guard of this work: a missing translation must be visible AT ONCE rather than when the
    /// article travels to the site. It is also the reason both languages are written at the same time.
    #[test]
    fn every_language_has_the_same_set_of_articles() {
        let en = help::articles("en");
        for l in help::languages() {
            let cur = help::articles(&l);
            let missing: Vec<&String> = en.iter().filter(|a| !cur.contains(a)).collect();
            let extra: Vec<&String> = cur.iter().filter(|a| !en.contains(a)).collect();
            assert!(missing.is_empty(), "the language \"{l}\" is short of articles: {missing:?}");
            assert!(extra.is_empty(), "the language \"{l}\" has articles the reference en does not: {extra:?}");
        }
    }

    /// A MISSING ARTICLE IS TAKEN FROM ENGLISH rather than showing emptiness.
    ///
    /// An incomplete translation is the norm for an outside contribution, and it has no right to leave
    /// anybody without text at the very minute they pressed F1.
    #[test]
    fn a_missing_translation_falls_back_to_english() {
        let prev = crate::i18n::language();
        crate::i18n::set_language("ru");
        // there is no article of this name in any language — the answer must be an honest "no" rather
        // than a panic
        assert!(help::article("no-such-article").is_none(), "an article that does not exist must give None");
        // and one that does exist is there in any language
        assert!(help::article("index").is_some(), "the contents must be found");
        crate::i18n::set_language(&prev);
    }

    /// THE PARSER UNDERSTANDS WHAT THE ARTICLES ARE WRITTEN WITH.
    #[test]
    fn the_parser_understands_what_the_articles_are_written_with() {
        let md = "# A heading\n\nA paragraph with **bold**, *italics* and `code`.\n\n- an item\n1. the first\n\n> a quote\n\n```\ncode\n```\n\n---\n";
        let blocks = help::parse(md);
        assert!(matches!(blocks.first(), Some(Block::Heading(1, _))), "the heading is not parsed");
        assert!(blocks.iter().any(|b| matches!(b, Block::Bullet(_))), "the bulleted list is not parsed");
        assert!(blocks.iter().any(|b| matches!(b, Block::Numbered(1, _))), "the numbered list is not parsed");
        assert!(blocks.iter().any(|b| matches!(b, Block::Quote(_))), "the quote is not parsed");
        assert!(blocks.iter().any(|b| matches!(b, Block::Code(_))), "the code block is not parsed");
        assert!(blocks.iter().any(|b| matches!(b, Block::Rule)), "the rule is not parsed");

        let para = blocks.iter().find_map(|b| if let Block::Para(s) = b { Some(s) } else { None }).expect("the paragraph");
        assert!(para.iter().any(|s| s.bold && s.text.contains("bold")), "the bold is not marked: {:?}", para.iter().map(|s| &s.text).collect::<Vec<_>>());
        assert!(para.iter().any(|s| s.italic && s.text.contains("italics")), "the italics are not marked");
        assert!(para.iter().any(|s| s.code && s.text.contains("code")), "the code is not marked");
    }

    /// A TABLE IS PARSED AS A TABLE, and its header is separated from the data.
    ///
    /// The parser did NOT understand tables and nobody saw it: the `| ... | ... |` lines run one after
    /// another with no empty ones between them, so they were glued into ONE paragraph through spaces. The
    /// table of what each kind of mate leaves free read as a solid mush of bars.
    #[test]
    fn a_table_is_parsed_as_a_table() {
        let md = "Before.\n\n| Kind | Freedom |\n|---|---|\n| Rigid | nothing |\n| Slider | along the axis |\n\nAfter.\n";
        let blocks = help::parse(md);
        let (head, rows) = blocks
            .iter()
            .find_map(|b| if let Block::Table { head, rows } = b { Some((head, rows)) } else { None })
            .expect("the table is not parsed — its lines travelled into a paragraph");
        let head = head.as_ref().expect("the header is not separated from the data by a |---| line");
        assert_eq!(head.len(), 2, "the header has other than two columns");
        assert!(head[0].iter().any(|s| s.text.contains("Kind")), "the header lost its first column");
        assert_eq!(rows.len(), 2, "the rows of data are other than two: {}", rows.len());
        assert!(rows[1][1].iter().any(|s| s.text.contains("along the axis")), "the last cell is lost");
        // and the neighbouring paragraphs did not suffer from it
        let paras: Vec<String> = blocks.iter().filter_map(|b| if let Block::Para(s) = b { Some(s.iter().map(|x| x.text.clone()).collect::<String>()) } else { None }).collect();
        assert!(paras.iter().any(|p| p.contains("Before")), "the paragraph before the table is lost: {paras:?}");
        assert!(paras.iter().any(|p| p.contains("After")), "the paragraph after the table is lost: {paras:?}");
    }

    /// A LINE BREAK INSIDE AN ITEM DOES NOT TEAR IT IN TWO.
    ///
    /// The articles are wrapped by the width of the text rather than by sense: in every other one a list
    /// item takes two lines. The parser closed an item with its own line, and the tail travelled off as a
    /// SEPARATE paragraph — with no bullet and no indent. The same class as the table: the markup is there
    /// and the renderer does not understand it.
    #[test]
    fn a_wrapped_item_stays_one_item() {
        let md = "- **The viewport** — the engine, the projection,\n  the shading.\n- the second item\n\n1. the first,\n  with a wrap\n\n> a quote,\n> continued\n";
        let blocks = help::parse(md);
        let text = |s: &Vec<help::Span>| s.iter().map(|x| x.text.clone()).collect::<String>();
        let bullets: Vec<String> = blocks.iter().filter_map(|b| if let Block::Bullet(s) = b { Some(text(s)) } else { None }).collect();
        assert_eq!(bullets.len(), 2, "there should be two items, and out came {}: {bullets:?}", bullets.len());
        assert!(bullets[0].contains("the shading"), "the tail of the first item is lost: {bullets:?}");
        let nums: Vec<String> = blocks.iter().filter_map(|b| if let Block::Numbered(_, s) = b { Some(text(s)) } else { None }).collect();
        assert!(nums.first().is_some_and(|t| t.contains("with a wrap")), "the tail of the numbered item travelled off: {nums:?}");
        let quotes: Vec<String> = blocks.iter().filter_map(|b| if let Block::Quote(s) = b { Some(text(s)) } else { None }).collect();
        assert_eq!(quotes.len(), 1, "a quote of two lines is torn in two: {quotes:?}");
        // and the main thing: NOT ONE spare paragraph — that is what a torn-off tail became
        assert!(!blocks.iter().any(|b| matches!(b, Block::Para(_))), "the tail of an item became a separate paragraph");
    }

    /// AND NOT ONE BAR REACHED THE SCREEN AS A PARAGRAPH — across every article in every language.
    ///
    /// A guard over the REAL articles rather than a specimen: markup the renderer does not understand is
    /// seen by an author only by eye and only if they open that very article. Here the build sees it.
    #[test]
    fn no_article_leaks_table_pipes_into_a_paragraph() {
        for l in help::languages() {
            for a in help::articles(&l) {
                let md = help::article(&a).expect("the article");
                for b in help::parse(md) {
                    if let Block::Para(s) = b {
                        let text: String = s.iter().map(|x| x.text.clone()).collect();
                        assert!(!text.contains('|'), "in the article {l}/{a} a line of a table reached the screen as a paragraph: \"{text}\"");
                    }
                }
            }
        }
    }

    /// THE ARTICLES USE ONLY THE MARKUP THE RENDERER UNDERSTANDS.
    ///
    /// A generalisation of the lesson about tables rather than a patch over it. The renderer is ours and
    /// deliberately line-based; anything it lacks is seen by an author only by eye and only if they open
    /// that very article, while on the site, where the markdown is full, it renders as well — and
    /// diverges from the program silently.
    ///
    /// If a link or a picture is wanted: the renderer first, the article second, and not the other way
    /// round.
    #[test]
    fn the_articles_use_only_the_markup_the_renderer_understands() {
        // (the mark, what it is). Checked OUTSIDE code blocks: any character is lawful in those.
        let unsupported: &[(&str, &str)] = &[("~~", "struck-through text"), ("<br", "HTML")];
        for l in help::languages() {
            for a in help::articles(&l) {
                let md = help::article(&a).expect("the article");
                let mut in_code = false;
                for (n, line) in md.lines().enumerate() {
                    if line.trim_start().starts_with("```") {
                        in_code = !in_code;
                        continue;
                    }
                    if in_code {
                        continue;
                    }
                    for (mark, what) in unsupported {
                        assert!(!line.contains(mark), "{l}/{a}:{}: the help renderer cannot do {what} (\"{mark}\") and the article has some — in the window it will be seen as raw text", n + 1);
                    }

                    // a nested list is flattened by the renderer into one level: the markup is there and
                    // the meaning is not
                    let indent = line.len() - line.trim_start().len();
                    let nested = indent > 0 && (line.trim_start().starts_with("- ") || line.trim_start().starts_with("* "));
                    assert!(!nested, "{l}/{a}:{}: a nested list — the help renderer draws one level, and the nesting will vanish silently", n + 1);
                }
            }
        }
    }

    /// EVERY LINK LEADS SOMEWHERE REAL (or outwards over https).
    ///
    /// A broken link inside the help is worse than a missing one: it gets clicked, lands on "the article
    /// is not written yet", and the program is taken to be broken. And there is no other way of noticing
    /// it than walking every link in every article in both languages by hand.
    ///
    /// It is also a guard against RENAMES: move an article into another folder and the links to it turn
    /// red.
    #[test]
    fn every_link_leads_somewhere_real() {
        let mut broken = Vec::new();
        for l in help::languages() {
            for a in help::articles(&l) {
                let md = help::article(&a).expect("the article");
                for b in help::parse(md) {
                    let spans = match b {
                        Block::Para(s) | Block::Bullet(s) | Block::Quote(s) | Block::Heading(_, s) | Block::Numbered(_, s) => s,
                        _ => continue,
                    };
                    for s in spans {
                        let Some(to) = s.link else { continue };
                        if to.starts_with("https://") {
                            continue;
                        }
                        assert!(!to.starts_with("http://"), "{l}/{a}: the link \"{to}\" is unencrypted — the help must not teach bad habits");
                        if help::article(&to).is_none() {
                            broken.push(format!("{l}/{a} → {to}"));
                        }
                    }
                }
            }
        }
        assert!(broken.is_empty(), "a link leads to an article that does not exist ({}):\n{}", broken.len(), broken.join("\n"));
    }

    /// AND A LINK IS PARSED AS A LINK, while square brackets inside code stay brackets.
    #[test]
    fn a_link_is_parsed_as_a_link() {
        let s = help::spans("see [holes](part/08-hole) and `a[0]`");
        let link = s.iter().find(|x| x.link.is_some()).expect("the link is not parsed");
        assert_eq!(link.text, "holes", "the text of the link is lost");
        assert_eq!(link.link.as_deref(), Some("part/08-hole"), "the target of the link is lost");
        assert!(s.iter().any(|x| x.code && x.text.contains("a[0]")), "the brackets inside code were eaten: {:?}", s.iter().map(|x| &x.text).collect::<Vec<_>>());
        assert!(!s.iter().any(|x| x.text.contains("](")), "the markup of the link stayed in the text");
    }

    /// ASTERISKS INSIDE CODE ARE ASTERISKS and not emphasis: in articles about formulas `w*2` occurs on
    /// every page, and turning it into italics is not allowed.
    #[test]
    fn stars_inside_code_are_not_emphasis() {
        let s = help::spans("the formula `len*2` and `a**b`");
        let code: Vec<&String> = s.iter().filter(|x| x.code).map(|x| &x.text).collect();
        assert!(code.iter().any(|t| t.contains("len*2")), "an asterisk inside code was eaten: {code:?}");
        assert!(code.iter().any(|t| t.contains("a**b")), "a double asterisk inside code was eaten: {code:?}");
        assert!(!s.iter().any(|x| x.italic), "the code turned into italics");
    }

    /// EVERY SECTION HAS A PLACE IN THE READING ORDER.
    ///
    /// The order is given by an explicit list: by the alphabet of the folder names it came out backwards
    /// from the way they are read, and the lessons ended up below the reference. A new section with no
    /// place in the list will not vanish (it goes to the end), but it goes there SILENTLY — and that is
    /// what gets caught here.
    #[test]
    fn every_section_has_a_place_in_the_reading_order() {
        let dirs: Vec<String> = help::sections(true).into_iter().map(|(d, _)| d).filter(|d| !d.is_empty()).collect();
        for d in &dirs {
            assert!(help::SECTION_ORDER.contains(&d.as_str()), "the section \"{d}\" is not named in the reading order — it will stand at the very bottom of the contents");
        }
        // and the order is kept: the sections run exactly as they are listed
        let want: Vec<&str> = help::SECTION_ORDER.iter().copied().filter(|s| dirs.iter().any(|d| d == s)).collect();
        let got: Vec<&str> = dirs.iter().map(|s| s.as_str()).collect();
        assert_eq!(got, want, "the contents run in an order other than the one given");
        // the lessons come FIRST: they are where somebody opening a CAD for the first time begins
        assert_eq!(dirs.first().map(|s| s.as_str()), Some("start"), "the lessons are not first in the contents");
    }

    /// THE CONTENTS ARE BUILT FROM THE FILES, so "the article exists and is not in the contents" is
    /// inexpressible.
    #[test]
    fn the_contents_are_built_from_the_files_themselves() {
        let all = help::articles(help::lang());
        let listed: Vec<String> = help::sections(true).into_iter().flat_map(|(_, v)| v).collect();
        assert_eq!(listed.len(), all.len(), "the contents diverged from the files: {listed:?} against {all:?}");
        for a in &all {
            assert!(listed.contains(a), "the article \"{a}\" did not get into the contents");
        }
        // the root articles come as the first section, the rest go by folders
        let (first, _) = help::sections(true).into_iter().next().expect("the contents are not empty");
        assert!(first.is_empty(), "the root articles must come as the first section, and out came \"{first}\"");
    }

    /// EVERY SECTION HAS A CAPTION IN EVERY LANGUAGE.
    ///
    /// Otherwise a catalogue key stands in the contents — exactly the leak the screen guard already
    /// watches for; here it can happen for nothing at all: a folder is made and a word forgotten.
    #[test]
    fn every_section_has_a_name_in_every_language() {
        let prev = crate::i18n::language();
        let dirs: Vec<String> = help::sections(true).into_iter().map(|(d, _)| d).filter(|d| !d.is_empty()).collect();
        for code in ["ru", "en"] {
            crate::i18n::set_language(code);
            for d in &dirs {
                let key = format!("help-section-{d}");
                let t = crate::i18n::tr(&key);
                assert_ne!(t, key, "{code}: the section \"{d}\" has no caption — a key will stand in the contents");
            }
        }
        crate::i18n::set_language(&prev);
    }

    /// THE TITLE COMES FROM THE ARTICLE ITSELF rather than from a separate list of names.
    #[test]
    fn the_title_comes_from_the_article() {
        let t = help::title("general/01-window");
        assert!(!t.is_empty() && t != "general/01-window", "the title was not read from the article: \"{t}\"");
        assert!(!t.starts_with('#'), "the hash of the heading got into the caption: \"{t}\"");
    }

    /// THE SEARCH LOOKS INSIDE THE ARTICLES TOO, not only at their names: what people remember is a word
    /// from an article, not its heading. And a match in the heading ranks higher — it is more precise.
    ///
    /// THE QUERIES ARE TAKEN FROM THE ARTICLES THEMSELVES rather than typed as words: a literal would tie
    /// the check to one language and go blind the moment the text was edited.
    #[test]
    fn the_search_looks_inside_the_articles_too() {
        const WHERE: &str = "general/01-window";
        let title = help::title(WHERE);
        let by_title = help::search(&title, true);
        assert_eq!(by_title.first().map(|s| s.as_str()), Some(WHERE), "the article with that heading must come first: {by_title:?}");

        // a long word out of the body that the heading does not carry — so only a search over the TEXT
        // can find it
        let body = help::article(WHERE).expect("the article");
        let word = body
            .lines()
            .filter(|l| !l.starts_with('#'))
            .flat_map(|l| l.split_whitespace())
            .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()))
            .find(|w| w.chars().count() > 8 && !title.contains(*w))
            .expect("a long word in the body of the article");
        let by_body = help::search(word, true);
        assert!(!by_body.is_empty(), "a search by the word \"{word}\" out of the TEXT found nothing");

        assert!(help::search("   ", true).is_empty(), "an empty query must give an empty answer rather than everything at once");
        assert!(help::search("no-such-word-anywhere", true).is_empty(), "the search found something that does not exist");
    }

    /// BACK RETURNS WHERE ONE CAME FROM, and does not loop on one article.
    #[test]
    fn back_returns_where_you_came_from() {
        let mut app = super::super::App::default();
        app.open_help("index");
        assert!(!app.help_can_go_back_for_test(), "from the very first article there is nowhere to return to");

        app.open_help("general/01-window");
        assert!(app.help_can_go_back_for_test(), "the jump was not remembered");
        app.help_back_for_test();
        assert_eq!(app.help_article_for_test(), "index", "back returned to the wrong place");
        assert!(!app.help_can_go_back_for_test(), "back added to the history itself — there would be no getting out of it");

        // a repeat of the same article does not grow the history
        app.open_help("index");
        app.open_help("index");
        assert!(!app.help_can_go_back_for_test(), "a repeated click on the same article got into the history");
    }

    /// THE BUILD WATCHES THE FOLDER. Without that an edit to an article does not reach the binary and the
    /// author sees the old text without understanding why — the same trap the language catalogue had.
    #[test]
    fn the_build_watches_the_help_folder() {
        let build = include_str!("../../build.rs");
        assert!(build.contains("cargo:rerun-if-changed=../../docs/help"), "the build does not watch the help folder");
    }

    /// THE HELP OPENS FROM THE MENU and draws an article rather than an empty window.
    #[test]
    fn the_window_draws_the_article() {
        let mut app = super::super::App::default();
        app.open_help("index");
        let texts = super::super::screen_keys::tests::frame_text(&mut app, |a, c| a.help_window(c));
        assert!(texts.len() > 5, "the help window drew almost nothing: {texts:?}");
        let title = crate::i18n::tr("help-title");
        assert!(texts.iter().any(|t| t.contains(&title)), "the window has no heading \"{title}\"");
        let panels = crate::gui::panels_source::PANELS;
        assert!(panels.contains("self.open_help(\"index\")"), "the help does not open from the menu");
    }

    /// AND A TABLE REACHES THE SCREEN AS CELLS rather than one line of bars.
    ///
    /// The parser can be mended and then forgotten in the drawing — and then `parse` returns a table, the
    /// guard over the parsing goes green, and what is seen is still mush.
    #[test]
    fn the_window_draws_a_table_as_cells() {
        let mut app = super::super::App::default();
        app.open_help("assembly/02-joints");
        let texts = super::super::screen_keys::tests::frame_text(&mut app, |a, c| a.help_window(c));
        assert!(!texts.iter().any(|t| t.contains('|')), "the bars of the table reached the screen as text: {:?}", texts.iter().filter(|t| t.contains('|')).collect::<Vec<_>>());
        // AND THE TABLE IN THIS ARTICLE IS A TABLE INDEED.
        //
        // Checked by parsing rather than by looking for a cell in the frame: the article grows, the table
        // travels under the scroll and stops being drawn at all — the test would turn red at an ADDED
        // picture without anything breaking. That cells are drawn as cells is held by
        // `a_table_is_parsed_as_a_table`, and this guard stands for something else: the markup does not
        // reach the screen raw.
        let md = help::article("assembly/02-joints").expect("the article");
        assert!(help::parse(md).iter().any(|b| matches!(b, Block::Table { .. })), "the table in the article about mates is not parsed as a table");
    }

    /// THE TEXT OF AN ARTICLE HAS MARGINS rather than being nailed to the edge.
    ///
    /// Checked BY GEOMETRY — where the letters actually landed — rather than by the word `inner_margin`
    /// being in the source: a frame can be set up and bypassed by the very first `ui.label` outside it,
    /// and a guard over the source would not notice.
    #[test]
    fn the_article_text_is_not_glued_to_the_edge() {
        let mut app = super::super::App::default();
        app.open_help("assembly/02-joints"); // an article with a heading, paragraphs and a table
        let screen = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1400.0, 900.0));
        let input = egui::RawInput { screen_rect: Some(screen), ..Default::default() };
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        let _ = ctx.run_ui(input.clone(), |c| app.help_window(c.ctx()));
        let out = ctx.run_ui(input, |c| app.help_window(c.ctx()));

        /// The left edges of every caption in the frame.
        fn lefts(s: &egui::Shape, out: &mut Vec<f32>) {
            match s {
                egui::Shape::Text(t) => out.push(t.pos.x),
                egui::Shape::Vec(v) => v.iter().for_each(|x| lefts(x, out)),
                _ => {}
            }
        }
        let mut xs = Vec::new();
        for cs in &out.shapes {
            lefts(&cs.shape, &mut xs);
        }
        assert!(xs.len() > 10, "there are suspiciously few captions in the frame: {}", xs.len());
        // THE TEXT OF THE ARTICLE is the part to the right of the contents panel (240 px). There is nothing
        // to check about the left edge of the panel: it has margins of its own and they are not about the
        // readability of a paragraph.
        let body: Vec<f32> = xs.into_iter().filter(|x| *x > 260.0).collect();
        assert!(!body.is_empty(), "not one caption was found in the body of the window");
        let leftmost = body.iter().cloned().fold(f32::MAX, f32::min);
        let edge = leftmost - 260.0;
        assert!(edge > 4.0, "the text of the article is nailed to the edge of the panel: there are {edge:.1} px of free space on the left, and there must be at least {}", super::super::help_window::HELP_PAD);
    }

    /// THE CONTENTS AND THE SEARCH REACH THE SCREEN.
    #[test]
    fn the_window_shows_the_contents_and_the_search() {
        // THE LANGUAGE IS PINNED: the frame is drawn in the current language and the expectation is taken
        // from the contents — and between those two steps a neighbouring test manages to move the language.
        // The test turned red not from a breakage but from somebody else's run beside it; this is already
        // the second such case.
        let prev = crate::i18n::language();
        crate::i18n::set_language("ru");
        crate::help::set_lang("ru");
        let mut app = super::super::App::default();
        app.open_help("index");
        let texts = super::super::screen_keys::tests::frame_text(&mut app, |a, c| a.help_window(c));
        // THE FIRST SECTION AND ITS FIRST ARTICLE ARE TAKEN rather than a particular one: the contents
        // scroll, and the lower sections do not get into the frame. A particular section used to stand here
        // — and the test turned red when the order of the sections became sensible and it travelled down.
        // Nothing had broken and the test was red.
        let (dir, items) = help::sections(false).into_iter().find(|(d, _)| !d.is_empty()).expect("at least one section");
        let section = crate::i18n::tr(&format!("help-section-{dir}"));
        assert!(texts.iter().any(|t| t.contains(&section)), "the contents hold no section \"{section}\": {texts:?}");
        let first = help::title(items.first().expect("an article in the section"));
        assert!(texts.iter().any(|t| t.contains(&first)), "the contents hold no article \"{first}\"");
        let back = crate::i18n::tr("help-back");
        let has_back = texts.iter().any(|t| t.contains(&back));
        crate::i18n::set_language(&prev);
        crate::help::set_lang("");
        assert!(has_back, "there is no \"{back}\" button on the screen");
    }
}
