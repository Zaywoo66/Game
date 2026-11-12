/* eslint-env browser */
import { expect, test } from '@playwright/test';

const clock = { date: '1440-01-01', tick: 0, paused: true, speed: 1 };
const pending = {
  country_id: 1, key: 'harvest_surplus', title_key: 'event_harvest_surplus_title',
  desc_key: 'event_harvest_surplus_desc', options: [
    { text_key: 'event_harvest_surplus_collect' }, { text_key: 'event_harvest_surplus_store' },
  ],
};

test('M7 localized choice waits for host update and sends the chosen option', async ({ page }) => {
  await page.addInitScript(({ clock, pending }) => {
    const listeners = new Set();
    const commands = [];
    window.__eventMock = {
      commands,
      emit: (update) => { for (const callback of listeners) callback({ payload: update }); },
    };
    window.__TAURI__ = {
      event: { async listen(name, callback) {
        if (name !== 'strategy:update') throw Error(name);
        listeners.add(callback);
        return () => listeners.delete(callback);
      } },
      core: { async invoke(name, args) {
        if (name === 'game_snapshot') return {
          sequence: 1, clock, local_country_id: 1, countries: [], owners: [],
          max_tax_rate: 200, max_monthly_spending: '10000',
          pending_events: [pending, { ...pending, country_id: 2, key: 'other_event' }],
        };
        if (name === 'game_command') { commands.push(args.command); return null; }
        throw Error(name);
      } },
    };
  }, { clock, pending });
  await page.goto('/?verify=1&map=western_europe&verifyHost=1');
  const dialog = page.getByRole('dialog', { name: 'A Surplus Harvest' });
  await expect(dialog).toBeVisible({ timeout: 60_000 });
  await expect(dialog.getByText('Farmers report full granaries.', { exact: false })).toBeVisible();
  await dialog.getByRole('button', { name: 'Store grain for later markets' }).click();
  await expect.poll(() => page.evaluate(() => window.__eventMock.commands)).toEqual([
    { type: 'choose_event', country_id: 1, key: 'harvest_surplus', option: 1 },
  ]);
  // The UI keeps the event until the authoritative host state changes.
  await expect(dialog).toBeVisible();
  await page.evaluate((clock) => window.__eventMock.emit({
    sequence: 2, clock, countries: [], pending_events: [],
  }), clock);
  await expect(dialog).toHaveCount(0);
});

test('M7 event popup selects the Russian catalog for a Russian browser locale', async ({ browser }) => {
  const context = await browser.newContext({ locale: 'ru-RU', viewport: { width: 1280, height: 900 } });
  const page = await context.newPage();
  await page.addInitScript(({ clock, pending }) => {
    window.__TAURI__ = {
      event: { async listen() { return () => undefined; } },
      core: { async invoke(name) {
        if (name !== 'game_snapshot') throw Error(name);
        return {
          sequence: 1, clock, local_country_id: 1, countries: [], owners: [],
          max_tax_rate: 200, max_monthly_spending: '10000', pending_events: [pending],
        };
      } },
    };
  }, { clock, pending });
  await page.goto('/?verify=1&map=western_europe&verifyHost=1');
  const dialog = page.getByRole('dialog', { name: 'Изобильный урожай' });
  await expect(dialog).toBeVisible({ timeout: 60_000 });
  await expect(dialog.getByText(/Земледельцы сообщают о полных амбарах/)).toBeVisible();
  await expect(dialog.getByRole('button', { name: 'Оставить зерно на будущее' })).toBeVisible();
  await context.close();
});
