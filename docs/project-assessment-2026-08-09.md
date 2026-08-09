# QiRing Project Assessment — 2026-08-09

**Baseline:** [project-assessment-2026-08-08.md](./project-assessment-2026-08-08.md) and its [remediation record](./assessment-remediation-2026-08-08.md), both against commit `3b8e41b`.

**This assessment's commit:** `b03a631` (three commits past the remediation baseline: `a6cc5e9`, `b28ddcd`, `b03a631`).

**Scope:** Independent re-verification of the prior assessment's claims against the current code, plus new findings in the Tauri command layer, crypto/storage crates, frontend, dependency posture, and product surface. This is an engineering review, not a formal cryptographic audit or penetration test.

**Method:** Read the two prior docs and the threat model first, then re-derived every claim from the current source rather than trusting the remediation record's summary. Ran `cargo audit` and `npm audit` fresh. Two independent deep-dive passes covered the Rust backend (Tauri commands, `qiring-crypto`, `qiring-storage`, `qiring-core::service`) and the frontend (`main.js`, `dom.js`, `api.js`, styles, accessibility).

## Executive summary

The prior remediation work holds up well under independent re-review. The P0 issues from the 2026-08-08 assessment — the Windows command-injection URL launcher, missing CSP, global Tauri bridge, `innerHTML` injection, unusable recovery ceremony, unauthenticated vault metadata, and the password-remask bug — are genuinely fixed in the current code, not just claimed fixed. `cargo audit` and `npm audit` both come back clean of vulnerability-class findings. This is now a substantially hardened prototype.

Independent re-review did surface a few things the remediation record didn't mention: a **snapshot-restore path that verifies a candidate file is a well-formed vault but never checks it belongs to the current vault** (any parseable `.qiring-snapshot`-shaped file placed in the configured backup directory will be accepted and become the active vault), and a **backup-directory permission side effect** that silently `chmod 0700`s a user-chosen folder on every write. Neither is a new hole in the trust boundary the app already defends (both stay inside the "local attacker with filesystem write access" adversary the threat model already assumes), but both weaken specific invariants the threat model claims are enforced, and both are cheap to fix. On the frontend side, the main remaining issues are cosmetic/consistency ones: a few destructive or consequential actions (master-password rotation, recovery-key regeneration, snapshot restore) still use the raw browser `confirm()` dialog instead of the app's own accessible confirmation component that other destructive actions already use, and the Password Profiles editor doesn't validate range constraints (`sum(min) ≤ length`, etc.) before submit, so users only learn about an invalid policy from a generic error toast.

No new critical or unauthenticated-remote-input issues were found. Everything below assumes the existing local-attacker threat model in `docs/threat-model.md`.

## Current health snapshot

| Area | Status | Notes |
| --- | --- | --- |
| Desktop trust boundary | Hardened | Strict CSP, `withGlobalTauri: false`, enumerated per-command capabilities, no `innerHTML`, single typed IPC adapter. Re-verified directly. |
| Cryptography | Sound | Argon2id + XChaCha20-Poly1305, versioned AAD-bound metadata, bounded KDF params, independent master/recovery salts. TOTP RFC 6238 vectors match spec. Password generator's class-minimum guarantee is provably unreachable-underflow-safe. |
| Recovery & locking | Implemented | Recovery ceremony, recovery unlock, rotation, and real Rust-owned idle/suspend/minimize/focus-loss locking all present and exercised by tests. |
| Backup/restore | Mostly sound, two gaps | Atomic, bounded, previewed, safety-snapshotted — but restore doesn't check the snapshot's `vault_id` against the live vault, and writing any snapshot silently re-chmods the user's chosen backup directory. |
| Dependency audit | Clean | `cargo audit`: 0 vulnerability-class findings (17 informational unmaintained/unsound warnings, all pre-existing and documented in the threat model). `npm audit --audit-level=moderate`: 0 vulnerabilities. |
| CI / release | Solid | SHA-pinned actions, signed/notarized bundles gated on tag pushes, SBOM + checksums + provenance, `cargo-audit` blocking. |
| UI/UX | Mature prototype | Toasts, keyboard workflow, debounced race-safe search, accessible drag/keyboard reordering, and named delete confirmations are all genuinely well done. A handful of destructive flows still use native `confirm()` instead of the app's own dialog; profile-range validation is server-only. |
| Documentation | One stale section | `README.md`'s "Current UI flow" describes a tabbed `Info/Key/Questions` editor and "5 view layouts" that no longer exist in the current single-form UI. |

## Findings

### Medium — Backup/restore trust gaps

#### M1. Snapshot restore doesn't verify the snapshot belongs to the current vault

**Evidence:** `crates/qiring-core/src/service.rs:738-759` (`restore_snapshot`).

`restore_snapshot` checks only that the requested path is one of the files currently present in the configured backup directory (via `list_snapshots_for_preferences`) and that the bytes parse as a structurally valid encrypted vault (`vault_identity`). It never compares the snapshot's `vault_id` to the currently open vault's `vault_id`. A `.qiring-snapshot`-shaped file dropped into the user's configured backup directory by any other local process, a synced folder, or an attacker with write access to that directory — as long as it parses as a valid QiRing vault — will be accepted and become the active vault after restore.

A pre-restore safety snapshot is always taken first (`write_pre_restore_snapshot`, line 750), so this is recoverable and not silent data loss, and the adversary still needs local filesystem write access to the backup directory, which the threat model already treats as a meaningful capability. But it means `docs/threat-model.md:21`'s invariant ("Restore never replaces the current vault without a verified backup") is enforced only as "structurally valid," not "belongs to this vault" — a foreign vault the attacker controls the credentials for can be swapped in, and the user has no signal that what they just unlocked isn't their own data.

**Recommendation:** Store the originating `vault_id` in the snapshot filename or a sibling manifest at write time, and either reject a restore whose `vault_id` doesn't match the live vault, or surface an explicit "this snapshot belongs to a different vault" warning requiring confirmation before proceeding.

#### M2. Writing to a backup directory silently forces its permissions to `0700`

**Evidence:** `crates/qiring-storage/src/lib.rs:175-196` (`save_bytes_atomic`) calls `ensure_private_directory(parent)` (line 269-278) unconditionally, which `chmod`s the parent directory to `0700` on every write — including the user's freely-chosen backup/snapshot directory (`crates/qiring-core/src/service.rs:862-898`), selected via the system file dialog.

This is reasonable for the vault's own app-data directory, but a backup directory is explicitly user-chosen and may be a shared folder, a synced cloud-storage directory, or a location other tooling/users expect to read. Silently narrowing its permissions on every automatic snapshot is a surprising side effect with no user-facing disclosure.

**Recommendation:** Reserve the forced-private-directory behavior for QiRing's own app-data paths. For user-selected backup directories, either leave existing permissions alone (only enforce `0600` on the file itself) or document the behavior explicitly in the backup directory picker and in `docs/user-guide.md`.

### Low

#### L1. Icon data URLs aren't format-validated at the core-crate boundary

**Evidence:** `crates/qiring-core/src/validation.rs:154-175` (`validate_icon_data_url`) checks only that the payload is valid base64 alphabet up to ~700,000 characters — it never decodes and magic-byte-sniffs the result. The threat model's claim that icons are "magic-byte-validated" (`docs/threat-model.md:71`) is only true for the two dialog/favicon import paths (`apps/desktop/src-tauri/src/lib.rs:651-677`), which do sniff bytes before handing off — not for `add_item`/`update_item` called directly over IPC, which only run this weaker check.

**Recommendation:** Move magic-byte sniffing into `validate_icon_data_url` (or a shared helper) so every entry point enforces the same guarantee the threat model documents.

#### L2. `README.md`'s UI flow description is stale

**Evidence:** `README.md:41-66` still describes a tabbed `Info/Key/Questions` Qi editor and "5 view layouts (Vertical 1/2, Horizontal 1/2, Compact)." Neither exists in the current single-form editor and header/menu-driven navigation model built in the recent UI rebuild. This will mislead a new contributor or user who treats the README as authoritative.

**Recommendation:** Rewrite the "Current UI flow" section to match the current screens (Vault/Ring/Qi editor, Password Profiles, Health, Backups, Settings, Help) and the keyboard workflow already documented in `docs/user-guide.md`.

### Frontend consistency issues (not security bugs)

#### F1. Some destructive/high-consequence actions use the native `confirm()` dialog instead of the app's accessible confirmation component

**Evidence:** `apps/desktop/src/main.js:1333, 1343, 1385, 1404` — master-password rotation, recovery-key regeneration, and backup/snapshot restore all gate on `window.confirm()`. Item and profile deletion, by contrast, use the styled, accessible `askConfirmation` dialog that names its target (`main.js:341-358`, e.g. `deleteItem` at line 1030-1034).

Rotation and recovery-key regeneration are at least as consequential as a delete (recovery-key regeneration immediately invalidates the previous key; snapshot restore replaces the live vault), but they get a plainer, non-styled, non-focus-managed OS dialog. The infrastructure to fix this already exists in the same file.

**Recommendation:** Route these four confirmations through `askConfirmation`/`askUnsavedDecision` for visual and behavioral consistency.

#### F2. Similarly, some "new item / switch item" flows use `window.confirm()` instead of the richer unsaved-changes dialog

**Evidence:** `apps/desktop/src/main.js:843, 1138, 1158` (`newItem`, `newProfile`, `selectProfile`) use plain `window.confirm()`, while the equivalent Qi-switch path (`selectItem`, line 865-871) uses `askUnsavedDecision`, which offers Save/Discard/Stay rather than a binary OK/Cancel. A user switching profiles or starting a new Qi/profile loses the "save first" option that Qi-switching already has.

**Recommendation:** Use `askUnsavedDecision` uniformly for every unsaved-changes interruption.

#### F3. Password Profiles editor has no client-side range validation before submit

**Evidence:** `apps/desktop/src/main.js:1201-1210, 1229-1233` (`saveProfile`, `testProfile`) call only the native `reportValidity()` (which checks each field's own `min`/`max` attribute in isolation). There is no check that `sum(min) ≤ length`, that each class's `min ≤ max`, or that `length ≤ sum(max)` before calling the backend — exactly the validation the original 2026-08-08 assessment asked for on the Rust side (now present there) but not mirrored in the UI. A user who enters an impossible combination only discovers the problem from a generic error toast after clicking Save or Test, with no indication of which field is wrong.

**Recommendation:** Add a live cross-field validation summary (e.g., "requires at least N characters, but length is set to M") that updates as the user edits the policy, mirroring the bounds already enforced in `crates/qiring-core/src/validation.rs`.

## What's confirmed fixed (re-verified, not re-litigated)

The following P0/P1 items from the 2026-08-08 assessment were independently checked against current code and found genuinely resolved — listed briefly for completeness, not because new evidence is needed:

- URL launcher: OS shell invocation is gone; scoped `opener:allow-open-url` capability restricted to `http(s)://`, frontend validates full URLs before calling it.
- Webview trust boundary: CSP is strict and present in `tauri.conf.json`, `withGlobalTauri: false`, no `innerHTML`/HTML-string construction anywhere in `apps/desktop/src/`, single IPC adapter (`api.js`), per-command capability allowlist (`capabilities/main.json`) matches the registered command set.
- Auto-lock: Rust owns a monotonic idle timer with suspend/resume/minimize/focus-loss handling and session teardown; not just a UI setting.
- Password remasking on item switch: `remaskPassword()` is invoked from every navigation path that changes the selected item (`main.js:1049`).
- Recovery ceremony and recovery unlock: implemented, tested, rotates both credentials.
- Vault metadata authentication: schema v2 binds vault ID/timestamp/schema/KDF slot as AEAD associated data; KDF parameters are bounded before Argon2 runs.
- File permissions: vault/backup files are `0600`, app-data directories `0700`, symlink targets rejected, atomic same-directory write + fsync + parent-directory sync on Unix.
- `cargo audit` / `npm audit`: both clean of vulnerability-class findings as of this assessment.
- Password generator: class-minimum guarantees are provably safe (`validate_policy` enforces `minimum ≤ length ≤ maximum` before the generator runs, so the flagged underflow path is unreachable).
- TOTP: RFC 6238 test vectors match; clock-skew guidance is documented in the user guide.

## New feature opportunities

These are additive product ideas, independent of the bug/security findings above, aimed at closing gaps against mainstream password-manager expectations while respecting the project's explicit "local-only, no sync, no autofill yet" scope boundary (`docs/threat-model.md:9`).

### Near-term, fits current scope

1. **CSV/JSON import from other password managers** (1Password, Bitwarden, Chrome/Firefox exports). This is likely the single biggest adoption blocker right now — there's no on-ramp for someone with an existing vault elsewhere. Should reuse the existing bounded/previewed import pipeline built for QiRing's own backups.
2. **Inline profile-range validation** (see F3) — cheap, and directly improves a screen that currently only fails at submit time.
3. **Consistent confirmation dialogs** (see F1/F2) — cheap, same session.
4. **Snapshot vault-id binding** (see M1) — should ship alongside any import/restore UI work since it touches the same code path.
5. **Password strength meter / breach-free "weak password" nudge at entry time**, distinct from the existing offline health report — surfacing weakness *while typing* a new password, not only after the fact in the Health screen.
6. **Duplicate-URL/duplicate-username detection** as part of the health report, to catch accidental double-entry of the same login.

### Later, still local-only

7. **Encrypted export to a portable single-file format** with a QR-code option for the recovery key specifically (many password managers offer a printable/scannable recovery sheet; QiRing's is copy/save/print but not QR).
8. **Per-item custom fields** (beyond username/password/URL/notes/questions) for things like PINs, security codes, or membership numbers — a common gap in fixed-schema vaults.
9. **Vault-wide search filters by field type** (e.g., "items with no password set," "items last modified > 1 year ago") building on the existing health-report infrastructure.

### Explicitly out of scope for now (matches existing threat model boundary)

Browser autofill/extension integration, encrypted sync, and online breach-checking remain correctly deferred pending their own security design, per `docs/threat-model.md:9` and the remediation record. No change recommended here — flagging only to confirm this assessment didn't silently expand scope.

## Verification performed

| Check | Result |
| --- | --- |
| `cargo audit` (from repo root, using root `Cargo.lock`) | Pass: 0 vulnerability-class findings; 17 pre-existing informational warnings |
| `npm audit --audit-level=moderate` (`apps/desktop`) | Pass: 0 vulnerabilities |
| Manual re-read of `tauri.conf.json` / `capabilities/main.json` | CSP strict, capability list matches registered commands, `http://` and `https://` both permitted for opener (matches documented compatibility tradeoff) |
| Independent deep-dive of `apps/desktop/src-tauri/src/lib.rs`, `qiring-crypto`, `qiring-storage`, `qiring-core::service` | 2 medium, 2 low findings (above); all prior P0/P1 items re-confirmed fixed |
| Independent deep-dive of `apps/desktop/src/main.js`, `dom.js`, `api.js`, styles | 3 consistency findings (above); no `innerHTML`/injection surface found; accessibility infrastructure (toasts, keyboard nav, focus management) confirmed solid |

## References

- [project-assessment-2026-08-08.md](./project-assessment-2026-08-08.md) — original baseline assessment
- [assessment-remediation-2026-08-08.md](./assessment-remediation-2026-08-08.md) — remediation record this document re-verifies
- [threat-model.md](./threat-model.md)
- [user-guide.md](./user-guide.md)
