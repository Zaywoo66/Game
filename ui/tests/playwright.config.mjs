import { fileURLToPath } from 'node:url';
import { defineConfig } from '@playwright/test';

const baseURL = 'http://127.0.0.1:5174';

export default defineConfig({
  testDir: '.',
  outputDir: '../node_modules/.cache/map-test-results',
  workers: 1,
  timeout: 120_000,
  use: {
    baseURL,
    browserName: 'chromium',
    headless: true,
    viewport: { width: 1280, height: 900 },
    launchOptions: {
      args: ['--use-angle=swiftshader', '--enable-unsafe-swiftshader'],
    },
  },
  webServer: {
    command: 'npm run dev -- --host 127.0.0.1 --port 5174',
    cwd: fileURLToPath(new URL('..', import.meta.url)),
    url: baseURL,
    reuseExistingServer: false,
  },
});
