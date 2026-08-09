use crate::model::*;
use crate::passwords::generate_password_value;
use crate::totp::current_totp_code;
use crate::validation::{
    validate_item_input, validate_item_patch, validate_master_password, validate_profile,
    validate_recovery_key, validate_settings,
};
use crate::COMMAND_VERSION;
use anyhow::Context;
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use qiring_crypto::{
    decrypt, decrypt_with_aad, derive_kek, derive_recovery_kek, encrypt_with_aad, generate_recovery_key,
    random_dek, random_salt, unwrap_dek, unwrap_dek_with_aad, wrap_dek_with_aad, KdfParams, KEY_LEN,
    NONCE_LEN,
};
use qiring_storage::{
    decrypt_legacy_vault_payload, decrypt_vault_payload, encrypt_vault_payload, load_encrypted_vault,
    load_vault_file, metadata_aad, new_metadata, parse_vault_bytes, read_bounded, save_bytes_atomic,
    save_bytes_atomic_user_directory, save_encrypted_vault, EncryptedVault, KdfSlot, LegacyEncryptedVault,
    VaultFile, VaultMetadata, WrappedKeys,
};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};
use uuid::Uuid;
use zeroize::{Zeroize, Zeroizing};

const MAX_BACKUP_FILE_BYTES: u64 = 128 * 1024 * 1024;
const PASSWORD_HISTORY_LIMIT: usize = 10;
const DELETED_ITEM_LIMIT: usize = 20;
const OLD_PASSWORD_DAYS: i64 = 180;

struct UnlockedSession {
    dek: Zeroizing<[u8; KEY_LEN]>,
    doc: VaultDocument,
    last_activity: Instant,
    last_activity_wall: SystemTime,
}

impl Drop for UnlockedSession {
    fn drop(&mut self) {
        for item in self.doc.items.values_mut() {
            zeroize_item(item);
        }
        for deleted in &mut self.doc.deleted_items {
            zeroize_item(&mut deleted.item);
        }
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

    pub fn vault_exists(&self) -> bool {
        self.vault_path.is_file()
    }

    pub fn create_vault(
        &mut self,
        master_password: &str,
        settings: AppSettings,
    ) -> anyhow::Result<(VaultSummary, RecoveryMaterial)> {
        if self.vault_path.exists() {
            return Err(CoreError::VaultAlreadyExists.into());
        }
        validate_master_password(master_password)?;
        validate_settings(&settings)?;

        let recovery_key = generate_recovery_key();
        let recovery = recovery_material(&recovery_key);
        let mut document = VaultDocument {
            settings,
            ..Default::default()
        };
        ensure_document_defaults(&mut document);
        let dek = Zeroizing::new(random_dek());
        let encrypted = build_current_vault(
            Uuid::new_v4(),
            Utc::now(),
            master_password,
            &recovery_key,
            &dek,
            &document,
        )?;
        let summary = summary_from_metadata(&encrypted.metadata);
        save_encrypted_vault(&self.vault_path, &encrypted)?;
        Ok((summary, recovery))
    }

    pub fn unlock_vault_master(&mut self, master_password: &str) -> anyhow::Result<UnlockResult> {
        validate_master_password(master_password)?;
        match load_vault_file(&self.vault_path)? {
            VaultFile::Current(encrypted) => {
                let dek = unlock_current_master(&encrypted, master_password)?;
                let clear = Zeroizing::new(
                    decrypt_vault_payload(&encrypted.vault_blob, &dek, &encrypted.metadata)
                        .map_err(|_| CoreError::AuthenticationFailed)?,
                );
                let mut document = parse_document(&clear)?;
                ensure_document_defaults(&mut document);
                let session = self.start_session(dek, document);
                Ok(UnlockResult {
                    session,
                    migrated_recovery: None,
                })
            }
            VaultFile::Legacy(encrypted) => self.unlock_and_migrate_legacy(encrypted, master_password),
        }
    }

    pub fn unlock_vault_recovery(
        &mut self,
        recovery_key: &str,
        new_master_password: &str,
    ) -> anyhow::Result<RecoveryUnlockResult> {
        validate_recovery_key(recovery_key)?;
        validate_master_password(new_master_password)?;

        let (vault_id, created_at, dek, clear) = match load_vault_file(&self.vault_path)? {
            VaultFile::Current(encrypted) => {
                let slot = &encrypted.metadata.recovery_kdf;
                let recovery_kek = derive_recovery_kek(recovery_key, &slot.salt, &slot.params)
                    .map_err(|_| CoreError::AuthenticationFailed)?;
                let aad = metadata_aad(&encrypted.metadata, "recovery-wrapped-dek")?;
                let dek = unwrap_dek_with_aad(
                    recovery_kek.as_ref(),
                    &encrypted.wrapped_keys.wrapped_dek_by_recovery,
                    &aad,
                )
                .map_err(|_| CoreError::AuthenticationFailed)?;
                let clear = decrypt_vault_payload(&encrypted.vault_blob, &dek, &encrypted.metadata)
                    .map_err(|_| CoreError::AuthenticationFailed)?;
                (
                    encrypted.metadata.vault_id,
                    encrypted.metadata.created_at,
                    dek,
                    clear,
                )
            }
            VaultFile::Legacy(encrypted) => {
                let recovery_kek = derive_recovery_kek(
                    recovery_key,
                    &encrypted.metadata.salt,
                    &encrypted.metadata.kdf_params,
                )
                .map_err(|_| CoreError::AuthenticationFailed)?;
                let dek = unwrap_dek(
                    recovery_kek.as_ref(),
                    &encrypted.wrapped_keys.wrapped_dek_by_recovery,
                )
                .map_err(|_| CoreError::AuthenticationFailed)?;
                let clear = decrypt_legacy_vault_payload(&encrypted.vault_blob, &dek)
                    .map_err(|_| CoreError::AuthenticationFailed)?;
                (
                    encrypted.metadata.vault_id,
                    encrypted.metadata.created_at,
                    dek,
                    clear,
                )
            }
        };

        let clear = Zeroizing::new(clear);
        let mut document = parse_document(&clear)?;
        ensure_document_defaults(&mut document);
        let dek = Zeroizing::new(dek);
        let next_recovery_key = generate_recovery_key();
        let encrypted = build_current_vault(
            vault_id,
            created_at,
            new_master_password,
            &next_recovery_key,
            &dek,
            &document,
        )?;
        save_encrypted_vault(&self.vault_path, &encrypted)?;
        let session = self.start_session(*dek, document);

        Ok(RecoveryUnlockResult {
            session,
            recovery: recovery_material(&next_recovery_key),
        })
    }

    pub fn regenerate_recovery_key(&mut self, master_password: &str) -> anyhow::Result<RecoveryMaterial> {
        validate_master_password(master_password)?;
        let mut encrypted = load_encrypted_vault(&self.vault_path)?;
        let dek = Zeroizing::new(unlock_current_master(&encrypted, master_password)?);
        let document_bytes = Zeroizing::new({
            let session = self.session_mut()?;
            serde_json::to_vec(&session.doc).context("serialize vault document")?
        });
        let next_key = generate_recovery_key();
        encrypted.metadata.recovery_kdf = KdfSlot {
            params: KdfParams::default(),
            salt: random_salt().to_vec(),
        };
        let recovery_kek = derive_recovery_kek(
            &next_key,
            &encrypted.metadata.recovery_kdf.salt,
            &encrypted.metadata.recovery_kdf.params,
        )?;
        let recovery_aad = metadata_aad(&encrypted.metadata, "recovery-wrapped-dek")?;
        encrypted.wrapped_keys.wrapped_dek_by_recovery =
            wrap_dek_with_aad(recovery_kek.as_ref(), &dek, &recovery_aad)?;
        encrypted.vault_blob = encrypt_vault_payload(&document_bytes, &dek, &encrypted.metadata)?;
        save_encrypted_vault(&self.vault_path, &encrypted)?;
        Ok(recovery_material(&next_key))
    }

    pub fn rotate_master_password(&mut self, old_password: &str, new_password: &str) -> anyhow::Result<()> {
        validate_master_password(old_password)?;
        validate_master_password(new_password)?;
        let mut encrypted = load_encrypted_vault(&self.vault_path)?;
        let dek = Zeroizing::new(unlock_current_master(&encrypted, old_password)?);
        let clear = Zeroizing::new(
            decrypt_vault_payload(&encrypted.vault_blob, &dek, &encrypted.metadata)
                .map_err(|_| CoreError::AuthenticationFailed)?,
        );

        encrypted.metadata.master_kdf = KdfSlot {
            params: KdfParams::default(),
            salt: random_salt().to_vec(),
        };
        let new_kek = derive_kek(
            new_password,
            &encrypted.metadata.master_kdf.salt,
            &encrypted.metadata.master_kdf.params,
        )?;
        let master_aad = metadata_aad(&encrypted.metadata, "master-wrapped-dek")?;
        encrypted.wrapped_keys.wrapped_dek_by_master =
            wrap_dek_with_aad(new_kek.as_ref(), &dek, &master_aad)?;
        encrypted.vault_blob = encrypt_vault_payload(&clear, &dek, &encrypted.metadata)?;
        save_encrypted_vault(&self.vault_path, &encrypted)
    }

    pub fn lock_vault(&mut self) {
        self.session = None;
    }

    pub fn touch_activity(&mut self) -> anyhow::Result<()> {
        let session = self.session_mut()?;
        session.last_activity = Instant::now();
        session.last_activity_wall = SystemTime::now();
        Ok(())
    }

    pub fn touch_activity_at(&mut self, now: Instant) -> anyhow::Result<()> {
        let session = self.session_mut()?;
        session.last_activity = now;
        session.last_activity_wall = SystemTime::now();
        Ok(())
    }

    pub fn lock_if_idle(&mut self) -> bool {
        self.lock_if_idle_at_clocks(Instant::now(), SystemTime::now())
    }

    pub fn lock_if_idle_at(&mut self, now: Instant) -> bool {
        self.lock_if_idle_at_clocks(now, SystemTime::now())
    }

    fn lock_if_idle_at_clocks(&mut self, now: Instant, wall_now: SystemTime) -> bool {
        let should_lock = self.session.as_ref().is_some_and(|session| {
            let timeout = Duration::from_secs(u64::from(session.doc.settings.auto_lock_minutes) * 60);
            let monotonic_expired = now
                .checked_duration_since(session.last_activity)
                .is_some_and(|elapsed| elapsed >= timeout);
            let wall_expired = wall_now
                .duration_since(session.last_activity_wall)
                .is_ok_and(|elapsed| elapsed >= timeout);
            monotonic_expired || wall_expired
        });
        if should_lock {
            self.lock_vault();
        }
        should_lock
    }

    pub fn should_lock_on_window_blur(&self) -> bool {
        self.session
            .as_ref()
            .is_some_and(|session| session.doc.settings.lock_on_window_blur)
    }

    pub fn should_lock_on_minimize(&self) -> bool {
        self.session
            .as_ref()
            .is_some_and(|session| session.doc.settings.lock_on_minimize)
    }

    pub fn add_item(&mut self, input: ItemInput) -> anyhow::Result<Uuid> {
        validate_item_input(&input)?;
        if let Some(secret) = input.totp_secret.as_deref() {
            crate::generate_totp_code(secret, 0)?;
        }
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
            icon_data_url: input.icon_data_url,
            security_questions: input.security_questions,
            totp_secret: input.totp_secret,
            password_history: Vec::new(),
            created_at: now,
            updated_at: now,
        };
        self.session_mut()?.doc.items.insert(id, item);
        self.flush()?;
        Ok(id)
    }

    pub fn update_item(&mut self, item_id: Uuid, patch: ItemPatch) -> anyhow::Result<()> {
        validate_item_patch(&patch)?;
        if let Some(Some(secret)) = patch.totp_secret.as_ref() {
            crate::generate_totp_code(secret, 0)?;
        }
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
            if password != item.password {
                if let Some(previous) = item.password.take() {
                    item.password_history.insert(
                        0,
                        PasswordHistoryEntry {
                            password: previous,
                            changed_at: Utc::now(),
                        },
                    );
                    truncate_password_history(&mut item.password_history);
                }
                item.password = password;
            }
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
        if let Some(icon_data_url) = patch.icon_data_url {
            item.icon_data_url = icon_data_url;
        }
        if let Some(questions) = patch.security_questions {
            item.security_questions = questions;
        }
        if let Some(secret) = patch.totp_secret {
            item.totp_secret = secret;
        }
        item.updated_at = Utc::now();
        self.flush()
    }

    pub fn delete_item(&mut self, item_id: Uuid) -> anyhow::Result<()> {
        let session = self.session_mut()?;
        let item = session
            .doc
            .items
            .remove(&item_id)
            .ok_or(CoreError::ItemNotFound)?;
        session.doc.deleted_items.push(DeletedItem {
            item,
            deleted_at: Utc::now(),
        });
        if session.doc.deleted_items.len() > DELETED_ITEM_LIMIT {
            let mut evicted = session.doc.deleted_items.remove(0);
            zeroize_item(&mut evicted.item);
        }
        self.flush()
    }

    pub fn undo_delete(&mut self) -> anyhow::Result<Uuid> {
        let session = self.session_mut()?;
        let deleted = session.doc.deleted_items.pop().ok_or(CoreError::NothingToUndo)?;
        let id = deleted.item.id;
        session.doc.items.insert(id, deleted.item);
        self.flush()?;
        Ok(id)
    }

    pub fn list_items(&mut self, filter: ListFilter) -> anyhow::Result<Vec<ItemSummary>> {
        if filter
            .query
            .as_ref()
            .is_some_and(|query| query.chars().count() > 256)
        {
            return Err(CoreError::InvalidInput("search query is too long".into()).into());
        }
        if filter.tag.as_ref().is_some_and(|tag| tag.chars().count() > 64) {
            return Err(CoreError::InvalidInput("tag filter is too long".into()).into());
        }
        let session = self.session_mut()?;
        let query = filter.query.unwrap_or_default().to_lowercase();
        let mut output = session
            .doc
            .items
            .values()
            .filter(|item| {
                if let Some(item_type) = &filter.item_type {
                    if &item.item_type != item_type {
                        return false;
                    }
                }
                if !query.is_empty()
                    && !item.title.to_lowercase().contains(&query)
                    && !item
                        .username
                        .as_deref()
                        .unwrap_or_default()
                        .to_lowercase()
                        .contains(&query)
                    && !item
                        .notes
                        .as_deref()
                        .unwrap_or_default()
                        .to_lowercase()
                        .contains(&query)
                    && !item.tags.iter().any(|tag| tag.to_lowercase().contains(&query))
                {
                    return false;
                }
                if filter
                    .tag
                    .as_ref()
                    .is_some_and(|tag| !item.tags.iter().any(|candidate| candidate == tag))
                {
                    return false;
                }
                if filter
                    .folder
                    .as_ref()
                    .is_some_and(|folder| item.folder.as_deref() != Some(folder.as_str()))
                {
                    return false;
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
                icon_data_url: item.icon_data_url.clone(),
                has_totp: item.totp_secret.is_some(),
                updated_at: item.updated_at,
            })
            .collect::<Vec<_>>();
        output.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        Ok(output)
    }

    pub fn get_item(&mut self, item_id: Uuid) -> anyhow::Result<VaultItem> {
        self.session_mut()?
            .doc
            .items
            .get(&item_id)
            .cloned()
            .ok_or(CoreError::ItemNotFound.into())
    }

    pub fn get_totp_code(&mut self, item_id: Uuid) -> anyhow::Result<TotpCode> {
        let item = self
            .session_mut()?
            .doc
            .items
            .get(&item_id)
            .ok_or(CoreError::ItemNotFound)?;
        let secret = item
            .totp_secret
            .as_deref()
            .ok_or_else(|| CoreError::InvalidInput("item has no TOTP secret".into()))?;
        current_totp_code(secret)
    }

    pub fn generate_password(&self, policy: PasswordPolicy) -> anyhow::Result<GeneratedPassword> {
        Ok(GeneratedPassword {
            value: generate_password_value(&policy)?,
        })
    }

    pub fn list_profiles(&mut self) -> anyhow::Result<Vec<PasswordProfile>> {
        let mut profiles = self
            .session_mut()?
            .doc
            .profiles
            .values()
            .cloned()
            .collect::<Vec<_>>();
        profiles.sort_by(|left, right| left.name.to_lowercase().cmp(&right.name.to_lowercase()));
        Ok(profiles)
    }

    pub fn save_profile(&mut self, mut profile: PasswordProfile) -> anyhow::Result<Uuid> {
        validate_profile(&profile)?;
        let is_new = profile.id.is_nil() || !self.session_mut()?.doc.profiles.contains_key(&profile.id);
        if is_new && self.session_mut()?.doc.profiles.len() >= 100 {
            return Err(
                CoreError::InvalidInput("a vault may contain at most 100 password profiles".into()).into(),
            );
        }
        if profile.id.is_nil() {
            profile.id = Uuid::new_v4();
        }
        let id = profile.id;
        self.session_mut()?.doc.profiles.insert(id, profile);
        self.flush()?;
        Ok(id)
    }

    pub fn delete_profile(&mut self, profile_id: Uuid) -> anyhow::Result<()> {
        let session = self.session_mut()?;
        if session.doc.profiles.len() <= 1 {
            return Err(CoreError::InvalidInput("at least one password profile is required".into()).into());
        }
        session
            .doc
            .profiles
            .remove(&profile_id)
            .ok_or(CoreError::ProfileNotFound)?;
        self.flush()
    }

    pub fn get_settings(&mut self) -> anyhow::Result<AppSettings> {
        Ok(self.session_mut()?.doc.settings.clone())
    }

    pub fn update_settings(&mut self, settings: AppSettings) -> anyhow::Result<()> {
        validate_settings(&settings)?;
        self.session_mut()?.doc.settings = settings;
        self.flush()
    }

    pub fn health_report(&mut self) -> anyhow::Result<HealthReport> {
        let session = self.session_mut()?;
        let mut issues = Vec::new();
        let mut password_items: HashMap<&str, Vec<&VaultItem>> = HashMap::new();
        for item in session.doc.items.values() {
            if let Some(password) = item.password.as_deref() {
                password_items.entry(password).or_default().push(item);
                if password_is_weak(password) {
                    issues.push(HealthIssue {
                        item_id: item.id,
                        title: item.title.clone(),
                        kind: HealthIssueKind::Weak,
                        detail: "Password is short or lacks character variety.".into(),
                    });
                }
                if item.updated_at < Utc::now() - ChronoDuration::days(OLD_PASSWORD_DAYS) {
                    issues.push(HealthIssue {
                        item_id: item.id,
                        title: item.title.clone(),
                        kind: HealthIssueKind::Old,
                        detail: format!("Password has not changed in over {OLD_PASSWORD_DAYS} days."),
                    });
                }
            }
        }
        for matches in password_items.values().filter(|matches| matches.len() > 1) {
            for item in matches {
                issues.push(HealthIssue {
                    item_id: item.id,
                    title: item.title.clone(),
                    kind: HealthIssueKind::Reused,
                    detail: format!("Password is reused across {} entries.", matches.len()),
                });
            }
        }
        let weak_count = issues
            .iter()
            .filter(|issue| matches!(issue.kind, HealthIssueKind::Weak))
            .count();
        let reused_count = issues
            .iter()
            .filter(|issue| matches!(issue.kind, HealthIssueKind::Reused))
            .count();
        let old_count = issues
            .iter()
            .filter(|issue| matches!(issue.kind, HealthIssueKind::Old))
            .count();
        let analyzed_items = password_items.values().map(Vec::len).sum();
        Ok(HealthReport {
            analyzed_items,
            weak_count,
            reused_count,
            old_count,
            issues,
        })
    }

    pub fn export_backup(
        &mut self,
        path: impl AsRef<Path>,
        passphrase: &str,
    ) -> anyhow::Result<BackupManifest> {
        validate_master_password(passphrase)?;
        let include_settings = self
            .session_mut()?
            .doc
            .settings
            .backup_preferences
            .include_settings;
        let path = path.as_ref();
        if path == self.vault_path {
            return Err(CoreError::InvalidInput("backup path must differ from vault path".into()).into());
        }
        let vault_bytes = if include_settings {
            read_bounded(&self.vault_path, qiring_storage::MAX_VAULT_FILE_BYTES)?
        } else {
            #[derive(serde::Serialize)]
            struct PortableDocument<'a> {
                items: &'a HashMap<Uuid, VaultItem>,
                profiles: &'a HashMap<Uuid, PasswordProfile>,
                settings: AppSettings,
                deleted_items: &'a [DeletedItem],
            }

            let (document_bytes, dek) = {
                let session = self.session_mut()?;
                let document = PortableDocument {
                    items: &session.doc.items,
                    profiles: &session.doc.profiles,
                    settings: AppSettings::default(),
                    deleted_items: &session.doc.deleted_items,
                };
                (
                    Zeroizing::new(serde_json::to_vec(&document).context("serialize backup document")?),
                    Zeroizing::new(*session.dek),
                )
            };
            let mut portable = load_encrypted_vault(&self.vault_path)?;
            portable.vault_blob = encrypt_vault_payload(&document_bytes, &dek, &portable.metadata)?;
            serde_json::to_vec_pretty(&portable).context("serialize portable vault backup")?
        };
        let metadata = BackupMetadata {
            schema_version: 2,
            created_at: Utc::now(),
            salt: random_salt().to_vec(),
            kdf_params: KdfParams::default(),
        };
        let key = derive_kek(passphrase, &metadata.salt, &metadata.kdf_params)?;
        let aad = backup_aad(&metadata)?;
        let backup = EncryptedBackupFile {
            blob: encrypt_with_aad(key.as_ref(), &vault_bytes, &aad)?,
            metadata: metadata.clone(),
        };
        let serialized = serde_json::to_vec_pretty(&backup).context("serialize backup")?;
        if serialized.len() as u64 > MAX_BACKUP_FILE_BYTES {
            return Err(CoreError::InvalidInput("backup is too large".into()).into());
        }
        save_bytes_atomic_user_directory(path, &serialized)?;
        Ok(BackupManifest {
            path: path.display().to_string(),
            created_at: metadata.created_at,
            schema_version: metadata.schema_version,
            size_bytes: serialized.len() as u64,
        })
    }

    pub fn preview_backup(
        &mut self,
        path: impl AsRef<Path>,
        passphrase: &str,
    ) -> anyhow::Result<BackupPreview> {
        self.session_mut()?;
        let (vault_bytes, backup_created_at) = decrypt_backup(path.as_ref(), passphrase)?;
        let (vault_id, vault_created_at, vault_schema_version) = vault_identity(&vault_bytes)?;
        Ok(BackupPreview {
            vault_id,
            vault_created_at,
            vault_schema_version,
            backup_created_at,
            size_bytes: vault_bytes.len() as u64,
        })
    }

    pub fn import_backup(
        &mut self,
        path: impl AsRef<Path>,
        passphrase: &str,
    ) -> anyhow::Result<ImportReport> {
        self.session_mut()?;
        let (vault_bytes, _) = decrypt_backup(path.as_ref(), passphrase)?;
        let (vault_id, _, schema_version) = vault_identity(&vault_bytes)?;
        let safety_snapshot_path = self.write_pre_restore_snapshot()?;
        save_bytes_atomic(&self.vault_path, &vault_bytes)?;
        self.lock_vault();
        Ok(ImportReport {
            restored_vault_id: vault_id,
            restored_schema_version: schema_version,
            size_bytes: vault_bytes.len() as u64,
            safety_snapshot_path: safety_snapshot_path.display().to_string(),
        })
    }

    pub fn list_snapshots(&mut self) -> anyhow::Result<Vec<BackupSnapshot>> {
        let preferences = self.session_mut()?.doc.settings.backup_preferences.clone();
        list_snapshots_for_preferences(&preferences)
    }

    pub fn restore_snapshot(&mut self, path: impl AsRef<Path>) -> anyhow::Result<ImportReport> {
        let allowed = self
            .list_snapshots()?
            .into_iter()
            .any(|snapshot| Path::new(&snapshot.path) == path.as_ref());
        if !allowed {
            return Err(
                CoreError::InvalidInput("snapshot path is outside the configured backup set".into()).into(),
            );
        }
        let bytes = read_bounded(path.as_ref(), qiring_storage::MAX_VAULT_FILE_BYTES)?;
        let (vault_id, _, schema_version) = vault_identity(&bytes)?;
        let current_bytes = read_bounded(&self.vault_path, qiring_storage::MAX_VAULT_FILE_BYTES)?;
        let (current_vault_id, _, _) = vault_identity(&current_bytes)?;
        if vault_id != current_vault_id {
            return Err(CoreError::InvalidInput(
                "snapshot belongs to a different vault; restore refused".into(),
            )
            .into());
        }
        let safety_snapshot_path = self.write_pre_restore_snapshot()?;
        save_bytes_atomic(&self.vault_path, &bytes)?;
        self.lock_vault();
        Ok(ImportReport {
            restored_vault_id: vault_id,
            restored_schema_version: schema_version,
            size_bytes: bytes.len() as u64,
            safety_snapshot_path: safety_snapshot_path.display().to_string(),
        })
    }

    pub fn get_security_status(&mut self) -> anyhow::Result<SecurityStatus> {
        let schema_version = match load_vault_file(&self.vault_path)? {
            VaultFile::Current(vault) => vault.metadata.schema_version,
            VaultFile::Legacy(vault) => vault.metadata.schema_version,
        };
        let settings = self
            .session
            .as_mut()
            .map(|session| {
                session.last_activity = Instant::now();
                session.doc.settings.clone()
            })
            .unwrap_or_default();
        Ok(SecurityStatus {
            schema_version,
            command_version: COMMAND_VERSION.to_string(),
            biometric_available: false,
            biometric_enabled: false,
            auto_lock_minutes: settings.auto_lock_minutes,
            clipboard_clear_seconds: settings.clipboard_clear_seconds,
            lock_on_window_blur: settings.lock_on_window_blur,
            lock_on_minimize: settings.lock_on_minimize,
        })
    }

    fn unlock_and_migrate_legacy(
        &mut self,
        encrypted: LegacyEncryptedVault,
        master_password: &str,
    ) -> anyhow::Result<UnlockResult> {
        let kek = derive_kek(
            master_password,
            &encrypted.metadata.salt,
            &encrypted.metadata.kdf_params,
        )
        .map_err(|_| CoreError::AuthenticationFailed)?;
        let dek = unwrap_dek(kek.as_ref(), &encrypted.wrapped_keys.wrapped_dek_by_master)
            .map_err(|_| CoreError::AuthenticationFailed)?;
        let clear = Zeroizing::new(
            decrypt_legacy_vault_payload(&encrypted.vault_blob, &dek)
                .map_err(|_| CoreError::AuthenticationFailed)?,
        );
        let mut document = parse_document(&clear)?;
        ensure_document_defaults(&mut document);
        let dek = Zeroizing::new(dek);
        let recovery_key = generate_recovery_key();
        let migrated = build_current_vault(
            encrypted.metadata.vault_id,
            encrypted.metadata.created_at,
            master_password,
            &recovery_key,
            &dek,
            &document,
        )?;
        save_encrypted_vault(&self.vault_path, &migrated)?;
        let session = self.start_session(*dek, document);
        Ok(UnlockResult {
            session,
            migrated_recovery: Some(recovery_material(&recovery_key)),
        })
    }

    fn start_session(&mut self, dek: [u8; KEY_LEN], document: VaultDocument) -> SessionToken {
        let token = SessionToken {
            token: Uuid::new_v4().to_string(),
            unlocked_at: Utc::now(),
        };
        self.session = Some(UnlockedSession {
            dek: Zeroizing::new(dek),
            doc: document,
            last_activity: Instant::now(),
            last_activity_wall: SystemTime::now(),
        });
        token
    }

    fn session_mut(&mut self) -> anyhow::Result<&mut UnlockedSession> {
        let session = self.session.as_mut().ok_or(CoreError::VaultLocked)?;
        session.last_activity = Instant::now();
        session.last_activity_wall = SystemTime::now();
        Ok(session)
    }

    fn flush(&mut self) -> anyhow::Result<()> {
        let (bytes, dek, preferences) = {
            let session = self.session_mut()?;
            (
                Zeroizing::new(serde_json::to_vec(&session.doc).context("serialize vault document")?),
                Zeroizing::new(*session.dek),
                session.doc.settings.backup_preferences.clone(),
            )
        };
        let mut encrypted = load_encrypted_vault(&self.vault_path)?;
        encrypted.vault_blob = encrypt_vault_payload(&bytes, &dek, &encrypted.metadata)?;
        save_encrypted_vault(&self.vault_path, &encrypted)?;
        if preferences.automatic_enabled {
            self.write_automatic_snapshot(&preferences)?;
        }
        Ok(())
    }

    fn write_automatic_snapshot(&self, preferences: &BackupPreferences) -> anyhow::Result<()> {
        let directory = preferences
            .directory
            .as_deref()
            .ok_or_else(|| CoreError::InvalidInput("automatic backups require a directory".into()))?;
        let filename = format!(
            "qiring-{}-{}.qiring-snapshot",
            Utc::now().format("%Y%m%dT%H%M%S%.3fZ"),
            Uuid::new_v4().simple()
        );
        let bytes = read_bounded(&self.vault_path, qiring_storage::MAX_VAULT_FILE_BYTES)?;
        save_bytes_atomic_user_directory(&Path::new(directory).join(filename), &bytes)?;
        prune_snapshots(preferences)
    }

    fn write_pre_restore_snapshot(&self) -> anyhow::Result<PathBuf> {
        let directory = self
            .vault_path
            .parent()
            .context("vault path has no parent directory")?
            .join("restore-safety");
        let path = directory.join(format!(
            "pre-restore-{}-{}.qiring-snapshot",
            Utc::now().format("%Y%m%dT%H%M%S%.3fZ"),
            Uuid::new_v4().simple()
        ));
        let current = read_bounded(&self.vault_path, qiring_storage::MAX_VAULT_FILE_BYTES)?;
        save_bytes_atomic(&path, &current)?;

        let preferences = BackupPreferences {
            directory: Some(directory.display().to_string()),
            retention_count: 5,
            ..Default::default()
        };
        prune_snapshots(&preferences)?;
        Ok(path)
    }
}

fn build_current_vault(
    vault_id: Uuid,
    created_at: DateTime<Utc>,
    master_password: &str,
    recovery_key: &str,
    dek: &[u8; KEY_LEN],
    document: &VaultDocument,
) -> anyhow::Result<EncryptedVault> {
    let mut metadata = new_metadata(
        KdfSlot {
            params: KdfParams::default(),
            salt: random_salt().to_vec(),
        },
        KdfSlot {
            params: KdfParams::default(),
            salt: random_salt().to_vec(),
        },
    );
    metadata.vault_id = vault_id;
    metadata.created_at = created_at;
    let master_kek = derive_kek(
        master_password,
        &metadata.master_kdf.salt,
        &metadata.master_kdf.params,
    )?;
    let recovery_kek = derive_recovery_kek(
        recovery_key,
        &metadata.recovery_kdf.salt,
        &metadata.recovery_kdf.params,
    )?;
    let master_aad = metadata_aad(&metadata, "master-wrapped-dek")?;
    let recovery_aad = metadata_aad(&metadata, "recovery-wrapped-dek")?;
    let bytes = Zeroizing::new(serde_json::to_vec(document).context("serialize vault document")?);
    Ok(EncryptedVault {
        wrapped_keys: WrappedKeys {
            wrapped_dek_by_master: wrap_dek_with_aad(master_kek.as_ref(), dek, &master_aad)?,
            wrapped_dek_by_recovery: wrap_dek_with_aad(recovery_kek.as_ref(), dek, &recovery_aad)?,
        },
        vault_blob: encrypt_vault_payload(&bytes, dek, &metadata)?,
        metadata,
    })
}

fn unlock_current_master(encrypted: &EncryptedVault, master_password: &str) -> anyhow::Result<[u8; KEY_LEN]> {
    let slot = &encrypted.metadata.master_kdf;
    let kek =
        derive_kek(master_password, &slot.salt, &slot.params).map_err(|_| CoreError::AuthenticationFailed)?;
    let aad = metadata_aad(&encrypted.metadata, "master-wrapped-dek")?;
    unwrap_dek_with_aad(kek.as_ref(), &encrypted.wrapped_keys.wrapped_dek_by_master, &aad)
        .map_err(|_| CoreError::AuthenticationFailed.into())
}

fn parse_document(bytes: &[u8]) -> anyhow::Result<VaultDocument> {
    if bytes.len() > qiring_storage::MAX_VAULT_FILE_BYTES as usize {
        return Err(CoreError::InvalidInput("decrypted vault document is too large".into()).into());
    }
    serde_json::from_slice(bytes).context("deserialize vault document")
}

fn ensure_document_defaults(document: &mut VaultDocument) {
    if document.profiles.is_empty() {
        let profile = PasswordProfile::default();
        document.profiles.insert(profile.id, profile);
    }
}

fn recovery_material(key: &str) -> RecoveryMaterial {
    RecoveryMaterial {
        recovery_key: key.to_string(),
        recovery_key_fingerprint: key.chars().take(10).collect(),
    }
}

fn summary_from_metadata(metadata: &VaultMetadata) -> VaultSummary {
    VaultSummary {
        vault_id: metadata.vault_id,
        created_at: metadata.created_at,
        schema_version: metadata.schema_version,
    }
}

fn backup_aad(metadata: &BackupMetadata) -> anyhow::Result<Vec<u8>> {
    metadata.kdf_params.validate()?;
    if metadata.schema_version != 2 || metadata.salt.len() != qiring_crypto::SALT_LEN {
        return Err(CoreError::InvalidInput("backup metadata is invalid".into()).into());
    }
    serde_json::to_vec(&("qiring-backup-v2", metadata)).context("serialize backup metadata")
}

enum ParsedBackupEnvelope {
    Current(EncryptedBackupFile),
    Legacy(LegacyEncryptedBackupFile),
}

fn parse_backup_envelope(payload: &[u8]) -> anyhow::Result<ParsedBackupEnvelope> {
    if payload.len() as u64 > MAX_BACKUP_FILE_BYTES {
        return Err(CoreError::InvalidInput("backup is too large".into()).into());
    }
    let value: serde_json::Value = serde_json::from_slice(payload).context("parse backup")?;
    if value.get("metadata").is_some() {
        let backup: EncryptedBackupFile = serde_json::from_value(value).context("parse backup")?;
        backup_aad(&backup.metadata)?;
        validate_backup_blob(&backup.blob)?;
        Ok(ParsedBackupEnvelope::Current(backup))
    } else {
        let backup: LegacyEncryptedBackupFile =
            serde_json::from_value(value).context("parse legacy backup")?;
        if backup.schema_version != 1 || backup.salt.len() != qiring_crypto::SALT_LEN {
            return Err(CoreError::InvalidInput("backup metadata is invalid".into()).into());
        }
        backup.kdf_params.validate()?;
        validate_backup_blob(&backup.blob)?;
        Ok(ParsedBackupEnvelope::Legacy(backup))
    }
}

fn validate_backup_blob(blob: &qiring_crypto::CipherBlob) -> anyhow::Result<()> {
    if blob.nonce.len() != NONCE_LEN
        || blob.ciphertext.len() < 16
        || blob.ciphertext.len() as u64 > MAX_BACKUP_FILE_BYTES
    {
        return Err(CoreError::InvalidInput("backup ciphertext shape is invalid".into()).into());
    }
    Ok(())
}

/// Parse and validate the bounded backup envelope without running Argon2.
/// Decryptability is still checked by preview/import with the backup passphrase.
pub fn validate_backup_envelope_bytes(payload: &[u8]) -> anyhow::Result<()> {
    parse_backup_envelope(payload).map(|_| ())
}

fn decrypt_backup(path: &Path, passphrase: &str) -> anyhow::Result<(Vec<u8>, DateTime<Utc>)> {
    validate_master_password(passphrase)?;
    let payload = read_bounded(path, MAX_BACKUP_FILE_BYTES)?;
    match parse_backup_envelope(&payload)? {
        ParsedBackupEnvelope::Current(backup) => {
            let aad = backup_aad(&backup.metadata)?;
            let key = derive_kek(passphrase, &backup.metadata.salt, &backup.metadata.kdf_params)
                .map_err(|_| CoreError::AuthenticationFailed)?;
            let clear = decrypt_with_aad(key.as_ref(), &backup.blob, &aad)
                .map_err(|_| CoreError::AuthenticationFailed)?;
            parse_vault_bytes(&clear).map_err(|_| CoreError::AuthenticationFailed)?;
            Ok((clear, backup.metadata.created_at))
        }
        ParsedBackupEnvelope::Legacy(backup) => {
            let key = derive_kek(passphrase, &backup.salt, &backup.kdf_params)
                .map_err(|_| CoreError::AuthenticationFailed)?;
            let clear = decrypt(key.as_ref(), &backup.blob).map_err(|_| CoreError::AuthenticationFailed)?;
            parse_vault_bytes(&clear).map_err(|_| CoreError::AuthenticationFailed)?;
            Ok((clear, backup.created_at))
        }
    }
}

fn vault_identity(bytes: &[u8]) -> anyhow::Result<(Uuid, DateTime<Utc>, u32)> {
    match parse_vault_bytes(bytes)? {
        VaultFile::Current(vault) => Ok((
            vault.metadata.vault_id,
            vault.metadata.created_at,
            vault.metadata.schema_version,
        )),
        VaultFile::Legacy(vault) => Ok((
            vault.metadata.vault_id,
            vault.metadata.created_at,
            vault.metadata.schema_version,
        )),
    }
}

fn list_snapshots_for_preferences(preferences: &BackupPreferences) -> anyhow::Result<Vec<BackupSnapshot>> {
    let Some(directory) = preferences.directory.as_deref() else {
        return Ok(Vec::new());
    };
    let directory = Path::new(directory);
    if !directory.exists() {
        return Ok(Vec::new());
    }
    let mut snapshots = Vec::new();
    for entry in fs::read_dir(directory).context("read backup directory")? {
        let entry = entry.context("read backup entry")?;
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("qiring-snapshot") {
            continue;
        }
        let metadata = entry.metadata().context("inspect backup snapshot")?;
        if !metadata.is_file() || metadata.len() > qiring_storage::MAX_VAULT_FILE_BYTES {
            continue;
        }
        let created_at = metadata
            .modified()
            .map(DateTime::<Utc>::from)
            .unwrap_or_else(|_| Utc::now());
        snapshots.push(BackupSnapshot {
            path: path.display().to_string(),
            created_at,
            size_bytes: metadata.len(),
        });
    }
    snapshots.sort_by(|left, right| right.created_at.cmp(&left.created_at));
    Ok(snapshots)
}

fn prune_snapshots(preferences: &BackupPreferences) -> anyhow::Result<()> {
    let snapshots = list_snapshots_for_preferences(preferences)?;
    for snapshot in snapshots.into_iter().skip(preferences.retention_count as usize) {
        fs::remove_file(snapshot.path).context("remove expired backup snapshot")?;
    }
    Ok(())
}

fn password_is_weak(password: &str) -> bool {
    let classes = [
        password.chars().any(|character| character.is_ascii_lowercase()),
        password.chars().any(|character| character.is_ascii_uppercase()),
        password.chars().any(|character| character.is_ascii_digit()),
        password
            .chars()
            .any(|character| !character.is_ascii_alphanumeric()),
    ];
    password.chars().count() < 12 || classes.into_iter().filter(|present| *present).count() < 3
}

fn truncate_password_history(history: &mut Vec<PasswordHistoryEntry>) {
    while history.len() > PASSWORD_HISTORY_LIMIT {
        if let Some(mut entry) = history.pop() {
            entry.password.zeroize();
        }
    }
}

fn zeroize_item(item: &mut VaultItem) {
    item.title.zeroize();
    if let Some(value) = &mut item.username {
        value.zeroize();
    }
    if let Some(value) = &mut item.password {
        value.zeroize();
    }
    if let Some(value) = &mut item.url {
        value.zeroize();
    }
    if let Some(value) = &mut item.notes {
        value.zeroize();
    }
    for tag in &mut item.tags {
        tag.zeroize();
    }
    if let Some(value) = &mut item.folder {
        value.zeroize();
    }
    if let Some(value) = &mut item.icon_data_url {
        value.zeroize();
    }
    if let Some(value) = &mut item.totp_secret {
        value.zeroize();
    }
    for question in &mut item.security_questions {
        question.question.zeroize();
        question.answer.zeroize();
    }
    for entry in &mut item.password_history {
        entry.password.zeroize();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MASTER: &str = "correct horse battery staple";

    fn test_service() -> (tempfile::TempDir, VaultService) {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("vault.qiring");
        (dir, VaultService::new(path))
    }

    fn login(title: &str, password: &str) -> ItemInput {
        ItemInput {
            item_type: VaultItemType::Login,
            title: title.to_string(),
            username: Some("user@example.com".to_string()),
            password: Some(password.to_string()),
            url: Some("https://example.com".to_string()),
            notes: None,
            tags: vec!["test".to_string()],
            folder: Some("work".to_string()),
            icon_data_url: None,
            security_questions: Vec::new(),
            totp_secret: None,
        }
    }

    #[test]
    fn create_unlock_add_list_cycle() {
        let (_dir, mut service) = test_service();
        service
            .create_vault(MASTER, AppSettings::default())
            .expect("create");
        service.unlock_vault_master(MASTER).expect("unlock");
        let item_id = service.add_item(login("GitHub", "secret")).expect("add");
        assert_eq!(service.list_items(ListFilter::default()).expect("list").len(), 1);
        assert_eq!(service.get_item(item_id).expect("get").title, "GitHub");
        assert_eq!(
            service
                .list_items(ListFilter {
                    query: Some("TEST".into()),
                    ..Default::default()
                })
                .expect("search tags")
                .len(),
            1
        );
        assert_eq!(
            service
                .list_items(ListFilter {
                    tag: Some("test".into()),
                    ..Default::default()
                })
                .expect("filter tags")
                .len(),
            1
        );
    }

    #[test]
    fn item_icons_round_trip_inside_the_vault_document() {
        let (_dir, mut service) = test_service();
        service
            .create_vault(MASTER, AppSettings::default())
            .expect("create");
        service.unlock_vault_master(MASTER).expect("unlock");
        let mut input = login("With icon", "secret");
        input.icon_data_url = Some("data:image/png;base64,iVBORw0KGgo=".into());
        let item_id = service.add_item(input).expect("add");
        assert!(service
            .list_items(ListFilter::default())
            .expect("list")
            .first()
            .and_then(|item| item.icon_data_url.as_deref())
            .is_some_and(|value| value.starts_with("data:image/png;base64,")));
        assert_eq!(
            service.get_item(item_id).expect("get").icon_data_url.as_deref(),
            Some("data:image/png;base64,iVBORw0KGgo=")
        );
    }

    #[test]
    fn create_refuses_to_overwrite_existing_vault() {
        let (_dir, mut service) = test_service();
        service
            .create_vault(MASTER, AppSettings::default())
            .expect("create");
        assert!(service.create_vault(MASTER, AppSettings::default()).is_err());
    }

    #[test]
    fn wrong_password_fails_unlock() {
        let (_dir, mut service) = test_service();
        service
            .create_vault(MASTER, AppSettings::default())
            .expect("create");
        assert!(service
            .unlock_vault_master("wrong password that is long enough")
            .is_err());
    }

    #[test]
    fn recovery_unlock_rotates_master_and_recovery_credentials() {
        let (_dir, mut service) = test_service();
        let (_, recovery) = service
            .create_vault(MASTER, AppSettings::default())
            .expect("create");
        let next_master = "a replacement master password";
        let result = service
            .unlock_vault_recovery(&recovery.recovery_key, next_master)
            .expect("recover");
        service.lock_vault();
        assert!(service.unlock_vault_master(MASTER).is_err());
        service
            .unlock_vault_master(next_master)
            .expect("new master works");
        service.lock_vault();
        assert!(service
            .unlock_vault_recovery(&recovery.recovery_key, "another replacement password")
            .is_err());
        assert_ne!(result.recovery.recovery_key, recovery.recovery_key);
    }

    #[test]
    fn password_history_and_delete_undo_are_preserved() {
        let (_dir, mut service) = test_service();
        service
            .create_vault(MASTER, AppSettings::default())
            .expect("create");
        service.unlock_vault_master(MASTER).expect("unlock");
        let id = service.add_item(login("Example", "old-password")).expect("add");
        service
            .update_item(
                id,
                ItemPatch {
                    password: Some(Some("new-password".into())),
                    ..Default::default()
                },
            )
            .expect("update");
        assert_eq!(service.get_item(id).expect("get").password_history.len(), 1);
        service.delete_item(id).expect("delete");
        assert_eq!(service.undo_delete().expect("undo"), id);
    }

    #[test]
    fn trusted_clock_auto_lock_expires_session() {
        let (_dir, mut service) = test_service();
        let settings = AppSettings {
            auto_lock_minutes: 1,
            ..Default::default()
        };
        service.create_vault(MASTER, settings).expect("create");
        service.unlock_vault_master(MASTER).expect("unlock");
        let start = Instant::now();
        service.touch_activity_at(start).expect("touch");
        assert!(!service.lock_if_idle_at(start + Duration::from_secs(59)));
        assert!(service.lock_if_idle_at(start + Duration::from_secs(60)));
        assert!(service.list_items(ListFilter::default()).is_err());
    }

    #[test]
    fn wall_clock_backstop_locks_after_system_suspend() {
        let (_dir, mut service) = test_service();
        let settings = AppSettings {
            auto_lock_minutes: 1,
            ..Default::default()
        };
        service.create_vault(MASTER, settings).expect("create");
        service.unlock_vault_master(MASTER).expect("unlock");

        let monotonic_start = Instant::now();
        let wall_start = SystemTime::now();
        let session = service.session.as_mut().expect("unlocked session");
        session.last_activity = monotonic_start;
        session.last_activity_wall = wall_start;

        assert!(service.lock_if_idle_at_clocks(
            monotonic_start + Duration::from_secs(1),
            wall_start + Duration::from_secs(60),
        ));
        assert!(service.list_items(ListFilter::default()).is_err());
    }

    #[test]
    fn backup_export_preview_import_round_trip() {
        let (directory, mut service) = test_service();
        let backup_path = directory.path().join("vault.qiring-backup");
        service
            .create_vault(MASTER, AppSettings::default())
            .expect("create");
        service.unlock_vault_master(MASTER).expect("unlock");
        service.add_item(login("Example", "secret-value")).expect("add");
        service
            .export_backup(&backup_path, "backup passphrase 123")
            .expect("export");
        let preview = service
            .preview_backup(&backup_path, "backup passphrase 123")
            .expect("preview");
        assert_eq!(preview.vault_schema_version, qiring_storage::SCHEMA_VERSION);
        service
            .import_backup(&backup_path, "backup passphrase 123")
            .expect("import");
        assert!(service.list_items(ListFilter::default()).is_err());
    }

    #[test]
    fn tampered_metadata_cannot_unlock() {
        let (_dir, mut service) = test_service();
        service
            .create_vault(MASTER, AppSettings::default())
            .expect("create");
        let mut encrypted = load_encrypted_vault(&service.vault_path).expect("load");
        encrypted.metadata.vault_id = Uuid::new_v4();
        save_encrypted_vault(&service.vault_path, &encrypted).expect("save tampered");
        assert!(service.unlock_vault_master(MASTER).is_err());
    }

    #[test]
    fn tampered_wrapped_key_cannot_unlock() {
        let (_dir, mut service) = test_service();
        service
            .create_vault(MASTER, AppSettings::default())
            .expect("create");
        let mut encrypted = load_encrypted_vault(&service.vault_path).expect("load");
        encrypted.wrapped_keys.wrapped_dek_by_master.ciphertext[0] ^= 0x80;
        save_encrypted_vault(&service.vault_path, &encrypted).expect("save tampered");
        assert!(service.unlock_vault_master(MASTER).is_err());
    }

    #[test]
    fn failed_backup_import_preserves_current_vault_and_session() {
        let (directory, mut service) = test_service();
        let backup_path = directory.path().join("tampered.qiring-backup");
        service
            .create_vault(MASTER, AppSettings::default())
            .expect("create");
        service.unlock_vault_master(MASTER).expect("unlock");
        service.add_item(login("Current", "secret-value")).expect("add");
        service
            .export_backup(&backup_path, "backup passphrase 123")
            .expect("export");

        let mut backup: EncryptedBackupFile =
            serde_json::from_slice(&fs::read(&backup_path).expect("read backup")).expect("parse backup");
        backup.metadata.created_at += ChronoDuration::seconds(1);
        qiring_storage::save_bytes_atomic(
            &backup_path,
            &serde_json::to_vec_pretty(&backup).expect("serialize tampered backup"),
        )
        .expect("write tampered backup");
        let vault_before = fs::read(&service.vault_path).expect("read current vault");

        assert!(service
            .import_backup(&backup_path, "backup passphrase 123")
            .is_err());
        assert_eq!(
            fs::read(&service.vault_path).expect("read preserved vault"),
            vault_before
        );
        assert_eq!(
            service
                .list_items(ListFilter::default())
                .expect("session retained")
                .len(),
            1
        );
    }

    #[test]
    fn legacy_vault_unlock_migrates_and_requires_recovery_ceremony() {
        let (_directory, mut service) = test_service();
        let salt = qiring_crypto::random_salt();
        let params = KdfParams::default();
        let kek = qiring_crypto::derive_kek(MASTER, &salt, &params).expect("derive master");
        let recovery_key = "legacy-recovery-key-with-sufficient-length";
        let recovery_kek =
            qiring_crypto::derive_recovery_kek(recovery_key, &salt, &params).expect("derive recovery");
        let dek = qiring_crypto::random_dek();
        let document = VaultDocument::default();
        let document_bytes = serde_json::to_vec(&document).expect("serialize document");
        let legacy = qiring_storage::LegacyEncryptedVault {
            metadata: qiring_storage::LegacyVaultMetadata {
                vault_id: Uuid::new_v4(),
                created_at: Utc::now(),
                schema_version: qiring_storage::LEGACY_SCHEMA_VERSION,
                kdf_params: params,
                salt: salt.to_vec(),
            },
            wrapped_keys: WrappedKeys {
                wrapped_dek_by_master: qiring_crypto::wrap_dek(kek.as_ref(), &dek).expect("wrap master"),
                wrapped_dek_by_recovery: qiring_crypto::wrap_dek(recovery_kek.as_ref(), &dek)
                    .expect("wrap recovery"),
            },
            vault_blob: qiring_storage::encrypt_legacy_vault_payload(&document_bytes, &dek)
                .expect("encrypt legacy"),
        };
        qiring_storage::save_bytes_atomic(
            &service.vault_path,
            &serde_json::to_vec_pretty(&legacy).expect("serialize legacy"),
        )
        .expect("save legacy");

        let result = service.unlock_vault_master(MASTER).expect("migrate");
        assert!(result.migrated_recovery.is_some());
        assert!(matches!(
            qiring_storage::load_vault_file(&service.vault_path).expect("load migrated"),
            VaultFile::Current(_)
        ));
    }

    #[test]
    fn automatic_snapshots_respect_retention() {
        let (directory, mut service) = test_service();
        let backup_directory = directory.path().join("snapshots");
        let mut settings = AppSettings::default();
        settings.backup_preferences.automatic_enabled = true;
        settings.backup_preferences.directory = Some(backup_directory.display().to_string());
        settings.backup_preferences.retention_count = 2;
        service.create_vault(MASTER, settings).expect("create");
        service.unlock_vault_master(MASTER).expect("unlock");
        for index in 0..4 {
            service
                .add_item(login(&format!("Example {index}"), "secret-value"))
                .expect("add");
        }
        let snapshots = service.list_snapshots().expect("snapshots");
        assert_eq!(snapshots.len(), 2);
        for snapshot in snapshots {
            assert!(qiring_storage::load_vault_file(Path::new(&snapshot.path)).is_ok());
        }
    }

    #[test]
    fn restore_snapshot_rejects_a_snapshot_from_a_different_vault() {
        let (directory, mut service) = test_service();
        let backup_directory = directory.path().join("snapshots");
        let mut settings = AppSettings::default();
        settings.backup_preferences.directory = Some(backup_directory.display().to_string());
        service.create_vault(MASTER, settings.clone()).expect("create");
        service.unlock_vault_master(MASTER).expect("unlock");
        service.add_item(login("Current", "secret-value")).expect("add");

        let (foreign_directory, mut foreign_service) = test_service();
        let _ = &foreign_directory;
        foreign_service
            .create_vault(MASTER, AppSettings::default())
            .expect("create foreign vault");
        let foreign_bytes = fs::read(&foreign_service.vault_path).expect("read foreign vault");

        fs::create_dir_all(&backup_directory).expect("create backup directory");
        let foreign_snapshot_path = backup_directory.join("qiring-foreign.qiring-snapshot");
        fs::write(&foreign_snapshot_path, &foreign_bytes).expect("plant foreign snapshot");

        let vault_before = fs::read(&service.vault_path).expect("read current vault");
        let result = service.restore_snapshot(&foreign_snapshot_path);
        assert!(result.is_err());
        assert_eq!(
            fs::read(&service.vault_path).expect("read preserved vault"),
            vault_before
        );
        assert_eq!(
            service
                .list_items(ListFilter::default())
                .expect("session retained")
                .len(),
            1
        );
    }

    #[test]
    fn manual_backup_can_exclude_vault_settings() {
        let (directory, mut service) = test_service();
        let backup_path = directory.path().join("portable.qiring-backup");
        let settings = AppSettings {
            auto_lock_minutes: 17,
            backup_preferences: BackupPreferences {
                include_settings: false,
                ..Default::default()
            },
            ..Default::default()
        };
        service.create_vault(MASTER, settings).expect("create");
        service.unlock_vault_master(MASTER).expect("unlock");
        service
            .export_backup(&backup_path, "backup passphrase 123")
            .expect("export");
        service
            .import_backup(&backup_path, "backup passphrase 123")
            .expect("restore");
        service.unlock_vault_master(MASTER).expect("unlock restored");
        assert_eq!(service.get_settings().expect("settings").auto_lock_minutes, 5);
    }

    #[test]
    fn ring_sort_preferences_persist_and_reject_duplicate_order_entries() {
        let (_directory, mut service) = test_service();
        service
            .create_vault(MASTER, AppSettings::default())
            .expect("create");
        service.unlock_vault_master(MASTER).expect("unlock");

        let first = Uuid::new_v4();
        let second = Uuid::new_v4();
        let mut settings = service.get_settings().expect("settings");
        settings.ring_sort_mode = "custom".to_string();
        settings.ring_category_order = vec!["Work".to_string(), "Personal".to_string()];
        settings.ring_item_order = vec![second, first];
        service.update_settings(settings).expect("update Ring order");
        service.lock_vault();
        service.unlock_vault_master(MASTER).expect("unlock again");

        let persisted = service.get_settings().expect("persisted settings");
        assert_eq!(persisted.ring_category_order, ["Work", "Personal"]);
        assert_eq!(persisted.ring_item_order, [second, first]);

        let mut invalid = persisted;
        invalid.ring_item_order = vec![first, first];
        assert!(service.update_settings(invalid).is_err());
    }
}
