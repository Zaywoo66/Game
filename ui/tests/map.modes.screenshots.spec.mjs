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
  const metadata = await (await page.request.get('/map/western_europe/provinces.json')).json();
  const london = metadata.provinces.find((province) => province.key === 'london');
  const neighbor = london.neighbors.map(({ id }) => metadata.provinces.find((province) => province.id === id))
    .find((province) => province?.kind === 'land');
  expect(neighbor).toBeTruthy();
  await page.getByRole('button', { name: 'Game political' }).click();
  await page.evaluate(({ londonId, neighborId }) => {
    const map = window.strategyMapTest;
    map.setCameraPreset('province');
    map.setOwnerColor(londonId, [188, 88, 72, 255]);
    map.setOwnerColor(neighborId, [188, 88, 72, 255]);
    map.setArmies([{ id: 1, province: londonId, destination: neighborId, progress: 250,
      color: '#bc5848', soldiers: 1200 }]);
    map.setArmySelection(1, false);
    map.renderFrame();
  }, { londonId: london.id, neighborId: neighbor.id });
  await expect(page.locator('.map-army-badge[data-army-id="1"]')).toBeVisible();
  await expect(page.locator('.map-army-path')).toBeVisible();
  await expect(page.locator('[data-map-viewport]')).toHaveAttribute('data-map-army-instances', '1', { timeout: 30_000 });
  await page.evaluate(() => window.strategyMapTest.renderFrame());
  await page.screenshot({ path: join(output, 'army-marker-demo.png'), animations: 'disabled' });
});
