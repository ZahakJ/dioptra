import { expect, test } from '@playwright/test';
import { boot, cell } from './helpers';

// A 1,000 x 1,000 table (10^6 cells) made with ImportText. Scrolling must keep only the
// viewport in the DOM, fetch cache blocks on demand and keep frames short.
test('scrolls a 1,000 x 1,000 table smoothly with only the viewport rendered', async ({ page }, info) => {
  test.setTimeout(180_000);
  await boot(page);
  const importMs = await page.evaluate(async () => {
    const lines: string[] = [];
    for (let r = 0; r < 1000; r++) {
      const row: string[] = [];
      for (let c = 0; c < 1000; c++) row.push(String(((r * 1000 + c) % 9973) / 8));
      lines.push(row.join('\t'));
    }
    const t0 = performance.now();
    const res = (await window.__dioptra!.dispatch({
      type: 'import_text',
      text: lines.join('\n'),
      kind: 'column',
      title: 'Big',
      options: { header: false },
    })) as { type: string; changed: { ids: string[]; tree: boolean; created?: string } };
    if (res.type !== 'paste_report') throw new Error(JSON.stringify(res).slice(0, 200));
    await window.__dioptra!.applyChanged(res.changed);
    return performance.now() - t0;
  });
  await expect(page.getByTestId('rail-item-Big')).toBeVisible();
  await expect(cell(page, 0, 1)).toHaveText('0.125');

  const stats = await page.evaluate(async () => {
    const g = document.querySelector('[data-grid]') as HTMLElement;
    const frame = () => new Promise<number>((r) => requestAnimationFrame(r));
    await frame();
    const frames: number[] = [];
    let maxCells = 0;
    let last = await frame();
    // Diagonal scroll: 300 frames, 240 px down and 64 px right per frame.
    for (let i = 0; i < 300; i++) {
      g.scrollTop += 240;
      g.scrollLeft += 64;
      const now = await frame();
      frames.push(now - last);
      last = now;
      maxCells = Math.max(maxCells, g.querySelectorAll('.gc').length);
    }
    // Jump far and time until the new viewport is filled from the engine.
    const t0 = performance.now();
    g.scrollTop = 900 * 24;
    g.scrollLeft = 880 * 96;
    let filledMs = -1;
    for (let i = 0; i < 300; i++) {
      await frame();
      const c = g.querySelector('[data-cell="905:885"]');
      if (c && c.textContent) {
        filledMs = performance.now() - t0;
        break;
      }
    }
    const sorted = [...frames].sort((a, b) => a - b);
    const q = (p: number) => sorted[Math.min(sorted.length - 1, Math.floor(p * sorted.length))]!;
    return {
      frames: frames.length,
      meanMs: frames.reduce((a, b) => a + b, 0) / frames.length,
      medianMs: q(0.5),
      p95Ms: q(0.95),
      maxMs: sorted[sorted.length - 1]!,
      maxCells,
      domNodes: document.getElementsByTagName('*').length,
      filledMs,
    };
  });
  const summary = { importMs: Math.round(importMs), ...stats };
  console.log('1000x1000 scroll:', JSON.stringify(summary));
  await info.attach('scroll-stats.json', { body: JSON.stringify(summary, null, 2), contentType: 'application/json' });
  expect(stats.maxCells).toBeLessThan(3000);
  expect(stats.filledMs).toBeGreaterThan(0);
  expect(stats.filledMs).toBeLessThan(1000);
  expect(stats.p95Ms).toBeLessThan(50);
});
