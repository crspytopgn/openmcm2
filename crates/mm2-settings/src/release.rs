//! Which OpenMCM2 release this is, and how release tags compare: shared by
//! the launcher's updater and the game's main menu notice.

/// The latest release, from GitHub's API.
pub const LATEST: &str = "https://api.github.com/repos/crspytopgn/openmcm2/releases/latest";
/// The release page people see.
pub const PAGE: &str = "https://github.com/crspytopgn/openmcm2/releases/latest";

/// This build's release tag, from the release build (`OPENMCM2_VERSION`,
/// or the release workflow's `VERSION`). Builds made from source have
/// none.
pub fn current() -> Option<&'static str> {
    [option_env!("OPENMCM2_VERSION"), option_env!("VERSION")]
        .into_iter()
        .flatten()
        .find(|v| parse(v).is_some())
}

/// `v1.2.3` as numbers, or none if it isn't a version.
fn parse(tag: &str) -> Option<Vec<u64>> {
    let t = tag.trim().trim_start_matches(['v', 'V']);
    let parts = t.split('.').map(|p| p.parse().ok()).collect::<Option<Vec<u64>>>()?;
    (!parts.is_empty()).then_some(parts)
}

/// Whether `tag` is a later version than `than`.
pub fn newer(tag: &str, than: &str) -> bool {
    match (parse(tag), parse(than)) {
        (Some(mut a), Some(mut b)) => {
            let n = a.len().max(b.len());
            a.resize(n, 0);
            b.resize(n, 0);
            a > b
        }
        _ => false,
    }
}

/// The end of this platform's release archive name, as the release
/// workflow names them.
pub fn platform() -> Option<&'static str> {
    if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        Some("-macos-arm64.zip")
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        Some("-macos-x86_64.zip")
    } else if cfg!(all(windows, target_arch = "x86_64")) {
        Some("-windows-x86_64.zip")
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        Some("-linux-x86_64.tar.gz")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_by_number() {
        assert!(newer("v0.1.6", "v0.1.5"));
        assert!(newer("v0.2", "v0.1.9"));
        assert!(newer("v0.1.10", "v0.1.9"));
        assert!(!newer("v0.1.5", "v0.1.5"));
        assert!(!newer("v0.1.5.0", "v0.1.5"));
        assert!(!newer("v0.1.4", "v0.1.5"));
        assert!(!newer("nightly", "v0.1.5"));
    }
}
