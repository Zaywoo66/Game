/* eslint-env browser */
import { expect, test } from '@playwright/test';

const clock = { date: '1440-01-01', tick: 0, paused: true, speed: 1 };
const local = {
  id: 1, tag: 'eng', name: 'England', color: '#b32d32', capital_id: 1, capital: 'London',
  treasury: '100000', income: '0', expenses: '0', tax_rate: 100, monthly_spending: '0',
  bankrupt: false, regency: false, ruler: null, heir: null,
  research_points: 120, known_technologies: [],
};
const technologies = [
  {
    key: 'recordkeeping', branch: 'administration', cost: 100, requires: [],
    pos_x: -2, pos_y: 3,
    effects: [{ key: 'tax_income', value: '100', kind: 'multiplicative', source_key: 'recordkeeping' }],
  },
  {
    key: 'professional_officers', branch: 'military', cost: 250,
    requires: ['recordkeeping'], pos_x: 1, pos_y: -1,
    effects: [{ key: 'army_morale', value: '50', kind: 'additive', source_key: 'professional_officers' }],
  },
];

test('M9 technology tree uses data positions and waits for authoritative research updates', async ({ page }) => {
  await page.addInitScript(({ clock, local, technologies }) => {
    const listeners = new Set();
    const commands = [];
    window.__strategyTechHost = {
      commands,
      emit(update) { for (const listener of listeners) listener({ payload: update }); },
    };
    window.__TAURI__ = {
      event: {
        async listen(name, callback) {
          if (name !== 'strategy:update') throw new Error(name);
          listeners.add(callback);
          return () => listeners.delete(callback);
        },
      },
      core: {
        async invoke(name, args) {
          if (name === 'game_snapshot') return {
            sequence: 1, clock, local_country_id: 1, countries: [local], technologies,
            owners: [{ province_id: 1, country_id: 1 }], guarantees: [],
            max_tax_rate: 200, max_monthly_spending: '100000',
          };
          if (name === 'game_command') {
            commands.push(args.command);
            return null;
          }
          throw new Error(name);
        },
      },
    };
  }, { clock, local, technologies });

  await page.goto('/?verify=1&map=western_europe&verifyHost=1');
  await page.getByRole('button', { name: 'Technology', exact: true }).click();
  const tree = page.getByRole('region', { name: 'Technology tree' });
  const root = tree.locator('[data-tech-key="recordkeeping"]');
  const military = tree.locator('[data-tech-key="professional_officers"]');
  await expect(root.getByRole('heading', { name: 'Recordkeeping' })).toBeVisible();
  await expect(military.getByRole('heading', { name: 'Professional officers' })).toBeVisible();
  await expect(root).toHaveAttribute('data-tech-x', '-2');
  await expect(root).toHaveAttribute('data-tech-y', '3');
  await expect(military).toHaveAttribute('data-tech-x', '1');
  await expect(military).toHaveAttribute('data-tech-y', '-1');
  expect(await root.evaluate((element) => element.style.left)).not.toBe(
    await military.evaluate((element) => element.style.left));
  await expect(military.getByText('Requires: Recordkeeping')).toBeVisible();
  await expect(military.getByRole('button', { name: 'Research' })).toBeDisabled();
  await expect(root).toHaveAttribute('title', /Tax income \+0\.1 multiplicative · Source: Recordkeeping/);

  await root.getByRole('button', { name: 'Research' }).click();
  await expect.poll(() => page.evaluate(() => window.__strategyTechHost.commands)).toEqual([
    { type: 'unlock_technology', country_id: 1, key: 'recordkeeping' },
  ]);
  await expect(root).toHaveAttribute('data-tech-status', 'ready');
  await page.evaluate(({ clock, local }) => window.__strategyTechHost.emit({
    sequence: 2, clock, countries: [{ ...local, research_points: 20,
      known_technologies: ['recordkeeping'] }], guarantees: [], owners: [],
  }), { clock, local });
  await expect(root).toHaveAttribute('data-tech-status', 'known');
  await expect(military).toHaveAttribute('data-tech-status', 'locked');
  await expect(military.getByText('Needs 230 more research points')).toBeVisible();

  await page.evaluate(({ clock, local }) => window.__strategyTechHost.emit({
    sequence: 3, clock, countries: [{ ...local, research_points: 300,
      known_technologies: ['recordkeeping'] }], guarantees: [], owners: [],
  }), { clock, local });
  await expect(military).toHaveAttribute('data-tech-status', 'ready');
  await military.getByRole('button', { name: 'Research' }).click();
  await expect.poll(() => page.evaluate(() => window.__strategyTechHost.commands.at(-1))).toEqual(
    { type: 'unlock_technology', country_id: 1, key: 'professional_officers' },
  );
});
