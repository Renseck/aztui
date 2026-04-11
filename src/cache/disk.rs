use std::collections::HashMap;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::models::{AzureContext, Subscription, Tenant};
use crate::errors::{AppError, ErrorKind};
use crate::security::{SecurityManager};

/* ============================================================================================== */
/*                                        Disk cache format                                       */
/* ============================================================================================== */

/// Version byte preprended to encrypted cache files.
const CACHE_FORMAT_VERSION: u8 = 1;

/// Serializable snapshot of cached data for disk persistence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskCacheData {
    pub version: u8,
    pub saved_at: String,
    pub tenants: Vec<Tenant>,
    pub subscription_by_tenant: HashMap<String, Vec<Subscription>>,
    pub recent_contexts: Vec<AzureContext>,
}

/* ============================================================================================== */
/*                                            DiskCache                                           */
/* ============================================================================================== */

/// Handles reading/writing the cache to disk, with optional AES-256-GCM encryption.
pub struct DiskCache {
    data_dir: PathBuf,
}

impl DiskCache {
    pub fn new(data_dir: &Path) -> Self {
        Self {
            data_dir: data_dir.to_path_buf(),
        }
    }

    /* ========================================================================================== */
    /// Saves cache data to disk. Encrypts if security is enabled and unlocked.
    ///
    /// - Encrypted: writes `{data_dir}/cache.dat` as `[version byte][encrypted JSON]`
    /// - Plain: writes `{data_dir}/cache.json` as readable JSON
    pub fn save(&self, data: &DiskCacheData, security: &SecurityManager) -> Result<(), AppError> {
        std::fs::create_dir_all(&self.data_dir)
            .map_err(|e| AppError::new(ErrorKind::CacheError, format!("Cannot create cache dir: {}", e)))?;

        let json = serde_json::to_string_pretty(data)
            .map_err(|e| AppError::new(ErrorKind::CacheError, format!("Cannot serialize cache: {}", e)))?;

        if security.is_enabled() && security.is_unlocked() {
            let encrypted = security.encrypt(json.as_bytes())?;
            let mut output = vec![CACHE_FORMAT_VERSION];
            output.extend_from_slice(&encrypted);

            let path = self.data_dir.join("cache.dat");
            std::fs::write(&path, &output)
                .map_err(|e| AppError::new(ErrorKind::CacheError, format!("Cannot write {:?}: {}", path, e)))?;

            // Remove plain cache if it exists (migration from unencrypted to encrypted).
            let plain_path = self.data_dir.join("cache.json");
            if plain_path.exists() {
                let _ = std::fs::remove_file(&plain_path);
            }
        } else {
            let path = self.data_dir.join("cache.json");
            std::fs::write(&path, &json)
                .map_err(|e| AppError::new(ErrorKind::CacheError, format!("Cannot write {:?}: {}", path, e)))?;
        }

        Ok(())
    }

    /* ========================================================================================== */
    /// Loads cache data from disk. Decrypts if necessary.
    ///
    /// Returns `None` if no cache file exists or the data is expired.
    /// On corruption or decryption failure, logs a warning and returns `None`
    /// (the app will fall back to fresh CLI calls).
    pub fn load(
        &self,
        security: &SecurityManager,
        hard_ttl: std::time::Duration,
    ) -> Result<Option<DiskCacheData>, AppError> {
        // Try encrypted cache first, then plain.
        let encrypted_path = self.data_dir.join("cache.dat");
        let plain_path = self.data_dir.join("cache.json");

        let json_str = if encrypted_path.exists() && security.is_enabled() {
            let raw = std::fs::read(&encrypted_path)
                .map_err(|e| AppError::new(ErrorKind::CacheError, format!("Cannot read {:?}: {}", encrypted_path, e)))?;

            if raw.is_empty() {
                return Ok(None);
            }

            let version = raw[0];
            if version != CACHE_FORMAT_VERSION {
                // Unknown format version — discard.
                return Ok(None);
            }

            let decrypted = security.decrypt(&raw[1..])?;
            String::from_utf8(decrypted)
                .map_err(|_| AppError::new(ErrorKind::CacheError, "Cache data is not valid UTF-8"))?
        } else if plain_path.exists() {
            std::fs::read_to_string(&plain_path)
                .map_err(|e| AppError::new(ErrorKind::CacheError, format!("Cannot read {:?}: {}", plain_path, e)))?
        } else {
            return Ok(None);
        };

        let data: DiskCacheData = match serde_json::from_str(&json_str) {
            Ok(d) => d,
            Err(_) => return Ok(None), // Corrupt — start fresh.
        };

        // Check if data is older than hard TTL.
        if let Ok(saved_at) = data.saved_at.parse::<DateTime<Utc>>() {
            let age = Utc::now().signed_duration_since(saved_at);
            if let Ok(age_std) = age.to_std() {
                if age_std > hard_ttl {
                    return Ok(None); // Expired.
                }
            }
        }

        Ok(Some(data))
    }
}

/* ============================================================================================== */
impl DiskCacheData {
    /// Creates a snapshot from current application state.
    pub fn from_state(
        tenants: &[Tenant],
        subscription_by_tenant: &HashMap<String, Vec<Subscription>>,
        recent_contexts: &[AzureContext],
    ) -> Self {
        Self {
            version: CACHE_FORMAT_VERSION,
            saved_at: Utc::now().to_rfc3339(),
            tenants: tenants.to_vec(),
            subscription_by_tenant: subscription_by_tenant.clone(),
            recent_contexts: recent_contexts.to_vec(),
        }
    }
}