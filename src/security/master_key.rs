// Phase 2 — Argon2id key derivation from master password.use std::path::Path;

use std::path::Path;

use aes_gcm::aead::rand_core::RngCore;
use aes_gcm::aead::OsRng;
use argon2::{Algorithm, Argon2, Params, Version};
use serde::{Deserialize, Serialize};

use crate::errors::{AppError, ErrorKind};
use crate::security::crypto;

/* ============================================================================================== */
/*                                    Default Argon2id parameters                                 */
/* ============================================================================================== */

/// Memory cost in KiB. OWASP recommendation for Argon2id.
const DEFAULT_M_COST: u32 = 19_456;
/// Time cost (iterations).
const DEFAULT_T_COST: u32 = 2;
/// Parallelism factor.
const DEFAULT_P_COST: u32 = 1;
/// Key output length in bytes (256-bit key for AES-256).
const KEY_LEN: usize = 32;
/// Known plaintext used to create a verification blob.
pub(crate) const VERIFICATION_PLAINTEXT: &[u8] = b"aztui-verification-v1";

/* ============================================================================================== */
/*                                       StoredKeyParams                                         */
/* ============================================================================================== */

/// Argon2id parameters and a verification blob, persisted to `master.json`.
/// Created once during password setup; loaded on subsequent launches.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredKeyParams {
    pub salt_hex: String,
    pub m_cost: u32,
    pub t_cost: u32,
    pub p_cost: u32,
    /// AES-256-GCM encrypted `VERIFICATION_PLAINTEXT`, used to verify the password.
    pub verification_blob_hex: String,
}

/* ============================================================================================== */
/*                                     Public free functions                                      */
/* ============================================================================================== */

/// Generates a cryptographically random 16-byte salt.
pub fn generate_salt() -> [u8; 16] {
    let mut salt = [0u8; 16];
    OsRng.fill_bytes(&mut salt);
    salt
}

/* ============================================================================================== */

/// Derives a 256-bit key from `password` and `salt` using Argon2id.
///
/// This is CPU-intensive and should be called from [`tokio::task::spawn_blocking`].
pub fn derive_key(password: &str, salt: &[u8], m_cost: u32, t_cost: u32, p_cost: u32) -> Result<[u8; 32], AppError> {
    let params = Params::new(m_cost, t_cost, p_cost, Some(KEY_LEN))
        .map_err(|e| AppError::new(ErrorKind::Unknown, format!("Argon2 params error: {}", e)))?;

    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = [0u8; KEY_LEN];

    argon2
        .hash_password_into(password.as_bytes(), salt, &mut key)
        .map_err(|e| AppError::new(ErrorKind::Unknown, format!("Argon2 derivation error: {}", e)))?;

    Ok(key)
}

/* ============================================================================================== */

/// Creates new key params + derives the key for first-time password setup.
///
/// Returns `(StoredKeyParams, derived_key)`. The caller is responsible for
/// saving `StoredKeyParams` to disk and storing the key in the `SecurityManager`.
///
/// CPU-intensive — call from [`tokio::task::spawn_blocking`].
pub fn create_params_and_key(password: &str) -> Result<(StoredKeyParams, [u8; 32]), AppError> {
    let salt = generate_salt();
    let key = derive_key(password, &salt, DEFAULT_M_COST, DEFAULT_T_COST, DEFAULT_P_COST)?;

    // Create verification blob: encrypt known plaintext with the derived key.
    let verification_blob = crypto::encrypt(&key, VERIFICATION_PLAINTEXT)?;

    let params = StoredKeyParams {
        salt_hex: bytes_to_hex(&salt),
        m_cost: DEFAULT_M_COST,
        t_cost: DEFAULT_T_COST,
        p_cost: DEFAULT_P_COST,
        verification_blob_hex: bytes_to_hex(&verification_blob),
    };

    Ok((params, key))
}

/* ============================================================================================== */

/// Derives the key from `password` and verifies it against the stored verification blob.
///
/// Returns the derived key on success.
///
/// CPU-intensive — call from [`tokio::task::spawn_blocking`].
pub fn derive_and_verify(password: &str, params: &StoredKeyParams) -> Result<[u8; 32], AppError> {
    let salt = hex_to_bytes(&params.salt_hex)?;
    let key = derive_key(password, &salt, params.m_cost, params.t_cost, params.p_cost)?;

    // Verify by decrypting the verification blob.
    let verification_blob = hex_to_bytes(&params.verification_blob_hex)?;
    let plaintext = crypto::decrypt(&key, &verification_blob).map_err(|_| {
        AppError::new(ErrorKind::MasterPasswordWrong, "Incorrect master password")
    })?;

    if plaintext != VERIFICATION_PLAINTEXT {
        return Err(AppError::new(ErrorKind::MasterPasswordWrong, "Incorrect master password"));
    }

    Ok(key)
}

/* ============================================================================================== */

/// Loads [`StoredKeyParams`] from `{data_dir}/master.json`.
/// Returns `None` if the file does not exist.
pub fn load_params(data_dir: &Path) -> Result<Option<StoredKeyParams>, AppError> {
    let path = data_dir.join("master.json");
    if !path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&path)
        .map_err(|e| AppError::config_error(format!("Cannot read {:?}: {}", path, e)))?;

    let params: StoredKeyParams = serde_json::from_str(&content)
        .map_err(|e| AppError::config_error(format!("Cannot parse {:?}: {}", path, e)))?;

    Ok(Some(params))
}

/* ============================================================================================== */

/// Persists [`StoredKeyParams`] to `{data_dir}/master.json`.
/// Creates the directory if it does not exist.
pub fn save_params(data_dir: &Path, params: &StoredKeyParams) -> Result<(), AppError> {
    std::fs::create_dir_all(data_dir)
        .map_err(|e| AppError::config_error(format!("Cannot create {:?}: {}", data_dir, e)))?;

    let path = data_dir.join("master.json");
    let content = serde_json::to_string_pretty(params)
        .map_err(|e| AppError::config_error(format!("Cannot serialize key params: {}", e)))?;

    std::fs::write(&path, content)
        .map_err(|e| AppError::config_error(format!("Cannot write {:?}: {}", path, e)))?;

    Ok(())
}

/* ============================================================================================== */
/*                                       Hex helpers                                              */
/* ============================================================================================== */

pub(crate) fn bytes_to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

/* ============================================================================================== */

pub(crate) fn hex_to_bytes(hex: &str) -> Result<Vec<u8>, AppError> {
    if hex.len() % 2 != 0 {
        return Err(AppError::new(ErrorKind::Unknown, "Invalid hex string length"));
    }
    (0..hex.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&hex[i..i + 2], 16)
                .map_err(|_| AppError::new(ErrorKind::Unknown, "Invalid hex character"))
        })
        .collect()
}

/* ============================================================================================== */
/*                                            Tests                                               */
/* ============================================================================================== */

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derive_key_deterministic() {
        let salt = [0xAA; 16];
        let key1 = derive_key("password", &salt, 256, 1, 1).unwrap();
        let key2 = derive_key("password", &salt, 256, 1, 1).unwrap();
        assert_eq!(key1, key2);
    }

    #[test]
    fn derive_key_different_passwords_different_keys() {
        let salt = [0xAA; 16];
        let key1 = derive_key("password1", &salt, 256, 1, 1).unwrap();
        let key2 = derive_key("password2", &salt, 256, 1, 1).unwrap();
        assert_ne!(key1, key2);
    }

    #[test]
    fn create_and_verify_round_trip() {
        let (params, _key) = create_params_and_key("test-password").unwrap();
        let verified_key = derive_and_verify("test-password", &params).unwrap();
        // Keys should match.
        let salt = hex_to_bytes(&params.salt_hex).unwrap();
        let expected = derive_key("test-password", &salt, params.m_cost, params.t_cost, params.p_cost).unwrap();
        assert_eq!(verified_key, expected);
    }

    #[test]
    fn verify_wrong_password_fails() {
        let (params, _) = create_params_and_key("correct").unwrap();
        let result = derive_and_verify("wrong", &params);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind, ErrorKind::MasterPasswordWrong);
    }

    #[test]
    fn hex_round_trip() {
        let bytes = [0xDE, 0xAD, 0xBE, 0xEF];
        let hex = bytes_to_hex(&bytes);
        assert_eq!(hex, "deadbeef");
        let back = hex_to_bytes(&hex).unwrap();
        assert_eq!(back, bytes);
    }
}
