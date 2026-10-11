import { defineConfig } from '@playwright/test';

/**
 * End-to-end tests run the real Rust engine compiled to WebAssembly inside the production
 * frontend build, served by `vite preview`. Run `../../scripts/build-wasm.sh` and `pnpm build`
 * first (the `verify` script does). One worker, one headless system Chrome.
 */
export default defineConfig({
  testDir: 'tests/e2e',
  timeout: 90_000,
  expect: { timeout: 15_000 },
  workers: 1,
  fullyParallel: false,
  retries: 0,
  reporter: [['list']],
  use: {
    channel: 'chrome',
    headless: true,
    baseURL: 'http://127.0.0.1:5762',
    colorScheme: 'light',
    viewport: { width: 1440, height: 900 },
    trace: 'retain-on-failure',
  },
  webServer: {
    command: 'pnpm preview',
    url: 'http://127.0.0.1:5762',
    reuseExistingServer: true,
    timeout: 60_000,
  },
});
