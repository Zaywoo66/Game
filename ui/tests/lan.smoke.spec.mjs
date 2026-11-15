/* eslint-env browser */
import { expect, test } from '@playwright/test';

const clock = { date: '2022-01-01', tick: 0, paused: true, speed: 1 };
const country = {
  id: 1, tag: 'one', name: 'First Realm', color: '#ed9470', capital_id: 1, capital: 'Capital',
  treasury: '1000', income: '500', expenses: '200', tax_rate: 100, monthly_spending: '200',
  bankrupt: false, regency: false, ruler: null, heir: null,
};
const countries = [country, { ...country, id: 2, tag: 'two', name: 'Second Realm' }];
const solo = {
  mode: 'solo', state: 'idle', room: null, address: null, guest_connected: false,
  host_country_id: null, guest_country_id: null, started: false, can_takeover: false,
  temporary_ai: false, verified_checksums: 0, checksum_mismatches: 0,
  detail: null, generation: 0,
};

async function mockLan(page, { snapshotError = false } = {}) {
  await page.addInitScript(({ clock, countries, solo, snapshotError }) => {
    const listeners = new Map();
    const state = {
      status: solo,
      snapshot: {
        generation: 0, sequence: 7, clock, countries, local_country_id: 1, owners: [],
        max_tax_rate: 200, max_monthly_spending: '2000',
      },
      calls: [],
      changeStatus(change) {
        state.status = { ...state.status, ...change };
        state.snapshot.generation = state.status.generation;
        for (const callback of listeners.get('strategy:lan') ?? []) callback({ payload: state.status });
        return state.status;
      },
    };
    window.__strategyMockLan = state;
    window.__TAURI__ = {
      event: {
        async listen(event, callback) {
          const callbacks = listeners.get(event) ?? new Set();
          callbacks.add(callback);
          listeners.set(event, callbacks);
          return () => callbacks.delete(callback);
        },
      },
      core: {
        async invoke(name, args) {
          state.calls.push({ name, args });
          if (name === 'lan_status') return state.status;
          if (name === 'game_snapshot') {
            if (window.__strategyMockLan.snapshotError) throw new Error('Scenario snapshot unavailable');
            return state.snapshot;
          }
          if (name === 'scenario_country_options') return countries.map(({ id, tag, name }) => ({
            id, tag, name, color: id === 1 ? '#ed9470' : '#759fb9',
          }));
          if (name === 'lan_discover') return [{
            name: 'Test room', address: '192.168.2.12:47620', player_count: 1, version: 1,
          }];
          if (name === 'lan_start') {
            return state.changeStatus({
              mode: 'host', state: 'hosting', room: args.config.room, address: '192.168.2.10:47620',
              host_country_id: null, guest_country_id: null, guest_connected: false,
              started: false, generation: state.status.generation + 1,
            });
          }
          if (name === 'lan_join') {
            return state.changeStatus({
              mode: 'guest', state: 'connected', room: 'Test room', address: args.address,
              host_country_id: 1, guest_country_id: null, guest_connected: true,
              started: false, generation: state.status.generation + 1,
            });
          }
          if (name === 'lan_claim_country') {
            const key = state.status.mode === 'host' ? 'host_country_id' : 'guest_country_id';
            return state.changeStatus({ [key]: args.countryId });
          }
          if (name === 'lan_start_campaign') {
            return state.changeStatus({ started: true });
          }
          if (name === 'solo_start_campaign') return null;
          if (name === 'lan_leave') {
            return state.changeStatus({ ...solo, generation: state.status.generation + 1 });
          }
          throw new Error('Unexpected command ' + name);
        },
      },
    };
    state.snapshotError = snapshotError;
  }, { clock, countries, solo, snapshotError });
}

async function ipcCall(page, name) {
  return page.evaluate((command) => window.__strategyMockLan.calls.findLast((call) => call.name === command), name);
}

test('launch offers solo country selection and opens multiplayer choices before the campaign', async ({ page }) => {
  await mockLan(page);
  await page.goto('/');
  await expect(page.getByRole('heading', { name: 'Strategy' })).toBeVisible();
  await expect(page.locator('.map-page')).toHaveCount(0);

  await page.getByRole('button', { name: /Network game/ }).click();
  await expect(page.getByRole('heading', { name: 'Network game' })).toBeVisible();
  await expect(page.getByRole('button', { name: /Create a room/ })).toBeVisible();
  await expect(page.getByRole('button', { name: /Join a room/ })).toBeVisible();
  await page.getByRole('button', { name: /Main menu/ }).click();
  await page.getByRole('button', { name: /Single player/ }).click();
  await expect(page.getByRole('heading', { name: 'Choose your country' })).toBeVisible();
  await expect(page.locator('.map-page')).toHaveCount(0);
  await page.getByLabel('Your country').selectOption('2');
  await page.getByRole('button', { name: 'Start campaign' }).click();
  await expect.poll(() => ipcCall(page, 'solo_start_campaign')).toEqual({
    name: 'solo_start_campaign', args: { countryId: 2 },
  });
  await expect(page.locator('.map-page')).toBeVisible();
  await expect(page.getByRole('button', { name: 'Historical 1440' })).toHaveCount(0);
});

test('host creates an empty room, claims a country and starts after the guest selects theirs', async ({ page }) => {
  await mockLan(page);
  await page.goto('/');
  await page.getByRole('button', { name: /Network game/ }).click();
  await page.getByRole('button', { name: /Create a room/ }).click();
  await page.getByLabel('Room name').fill('Western Front');
  await page.getByLabel('Your country').selectOption('1');
  await page.getByRole('button', { name: 'Open room' }).click();
  await expect.poll(() => ipcCall(page, 'lan_start')).toEqual({
    name: 'lan_start', args: { config: {
      room: 'Western Front', host_country_id: 1, guest_country_id: 0,
      ai_preset: 'balanced', grace_minutes: 2, port: 47620,
    } },
  });
  await expect(page.getByRole('region', { name: 'Room country selection' })).toBeVisible();
  const hostSelect = page.getByLabel('Choose your country');
  await hostSelect.selectOption('1');
  await expect.poll(() => ipcCall(page, 'lan_claim_country')).toEqual({
    name: 'lan_claim_country', args: { countryId: 1 },
  });
  const start = page.getByRole('button', { name: 'Start campaign' });
  await expect(start).toBeDisabled();

  await page.evaluate(() => window.__strategyMockLan.changeStatus({
    guest_connected: true, guest_country_id: 2,
  }));
  await expect(start).toBeEnabled();
  await start.click();
  await expect.poll(() => ipcCall(page, 'lan_start_campaign')).toEqual({ name: 'lan_start_campaign' });
  await expect(page.locator('.map-page')).toBeVisible();
});

test('guest discovers a room and chooses their own country inside it', async ({ page }) => {
  await mockLan(page);
  await page.goto('/');
  await page.getByRole('button', { name: /Network game/ }).click();
  await page.getByRole('button', { name: /Join a room/ }).click();
  await page.getByLabel('Player name').fill('Second player');
  await page.getByRole('button', { name: 'Search LAN' }).click();
  await page.getByRole('button', { name: 'Join', exact: true }).click();
  await expect.poll(() => ipcCall(page, 'lan_join')).toEqual({
    name: 'lan_join', args: { address: '192.168.2.12:47620', name: 'Second player' },
  });
  await expect(page.getByRole('region', { name: 'Room country selection' })).toBeVisible();
  await expect(page.getByText('First Realm').last()).toBeVisible();
  await page.getByLabel('Choose your country').selectOption('2');
  await expect.poll(() => ipcCall(page, 'lan_claim_country')).toEqual({
    name: 'lan_claim_country', args: { countryId: 2 },
  });
  await expect(page.getByLabel('Choose your country')).toHaveValue('2');
  await expect(page.getByText('When both players have chosen, the host can start the campaign.')).toBeVisible();
});

test('country roster failures show an actionable error instead of loading forever', async ({ page }) => {
  await mockLan(page, { snapshotError: true });
  await page.goto('/');
  await page.getByRole('button', { name: /Network game/ }).click();
  await page.getByRole('button', { name: /Create a room/ }).click();
  await expect(page.getByRole('alert')).toContainText('Scenario snapshot unavailable');
  await expect(page.getByLabel('Your country')).toContainText('Country data unavailable');
  await expect(page.getByLabel('Your country')).not.toContainText('Loading countries');
});
