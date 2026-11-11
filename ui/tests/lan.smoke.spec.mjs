/* eslint-env browser */
import { expect, test } from '@playwright/test';

const clock = { date: '1440-01-01', tick: 0, paused: false, speed: 1 };
const country = {
  id: 1, tag: 'one', name: 'First Realm', color: '#ed9470', capital_id: 1, capital: 'Capital',
  treasury: '1000', income: '500', expenses: '200', tax_rate: 100, monthly_spending: '200',
  bankrupt: false, regency: false, ruler: null, heir: null,
};
const countries = [country, { ...country, id: 2, tag: 'two', name: 'Second Realm' }];
const solo = {
  mode: 'solo', state: 'idle', room: null, address: null, guest_connected: false,
  guest_country_id: null, can_takeover: false, temporary_ai: false,
  verified_checksums: 0, checksum_mismatches: 0, detail: null, generation: 0,
};

async function mockLan(page) {
  await page.addInitScript(({ clock, countries, solo }) => {
    const listeners = new Map();
    const state = {
      status: solo,
      snapshot: {
        generation: 0, sequence: 7, clock, countries, local_country_id: 1, owners: [],
        max_tax_rate: 200, max_monthly_spending: '2000',
      },
      calls: [],
      commands: [],
      emit(event, payload) {
        for (const callback of listeners.get(event) ?? []) callback({ payload });
      },
      changeStatus(change) {
        state.status = { ...state.status, ...change };
        state.snapshot.generation = state.status.generation;
        state.emit('strategy:lan', state.status);
        return state.status;
      },
      installSnapshot(change) {
        state.snapshot = { ...state.snapshot, ...change };
      },
    };
    window.__strategyMockLan = state;
    window.__TAURI__ = {
      event: {
        async listen(event, callback) {
          const callbacks = listeners.get(event) ?? new Set();
          callbacks.add(callback);
          listeners.set(event, callbacks);
          return () => { callbacks.delete(callback); };
        },
      },
      core: {
        async invoke(name, args) {
          state.calls.push({ name, args });
          if (name === 'lan_status') return state.status;
          if (name === 'game_snapshot') return state.snapshot;
          if (name === 'game_command') {
            state.commands.push(args.command);
            return null;
          }
          if (name === 'lan_discover') return [{
            name: 'Test room', address: '192.168.2.12:47620', player_count: 1, version: 1,
          }];
          if (name === 'lan_start') {
            state.installSnapshot({ sequence: 0, local_country_id: args.config.host_country_id, clock: { ...clock, paused: true } });
            return state.changeStatus({
              mode: 'host', state: 'hosting', room: args.config.room, address: '0.0.0.0:' + args.config.port,
              guest_country_id: args.config.guest_country_id, generation: state.status.generation + 1,
            });
          }
          if (name === 'lan_join') {
            state.installSnapshot({ sequence: 20, local_country_id: 2, clock: { ...clock, paused: true } });
            return state.changeStatus({
              mode: 'guest', state: 'connected', room: 'Test room', address: args.address,
              guest_country_id: 2, guest_connected: true, generation: state.status.generation + 1,
            });
          }
          if (name === 'lan_takeover') {
            return state.changeStatus({ temporary_ai: true, can_takeover: false });
          }
          if (name === 'lan_leave') {
            state.installSnapshot({ sequence: 0, local_country_id: 1, clock });
            return state.changeStatus({ ...solo, generation: state.status.generation + 1 });
          }
          throw new Error('Unexpected command ' + name);
        },
      },
    };
  }, { clock, countries, solo });
}

async function openLobby(page) {
  await page.getByRole('button', { name: 'LAN multiplayer' }).click();
  return page.getByRole('dialog', { name: 'LAN multiplayer' });
}

async function ipcCall(page, name) {
  return page.evaluate((command) => window.__strategyMockLan.calls.findLast((call) => call.name === command), name);
}

test('M5 host selects both human seats and opts into AI after the grace period', async ({ page }) => {
  await mockLan(page);
  await page.goto('/?testFrames=1');
  const lobby = await openLobby(page);
  const form = lobby.getByRole('form', { name: 'Host a LAN game' });
  await expect(form).toBeVisible();
  await expect(form.getByRole('combobox', { name: 'Host country' })).toHaveValue('1');
  await expect(form.getByRole('combobox', { name: 'Guest country' })).toHaveValue('2');
  await form.getByRole('combobox', { name: 'Host country' }).selectOption('2');
  await expect(form.getByRole('combobox', { name: 'Guest country' })).toHaveValue('1');
  await form.getByRole('combobox', { name: 'AI preset' }).selectOption('cautious');
  await form.getByLabel('Room name').fill('Western Front');
  await form.getByLabel('Disconnect grace (minutes)').fill('7');
  await form.getByLabel('Port').fill('49000');
  await form.getByRole('button', { name: 'Host LAN game' }).click();
  await expect.poll(() => ipcCall(page, 'lan_start')).toEqual({
    name: 'lan_start',
    args: {
      config: { room: 'Western Front', host_country_id: 2, guest_country_id: 1, ai_preset: 'cautious', grace_minutes: 7, port: 49000 },
    },
  });
  const active = lobby.getByRole('region', { name: 'Active LAN room' });
  await expect(active.getByText('Host · Waiting for guest')).toBeVisible();
  await expect(active.getByText('Address · 0.0.0.0:49000')).toBeVisible();
  await expect(active.getByRole('button', { name: 'Give guest country to AI' })).toHaveCount(0);

  await page.evaluate(() => {
    window.__strategyMockLan.changeStatus({ can_takeover: true, detail: 'Disconnect grace expired' });
  });
  await expect(active.getByText('Disconnect grace expired')).toBeVisible();
  await active.getByRole('button', { name: 'Give guest country to AI' }).click();
  await expect(active.getByText('Temporary AI controls the guest country')).toBeVisible();
  await expect.poll(() => ipcCall(page, 'lan_takeover')).toEqual({ name: 'lan_takeover' });

  await active.getByRole('button', { name: 'Close room' }).click();
  await expect(form).toBeVisible();
  await lobby.getByRole('button', { name: 'Close LAN lobby' }).click();
  await expect(page.getByRole('region', { name: 'Game clock' }).getByRole('combobox', { name: 'Speed' })).toBeEnabled();
});

test('M5 guest joins discovery or manual address and refreshes the authoritative snapshot on reconnect', async ({ page }) => {
  await mockLan(page);
  await page.goto('/?testFrames=1');
  const lobby = await openLobby(page);
  await lobby.getByLabel('Player name').fill('Second player');
  await lobby.getByRole('button', { name: 'Find LAN rooms' }).click();
  await lobby.getByRole('button', { name: 'Join Test room' }).click();
  await expect.poll(() => ipcCall(page, 'lan_join')).toEqual({
    name: 'lan_join', args: { address: '192.168.2.12:47620', name: 'Second player' },
  });
  await expect(lobby.getByText('Guest · Connected')).toBeVisible();
  await lobby.getByRole('button', { name: 'Close LAN lobby' }).click();

  const gameClock = page.getByRole('region', { name: 'Game clock' });
  const speed = gameClock.getByRole('combobox', { name: 'Speed' });
  const resume = gameClock.getByRole('button', { name: 'Resume' });
  const pause = gameClock.getByRole('button', { name: 'Pause' });
  await expect(speed).toBeDisabled();
  await expect(resume).toBeEnabled();
  await resume.click();
  await expect.poll(() => page.evaluate(() => window.__strategyMockLan.commands))
    .toEqual([{ type: 'set_paused', paused: false }]);
  await expect(resume).toBeVisible();
  await page.evaluate((clock) => {
    window.__strategyMockLan.emit('strategy:update', { generation: 1, sequence: 21, clock, countries: [] });
  }, clock);
  await expect(pause).toBeVisible();

  await page.evaluate(() => window.__strategyMockLan.changeStatus({ state: 'reconnecting', guest_connected: false }));
  await expect(page.locator('[data-lan-state="reconnecting"]')).toHaveText('Reconnecting');
  await expect(pause).toBeDisabled();
  await page.evaluate(() => window.__strategyMockLan.changeStatus({ state: 'resyncing', detail: 'Replacing snapshot' }));
  await expect(page.locator('[data-lan-state="resyncing"]')).toHaveText('Resynchronizing');
  await page.evaluate((clock) => {
    const mock = window.__strategyMockLan;
    mock.installSnapshot({ sequence: 2, clock: { ...clock, date: '1440-02-15', speed: 4 } });
    mock.changeStatus({
      generation: 2, state: 'connected', guest_connected: true, verified_checksums: 4, checksum_mismatches: 1,
      detail: null,
    });
  }, clock);
  await expect(gameClock.locator('time')).toHaveText('1440-02-15');
  await expect(speed).toHaveValue('4');
  await expect(speed).toBeDisabled();
  await expect(pause).toBeEnabled();
  await page.evaluate((clock) => {
    window.__strategyMockLan.emit('strategy:update', { generation: 1, sequence: 99, clock, countries: [] });
  }, clock);
  await expect(gameClock.locator('time')).toHaveText('1440-02-15');

  const room = await openLobby(page);
  await expect(room.getByText('Verified checksums')).toBeVisible();
  await expect(room.getByText('Checksum mismatches')).toBeVisible();
  await expect(room.locator('dd')).toHaveText(['4', '1']);
  await room.getByRole('button', { name: 'Leave room' }).click();
  await room.getByLabel('Player name').fill('Manual player');
  await room.getByLabel('IP address and port').fill('192.168.2.50:49000');
  await room.getByRole('button', { name: 'Join by address' }).click();
  await expect.poll(() => ipcCall(page, 'lan_join')).toEqual({
    name: 'lan_join', args: { address: '192.168.2.50:49000', name: 'Manual player' },
  });
  await expect(room.getByText('Guest · Connected')).toBeVisible();
});
