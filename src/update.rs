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