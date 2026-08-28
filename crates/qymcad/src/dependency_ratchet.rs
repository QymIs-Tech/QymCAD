//! HOW FAR BEHIND THE WORLD WE ARE — and it says so out loud.
//!
//! Three breakages in two days turned out to be one fault wearing three coats: MSYS2 moved under the
//! build, `rust-version` went stale unnoticed, and a graphics stack two years old started killing the
//! program on other people's Windows 11. Nothing watched the gap. There were 186 sets of checks here,
//! and half a dozen guards catching a Russian letter in a comment or text painted past the edge of a
//! window - and not one of them noticed that the renderer was seven releases behind.
//!
//! THIS GUARD DOES NOT DECIDE ANYTHING. Its job is to put the gap in front of a person: which package,
//! how far behind, and what that package does in the program. Whether to raise a version is decided
//! together and deliberately - a version raised quietly and shipped means the trouble reaches people
//! while the person answering for the release knows nothing about it.
//!
//! It works off `tools/deps.toml` rather than the network. A test that reaches for the network goes red
//! on a train and in an aeroplane, and a test like that is the first one silenced. The snapshot is
//! refreshed by hand: `python3 tools/check_deps.py --refresh`.
#[cfg(test)]
mod tests {
    /// TOTAL RELEASES BEHIND, over every direct dependency.
    ///
    /// It is a MARK, like the ceiling on Russian literals - not a permission to be this far behind.
    /// Growth is red, and so is slack: under a ceiling nobody lowers, a whole stack quietly ages, which
    /// is exactly how the graphics reached seven releases without a word.
    ///
    /// THE PATH OF THE MARK: 47 (27.08.2026, measured for the first time) -> 29 (the graphics stack
    /// brought up: eframe and egui 0.29 -> 0.35, wgpu 22 -> 30, egui-phosphor 0.7 -> 0.13) -> 19 (the
    /// document format: ron 0.8 -> 0.12, zip 2 -> 8) -> 2 (everything else: rfd, directories, usvg,
    /// stl_io, nalgebra, num-dual, cavalier_contours).
    ///
    /// THE LAST THREE ARE NOT OUR DEBT, and each is somebody else's version to move.
    ///
    /// Two of them are one release of `egui` and `eframe`, held back by `egui-phosphor`, which has no
    /// build for 0.36 yet. Lowering that would mean dropping the icons or vendoring somebody else's font,
    /// and neither is worth one release. It moves when phosphor does.
    ///
    /// The third is `pollster` (0.4 against 1.0.1): `wgpu` and `rfd` both pull 0.4, and it is 0.4 that is
    /// already compiled in. Declaring 1.0 would put a SECOND copy of the same forty lines in the binary
    /// to close a gap on paper. It moves when the graphics stack moves.
    const CEILING: usize = 3;

    struct Dep {
        name: String,
        declared: String,
        latest: String,
        what: String,
    }

    fn root() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    /// The direct dependencies of every manifest: name -> the version we declare.
    fn declared() -> Vec<(String, String)> {
        let mut out = Vec::new();
        let mut manifests = vec![root().join("Cargo.toml")];
        let mut crates: Vec<_> = std::fs::read_dir(root().join("crates"))
            .expect("the crates directory reads")
            .flatten()
            .map(|e| e.path().join("Cargo.toml"))
            .filter(|p| p.is_file())
            .collect();
        crates.sort();
        manifests.append(&mut crates);

        for path in manifests {
            let text = std::fs::read_to_string(&path).expect("a manifest reads");
            let mut in_deps = false;
            for line in text.lines() {
                let t = line.trim();
                if t.starts_with('[') {
                    in_deps = t.contains("dependencies]");
                    continue;
                }
                if !in_deps || t.is_empty() || t.starts_with('#') {
                    continue;
                }
                let Some((name, rhs)) = t.split_once('=') else { continue };
                let name = name.trim();
                // Our own crates and inherited entries are not the world's to move.
                if rhs.contains("path =") || rhs.contains("workspace = true") {
                    continue;
                }
                let version = rhs
                    .split_once("version = \"")
                    .map(|(_, v)| v)
                    .or_else(|| rhs.trim().strip_prefix('"'))
                    .and_then(|v| v.split('"').next());
                if let Some(v) = version {
                    if !out.iter().any(|(n, _): &(String, String)| n == name) {
                        out.push((name.to_string(), v.to_string()));
                    }
                }
            }
        }
        out
    }

    /// The snapshot of what has been released, plus what each package does here.
    fn snapshot() -> Vec<(String, String, String)> {
        let text = std::fs::read_to_string(root().join("tools/deps.toml")).expect("tools/deps.toml reads");
        let mut out = Vec::new();
        let (mut name, mut latest, mut what) = (String::new(), String::new(), String::new());
        for line in text.lines() {
            let t = line.trim();
            if let Some(n) = t.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
                if !name.is_empty() {
                    out.push((name.clone(), latest.clone(), what.clone()));
                }
                name = n.to_string();
                latest.clear();
                what.clear();
            } else if let Some(v) = t.strip_prefix("latest = \"").and_then(|s| s.strip_suffix('"')) {
                latest = v.to_string();
            } else if let Some(v) = t.strip_prefix("what = \"").and_then(|s| s.strip_suffix('"')) {
                what = v.to_string();
            }
        }
        if !name.is_empty() {
            out.push((name, latest, what));
        }
        out
    }

    /// How many releases behind.
    ///
    /// For a `0.x` version the SECOND number is the release: 0.29 -> 0.36 is seven, not zero. That is how
    /// Rust itself treats them, and counting otherwise means missing the largest gap there is.
    fn gap(declared: &str, latest: &str) -> usize {
        let num = |s: &str| -> (u64, u64) {
            let mut it = s.split(['.', '-']).filter_map(|p| p.parse::<u64>().ok());
            (it.next().unwrap_or(0), it.next().unwrap_or(0))
        };
        let (dm, dn) = num(declared);
        let (lm, ln) = num(latest);
        if dm == 0 && lm == 0 {
            ln.saturating_sub(dn) as usize
        } else {
            lm.saturating_sub(dm) as usize
        }
    }

    fn collect() -> Vec<Dep> {
        let snap = snapshot();
        declared()
            .into_iter()
            .map(|(name, declared)| {
                let (latest, what) = snap
                    .iter()
                    .find(|(n, _, _)| *n == name)
                    .map(|(_, l, w)| (l.clone(), w.clone()))
                    .unwrap_or_default();
                Dep { name, declared, latest, what }
            })
            .collect()
    }

    /// EVERY DIRECT DEPENDENCY IS WATCHED, and every one says what it is for.
    ///
    /// A package added without a note is a package nobody can weigh when the day comes: seven releases
    /// behind on the renderer and on a font-name parser are different conversations.
    #[test]
    fn nothing_slips_in_unwatched() {
        let deps = collect();
        assert!(deps.len() > 15, "suspiciously few direct dependencies found: {}", deps.len());

        let unknown: Vec<&str> = deps.iter().filter(|d| d.latest.is_empty()).map(|d| d.name.as_str()).collect();
        assert!(
            unknown.is_empty(),
            "these are not in tools/deps.toml: {unknown:?}\n\
             run `python3 tools/check_deps.py --refresh` and write what each one does"
        );

        let mute: Vec<&str> = deps.iter().filter(|d| d.what.is_empty()).map(|d| d.name.as_str()).collect();
        assert!(mute.is_empty(), "these have no note saying what they do here: {mute:?}");
    }

    /// THE GAP IS A CONVERSATION, NOT A CHORE.
    ///
    /// Red here does not mean "go and upgrade". It means the distance changed and somebody should look:
    /// the message names the package, the distance and the job it does, so the decision is made knowing
    /// what is at stake.
    #[test]
    fn the_distance_from_the_world_is_known() {
        let mut deps = collect();
        deps.sort_by(|a, b| gap(&b.declared, &b.latest).cmp(&gap(&a.declared, &a.latest)));
        let total: usize = deps.iter().map(|d| gap(&d.declared, &d.latest)).sum();

        let table: String = deps
            .iter()
            .filter(|d| gap(&d.declared, &d.latest) > 0)
            .map(|d| format!("  {:>2} behind  {:<20} {} -> {}  ({})\n", gap(&d.declared, &d.latest), d.name, d.declared, d.latest, d.what))
            .collect();

        assert!(
            total <= CEILING,
            "we have fallen further behind: {total} releases against a mark of {CEILING}.\n\
             This is not an instruction to upgrade - it is a reason to talk about it.\n{table}"
        );
        assert_eq!(
            total, CEILING,
            "the mark is stale: {total} releases behind now, set CEILING to that.\n\
             Slack is as bad as growth - under a ceiling nobody lowers, a whole stack ages in silence,\n\
             which is exactly how the graphics reached seven releases without a word.\n{table}"
        );
    }
}
