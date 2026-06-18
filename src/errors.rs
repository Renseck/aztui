use std::fmt;

use crate::command::Command;

/// The single error type for the entire application.
/// Every error carries enough context for the UI to guide the user.

#[derive(Debug, Clone)]
pub struct AppError {
    pub kind: ErrorKind,
    pub message: String,
    pub recovery: Option<RecoveryAction>,
    pub source_detail: Option<String>,
}

/* ============================================================================================== */
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorKind {
    // Auth
    AuthExpired,
    AuthFailed,
    TenantNotFound,
    SubscriptionNotFound,

    // AZ CLI
    CliNotFound,
    CliExecutionFailed,
    CliTimeout,
    CliParseError,
    CliExtensionMissing,

    // Network
    NetworkError,

    // Security
    MasterPasswordWrong,
    CacheDecryptionFailed,

    // System
    ConfigError,
    CacheError,
    Unknown,
}

/* ============================================================================================== */
/// Suggested recovery the UI can offer to the user.
#[derive(Debug, Clone)]
pub enum RecoveryAction {
    ReLogin,
    LoginToTenant(String),
    Retry(Box<Command>),
    OpenSettings,
    Manual(String),
    /// Install a missing `az` CLI extension by name (e.g. "resource-graph").
    InstallExtension(String),
}

/* ============================================================================================== */
/*                                    Convenience constructors                                    */
/* ============================================================================================== */

impl AppError {
    /// Creates a minimal error with the given kind and message.
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            recovery: None,
            source_detail: None,
        }
    }

    /* ========================================================================================== */
    /// Attaches a suggested recovery action.
    pub fn with_recovery(mut self, recovery: RecoveryAction) -> Self {
        self.recovery = Some(recovery);
        self
    }

    /* ========================================================================================== */
    ///Attaches raw source detail (e.g. CLI stderr).
    pub fn with_source(mut self, detail: impl Into<String>) -> Self {
        self.source_detail = Some(detail.into());
        self
    }

    /* ========================================================================================== */
    pub fn cli_not_found() -> Self {
        Self::new(
            ErrorKind::CliNotFound,
            "Azure CLI (`az`) not found on PATH. Install it from https://docs.microsoft.com/cli/azure/install-azure-cli",
        )
        .with_recovery(RecoveryAction::Manual(
            "Install the Azure CLI and ensure `az` is on your PATH, then restart aztui.".into(),
        ))
    }

    /* ========================================================================================== */
    pub fn cli_execution_failed(stderr: impl Into<String>) -> Self {
        Self::new(ErrorKind::CliExecutionFailed, "Azure CLI command failed with a non-zero exit code")
            .with_source(stderr)
    }

    /* ========================================================================================== */
    pub fn cli_timeout() -> Self {
        Self::new(ErrorKind::CliTimeout, "Azure CLI command timed out")
            .with_recovery(RecoveryAction::Retry(Box::new(Command::RefreshContextList)))
    }

    /* ========================================================================================== */
    pub fn cli_parse_error(detail: impl Into<String>) -> Self {
        Self::new(ErrorKind::CliParseError, "Failed to parse Azure CLI JSON output")
            .with_source(detail)
    }

    /* ========================================================================================== */
    pub fn auth_expired(tenant_id: impl Into<String>) -> Self {
        let tid = tenant_id.into();
        Self::new(ErrorKind::AuthExpired, "Azure authentication has expired")
            .with_recovery(RecoveryAction::LoginToTenant(tid))
    }

    /* ========================================================================================== */
    pub fn config_error(detail: impl Into<String>) -> Self {
        Self::new(ErrorKind::ConfigError, "Configuration error")
            .with_source(detail)
    }
    
    /* ========================================================================================== */
    pub fn unknown(detail: impl Into<String>) -> Self {
        Self::new(ErrorKind::Unknown, "An unexpected error occurred")
            .with_source(detail)
    }

    /* ========================================================================================== */

    /// Returns a short single-line label for the error kind, suitable for the status bar.
    pub fn kind_label(&self) -> &'static str {
        match self.kind {
            ErrorKind::AuthExpired => "Auth expired",
            ErrorKind::AuthFailed => "Auth failed",
            ErrorKind::TenantNotFound => "Tenant not found",
            ErrorKind::SubscriptionNotFound => "Subscription not found",
            ErrorKind::CliNotFound => "CLI not found",
            ErrorKind::CliExecutionFailed => "CLI error",
            ErrorKind::CliTimeout => "CLI Timeout",
            ErrorKind::CliParseError => "Parse error",
            ErrorKind::CliExtensionMissing => "Extension missing",
            ErrorKind::NetworkError => "Network error",
            ErrorKind::MasterPasswordWrong => "Wrong password",
            ErrorKind::CacheDecryptionFailed => "Cache decrypt failed",
            ErrorKind::ConfigError => "Config error",
            ErrorKind::CacheError => "Cache error",
            ErrorKind::Unknown => "Unknown error",
        }
    }
}

/* ============================================================================================== */
impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for AppError {}


/* ============================================================================================== */
/*                                      CLI stderr classification                                 */
/* ============================================================================================== */

/// Classifies an `az` CLI stderr string into a specific [`ErrorKind`] and, where
/// the recovery is context-free, an attached [`RecoveryAction`].
///
/// Matching is case-insensitive substring matching, ordered most-specific-first,
/// and always falls back to [`ErrorKind::CliExecutionFailed`] with no recovery.
/// The matchers are intentionally conservative and English-only — `az` stderr is
/// locale-sensitive, so unknown text must degrade gracefully rather than be
/// mis-classified.
///
/// Tenant-specific recovery (`LoginToTenant`) is **not** produced here; it is
/// attached one layer up in the auth provider, which knows the tenant id.
pub fn classify_cli_error(stderr: &str) -> (ErrorKind, Option<RecoveryAction>) {
    let s = stderr.to_ascii_lowercase();

    let contains_any = |needles: &[&str]| needles.iter().any(|n| s.contains(n));

    // Missing CLI extension — most specific, checked first.
    if contains_any(&[
        "the command requires the extension resource-graph",
        "requires the extension resource-graph",
        "'graph' is misspelled",
        "az graph",
    ]) && s.contains("graph")
    {
        return (
            ErrorKind::CliExtensionMissing,
            Some(RecoveryAction::InstallExtension("resource-graph".to_string())),
        );
    }

    // Expired tenant token (recovery filled in by the auth provider).
    if contains_any(&["aadsts", "token has expired", "refresh token has expired"]) {
        return (ErrorKind::AuthExpired, None);
    }

    // Not logged in / no usable account.
    if contains_any(&[
        "please run 'az login'",
        "run 'az login'",
        "no subscription found",
        "interactive authentication is needed",
        "no account currently logged in",
    ]) {
        return (ErrorKind::AuthFailed, Some(RecoveryAction::ReLogin));
    }

    // Network / DNS / TLS failures (caller attaches Retry).
    if contains_any(&[
        "getaddrinfo",
        "connection aborted",
        "failed to establish a new connection",
        "temporary failure in name resolution",
        "network is unreachable",
        "ssl",
    ]) {
        return (ErrorKind::NetworkError, None);
    }

    (ErrorKind::CliExecutionFailed, None)
}

/* ============================================================================================== */
/// Builds a fully-formed [`AppError`] from an `az` stderr string: classifies the
/// kind, attaches any context-free recovery, and preserves the raw stderr as
/// source detail. This is the single construction point used by the executor.
pub fn error_from_cli_stderr(stderr: &str) -> AppError {
    let (kind, recovery) = classify_cli_error(stderr);
    let message = match kind {
        ErrorKind::AuthFailed => "Not logged in to Azure",
        ErrorKind::AuthExpired => "Azure authentication has expired",
        ErrorKind::CliExtensionMissing => "A required Azure CLI extension is not installed",
        ErrorKind::NetworkError => "Could not reach Azure (network error)",
        _ => "Azure CLI command failed with a non-zero exit code",
    };
    let mut err = AppError::new(kind, message).with_source(stderr.to_string());
    if let Some(r) = recovery {
        err = err.with_recovery(r);
    }
    err
}


/* ============================================================================================== */
/*                                              Tests                                             */
/* ============================================================================================== */

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_missing_has_label() {
        let err = AppError::new(ErrorKind::CliExtensionMissing, "x");
        assert_eq!(err.kind_label(), "Extension missing");
    }

    #[test]
    fn install_extension_recovery_constructs() {
        let err = AppError::new(ErrorKind::CliExtensionMissing, "x")
            .with_recovery(RecoveryAction::InstallExtension("resource-graph".into()));
        assert!(matches!(err.recovery, Some(RecoveryAction::InstallExtension(_))));
    }

    #[test]
    fn classifies_not_logged_in_as_auth_failed_with_relogin() {
        let (kind, rec) = classify_cli_error("ERROR: Please run 'az login' to setup account.");
        assert_eq!(kind, ErrorKind::AuthFailed);
        assert!(matches!(rec, Some(RecoveryAction::ReLogin)));
    }

    #[test]
    fn classifies_aadsts_as_auth_expired_without_recovery() {
        let (kind, rec) = classify_cli_error("AADSTS700082: The refresh token has expired.");
        assert_eq!(kind, ErrorKind::AuthExpired);
        assert!(rec.is_none());
    }

    #[test]
    fn classifies_missing_extension_with_install_recovery() {
        let (kind, rec) =
            classify_cli_error("ERROR: The command requires the extension resource-graph.");
        assert_eq!(kind, ErrorKind::CliExtensionMissing);
        match rec {
            Some(RecoveryAction::InstallExtension(name)) => assert_eq!(name, "resource-graph"),
            other => panic!("expected InstallExtension, got {:?}", other),
        }
    }

    #[test]
    fn classifies_dns_failure_as_network_error_without_recovery() {
        let (kind, rec) =
            classify_cli_error("Could not connect: [Errno 11001] getaddrinfo failed");
        assert_eq!(kind, ErrorKind::NetworkError);
        assert!(rec.is_none());
    }

    #[test]
    fn unknown_stderr_falls_back_to_execution_failed() {
        let (kind, rec) = classify_cli_error("ERROR: something totally unexpected happened");
        assert_eq!(kind, ErrorKind::CliExecutionFailed);
        assert!(rec.is_none());
    }

    #[test]
    fn error_from_stderr_attaches_kind_recovery_and_source() {
        let err = error_from_cli_stderr("ERROR: Please run 'az login'");
        assert_eq!(err.kind, ErrorKind::AuthFailed);
        assert!(matches!(err.recovery, Some(RecoveryAction::ReLogin)));
        assert!(err.source_detail.is_some());
    }
}