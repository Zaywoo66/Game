import { spawnSync } from 'node:child_process';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('..', import.meta.url));
const cli = fileURLToPath(new URL('../node_modules/@playwright/test/cli.js', import.meta.url));
const run = spawnSync(process.execPath, [cli, 'test', '--config', 'tests/playwright.config.mjs', 'tests/map.modes.screenshots.spec.mjs'], {
  cwd: root,
  env: { ...process.env, CAPTURE_MAP_MODES: '1' },
  stdio: 'inherit',
});
process.exit(run.status ?? 1);
