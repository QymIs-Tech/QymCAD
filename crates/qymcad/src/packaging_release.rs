//! THE RELEASE RUN HANDS OVER EVERY PACKAGE IT BUILT.
//!
//! Reported behaviour: "When 'archive' is set to false, only a single file can be uploaded. Found 2
//! files to upload." The Windows job built the portable zip and the MSI installer, then died at the
//! upload step - after every minute of compilation had already been spent.
//!
//! WHY IT HAPPENED. The installer was added to the release run by widening the path of the existing
//! upload step from one glob to two. `archive: false` takes exactly ONE file: it exists so that a
//! package which is already a `.zip` does not arrive zipped a second time, and it has no meaning for a
//! pair. The two settings were edited a minute apart and only one of them was thought about.
//!
//! WHY A CHECK AND NOT JUST THE FIX. Both halves of this are the kind that is edited by hand and read by
//! nobody: how many files a step uploads, and how many packages the publishing job expects to find. They
//! are in different jobs a hundred and fifty lines apart, and they must agree. The failure costs a whole
//! run - the packages are built first and handed over last.
#[cfg(test)]
mod tests {
    fn workflow() -> String {
        let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.github/workflows/release.yml");
        std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("the release workflow must be readable: {e}"))
    }

    /// The `with:` block of every `actions/upload-artifact` step, as written.
    fn upload_steps() -> Vec<String> {
        let wf = workflow();
        let mut out = Vec::new();
        let mut lines = wf.lines().peekable();
        while let Some(l) = lines.next() {
            if !l.contains("uses: actions/upload-artifact") {
                continue;
            }
            // everything up to the next step, which starts with a `- ` at the same or a smaller indent
            let mut block = String::new();
            while let Some(n) = lines.peek() {
                if n.trim_start().starts_with("- ") || (!n.trim().is_empty() && !n.starts_with("      ")) {
                    break;
                }
                block.push_str(n);
                block.push('\n');
                lines.next();
            }
            out.push(block);
        }
        out
    }

    /// A STEP THAT UPLOADS THE FILE AS IT IS UPLOADS EXACTLY ONE.
    ///
    /// `archive: false` and a two-line `path:` cannot be combined, and the action says so only after the
    /// whole job has been built.
    #[test]
    fn a_file_uploaded_as_it_is_comes_alone() {
        let mut wrong = Vec::new();
        for block in upload_steps() {
            if !block.contains("archive: false") {
                continue;
            }
            // `path: dist/*.zip` is one; a `path: |` followed by several lines is not
            let globs = block
                .lines()
                .skip_while(|l| !l.trim_start().starts_with("path:"))
                .take_while(|l| !l.trim_start().starts_with("archive:") && !l.trim().is_empty())
                .filter(|l| l.contains('/') || l.contains('*'))
                .count();
            if globs != 1 {
                wrong.push(format!("a step uploads {globs} paths with `archive: false`, which takes one:\n{block}"));
            }
        }
        assert!(wrong.is_empty(), "the run would stop at the upload, after everything was built:\n{}", wrong.join("\n"));
    }

    /// AND THE PUBLISHING JOB EXPECTS AS MANY PACKAGES AS ARE HANDED OVER.
    ///
    /// The number there is written out by hand on purpose - a package that silently stopped being built
    /// is what that step exists to catch - and therefore drifts the moment a package is added.
    #[test]
    fn the_publishing_job_counts_the_packages_that_are_built() {
        let uploads = upload_steps().len();
        let wf = workflow();
        let line = wf
            .lines()
            .find(|l| l.contains("-eq") && l.contains("packages were expected"))
            .expect("the publishing job no longer counts what arrived");
        let expected: usize = line
            .split("-eq")
            .nth(1)
            .and_then(|t| t.split(']').next())
            .and_then(|t| t.trim().parse().ok())
            .expect("the count in the publishing job is not a number");
        assert_eq!(
            expected, uploads,
            "the run hands over {uploads} packages and the publishing job waits for {expected}; the Release page would go up short, or the run would stop at the last step"
        );
    }

    /// THE STEP THAT COMPILES THE PROGRAM CARRIES THE TAG.
    ///
    /// `build.rs` stamps the release from `QYMCAD_VERSION`, so the variable has to be present where
    /// `cargo build` runs. It was present at the steps that PACK the program instead - and packing
    /// happens after compiling, so the name of the file said `dev.20260828` while the program inside it
    /// could only name the manifest number.
    ///
    /// Found on the live workflow: two of the three systems built their binary at a step of their own
    /// without the variable (Windows line 129, macOS line 285), and only Linux got it, by accident of
    /// building inside the same script that packs. Nothing would have shown it - the packages are named
    /// correctly either way, and the difference is visible only in the About window and to whoever
    /// compares a version against the site.
    #[test]
    fn the_release_tag_reaches_the_step_that_compiles() {
        let wf = workflow();
        let mut naked = Vec::new();
        // walk the steps: a step starts at `- name:`/`- uses:` and ends where the next one starts
        let lines: Vec<&str> = wf.lines().collect();
        let mut start = 0usize;
        for (i, l) in lines.iter().enumerate() {
            let is_step = l.trim_start().starts_with("- name:") || l.trim_start().starts_with("- uses:");
            if !is_step && i + 1 != lines.len() {
                continue;
            }
            let end = if is_step { i } else { lines.len() };
            let step = lines[start..end].join("\n");
            // only the steps that compile the program itself; the docker one hands the variable over with -e
            if step.contains("cargo build") && !step.contains("docker run") && !step.contains("QYMCAD_VERSION") {
                let name = step.lines().next().unwrap_or("").trim().to_string();
                naked.push(format!("{name} (line {})", start + 1));
            }
            start = i;
        }
        assert!(
            naked.is_empty(),
            "a step compiles the program without QYMCAD_VERSION, so the binary cannot say which release it is while the file around it is named by the tag: {naked:?}"
        );
    }
}
