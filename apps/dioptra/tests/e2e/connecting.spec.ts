// Paired data → paired t test → the default graph is bars with connecting lines → means
// only → export SVG and PDF → save → reopen: the same graph.
import { readFileSync } from 'node:fs';
import { expect, test, type Page } from '@playwright/test';
import { boot, cell, clickCell, menu, mod, newColumnTable, pasteIntoGrid } from './helpers';

test.beforeEach(async ({ page }) => {
  await boot(page);
});

const graphSvg = (page: Page) => page.evaluate(() => document.querySelector('[data-testid="graph-svg"] svg')?.outerHTML ?? '');
const count = (page: Page, prefix: string) =>
  page.evaluate((p) => document.querySelectorAll(`[data-testid="graph-svg"] [id^="${p}"]`).length, prefix);

test('(c1) a paired t test graphs with connecting lines; means only; export; save and reopen', async ({ page }, info) => {
  await newColumnTable(page, 'column');
  await clickCell(page, 0, 0);
  // Six subjects; the fourth has no "after" value.
  await pasteIntoGrid(page, '120\t111\n131\t122\n118\t116\n142\t\n109\t104\n125\t117');
  await expect(cell(page, 5, 1)).toHaveText('117');

  await page.getByTestId('tool-analyze').click();
  await page.getByTestId('analysis-t_test').click();
  await page.getByTestId('opt-variant').getByRole('button').first().click();
  await page.getByRole('option', { name: /^Paired/ }).click();
  await page.getByTestId('analysis-run').click();
  await expect(page.getByTestId('sheet-summary')).toContainText('t(');

  // The sheet offers its natural graph: bars with a line per subject.
  const make = page.getByTestId('sheet-make-graph');
  await expect(make).toContainText('Graph with connecting lines');
  await make.click();
  await expect(page.locator('[data-testid="graph-svg"] svg')).toBeVisible();
  await expect(page.getByTestId('tb-family')).toHaveAttribute('data-value', 'column');
  await expect.poll(() => count(page, 'subject-')).toBe(5);
  await expect.poll(() => count(page, 'connect-summary')).toBe(0);

  // A subject line is a target: clicking it opens the connecting-line controls.
  await page.getByTestId('target-subject-1').click({ force: true });
  await expect(page.getByTestId('graph-selection')).toHaveAttribute('data-target', '/options/connecting');
  await expect(page.getByTestId('graph-selection').getByTestId('connecting-fields')).toBeVisible();

  // Means only, from the Connecting lines section.
  const section = page.getByTestId('insp-graph-connecting');
  await section.getByTestId('conn-means').click();
  await expect.poll(() => count(page, 'subject-')).toBe(0);
  await expect.poll(() => count(page, 'connect-summary')).toBe(1);
  // Undo brings the subject lines back; redo removes them again.
  await page.getByTestId('graph-view').click({ position: { x: 5, y: 5 } });
  await page.keyboard.press(`${mod}+z`);
  await expect.poll(() => count(page, 'subject-')).toBe(5);
  await page.keyboard.press(`${mod}+Shift+z`);
  await expect.poll(() => count(page, 'connect-summary')).toBe(1);
  const before = await graphSvg(page);

  // Export SVG and PDF.
  await page.getByTestId('tb-export').click();
  const dlg = page.getByTestId('graph-export-dialog');
  await dlg.getByRole('radio', { name: 'SVG' }).click();
  const [svgDl] = await Promise.all([page.waitForEvent('download'), page.getByTestId('export-go').click()]);
  const svg = readFileSync((await svgDl.path())!, 'utf8');
  expect(svg).toContain('id="connect-summary"');
  expect(svg).not.toContain('id="subject-');
  await page.getByTestId('tb-export').click();
  await dlg.getByRole('radio', { name: 'PDF' }).click();
  const [pdfDl] = await Promise.all([page.waitForEvent('download'), page.getByTestId('export-go').click()]);
  expect(pdfDl.suggestedFilename()).toMatch(/\.pdf$/);
  expect(readFileSync((await pdfDl.path())!).subarray(0, 7).toString()).toBe('%PDF-1.');

  // Save, start over, reopen: the same drawing.
  const download = page.waitForEvent('download');
  await page.keyboard.press(`${mod}+s`);
  const file = await download;
  const path = info.outputPath(file.suggestedFilename());
  await file.saveAs(path);
  await menu(page, 'File', 'file.new');
  const chooser = page.waitForEvent('filechooser');
  await menu(page, 'File', 'file.open');
  await (await chooser).setFiles(path);
  await page.locator('[data-testid^="rail-item-Graph of"]').first().click();
  await expect(page.locator('[data-testid="graph-svg"] svg')).toBeVisible();
  await expect.poll(() => count(page, 'connect-summary')).toBe(1);
  expect(await graphSvg(page)).toBe(before);
});

test('(c2) connecting lines on a column graph: subjects, colour by data set, horizontal', async ({ page }) => {
  await newColumnTable(page, 'column');
  await clickCell(page, 0, 0);
  await pasteIntoGrid(page, '1\t2\t3\n2\t3\t5\n3\t3.5\t4');
  await page.getByTestId('tool-graph').click();
  await page.getByTestId('family-column').click();
  await page.getByTestId('new-graph-create').click();
  await expect(page.locator('[data-testid="graph-svg"] svg')).toBeVisible();
  const section = page.getByTestId('insp-graph-connecting');
  await expect(section).toBeVisible();
  await section.getByTestId('conn-both').click();
  await expect.poll(() => count(page, 'subject-')).toBe(3);
  await expect.poll(() => count(page, 'connect-summary')).toBe(1);
  await section.getByTestId('conn-colour-by').click();
  await page.getByTestId('conn-colour-by-option-dataset').click();
  // Segments now take the data-set colours: more than one stroke colour among the lines.
  await expect
    .poll(() =>
      page.evaluate(
        () => new Set([...document.querySelectorAll('[data-testid="graph-svg"] [id^="subject-"] polyline')].map((p) => p.getAttribute('stroke'))).size,
      ),
    )
    .toBeGreaterThan(1);
  // Horizontal: the category labels move to the left axis.
  await page.getByTestId('insp-graph-bars').locator('.insp-head').click();
  await page.getByTestId('bar-orientation').getByRole('radio', { name: 'Horizontal' }).click();
  await expect
    .poll(() => page.evaluate(() => document.querySelector('[data-testid="graph-svg"] #axis-y-labels')?.textContent ?? ''))
    .toContain('Data set');
  // A family without connecting lines hides the section.
  await page.getByTestId('tb-family').click();
  await page.getByTestId('tb-family-option-histogram').click();
  await expect(page.getByTestId('insp-graph-connecting')).toHaveCount(0);
});
