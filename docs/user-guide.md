# QiRing user guide

QiRing stores its vault locally. Create a vault with a unique master password, complete the recovery ceremony, and keep the recovery key offline. Recovery rotates both the master-password wrapping and recovery-key wrapping, so the key used for recovery immediately becomes obsolete.

## Keyboard workflow

Use `Ctrl` on Windows/Linux or `Command` on macOS:

| Shortcut | Action |
| --- | --- |
| `Ctrl/Command + K` | Open Vault and focus search |
| `Ctrl/Command + N` | New Qi or new password profile in the active module |
| `Ctrl/Command + S` | Save the active Qi, profile, or settings screen |
| `Ctrl/Command + Shift + U` | Copy the current username |
| `Ctrl/Command + Shift + P` | Copy the current password |
| `Ctrl/Command + L` | Lock the vault |
| `Ctrl/Command + 1…5` | Switch between Vault, Profiles, Health, Backups, and Settings |
| `Escape` | Close the menu and remask the visible password |

The unlock-method tabs support Left/Right, Home, and End. The application menu supports Up/Down, Home, End, and Escape.

QiRing prompts to save, discard, or stay when you leave an edited screen, switch to another Qi, or manually lock the vault. Saving Settings before lock makes theme and button-display changes available after the next unlock.

## Ring organization

The sort control above Ring search cycles through **Custom**, **A–Z**, and **Z–A**. Alphabetic modes sort both category names and the Qi names inside each category. Custom mode preserves its own category and Qi order even while you temporarily use an alphabetic mode.

In Custom mode, drag the grip beside a category or Qi to reorder it. Qi can be reordered within its current category; edit the Qi's Category field to move it to another category. Dragging is disabled while search is filtering the Ring and in A–Z or Z–A mode. For a keyboard alternative, focus a Qi grip and use Up/Down or Home/End; focus a category header and use Alt+Up/Down or Alt+Home/End.

## Appearance and window placement

Settings can show supported buttons as **Icon + label** (the default), **Icon only**, or **Label only**. Icon-only buttons retain accessible names and show their label as a hover tooltip. Navigation menu labels always remain visible so destinations do not become ambiguous.

QiRing remembers the main window's last normal size, position, and maximized state. At startup it checks the current monitors, clamps the saved rectangle to an available display, and centers it on the primary display when the previous monitor is no longer present. This prevents a prior multi-monitor layout or resolution change from reopening the title bar off screen.

## Qi icons and favicons

Each Qi entry can store a PNG, JPEG, WebP, GIF, or ICO image up to 512 KiB. Use **Upload** to choose a local image or **From website** to request the site's `/favicon.ico`. Save the Qi after the preview appears. The image is stored with the Qi inside the encrypted vault and appears in the Ring index.

Favicon import is explicit and direct: QiRing does not use a third-party favicon service. It connects only to standard HTTP/HTTPS ports, disables proxies for the request, pins a validated public DNS result, revalidates redirects, blocks local/private/reserved addresses, and enforces image-type, response-size, redirect, and timeout limits. Requesting a favicon reveals your IP address and the site's hostname to that site; use Upload instead when that disclosure is undesirable.

## Clipboard limits

QiRing clears a copied value only if the clipboard still contains that exact QiRing-owned value. Copying something else prevents QiRing from erasing the newer clipboard contents. Clipboard managers and operating-system history may retain prior values beyond QiRing’s control; disable clipboard history or use it cautiously for sensitive credentials.

## Backups

Manual exports are encrypted with a separate backup passphrase and require a system file-dialog selection. Always preview an import before restore. An atomic restore locks the current session. Automatic snapshots contain the already-encrypted vault and are retained according to Settings.

## TOTP

TOTP codes use the device clock and the standard 30-second window. If a code is rejected, verify that automatic date and time are enabled on the device before requesting a fresh code.
