import { expect, test, type Browser, type Page } from '@playwright/test';

import {
  assertCoreVisible,
  assertExactTableSelectorScope,
  assertHonest320Reflow,
  assertKeyboardFocusVisible,
  assertMinimumTargetSizes,
  assertNoFakeExecutionControls,
  assertNoPageOverflow,
  assertNoSeriousAxeViolations,
  assertOrdinaryRoutePage,
  assertOrdinaryRoutePlan,
  assertParentCylinderRed,
  assertParentForcedTableBoundary,
  assertProductTableRouteGreen,
  assertReducedMotion,
  assertSemanticStages,
  assertSupportedStatement,
  assertTableInventory,
  assertVisibleSourceFallback,
  attachGrossScreenshot,
  BASE_URL,
  createOrdinaryRoutePlan,
  DIAGNOSTIC_ROUTE,
  installTableObserver,
  launchOfficialBrowser,
  layoutCssState,
  PARENT_LAYOUT_SHA256,
  rejectExternalRequests,
  ROUTES,
  SITE_ROUTES,
  TABLE_ROUTES,
} from './support';

async function assertCylinderContent(page: Page): Promise<void> {
  const sourceSha = process.env.EQIORA_SITE_SOURCE_SHA;
  expect(sourceSha).toMatch(/^[0-9a-f]{40}$/u);
  await assertSemanticStages(page);
  await assertSupportedStatement(page);
  await assertVisibleSourceFallback(page);
  const figures = page.getByRole('figure').getByRole('img');
  await expect(figures).toHaveCount(2);
  for (let offset = 0; offset < 2; offset += 1) await expect(figures.nth(offset)).toBeVisible();
  expect(await page.locator('math').count()).toBeGreaterThanOrEqual(2);
  expect(await page.locator('math[display="block"]').count()).toBeGreaterThan(0);
  expect(await page.locator('math:not([display="block"])').count()).toBeGreaterThan(0);
  expect(await page.locator('.katex-html').count()).toBeGreaterThanOrEqual(2);
  await expect(
    page.getByRole('link', { name: 'Stage 4 Submit and result', exact: true }),
  ).toHaveAttribute('href', '#submit-and-result');
  await expect(
    page.getByRole('link', {
      name: 'Eqiora source form: canonical intent/submit/result cells',
      exact: true,
    }),
  ).toHaveAttribute(
    'href',
    `https://github.com/nkiyohara/eqiora/blob/${sourceSha}/examples/python/exact_cylinder_stokes_marimo.py#L77-L95`,
  );
  await assertNoFakeExecutionControls(page);
}

async function assertDiagnosticIdentity(page: Page): Promise<void> {
  await expect(page).toHaveTitle('Diagnostic in eqiora - Rust');
  await expect(page.locator('body')).toHaveClass(/\brustdoc\b.*\bstruct\b/);
  await expect(page.locator('main')).toHaveCount(1);
  await expect(page.locator('main h1')).toContainText('Struct Diagnostic');
  await expect(page.locator('main h1')).toContainText('Copy item path');
  await assertNoSeriousAxeViolations(page);
}

async function runOrdinaryChunk(id: 'A' | 'B' | 'C'): Promise<void> {
  const plan = createOrdinaryRoutePlan();
  expect(assertOrdinaryRoutePlan(plan)).toEqual([...SITE_ROUTES]);
  expect(ROUTES).toHaveLength(35);
  const context = await browser.newContext({
    baseURL: BASE_URL,
    locale: 'en-GB',
    serviceWorkers: 'block',
  });
  const page = await context.newPage();
  const external = await rejectExternalRequests(page);
  await page.setViewportSize({ width: 1280, height: 900 });
  for (const route of plan[id]) {
    const response = await page.goto(route);
    expect(response?.ok(), route).toBe(true);
    await assertOrdinaryRoutePage(page, route);
    if (route !== '/gallery/exact-cylinder-steady-stokes/') {
      await assertNoSeriousAxeViolations(page);
    }
  }
  expect(external).toEqual([]);
  await context.close();
  expect(context.pages()).toEqual([]);
}

type ProductTableMatrixCell = Readonly<{
  forcedColors: 'none' | 'active';
  width: 1280 | 390 | 320;
}>;

const PRODUCT_TABLE_MATRIX = [
  { forcedColors: 'none', width: 1280 },
  { forcedColors: 'none', width: 390 },
  { forcedColors: 'none', width: 320 },
  { forcedColors: 'active', width: 1280 },
  { forcedColors: 'active', width: 390 },
  { forcedColors: 'active', width: 320 },
] as const satisfies readonly ProductTableMatrixCell[];

const EXPECTED_PRODUCT_TABLE_MATRIX_KEYS = [
  'none/1280',
  'none/390',
  'none/320',
  'active/1280',
  'active/390',
  'active/320',
] as const;

function tableMatrixKey(cell: ProductTableMatrixCell): string {
  return `${cell.forcedColors}/${cell.width}`;
}

function validateProductTableMatrix(plan: readonly ProductTableMatrixCell[]): void {
  const expectedKeys: readonly string[] = EXPECTED_PRODUCT_TABLE_MATRIX_KEYS;
  const expected = new Set(expectedKeys);
  const actualKeys = plan.map(tableMatrixKey);
  const unknown = actualKeys.find((key) => !expected.has(key));
  if (unknown) throw new Error(`TABLE-MATRIX-UNKNOWN: ${unknown}`);
  const seen = new Set<string>();
  for (const key of actualKeys) {
    if (seen.has(key)) throw new Error(`TABLE-MATRIX-DUPLICATE: ${key}`);
    seen.add(key);
  }
  const missing = expectedKeys.find((key) => !seen.has(key));
  if (missing) throw new Error(`TABLE-MATRIX-MISSING: ${missing}`);
  if (actualKeys.length !== expectedKeys.length) {
    throw new Error(`TABLE-MATRIX-CARDINALITY: ${actualKeys.length}`);
  }
  const outOfOrder = actualKeys.findIndex((key, index) => key !== expectedKeys[index]);
  if (outOfOrder !== -1) {
    throw new Error(
      `TABLE-MATRIX-ORDER: ${actualKeys[outOfOrder]} at ${outOfOrder}, expected ${expectedKeys[outOfOrder]}`,
    );
  }
}

function assertProductTableMatrixPlan(): void {
  const positive = PRODUCT_TABLE_MATRIX.map((cell) => ({ ...cell }));
  expect(() => validateProductTableMatrix(positive)).not.toThrow();

  const missing = positive.slice(0, -1);
  expect(() => validateProductTableMatrix(missing)).toThrow(
    'TABLE-MATRIX-MISSING: active/320',
  );

  const duplicated = [...positive.slice(0, -1), { ...positive[0] }];
  expect(() => validateProductTableMatrix(duplicated)).toThrow(
    'TABLE-MATRIX-DUPLICATE: none/1280',
  );

  const forcedBeforeOrdinary = [...positive.slice(3), ...positive.slice(0, 3)];
  expect(() => validateProductTableMatrix(forcedBeforeOrdinary)).toThrow(
    'TABLE-MATRIX-ORDER: active/1280 at 0, expected none/1280',
  );
}

test.describe.configure({ mode: 'serial' });

let browser: Browser;

test.beforeAll(async () => {
  browser = await launchOfficialBrowser(true);
});

test.afterAll(async () => {
  await browser?.close();
});

for (const id of ['A', 'B', 'C'] as const) {
  test(`00${id} exact deterministic ordinary site chunk is complete and green`, async () => {
    test.setTimeout(300_000);
    await runOrdinaryChunk(id);
  });
}

test('00D exact real Rustdoc Diagnostic ordinary chunk is complete and green', async () => {
  test.setTimeout(300_000);
  const plan = createOrdinaryRoutePlan();
  expect(assertOrdinaryRoutePlan(plan)).toEqual([...SITE_ROUTES]);
  expect(ROUTES).toHaveLength(35);
  const context = await browser.newContext({
    baseURL: BASE_URL,
    locale: 'en-GB',
    serviceWorkers: 'block',
  });
  const page = await context.newPage();
  const external = await rejectExternalRequests(page);
  await page.setViewportSize({ width: 1280, height: 900 });
  const response = await page.goto(DIAGNOSTIC_ROUTE);
  expect(response?.ok()).toBe(true);
  await assertOrdinaryRoutePage(page, DIAGNOSTIC_ROUTE);
  await assertDiagnosticIdentity(page);
  expect(external).toEqual([]);
  await context.close();
  expect(context.pages()).toEqual([]);
});

test('01 honest 320px O-1 through O-4 composition and retained interaction controls pass', async ({}, testInfo) => {
  test.setTimeout(180_000);
  const context = await browser.newContext({
    baseURL: BASE_URL,
    locale: 'en-GB',
    serviceWorkers: 'block',
  });
  const page = await context.newPage();
  const external = await rejectExternalRequests(page);
  await page.setViewportSize({ width: 320, height: 844 });
  await page.goto('/');
  await assertHonest320Reflow(page);
  await assertNoPageOverflow(page);
  await assertMinimumTargetSizes(page.getByRole('banner').getByRole('link'));
  await assertMinimumTargetSizes(page.getByRole('banner').getByRole('button'));
  await assertMinimumTargetSizes(page.getByRole('link', { name: 'Get started', exact: true }));
  await assertMinimumTargetSizes(page.getByRole('link', { name: 'Explore gallery', exact: true }));
  await assertKeyboardFocusVisible(page, page.getByRole('link', { name: 'Get started', exact: true }));

  await page.goto('/gallery/exact-cylinder-steady-stokes/');
  await assertHonest320Reflow(page);
  await assertCoreVisible(page);
  await assertNoPageOverflow(page);
  await assertCylinderContent(page);

  for (const colorScheme of ['light', 'dark'] as const) {
    await page.emulateMedia({ colorScheme, forcedColors: 'none' });
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
  }

  await page.emulateMedia({ colorScheme: 'light', forcedColors: 'none', reducedMotion: 'reduce' });
  await page.goto('/gallery/exact-cylinder-steady-stokes/');
  await assertReducedMotion(page);
  expect(external).toEqual([]);
  await context.close();
});

test('02 exact table inventory is complete before parent or product matrix results', async () => {
  test.setTimeout(600_000);
  assertProductTableMatrixPlan();
  const context = await browser.newContext({
    baseURL: BASE_URL,
    locale: 'en-GB',
    serviceWorkers: 'block',
  });
  const page = await context.newPage();
  await installTableObserver(page);
  const external = await rejectExternalRequests(page);
  await page.setViewportSize({ width: 1280, height: 900 });

  let tableTotal = 0;
  let directTotal = 0;
  let componentTotal = 0;
  for (const expected of TABLE_ROUTES) {
    await page.goto(expected.route);
    await assertTableInventory(page, expected);
    tableTotal += expected.tables;
    directTotal += expected.direct;
    componentTotal += expected.component;
  }
  expect({ tableTotal, directTotal, componentTotal }).toEqual({
    tableTotal: 1133,
    directTotal: 1132,
    componentTotal: 1,
  });
  await page.goto('/reference/python/eqiora/');
  await expect(page.locator('main table')).toHaveCount(0);

  expect(external).toEqual([]);
  await context.close();
  expect(context.pages()).toEqual([]);
});

test('02A authenticated parent table boundary is complete or product sentinel is exact', async () => {
  test.setTimeout(600_000);
  const state = await layoutCssState();
  if (!state.parent) {
    expect(state.sha256).not.toBe(PARENT_LAYOUT_SHA256);
    assertExactTableSelectorScope(state.css);
    return;
  }

  expect(state.sha256).toBe(PARENT_LAYOUT_SHA256);
  const context = await browser.newContext({
    baseURL: BASE_URL,
    locale: 'en-GB',
    serviceWorkers: 'block',
  });
  const page = await context.newPage();
  await installTableObserver(page);
  const external = await rejectExternalRequests(page);

  try {
    for (const forcedColors of ['none', 'active'] as const) {
      await page.emulateMedia({ forcedColors });
      for (const width of [1280, 390, 320]) {
        await page.setViewportSize({ width, height: 900 });
        await assertParentCylinderRed(page);
      }
    }
    await page.emulateMedia({ forcedColors: 'active' });
    await page.setViewportSize({ width: 320, height: 900 });
    for (const expected of TABLE_ROUTES) {
      await assertParentForcedTableBoundary(page, expected);
    }
    expect(external).toEqual([]);
  } finally {
    await context.close();
  }
  expect(context.pages()).toEqual([]);
});

for (const { forcedColors, width } of PRODUCT_TABLE_MATRIX) {
  test(`02B product table matrix forcedColors=${forcedColors} width=${width} is complete or parent sentinel is exact`, async () => {
    test.setTimeout(600_000);
    const state = await layoutCssState();
    if (state.parent) {
      expect(state.sha256).toBe(PARENT_LAYOUT_SHA256);
      return;
    }

    expect(state.sha256).not.toBe(PARENT_LAYOUT_SHA256);
    assertExactTableSelectorScope(state.css);
    const context = await browser.newContext({
      baseURL: BASE_URL,
      locale: 'en-GB',
      serviceWorkers: 'block',
    });
    const page = await context.newPage();
    await installTableObserver(page);
    const external = await rejectExternalRequests(page);
    await page.emulateMedia({ forcedColors });
    await page.setViewportSize({ width, height: 900 });

    try {
      for (const expected of TABLE_ROUTES) {
        await assertProductTableRouteGreen(page, expected);
      }
      expect(external).toEqual([]);
    } finally {
      await context.close();
    }
    expect(context.pages()).toEqual([]);
  });
}

test('03 forced colours retain core content and exact non-table accessibility boundaries', async () => {
  test.setTimeout(300_000);
  const context = await browser.newContext({
    baseURL: BASE_URL,
    locale: 'en-GB',
    serviceWorkers: 'block',
  });
  const page = await context.newPage();
  const external = await rejectExternalRequests(page);
  await page.emulateMedia({ forcedColors: 'active' });
  await page.setViewportSize({ width: 320, height: 844 });
  for (const route of ROUTES) {
    await page.goto(route);
    await assertCoreVisible(page);
    await assertNoPageOverflow(page);
    if (route === '/gallery/exact-cylinder-steady-stokes/') {
      await assertCylinderContent(page);
    }
    if (!TABLE_ROUTES.some((expected) => expected.route === route)) {
      await assertNoSeriousAxeViolations(page);
    }
  }
  await page.goto('/');
  await assertKeyboardFocusVisible(page, page.getByRole('link', { name: 'Get started', exact: true }));
  expect(external).toEqual([]);
  await context.close();
});
