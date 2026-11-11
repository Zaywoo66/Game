/* eslint-env browser */
import { expect, test } from '@playwright/test';

const initialClock = { date: '1440-01-31', tick: 30, paused: false, speed: 1 };
const ruler = {
  id: 9, name: 'Elinor', dynasty: 'House of the Oak', age: 29, adm: 7, dip: 8, mil: 5,
  traits: ['calm_ruler'], death: null,
};
const local = {
  id: 1, tag: 'tst', name: 'Test Realm', color: '#f70bab', capital_id: 1, capital: 'Capital City',
  treasury: '9007199254740993001', income: '12250', expenses: '7100', tax_rate: 100,
  monthly_spending: '7000', bankrupt: false, regency: true, ruler,
  heir: { ...ruler, id: 10, name: 'Matilda', age: 4 },
};
const other = {
  ...local, id: 2, tag: 'oth', name: 'Other Realm', color: '#2fc953',
  capital: 'Other Capital', ruler: null, heir: null, regency: false,
};

async function mockHost(page, provinceId) {
  await page.addInitScript(({ initialClock, local, other, provinceId }) => {
    const listeners = new Set();
    const commands = [];
    const calls = [];
    const emit = (update) => {
      for (const callback of listeners) callback({ payload: update });
    };
    const mock = { commands, calls, emit, nextError: '' };
    window.__strategyMockHost = mock;
    window.__TAURI__ = {
      event: {
        async listen(event, callback) {
          if (event !== 'strategy:update') throw new Error('Unexpected event ' + event);
          listeners.add(callback);
          return () => { listeners.delete(callback); };
        },
      },
      core: {
        async invoke(name, args) {
          calls.push(name);
          if (name === 'game_snapshot') {
            // The UI must subscribe before this command and buffer the newer event.
            emit({ sequence: 10, clock: { ...initialClock, date: '1440-02-01', tick: 31 }, countries: [] });
            return {
              sequence: 9, clock: initialClock, local_country_id: local.id,
              countries: [local, other], owners: [{ province_id: provinceId, country_id: local.id }],
              max_tax_rate: 200, max_monthly_spending: '999999999999999999',
            };
          }
          if (name === 'game_command') {
            commands.push(args.command);
            if (mock.nextError) {
              const error = mock.nextError;
              mock.nextError = '';
              throw error;
            }
            return null;
          }
          throw new Error('Unexpected command ' + name);
        },
      },
    };
  }, { initialClock, local, other, provinceId });
}

test('M3 panels apply host deltas and issue commands without optimistic writes', async ({ page }) => {
  await mockHost(page, 1);
  await page.goto('/');
  const panel = page.getByRole('region', { name: 'Country and rulers' });
  const clock = page.getByRole('region', { name: 'Game clock' });
  await expect(panel).toBeVisible({ timeout: 60_000 });
  await expect(clock.locator('time')).toHaveText('1440-02-01');
  await expect(panel.getByText('9007199254740993.001')).toBeVisible();
  await expect(panel.getByText('Capital · Capital City')).toBeVisible();
  await expect(panel.getByText('Regency in effect')).toBeVisible();
  await expect(panel.getByRole('region', { name: 'Ruler' }).getByText('Elinor')).toBeVisible();
  await expect(panel.getByRole('region', { name: 'Heir' }).getByText('Matilda')).toBeVisible();

  await panel.getByLabel('Tax rate (%)').fill('12.3');
  await panel.getByLabel('Monthly spending').fill('987654321234567.891');
  await panel.getByRole('button', { name: 'Apply fiscal policy' }).click();
  await expect.poll(() => page.evaluate(() => window.__strategyMockHost.commands)).toEqual([
    {
      type: 'set_fiscal_policy', country_id: 1, tax_rate: 123,
      monthly_spending: '987654321234567891',
    },
  ]);
  await expect(panel.getByText('10%', { exact: true })).toBeVisible();
  await expect(panel.getByText('12.3%', { exact: true })).toHaveCount(0);

  await page.evaluate(({ row, clock }) => {
    window.__strategyMockHost.emit({ sequence: 11, clock, countries: [row] });
  }, {
    row: { ...local, tax_rate: 123, monthly_spending: '987654321234567891' },
    clock: { ...initialClock, date: '1440-02-02', tick: 32 },
  });
  await expect(panel.getByText('12.3%', { exact: true })).toBeVisible();

  await clock.getByRole('button', { name: 'Pause' }).click();
  await expect.poll(() => page.evaluate(() => window.__strategyMockHost.commands.at(-1)))
    .toEqual({ type: 'set_paused', paused: true });
  await expect(clock.getByRole('button', { name: 'Pause' })).toBeVisible();

  await page.evaluate((clock) => {
    window.__strategyMockHost.emit({ sequence: 12, clock: { ...clock, paused: true }, countries: [] });
    window.__strategyMockHost.emit({ sequence: 11, clock, countries: [] });
  }, { ...initialClock, date: '1440-02-02', tick: 32 });
  await expect(clock.getByRole('button', { name: 'Resume' })).toBeVisible();
  await expect(panel.getByText('12.3%', { exact: true })).toBeVisible();

  await page.evaluate(() => { window.__strategyMockHost.nextError = 'Speed denied by host'; });
  await clock.getByLabel('Speed').selectOption('5');
  await expect.poll(() => page.evaluate(() => window.__strategyMockHost.commands.at(-1)))
    .toEqual({ type: 'set_speed', speed: 5 });
  await expect(clock.getByRole('alert')).toHaveText('Speed denied by host');
  await expect(clock.getByLabel('Speed')).toHaveValue('1');

  await panel.getByRole('combobox', { name: 'Country', exact: true }).selectOption('2');
  await expect(panel.getByRole('heading', { name: 'Other Realm' })).toBeVisible();
  await expect(panel.getByRole('form', { name: 'Fiscal policy' })).toHaveCount(0);
});

test('M3 colors a province from the host ownership snapshot', async ({ page }) => {
  const map = await page.request.get('/map/western_europe/provinces.json');
  expect(map.ok()).toBe(true);
  const provinces = (await map.json()).provinces;
  await page.goto('/?verify=1');
  const ready = page.locator('main[data-map-status="ready"]');
  await expect(ready).toBeVisible({ timeout: 60_000 });

  const representative = await page.evaluate((ids) => {
    for (const id of ids) {
      const point = window.strategyMapTest.projectProvince(id);
      if (point && point.x > 300 && point.x < 830 && point.y > 140 && point.y < 800) {
        return { id, point };
      }
    }
    return null;
  }, provinces.filter(({ kind }) => kind === 'land').map(({ id }) => id));
  expect(representative).not.toBeNull();
  const { id, point } = representative;
  const clip = { x: Math.round(point.x), y: Math.round(point.y), width: 1, height: 1 };
  const before = await page.screenshot({ clip });

  await mockHost(page, id);
  await page.goto('/');
  await expect(ready).toBeVisible({ timeout: 60_000 });
  await expect(page.getByRole('region', { name: 'Country and rulers' })).toBeVisible();
  await expect.poll(async () => (await page.screenshot({ clip })).equals(before)).toBe(false);
  await page.mouse.click(point.x, point.y);
  const selected = page.locator('[data-map-selected]');
  await expect(selected).toHaveAttribute('data-map-selected', String(id));
  await expect(selected.getByText('Owner · Test Realm')).toBeVisible();
});
