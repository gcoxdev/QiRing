const COMMANDS: &[&str] = &[
    "create_vault",
    "unlock_vault_master",
    "unlock_vault_recovery",
    "regenerate_recovery_key",
    "save_recovery_key_dialog",
    "prepare_recovery_print",
    "lock_vault",
    "touch_activity",
    "add_item",
    "update_item",
    "delete_item",
    "undo_delete",
    "list_items",
    "get_item",
    "get_totp_code",
    "generate_password",
    "list_profiles",
    "save_profile",
    "delete_profile",
    "get_settings",
    "update_settings",
    "get_bootstrap_theme",
    "set_bootstrap_theme",
    "health_report",
    "choose_backup_directory",
    "select_item_icon_dialog",
    "fetch_favicon",
    "export_backup_dialog",
    "select_backup_file",
    "preview_selected_backup",
    "import_selected_backup",
    "save_csv_template_dialog",
    "export_plaintext_csv_dialog",
    "select_plaintext_csv_file",
    "preview_selected_plaintext_csv",
    "import_selected_plaintext_csv",
    "list_snapshots",
    "restore_snapshot",
    "rotate_master_password",
    "get_security_status",
    "vault_exists",
    "copy_secret",
];

fn main() {
    // Vite runs before Cargo, so force Tauri to re-embed the freshly built assets.
    println!("cargo:rerun-if-changed=../web-dist");
    let manifest = tauri_build::AppManifest::new().commands(COMMANDS);
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(manifest))
        .expect("failed to generate Tauri build context and command permissions");
}
