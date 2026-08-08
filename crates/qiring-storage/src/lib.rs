use anyhow::Context;
use chrono::{DateTime, Utc};
use qiring_crypto::{decrypt, encrypt, CipherBlob, KdfParams, KEY_LEN};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;
use uuid::Uuid;

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultMetadata {
    pub vault_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub schema_version: u32,
    pub kdf_params: KdfParams,
    pub salt: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WrappedKeys {
    pub wrapped_dek_by_master: CipherBlob,
    pub wrapped_dek_by_recovery: CipherBlob,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedVault {
    pub metadata: VaultMetadata,
    pub wrapped_keys: WrappedKeys,
    pub vault_blob: CipherBlob,
}

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("vault data is corrupt or unreadable")]
    Corrupt,
}

pub fn new_metadata(kdf_params: KdfParams, salt: Vec<u8>) -> VaultMetadata {
    VaultMetadata {
        vault_id: Uuid::new_v4(),
        created_at: Utc::now(),
        schema_version: SCHEMA_VERSION,
        kdf_params,
        salt,
    }
}

pub fn encrypt_vault_payload(payload: &[u8], dek: &[u8; KEY_LEN]) -> anyhow::Result<CipherBlob> {
    encrypt(dek, payload).map_err(Into::into)
}

pub fn decrypt_vault_payload(vault_blob: &CipherBlob, dek: &[u8; KEY_LEN]) -> anyhow::Result<Vec<u8>> {
    decrypt(dek, vault_blob).map_err(|_| StorageError::Corrupt.into())
}

pub fn save_encrypted_vault(path: &Path, vault: &EncryptedVault) -> anyhow::Result<()> {
    let parent = path.parent().context("vault path must have parent directory")?;
    fs::create_dir_all(parent).context("failed to create vault directory")?;

    let temp_path = temp_path(path);
    let json = serde_json::to_vec_pretty(vault).context("failed to serialize vault")?;

    fs::write(&temp_path, json).context("failed to write temp vault file")?;
    fs::rename(&temp_path, path).context("failed to atomically replace vault file")?;

    Ok(())
}

pub fn load_encrypted_vault(path: &Path) -> anyhow::Result<EncryptedVault> {
    let data = fs::read(path).context("failed to read vault file")?;
    let parsed: EncryptedVault = serde_json::from_slice(&data).map_err(|_| StorageError::Corrupt)?;
    if parsed.metadata.schema_version != SCHEMA_VERSION {
        anyhow::bail!("unsupported schema version: {}", parsed.metadata.schema_version);
    }
    Ok(parsed)
}

fn temp_path(path: &Path) -> PathBuf {
    let mut p = path.as_os_str().to_os_string();
    p.push(".tmp");
    PathBuf::from(p)
}

#[cfg(test)]
mod tests {
    use super::*;
    use qiring_crypto::{random_dek, KdfParams};

    #[test]
    fn encrypt_decrypt_payload_round_trip() {
        let dek = random_dek();
        let payload = br#"{\"items\":[]}"#;
        let blob = encrypt_vault_payload(payload, &dek).expect("encrypt");
        let clear = decrypt_vault_payload(&blob, &dek).expect("decrypt");
        assert_eq!(clear, payload);
    }

    #[test]
    fn save_and_load_round_trip() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("vault.qiring");

        let dek = random_dek();
        let encrypted = encrypt_vault_payload(br#"{}"#, &dek).expect("encrypt");
        let wrapped = qiring_crypto::wrap_dek(&dek, &dek).expect("wrap");

        let vault = EncryptedVault {
            metadata: new_metadata(KdfParams::default(), vec![0u8; 16]),
            wrapped_keys: WrappedKeys {
                wrapped_dek_by_master: wrapped.clone(),
                wrapped_dek_by_recovery: wrapped,
            },
            vault_blob: encrypted,
        };

        save_encrypted_vault(&path, &vault).expect("save");
        let loaded = load_encrypted_vault(&path).expect("load");
        assert_eq!(loaded.metadata.schema_version, SCHEMA_VERSION);
    }
}
