// For macOS and Linux
pub fn occt_default_location(target_os: &str, target_arch: &str) -> (&'static str, &'static str) {
    match (target_os, target_arch) {
        ("macos", "aarch64") => ("/opt/homebrew/include/opencascade", "/opt/homebrew/lib"),
        ("macos", _) => ("/usr/local/include/opencascade", "/usr/local/lib"),
        _ => ("/usr/include/opencascade", "/usr/lib"),
    }
}

#[cfg(test)]
mod tests {
    use super::occt_default_location;

    #[test]
    fn apple_silicon_finds_the_kernel_homebrew_installed() {
        assert_eq!(occt_default_location("macos", "aarch64"), ("/opt/homebrew/include/opencascade", "/opt/homebrew/lib"));
    }

    #[test]
    fn an_intel_mac_finds_the_kernel_homebrew_installed() {
        assert_eq!(occt_default_location("macos", "x86_64"), ("/usr/local/include/opencascade", "/usr/local/lib"));
    }

    #[test]
    fn linux_keeps_the_distribution_paths_on_every_processor() {
        for arch in ["x86_64", "aarch64"] {
            assert_eq!(occt_default_location("linux", arch), ("/usr/include/opencascade", "/usr/lib"));
        }
    }
}
