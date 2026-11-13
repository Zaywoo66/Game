import { expect, test } from '@playwright/test';

test.beforeEach(async ({ page }) => {
  const webglErrors = [];
  page.on('console', (message) => {
    if (/WebGL.*(?:GL_INVALID_|Failed to compile shader|Shader Error)|GL_INVALID_(?:ENUM|OPERATION|VALUE)/i.test(message.text())) {
      webglErrors.push(message.text());
    }
  });
  await page.goto('/?verify=1&map=western_europe');
  await expect(page.locator('main[data-map-status="ready"]')).toBeVisible({
    timeout: 60_000,
  });
  await expect(page.locator('[data-map-viewport] canvas')).toBeVisible();
  const renderer = await page.locator('main').getAttribute('data-map-renderer');
  expect(renderer).toBeTruthy();
  console.info('Map test WebGL renderer: ' + renderer);
  await expect(page.locator('main')).toHaveAttribute('data-map-mode', 'historical');
  await expect(page.getByText(/placeholder colors do not (?:assert game|imply) ownership/i)).toBeVisible();
  expect(webglErrors, 'WebGL initialization must not report texture or shader errors').toEqual([]);
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

test('province hover shows a delayed name tooltip and selection remains available', async ({ page }) => {
  const provinces = await loadProvinces(page);
  const province = provinces.find(({ kind }) => kind === 'land');
  expect(province).toBeTruthy();
  const names = await (await page.request.get('/map/western_europe/names.en.json')).json();
  const point = await page.evaluate((id) => window.strategyMapTest.projectProvince(id), province.id);
  expect(point).toBeTruthy();
  await page.mouse.move(point.x, point.y);
  const tooltip = page.getByRole('tooltip');
  await expect(tooltip).toHaveText(names[province.name_key], { timeout: 2_000 });
  await page.mouse.click(point.x, point.y);
  await expect(page.locator('[data-map-selected]')).toHaveAttribute('data-map-selected', String(province.id));
});

test('fleet markers show ship counts, movement paths, and select the fleet panel', async ({ page }) => {
  const seaIds = (await loadProvinces(page)).filter((province) => province.kind === 'sea').slice(0, 2).map((province) => province.id);
  expect(seaIds).toHaveLength(2);
  await page.evaluate(([province, destination]) => {
    window.strategyMapTest.setFleets([{
      id: 77, province, destination, progress: 350, color: '#526f9b', ships: 4,
    }]);
    window.strategyMapTest.renderFrame();
  }, seaIds);
  const fleet = page.getByRole('button', { name: 'Fleet 77, 4 ships' });
  await expect(fleet).toBeVisible();
  await expect(page.locator('[data-map-viewport]')).toHaveAttribute('data-map-fleet-mesh-instances', '1');
  const fleetPosition = await page.evaluate(() => window.strategyMapTest.readFleetPosition(77));
  expect(fleetPosition).toHaveLength(3);
  console.info('Fleet model position:', fleetPosition);
  await fleet.click();
  await expect(page.locator('main')).toHaveAttribute('data-map-selected-fleet', '77');
  await expect(fleet).toHaveAttribute('data-selected', 'true');
  await expect(page.locator('.map-fleet-path')).not.toBeHidden();
});

test('country labels stay clear of map controls and panels', async ({ page }) => {
  await page.evaluate(() => window.strategyMapTest.setCameraPreset('whole-region'));
  const overlaps = await page.evaluate(() => {
    const panels = [...document.querySelectorAll('.map-panel, .event-popup')]
      .map((element) => element.getBoundingClientRect());
    return [...document.querySelectorAll('.map-label')].filter((label) => {
      const rect = label.getBoundingClientRect();
      return panels.some((panel) => rect.right > panel.left && rect.left < panel.right
        && rect.bottom > panel.top && rect.top < panel.bottom);
    }).map((label) => label.textContent);
  });
  expect(overlaps).toEqual([]);
});

test('owner recoloring changes the rendered province in place', async ({ page }) => {
  await page.getByRole('button', { name: 'Game political' }).click();
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

test('adjacent historical countries receive clearly separated atlas colors', async ({ page }) => {
  const [mapResponse, evidenceResponse] = await Promise.all([
    page.request.get('/map/western_europe/provinces.json'),
    page.request.get('/map/western_europe/polities_1440.json'),
  ]);
  expect(mapResponse.ok() && evidenceResponse.ok()).toBe(true);
  const [map, evidence] = await Promise.all([mapResponse.json(), evidenceResponse.json()]);
  const provinces = new Map(map.provinces.map((province) => [province.id, province]));
  const attributions = new Map(evidence.assignments.map((entry) => [entry.province_id, entry]));
  const pair = [...provinces.values()].flatMap((province) => province.neighbors.flatMap(({ id }) => {
    const left = attributions.get(province.id);
    const right = attributions.get(id);
    return left?.polity_name && right?.polity_name && left.polity_name !== right.polity_name
      ? [[province.id, id]] : [];
  }))[0];
  expect(pair).toBeTruthy();
  await page.getByRole('button', { name: 'Historical 1440' }).click();
  const [left, right] = await page.evaluate(([a, b]) => [
    window.strategyMapTest.readHistoricalColor(a),
    window.strategyMapTest.readHistoricalColor(b),
  ], pair);
  const distance = Math.sqrt(0.30 * (left[0] - right[0]) ** 2
    + 0.59 * (left[1] - right[1]) ** 2 + 0.11 * (left[2] - right[2]) ** 2);
  expect(distance).toBeGreaterThan(35);
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
  expect(result.samples['whole-region'].zoom).toBeCloseTo(0.96, 2);
  expect(result.samples.province.zoom).toBeGreaterThan(result.samples.britain.zoom);
  expect(result.margin[2]).toBeGreaterThan(result.margin[0] * 2);
  expect(result.margin).not.toEqual([21, 35, 44, 255]);
});

test('keyboard camera controls frame, pan, and double-click to focus a province', async ({ page }) => {
  const province = (await loadProvinces(page)).find(({ kind }) => kind === 'land');
  expect(province).toBeTruthy();
  const canvas = page.locator('[data-map-viewport] canvas');
  await page.evaluate(() => window.strategyMapTest.setCameraPreset('province'));
  await canvas.focus();
  await page.keyboard.press('Home');
  await expect.poll(() => page.evaluate(() => window.strategyMapTest.readCamera().zoom)).toBeCloseTo(0.96, 2);

  // The fully framed map is intentionally clamped to its center. Test keyboard
  // panning after zooming into a subregion where the camera has room to move.
  await page.evaluate(() => window.strategyMapTest.setCameraPreset('britain'));
  const beforePan = await page.evaluate(() => window.strategyMapTest.readCamera().target);
  await page.keyboard.press('ArrowRight');
  const afterPan = await page.evaluate(() => window.strategyMapTest.readCamera().target);
  expect(afterPan[0]).toBeGreaterThan(beforePan[0]);

  await page.evaluate(() => window.strategyMapTest.setCameraPreset('whole-region'));
  const point = await page.evaluate((id) => window.strategyMapTest.projectProvince(id), province.id);
  expect(point).toBeTruthy();
  const beforeFocus = await page.evaluate(() => window.strategyMapTest.readCamera());
  await page.mouse.dblclick(point.x, point.y);
  await expect.poll(() => page.evaluate(() => window.strategyMapTest.readCamera().zoom)).toBe(2.5);
  const afterFocus = await page.evaluate(() => window.strategyMapTest.readCamera());
  expect(afterFocus.target).not.toEqual(beforeFocus.target);
});

test('map modes keep host owner bytes separate from 1440 evidence', async ({ page }) => {
  await page.getByRole('button', { name: 'Game political' }).click();
  const evidence = await (await page.request.get('/map/western_europe/polities_1440.json')).json();
  const polygon = evidence.assignments.find((entry) => entry.resolution === 'polygon');
  expect(polygon).toBeTruthy();
  await page.evaluate((id) => {
    window.strategyMapTest.focusProvince(id);
    window.strategyMapTest.setOwnerColor(id, [247, 11, 171, 255]);
    window.strategyMapTest.renderFrame();
  }, polygon.province_id);
  const point = await page.evaluate((id) => window.strategyMapTest.projectProvince(id), polygon.province_id);
  await page.mouse.click(point.x, point.y);
  // Keep the hover lookup identical for the political and restored frames.
  await page.mouse.move(0, 0);
  const political = await page.evaluate(({ x, y }) => window.strategyMapTest.readPixel(x, y), point);
  await page.getByRole('button', { name: 'Historical 1440' }).click();
  await expect.poll(() => page.evaluate(() => window.strategyMapTest.readMapMode())).toBe('historical');
  const historical = await page.evaluate(({ x, y }) => window.strategyMapTest.readPixel(x, y), point);
  expect(historical).not.toEqual(political);
  await expect(page.locator('[data-map-selected]')).toHaveAttribute('data-map-selected', String(polygon.province_id));
  await expect(page.locator('[data-historical-resolution]')).toHaveAttribute('data-historical-resolution', 'polygon');
  await page.getByRole('button', { name: 'Terrain', exact: true }).click();
  await expect.poll(() => page.evaluate(() => window.strategyMapTest.readMapMode())).toBe('terrain');
  const terrain = await page.evaluate(({ x, y }) => window.strategyMapTest.readPixel(x, y), point);
  expect(terrain).not.toEqual(political);
  await page.getByRole('button', { name: 'Game political' }).click();
  await expect.poll(() => page.evaluate(() => window.strategyMapTest.readMapMode())).toBe('political');
  const restored = await page.evaluate(({ x, y }) => window.strategyMapTest.readPixel(x, y), point);
  expect(restored).toEqual(political);
  expect(await page.evaluate((id) => window.strategyMapTest.readOwnerColor(id), polygon.province_id))
    .toEqual([247, 11, 171, 255]);
});

test('historical unknown and coarse entries are explicit while labels and sea picking work', async ({ page }) => {
  const evidence = await (await page.request.get('/map/western_europe/polities_1440.json')).json();
  await page.getByRole('button', { name: 'Historical 1440' }).click();
  await page.evaluate(() => window.strategyMapTest.setCameraPreset('whole-region'));
  await expect(page.locator('.map-label', { hasText: 'Kingdom of England' })).toBeVisible();
  await expect(page.locator('.map-label', { hasText: 'Earldom of Desmond' })).toHaveCount(0);
  await page.evaluate(() => window.strategyMapTest.setCameraPreset('britain'));
  await expect(page.locator('.map-label', { hasText: 'Earldom of Desmond' })).toBeVisible();
  for (const [resolution, expected] of [['unmatched', 'Unknown attribution'], ['coarse_group', 'Low precision group']]) {
    const entry = evidence.assignments.find((item) => item.resolution === resolution);
    expect(entry).toBeTruthy();
    const point = await page.evaluate((id) => {
      window.strategyMapTest.focusProvince(id);
      return window.strategyMapTest.projectProvince(id);
    }, entry.province_id);
    await page.mouse.click(point.x, point.y);
    await expect(page.locator(`[data-historical-resolution="${resolution}"]`)).toContainText(expected);
    const referenceColor = await page.evaluate((id) => window.strategyMapTest.readHistoricalColor(id), entry.province_id);
    expect(referenceColor).toHaveLength(4);
    expect(referenceColor[3]).toBe(255);
    expect(referenceColor).not.toEqual([113, 120, 116, 255]);
    if (resolution === 'coarse_group' && entry.polity_name) {
      await expect(page.locator('.map-label', { hasText: entry.polity_name })).toHaveCount(0);
    }
    const province = (await loadProvinces(page)).find((item) => item.id === entry.province_id);
    const names = await (await page.request.get('/map/western_europe/names.en.json')).json();
    await expect(page.locator('.map-inspector h2')).toHaveText(names[province.name_key]);
  }
  await page.evaluate(() => window.strategyMapTest.setCameraPreset('britain'));
  await expect(page.locator('.map-label[data-kind="country"]').first()).toBeVisible();
  await page.evaluate(() => window.strategyMapTest.setCameraPreset('province'));
  await expect(page.locator('.map-label[data-kind="province"]').first()).toBeVisible();
  const sea = (await loadProvinces(page)).find((province) => province.kind === 'sea');
  const point = await page.evaluate((id) => {
    window.strategyMapTest.focusProvince(id);
    return window.strategyMapTest.projectProvince(id);
  }, sea.id);
  await page.mouse.click(point.x, point.y);
  await expect(page.locator('[data-map-selected]')).toHaveAttribute('data-map-selected', String(sea.id));
});

test('rejects historical evidence from a different map hash', async ({ page }) => {
  await page.route('**/polities_1440.json', async (route) => {
    const response = await route.fetch();
    const evidence = await response.json();
    evidence.map_hash = '0'.repeat(64);
    await route.fulfill({ response, json: evidence });
  });
  await page.reload();
  await expect(page.locator('main[data-map-status="error"]')).toBeVisible();
  await expect(page.getByText(/does not match the loaded map identity/)).toBeVisible();
});

test('rejects a missing localized province name', async ({ page }) => {
  const provinces = await loadProvinces(page);
  await page.route('**/names.en.json', async (route) => {
    const response = await route.fetch();
    const names = await response.json();
    delete names[provinces[0].name_key];
    await route.fulfill({ response, json: names });
  });
  await page.reload();
  await expect(page.locator('main[data-map-status="error"]')).toBeVisible();
  await expect(page.getByText(/no localized display name/)).toBeVisible();
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
