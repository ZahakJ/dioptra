import { expect, type Page } from '@playwright/test';

export async function boot(page: Page): Promise<void> {
  await page.goto('/');
  await expect(page.getByTestId('app')).toHaveAttribute('data-ready', 'true', { timeout: 60_000 });
}

/** New table through the dialog. */
export async function newColumnTable(page: Page, kind = 'column'): Promise<void> {
  await page.getByTestId('rail-new-table').click();
  await expect(page.getByTestId('new-table-dialog')).toBeVisible();
  await page.getByTestId(`kind-${kind}`).click();
  await page.getByTestId('new-table-create').click();
  await expect(page.getByTestId('grid')).toBeVisible();
}

export const cell = (page: Page, row: number, col: number) => page.locator(`[data-cell="${row}:${col}"]`);

export async function clickCell(page: Page, row: number, col: number): Promise<void> {
  await cell(page, row, col).click();
  await expect(page.getByTestId('active-cell')).toHaveAttribute('data-row', String(row));
  await expect(page.getByTestId('active-cell')).toHaveAttribute('data-col', String(col));
}

/** Paste text into the grid through a synthetic ClipboardEvent, as Ctrl+V would. */
export async function pasteIntoGrid(page: Page, text: string): Promise<void> {
  await page.evaluate((t) => {
    const dt = new DataTransfer();
    dt.setData('text/plain', t);
    const ev = new ClipboardEvent('paste', { clipboardData: dt, bubbles: true, cancelable: true });
    document.querySelector('[data-grid]')!.dispatchEvent(ev);
  }, text);
}

export const mod = process.platform === 'darwin' ? 'Meta' : 'Control';

/** A column table "Table 1" with three groups of three values (means 2, 5 and 8.1667). */
export async function threeGroupTable(page: Page): Promise<void> {
  await newColumnTable(page, 'column');
  await clickCell(page, 0, 0);
  await pasteIntoGrid(page, '1\t4\t7\n2\t5\t8\n3\t6\t9.5');
  await expect(cell(page, 2, 2)).toHaveText('9.5');
}

/** New graph of the active table in a family, through the Graph tool button. */
export async function newGraphOf(page: Page, family = 'column'): Promise<void> {
  await page.getByTestId('tool-graph').click();
  await expect(page.getByTestId('new-graph-dialog')).toBeVisible();
  await page.getByTestId(`family-${family}`).click();
  await page.getByTestId('new-graph-create').click();
  await expect(page.locator('[data-testid="graph-svg"] svg')).toBeVisible();
}
