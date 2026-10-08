//! EVERY EXAMPLE IN THE DISTRIBUTION OPENS. A format change that leaves them behind is a change that
//! breaks the first thing a newcomer clicks on.
#[test]
fn every_example_opens() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let mut seen = 0;
    let mut bad = Vec::new();
    for e in std::fs::read_dir(&dir).expect("the examples directory").flatten() {
        let p = e.path();
        if p.extension().map(|x| x != "qcad").unwrap_or(true) {
            continue;
        }
        seen += 1;
        if let Err(why) = qymcad_io::load_project(&p.to_string_lossy()) {
            bad.push(format!("{}: {why}", p.file_name().unwrap_or_default().to_string_lossy()));
        }
    }
    assert!(seen > 0, "no examples were found in {}", dir.display());
    assert!(bad.is_empty(), "examples that do not open ({seen} looked at):\n{}", bad.join("\n"));
}

/// EVERY EXAMPLE OPENS SETTLED: what a rebuild would bring a document read from a file to - the frame of each sketch,
/// its contours in order - is done as it is read, so the document opened is the document that stays untouched.
///
/// Reported behaviour: a project just opened, with nothing done in it, titled itself unsaved - the first rebuild
/// gave its sketches the frame the file lacked, ten ids later than the file said.
#[test]
fn every_example_opens_settled() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let mut moved = Vec::new();
    for e in std::fs::read_dir(&dir).expect("the examples directory").flatten() {
        let p = e.path();
        if p.extension().map(|x| x != "qcad").unwrap_or(true) {
            continue;
        }
        let Ok(mut project) = qymcad_io::load_project(&p.to_string_lossy()) else { continue };
        let read = project.state_key();
        project.settle_loaded();
        if project.state_key() != read {
            moved.push(p.file_name().unwrap_or_default().to_string_lossy().into_owned());
        }
    }
    assert!(moved.is_empty(), "examples that change when settled after being read: {moved:?}");
}

/// A SKETCH SOLVED ONCE STAYS WHERE IT IS WHEN SOLVED AGAIN: every sketch of every example, solved after the document
/// settled, moves no point. Reported behaviour: nine sketches of a sample moved points by up to 0.0023 mm on every
/// solve, so any rebuild changed the document and the sample opened unsaved after it.
#[test]
fn every_example_sketch_stays_put_when_solved_again() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let mut moved = Vec::new();
    for e in std::fs::read_dir(&dir).expect("the examples directory").flatten() {
        let p = e.path();
        if p.extension().map(|x| x != "qcad").unwrap_or(true) {
            continue;
        }
        let Ok(mut project) = qymcad_io::load_project(&p.to_string_lossy()) else { continue };
        project.settle_loaded();
        // solving again is no edit: the document must not count as changed either, now that the key holds every
        // field a sketch saves (a circle's radius the solver writes back among them)
        let key = project.state_key();
        for si in 0..project.sketches.len() {
            let before: Vec<(f64, f64)> = project.sketches[si].points.iter().map(|q| (q.x, q.y)).collect();
            project.solve_sketch(si);
            let most = project.sketches[si].points.iter().zip(&before).map(|(q, (x, y))| (q.x - x).hypot(q.y - y)).fold(0.0, f64::max);
            if most > 1e-9 {
                moved.push(format!("{} / {}: {most:.6} mm", p.file_name().unwrap_or_default().to_string_lossy(), project.sketches[si].name));
            }
        }
        if project.state_key() != key {
            moved.push(format!("{}: the document counts as changed", p.file_name().unwrap_or_default().to_string_lossy()));
        }
    }
    assert!(moved.is_empty(), "sketches that move when solved again:\n{}", moved.join("\n"));
}
