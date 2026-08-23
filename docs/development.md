# QiRing development and builds

This guide covers contributor setup, validation, native bundles, data locations, portable mode, and Linux desktop troubleshooting. User-facing behavior is documented in the [user guide](user-guide.md); release signing and publication are documented in the [release process](release-process.md).

## Toolchain

QiRing currently uses:

- Rust `1.93.1`, selected automatically by `rust-toolchain.toml`
- Node.js 24 LTS and npm
- Tauri 2 and Vite
- Native Tauri prerequisites for the host operating system

Install [Rust with rustup](https://www.rust-lang.org/tools/install), Node.js 24 LTS, and the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/). Install the locked frontend dependencies from the repository root:

```bash
npm --prefix apps/desktop ci
```

## Run locally

Start Tauri with Vite hot reload on any supported host:

```bash
cd apps/desktop
npm run tauri -- dev
```

On Linux, the repository launcher applies safe defaults for known WebKitGTK and GBM rendering problems:

```bash
./scripts/run-desktop.sh
```

The default backend respects the current desktop session. Use `--x11` or `--wayland` to select a backend explicitly:

```bash
./scripts/run-desktop.sh --x11
./scripts/run-desktop.sh --wayland
```

When X11 is selected and `DISPLAY` is unset, the launcher probes `:0`, `:1`, and `:2`. Wayland restores size and maximized state, but the compositor controls absolute placement. X11/XWayland can restore the saved position as well.

If a graphics driver still produces a black WebView, opt into software rendering:

```bash
QIRING_SOFTWARE_RENDERING=1 ./scripts/run-desktop.sh
```

## Validate the workspace

Run the same principal checks used by CI:

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

## Build commands

Run these shortcuts from the repository root after installing frontend dependencies:

| Command | Result |
| --- | --- |
| `npm run build:native` | All configured bundles supported by the current host |
| `npm run build:linux` | AppImage and DEB |
| `npm run build:linux-appimage` | AppImage only |
| `npm run build:linux-appimage-portable` | AppImage plus portable marker |
| `npm run build:linux-deb` | DEB only |
| `npm run build:windows` | MSI |
| `npm run build:windows-portable` | Standalone executable plus portable marker |
| `npm run build:macos` | Native-architecture DMG |
| `npm run build:macos-universal` | Universal Intel/Apple Silicon DMG |
| `npm run build:binary` | Unbundled release executable |
| `npm run build:binary-debug` | Unbundled debug executable |

Native installers must be built on their corresponding operating systems: AppImage and DEB on Linux, DMG on macOS, and MSI on Windows. Public tagged releases additionally require signing credentials.

### Artifact locations

| Artifact | Location |
| --- | --- |
| AppImage | `target/release/bundle/appimage/*.AppImage` |
| DEB | `target/release/bundle/deb/*.deb` |
| macOS DMG | `target/release/bundle/dmg/*.dmg` |
| macOS app | `target/release/bundle/macos/QiRing.app` |
| Windows MSI | `target/release/bundle/msi/*.msi` |
| Unbundled executable | `target/release/qiring-desktop` or `qiring-desktop.exe` |
| Portable archives | `target/release/bundle/portable/` |

The local portable shortcuts create the launcher and adjacent `qiring-portable` marker. The release workflow runs `scripts/package-portable.mjs` afterward to place both files in a Linux `.tar.gz` or Windows `.zip` archive.

### Custom artifact names

Set `QIRING_OUTPUT_NAME` to replace the generated artifact basename while retaining the appropriate extension:

```bash
QIRING_OUTPUT_NAME=QiRing-Preview npm run build:linux-appimage
```

In PowerShell:

```powershell
$env:QIRING_OUTPUT_NAME = "QiRing-Preview"
npm run build:windows
Remove-Item Env:QIRING_OUTPUT_NAME
```

Renaming changes only the output filename. The embedded application version and package metadata remain authoritative.

## Platform notes

### Linux

Install the WebKitGTK 4.1, GTK 3, AppIndicator, librsvg, OpenSSL, build-tool, and packaging dependencies specified by the Tauri prerequisites for your distribution. AppImage execution may also require FUSE 2.

The AppImage commands set `NO_STRIP=1` because some `linuxdeploy` environments cannot strip every bundled binary. This does not change application behavior.

Run an AppImage directly:

```bash
chmod +x target/release/bundle/appimage/*.AppImage
target/release/bundle/appimage/*.AppImage
```

### macOS

Install Apple's command-line development tools:

```bash
xcode-select --install
```

For a universal build, install both Rust targets before running the universal shortcut:

```bash
rustup target add x86_64-apple-darwin aarch64-apple-darwin
npm run build:macos-universal
```

### Windows

Install Microsoft C++ Build Tools with **Desktop development with C++**, the Edge WebView2 Runtime, Node.js 24 LTS, and Rustup using the MSVC toolchain. MSI creation uses WiX and requires the Windows VBSCRIPT optional feature.

If Rust uses a GNU host, select MSVC in PowerShell:

```powershell
rustup default stable-msvc
```

## Frontend and Rust-only builds

Build only the embedded frontend:

```bash
cd apps/desktop
npm run build
```

Vite writes the result to `apps/desktop/web-dist`. It is embedded in Tauri and is not a supported standalone web application.

Build the Rust workspace without creating installers:

```bash
cargo build --workspace --release
```

## Ring storage

Normal development runs and installed applications use private per-user directories under the `app.qiring.desktop` identifier:

| Platform | Encrypted Ring | Window state and pre-unlock preferences |
| --- | --- | --- |
| Linux | `$XDG_DATA_HOME/app.qiring.desktop/vault.qiring`, or `~/.local/share/app.qiring.desktop/vault.qiring` | `$XDG_CONFIG_HOME/app.qiring.desktop/`, or `~/.config/app.qiring.desktop/` |
| macOS | `~/Library/Application Support/app.qiring.desktop/vault.qiring` | `~/Library/Application Support/app.qiring.desktop/` |
| Windows | `%APPDATA%\app.qiring.desktop\vault.qiring` | `%APPDATA%\app.qiring.desktop\` |

## Portable mode

Portable mode is supported by AppImage and standalone Windows builds. It keeps QiRing-owned persistent files beside the launcher:

```text
QiRing.AppImage or qiring-desktop.exe
qiring-portable
QiRingData/
  vault.qiring
  window-state.json
  ui-preferences.json
  restore-safety/
```

Use a portable build shortcut, place an empty `qiring-portable` marker beside an existing supported launcher, launch an AppImage with `--portable`, or set `QIRING_PORTABLE=1`. Move the launcher, marker, and `QiRingData` together.

Portable mode does not apply to DEB, MSI-installed, or macOS application bundles because their installation locations may be read-only, shared, replaced during upgrades, or protected by signing rules. Manual backup exports, recovery-key files, and user-selected automatic-backup directories remain in their selected locations.

`QiRing.AppDir` is an intermediate Linux bundle directory used to assemble an AppImage. It is not part of either normal or portable runtime operation and should not be distributed.

## Legacy data migration

When QiRing detects an older application identifier or storage location, it validates and copies a single unambiguous Ring into the current location without deleting the source. If different Ring identities exist in multiple legacy locations, QiRing refuses to choose automatically and reports the paths for manual resolution.
