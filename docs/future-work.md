# QiRing future work

Consolidated, deduplicated product and feature ideas from [project-assessment-2026-08-08.md](./project-assessment-2026-08-08.md) and [project-assessment-2026-08-09.md](./project-assessment-2026-08-09.md). This file supersedes the "Other useful product features" and "New feature opportunities" sections of those two documents as the single place to track outstanding ideas — check here before re-proposing something.

Items already implemented since the 2026-08-08 assessment (Settings screen, secure notes, password history/undo, offline health report, TOTP, documented keyboard workflow, backups UI) are intentionally omitted; see [assessment-remediation-2026-08-08.md](./assessment-remediation-2026-08-08.md) for that record.

## Near-term, fits current local-only scope

1. **Import from other password managers.** CSV/JSON import from 1Password, Bitwarden, Chrome/Firefox exports, etc. Likely the single biggest adoption blocker — there is currently no on-ramp for someone with an existing vault elsewhere. Should reuse the existing bounded/previewed import pipeline built for QiRing's own backups.
2. **Password strength meter at entry time — Implemented.** The Qi editor now rates passwords live from weak through very strong using length and character variety, while keeping the existing after-the-fact Health report as a separate audit.
3. **Duplicate-URL / duplicate-username detection.** Extend the Health report to catch accidental double-entry of the same login, not just reused passwords.
4. **Per-item custom fields — Implemented.** Qi entries now support up to 50 labeled values for PINs, security codes, membership numbers, and similar data; values can be concealed, are encrypted with the item, and participate in Ring search.
5. **Vault-wide search filters by field state.** E.g. "items with no password set," "items last modified > 1 year ago," building on the existing health-report infrastructure.
6. **Recovery-key QR code — Implemented.** The recovery ceremony can now render the key as an opt-in, locally generated QR code for scanning or printing, and clears the QR canvas when it is hidden or the ceremony ends.
7. **Portable single-file encrypted export**, distinct from the existing backup format, aimed at manual archival/migration rather than QiRing-to-QiRing restore.

## Later, after further design work (still local-only)

These don't have open security blockers the way sync/autofill do, but they're larger scope and deserve their own design pass before implementation:

- Bulk item operations (multi-select tagging, category moves, bulk delete).
- Vault-level export/print of a redacted (no-password) inventory for offline reference.
- Configurable password-generator presets beyond the current per-profile model (e.g. quick-pick length/complexity without opening Profiles).

## Explicitly out of scope until a separate security design exists

Carried forward unchanged from both prior assessments — these materially change QiRing's local-only trust model and each needs its own threat-modeled design before any implementation work starts:

- **Browser integration / autofill**, via a separately threat-modeled, authenticated native-messaging channel.
- **Encrypted synchronization**, with conflict handling and end-to-end key separation.
- **Online password-breach checking**, only through an explicit privacy-preserving design (e.g. k-anonymity range queries), with clear network disclosure and an offline/disabled default.

## References

- [project-assessment-2026-08-08.md](./project-assessment-2026-08-08.md) — original baseline assessment (source of most "Later" and "Explicitly out of scope" items)
- [project-assessment-2026-08-09.md](./project-assessment-2026-08-09.md) — re-verification pass (source of most "Near-term" items)
- [assessment-remediation-2026-08-08.md](./assessment-remediation-2026-08-08.md) — record of what has already shipped
- [threat-model.md](./threat-model.md)
