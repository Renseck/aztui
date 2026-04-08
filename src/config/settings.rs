use std::path::PathBuf;
use std::time::Duration;

use serde::Deserialize;

use crate::errors::AppError;

/* ============================================================================================== */
/*                                       Top-Level AppConfig                                      */
/* ============================================================================================== */

/// Application configuration loaded from `~/.aztui/config.toml` at startup.
/// Immutable at runtime. All fields have sensible defaults so the app works
/// without a config file.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub general: GeneralConfig,
    pub cache: CacheConfig,
    pub security: SecurityConfig,
    pub ui: UiConfig,
    pub cli: CliConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            general: GeneralConfig::default(),
            cache: CacheConfig::default(),
            security: SecurityConfig::default(),
            ui: UiConfig::default(),
            cli: CliConfig::default(),
        }
    }
}

impl AppConfig {
    /// Loads configuration from the given path, or from `~/.aztui/config.toml`
    /// if no path is provided. Returns defaults if the file does not exist.
    /// 
    /// # Errors
    /// Returns [`AppError`] if the file exists but cannot be read or parsed.
    pub fn load(path: Option<PathBuf>) -> Result<Self, AppError> {
        let config_path = match path {
            Some(p) => p,
            None => {
                let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
                home.join(".aztui").join("config.toml")
            }
        };

        if !config_path.exists() {
            return Ok(Self::default());
        }

        let content = std::fs::read_to_string(&config_path).map_err(|e| {
            AppError::config_error(format!("Cannot read {:?}: {}", config_path, e))
        })?;

        toml::from_str(&content).map_err(|e| {
            AppError::config_error(format!("Cannot parse {:?}: {}", config_path, e))
        })
    }
}

/* ============================================================================================== */
/*                                       Sub-configurations                                       */
/* ============================================================================================== */

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct GeneralConfig {
    pub data_dir: PathBuf,
    pub default_tenant: Option<String>,
    pub default_subscripton: Option<String>,
    pub max_recent_contexts: usize,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        Self {
            data_dir: home.join(".aztui"),
            default_tenant: None,
            default_subscripton: None,
            max_recent_contexts: 10,
        }
    }
}

/* ============================================================================================== */
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct CacheConfig {
    #[serde(with = "duration_secs")]
    pub context_soft_ttl: Duration,
    #[serde(with = "duration_secs")]
    pub context_hard_ttl: Duration,
    #[serde(with = "duration_secs")]
    pub resource_soft_ttl: Duration,
    #[serde(with = "duration_secs")]
    pub resource_hard_ttl: Duration,
    #[serde(with = "duration_secs")]
    pub cost_soft_ttl: Duration,
    #[serde(with = "duration_secs")]
    pub cost_hard_ttl: Duration,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            context_soft_ttl: Duration::from_secs(300),
            context_hard_ttl: Duration::from_secs(3600),
            resource_soft_ttl: Duration::from_secs(60),
            resource_hard_ttl: Duration::from_secs(300),
            cost_soft_ttl: Duration::from_secs(300),
            cost_hard_ttl: Duration::from_secs(1800),
        }
    }
}

/* ============================================================================================== */
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct SecurityConfig {
    pub master_password_enabled: bool,
    pub inactivity_timeout_secs: Option<u64>,
    pub use_os_keyring: bool,
}

impl SecurityConfig {
    pub fn inactivity_timeout(&self) -> Option<Duration> {
        self.inactivity_timeout_secs.map(Duration::from_secs)
    }
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            master_password_enabled: false,
            inactivity_timeout_secs: Some(10 * 60),
            use_os_keyring: false,
        }
    }
}

/* ============================================================================================== */
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct UiConfig {
    pub mouse_enabled: bool,
    pub status_bar_position: StatusBarPosition,
    pub show_operation_timing: bool,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            mouse_enabled: true,
            status_bar_position: StatusBarPosition::default(),
            show_operation_timing: true,
        }
    }
}

/* ============================================================================================== */

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StatusBarPosition {
    Top,
    Bottom,
}

impl Default for StatusBarPosition {
    fn default() -> Self {
        StatusBarPosition::Bottom
    }
}

/* ============================================================================================== */
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct CliConfig {
    pub az_path: Option<PathBuf>,
    #[serde(with = "duration_secs")]
    pub default_timeout: Duration,
    #[serde(with = "duration_secs")]
    pub login_timeout: Duration,
    pub output_format: String,
}

impl Default for CliConfig {
    fn default() -> Self {
        Self {
            az_path: None,
            default_timeout: Duration::from_secs(30),
            login_timeout: Duration::from_secs(120),
            output_format: "json".to_string(),
        }
    }
}

/* ============================================================================================== */
/*                              Serde helper: Duration as seconds u64                             */
/* ============================================================================================== */

mod duration_secs {
    use std::time::Duration;

    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(d: Duration, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u64(d.as_secs())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Duration, D::Error> {
        let secs = u64::deserialize(d)?;
        Ok(Duration::from_secs(secs))
    }
}