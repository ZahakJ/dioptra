import { expect, test } from '@playwright/test';
import { boot, cell, clickCell, newColumnTable, menu } from './helpers';

test.beforeEach(async ({ page }) => {
  await boot(page);
});

test('imports a CSV file and exports the table as CSV', async ({ page }) => {
  await menu(page, 'File', 'file.import');
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Choose file' }).click();
  await (await chooser).setFiles({
    name: 'assay.csv',
    mimeType: 'text/csv',
    buffer: Buffer.from('Control,Treated\n1.5,2.5\n1.75,3.25\n2,n.d.\n'),
  });
  await expect(page.getByTestId('notice-report')).toContainText('Imported 3 × 2');
  await expect(page.getByTestId('rail-item-assay')).toBeVisible();
  await expect(cell(page, 1, 1)).toHaveText('3.25');
  await expect(cell(page, 2, 1)).toHaveText('n.d.');

  await menu(page, 'File', 'file.export');
  await expect(page.getByTestId('export-dialog')).toBeVisible();
  const download = page.waitForEvent('download');
  await page.getByRole('button', { name: 'Download' }).click();
  const file = await download;
  expect(file.suggestedFilename()).toMatch(/\.csv$/);
  const text = await (await import('node:fs/promises')).readFile(await file.path(), 'utf8');
  expect(text).toContain('1.75');
  expect(text).toContain('3.25');
});

test('pastes text as a new table with a suggested kind', async ({ page }) => {
  await menu(page, 'File', 'file.pasteNew');
  await page.getByTestId('paste-import-text').fill('Dose\tResp\n0.1\t2.5\n1\t3.9\n10\t7.2\n100\t9.9');
  await expect(page.getByTestId('kind-suggestion')).toContainText('XY');
  await page.getByTestId('paste-import-create').click();
  await expect(page.getByTestId('notice-report')).toContainText('Imported 4 × 2');
  await expect(page.getByTestId('col-head-0')).toContainText('X');
  await expect(cell(page, 3, 1)).toHaveText('9.9');
});

test('renames and deletes objects in the rail', async ({ page }) => {
  await newColumnTable(page);
  await page.getByTestId('rail-item-Table 1').dblclick();
  await page.getByTestId('rail-rename').fill('Weights');
  await page.getByTestId('rail-rename').press('Enter');
  await expect(page.getByTestId('rail-item-Weights')).toBeVisible();
  await expect(page.getByTestId('tab-Weights')).toBeVisible();
  await page.getByTestId('rail-item-Weights').focus();
  await page.keyboard.press('Delete');
  await expect(page.getByTestId('confirm-dialog')).toContainText('Delete "Weights"');
  await page.getByTestId('confirm-dialog').getByRole('button', { name: 'Delete' }).click();
  await expect(page.getByTestId('rail-item-Weights')).toHaveCount(0);
  await expect(page.getByTestId('empty-table.new')).toBeVisible();
});

test('header context menu renames a data set and sets decimals; widths persist', async ({ page }) => {
  await newColumnTable(page);
  await clickCell(page, 0, 0);
  await page.keyboard.type('1.23456');
  await page.keyboard.press('Enter');
  await page.getByTestId('col-head-0').click({ button: 'right' });
  await page.getByTestId('context-menu').getByText('Rename data set...').click();
  await page.getByTestId('prompt-dialog').locator('input').fill('Control');
  await page.getByTestId('prompt-dialog').locator('input').press('Enter');
  await expect(page.locator('.gh-group').first()).toHaveText('Control');
  await page.getByTestId('col-head-0').click({ button: 'right' });
  await page.getByTestId('context-menu').getByText('Decimals...').click();
  await page.getByTestId('prompt-dialog').locator('input').fill('2');
  await page.getByTestId('prompt-dialog').locator('input').press('Enter');
  await expect(cell(page, 0, 0)).toHaveText('1.23');
  // Resize column A by dragging its header edge.
  const handle = page.getByTestId('col-head-0').locator('.col-resize');
  const box = (await handle.boundingBox())!;
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  await page.mouse.down();
  await page.mouse.move(box.x + 80, box.y + box.height / 2, { steps: 4 });
  await page.mouse.up();
  const w = (await page.getByTestId('col-head-0').boundingBox())!.width;
  expect(w).toBeGreaterThan(150);
  const stored = await page.evaluate(() => localStorage.getItem('dioptra-ui') ?? '');
  expect(stored).toContain('colWidths');
});

test('F2 edits in place and Escape cancels; the inspector shows provenance', async ({ page }) => {
  await newColumnTable(page);
  await clickCell(page, 0, 0);
  await page.keyboard.type('5');
  await page.keyboard.press('Enter');
  await clickCell(page, 0, 0);
  await page.keyboard.press('F2');
  await expect(page.getByTestId('cell-editor')).toHaveValue('5');
  await page.keyboard.type('0');
  await page.keyboard.press('Escape');
  await expect(page.getByTestId('cell-editor')).toHaveCount(0);
  await expect(cell(page, 0, 0)).toHaveText('5');
  await cell(page, 0, 0).dblclick();
  await page.keyboard.type('5');
  await page.keyboard.press('Enter');
  await expect(cell(page, 0, 0)).toHaveText('55');
  await clickCell(page, 0, 0);
  await expect(page.getByTestId('cell-info')).toContainText('Edited by hand');
});
