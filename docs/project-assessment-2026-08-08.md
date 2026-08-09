# QiRing Project Assessment

> This is the original point-in-time assessment. Its actionable items, except the three ideas explicitly listed under **Later, after the security boundary is hardened**, were implemented in the [assessment remediation record](./assessment-remediation-2026-08-08.md).

**Assessment date:** 2026-08-08  
**Scope:** Current repository, desktop UI, Tauri command boundary, cryptography and storage code, automated checks, release workflow, and product opportunities.  
**Assessment type:** Engineering review, not a formal cryptographic audit or penetration test.

## Executive summary

QiRing has a promising foundation: the Rust workspace has clear crypto, storage, domain, and desktop boundaries; the vault uses Argon2id and XChaCha20-Poly1305; the core code is small enough to reason about; formatting and Clippy are clean; and all current tests pass.

It is still a prototype rather than a production-ready password manager. The most important issue is the desktop trust boundary. A local HTML file with inline script has no Content Security Policy, exposes Tauri through a global object, builds portions of the UI with `innerHTML`, and registers every application command for the main webview. The URL-opening command then passes vault-controlled text to operating-system launchers without restricting the URL scheme; the Windows `cmd /C start` implementation is a command-injection risk. Those conditions should be fixed together.

The next major gap is the difference between advertised security behavior and implemented behavior. Auto-lock is stored as a setting but never enforced, biometric unlock always fails even though it defaults to enabled, the recovery key is hidden immediately after vault creation, and no recovery unlock command exists. For a password manager, recovery, locking, and data-loss prevention should be release-blocking behavior.

The UI direction in the request is sound. A context-sensitive top action bar, master-detail password-profile layout, dark native select styling, toast notifications, explicit window minimums, and tighter spacing will make the application substantially more usable without requiring a framework rewrite.

## Current health snapshot

| Area | Status | Notes |
| --- | --- | --- |
| Architecture | Good prototype | Clear crate boundaries; `VaultService` is the main cross-layer hub. |
| Cryptographic primitives | Promising | Appropriate modern primitives; metadata binding, parameter bounds, and memory lifecycle need hardening. |
| Desktop trust boundary | High risk | No CSP, global Tauri bridge, broad command exposure, and unsafe URL launching. |
| Recovery and locking | Incomplete | Recovery flow is not usable; auto-lock is configured but not implemented. |
| UI/UX | Functional prototype | Core flow exists, but layout density, context actions, accessibility, and state protection need work. |
| Automated checks | Mixed | Format, Clippy, and 9 tests pass; RustSec audit fails on 2 high advisories. |
| Release process | Prototype | Workflow builds raw backend binaries rather than signed/notarized Tauri bundles. |

## Recommended priority order

### P0 — address before real credentials or public release

#### 1. Replace the custom URL launcher and restrict schemes

**Evidence:** `apps/desktop/src-tauri/src/lib.rs:198-229`

`open_url` accepts any non-empty string. On Windows it invokes `cmd /C start` with vault-controlled input, which creates a command-injection surface. On every platform it also permits schemes such as `file:`, arbitrary custom handlers, or option-like input that a password field should not be able to launch.

**Recommendation:** Remove the shell-specific implementation. Parse with a URL library, allow only `https:` and optionally `http:` after an explicit warning, and use Tauri's scoped opener API. Configure the narrowest opener permission possible. Do not permit local file paths from a credential URL field.

#### 2. Harden the webview-to-Rust trust boundary as one change

**Evidence:**

- `apps/desktop/src-tauri/tauri.conf.json:9-21` — `withGlobalTauri: true` and `csp: null`.
- `apps/desktop/web-dist/index.html:304-902` — all JavaScript is inline.
- `apps/desktop/web-dist/index.html:391-400`, `449-487`, `535-548` — dynamic `innerHTML` construction.
- `apps/desktop/web-dist/index.html:538`, `546-548` — locally stored profile IDs and lengths are inserted without complete contextual escaping.
- `apps/desktop/src-tauri/src/lib.rs:240-259` — all custom commands are registered for the webview.

The profile name is escaped, but a tampered local-storage profile can still place untrusted `id` or `length` values into HTML. With no CSP and access to privileged commands, a frontend injection has much greater impact than a normal web XSS.

**Recommendation:**

1. Move inline CSS and JavaScript to bundled files so a strict CSP is practical.
2. Enable a restrictive CSP (`default-src 'self'`; only the IPC/connect and style directives actually required).
3. Prefer DOM construction with `createElement`, `textContent`, and property assignment over HTML strings.
4. Validate locally stored objects against a schema before using them.
5. Turn off the global Tauri object when a bundled module import can replace it.
6. Define explicit application permissions/capabilities and expose only commands needed by the main window.
7. Remove backup, password-rotation, biometric, or other commands from the IPC surface until their UI and authorization flows exist.

Tauri documents CSP as a mitigation for webview vulnerabilities and notes that registered application commands are allowed to all app windows by default unless the app manifest/capability setup narrows them.

#### 3. Implement actual auto-lock and secret-screen lifecycle behavior

**Evidence:**

- `crates/qiring-core/src/lib.rs:44-63` stores `auto_lock_minutes`.
- `crates/qiring-core/src/lib.rs:548-561` reports it.
- No timer, last-activity check, suspend handler, minimize handler, or focus-loss lock exists.
- `docs/threat-model.md:28-30` lists auto-lock as a current mitigation.

**Recommendation:** Enforce lock in the trusted Rust layer, not only in JavaScript. Track monotonic last activity; lock after the configured idle interval and on suspend. Add user settings for lock-on-minimize and lock-on-focus-loss. On lock, remask and clear frontend fields, cancel clipboard timers, clear toast content, and discard the decrypted session. Test timing behavior deterministically.

Also reset the password input to `type="password"` whenever an item changes. Currently, if a user selects **Show** and then selects another Qi, the next password is immediately shown because `selectQi` changes the value but not the input type (`apps/desktop/web-dist/index.html:490-503`, `793-799`).

#### 4. Repair the vault creation and recovery flow

**Evidence:**

- `apps/desktop/web-dist/index.html:686-688` writes the recovery key into the Create screen and immediately hides that screen.
- `crates/qiring-core/src/lib.rs:246-285` creates and wraps a recovery key.
- No recovery-unlock Tauri command or usable recovery screen exists.
- `VaultService::create_vault` does not refuse to replace an existing vault (`crates/qiring-core/src/lib.rs:237-287`).

The recovery key is effectively never presented to the user, remains in a hidden DOM node, and cannot be used to unlock the vault. Separately, a direct `create_vault` IPC call can replace an existing vault on platforms where rename-over-target succeeds.

**Recommendation:** Add a required recovery ceremony: show the key, offer copy/save/print, verify selected words or segments, and require explicit acknowledgment before continuing. Clear it from the DOM afterward. Implement and test recovery unlock, then permit recovery-key rotation. Make `create_vault` fail closed when a vault exists; require a separately authenticated, explicit reset workflow for destructive replacement.

#### 5. Authenticate and bound vault metadata before expensive work

**Evidence:**

- `crates/qiring-storage/src/lib.rs:12-32` stores KDF parameters, salt, schema, vault ID, and timestamps outside the encrypted vault blob.
- `crates/qiring-crypto/src/lib.rs:63-75` accepts stored KDF parameters without application-level upper bounds.
- `crates/qiring-core/src/lib.rs:289-298` runs the stored KDF before any authenticated metadata check.

XChaCha20-Poly1305 authenticates each ciphertext, but the external metadata is not bound as AEAD associated data. A local attacker can alter non-secret metadata without detection and can supply extreme KDF parameters to force excessive CPU or memory use before unlock fails.

**Recommendation:** Define a versioned canonical header; validate strict minimum and maximum KDF values before deriving; bind the header, key slot, vault ID, and schema version as AEAD associated data. Give master and recovery key slots independent salts/parameters. Add tamper tests for every header field and fuzz the vault parser.

#### 6. Clear the dependency audit before release

`cargo audit` on 2026-08-08 scanned 481 locked dependencies and failed with:

- `quick-xml 0.38.4`: [RUSTSEC-2026-0194](https://rustsec.org/advisories/RUSTSEC-2026-0194.html) and [RUSTSEC-2026-0195](https://rustsec.org/advisories/RUSTSEC-2026-0195.html), both high-severity denial-of-service advisories; patched in `>=0.41.0`.
- Dependency path: `quick-xml -> plist 1.8.0 -> tauri 2.10.2` (including Tauri code generation). No current QiRing feature directly parses user-supplied XML, so demonstrated reachability appears low, but the release gate is correctly failing and the dependency should still be updated.
- Direct `anyhow 1.0.102`: [RUSTSEC-2026-0190](https://rustsec.org/advisories/RUSTSEC-2026-0190), patched in `>=1.0.103`. No `downcast_mut` use was found, so the affected function is not currently reachable in project code.
- Direct `rand 0.8.5`: [RUSTSEC-2026-0097](https://rustsec.org/advisories/RUSTSEC-2026-0097), patched in `>=0.8.6`. The advisory needs a custom logger and reseeding re-entry; no such logger was found, but updating is straightforward and important for the password generator.
- RustSec also reports unmaintained GTK3-related transitive crates on Linux. These are inherited from the current desktop stack; monitor Tauri's supported migration path rather than suppressing them without ownership and review dates.

**Recommendation:** Upgrade Tauri/plist until `quick-xml >=0.41`, pin `anyhow >=1.0.103`, and use a patched `rand`. Keep `cargo audit` blocking CI. Record any temporary advisory exception with reachability evidence, an owner, and an expiration date.

### P1 — security and reliability hardening

#### 7. Restrict vault and backup files to the current user

**Evidence:** `crates/qiring-storage/src/lib.rs:58-67`, `apps/desktop/src-tauri/src/lib.rs:264-302`

Files and directories use process umask defaults. Under a common `0022` umask, a newly created vault can be world-readable ciphertext. If normal data-directory variables are missing, the application falls back to a predictable system temporary directory.

**Recommendation:** Create directories as user-only and vault/backup files as user read-write only (`0700`/`0600` on Unix; restricted ACL on Windows). Fail closed if a durable app-data directory cannot be resolved; never store a password vault in a shared temp directory. Reject symlinked vault targets where practical.

#### 8. Make persistence crash-safe and cross-platform

**Evidence:** `crates/qiring-storage/src/lib.rs:58-67`, `crates/qiring-core/src/lib.rs:497-525`

The vault uses a predictable `.tmp` file and rename but does not sync the file or parent directory. Replacement semantics need explicit Windows coverage. Backup export and restore use direct `fs::write`; restore is non-atomic, can leave the current decrypted session stale, and reports `imported_items: 0` as a placeholder.

**Recommendation:** Use a proven atomic-write pattern/library: unique temp file in the destination directory, restrictive permissions, flush + `sync_all`, atomic replace, then directory sync where supported. Validate size before reading, verify imported vault schema and decryptability, lock the current session before replacement, and return an accurate import report. Keep a recoverable pre-import backup.

#### 9. Tighten sensitive-memory handling

**Evidence:**

- `UnlockedSession` derives `Clone` and contains the full plaintext vault (`crates/qiring-core/src/lib.rs:212-221`).
- `flush` clones that entire session (`crates/qiring-core/src/lib.rs:572-579`).
- Only the DEK is zeroized; passwords, notes, master/recovery input strings, decrypted buffers, and generated passwords are ordinary allocations.

**Recommendation:** Remove the session clone from `flush`, wrap feasible secret buffers in zeroizing types, zeroize decrypted byte vectors after deserialization, and avoid unnecessary secret copies across Tauri serialization. Document the limits of clearing JavaScript strings and OS/webview memory. Add a memory-lifecycle section to the threat model.

#### 10. Improve clipboard behavior

**Evidence:** `apps/desktop/web-dist/index.html:425-431`

The timeout writes an empty string regardless of whether the user has copied something else since the secret. This can destroy unrelated clipboard content and cannot guarantee deletion from clipboard managers.

**Recommendation:** Use a trusted desktop clipboard integration that clears only if the clipboard still contains the value or an ownership token QiRing placed there. Cancel prior timers, clear on lock, expose the duration in settings, and state the clipboard-manager limitation in the threat model. Consider a default shorter than 30 seconds and an option to avoid clipboard entirely for future autofill integrations.

#### 11. Validate all resource-consuming input

Add application-level caps before allocation or KDF work: vault/backup file size, item/title/note lengths, tags, password length (for example 8-256), KDF memory/iterations/parallelism, profile count, and question count. Tauri commands are a trust boundary even when the current UI only sends reasonable values.

#### 12. Make the release pipeline produce trustworthy desktop artifacts

**Evidence:** `.github/workflows/release.yml:25-49`, `apps/desktop/src-tauri/tauri.conf.json:23-27`

The release workflow runs `cargo build`, uploads raw executable/library files, and does not invoke Tauri bundling despite configured DMG/MSI/AppImage/DEB targets. There is no signing, notarization, updater verification, checksums, SBOM, or provenance. GitHub Actions use mutable version tags.

**Recommendation:** Build actual Tauri bundles, sign each platform, notarize macOS, publish checksums and an SBOM, generate provenance, and test install/uninstall/update. Pin actions to immutable commit SHAs with a dependency updater. Pin the Rust toolchain and `cargo-audit` version for reproducibility. Replace the placeholder repository URL before publishing.

## Requested UI/UX improvements

### Context-sensitive header actions

Move page-level actions into a right-aligned header cluster immediately before **Menu**. Track an explicit authenticated route/screen instead of inferring it from hidden DOM nodes.

```text
QiRing / Qi & Ring       [New Qi] [Save Qi] [Delete Qi] [Menu]
QiRing / Profiles  [New Profile] [Save Profile] [Delete Profile] [Menu]
```

- Render Qi actions only on the Qi/Ring screen and profile actions only on Password Profiles.
- Keep the action locations stable within each context.
- Disable **Save** until the form is valid/dirty and disable **Delete** when nothing is selected. Hiding based on page context is useful; disabling based on selection prevents layout jumps.
- Keep **Lock Vault** globally available in the authenticated menu/header, not mixed with record CRUD.
- Hide authenticated navigation while creating or unlocking a vault. The current menu can display workspace/profile screens while locked (`apps/desktop/web-dist/index.html:632-644`).
- Add an unsaved-changes prompt when selecting another Qi/profile, starting a new one, navigating, locking, or closing.

### Password Profiles master-detail layout

The current profile screen uses a select and then repeats profiles as summary cards below the form. The live render also shows broken checkbox alignment because the global `input { width: 100% }` rule applies to checkboxes.

Recommended desktop structure:

```text
┌ Profiles (240-280 px) ───┬ Profile Settings ──────────────────────┐
│ Strong 20                │ Name            [Strong 20          ] │
│ PIN                       │ Total length    [20]                   │
│ Legacy Site              │ Uppercase       min [2]  max [optional]│
│                           │ Lowercase       min [2]  max [optional]│
│                           │ Numbers         min [2]  max [optional]│
│                           │ Symbols         min [2]  max [optional]│
└───────────────────────────┴────────────────────────────────────────┘
```

- Use a scrollable list of profile buttons on the left and the selected profile editor on the right.
- On a narrow window, collapse to list-then-editor navigation rather than stacking an always-visible long list above the form.
- Scope full-width input rules so checkbox/radio controls retain their native size. Wrap each in a flex label with one continuous hit target.
- Persist profiles in the encrypted vault and scope them to that vault. `localStorage` currently makes them browser-profile-global and outside backup/recovery (`apps/desktop/web-dist/index.html:506-533`). Profiles are not secrets, but they are user data and should behave consistently.

### Password composition ranges

Replace the four inclusion booleans with explicit constraints while retaining a simple preset UI.

| Field | Recommended behavior |
| --- | --- |
| `length` | Integer, 8-256. |
| `upper.min` / `upper.max` | Minimum required; optional maximum. |
| `lower.min` / `lower.max` | Minimum required; optional maximum. |
| `numbers.min` / `numbers.max` | Minimum required; optional maximum. |
| `symbols.min` / `symbols.max` | Minimum required; optional maximum. |
| `allowed_symbols` | Optional advanced field with a safe default. |
| `avoid_ambiguous` | Optional preset, clearly showing the reduced alphabet. |

Validate `sum(min) <= length`, every `min <= max`, and `length <= sum(max)` when all maxima are present. Generate the required counts with a CSPRNG, fill the remaining positions from allowed classes without violating maxima, then perform an unbiased secure shuffle. The current generator samples the combined alphabet and does **not** guarantee even one character from each enabled class (`crates/qiring-core/src/lib.rs:445-476`). Add property tests for every constraint combination.

### Dark native dropdowns

**Evidence:** `apps/desktop/web-dist/index.html:73-81`, `250`, `276`

The closed select is dark in Chromium, but popup options are platform-native and the `<option>` elements have no explicit theme. This explains the reported white-on-white behavior in WebKit/OS-native dropdowns.

**Recommendation:** Set `color-scheme: dark` on the root and explicitly style both `select` and `option` with dark background and light foreground. Test the actual popup on Windows WebView2, macOS WKWebView, and Linux WebKitGTK. Keep a native select unless cross-platform testing proves it cannot meet contrast requirements; a custom combobox carries substantial keyboard and screen-reader obligations.

### Window minimums and responsive sizing

**Evidence:**

- `apps/desktop/src-tauri/tauri.conf.json:11-17` has no `minWidth` or `minHeight`.
- `apps/desktop/web-dist/index.html:101-105` fixes each pane to 620 px high.
- A 1280 px-wide live render at a 657 px content viewport already pushed the bottom action row below the fold.

**Recommendation:** Start with `minWidth: 800` and `minHeight: 600` logical pixels, then confirm on all target platforms and at 200% scaling. Use the Tauri window constraints, plus CSS based on available height: `min-height: 0` for grid children and `height: calc(100dvh - header)` rather than a fixed 620 px pane. Let only the list/editor regions scroll; keep the header and context actions visible. The minimum is a guardrail, not a substitute for responsive behavior.

### Replace the status bar with accessible toasts

**Evidence:** `apps/desktop/web-dist/index.html:36-49`, `191`, `320-333`

Remove the persistent status row. Add a toast region that does not take layout space:

- Success/info: auto-dismiss after about 3-5 seconds.
- Errors requiring action: persist until dismissed or fixed.
- `aria-live="polite"` for routine updates and `role="alert"`/assertive behavior for blocking errors.
- Pause dismissal on hover/focus and include a visible close control.
- Never place passwords, recovery keys, answers, or full usernames in toast content.
- Announce clipboard expiration succinctly without a second toast when it clears.

### Tighten spacing and reclaim vertical room

- Reset `h2`/`h3` top margins inside panels and use an 8 px panel/header inset at normal density.
- Reduce the gap between the Qi/Ring headings and their first controls.
- Keep 44 px touch targets where necessary, but reduce decorative empty space rather than control hit areas.
- Remove the fixed status row before reducing form spacing; it is the largest avoidable vertical cost.
- Use a single scrolling region per pane to avoid the current document scrollbar plus pane scrollbars.

## Additional UI and accessibility findings

The review used the current [Vercel Web Interface Guidelines](https://raw.githubusercontent.com/vercel-labs/web-interface-guidelines/main/command.md) and live Chromium renders of Create Vault, Password Profiles, and Qi/Ring states.

- `apps/desktop/web-dist/index.html:5` — add a dark `theme-color`; pair with root `color-scheme: dark`.
- `apps/desktop/web-dist/index.html:83-94` — add distinct hover, active, disabled, and `:focus-visible` states. Do not rely on the active outline as the keyboard focus indicator.
- `apps/desktop/web-dist/index.html:175-187` — menu needs `aria-expanded`, `aria-controls`, Escape handling, focus management, and menu/dialog semantics appropriate to the chosen behavior.
- `apps/desktop/web-dist/index.html:193-203` — heading hierarchy skips from `h1` to `h3`; use `h2`. Add form submit behavior, field names, and `autocomplete="new-password"` / `autocomplete="current-password"`.
- `apps/desktop/web-dist/index.html:215-216` — search needs a visible label or `aria-label`; **X** should be **Clear Search** text or an icon with an accessible name.
- `apps/desktop/web-dist/index.html:223-227` — tabs need tablist/tab/tabpanel semantics, selected state, roving tabindex, and arrow-key navigation.
- `apps/desktop/web-dist/index.html:243-251` — the profile select has no label; label it **Password Profile**.
- `apps/desktop/web-dist/index.html:391-399` — dynamic question and answer fields need programmatic labels; answers should default to masked if they are treated as secrets.
- `apps/desktop/web-dist/index.html:624`, `874` — destructive confirmation should name the target. Prefer an accessible modal and consider a short undo window for deletes.
- `apps/desktop/web-dist/index.html:719-722` — debounce search and prevent stale async responses from replacing newer results.
- For lists above roughly 50 entries, add `content-visibility` or virtualization and preserve selection/focus during rerender.
- Add empty, loading, saving, error, disabled, and first-use states. Current async actions can be clicked repeatedly and provide no in-control busy state.

## Other useful product features

### Near term

1. **Settings screen:** enforce auto-lock, clipboard duration, theme, lock-on-minimize, and backup preferences that already exist in the data model.
2. **Recovery management:** recovery unlock, recovery-key verification, replacement, and clear warnings about losing both factors.
3. **Secure notes UI:** `VaultItemType::SecureNote` exists but the UI always creates `login` items.
4. **Password history and undo:** retain a small encrypted history per item and offer recoverable deletion.
5. **Offline credential health:** identify reused, weak, old, and missing passwords without sending vault data anywhere.
6. **TOTP:** encrypted seed storage, masked seed display, copy countdown, and clock-skew handling.
7. **Keyboard workflow:** command palette or documented shortcuts for search, new, save, copy username, copy password, lock, and pane switching.
8. **Backups UI:** file dialogs, scheduled encrypted backups, restore preview, retention, and verified restore tests.

### Later, after the security boundary is hardened

- Browser integration/autofill with a separately threat-modeled, authenticated native-messaging channel.
- Optional encrypted synchronization with conflict handling and end-to-end key separation. This materially changes the current local-only trust model and should be a separate design project.
- Password-breach checking only through an explicit privacy-preserving design, with clear network disclosure and an offline/disabled default.

## Architecture and maintainability

The generated project graph contains 249 nodes, 564 relationships, and 15 communities. `VaultService` is the most connected node and bridges storage, cryptography, Tauri commands, and domain models. That is reasonable for a small prototype, but it will become a change bottleneck as settings, recovery, history, TOTP, and syncing arrive.

Recommended decomposition:

- Keep cryptographic formats and primitives in `qiring-crypto` with versioned, testable APIs.
- Give persistence its own atomic-file and migration layer; do not make `VaultService` responsible for filesystem protocol details.
- Split domain services into session/lock, item management, profile/generator, recovery/key management, and backup/restore.
- Split the 904-line HTML file into semantic HTML, CSS, and small modules for routing, Tauri API calls, Ring, Qi editor, profiles, toasts, and dialogs. A framework is optional; modularity and tests are the requirement.
- Introduce a typed frontend adapter that is the only code allowed to invoke Tauri commands.
- Version commands and persisted formats explicitly and add migrations before schema v2.

## Test plan additions

The current suite has 9 passing tests: 4 core, 3 crypto, 2 storage, and no desktop/UI tests.

Add these before release:

- Tamper tests for nonces, ciphertext, wrapped keys, metadata, schema, KDF params, and truncated/oversized files.
- Property tests for password length/composition constraints and unbiased category coverage.
- Fuzz targets for vault, backup, notes metadata, and profile parsing.
- Recovery create/unlock/rotate tests and existing-vault overwrite refusal.
- Idle, minimize, suspend, lock, and clipboard timer tests using a fake clock.
- Atomic save/restore failure injection and Windows/macOS/Linux filesystem tests.
- Tauri command authorization, URL scheme, and path-scope tests.
- UI tests for every screen, context action set, unsaved changes, keyboard-only operation, focus restoration, and toast announcements.
- Cross-platform native dropdown and 200% scaling screenshots.
- Release smoke tests that install the signed bundle and open a temporary test vault.

## Verification performed

| Check | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Pass |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | Pass |
| `cargo test --workspace` | Pass: 9 tests |
| `cargo audit` | Fail: 2 vulnerabilities, plus warnings described above |
| Live UI render | Reviewed Create Vault, Password Profiles, and Qi/Ring at 1280 px width |
| Architecture graph | 249 nodes, 564 edges, 15 communities; no import cycles detected |

## Suggested implementation sequence

1. **Security boundary milestone:** URL opener, CSP, DOM-safe rendering, capability/command minimization, dependency updates, vault overwrite refusal.
2. **Session safety milestone:** real auto-lock, lock-on-lifecycle events, password remasking, clipboard ownership, secret clearing.
3. **Recovery and persistence milestone:** usable recovery unlock, metadata authentication/bounds, permissions, atomic save/import, parser caps and fuzzing.
4. **UI shell milestone:** context header actions, authenticated routing, toast system, window minimums, responsive pane sizing, tighter density, accessibility foundations.
5. **Profile milestone:** encrypted master-detail profiles, min/max composition model, guaranteed generator, property tests.
6. **Release milestone:** bundled/signable artifacts, SBOM/provenance, install smoke tests, expanded threat model, and independent security review.

## References

- [Tauri Content Security Policy](https://v2.tauri.app/security/csp/)
- [Tauri Capabilities](https://v2.tauri.app/security/capabilities/)
- [Tauri Permissions](https://v2.tauri.app/security/permissions/)
- [Tauri scoped opener API](https://v2.tauri.app/reference/javascript/opener/)
- [Tauri window configuration reference](https://v2.tauri.app/reference/config/#windowconfig)
- [Web Interface Guidelines](https://raw.githubusercontent.com/vercel-labs/web-interface-guidelines/main/command.md)
- `docs/threat-model.md`
- `graphify-out/GRAPH_REPORT.md`
