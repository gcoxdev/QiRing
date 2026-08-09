# QiRing assessment remediation

**Implemented:** 2026-08-08

**Baseline commit:** `3b8e41b`

**Source assessment:** [project-assessment-2026-08-08.md](./project-assessment-2026-08-08.md)

All actionable work before and after the assessment's **Later, after the security boundary is hardened** section is represented here. Browser integration/autofill, encrypted synchronization, and breach checking remain intentionally deferred as requested.

## Security and reliability

| Assessment item | Resolution |
| --- | --- |
| Unsafe custom URL launcher | Removed OS command launchers. The frontend validates complete URLs and the Tauri opener capability permits only HTTP/HTTPS. |
| Webview trust boundary | Moved inline code to a Vite bundle, enabled a strict CSP, disabled the global Tauri object, removed HTML-string injection, added a single IPC adapter, and generated explicit per-command capabilities. |
| Auto-lock/lifecycle | Rust now owns idle enforcement using monotonic time with a suspend/resume wall-clock backstop. Native minimize/focus policies, session clearing, remasking, toast clearing, authorization invalidation, and clipboard clearing are wired. |
| Creation/recovery | Existing-vault overwrite is refused. Creation, schema migration, recovery unlock, and recovery-key replacement use a mandatory copy/save/print/verify/acknowledge ceremony and clear the key from the DOM. Recovery rotates both credentials. |
| Vault metadata/KDF | Schema v2 authenticates purpose-specific canonical metadata as AEAD associated data, uses independent master/recovery KDF slots, validates nonce/ciphertext shape, and bounds Argon2 before work. Schema v1 migrates after authenticated unlock. |
| Vulnerable dependencies | Tauri/plist/quick-xml, anyhow, and rand are updated. `cargo-audit` is a pinned blocking CI gate; the current lock has no vulnerability-class findings. Unsuppressed transitive maintenance/unsoundness warnings are recorded in the threat model. |
| File privacy/path handling | Durable per-user app data is required; shared-temp fallback was removed. Unix permissions are `0700`/`0600`, platform user-directory ACLs are inherited elsewhere, and vault/backup targets reject symlinks. |
| Crash-safe persistence/restore | Saves use unique same-directory atomic writes, file sync, replacement, and parent-directory sync on Unix. Imports are bounded and authenticated, previewed first, atomic, session-locking, and retain a five-copy safety set. |
| Sensitive memory | Decrypted buffers and keys use zeroizing storage where feasible, whole-session cloning was removed, serialized plaintext is zeroized, IPC credentials are wrapped immediately, and document strings are cleared on session drop. Platform/webview limits are documented. |
| Clipboard | A single Rust monitor clears only an unchanged QiRing-owned value, replaces prior ownership, clears on lock, and exposes a validated 5–300 second policy. Clipboard-history limitations are documented. |
| Resource limits | Vault/backup sizes, KDF cost, password length, item fields, notes, tags, questions, profiles, search text, settings, and clipboard payloads are capped at the Rust boundary. |
| Releases | CI uses immutable action SHAs and pinned Rust/audit tooling. Release jobs test/build native Tauri bundles on all three desktop platforms, require signing credentials for tags, notarize/staple macOS, sign Windows/AppImage artifacts, smoke-extract installers, and publish checksums, SPDX SBOMs, and provenance. The placeholder repository URL was removed. |

The expanded [threat model](./threat-model.md) defines assets, adversaries, boundaries, invariants, implemented controls, and unavoidable memory/clipboard/host-OS limitations.

## UI and product work

- Adopted Concept 1, redrew it as a production SVG, applied it throughout the UI, and generated the complete Tauri platform icon set.
- Added context-sensitive header actions immediately before Menu: Qi actions, profile actions, Settings save, and Backup export. Save/delete state follows dirtiness and selection.
- Rebuilt Password Profiles as a left master list and right policy editor with total length, per-class minimum/maximum, allowed symbols, ambiguous-character filtering, encrypted persistence, and generator sampling.
- Styled native selects/options from theme variables so dark and light popup controls retain readable contrast.
- Set native and CSS minimums to 800×600; replaced fixed pane heights with bounded viewport grids and pane-level scrolling.
- Removed the status bar and added non-layout toasts with live-region behavior, persistent errors, dismissal, action buttons, and hover/focus pause.
- Tightened Qi/Ring spacing while preserving usable hit targets. The header stays visible and the primary panes own scrolling.
- Added focus-visible, disabled/busy, tabs, menu keyboard/focus behavior, semantic headings/labels, masked answers, unsaved-change guards, search debounce/stale-response protection, list rendering containment, and reduced-motion support.
- Added a default icon-and-label button system with encrypted settings for icon-only or label-only display, immediate painted unlock feedback, save-before-lock handling, simplified unclipped index rows, and matched search/counter control heights.
- Kept navigation-menu labels visible in icon-only mode, added native label tooltips, replaced the Settings glyph with a recognizable gear, and removed redundant lock/unlock notifications.
- Added save/discard/stay handling when switching Qi entries and encrypted three-mode Ring ordering: A–Z, Z–A, and persistent custom category/Qi order with mouse and keyboard reordering.
- Rebalanced the 800 px master-detail grids and made range/icon action rows intrinsically shrinkable so editor controls remain inside their panes at the supported minimum window size.
- Added encrypted per-Qi image upload and direct favicon import with SSRF/redirect/size/type protections, plus icon rendering in the Ring index.
- Added native window size/position persistence with current-monitor clamping and primary-display fallback when monitor topology or resolution changes.
- Added Settings, secure notes, encrypted password history, deletion undo, offline health, RFC 6238 TOTP with countdown/clock guidance, documented keyboard workflow, manual encrypted backups, automatic snapshots/retention, restore preview, master rotation, and recovery-key management.

## Architecture and test work

- Split the original monolithic core into model, validation, password, TOTP, and service modules.
- Split the former inline desktop document into semantic HTML, standalone styling, DOM helpers, a narrow typed-by-convention Tauri adapter, and application orchestration. Frontend state no longer uses `localStorage`.
- Added four fuzz targets for vault, backup, profile, and item/secure-note parsers.
- Expanded Rust coverage for metadata/ciphertext/nonce/wrapped-key tampering, truncation/oversize/KDF bounds, recovery and migration, overwrite refusal, atomic permissions/symlinks, backup safety and retention, failed-restore preservation, password constraints, TOTP vectors, and idle/suspend locking.
- Added UI security-contract tests plus Playwright/axe coverage for creation/recovery, all authenticated modules, contextual actions, password profiles, unsaved navigation, keyboard/focus behavior, toast semantics, native dark selects, accessibility, and the 800×600 boundary.

## Release-operator gates

The code now enforces the release mechanics, but signing identities are intentionally not stored in the repository. Before a tagged release, configure the protected secrets in [release-process.md](./release-process.md). The tag workflow fails closed when they are missing.

On clean Windows, macOS, and Linux test machines, verify installation/removal, recovery from a temporary test vault, native dropdown popup contrast, 200% display scaling, signature identity, and upgrade behavior. This human check is retained because CI browser rendering cannot prove native WebView popup or OS installer behavior on a user's desktop.

## Verification record

| Check | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Pass |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | Pass |
| `cargo test --workspace` | Pass: 38 tests |
| `cargo check --manifest-path fuzz/Cargo.toml --all-targets` | Pass: four fuzz targets compile |
| `npm run build` | Pass |
| `npm run test:ui-contract` | Pass |
| `npm run test:e2e` | Pass: 16 Playwright/axe flows |
| `npm audit --audit-level=moderate` | Pass: 0 vulnerabilities |
| `cargo audit` | Pass: 0 vulnerability-class findings; 17 unsuppressed informational warnings documented in the threat model |
| `cargo audit --no-fetch --file fuzz/Cargo.lock` | Pass |
| `npm run tauri -- build --debug --no-bundle` | Pass |
| Workflow/config syntax | Pass: CI, release, Dependabot YAML and release Node scripts parse |
