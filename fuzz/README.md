# Parser fuzzing

Install `cargo-fuzz`, then run these targets from this directory:

- `cargo fuzz run parse_vault`
- `cargo fuzz run parse_backup`
- `cargo fuzz run parse_profile`
- `cargo fuzz run parse_item`

Together they exercise vault/backup schema dispatch, input-size and KDF bounds, nonce parsing, password-profile constraints, and secure-note/item metadata without invoking Argon2 or touching the filesystem.
