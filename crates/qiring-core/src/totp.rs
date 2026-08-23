use crate::{CoreError, TotpCode};
use data_encoding::BASE32_NOPAD;
use hmac::{Hmac, KeyInit, Mac};
use sha1::Sha1;
use std::time::{SystemTime, UNIX_EPOCH};

type HmacSha1 = Hmac<Sha1>;

pub fn generate_totp_code(secret: &str, timestamp_seconds: u64) -> anyhow::Result<TotpCode> {
    let normalized = secret
        .chars()
        .filter(|character| !character.is_ascii_whitespace() && *character != '-')
        .flat_map(char::to_uppercase)
        .collect::<String>();
    if normalized.is_empty() || normalized.len() > 1024 {
        return Err(CoreError::InvalidInput("TOTP secret is invalid".into()).into());
    }
    let key = BASE32_NOPAD
        .decode(normalized.as_bytes())
        .map_err(|_| CoreError::InvalidInput("TOTP secret must be Base32".into()))?;
    if key.len() < 10 {
        return Err(CoreError::InvalidInput("TOTP secret is too short".into()).into());
    }

    let counter = timestamp_seconds / 30;
    let mut mac = HmacSha1::new_from_slice(&key)
        .map_err(|_| CoreError::InvalidInput("TOTP secret is invalid".into()))?;
    mac.update(&counter.to_be_bytes());
    let digest = mac.finalize().into_bytes();
    let offset = (digest[19] & 0x0f) as usize;
    let binary = (u32::from(digest[offset] & 0x7f) << 24)
        | (u32::from(digest[offset + 1]) << 16)
        | (u32::from(digest[offset + 2]) << 8)
        | u32::from(digest[offset + 3]);

    Ok(TotpCode {
        code: format!("{:06}", binary % 1_000_000),
        valid_for_seconds: 30 - (timestamp_seconds % 30),
    })
}

pub(crate) fn current_totp_code(secret: &str) -> anyhow::Result<TotpCode> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| CoreError::InvalidInput("system clock is before Unix epoch".into()))?
        .as_secs();
    generate_totp_code(secret, now)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_rfc_6238_sha1_vector_truncated_to_six_digits() {
        let secret = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ";
        let code = generate_totp_code(secret, 59).expect("code");
        assert_eq!(code.code, "287082");
        assert_eq!(code.valid_for_seconds, 1);
    }

    #[test]
    fn rejects_invalid_base32() {
        assert!(generate_totp_code("not-a-valid-!", 0).is_err());
    }
}
