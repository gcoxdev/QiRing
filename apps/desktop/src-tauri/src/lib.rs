use anyhow::Context;
use base64::Engine;
use qiring_core::{
    sniff_image_media_type, AppSettings, BackupManifest, BackupPreview, BackupSnapshot, GeneratedPassword,
    HealthReport, ImportReport, ItemInput, ItemPatch, ItemSummary, ListFilter, PasswordPolicy,
    PasswordProfile, RecoveryMaterial, RecoveryUnlockResult, SecurityStatus, TotpCode, UnlockResult,
    VaultItem, VaultService, VaultSummary,
};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::io::Read;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, State, WindowEvent};
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
    window_bounds: Arc<WindowBoundsGuard>,
    pending_print_basename: Arc<Mutex<Option<String>>>,
}

impl AppState {
    fn new(vault_path: PathBuf, window_state_path: PathBuf) -> Self {
        Self {
            service: Arc::new(Mutex::new(VaultService::new(vault_path))),
            clipboard: Arc::new(ClipboardGuard::default()),
            selected_backups: Arc::new(Mutex::new(HashMap::new())),
            approved_backup_directories: Arc::new(Mutex::new(HashSet::new())),
            window_bounds: Arc::new(WindowBoundsGuard {
                path: window_state_path,
                current: Mutex::new(WindowBoundsState::default()),
                persistence: Mutex::new(()),
            }),
            pending_print_basename: Arc::new(Mutex::new(None)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, Serialize)]
struct PersistedWindowBounds {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    maximized: bool,
}

struct WindowBoundsGuard {
    path: PathBuf,
    current: Mutex<WindowBoundsState>,
    persistence: Mutex<()>,
}

#[derive(Default)]
struct WindowBoundsState {
    bounds: Option<PersistedWindowBounds>,
    revision: u64,
    persisted_revision: u64,
    changed_at: Option<Instant>,
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

const MAX_QI_ICON_BYTES: usize = 512 * 1024;
const WINDOW_MIN_WIDTH: u32 = 800;
const WINDOW_MIN_HEIGHT: u32 = 600;
const WINDOW_BOUNDS_PERSIST_DELAY: Duration = Duration::from_millis(500);
const WINDOW_BOUNDS_PERSIST_POLL: Duration = Duration::from_millis(200);
const RECOVERY_PRINT_TITLE_PREFIX: &str = "QiRing-Recovery-Key-";

#[tauri::command]
async fn create_vault(
    state: State<'_, AppState>,
    master_password: String,
    settings: Option<AppSettings>,
) -> Result<CreateVaultResult, String> {
    let master_password = Zeroizing::new(master_password);
    let service = Arc::clone(&state.service);
    run_service_blocking(service, move |service| {
        let (summary, recovery) = service
            .create_vault(&master_password, settings.unwrap_or_default())
            .map_err(display_error)?;
        Ok(CreateVaultResult { summary, recovery })
    })
    .await
}

#[tauri::command]
async fn unlock_vault_master(
    state: State<'_, AppState>,
    master_password: String,
) -> Result<UnlockResult, String> {
    let master_password = Zeroizing::new(master_password);
    let service = Arc::clone(&state.service);
    run_service_blocking(service, move |service| {
        service
            .unlock_vault_master(&master_password)
            .map_err(display_error)
    })
    .await
}

#[tauri::command]
async fn unlock_vault_recovery(
    state: State<'_, AppState>,
    recovery_key: String,
    new_master_password: String,
) -> Result<RecoveryUnlockResult, String> {
    let recovery_key = Zeroizing::new(recovery_key);
    let new_master_password = Zeroizing::new(new_master_password);
    let service = Arc::clone(&state.service);
    run_service_blocking(service, move |service| {
        service
            .unlock_vault_recovery(&recovery_key, &new_master_password)
            .map_err(display_error)
    })
    .await
}

#[tauri::command]
async fn regenerate_recovery_key(
    state: State<'_, AppState>,
    master_password: String,
) -> Result<RecoveryMaterial, String> {
    let master_password = Zeroizing::new(master_password);
    let service = Arc::clone(&state.service);
    run_service_blocking(service, move |service| {
        service
            .regenerate_recovery_key(&master_password)
            .map_err(display_error)
    })
    .await
}

#[tauri::command]
async fn save_recovery_key_dialog(
    app: AppHandle,
    recovery_key: String,
    basename: String,
) -> Result<Option<String>, String> {
    let recovery_key = Zeroizing::new(recovery_key);
    let basename = recovery_print_basename(&basename)
        .ok_or_else(|| "The recovery-key export filename is invalid.".to_string())?;
    let filename = format!("{basename}.txt");
    let picker = app.clone();
    let selection = tauri::async_runtime::spawn_blocking(move || {
        picker
            .dialog()
            .file()
            .set_title("Save QiRing recovery key")
            .set_file_name(&filename)
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
        "QiRing recovery key\n\n{}\n\nStore this file offline. Anyone with this key and the Ring file can reset the Ring master password.\n",
        recovery_key.as_str()
    ));
    qiring_storage::save_bytes_atomic_user_directory(&path, contents.as_bytes()).map_err(display_error)?;
    Ok(Some(path.display().to_string()))
}

#[tauri::command]
fn prepare_recovery_print(state: State<'_, AppState>, basename: String) -> Result<(), String> {
    let basename = recovery_print_basename(&basename)
        .ok_or_else(|| "The recovery print filename is invalid.".to_string())?;
    *state
        .pending_print_basename
        .lock()
        .map_err(|_| "recovery print state lock poisoned".to_string())? = Some(basename.to_owned());
    Ok(())
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
async fn select_item_icon_dialog(app: AppHandle) -> Result<Option<String>, String> {
    let picker = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let selection = picker
            .dialog()
            .file()
            .set_title("Choose a Qi icon")
            .add_filter("Image", &["png", "jpg", "jpeg", "webp", "gif", "ico"])
            .blocking_pick_file();
        let Some(selection) = selection else {
            return Ok(None);
        };
        let path = selection
            .into_path()
            .map_err(|error| format!("selected image is not a local path: {error}"))?;
        let metadata =
            fs::metadata(&path).map_err(|error| format!("could not inspect the selected image: {error}"))?;
        if metadata.len() > MAX_QI_ICON_BYTES as u64 {
            return Err("Qi icons are limited to 512 KiB. Choose a smaller image.".into());
        }
        let bytes = fs::read(&path).map_err(|error| format!("could not read the selected image: {error}"))?;
        image_data_url(&bytes).map(Some)
    })
    .await
    .map_err(|error| format!("Qi icon dialog failed: {error}"))?
}

#[tauri::command]
async fn fetch_favicon(url: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || fetch_favicon_data_url(&url))
        .await
        .map_err(|error| format!("favicon task failed: {error}"))?
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
            .set_file_name("qiring-ring.qiring-backup")
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
async fn rotate_master_password(
    state: State<'_, AppState>,
    old_password: String,
    new_password: String,
) -> Result<(), String> {
    let old_password = Zeroizing::new(old_password);
    let new_password = Zeroizing::new(new_password);
    let service = Arc::clone(&state.service);
    run_service_blocking(service, move |service| {
        service
            .rotate_master_password(&old_password, &new_password)
            .map_err(display_error)
    })
    .await
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
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(|app| {
            let vault_path = resolve_vault_path(app)?;
            let window_state_path = app.path().app_config_dir()?.join("window-state.json");
            let state = AppState::new(vault_path, window_state_path);
            let background_state = state.clone();
            let app_handle = app.handle().clone();
            app.manage(state);

            if let Some(window) = app.get_webview_window("main") {
                #[cfg(target_os = "linux")]
                configure_linux_print_defaults(
                    &window,
                    Arc::clone(&app.state::<AppState>().pending_print_basename),
                )?;

                let bounds = app.state::<AppState>().window_bounds.clone();
                if !restore_window_bounds(&window, &bounds) {
                    capture_window_bounds(&window.as_ref().window(), &bounds);
                }

                let persistence_bounds = bounds.clone();
                std::thread::spawn(move || loop {
                    std::thread::sleep(WINDOW_BOUNDS_PERSIST_POLL);
                    persist_window_bounds_if_settled(&persistence_bounds);
                });
            }

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
            prepare_recovery_print,
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
            select_item_icon_dialog,
            fetch_favicon,
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
        .build(tauri::generate_context!())
        .expect("failed to build QiRing desktop app");

    app.run(|app_handle, event| {
        if matches!(
            event,
            tauri::RunEvent::ExitRequested { .. } | tauri::RunEvent::Exit
        ) {
            let state = app_handle.state::<AppState>();
            clear_owned_clipboard(app_handle, &state);
        }
    });
}

fn recovery_print_basename(title: &str) -> Option<&str> {
    (title.starts_with(RECOVERY_PRINT_TITLE_PREFIX)
        && title.len() <= 64
        && title
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-'))
    .then_some(title)
}

#[cfg(target_os = "linux")]
fn configure_linux_print_defaults(
    window: &tauri::WebviewWindow,
    pending_basename: Arc<Mutex<Option<String>>>,
) -> tauri::Result<()> {
    window.with_webview(|platform_webview| {
        use webkit2gtk::{PrintOperationExt, WebViewExt};

        platform_webview.inner().connect_print(move |webview, operation| {
            let basename = pending_basename
                .lock()
                .ok()
                .and_then(|mut pending| pending.take())
                .or_else(|| {
                    webview
                        .title()
                        .and_then(|title| recovery_print_basename(title.as_str()).map(str::to_owned))
                });
            let Some(basename) = basename else {
                return false;
            };
            let settings = operation.print_settings().unwrap_or_else(gtk::PrintSettings::new);
            settings.set("output-basename", Some(&basename));
            // Leave the file format to GTK's Print to File controls. GTK 3's file
            // backend compares this private setting against lowercase values and
            // aborts the process when an unrecognized value is supplied.
            operation.set_print_settings(&settings);
            false
        });
    })
}

fn handle_window_event(window: &tauri::Window, event: &WindowEvent) {
    let state = window.state::<AppState>();
    match event {
        WindowEvent::Moved(_) | WindowEvent::Resized(_) => {
            capture_window_bounds(window, &state.window_bounds);
        }
        WindowEvent::CloseRequested { .. } => {
            capture_window_bounds(window, &state.window_bounds);
            persist_window_bounds(&state.window_bounds);
            clear_owned_clipboard(window.app_handle(), &state);
        }
        _ => {}
    }
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

fn image_data_url(bytes: &[u8]) -> Result<String, String> {
    if bytes.is_empty() || bytes.len() > MAX_QI_ICON_BYTES {
        return Err("Qi icons must be non-empty and no larger than 512 KiB.".into());
    }
    let media_type = sniff_image_media_type(bytes)
        .ok_or_else(|| "Qi icons must contain a valid PNG, JPEG, WebP, GIF, or ICO image.".to_string())?;
    Ok(format!(
        "data:{media_type};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

fn fetch_favicon_data_url(raw_url: &str) -> Result<String, String> {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let mut target = reqwest::Url::parse(raw_url)
        .map_err(|_| "Enter a complete website URL beginning with http:// or https://.".to_string())?;
    validate_favicon_url(&target)?;
    target.set_path("/favicon.ico");
    target.set_query(None);
    target.set_fragment(None);

    for _ in 0..=4 {
        let (host, endpoint) = resolve_public_endpoint(&target)?;
        let mut builder = reqwest::blocking::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(4))
            .timeout(Duration::from_secs(8))
            .user_agent("QiRing/0.1 favicon import");
        if host.parse::<IpAddr>().is_err() {
            builder = builder.resolve(&host, endpoint);
        }
        let client = builder
            .build()
            .map_err(|error| format!("could not initialize the favicon client: {error}"))?;
        let response = client
            .get(target.clone())
            .send()
            .map_err(|error| format!("could not fetch the website favicon: {error}"))?;

        if response.status().is_redirection() {
            let location = response
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|value| value.to_str().ok())
                .ok_or_else(|| "The favicon redirect did not provide a valid destination.".to_string())?;
            target = target
                .join(location)
                .map_err(|_| "The favicon redirected to an invalid URL.".to_string())?;
            validate_favicon_url(&target)?;
            continue;
        }
        if !response.status().is_success() {
            return Err(format!(
                "The website did not provide a favicon (HTTP {}).",
                response.status().as_u16()
            ));
        }
        if response
            .content_length()
            .is_some_and(|length| length > MAX_QI_ICON_BYTES as u64)
        {
            return Err("The website favicon is larger than 512 KiB.".into());
        }
        let mut bytes = Vec::new();
        response
            .take((MAX_QI_ICON_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|error| format!("could not read the website favicon: {error}"))?;
        if bytes.len() > MAX_QI_ICON_BYTES {
            return Err("The website favicon is larger than 512 KiB.".into());
        }
        return image_data_url(&bytes);
    }
    Err("The website favicon redirected too many times.".into())
}

fn validate_favicon_url(url: &reqwest::Url) -> Result<(), String> {
    if !matches!(url.scheme(), "http" | "https") {
        return Err("Favicon import only supports HTTP and HTTPS websites.".into());
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("Website URLs containing embedded credentials are not supported.".into());
    }
    if url.host_str().is_none() {
        return Err("The website URL does not contain a host.".into());
    }
    let port = url
        .port_or_known_default()
        .ok_or_else(|| "The website URL does not use a supported port.".to_string())?;
    if !matches!(port, 80 | 443) {
        return Err("Favicon import is limited to standard HTTP and HTTPS ports.".into());
    }
    Ok(())
}

fn resolve_public_endpoint(url: &reqwest::Url) -> Result<(String, SocketAddr), String> {
    validate_favicon_url(url)?;
    let host = url.host_str().expect("validated URL host").to_string();
    let port = url.port_or_known_default().expect("validated URL port");
    let addresses = (host.as_str(), port)
        .to_socket_addrs()
        .map_err(|_| "The website host could not be resolved.".to_string())?
        .collect::<Vec<_>>();
    if addresses.is_empty() {
        return Err("The website host did not resolve to an address.".into());
    }
    if addresses.iter().any(|address| !is_public_ip(address.ip())) {
        return Err("Favicon import blocks local, private, and reserved network addresses.".into());
    }
    Ok((host, addresses[0]))
}

fn is_public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => is_public_ipv4(ip),
        IpAddr::V6(ip) => is_public_ipv6(ip),
    }
}

fn is_public_ipv4(ip: Ipv4Addr) -> bool {
    let [a, b, c, _] = ip.octets();
    !(ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_multicast()
        || ip.is_unspecified()
        || a == 0
        || a >= 224
        || (a == 100 && (64..=127).contains(&b))
        || (a == 192 && b == 0 && matches!(c, 0 | 2))
        || (a == 198 && matches!(b, 18 | 19))
        || (a == 198 && b == 51 && c == 100)
        || (a == 203 && b == 0 && c == 113))
}

fn is_public_ipv6(ip: Ipv6Addr) -> bool {
    if let Some(mapped) = ip.to_ipv4_mapped() {
        return is_public_ipv4(mapped);
    }
    let segments = ip.segments();
    !(ip.is_loopback()
        || ip.is_unspecified()
        || ip.is_multicast()
        || (segments[0] & 0xfe00) == 0xfc00
        || (segments[0] & 0xffc0) == 0xfe80
        || (segments[0] == 0x2001 && segments[1] == 0x0db8))
}

fn restore_window_bounds(window: &tauri::WebviewWindow, guard: &WindowBoundsGuard) -> bool {
    let Ok(bytes) = fs::read(&guard.path) else {
        return false;
    };
    let Ok(saved) = serde_json::from_slice::<PersistedWindowBounds>(&bytes) else {
        return false;
    };
    if saved.width == 0 || saved.height == 0 || saved.width > 32_768 || saved.height > 32_768 {
        return false;
    }
    let Ok(monitors) = window.available_monitors() else {
        return false;
    };
    if monitors.is_empty() {
        return false;
    }

    let saved_right = i64::from(saved.x) + i64::from(saved.width);
    let saved_bottom = i64::from(saved.y) + i64::from(saved.height);
    let preferred = monitors.iter().find(|monitor| {
        let position = monitor.position();
        let size = monitor.size();
        let right = i64::from(position.x) + i64::from(size.width);
        let bottom = i64::from(position.y) + i64::from(size.height);
        i64::from(saved.x) < right
            && saved_right > i64::from(position.x)
            && i64::from(saved.y) < bottom
            && saved_bottom > i64::from(position.y)
    });
    let primary = window.primary_monitor().ok().flatten();
    let monitor = preferred.or(primary.as_ref()).unwrap_or(&monitors[0]);
    let monitor_position = monitor.position();
    let monitor_size = monitor.size();
    let width = saved
        .width
        .max(WINDOW_MIN_WIDTH.min(monitor_size.width))
        .min(monitor_size.width);
    let height = saved
        .height
        .max(WINDOW_MIN_HEIGHT.min(monitor_size.height))
        .min(monitor_size.height);
    let max_x = i64::from(monitor_position.x) + i64::from(monitor_size.width - width);
    let max_y = i64::from(monitor_position.y) + i64::from(monitor_size.height - height);
    let centered_x = i64::from(monitor_position.x) + i64::from(monitor_size.width - width) / 2;
    let centered_y = i64::from(monitor_position.y) + i64::from(monitor_size.height - height) / 2;
    let saved_intersects_preferred = preferred.is_some();
    let x = if saved_intersects_preferred {
        i64::from(saved.x).clamp(i64::from(monitor_position.x), max_x)
    } else {
        centered_x
    } as i32;
    let y = if saved_intersects_preferred {
        i64::from(saved.y).clamp(i64::from(monitor_position.y), max_y)
    } else {
        centered_y
    } as i32;

    let restored = PersistedWindowBounds {
        x,
        y,
        width,
        height,
        maximized: saved.maximized,
    };
    if let Ok(mut state) = guard.current.lock() {
        state.bounds = Some(restored);
        if restored != saved {
            state.revision = 1;
            state.changed_at = Some(Instant::now());
        }
    }

    let _ = window.set_size(PhysicalSize::new(restored.width, restored.height));
    let _ = window.set_position(PhysicalPosition::new(restored.x, restored.y));
    if restored.maximized {
        let _ = window.maximize();
    }
    true
}

fn capture_window_bounds(window: &tauri::Window, guard: &WindowBoundsGuard) {
    let maximized = window.is_maximized().unwrap_or(false);
    let minimized = window.is_minimized().unwrap_or(false);
    if minimized {
        return;
    }
    let Ok(mut current) = guard.current.lock() else {
        return;
    };
    if maximized {
        if let Some(mut bounds) = current.bounds {
            bounds.maximized = true;
            record_window_bounds(&mut current, bounds, Instant::now());
        }
        return;
    }
    let Ok(size) = window.inner_size() else {
        return;
    };
    let position = absolute_window_position_supported()
        .then(|| window.outer_position().ok())
        .flatten();
    let (x, y) = position
        .map(|position| (position.x, position.y))
        .or_else(|| current.bounds.map(|bounds| (bounds.x, bounds.y)))
        .unwrap_or((0, 0));
    let bounds = PersistedWindowBounds {
        x,
        y,
        width: size.width,
        height: size.height,
        maximized: false,
    };
    record_window_bounds(&mut current, bounds, Instant::now());
}

fn record_window_bounds(state: &mut WindowBoundsState, bounds: PersistedWindowBounds, changed_at: Instant) {
    if state.bounds == Some(bounds) {
        return;
    }
    state.bounds = Some(bounds);
    state.revision = state.revision.saturating_add(1);
    state.changed_at = Some(changed_at);
}

fn pending_window_bounds(
    guard: &WindowBoundsGuard,
    now: Instant,
    force: bool,
) -> Option<(PersistedWindowBounds, u64)> {
    let state = guard.current.lock().ok()?;
    if state.revision == state.persisted_revision {
        return None;
    }
    if !force
        && state
            .changed_at
            .is_none_or(|changed_at| now.saturating_duration_since(changed_at) < WINDOW_BOUNDS_PERSIST_DELAY)
    {
        return None;
    }
    Some((state.bounds?, state.revision))
}

fn persist_window_bounds_if_settled(guard: &WindowBoundsGuard) {
    persist_window_bounds_inner(guard, false);
}

fn persist_window_bounds(guard: &WindowBoundsGuard) {
    persist_window_bounds_inner(guard, true);
}

fn persist_window_bounds_inner(guard: &WindowBoundsGuard, force: bool) {
    let Ok(_persistence) = guard.persistence.lock() else {
        return;
    };
    let Some((bounds, revision)) = pending_window_bounds(guard, Instant::now(), force) else {
        return;
    };
    let result = (|| -> anyhow::Result<()> {
        let bytes = serde_json::to_vec_pretty(&bounds).context("failed to serialize window state")?;
        if let Some(parent) = guard.path.parent() {
            fs::create_dir_all(parent).context("failed to create window-state directory")?;
        }
        qiring_storage::save_bytes_atomic(&guard.path, &bytes).context("failed to save window state")
    })();

    let Ok(mut state) = guard.current.lock() else {
        return;
    };
    if let Err(error) = result {
        if state.revision == revision {
            state.changed_at = Some(Instant::now());
        }
        eprintln!("QiRing could not save window state: {error:#}");
        return;
    }
    state.persisted_revision = revision;
    if state.revision == revision {
        state.changed_at = None;
    }
}

#[cfg(target_os = "linux")]
fn absolute_window_position_supported() -> bool {
    let configured_backend = env::var("WINIT_UNIX_BACKEND")
        .ok()
        .or_else(|| env::var("GDK_BACKEND").ok())
        .and_then(|value| value.split(',').next().map(str::trim).map(str::to_owned));
    match configured_backend.as_deref() {
        Some("x11") => true,
        Some("wayland") => false,
        _ => !env::var("XDG_SESSION_TYPE").is_ok_and(|session| session.eq_ignore_ascii_case("wayland")),
    }
}

#[cfg(not(target_os = "linux"))]
fn absolute_window_position_supported() -> bool {
    true
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

async fn run_service_blocking<T, F>(service: Arc<Mutex<VaultService>>, operation: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce(&mut VaultService) -> Result<T, String> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(move || {
        let mut service = service.lock().map_err(|_| "state lock poisoned".to_string())?;
        operation(&mut service)
    })
    .await
    .map_err(|error| format!("Ring operation task failed: {error}"))?
}

fn clear_owned_clipboard(app: &AppHandle, state: &AppState) {
    clear_owned_clipboard_from_guard(app, &state.clipboard);
}

fn clear_owned_clipboard_from_guard(app: &AppHandle, guard: &ClipboardGuard) {
    if let Ok(mut owned) = guard.owned.lock() {
        let Ok(clipboard_value) = app.clipboard().read_text() else {
            return;
        };
        let should_clear = forget_owned_clipboard(&mut owned, &clipboard_value);
        if should_clear {
            let _ = app.clipboard().clear();
        }
    }
}

fn clear_expired_clipboard_from_guard(app: &AppHandle, guard: &ClipboardGuard) {
    if let Ok(mut owned) = guard.owned.lock() {
        if owned.expires_at.is_none_or(|expiry| Instant::now() < expiry) {
            return;
        }
        let Ok(clipboard_value) = app.clipboard().read_text() else {
            return;
        };
        let should_clear = forget_owned_clipboard(&mut owned, &clipboard_value);
        if should_clear {
            let _ = app.clipboard().clear();
        }
    }
}

fn forget_owned_clipboard(owned: &mut OwnedClipboard, clipboard_value: &str) -> bool {
    let should_clear = owned
        .value
        .as_ref()
        .is_some_and(|value| clipboard_value == value.as_str());
    owned.value = None;
    owned.expires_at = None;
    should_clear
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

    fn sample_window_bounds(width: u32) -> PersistedWindowBounds {
        PersistedWindowBounds {
            x: 120,
            y: 80,
            width,
            height: 700,
            maximized: false,
        }
    }

    #[test]
    fn backup_paths_require_an_unforgeable_dialog_selection_token() {
        let state = AppState::new(
            PathBuf::from("/tmp/qiring-test-vault"),
            PathBuf::from("/tmp/qiring-test-window-state"),
        );
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

    #[test]
    fn window_bounds_persist_only_after_the_resize_quiet_period() {
        let guard = WindowBoundsGuard {
            path: PathBuf::from("/tmp/qiring-test-window-state"),
            current: Mutex::new(WindowBoundsState::default()),
            persistence: Mutex::new(()),
        };
        let started = Instant::now();
        {
            let mut state = guard.current.lock().expect("window state lock");
            record_window_bounds(&mut state, sample_window_bounds(900), started);
        }

        assert!(pending_window_bounds(
            &guard,
            started + WINDOW_BOUNDS_PERSIST_DELAY - Duration::from_millis(1),
            false
        )
        .is_none());
        assert_eq!(
            pending_window_bounds(&guard, started, true),
            Some((sample_window_bounds(900), 1))
        );
        assert_eq!(
            pending_window_bounds(&guard, started + WINDOW_BOUNDS_PERSIST_DELAY, false),
            Some((sample_window_bounds(900), 1))
        );

        let resized_at = started + Duration::from_secs(1);
        {
            let mut state = guard.current.lock().expect("window state lock");
            record_window_bounds(&mut state, sample_window_bounds(1_000), resized_at);
        }
        assert!(pending_window_bounds(&guard, resized_at + Duration::from_millis(100), false).is_none());
        assert_eq!(
            pending_window_bounds(&guard, resized_at + WINDOW_BOUNDS_PERSIST_DELAY, false),
            Some((sample_window_bounds(1_000), 2))
        );
    }

    #[test]
    fn favicon_import_rejects_local_networks_and_non_images() {
        for address in [
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
            IpAddr::V4(Ipv4Addr::new(169, 254, 1, 1)),
            IpAddr::V6(Ipv6Addr::LOCALHOST),
        ] {
            assert!(!is_public_ip(address));
        }
        assert!(is_public_ip(IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34))));
        assert!(image_data_url(b"not an image").is_err());
    }

    #[test]
    fn favicon_import_accepts_supported_magic_bytes() {
        let data_url = image_data_url(b"\x89PNG\r\n\x1a\nmock").expect("PNG");
        assert!(data_url.starts_with("data:image/png;base64,"));
    }

    #[test]
    fn print_basename_accepts_only_bounded_recovery_titles() {
        assert_eq!(
            recovery_print_basename("QiRing-Recovery-Key-2026-08-09"),
            Some("QiRing-Recovery-Key-2026-08-09")
        );
        assert_eq!(recovery_print_basename("Vault — QiRing"), None);
        assert_eq!(recovery_print_basename("QiRing-Recovery-Key-../../vault"), None);
        assert_eq!(
            recovery_print_basename(
                "QiRing-Recovery-Key-2026-08-09-extra-extra-extra-extra-extra-extra-extra-extra"
            ),
            None
        );
    }

    #[test]
    fn clipboard_cleanup_clears_only_an_unchanged_owned_value() {
        let mut matching = OwnedClipboard {
            value: Some(Zeroizing::new("copied secret".to_string())),
            expires_at: Instant::now().checked_add(Duration::from_secs(30)),
        };
        assert!(forget_owned_clipboard(&mut matching, "copied secret"));
        assert!(matching.value.is_none());
        assert!(matching.expires_at.is_none());

        let mut replaced = OwnedClipboard {
            value: Some(Zeroizing::new("copied secret".to_string())),
            expires_at: Instant::now().checked_add(Duration::from_secs(30)),
        };
        assert!(!forget_owned_clipboard(&mut replaced, "newer clipboard content"));
        assert!(replaced.value.is_none());
        assert!(replaced.expires_at.is_none());
    }
}
