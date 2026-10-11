import { expect, test } from '@playwright/test';
import { analysisImplemented, boot, clickCell, mod, threeGroupTable } from './helpers';

test.beforeEach(async ({ page }) => {
  await boot(page);
});

test('(a) Analyze a three-group column table: descriptive statistics show the means', async ({ page }) => {
  await threeGroupTable(page);
  await page.keyboard.press(`${mod}+Shift+A`);
  const dialog = page.getByTestId('analyze-dialog');
  await expect(dialog).toBeVisible();
  // Fitting analyses come first with a sentence; unfit ones say why.
  await expect(page.getByTestId('analysis-descriptive')).toContainText('Mean, SD, SEM');
  await expect(page.getByTestId('analysis-chi_square')).toHaveAttribute('aria-disabled', 'true');
  await expect(page.getByTestId('analysis-chi_square')).toContainText('contingency');
  await page.getByTestId('analysis-descriptive').click();
  await expect(page.getByTestId('options-panel')).toContainText('Confidence level');
  await page.getByTestId('analysis-run').click();

  const sheet = page.getByTestId('results-sheet');
  await expect(sheet).toBeVisible();
  // The table stays beside the sheet.
  await expect(page.getByTestId('split-view')).toBeVisible();
  await expect(page.getByTestId('rail-item-Descriptive statistics of Table 1')).toBeVisible();
  const t = page.getByTestId('results-table-0');
  await expect(t.locator('th').nth(2)).toHaveText('Mean');
  await expect(t.locator('tbody tr').nth(0).locator('td').nth(2)).toHaveText('2');
  await expect(t.locator('tbody tr').nth(1).locator('td').nth(2)).toHaveText('5');
  await expect(t.locator('tbody tr').nth(2).locator('td').nth(2)).toHaveText('8.167');
  await expect(page.getByTestId('sheet-assumptions')).toContainText('Assumptions');
  await expect(page.getByTestId('sheet-summary')).toContainText('M = 8.167');

  // Copy a cell: full precision, not the four digits shown.
  await t.locator('tbody tr').nth(2).locator('td').nth(2).click();
  const copied = await page.evaluate(() => {
    const dt = new DataTransfer();
    const ev = new ClipboardEvent('copy', { clipboardData: dt, bubbles: true, cancelable: true });
    document.querySelector('[data-testid="results-table-0"]')!.dispatchEvent(ev);
    return dt.getData('text/plain');
  });
  expect(copied).toBe('8.166666666666666');

  // Changing the data refreshes the sheet.
  await page.getByTestId('tab-Table 1').click();
  await clickCell(page, 0, 0);
  await page.keyboard.type('4');
  await page.keyboard.press('Enter');
  await page.getByTestId('tab-Descriptive statistics of Table 1').click();
  await expect(page.getByTestId('results-table-0').locator('tbody tr').nth(0).locator('td').nth(2)).toHaveText('3');

  // Options reopen with the current spec and update the analysis.
  await page.getByTestId('sheet-options').click();
  await expect(page.getByTestId('analysis-options')).toBeVisible();
  await page.getByTestId('analysis-options').getByRole('textbox', { name: 'Confidence level' }).fill('90');
  await page.getByTestId('analysis-options').getByRole('textbox', { name: 'Confidence level' }).press('Tab');
  await page.getByTestId('analysis-run').click();
  await expect(page.getByTestId('results-table-0').locator('th').nth(5)).toHaveText('Lower 90% CI of mean');
});

test('(g) t test on two columns: the summary reports t(df) and a p value', async ({ page }) => {
  await threeGroupTable(page);
  test.skip(
    !(await analysisImplemented(page, 't_test')),
    'The statistics glue (crates/dioptra-core analysis glue) has not landed in this engine build: the t test sheet still says "not available in this build yet".',
  );
  await page.getByTestId('tool-analyze').click();
  await page.getByTestId('analysis-t_test').click();
  await expect(page.getByTestId('options-panel')).toContainText('Welch');
  await page.getByTestId('analysis-run').click();
  const summary = page.getByTestId('sheet-summary');
  await expect(summary).toContainText('t(');
  await expect(summary).toContainText(/\bp\s*[=<>]\s*\.?\d/);
});

test('(h) one-way ANOVA with Tukey: a multiple-comparisons table', async ({ page }) => {
  await threeGroupTable(page);
  test.skip(
    !(await analysisImplemented(page, 'one_way_anova')),
    'The statistics glue (crates/dioptra-core analysis glue) has not landed in this engine build: one-way ANOVA still says "not available in this build yet".',
  );
  await page.getByTestId('tool-analyze').click();
  await page.getByTestId('analysis-one_way_anova').click();
  await expect(page.getByTestId('posthoc-method')).toHaveValue('tukey');
  await page.getByTestId('analysis-run').click();
  await expect(page.getByTestId('results-sheet')).toBeVisible();
  await expect(page.locator('.sheet-section h2', { hasText: /compar/i })).toBeVisible();
  // Three groups: three pairwise comparisons.
  const sec = page.locator('.sheet-section', { has: page.locator('h2', { hasText: /compar/i }) });
  await expect(sec.locator('tbody tr')).toHaveCount(3);
});
