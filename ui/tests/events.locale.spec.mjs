import { expect, test } from '@playwright/test';

test('event locale catalog is served by the dev app', async ({ request }) => {
  const response = await request.get('/locales/events.en.json');
  expect(response.ok()).toBe(true);
  expect(response.headers()['content-type']).toContain('application/json');
  const catalog = await response.json();
  expect(catalog.event_harvest_surplus_title).toBe('A Surplus Harvest');
  expect(catalog.event_frontier_patrol_drill).toBe('Drill the field army');
});
