import { readFileSync } from 'node:fs';
import { expect, test, type Page } from '@playwright/test';
import { boot, newGraphOf, threeGroupTable } from './helpers';

// The test key from crates/dioptra-license: valid through January 2026.
const TEST_KEY =
  'DIOPTRA-LAB-1.eyJpZCI6IlRFU1QtMDAwMSIsImxpY2Vuc2VlIjoiRGlvcHRyYSB0ZXN0IHN1aXRlIiwiZW1haWwiOiJ0ZXN0QGV4YW1wbGUub3JnIiwicGxhbiI6InRyaWFsIiwic2VhdHMiOjEsImlzc3VlZCI6IjIwMjYtMDEtMDEiLCJleHBpcmVzIjoiMjAyNi0wMS0zMSJ9.NQ7t5Q4k8yw4hlyf1WHdIy6-YCV5c9-WIYRRX0RjWOqb_9wb72J-dNNA39sm9GGc0XqGDnmgSXhteBCose36Cw';

async function withLab(page: Page): Promise<void> {
  await page.clock.setFixedTime(new Date('2026-01-15T12:00:00'));
  await page.addInitScript((k) => localStorage.setItem('dioptra.lab.key', k), TEST_KEY);
}

test('(a) without Lab the Journal control says what it does and opens Dioptra Lab', async ({ page }) => {
  await boot(page);
  await threeGroupTable(page);
  await newGraphOf(page, 'column');
  await page.getByTestId('graph-journal').click();
  await expect(page.getByTestId('journal-lab-off')).toContainText('Size figures for a journal');
  await page.getByTestId('journal-lab-open').click();
  await expect(page.getByTestId('lab-dialog')).toBeVisible();
});

test('(b) a preset sizes the graph and the figure check lists what is left', async ({ page }) => {
  await withLab(page);
  await boot(page);
  await threeGroupTable(page);
  await newGraphOf(page, 'column');
  await page.getByTestId('graph-journal').click();
  const panel = page.getByTestId('journal-panel');
  await expect(panel.getByTestId('journal-preset')).toHaveAttribute('data-value', 'nature');
  // The default 85 mm figure is not a Nature width.
  await expect(panel.getByTestId('figure-issue-size')).toBeVisible();
  await panel.getByTestId('journal-apply').click();
  await expect(page.getByTestId('graph-size-mm')).toContainText('89 ×');
  await expect(panel.getByTestId('figure-check-summary')).not.toContainText(/\d+ to fix/);
  // Text too small: the issue selects the element.
  await panel.getByTestId('journal-preset').click();
  await page.getByTestId('journal-preset-option-plos').click();
  await expect(panel.locator('[data-testid="figure-issue-font_size"][data-severity="error"]').first()).toBeVisible();
  await panel.locator('[data-testid="figure-issue-font_size"] button').first().click();
  await expect(page.locator('.tg.is-selected')).toHaveCount(1);
});

test('(c) a layout at final size: lower-case letters and one page width', async ({ page }) => {
  await withLab(page);
  await boot(page);
  await threeGroupTable(page);
  await newGraphOf(page, 'column');
  await page.getByTestId('tab-Table 1').click();
  await newGraphOf(page, 'box');
  await page.getByTestId('open-palette').click();
  await page.keyboard.type('New layout');
  await page.keyboard.press('Enter');
  await page.getByTestId('layout-create').click();
  await page.getByTestId('layout-journal').click();
  await page.getByTestId('layout-final-size').getByRole('radio', { name: 'a b c', exact: true }).click();
  await expect(page.locator('[data-testid="layout-svg"] #panel-1-letter')).toHaveText('b');
  const width = page.getByTestId('layout-final-size').getByTestId('layout-width');
  await width.fill('183');
  await width.press('Enter');
  await expect(page.locator('.layout-view .gf-size')).toContainText('183 ×');
});

test('(d) TIFF export and the project report download', async ({ page }) => {
  await withLab(page);
  await boot(page);
  await threeGroupTable(page);
  await newGraphOf(page, 'column');
  await page.getByTestId('tb-export').click();
  const dlg = page.getByTestId('graph-export-dialog');
  await dlg.getByRole('radio', { name: 'TIFF' }).click();
  await dlg.getByRole('radio', { name: '150 dpi' }).click();
  await expect(page.getByTestId('export-detail')).toContainText('502 × 413 pixels at 150 dpi');
  const [tif] = await Promise.all([page.waitForEvent('download'), page.getByTestId('export-go').click()]);
  expect(tif.suggestedFilename()).toBe('Graph of Table 1.tiff');
  expect([...readFileSync((await tif.path())!).subarray(0, 4)]).toEqual([0x49, 0x49, 0x2a, 0x00]);

  await page.getByTestId('app-menu').click();
  await page.getByTestId('menubar-File').click();
  await page.getByTestId('menu-file.exportReport').click();
  const [doc] = await Promise.all([page.waitForEvent('download'), page.getByRole('button', { name: 'Download' }).click()]);
  expect(doc.suggestedFilename()).toBe('Untitled report.docx');
  expect([...readFileSync((await doc.path())!).subarray(0, 2)]).toEqual([0x50, 0x4b]);
});
