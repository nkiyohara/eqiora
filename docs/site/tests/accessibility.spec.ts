import { expect, test } from '@playwright/test';

import {
  assertCoreVisible,
  assertKeyboardFocusVisible,
  assertMinimumTargetSizes,
  assertNoPageOverflow,
  assertNoSeriousAxeViolations,
  assertReducedMotion,
  attachGrossScreenshot,
  rejectExternalRequests,
  ROUTES,
} from './support';

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

  await page.emulateMedia({ forcedColors: 'active' });
  await page.goto('/');
  await assertKeyboardFocusVisible(page, page.getByRole('link', { name: 'Get started', exact: true }));

  await page.emulateMedia({ forcedColors: 'none', reducedMotion: 'reduce' });
  await page.goto('/gallery/exact-cylinder-steady-stokes/');
  await assertReducedMotion(page);
  expect(external).toEqual([]);
});

for (const colorScheme of ['light', 'dark'] as const) {
  test(`gross desktop and mobile layout capture in ${colorScheme}`, async ({ page }, testInfo) => {
    await rejectExternalRequests(page);
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
  });
}
