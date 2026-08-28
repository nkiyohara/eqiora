import { expect, test } from '@playwright/test';

import { rejectExternalRequests } from './support';

test('Eqiora source blocks use the local grammar in light and dark themes', async ({ page }) => {
  const external = await rejectExternalRequests(page);
  await page.goto('/gallery/exact-cylinder-steady-stokes/');

  const source = page.locator('pre[data-language="eqiora"]').first();
  await expect(source).toBeVisible();
  await expect(source).toHaveAttribute('aria-label', 'eqiora');
  await expect(page.locator('.eq-code-region-label', { hasText: 'eqiora' }).first()).toBeVisible();

  const tokenStyles = await source.locator('code span[style]').evaluateAll((tokens) =>
    new Set(tokens.map((token) => token.getAttribute('style')).filter(Boolean)).size,
  );
  expect(tokenStyles).toBeGreaterThan(2);

  await page.locator('html').evaluate((root) => root.setAttribute('data-theme', 'dark'));
  await expect(source).toBeVisible();
  expect(external).toEqual([]);
});
