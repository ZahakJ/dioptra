import { readFileSync } from 'node:fs';
import { expect, test, type Page } from '@playwright/test';
import { boot, mod, newGraphOf, threeGroupTable } from './helpers';

test.beforeEach(async ({ page }) => {
  await boot(page);
});

const svgText = (page: Page, id: string) =>
  page.evaluate((i) => document.querySelector(`[data-testid="graph-svg"] #${i}`)?.textContent ?? null, id);

/** Click a target box near its right edge (the axis line rather than the labels on top of it). */
async function clickTargetEdge(page: Page, id: string): Promise<void> {
  const b = (await page.getByTestId(`target-${id}`).boundingBox())!;
  await page.mouse.click(b.x + b.width - 3, b.y + b.height / 2);
}

test('(b) select the y axis through the overlay, edit its title in the inspector, the SVG follows', async ({ page }) => {
  await threeGroupTable(page);
  await newGraphOf(page, 'column');
  await expect(page.getByTestId('rail-item-Graph of Table 1')).toBeVisible();
  await clickTargetEdge(page, 'axis-y');
  const sel = page.getByTestId('graph-selection');
  await expect(sel).toHaveAttribute('data-target', /^\/axes\/y/);
  await sel.getByTestId('axis-title').fill('Response (pg/mL)');
  await expect.poll(() => svgText(page, 'axis-y-title')).toBe('Response (pg/mL)');
  // Now the title is its own target.
  await page.getByTestId('target-axis-y-title').click();
  await expect(sel).toHaveAttribute('data-target', '/axes/y/title');
  await sel.getByTestId('axis-title').fill('IL-6 (pg/mL)');
  await expect.poll(() => svgText(page, 'axis-y-title')).toBe('IL-6 (pg/mL)');
  // Double-click edits the text in place.
  await page.getByTestId('target-axis-y-title').dblclick();
  await page.getByTestId('inline-edit').fill('Interleukin 6');
  await page.getByTestId('inline-edit').press('Enter');
  await expect.poll(() => svgText(page, 'axis-y-title')).toBe('Interleukin 6');
});

test('(c) theme night and family box', async ({ page }) => {
  await threeGroupTable(page);
  await newGraphOf(page, 'column');
  await page.getByTestId('tb-theme').selectOption('night');
  await expect
    .poll(() => page.evaluate(() => document.querySelector('[data-testid="graph-svg"] #background')?.getAttribute('fill')))
    .toBe('#16191a');
  await page.getByTestId('tb-family').selectOption('box');
  await expect(page.getByTestId('rail-item-Graph of Table 1')).toContainText('box');
  await expect(page.getByTestId('tb-family')).toHaveValue('box');
  // The theme survived the family switch.
  await expect(page.getByTestId('tb-theme')).toHaveValue('night');
});

test('(d) export SVG and PNG downloads', async ({ page }) => {
  await threeGroupTable(page);
  await newGraphOf(page, 'column');

  await page.getByTestId('tb-export').click();
  const dlg = page.getByTestId('graph-export-dialog');
  await dlg.getByRole('radio', { name: 'SVG' }).click();
  await expect(page.getByTestId('export-detail')).toContainText('Graph of Table 1.svg');
  const [svgDl] = await Promise.all([page.waitForEvent('download'), page.getByTestId('export-go').click()]);
  expect(svgDl.suggestedFilename()).toBe('Graph of Table 1.svg');
  const svg = readFileSync((await svgDl.path())!, 'utf8');
  expect(svg.startsWith('<svg')).toBe(true);
  expect(svg).toContain('width="85mm"');

  await page.getByTestId('tb-export').click();
  await dlg.getByRole('radio', { name: 'PNG' }).click();
  await dlg.getByRole('radio', { name: '150 dpi' }).click();
  await expect(page.getByTestId('export-detail')).toContainText('502 × 413 pixels at 150 dpi');
  const [pngDl] = await Promise.all([page.waitForEvent('download'), page.getByTestId('export-go').click()]);
  expect(pngDl.suggestedFilename()).toBe('Graph of Table 1.png');
  const png = readFileSync((await pngDl.path())!);
  expect([...png.subarray(0, 8)]).toEqual([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
});

test('(e) a layout of two graphs renders with panel letters', async ({ page }) => {
  await threeGroupTable(page);
  await newGraphOf(page, 'column');
  await page.getByTestId('tab-Table 1').click();
  await newGraphOf(page, 'box');
  await page.getByTestId('open-palette').click();
  await page.keyboard.type('New layout');
  await page.keyboard.press('Enter');
  const dlg = page.getByTestId('layout-dialog');
  await expect(dlg).toBeVisible();
  await expect(dlg.getByTestId('layout-pick-Graph of Table 1')).toBeChecked();
  await expect(dlg.getByTestId('layout-pick-Graph of Table 1 2')).toBeChecked();
  await page.getByTestId('layout-create').click();
  await expect(page.getByTestId('layout-view')).toBeVisible();
  await expect(page.locator('[data-testid="layout-svg"] svg')).toBeVisible();
  await expect(page.getByTestId('layout-panel-0')).toBeVisible();
  await expect(page.getByTestId('layout-panel-1')).toBeVisible();
  const letters = await page.evaluate(() =>
    ['panel-0-letter', 'panel-1-letter'].map((i) => document.querySelector(`[data-testid="layout-svg"] #${i}`)?.textContent),
  );
  expect(letters).toEqual(['A', 'B']);
});

test('(f) undo removes the last graph edit', async ({ page }) => {
  await threeGroupTable(page);
  await newGraphOf(page, 'column');
  await page.getByTestId('g-title').fill('Undo me');
  await expect.poll(() => svgText(page, 'title')).toBe('Undo me');
  // Leave the text field, then undo from the keyboard.
  await page.getByTestId('graph-view').click({ position: { x: 5, y: 5 } });
  await page.keyboard.press(`${mod}+z`);
  await expect.poll(() => svgText(page, 'title')).toBeNull();
  await expect(page.getByTestId('g-title')).toHaveValue('');
  await page.keyboard.press(`${mod}+Shift+z`);
  await expect.poll(() => svgText(page, 'title')).toBe('Undo me');
});

test('(e3) a layout changes its columns, survives save and reopen, and opens a panel graph', async ({ page }, info) => {
  await threeGroupTable(page);
  await newGraphOf(page, 'column');
  await page.getByTestId('tab-Table 1').click();
  await newGraphOf(page, 'box');
  await page.getByTestId('open-palette').click();
  await page.keyboard.type('New layout');
  await page.keyboard.press('Enter');
  await page.getByTestId('layout-create').click();
  await expect(page.getByTestId('layout-panel-1')).toBeVisible();
  const box = async (i: number) => (await page.getByTestId(`layout-panel-${i}`).boundingBox())!;
  // Two columns: side by side. One column: stacked.
  await expect(page.getByTestId('layout-columns')).toHaveValue('2');
  expect((await box(1)).x).toBeGreaterThan((await box(0)).x + 10);
  await page.getByTestId('layout-columns').selectOption('1');
  await expect.poll(async () => (await box(1)).y > (await box(0)).y + 10).toBe(true);
  // Save, start over, reopen: the layout knows its graphs from the engine.
  const download = page.waitForEvent('download');
  await page.keyboard.press(`${mod}+s`);
  const file = await download;
  const path = info.outputPath(file.suggestedFilename());
  await file.saveAs(path);
  await page.getByTestId('menubar-File').click();
  await page.getByTestId('menu-file.new').click();
  await expect(page.getByTestId('rail-item-Layout 1')).toHaveCount(0);
  const chooser = page.waitForEvent('filechooser');
  await page.getByTestId('menubar-File').click();
  await page.getByTestId('menu-file.open').click();
  await (await chooser).setFiles(path);
  await page.getByTestId('rail-item-Layout 1').click();
  await expect(page.getByTestId('layout-columns')).toHaveValue('1');
  await page.getByTestId('layout-panel-1').dblclick();
  await expect(page.getByTestId('tb-family')).toHaveValue('box');
});

test('(f2) typing a title with pauses is one undo step', async ({ page }) => {
  await threeGroupTable(page);
  await newGraphOf(page, 'column');
  const field = page.getByTestId('g-title');
  await field.click();
  // Pauses longer than the 150 ms debounce: one patch per word reaches the engine.
  for (const w of ['Plasma', ' IL-6', ' levels']) {
    await field.pressSequentially(w);
    await page.waitForTimeout(300);
  }
  await expect.poll(() => svgText(page, 'title')).toBe('Plasma IL-6 levels');
  await page.getByTestId('graph-view').click({ position: { x: 5, y: 5 } });
  await page.keyboard.press(`${mod}+z`);
  await expect.poll(() => svgText(page, 'title')).toBeNull();
  await expect(field).toHaveValue('');
});
