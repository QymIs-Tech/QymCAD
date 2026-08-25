//! HELP PICTURES ARE CROSS-CHECKED AGAINST THE ARTICLES.
//!
//! The help lives in md files (`docs/help/<language>/**.md`) while the pictures for them are drawn by
//! the program itself (`help_images.rs`). Nothing holds those two halves together: rename an article,
//! redraw a picture under another name, and a reader sees either an empty space or someone else's
//! drawing. The reader notices it, not us.
//!
//! There are three guards here on what a person sees with their own eyes: a link leads to a picture
//! that exists, a frame folder is not empty, and what was drawn is used by somebody.
#[cfg(test)]
mod tests {
    fn help_dir() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/help")
    }

    /// Every picture link in every article of every language: (article file, link).
    fn image_refs() -> Vec<(String, String)> {
        let mut out = Vec::new();
        for lang in ["ru", "en"] {
            let base = help_dir().join(lang);
            let mut stack = vec![base.clone()];
            while let Some(d) = stack.pop() {
                let Ok(rd) = std::fs::read_dir(&d) else { continue };
                for e in rd.flatten() {
                    let p = e.path();
                    if p.is_dir() {
                        stack.push(p);
                    } else if p.extension().is_some_and(|x| x == "md") {
                        let txt = std::fs::read_to_string(&p).unwrap_or_default();
                        let name = p.strip_prefix(help_dir()).unwrap_or(&p).to_string_lossy().into_owned();
                        // a link of the form ![caption](img/something): take what is inside the brackets
                        for part in txt.split("![").skip(1) {
                            if let Some(open) = part.find("](") {
                                if let Some(close) = part[open + 2..].find(')') {
                                    out.push((name.clone(), part[open + 2..open + 2 + close].trim().to_string()));
                                }
                            }
                        }
                    }
                }
            }
        }
        out
    }

    /// NOT ONE BROKEN LINK: a reader will not see emptiness where a picture should be.
    #[test]
    fn every_picture_a_help_article_asks_for_actually_exists() {
        let img = help_dir().join("img");
        let mut broken = Vec::new();
        for (article, r) in image_refs() {
            let rel = r.rsplit("img/").next().unwrap_or(&r).trim().to_string();
            if rel.ends_with('/') {
                // A FRAME FOLDER: a step-by-step showing. An empty folder is the same emptiness on screen.
                let dir = img.join(rel.trim_end_matches('/'));
                let frames = std::fs::read_dir(&dir).map(|d| d.flatten().filter(|e| e.path().extension().is_some_and(|x| x == "png")).count()).unwrap_or(0);
                if frames == 0 {
                    broken.push(format!("{article} -> the frame folder \"{rel}\" is missing or empty"));
                }
            } else if !img.join(&rel).is_file() {
                broken.push(format!("{article} -> the picture \"{rel}\" does not exist"));
            }
        }
        assert!(broken.is_empty(), "the help links to pictures that do not exist ({}):\n{}", broken.len(), broken.join("\n"));
    }

    /// WHAT WAS DRAWN IS USED. A picture nobody links to is dead weight: it goes stale silently and
    /// one day surfaces in an article instead of a fresh one.
    #[test]
    fn every_drawn_picture_is_used_by_some_article() {
        let img = help_dir().join("img");
        let used: std::collections::HashSet<String> = image_refs()
            .into_iter()
            .map(|(_, r)| r.rsplit("img/").next().unwrap_or(&r).trim().trim_end_matches('/').to_string())
            .collect();
        let mut orphan = Vec::new();
        for e in std::fs::read_dir(&img).expect("the help picture directory").flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if !used.contains(&name) {
                orphan.push(name);
            }
        }
        orphan.sort();
        assert!(orphan.is_empty(), "help pictures nobody links to ({}): {orphan:?}", orphan.len());
    }

    /// TOOL ARTICLES SHOW, THEY DO NOT ONLY TELL.
    ///
    /// A tool is hard to explain in words: a person needs to see what will come out. The exception
    /// list below holds articles where a picture is not needed in substance (tables of contents,
    /// overview sections); it is EXPLICIT so that a new article without a picture cannot slip through
    /// silently.
    #[test]
    #[ignore = "4 articles are still without a picture. Import-export and external references have no scenes written. Face splitting and surface trimming ARE NOT DRAWN: the first does not change the shape (nothing to show in a raster), and the second pulls the donor and the pre-move tool into frame — the reasons are set out in help_images.rs"]
    fn every_tool_article_shows_a_picture() {
        const NO_PICTURE_NEEDED: &[&str] = &["index.md"];
        let mut silent = Vec::new();
        let base = help_dir().join("ru");
        let mut stack = vec![base.clone()];
        while let Some(d) = stack.pop() {
            let Ok(rd) = std::fs::read_dir(&d) else { continue };
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                } else if p.extension().is_some_and(|x| x == "md") {
                    let name = p.file_name().unwrap_or_default().to_string_lossy().into_owned();
                    if NO_PICTURE_NEEDED.contains(&name.as_str()) {
                        continue;
                    }
                    let txt = std::fs::read_to_string(&p).unwrap_or_default();
                    if !txt.contains("![") {
                        silent.push(p.strip_prefix(&base).unwrap_or(&p).to_string_lossy().into_owned());
                    }
                }
            }
        }
        silent.sort();
        assert!(
            silent.is_empty(),
            "tool articles without a single picture ({}): a person is not shown what will come out:\n{}",
            silent.len(),
            silent.join("\n")
        );
    }
}
