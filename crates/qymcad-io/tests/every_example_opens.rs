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
