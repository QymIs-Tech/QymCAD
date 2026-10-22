//! PATTERNS OF PARTS ACROSS COUNTS AND DIRECTIONS: the block of the first part picked in the assembly, a row of it
//! laid along X and along Y at the pitch the tool opens with (30), two, three and five long; a ring of it about the
//! upright axis, three, four and six round; a row of one and of none refused. What is read back is the number of parts
//! and where each stands.
use qymcad::{Key, Session};
use qymcad_acceptance::contract::fixtures::Fixture;
use qymcad_acceptance::probe;

/// The pattern the button with `hint` names, of `count`, along the direction `dir` names when there is one.
fn pattern(hint: &str, count: &str, dir: Option<&str>) -> Session {
    let mut s = Fixture::BlockInAssemblyPicked.start();
    let hint = s.word(hint);
    s.press_hint(&hint);
    // the direction is a choice of the row "Axis" of the box; the properties beside the view name X, Y and Z too
    if let Some(d) = dir {
        let axis = s.word("box-axis");
        let row = s.find(&axis, qymcad::pos2(0.0, 0.0)).unwrap_or_else(|| panic!("the box names no axis; on screen: {:?}", s.words()));
        s.press_word_near(d, row.center());
    }
    let copies = s.word("cmd-copies");
    s.fill(&copies, count);
    s.key(Key::Enter).key(Key::Enter);
    s
}

/// A case by its name, and its check: what went wrong, or nothing.
struct Case {
    what: String,
    check: Box<dyn Fn() -> Option<String>>,
}

/// Every case, all that go wrong reported at once.
fn run(cases: Vec<Case>) {
    let n = cases.len();
    let failed: Vec<String> = cases
        .into_iter()
        .filter_map(|Case { what, check: c }| {
            let mut out = None;
            let problem = qymcad_acceptance::refusal(|| out = c());
            let problem = if problem.is_empty() { out } else { Some(problem) };
            problem.map(|p| format!("{what}: {p}"))
        })
        .collect();
    assert!(failed.is_empty(), "{} of {n} cases went wrong:\n{}", failed.len(), failed.join("\n"));
}

probe! {
    budget = 1800;
    /// A ROW of the part along X and along Y, two, three and five long: the part and its copies stand 30 apart.
    fn rows_of_parts() {
        let mut cases: Vec<Case> = Vec::new();
        for (dir, k) in [("X", 0usize), ("Y", 1usize)] {
            for n in [2usize, 3, 5] {
                cases.push(Case { what: format!("{n} along {dir}"), check: Box::new(move || {
                    let mut s = pattern("tb-comp-lin-array-hint", &n.to_string(), Some(dir));
                    let mut along: Vec<f64> = s.document().parts.iter().map(|p| p.at[k]).collect();
                    along.sort_by(f64::total_cmp);
                    let want: Vec<f64> = (0..n).map(|i| 30.0 * i as f64).collect();
                    let fits = along.len() == n && along.iter().zip(&want).all(|(a, w)| (a - w).abs() < 1e-3);
                    (!fits).then(|| format!("the parts stand at {along:?} along {dir}, they should at {want:?}; the program says {:?}", s.status()))
                }) });
            }
        }
        run(cases);
    }
}

probe! {
    budget = 1800;
    /// A RING of the part about the upright axis, three, four and six round: as many parts, each at a place of its own.
    fn rings_of_parts() {
        run([3usize, 4, 6]
            .into_iter()
            .map(|n| {
                Case { what: format!("{n} round"), check: Box::new(move || {
                    let mut s = pattern("tb-comp-circ-array-hint", &n.to_string(), None);
                    let parts = s.document().parts;
                    let mut places: Vec<[i64; 3]> = parts.iter().map(|p| p.at.map(|v| (v * 1000.0).round() as i64)).collect();
                    let mut turns: Vec<[[i64; 3]; 3]> = parts.iter().map(|p| p.axes.map(|r| r.map(|v| (v * 1000.0).round() as i64))).collect();
                    places.sort();
                    places.dedup();
                    turns.sort();
                    turns.dedup();
                    (parts.len() != n || places.len().max(turns.len()) != n).then(|| format!("{} parts, {} places and {} turns among them, not {n} of each; the program says {:?}", parts.len(), places.len(), turns.len(), s.status()))
                }) }
            })
            .collect());
    }
}

probe! {
    budget = 900;
    /// A ROW OF ONE AND OF NONE makes no copy, and says so.
    fn rows_of_one_and_none_refused() {
        run(["1", "0"]
            .into_iter()
            .map(|n| {
                Case { what: format!("a row of {n}"), check: Box::new(move || {
                    let mut s = Fixture::BlockInAssemblyPicked.start();
                    let status = s.status();
                    let hint = s.word("tb-comp-lin-array-hint");
                    s.press_hint(&hint);
                    let copies = s.word("cmd-copies");
                    s.fill(&copies, n);
                    s.key(Key::Enter).key(Key::Enter);
                    let parts = s.document().parts.len();
                    (parts != 1 || s.status() == status).then(|| format!("{parts} parts, and the program says {:?}", s.status()))
                }) }
            })
            .collect());
    }
}
