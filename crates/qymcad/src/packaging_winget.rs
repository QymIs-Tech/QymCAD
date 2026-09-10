//! THE WINGET MANIFEST SAYS ONE THING IN THREE FILES, AND THE PRODUCT CODE IS THE ONE THE MSI CARRIES.
//!
//! A winget manifest is three YAML files - a version, a locale and an installer - and every one of them
//! repeats the package identifier and the version. They are submitted together and rejected together, and
//! the rejection is a bot comment three days later.
//!
//! THE PRODUCT CODE IS THE ONE THAT MATTERS. Windows tells one installed version from another by it, and
//! `winget upgrade` uses it to recognise what it put there. It must differ between versions - otherwise an
//! upgrade turns into "the same thing is already installed" - so it cannot be a constant; and it must be
//! knowable without opening the MSI, so it cannot be random either. It is computed from the version, by
//! the same rule in two places: `build-msi.ps1` (PowerShell, on a Windows runner) writes it into the
//! installer, `update-manifests.sh` (shell, on Linux, hours later) writes it into the manifest. Two
//! languages, two machines, one number - which is exactly the shape of thing that drifts.
#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    fn root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn read(rel: &str) -> String {
        let p = root().join(rel);
        std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{rel} must be readable: {e}"))
    }

    /// THE THREE TEMPLATES AGREE ON WHO AND WHAT THIS IS.
    #[test]
    fn the_three_manifest_files_name_the_same_package() {
        let files = ["version.yaml", "locale.en-US.yaml", "installer.yaml"];
        let mut wrong = Vec::new();
        for f in files {
            let src = read(&format!("packaging/winget/templates/{f}"));
            if !src.contains("PackageIdentifier: QymIsTech.QymCAD") {
                wrong.push(format!("{f}: does not name the package identifier"));
            }
            if !src.contains("PackageVersion: \"@VERSION@\"") {
                wrong.push(format!("{f}: does not take the version from the release"));
            }
            if !src.contains("ManifestVersion: 1.12.0") {
                wrong.push(format!("{f}: names a different schema version than its neighbours"));
            }
        }
        assert!(wrong.is_empty(), "the three files of one manifest disagree, and winget rejects them together:\n{}", wrong.join("\n"));
    }

    /// THE INSTALLER FILE CARRIES WHAT WINGET NEEDS TO INSTALL AND TO UPGRADE.
    ///
    /// Silence is a requirement of the repository, not a nicety: "all tools must support a silent install",
    /// and a package that cannot be installed unattended is not accepted at all.
    #[test]
    fn the_installer_file_says_how_to_install_silently_and_what_to_upgrade() {
        let src = read("packaging/winget/templates/installer.yaml");
        for what in ["InstallerType: wix", "silent", "ProductCode: \"@PRODUCTCODE@\"", "InstallerSha256: \"@SHA256@\"", "Architecture: x64"] {
            assert!(src.contains(what), "the installer manifest does not say {what:?}");
        }
    }

    /// THE PRODUCT CODE IS COMPUTED THE SAME WAY IN BOTH PLACES.
    ///
    /// Checked by the two things that decide the answer: the upgrade code they start from, and the fact
    /// that both derive it from the MSI version rather than from anything else. A guard that ran the two
    /// scripts would need Windows for one of them.
    #[test]
    fn both_halves_compute_the_product_code_from_the_same_upgrade_code() {
        const UPGRADE: &str = "DF949703-CA05-535F-80FD-8404D9704F33";
        let ps = read("packaging/win/build-msi.ps1");
        let sh = read("packaging/winget/update-manifests.sh");
        let wxs = read("packaging/win/qymcad.wxs");

        for (name, src) in [("build-msi.ps1", &ps), ("update-manifests.sh", &sh), ("qymcad.wxs", &wxs)] {
            assert!(src.contains(UPGRADE), "{name} does not use the upgrade code {UPGRADE}, so the three would describe different applications");
        }
        assert!(ps.contains("New-DeterministicGuid $upgradeCode $msiVersion"), "the installer no longer derives the product code from the version");
        assert!(sh.contains("uuid5(uuid.UUID('DF949703-CA05-535F-80FD-8404D9704F33'), sys.argv[1])"), "the manifest no longer derives the product code the same way");
    }

    /// THE UPGRADE CODE NEVER CHANGES, and the check says so out loud.
    ///
    /// It is what tells Windows that this MSI and the one installed last month are the same application.
    /// Change it and every existing installation becomes a stranger: the new version installs beside the
    /// old one instead of replacing it, and both sit in "Apps & features" for ever.
    #[test]
    fn the_upgrade_code_is_the_one_already_in_the_world() {
        let wxs = read("packaging/win/qymcad.wxs");
        assert!(
            wxs.contains("UpgradeCode=\"DF949703-CA05-535F-80FD-8404D9704F33\""),
            "the upgrade code changed: every copy already installed would stop being recognised as this program"
        );
    }

    /// AND NO PLACEHOLDER MAY REACH A SUBMITTED MANIFEST.
    #[test]
    fn the_script_refuses_to_leave_a_placeholder_behind() {
        let sh = read("packaging/winget/update-manifests.sh");
        assert!(sh.contains("@[A-Z0-9]*@"), "the script does not look for unfilled placeholders before finishing");
        let after = sh.split("@[A-Z0-9]*@").nth(1).expect("something follows the search");
        assert!(after.contains("exit 1"), "the script finds an unfilled placeholder and carries on anyway");
    }

    /// THE TOOL THAT BUILDS THE INSTALLER IS PINNED TO A VERSION.
    ///
    /// Reported behaviour: the first build of the MSI stopped with "WIX7015: You must accept the Open
    /// Source Maintenance Fee (OSMF) EULA to use WiX Toolset v7".
    ///
    /// Nothing of ours had changed. `dotnet tool install --global wix` takes the newest release, and the
    /// newest had become v7 - a version that refuses to run until the maintenance fee is declared paid.
    /// The fee arrived in v6 and is asked of everyone earning money from the toolset; v5 predates it and
    /// builds the same `.wxs`.
    ///
    /// THE FLOATING VERSION IS THE FAULT, not the fee. A build that worked yesterday broke because somebody
    /// else released something. Every other tool here is pinned - the Rust toolchain, the kernel - and this
    /// one was not.
    #[test]
    fn the_installer_tool_is_pinned() {
        let wf = read(".github/workflows/release.yml");
        // The command is quoted in the comment right above it, so match the step that runs, not the prose.
        let line = wf
            .lines()
            .map(str::trim)
            .find(|l| l.starts_with("run:") && l.contains("dotnet tool install") && l.contains("wix"))
            .expect("the workflow no longer installs WiX at all");
        assert!(
            line.contains("--version"),
            "WiX is installed without a version, so the build takes whatever was released last: {}",
            line.trim()
        );
    }

    /// THE INSTALLER PUTS A SHORTCUT WHERE PEOPLE LOOK FOR ONE.
    ///
    /// Reported behaviour: "the installer installs and uninstalls without asking anything, the program
    /// works - only it is not clear why no shortcut lands on the desktop, its absence is illogical."
    ///
    /// It was in the Start menu and nowhere else. That is defensible for a service and wrong for a
    /// program somebody opens every day: the desktop is where a person looks first, and an installer
    /// that leaves nothing there reads as an installer that did not finish.
    ///
    /// Both are asked for by name. A shortcut is a component like any other, so it disappears on
    /// uninstall by itself - which is the other half of what was reported working.
    #[test]
    fn the_installer_leaves_a_shortcut_in_the_start_menu_and_on_the_desktop() {
        let wxs = read("packaging/win/qymcad.wxs");
        for (folder, what) in [("ProgramMenuFolder", "the Start menu"), ("DesktopFolder", "the desktop")] {
            assert!(wxs.contains(folder), "the installer leaves nothing on {what}: no {folder} in the package");
        }
        // and each of them is an actual shortcut, not merely a mentioned folder
        assert_eq!(wxs.matches("<Shortcut").count(), 2, "there are not exactly two shortcuts - one for the Start menu, one for the desktop");
    }
}
