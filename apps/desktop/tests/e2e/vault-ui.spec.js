import { test, expect } from "@playwright/test";
import AxeBuilder from "@axe-core/playwright";

async function installTauriMock(page) {
  await page.addInitScript(() => {
    let frameCounter = 0;
    const countFrame = () => {
      frameCounter += 1;
      window.__frameCounter = frameCounter;
      window.requestAnimationFrame(countFrame);
    };
    window.requestAnimationFrame(countFrame);
    const callbacks = new Map();
    let callbackId = 1;
    const profiles = [
      {
        id: "11111111-1111-4111-8111-111111111111",
        name: "Strong 20",
        policy: {
          length: 20,
          upper: { min: 1, max: 20 },
          lower: { min: 1, max: 20 },
          numbers: { min: 1, max: 20 },
          symbols: { min: 1, max: 20 },
          allowed_symbols: "!@#$%^&*()-_=+[]{};:,.?/",
          avoid_ambiguous: false
        }
      },
      {
        id: "33333333-3333-4333-8333-333333333333",
        name: "Alphanumeric 20",
        policy: {
          length: 20,
          upper: { min: 1, max: 20 },
          lower: { min: 1, max: 20 },
          numbers: { min: 1, max: 20 },
          symbols: { min: 0, max: 0 },
          allowed_symbols: "!@#$%^&*()-_=+[]{};:,.?/",
          avoid_ambiguous: true
        }
      }
    ];
    const items = [
      { id: "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa", item_type: "login", title: "Admin", username: "admin@example.com", folder: "Work", tags: ["critical", "work"], icon_data_url: null, has_totp: true, updated_at: new Date().toISOString() },
      { id: "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb", item_type: "login", title: "Billing", username: "billing@example.com", folder: "Work", tags: ["finance", "work"], icon_data_url: null, has_totp: false, updated_at: new Date().toISOString() },
      { id: "cccccccc-cccc-4ccc-8ccc-cccccccccccc", item_type: "secure_note", title: "Passport", username: null, folder: "Personal", tags: ["identity"], icon_data_url: null, has_totp: false, updated_at: new Date().toISOString() }
    ];
    const itemRecords = new Map(items.map((item) => [item.id, {
      ...item,
      password: null,
      url: null,
      notes: null,
      security_questions: [],
      custom_fields: [],
      totp_secret: null,
      password_history: []
    }]));
    const settings = {
      auto_lock_minutes: 5,
      clipboard_clear_seconds: 30,
      lock_on_window_blur: false,
      lock_on_minimize: true,
      biometric_enabled: false,
      theme: "dark",
      button_display: "both",
      ring_sort_mode: "custom",
      ring_category_order: [],
      ring_item_order: [],
      backup_preferences: {
        include_settings: true,
        automatic_enabled: false,
        directory: null,
        retention_count: 10
      }
    };
    let csvPreviewCount = 0;

    window.__TAURI_INTERNALS__ = {
      transformCallback(callback, once = false) {
        const id = callbackId++;
        callbacks.set(id, (payload) => {
          callback(payload);
          if (once) callbacks.delete(id);
        });
        return id;
      },
      unregisterCallback(id) {
        callbacks.delete(id);
      },
      async invoke(command, args = {}) {
        switch (command) {
          case "plugin:event|listen": return args.handler;
          case "plugin:event|unlisten": return null;
          case "get_bootstrap_theme": return window.sessionStorage.getItem("mock.qiring.theme") || "dark";
          case "set_bootstrap_theme": window.sessionStorage.setItem("mock.qiring.theme", args.theme); return null;
          case "vault_exists": return true;
          case "unlock_vault_master": {
            window.__unlockFeedbackAtInvoke = {
              frame: frameCounter,
              busy: document.querySelector("#masterUnlockPanel button[type='submit']").getAttribute("aria-busy"),
              label: document.querySelector("#masterUnlockPanel button[type='submit']").textContent.trim()
            };
            await new Promise((resolve) => window.setTimeout(resolve, 800));
            return { session: { token: "session", unlocked_at: new Date().toISOString() }, migrated_recovery: null };
          }
          case "get_security_status": return {
            schema_version: 2,
            command_version: "8",
            biometric_available: false,
            biometric_enabled: false,
            auto_lock_minutes: settings.auto_lock_minutes,
            clipboard_clear_seconds: settings.clipboard_clear_seconds,
            lock_on_window_blur: settings.lock_on_window_blur,
            lock_on_minimize: settings.lock_on_minimize
          };
          case "list_profiles": return structuredClone(profiles);
          case "list_items": {
            const query = args.filter?.query?.toLocaleLowerCase();
            const tag = args.filter?.tag;
            return structuredClone(items.filter((item) => {
              const matchesQuery = !query || [item.title, item.username, item.folder, ...(item.tags || [])]
                .some((value) => value?.toLocaleLowerCase().includes(query));
              return matchesQuery && (!tag || item.tags.includes(tag));
            }));
          }
          case "get_item": return structuredClone(itemRecords.get(args.itemId));
          case "add_item": {
            window.__lastItemInput = structuredClone(args.input);
            const id = "dddddddd-dddd-4ddd-8ddd-dddddddddddd";
            const record = {
              id,
              ...structuredClone(args.input),
              has_totp: Boolean(args.input.totp_secret),
              updated_at: new Date().toISOString(),
              password_history: []
            };
            itemRecords.set(id, record);
            items.push({
              id,
              item_type: record.item_type,
              title: record.title,
              username: record.username,
              folder: record.folder,
              tags: record.tags,
              icon_data_url: record.icon_data_url,
              has_totp: record.has_totp,
              updated_at: record.updated_at
            });
            return id;
          }
          case "update_item": {
            window.__lastItemPatch = structuredClone(args.patch);
            const record = itemRecords.get(args.itemId);
            Object.assign(record, structuredClone(args.patch), { updated_at: new Date().toISOString() });
            const summary = items.find((item) => item.id === args.itemId);
            Object.assign(summary, {
              title: record.title,
              username: record.username,
              folder: record.folder,
              tags: record.tags,
              icon_data_url: record.icon_data_url,
              has_totp: Boolean(record.totp_secret),
              updated_at: record.updated_at
            });
            return null;
          }
          case "delete_item": {
            const index = items.findIndex((item) => item.id === args.itemId);
            if (index >= 0) items.splice(index, 1);
            itemRecords.delete(args.itemId);
            return null;
          }
          case "select_item_icon_dialog": return "data:image/png;base64,iVBORw0KGgo=";
          case "fetch_favicon": return "data:image/png;base64,iVBORw0KGgo=";
          case "get_totp_code": return { code: "123456", valid_for_seconds: 30 };
          case "regenerate_recovery_key": return {
            recovery_key: "replacement-recovery-key-123456",
            recovery_key_fingerprint: "replacement"
          };
          case "rotate_master_password":
            window.__rotatedMasterPassword = structuredClone(args);
            return null;
          case "get_settings": return structuredClone(settings);
          case "update_settings": Object.assign(settings, structuredClone(args.settings)); return null;
          case "generate_password": return { value: "Aaaa1111!!!!MockPass".slice(0, args.policy.length) };
          case "save_profile": {
            const profile = structuredClone(args.profile);
            if (profile.id.startsWith("00000000")) profile.id = "22222222-2222-4222-8222-222222222222";
            const index = profiles.findIndex((entry) => entry.id === profile.id);
            if (index >= 0) profiles[index] = profile;
            else profiles.push(profile);
            return profile.id;
          }
          case "delete_profile": {
            const index = profiles.findIndex((profile) => profile.id === args.profileId);
            if (index >= 0) profiles.splice(index, 1);
            return null;
          }
          case "health_report": return window.__healthWithIssue
            ? {
                analyzed_items: items.length,
                weak_count: 1,
                reused_count: 0,
                old_count: 0,
                issues: [{
                  item_id: "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
                  title: "Admin",
                  kind: "weak",
                  detail: "Password is short or lacks character variety."
                }]
              }
            : { analyzed_items: items.length, weak_count: 0, reused_count: 0, old_count: 0, issues: [] };
          case "list_snapshots": return [];
          case "save_csv_template_dialog": return "/tmp/qiring-import-template.csv";
          case "export_plaintext_csv_dialog": return { path: "/tmp/export.csv", row_count: items.length, size_bytes: 2048 };
          case "select_plaintext_csv_file": return { token: "csv-token", display_path: "/tmp/import.csv" };
          case "preview_selected_plaintext_csv": {
            csvPreviewCount += 1;
            return {
              headers: ["Name", "Login", "Password", "Extra"],
              row_count: 2,
              sample_rows: [
                [csvPreviewCount > 1 ? "Example One fixed" : "Example One", "alice@example.com", "secret-one", "Member: 42"],
                ["Example Two", "bob@example.com", "secret-two", "Color: green"]
              ],
              canonical: false,
              suggested_mapping: {
                item_type: null,
                title: "Name",
                username: "Login",
                password: "Password",
                url: null,
                notes: null,
                tags: null,
                category: null,
                security_questions: null,
                custom_fields: null,
                totp_secret: null,
                include_unmapped_in_notes: false
              },
              warnings: ["Review every suggested column mapping before importing."]
            };
          }
          case "import_selected_plaintext_csv": {
            window.__csvImportMapping = structuredClone(args.mapping);
            if (window.__failCsvImportOnce) {
              window.__failCsvImportOnce = false;
              throw new Error("CSV row 3: item_type must be login or secure_note");
            }
            return { imported_count: 2, warnings: [] };
          }
          case "touch_activity": return null;
          case "copy_secret": return 30;
          case "lock_vault": return null;
          default: throw new Error(`Unhandled Tauri mock command: ${command}`);
        }
      }
    };
  });
}

test.beforeEach(async ({ page }) => {
  await installTauriMock(page);
  await page.goto("/");
  await expect(page.locator(".auth-brand")).toHaveText("QiRing");
  await expect(page.locator("#unlockScreen .section-code, .auth-lede, .security-spec, .eyebrow")).toHaveCount(0);
  await expect(page.locator("#unlockTitle")).toHaveText("Unlock QiRing");
  await page.getByRole("button", { name: "Show master password" }).click();
  await expect(page.locator("#unlockMaster")).toHaveAttribute("type", "text");
  await expect(page.getByRole("button", { name: "Hide master password" })).toHaveAttribute("aria-pressed", "true");
  await page.getByRole("button", { name: "Hide master password" }).click();
  await page.locator("#unlockMaster").fill("correct horse battery staple");
  const unlockButton = page.locator("#masterUnlockPanel button[type=submit]");
  const frameBeforeUnlock = await page.evaluate(() => new Promise((resolve) => requestAnimationFrame(() => resolve(window.__frameCounter))));
  await unlockButton.click();
  await expect(unlockButton).toHaveAttribute("aria-busy", "true");
  await expect(unlockButton).toHaveText("Unlocking…");
  const paintedFrames = await page.evaluate(async () => {
    const start = window.__frameCounter;
    await new Promise((resolve) => window.setTimeout(resolve, 220));
    return window.__frameCounter - start;
  });
  // One frame proves the invoke yielded to the renderer; headless Chromium can
  // throttle requestAnimationFrame heavily even when the CSS spinner is active.
  expect(paintedFrames).toBeGreaterThan(1);
  await expect(page.getByRole("heading", { name: "Ring", exact: true })).toBeVisible({ timeout: 15_000 });
  await expect(page.getByLabel("Name", { exact: true })).toBeFocused();
  const feedback = await page.evaluate(() => window.__unlockFeedbackAtInvoke);
  expect(feedback.busy).toBe("true");
  expect(feedback.label).toBe("Unlocking…");
  expect(feedback.frame).toBeGreaterThan(frameBeforeUnlock);
});

test("context actions and profile master-detail editor follow the active module", async ({ page }) => {
  await expect(page.locator("#viewTitle")).toHaveText("Ring");
  await expect(page.getByRole("button", { name: "New Qi" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Save Qi" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Delete Qi" })).toBeVisible();

  await page.getByRole("button", { name: "Open navigation menu" }).click();
  await page.getByRole("menuitem", { name: /Password Profiles/ }).click();
  await expect(page.getByRole("button", { name: "New Profile" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Save Profile" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Profiles", exact: true })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Strong 20" })).toBeVisible();

  await page.getByRole("button", { name: "New Profile" }).click();
  await page.getByLabel("Profile name").fill("Strict 24");
  await page.getByLabel("Total length").fill("24");
  await page.getByLabel("Uppercase maximum").fill("8");
  await page.getByLabel("Lowercase maximum").fill("12");
  await page.getByLabel("Numbers maximum").fill("8");
  await page.getByLabel("Symbols maximum").fill("8");
  await page.getByRole("button", { name: "Test profile" }).click();
  await expect(page.locator("#profileTestOutput")).not.toHaveText("Generated samples appear here.");
  await page.getByRole("button", { name: "Save Profile" }).click();

  await page.getByRole("button", { name: "Open navigation menu" }).click();
  await page.getByRole("menuitem", { name: /Ring Health/ }).click();
  await expect(page.getByRole("button", { name: "New Profile" })).toBeHidden();
  await expect(page.getByRole("button", { name: "Save Profile" })).toBeHidden();
});

test("CSV import validates and maps before adding rows", async ({ page }) => {
  await page.getByRole("button", { name: "Open navigation menu" }).click();
  await page.getByRole("menuitem", { name: /Backups/ }).click();
  await expect(page.getByText("Anyone who can open an exported CSV")).toBeVisible();

  await page.getByRole("button", { name: "Choose CSV" }).click();
  await page.getByRole("button", { name: "Validate and map" }).click();
  await expect(page.getByText("CSV structure is valid")).toBeVisible();
  await expect(page.locator('[data-csv-field="title"]')).toHaveValue("Name");
  await expect(page.locator('[data-csv-field="username"]')).toHaveValue("Login");
  const mappingLabelWeights = await page.locator('.csv-mapping-grid label:has([data-csv-field="title"]), .csv-mapping-grid label:has([data-csv-field="username"])').evaluateAll(([title, username]) => ({
    title: getComputedStyle(title.querySelector("span")).fontWeight,
    username: getComputedStyle(username).fontWeight
  }));
  expect(mappingLabelWeights.title).toBe(mappingLabelWeights.username);
  await expect(page.getByLabel("Append every non-empty unmapped column to Notes")).toBeChecked();
  await expect(page.locator("#csvPreviewHead")).toContainText("Qi Name");
  await expect(page.locator("#csvPreviewHead")).toContainText("Username");
  await expect(page.locator("#csvPreviewBody")).toContainText("Example One");
  await expect(page.locator("#csvPreviewBody")).toContainText("••••••••");
  await expect(page.locator("#csvPreviewBody")).not.toContainText("secret-one");
  await page.locator('[data-csv-field="username"]').selectOption("");
  await expect(page.locator("#csvPreviewHead")).not.toContainText("Username");
  await page.locator('[data-csv-field="username"]').selectOption("Login");
  await page.evaluate(() => { window.__failCsvImportOnce = true; });
  await page.getByRole("button", { name: "Import validated rows" }).click();
  await expect(page.getByRole("heading", { name: "Import 2 Qi?" })).toBeVisible();
  await page.getByRole("button", { name: "Import Qi" }).click();
  await expect(page.getByText("CSV row 3: item_type must be login or secure_note")).toBeVisible();
  await expect(page.locator("#csvPreviewBody")).toContainText("Example One fixed");
  await expect(page.locator('[data-csv-field="username"]')).toHaveValue("Login");
  await expect(page.getByLabel("Append every non-empty unmapped column to Notes")).toBeChecked();
  await page.getByRole("button", { name: "Import validated rows" }).click();
  await page.getByRole("button", { name: "Import Qi" }).click();
  await expect(page.getByText("Imported 2 Qi from CSV.")).toBeVisible();
  expect(await page.evaluate(() => window.__csvImportMapping)).toMatchObject({
    title: "Name",
    username: "Login",
    password: "Password",
    include_unmapped_in_notes: true
  });
});

test("controls align and the 800 by 600 layouts do not clip", async ({ page }) => {
  await page.setViewportSize({ width: 800, height: 600 });
  const selectStyle = await page.locator("#profileSelect").evaluate((element) => {
    const style = getComputedStyle(element);
    return { background: style.backgroundColor, color: style.color };
  });
  expect(selectStyle.background).toBe("rgb(21, 27, 24)");
  expect(selectStyle.color).toBe("rgb(238, 246, 241)");
  const qiControlHeights = await page.locator("#itemTitle, #itemType, #profileSelect, #generatePassword, #openUrlButton").evaluateAll((controls) => controls.map((control) => control.getBoundingClientRect().height));
  expect(new Set(qiControlHeights).size).toBe(1);
  expect(qiControlHeights[0]).toBe(38);
  await expect(page.locator(".credential-grid")).toHaveCSS("margin-top", "12px");
  const qiIconBounds = await page.locator(".qi-icon-row").evaluate((container) => {
    const outer = container.getBoundingClientRect();
    const actions = container.querySelector(".button-row").getBoundingClientRect();
    return { outerRight: outer.right, actionsRight: actions.right, viewport: window.innerWidth };
  });
  expect(qiIconBounds.actionsRight).toBeLessThanOrEqual(qiIconBounds.outerRight + 1);
  expect(qiIconBounds.outerRight).toBeLessThanOrEqual(qiIconBounds.viewport);

  await page.getByRole("button", { name: "Open navigation menu" }).click();
  await page.getByRole("menuitem", { name: /Password Profiles/ }).click();
  const profileRangeBounds = await page.locator(".range-table").evaluate((container) => {
    const outer = container.getBoundingClientRect();
    const inputs = [...container.querySelectorAll("input")].map((input) => input.getBoundingClientRect().right);
    return { outerRight: outer.right, rightmostInput: Math.max(...inputs), viewport: window.innerWidth };
  });
  expect(profileRangeBounds.rightmostInput).toBeLessThanOrEqual(profileRangeBounds.outerRight + 1);
  expect(profileRangeBounds.outerRight).toBeLessThanOrEqual(profileRangeBounds.viewport);

  await page.getByRole("button", { name: "Open navigation menu" }).click();
  await page.getByRole("menuitem", { name: /Settings/ }).click();
  const settingsControlHeights = await page.locator("#autoLockMinutes, #themeSelect").evaluateAll((controls) => controls.map((control) => control.getBoundingClientRect().height));
  expect(new Set(settingsControlHeights).size).toBe(1);
  await expect(page.locator("body")).toHaveJSProperty("scrollHeight", 600);
  const vaultBounds = await page.evaluate(() => ({ viewport: window.innerWidth, document: document.documentElement.scrollWidth, body: document.body.scrollWidth }));
  expect(vaultBounds.document).toBeLessThanOrEqual(vaultBounds.viewport);
  expect(vaultBounds.body).toBeLessThanOrEqual(vaultBounds.viewport);
  await expect(page.getByRole("button", { name: "Open navigation menu" })).toBeVisible();

  await page.getByRole("button", { name: "Open navigation menu" }).click();
  await page.getByRole("menuitem", { name: /^Lock/ }).click();
  const unlockBounds = await page.evaluate(() => ({ viewport: window.innerWidth, document: document.documentElement.scrollWidth, body: document.body.scrollWidth }));
  expect(unlockBounds.document).toBeLessThanOrEqual(unlockBounds.viewport);
  expect(unlockBounds.body).toBeLessThanOrEqual(unlockBounds.viewport);
  const authAlignment = await page.evaluate(() => {
    const mark = document.querySelector(".auth-mark").getBoundingClientRect();
    const title = document.querySelector("#authBrandTitle").getBoundingClientRect();
    const visibleAuth = [...document.querySelectorAll("#unlockScreen .auth-tabs, #masterUnlockPanel")].map((node) => node.getBoundingClientRect());
    return {
      logoBeforeTitle: mark.right < title.left,
      brandCenter: (Math.min(mark.top, title.top) + Math.max(mark.bottom, title.bottom)) / 2,
      formCenter: (Math.min(...visibleAuth.map((rect) => rect.top)) + Math.max(...visibleAuth.map((rect) => rect.bottom))) / 2,
      viewportCenter: window.innerHeight / 2
    };
  });
  expect(authAlignment.logoBeforeTitle).toBe(true);
  expect(Math.abs(authAlignment.brandCenter - authAlignment.viewportCenter)).toBeLessThan(2);
  expect(Math.abs(authAlignment.formCenter - authAlignment.viewportCenter)).toBeLessThan(12);
});

test("credential, TOTP, and security-question controls reflow at minimum width", async ({ page }) => {
  await page.setViewportSize({ width: 800, height: 600 });
  await page.getByRole("button", { name: "Expand all categories" }).click();
  await page.getByRole("button", { name: "Admin", exact: true }).click();

  const credentialPositions = await page.locator("#itemUsername, #itemPassword").evaluateAll(([username, password]) => {
    const usernameField = username.closest(".field").getBoundingClientRect();
    const passwordField = password.closest(".field").getBoundingClientRect();
    return { usernameBottom: usernameField.bottom, passwordTop: passwordField.top };
  });
  expect(credentialPositions.passwordTop).toBeGreaterThanOrEqual(credentialPositions.usernameBottom);

  await page.getByRole("button", { name: "Show current code" }).click();
  await expect(page.locator("#totpCode")).toHaveText("123456");
  await page.locator("#itemTotpSecret").fill("JBSWY3DPEHPK3PXP");
  await page.getByRole("button", { name: "Show TOTP secret" }).click();
  await expect(page.locator("#itemTotpSecret")).toHaveAttribute("type", "text");
  await page.getByRole("button", { name: "Hide TOTP secret" }).click();
  await expect(page.locator("#itemTotpSecret")).toHaveAttribute("type", "password");

  const totpPositions = await page.locator(".totp-stack").evaluate((stack) => {
    const secret = stack.querySelector(".field").getBoundingClientRect();
    const display = stack.querySelector(".totp-display").getBoundingClientRect();
    const code = stack.querySelector("#totpCode").getBoundingClientRect();
    const button = stack.querySelector("#refreshTotp").getBoundingClientRect();
    return {
      displayBelowSecret: display.top >= secret.bottom,
      buttonBelowCode: button.top >= code.bottom,
      codeWhiteSpace: getComputedStyle(stack.querySelector("#totpCode")).whiteSpace,
      codeHeight: code.height
    };
  });
  expect(totpPositions.displayBelowSecret).toBe(true);
  expect(totpPositions.buttonBelowCode).toBe(true);
  expect(totpPositions.codeWhiteSpace).toBe("nowrap");
  expect(totpPositions.codeHeight).toBeLessThan(32);

  await page.getByRole("button", { name: "Add question" }).click();
  const answerInput = page.locator(".question-row .answer-input");
  await answerInput.fill("North Harbor");
  await expect(answerInput).toHaveAttribute("type", "password");
  await page.getByRole("button", { name: "Show security answer" }).click();
  await expect(answerInput).toHaveAttribute("type", "text");
  await expect(page.getByRole("button", { name: "Hide security answer" })).toHaveAttribute("aria-pressed", "true");
  const questionPositions = await page.locator(".question-row").evaluate((row) => {
    const question = row.querySelector(".question-input").getBoundingClientRect();
    const answer = row.querySelector(".answer-input").getBoundingClientRect();
    const answerControl = row.querySelector(".question-answer-control").getBoundingClientRect();
    const toggle = row.querySelector(".toggle-question").getBoundingClientRect();
    const copy = row.querySelector(".copy-question").getBoundingClientRect();
    const remove = row.querySelector(".remove-question").getBoundingClientRect();
    const bounds = row.getBoundingClientRect();
    return {
      answerBelowQuestion: answer.top >= question.bottom,
      actionsShareRow: Math.abs(answerControl.top - copy.top) < 1 && Math.abs(copy.top - remove.top) < 1,
      toggleInsideAnswer: toggle.left >= answer.left && toggle.right <= answer.right && Math.abs((toggle.top + toggle.bottom) / 2 - (answer.top + answer.bottom) / 2) < 1,
      removeInside: remove.right <= bounds.right + 1
    };
  });
  expect(questionPositions.answerBelowQuestion).toBe(true);
  expect(questionPositions.actionsShareRow).toBe(true);
  expect(questionPositions.toggleInsideAnswer).toBe(true);
  expect(questionPositions.removeInside).toBe(true);

  await page.getByRole("button", { name: "Add field" }).click();
  const customFieldBounds = await page.locator(".custom-field-row").evaluate((row) => {
    const value = row.querySelector(".custom-field-value").getBoundingClientRect();
    const secret = row.querySelector(".custom-field-secret").getBoundingClientRect();
    const remove = row.querySelector(".custom-field-remove").getBoundingClientRect();
    const bounds = row.getBoundingClientRect();
    return {
      valueBelowLabel: value.top >= row.querySelector(".custom-field-label").getBoundingClientRect().bottom,
      actionsShareRow: Math.abs(value.top - secret.top) < 1,
      secretAlignment: getComputedStyle(row.querySelector(".custom-field-secret")).alignItems,
      removeInside: remove.right <= bounds.right + 1
    };
  });
  expect(customFieldBounds.valueBelowLabel).toBe(true);
  expect(customFieldBounds.actionsShareRow).toBe(true);
  expect(customFieldBounds.secretAlignment).toBe("center");
  expect(customFieldBounds.removeInside).toBe(true);
});

test("secure notes hide the website icon action", async ({ page }) => {
  await page.getByRole("button", { name: "Expand all categories" }).click();
  await page.getByRole("button", { name: "Passport", exact: true }).click();
  await expect(page.getByRole("button", { name: "From website" })).toBeHidden();
  await page.getByRole("button", { name: "Admin", exact: true }).click();
  await expect(page.getByRole("button", { name: "From website" })).toBeVisible();
});

test("password strength updates live and custom fields round-trip through the Qi editor", async ({ page }) => {
  await page.getByRole("button", { name: "Expand all categories" }).click();
  await page.getByRole("button", { name: "Admin", exact: true }).click();

  const password = page.getByLabel("Password", { exact: true });
  await password.fill("short");
  await expect(page.locator("#passwordStrengthLabel")).toHaveText("Weak");
  await expect(page.locator("#passwordStrengthMeter")).toHaveAttribute("aria-valuenow", "1");
  await password.fill("Longer-Password-123!secure");
  await expect(page.locator("#passwordStrengthLabel")).toHaveText("Very strong");
  await expect(page.locator("#passwordStrengthMeter")).toHaveAttribute("aria-valuetext", "Very strong");

  await page.getByRole("button", { name: "Add field" }).click();
  const row = page.locator(".custom-field-row");
  await row.getByLabel("Label").fill("Door PIN");
  await row.getByLabel("Value", { exact: true }).fill("7391");
  await row.getByLabel("Keep value secret").check();
  await expect(row.getByLabel("Value", { exact: true })).toHaveAttribute("type", "password");
  await row.getByRole("button", { name: "Show secret custom field value" }).click();
  await expect(row.getByLabel("Value", { exact: true })).toHaveAttribute("type", "text");
  await expect(row.getByLabel("Keep value secret")).toBeChecked();
  await row.getByRole("button", { name: "Hide secret custom field value" }).click();
  await expect(row.getByLabel("Value", { exact: true })).toHaveAttribute("type", "password");
  await expect(row.getByLabel("Keep value secret")).toBeChecked();
  await page.getByRole("button", { name: "Save Qi" }).click();
  await expect(page.locator("#dirtyIndicator")).toBeHidden();

  const saved = await page.evaluate(() => window.__lastItemPatch);
  expect(saved.custom_fields).toEqual([{ label: "Door PIN", value: "7391", concealed: true }]);
  await expect(page.locator(".custom-field-row")).toHaveCount(1);
  await expect(page.locator(".custom-field-row").getByLabel("Label")).toHaveValue("Door PIN");
  await expect(page.locator(".custom-field-row").getByLabel("Value", { exact: true })).toHaveAttribute("type", "password");
  await expect(page.locator(".custom-field-row").getByRole("button", { name: "Show secret custom field value" })).toBeVisible();
});

test("new Qi entries default to the Strong 20 password profile", async ({ page }) => {
  await page.locator("#profileSelect").evaluate((select) => {
    select.value = [...select.options].find((option) => option.textContent.startsWith("Alphanumeric 20"))?.value || "";
  });
  await page.getByRole("button", { name: "New Qi" }).click();
  await expect(page.locator("#profileSelect option:checked")).toContainText("Strong 20");
});

test("TOTP actions sit beside the code and the generator stays intrinsic at wider widths", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.getByRole("button", { name: "Expand all categories" }).click();
  await page.getByRole("button", { name: "Admin", exact: true }).click();
  await page.getByRole("button", { name: "Show current code" }).click();

  const layout = await page.locator("#itemForm").evaluate((editor) => {
    const code = editor.querySelector("#totpCode").getBoundingClientRect();
    const totpButton = editor.querySelector("#refreshTotp").getBoundingClientRect();
    const note = editor.querySelector("#totpRemaining").getBoundingClientRect();
    const generator = editor.querySelector(".generator-strip").getBoundingClientRect();
    const profile = editor.querySelector("#profileSelect").getBoundingClientRect();
    const generate = editor.querySelector("#generatePassword").getBoundingClientRect();
    return {
      buttonBesideCode: totpButton.left >= code.right,
      noteBesideCode: note.left >= code.right,
      profileEndsBeforeButton: profile.right <= generate.left,
      generateWidth: generate.width,
      generatorWidth: generator.width,
      generateCssWidth: getComputedStyle(editor.querySelector("#generatePassword")).width
    };
  });
  expect(layout.buttonBesideCode).toBe(true);
  expect(layout.noteBesideCode).toBe(true);
  expect(layout.profileEndsBeforeButton).toBe(true);
  expect(layout.generateWidth).toBeLessThan(layout.generatorWidth / 2);
  expect(Number.parseFloat(layout.generateCssWidth)).toBeCloseTo(layout.generateWidth, 0);
});

test("Ring search and tag filtering discover tagged Qi", async ({ page }) => {
  const tagFilter = page.getByLabel("Filter Ring by tag");
  await expect(tagFilter.locator("option")).toHaveText(["All tags", "critical", "finance", "identity", "work"]);
  await tagFilter.selectOption("finance");
  await expect(page.getByRole("button", { name: "Billing", exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: "Admin", exact: true })).toHaveCount(0);

  await page.getByRole("button", { name: "Clear" }).click();
  await page.getByLabel("Search Qi").fill("identity");
  await expect(page.getByRole("button", { name: "Passport", exact: true })).toBeVisible();
  await expect(page.locator("#itemCount")).toHaveText("1");
});

test("health issue actions keep readable labels at minimum width", async ({ page }) => {
  await page.setViewportSize({ width: 800, height: 600 });
  await page.evaluate(() => { window.__healthWithIssue = true; });
  await page.getByRole("button", { name: "Open navigation menu" }).click();
  await page.getByRole("menuitem", { name: /Ring Health/ }).click();
  await expect(page.locator("#healthAnalyzed")).toHaveText("3");
  const widths = await page.locator("#runHealth, #healthIssues .issue-row button").evaluateAll((buttons) => buttons.map((button) => ({
    width: button.getBoundingClientRect().width,
    whiteSpace: getComputedStyle(button).whiteSpace
  })));
  expect(widths).toEqual([
    expect.objectContaining({ whiteSpace: "nowrap" }),
    expect.objectContaining({ whiteSpace: "nowrap" })
  ]);
  expect(widths[0].width).toBeGreaterThanOrEqual(142);
  expect(widths[1].width).toBeGreaterThanOrEqual(96);
  const bounds = await page.locator("#healthIssues").evaluate((list) => {
    const outer = list.getBoundingClientRect();
    const row = list.querySelector(".issue-row").getBoundingClientRect();
    return { outerRight: outer.right, rowRight: row.right, viewport: window.innerWidth };
  });
  expect(bounds.rowRight).toBeLessThanOrEqual(bounds.outerRight + 1);
  expect(bounds.rowRight).toBeLessThanOrEqual(bounds.viewport);
});

test("destructive Qi and profile actions use explicit in-app confirmation", async ({ page }) => {
  await page.getByRole("button", { name: "Expand all categories" }).click();
  await page.getByRole("button", { name: "Admin", exact: true }).click();
  await page.getByRole("button", { name: "Delete Qi" }).click();
  const qiDialog = page.getByRole("dialog", { name: "Delete Qi?" });
  await expect(qiDialog).toContainText("Delete “Admin” from the Ring?");
  await qiDialog.getByRole("button", { name: "Cancel" }).click();
  await expect(page.getByRole("button", { name: "Admin", exact: true })).toBeVisible();

  await page.getByRole("button", { name: "Delete Qi" }).click();
  await qiDialog.getByRole("button", { name: "Delete Qi" }).click();
  await expect(page.getByRole("button", { name: "Admin", exact: true })).toHaveCount(0);

  await page.getByRole("button", { name: "Open navigation menu" }).click();
  await page.getByRole("menuitem", { name: /Password Profiles/ }).click();
  await page.getByRole("button", { name: "New Profile" }).click();
  await page.getByLabel("Profile name").fill("Disposable Profile");
  await page.getByRole("button", { name: "Save Profile" }).click();
  await page.getByRole("button", { name: "Delete Profile" }).click();
  const profileDialog = page.getByRole("dialog", { name: "Delete password profile?" });
  await expect(profileDialog).toContainText("Delete “Disposable Profile”?");
  await profileDialog.getByRole("button", { name: "Delete Profile" }).click();
  await expect(page.locator("#profileList")).not.toContainText("Disposable Profile");
});

test("accent headings establish hierarchy without redundant section codes", async ({ page }) => {
  const accent = "rgb(157, 247, 199)";
  await expect(page.locator("#viewTitle")).toHaveCSS("color", accent);
  await expect(page.locator("#viewTitleIcon[data-icon='ring'] svg")).toBeVisible();
  await expect(page.getByRole("heading", { name: "Ring", exact: true })).toHaveCSS("color", accent);
  await page.getByRole("button", { name: "Expand all categories" }).click();
  await page.getByRole("button", { name: "Admin", exact: true }).click();
  await expect(page.locator("#itemEditorTitle")).toHaveCSS("color", accent);

  const modules = [
    ["Password Profiles", "#profilesView", "Password Profiles", "profile"],
    ["Ring Health", "#healthView", "Ring Health", "shield"],
    ["Backups", "#backupsView", "Backups & Transfer", "backup"],
    ["Settings", "#settingsView", "Settings", "settings"],
    ["Help", "#helpView", "Help", "question"]
  ];
  for (const [menuName, view, pageTitle, icon] of modules) {
    await page.getByRole("button", { name: "Open navigation menu" }).click();
    await page.getByRole("menuitem", { name: new RegExp(menuName) }).click();
    await expect(page.locator(view)).toBeVisible();
    await expect(page.locator("#viewTitle")).toHaveText(pageTitle);
    await expect(page.locator("#viewTitle")).toHaveCSS("color", accent);
    await expect(page.locator(`#viewTitleIcon[data-icon='${icon}'] svg`)).toBeVisible();
  }
  await expect(page.locator("#healthHeading, #backupsHeading, #settingsHeading, #helpHeading, .module-heading")).toHaveCount(0);
});

test("Help documents every module, setting, and keyboard route", async ({ page }) => {
  await page.getByRole("button", { name: "Open navigation menu" }).click();
  await page.getByRole("menuitem", { name: /^Help/ }).click();
  await expect(page.locator("#viewTitle")).toHaveText("Help");
  for (const heading of ["Getting started", "Ring storage & portable mode", "Create, unlock & recovery", "Ring & Qi editor", "Password profiles", "Ring health", "Backups & CSV transfer", "Settings", "Navigation & shortcuts"]) {
    await expect(page.getByRole("heading", { name: heading, exact: true })).toBeVisible();
  }
  await page.setViewportSize({ width: 700, height: 700 });
  const wrappedStorageTopic = page.getByRole("button", { name: "Ring storage & portable mode" });
  const readStorageTopicBounds = () => wrappedStorageTopic.evaluate((button) => ({
    clientHeight: button.clientHeight,
    scrollHeight: button.scrollHeight,
    renderedHeight: button.getBoundingClientRect().height
  }));
  await expect.poll(async () => (await readStorageTopicBounds()).renderedHeight).toBeGreaterThan(38);
  let storageTopicBounds = await readStorageTopicBounds();
  expect(storageTopicBounds.scrollHeight).toBeLessThanOrEqual(storageTopicBounds.clientHeight);

  await page.setViewportSize({ width: 1280, height: 700 });
  await expect.poll(async () => (await readStorageTopicBounds()).renderedHeight).toBe(38);

  await page.setViewportSize({ width: 700, height: 700 });
  await expect.poll(async () => (await readStorageTopicBounds()).renderedHeight).toBeGreaterThan(38);
  storageTopicBounds = await readStorageTopicBounds();
  expect(storageTopicBounds.scrollHeight).toBeLessThanOrEqual(storageTopicBounds.clientHeight);
  for (const setting of ["Auto-lock minutes", "Clipboard clear seconds", "Theme", "Button display", "Backup directory", "Rotate master password", "Replace recovery key"]) {
    await expect(page.locator("#helpView dt", { hasText: setting })).toBeVisible();
  }
  const accessibility = await new AxeBuilder({ page }).analyze();
  expect(accessibility.violations).toEqual([]);
  await page.keyboard.press("Control+1");
  await expect(page.getByRole("heading", { name: "Ring", exact: true })).toBeVisible();
  await page.keyboard.press("Control+6");
  await expect(page.locator("#helpView")).toBeVisible();
  await expect(page.locator("#viewTitle")).toHaveText("Help");

  await page.getByRole("button", { name: "Password profiles" }).click();
  await expect(page.locator("#help-profiles")).toBeInViewport();
  await expect(page.locator(".help-nav-link.active")).toHaveText("Password profiles");

  await page.locator("#helpSearch").fill("clipboard");
  await expect(page.locator(".help-nav-link:visible")).toHaveCount(3);
  await expect(page.getByRole("button", { name: "Create, unlock & recovery" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Password profiles" })).toBeHidden();
  await expect(page.locator("#help-vault h4", { hasText: "Ring (left pane)" })).toBeHidden();
  await expect(page.locator("#help-vault dt", { hasText: "Password" }).first()).toBeVisible();

  await page.locator("#clearHelpSearch").click();
  await expect(page.locator("#helpSearch")).toHaveValue("");
  await expect(page.locator(".help-nav-link:visible")).toHaveCount(9);
});

test("ring categories expose collapsible groups with live counters", async ({ page }) => {
  const work = page.locator(".category-group").filter({ has: page.getByText("Work", { exact: true }) });
  const personal = page.locator(".category-group").filter({ has: page.getByText("Personal", { exact: true }) });
  const workDisclosure = work.locator(".category-disclosure");
  const personalDisclosure = personal.locator(".category-disclosure");

  await expect(work.locator(".category-count")).toHaveText("2");
  await expect(personal.locator(".category-count")).toHaveText("1");
  await expect(workDisclosure).not.toHaveAttribute("open", "");
  await expect(work.getByRole("button", { name: /Admin/ })).toBeHidden();
  await expect(page.getByRole("button", { name: "Collapse all categories" })).toBeDisabled();
  const chevron = await work.locator(".category-chevron").evaluate((element) => {
    const bounds = element.getBoundingClientRect();
    const [originX, originY] = getComputedStyle(element).transformOrigin.split(" ").map(Number.parseFloat);
    return { width: bounds.width, height: bounds.height, originX, originY };
  });
  expect(chevron).toEqual({ width: 12, height: 12, originX: 6, originY: 6 });
  await page.getByRole("button", { name: "Expand all categories" }).click();
  await expect(work.getByRole("button", { name: "Admin", exact: true }).locator("strong")).toHaveText("Admin");
  await expect(work).not.toContainText("admin@example.com");
  await expect(workDisclosure).toHaveAttribute("open", "");
  await expect(personalDisclosure).toHaveAttribute("open", "");
  await page.getByRole("button", { name: "Collapse all categories" }).click();
  await expect(workDisclosure).not.toHaveAttribute("open", "");
  await work.locator("summary").click();
  await expect(work.getByRole("button", { name: "Admin", exact: true })).toBeVisible();
});

test("selecting Ring entries preserves every expanded category", async ({ page }) => {
  await page.getByRole("button", { name: "Expand all categories" }).click();
  const categories = page.locator("#itemList > .category-group");
  await expect(categories).toHaveCount(2);

  for (const name of ["Admin", "Passport", "Billing", "Admin"]) {
    await page.getByRole("button", { name, exact: true }).click();
    for (const category of await categories.all()) await expect(category.locator(".category-disclosure")).toHaveAttribute("open", "");
  }
});

test("switching Ring entries scrolls the Qi editor to the top", async ({ page }) => {
  await page.setViewportSize({ width: 800, height: 600 });
  await page.getByRole("button", { name: "Expand all categories" }).click();
  await page.getByRole("button", { name: "Admin", exact: true }).click();
  const editor = page.locator("#itemForm");
  await editor.evaluate((form) => { form.scrollTop = form.scrollHeight; });
  await expect.poll(() => editor.evaluate((form) => form.scrollTop)).toBeGreaterThan(0);

  await page.getByRole("button", { name: "Billing", exact: true }).click();
  await expect(editor).toHaveJSProperty("scrollTop", 0);
});

test("Ring sorting cycles through alphabetic modes and preserves draggable custom order", async ({ page }) => {
  const categoryNames = () => page.locator("#itemList > .category-group > .category-disclosure > .category-summary strong").allTextContents();
  const sortButton = page.locator("#ringSortMode");
  await expect(sortButton.locator(".button-label")).toHaveText("Custom");
  expect(await categoryNames()).toEqual(["Personal", "Work"]);

  await sortButton.click();
  await expect(sortButton.locator(".button-label")).toHaveText("A–Z");
  expect(await categoryNames()).toEqual(["Personal", "Work"]);
  await sortButton.click();
  await expect(sortButton.locator(".button-label")).toHaveText("Z–A");
  expect(await categoryNames()).toEqual(["Work", "Personal"]);
  await sortButton.click();
  await expect(sortButton.locator(".button-label")).toHaveText("Custom");

  const workGroup = page.locator("#itemList > .category-group").filter({ has: page.getByText("Work", { exact: true }) });
  const workHandle = page.locator('.drag-handle[data-order-kind="category"][data-order-id="Work"]');
  await expect(workHandle).toHaveAttribute("tabindex", "0");
  await workHandle.focus();
  await page.keyboard.press("Tab");
  await expect(workGroup.locator("summary")).toBeFocused();
  await workHandle.focus();
  await page.keyboard.press("Home");
  await expect.poll(categoryNames).toEqual(["Work", "Personal"]);
  await expect(workHandle).toBeFocused();

  await workGroup.locator("summary").click();
  const billingHandle = workGroup.locator('.drag-handle[data-order-kind="item"][data-order-id="bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb"]');
  const adminRow = workGroup.locator(".category-item-row").filter({ has: page.getByRole("button", { name: "Admin", exact: true }) });
  await billingHandle.dragTo(adminRow, { targetPosition: { x: 40, y: 2 } });
  await expect(workGroup.locator(".master-list-item strong").first()).toHaveText("Billing");

  await sortButton.click();
  await sortButton.click();
  await sortButton.click();
  expect(await categoryNames()).toEqual(["Work", "Personal"]);
  await expect(page.locator("#itemList > .category-group").first().locator(".master-list-item strong").first()).toHaveText("Billing");
});

test("switching Qi offers save, discard, and stay choices", async ({ page }) => {
  await page.getByRole("button", { name: "Expand all categories" }).click();
  await page.getByRole("button", { name: "Admin", exact: true }).click();
  await page.getByLabel("Name", { exact: true }).fill("Edited Admin");
  await page.getByRole("button", { name: "Billing", exact: true }).click();
  const prompt = page.getByRole("dialog", { name: "Save changes?" });
  await expect(prompt).toContainText("Save your changes before opening “Billing”?");
  await prompt.getByRole("button", { name: "Stay" }).click();
  await expect(page.getByLabel("Name", { exact: true })).toHaveValue("Edited Admin");

  await page.getByRole("button", { name: "Billing", exact: true }).click();
  await prompt.getByRole("button", { name: "Save & Continue" }).click();
  await expect(page.getByLabel("Name", { exact: true })).toHaveValue("Billing");
  await expect(page.getByRole("button", { name: "Edited Admin", exact: true })).toBeVisible();
});

test("list controls align and profile rows show unclipped names only", async ({ page }) => {
  await page.setViewportSize({ width: 800, height: 600 });
  await expect(page.locator("#qiringView > .master-pane")).toHaveCSS("width", "265px");
  const ringControls = await page.locator("#qiringView .ring-controls").evaluate((controls) => {
    const search = controls.querySelector(".ring-search-row").getBoundingClientRect();
    const tag = controls.querySelector(".tag-filter").getBoundingClientRect();
    const tools = controls.querySelector(".ring-pane-tools").getBoundingClientRect();
    const sortButton = controls.querySelector("#ringSortMode").getBoundingClientRect();
    const sortLabel = controls.querySelector("#ringSortMode .button-label").getBoundingClientRect();
    const bounds = controls.getBoundingClientRect();
    return {
      correctOrder: search.bottom <= tag.top && tag.bottom <= tools.top,
      toolsInside: tools.left >= bounds.left && tools.right <= bounds.right,
      sortLabelFits: sortLabel.height <= sortButton.height
    };
  });
  expect(ringControls.correctOrder).toBe(true);
  expect(ringControls.toolsInside).toBe(true);
  expect(ringControls.sortLabelFits).toBe(true);
  const ringHeights = await page.locator("#searchInput, #clearSearch").evaluateAll((controls) => controls.map((control) => control.getBoundingClientRect().height));
  expect(new Set(ringHeights).size).toBe(1);
  const counterHeights = await page.locator("#itemCount, #ringSortMode, #expandCategories, #collapseCategories").evaluateAll((controls) => controls.map((control) => control.getBoundingClientRect().height));
  expect(new Set(counterHeights).size).toBe(1);

  await page.getByRole("button", { name: "Open navigation menu" }).click();
  await page.getByRole("menuitem", { name: /Password Profiles/ }).click();
  const profileRow = page.locator("#profileList .master-list-item").first();
  await expect(profileRow).toHaveText("Strong 20");
  await expect(profileRow).not.toContainText("CHARACTERS");
});

test("light theme styles controls and survives a fresh locked startup", async ({ page }) => {
  await page.getByRole("button", { name: "Open navigation menu" }).click();
  await page.getByRole("menuitem", { name: /Settings/ }).click();
  await page.getByLabel("Theme").selectOption("light");
  const selectStyle = await page.getByLabel("Theme").evaluate((element) => {
    const style = getComputedStyle(element);
    return { background: style.backgroundColor, color: style.color };
  });
  expect(selectStyle.background).toBe("rgb(238, 244, 240)");
  expect(selectStyle.color).toBe("rgb(20, 32, 25)");

  await page.getByRole("button", { name: "Open navigation menu" }).click();
  await page.getByRole("menuitem", { name: /^Lock/ }).click();
  await expect(page.locator("#unsavedDialogMessage")).toHaveText("Save your changes before locking?");
  await page.getByRole("button", { name: "Save & Continue" }).click();
  await expect(page.locator("#unlockScreen")).toBeVisible();
  await expect(page.locator(".toast")).toHaveCount(0);
  await expect(page.locator(".auth-brand")).toHaveCSS("background-color", "rgb(248, 251, 249)");

  await page.reload();
  await expect(page.locator("#unlockScreen")).toBeVisible();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "light");
  await expect(page.locator(".auth-brand")).toHaveCSS("background-color", "rgb(248, 251, 249)");
});

test("recovery replacement is saved independently of the settings form", async ({ page }) => {
  await page.getByRole("button", { name: "Open navigation menu" }).click();
  await page.getByRole("menuitem", { name: /Settings/ }).click();

  const saveSettings = page.getByRole("button", { name: "Save Settings" });
  await expect(saveSettings).toBeDisabled();
  await expect(page.locator("#recoveryKeyStatus")).toContainText("saved immediately");

  await page.locator("#recoveryMasterPassword").fill("correct horse battery staple");
  await page.getByRole("button", { name: "Show recovery master password" }).click();
  await expect(page.locator("#recoveryMasterPassword")).toHaveAttribute("type", "text");
  await expect(saveSettings).toBeDisabled();
  await page.getByRole("button", { name: "Replace recovery key" }).click();
  await page.getByRole("button", { name: "Replace key", exact: true }).click();

  const dialog = page.getByRole("dialog", { name: "Store this key offline" });
  await expect(dialog).toBeVisible();
  await expect(page.locator("#recoveryMasterPassword")).toHaveAttribute("type", "password");
  await expect(page.locator("#recoveryKeyStatus")).toContainText("replaced and saved");
  await expect(page.locator("#recoveryActionStatus")).toContainText("already saved");
  await expect(page.locator(".toast")).toHaveCount(0);
  await page.getByLabel("Type the final six characters of the key").fill("123456");
  await page.getByLabel("I stored this recovery key somewhere safe.").check();
  await page.getByRole("button", { name: "Continue" }).click();

  await expect(dialog).toBeHidden();
  await expect(page.locator("#recoveryKeyStatus")).toHaveText("Recovery key replaced and saved. No settings save is required.");
  await expect(saveSettings).toBeDisabled();
});

test("master password rotation confirms, measures, reveals, and clears secret fields", async ({ page }) => {
  await page.getByRole("button", { name: "Open navigation menu" }).click();
  await page.getByRole("menuitem", { name: /Settings/ }).click();

  const nextPassword = "A very strong replacement 42!";
  await page.locator("#oldMasterPassword").fill("correct horse battery staple");
  await page.locator("#newMasterPassword").fill(nextPassword);
  await page.locator("#newMasterPasswordConfirm").fill(nextPassword);
  await expect(page.locator("#newMasterPasswordStrengthLabel")).toHaveText("Very strong");

  await page.getByRole("button", { name: "Show current master password" }).click();
  await expect(page.locator("#oldMasterPassword")).toHaveAttribute("type", "text");
  await page.locator("#toggleNewMasterPassword").click();
  await expect(page.locator("#newMasterPassword")).toHaveAttribute("type", "text");
  await expect(page.locator("#newMasterPasswordConfirm")).toHaveAttribute("type", "password");
  await page.getByRole("button", { name: "Show confirm new master password" }).click();
  await expect(page.locator("#newMasterPasswordConfirm")).toHaveAttribute("type", "text");
  await page.getByRole("button", { name: "Rotate master password" }).click();
  await page.getByRole("button", { name: "Rotate password", exact: true }).click();
  await expect.poll(() => page.evaluate(() => window.__rotatedMasterPassword)).toEqual({
    oldPassword: "correct horse battery staple",
    newPassword: nextPassword
  });
  await expect(page.locator("#newMasterPassword")).toHaveValue("");
  await expect(page.locator("#newMasterPasswordStrengthLabel")).toHaveText("No password");

  await page.locator("#oldMasterPassword").fill("temporary current secret");
  await page.locator("#newMasterPassword").fill("temporary replacement secret 42!");
  await page.locator("#newMasterPasswordConfirm").fill("temporary replacement secret 42!");
  await page.locator("#recoveryMasterPassword").fill("temporary recovery secret");
  await page.getByRole("button", { name: "Open navigation menu" }).click();
  await page.getByRole("menuitem", { name: /Backups/ }).click();
  await page.getByRole("button", { name: "Open navigation menu" }).click();
  await page.getByRole("menuitem", { name: /Settings/ }).click();
  for (const selector of ["#oldMasterPassword", "#newMasterPassword", "#newMasterPasswordConfirm", "#recoveryMasterPassword"]) {
    await expect(page.locator(selector)).toHaveValue("");
    await expect(page.locator(selector)).toHaveAttribute("type", "password");
  }
});

test("button display preference and encrypted Qi icons update the interface", async ({ page }) => {
  await page.getByRole("button", { name: "Expand all categories" }).click();
  await page.getByRole("button", { name: "Admin", exact: true }).click();
  await page.locator("#itemUrl").fill("https://example.com/account");
  await page.getByRole("button", { name: "From website" }).click();
  await expect(page.locator("#itemIconPreview")).toBeVisible();
  await page.getByRole("button", { name: "Save Qi" }).click();
  await expect(page.locator("#itemList img")).toHaveCount(1);
  const darkFaviconStyle = await page.locator("#itemList img").evaluate((image) => {
    const style = getComputedStyle(image);
    return { filter: style.filter, padding: style.padding };
  });
  expect(darkFaviconStyle.filter).not.toBe("none");
  expect(darkFaviconStyle.padding).toBe("2px");

  await page.getByRole("button", { name: "Open navigation menu" }).click();
  await page.getByRole("menuitem", { name: /Settings/ }).click();
  await page.getByLabel("Theme").selectOption("light");
  const lightFaviconFilter = await page.locator("#itemList img").evaluate((image) => getComputedStyle(image).filter);
  expect(lightFaviconFilter).not.toBe(darkFaviconStyle.filter);
  await page.emulateMedia({ colorScheme: "dark" });
  await page.getByLabel("Theme").selectOption("system");
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  const systemDarkFaviconFilter = await page.locator("#itemList img").evaluate((image) => getComputedStyle(image).filter);
  expect(systemDarkFaviconFilter).toBe(darkFaviconStyle.filter);
  await page.getByLabel("Button display").selectOption("icons");
  await expect(page.locator("html")).toHaveAttribute("data-button-display", "icons");
  await expect(page.locator("#chooseBackupDirectory .button-label")).toBeHidden();
  await expect(page.locator("#chooseBackupDirectory")).toHaveAttribute("aria-label", "Choose");
  await expect(page.locator("#chooseBackupDirectory")).toHaveAttribute("title", "Choose");
  await page.getByRole("button", { name: "Open navigation menu" }).click();
  await expect(page.locator("#appMenu .button-label").first()).toBeVisible();
  await expect(page.getByRole("menuitem", { name: /Settings/ }).locator(".button-label")).toHaveText("Settings");
});

test("standard workspace controls share a common height", async ({ page }) => {
  for (const menuName of ["Ring", "Password Profiles", "Ring Health", "Backups", "Settings"]) {
    if (menuName !== "Ring") {
      await page.getByRole("button", { name: "Open navigation menu" }).click();
      await page.getByRole("menuitem", { name: new RegExp(menuName) }).click();
    }
    const heights = await page.locator(".workspace input:not([type=checkbox]):not([type=radio]), .workspace select, .workspace button:not(.compact):not(.compact-button):not(.category-action):not(.sort-action):not(.secret-toggle)").evaluateAll((controls) => controls
      .filter((control) => control.getClientRects().length > 0)
      .map((control) => control.getBoundingClientRect().height));
    expect([...new Set(heights)]).toEqual([38]);
  }
});

test("backup and credential actions keep separation from their fields", async ({ page }) => {
  await page.getByRole("button", { name: "Open navigation menu" }).click();
  await page.getByRole("menuitem", { name: /Backups/ }).click();
  await expect(page.locator("#exportBackup")).toHaveCSS("margin-top", "12px");
  await page.locator("#backupPassphrase").fill("export backup secret");
  await page.getByRole("button", { name: "Show export backup passphrase" }).click();
  await expect(page.locator("#backupPassphrase")).toHaveAttribute("type", "text");
  await page.locator("#restorePassphrase").fill("restore backup secret");
  await page.getByRole("button", { name: "Show restore backup passphrase" }).click();
  await expect(page.locator("#restorePassphrase")).toHaveAttribute("type", "text");

  await page.getByRole("button", { name: "Open navigation menu" }).click();
  await page.getByRole("menuitem", { name: /Settings/ }).click();
  await expect(page.locator("#backupPassphrase")).toHaveAttribute("type", "password");
  await expect(page.locator("#restorePassphrase")).toHaveAttribute("type", "password");
  await expect(page.locator("#rotateMaster")).toHaveCSS("margin-top", "12px");
  await expect(page.locator("#regenerateRecovery")).toHaveCSS("margin-top", "12px");
});

test("toast countdown resets while hovered and resumes after pointer exit", async ({ page }) => {
  await expect(page.locator(".toast")).toHaveCount(0);
  await page.getByRole("button", { name: "Expand all categories" }).click();
  await page.getByRole("button", { name: "Admin", exact: true }).click();
  await page.getByRole("button", { name: "Generate password" }).click();
  const notification = page.locator(".toast").filter({ hasText: "Generated a 20-character password" });
  const progress = notification.locator(".toast-progress-fill");
  await expect(progress).toBeVisible();

  await notification.hover();
  await page.waitForTimeout(250);
  await expect(progress).toHaveCSS("transform", "matrix(1, 0, 0, 1, 0, 0)");

  await page.mouse.move(2, 2);
  await expect(notification).toBeHidden({ timeout: 6_000 });
});

test("authenticated workspace has no automatic accessibility violations", async ({ page }) => {
  const results = await new AxeBuilder({ page }).analyze();
  expect(results.violations).toEqual([]);
});

test("Settings navigation uses a gear icon", async ({ page }) => {
  await page.getByRole("button", { name: "Open navigation menu" }).click();
  const settingsIcon = page.getByRole("menuitem", { name: /Settings/ }).locator("svg path").first();
  await expect(settingsIcon).toHaveAttribute("d", /^M12\.22 2h-/);
  const ringIcon = page.getByRole("menuitem", { name: /^Ring/ }).locator("svg path").first();
  await expect(ringIcon).toHaveAttribute("d", /^M11 3a7 7/);
});

test("shortcut hints follow the current operating system", async ({ page }) => {
  const commandKey = await page.evaluate(() => {
    const platform = navigator.userAgentData?.platform || navigator.platform || navigator.userAgent;
    return /mac|iphone|ipad|ipod/i.test(platform);
  });
  const modifier = commandKey ? "⌘" : "Ctrl";

  await page.getByRole("button", { name: "Open navigation menu" }).click();
  await expect(page.getByRole("menuitem", { name: "Ring", exact: true }).locator("kbd")).toHaveText(commandKey ? "⌘1" : "Ctrl+1");
  await page.getByRole("menuitem", { name: /Help/ }).click();
  await expect(page.locator("#help-shortcuts [data-shortcut-modifier]")).toHaveText(modifier);
  await expect(page.locator('#help-shortcuts [data-shortcut="K"]')).toHaveText(commandKey ? "⌘K" : "Ctrl+K");
  await expect(page.locator('#help-shortcuts [data-shortcut="Shift+U"]')).toHaveText(commandKey ? "⌘⇧U" : "Ctrl+Shift+U");
});

test("all authenticated modules and keyboard navigation remain usable", async ({ page }) => {
  const modules = [
    ["Password Profiles", "#profilesView", "Password Profiles"],
    ["Ring Health", "#healthView", "Ring Health"],
    ["Backups", "#backupsView", "Backups & Transfer"],
    ["Settings", "#settingsView", "Settings"],
    ["Help", "#helpView", "Help"]
  ];
  for (const [menuName, viewSelector, pageTitle] of modules) {
    await page.getByRole("button", { name: "Open navigation menu" }).click();
    await page.getByRole("menuitem", { name: new RegExp(menuName) }).click();
    await expect(page.locator(viewSelector)).toBeVisible();
    await expect(page.locator("#viewTitle")).toHaveText(pageTitle);
  }

  await page.keyboard.press("Control+1");
  await expect(page.getByRole("heading", { name: "Ring", exact: true })).toBeVisible();
  await page.keyboard.press("Control+k");
  await expect(page.locator("#searchInput")).toBeFocused();

  await page.getByRole("button", { name: "Open navigation menu" }).click();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("button", { name: "Open navigation menu" })).toBeFocused();
});

test("unsaved navigation offers save, discard, and stay choices", async ({ page }) => {
  await page.locator("#itemTitle").fill("Unsaved credential");
  await page.getByRole("button", { name: "Open navigation menu" }).click();
  await page.getByRole("menuitem", { name: /Ring Health/ }).click();
  const prompt = page.getByRole("dialog", { name: "Save changes?" });
  await expect(prompt).toBeVisible();
  await expect(prompt).toContainText("Save your changes before opening Ring Health?");
  await prompt.getByRole("button", { name: "Stay" }).click();
  await expect(page.locator("#viewTitle")).toHaveText("Ring");

  await page.getByRole("button", { name: "Open navigation menu" }).click();
  await page.getByRole("menuitem", { name: /Ring Health/ }).click();
  await prompt.getByRole("button", { name: "Save & Continue" }).click();
  await expect(page.locator("#viewTitle")).toHaveText("Ring Health");
  await expect(page.locator("#healthIssues")).toContainText("No weak, reused, or old passwords");
  await expect(page.locator("#toastRegion")).toHaveAttribute("aria-live", "polite");
});
