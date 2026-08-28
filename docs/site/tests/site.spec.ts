import { expect, test } from '@playwright/test';

import {
  assertAccessibleTooltip,
  assertCoreVisible,
  assertNoFakeExecutionControls,
  assertNoSeriousAxeViolations,
  assertSemanticStages,
  assertVisibleSourceFallback,
  rejectExternalRequests,
  ROUTES,
} from './support';

test('required routes, semantic stages, controls, and 404 are real static surfaces', async ({ page }) => {
  const external = await rejectExternalRequests(page);
  for (const route of ROUTES) {
    const response = await page.goto(route);
    expect(response?.status(), route).toBe(200);
    await assertCoreVisible(page);
  }

  await page.goto('/');
  await expect(page.getByRole('banner').getByRole('link', { name: 'Eqiora', exact: true })).toHaveAttribute('href', '/');
  await expect(page.getByRole('link', { name: 'Get started', exact: true })).toHaveAttribute('href', '/get-started/');
  await expect(page.getByRole('link', { name: 'Explore gallery', exact: true })).toHaveAttribute('href', '/gallery/');
  await expect(page.getByRole('img', { name: /Pressure in pascals for a 2D steady-Stokes exact-cylinder/i })).toBeVisible();
  await assertAccessibleTooltip(
    page,
    page.getByRole('button', { name: /search/i }).filter({ visible: true }).first(),
    /search/i,
  );
  await assertAccessibleTooltip(
    page,
    page.getByRole('combobox', { name: /theme/i }).filter({ visible: true }).first(),
    /theme/i,
  );

  await page.goto('/gallery/');
  const card = page.getByRole('link', { name: /Exact-cylinder steady Stokes/i }).first();
  await expect(card).toHaveAttribute('href', '/gallery/exact-cylinder-steady-stokes/');
  await card.focus();
  await page.keyboard.press('Enter');
  await expect(page).toHaveURL(/\/gallery\/exact-cylinder-steady-stokes\/$/);

  await assertSemanticStages(page);
  await assertNoFakeExecutionControls(page);
  const missing = await page.goto('/this-route-does-not-exist');
  expect(missing?.status()).toBe(404);
  await expect(page.getByRole('heading', { level: 1, name: '404', exact: true })).toBeVisible();
  await expect(page.getByText(/Page not found/i)).toBeVisible();

  const oldSocial = await page.goto('/assets/social-card.svg');
  expect(oldSocial?.status()).toBe(404);
  expect(oldSocial?.headers().location).toBeUndefined();
  const social = await page.goto('/social-card.svg');
  expect(social?.status()).toBe(200);
  expect(social?.headers()['content-type']).toContain('image/svg+xml');
  expect(external).toEqual([]);
});

test('mixed-boundary elasticity is a static source-traced second gallery surface', async ({ page }) => {
  const sourceSha = process.env.EQIORA_SITE_SOURCE_SHA;
  expect(sourceSha).toMatch(/^[0-9a-f]{40}$/u);
  const external = await rejectExternalRequests(page);
  await page.goto('/gallery/');
  const card = page.getByRole('link', { name: /Mixed-boundary linear elasticity/i });
  await expect(card).toHaveAttribute('href', '/gallery/mixed-boundary-elasticity/');
  await card.click();
  await expect(page).toHaveURL(/\/gallery\/mixed-boundary-elasticity\/$/);
  await expect(
    page.getByRole('img', {
      name: /Reference and deformed meshes for the bounded 2D mixed-boundary/i,
    }),
  ).toBeVisible();
  await expect(page.getByText('Presentation, not evidence.', { exact: true })).toBeVisible();
  await expect(page.getByRole('link', { name: 'Open the Marimo surface', exact: true })).toHaveAttribute(
    'href',
    `https://github.com/nkiyohara/eqiora/blob/${sourceSha}/examples/python/mixed_boundary_elasticity_marimo.py`,
  );
  await expect(page.getByRole('link', { name: 'Open the Jupyter surface', exact: true })).toHaveAttribute(
    'href',
    `https://github.com/nkiyohara/eqiora/blob/${sourceSha}/examples/python/mixed_boundary_elasticity_jupyter.ipynb`,
  );
  await assertNoFakeExecutionControls(page);
  expect(external).toEqual([]);
});

test('transient cylinder startup publishes accessible caller-owned motion', async ({ page }) => {
  const external = await rejectExternalRequests(page);
  await page.goto('/gallery/');
  const card = page.getByRole('link', { name: /Transient cylinder-flow startup/i });
  await expect(card).toHaveAttribute('href', '/gallery/transient-cylinder-startup/');
  await card.click();
  await expect(page).toHaveURL(/\/gallery\/transient-cylinder-startup\/$/);

  const video = page.locator('video.eq-gallery-motion__video');
  await expect(video).toHaveAttribute('controls', '');
  await expect(video.locator('source[type="video/webm"]')).toHaveCount(1);
  await expect(video.locator('source[type="video/mp4"]')).toHaveCount(1);
  await expect(video).not.toHaveAttribute('autoplay', /.*/u);
  await expect(page.locator('#startup-motion-description')).toContainText(
    'It does not show, and must not be read as, periodic shedding or a developed vortex street.',
  );
  await assertNoSeriousAxeViolations(page);

  await page.emulateMedia({ reducedMotion: 'reduce' });
  await expect(video).toBeHidden();
  await expect(page.locator('img.eq-gallery-motion__still')).toBeVisible();
  await assertNoFakeExecutionControls(page);
  expect(external).toEqual([]);
});

test('transient cylinder Colab launch remains release-gated', async ({ page }) => {
  await page.goto('/gallery/transient-cylinder-startup/');
  const launch = page.getByRole('link', { name: 'Open in Colab', exact: true });
  const release = process.env.EQIORA_SITE_PYTHON_VERSION;
  if (/^0\.1\.0a([4-9]|[1-9][0-9]+)$/u.test(release ?? '')) {
    const sourceSha = process.env.EQIORA_SITE_SOURCE_SHA;
    await expect(launch).toHaveAttribute(
      'href',
      `https://colab.research.google.com/github/nkiyohara/eqiora/blob/${sourceSha}/examples/python/transient_cylinder_wake_jupyter.ipynb`,
    );
  } else {
    await expect(launch).toHaveCount(0);
  }
});

test('Pagefind returns one representative from every frozen reference family', async ({ page }) => {
  const external = await rejectExternalRequests(page);
  await page.goto('/');
  const expectations = [
    ['eqiora Diagnostic', '/reference/python/eqiora/'],
    ['eqiora::Diagnostic stable', '/reference/rust/'],
    ['eqiora::api::CadBoxIntentV1 transitional', '/reference/rust/'],
    ['eqiora::api module', '/reference/rust/'],
    ['eqiora check', '/reference/cli/'],
    ['eqiora.control/v2', '/reference/control-v2/'],
    ['eqiora.model.compile_check', '/reference/mcp/'],
  ] as const;
  for (const [query, expectedRoute] of expectations) {
    const urls = await page.evaluate(async (searchQuery) => {
      const dynamicImport = new Function('specifier', 'return import(specifier)') as (
        specifier: string,
      ) => Promise<{ search: (query: string) => Promise<{ results: Array<{ data: () => Promise<{ url: string }> }> }> }>;
      const pagefind = await dynamicImport('/pagefind/pagefind.js');
      const result = await pagefind.search(searchQuery);
      return Promise.all(result.results.slice(0, 12).map(async (entry) => (await entry.data()).url));
    }, query);
    const paths = urls.map((url) => new URL(url, 'http://127.0.0.1:4173').pathname);
    expect(paths, query).toContain(expectedRoute);
  }
  expect(external).toEqual([]);
});

test('JavaScript-disabled core remains navigable and mathematically complete', async ({ browser }) => {
  test.setTimeout(120_000);
  const context = await browser.newContext({
    baseURL: 'http://127.0.0.1:4173',
    javaScriptEnabled: false,
    serviceWorkers: 'block',
    viewport: { width: 390, height: 844 },
  });
  const page = await context.newPage();
  const external = await rejectExternalRequests(page);
  for (const route of ROUTES) {
    await page.goto(route);
    await assertCoreVisible(page);
  }
  await page.goto('/gallery/exact-cylinder-steady-stokes/');
  await assertSemanticStages(page);
  await expect(page.getByRole('img', { name: /Pressure in pascals for a 2D steady-Stokes exact-cylinder/i })).toBeVisible();
  expect(await page.locator('math').count()).toBeGreaterThanOrEqual(2);
  expect(await page.locator('.katex-html').count()).toBeGreaterThanOrEqual(2);
  await assertVisibleSourceFallback(page);
  await page.getByRole('link', { name: 'Gallery', exact: true }).filter({ visible: true }).first().click();
  await expect(page).toHaveURL(/\/gallery\/$/);
  expect(external).toEqual([]);
  await context.close();
});
