mod model;
mod passwords;
mod service;
mod totp;
mod validation;

pub use model::*;
pub use passwords::generate_password_value;
pub use service::{validate_backup_envelope_bytes, VaultService};
pub use totp::generate_totp_code;
pub use validation::sniff_image_media_type;

pub const COMMAND_VERSION: &str = "4";

const MAX_FUZZ_MODEL_BYTES: usize = 256 * 1024;

/// Parse and validate a password profile without generating a password.
/// This narrow entry point is also used by the profile parser fuzz target.
pub fn validate_password_profile_bytes(bytes: &[u8]) -> anyhow::Result<()> {
    if bytes.len() > MAX_FUZZ_MODEL_BYTES {
        anyhow::bail!("password profile exceeds the supported size");
    }
    let profile: PasswordProfile = serde_json::from_slice(bytes)?;
    validation::validate_profile(&profile)
}

/// Parse and validate an item payload, including secure-note metadata.
/// This avoids filesystem and KDF work in the model fuzz target.
pub fn validate_item_input_bytes(bytes: &[u8]) -> anyhow::Result<()> {
    if bytes.len() > MAX_FUZZ_MODEL_BYTES {
        anyhow::bail!("item payload exceeds the supported size");
    }
    let item: ItemInput = serde_json::from_slice(bytes)?;
    validation::validate_item_input(&item)
}
