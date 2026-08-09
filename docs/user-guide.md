# QiRing user guide

QiRing stores its vault locally. Create a vault with a unique master password, complete the recovery ceremony, and keep the recovery key offline. Recovery rotates both the master-password wrapping and recovery-key wrapping, so the key used for recovery immediately becomes obsolete.

## Keyboard workflow

Use `Ctrl` on Windows/Linux or `Command` on macOS:

| Shortcut | Action |
| --- | --- |
| `Ctrl/Command + K` | Open Qi Ring and focus search |
| `Ctrl/Command + N` | New Qi or new password profile in the active module |
| `Ctrl/Command + S` | Save the active Qi, profile, or settings screen |
| `Ctrl/Command + Shift + U` | Copy the current username |
| `Ctrl/Command + Shift + P` | Copy the current password |
| `Ctrl/Command + L` | Lock the vault |
| `Ctrl/Command + 1…5` | Switch between Qi Ring, Profiles, Health, Backups, and Settings |
| `Escape` | Close the menu and remask the visible password |

The unlock-method tabs support Left/Right, Home, and End. The application menu supports Up/Down, Home, End, and Escape.

## Clipboard limits

QiRing clears a copied value only if the clipboard still contains that exact QiRing-owned value. Copying something else prevents QiRing from erasing the newer clipboard contents. Clipboard managers and operating-system history may retain prior values beyond QiRing’s control; disable clipboard history or use it cautiously for sensitive credentials.

## Backups

Manual exports are encrypted with a separate backup passphrase and require a system file-dialog selection. Always preview an import before restore. An atomic restore locks the current session. Automatic snapshots contain the already-encrypted vault and are retained according to Settings.

## TOTP

TOTP codes use the device clock and the standard 30-second window. If a code is rejected, verify that automatic date and time are enabled on the device before requesting a fresh code.
