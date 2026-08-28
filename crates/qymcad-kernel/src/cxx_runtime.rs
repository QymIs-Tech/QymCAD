// WHICH C++ RUNTIME THE LINKER MUST BE TOLD ABOUT, BY TARGET.
//
// The bridge is C++ and OCCT is C++, so nothing links without one - but the runtime does not carry the same
// name everywhere, and naming a library that does not exist fails the link outright, at the very last step,
// after everything else has been built.
//
// * GNU toolchains - Linux, and `*-pc-windows-gnu` - call it `stdc++`;
// * Apple ships libc++ and nothing else, and there the name is `c++`;
// * MSVC links its own runtime by itself and takes no name at all.
//
// Kept apart from `build.rs` (which `include!`s this file) so that the choice can be checked for every target
// at once, on any machine, instead of only on the machine that happens to be building.
pub fn cxx_runtime(target_os: &str, target_env: &str) -> Option<&'static str> {
    match (target_os, target_env) {
        (_, "msvc") => None,
        ("macos" | "ios", _) => Some("c++"),
        _ => Some("stdc++"),
    }
}

#[cfg(test)]
mod tests {
    use super::cxx_runtime;

    // Reported behaviour: a macOS release build failed after 18 minutes with `ld: library 'stdc++' not
    // found`. libstdc++ is the GNU runtime and Apple does not ship it; the one on that machine is libc++.
    #[test]
    fn apple_is_told_about_libcxx_and_never_about_libstdcxx() {
        assert_eq!(cxx_runtime("macos", ""), Some("c++"));
        assert_eq!(cxx_runtime("ios", ""), Some("c++"));
    }

    #[test]
    fn gnu_toolchains_keep_libstdcxx() {
        assert_eq!(cxx_runtime("linux", "gnu"), Some("stdc++"));
        assert_eq!(cxx_runtime("windows", "gnu"), Some("stdc++"));
    }

    // MSVC brings its own runtime; a name here would be a library that does not exist.
    #[test]
    fn msvc_is_told_about_nothing() {
        assert_eq!(cxx_runtime("windows", "msvc"), None);
    }
}
