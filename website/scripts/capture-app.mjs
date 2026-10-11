import { spawn } from 'node:child_process';
import { existsSync } from 'node:fs';
import { mkdir } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { chromium, expect } from '@playwright/test';
import { installDemoBridge } from './demo-data.mjs';

const app = fileURLToPath(new URL('../../apps/desktop/', import.meta.url));
const output = fileURLToPath(new URL('../public/screenshots/', import.meta.url));
const origin = 'http://127.0.0.1:1432';
if (!existsSync(`${app}/node_modules/vite/bin/vite.js`)) throw new Error('Run npm ci in apps/desktop before capturing screenshots.');
const server = spawn(process.execPath, ['node_modules/vite/bin/vite.js', '--host', '127.0.0.1', '--port', '1432', '--strictPort'], { cwd: app, stdio: 'pipe' });
let serverLog = '';
server.stdout.on('data', data => { serverLog += data; });
server.stderr.on('data', data => { serverLog += data; });
let browser;
try {
  for (let attempt = 0; ; attempt++) {
    if (server.exitCode !== null) throw new Error(serverLog);
    try { if ((await fetch(origin)).ok) break; } catch {}
    if (attempt === 100) throw new Error(`App preview did not start: ${serverLog}`);
    await new Promise(resolve => setTimeout(resolve, 200));
  }
  browser = await chromium.launch({ executablePath: process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH || undefined });
  const context = await browser.newContext({ viewport: { width: 1280, height: 860 }, deviceScaleFactor: 1, reducedMotion: 'reduce', locale: 'en-US', colorScheme: 'dark' });
  // No external requests, no Tauri process, no filesystem access from the app.
  await context.route('**/*', route => new URL(route.request().url()).origin === origin ? route.continue() : route.abort());
  await context.addInitScript(installDemoBridge);
  const page = await context.newPage();
  page.setDefaultTimeout(15_000);
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  await mkdir(output, { recursive: true });
  await page.goto(origin);
  await expect(page.locator('#unlockTitle')).toBeVisible();
  await expect(page.locator('#unlockTitle')).toHaveText('Unlock QiRing');
  await page.evaluate(() => document.fonts.ready);
  await page.screenshot({ path: `${output}/unlock.png`, animations: 'disabled' });
  await page.locator('#unlockMaster').fill('Fictional-demo-master-42!');
  await page.locator('#masterUnlockPanel button[type=submit]').click();
  await expect(page.locator('#viewTitle')).toHaveText('Ring');
  await expect(page.locator('#unlockMaster')).toBeHidden();
  await page.getByRole('button', { name: 'Expand all categories' }).click();
  await page.getByRole('button', { name: 'Juniper Mail', exact: true }).click();
  await page.evaluate(() => document.fonts.ready);
  await page.screenshot({ path: `${output}/ring.png`, animations: 'disabled' });
  await page.getByRole('button', { name: 'Open navigation menu' }).click();
  await page.getByRole('menuitem', { name: /Password Profiles/ }).click();
  await expect(page.locator('#viewTitle')).toHaveText('Password Profiles');
  await page.screenshot({ path: `${output}/profiles.png`, animations: 'disabled' });
  await page.getByRole('button', { name: 'Open navigation menu' }).click();
  await page.getByRole('menuitem', { name: /Ring Health/ }).click();
  await expect(page.locator('#healthAnalyzed')).toHaveText('8');
  await page.screenshot({ path: `${output}/health.png`, animations: 'disabled' });
  const unexpected = await page.evaluate(() => window.__demoUnexpectedCommands);
  if (errors.length || unexpected.length) throw new Error(JSON.stringify({ errors, unexpected }));
  console.log('Captured four app screenshots using only fictional in-memory data.');
} finally {
  await browser?.close();
  server.kill('SIGTERM');
}
