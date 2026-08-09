import { defineConfig } from "@playwright/test";
import { existsSync } from "node:fs";

const systemChromium = "/usr/bin/chromium";

export default defineConfig({
  testDir: "./tests/e2e",
  timeout: 30_000,
  fullyParallel: true,
  forbidOnly: Boolean(process.env.CI),
  retries: process.env.CI ? 1 : 0,
  reporter: process.env.CI ? "github" : "list",
  use: {
    baseURL: "http://127.0.0.1:1421",
    viewport: { width: 1280, height: 800 },
    trace: "retain-on-failure",
    launchOptions: existsSync(systemChromium) ? { executablePath: systemChromium } : {}
  },
  webServer: {
    command: "npm run test:serve",
    url: "http://127.0.0.1:1421",
    reuseExistingServer: !process.env.CI,
    timeout: 30_000
  },
  projects: [{ name: "chromium", use: { browserName: "chromium" } }]
});
