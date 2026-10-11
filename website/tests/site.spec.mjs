import { test, expect } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';
import topics from '../src/data/help.json' with { type: 'json' };
import release from '../src/data/release.json' with { type: 'json' };

const pages = ['/', '/features/', '/guide/', '/security/', '/downloads/', ...topics.map(topic => `/guide/${topic.slug}/`)];
for (const url of pages) {
  test(`${url} is readable and accessible`, async ({ page }) => {
    const errors = [];
    page.on('pageerror', error => errors.push(error.message));
    await page.goto(url);
    await page.evaluate(() => document.fonts.ready);
    await expect(page.locator('h1')).toBeVisible();
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    await page.locator('img').evaluateAll(images => Promise.all(images.map(image => { image.loading = 'eager'; return image.decode(); })));
    expect(await page.locator('img').evaluateAll(images => images.every(image => image.complete && image.naturalWidth > 0))).toBe(true);
    const results = await new AxeBuilder({ page }).withTags(['wcag2a', 'wcag2aa', 'wcag21aa']).analyze();
    expect(results.violations).toEqual([]);
    expect(errors).toEqual([]);
  });
}

test('guide search handles a match, no match, and clear', async ({ page }) => {
  await page.goto('/guide/');
  await page.getByLabel('Find a topic').fill('portable');
  await expect(page.locator('[data-topic]:visible')).not.toHaveCount(0);
  await expect(page.locator('[data-topic]:visible')).not.toHaveCount(9);
  await page.getByLabel('Find a topic').fill('nonexistenttopicxyz');
  await expect(page.locator('#no-results')).toBeVisible();
  await page.getByRole('button', { name: 'Clear', exact: true }).click();
  await expect(page.locator('[data-topic]:visible')).toHaveCount(9);
  await page.getByRole('link', { name: /Getting started/ }).click();
  await expect(page.locator('h1')).toHaveText('Getting started');
});

test('navigation reaches downloads and uses the actual release assets', async ({ page }, testInfo) => {
  await page.goto('/');
  if (testInfo.project.name === 'mobile') {
    await page.locator('.mobile-menu summary').click();
    await page.getByRole('navigation', { name: 'Mobile navigation' }).getByRole('link', { name: 'Downloads', exact: true }).click();
  } else {
    await page.locator('header').getByRole('link', { name: /Get QiRing/ }).click();
  }
  await expect(page).toHaveURL(/\/downloads\/$/);
  for (const asset of release.assets) await expect(page.locator(`a[href="${asset.url}"]`).first()).toBeVisible();
  await expect(page.getByText('prerelease', { exact: false }).first()).toBeVisible();
});

test('installation notes expand without overflowing the screen', async ({ page }) => {
  await page.goto('/downloads/');
  for (const details of await page.locator('.faq-list details').all()) {
    if (await details.getAttribute('open') === null) await details.locator('summary').click();
    await expect(details.locator('p').first()).toBeVisible();
  }
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  expect((await new AxeBuilder({ page }).withTags(['wcag2a', 'wcag2aa', 'wcag21aa']).analyze()).violations).toEqual([]);
});

test('keyboard skip link and missing-page recovery work', async ({ page }) => {
  await page.goto('/');
  await page.keyboard.press('Tab');
  await expect(page.getByRole('link', { name: 'Skip to content' })).toBeFocused();
  await page.keyboard.press('Enter');
  await expect(page).toHaveURL(/#main$/);
  const response = await page.goto('/missing-page/');
  expect(response.status()).toBe(404);
  await expect(page.locator('h1')).toBeVisible();
  await expect(page.getByRole('link', { name: 'Back to home', exact: true })).toBeVisible();
});

test('guide remains usable without JavaScript', async ({ browser, baseURL }) => {
  const context = await browser.newContext({ javaScriptEnabled: false });
  const page = await context.newPage();
  await page.goto(`${baseURL}/guide/`);
  await expect(page.locator('[data-topic]:visible')).toHaveCount(9);
  await expect(page.getByLabel('Find a topic')).toBeHidden();
  await context.close();
});
