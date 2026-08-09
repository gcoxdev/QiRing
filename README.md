# QiRing

QiRing is a security-first, local-only desktop password manager.

## Workspace layout

- `apps/desktop/src-tauri`: Tauri desktop backend commands
- `crates/qiring-crypto`: cryptographic primitives and key hierarchy
- `crates/qiring-storage`: encrypted vault file persistence
- `crates/qiring-core`: domain logic, session state, CRUD, backup operations

## Quick start

```bash
cargo test -p qiring-crypto -p qiring-storage -p qiring-core
```

## Run desktop app

Use the launcher script from the repository root:

```bash
./scripts/run-desktop.sh
```

On Linux, the launcher sets safe defaults for known WebKitGTK/GBM rendering issues:

- `WEBKIT_DISABLE_DMABUF_RENDERER=1`
- `LIBGL_ALWAYS_SOFTWARE=1`

When X11 is requested and `DISPLAY` is not set, the launcher probes `:0`, `:1`, `:2` and picks the first working X display. In automatic mode it probes only when neither a Wayland nor X display is already available.

The default `auto` backend respects the current Linux desktop session. Wayland restores the saved size and maximized state, but the compositor controls absolute placement. To restore the exact saved position, launch through X11/XWayland:

```bash
./scripts/run-desktop.sh --x11
```

Use `--wayland` to explicitly select Wayland, or set `QIRING_WINDOW_BACKEND=auto|x11|wayland`. The backend flags are consumed by the launcher and are not passed to Cargo.

## Current UI flow

The desktop UI currently supports:

- Startup flow:
  - If a vault does not exist: Create Vault screen only, with a mandatory recovery-key ceremony (copy/save/print/verify/acknowledge) before the vault opens.
  - If a vault exists: Unlock screen only, with tabs for master-password and recovery-key unlock.
  - The authenticated vault shell is shown only after a successful unlock.
- Context-sensitive header actions sit immediately before the navigation **Menu** and change per module (Qi actions on the Vault view, profile actions on Password Profiles, Save on Settings, Export on Backups). **Lock Vault** stays in the menu.
- Modules, reachable from the menu or `Ctrl/Command + 1…6`:
  - **Vault**: a two-pane `Ring`/`Qi` layout. `Ring` is a categorized, expandable/collapsible, searchable and taggable list of Qi entries with drag-and-drop or keyboard custom ordering (plus A–Z/Z–A modes). `Qi` is a single-form editor (no tabs) with Info fields (category, name, tags, URL with external open), credential fields (username, password with copy/show-hide, TOTP with countdown), notes, security questions, and password history.
  - **Password Profiles**: a master-detail screen — a scrollable profile list on the left and a policy editor on the right (total length, per-class min/max ranges, allowed symbols, ambiguous-character filtering).
  - **Health**: an offline report of reused, weak, old, and missing passwords, computed locally from the decrypted vault.
  - **Backups**: manual passphrase-protected export/import with a mandatory preview step, plus automatic snapshot listing and restore.
  - **Settings**: session (auto-lock, clipboard clear, lock-on-minimize/blur), theme, button display, automatic-snapshot preferences, master-password rotation, and recovery-key replacement.
  - **Help**: in-app reference covering every page, setting, and keyboard shortcut.
- Qi actions: `New Qi`, `Save Qi`, `Delete Qi` (with a named confirmation and a short undo window).
