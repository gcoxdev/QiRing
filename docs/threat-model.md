# QiRing Threat Model (Draft v0)

## Assets

- Master password
- Recovery key
- Vault encryption key (DEK)
- Stored vault entries (passwords, notes)

## Security objectives

- Confidentiality of vault data at rest
- Integrity validation on decrypt
- Offline operation with no network trust boundary

## Adversaries

- Local attacker reading disk files
- Malware observing clipboard and process memory
- Brute-force attacker with stolen vault file

## Mitigations (current)

- Argon2id KEK derivation
- XChaCha20-Poly1305 authenticated encryption
- DEK wrapping with master and recovery KEKs
- No plaintext secret logging
- Clipboard clear and auto-lock defaults in settings

## Open items

- Platform biometric key retrieval hardening
- Memory lifecycle review for all sensitive buffers
- Optional unlock backoff and failed-attempt policy
- Backup encryption format with authenticated metadata
