//! EVERY `try` OF THE BRIDGE CATCHES A FAULT OF OCCT. A fault inside an algorithm of OCCT - memory read that is not its
//! own, a division by zero - is a signal, and a `catch` sees it only when the `try` around the call opened with
//! `QYM_SIGNALS`: the handlers of `OSD` set and a point for the signal to jump to. A `try` without it lets the fault end
//! the program, as a rounding of R9.9999998 on the arc of an R10 rounding did.

/// Every C++ file under the `src` of every crate, as text: C++ that calls OCCT outside the bridge would be named too.
fn sources() -> Vec<(String, String)> {
    let crates = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("the crates directory");
    let mut stack: Vec<std::path::PathBuf> = std::fs::read_dir(crates).expect("the crates read").flatten().map(|e| e.path().join("src")).filter(|p| p.is_dir()).collect();
    let mut out: Vec<(String, String)> = Vec::new();
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir).expect("the sources read").flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "cpp") {
                let name = p.strip_prefix(crates).unwrap_or(&p).to_string_lossy().replace('\\', "/");
                out.push((name, std::fs::read_to_string(&p).unwrap_or_default()));
            }
        }
    }
    assert!(out.iter().any(|(n, _)| n.ends_with("occt_bridge.cpp")), "the bridge itself was not found");
    out
}

#[test]
fn every_try_opens_with_the_signals() {
    let mut bare: Vec<String> = Vec::new();
    let mut seen = 0;
    for (name, text) in sources() {
        let lines: Vec<&str> = text.lines().collect();
        for (n, line) in lines.iter().enumerate() {
            let t = line.trim_start();
            if t.starts_with("//") {
                continue;
            }
            for (at, _) in line.match_indices("try {") {
                seen += 1;
                let rest = line[at + "try {".len()..].trim();
                let next = if rest.is_empty() { lines.get(n + 1).map(|l| l.trim()).unwrap_or_default() } else { rest };
                if !next.starts_with("QYM_SIGNALS;") {
                    bare.push(format!("{name}:{}", n + 1));
                }
            }
        }
    }
    assert!(seen > 100, "only {seen} try blocks found: the walk does not see the bridge");
    assert!(bare.is_empty(), "{} try blocks of the bridge do not open with QYM_SIGNALS, and a fault of OCCT inside them ends the program:\n{}", bare.len(), bare.join("\n"));
}
