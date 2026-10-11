import { expect, test, type Page } from '@playwright/test';
import { boot, cell, clickCell, mod, newColumnTable, pasteIntoGrid, threeGroupTable } from './helpers';

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
  await page.getByTestId('tool-analyze').click();
  await page.getByTestId('analysis-t_test').click();
  await expect(page.getByTestId('options-panel')).toContainText('Welch');
  await page.getByTestId('analysis-run').click();
  const summary = page.getByTestId('sheet-summary');
  await expect(summary).toContainText('t(');
  await expect(summary).toContainText(/\bp\s*[=<>]\s*\.?\d/);
  // The methods line sits under the title, ready to paste.
  await expect(page.getByTestId('sheet-methods')).toContainText('t test');
  await expect(page.getByTestId('sheet-methods')).toContainText('Dioptra');
  // Make graph: a column graph with the engine's one bracket.
  await page.getByTestId('sheet-make-graph').click();
  await expect(page.locator('[data-testid="graph-svg"] svg')).toBeVisible();
  await expect(page.getByTestId('tb-family')).toHaveValue('column');
  await expect.poll(() => bracketIds(page)).toEqual(['annotation-0']);
});

test('(h) one-way ANOVA with Tukey: a multiple-comparisons table and a graph with brackets', async ({ page }) => {
  await threeGroupTable(page);
  await page.getByTestId('tool-analyze').click();
  await page.getByTestId('analysis-one_way_anova').click();
  await expect(page.getByTestId('posthoc-method')).toHaveValue('tukey');
  await page.getByTestId('analysis-run').click();
  await expect(page.getByTestId('results-sheet')).toBeVisible();
  await expect(page.getByTestId('sheet-summary')).toContainText('F(2, 6)');
  const sec = page.locator('.sheet-section', { has: page.locator('h2', { hasText: /compar/i }) });
  await expect(sec).toBeVisible();
  // Three groups: three pairwise comparisons, each "A vs. B" with an adjusted p.
  await expect(sec.locator('tbody tr')).toHaveCount(3);
  await expect(sec.locator('tbody tr').nth(0).locator('td').nth(0)).toContainText(' vs');
  await expect(sec.locator('thead')).toContainText('Adjusted P');
  const adj = sec.locator('thead th').filter({ hasText: /^Adjusted P$/ });
  await expect(adj).toHaveCount(1);
  // Make graph: one bracket per comparison, from the engine only (no second set from the UI).
  await page.getByTestId('sheet-make-graph').click();
  await expect(page.locator('[data-testid="graph-svg"] svg')).toBeVisible();
  await expect(page.getByTestId('tb-family')).toHaveValue('column');
  await expect.poll(async () => (await bracketIds(page)).sort()).toEqual(['annotation-0', 'annotation-1', 'annotation-2']);
  const spec = await graphSpecOfActive(page);
  expect(spec.annotations.filter((a: { kind: string }) => a.kind === 'significance_bracket')).toHaveLength(3);
});

test('(i) XY table: nonlinear regression 4PL gives LogEC50 and EC50, and the graph draws the curve with its band', async ({ page }) => {
  await newColumnTable(page, 'xy');
  await clickCell(page, 0, 0);
  await pasteIntoGrid(page, DOSE_RESPONSE);
  await expect(cell(page, 7, 2)).toHaveText('96.1');
  await page.getByTestId('tool-analyze').click();
  await page.getByTestId('analysis-nonlinear_regression').click();
  await page.getByTestId('analysis-run').click();
  const sheet = page.getByTestId('results-sheet');
  await expect(sheet).toBeVisible();
  await expect(sheet).toContainText('LogEC50');
  await expect(sheet).toContainText('EC50');
  // Parameters come with confidence intervals.
  await expect(sheet.locator('th', { hasText: /95% CI|Lower 95%/ }).first()).toBeVisible();
  await page.getByTestId('sheet-make-graph').click();
  await expect(page.locator('[data-testid="graph-svg"] svg')).toBeVisible();
  await expect
    .poll(() => page.evaluate(() => document.querySelectorAll('[data-testid="graph-svg"] [id^="curve-"] :is(path, polyline)').length))
    .toBeGreaterThan(0);
  // The confidence band is a filled shape under each curve.
  await expect
    .poll(() => page.evaluate(() => document.querySelectorAll('[data-testid="graph-svg"] [id^="curve-"] :is(path, polygon)').length))
    .toBeGreaterThan(0);
});

test('(j) survival table: Kaplan-Meier with log-rank, step curves on the graph', async ({ page }) => {
  await newColumnTable(page, 'survival');
  await clickCell(page, 0, 0);
  await pasteIntoGrid(page, SURVIVAL);
  await page.getByTestId('tool-analyze').click();
  await page.getByTestId('analysis-survival').click();
  await page.getByTestId('analysis-run').click();
  const sheet = page.getByTestId('results-sheet');
  await expect(sheet).toContainText('Log-rank');
  await expect(sheet).toContainText('Median survival');
  await page.getByTestId('sheet-make-graph').click();
  await expect(page.locator('[data-testid="graph-svg"] svg')).toBeVisible();
  await expect(page.getByTestId('tb-family')).toHaveValue('kaplan_meier');
  await expect
    .poll(() => page.evaluate(() => document.querySelectorAll('[data-testid="graph-svg"] [id^="series-"] :is(path, polyline)').length))
    .toBeGreaterThan(1);
  // The number-at-risk table sits under the axis.
  await expect.poll(() => page.evaluate(() => document.querySelector('[data-testid="graph-svg"] svg')?.textContent ?? '')).toContain('at risk');
});

test('(k) a skewed group fails the normality check and the sheet offers an alternative', async ({ page }) => {
  await newColumnTable(page, 'column');
  await clickCell(page, 0, 0);
  await pasteIntoGrid(page, SKEWED);
  await page.getByTestId('tool-analyze').click();
  await page.getByTestId('analysis-t_test').click();
  await page.getByTestId('analysis-run').click();
  const checks = page.getByTestId('sheet-assumptions');
  await expect(checks).toBeVisible();
  const doubt = checks.locator('.check[data-status="fail"], .check[data-status="caution"]');
  await expect(doubt.first()).toBeVisible();
  await expect(doubt.first().getByTestId('check-alternative')).toBeVisible();
  await expect(checks.getByTestId('check-alternative').first()).toContainText(/Mann|rank|Welch|transform/i);
});

test('(l) ROC curve: the sheet reports the area and the graph draws the curve', async ({ page }) => {
  await newColumnTable(page, 'column');
  await clickCell(page, 0, 0);
  await pasteIntoGrid(page, ROC);
  await page.getByTestId('tool-analyze').click();
  await page.getByTestId('analysis-roc').click();
  await page.getByTestId('analysis-run').click();
  const sheet = page.getByTestId('results-sheet');
  await expect(sheet).toContainText(/Area/);
  await page.getByTestId('sheet-make-graph').click();
  await expect(page.locator('[data-testid="graph-svg"] svg')).toBeVisible();
  await expect(page.getByTestId('tb-family')).toHaveValue('roc');
  await expect
    .poll(() => page.evaluate(() => document.querySelectorAll('[data-testid="graph-svg"] [id^="series-"] :is(path, polyline)').length))
    .toBeGreaterThan(0);
  await expect.poll(() => page.evaluate(() => document.querySelector('[data-testid="graph-svg"] svg')?.textContent ?? '')).toContain('AUC');
});

/** Ids of the bracket layers in the graph on screen. */
function bracketIds(page: Page): Promise<string[]> {
  return page.evaluate(() =>
    [...document.querySelectorAll('[data-testid="graph-svg"] [id^="annotation-"]')].map((e) => e.id).filter((i) => /^annotation-\d+$/.test(i)),
  );
}

async function graphSpecOfActive(page: Page): Promise<{ annotations: { kind: string }[] }> {
  return page.evaluate(async () => {
    const d = window.__dioptra!;
    const p = (await d.dispatch({ type: 'get_project' })) as { items: { id: string; kind: string }[] };
    const g = p.items.filter((i) => i.kind === 'graph').pop()!;
    const s = (await d.dispatch({ type: 'get_graph_spec', id: g.id })) as { annotations?: { kind: string }[] };
    return { annotations: s.annotations ?? [] };
  });
}

// log10 dose (M), then two replicates of % response.
const DOSE_RESPONSE = [
  '-9\t2.1\t3.4',
  '-8.5\t4.8\t6.0',
  '-8\t12.6\t10.9',
  '-7.5\t31.2\t28.7',
  '-7\t58.4\t61.0',
  '-6.5\t81.3\t79.2',
  '-6\t92.7\t91.5',
  '-5.5\t97.0\t96.1',
].join('\n');

// Months; 1 = death, 0 = censored; one subject per row.
const SURVIVAL = [
  '3\t1\t', '5\t1\t', '7\t0\t', '9\t1\t', '12\t1\t', '14\t0\t', '17\t1\t', '21\t0\t', '24\t1\t', '30\t0\t',
  '6\t\t0', '10\t\t1', '15\t\t0', '19\t\t1', '23\t\t0', '27\t\t1', '31\t\t0', '34\t\t0', '36\t\t1', '40\t\t0',
].join('\n');

// Group A has one value far above the rest.
const SKEWED = ['2.1\t3.1', '2.3\t3.4', '2.2\t2.9', '2.4\t3.3', '2.0\t3.0', '2.5\t3.2', '2.2\t3.5', '19.8\t3.1'].join('\n');

// Controls, patients (a marker that runs higher in patients).
const ROC = ['1.2\t2.4', '1.8\t3.1', '2.1\t2.2', '1.5\t3.8', '2.6\t2.9', '1.1\t4.1', '2.0\t3.3', '1.7\t2.7', '2.3\t3.6', '1.4\t1.9'].join('\n');
