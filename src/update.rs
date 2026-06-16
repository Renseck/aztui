//! Self-update: checking GitHub releases and replacing the running binary.

use crate::errors::AppError;

/* ============================================================================================== */
/*                                       Version comparison                                       */
/* ============================================================================================== */

/// Returns `true` if `latest` is a strictly newer semantic version than
/// `current`. Both may carry a leading `v` (e.g. `v0.4.0`). Non-numeric or
/// malformed inputs compare as "not newer" so we never offer a bogus update.
pub fn is_newer(current: &str, latest: &str) -> bool {
    match (parse_semver(current), parse_semver(latest)) {
        (Some(c), Some(l)) => l > c,
        _ => false,
    }
}

/// Parses a `MAJOR.MINOR.PATCH` string (optionally `v`-prefixed) into a tuple
/// for ordering. Returns `None` if it doesn't have three numeric components.
fn parse_semver(s: &str) -> Option<(u64, u64, u64)> {
    let s = s.trim().trim_start_matches('v');
    let mut parts = s.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some((major, minor, patch))
}

/* ============================================================================================== */
/*                                       Release metadata                                         */
/* ============================================================================================== */

const REPO_OWNER: &str = "Renseck";
const REPO_NAME: &str = "aztui";
const BIN_ASSET: &str = "aztui.exe";
const SHA_ASSET: &str = "aztui.exe.sha256";

/// Metadata for the latest GitHub release relevant to updating.
#[derive(Debug, Clone)]
pub struct ReleaseInfo {
    pub version: String,
    pub bin_url: String,
    pub sha256_url: Option<String>,
}

/// Fetches the latest release from GitHub. Synchronous (blocking HTTP); call via
/// `spawn_blocking` from async contexts.
pub fn fetch_latest_release() -> Result<ReleaseInfo, AppError> {
    let releases = self_update::backends::github::ReleaseList::configure()
        .repo_owner(REPO_OWNER)
        .repo_name(REPO_NAME)
        .build()
        .map_err(|e| AppError::unknown(format!("update: configure release list: {}", e)))?
        .fetch()
        .map_err(|e| AppError::unknown(format!("update: fetch releases: {}", e)))?;

    let latest = releases
        .first()
        .ok_or_else(|| AppError::unknown("update: no releases found"))?;

    let bin_url = latest
        .assets
        .iter()
        .find(|a| a.name == BIN_ASSET)
        .map(|a| a.download_url.clone())
        .ok_or_else(|| AppError::unknown(format!("update: release has no {} asset", BIN_ASSET)))?;

    let sha256_url = latest
        .assets
        .iter()
        .find(|a| a.name == SHA_ASSET)
        .map(|a| a.download_url.clone());

    Ok(ReleaseInfo {
        version: latest.version.clone(),
        bin_url,
        sha256_url,
    })
}

/* ============================================================================================== */
/*                                              Tests                                             */
/* ============================================================================================== */

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newer_patch_minor_major() {
        assert!(is_newer("0.3.1", "0.3.2"));
        assert!(is_newer("0.3.9", "0.4.0"));
        assert!(is_newer("0.9.9", "1.0.0"));
    }

    #[test]
    fn same_or_older_is_not_newer() {
        assert!(!is_newer("0.4.0", "0.4.0"));
        assert!(!is_newer("0.4.0", "0.3.9"));
    }

    #[test]
    fn handles_v_prefix() {
        assert!(is_newer("v0.3.1", "v0.4.0"));
        assert!(is_newer("0.3.1", "v0.3.2"));
    }

    #[test]
    fn malformed_never_newer() {
        assert!(!is_newer("0.3", "0.4.0"));
        assert!(!is_newer("garbage", "0.4.0"));
        assert!(!is_newer("0.3.1", "not-a-version"));
    }
}