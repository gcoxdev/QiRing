use anyhow::Context;
use qiring_core::{
    AppSettings, BackupManifest, BackupPreview, BackupSnapshot, GeneratedPassword, HealthReport,
    ImportReport, ItemInput, ItemPatch, ItemSummary, ListFilter, PasswordPolicy, PasswordProfile,
    RecoveryMaterial, RecoveryUnlockResult, SecurityStatus, TotpCode, UnlockResult, VaultItem, VaultService,
    VaultSummary,
};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, State, WindowEvent};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_dialog::DialogExt;
use uuid::Uuid;
use zeroize::Zeroizing;

#[derive(Clone)]
struct AppState {
    service: Arc<Mutex<VaultService>>,
    clipboard: Arc<ClipboardGuard>,
    selected_backups: Arc<Mutex<HashMap<String, PathBuf>>>,
    approved_backup_directories: Arc<Mutex<HashSet<PathBuf>>>,
}

impl AppState {
    fn new(vault_path: PathBuf) -> Self {
        Self {
            service: Arc::new(Mutex::new(VaultService::new(vault_path))),
            clipboard: Arc::new(ClipboardGuard::default()),
            selected_backups: Arc::new(Mutex::new(HashMap::new())),
            approved_backup_directories: Arc::new(Mutex::new(HashSet::new())),
        }
    }
}

#[derive(Default)]
struct ClipboardGuard {
    owned: Mutex<OwnedClipboard>,
}

#[derive(Default)]
struct OwnedClipboard {
    value: Option<Zeroizing<String>>,
    expires_at: Option<Instant>,
}

#[derive(Serialize)]
struct CreateVaultResult {
    summary: VaultSummary,
    recovery: RecoveryMaterial,
}

#[derive(Serialize)]
struct SelectedBackup {
    token: String,
    display_path: String,
}

#[tauri::command]
fn create_vault(
    state: State<'_, AppState>,
    master_password: String,
    settings: Option<AppSettings>,
) -> Result<CreateVaultResult, String> {
    let master_password = Zeroizing::new(master_password);
    let mut service = lock_service(&state)?;
    let (summary, recovery) = service
        .create_vault(&master_password, settings.unwrap_or_default())
        .map_err(display_error)?;
    Ok(CreateVaultResult { summary, recovery })
}

#[tauri::command]
fn unlock_vault_master(state: State<'_, AppState>, master_password: String) -> Result<UnlockResult, String> {
    let master_password = Zeroizing::new(master_password);
    lock_service(&state)?
        .unlock_vault_master(&master_password)
        .map_err(display_error)
}

#[tauri::command]
fn unlock_vault_recovery(
    state: State<'_, AppState>,
    recovery_key: String,
    new_master_password: String,
) -> Result<RecoveryUnlockResult, String> {
    let recovery_key = Zeroizing::new(recovery_key);
    let new_master_password = Zeroizing::new(new_master_password);
    lock_service(&state)?
        .unlock_vault_recovery(&recovery_key, &new_master_password)
        .map_err(display_error)
}

#[tauri::command]
fn regenerate_recovery_key(
    state: State<'_, AppState>,
    master_password: String,
) -> Result<RecoveryMaterial, String> {
    let master_password = Zeroizing::new(master_password);
    lock_service(&state)?
        .regenerate_recovery_key(&master_password)
        .map_err(display_error)
}

#[tauri::command]
async fn save_recovery_key_dialog(app: AppHandle, recovery_key: String) -> Result<Option<String>, String> {
    let recovery_key = Zeroizing::new(recovery_key);
    let picker = app.clone();
    let selection = tauri::async_runtime::spawn_blocking(move || {
        picker
            .dialog()
            .file()
            .set_title("Save QiRing recovery key")
            .set_file_name("qiring-recovery-key.txt")
            .add_filter("Plain text", &["txt"])
            .blocking_save_file()
    })
    .await
    .map_err(|error| format!("recovery-key save dialog failed: {error}"))?;
    let Some(selection) = selection else {
        return Ok(None);
    };
    let path = selection
        .into_path()
        .map_err(|error| format!("selected file is not a local path: {error}"))?;
    let contents = Zeroizing::new(format!(
        "QiRing recovery key\n\n{}\n\nStore this file offline. Anyone with this key and the vault file can reset the vault master password.\n",
        recovery_key.as_str()
    ));
    qiring_storage::save_bytes_atomic(&path, contents.as_bytes()).map_err(display_error)?;
    Ok(Some(path.display().to_string()))
}

#[tauri::command]
fn lock_vault(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    lock_service(&state)?.lock_vault();
    clear_owned_clipboard(&app, &state);
    clear_ephemeral_authorizations(&state);
    Ok(())
}

#[tauri::command]
fn touch_activity(state: State<'_, AppState>) -> Result<(), String> {
    lock_service(&state)?.touch_activity().map_err(display_error)
}

#[tauri::command]
fn add_item(state: State<'_, AppState>, input: ItemInput) -> Result<Uuid, String> {
    lock_service(&state)?.add_item(input).map_err(display_error)
}

#[tauri::command]
fn update_item(state: State<'_, AppState>, item_id: Uuid, patch: ItemPatch) -> Result<(), String> {
    lock_service(&state)?
        .update_item(item_id, patch)
        .map_err(display_error)
}

#[tauri::command]
fn delete_item(state: State<'_, AppState>, item_id: Uuid) -> Result<(), String> {
    lock_service(&state)?.delete_item(item_id).map_err(display_error)
}

#[tauri::command]
fn undo_delete(state: State<'_, AppState>) -> Result<Uuid, String> {
    lock_service(&state)?.undo_delete().map_err(display_error)
}

#[tauri::command]
fn list_items(state: State<'_, AppState>, filter: Option<ListFilter>) -> Result<Vec<ItemSummary>, String> {
    lock_service(&state)?
        .list_items(filter.unwrap_or_default())
        .map_err(display_error)
}

#[tauri::command]
fn get_item(state: State<'_, AppState>, item_id: Uuid) -> Result<VaultItem, String> {
    lock_service(&state)?.get_item(item_id).map_err(display_error)
}

#[tauri::command]
fn get_totp_code(state: State<'_, AppState>, item_id: Uuid) -> Result<TotpCode, String> {
    lock_service(&state)?
        .get_totp_code(item_id)
        .map_err(display_error)
}

#[tauri::command]
fn generate_password(
    state: State<'_, AppState>,
    policy: Option<PasswordPolicy>,
) -> Result<GeneratedPassword, String> {
    lock_service(&state)?
        .generate_password(policy.unwrap_or_default())
        .map_err(display_error)
}

#[tauri::command]
fn list_profiles(state: State<'_, AppState>) -> Result<Vec<PasswordProfile>, String> {
    lock_service(&state)?.list_profiles().map_err(display_error)
}

#[tauri::command]
fn save_profile(state: State<'_, AppState>, profile: PasswordProfile) -> Result<Uuid, String> {
    lock_service(&state)?.save_profile(profile).map_err(display_error)
}

#[tauri::command]
fn delete_profile(state: State<'_, AppState>, profile_id: Uuid) -> Result<(), String> {
    lock_service(&state)?
        .delete_profile(profile_id)
        .map_err(display_error)
}

#[tauri::command]
fn get_settings(state: State<'_, AppState>) -> Result<AppSettings, String> {
    lock_service(&state)?.get_settings().map_err(display_error)
}

#[tauri::command]
fn update_settings(state: State<'_, AppState>, settings: AppSettings) -> Result<(), String> {
    let current = lock_service(&state)?.get_settings().map_err(display_error)?;
    if settings.backup_preferences.directory != current.backup_preferences.directory {
        let selected = settings.backup_preferences.directory.as_ref().map(PathBuf::from);
        if selected.as_ref().is_some_and(|path| {
            !state
                .approved_backup_directories
                .lock()
                .map(|approved| approved.contains(path))
                .unwrap_or(false)
        }) {
            return Err("Choose the automatic backup directory with the system dialog first.".into());
        }
    }
    lock_service(&state)?
        .update_settings(settings)
        .map_err(display_error)
}

#[tauri::command]
fn health_report(state: State<'_, AppState>) -> Result<HealthReport, String> {
    lock_service(&state)?.health_report().map_err(display_error)
}

#[tauri::command]
async fn choose_backup_directory(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<String>, String> {
    let picker = app.clone();
    let selection = tauri::async_runtime::spawn_blocking(move || {
        picker
            .dialog()
            .file()
            .set_title("Choose automatic backup directory")
            .blocking_pick_folder()
    })
    .await
    .map_err(|error| format!("backup directory dialog failed: {error}"))?;
    let Some(selection) = selection else {
        return Ok(None);
    };
    let path = selection
        .into_path()
        .map_err(|error| format!("selected directory is not a local path: {error}"))?;
    state
        .approved_backup_directories
        .lock()
        .map_err(|_| "state lock poisoned".to_string())?
        .insert(path.clone());
    Ok(Some(path.display().to_string()))
}

#[tauri::command]
async fn export_backup_dialog(
    app: AppHandle,
    state: State<'_, AppState>,
    passphrase: String,
) -> Result<Option<BackupManifest>, String> {
    let passphrase = Zeroizing::new(passphrase);
    let picker = app.clone();
    let selection = tauri::async_runtime::spawn_blocking(move || {
        picker
            .dialog()
            .file()
            .set_title("Export encrypted QiRing backup")
            .set_file_name("qiring-vault.qiring-backup")
            .add_filter("QiRing encrypted backup", &["qiring-backup"])
            .blocking_save_file()
    })
    .await
    .map_err(|error| format!("backup save dialog failed: {error}"))?;
    let Some(selection) = selection else {
        return Ok(None);
    };
    let path = selection
        .into_path()
        .map_err(|error| format!("selected file is not a local path: {error}"))?;
    lock_service(&state)?
        .export_backup(path, &passphrase)
        .map(Some)
        .map_err(display_error)
}

#[tauri::command]
async fn select_backup_file(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<SelectedBackup>, String> {
    let picker = app.clone();
    let selection = tauri::async_runtime::spawn_blocking(move || {
        picker
            .dialog()
            .file()
            .set_title("Select encrypted QiRing backup")
            .add_filter("QiRing encrypted backup", &["qiring-backup"])
            .blocking_pick_file()
    })
    .await
    .map_err(|error| format!("backup open dialog failed: {error}"))?;
    let Some(selection) = selection else {
        return Ok(None);
    };
    let path = selection
        .into_path()
        .map_err(|error| format!("selected file is not a local path: {error}"))?;
    let token = Uuid::new_v4().to_string();
    let mut selections = state
        .selected_backups
        .lock()
        .map_err(|_| "state lock poisoned".to_string())?;
    selections.clear();
    selections.insert(token.clone(), path.clone());
    Ok(Some(SelectedBackup {
        token,
        display_path: path.display().to_string(),
    }))
}

#[tauri::command]
fn preview_selected_backup(
    state: State<'_, AppState>,
    token: String,
    passphrase: String,
) -> Result<BackupPreview, String> {
    let passphrase = Zeroizing::new(passphrase);
    let path = selected_backup_path(&state, &token)?;
    lock_service(&state)?
        .preview_backup(path, &passphrase)
        .map_err(display_error)
}

#[tauri::command]
fn import_selected_backup(
    state: State<'_, AppState>,
    token: String,
    passphrase: String,
) -> Result<ImportReport, String> {
    let passphrase = Zeroizing::new(passphrase);
    let path = state
        .selected_backups
        .lock()
        .map_err(|_| "state lock poisoned".to_string())?
        .remove(&token)
        .ok_or_else(|| "Backup selection expired; choose the file again.".to_string())?;
    lock_service(&state)?
        .import_backup(path, &passphrase)
        .map_err(display_error)
}

#[tauri::command]
fn list_snapshots(state: State<'_, AppState>) -> Result<Vec<BackupSnapshot>, String> {
    lock_service(&state)?.list_snapshots().map_err(display_error)
}

#[tauri::command]
fn restore_snapshot(state: State<'_, AppState>, path: String) -> Result<ImportReport, String> {
    lock_service(&state)?
        .restore_snapshot(path)
        .map_err(display_error)
}

#[tauri::command]
fn rotate_master_password(
    state: State<'_, AppState>,
    old_password: String,
    new_password: String,
) -> Result<(), String> {
    let old_password = Zeroizing::new(old_password);
    let new_password = Zeroizing::new(new_password);
    lock_service(&state)?
        .rotate_master_password(&old_password, &new_password)
        .map_err(display_error)
}

#[tauri::command]
fn get_security_status(state: State<'_, AppState>) -> Result<SecurityStatus, String> {
    lock_service(&state)?.get_security_status().map_err(display_error)
}

#[tauri::command]
fn vault_exists(state: State<'_, AppState>) -> Result<bool, String> {
    Ok(lock_service(&state)?.vault_exists())
}

#[tauri::command]
fn copy_secret(app: AppHandle, state: State<'_, AppState>, value: String) -> Result<u32, String> {
    let value = Zeroizing::new(value);
    if value.is_empty() || value.len() > 100_000 {
        return Err("Clipboard value is empty or too large.".into());
    }
    let seconds = lock_service(&state)?
        .get_security_status()
        .map_err(display_error)?
        .clipboard_clear_seconds;
    app.clipboard()
        .write_text(value.as_str())
        .map_err(|error| format!("failed to write clipboard: {error}"))?;

    let mut owned = state
        .clipboard
        .owned
        .lock()
        .map_err(|_| "clipboard state lock poisoned".to_string())?;
    owned.value = Some(value);
    owned.expires_at = Instant::now().checked_add(Duration::from_secs(u64::from(seconds)));
    Ok(seconds)
}

pub fn run() {
    apply_platform_runtime_defaults();
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(|app| {
            let vault_path = resolve_vault_path(app)?;
            let state = AppState::new(vault_path);
            let background_state = state.clone();
            let app_handle = app.handle().clone();
            app.manage(state);

            std::thread::spawn(move || loop {
                std::thread::sleep(Duration::from_secs(1));
                clear_expired_clipboard_from_guard(&app_handle, &background_state.clipboard);
                let locked = background_state
                    .service
                    .lock()
                    .map(|mut service| service.lock_if_idle())
                    .unwrap_or(true);
                if locked {
                    clear_owned_clipboard_from_guard(&app_handle, &background_state.clipboard);
                    clear_ephemeral_authorizations(&background_state);
                    let _ = app_handle.emit("vault-locked", "idle");
                }
            });
            Ok(())
        })
        .on_window_event(handle_window_event)
        .invoke_handler(tauri::generate_handler![
            create_vault,
            unlock_vault_master,
            unlock_vault_recovery,
            regenerate_recovery_key,
            save_recovery_key_dialog,
            lock_vault,
            touch_activity,
            add_item,
            update_item,
            delete_item,
            undo_delete,
            list_items,
            get_item,
            get_totp_code,
            generate_password,
            list_profiles,
            save_profile,
            delete_profile,
            get_settings,
            update_settings,
            health_report,
            choose_backup_directory,
            export_backup_dialog,
            select_backup_file,
            preview_selected_backup,
            import_selected_backup,
            list_snapshots,
            restore_snapshot,
            rotate_master_password,
            get_security_status,
            vault_exists,
            copy_secret,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run QiRing desktop app");
}

fn handle_window_event(window: &tauri::Window, event: &WindowEvent) {
    let state = window.state::<AppState>();
    let should_lock = match event {
        WindowEvent::Focused(false) => state
            .service
            .lock()
            .map(|service| service.should_lock_on_window_blur())
            .unwrap_or(true),
        WindowEvent::Resized(_) => {
            window.is_minimized().unwrap_or(false)
                && state
                    .service
                    .lock()
                    .map(|service| service.should_lock_on_minimize())
                    .unwrap_or(true)
        }
        _ => false,
    };
    if should_lock {
        if let Ok(mut service) = state.service.lock() {
            service.lock_vault();
        }
        clear_owned_clipboard(window.app_handle(), &state);
        clear_ephemeral_authorizations(&state);
        let _ = window.app_handle().emit("vault-locked", "window");
    }
}

fn selected_backup_path(state: &AppState, token: &str) -> Result<PathBuf, String> {
    state
        .selected_backups
        .lock()
        .map_err(|_| "state lock poisoned".to_string())?
        .get(token)
        .cloned()
        .ok_or_else(|| "Backup selection expired; choose the file again.".to_string())
}

fn lock_service(state: &AppState) -> Result<std::sync::MutexGuard<'_, VaultService>, String> {
    state
        .service
        .lock()
        .map_err(|_| "state lock poisoned".to_string())
}

fn clear_owned_clipboard(app: &AppHandle, state: &AppState) {
    clear_owned_clipboard_from_guard(app, &state.clipboard);
}

fn clear_owned_clipboard_from_guard(app: &AppHandle, guard: &ClipboardGuard) {
    if let Ok(mut owned) = guard.owned.lock() {
        let should_clear = owned
            .value
            .as_ref()
            .is_some_and(|owned| app.clipboard().read_text().ok().as_deref() == Some(owned.as_str()));
        if should_clear {
            let _ = app.clipboard().clear();
        }
        owned.value = None;
        owned.expires_at = None;
    }
}

fn clear_expired_clipboard_from_guard(app: &AppHandle, guard: &ClipboardGuard) {
    if let Ok(mut owned) = guard.owned.lock() {
        if owned.expires_at.is_none_or(|expiry| Instant::now() < expiry) {
            return;
        }
        let should_clear = owned
            .value
            .as_ref()
            .is_some_and(|value| app.clipboard().read_text().ok().as_deref() == Some(value.as_str()));
        if should_clear {
            let _ = app.clipboard().clear();
        }
        owned.value = None;
        owned.expires_at = None;
    }
}

fn clear_ephemeral_authorizations(state: &AppState) {
    if let Ok(mut selections) = state.selected_backups.lock() {
        selections.clear();
    }
    if let Ok(mut directories) = state.approved_backup_directories.lock() {
        directories.clear();
    }
}

fn resolve_vault_path(app: &tauri::App) -> anyhow::Result<PathBuf> {
    let app_data = app
        .path()
        .app_data_dir()
        .context("operating system did not provide an application data directory")?;
    ensure_private_directory(&app_data)?;
    let preferred = app_data.join("vault.qiring");
    if preferred.exists() {
        return Ok(preferred);
    }

    if let Some(legacy) = legacy_vault_path().filter(|path| path.is_file()) {
        if let Some(parent) = legacy.parent() {
            ensure_private_directory(parent)?;
        }
        return Ok(legacy);
    }
    Ok(preferred)
}

fn legacy_vault_path() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    let base = env::var_os("APPDATA").map(|value| PathBuf::from(value).join("QiRing"));
    #[cfg(target_os = "macos")]
    let base = env::var_os("HOME").map(|value| {
        PathBuf::from(value)
            .join("Library")
            .join("Application Support")
            .join("QiRing")
    });
    #[cfg(target_os = "linux")]
    let base = env::var_os("XDG_DATA_HOME")
        .map(|value| PathBuf::from(value).join("qiring"))
        .or_else(|| {
            env::var_os("HOME").map(|value| PathBuf::from(value).join(".local").join("share").join("qiring"))
        });
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    let base: Option<PathBuf> = None;
    base.map(|path| path.join(".qiring").join("vault.qiring"))
}

fn ensure_private_directory(path: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(path).context("failed to create application data directory")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .context("failed to restrict application data directory")?;
    }
    Ok(())
}

fn display_error(error: anyhow::Error) -> String {
    error.to_string()
}

fn apply_platform_runtime_defaults() {
    #[cfg(target_os = "linux")]
    if env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backup_paths_require_an_unforgeable_dialog_selection_token() {
        let state = AppState::new(PathBuf::from("/tmp/qiring-test-vault"));
        assert!(selected_backup_path(&state, "unknown").is_err());
        state
            .selected_backups
            .lock()
            .expect("lock")
            .insert("approved".into(), PathBuf::from("/tmp/selected.qiring-backup"));
        assert_eq!(
            selected_backup_path(&state, "approved").expect("approved path"),
            PathBuf::from("/tmp/selected.qiring-backup")
        );
    }

    #[test]
    fn opener_capability_allows_only_http_and_https() {
        let capability: serde_json::Value =
            serde_json::from_str(include_str!("../capabilities/main.json")).expect("capability JSON");
        let permission = capability["permissions"]
            .as_array()
            .expect("permissions")
            .iter()
            .find(|permission| permission["identifier"] == "opener:allow-open-url")
            .expect("scoped opener permission");
        let urls = permission["allow"]
            .as_array()
            .expect("allow scopes")
            .iter()
            .map(|scope| scope["url"].as_str().expect("URL scope"))
            .collect::<HashSet<_>>();
        assert_eq!(urls, HashSet::from(["http://*", "https://*"]));
    }
}
