# QiRing release process

The release workflow builds native Tauri installers on Linux, macOS, and Windows from the pinned Rust and Node dependency locks. Linux artifacts include DEB, AppImage, and a portable AppImage archive; Windows artifacts include MSI and a portable executable archive. Portable archives keep the launcher beside the required `qiring-portable` marker. Each platform artifact is accompanied by an SPDX JSON SBOM, SHA-256 checksums, and a GitHub build-provenance attestation. GitHub Actions are pinned to immutable commit SHAs.

## Signing secrets

Configure only the secrets needed by the target platform:

- `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, and `KEYCHAIN_PASSWORD` for Apple signing. `APPLE_SIGNING_IDENTITY` is optional because the workflow can derive it.
- Either `APPLE_API_ISSUER`, `APPLE_API_KEY` (key ID), and `APPLE_API_PRIVATE_KEY` (the `.p8` contents), or `APPLE_ID`, `APPLE_PASSWORD`, and `APPLE_TEAM_ID`, for Apple notarization.
- `WINDOWS_CERTIFICATE`, `WINDOWS_CERTIFICATE_PASSWORD`, and the certificate-provider `WINDOWS_TIMESTAMP_URL` for Windows signing.
- `LINUX_GPG_PRIVATE_KEY` (base64-encoded export), `LINUX_GPG_KEY_ID`, and `LINUX_GPG_PASSPHRASE` for AppImage signing.
- `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` when updater artifacts are enabled in a future release.

Unsigned workflow-dispatch builds are useful for smoke testing. Tagged builds fail before compilation if any platform signing/notarization credential set is incomplete. The workflow imports the platform keys, verifies Windows/macOS signatures and Apple stapling, and checks that the AppImage contains a signature. Keep private keys only in protected environment secrets.

## Publishing

1. Confirm CI passes and update the application version in the workspace and Tauri configuration.
2. Create and push a signed `v*` Git tag.
3. The workflow tests and bundles on each platform. It extracts the Linux AppImage/DEB and performs administrative MSI extraction on Windows; macOS verifies the DMG and application bundle before inventory, checksums, attestations, and upload.
4. Verify `SHA256SUMS`, the GitHub attestation, installer signatures, first-run vault creation, unlock/recovery, and upgrade behavior on clean platform VMs before announcing the release.
