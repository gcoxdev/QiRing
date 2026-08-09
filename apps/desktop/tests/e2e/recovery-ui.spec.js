import { test, expect } from "@playwright/test";
import AxeBuilder from "@axe-core/playwright";

const RECOVERY_KEY = "abcdefghijklmnopqrstuvwxyz123456";

test.beforeEach(async ({ page }) => {
  await page.addInitScript(({ recoveryKey }) => {
    const callbacks = new Map();
    let callbackId = 1;
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
      async invoke(command) {
        if (command === "plugin:event|listen") return callbackId++;
        if (command === "plugin:event|unlisten") return null;
        if (command === "vault_exists") return false;
        if (command === "create_vault") {
          return {
            summary: { vault_id: "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa", created_at: new Date().toISOString(), schema_version: 2 },
            recovery: { recovery_key: recoveryKey, recovery_key_fingerprint: recoveryKey.slice(0, 10) }
          };
        }
        if (command === "copy_secret") return 30;
        throw new Error(`Unhandled Tauri mock command: ${command}`);
      }
    };
  }, { recoveryKey: RECOVERY_KEY });
  await page.goto("/");
});

test("vault creation requires a verified recovery ceremony and clears the key", async ({ page }) => {
  await page.locator("#createMaster").fill("correct horse battery staple");
  await page.locator("#createConfirm").fill("correct horse battery staple");
  await page.getByRole("button", { name: "Initialize encrypted vault" }).click();

  const dialog = page.getByRole("dialog", { name: "Store this key offline" });
  await expect(dialog).toBeVisible();
  await expect(page.locator("#recoveryKeyOutput")).toHaveText(RECOVERY_KEY);
  await expect(page.getByRole("button", { name: "Continue" })).toBeDisabled();
  await page.getByLabel("Type the final six characters of the key").fill(RECOVERY_KEY.slice(-6));
  await page.getByLabel("I stored this recovery key somewhere safe.").check();
  await expect(page.getByRole("button", { name: "Continue" })).toBeEnabled();

  const accessibility = await new AxeBuilder({ page }).include("#recoveryDialog").analyze();
  expect(accessibility.violations).toEqual([]);

  await page.getByRole("button", { name: "Continue" }).click();
  await expect(page.locator("#unlockTitle")).toHaveText("Unlock QiRing");
  await expect(page.locator(".auth-brand")).toHaveText("QiRing");
  await expect(page.locator("#recoveryKeyOutput")).toHaveText("");
});
