import { test, expect } from '@playwright/test';
import { mkdir } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';

test.skip(!process.env.CAPTURE_EUROPE, 'Run only for requested visual review');

test('full Europe 1080p reference map', async ({ page }) => {
  await page.setViewportSize({ width: 1920, height: 1080 });
  await page.goto('/?verify=1&map=europe');
  await expect(page.locator('[data-map-viewport] canvas')).toBeVisible();
  await expect(page.locator('.map-mode-note')).toContainText('Solid: named polygon');
  await page.waitForTimeout(1200);
  const output = fileURLToPath(new URL('../../docs/screenshots/europe-alpha/europe-1080p.png', import.meta.url));
  await mkdir(fileURLToPath(new URL('../../docs/screenshots/europe-alpha/', import.meta.url)), { recursive: true });
  await page.screenshot({ path: output, animations: 'disabled' });
});
