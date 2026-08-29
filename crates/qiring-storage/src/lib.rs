use anyhow::Context;
use atomic_write_file::AtomicWriteFile;
use chrono::{DateTime, Utc};
use qiring_crypto::{
    decrypt, decrypt_with_aad, encrypt, encrypt_with_aad, CipherBlob, KdfParams, KEY_LEN, NONCE_LEN, SALT_LEN,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::Path;
use thiserror::Error;
use uuid::Uuid;

pub const SCHEMA_VERSION: u32 = 2;
pub const LEGACY_SCHEMA_VERSION: u32 = 1;
pub const MAX_VAULT_FILE_BYTES: u64 = 64 * 1024 * 1024;
const AEAD_TAG_LEN: usize = 16;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KdfSlot {
    pub params: KdfParams,
    pub salt: Vec<u8>,
}

impl KdfSlot {
    pub fn validate(&self) -> anyhow::Result<()> {
        if self.salt.len() != SALT_LEN {
            anyhow::bail!("invalid KDF salt length");
        }
        self.params.validate()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultMetadata {
    pub vault_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub schema_version: u32,
    pub master_kdf: KdfSlot,
    pub recovery_kdf: KdfSlot,
}

impl VaultMetadata {
    pub fn validate(&self) -> anyhow::Result<()> {
        if self.schema_version != SCHEMA_VERSION {
            anyhow::bail!("unsupported schema version: {}", self.schema_version);
        }
        self.master_kdf.validate()?;
        self.recovery_kdf.validate()?;
        if self.master_kdf.salt == self.recovery_kdf.salt {
            anyhow::bail!("master and recovery KDF salts must be independent");
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegacyVaultMetadata {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegacyEncryptedVault {
    pub metadata: LegacyVaultMetadata,
    pub wrapped_keys: WrappedKeys,
    pub vault_blob: CipherBlob,
}

#[derive(Debug, Clone)]
pub enum VaultFile {
    Current(EncryptedVault),
    Legacy(LegacyEncryptedVault),
}

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("Ring data is corrupt or unreadable")]
    Corrupt,
    #[error("Ring file exceeds the maximum supported size")]
    TooLarge,
    #[error("Ring path must not be a symbolic link")]
    Symlink,
}

pub fn new_metadata(master_kdf: KdfSlot, recovery_kdf: KdfSlot) -> VaultMetadata {
    VaultMetadata {
        vault_id: Uuid::new_v4(),
        created_at: Utc::now(),
        schema_version: SCHEMA_VERSION,
        master_kdf,
        recovery_kdf,
    }
}

pub fn metadata_aad(metadata: &VaultMetadata, purpose: &str) -> anyhow::Result<Vec<u8>> {
    #[derive(Serialize)]
    struct AuthenticatedMetadata<'a> {
        format: &'static str,
        purpose: &'a str,
        vault_id: Uuid,
        created_at: DateTime<Utc>,
        schema_version: u32,
        master_kdf: Option<&'a KdfSlot>,
        recovery_kdf: Option<&'a KdfSlot>,
    }

    metadata.validate()?;
    let (master_kdf, recovery_kdf) = match purpose {
        "master-wrapped-dek" => (Some(&metadata.master_kdf), None),
        "recovery-wrapped-dek" => (None, Some(&metadata.recovery_kdf)),
        "vault-payload" => (Some(&metadata.master_kdf), Some(&metadata.recovery_kdf)),
        _ => anyhow::bail!("unknown authenticated metadata purpose"),
    };
    serde_json::to_vec(&AuthenticatedMetadata {
        format: "qiring-vault-v2",
        purpose,
        vault_id: metadata.vault_id,
        created_at: metadata.created_at,
        schema_version: metadata.schema_version,
        master_kdf,
        recovery_kdf,
    })
    .context("serialize authenticated Ring metadata")
}

pub fn encrypt_vault_payload(
    payload: &[u8],
    dek: &[u8; KEY_LEN],
    metadata: &VaultMetadata,
) -> anyhow::Result<CipherBlob> {
    let aad = metadata_aad(metadata, "vault-payload")?;
    encrypt_with_aad(dek, payload, &aad).map_err(Into::into)
}

pub fn decrypt_vault_payload(
    vault_blob: &CipherBlob,
    dek: &[u8; KEY_LEN],
    metadata: &VaultMetadata,
) -> anyhow::Result<Vec<u8>> {
    let aad = metadata_aad(metadata, "vault-payload")?;
    decrypt_with_aad(dek, vault_blob, &aad).map_err(|_| StorageError::Corrupt.into())
}

pub fn encrypt_legacy_vault_payload(payload: &[u8], dek: &[u8; KEY_LEN]) -> anyhow::Result<CipherBlob> {
    encrypt(dek, payload).map_err(Into::into)
}

pub fn decrypt_legacy_vault_payload(vault_blob: &CipherBlob, dek: &[u8; KEY_LEN]) -> anyhow::Result<Vec<u8>> {
    decrypt(dek, vault_blob).map_err(|_| StorageError::Corrupt.into())
}

pub fn save_encrypted_vault(path: &Path, vault: &EncryptedVault) -> anyhow::Result<()> {
    vault.metadata.validate()?;
    let json = serde_json::to_vec_pretty(vault).context("failed to serialize Ring")?;
    if json.len() as u64 > MAX_VAULT_FILE_BYTES {
        return Err(StorageError::TooLarge.into());
    }
    save_bytes_atomic(path, &json)
}

/// Writes `bytes` atomically to `path`, forcing the parent directory to be
/// private (`0700` on Unix). Use this only for QiRing-owned app-data paths
/// (the vault file, its window-state sibling, and internal safety-snapshot
/// directories) — never for a directory the user selected themselves, since
/// silently narrowing its permissions is a surprising side effect on a
/// location the user may share with other tools or users.
pub fn save_bytes_atomic(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    let parent = path.parent().context("Ring path must have parent directory")?;
    ensure_private_directory(parent)?;
    save_bytes_atomic_at_existing_directory(path, bytes)
}

/// Writes `bytes` atomically to `path` without changing the permissions of
/// the parent directory. Use this for files written into a directory the
/// user chose explicitly (backup exports, automatic snapshots, the recovery
/// key text file). The parent directory is created if missing but its mode
/// is left as the filesystem default; the written file itself is still
/// restricted to the current user (`0600` on Unix).
pub fn save_bytes_atomic_user_directory(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    let parent = path.parent().context("path must have parent directory")?;
    fs::create_dir_all(parent).context("failed to create backup directory")?;
    save_bytes_atomic_at_existing_directory(path, bytes)
}

fn save_bytes_atomic_at_existing_directory(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    let parent = path.parent().context("Ring path must have parent directory")?;
    reject_symlink(path)?;

    let options = AtomicWriteFile::options();
    #[cfg(unix)]
    let options = {
        use atomic_write_file::unix::OpenOptionsExt as AtomicOpenOptionsExt;
        use std::os::unix::fs::OpenOptionsExt as StdOpenOptionsExt;

        let mut options = options;
        AtomicOpenOptionsExt::preserve_mode(&mut options, false);
        StdOpenOptionsExt::mode(&mut options, 0o600);
        options
    };

    let mut file = options.open(path).context("failed to open atomic Ring writer")?;
    file.write_all(bytes)
        .context("failed to write complete Ring file")?;
    file.flush().context("failed to flush Ring file")?;
    file.commit().context("failed to atomically commit Ring file")?;
    enforce_private_file(path)?;
    sync_parent_directory(parent)
}

pub fn load_encrypted_vault(path: &Path) -> anyhow::Result<EncryptedVault> {
    match load_vault_file(path)? {
        VaultFile::Current(vault) => Ok(vault),
        VaultFile::Legacy(_) => anyhow::bail!("legacy Ring must be migrated before use"),
    }
}

pub fn load_vault_file(path: &Path) -> anyhow::Result<VaultFile> {
    let data = read_bounded(path, MAX_VAULT_FILE_BYTES)?;
    parse_vault_bytes(&data)
}

pub fn parse_vault_bytes(bytes: &[u8]) -> anyhow::Result<VaultFile> {
    if bytes.len() as u64 > MAX_VAULT_FILE_BYTES {
        return Err(StorageError::TooLarge.into());
    }
    let value: serde_json::Value = serde_json::from_slice(bytes).map_err(|_| StorageError::Corrupt)?;
    let version = value
        .get("metadata")
        .and_then(|metadata| metadata.get("schema_version"))
        .and_then(serde_json::Value::as_u64)
        .ok_or(StorageError::Corrupt)?;

    match version as u32 {
        SCHEMA_VERSION => {
            let vault: EncryptedVault = serde_json::from_value(value).map_err(|_| StorageError::Corrupt)?;
            vault.metadata.validate().map_err(|_| StorageError::Corrupt)?;
            validate_cipher_shapes(&vault.wrapped_keys, &vault.vault_blob)?;
            Ok(VaultFile::Current(vault))
        }
        LEGACY_SCHEMA_VERSION => {
            let vault: LegacyEncryptedVault =
                serde_json::from_value(value).map_err(|_| StorageError::Corrupt)?;
            vault
                .metadata
                .kdf_params
                .validate()
                .map_err(|_| StorageError::Corrupt)?;
            if vault.metadata.salt.len() != SALT_LEN {
                return Err(StorageError::Corrupt.into());
            }
            validate_cipher_shapes(&vault.wrapped_keys, &vault.vault_blob)?;
            Ok(VaultFile::Legacy(vault))
        }
        other => anyhow::bail!("unsupported schema version: {other}"),
    }
}

fn validate_cipher_shapes(wrapped: &WrappedKeys, vault_blob: &CipherBlob) -> anyhow::Result<()> {
    let wrapped_is_valid =
        |blob: &CipherBlob| blob.nonce.len() == NONCE_LEN && blob.ciphertext.len() == KEY_LEN + AEAD_TAG_LEN;
    if !wrapped_is_valid(&wrapped.wrapped_dek_by_master)
        || !wrapped_is_valid(&wrapped.wrapped_dek_by_recovery)
        || vault_blob.nonce.len() != NONCE_LEN
        || vault_blob.ciphertext.len() < AEAD_TAG_LEN
        || vault_blob.ciphertext.len() as u64 > MAX_VAULT_FILE_BYTES
    {
        return Err(StorageError::Corrupt.into());
    }
    Ok(())
}

pub fn read_bounded(path: &Path, maximum: u64) -> anyhow::Result<Vec<u8>> {
    reject_symlink(path)?;
    let metadata = fs::metadata(path).context("failed to inspect Ring file")?;
    if metadata.len() > maximum {
        return Err(StorageError::TooLarge.into());
    }
    fs::read(path).context("failed to read Ring file")
}

pub fn ensure_private_directory(path: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(path).context("failed to create private directory")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .context("failed to restrict private directory permissions")?;
    }
    Ok(())
}

fn enforce_private_file(path: &Path) -> anyhow::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))
            .context("failed to restrict Ring file permissions")?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

fn sync_parent_directory(path: &Path) -> anyhow::Result<()> {
    #[cfg(unix)]
    {
        fs::File::open(path)
            .context("failed to open parent directory for sync")?
            .sync_all()
            .context("failed to sync parent directory")?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

fn reject_symlink(path: &Path) -> anyhow::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(StorageError::Symlink.into()),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).context("failed to inspect storage path"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use qiring_crypto::{random_dek, random_salt, wrap_dek_with_aad, KdfParams};

    fn sample_vault() -> EncryptedVault {
        let dek = random_dek();
        let metadata = new_metadata(
            KdfSlot {
                params: KdfParams::default(),
                salt: random_salt().to_vec(),
            },
            KdfSlot {
                params: KdfParams::default(),
                salt: random_salt().to_vec(),
            },
        );
        let encrypted = encrypt_vault_payload(br#"{}"#, &dek, &metadata).expect("encrypt");
        let master_aad = metadata_aad(&metadata, "master-wrapped-dek").expect("aad");
        let recovery_aad = metadata_aad(&metadata, "recovery-wrapped-dek").expect("aad");
        EncryptedVault {
            metadata,
            wrapped_keys: WrappedKeys {
                wrapped_dek_by_master: wrap_dek_with_aad(&dek, &dek, &master_aad).expect("wrap"),
                wrapped_dek_by_recovery: wrap_dek_with_aad(&dek, &dek, &recovery_aad).expect("wrap"),
            },
            vault_blob: encrypted,
        }
    }

    #[test]
    fn encrypt_decrypt_payload_round_trip() {
        let vault = sample_vault();
        let dek = random_dek();
        let blob = encrypt_vault_payload(br#"{\"items\":[]}"#, &dek, &vault.metadata).expect("encrypt");
        let clear = decrypt_vault_payload(&blob, &dek, &vault.metadata).expect("decrypt");
        assert_eq!(clear, br#"{\"items\":[]}"#);
    }

    #[test]
    fn metadata_tampering_breaks_payload_authentication() {
        let dek = random_dek();
        let original = sample_vault().metadata;
        let blob = encrypt_vault_payload(b"secret", &dek, &original).expect("encrypt");
        let mut mutations = Vec::new();

        let mut changed = original.clone();
        changed.vault_id = Uuid::new_v4();
        mutations.push(changed);
        let mut changed = original.clone();
        changed.created_at += chrono::Duration::seconds(1);
        mutations.push(changed);
        let mut changed = original.clone();
        changed.schema_version += 1;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.master_kdf.params.iterations += 1;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.recovery_kdf.params.iterations += 1;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.master_kdf.salt[0] ^= 0x80;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.recovery_kdf.salt[0] ^= 0x80;
        mutations.push(changed);

        for metadata in mutations {
            assert!(decrypt_vault_payload(&blob, &dek, &metadata).is_err());
        }
    }

    #[test]
    fn save_and_load_round_trip_with_private_permissions() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("vault.qiring");
        save_encrypted_vault(&path, &sample_vault()).expect("save");
        let loaded = load_encrypted_vault(&path).expect("load");
        assert_eq!(loaded.metadata.schema_version, SCHEMA_VERSION);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(path).expect("metadata").permissions().mode() & 0o777,
                0o600
            );
        }
    }

    #[test]
    fn rejects_oversized_vault_before_parsing() {
        let bytes = vec![b'x'; (MAX_VAULT_FILE_BYTES + 1) as usize];
        assert!(parse_vault_bytes(&bytes).is_err());
    }

    #[test]
    fn refuses_to_save_a_vault_that_cannot_be_loaded() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("vault.qiring");
        save_encrypted_vault(&path, &sample_vault()).expect("save valid vault");
        let original = fs::read(&path).expect("read valid vault");
        let mut vault = sample_vault();
        // Pretty-printed byte arrays require several output bytes per input byte.
        vault.vault_blob.ciphertext = vec![10; (MAX_VAULT_FILE_BYTES / 4) as usize];

        let error = save_encrypted_vault(&path, &vault).expect_err("oversized save");
        assert!(error
            .downcast_ref::<StorageError>()
            .is_some_and(|error| matches!(error, StorageError::TooLarge)));
        assert_eq!(fs::read(&path).expect("read preserved vault"), original);
        load_encrypted_vault(&path).expect("load preserved vault");
    }

    #[test]
    fn rejects_truncated_vault_and_tampered_wrapped_key_shape() {
        let vault = sample_vault();
        let bytes = serde_json::to_vec(&vault).expect("serialize");
        assert!(parse_vault_bytes(&bytes[..bytes.len() / 2]).is_err());

        let mut malformed = vault;
        malformed.wrapped_keys.wrapped_dek_by_master.nonce.pop();
        let bytes = serde_json::to_vec(&malformed).expect("serialize malformed");
        assert!(parse_vault_bytes(&bytes).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn atomic_writer_rejects_symlinks_without_touching_the_target() {
        use std::os::unix::fs::symlink;

        let dir = tempfile::tempdir().expect("tempdir");
        let target = dir.path().join("target");
        let link = dir.path().join("vault.qiring");
        fs::write(&target, b"original").expect("write target");
        symlink(&target, &link).expect("create symlink");
        assert!(save_bytes_atomic(&link, b"replacement").is_err());
        assert_eq!(fs::read(target).expect("read target"), b"original");
    }
}
