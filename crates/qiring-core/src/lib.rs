use anyhow::Context;
use chrono::{DateTime, Utc};
use qiring_crypto::{
    derive_kek, derive_recovery_kek, generate_recovery_key, random_dek, random_salt, unwrap_dek, wrap_dek,
    KdfParams, KEY_LEN,
};
use qiring_storage::{
    decrypt_vault_payload, encrypt_vault_payload, load_encrypted_vault, new_metadata, save_encrypted_vault,
    EncryptedVault, WrappedKeys,
};
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;
use uuid::Uuid;
use zeroize::Zeroize;

pub const COMMAND_VERSION: &str = "1";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VaultItemType {
    Login,
    SecureNote,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultItem {
    pub id: Uuid,
    pub item_type: VaultItemType,
    pub title: String,
    pub username: Option<String>,
    pub password: Option<String>,
    pub url: Option<String>,
    pub notes: Option<String>,
    pub tags: Vec<String>,
    pub folder: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub auto_lock_minutes: u32,
    pub clipboard_clear_seconds: u32,
    pub biometric_enabled: bool,
    pub theme: String,
    pub backup_preferences: BackupPreferences,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            auto_lock_minutes: 5,
            clipboard_clear_seconds: 30,
            biometric_enabled: true,
            theme: "system".to_string(),
            backup_preferences: BackupPreferences::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupPreferences {
    pub include_settings: bool,
}

impl Default for BackupPreferences {
    fn default() -> Self {
        Self {
            include_settings: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct VaultDocument {
    pub items: HashMap<Uuid, VaultItem>,
    pub settings: AppSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EncryptedBackupFile {
    schema_version: u32,
    created_at: DateTime<Utc>,
    salt: Vec<u8>,
    kdf_params: KdfParams,
    blob: qiring_crypto::CipherBlob,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultSummary {
    pub vault_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub schema_version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryMaterial {
    pub recovery_key: String,
    pub recovery_key_fingerprint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionToken {
    pub token: String,
    pub unlocked_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemInput {
    pub item_type: VaultItemType,
    pub title: String,
    pub username: Option<String>,
    pub password: Option<String>,
    pub url: Option<String>,
    pub notes: Option<String>,
    pub tags: Vec<String>,
    pub folder: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ItemPatch {
    pub title: Option<String>,
    pub username: Option<Option<String>>,
    pub password: Option<Option<String>>,
    pub url: Option<Option<String>>,
    pub notes: Option<Option<String>>,
    pub tags: Option<Vec<String>>,
    pub folder: Option<Option<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ListFilter {
    pub query: Option<String>,
    pub tag: Option<String>,
    pub folder: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemSummary {
    pub id: Uuid,
    pub item_type: VaultItemType,
    pub title: String,
    pub username: Option<String>,
    pub tags: Vec<String>,
    pub folder: Option<String>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PasswordPolicy {
    pub length: usize,
    pub include_upper: bool,
    pub include_lower: bool,
    pub include_numbers: bool,
    pub include_symbols: bool,
}

impl Default for PasswordPolicy {
    fn default() -> Self {
        Self {
            length: 20,
            include_upper: true,
            include_lower: true,
            include_numbers: true,
            include_symbols: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedPassword {
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupManifest {
    pub path: String,
    pub created_at: DateTime<Utc>,
    pub schema_version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportReport {
    pub imported_items: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityStatus {
    pub schema_version: u32,
    pub command_version: String,
    pub biometric_enabled: bool,
    pub auto_lock_minutes: u32,
    pub clipboard_clear_seconds: u32,
}

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("vault is locked")]
    VaultLocked,
    #[error("item not found")]
    ItemNotFound,
    #[error("invalid input")]
    InvalidInput,
    #[error("authentication failed")]
    AuthenticationFailed,
}

#[derive(Clone)]
struct UnlockedSession {
    dek: [u8; KEY_LEN],
    doc: VaultDocument,
}

impl Drop for UnlockedSession {
    fn drop(&mut self) {
        self.dek.zeroize();
    }
}

pub struct VaultService {
    vault_path: PathBuf,
    session: Option<UnlockedSession>,
}

impl VaultService {
    pub fn new(vault_path: impl AsRef<Path>) -> Self {
        Self {
            vault_path: vault_path.as_ref().to_path_buf(),
            session: None,
        }
    }

    pub fn create_vault(
        &mut self,
        master_password: &str,
        settings: AppSettings,
    ) -> anyhow::Result<(VaultSummary, RecoveryMaterial)> {
        if master_password.len() < 12 {
            return Err(CoreError::InvalidInput.into());
        }

        let recovery_key = generate_recovery_key();
        let recovery_fingerprint = recovery_key.chars().take(8).collect::<String>();
        let salt = random_salt();
        let params = KdfParams::default();

        let kek = derive_kek(master_password, &salt, &params)?;
        let recovery_kek = derive_recovery_kek(&recovery_key, &salt, &params)?;
        let dek = random_dek();

        let wrapped_dek_by_master = wrap_dek(kek.as_ref(), &dek)?;
        let wrapped_dek_by_recovery = wrap_dek(recovery_kek.as_ref(), &dek)?;

        let metadata = new_metadata(params.clone(), salt.to_vec());
        let doc = VaultDocument {
            items: HashMap::new(),
            settings,
        };
        let bytes = serde_json::to_vec(&doc).context("serialize vault document")?;
        let vault_blob = encrypt_vault_payload(&bytes, &dek)?;

        let encrypted = EncryptedVault {
            metadata: metadata.clone(),
            wrapped_keys: WrappedKeys {
                wrapped_dek_by_master,
                wrapped_dek_by_recovery,
            },
            vault_blob,
        };
        save_encrypted_vault(&self.vault_path, &encrypted)?;

        Ok((
            VaultSummary {
                vault_id: metadata.vault_id,
                created_at: metadata.created_at,
                schema_version: metadata.schema_version,
            },
            RecoveryMaterial {
                recovery_key,
                recovery_key_fingerprint: recovery_fingerprint,
            },
        ))
    }

    pub fn unlock_vault_master(&mut self, master_password: &str) -> anyhow::Result<SessionToken> {
        let encrypted = load_encrypted_vault(&self.vault_path)?;
        let salt = encrypted.metadata.salt.clone();
        let kek = derive_kek(master_password, &salt, &encrypted.metadata.kdf_params)
            .map_err(|_| CoreError::AuthenticationFailed)?;
        let dek = unwrap_dek(kek.as_ref(), &encrypted.wrapped_keys.wrapped_dek_by_master)
            .map_err(|_| CoreError::AuthenticationFailed)?;
        let clear = decrypt_vault_payload(&encrypted.vault_blob, &dek)
            .map_err(|_| CoreError::AuthenticationFailed)?;
        let doc: VaultDocument = serde_json::from_slice(&clear).context("deserialize document")?;

        let token = SessionToken {
            token: Uuid::new_v4().to_string(),
            unlocked_at: Utc::now(),
        };

        self.session = Some(UnlockedSession { dek, doc });

        Ok(token)
    }

    pub fn unlock_vault_biometric(&mut self) -> anyhow::Result<SessionToken> {
        Err(CoreError::AuthenticationFailed.into())
    }

    pub fn lock_vault(&mut self) {
        self.session = None;
    }

    pub fn add_item(&mut self, input: ItemInput) -> anyhow::Result<Uuid> {
        let session = self.session_mut()?;
        let now = Utc::now();
        let id = Uuid::new_v4();

        let item = VaultItem {
            id,
            item_type: input.item_type,
            title: input.title,
            username: input.username,
            password: input.password,
            url: input.url,
            notes: input.notes,
            tags: input.tags,
            folder: input.folder,
            created_at: now,
            updated_at: now,
        };

        session.doc.items.insert(id, item);
        self.flush()?;
        Ok(id)
    }

    pub fn update_item(&mut self, item_id: Uuid, patch: ItemPatch) -> anyhow::Result<()> {
        let session = self.session_mut()?;
        let item = session
            .doc
            .items
            .get_mut(&item_id)
            .ok_or(CoreError::ItemNotFound)?;

        if let Some(title) = patch.title {
            item.title = title;
        }
        if let Some(username) = patch.username {
            item.username = username;
        }
        if let Some(password) = patch.password {
            item.password = password;
        }
        if let Some(url) = patch.url {
            item.url = url;
        }
        if let Some(notes) = patch.notes {
            item.notes = notes;
        }
        if let Some(tags) = patch.tags {
            item.tags = tags;
        }
        if let Some(folder) = patch.folder {
            item.folder = folder;
        }

        item.updated_at = Utc::now();
        self.flush()?;
        Ok(())
    }

    pub fn delete_item(&mut self, item_id: Uuid) -> anyhow::Result<()> {
        let session = self.session_mut()?;
        session
            .doc
            .items
            .remove(&item_id)
            .ok_or(CoreError::ItemNotFound)?;
        self.flush()?;
        Ok(())
    }

    pub fn list_items(&self, filter: ListFilter) -> anyhow::Result<Vec<ItemSummary>> {
        let session = self.session_ref()?;
        let query = filter.query.unwrap_or_default().to_lowercase();

        let mut out = session
            .doc
            .items
            .values()
            .filter(|item| {
                if !query.is_empty() {
                    let title = item.title.to_lowercase();
                    let username = item.username.clone().unwrap_or_default().to_lowercase();
                    let notes = item.notes.clone().unwrap_or_default().to_lowercase();
                    if !title.contains(&query) && !username.contains(&query) && !notes.contains(&query) {
                        return false;
                    }
                }

                if let Some(tag) = &filter.tag {
                    if !item.tags.iter().any(|t| t == tag) {
                        return false;
                    }
                }

                if let Some(folder) = &filter.folder {
                    if item.folder.as_deref() != Some(folder.as_str()) {
                        return false;
                    }
                }
                true
            })
            .map(|item| ItemSummary {
                id: item.id,
                item_type: item.item_type.clone(),
                title: item.title.clone(),
                username: item.username.clone(),
                tags: item.tags.clone(),
                folder: item.folder.clone(),
                updated_at: item.updated_at,
            })
            .collect::<Vec<_>>();

        out.sort_by_key(|i| i.updated_at);
        out.reverse();
        Ok(out)
    }

    pub fn get_item(&self, item_id: Uuid) -> anyhow::Result<VaultItem> {
        let session = self.session_ref()?;
        session
            .doc
            .items
            .get(&item_id)
            .cloned()
            .ok_or(CoreError::ItemNotFound.into())
    }

    pub fn generate_password(&self, policy: PasswordPolicy) -> anyhow::Result<GeneratedPassword> {
        if policy.length < 8 {
            return Err(CoreError::InvalidInput.into());
        }

        let mut alphabet = String::new();
        if policy.include_lower {
            alphabet.push_str("abcdefghijklmnopqrstuvwxyz");
        }
        if policy.include_upper {
            alphabet.push_str("ABCDEFGHIJKLMNOPQRSTUVWXYZ");
        }
        if policy.include_numbers {
            alphabet.push_str("0123456789");
        }
        if policy.include_symbols {
            alphabet.push_str("!@#$%^&*()-_=+[]{};:,.?/");
        }

        if alphabet.is_empty() {
            return Err(CoreError::InvalidInput.into());
        }

        let chars: Vec<char> = alphabet.chars().collect();
        let mut rng = rand::thread_rng();
        let mut value = String::with_capacity(policy.length);
        for _ in 0..policy.length {
            let c = chars.choose(&mut rng).copied().ok_or(CoreError::InvalidInput)?;
            value.push(c);
        }

        Ok(GeneratedPassword { value })
    }

    pub fn export_backup(&self, path: impl AsRef<Path>, passphrase: &str) -> anyhow::Result<BackupManifest> {
        if passphrase.len() < 12 {
            return Err(CoreError::InvalidInput.into());
        }

        let vault_bytes = fs::read(&self.vault_path).context("read vault for backup")?;
        let backup_salt = random_salt();
        let kdf_params = KdfParams::default();
        let backup_key = derive_kek(passphrase, &backup_salt, &kdf_params)?;
        let blob = qiring_crypto::encrypt(backup_key.as_ref(), &vault_bytes)?;

        let backup = EncryptedBackupFile {
            schema_version: 1,
            created_at: Utc::now(),
            salt: backup_salt.to_vec(),
            kdf_params,
            blob,
        };
        let serialized = serde_json::to_vec_pretty(&backup).context("serialize backup")?;
        fs::write(path.as_ref(), serialized).context("write backup")?;

        Ok(BackupManifest {
            path: path.as_ref().display().to_string(),
            created_at: Utc::now(),
            schema_version: 1,
        })
    }

    pub fn import_backup(
        &mut self,
        path: impl AsRef<Path>,
        passphrase: &str,
    ) -> anyhow::Result<ImportReport> {
        let payload = fs::read(path).context("read backup")?;
        let backup: EncryptedBackupFile = serde_json::from_slice(&payload).context("parse backup")?;
        if backup.schema_version != 1 {
            return Err(CoreError::InvalidInput.into());
        }

        let backup_key = derive_kek(passphrase, &backup.salt, &backup.kdf_params)
            .map_err(|_| CoreError::AuthenticationFailed)?;
        let vault_raw = qiring_crypto::decrypt(backup_key.as_ref(), &backup.blob)
            .map_err(|_| CoreError::AuthenticationFailed)?;
        if load_encrypted_vault_from_bytes(&vault_raw).is_err() {
            return Err(CoreError::AuthenticationFailed.into());
        }
        fs::write(&self.vault_path, vault_raw).context("restore vault")?;

        Ok(ImportReport { imported_items: 0 })
    }

    pub fn rotate_master_password(&mut self, old_password: &str, new_password: &str) -> anyhow::Result<()> {
        if new_password.len() < 12 {
            return Err(CoreError::InvalidInput.into());
        }

        let mut encrypted = load_encrypted_vault(&self.vault_path)?;
        let salt = encrypted.metadata.salt.clone();
        let old_kek = derive_kek(old_password, &salt, &encrypted.metadata.kdf_params)
            .map_err(|_| CoreError::AuthenticationFailed)?;
        let dek = unwrap_dek(old_kek.as_ref(), &encrypted.wrapped_keys.wrapped_dek_by_master)
            .map_err(|_| CoreError::AuthenticationFailed)?;

        let new_kek = derive_kek(new_password, &salt, &encrypted.metadata.kdf_params)?;
        encrypted.wrapped_keys.wrapped_dek_by_master = wrap_dek(new_kek.as_ref(), &dek)?;
        save_encrypted_vault(&self.vault_path, &encrypted)?;
        Ok(())
    }

    pub fn get_security_status(&self) -> anyhow::Result<SecurityStatus> {
        let encrypted = load_encrypted_vault(&self.vault_path)?;
        let settings = self
            .session_ref()
            .map(|s| s.doc.settings.clone())
            .unwrap_or_default();

        Ok(SecurityStatus {
            schema_version: encrypted.metadata.schema_version,
            command_version: COMMAND_VERSION.to_string(),
            biometric_enabled: settings.biometric_enabled,
            auto_lock_minutes: settings.auto_lock_minutes,
            clipboard_clear_seconds: settings.clipboard_clear_seconds,
        })
    }

    fn session_mut(&mut self) -> anyhow::Result<&mut UnlockedSession> {
        self.session.as_mut().ok_or(CoreError::VaultLocked.into())
    }

    fn session_ref(&self) -> anyhow::Result<&UnlockedSession> {
        self.session.as_ref().ok_or(CoreError::VaultLocked.into())
    }

    fn flush(&mut self) -> anyhow::Result<()> {
        let session = self.session_ref()?.clone();
        let bytes = serde_json::to_vec(&session.doc).context("serialize")?;
        let vault_blob = encrypt_vault_payload(&bytes, &session.dek)?;

        let mut encrypted = load_encrypted_vault(&self.vault_path)?;
        encrypted.vault_blob = vault_blob;
        save_encrypted_vault(&self.vault_path, &encrypted)
    }
}

fn load_encrypted_vault_from_bytes(bytes: &[u8]) -> anyhow::Result<EncryptedVault> {
    let parsed: EncryptedVault = serde_json::from_slice(bytes).context("parse encrypted vault bytes")?;
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_service() -> (tempfile::TempDir, VaultService) {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("vault.qiring");
        (dir, VaultService::new(path))
    }

    #[test]
    fn create_unlock_add_list_cycle() {
        let (_dir, mut svc) = test_service();
        let settings = AppSettings::default();
        let _ = svc
            .create_vault("correct horse battery staple", settings)
            .expect("create");
        svc.unlock_vault_master("correct horse battery staple")
            .expect("unlock");

        let item_id = svc
            .add_item(ItemInput {
                item_type: VaultItemType::Login,
                title: "GitHub".to_string(),
                username: Some("user@example.com".to_string()),
                password: Some("secret".to_string()),
                url: Some("https://github.com".to_string()),
                notes: None,
                tags: vec!["dev".to_string()],
                folder: Some("work".to_string()),
            })
            .expect("add");

        let all = svc.list_items(ListFilter::default()).expect("list");
        assert_eq!(all.len(), 1);
        let one = svc.get_item(item_id).expect("get");
        assert_eq!(one.title, "GitHub");
    }

    #[test]
    fn wrong_password_fails_unlock() {
        let (_dir, mut svc) = test_service();
        svc.create_vault("correct horse battery staple", AppSettings::default())
            .expect("create");

        let err = svc.unlock_vault_master("wrong pass").expect_err("must fail");
        assert!(err.to_string().contains("authentication"));
    }

    #[test]
    fn password_generation_obeys_length() {
        let (_dir, svc) = test_service();
        let pwd = svc
            .generate_password(PasswordPolicy {
                length: 32,
                ..Default::default()
            })
            .expect("generate");
        assert_eq!(pwd.value.chars().count(), 32);
    }

    #[test]
    fn backup_export_import_round_trip() {
        let (dir, mut svc) = test_service();
        let backup_path = dir.path().join("vault.qiring.bak");

        svc.create_vault("correct horse battery staple", AppSettings::default())
            .expect("create");
        svc.unlock_vault_master("correct horse battery staple")
            .expect("unlock");
        svc.add_item(ItemInput {
            item_type: VaultItemType::SecureNote,
            title: "Server Root".to_string(),
            username: None,
            password: None,
            url: None,
            notes: Some("rotates monthly".to_string()),
            tags: vec!["ops".to_string()],
            folder: Some("infra".to_string()),
        })
        .expect("add");

        let _manifest = svc
            .export_backup(&backup_path, "backup passphrase 123")
            .expect("export");
        svc.import_backup(&backup_path, "backup passphrase 123")
            .expect("import");
    }
}
