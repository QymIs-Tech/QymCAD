//! Points 16-19: the contexts, a refusal of the kernel, the time budget and the help.

use super::*;

/// 16. THE TOOL WORKS IN EVERY CONTEXT IT IS DESCRIBED FOR, or refuses in words.
pub(crate) fn contexts(tool: &Tool) {
    assert!(!tool.contexts.is_empty(), "{}: the description names no context besides a part of its own", tool.id);
    let mut problems = Vec::new();
    for c in tool.contexts {
        let mut s = Session::start();
        build::into_the_first_part(&mut s);
        let label = match c {
            Context::SecondPart => {
                build::into_a_new_part(&mut s);
                "a second part of the assembly".to_string()
            }
            Context::AfterTool(other) => {
                other.fixture.build_here(&mut s);
                take(&mut s, other);
                for p in other.picks {
                    click(&mut s, p);
                }
                enter_key(&mut s);
                format!("after {}", other.id)
            }
        };
        tool.fixture.build_here(&mut s);
        let status = s.status();
        take(&mut s, tool);
        match tool.flow {
            Flow::Command => {
                for p in tool.picks {
                    click(&mut s, p);
                }
            }
            Flow::Drawing(clicks, _) => {
                for (x, y) in clicks.iter().take(clicks.len().saturating_sub(1)) {
                    s.click_on_sketch(*x, *y);
                }
            }
            Flow::Action => {}
        }
        finish_all(&mut s, tool);
        let doc = s.document();
        let made = match tool.flow {
            Flow::Command if tool.node.is_empty() => matches(&mut s, &tool.result).is_ok(),
            Flow::Command => doc.features.iter().any(|f| f.kind == tool.node && f.error.is_none()),
            Flow::Drawing(..) | Flow::Action => matches(&mut s, &tool.result).is_ok(),
        };
        if !made && s.status() == status {
            problems.push(format!("{label}: nothing was made and nothing was said"));
        }
    }
    assert!(problems.is_empty(), "{}: contexts:\n{}", tool.id, problems.join("\n"));
}

/// 17. A VALUE OF THE RIGHT FORM THE KERNEL CANNOT BUILD IS REFUSED IN WORDS, and the document is whole.
pub(crate) fn kernel_refusal(tool: &Tool) {
    let value = tool.refusal.unwrap_or_else(|| panic!("{}: the description gives no value the kernel cannot build, and does not say why", tool.id));
    let f = first_field(tool);
    let mut s = value_ready(tool, f, &fmt(value));
    let body = the_body(&s.document()).map(|b| (b.volume, b.faces)).ok();
    let status = s.status();
    finish_with(&mut s, tool, f, &fmt(value));
    let doc = s.document();
    let mut problems = Vec::new();
    let red = doc.features.iter().find(|f| f.kind == tool.node).and_then(|f| f.error.clone());
    let node_made = doc.features.iter().any(|f| f.kind == tool.node);
    if node_made && red.is_none() {
        problems.push(format!("{value} was applied as if it built: {:?}", doc.features));
    }
    if !node_made && s.status() == status {
        problems.push(format!("{value}: nothing was made and nothing was said"));
    }
    if body.is_some() && the_body(&doc).map(|b| (b.volume, b.faces)).ok() != body {
        problems.push("the body it was applied to is not whole any more".to_string());
    }
    assert!(problems.is_empty(), "{}: a value the kernel cannot build:\n{}", tool.id, problems.join("\n"));
}

/// 18. APPLYING AND EVERY FRAME ARE WITHIN THEIR TIME.
pub(crate) fn budget(tool: &Tool) {
    let mut s = ready(tool);
    let _ = s.worst_frame();
    let began = Instant::now();
    finish_all(&mut s, tool);
    let _ = s.document();
    let (took, frame) = (began.elapsed(), s.worst_frame());
    let (apply_budget, frame_budget) = (Duration::from_secs(tool.budget.0), Duration::from_millis(tool.budget.1));
    assert!(took <= apply_budget && frame <= frame_budget, "{}: applying took {took:?} of {apply_budget:?}, the longest frame {frame:?} of {frame_budget:?}", tool.id);
}

/// 19. F1 WITH THE TOOL IN HAND OPENS ITS ARTICLE.
pub(crate) fn help(tool: &Tool) {
    let mut s = tool.fixture.start();
    take(&mut s, tool);
    s.key(Key::F1);
    let article = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../docs/help/{}/{}.md", s.language(), tool.help));
    let text = std::fs::read_to_string(&article).unwrap_or_else(|_| panic!("{}: there is no article {}", tool.id, article.display()));
    let title = text.lines().next().unwrap_or_default().trim_start_matches('#').trim().to_string();
    let window = s.word("help-title");
    let frame = s.widgets().into_iter().find(|w| w.kind == qymcad::Kind::Window && w.label == window).map(|w| w.rect);
    let shown_in_it = frame.is_some_and(|f| s.words_at().iter().any(|(w, r)| *w == title && f.contains(r.center())));
    assert!(
        shown_in_it,
        "{}: F1 with the tool in hand: the help window is {}, and the article {title:?} is not in it; windows open: {:?}",
        tool.id,
        if frame.is_some() { "open" } else { "not open" },
        s.windows()
    );
}
