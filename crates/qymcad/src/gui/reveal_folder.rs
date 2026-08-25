//! WHAT OPENS THE SETTINGS FOLDER — an answer that can be checked.
//!
//! The "Open folder" button called a file manager through `cfg!(target_os = ...)`, so the choice was
//! made AT BUILD TIME. On a developer machine exactly one of the three branches exists, and neither
//! the compiler nor a test checks the other two: a typo in the name `explorer` would reach a Windows
//! user and look like a button that does not work.
//!
//! Now a pure function of the OS gives the answer, and the test asks it for all three systems at once.
#[cfg(test)]
mod tests {
    use egui::os::OperatingSystem as OS;

    /// EVERY SYSTEM GETS ITS OWN FILE MANAGER, named correctly.
    #[test]
    fn every_system_gets_its_own_file_manager() {
        let dir = std::path::Path::new("/tmp/qym cad settings");
        for (os, want) in [(OS::Windows, "explorer"), (OS::Mac, "open"), (OS::Nix, "xdg-open"), (OS::Unknown, "xdg-open")] {
            let (bin, args) = super::super::reveal_command(os, dir);
            assert_eq!(bin, want, "{os:?}: the file manager is named \"{bin}\" and \"{want}\" was expected");
            assert_eq!(args, vec![dir.to_string_lossy().into_owned()], "{os:?}: the path must go out as one argument");
        }
    }

    /// A PATH WITH A SPACE GOES OUT AS ONE ARGUMENT rather than falling apart into two.
    ///
    /// A home directory with a space is normal on macOS and Windows ("Application Support", "My
    /// Documents"). Assembling the command as a string would open the wrong folder, or none.
    #[test]
    fn a_path_with_spaces_stays_one_argument() {
        let dir = std::path::Path::new("/home/user/Application Support/qym cad");
        let (_, args) = super::super::reveal_command(OS::Mac, dir);
        assert_eq!(args.len(), 1, "the path fell apart into {} arguments: {args:?}", args.len());
        assert!(args[0].contains("Application Support"), "the path was lost: {args:?}");
    }

    /// AND THE BUTTON CALLS THAT FUNCTION rather than assembling the command in place.
    #[test]
    fn the_button_asks_the_function_instead_of_deciding_itself() {
        let src = crate::gui::panels_source::PANELS;
        assert!(src.contains("crate::gui::reveal_command(ui.ctx().os()"), "the \"Open folder\" button stopped asking `reveal_command`");
        assert!(!src.contains("cfg!(target_os = \"windows\")"), "the file manager choice went back into `cfg!` — on a developer machine that checks one branch out of three");
    }
}
