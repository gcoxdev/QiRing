use chrono::{DateTime, Utc};
use qiring_crypto::{CipherBlob, KdfParams};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

pub(crate) const DEFAULT_PROFILES_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VaultItemType {
    Login,
    SecureNote,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SecurityQuestion {
    pub question: String,
    pub answer: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomField {
    pub label: String,
    pub value: String,
    #[serde(default)]
    pub concealed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PasswordHistoryEntry {
    pub password: String,
    pub changed_at: DateTime<Utc>,
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
    #[serde(default)]
    pub icon_data_url: Option<String>,
    #[serde(default)]
    pub security_questions: Vec<SecurityQuestion>,
    #[serde(default)]
    pub custom_fields: Vec<CustomField>,
    #[serde(default)]
    pub totp_secret: Option<String>,
    #[serde(default)]
    pub password_history: Vec<PasswordHistoryEntry>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub auto_lock_minutes: u32,
    pub clipboard_clear_seconds: u32,
    pub lock_on_window_blur: bool,
    pub lock_on_minimize: bool,
    pub biometric_enabled: bool,
    pub theme: String,
    pub button_display: String,
    pub ring_sort_mode: String,
    pub ring_category_order: Vec<String>,
    pub ring_item_order: Vec<Uuid>,
    pub backup_preferences: BackupPreferences,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            auto_lock_minutes: 5,
            clipboard_clear_seconds: 30,
            lock_on_window_blur: false,
            lock_on_minimize: true,
            biometric_enabled: false,
            theme: "system".to_string(),
            button_display: "both".to_string(),
            ring_sort_mode: "custom".to_string(),
            ring_category_order: Vec::new(),
            ring_item_order: Vec::new(),
            backup_preferences: BackupPreferences::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct BackupPreferences {
    pub include_settings: bool,
    pub automatic_enabled: bool,
    pub directory: Option<String>,
    pub retention_count: u32,
}

impl Default for BackupPreferences {
    fn default() -> Self {
        Self {
            include_settings: true,
            automatic_enabled: false,
            directory: None,
            retention_count: 10,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CharacterRange {
    pub min: usize,
    pub max: usize,
}

impl CharacterRange {
    pub const fn new(min: usize, max: usize) -> Self {
        Self { min, max }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PasswordPolicy {
    pub length: usize,
    pub upper: CharacterRange,
    pub lower: CharacterRange,
    pub numbers: CharacterRange,
    pub symbols: CharacterRange,
    #[serde(default = "default_allowed_symbols")]
    pub allowed_symbols: String,
    #[serde(default)]
    pub avoid_ambiguous: bool,
}

impl Default for PasswordPolicy {
    fn default() -> Self {
        Self {
            length: 20,
            upper: CharacterRange::new(1, 20),
            lower: CharacterRange::new(1, 20),
            numbers: CharacterRange::new(1, 20),
            symbols: CharacterRange::new(1, 20),
            allowed_symbols: default_allowed_symbols(),
            avoid_ambiguous: false,
        }
    }
}

fn default_allowed_symbols() -> String {
    "!@#$%^&*()-_=+[]{};:,.?/".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PasswordProfile {
    pub id: Uuid,
    pub name: String,
    pub policy: PasswordPolicy,
}

impl Default for PasswordProfile {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            name: "Strong 20".to_string(),
            policy: PasswordPolicy::default(),
        }
    }
}

pub(crate) fn default_password_profiles() -> Vec<PasswordProfile> {
    vec![
        PasswordProfile::default(),
        PasswordProfile {
            id: Uuid::new_v4(),
            name: "Strong 32".to_string(),
            policy: PasswordPolicy {
                length: 32,
                upper: CharacterRange::new(2, 32),
                lower: CharacterRange::new(2, 32),
                numbers: CharacterRange::new(2, 32),
                symbols: CharacterRange::new(2, 32),
                avoid_ambiguous: true,
                ..PasswordPolicy::default()
            },
        },
        PasswordProfile {
            id: Uuid::new_v4(),
            name: "Website Compatible 16".to_string(),
            policy: PasswordPolicy {
                length: 16,
                upper: CharacterRange::new(1, 16),
                lower: CharacterRange::new(1, 16),
                numbers: CharacterRange::new(1, 16),
                symbols: CharacterRange::new(1, 4),
                allowed_symbols: "!@#$%*-_".to_string(),
                avoid_ambiguous: true,
            },
        },
        PasswordProfile {
            id: Uuid::new_v4(),
            name: "Alphanumeric 20".to_string(),
            policy: PasswordPolicy {
                length: 20,
                upper: CharacterRange::new(1, 20),
                lower: CharacterRange::new(1, 20),
                numbers: CharacterRange::new(1, 20),
                symbols: CharacterRange::new(0, 0),
                avoid_ambiguous: true,
                ..PasswordPolicy::default()
            },
        },
    ]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct DeletedItem {
    pub item: VaultItem,
    pub deleted_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct VaultDocument {
    pub items: HashMap<Uuid, VaultItem>,
    pub profiles: HashMap<Uuid, PasswordProfile>,
    #[serde(default)]
    pub default_profiles_version: u32,
    pub settings: AppSettings,
    pub deleted_items: Vec<DeletedItem>,
}

impl Default for VaultDocument {
    fn default() -> Self {
        let profiles = default_password_profiles()
            .into_iter()
            .map(|profile| (profile.id, profile))
            .collect();
        Self {
            items: HashMap::new(),
            profiles,
            default_profiles_version: DEFAULT_PROFILES_VERSION,
            settings: AppSettings::default(),
            deleted_items: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct BackupMetadata {
    pub schema_version: u32,
    pub created_at: DateTime<Utc>,
    pub kdf_params: KdfParams,
    pub salt: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct EncryptedBackupFile {
    pub metadata: BackupMetadata,
    pub blob: CipherBlob,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct LegacyEncryptedBackupFile {
    pub schema_version: u32,
    pub created_at: DateTime<Utc>,
    pub salt: Vec<u8>,
    pub kdf_params: KdfParams,
    pub blob: CipherBlob,
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
pub struct UnlockResult {
    pub session: SessionToken,
    pub migrated_recovery: Option<RecoveryMaterial>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryUnlockResult {
    pub session: SessionToken,
    pub recovery: RecoveryMaterial,
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
    #[serde(default)]
    pub icon_data_url: Option<String>,
    #[serde(default)]
    pub security_questions: Vec<SecurityQuestion>,
    #[serde(default)]
    pub custom_fields: Vec<CustomField>,
    #[serde(default)]
    pub totp_secret: Option<String>,
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
    pub icon_data_url: Option<Option<String>>,
    pub security_questions: Option<Vec<SecurityQuestion>>,
    pub custom_fields: Option<Vec<CustomField>>,
    pub totp_secret: Option<Option<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ListFilter {
    pub query: Option<String>,
    pub tag: Option<String>,
    pub folder: Option<String>,
    pub item_type: Option<VaultItemType>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemSummary {
    pub id: Uuid,
    pub item_type: VaultItemType,
    pub title: String,
    pub username: Option<String>,
    pub tags: Vec<String>,
    pub folder: Option<String>,
    pub icon_data_url: Option<String>,
    pub has_totp: bool,
    pub updated_at: DateTime<Utc>,
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
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupPreview {
    pub vault_id: Uuid,
    pub vault_created_at: DateTime<Utc>,
    pub vault_schema_version: u32,
    pub backup_created_at: DateTime<Utc>,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportReport {
    pub restored_vault_id: Uuid,
    pub restored_schema_version: u32,
    pub size_bytes: u64,
    pub safety_snapshot_path: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CsvColumnMapping {
    pub item_type: Option<String>,
    pub title: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub url: Option<String>,
    pub notes: Option<String>,
    pub tags: Option<String>,
    pub category: Option<String>,
    pub security_questions: Option<String>,
    pub custom_fields: Option<String>,
    pub totp_secret: Option<String>,
    #[serde(default)]
    pub include_unmapped_in_notes: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CsvImportPreview {
    pub headers: Vec<String>,
    pub row_count: usize,
    pub sample_rows: Vec<Vec<String>>,
    pub canonical: bool,
    pub suggested_mapping: CsvColumnMapping,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CsvImportReport {
    pub imported_count: usize,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaintextExportManifest {
    pub path: String,
    pub row_count: usize,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupSnapshot {
    pub path: String,
    pub created_at: DateTime<Utc>,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityStatus {
    pub schema_version: u32,
    pub command_version: String,
    pub biometric_available: bool,
    pub biometric_enabled: bool,
    pub auto_lock_minutes: u32,
    pub clipboard_clear_seconds: u32,
    pub lock_on_window_blur: bool,
    pub lock_on_minimize: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthIssueKind {
    Weak,
    Reused,
    Old,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthIssue {
    pub item_id: Uuid,
    pub title: String,
    pub kind: HealthIssueKind,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthReport {
    pub analyzed_items: usize,
    pub weak_count: usize,
    pub reused_count: usize,
    pub old_count: usize,
    pub issues: Vec<HealthIssue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TotpCode {
    pub code: String,
    pub valid_for_seconds: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("Ring is locked")]
    VaultLocked,
    #[error("item not found")]
    ItemNotFound,
    #[error("password profile not found")]
    ProfileNotFound,
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("authentication failed")]
    AuthenticationFailed,
    #[error("a Ring already exists at this location")]
    VaultAlreadyExists,
    #[error("no deleted item is available to restore")]
    NothingToUndo,
}
