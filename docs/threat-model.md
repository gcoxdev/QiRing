# QiRing threat model

**Version:** 1.0

**Reviewed:** 2026-08-08

**Scope:** Local desktop vault, webview, Tauri command boundary, file storage, clipboard, recovery, TOTP, and encrypted backups.

QiRing is an offline-first password manager. Browser integration, synchronization, and breach checking are intentionally out of scope until each receives a separate security design.

## Assets and security objectives

The protected assets are the master password, recovery key, backup passphrases, data-encryption key (DEK), TOTP seeds, password history, security answers, notes, and all other decrypted vault content.

The system must preserve these invariants:

1. A stolen vault or backup does not reveal plaintext without its credential.
2. Modified encrypted data, metadata, wrapped keys, or schema fields fail authentication.
3. Untrusted file values cannot request excessive KDF resources or unbounded allocations.
4. A webview may access only the commands, URLs, and user-approved paths required by the main window.
5. Restore never replaces the current vault without a verified backup and a recoverable pre-restore snapshot.
6. Locking drops the decrypted Rust session, clears displayed fields, invalidates temporary file authorizations, and clears the clipboard only when QiRing still owns its value.
7. Creating a vault never silently overwrites an existing vault.

## Trust boundaries

| Boundary | Untrusted input | Enforcement |
| --- | --- | --- |
| Webview to Rust | Every IPC argument and event | Rust validation, resource caps, explicit command permissions, no global Tauri bridge |
| Vault/backup files | JSON, schema, metadata, salts, KDF parameters, nonces, ciphertext | 64 MiB vault/128 MiB backup bounds, schema dispatch, KDF bounds, nonce validation, AEAD authentication |
| Filesystem paths | Existing vault path, backup destinations, restore sources | Durable per-user app-data path, symlink rejection, system file dialogs, unforgeable selection tokens, approved backup directories |
| URL opener | Credential URL fields | Frontend URL parsing plus Tauri capability restricted to HTTP and HTTPS |
| Clipboard | Secrets copied by the user | Rust-owned generation/value guard, clear-if-unchanged timer, clear on lock |

## Adversaries

- An offline attacker who obtains a vault, snapshot, or backup file.
- A local user or process that can alter encrypted files or invoke exposed webview commands.
- Malicious or corrupted vault content rendered after a successful unlock.
- Opportunistic observation through clipboard history, process memory, screenshots, swap, crash dumps, or shoulder surfing.
- Crashes or power loss during save or restore.

Fully compromised operating systems, privileged malware, hardware keyloggers, and live process debuggers are outside the guarantees of an application-level password manager. QiRing reduces exposure in those cases but cannot make the decrypted session invisible to the host OS.

## Implemented controls

### Cryptography and formats

- Argon2id derives independent master and recovery key-encryption keys from independent 128-bit salts.
- XChaCha20-Poly1305 encrypts the vault and wrapped DEK. Versioned, purpose-specific associated data binds vault ID, timestamp, schema, and relevant KDF slots.
- KDF memory is limited to 8-256 MiB, iterations to 1-10, and parallelism to 1-4 before Argon2 runs.
- Schema v1 vaults migrate after a successful master unlock and force a new recovery-key ceremony. Schema v2 is the current format.
- Recovery unlock rotates both the master and recovery credentials. Recovery-key regeneration invalidates the previous key.
- Manual backups use a separate passphrase, KDF metadata authenticated as AEAD associated data, bounded parsing, preview-before-import, atomic restore, and a five-copy pre-restore safety set.

### Persistence

- Writes use a unique same-directory atomic writer, file sync, atomic replacement, and parent-directory sync where supported.
- Unix app-data directories and secret files are forced to `0700` and `0600`. Windows and macOS use their protected per-user application-data directory and inherited platform ACLs.
- The application fails closed if a durable app-data directory cannot be resolved and never falls back to a shared temporary directory.
- Vault and backup reads reject symbolic-link targets and oversized files.

### Desktop and webview

- Production CSP disallows inline/evaluated scripts, objects, external frames, and external content; packaged fonts and assets are local.
- The global Tauri object is disabled. A single narrow frontend adapter owns IPC calls.
- User-controlled content is inserted with DOM properties and `textContent`; no HTML-string injection API is used.
- The main window has an explicit capability containing only its application commands and scoped HTTP/HTTPS opening.
- Backup imports use a short-lived opaque selection token rather than accepting an arbitrary webview path. Automatic-backup directories must be selected through the system dialog.

### Session lifecycle

- Rust owns the authoritative idle timer. Monotonic time is primary; wall time is a suspend/resume backstop.
- Configurable lock-on-minimize and lock-on-focus-loss policies are enforced from native window events.
- Lock drops the DEK and decrypted document, zeroizes feasible Rust secret buffers, cancels clipboard ownership, invalidates file authorizations, remasks fields, and clears frontend session state/toasts.
- Sensitive Tauri string inputs are wrapped in zeroizing buffers as soon as they cross the command boundary.

## Residual risks and limitations

- JavaScript and platform clipboard APIs use immutable strings that cannot be reliably zeroized. Decrypted values can also exist transiently in webview, allocator, GPU, swap, crash-dump, and accessibility memory.
- Clipboard managers and OS clipboard history may retain old values after QiRing clears the live clipboard. QiRing never erases newer non-QiRing clipboard content.
- HTTP URLs are allowed for compatibility and provide no transport confidentiality. Users should prefer HTTPS; adding an explicit HTTP warning remains a defense-in-depth option.
- TOTP depends on the host clock and the standard 30-second window. A skewed clock can cause valid credentials to be rejected.
- Offline KDF protection slows guessing but cannot compensate for a weak master password. The current minimum is 12 characters.
- Automatic snapshots contain the already-encrypted vault. Manual backups have independent passphrase protection. Anyone who can delete every local copy can still cause denial of service.
- Biometric unlock is not exposed or advertised; it remains disabled until platform key retrieval and fallback behavior receive a separate design and review.
- Linux desktop dependencies currently include Tauri/WebKitGTK's GTK3-era unmaintained crates and `glib 0.18.5`, whose `VariantStrIter` iterator methods carry the informational `RUSTSEC-2024-0429` unsoundness warning. QiRing does not call those methods directly. These transitive warnings are monitored and not suppressed; there are no RustSec vulnerability-class findings in the current lock.

## Security verification

- Unit tests cover AEAD/AAD tampering, KDF bounds, schema migration, recovery rotation, overwrite refusal, atomic permissions/symlink behavior, backup restore/safety snapshots, password constraints, and idle/suspend expiry.
- Fuzz harnesses cover vault, backup, profile, and item/secure-note parsing without running Argon2.
- UI contract tests enforce CSP, global bridge, opener scope, and DOM-injection rules. Playwright and axe cover the authenticated UI and 800×600 minimum layout.
- CI blocks on format, Clippy warnings, Rust tests, frontend build/tests, `npm audit`, and pinned `cargo-audit 0.22.2`.
- Release jobs test and build Tauri bundles on Linux, macOS, and Windows, then publish checksums, SPDX SBOMs, and provenance attestations. Signing/notarization and clean-machine smoke verification remain release-operator gates.

Report suspected security issues privately to the repository maintainers. Do not attach real vaults, recovery keys, or credentials to an issue.
