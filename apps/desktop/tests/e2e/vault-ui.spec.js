import { test, expect } from "@playwright/test";
import AxeBuilder from "@axe-core/playwright";

async function installTauriMock(page) {
  await page.addInitScript(() => {
    const callbacks = new Map();
    let callbackId = 1;
    const profiles = [{
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
    }];
    const items = [
      { id: "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa", item_type: "login", title: "Admin", username: "admin@example.com", folder: "Work", tags: [], has_totp: true, updated_at: new Date().toISOString() },
      { id: "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb", item_type: "login", title: "Billing", username: "billing@example.com", folder: "Work", tags: [], has_totp: false, updated_at: new Date().toISOString() },
      { id: "cccccccc-cccc-4ccc-8ccc-cccccccccccc", item_type: "secure_note", title: "Passport", username: null, folder: "Personal", tags: [], has_totp: false, updated_at: new Date().toISOString() }
    ];
    const settings = {
      auto_lock_minutes: 5,
      clipboard_clear_seconds: 30,
      lock_on_window_blur: false,
      lock_on_minimize: true,
      biometric_enabled: false,
      theme: "dark",
      backup_preferences: {
        include_settings: true,
        automatic_enabled: false,
        directory: null,
        retention_count: 10
      }
    };

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
          case "vault_exists": return true;
          case "unlock_vault_master": return { session: { token: "session", unlocked_at: new Date().toISOString() }, migrated_recovery: null };
          case "get_security_status": return {
            schema_version: 2,
            command_version: "2",
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
            return structuredClone(query
              ? items.filter((item) => [item.title, item.username, item.folder].some((value) => value?.toLocaleLowerCase().includes(query)))
              : items);
          }
          case "get_settings": return structuredClone(settings);
          case "generate_password": return { value: "Aaaa1111!!!!MockPass".slice(0, args.policy.length) };
          case "save_profile": {
            const profile = structuredClone(args.profile);
            if (profile.id.startsWith("00000000")) profile.id = "22222222-2222-4222-8222-222222222222";
            const index = profiles.findIndex((entry) => entry.id === profile.id);
            if (index >= 0) profiles[index] = profile;
            else profiles.push(profile);
            return profile.id;
          }
          case "health_report": return { analyzed_items: 0, weak_count: 0, reused_count: 0, old_count: 0, issues: [] };
          case "list_snapshots": return [];
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
  await page.locator("#unlockMaster").fill("correct horse battery staple");
  await page.getByRole("button", { name: "Unlock vault" }).click();
  await expect(page.getByRole("heading", { name: "Stored Qi" })).toBeVisible();
});

test("context actions and profile master-detail editor follow the active module", async ({ page }) => {
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
  await page.getByRole("menuitem", { name: /Vault Health/ }).click();
  await expect(page.getByRole("button", { name: "New Profile" })).toBeHidden();
  await expect(page.getByRole("button", { name: "Save Profile" })).toBeHidden();
});

test("native selects remain legible and the 800 by 600 layout stays bounded", async ({ page }) => {
  await page.setViewportSize({ width: 800, height: 600 });
  const selectStyle = await page.locator("#profileSelect").evaluate((element) => {
    const style = getComputedStyle(element);
    return { background: style.backgroundColor, color: style.color };
  });
  expect(selectStyle.background).toBe("rgb(21, 27, 24)");
  expect(selectStyle.color).toBe("rgb(238, 246, 241)");
  const qiControlHeights = await page.locator("#itemTitle, #itemType, #profileSelect").evaluateAll((controls) => controls.map((control) => control.getBoundingClientRect().height));
  expect(new Set(qiControlHeights).size).toBe(1);

  await page.getByRole("button", { name: "Open navigation menu" }).click();
  await page.getByRole("menuitem", { name: /Settings/ }).click();
  const settingsControlHeights = await page.locator("#autoLockMinutes, #themeSelect").evaluateAll((controls) => controls.map((control) => control.getBoundingClientRect().height));
  expect(new Set(settingsControlHeights).size).toBe(1);
  await expect(page.locator("body")).toHaveJSProperty("scrollHeight", 600);
  await expect(page.getByRole("button", { name: "Open navigation menu" })).toBeVisible();
});

test("ring categories expose collapsible groups with live counters", async ({ page }) => {
  const work = page.locator("details.category-group").filter({ has: page.getByText("Work", { exact: true }) });
  const personal = page.locator("details.category-group").filter({ has: page.getByText("Personal", { exact: true }) });

  await expect(work.locator(".category-count")).toHaveText("2");
  await expect(personal.locator(".category-count")).toHaveText("1");
  await expect(work).toHaveAttribute("open", "");
  await work.locator("summary").click();
  await expect(work).not.toHaveAttribute("open", "");
  await expect(work.getByRole("button", { name: /Admin/ })).toBeHidden();
  await work.locator("summary").click();
  await expect(work.getByRole("button", { name: /Admin/ })).toBeVisible();
});

test("backup and credential actions keep separation from their fields", async ({ page }) => {
  await page.getByRole("button", { name: "Open navigation menu" }).click();
  await page.getByRole("menuitem", { name: /Backups/ }).click();
  await expect(page.locator("#exportBackup")).toHaveCSS("margin-top", "12px");

  await page.getByRole("button", { name: "Open navigation menu" }).click();
  await page.getByRole("menuitem", { name: /Settings/ }).click();
  await expect(page.locator("#rotateMaster")).toHaveCSS("margin-top", "12px");
  await expect(page.locator("#regenerateRecovery")).toHaveCSS("margin-top", "12px");
});

test("toast countdown resets while hovered and resumes after pointer exit", async ({ page }) => {
  const notification = page.locator(".toast").filter({ hasText: "Vault unlocked." });
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

test("all authenticated modules and keyboard navigation remain usable", async ({ page }) => {
  const modules = [
    ["Password Profiles", "#profilesHeading", "Profiles"],
    ["Vault Health", "#healthHeading", "Vault health"],
    ["Backups", "#backupsHeading", "Encrypted backups"],
    ["Settings", "#settingsHeading", "Settings"]
  ];
  for (const [menuName, headingSelector, heading] of modules) {
    await page.getByRole("button", { name: "Open navigation menu" }).click();
    await page.getByRole("menuitem", { name: new RegExp(menuName) }).click();
    await expect(page.locator(headingSelector)).toHaveText(heading);
  }

  await page.keyboard.press("Control+1");
  await expect(page.getByRole("heading", { name: "Stored Qi" })).toBeVisible();
  await page.keyboard.press("Control+k");
  await expect(page.locator("#searchInput")).toBeFocused();

  await page.getByRole("button", { name: "Open navigation menu" }).click();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("button", { name: "Open navigation menu" })).toBeFocused();
});

test("unsaved navigation is guarded and routine updates use the live toast region", async ({ page }) => {
  await page.locator("#itemTitle").fill("Unsaved credential");
  page.once("dialog", (dialog) => dialog.dismiss());
  await page.getByRole("button", { name: "Open navigation menu" }).click();
  await page.getByRole("menuitem", { name: /Vault Health/ }).click();
  await expect(page.getByRole("heading", { name: "Stored Qi" })).toBeVisible();

  page.once("dialog", (dialog) => dialog.accept());
  await page.getByRole("menuitem", { name: /Vault Health/ }).click();
  await page.getByRole("button", { name: "Run analysis" }).click();
  await expect(page.locator("#healthIssues")).toContainText("No weak, reused, or old passwords");
  await expect(page.locator("#toastRegion")).toHaveAttribute("aria-live", "polite");
});
