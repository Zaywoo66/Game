import { expect, test } from '@playwright/test';

test.beforeEach(async ({ page }) => {
  await page.goto('/?verify=1');
  await expect(page.locator('main[data-map-status="ready"]')).toBeVisible({
    timeout: 60_000,
  });
  await expect(page.locator('[data-map-viewport] canvas')).toBeVisible();
  const renderer = await page.locator('main').getAttribute('data-map-renderer');
  expect(renderer).toBeTruthy();
  console.info('Map test WebGL renderer: ' + renderer);
});

test('projected provinces pick their canonical IDs and representative clicks select them', async ({ page }) => {
  const provinces = await loadProvinces(page);
  const selected = page.locator('[data-map-selected]');
  expect(provinces.length).toBeGreaterThan(0);
  expect(await page.evaluate(() => window.strategyMapTest.decodeProvinceId(0x12, 0x34, 0x56)))
    .toBe(0x123456);

  const picks = await page.evaluate((ids) => {
    const map = window.strategyMapTest;
    return ids.map((id) => {
      const point = map.projectProvince(id);
      return { id, point, picked: point ? map.pickAt(point.x, point.y) : null };
    });
  }, provinces.map(({ id }) => id));
  expect(picks.map(({ picked }) => picked)).toEqual(provinces.map(({ id }) => id));

  // Exercise the actual pointer handler once for each terrain kind.
  for (const kind of new Set(provinces.map((province) => province.kind))) {
    const index = provinces.findIndex((province) => province.kind === kind);
    const { id, point } = picks[index];
    await page.mouse.click(point.x, point.y);
    await expect(selected).toHaveAttribute('data-map-selected', String(id));
  }
});

test('owner recoloring changes the rendered province in place', async ({ page }) => {
  const land = (await loadProvinces(page)).filter(({ kind }) => kind === 'land');
  expect(land.length).toBeGreaterThanOrEqual(2);
  const [target, other] = land;
  const point = await projectProvince(page, target.id);
  const otherColor = [47, 201, 83, 255];
  const targetColor = [247, 11, 171, 255];

  await page.evaluate(({ id, color }) => {
    window.strategyMapTest.setOwnerColor(id, color);
    window.strategyMapTest.renderFrame();
  }, { id: other.id, color: otherColor });

  const clip = { x: Math.round(point.x), y: Math.round(point.y), width: 1, height: 1 };
  const before = await page.screenshot({ clip });
  const updated = await page.evaluate(({ id, color }) => {
    const updated = window.strategyMapTest.setOwnerColor(id, color);
    window.strategyMapTest.renderFrame();
    return updated;
  }, { id: target.id, color: targetColor });
  expect(updated.before).not.toEqual(targetColor);
  expect(updated.after).toEqual(targetColor);
  expect(updated.ownerTextureUnchanged).toBe(true);
  expect(updated.rasterUnchanged).toBe(true);

  const after = await page.screenshot({ clip });
  expect(after.equals(before)).toBe(false);

  const otherAfter = await page.evaluate(({ id, color }) => {
    return window.strategyMapTest.setOwnerColor(id, color).before;
  }, { id: other.id, color: otherColor });
  expect(otherAfter).toEqual(otherColor);
  expect(await page.evaluate(({ x, y }) => window.strategyMapTest.pickAt(x, y), point))
    .toBe(target.id);
});

test('camera presets stay bounded and wide margins render as ocean', async ({ page }) => {
  const result = await page.evaluate(() => {
    const map = window.strategyMapTest;
    const samples = {};
    for (const name of ['whole-region', 'britain', 'iberia', 'province']) {
      map.setCameraPreset(name);
      samples[name] = map.readCamera();
    }
    map.setCameraPreset('whole-region');
    const margin = map.readPixel(8, window.innerHeight / 2);
    return { samples, margin };
  });
  for (const camera of Object.values(result.samples)) {
    expect(camera.target[0]).toBeGreaterThanOrEqual(0);
    expect(camera.target[0]).toBeLessThanOrEqual(800);
    expect(camera.target[1]).toBeGreaterThanOrEqual(0);
    expect(camera.target[1]).toBeLessThanOrEqual(744);
    expect(camera.polarAngle).toBeLessThanOrEqual(0.72);
  }
  expect(result.samples['whole-region'].zoom).toBe(1);
  expect(result.samples.province.zoom).toBeGreaterThan(result.samples.britain.zoom);
  expect(result.margin[2]).toBeGreaterThan(result.margin[0] * 2);
  expect(result.margin).not.toEqual([21, 35, 44, 255]);
});

async function loadProvinces(page) {
  const response = await page.request.get('/map/western_europe/provinces.json');
  expect(response.ok()).toBe(true);
  return (await response.json()).provinces;
}

async function projectProvince(page, id) {
  const point = await page.evaluate((provinceId) => {
    return window.strategyMapTest.projectProvince(provinceId);
  }, id);
  expect(point, `Province ${id} must project into the initial viewport`).not.toBeNull();
  const viewport = page.viewportSize();
  expect(point.x).toBeGreaterThanOrEqual(0);
  expect(point.x).toBeLessThan(viewport.width);
  expect(point.y).toBeGreaterThanOrEqual(0);
  expect(point.y).toBeLessThan(viewport.height);
  return point;
}
