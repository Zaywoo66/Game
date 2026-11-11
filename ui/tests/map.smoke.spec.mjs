import { expect, test } from '@playwright/test';

test.beforeEach(async ({ page }) => {
  await page.goto('/?verify=1');
  await expect(page.locator('main[data-map-status="ready"]')).toBeVisible({
    timeout: 60_000,
  });
  await expect(page.locator('[data-map-viewport] canvas')).toBeVisible();
});

test('clicking each projected province selects its canonical ID', async ({ page }) => {
  const provinces = await loadProvinces(page);
  const selected = page.locator('[data-map-selected]');
  expect(provinces.length).toBeGreaterThan(0);
  expect(await page.evaluate(() => window.strategyMapTest.decodeProvinceId(0x12, 0x34, 0x56)))
    .toBe(0x123456);

  for (const { id } of provinces) {
    await test.step(`Province ${id}`, async () => {
      const point = await projectProvince(page, id);
      const picked = await page.evaluate(({ x, y }) => {
        return window.strategyMapTest.pickAt(x, y);
      }, point);
      expect(picked).toBe(id);
      await page.mouse.click(point.x, point.y);
      await expect(selected).toHaveAttribute('data-map-selected', String(id));
    });
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
  }, { id: other.id, color: otherColor });

  const clip = { x: Math.round(point.x), y: Math.round(point.y), width: 1, height: 1 };
  const before = await page.screenshot({ clip });
  const updated = await page.evaluate(({ id, color }) => {
    return window.strategyMapTest.setOwnerColor(id, color);
  }, { id: target.id, color: targetColor });
  expect(updated.before).not.toEqual(targetColor);
  expect(updated.after).toEqual(targetColor);
  expect(updated.ownerTextureUnchanged).toBe(true);
  expect(updated.rasterUnchanged).toBe(true);

  await expect.poll(async () => {
    const after = await page.screenshot({ clip });
    return after.equals(before);
  }).toBe(false);

  const otherAfter = await page.evaluate(({ id, color }) => {
    return window.strategyMapTest.setOwnerColor(id, color).before;
  }, { id: other.id, color: otherColor });
  expect(otherAfter).toEqual(otherColor);
  expect(await page.evaluate(({ x, y }) => window.strategyMapTest.pickAt(x, y), point))
    .toBe(target.id);
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
