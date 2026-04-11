use aes_gcm::aead::{Aead, AeadCore, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Key, Nonce};

use crate::errors::{AppError, ErrorKind};

/* ============================================================================================== */
/// Encrypts `plaintext` with AES-256-GCM using the given 256-bit key.
///
/// Output format: `[nonce (12 bytes)][ciphertext + auth tag]`.
/// A fresh random nonce is generated per call — never reuses nonces.
///
/// # Errors
/// Returns [`AppError`] on encryption failure.
pub fn encrypt(key: &[u8; 32], plaintext: &[u8]) -> Result<Vec<u8>, AppError> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);

    let ciphertext = cipher
        .encrypt(&nonce, plaintext)
        .map_err(|_| AppError::new(ErrorKind::CacheDecryptionFailed, "AES-256-GCM encryption failed"))?;

    let mut output = Vec::with_capacity(12 + ciphertext.len());
    output.extend_from_slice(&nonce);
    output.extend_from_slice(&ciphertext);
    Ok(output)
}

/* ============================================================================================== */
/// Decrypts a blob produced by [`encrypt`].
///
/// Expects input format: `[nonce (12 bytes)][ciphertext + auth tag]`.
///
/// # Errors
/// Returns [`AppError`] with [`ErrorKind::CacheDecryptionFailed`] if the key
/// is wrong, the data is tampered, or the input is too short.
pub fn decrypt(key: &[u8; 32], data: &[u8]) -> Result<Vec<u8>, AppError> {
    if data.len() < 12 {
        return Err(AppError::new(
            ErrorKind::CacheDecryptionFailed,
            "Encrypted data is too short (missing nonce)",
        ));
    }
    let (nonce_bytes, ciphertext) = data.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));

    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| {
            AppError::new(
                ErrorKind::CacheDecryptionFailed,
                "AES-256-GCM decryption failed - wrong password or corrupted data",
            )
            .with_recovery(crate::errors::RecoveryAction::Manual(
                "Re-enter your master password, or delete the cache file to start fresh.".into(),
            ))
        })
}

/* ============================================================================================== */
/*                                            Tests                                               */
/* ============================================================================================== */

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypt_decrypt_round_trip() {
        let key = [0xABu8; 32];
        let plaintext = b"hello, aztui!";
        let encrypted = encrypt(&key, plaintext).unwrap();
        let decrypted = decrypt(&key, &encrypted).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn decrypt_wrong_key_fails() {
        let key_a = [0xABu8; 32];
        let key_b = [0xCDu8; 32];
        let encrypted = encrypt(&key_a, b"secret").unwrap();
        let result = decrypt(&key_b, &encrypted);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind, ErrorKind::CacheDecryptionFailed);
    }

    #[test]
    fn decrypt_tampered_data_fails() {
        let key = [0xABu8; 32];
        let mut encrypted = encrypt(&key, b"secret").unwrap();
        // Flip a byte in the ciphertext portion.
        if let Some(byte) = encrypted.last_mut() {
            *byte ^= 0xFF;
        }
        assert!(decrypt(&key, &encrypted).is_err());
    }

    #[test]
    fn decrypt_too_short_fails() {
        let key = [0xABu8; 32];
        assert!(decrypt(&key, &[0u8; 5]).is_err());
    }
}