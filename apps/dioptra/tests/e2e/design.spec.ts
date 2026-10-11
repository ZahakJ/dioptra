import { expect, test } from '@playwright/test';
import { boot, cell, clickCell, newColumnTable } from './helpers';

test.beforeEach(async ({ page }) => {
  await boot(page);
});

test('the empty state opens the bundled sample project', async ({ page }) => {
  await expect(page.getByTestId('welcome')).toBeVisible();
  await expect(page.getByTestId('welcome')).toContainText('A sheet for your data');
  await page.getByTestId('welcome-sample').click();
  await expect(page.getByTestId('project-title')).toContainText('Cytokine study');
  for (const title of ['Plasma IL-6', 'Dose response', 'Overall survival', 'IL-6 by dose', 'Dose-response curves', 'Kaplan-Meier curves']) {
    await expect(page.getByTestId(`rail-item-${title}`)).toBeVisible();
  }
  // The first table opens with its data, and the bounded sheet ends 12 rows below it.
  await expect(cell(page, 0, 0)).toHaveText('12.4');
  await expect(page.locator('.rh-num').last()).toHaveText('20');
  // Hovering the active cell says where its value came from, in words rather than ISO 8601.
  await cell(page, 0, 0).click();
  await expect(cell(page, 0, 0)).toHaveAttribute('data-tip', /^Pasted .*(today|yesterday|\d{4}) at \d{2}:\d{2}/);
  await page.mouse.move(5, 5);
  await cell(page, 0, 0).hover();
  await expect(page.getByTestId('tooltip')).toContainText('Pasted');
  await expect(page.locator('.provenance')).toContainText('Pasted');
  await expect(page.locator('.provenance')).not.toHaveText(/\d{4}-\d{2}-\d{2}T/);
  // Its analysis computes from the sample's data.
  await page.getByTestId('rail-item-One-way ANOVA of Plasma IL-6').click();
  await expect(page.getByTestId('sheet-summary')).toContainText('F(3, 28)');
  await expect(page.getByTestId('sheet-assumptions')).toHaveAttribute('data-worst', /pass|caution|fail/);
});

test('density switches between comfortable and compact rows and is remembered', async ({ page }) => {
  await newColumnTable(page);
  const grid = page.getByTestId('grid');
  await expect(grid).toHaveAttribute('data-density', 'comfortable');
  await clickCell(page, 1, 0);
  expect((await cell(page, 1, 0).boundingBox())!.height).toBeCloseTo(28, 0);

  await page.getByTestId('open-palette').click();
  await page.getByTestId('palette-input').fill('compact rows');
  await expect(page.getByTestId('palette-item-view.densityCompact')).toHaveAttribute('aria-selected', 'true');
  await page.keyboard.press('Enter');
  await expect(grid).toHaveAttribute('data-density', 'compact');
  expect((await cell(page, 1, 0).boundingBox())!.height).toBeCloseTo(22, 0);
  // The active cell overlay follows the new row height.
  const active = (await page.getByTestId('active-cell').boundingBox())!;
  const c = (await cell(page, 1, 0).boundingBox())!;
  expect(Math.abs(active.y - c.y)).toBeLessThan(1.5);

  // "+ rows" grows the bounded sheet by 20.
  const lastBefore = Number(await page.locator('.rh-num').last().textContent());
  await page.getByTestId('add-rows').click();
  await page.getByTestId('grid').evaluate((g) => (g.scrollTop = g.scrollHeight));
  await expect(page.locator('.rh-num').last()).toHaveText(String(lastBefore + 20));

  await page.reload();
  await expect(page.getByTestId('app')).toHaveAttribute('data-ready', 'true', { timeout: 60_000 });
  await expect.poll(() => page.evaluate(() => JSON.parse(localStorage.getItem('dioptra-ui') ?? '{}').state?.density)).toBe('compact');

  await page.getByTestId('app-menu').click();
  await page.getByTestId('menubar-View').click();
  await page.getByTestId('menu-view.densityComfortable').click();
  await expect.poll(() => page.evaluate(() => JSON.parse(localStorage.getItem('dioptra-ui') ?? '{}').state?.density)).toBe('comfortable');
});
