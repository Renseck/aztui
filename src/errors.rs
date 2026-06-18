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
}