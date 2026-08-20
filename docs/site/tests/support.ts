import AxeBuilder from '@axe-core/playwright';
import {
  chromium,
  expect,
  type Browser,
  type Locator,
  type Page,
  type TestInfo,
} from '@playwright/test';
import { createHash } from 'node:crypto';
import { createRequire } from 'node:module';
import { dirname, resolve } from 'node:path';
import { lstat, readFile, realpath } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';

export const BASE_URL = 'http://127.0.0.1:4173';
export const DIAGNOSTIC_ROUTE = '/reference/rust/api/eqiora/struct.Diagnostic.html';
export const SITE_ROUTES = [
  '/',
  '/api/',
  '/architecture/',
  '/capabilities/',
  '/concepts/',
  '/contributing/',
  '/evidence/',
  '/examples/',
  '/gallery/',
  '/gallery/exact-cylinder-steady-stokes/',
  '/get-started/',
  '/python/',
  '/python/differentiation/',
  '/python/execution-and-arrays/',
  '/python/modeling/',
  '/reference/',
  '/reference/cli/',
  '/reference/control-v2/',
  '/reference/mcp/',
  '/reference/python/',
  '/reference/python/diff/',
  '/reference/python/eqiora/',
  '/reference/python/fluid/',
  '/reference/python/fsi/',
  '/reference/python/geometry/',
  '/reference/python/jax/',
  '/reference/python/matplotlib/',
  '/reference/python/meshing/',
  '/reference/python/solid/',
  '/reference/python/torch/',
  '/reference/python/trajectory/',
  '/reference/rust/',
  '/release-notes/',
  '/404.html',
] as const;

export const ROUTES = [...SITE_ROUTES, DIAGNOSTIC_ROUTE] as const;

export const STAGES = [
  { id: 'problem-setup', step: 1, title: 'Problem setup' },
  { id: 'model-definition', step: 2, title: 'Eqiora model definition' },
  { id: 'mesh-and-boundaries', step: 3, title: 'Mesh and boundaries' },
  { id: 'submit-and-result', step: 4, title: 'Submit and result' },
  { id: 'pressure-visualization', step: 5, title: 'Pressure visualization' },
  { id: 'verified-boundary', step: 6, title: 'Verified and not claimed' },
] as const;

export const SUPPORTED_STATEMENT =
  'One frozen 2D steady incompressible Stokes exact-cylinder demonstration, rendered from its accepted public Result path and linked evidence.';

export const TABLE_SELECTORS = {
  generic: '.sl-markdown-content table:not(:where(.not-content *))',
  direct: '.sl-markdown-content > table',
  component: '.sl-markdown-content .eq-stage__body > table',
} as const;

export const TABLE_ROUTES = [
  { route: '/capabilities/', tables: 1, direct: 1, component: 0 },
  { route: '/evidence/', tables: 1121, direct: 1121, component: 0 },
  { route: '/gallery/exact-cylinder-steady-stokes/', tables: 1, direct: 0, component: 1 },
  { route: '/reference/control-v2/', tables: 1, direct: 1, component: 0 },
  { route: '/reference/python/', tables: 2, direct: 2, component: 0 },
  { route: '/reference/rust/', tables: 3, direct: 3, component: 0 },
] as const;

export type TableRoute = (typeof TABLE_ROUTES)[number];

const CHROME_VERSION = '151.0.7922.34';
const CHROME_BYTES = 290_614_600;
const CHROME_SHA256 = '0b20b130e7edd9dd51873be867761295fe0cfad490c2b9a64f95bd3cfc08fa71';
const BROWSERS_JSON_SHA256 = 'f306eed529599b1eaf2f8a85db9de2b23e1a3fe36c2b66434b7c9434fb627a99';
export const PARENT_LAYOUT_SHA256 =
  '9a6aa92fe82bdc7ed7d70a3891c369d10bd73ae0cf2f11d087dd721d02befe0a';

export async function launchOfficialBrowser(headless = true): Promise<Browser> {
  const browserRoot = process.env.PLAYWRIGHT_BROWSERS_PATH;
  expect(browserRoot?.endsWith('/eqiora-pw-1.62.1-r1234')).toBe(true);
  const root = await realpath(browserRoot!);
  expect(root.endsWith('/eqiora-pw-1.62.1-r1234')).toBe(true);
  const executablePath = resolve(root, 'chromium-1234/chrome-linux64/chrome');
  const executableLink = await lstat(executablePath);
  expect(executableLink.isSymbolicLink()).toBe(false);
  expect(executableLink.isFile()).toBe(true);
  expect(executableLink.mode & 0o777).toBe(0o755);
  expect(executableLink.size).toBe(CHROME_BYTES);
  expect(createHash('sha256').update(await readFile(executablePath)).digest('hex')).toBe(
    CHROME_SHA256,
  );

  const require = createRequire(import.meta.url);
  const corePackage = require.resolve('playwright-core/package.json');
  const browsersJson = resolve(dirname(corePackage), 'browsers.json');
  expect(createHash('sha256').update(await readFile(browsersJson)).digest('hex')).toBe(
    BROWSERS_JSON_SHA256,
  );
  const manifest = JSON.parse(await readFile(browsersJson, 'utf8')) as {
    browsers: Array<{ name: string; revision: string; browserVersion: string }>;
  };
  for (const name of ['chromium', 'chromium-headless-shell']) {
    expect(manifest.browsers.find((entry) => entry.name === name)).toMatchObject({
      revision: '1234',
      browserVersion: CHROME_VERSION,
    });
  }

  const browser = await chromium.launch({ executablePath, headless });
  expect(browser.version()).toBe(CHROME_VERSION);
  return browser;
}

export async function rejectExternalRequests(page: Page): Promise<string[]> {
  const attempts: string[] = [];
  await page.route('**/*', async (route) => {
    const url = new URL(route.request().url());
    if (
      url.protocol === 'data:' ||
      url.protocol === 'blob:' ||
      url.origin === BASE_URL
    ) {
      await route.continue();
      return;
    }
    attempts.push(url.href);
    await route.abort('blockedbyclient');
  });
  return attempts;
}

export async function assertCoreVisible(page: Page): Promise<void> {
  const main = page.getByRole('main');
  await expect(main).toBeVisible();
  await expect(main.getByRole('heading', { level: 1 })).toBeVisible();
  expect((await main.innerText()).trim().length).toBeGreaterThan(20);
}

export async function assertSemanticStages(page: Page): Promise<void> {
  const sections = page.locator('main section.eq-stage');
  await expect(sections).toHaveCount(STAGES.length);
  for (const [offset, stage] of STAGES.entries()) {
    const name = `Stage ${stage.step} ${stage.title}`;
    const section = sections.nth(offset);
    await expect(section).toHaveAttribute('id', stage.id);
    await expect(section).toHaveAttribute('data-step', String(stage.step));
    const heading = section.getByRole('heading', { level: 2, name, exact: true });
    await expect(heading).toHaveCount(1);
    await expect(heading).toBeVisible();
    const region = page.getByRole('region', { name, exact: true });
    await expect(region).toHaveCount(1);
    expect(await region.evaluate((element, expected) => element === expected, await section.elementHandle())).toBe(
      true,
    );
    const emoji = heading.locator('.eq-stage-marker__emoji');
    await expect(emoji).toHaveCount(1);
    await expect(emoji).toHaveAttribute('aria-hidden', 'true');
  }
}

export async function assertSupportedStatement(page: Page): Promise<void> {
  const statement = page.getByText(SUPPORTED_STATEMENT, { exact: true });
  await expect(statement).toHaveCount(1);
  await expect(statement).toBeVisible();
  expect(
    await statement.evaluate((element) =>
      Boolean(
        element.closest(
          '.eq-claim-boundary__panel--supported[role="group"][aria-label="Supported"]',
        ),
      ),
    ),
  ).toBe(true);
  expect(
    await statement.evaluate((element) =>
      Boolean(element.closest('.eq-claim-boundary__panel--not-claimed')),
    ),
  ).toBe(false);
}

export async function assertVisibleSourceFallback(page: Page): Promise<void> {
  const fallbacks = page.getByText('Eqiora source form', { exact: true });
  expect(await fallbacks.count()).toBeGreaterThan(0);
  for (let offset = 0; offset < (await fallbacks.count()); offset += 1) {
    await expect(fallbacks.nth(offset)).toBeVisible();
  }
}

export async function seriousAxeViolations(page: Page) {
  const result = await new AxeBuilder({ page }).analyze();
  return result.violations.filter(
    (violation) => violation.impact === 'serious' || violation.impact === 'critical',
  );
}

export async function assertNoSeriousAxeViolations(page: Page): Promise<void> {
  expect(await seriousAxeViolations(page)).toEqual([]);
}

export async function assertTextContrast(locator: Locator, minimum = 4.5): Promise<void> {
  const ratio = await locator.evaluate((element) => {
    const parse = (value: string) =>
      (value.match(/[\d.]+/g) ?? []).slice(0, 3).map((channel) => Number.parseFloat(channel) / 255);
    const luminance = (channels: number[]) =>
      channels.reduce((total, channel, index) => {
        const linear = channel <= 0.04045 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4;
        return total + linear * [0.2126, 0.7152, 0.0722][index];
      }, 0);
    const style = getComputedStyle(element);
    const foreground = luminance(parse(style.color));
    const background = luminance(parse(style.backgroundColor));
    return (Math.max(foreground, background) + 0.05) / (Math.min(foreground, background) + 0.05);
  });
  expect(ratio).toBeGreaterThanOrEqual(minimum);
}

export async function assertNoPageOverflow(page: Page): Promise<void> {
  const dimensions = await page.evaluate(() => ({
    client: document.documentElement.clientWidth,
    documentScroll: document.documentElement.scrollWidth,
    bodyScroll: document.body.scrollWidth,
  }));
  expect(
    dimensions.documentScroll,
    `document overflowed: ${JSON.stringify(dimensions)}`,
  ).toBeLessThanOrEqual(dimensions.client + 1);
  expect(dimensions.bodyScroll, `body overflowed: ${JSON.stringify(dimensions)}`).toBeLessThanOrEqual(
    dimensions.client + 1,
  );
}

export async function assertHonest320Reflow(page: Page): Promise<void> {
  expect(page.viewportSize()).toEqual({ width: 320, height: 844 });
  const observation = await page.evaluate(() => ({
    innerWidth,
    documentClient: document.documentElement.clientWidth,
    visualScale: visualViewport?.scale ?? 1,
    viewportMeta: document.querySelector('meta[name="viewport"]')?.getAttribute('content') ?? null,
    roots: ['html', 'body', 'main'].map((selector) => {
      const node = document.querySelector(selector);
      if (!node) throw new Error(`missing ${selector}`);
      const style = getComputedStyle(node);
      return { selector, zoom: style.zoom, transform: style.transform };
    }),
  }));
  expect(observation.innerWidth).toBe(320);
  expect(observation.documentClient).toBeGreaterThan(0);
  expect(observation.documentClient).toBeLessThanOrEqual(320);
  expect(observation.visualScale).toBe(1);
  if (observation.viewportMeta !== null) {
    const directives = observation.viewportMeta
      .toLowerCase()
      .split(',')
      .map((directive) => directive.trim());
    expect(directives).toContain('width=device-width');
    const initialScale = directives.find((directive) => directive.startsWith('initial-scale='));
    if (initialScale !== undefined) expect(initialScale).toBe('initial-scale=1');
    expect(directives.some((directive) => directive.startsWith('minimum-scale='))).toBe(false);
    expect(directives.some((directive) => directive.startsWith('maximum-scale='))).toBe(false);
    expect(directives.some((directive) => directive.startsWith('user-scalable='))).toBe(false);
  }
  for (const root of observation.roots) {
    expect(root.zoom, `${root.selector} scales the CSS viewport`).toBe('1');
    expect(root.transform, `${root.selector} transforms the CSS viewport`).toBe('none');
  }
}

export async function assertMinimumTargetSizes(locator: Locator): Promise<void> {
  const count = await locator.count();
  expect(count).toBeGreaterThan(0);
  for (let offset = 0; offset < count; offset += 1) {
    const target = locator.nth(offset);
    if (!(await target.isVisible())) continue;
    const box = await target.boundingBox();
    expect(box, `target ${offset} has no box`).not.toBeNull();
    expect(box!.width, `target ${offset} is narrower than 44px`).toBeGreaterThanOrEqual(44);
    expect(box!.height, `target ${offset} is shorter than 44px`).toBeGreaterThanOrEqual(44);
  }
}

export async function assertKeyboardFocusVisible(page: Page, target: Locator): Promise<void> {
  await page.locator('body').click({ position: { x: 1, y: 1 } });
  const handle = await target.elementHandle();
  expect(handle).not.toBeNull();
  let reached = false;
  for (let offset = 0; offset < 100; offset += 1) {
    await page.keyboard.press('Tab');
    reached = await page.evaluate((element) => document.activeElement === element, handle);
    if (reached) break;
  }
  expect(reached, 'target is absent from the keyboard focus order').toBe(true);
  const focus = await target.evaluate((element) => {
    const style = getComputedStyle(element);
    return {
      visiblePseudo: element.matches(':focus-visible'),
      outlineStyle: style.outlineStyle,
      outlineWidth: Number.parseFloat(style.outlineWidth),
      boxShadow: style.boxShadow,
    };
  });
  expect(focus.visiblePseudo).toBe(true);
  expect(
    (focus.outlineStyle !== 'none' && focus.outlineWidth >= 1) || focus.boxShadow !== 'none',
    `focus is not visibly drawn: ${JSON.stringify(focus)}`,
  ).toBe(true);
}

export async function assertAccessibleTooltip(
  page: Page,
  control: Locator,
  name: RegExp,
): Promise<void> {
  await expect(control).toBeVisible();
  const nativeTitle = (await control.getAttribute('title'))?.trim();
  if (nativeTitle) {
    expect(nativeTitle).toMatch(name);
    return;
  }
  await control.hover();
  await expect(page.getByRole('tooltip', { name })).toBeVisible();
}

export async function assertNoFakeExecutionControls(page: Page): Promise<void> {
  const executionLabel = /\b(run|submit|reset|start|begin|try|solv\w*|execut\w*|simulat\w*|comput\w*|calculat\w*|launch\w*|evaluat\w*|process\w*|generat\w*|analy[sz]\w*|predict\w*)\b/i;
  const actionLeading = /^\s*(run|submit|reset|start|begin|try|solv\w*|execut\w*|simulat\w*|comput\w*|calculat\w*|launch\w*|evaluat\w*|process\w*|generat\w*|analy[sz]\w*|predict\w*)\b/i;
  const normalize = (value: string) => value.trim().split(/\s+/u).filter(Boolean).join(' ');
  expect(await page.getByRole('button', { name: executionLabel }).count()).toBe(0);
  expect(await page.getByRole('link', { name: actionLeading }).count()).toBe(0);
  const controls = page.locator('button, [role="button"], input[type="button" i], input[type="submit" i], input[type="reset" i], input[type="image" i]');
  for (let offset = 0; offset < (await controls.count()); offset += 1) {
    const control = controls.nth(offset);
    expect(normalize(await control.innerText())).not.toMatch(executionLabel);
    const alternatives = await control.locator('img').evaluateAll((images) =>
      images
        .filter((image) => !image.closest('[hidden], [aria-hidden="true"]'))
        .map((image) => (image as HTMLImageElement).alt),
    );
    for (const alternative of alternatives) {
      expect(normalize(alternative)).not.toMatch(executionLabel);
    }
  }
  const inputs = page.locator('input[type="button" i], input[type="submit" i], input[type="reset" i], input[type="image" i]');
  for (let offset = 0; offset < (await inputs.count()); offset += 1) {
    expect(normalize(await inputs.nth(offset).inputValue())).not.toMatch(executionLabel);
  }
  const links = page.locator('a, [role="link"]');
  for (let offset = 0; offset < (await links.count()); offset += 1) {
    const link = links.nth(offset);
    expect(normalize(await link.innerText())).not.toMatch(actionLeading);
    const href = (await link.getAttribute('href'))?.trim().toLowerCase();
    expect(['', '#', 'javascript:void(0)']).not.toContain(href ?? '');
  }
}

type TableObservation = {
  document: { inner: number; client: number; scroll: number; bodyScroll: number };
  counts: { main: number; all: number; generic: number; direct: number; component: number };
  failures: {
    tag: number;
    relation: number;
    wrapper: number;
    role: number;
    focus: number;
    handler: number;
    rows: number;
    headers: number;
    cells: number;
    text: number;
    selection: number;
    size: number;
    bounds: number;
    overlap: number;
    concealment: number;
    localOverflow: number;
  };
  tables: Array<{
    client: number;
    scroll: number;
    left: number;
    right: number;
    display: string;
    overflowX: string;
    tableLayout: string;
    direct: boolean;
    component: boolean;
  }>;
};

async function observeTables(page: Page): Promise<TableObservation> {
  return page.evaluate((selectors) => {
    const normalize = (value: string) => value.trim().split(/\s+/u).filter(Boolean).join(' ');
    const tables = Array.from(document.querySelectorAll<HTMLTableElement>('main table'));
    const failures = {
      tag: 0,
      relation: 0,
      wrapper: 0,
      role: 0,
      focus: 0,
      handler: 0,
      rows: 0,
      headers: 0,
      cells: 0,
      text: 0,
      selection: 0,
      size: 0,
      bounds: 0,
      overlap: 0,
      concealment: 0,
      localOverflow: 0,
    };
    const observations = tables.map((table) => {
      const style = getComputedStyle(table);
      const box = table.getBoundingClientRect();
      const direct = table.matches(selectors.direct);
      const component = table.matches(selectors.component);
      const generic = table.matches(selectors.generic);
      const structural = [
        table,
        ...table.querySelectorAll('table, thead, tbody, tfoot, tr, th, td'),
      ];
      const cells = Array.from(table.querySelectorAll<HTMLTableCellElement>('th, td'));
      failures.tag += table.tagName === 'TABLE' ? 0 : 1;
      failures.relation += generic && direct !== component ? 0 : 1;
      failures.wrapper +=
        table.parentElement?.matches('.sl-markdown-content, .eq-stage__body') === true ? 0 : 1;
      failures.role += structural.some((node) => node.hasAttribute('role')) ? 1 : 0;
      failures.focus += table.hasAttribute('tabindex') || table.querySelector('[tabindex]') ? 1 : 0;
      failures.handler += structural.some((node) =>
        ['onkeydown', 'onkeypress', 'onkeyup'].some((name) => node.hasAttribute(name)),
      )
        ? 1
        : 0;
      failures.rows += table.rows.length > 0 && Array.from(table.rows).every((row) => row.cells.length > 0)
        ? 0
        : 1;
      failures.headers += table.querySelectorAll('th').length > 0 ? 0 : 1;
      failures.cells += cells.length > 0 ? 0 : 1;
      failures.text += cells.every((cell) => {
        const visible = normalize(cell.innerText);
        return visible.length > 0 && visible === normalize(cell.textContent ?? '');
      })
        ? 0
        : 1;
      failures.selection +=
        style.userSelect !== 'none' && cells.every((cell) => getComputedStyle(cell).userSelect !== 'none')
          ? 0
          : 1;
      failures.size +=
        box.width > 0 &&
        box.height > 0 &&
        cells.every((cell) => {
          const cellBox = cell.getBoundingClientRect();
          return cellBox.width > 0 && cellBox.height > 0 && Number.parseFloat(getComputedStyle(cell).fontSize) >= 12;
        })
          ? 0
          : 1;
      failures.bounds += box.left >= -1 && box.right <= document.documentElement.clientWidth + 1 ? 0 : 1;
      const cellBoxes = cells.map((cell) => cell.getBoundingClientRect());
      failures.overlap += cellBoxes.some((first, firstIndex) =>
        cellBoxes.slice(firstIndex + 1).some((second) =>
          Math.min(first.right, second.right) - Math.max(first.left, second.left) > 1 &&
          Math.min(first.bottom, second.bottom) - Math.max(first.top, second.top) > 1,
        ),
      )
        ? 1
        : 0;
      failures.concealment +=
        style.visibility === 'visible' &&
        style.opacity === '1' &&
        style.clip === 'auto' &&
        style.clipPath === 'none' &&
        !/hidden|clip/.test(style.overflowX) &&
        cells.every((cell) => {
          const cellStyle = getComputedStyle(cell);
          return (
            cellStyle.visibility === 'visible' &&
            cellStyle.opacity === '1' &&
            cellStyle.clip === 'auto' &&
            cellStyle.clipPath === 'none' &&
            !/hidden|clip/.test(cellStyle.overflowX) &&
            cellStyle.textOverflow !== 'ellipsis'
          );
        })
          ? 0
          : 1;
      failures.localOverflow += table.scrollWidth > table.clientWidth + 1 ? 1 : 0;
      return {
        client: table.clientWidth,
        scroll: table.scrollWidth,
        left: box.left,
        right: box.right,
        display: style.display,
        overflowX: style.overflowX,
        tableLayout: style.tableLayout,
        direct,
        component,
      };
    });
    return {
      document: {
        inner: innerWidth,
        client: document.documentElement.clientWidth,
        scroll: document.documentElement.scrollWidth,
        bodyScroll: document.body.scrollWidth,
      },
      counts: {
        main: tables.length,
        all: document.querySelectorAll('table').length,
        generic: document.querySelectorAll(selectors.generic).length,
        direct: document.querySelectorAll(selectors.direct).length,
        component: document.querySelectorAll(selectors.component).length,
      },
      failures,
      tables: observations,
    };
  }, TABLE_SELECTORS);
}

export async function assertTableInventory(page: Page, expected: TableRoute): Promise<TableObservation> {
  expect(new URL(page.url()).pathname).toBe(expected.route);
  const observation = await observeTables(page);
  expect(observation.counts).toEqual({
    main: expected.tables,
    all: expected.tables,
    generic: expected.tables,
    direct: expected.direct,
    component: expected.component,
  });
  expect(observation.failures.relation).toBe(0);
  expect(observation.tables.filter((table) => table.direct)).toHaveLength(expected.direct);
  expect(observation.tables.filter((table) => table.component)).toHaveLength(expected.component);
  return observation;
}

function assertTableContentAndSemantics(observation: TableObservation): void {
  expect(observation.document.inner).toBeGreaterThanOrEqual(observation.document.client);
  expect(observation.document.scroll).toBeLessThanOrEqual(observation.document.client + 1);
  expect(observation.document.bodyScroll).toBeLessThanOrEqual(observation.document.client + 1);
  const { localOverflow: _, ...failures } = observation.failures;
  expect(failures).toEqual({
    tag: 0,
    relation: 0,
    wrapper: 0,
    role: 0,
    focus: 0,
    handler: 0,
    rows: 0,
    headers: 0,
    cells: 0,
    text: 0,
    selection: 0,
    size: 0,
    bounds: 0,
    overlap: 0,
    concealment: 0,
  });
}

export async function assertParentCylinderRed(page: Page): Promise<void> {
  const expected = TABLE_ROUTES.find(({ route }) => route === '/gallery/exact-cylinder-steady-stokes/');
  expect(expected).toBeDefined();
  await page.goto(expected!.route);
  await assertCoreVisible(page);
  await assertSemanticStages(page);
  await assertSupportedStatement(page);
  const observation = await assertTableInventory(page, expected!);
  assertTableContentAndSemantics(observation);
  expect(observation.failures.localOverflow).toBe(1);
  expect(observation.tables).toHaveLength(1);
  expect(observation.tables[0]).toMatchObject({
    display: 'block',
    overflowX: 'auto',
    direct: false,
    component: true,
  });
  expect(observation.tables[0].scroll).toBeGreaterThan(observation.tables[0].client + 1);
  const violations = await seriousAxeViolations(page);
  expect(violations).toHaveLength(1);
  expect(violations[0].id).toBe('scrollable-region-focusable');
  expect(violations[0].impact).toBe('serious');
  expect(violations[0].nodes).toHaveLength(1);
  expect(violations[0].nodes[0].target).toEqual(['table']);
}

export async function assertProductTableRouteGreen(page: Page, expected: TableRoute): Promise<void> {
  await page.goto(expected.route);
  await assertCoreVisible(page);
  const observation = await assertTableInventory(page, expected);
  assertTableContentAndSemantics(observation);
  expect(observation.failures.localOverflow).toBe(0);
  expect(await seriousAxeViolations(page)).toEqual([]);
}

export async function assertTableFixtureGreen(page: Page): Promise<void> {
  const observation = await observeTables(page);
  expect(observation.counts).toEqual({ main: 2, all: 2, generic: 2, direct: 1, component: 1 });
  assertTableContentAndSemantics(observation);
  expect(observation.failures.localOverflow).toBe(0);
  expect(await seriousAxeViolations(page)).toEqual([]);
}

export function assertExactTableSelectorScope(css: string): void {
  const normalized = css.replace(/\/\*[\s\S]*?\*\//gu, '').replace(/\s+/gu, ' ').trim();
  expect(normalized).toMatch(
    /\.sl-markdown-content\s*>\s*table\s*,\s*\.sl-markdown-content\s+\.eq-stage__body\s*>\s*table\s*\{/u,
  );
  expect(normalized).not.toMatch(/\.sl-markdown-content\s+table\s*\{/u);
}

export async function layoutCssState(): Promise<{ css: string; sha256: string; parent: boolean }> {
  const path = resolve(dirname(fileURLToPath(import.meta.url)), '../src/styles/site/layout.css');
  const css = await readFile(path, 'utf8');
  const sha256 = createHash('sha256').update(css).digest('hex');
  return { css, sha256, parent: sha256 === PARENT_LAYOUT_SHA256 };
}

export async function assertReducedMotion(page: Page): Promise<void> {
  const moving = await page.evaluate(() => {
    const duration = (value: string) =>
      value
        .split(',')
        .map((part) => part.trim())
        .some((part) => (part.endsWith('ms') ? Number.parseFloat(part) : Number.parseFloat(part) * 1000) > 1);
    return [...document.querySelectorAll('header *, main *')]
      .filter((element) => {
        const style = getComputedStyle(element);
        return duration(style.animationDuration) || duration(style.transitionDuration);
      })
      .map((element) => `${element.tagName.toLowerCase()} ${element.getAttribute('aria-label') ?? element.textContent?.trim().slice(0, 40) ?? ''}`);
  });
  expect(moving).toEqual([]);
}

export async function attachGrossScreenshot(
  page: Page,
  testInfo: TestInfo,
  name: string,
): Promise<void> {
  await assertCoreVisible(page);
  const main = await page.getByRole('main').boundingBox();
  expect(main).not.toBeNull();
  expect(main!.width).toBeGreaterThan(100);
  expect(main!.height).toBeGreaterThan(100);
  await testInfo.attach(name, {
    body: await page.screenshot({ fullPage: true }),
    contentType: 'image/png',
  });
}
