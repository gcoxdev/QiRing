use qiring_core::{
    AppSettings, BackupManifest, GeneratedPassword, ImportReport, ItemInput, ItemPatch, ItemSummary,
    ListFilter, PasswordPolicy, RecoveryMaterial, SecurityStatus, SessionToken, VaultItem, VaultService,
    VaultSummary,
};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;
use tauri::State;
use uuid::Uuid;

pub struct AppState {
    pub service: Mutex<VaultService>,
}

impl AppState {
    fn new(vault_path: PathBuf) -> Self {
        Self {
            service: Mutex::new(VaultService::new(vault_path)),
        }
    }
}

#[derive(serde::Serialize)]
pub struct CreateVaultResult {
    pub summary: VaultSummary,
    pub recovery: RecoveryMaterial,
}

#[tauri::command]
fn create_vault(
    state: State<'_, AppState>,
    master_password: String,
    settings: Option<AppSettings>,
) -> Result<CreateVaultResult, String> {
    let mut service = state
        .service
        .lock()
        .map_err(|_| "state lock poisoned".to_string())?;
    let (summary, recovery) = service
        .create_vault(&master_password, settings.unwrap_or_default())
        .map_err(|e| e.to_string())?;
    Ok(CreateVaultResult { summary, recovery })
}

#[tauri::command]
fn unlock_vault_master(state: State<'_, AppState>, master_password: String) -> Result<SessionToken, String> {
    let mut service = state
        .service
        .lock()
        .map_err(|_| "state lock poisoned".to_string())?;
    service
        .unlock_vault_master(&master_password)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn unlock_vault_biometric(state: State<'_, AppState>) -> Result<SessionToken, String> {
    let mut service = state
        .service
        .lock()
        .map_err(|_| "state lock poisoned".to_string())?;
    service.unlock_vault_biometric().map_err(|e| e.to_string())
}

#[tauri::command]
fn lock_vault(state: State<'_, AppState>) -> Result<(), String> {
    let mut service = state
        .service
        .lock()
        .map_err(|_| "state lock poisoned".to_string())?;
    service.lock_vault();
    Ok(())
}

#[tauri::command]
fn add_item(state: State<'_, AppState>, input: ItemInput) -> Result<Uuid, String> {
    let mut service = state
        .service
        .lock()
        .map_err(|_| "state lock poisoned".to_string())?;
    service.add_item(input).map_err(|e| e.to_string())
}

#[tauri::command]
fn update_item(state: State<'_, AppState>, item_id: Uuid, patch: ItemPatch) -> Result<(), String> {
    let mut service = state
        .service
        .lock()
        .map_err(|_| "state lock poisoned".to_string())?;
    service.update_item(item_id, patch).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_item(state: State<'_, AppState>, item_id: Uuid) -> Result<(), String> {
    let mut service = state
        .service
        .lock()
        .map_err(|_| "state lock poisoned".to_string())?;
    service.delete_item(item_id).map_err(|e| e.to_string())
}

#[tauri::command]
fn list_items(state: State<'_, AppState>, filter: Option<ListFilter>) -> Result<Vec<ItemSummary>, String> {
    let service = state
        .service
        .lock()
        .map_err(|_| "state lock poisoned".to_string())?;
    service
        .list_items(filter.unwrap_or_default())
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn get_item(state: State<'_, AppState>, item_id: Uuid) -> Result<VaultItem, String> {
    let service = state
        .service
        .lock()
        .map_err(|_| "state lock poisoned".to_string())?;
    service.get_item(item_id).map_err(|e| e.to_string())
}

#[tauri::command]
fn generate_password(
    state: State<'_, AppState>,
    policy: Option<PasswordPolicy>,
) -> Result<GeneratedPassword, String> {
    let service = state
        .service
        .lock()
        .map_err(|_| "state lock poisoned".to_string())?;
    service
        .generate_password(policy.unwrap_or_default())
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn export_backup(
    state: State<'_, AppState>,
    path: String,
    passphrase: String,
) -> Result<BackupManifest, String> {
    let service = state
        .service
        .lock()
        .map_err(|_| "state lock poisoned".to_string())?;
    service
        .export_backup(path, &passphrase)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn import_backup(
    state: State<'_, AppState>,
    path: String,
    passphrase: String,
) -> Result<ImportReport, String> {
    let mut service = state
        .service
        .lock()
        .map_err(|_| "state lock poisoned".to_string())?;
    service
        .import_backup(path, &passphrase)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn rotate_master_password(
    state: State<'_, AppState>,
    old_password: String,
    new_password: String,
) -> Result<(), String> {
    let mut service = state
        .service
        .lock()
        .map_err(|_| "state lock poisoned".to_string())?;
    service
        .rotate_master_password(&old_password, &new_password)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn get_security_status(state: State<'_, AppState>) -> Result<SecurityStatus, String> {
    let service = state
        .service
        .lock()
        .map_err(|_| "state lock poisoned".to_string())?;
    service.get_security_status().map_err(|e| e.to_string())
}

#[tauri::command]
fn vault_exists() -> bool {
    app_data_dir().join(".qiring").join("vault.qiring").exists()
}

#[tauri::command]
fn open_url(url: String) -> Result<(), String> {
    if url.trim().is_empty() {
        return Err("URL is empty".to_string());
    }

    #[cfg(target_os = "windows")]
    {
        Command::new("cmd")
            .args(["/C", "start", "", &url])
            .spawn()
            .map_err(|e| format!("failed to open URL: {e}"))?;
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(&url)
            .spawn()
            .map_err(|e| format!("failed to open URL: {e}"))?;
        return Ok(());
    }

    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open")
            .arg(&url)
            .spawn()
            .map_err(|e| format!("failed to open URL: {e}"))?;
        return Ok(());
    }

    #[allow(unreachable_code)]
    Err("open_url is not supported on this platform".to_string())
}

pub fn run() {
    apply_platform_runtime_defaults();
    let context = tauri::generate_context!();
    let vault_path = app_data_dir().join(".qiring").join("vault.qiring");

    tauri::Builder::default()
        .manage(AppState::new(vault_path))
        .invoke_handler(tauri::generate_handler![
            create_vault,
            unlock_vault_master,
            unlock_vault_biometric,
            lock_vault,
            add_item,
            update_item,
            delete_item,
            list_items,
            get_item,
            generate_password,
            export_backup,
            import_backup,
            rotate_master_password,
            get_security_status,
            vault_exists,
            open_url,
        ])
        .run(context)
        .expect("failed to run qiring desktop app");
}

fn app_data_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Some(appdata) = env::var_os("APPDATA") {
            let base = PathBuf::from(appdata).join("QiRing");
            let _ = fs::create_dir_all(&base);
            return base;
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(home) = env::var_os("HOME") {
            let base = PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("QiRing");
            let _ = fs::create_dir_all(&base);
            return base;
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Some(xdg_data_home) = env::var_os("XDG_DATA_HOME") {
            let base = PathBuf::from(xdg_data_home).join("qiring");
            let _ = fs::create_dir_all(&base);
            return base;
        }
        if let Some(home) = env::var_os("HOME") {
            let base = PathBuf::from(home).join(".local").join("share").join("qiring");
            let _ = fs::create_dir_all(&base);
            return base;
        }
    }

    let base = env::temp_dir().join("qiring");
    let _ = fs::create_dir_all(&base);
    base
}

fn apply_platform_runtime_defaults() {
    #[cfg(target_os = "linux")]
    {
        if env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
            env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
        }
    }
}
