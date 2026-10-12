// Captures for the manual page "Connecting lines and paired graphs": the graph editor with
// a paired t test's default graph and the Connecting lines section, in the light and dark
// app themes. Needs the production build served (`cd apps/dioptra && pnpm build && pnpm
// preview`) and Chrome:
//   node scripts/capture-connecting.mjs [base-url]
import { createRequire } from 'node:module';
import { resolve } from 'node:path';

const require = createRequire(resolve('apps/dioptra/package.json'));
const { chromium } = require('@playwright/test');

const base = process.argv[2] ?? 'http://127.0.0.1:5762';
const out = (n) => resolve(`docs/manual/src/images/${n}`);
const browser = await chromium.launch({ channel: 'chrome', headless: true });

async function paste(page, text) {
  await page.evaluate((t) => {
    const dt = new DataTransfer();
    dt.setData('text/plain', t);
    document.querySelector('[data-grid]').dispatchEvent(new ClipboardEvent('paste', { clipboardData: dt, bubbles: true, cancelable: true }));
  }, text);
}

try {
  for (const scheme of ['light', 'dark']) {
    const page = await browser.newPage({ viewport: { width: 1440, height: 860 }, deviceScaleFactor: 1, colorScheme: scheme });
    await page.goto(base);
    await page.getByTestId('app').and(page.locator('[data-ready="true"]')).waitFor({ timeout: 60000 });
    await page.getByTestId('rail-new-table').click();
    await page.getByTestId('kind-column').click();
    await page.getByTestId('new-table-create').click();
    await page.locator('[data-cell="0:0"]').click();
    await paste(page, 'Baseline\tWeek 4\tWeek 8\n131\t124\t118\n118\t116\t105\n142\t\t127\n109\t104\t101\n125\t117\t112\n137\t126\t119\n114\t112\t104\n128\t119\t116');
    await page.waitForTimeout(300);
    await page.getByTestId('tool-analyze').click();
    await page.getByTestId('analysis-repeated_measures_anova').click();
    await page.getByTestId('analysis-run').click();
    await page.getByTestId('sheet-make-graph').click();
    await page.locator('[data-testid="graph-svg"] svg').waitFor();
    const sec = page.getByTestId('insp-graph-connecting');
    await sec.getByTestId('conn-both').click();
    await page.waitForTimeout(500);
    // Room for the Connecting lines section: fold the Graph section, close the paste notice.
    await page.getByTestId('insp-graph').locator('.insp-head').click();
    await page.getByTestId('graph-view').click({ position: { x: 5, y: 5 } });
    await page.locator('.notice button[aria-label="Dismiss"], [data-testid="notice-close"]').first().click({ timeout: 2000 }).catch(() => {});
    await page.waitForTimeout(6000);
    await page.evaluate(() => document.fonts.ready);
    await page.screenshot({ path: out(`connecting-${scheme}.png`) });
    console.log('wrote', out(`connecting-${scheme}.png`));
    await page.close();
  }
} finally {
  await browser.close();
}
