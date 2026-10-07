// For macOS and Linux
#[derive(Debug, PartialEq, Eq)]
pub struct OcctLocation {
    /// The folder that holds `Standard.hxx`.
    pub include: &'static str,
    /// The folder that holds `libTKernel`.
    pub lib: &'static str,
}

pub fn occt_default_location(target_os: &str, target_arch: &str) -> OcctLocation {
    match (target_os, target_arch) {
        ("macos", "aarch64") => OcctLocation { include: "/opt/homebrew/opt/opencascade/include/opencascade", lib: "/opt/homebrew/opt/opencascade/lib" },
        ("macos", _) => OcctLocation { include: "/usr/local/opt/opencascade/include/opencascade", lib: "/usr/local/opt/opencascade/lib" },
        _ => OcctLocation { include: "/usr/include/opencascade", lib: "/usr/lib" },
    }
}

#[cfg(test)]
mod tests {
    use super::{occt_default_location, OcctLocation};

    #[test]
    fn apple_silicon_finds_the_kernel_homebrew_installed() {
        assert_eq!(occt_default_location("macos", "aarch64"), OcctLocation { include: "/opt/homebrew/opt/opencascade/include/opencascade", lib: "/opt/homebrew/opt/opencascade/lib" });
    }

    #[test]
    fn an_intel_mac_finds_the_kernel_homebrew_installed() {
        assert_eq!(occt_default_location("macos", "x86_64"), OcctLocation { include: "/usr/local/opt/opencascade/include/opencascade", lib: "/usr/local/opt/opencascade/lib" });
    }

    #[test]
    fn linux_keeps_the_distribution_paths_on_every_processor() {
        for arch in ["x86_64", "aarch64"] {
            assert_eq!(occt_default_location("linux", arch), OcctLocation { include: "/usr/include/opencascade", lib: "/usr/lib" });
        }
    }
}
