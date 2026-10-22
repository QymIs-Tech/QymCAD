//! THE BOXES OF THE TOOLS AS A PERSON SEES THEM: every tool described taken as a person takes it, its picks made and
//! its drawing finished, and the window drawn into `target/box-look/<tool>.png`. Nothing is checked: the pictures are
//! looked at. The run: `BOX_LOOK=<part of an id> cargo test -p qymcad-acceptance --lib box_look -- --ignored`.
#[cfg(test)]
mod tests {
    use crate::contract::run::ready_after;
    use crate::contract::Flow;

    #[test]
    #[ignore = "pictures of the boxes of the tools: they land in target/box-look"]
    fn box_look() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/box-look");
        std::fs::create_dir_all(&dir).expect("the directory of the pictures");
        let only = std::env::var("BOX_LOOK").unwrap_or_default();
        for tool in crate::tools::ALL.iter().filter(|t| only.is_empty() || only.split(',').any(|o| t.id.contains(o))) {
            let shot = std::panic::catch_unwind(|| {
                // a window wide enough for every button of the panel to stand on it
                let mut s = ready_after(tool, |s| {
                    s.resize(qymcad::vec2(1600.0, 1000.0));
                });
                if let Flow::Drawing(clicks, _) = tool.flow {
                    if let Some(at) = clicks.last() {
                        s.click_on_sketch(at.0, at.1);
                    }
                }
                // a tool with no box at the geometry - nothing to type - is no picture of a box
                // the bar and the box each end with Apply: two of them on screen are a box at the geometry
                let apply = s.word("cmd-apply-enter");
                let boxed = s.words_at().iter().filter(|(w, _)| *w == apply).count() >= if tool.id.contains("piece") { 1 } else { 2 };
                boxed.then(|| s.snapshot().png())
            });
            match shot {
                Ok(Some(png)) => std::fs::write(dir.join(format!("{}.png", tool.id)), png).expect("the picture is written"),
                Ok(None) => eprintln!("{}: no box", tool.id),
                Err(_) => eprintln!("{}: not taken", tool.id),
            }
        }
    }
}
