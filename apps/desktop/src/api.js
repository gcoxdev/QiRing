import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { openUrl as openExternalUrl } from "@tauri-apps/plugin-opener";

export const COMMAND_VERSION = "4";

export const vaultApi = Object.freeze({
  exists: () => invoke("vault_exists"),
  create: (masterPassword, settings) => invoke("create_vault", { masterPassword, settings }),
  unlockMaster: (masterPassword) => invoke("unlock_vault_master", { masterPassword }),
  unlockRecovery: (recoveryKey, newMasterPassword) =>
    invoke("unlock_vault_recovery", { recoveryKey, newMasterPassword }),
  regenerateRecovery: (masterPassword) => invoke("regenerate_recovery_key", { masterPassword }),
  saveRecoveryKey: (recoveryKey) => invoke("save_recovery_key_dialog", { recoveryKey }),
  rotateMaster: (oldPassword, newPassword) =>
    invoke("rotate_master_password", { oldPassword, newPassword }),
  lock: () => invoke("lock_vault"),
  touch: () => invoke("touch_activity"),
  onLocked: (handler) => listen("vault-locked", handler),

  listItems: (filter = null) => invoke("list_items", { filter }),
  getItem: (itemId) => invoke("get_item", { itemId }),
  addItem: (input) => invoke("add_item", { input }),
  updateItem: (itemId, patch) => invoke("update_item", { itemId, patch }),
  selectItemIcon: () => invoke("select_item_icon_dialog"),
  fetchFavicon: (url) => invoke("fetch_favicon", { url }),
  deleteItem: (itemId) => invoke("delete_item", { itemId }),
  undoDelete: () => invoke("undo_delete"),
  totp: (itemId) => invoke("get_totp_code", { itemId }),

  generatePassword: (policy) => invoke("generate_password", { policy }),
  listProfiles: () => invoke("list_profiles"),
  saveProfile: (profile) => invoke("save_profile", { profile }),
  deleteProfile: (profileId) => invoke("delete_profile", { profileId }),

  getSettings: () => invoke("get_settings"),
  updateSettings: (settings) => invoke("update_settings", { settings }),
  securityStatus: () => invoke("get_security_status"),
  health: () => invoke("health_report"),

  chooseBackupDirectory: () => invoke("choose_backup_directory"),
  exportBackup: (passphrase) => invoke("export_backup_dialog", { passphrase }),
  selectBackup: () => invoke("select_backup_file"),
  previewBackup: (token, passphrase) =>
    invoke("preview_selected_backup", { token, passphrase }),
  importBackup: (token, passphrase) =>
    invoke("import_selected_backup", { token, passphrase }),
  listSnapshots: () => invoke("list_snapshots"),
  restoreSnapshot: (path) => invoke("restore_snapshot", { path }),

  copySecret: (value) => invoke("copy_secret", { value }),
  openUrl: (url) => openExternalUrl(url)
});
