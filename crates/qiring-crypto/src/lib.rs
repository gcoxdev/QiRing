use argon2::{password_hash::SaltString, Argon2, Params};
use chacha20poly1305::{
    aead::{Aead, KeyInit},
    XChaCha20Poly1305, XNonce,
};
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use zeroize::Zeroizing;

pub const SALT_LEN: usize = 16;
pub const NONCE_LEN: usize = 24;
pub const KEY_LEN: usize = 32;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KdfParams {
    pub memory_cost_kib: u32,
    pub iterations: u32,
    pub parallelism: u32,
}

impl Default for KdfParams {
    fn default() -> Self {
        Self {
            memory_cost_kib: 64 * 1024,
            iterations: 3,
            parallelism: 1,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CipherBlob {
    pub nonce: Vec<u8>,
    pub ciphertext: Vec<u8>,
}

#[derive(Debug, Error)]
pub enum CryptoError {
    #[error("encryption failure")]
    Encrypt,
    #[error("decryption failure")]
    Decrypt,
    #[error("invalid length")]
    InvalidLength,
}

pub fn random_bytes(len: usize) -> Vec<u8> {
    let mut out = vec![0u8; len];
    OsRng.fill_bytes(&mut out);
    out
}

pub fn derive_kek(
    master_password: &str,
    salt: &[u8],
    params: &KdfParams,
) -> anyhow::Result<Zeroizing<[u8; KEY_LEN]>> {
    if salt.len() != SALT_LEN {
        anyhow::bail!("salt must be {SALT_LEN} bytes");
    }

    let p = Params::new(
        params.memory_cost_kib,
        params.iterations,
        params.parallelism,
        Some(KEY_LEN),
    )
    .map_err(|e| anyhow::anyhow!("invalid argon2 parameters: {e}"))?;
    let argon2 = Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, p);

    let mut out = Zeroizing::new([0u8; KEY_LEN]);
    argon2
        .hash_password_into(master_password.as_bytes(), salt, out.as_mut())
        .map_err(|e| anyhow::anyhow!("argon2 derivation failed: {e}"))?;
    Ok(out)
}

pub fn derive_recovery_kek(
    recovery_key: &str,
    salt: &[u8],
    params: &KdfParams,
) -> anyhow::Result<Zeroizing<[u8; KEY_LEN]>> {
    derive_kek(recovery_key, salt, params)
}

pub fn generate_recovery_key() -> String {
    let entropy = random_bytes(24);
    base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, entropy)
}

pub fn encrypt(key: &[u8], plaintext: &[u8]) -> Result<CipherBlob, CryptoError> {
    if key.len() != KEY_LEN {
        return Err(CryptoError::InvalidLength);
    }
    let cipher = XChaCha20Poly1305::new_from_slice(key).map_err(|_| CryptoError::InvalidLength)?;
    let nonce = random_bytes(NONCE_LEN);
    let ciphertext = cipher
        .encrypt(XNonce::from_slice(&nonce), plaintext)
        .map_err(|_| CryptoError::Encrypt)?;
    Ok(CipherBlob { nonce, ciphertext })
}

pub fn decrypt(key: &[u8], blob: &CipherBlob) -> Result<Vec<u8>, CryptoError> {
    if key.len() != KEY_LEN {
        return Err(CryptoError::InvalidLength);
    }
    if blob.nonce.len() != NONCE_LEN {
        return Err(CryptoError::InvalidLength);
    }
    let cipher = XChaCha20Poly1305::new_from_slice(key).map_err(|_| CryptoError::InvalidLength)?;
    cipher
        .decrypt(XNonce::from_slice(&blob.nonce), blob.ciphertext.as_ref())
        .map_err(|_| CryptoError::Decrypt)
}

pub fn wrap_dek(kek: &[u8], dek: &[u8; KEY_LEN]) -> Result<CipherBlob, CryptoError> {
    encrypt(kek, dek)
}

pub fn unwrap_dek(kek: &[u8], wrapped: &CipherBlob) -> Result<[u8; KEY_LEN], CryptoError> {
    let plain = decrypt(kek, wrapped)?;
    if plain.len() != KEY_LEN {
        return Err(CryptoError::InvalidLength);
    }

    let mut dek = [0u8; KEY_LEN];
    dek.copy_from_slice(&plain);
    Ok(dek)
}

pub fn random_salt() -> [u8; SALT_LEN] {
    let mut salt = [0u8; SALT_LEN];
    OsRng.fill_bytes(&mut salt);
    salt
}

pub fn random_dek() -> [u8; KEY_LEN] {
    let mut dek = [0u8; KEY_LEN];
    OsRng.fill_bytes(&mut dek);
    dek
}

pub fn default_salt_string() -> SaltString {
    SaltString::generate(&mut OsRng)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypt_round_trip() {
        let key = random_dek();
        let msg = b"qiring-secret";
        let blob = encrypt(&key, msg).expect("encrypt");
        let plain = decrypt(&key, &blob).expect("decrypt");
        assert_eq!(plain, msg);
    }

    #[test]
    fn wrap_unwrap_round_trip() {
        let kek = random_dek();
        let dek = random_dek();
        let wrapped = wrap_dek(&kek, &dek).expect("wrap");
        let unwrapped = unwrap_dek(&kek, &wrapped).expect("unwrap");
        assert_eq!(unwrapped, dek);
    }

    #[test]
    fn kdf_is_deterministic_for_same_input() {
        let params = KdfParams::default();
        let salt = random_salt();
        let one = derive_kek("master", &salt, &params).expect("kdf1");
        let two = derive_kek("master", &salt, &params).expect("kdf2");
        assert_eq!(one.as_slice(), two.as_slice());
    }
}
