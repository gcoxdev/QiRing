use crate::{AppSettings, CoreError, ItemInput, ItemPatch, PasswordProfile, SecurityQuestion};
use std::collections::HashSet;

const MAX_MASTER_PASSWORD_BYTES: usize = 1024;
const MAX_TITLE_CHARS: usize = 256;
const MAX_FIELD_CHARS: usize = 4_096;
const MAX_NOTES_CHARS: usize = 100_000;
const MAX_TAGS: usize = 50;
const MAX_TAG_CHARS: usize = 64;
const MAX_QUESTIONS: usize = 20;
const MAX_ICON_DATA_URL_CHARS: usize = 700_000;
const MAX_RING_CATEGORIES: usize = 10_000;
const MAX_RING_ITEMS: usize = 100_000;
const ICON_DATA_URL_PREFIXES: [&str; 5] = [
    "data:image/png;base64,",
    "data:image/jpeg;base64,",
    "data:image/webp;base64,",
    "data:image/gif;base64,",
    "data:image/x-icon;base64,",
];

pub(crate) fn validate_master_password(value: &str) -> anyhow::Result<()> {
    if value.chars().count() < 12 || value.len() > MAX_MASTER_PASSWORD_BYTES {
        return Err(CoreError::InvalidInput("master passwords must contain 12 to 1024 bytes".into()).into());
    }
    Ok(())
}

pub(crate) fn validate_recovery_key(value: &str) -> anyhow::Result<()> {
    if !(24..=256).contains(&value.len()) {
        return Err(CoreError::InvalidInput("recovery key has an invalid length".into()).into());
    }
    Ok(())
}

pub(crate) fn validate_item_input(input: &ItemInput) -> anyhow::Result<()> {
    validate_title(&input.title)?;
    validate_optional(&input.username, MAX_FIELD_CHARS, "username")?;
    validate_optional(&input.password, MAX_FIELD_CHARS, "password")?;
    validate_optional(&input.url, MAX_FIELD_CHARS, "URL")?;
    validate_optional(&input.notes, MAX_NOTES_CHARS, "notes")?;
    validate_optional(&input.folder, MAX_TITLE_CHARS, "folder")?;
    validate_icon_data_url(&input.icon_data_url)?;
    validate_tags(&input.tags)?;
    validate_questions(&input.security_questions)?;
    validate_optional(&input.totp_secret, 1024, "TOTP secret")
}

pub(crate) fn validate_item_patch(patch: &ItemPatch) -> anyhow::Result<()> {
    if let Some(title) = &patch.title {
        validate_title(title)?;
    }
    if let Some(value) = &patch.username {
        validate_optional(value, MAX_FIELD_CHARS, "username")?;
    }
    if let Some(value) = &patch.password {
        validate_optional(value, MAX_FIELD_CHARS, "password")?;
    }
    if let Some(value) = &patch.url {
        validate_optional(value, MAX_FIELD_CHARS, "URL")?;
    }
    if let Some(value) = &patch.notes {
        validate_optional(value, MAX_NOTES_CHARS, "notes")?;
    }
    if let Some(value) = &patch.folder {
        validate_optional(value, MAX_TITLE_CHARS, "folder")?;
    }
    if let Some(value) = &patch.icon_data_url {
        validate_icon_data_url(value)?;
    }
    if let Some(tags) = &patch.tags {
        validate_tags(tags)?;
    }
    if let Some(questions) = &patch.security_questions {
        validate_questions(questions)?;
    }
    if let Some(value) = &patch.totp_secret {
        validate_optional(value, 1024, "TOTP secret")?;
    }
    Ok(())
}

pub(crate) fn validate_profile(profile: &PasswordProfile) -> anyhow::Result<()> {
    validate_title(&profile.name)?;
    crate::passwords::validate_policy(&profile.policy)
}

pub(crate) fn validate_settings(settings: &AppSettings) -> anyhow::Result<()> {
    if !(1..=1_440).contains(&settings.auto_lock_minutes) {
        return Err(CoreError::InvalidInput("auto-lock must be between 1 minute and 24 hours".into()).into());
    }
    if !(5..=300).contains(&settings.clipboard_clear_seconds) {
        return Err(
            CoreError::InvalidInput("clipboard clearing must be between 5 and 300 seconds".into()).into(),
        );
    }
    if !matches!(settings.theme.as_str(), "system" | "dark" | "light") {
        return Err(CoreError::InvalidInput("theme must be system, dark, or light".into()).into());
    }
    if !matches!(settings.button_display.as_str(), "both" | "icons" | "labels") {
        return Err(CoreError::InvalidInput("button display must be both, icons, or labels".into()).into());
    }
    if !matches!(
        settings.ring_sort_mode.as_str(),
        "ascending" | "descending" | "custom"
    ) {
        return Err(CoreError::InvalidInput(
            "Ring sort mode must be ascending, descending, or custom".into(),
        )
        .into());
    }
    if settings.ring_category_order.len() > MAX_RING_CATEGORIES
        || settings
            .ring_category_order
            .iter()
            .any(|category| category.trim().is_empty() || category.chars().count() > MAX_TITLE_CHARS)
        || settings.ring_category_order.iter().collect::<HashSet<_>>().len()
            != settings.ring_category_order.len()
    {
        return Err(CoreError::InvalidInput("custom Ring category order is invalid".into()).into());
    }
    if settings.ring_item_order.len() > MAX_RING_ITEMS
        || settings.ring_item_order.iter().collect::<HashSet<_>>().len() != settings.ring_item_order.len()
    {
        return Err(CoreError::InvalidInput("custom Ring item order is invalid".into()).into());
    }
    let backup = &settings.backup_preferences;
    if !(1..=100).contains(&backup.retention_count) {
        return Err(CoreError::InvalidInput("backup retention must be between 1 and 100".into()).into());
    }
    if backup.automatic_enabled && backup.directory.as_deref().is_none_or(str::is_empty) {
        return Err(CoreError::InvalidInput("automatic backups require a selected directory".into()).into());
    }
    validate_optional(&backup.directory, MAX_FIELD_CHARS, "backup directory")
}

fn validate_title(value: &str) -> anyhow::Result<()> {
    let count = value.trim().chars().count();
    if count == 0 || count > MAX_TITLE_CHARS {
        return Err(
            CoreError::InvalidInput("title/name is required and limited to 256 characters".into()).into(),
        );
    }
    Ok(())
}

fn validate_optional(value: &Option<String>, maximum: usize, label: &str) -> anyhow::Result<()> {
    if value.as_ref().is_some_and(|text| text.chars().count() > maximum) {
        return Err(CoreError::InvalidInput(format!("{label} exceeds the supported size")).into());
    }
    Ok(())
}

fn validate_icon_data_url(value: &Option<String>) -> anyhow::Result<()> {
    let Some(value) = value else {
        return Ok(());
    };
    if value.len() > MAX_ICON_DATA_URL_CHARS {
        return Err(CoreError::InvalidInput("Qi icon exceeds the supported 512 KiB size".into()).into());
    }
    let payload = ICON_DATA_URL_PREFIXES
        .iter()
        .find_map(|prefix| value.strip_prefix(prefix))
        .filter(|payload| !payload.is_empty())
        .ok_or_else(|| {
            CoreError::InvalidInput("Qi icon must be a PNG, JPEG, WebP, GIF, or ICO image".into())
        })?;
    if payload
        .bytes()
        .any(|byte| !byte.is_ascii_alphanumeric() && !matches!(byte, b'+' | b'/' | b'='))
    {
        return Err(CoreError::InvalidInput("Qi icon contains invalid image data".into()).into());
    }
    Ok(())
}

fn validate_tags(tags: &[String]) -> anyhow::Result<()> {
    if tags.len() > MAX_TAGS || tags.iter().any(|tag| tag.chars().count() > MAX_TAG_CHARS) {
        return Err(CoreError::InvalidInput("too many tags or tag is too long".into()).into());
    }
    Ok(())
}

fn validate_questions(questions: &[SecurityQuestion]) -> anyhow::Result<()> {
    if questions.len() > MAX_QUESTIONS
        || questions.iter().any(|question| {
            question.question.chars().count() > MAX_FIELD_CHARS
                || question.answer.chars().count() > MAX_FIELD_CHARS
        })
    {
        return Err(CoreError::InvalidInput("security questions exceed supported limits".into()).into());
    }
    Ok(())
}
