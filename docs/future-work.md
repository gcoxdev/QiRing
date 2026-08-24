# QiRing future work

## Near-term, fits current local-only scope

1. **Possible duplicate login detection.** Extend the Health report to flag entries only when both the normalized URL and username match. Treat matches as review suggestions, not errors, because intentional duplicates are still possible.
2. **Vault-wide search filters by field state.** E.g. "items with no password set," "items last modified > 1 year ago," building on the existing health-report infrastructure.

## Later, after further design work (still local-only)

These don't have open security blockers the way sync/autofill do, but they're larger scope and deserve their own design pass before implementation:

- Bulk item operations (multi-select tagging, category moves, bulk delete).

## Explicitly out of scope until a separate security design exists

These materially change QiRing's local-only trust model and each needs its own threat-modeled design before implementation:

- **Browser integration / autofill**, via a separately threat-modeled, authenticated native-messaging channel.
- **Encrypted synchronization**, with conflict handling and end-to-end key separation.
- **Online password-breach checking**, only through an explicit privacy-preserving design (e.g. k-anonymity range queries), with clear network disclosure and an offline/disabled default.
