// Render docs/media/banner.svg to docs/media/banner.png (1640 x 640, the README banner and
// the landing page's Open Graph image). Needs Chrome and the app's Playwright install:
//   node scripts/make-banner.mjs
import { createRequire } from 'node:module';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

const require = createRequire(resolve('apps/dioptra/package.json'));
const { chromium } = require('@playwright/test');

const svg = resolve('docs/media/banner.svg');
const out = resolve('docs/media/banner.png');
const browser = await chromium.launch({ channel: 'chrome', headless: true });
try {
  const page = await browser.newPage({ viewport: { width: 1640, height: 640 }, deviceScaleFactor: 1 });
  await page.goto(pathToFileURL(svg).href);
  await page.evaluate(() => document.fonts.ready);
  await page.waitForTimeout(300);
  await page.screenshot({ path: out });
  console.log('wrote', out);
} finally {
  await browser.close();
}
