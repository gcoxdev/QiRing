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

If `DISPLAY` is not set, it probes `:0`, `:1`, `:2` and picks the first working X display.

## Current UI flow

The desktop UI currently supports:

- Startup flow:
  - If vault does not exist: Create Vault screen only
  - If vault exists: Unlock screen only
  - QiRing workspace is shown only after unlock
- QiRing has two equal-sized panes:
  - `Ring`: simple categorized expandable/collapsible list of Qi names + search/clear
  - `Qi`: tabbed editor (`Info`, `Key`, `Questions`)
- Menu-driven options (outside Qi/Ring pane controls):
  - Open QiRing workspace
  - Open Password Profiles management screen
  - Switch among the 5 view layouts (Vertical 1/2, Horizontal 1/2, Compact)
- Qi Info tab:
  - Category (existing or new), Qi Name, Tags, URL with external open button
  - Open URL action automatically switches to Key tab
- Qi Key tab:
  - Username copy
  - Password copy/show-hide
  - Generate from selected saved password profile
- Qi Questions tab:
  - Security question/answer rows with answer copy buttons
- Qi actions:
  - `New Qi`, `Save Qi`, `Delete Qi` (with confirmation prompt)
