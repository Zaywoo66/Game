/* eslint-env browser */
import { expect, test } from '@playwright/test';

const clock = { date: '1440-02-03', tick: 33, paused: true, speed: 1 };

async function landPoints(page) {
  const response = await page.request.get('/map/western_europe/provinces.json');
  expect(response.ok()).toBe(true);
  const { provinces } = await response.json();
  await page.goto('/?verify=1');
  await expect(page.locator('main[data-map-status="ready"]')).toBeVisible({ timeout: 60_000 });
  return page.evaluate((ids) => ids.flatMap((id) => {
    const point = window.strategyMapTest.projectProvince(id);
    return point && point.x > 380 && point.x < 850 && point.y > 170 && point.y < 700
      ? [{ id, point }] : [];
  }).slice(0, 2), provinces.filter((province) => province.kind === 'land').map((province) => province.id));
}

async function mockWarHost(page, capitalId, enemyId) {
  const character = {
    id: 1, name: 'Eleanor', dynasty: 'House of the Oak', age: 35, adm: 5, dip: 7, mil: 6,
    traits: ['resolute'], death: '1460-01-01',
  };
  const local = {
    id: 1, tag: 'tst', name: 'Test Realm', color: '#f70bab', capital_id: capitalId, capital: 'Capital City',
    treasury: '25000', income: '1700', expenses: '900', tax_rate: 100, monthly_spending: '1000',
    bankrupt: false, regency: false, ruler: character, heir: null,
  };
  const enemy = {
    ...local, id: 2, name: 'Other Realm', color: '#2fc953', capital_id: enemyId,
    ruler: null, capital: 'Other Capital',
  };
  const army = {
    id: 1, country_id: 1, province_id: capitalId, soldiers: 400, morale: 1000,
    to_province_id: null, progress: 0,
  };
  await page.addInitScript(({ clock, local, enemy, army }) => {
    const listeners = new Set();
    const mock = {
      commands: [],
      emit(update) {
        for (const callback of listeners) callback({ payload: { clock, countries: [], ...update } });
      },
    };
    window.__strategyMockWar = mock;
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
          if (name === 'game_snapshot') return {
            sequence: 10, clock, local_country_id: 1, countries: [local, enemy],
            owners: [
              { province_id: local.capital_id, country_id: local.id },
              { province_id: enemy.capital_id, country_id: enemy.id },
            ],
            max_tax_rate: 200, max_monthly_spending: '9000', min_raise_soldiers: 50, max_raise_soldiers: 1000,
            armies: [army, { ...army, id: 2, soldiers: 180 }, { ...army, id: 3, country_id: 2, province_id: enemy.capital_id }],
            wars: [], sieges: [], forts: [{ province_id: enemy.capital_id, level: 2 }],
          };
          if (name === 'game_command') {
            mock.commands.push(args.command);
            return null;
          }
          if (name === 'game_ai_log') return [
            { tick: 33, country_id: 2, reason: 'defend_capital', command: 'move_army', score: 250 },
          ];
          throw new Error('Unexpected command ' + name);
        },
      },
    };
  }, { clock, local, enemy, army });
  return army;
}

function lastCommand(page) {
  return page.evaluate(() => window.__strategyMockWar.commands.at(-1));
}

test('M4 renders armies and applies only authoritative land-war updates', async ({ page }) => {
  const [capital, enemy] = await landPoints(page);
  expect(capital && enemy).toBeTruthy();
  const army = await mockWarHost(page, capital.id, enemy.id);
  const modelResponse = page.waitForResponse((response) => response.url().endsWith('/models/infantry.glb'));
  await page.goto('/?verify=1&verifyHost=1');
  await expect(page.locator('main[data-map-status="ready"]')).toBeVisible({ timeout: 60_000 });
  expect((await modelResponse).ok()).toBe(true);
  const viewport = page.locator('[data-map-viewport]');
  await expect(viewport).toHaveAttribute('data-map-army-instances', '3', { timeout: 30_000 });
  await expect(page.getByText('Died 1460-01-01')).toHaveCount(0);
  await page.getByRole('button', { name: 'Military', exact: true }).click();
  const panel = page.getByRole('region', { name: 'Military' });
  await expect(panel).toBeVisible();

  const raiseForm = panel.getByRole('form', { name: 'Raise army' });
  await expect(raiseForm.getByRole('spinbutton')).toHaveValue('50');
  await expect(raiseForm.getByRole('button')).toBeEnabled();
  await raiseForm.getByRole('button').click();
  await expect.poll(() => lastCommand(page)).toEqual({
    type: 'raise_army', country_id: 1, province_id: capital.id, soldiers: 50,
  });
  await expect(viewport).toHaveAttribute('data-map-army-instances', '3');

  const picker = panel.getByRole('combobox', { name: 'Army', exact: true });
  await picker.selectOption('3');
  await expect(panel.getByRole('button', { name: 'Choose move destination' })).toHaveCount(0);
  await picker.selectOption('1');
  await panel.getByRole('button', { name: 'Split', exact: true }).click();
  await expect.poll(() => lastCommand(page)).toEqual({
    type: 'split_army', country_id: 1, army_id: 1, soldiers: 1,
  });
  await panel.getByRole('combobox', { name: 'Merge from' }).selectOption('2');
  await panel.getByRole('button', { name: 'Merge', exact: true }).click();
  await expect.poll(() => lastCommand(page)).toEqual({
    type: 'merge_armies', country_id: 1, into: 1, from: 2,
  });

  await panel.getByRole('button', { name: 'Choose move destination' }).click();
  await page.mouse.click(enemy.point.x, enemy.point.y);
  await expect.poll(() => lastCommand(page)).toEqual({
    type: 'move_army', country_id: 1, army_id: 1, destination: enemy.id,
  });
  await expect(panel.getByText(/Moving to/)).toHaveCount(0);
  const [start, finish] = await page.evaluate(() => [1, 3].map((id) => window.strategyMapTest.readArmyPosition(id)));
  expect(start).not.toBeNull();
  expect(finish).not.toBeNull();
  expect(start).not.toEqual(finish);
  await page.evaluate(({ army, enemyId }) => {
    window.__strategyMockWar.emit({
      sequence: 11, armies: [{ ...army, to_province_id: enemyId, progress: 500 }], removed_armies: [2],
    });
  }, { army, enemyId: enemy.id });
  await expect(panel.getByText(/Moving to.*50%/)).toBeVisible();
  await expect(viewport).toHaveAttribute('data-map-army-instances', '2');
  const midpoint = start.map((coordinate, index) => (coordinate + finish[index]) / 2);
  await expect.poll(async () => {
    const position = await page.evaluate(() => window.strategyMapTest.readArmyPosition(1));
    return position?.every((coordinate, index) => Math.abs(coordinate - midpoint[index]) < 0.05);
  }, { timeout: 20_000 }).toBe(true);

  await panel.getByRole('button', { name: 'Choose retreat destination' }).click();
  await page.mouse.click(capital.point.x, capital.point.y);
  await expect.poll(() => lastCommand(page)).toEqual({
    type: 'retreat_army', country_id: 1, army_id: 1, destination: capital.id,
  });

  await panel.getByRole('button', { name: 'Choose move destination' }).click();
  await page.mouse.click(enemy.point.x, enemy.point.y);
  await panel.getByRole('button', { name: /Declare war for/ }).click();
  await expect.poll(() => lastCommand(page)).toEqual({
    type: 'declare_war', country_id: 1, defender_id: 2, goal_province_id: enemy.id,
  });
  await expect(panel.getByText('No active wars')).toBeVisible();
  const war = { id: 4, attacker_id: 1, defender_id: 2, goal_province_id: enemy.id, score: 30, occupations: [], offer: null };
  await page.evaluate(({ war, provinceId }) => {
    window.__strategyMockWar.emit({
      sequence: 12, wars: [war],
      sieges: [{ province_id: provinceId, war_id: 4, country_id: 1, progress_days: 2, required_days: 10 }],
    });
  }, { war, provinceId: enemy.id });
  await expect(panel.getByText('Fort level 2')).toBeVisible();
  await expect(panel.getByText('Test Realm siege · 2/10 days')).toBeVisible();

  await panel.getByRole('button', { name: 'Offer white peace' }).click();
  await expect.poll(() => lastCommand(page)).toEqual({
    type: 'offer_peace', country_id: 1, war_id: 4, cede_goal: false,
  });
  await page.evaluate(({ war, provinceId }) => {
    window.__strategyMockWar.emit({
      sequence: 13, wars: [{
        ...war, occupations: [{ province_id: provinceId, country_id: 1 }],
        offer: { offered_by: 2, cede_goal: true },
      }], sieges: [],
    });
  }, { war, provinceId: enemy.id });
  await expect(page.getByText('Owner · Other Realm')).toBeVisible();
  await expect(page.getByText('Occupied by · Test Realm')).toBeVisible();
  await panel.getByRole('button', { name: 'Accept peace' }).click();
  await expect.poll(() => lastCommand(page)).toEqual({ type: 'accept_peace', country_id: 1, war_id: 4 });
  await page.evaluate((provinceId) => {
    window.__strategyMockWar.emit({
      sequence: 14, removed_wars: [4], owners: [{ province_id: provinceId, country_id: 1 }],
    });
  }, enemy.id);
  await expect(page.getByText('Owner · Test Realm')).toBeVisible();
  await expect(page.getByText('Occupied by · Test Realm')).toHaveCount(0);

  await panel.getByText('AI decisions', { exact: true }).click();
  await expect(panel.getByText('defend_capital · move_army')).toBeVisible();
  await expect(panel.getByText('Score · 250')).toBeVisible();
});
