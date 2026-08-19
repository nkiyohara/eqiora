import { expect, test } from '@playwright/test';

import {
  assertNoPageOverflow,
  assertVisibleSourceFallback,
  rejectExternalRequests,
} from './support';

test('inline and block math expose HTML, MathML, source fallback, and local overflow', async ({ page }) => {
  const external = await rejectExternalRequests(page);
  await page.setViewportSize({ width: 320, height: 844 });
  await page.goto('/gallery/exact-cylinder-steady-stokes/');
  const blocks = page.locator('.katex-display');
  expect(await blocks.count()).toBeGreaterThan(0);
  expect(await page.locator('.katex-display math[display="block"]').count()).toBeGreaterThan(0);
  expect(await page.locator('.katex:not(.katex-display .katex) math:not([display="block"])').count()).toBeGreaterThan(0);
  expect(await page.locator('.katex-mathml math').count()).toBeGreaterThanOrEqual(2);
  expect(await page.locator('.katex-html').count()).toBeGreaterThanOrEqual(2);
  await assertVisibleSourceFallback(page);
  const regions = page.getByRole('region', { name: /equation/i });
  expect(await regions.count()).toBeGreaterThan(0);
  for (let offset = 0; offset < (await regions.count()); offset += 1) {
    const observation = await regions.nth(offset).evaluate((element) => {
      const box = element.getBoundingClientRect();
      const style = getComputedStyle(element);
      return {
        left: box.left,
        right: box.right,
        viewport: document.documentElement.clientWidth,
        overflowX: style.overflowX,
        clipped: element.scrollWidth > element.clientWidth,
      };
    });
    expect(observation.left).toBeGreaterThanOrEqual(-1);
    expect(observation.right).toBeLessThanOrEqual(observation.viewport + 1);
    if (observation.clipped) expect(['auto', 'scroll']).toContain(observation.overflowX);
  }
  await assertNoPageOverflow(page);
  expect(external).toEqual([]);
});
