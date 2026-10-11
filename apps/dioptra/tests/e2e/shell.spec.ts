import { expect, test } from '@playwright/test';
import { boot, cell, clickCell, mod, newColumnTable, pasteIntoGrid } from './helpers';

test.beforeEach(async ({ page }) => {
  await boot(page);
});

test('(a) creates a column table and types values that show formatted', async ({ page }) => {
  await expect(page.getByTestId('engine-backend')).toContainText('WebAssembly engine');
  await newColumnTable(page, 'column');
  await expect(page.getByTestId('rail-item-Table 1')).toBeVisible();
  await expect(page.getByTestId('col-head-0')).toContainText('A');
  await clickCell(page, 0, 0);
  await page.keyboard.type('3.14159265358979');
  await page.keyboard.press('Enter');
  await page.keyboard.type('2');
  await page.keyboard.press('Enter');
  await page.keyboard.type('<LOD');
  await page.keyboard.press('Tab');
  await expect(cell(page, 0, 0)).toHaveText('3.14159');
  await expect(cell(page, 1, 0)).toHaveText('2');
  await expect(cell(page, 2, 0)).toHaveText('<LOD');
  await expect(cell(page, 2, 0)).toHaveClass(/gc-text/);
  // The formula bar shows the value at full precision.
  await clickCell(page, 0, 0);
  await expect(page.getByTestId('formula-input')).toHaveValue('3.14159265358979');
  // Status line summary over a range.
  await page.keyboard.press('Shift+ArrowDown');
  await expect(page.getByTestId('sum-n')).toContainText('2');
  await expect(page.getByTestId('sum-sum')).toContainText('5.14159');
  // Exclude marks the cell (struck through), the summary drops it.
  await clickCell(page, 1, 0);
  await page.keyboard.press(`${mod}+e`);
  await expect(cell(page, 1, 0)).toHaveClass(/gc-x/);
  await expect(page.getByTestId('insp-cell')).toContainText('excluded');
});

test('(b) pastes a 50 x 3 TSV block, reports it and shows the cells', async ({ page }) => {
  await newColumnTable(page, 'column');
  await clickCell(page, 0, 0);
  const rows = Array.from({ length: 50 }, (_, i) => [`${i + 1},5`, `${(i + 1) * 2},25`, i === 7 ? 'n.d.' : `${i},0`].join('\t'));
  await pasteIntoGrid(page, rows.join('\n'));
  const notice = page.getByTestId('notice-report');
  await expect(notice).toContainText('Pasted 50 × 3');
  await expect(notice).toContainText('tab-separated');
  await expect(notice).toContainText('decimal comma');
  await expect(notice).toContainText('1 text cell');
  await expect(cell(page, 0, 0)).toHaveText('1.5');
  await expect(cell(page, 7, 2)).toHaveText('n.d.');
  await expect(page.getByTestId('formula-address')).toHaveText('A1:C50');
  // Ctrl+Down runs to the end of the data and scrolls it into view.
  await clickCell(page, 0, 1);
  await page.keyboard.press(`${mod}+ArrowDown`);
  await expect(page.getByTestId('active-cell')).toHaveAttribute('data-row', '49');
  await expect(cell(page, 49, 1)).toHaveText('100.25');
  await page.keyboard.press(`${mod}+Home`);
  await expect(cell(page, 0, 0)).toBeVisible();
  // Copy writes TSV at full precision (1,5 was read as 1.5).
  await clickCell(page, 0, 0);
  await page.keyboard.press('Shift+ArrowRight');
  await page.keyboard.press('Shift+ArrowDown');
  const copied = await page.evaluate(() => {
    const dt = new DataTransfer();
    const ev = new ClipboardEvent('copy', { clipboardData: dt, bubbles: true, cancelable: true });
    document.querySelector('[data-grid]')!.dispatchEvent(ev);
    return dt.getData('text/plain');
  });
  expect(copied).toBe('1.5\t2.25\n2.5\t4.25');
  // The notice's Undo removes the paste.
  await notice.getByTestId('notice-action').click();
  await expect(cell(page, 0, 0)).toHaveText('');
});

test('(c) a column formula =log10(A) on B recomputes after editing A', async ({ page }) => {
  await newColumnTable(page, 'column');
  await clickCell(page, 0, 0);
  for (const v of ['10', '100', '1000']) {
    await page.keyboard.type(v);
    await page.keyboard.press('Enter');
  }
  await clickCell(page, 0, 1);
  await page.getByTestId('formula-mode-column').click();
  await page.getByTestId('formula-input').fill('=log10(A)');
  await page.getByTestId('formula-input').press('Enter');
  await expect(cell(page, 0, 1)).toHaveText('1');
  await expect(cell(page, 2, 1)).toHaveText('3');
  await expect(cell(page, 0, 1)).toHaveClass(/gc-f/);
  await expect(page.getByTestId('col-head-1')).toContainText('fx');
  await clickCell(page, 0, 0);
  await page.keyboard.type('10000');
  await page.keyboard.press('Enter');
  await expect(cell(page, 0, 1)).toHaveText('4');
  // Text in A gives an error in that row only.
  await clickCell(page, 1, 0);
  await page.keyboard.type('abc');
  await page.keyboard.press('Enter');
  await expect(cell(page, 1, 1)).toHaveClass(/gc-e/);
  await expect(cell(page, 2, 1)).toHaveText('3');
});

test('(d) undo and redo restore cells', async ({ page }) => {
  await newColumnTable(page, 'column');
  await clickCell(page, 0, 0);
  await page.keyboard.type('42');
  await page.keyboard.press('Enter');
  await page.keyboard.type('43');
  await page.keyboard.press('Enter');
  await expect(cell(page, 1, 0)).toHaveText('43');
  await page.keyboard.press(`${mod}+z`);
  await expect(cell(page, 1, 0)).toHaveText('');
  await expect(cell(page, 0, 0)).toHaveText('42');
  await page.keyboard.press(`${mod}+Shift+z`);
  await expect(cell(page, 1, 0)).toHaveText('43');
  // Clear, fill down, and undo of both.
  await clickCell(page, 0, 0);
  await page.keyboard.press('Shift+ArrowDown');
  await page.keyboard.press('Shift+ArrowDown');
  await page.keyboard.press(`${mod}+d`);
  await expect(cell(page, 2, 0)).toHaveText('42');
  await page.keyboard.press('Delete');
  await expect(cell(page, 0, 0)).toHaveText('');
  await page.keyboard.press(`${mod}+z`);
  await expect(cell(page, 0, 0)).toHaveText('42');
  await page.keyboard.press(`${mod}+z`);
  await expect(cell(page, 2, 0)).toHaveText('');
});

test('(e) saves the project as bytes and reopens it with values intact', async ({ page }, info) => {
  await newColumnTable(page, 'column');
  await clickCell(page, 0, 0);
  await page.keyboard.type('1.25');
  await page.keyboard.press('Enter');
  await page.keyboard.type('treated');
  await page.keyboard.press('Enter');
  await expect(cell(page, 1, 0)).toHaveText('treated');
  const download = page.waitForEvent('download');
  await page.keyboard.press(`${mod}+s`);
  const file = await download;
  expect(file.suggestedFilename()).toMatch(/\.dioptra$/);
  const path = info.outputPath(file.suggestedFilename());
  await file.saveAs(path);
  await expect(page.getByTestId('rail-item-Table 1')).toBeVisible();
  // New project (saved, so no guard), then open the file again.
  await page.getByTestId('menubar-File').click();
  await page.getByTestId('menu-file.new').click();
  await expect(page.getByTestId('rail-item-Table 1')).toHaveCount(0);
  const chooser = page.waitForEvent('filechooser');
  await page.getByTestId('menubar-File').click();
  await page.getByTestId('menu-file.open').click();
  await (await chooser).setFiles(path);
  await expect(page.getByTestId('rail-item-Table 1')).toBeVisible();
  await expect(cell(page, 0, 0)).toHaveText('1.25');
  await expect(cell(page, 1, 0)).toHaveText('treated');
  await expect(page).toHaveTitle(/^Dioptra — /);
});

test('(e2) the unsaved-changes guard asks before New project', async ({ page }) => {
  await newColumnTable(page, 'column');
  await page.getByTestId('menubar-File').click();
  await page.getByTestId('menu-file.new').click();
  await expect(page.getByTestId('confirm-dialog')).toBeVisible();
  await page.getByTestId('confirm-dialog').getByRole('button', { name: 'Cancel' }).click();
  await expect(page.getByTestId('rail-item-Table 1')).toBeVisible();
});

test('(f) the dark theme toggle changes data-theme', async ({ page }) => {
  const html = page.locator('html');
  await expect(html).toHaveAttribute('data-theme', 'light');
  await page.keyboard.press(`${mod}+Shift+l`);
  await expect(html).toHaveAttribute('data-theme', 'dark');
  await page.getByTestId('menubar-View').click();
  await page.getByTestId('menu-view.themeLight').click();
  await expect(html).toHaveAttribute('data-theme', 'light');
  // Persisted across reloads.
  await page.keyboard.press(`${mod}+Shift+l`);
  await page.reload();
  await expect(html).toHaveAttribute('data-theme', 'dark');
});

test('(g) the command palette opens with Ctrl-K and runs New table', async ({ page }) => {
  await page.keyboard.press(`${mod}+k`);
  await expect(page.getByTestId('palette')).toBeVisible();
  await page.getByTestId('palette-input').fill('new tabl');
  await expect(page.getByTestId('palette-item-table.new')).toHaveAttribute('aria-selected', 'true');
  await page.keyboard.press('Enter');
  await expect(page.getByTestId('palette')).toHaveCount(0);
  await expect(page.getByTestId('new-table-dialog')).toBeVisible();
  await page.getByTestId('kind-xy').click();
  await page.getByTestId('new-table-create').click();
  await expect(page.getByTestId('rail-item-Table 1')).toBeVisible();
  await expect(page.getByTestId('col-head-0')).toContainText('X');
  // Keyboard navigation within the palette.
  await page.keyboard.press(`${mod}+k`);
  await page.getByTestId('palette-input').fill('toggle dark');
  await page.keyboard.press('Enter');
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  await page.keyboard.press(`${mod}+k`);
  await page.keyboard.press('Escape');
  await expect(page.getByTestId('palette')).toHaveCount(0);
});
