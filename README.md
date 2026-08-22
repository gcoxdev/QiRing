# QiRing

QiRing is a security-first, local-only desktop password manager.

## Workspace layout

- `apps/desktop/src-tauri`: Tauri desktop backend commands
- `crates/qiring-crypto`: cryptographic primitives and key hierarchy
- `crates/qiring-storage`: encrypted vault file persistence
- `crates/qiring-core`: domain logic, session state, CRUD, backup operations

## Development and builds

QiRing uses Rust 1.93.1, Node.js, npm, Vite, and Tauri 2. Node.js 24 LTS is used by CI and is the recommended version for reproducible builds. The repository's `rust-toolchain.toml` makes `rustup` select the required Rust toolchain automatically.

Install [Rust with rustup](https://www.rust-lang.org/tools/install), [Node.js 24 LTS](https://nodejs.org/), and the [Tauri system prerequisites](https://v2.tauri.app/start/prerequisites/) for the host operating system. Then install the locked frontend dependencies from the repository root:

```bash
npm --prefix apps/desktop ci
```

The configured native distribution targets are:

| Host operating system | Bundle targets | Output |
| --- | --- | --- |
| Linux | AppImage and DEB | `target/release/bundle/appimage/*.AppImage`, `target/release/bundle/deb/*.deb` |
| macOS | DMG and the enclosed application bundle | `target/release/bundle/dmg/*.dmg`, `target/release/bundle/macos/QiRing.app` |
| Windows | MSI | `target/release/bundle/msi/*.msi` |

Native installers must be built on their corresponding operating system: MSI on Windows, DMG on macOS, and AppImage/DEB on Linux. The current project is not configured for Android, iOS, RPM, Flatpak, Snap, AUR, NSIS, or app-store packages.

### Build shortcuts

The following shortcuts can be run from the repository root after installing the desktop dependencies with `npm --prefix apps/desktop ci`:

| Command | Result |
| --- | --- |
| `npm run build:native` | All bundle formats configured for the current operating system |
| `npm run build:linux` | AppImage and DEB |
| `npm run build:linux-appimage` | AppImage only |
| `npm run build:linux-appimage-portable` | AppImage plus a portable-mode marker |
| `npm run build:linux-deb` | DEB only |
| `npm run build:windows` | Windows MSI |
| `npm run build:windows-portable` | Standalone Windows executable plus a portable-mode marker |
| `npm run build:macos` | macOS DMG |
| `npm run build:macos-universal` | Universal Intel/Apple Silicon DMG |
| `npm run build:binary` | Unbundled release executable |
| `npm run build:binary-debug` | Unbundled debug executable |

Platform bundles still must be built on their corresponding operating system. The same shortcuts are also available when working directly inside `apps/desktop`.

The AppImage-producing shortcuts, including the portable shortcut, set `NO_STRIP=1` for `linuxdeploy`. This avoids the `failed to run linuxdeploy` failure seen on distributions where `linuxdeploy` cannot strip one or more bundled binaries. The combined `build:linux` shortcut sets it as well because that command also produces an AppImage.

### Control the output filename

Tauri normally includes the application version and architecture in bundled filenames, such as `QiRing_0.1.0_amd64.AppImage`. Set `QIRING_OUTPUT_NAME` to replace that generated basename for any build shortcut. Supply a filename only—without `.AppImage`, `.deb`, `.dmg`, `.msi`, or `.exe`—and QiRing retains the appropriate extension.

On Linux or macOS:

```bash
QIRING_OUTPUT_NAME=QiRing-Manjaro npm run build:linux-appimage-portable
```

That example produces `target/release/bundle/appimage/QiRing-Manjaro.AppImage`. The portable marker is created beside the renamed file as usual.

In Windows PowerShell:

```powershell
$env:QIRING_OUTPUT_NAME = "QiRing-Setup"
npm run build:windows
Remove-Item Env:QIRING_OUTPUT_NAME
```

The Windows example produces `target\release\bundle\msi\QiRing-Setup.msi`. The same variable works with `build:native`, `build:binary`, `build:binary-debug`, `build:linux`, `build:linux-appimage`, `build:linux-deb`, `build:windows-portable`, `build:macos`, and `build:macos-universal`. Combined builds reuse the basename with each format's extension. If the variable is omitted, Tauri's normal versioned filenames remain unchanged. Renaming affects only the artifact filename; the embedded application version and package metadata remain authoritative.

### Validate the workspace

Run the core test suite:

```bash
cargo test --workspace
```

The principal CI checks can be run locally with:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace

cd apps/desktop
npm audit --audit-level=moderate
npm run build
npm run test:ui-contract
npx playwright install chromium
npm run test:e2e
```

### Run a development build

On any supported desktop host, start Tauri with Vite hot reload:

```bash
cd apps/desktop
npm run tauri -- dev
```

Linux also has a repository launcher that runs the Rust desktop application with safe defaults for known WebKitGTK/GBM rendering issues:

```bash
./scripts/run-desktop.sh
```

The Linux launcher sets `WEBKIT_DISABLE_DMABUF_RENDERER=1` to avoid known WebKitGTK/GBM buffer failures while leaving compositing enabled for correct repaint behavior. Its default `auto` backend respects the current desktop session. Wayland restores the saved size and maximized state, but the compositor controls absolute placement. To restore the exact saved position, launch through X11/XWayland:

```bash
./scripts/run-desktop.sh --x11
```

Use `--wayland` to explicitly select Wayland, or set `QIRING_WINDOW_BACKEND=auto|x11|wayland`. When X11 is requested and `DISPLAY` is unset, the launcher probes `:0`, `:1`, and `:2`. The backend flags are consumed by the launcher and are not passed to Cargo.

If a Linux graphics driver still produces a black WebView, opt into the slower software-rendering fallback:

```bash
QIRING_SOFTWARE_RENDERING=1 ./scripts/run-desktop.sh
```

### Build the frontend only

```bash
cd apps/desktop
npm run build
```

The static output is written to `apps/desktop/web-dist`. It is intended to be embedded in the Tauri application and is not a standalone browser version of QiRing.

### Build the Rust workspace only

```bash
cargo build --workspace --release
```

This compiles the Rust crates and desktop backend without creating a platform installer.

### Build an unbundled desktop executable

From the repository root, build a release executable without an installer:

```bash
npm run build:binary
```

The executable is written to `target/release/qiring-desktop` on Linux/macOS or `target/release/qiring-desktop.exe` on Windows. For a debug executable, use:

```bash
npm run build:binary-debug
```

Debug output is written under `target/debug`.

### Build Linux bundles

Install the native prerequisites before building. On Arch Linux or Manjaro:

```bash
sudo pacman -Syu
sudo pacman -S --needed \
  webkit2gtk-4.1 \
  base-devel \
  curl \
  wget \
  file \
  openssl \
  appmenu-gtk-module \
  libappindicator \
  librsvg \
  xdotool \
  patchelf \
  fuse2
```

`fuse2` is needed to run AppImages. On Debian or Ubuntu:

```bash
sudo apt update
sudo apt install \
  libwebkit2gtk-4.1-dev \
  build-essential \
  curl \
  wget \
  file \
  libxdo-dev \
  libssl-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev \
  patchelf
```

Build both configured Linux formats:

```bash
npm run build:linux
```

Build just one format when desired:

```bash
npm run build:linux-appimage
npm run build:linux-appimage-portable
npm run build:linux-deb
```

An AppImage can run directly after making it executable:

```bash
chmod +x target/release/bundle/appimage/*.AppImage
target/release/bundle/appimage/*.AppImage
```

Install a DEB on Debian or Ubuntu with:

```bash
sudo apt install target/release/bundle/deb/*.deb
```

AppImage is the recommended output for Manjaro and other non-Debian distributions.

The portable AppImage shortcut also writes `qiring-portable` beside the AppImage. Keep both files together. On first launch, QiRing creates a private `QiRingData` directory beside them.

### Build macOS bundles

Install Apple's command-line development tools first:

```bash
xcode-select --install
```

Then build the configured DMG on a Mac:

```bash
npm run build:macos
```

Open the resulting DMG and drag QiRing into Applications:

```bash
open target/release/bundle/dmg/*.dmg
```

The default build uses the Mac's native architecture. To create a universal Intel/Apple Silicon DMG, install both Rust targets and select Tauri's universal target:

```bash
rustup target add x86_64-apple-darwin aarch64-apple-darwin
npm run build:macos-universal
```

Universal artifacts are written below `target/universal-apple-darwin/release/bundle`.

### Build Windows bundles

Install the following on Windows before building:

- Microsoft C++ Build Tools with **Desktop development with C++** selected
- Microsoft Edge WebView2 Runtime; it is normally already installed on current Windows 10 and Windows 11 systems
- Node.js 24 LTS and npm
- Rustup using the MSVC host toolchain

If Rust was previously configured with a GNU host, select MSVC in PowerShell:

```powershell
rustup default stable-msvc
```

MSI creation also requires the Windows VBSCRIPT optional feature. It is normally enabled by default; if WiX reports that it cannot run `light.exe`, enable VBSCRIPT under **Settings > Apps > Optional features > More Windows features**.

Build the MSI from PowerShell or Command Prompt on Windows:

```powershell
npm run build:windows
```

The resulting MSI can be opened normally or installed from PowerShell:

```powershell
$installer = Get-ChildItem target\release\bundle\msi\*.msi | Select-Object -First 1
Start-Process msiexec.exe -Wait -ArgumentList '/i', $installer.FullName
```

To create a standalone portable Windows build instead of an installer:

```powershell
npm run build:windows-portable
```

Distribute `target\release\qiring-desktop.exe` and the adjacent `qiring-portable` marker together. QiRing creates `QiRingData` beside them at first launch. This mode is intended for a standalone executable in a user-writable private folder, not an executable installed under `Program Files`.

### Ring data and portable mode

QiRing's application identifier is `app.qiring.desktop`. Normal development runs and installed DEB, MSI, and macOS builds use the operating system's per-user application directories:

| Platform | Encrypted Ring | Window state and pre-unlock theme |
| --- | --- | --- |
| Linux | `$XDG_DATA_HOME/app.qiring.desktop/vault.qiring`, or `~/.local/share/app.qiring.desktop/vault.qiring` | `$XDG_CONFIG_HOME/app.qiring.desktop/`, or `~/.config/app.qiring.desktop/` |
| macOS | `~/Library/Application Support/app.qiring.desktop/vault.qiring` | `~/Library/Application Support/app.qiring.desktop/` |
| Windows | `%APPDATA%\app.qiring.desktop\vault.qiring` | `%APPDATA%\app.qiring.desktop\` |

Explicit portable mode is supported for AppImage and standalone Windows builds. It stores QiRing-owned persistent files in one private sidecar directory:

```text
QiRing.AppImage or qiring-desktop.exe
qiring-portable
QiRingData/
  vault.qiring
  window-state.json
  ui-preferences.json
  restore-safety/
```

Use the portable build shortcuts above, place an empty `qiring-portable` marker beside an existing supported launcher, launch an AppImage with `--portable`, or set `QIRING_PORTABLE=1`. Move the launcher, marker, and `QiRingData` together when relocating the application. Manual backup exports, recovery-key files, and user-selected automatic-backup directories remain wherever the user chose to save them.

Portable mode deliberately does not apply to DEB, MSI-installed, or macOS application bundles. Those locations can be read-only, shared between users, replaced during upgrades, or protected by code-signing rules. QiRing exits with a clear error if portable mode is requested on an unsupported build or its sidecar cannot be secured. Framework-owned WebView cache files may still use the operating system's standard cache/data location; no Ring contents or QiRing settings are stored there.

On the first launch after the identifier change, QiRing looks for the prior `dev.qiring.desktop` paths and the older Linux/macOS/Windows Ring location. If exactly one Ring identity is found, it validates and copies that encrypted file into the current location, then validates the copy. Window state and file-based UI preferences are copied separately when valid. Original files are never deleted. If different Ring identities are found, QiRing refuses to guess and reports the paths so the intended `vault.qiring` can be selected manually. A theme previously stored only in WebView local storage becomes available again after the first successful unlock, when the encrypted Ring setting is copied into `ui-preferences.json`.

### Build all bundles configured for the current host

This shortcut asks Tauri to select every configured bundle format that applies to the current operating system:

```bash
npm run build:native
```

Local bundles are unsigned unless platform signing credentials are configured. Public tagged releases must be signed; macOS releases must also be notarized. See [the release process](docs/release-process.md) for signing variables, checksums, SBOM generation, provenance attestations, and the GitHub Actions release flow.

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
