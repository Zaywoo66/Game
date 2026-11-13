import { mkdir } from 'node:fs/promises';
import { join } from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';
import { expect, test } from '@playwright/test';

test.skip(!process.env.CAPTURE_MAP, 'manual visual-review capture');
test.use({ viewport: { width: 1920, height: 1080 }, deviceScaleFactor: 1 });

const output = fileURLToPath(new URL('../../docs/screenshots/m5b4/', import.meta.url));

test('capture four named 1080p camera views', async ({ page }) => {
  await page.goto('/?verify=1&map=western_europe');
  await expect(page.locator('main[data-map-status="ready"]')).toBeVisible({ timeout: 60_000 });
  await page.getByRole('button', { name: 'Game political' }).click();
  await mkdir(output, { recursive: true });
  for (const name of ['whole-region', 'britain', 'iberia', 'province']) {
    await page.evaluate((preset) => window.strategyMapTest.setCameraPreset(preset), name);
    await page.screenshot({ path: join(output, `${name}.png`), animations: 'disabled' });
  }
});
