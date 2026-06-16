//! Self-update: checking GitHub releases and replacing the running binary.

use std::path::PathBuf;
use std::time::{Duration, SystemTime};
use sha2::{Digest, Sha256};

use crate::errors::AppError;


/* ============================================================================================== */
/*                                     Headless `update` command                                  */
/* ============================================================================================== */

/// Runs the `aztui update` subcommand. When `check_only`, reports availability
/// without changing anything. Prints progress to stdout. Synchronous.
pub fn run_update(check_only: bool) -> Result<(), AppError> {
    let current = env!("CARGO_PKG_VERSION");
    println!("  aztui v{} — checking for updates...", current);

    let info = fetch_latest_release()?;

    if !is_newer(current, &info.version) {
        println!("  You're on the latest version ({}).", current);
        return Ok(());
    }

    println!("  Update available: {} → {}", current, info.version);

    if check_only {
        println!("  Run `aztui update` to install it.");
        return Ok(());
    }

    println!("  Downloading and verifying...");
    apply_update(&info)?;
    println!("  Updated to {}. Restart aztui to use the new version.", info.version);
    Ok(())
}

/* ============================================================================================== */
/*                                        Apply an update                                         */
/* ============================================================================================== */

/// Downloads the release binary (and its `.sha256` sidecar, if present),
/// verifies the checksum, and replaces the running executable in place.
/// Synchronous; call via `spawn_blocking` from async contexts.
pub fn apply_update(info: &ReleaseInfo) -> Result<(), AppError> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("aztui-self-update")
        .build()
        .map_err(|e| AppError::unknown(format!("update: http client: {}", e)))?;

    // Download the binary into a temp file.
    let tmp_dir = tempfile::tempdir()
        .map_err(|e| AppError::unknown(format!("update: temp dir: {}", e)))?;
    let tmp_bin = tmp_dir.path().join("aztui-new.exe");

    let bytes = client
        .get(&info.bin_url)
        .header(reqwest::header::ACCEPT, "application/octet-stream")
        .send()
        .and_then(|r| r.error_for_status())
        .and_then(|r| r.bytes())
        .map_err(|e| AppError::unknown(format!("update: download binary: {}", e)))?;

    // Verify checksum if a sidecar is published.
    if let Some(sha_url) = &info.sha256_url {
        let expected = client
            .get(sha_url)
            .header(reqwest::header::ACCEPT, "application/octet-stream")
            .send()
            .and_then(|r| r.error_for_status())
            .and_then(|r| r.text())
            .map_err(|e| AppError::unknown(format!("update: download checksum: {}", e)))?;
        verify_sha256(&bytes, &expected)?;
    }

    std::fs::write(&tmp_bin, &bytes)
        .map_err(|e| AppError::unknown(format!("update: write temp binary: {}", e)))?;

    // Atomically replace the running executable.
    self_replace::self_replace(&tmp_bin)
        .map_err(|e| AppError::unknown(format!("update: replace binary: {}", e)))?;

    Ok(())
}

/// Compares the SHA-256 of `data` against the first whitespace-delimited token of
/// `checksum_text` (the standard `<hash>  <filename>` sidecar format).
fn verify_sha256(data: &[u8], checksum_text: &str) -> Result<(), AppError> {
    let expected = checksum_text
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_lowercase();

    let mut hasher = Sha256::new();
    hasher.update(data);
    let actual = hex_lower(&hasher.finalize());

    if actual == expected {
        Ok(())
    } else {
        Err(AppError::unknown(format!(
            "update: checksum mismatch (expected {}, got {})",
            expected, actual
        )))
    }
}

/// Lowercase hex encoding of a byte slice (avoids pulling in the `hex` crate).
fn hex_lower(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

/* ============================================================================================== */
/*                                     Throttled check helper                                     */
/* ============================================================================================== */

/// Path to the timestamp file used to throttle background update checks.
fn check_stamp_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".aztui").join(".update_check"))
}

/// Returns `true` if a background update check should run now (no stamp, or the
/// stamp is older than 24h). Best-effort: any error means "go ahead and check".
pub fn should_check_now() -> bool {
    let Some(path) = check_stamp_path() else {
        return true;
    };
    let Ok(meta) = std::fs::metadata(&path) else {
        return true;
    };
    let Ok(modified) = meta.modified() else {
        return true;
    };
    match SystemTime::now().duration_since(modified) {
        Ok(age) => age >= Duration::from_secs(24 * 60 * 60),
        Err(_) => true,
    }
}

/// Records that a background check just ran (touches the stamp file).
pub fn record_check() {
    if let Some(path) = check_stamp_path() {
        let _ = std::fs::write(&path, b"");
    }
}

/// Background check: if throttle allows and a newer release exists, returns its
/// version. Synchronous; call via `spawn_blocking`.
pub fn background_check() -> Option<String> {
    if !should_check_now() {
        return None;
    }
    record_check();
    let info = fetch_latest_release().ok()?;
    if is_newer(env!("CARGO_PKG_VERSION"), &info.version) {
        Some(info.version)
    } else {
        None
    }
}

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

    #[test]
    fn verify_sha256_accepts_matching_digest() {
        // echo -n "hello" | sha256sum
        let expected = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";
        let text = format!("{}  aztui.exe", expected);
        assert!(super::verify_sha256(b"hello", &text).is_ok());
    }

    #[test]
    fn verify_sha256_rejects_mismatch() {
        let text = "deadbeef  aztui.exe";
        assert!(super::verify_sha256(b"hello", text).is_err());
    }
}