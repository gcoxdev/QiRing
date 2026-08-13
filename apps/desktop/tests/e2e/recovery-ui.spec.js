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
      async invoke(command, args = {}) {
        if (command === "plugin:event|listen") return callbackId++;
        if (command === "plugin:event|unlisten") return null;
        if (command === "get_bootstrap_theme") return "system";
        if (command === "set_bootstrap_theme") return null;
        if (command === "vault_exists") return false;
        if (command === "prepare_recovery_print") {
          window.__recoveryPrintBasename = args.basename;
          return null;
        }
        if (command === "save_recovery_key_dialog") {
          window.__recoverySaveArgs = structuredClone(args);
          return `/tmp/${args.basename}.txt`;
        }
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

test("Ring creation requires a verified recovery ceremony and clears the key", async ({ page }) => {
  await expect(page.locator("#createScreen .section-code")).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Initialize encrypted Ring" })).toHaveAttribute("data-icon", "ring");
  await page.locator("#createMaster").fill("correct horse battery staple");
  await page.locator("#createConfirm").fill("correct horse battery staple");
  await page.getByRole("button", { name: "Initialize encrypted Ring" }).click();

  const dialog = page.getByRole("dialog", { name: "Store this key offline" });
  await expect(dialog).toBeVisible();
  await expect(dialog.locator(".section-code")).toHaveCount(0);
  await expect(page.locator("#recoveryKeyOutput")).toHaveText(RECOVERY_KEY);
  await expect(page.getByRole("button", { name: "Continue" })).toBeDisabled();
  await page.getByRole("button", { name: "Save private text file" }).click();
  const saveArgs = await page.evaluate(() => window.__recoverySaveArgs);
  expect(saveArgs.basename).toMatch(/^QiRing-Recovery-Key-\d{4}-\d{2}-\d{2}$/);
  expect(saveArgs.recoveryKey).toBe(RECOVERY_KEY);
  await expect(page.locator("#recoveryActionStatus")).toContainText(`${saveArgs.basename}.txt`);
  await page.getByRole("button", { name: "Show QR code" }).click();
  await expect(page.locator("#recoveryQrPanel")).toBeVisible();
  await expect(page.getByRole("button", { name: "Hide QR code" })).toHaveAttribute("aria-expanded", "true");
  const qrSize = await page.locator("#recoveryQr").evaluate((canvas) => ({ width: canvas.width, height: canvas.height }));
  expect(qrSize.width).toBeGreaterThan(100);
  expect(qrSize.height).toBeGreaterThan(100);
  await page.getByLabel("Type the final six characters of the key").fill(RECOVERY_KEY.slice(-6));
  await page.getByLabel("I stored this recovery key somewhere safe.").check();
  await expect(page.getByRole("button", { name: "Continue" })).toBeEnabled();

  const accessibility = await new AxeBuilder({ page }).include("#recoveryDialog").analyze();
  expect(accessibility.violations).toEqual([]);

  await page.getByRole("button", { name: "Continue" }).click();
  await expect(page.locator("#unlockTitle")).toHaveText("Unlock QiRing");
  await expect(page.locator(".auth-brand")).toHaveText("QiRing");
  await expect(page.locator("#recoveryKeyOutput")).toHaveText("");
  await expect(page.locator("#recoveryQrPanel")).toBeHidden();
  await expect(page.locator("#recoveryQr")).toHaveJSProperty("width", 1);
});

test("recovery printing uses a named white sheet containing the QR code and text key", async ({ page }) => {
  await page.locator("#createMaster").fill("correct horse battery staple");
  await page.locator("#createConfirm").fill("correct horse battery staple");
  await page.getByRole("button", { name: "Initialize encrypted Ring" }).click();
  await expect(page.getByRole("dialog", { name: "Store this key offline" })).toBeVisible();

  const originalTitle = await page.title();
  await page.evaluate(() => {
    window.print = () => {
      const dialog = document.querySelector("#recoveryDialog");
      const sheet = document.querySelector("#recoveryPrintSheet");
      window.__recoveryPrintSnapshot = {
        title: document.title,
        appDisplay: getComputedStyle(document.querySelector("#app")).display,
        dialogDisplay: getComputedStyle(dialog).display,
        dialogBackground: getComputedStyle(dialog).backgroundColor,
        formDisplay: getComputedStyle(dialog.querySelector("form")).display,
        sheetDisplay: getComputedStyle(sheet).display,
        key: document.querySelector("#recoveryPrintKey").textContent,
        fingerprint: document.querySelector("#recoveryPrintFingerprint").textContent,
        qrSource: document.querySelector("#recoveryPrintQr").getAttribute("src"),
        qrNaturalWidth: document.querySelector("#recoveryPrintQr").naturalWidth,
        qrRect: document.querySelector("#recoveryPrintQr").getBoundingClientRect().toJSON(),
        detailsRect: document.querySelector(".recovery-print-details").getBoundingClientRect().toJSON()
      };
    };
  });
  await page.emulateMedia({ media: "print" });
  await page.evaluate(() => document.querySelector("#printRecoveryKey").click());
  await expect.poll(() => page.evaluate(() => Boolean(window.__recoveryPrintSnapshot))).toBe(true);

  const snapshot = await page.evaluate(() => window.__recoveryPrintSnapshot);
  expect(snapshot.title).toMatch(/^QiRing-Recovery-Key-\d{4}-\d{2}-\d{2}$/);
  expect(snapshot.appDisplay).toBe("none");
  expect(snapshot.dialogDisplay).toBe("block");
  expect(snapshot.dialogBackground).toBe("rgb(255, 255, 255)");
  expect(snapshot.formDisplay).toBe("none");
  expect(snapshot.sheetDisplay).toBe("grid");
  expect(snapshot.key).toBe(RECOVERY_KEY);
  expect(snapshot.fingerprint).toBe(RECOVERY_KEY.slice(0, 10));
  expect(snapshot.qrSource).toMatch(/^data:image\/png;base64,/);
  expect(snapshot.qrNaturalWidth).toBe(384);
  expect(snapshot.qrRect.width).toBeGreaterThan(145);
  expect(snapshot.qrRect.width).toBeLessThan(160);
  expect(snapshot.qrRect.right).toBeLessThanOrEqual(snapshot.detailsRect.left);
  expect(await page.evaluate(() => window.__recoveryPrintBasename)).toMatch(/^QiRing-Recovery-Key-\d{4}-\d{2}-\d{2}$/);

  const pdf = await page.pdf({ format: "A4", printBackground: true });
  expect(pdf.byteLength).toBeGreaterThan(10_000);

  await page.evaluate(() => window.dispatchEvent(new Event("afterprint")));
  await expect(page).toHaveTitle(originalTitle);
  await expect(page.locator("#recoveryPrintKey")).toHaveText("");
  expect(await page.locator("#recoveryPrintQr").evaluate((image) => image.hasAttribute("src"))).toBe(false);
});
