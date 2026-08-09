import "./styles.css";
import { vaultApi, COMMAND_VERSION } from "./api.js";
import { byId, createElement, formatBytes, formatDate, setHidden } from "./dom.js";

const NIL_UUID = "00000000-0000-0000-0000-000000000000";
const DEFAULT_SETTINGS = {
  auto_lock_minutes: 5,
  clipboard_clear_seconds: 30,
  lock_on_window_blur: false,
  lock_on_minimize: true,
  biometric_enabled: false,
  theme: "system",
  backup_preferences: {
    include_settings: true,
    automatic_enabled: false,
    directory: null,
    retention_count: 10
  }
};

const elements = Object.fromEntries(
  [
    "authShell", "createScreen", "unlockScreen", "createForm", "createMaster", "createConfirm",
    "masterTab", "recoveryTab", "masterUnlockPanel", "recoveryUnlockPanel", "unlockMaster",
    "recoveryKey", "recoveryMaster", "recoveryConfirm", "vaultShell", "brandHome", "viewTitle",
    "newAction", "saveAction", "deleteAction", "menuButton", "appMenu", "lockButton", "qiringView",
    "profilesView", "healthView", "backupsView", "settingsView", "itemCount", "searchInput",
    "clearSearch", "itemList", "itemEditorTitle", "dirtyIndicator", "itemForm", "itemTitle", "itemType",
    "itemFolder", "categoryOptions", "itemTags", "credentialFields", "itemUrl", "openUrlButton",
    "itemUsername", "copyUsername", "itemPassword", "togglePassword", "copyPassword", "profileSelect",
    "generatePassword", "itemTotpSecret", "totpCode", "refreshTotp", "totpRemaining", "itemNotes",
    "questionSection", "addQuestion", "questionList", "historySection", "historyList", "profileCount",
    "profileList", "profileEditorTitle", "profileDirtyIndicator", "profileForm", "profileName", "profileLength",
    "upperMin", "upperMax", "lowerMin", "lowerMax", "numbersMin", "numbersMax", "symbolsMin", "symbolsMax", "allowedSymbols", "avoidAmbiguous",
    "testProfile", "profileTestOutput", "runHealth", "healthAnalyzed", "healthWeak", "healthReused",
    "healthOld", "healthIssues", "backupPassphrase", "exportBackup", "selectBackup", "selectedBackupPath",
    "restorePassphrase", "previewBackup", "restoreBackup", "backupPreview", "refreshSnapshots", "snapshotList",
    "settingsForm", "autoLockMinutes", "clipboardSeconds", "lockOnMinimize", "lockOnBlur", "themeSelect",
    "automaticBackups", "includeSettings", "backupRetention", "backupDirectory", "chooseBackupDirectory",
    "oldMasterPassword", "newMasterPassword", "rotateMaster", "recoveryMasterPassword", "regenerateRecovery",
    "recoveryDialog", "recoveryKeyOutput", "recoveryFingerprint", "copyRecoveryKey", "saveRecoveryKey", "printRecoveryKey", "recoveryVerify", "recoveryAcknowledged",
    "finishRecovery", "toastRegion"
  ].map((id) => [id, byId(id)])
);

const views = {
  qiring: elements.qiringView,
  profiles: elements.profilesView,
  health: elements.healthView,
  backups: elements.backupsView,
  settings: elements.settingsView
};

const viewLabels = {
  qiring: "Qi Ring",
  profiles: "Password Profiles",
  health: "Vault Health",
  backups: "Encrypted Backups",
  settings: "Settings"
};

const state = {
  view: "qiring",
  items: [],
  profiles: [],
  selectedItemId: null,
  selectedProfileId: null,
  itemDirty: false,
  profileDirty: false,
  settingsDirty: false,
  suppressDirty: false,
  selectedBackupToken: null,
  backupPreviewed: false,
  totpExpiresAt: 0,
  totpTimer: null,
  recoveryContinuation: null,
  recoveryVerification: "",
  lastActivityTouch: 0,
  searchSequence: 0,
  searchTimer: null,
  collapsedCategories: new Set(),
  unlocked: false
};

function errorMessage(error) {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return String(error);
}

function toast(message, { error = false, actionLabel = null, action = null, timeout = null } = {}) {
  const node = createElement("div", { className: `toast${error ? " error" : ""}` });
  if (error) node.setAttribute("role", "alert");
  const progressTrack = createElement("div", {
    className: "toast-progress",
    attributes: { "aria-hidden": "true" }
  });
  const progressFill = createElement("span", { className: "toast-progress-fill" });
  progressTrack.append(progressFill);
  node.append(progressTrack);
  const text = createElement("p", { text: message });
  node.append(text);
  let timer = null;
  let progressAnimation = null;
  const duration = timeout ?? (error ? 10_000 : action ? 12_000 : 4_500);
  const removeToast = () => {
    window.clearTimeout(timer);
    progressAnimation?.cancel();
    node.remove();
  };
  if (actionLabel && action) {
    const actionButton = createElement("button", { className: "toast-action", text: actionLabel, type: "button" });
    actionButton.addEventListener("click", async () => {
      try {
        await action();
        removeToast();
      } catch (errorValue) {
        toast(errorMessage(errorValue), { error: true });
      }
    });
    node.append(actionButton);
  }
  const close = createElement("button", {
    className: "toast-close",
    text: "Dismiss",
    type: "button",
    attributes: { "aria-label": "Dismiss notification" }
  });
  close.addEventListener("click", removeToast);
  node.append(close);
  elements.toastRegion.append(node);
  if (duration > 0) {
    let hovered = false;
    let focusWithin = false;
    const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    const pauseAndReset = () => {
      window.clearTimeout(timer);
      progressAnimation?.cancel();
      progressFill.style.transform = "scaleX(1)";
    };
    const startCountdown = () => {
      if (!node.isConnected || hovered || focusWithin) return;
      window.clearTimeout(timer);
      progressAnimation?.cancel();
      progressFill.style.transform = "scaleX(1)";
      if (!reducedMotion) {
        progressAnimation = progressFill.animate(
          [{ transform: "scaleX(1)" }, { transform: "scaleX(0)" }],
          { duration, easing: "linear", fill: "forwards" }
        );
      }
      timer = window.setTimeout(removeToast, duration);
    };
    node.addEventListener("mouseenter", () => {
      hovered = true;
      pauseAndReset();
    });
    node.addEventListener("mouseleave", () => {
      hovered = false;
      startCountdown();
    });
    node.addEventListener("focusin", () => {
      focusWithin = true;
      pauseAndReset();
    });
    node.addEventListener("focusout", (event) => {
      if (node.contains(event.relatedTarget)) return;
      focusWithin = false;
      startCountdown();
    });
    startCountdown();
  }
}

async function busy(button, task) {
  const previousDisabled = button?.disabled ?? false;
  if (button) {
    button.disabled = true;
    button.setAttribute("aria-busy", "true");
  }
  try {
    return await task();
  } finally {
    if (button) {
      button.disabled = previousDisabled;
      button.removeAttribute("aria-busy");
    }
  }
}

function setAuthScreen(name) {
  setHidden(elements.authShell, false);
  setHidden(elements.vaultShell, true);
  elements.authShell.dataset.screen = name;
  setHidden(elements.createScreen, name !== "create");
  setHidden(elements.unlockScreen, name !== "unlock");
  state.unlocked = false;
  document.title = name === "create" ? "Create vault — QiRing" : "Unlock — QiRing";
  window.setTimeout(() => (name === "create" ? elements.createMaster : elements.unlockMaster).focus(), 0);
}

function setUnlockTab(method) {
  const master = method === "master";
  elements.masterTab.classList.toggle("active", master);
  elements.recoveryTab.classList.toggle("active", !master);
  elements.masterTab.setAttribute("aria-selected", String(master));
  elements.recoveryTab.setAttribute("aria-selected", String(!master));
  setHidden(elements.masterUnlockPanel, !master);
  setHidden(elements.recoveryUnlockPanel, master);
  window.setTimeout(() => (master ? elements.unlockMaster : elements.recoveryKey).focus(), 0);
}

function showRecoveryCeremony(material, continuation) {
  state.recoveryContinuation = continuation;
  elements.recoveryKeyOutput.textContent = material.recovery_key;
  elements.recoveryFingerprint.textContent = material.recovery_key_fingerprint;
  elements.recoveryAcknowledged.checked = false;
  elements.recoveryVerify.value = "";
  state.recoveryVerification = material.recovery_key.slice(-6);
  elements.finishRecovery.disabled = true;
  elements.recoveryDialog.showModal();
  elements.recoveryAcknowledged.focus();
}

async function finishRecoveryCeremony() {
  if (!recoveryCeremonyReady()) return;
  elements.recoveryDialog.close();
  const continuation = state.recoveryContinuation;
  state.recoveryContinuation = null;
  state.recoveryVerification = "";
  elements.recoveryKeyOutput.textContent = "";
  elements.recoveryFingerprint.textContent = "";
  elements.recoveryVerify.value = "";
  if (continuation) await continuation();
}

function recoveryCeremonyReady() {
  return elements.recoveryAcknowledged.checked
    && elements.recoveryVerify.value === state.recoveryVerification;
}

function updateRecoveryReady() {
  elements.finishRecovery.disabled = !recoveryCeremonyReady();
}

async function saveRecoveryKey() {
  const path = await vaultApi.saveRecoveryKey(elements.recoveryKeyOutput.textContent);
  if (path) toast("Recovery key saved to the selected private text file. Move it offline when practical.");
}

function applyTheme(theme) {
  const systemDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
  document.documentElement.dataset.theme = theme === "light" || (theme === "system" && !systemDark) ? "light" : "dark";
}

async function enterVault() {
  const [security, profiles, items, settings] = await Promise.all([
    vaultApi.securityStatus(),
    vaultApi.listProfiles(),
    vaultApi.listItems({ query: null, tag: null, folder: null, item_type: null }),
    vaultApi.getSettings()
  ]);
  if (security.command_version !== COMMAND_VERSION) {
    throw new Error(`Desktop command version mismatch: expected ${COMMAND_VERSION}, received ${security.command_version}.`);
  }
  state.profiles = profiles;
  state.items = items;
  state.unlocked = true;
  setHidden(elements.authShell, true);
  setHidden(elements.vaultShell, false);
  applyTheme(settings.theme);
  renderProfiles();
  if (state.profiles[0]) selectProfile(state.profiles[0].id, { force: true });
  renderItems();
  fillSettings(settings);
  newItem({ force: true });
  await navigate("qiring", { force: true });
}

function hasUnsavedChanges() {
  return state.itemDirty || state.profileDirty || state.settingsDirty;
}

function confirmDiscard() {
  return !hasUnsavedChanges() || window.confirm("Discard unsaved changes before continuing?");
}

async function navigate(view, { force = false } = {}) {
  if (!views[view]) return;
  if (!force && view === state.view) {
    closeMenu();
    return;
  }
  if (!force && view !== state.view && !confirmDiscard()) return;
  remaskPassword();
  state.view = view;
  state.itemDirty = false;
  state.profileDirty = false;
  state.settingsDirty = false;
  updateDirtyIndicators();
  for (const [name, section] of Object.entries(views)) setHidden(section, name !== view);
  elements.viewTitle.textContent = viewLabels[view];
  document.title = `${viewLabels[view]} — QiRing`;
  document.querySelectorAll("#appMenu [data-view]").forEach((button) => {
    if (button.dataset.view === view) button.setAttribute("aria-current", "page");
    else button.removeAttribute("aria-current");
  });
  configureContextActions();
  closeMenu();
  elements.viewTitle.focus({ preventScroll: true });
  if (view === "health") await runHealthAnalysis();
  if (view === "backups") await refreshSnapshots();
  if (view === "settings") await loadSettings();
}

function configureContextActions() {
  const config = {
    qiring: ["New Qi", "Save Qi", "Delete Qi"],
    profiles: ["New Profile", "Save Profile", "Delete Profile"],
    settings: [null, "Save Settings", null],
    backups: [null, "Export Backup", null],
    health: [null, null, null]
  }[state.view];
  [elements.newAction, elements.saveAction, elements.deleteAction].forEach((button, index) => {
    const label = config[index];
    setHidden(button, !label);
    if (label) button.textContent = label;
  });
  updateContextActionState();
}

function updateContextActionState() {
  if (state.view === "qiring") {
    elements.saveAction.disabled = !state.itemDirty;
    elements.deleteAction.disabled = !state.selectedItemId;
  } else if (state.view === "profiles") {
    elements.saveAction.disabled = !state.profileDirty;
    elements.deleteAction.disabled = !state.selectedProfileId || state.profiles.length <= 1;
  } else if (state.view === "settings") {
    elements.saveAction.disabled = !state.settingsDirty;
  } else if (state.view === "backups") {
    elements.saveAction.disabled = elements.backupPassphrase.value.length < 12;
  }
}

function openMenu() {
  elements.appMenu.hidden = false;
  elements.menuButton.setAttribute("aria-expanded", "true");
  elements.appMenu.querySelector("button")?.focus();
}

function closeMenu({ restoreFocus = false } = {}) {
  elements.appMenu.hidden = true;
  elements.menuButton.setAttribute("aria-expanded", "false");
  if (restoreFocus) elements.menuButton.focus();
}

function updateDirtyIndicators() {
  elements.dirtyIndicator.hidden = !state.itemDirty;
  elements.profileDirtyIndicator.hidden = !state.profileDirty;
  updateContextActionState();
}

function markItemDirty() {
  if (state.suppressDirty) return;
  state.itemDirty = true;
  updateDirtyIndicators();
}

function markProfileDirty() {
  if (state.suppressDirty) return;
  state.profileDirty = true;
  updateDirtyIndicators();
}

function markSettingsDirty() {
  if (state.suppressDirty) return;
  state.settingsDirty = true;
  updateContextActionState();
}

function renderItems() {
  elements.itemList.replaceChildren();
  elements.itemCount.textContent = String(state.items.length);
  const categories = new Map();
  for (const item of state.items) {
    const category = item.folder || "Uncategorized";
    if (!categories.has(category)) categories.set(category, []);
    categories.get(category).push(item);
  }
  for (const [category, items] of [...categories.entries()].sort(([left], [right]) => left.localeCompare(right))) {
    const group = createElement("details", { className: "category-group" });
    group.open = Boolean(elements.searchInput.value.trim()) || !state.collapsedCategories.has(category);
    const summary = createElement("summary", { className: "category-summary" });
    summary.append(
      createElement("span", { className: "category-chevron", text: "›", attributes: { "aria-hidden": "true" } }),
      createElement("strong", { text: category }),
      createElement("span", { className: "category-count", text: String(items.length), attributes: { "aria-label": `${items.length} entries` } })
    );
    const entries = createElement("div", { className: "category-items" });
    for (const item of items.sort((left, right) => left.title.localeCompare(right.title))) {
      const button = createElement("button", { className: "master-list-item", type: "button" });
      button.classList.toggle("active", item.id === state.selectedItemId);
      button.setAttribute("aria-pressed", String(item.id === state.selectedItemId));
      button.append(
        createElement("strong", { text: item.title }),
        createElement("small", {
          text: `${item.item_type === "secure_note" ? "SECURE NOTE" : item.username || "LOGIN"}${item.has_totp ? " · TOTP" : ""}`
        })
      );
      button.addEventListener("click", () => selectItem(item.id));
      entries.append(button);
    }
    group.append(summary, entries);
    group.addEventListener("toggle", (event) => {
      if (!event.isTrusted) return;
      if (group.open) state.collapsedCategories.delete(category);
      else state.collapsedCategories.add(category);
    });
    elements.itemList.append(group);
  }
  if (state.items.length === 0) {
    elements.itemList.append(createElement("p", { className: "empty-message", text: "No Qi entries yet. Use New Qi to add a login or secure note." }));
  }
  const folders = [...new Set(state.items.map((item) => item.folder).filter(Boolean))].sort();
  elements.categoryOptions.replaceChildren(...folders.map((folder) => {
    const option = document.createElement("option");
    option.value = folder;
    return option;
  }));
}

async function refreshItems() {
  const request = ++state.searchSequence;
  const query = elements.searchInput.value.trim() || null;
  const items = await vaultApi.listItems({ query, tag: null, folder: null, item_type: null });
  if (request !== state.searchSequence) return;
  state.items = items;
  renderItems();
}

function scheduleSearch() {
  window.clearTimeout(state.searchTimer);
  state.searchTimer = window.setTimeout(() => {
    refreshItems().catch((error) => toast(errorMessage(error), { error: true }));
  }, 180);
}

function newItem({ force = false } = {}) {
  if (!force && state.itemDirty && !window.confirm("Discard the unsaved Qi entry?")) return;
  state.suppressDirty = true;
  state.selectedItemId = null;
  elements.itemForm.reset();
  elements.itemType.value = "login";
  elements.questionList.replaceChildren();
  elements.historyList.replaceChildren();
  elements.historySection.hidden = true;
  elements.itemEditorTitle.textContent = "New Qi";
  elements.totpCode.textContent = "— — — — — —";
  elements.totpRemaining.textContent = "Uses device time; verify automatic time if a code is rejected.";
  state.itemDirty = false;
  state.suppressDirty = false;
  updateItemTypeFields();
  updateDirtyIndicators();
  remaskPassword();
  renderItems();
  updateContextActionState();
  elements.itemTitle.focus();
}

async function selectItem(id, { force = false } = {}) {
  if (!force && state.itemDirty && !window.confirm("Discard changes to the current Qi entry?")) return;
  const item = await busy(null, () => vaultApi.getItem(id));
  state.suppressDirty = true;
  state.selectedItemId = id;
  elements.itemTitle.value = item.title || "";
  elements.itemType.value = item.item_type;
  elements.itemFolder.value = item.folder || "";
  elements.itemTags.value = (item.tags || []).join(", ");
  elements.itemUrl.value = item.url || "";
  elements.itemUsername.value = item.username || "";
  elements.itemPassword.value = item.password || "";
  elements.itemTotpSecret.value = item.totp_secret || "";
  elements.itemNotes.value = item.notes || "";
  elements.questionList.replaceChildren();
  for (const question of item.security_questions || []) addQuestionRow(question);
  renderHistory(item.password_history || []);
  elements.itemEditorTitle.textContent = item.title;
  state.itemDirty = false;
  state.suppressDirty = false;
  updateItemTypeFields();
  updateDirtyIndicators();
  remaskPassword();
  renderItems();
  updateContextActionState();
}

function updateItemTypeFields() {
  const isLogin = elements.itemType.value === "login";
  elements.credentialFields.hidden = !isLogin;
  elements.questionSection.hidden = !isLogin;
}

function collectQuestions() {
  return [...elements.questionList.querySelectorAll(".question-row")]
    .map((row) => ({
      question: row.querySelector(".question-input").value.trim(),
      answer: row.querySelector(".answer-input").value
    }))
    .filter((entry) => entry.question || entry.answer);
}

function addQuestionRow(question = { question: "", answer: "" }) {
  const row = createElement("div", { className: "question-row" });
  const questionInput = createElement("input", {
    className: "question-input",
    attributes: { "aria-label": "Security question", maxlength: "4096", placeholder: "Question" }
  });
  questionInput.value = question.question || "";
  const answerInput = createElement("input", {
    className: "answer-input",
    type: "password",
    attributes: { "aria-label": "Security answer", maxlength: "4096", placeholder: "Answer", autocomplete: "off" }
  });
  answerInput.value = question.answer || "";
  const copyButton = createElement("button", { className: "secondary", text: "Copy", type: "button" });
  copyButton.addEventListener("click", () => copySecret(answerInput.value, "Security answer"));
  const removeButton = createElement("button", { className: "danger remove-question", text: "Remove", type: "button" });
  removeButton.addEventListener("click", () => {
    row.remove();
    markItemDirty();
  });
  for (const input of [questionInput, answerInput]) input.addEventListener("input", markItemDirty);
  row.append(questionInput, answerInput, copyButton, removeButton);
  elements.questionList.append(row);
}

function renderHistory(history) {
  elements.historyList.replaceChildren();
  elements.historySection.hidden = history.length === 0;
  for (const entry of history) {
    const row = createElement("div", { className: "history-row" });
    row.append(
      createElement("code", { text: "••••••••••••" }),
      createElement("small", { text: formatDate(entry.changed_at) })
    );
    const copy = createElement("button", { className: "secondary compact", text: "Copy", type: "button" });
    copy.addEventListener("click", () => copySecret(entry.password, "Historical password"));
    const restore = createElement("button", { className: "secondary compact", text: "Restore", type: "button" });
    restore.addEventListener("click", () => {
      elements.itemPassword.value = entry.password;
      markItemDirty();
      toast("Historical password placed in the editor. Save Qi to apply it.");
    });
    row.append(copy, restore);
    elements.historyList.append(row);
  }
}

async function saveItem() {
  if (!elements.itemForm.reportValidity()) return;
  const input = {
    item_type: elements.itemType.value,
    title: elements.itemTitle.value.trim(),
    username: elements.itemType.value === "login" ? elements.itemUsername.value || null : null,
    password: elements.itemType.value === "login" ? elements.itemPassword.value || null : null,
    url: elements.itemType.value === "login" ? elements.itemUrl.value.trim() || null : null,
    notes: elements.itemNotes.value || null,
    tags: elements.itemTags.value.split(",").map((tag) => tag.trim()).filter(Boolean),
    folder: elements.itemFolder.value.trim() || null,
    security_questions: elements.itemType.value === "login" ? collectQuestions() : [],
    totp_secret: elements.itemType.value === "login" ? elements.itemTotpSecret.value.trim() || null : null
  };
  const id = await busy(elements.saveAction, async () => {
    if (state.selectedItemId) {
      await vaultApi.updateItem(state.selectedItemId, {
        title: input.title,
        username: input.username,
        password: input.password,
        url: input.url,
        notes: input.notes,
        tags: input.tags,
        folder: input.folder,
        security_questions: input.security_questions,
        totp_secret: input.totp_secret
      });
      return state.selectedItemId;
    }
    return vaultApi.addItem(input);
  });
  state.itemDirty = false;
  await refreshItems();
  await selectItem(id, { force: true });
  toast("Qi saved to the encrypted vault.");
}

async function deleteItem() {
  if (!state.selectedItemId) throw new Error("Select a Qi entry to delete.");
  if (!window.confirm(`Delete “${elements.itemTitle.value}”? You can undo this deletion from the notification.`)) return;
  await busy(elements.deleteAction, () => vaultApi.deleteItem(state.selectedItemId));
  newItem({ force: true });
  await refreshItems();
  toast("Qi moved to encrypted deletion history.", {
    actionLabel: "Undo",
    action: async () => {
      const restoredId = await vaultApi.undoDelete();
      await refreshItems();
      await selectItem(restoredId, { force: true });
      toast("Deleted Qi restored.");
    }
  });
}

function remaskPassword() {
  elements.itemPassword.type = "password";
  elements.togglePassword.textContent = "Show";
  elements.togglePassword.setAttribute("aria-pressed", "false");
  elements.itemTotpSecret.type = "password";
}

function togglePasswordVisibility() {
  const visible = elements.itemPassword.type === "password";
  elements.itemPassword.type = visible ? "text" : "password";
  elements.togglePassword.textContent = visible ? "Hide" : "Show";
  elements.togglePassword.setAttribute("aria-pressed", String(visible));
}

async function copySecret(value, label) {
  if (!value) throw new Error(`${label} is empty.`);
  const seconds = await vaultApi.copySecret(value);
  toast(`${label} copied. QiRing will clear it in ${seconds} seconds if it is still unchanged.`);
}

async function openCurrentUrl() {
  const raw = elements.itemUrl.value.trim();
  let parsed;
  try {
    parsed = new URL(raw);
  } catch {
    throw new Error("Enter a complete URL beginning with http:// or https://.");
  }
  if (!new Set(["http:", "https:"]).has(parsed.protocol)) {
    throw new Error("QiRing only opens HTTP and HTTPS URLs.");
  }
  await vaultApi.openUrl(parsed.toString());
}

async function generateFromSelectedProfile() {
  const profile = state.profiles.find((candidate) => candidate.id === elements.profileSelect.value);
  if (!profile) throw new Error("Select a password profile first.");
  const result = await vaultApi.generatePassword(profile.policy);
  elements.itemPassword.value = result.value;
  markItemDirty();
  toast(`Generated a ${profile.policy.length}-character password from “${profile.name}”.`);
}

async function refreshTotpCode() {
  if (!state.selectedItemId || state.itemDirty) {
    throw new Error("Save this Qi before requesting its one-time code.");
  }
  const result = await vaultApi.totp(state.selectedItemId);
  elements.totpCode.textContent = result.code;
  state.totpExpiresAt = Date.now() + result.valid_for_seconds * 1000;
  updateTotpCountdown();
}

function updateTotpCountdown() {
  window.clearInterval(state.totpTimer);
  const tick = () => {
    const seconds = Math.max(0, Math.ceil((state.totpExpiresAt - Date.now()) / 1000));
    elements.totpRemaining.textContent = seconds > 0 ? `Valid for ${seconds} second${seconds === 1 ? "" : "s"}.` : "Expired. Request a new code.";
    if (seconds === 0) window.clearInterval(state.totpTimer);
  };
  tick();
  state.totpTimer = window.setInterval(tick, 1000);
}

function renderProfiles() {
  elements.profileList.replaceChildren();
  elements.profileSelect.replaceChildren();
  elements.profileCount.textContent = String(state.profiles.length);
  for (const profile of state.profiles) {
    const option = document.createElement("option");
    option.value = profile.id;
    option.textContent = `${profile.name} · ${profile.policy.length}`;
    elements.profileSelect.append(option);

    const button = createElement("button", { className: "master-list-item", type: "button" });
    button.classList.toggle("active", profile.id === state.selectedProfileId);
    button.setAttribute("aria-pressed", String(profile.id === state.selectedProfileId));
    button.append(
      createElement("strong", { text: profile.name }),
      createElement("small", { text: `${profile.policy.length} CHARACTERS` })
    );
    button.addEventListener("click", () => selectProfile(profile.id));
    elements.profileList.append(button);
  }
  if (state.profiles.length === 0) {
    elements.profileList.append(createElement("p", { className: "empty-message", text: "No profiles available." }));
  }
}

function newProfile({ force = false } = {}) {
  if (!force && state.profileDirty && !window.confirm("Discard the unsaved password profile?")) return;
  state.suppressDirty = true;
  state.selectedProfileId = null;
  elements.profileForm.reset();
  elements.profileLength.value = "20";
  for (const id of ["upperMin", "lowerMin", "numbersMin", "symbolsMin"]) elements[id].value = "1";
  for (const id of ["upperMax", "lowerMax", "numbersMax", "symbolsMax"]) elements[id].value = "20";
  elements.allowedSymbols.value = "!@#$%^&*()-_=+[]{};:,.?/";
  elements.avoidAmbiguous.checked = false;
  elements.profileEditorTitle.textContent = "New profile";
  elements.profileTestOutput.textContent = "Generated samples appear here.";
  state.profileDirty = false;
  state.suppressDirty = false;
  updateDirtyIndicators();
  renderProfiles();
  updateContextActionState();
  elements.profileName.focus();
}

function selectProfile(id, { force = false } = {}) {
  if (!force && state.profileDirty && !window.confirm("Discard changes to this password profile?")) return;
  const profile = state.profiles.find((candidate) => candidate.id === id);
  if (!profile) return;
  state.suppressDirty = true;
  state.selectedProfileId = id;
  elements.profileName.value = profile.name;
  elements.profileLength.value = String(profile.policy.length);
  for (const [prefix, range] of Object.entries({
    upper: profile.policy.upper,
    lower: profile.policy.lower,
    numbers: profile.policy.numbers,
    symbols: profile.policy.symbols
  })) {
    elements[`${prefix}Min`].value = String(range.min);
    elements[`${prefix}Max`].value = String(range.max);
  }
  elements.allowedSymbols.value = profile.policy.allowed_symbols || "!@#$%^&*()-_=+[]{};:,.?/";
  elements.avoidAmbiguous.checked = Boolean(profile.policy.avoid_ambiguous);
  elements.profileEditorTitle.textContent = profile.name;
  state.profileDirty = false;
  state.suppressDirty = false;
  updateDirtyIndicators();
  renderProfiles();
  updateContextActionState();
}

function profileFromForm() {
  const number = (id) => Number(elements[id].value);
  return {
    id: state.selectedProfileId || NIL_UUID,
    name: elements.profileName.value.trim(),
    policy: {
      length: number("profileLength"),
      upper: { min: number("upperMin"), max: number("upperMax") },
      lower: { min: number("lowerMin"), max: number("lowerMax") },
      numbers: { min: number("numbersMin"), max: number("numbersMax") },
      symbols: { min: number("symbolsMin"), max: number("symbolsMax") },
      allowed_symbols: elements.allowedSymbols.value,
      avoid_ambiguous: elements.avoidAmbiguous.checked
    }
  };
}

async function saveProfile() {
  if (!elements.profileForm.reportValidity()) return;
  const id = await busy(elements.saveAction, () => vaultApi.saveProfile(profileFromForm()));
  state.profiles = await vaultApi.listProfiles();
  state.profileDirty = false;
  selectProfile(id, { force: true });
  renderProfiles();
  toast("Password profile saved inside the encrypted vault.");
}

async function deleteProfile() {
  if (!state.selectedProfileId) throw new Error("Select a password profile to delete.");
  if (!window.confirm(`Delete password profile “${elements.profileName.value}”?`)) return;
  await busy(elements.deleteAction, () => vaultApi.deleteProfile(state.selectedProfileId));
  state.profiles = await vaultApi.listProfiles();
  const next = state.profiles[0]?.id;
  if (next) selectProfile(next, { force: true });
  else newProfile({ force: true });
  renderProfiles();
  toast("Password profile deleted.");
}

async function testProfile() {
  if (!elements.profileForm.reportValidity()) return;
  const result = await vaultApi.generatePassword(profileFromForm().policy);
  elements.profileTestOutput.textContent = result.value;
}

async function runHealthAnalysis() {
  const report = await busy(elements.runHealth, () => vaultApi.health());
  elements.healthAnalyzed.textContent = String(report.analyzed_items);
  elements.healthWeak.textContent = String(report.weak_count);
  elements.healthReused.textContent = String(report.reused_count);
  elements.healthOld.textContent = String(report.old_count);
  elements.healthIssues.replaceChildren();
  for (const issue of report.issues) {
    const row = createElement("div", { className: "issue-row" });
    row.append(
      createElement("span", { className: "issue-kind", text: issue.kind.toUpperCase() }),
      createElement("strong", { text: issue.title }),
      createElement("p", { text: issue.detail })
    );
    const open = createElement("button", { className: "secondary compact", text: "Open Qi", type: "button" });
    open.addEventListener("click", async () => {
      await navigate("qiring", { force: true });
      await selectItem(issue.item_id, { force: true });
    });
    row.append(open);
    elements.healthIssues.append(row);
  }
  if (report.issues.length === 0) {
    elements.healthIssues.append(createElement("p", { className: "empty-message", text: "No weak, reused, or old passwords were detected." }));
  }
}

function fillSettings(settings) {
  state.suppressDirty = true;
  elements.autoLockMinutes.value = String(settings.auto_lock_minutes);
  elements.clipboardSeconds.value = String(settings.clipboard_clear_seconds);
  elements.lockOnMinimize.checked = settings.lock_on_minimize;
  elements.lockOnBlur.checked = settings.lock_on_window_blur;
  elements.themeSelect.value = settings.theme;
  elements.automaticBackups.checked = settings.backup_preferences.automatic_enabled;
  elements.includeSettings.checked = settings.backup_preferences.include_settings;
  elements.backupRetention.value = String(settings.backup_preferences.retention_count);
  elements.backupDirectory.value = settings.backup_preferences.directory || "";
  state.settingsDirty = false;
  state.suppressDirty = false;
  updateContextActionState();
}

async function loadSettings() {
  fillSettings(await vaultApi.getSettings());
}

function settingsFromForm() {
  return {
    auto_lock_minutes: Number(elements.autoLockMinutes.value),
    clipboard_clear_seconds: Number(elements.clipboardSeconds.value),
    lock_on_window_blur: elements.lockOnBlur.checked,
    lock_on_minimize: elements.lockOnMinimize.checked,
    biometric_enabled: false,
    theme: elements.themeSelect.value,
    backup_preferences: {
      include_settings: elements.includeSettings.checked,
      automatic_enabled: elements.automaticBackups.checked,
      directory: elements.backupDirectory.value || null,
      retention_count: Number(elements.backupRetention.value)
    }
  };
}

async function saveSettings() {
  if (!elements.settingsForm.reportValidity()) return;
  const settings = settingsFromForm();
  if (settings.backup_preferences.automatic_enabled && !settings.backup_preferences.directory) {
    throw new Error("Choose a directory before enabling automatic snapshots.");
  }
  await busy(elements.saveAction, () => vaultApi.updateSettings(settings));
  state.settingsDirty = false;
  applyTheme(settings.theme);
  updateContextActionState();
  toast("Encrypted vault settings saved and enforced.");
}

async function chooseBackupDirectory() {
  const path = await vaultApi.chooseBackupDirectory();
  if (!path) return;
  elements.backupDirectory.value = path;
  markSettingsDirty();
}

async function rotateMasterPassword() {
  const oldPassword = elements.oldMasterPassword.value;
  const newPassword = elements.newMasterPassword.value;
  if (!oldPassword || !newPassword) throw new Error("Enter both the current and new master passwords.");
  if (!window.confirm("Rotate the master password now? The old password will stop unlocking this vault.")) return;
  await busy(elements.rotateMaster, () => vaultApi.rotateMaster(oldPassword, newPassword));
  elements.oldMasterPassword.value = "";
  elements.newMasterPassword.value = "";
  toast("Master password rotated. Recovery access remains valid.");
}

async function regenerateRecovery() {
  const password = elements.recoveryMasterPassword.value;
  if (!password) throw new Error("Enter your master password first.");
  if (!window.confirm("Replace the recovery key? The current recovery key will stop working immediately.")) return;
  const material = await busy(elements.regenerateRecovery, () => vaultApi.regenerateRecovery(password));
  elements.recoveryMasterPassword.value = "";
  showRecoveryCeremony(material, async () => toast("Recovery key replaced."));
}

async function exportBackup() {
  const passphrase = elements.backupPassphrase.value;
  if (!passphrase) throw new Error("Enter a backup passphrase with at least 12 characters.");
  const manifest = await busy(elements.exportBackup, () => vaultApi.exportBackup(passphrase));
  if (!manifest) return;
  elements.backupPassphrase.value = "";
  toast(`Encrypted backup exported (${formatBytes(manifest.size_bytes)}).`);
}

async function selectBackup() {
  const selection = await busy(elements.selectBackup, () => vaultApi.selectBackup());
  if (!selection) return;
  state.selectedBackupToken = selection.token;
  state.backupPreviewed = false;
  elements.selectedBackupPath.textContent = selection.display_path;
  elements.backupPreview.hidden = true;
  elements.restoreBackup.disabled = true;
}

async function previewBackup() {
  if (!state.selectedBackupToken) throw new Error("Choose an encrypted backup first.");
  const passphrase = elements.restorePassphrase.value;
  const preview = await busy(elements.previewBackup, () => vaultApi.previewBackup(state.selectedBackupToken, passphrase));
  elements.backupPreview.replaceChildren(
    createElement("div", { text: `Vault: ${preview.vault_id}` }),
    createElement("div", { text: `Vault created: ${formatDate(preview.vault_created_at)}` }),
    createElement("div", { text: `Backup created: ${formatDate(preview.backup_created_at)}` }),
    createElement("div", { text: `Schema: v${preview.vault_schema_version} · ${formatBytes(preview.size_bytes)}` })
  );
  elements.backupPreview.hidden = false;
  elements.restoreBackup.disabled = false;
  state.backupPreviewed = true;
}

async function restoreBackup() {
  if (!state.selectedBackupToken || !state.backupPreviewed) throw new Error("Preview the backup before restoring it.");
  if (!window.confirm("Replace the current vault with the previewed backup? QiRing will lock immediately after the atomic restore.")) return;
  const report = await busy(elements.restoreBackup, () => vaultApi.importBackup(state.selectedBackupToken, elements.restorePassphrase.value));
  resetSessionUi();
  setAuthScreen("unlock");
  toast(`Backup restored. A recoverable pre-restore snapshot was retained (${formatBytes(report.size_bytes)} restored).`);
}

async function refreshSnapshots() {
  const snapshots = await busy(elements.refreshSnapshots, () => vaultApi.listSnapshots());
  elements.snapshotList.replaceChildren();
  for (const snapshot of snapshots) {
    const row = createElement("div", { className: "snapshot-row" });
    row.append(
      createElement("small", { text: snapshot.path }),
      createElement("span", { text: `${formatDate(snapshot.created_at)} · ${formatBytes(snapshot.size_bytes)}` })
    );
    const restore = createElement("button", { className: "danger compact", text: "Restore", type: "button" });
    restore.addEventListener("click", async () => {
      try {
        if (!window.confirm("Restore this automatic snapshot and replace the current vault? QiRing will lock.")) return;
        await vaultApi.restoreSnapshot(snapshot.path);
        resetSessionUi();
        setAuthScreen("unlock");
        toast("Automatic snapshot restored. Unlock to continue.");
      } catch (error) {
        toast(errorMessage(error), { error: true });
      }
    });
    row.append(restore);
    elements.snapshotList.append(row);
  }
  if (snapshots.length === 0) {
    elements.snapshotList.append(createElement("p", { className: "empty-message", text: "No automatic snapshots are available. Configure them in Settings." }));
  }
}

function resetSessionUi() {
  state.unlocked = false;
  state.items = [];
  state.profiles = [];
  state.selectedItemId = null;
  state.selectedProfileId = null;
  state.itemDirty = false;
  state.profileDirty = false;
  state.settingsDirty = false;
  state.selectedBackupToken = null;
  state.backupPreviewed = false;
  elements.toastRegion.replaceChildren();
  window.clearInterval(state.totpTimer);
  remaskPassword();
  elements.itemList.replaceChildren();
  elements.profileList.replaceChildren();
  elements.itemForm.reset();
  elements.profileForm.reset();
}

async function lockVault({ force = false } = {}) {
  if (!force && hasUnsavedChanges() && !window.confirm("Lock the vault and discard unsaved changes?")) return;
  await vaultApi.lock();
  resetSessionUi();
  setAuthScreen("unlock");
  toast("Vault locked and any QiRing-owned clipboard value was cleared.");
}

async function handleBackendLock(event) {
  if (!state.unlocked) return;
  resetSessionUi();
  setAuthScreen("unlock");
  toast(event.payload === "idle" ? "Vault locked after inactivity." : "Vault locked by window security policy.");
}

async function handleContextAction(kind) {
  if (kind === "new") {
    if (state.view === "qiring") newItem();
    if (state.view === "profiles") newProfile();
    return;
  }
  if (kind === "save") {
    if (state.view === "qiring") await saveItem();
    if (state.view === "profiles") await saveProfile();
    if (state.view === "settings") await saveSettings();
    if (state.view === "backups") await exportBackup();
    return;
  }
  if (kind === "delete") {
    if (state.view === "qiring") await deleteItem();
    if (state.view === "profiles") await deleteProfile();
  }
}

function runSafely(task, preventDefault = false) {
  return async (event) => {
    if (preventDefault) event?.preventDefault();
    try {
      await task(event);
    } catch (error) {
      toast(errorMessage(error), { error: true });
    }
  };
}

elements.createForm.addEventListener("submit", runSafely(async () => {
  if (elements.createMaster.value !== elements.createConfirm.value) throw new Error("Master password confirmation does not match.");
  const result = await busy(elements.createForm.querySelector("button[type=submit]"), () =>
    vaultApi.create(elements.createMaster.value, DEFAULT_SETTINGS)
  );
  elements.createForm.reset();
  showRecoveryCeremony(result.recovery, async () => {
    setAuthScreen("unlock");
    toast("Vault initialized. Unlock it with your master password.");
  });
}, true));

elements.masterUnlockPanel.addEventListener("submit", runSafely(async () => {
  const result = await busy(elements.masterUnlockPanel.querySelector("button[type=submit]"), () =>
    vaultApi.unlockMaster(elements.unlockMaster.value)
  );
  elements.unlockMaster.value = "";
  await enterVault();
  if (result.migrated_recovery) {
    showRecoveryCeremony(result.migrated_recovery, async () => toast("Legacy vault migrated to authenticated schema v2."));
  } else {
    toast("Vault unlocked.");
  }
}, true));

elements.recoveryUnlockPanel.addEventListener("submit", runSafely(async () => {
  if (elements.recoveryMaster.value !== elements.recoveryConfirm.value) throw new Error("New master password confirmation does not match.");
  const result = await busy(elements.recoveryUnlockPanel.querySelector("button[type=submit]"), () =>
    vaultApi.unlockRecovery(elements.recoveryKey.value, elements.recoveryMaster.value)
  );
  elements.recoveryUnlockPanel.reset();
  await enterVault();
  showRecoveryCeremony(result.recovery, async () => toast("Vault recovered. Master and recovery credentials were rotated."));
}, true));

elements.masterTab.addEventListener("click", () => setUnlockTab("master"));
elements.recoveryTab.addEventListener("click", () => setUnlockTab("recovery"));
for (const tab of [elements.masterTab, elements.recoveryTab]) {
  tab.addEventListener("keydown", (event) => {
    if (!new Set(["ArrowLeft", "ArrowRight", "Home", "End"]).has(event.key)) return;
    event.preventDefault();
    const master = event.key === "ArrowLeft" || event.key === "Home"
      ? true
      : event.key === "ArrowRight" || event.key === "End"
        ? false
        : tab === elements.recoveryTab;
    setUnlockTab(master ? "master" : "recovery");
    (master ? elements.masterTab : elements.recoveryTab).focus();
  });
}
elements.recoveryAcknowledged.addEventListener("change", updateRecoveryReady);
elements.recoveryVerify.addEventListener("input", updateRecoveryReady);
elements.finishRecovery.addEventListener("click", runSafely(finishRecoveryCeremony));
elements.recoveryDialog.addEventListener("cancel", (event) => event.preventDefault());
elements.copyRecoveryKey.addEventListener("click", runSafely(() => copySecret(elements.recoveryKeyOutput.textContent, "Recovery key")));
elements.saveRecoveryKey.addEventListener("click", runSafely(saveRecoveryKey));
elements.printRecoveryKey.addEventListener("click", () => window.print());

elements.menuButton.addEventListener("click", () => elements.appMenu.hidden ? openMenu() : closeMenu({ restoreFocus: true }));
elements.appMenu.addEventListener("keydown", (event) => {
  const buttons = [...elements.appMenu.querySelectorAll("button:not(:disabled)")];
  const current = buttons.indexOf(document.activeElement);
  let next = current;
  if (event.key === "ArrowDown") next = (current + 1) % buttons.length;
  else if (event.key === "ArrowUp") next = (current - 1 + buttons.length) % buttons.length;
  else if (event.key === "Home") next = 0;
  else if (event.key === "End") next = buttons.length - 1;
  else return;
  event.preventDefault();
  buttons[next]?.focus();
});
elements.brandHome.addEventListener("click", () => navigate("qiring"));
elements.appMenu.querySelectorAll("[data-view]").forEach((button) => button.addEventListener("click", () => runSafely(() => navigate(button.dataset.view))()));
elements.lockButton.addEventListener("click", runSafely(lockVault));
elements.newAction.addEventListener("click", runSafely(() => handleContextAction("new")));
elements.saveAction.addEventListener("click", runSafely(() => handleContextAction("save")));
elements.deleteAction.addEventListener("click", runSafely(() => handleContextAction("delete")));

elements.itemForm.addEventListener("input", markItemDirty);
elements.itemForm.addEventListener("change", markItemDirty);
elements.itemType.addEventListener("change", updateItemTypeFields);
elements.addQuestion.addEventListener("click", () => {
  addQuestionRow();
  markItemDirty();
});
elements.searchInput.addEventListener("input", scheduleSearch);
elements.clearSearch.addEventListener("click", runSafely(async () => {
  elements.searchInput.value = "";
  await refreshItems();
  elements.searchInput.focus();
}));
elements.togglePassword.addEventListener("click", togglePasswordVisibility);
elements.copyUsername.addEventListener("click", runSafely(() => copySecret(elements.itemUsername.value, "Username")));
elements.copyPassword.addEventListener("click", runSafely(() => copySecret(elements.itemPassword.value, "Password")));
elements.openUrlButton.addEventListener("click", runSafely(openCurrentUrl));
elements.generatePassword.addEventListener("click", runSafely(generateFromSelectedProfile));
elements.refreshTotp.addEventListener("click", runSafely(refreshTotpCode));

elements.profileForm.addEventListener("input", markProfileDirty);
elements.profileForm.addEventListener("change", markProfileDirty);
elements.testProfile.addEventListener("click", runSafely(testProfile));

elements.runHealth.addEventListener("click", runSafely(runHealthAnalysis));
elements.settingsForm.addEventListener("input", markSettingsDirty);
elements.settingsForm.addEventListener("change", markSettingsDirty);
elements.chooseBackupDirectory.addEventListener("click", runSafely(chooseBackupDirectory));
elements.rotateMaster.addEventListener("click", runSafely(rotateMasterPassword));
elements.regenerateRecovery.addEventListener("click", runSafely(regenerateRecovery));
elements.themeSelect.addEventListener("change", () => applyTheme(elements.themeSelect.value));
elements.backupPassphrase.addEventListener("input", updateContextActionState);

elements.exportBackup.addEventListener("click", runSafely(exportBackup));
elements.selectBackup.addEventListener("click", runSafely(selectBackup));
elements.previewBackup.addEventListener("click", runSafely(previewBackup));
elements.restoreBackup.addEventListener("click", runSafely(restoreBackup));
elements.refreshSnapshots.addEventListener("click", runSafely(refreshSnapshots));

document.addEventListener("click", (event) => {
  if (!event.target.closest(".menu-wrap")) closeMenu();
});

document.addEventListener("keydown", runSafely(async (event) => {
  if (event.key === "Escape") {
    if (!elements.appMenu.hidden) closeMenu({ restoreFocus: true });
    remaskPassword();
    return;
  }
  if (!state.unlocked) return;
  const modifier = event.metaKey || event.ctrlKey;
  if (!modifier) return;
  if (event.shiftKey && event.key.toLowerCase() === "u") {
    event.preventDefault();
    await copySecret(elements.itemUsername.value, "Username");
  } else if (event.shiftKey && event.key.toLowerCase() === "p") {
    event.preventDefault();
    await copySecret(elements.itemPassword.value, "Password");
  } else if (event.key.toLowerCase() === "s") {
    event.preventDefault();
    await handleContextAction("save");
  } else if (event.key.toLowerCase() === "n") {
    event.preventDefault();
    await handleContextAction("new");
  } else if (event.key.toLowerCase() === "l") {
    event.preventDefault();
    await lockVault();
  } else if (event.key.toLowerCase() === "k") {
    event.preventDefault();
    await navigate("qiring");
    elements.searchInput.focus();
  } else if (/^[1-5]$/.test(event.key)) {
    event.preventDefault();
    await navigate(["qiring", "profiles", "health", "backups", "settings"][Number(event.key) - 1]);
  }
}));

for (const eventName of ["pointerdown", "keydown"]) {
  document.addEventListener(eventName, () => {
    if (!state.unlocked || Date.now() - state.lastActivityTouch < 30_000) return;
    state.lastActivityTouch = Date.now();
    vaultApi.touch().catch(() => {});
  }, { passive: true });
}

window.addEventListener("beforeunload", (event) => {
  if (!hasUnsavedChanges()) return;
  event.preventDefault();
  event.returnValue = "";
});

window.matchMedia("(prefers-color-scheme: dark)").addEventListener("change", () => {
  if (elements.themeSelect.value === "system") applyTheme("system");
});

async function bootstrap() {
  await vaultApi.onLocked(handleBackendLock);
  const exists = await vaultApi.exists();
  setUnlockTab("master");
  setAuthScreen(exists ? "unlock" : "create");
}

bootstrap().catch((error) => {
  toast(`Startup failed: ${errorMessage(error)}`, { error: true });
  setAuthScreen("create");
});
