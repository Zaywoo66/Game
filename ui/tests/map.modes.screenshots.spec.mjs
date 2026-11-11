import { mkdir } from 'node:fs/promises';
import { join } from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';
import { expect, test } from '@playwright/test';

test.skip(!process.env.CAPTURE_MAP_MODES, 'manual visual-review capture');
test.use({ viewport: { width: 1920, height: 1080 }, deviceScaleFactor: 1 });

const output = fileURLToPath(new URL('../../docs/screenshots/m5b5/', import.meta.url));

test('capture historical and terrain modes at named views', async ({ page }) => {
  await page.goto('/?verify=1');
  await expect(page.locator('main[data-map-status="ready"]')).toBeVisible({ timeout: 60_000 });
  await mkdir(output, { recursive: true });
  for (const [mode, preset, name] of [
    ['historical', 'whole-region', 'historical-whole'],
    ['historical', 'britain', 'historical-britain'],
    ['terrain', 'iberia', 'terrain-iberia'],
    ['historical', 'province', 'historical-province'],
  ]) {
    await page.getByRole('button', { name: mode === 'historical' ? 'Historical 1440' : 'Terrain', exact: true }).click();
    await expect.poll(() => page.evaluate(() => window.strategyMapTest.readMapMode())).toBe(mode);
    await page.evaluate((camera) => window.strategyMapTest.setCameraPreset(camera), preset);
    await page.screenshot({ path: join(output, `${name}.png`), animations: 'disabled' });
  }
});
