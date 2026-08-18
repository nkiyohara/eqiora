import { expect, test, type Page } from '@playwright/test';

import {
  assertCoreVisible,
  assertKeyboardFocusVisible,
  assertMinimumTargetSizes,
  assertNoFakeExecutionControls,
  assertNoPageOverflow,
  assertNoSeriousAxeViolations,
  assertReducedMotion,
  assertSemanticStages,
  assertVisibleSourceFallback,
  attachGrossScreenshot,
  rejectExternalRequests,
  ROUTES,
} from './support';

async function assertCylinderContent(page: Page): Promise<void> {
  await assertSemanticStages(page);
  await assertVisibleSourceFallback(page);
  await expect(page.getByRole('figure').getByRole('img')).toBeVisible();
  expect(await page.locator('math').count()).toBeGreaterThanOrEqual(2);
  expect(await page.locator('math[display="block"]').count()).toBeGreaterThan(0);
  expect(await page.locator('math:not([display="block"])').count()).toBeGreaterThan(0);
  expect(await page.locator('.katex-html').count()).toBeGreaterThanOrEqual(2);
  await expect(
    page.getByText(
      'one frozen 2D steady incompressible Stokes exact-cylinder demonstration, rendered from its accepted public Result path and linked evidence.',
      { exact: true },
    ),
  ).toBeVisible();
  await assertNoFakeExecutionControls(page);
}

test('representative routes have no serious or critical automated accessibility violations', async ({ page }) => {
  const external = await rejectExternalRequests(page);
  for (const route of ROUTES) {
    await page.goto(route);
    await assertNoSeriousAxeViolations(page);
  }
  expect(external).toEqual([]);
});

test('320px, 400% zoom, target size, focus, forced colours, and reduced motion remain usable', async ({ page }) => {
  const external = await rejectExternalRequests(page);
  await page.setViewportSize({ width: 320, height: 844 });
  await page.goto('/');
  await assertNoPageOverflow(page);
  await assertMinimumTargetSizes(page.getByRole('banner').getByRole('link'));
  await assertMinimumTargetSizes(page.getByRole('banner').getByRole('button'));
  await assertMinimumTargetSizes(page.getByRole('link', { name: 'Get started', exact: true }));
  await assertMinimumTargetSizes(page.getByRole('link', { name: 'Explore gallery', exact: true }));
  await assertKeyboardFocusVisible(page, page.getByRole('link', { name: 'Get started', exact: true }));

  await page.setViewportSize({ width: 1280, height: 900 });
  await page.goto('/gallery/exact-cylinder-steady-stokes/');
  await page.evaluate(() => {
    document.documentElement.style.zoom = '4';
  });
  await assertCoreVisible(page);
  await assertNoPageOverflow(page);
  await assertCylinderContent(page);

  await page.emulateMedia({ forcedColors: 'active' });
  await page.setViewportSize({ width: 320, height: 844 });
  for (const route of ROUTES) {
    await page.goto(route);
    await assertCoreVisible(page);
    await assertNoPageOverflow(page);
    await assertNoSeriousAxeViolations(page);
    if (route === '/gallery/exact-cylinder-steady-stokes/') {
      await assertCylinderContent(page);
    }
  }
  await page.goto('/');
  await assertKeyboardFocusVisible(page, page.getByRole('link', { name: 'Get started', exact: true }));

  await page.emulateMedia({ forcedColors: 'none', reducedMotion: 'reduce' });
  await page.goto('/gallery/exact-cylinder-steady-stokes/');
  await assertReducedMotion(page);
  expect(external).toEqual([]);
});

for (const colorScheme of ['light', 'dark'] as const) {
  test(`gross desktop and mobile layout capture in ${colorScheme}`, async ({ page }, testInfo) => {
    const external = await rejectExternalRequests(page);
    await page.emulateMedia({ colorScheme });
    await page.setViewportSize({ width: 1440, height: 900 });
    await page.goto('/');
    await attachGrossScreenshot(page, testInfo, `home-desktop-${colorScheme}`);
    await page.goto('/gallery/exact-cylinder-steady-stokes/');
    await attachGrossScreenshot(page, testInfo, `cylinder-desktop-${colorScheme}`);
    await page.setViewportSize({ width: 390, height: 844 });
    await page.goto('/');
    await assertNoPageOverflow(page);
    await attachGrossScreenshot(page, testInfo, `home-mobile-${colorScheme}`);
    await page.goto('/gallery/exact-cylinder-steady-stokes/');
    await assertNoPageOverflow(page);
    await attachGrossScreenshot(page, testInfo, `cylinder-mobile-${colorScheme}`);
    expect(external).toEqual([]);
  });
}
