import AxeBuilder from '@axe-core/playwright';
import {
  chromium,
  expect,
  type Browser,
  type CDPSession,
  type Locator,
  type Page,
  type TestInfo,
} from '@playwright/test';
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
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
  '/gallery/mixed-boundary-elasticity/',
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
  'One presentation-only 2D steady incompressible Stokes exact-cylinder demonstration rendered through exact Geometry, typed Gmsh policy, and the root Result path; output counts, digests, numerical values, and pixels are not independently verified.';

export const TABLE_SELECTORS = {
  generic: '.sl-markdown-content table:not(:where(.not-content *))',
  direct: '.sl-markdown-content > table',
  component: '.sl-markdown-content .eq-stage__body > table',
} as const;

export const TABLE_ROUTES = [
  { route: '/capabilities/', tables: 1, direct: 1, component: 0 },
  { route: '/evidence/', tables: 966, direct: 966, component: 0 },
  { route: '/gallery/exact-cylinder-steady-stokes/', tables: 1, direct: 0, component: 1 },
  { route: '/reference/control-v2/', tables: 1, direct: 1, component: 0 },
  { route: '/reference/python/', tables: 2, direct: 2, component: 0 },
  { route: '/reference/rust/', tables: 3, direct: 3, component: 0 },
] as const;

export const PARENT_FORCED_TABLE_RESULTS = {
  '/capabilities/': null,
  '/evidence/': null,
  '/gallery/exact-cylinder-steady-stokes/': 'table',
  '/reference/control-v2/': 'table',
  '/reference/python/': 'table:nth-child(4)',
  '/reference/rust/': 'table:nth-child(5)',
} as const;

export type TableRoute = (typeof TABLE_ROUTES)[number];
export type OrdinaryRoutePlan = Readonly<{
  A: readonly string[];
  B: readonly string[];
  C: readonly string[];
}>;

const REFERENCE_START = SITE_ROUTES.indexOf('/reference/');

export function createOrdinaryRoutePlan(): OrdinaryRoutePlan {
  const plan = {
    A: SITE_ROUTES.filter((route) => route === '/evidence/'),
    B: SITE_ROUTES.slice(0, REFERENCE_START).filter((route) => route !== '/evidence/'),
    C: SITE_ROUTES.slice(REFERENCE_START),
  };
  return Object.freeze({
    A: Object.freeze(plan.A),
    B: Object.freeze(plan.B),
    C: Object.freeze(plan.C),
  });
}

export function assertOrdinaryRoutePlan(plan: OrdinaryRoutePlan): readonly string[] {
  if (REFERENCE_START < 1) throw new Error('route authority missing /reference/');
  if (SITE_ROUTES.length !== 35 || new Set(SITE_ROUTES).size !== 35) {
    throw new Error('route authority is not 35 unique entries');
  }
  const entries = (['A', 'B', 'C'] as const).flatMap((chunk) =>
    plan[chunk].map((route) => ({ chunk, route })),
  );
  const authority = new Set<string>(SITE_ROUTES);
  const unknown = entries.find(({ route }) => !authority.has(route));
  if (unknown) throw new Error(`ORDER-UNKNOWN ${unknown.chunk}: ${unknown.route}`);

  const seen = new Set<string>();
  for (const { route } of entries) {
    if (seen.has(route)) throw new Error(`ORDER-DUPLICATE: ${route}`);
    seen.add(route);
  }
  const missing = SITE_ROUTES.find((route) => !seen.has(route));
  if (missing) throw new Error(`ORDER-MISSING: ${missing}`);

  const expected = createOrdinaryRoutePlan();
  const cardinalities = { A: 1, B: 15, C: 19 } as const;
  for (const chunk of ['A', 'B', 'C'] as const) {
    if (plan[chunk].length !== cardinalities[chunk]) {
      throw new Error(`ORDER-CARDINALITY ${chunk}: ${plan[chunk].length}`);
    }
    const expectedMembers = new Set(expected[chunk]);
    const wrongMember = plan[chunk].find((route) => !expectedMembers.has(route));
    if (wrongMember) throw new Error(`ORDER-MEMBERSHIP ${chunk}: ${wrongMember}`);
    if (plan[chunk].some((route, index) => route !== expected[chunk][index])) {
      throw new Error(`ORDER-REORDER ${chunk}`);
    }
  }
  if (entries.length !== 35 || seen.size !== 35) {
    throw new Error('ORDER-UNION is not exactly 35 entries');
  }

  const byRoute = new Map(entries.map((entry) => [entry.route, entry]));
  if (byRoute.size !== 35) throw new Error('ORDER-CANONICAL duplicate identity');
  const canonical = SITE_ROUTES.map((route) => {
    const entry = byRoute.get(route);
    if (!entry) throw new Error(`ORDER-CANONICAL missing: ${route}`);
    return entry.route;
  });
  if (canonical.some((route, index) => route !== SITE_ROUTES[index])) {
    throw new Error('ORDER-CANONICAL authority mismatch');
  }
  return Object.freeze(canonical);
}

const CHROME_VERSION = '151.0.7922.34';
const CHROME_BYTES = 290_614_600;
const CHROME_SHA256 = '0b20b130e7edd9dd51873be867761295fe0cfad490c2b9a64f95bd3cfc08fa71';
const BROWSERS_JSON_SHA256 = 'f306eed529599b1eaf2f8a85db9de2b23e1a3fe36c2b66434b7c9434fb627a99';
export const PARENT_LAYOUT_SHA256 =
  '9a6aa92fe82bdc7ed7d70a3891c369d10bd73ae0cf2f11d087dd721d02befe0a';
const PARENT_LAYOUT_COMMIT = '4fc67a9fc94aeedd44a4ace31d406ac949c81f12';

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

export async function assertOrdinaryRoutePage(page: Page, route: string): Promise<void> {
  const observation = await page.evaluate((expectedRoute) => {
    const main = document.querySelector('main');
    const heading = main?.querySelector('h1');
    if (!(main instanceof HTMLElement) || !(heading instanceof HTMLElement)) {
      throw new Error(`missing ordinary core at ${expectedRoute}`);
    }
    const mainStyle = getComputedStyle(main);
    const headingStyle = getComputedStyle(heading);
    const mainBox = main.getBoundingClientRect();
    const headingBox = heading.getBoundingClientRect();
    return {
      pathname: location.pathname,
      mainTextLength: main.innerText.trim().length,
      mainVisible:
        mainStyle.display !== 'none' &&
        mainStyle.visibility === 'visible' &&
        Number.parseFloat(mainStyle.opacity) > 0 &&
        mainBox.width > 0 &&
        mainBox.height > 0,
      headingVisible:
        headingStyle.display !== 'none' &&
        headingStyle.visibility === 'visible' &&
        Number.parseFloat(headingStyle.opacity) > 0 &&
        headingBox.width > 0 &&
        headingBox.height > 0,
      documentClient: document.documentElement.clientWidth,
      documentScroll: document.documentElement.scrollWidth,
      bodyScroll: document.body.scrollWidth,
    };
  }, route);
  expect(observation.pathname).toBe(route);
  expect(observation.mainTextLength).toBeGreaterThan(20);
  expect(observation.mainVisible).toBe(true);
  expect(observation.headingVisible).toBe(true);
  expect(observation.documentClient).toBeGreaterThan(0);
  expect(observation.documentScroll).toBeLessThanOrEqual(observation.documentClient + 1);
  expect(observation.bodyScroll).toBeLessThanOrEqual(observation.documentClient + 1);
}

const tableObserverSessions = new WeakMap<Page, CDPSession>();

export async function installTableObserver(page: Page): Promise<void> {
  expect(page.url()).toBe('about:blank');
  await expect(page.locator('main, table')).toHaveCount(0);
  expect(tableObserverSessions.has(page)).toBe(false);
  const session = await page.context().newCDPSession(page);
  await session.send('Runtime.enable');
  tableObserverSessions.set(page, session);
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
    links: number;
    text: number;
    selection: number;
    size: number;
    bounds: number;
    overlap: number;
    concealment: number;
    localOverflow: number;
    parentLocalOverflow: number;
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
  const session = tableObserverSessions.get(page);
  if (!session) throw new Error('table observer was not installed before fixture or navigation');
  const observation = await page.evaluate((selectors) => {
    const normalize = (value: string) => value.trim().split(/\s+/u).filter(Boolean).join(' ');
    const tables = Array.from(document.querySelectorAll<HTMLTableElement>('main table'));
    const cssLength = (value: string, extent: number): number | null => {
      const match = value.trim().match(/^([+-]?(?:\d+(?:\.\d*)?|\.\d+))(px|%)?$/iu);
      if (!match) return null;
      const magnitude = Number.parseFloat(match[1]);
      return match[2]?.toLowerCase() === '%' ? (magnitude * extent) / 100 : magnitude;
    };
    const zeroLength = (value: string) => {
      const parsed = cssLength(value, 1);
      return parsed !== null && Math.abs(parsed) < Number.EPSILON;
    };
    const splitTopLevelComma = (value: string) => {
      const parts: string[] = [];
      let start = 0;
      let depth = 0;
      let quote = '';
      for (let offset = 0; offset < value.length; offset += 1) {
        const character = value[offset];
        if (quote) {
          if (character === '\\') offset += 1;
          else if (character === quote) quote = '';
        } else if (character === '"' || character === "'") quote = character;
        else if (character === '\\') offset += 1;
        else if (character === '(') depth += 1;
        else if (character === ')') depth -= 1;
        else if (character === ',' && depth === 0) {
          parts.push(value.slice(start, offset).trim());
          start = offset + 1;
        }
      }
      parts.push(value.slice(start).trim());
      return parts;
    };
    const fullyClipped = (element: Element, style: CSSStyleDeclaration) => {
      const legacy = style.clip.trim().match(/^rect\((.*)\)$/iu);
      if (legacy) {
        const coordinates = splitTopLevelComma(legacy[1]);
        if (coordinates.length === 4) {
          const [top, right, bottom, left] = coordinates.map((value) => cssLength(value, 1));
          if (
            top !== null &&
            right !== null &&
            bottom !== null &&
            left !== null &&
            (bottom <= top || right <= left)
          ) return true;
        }
      }

      const clipPath = style.clipPath.trim();
      if (!clipPath || clipPath === 'none') return false;
      const basicShape = clipPath.match(
        /^([a-z-]+)\(([\s\S]*)\)(?:\s+(?:margin-box|border-box|padding-box|content-box|fill-box|stroke-box|view-box))?$/iu,
      );
      if (!basicShape) return false;
      const name = basicShape[1].toLowerCase();
      const body = basicShape[2].trim();
      if (name === 'circle') {
        const radius = body.split(/\s+at\s+/iu, 1)[0].trim();
        return zeroLength(radius);
      }
      if (name === 'ellipse') {
        const radii = body.split(/\s+at\s+/iu, 1)[0].trim().split(/\s+/u);
        return radii.length === 2 && radii.some(zeroLength);
      }
      if (name === 'inset') {
        const offsets = body.split(/\s+round\s+/iu, 1)[0].trim().split(/\s+/u);
        if (offsets.length < 1 || offsets.length > 4) return false;
        const expanded =
          offsets.length === 1
            ? [offsets[0], offsets[0], offsets[0], offsets[0]]
            : offsets.length === 2
              ? [offsets[0], offsets[1], offsets[0], offsets[1]]
              : offsets.length === 3
                ? [offsets[0], offsets[1], offsets[2], offsets[1]]
                : offsets;
        const box = element.getBoundingClientRect();
        const top = cssLength(expanded[0], box.height);
        const right = cssLength(expanded[1], box.width);
        const bottom = cssLength(expanded[2], box.height);
        const left = cssLength(expanded[3], box.width);
        return (
          top !== null &&
          right !== null &&
          bottom !== null &&
          left !== null &&
          (top + bottom >= box.height || left + right >= box.width)
        );
      }
      if (name === 'polygon') {
        const points = splitTopLevelComma(body);
        const fillRule = /^(evenodd|nonzero)$/iu.exec(points[0] ?? '')?.[1]?.toLowerCase();
        if (fillRule) points.shift();
        if (points.length < 3) return false;
        const box = element.getBoundingClientRect();
        const coordinates = points.map((point) => {
          const pair = point.trim().split(/\s+/u);
          if (pair.length !== 2) return null;
          const x = cssLength(pair[0], box.width);
          const y = cssLength(pair[1], box.height);
          return x === null || y === null ? null : { x, y };
        });
        if (coordinates.some((point) => point === null)) return false;
        const concrete = coordinates as Array<{ x: number; y: number }>;
        const segments = concrete.map((start, index) => ({
          start,
          end: concrete[(index + 1) % concrete.length],
        }));
        const epsilon = 1e-7;
        const cross = (left: { x: number; y: number }, right: { x: number; y: number }) =>
          left.x * right.y - left.y * right.x;
        const yEvents = concrete.map(({ y }) => y);
        for (let left = 0; left < segments.length; left += 1) {
          const first = segments[left];
          const firstVector = {
            x: first.end.x - first.start.x,
            y: first.end.y - first.start.y,
          };
          for (let right = left + 1; right < segments.length; right += 1) {
            const second = segments[right];
            const secondVector = {
              x: second.end.x - second.start.x,
              y: second.end.y - second.start.y,
            };
            const denominator = cross(firstVector, secondVector);
            if (Math.abs(denominator) <= epsilon) continue;
            const displacement = {
              x: second.start.x - first.start.x,
              y: second.start.y - first.start.y,
            };
            const firstDistance = cross(displacement, secondVector) / denominator;
            const secondDistance = cross(displacement, firstVector) / denominator;
            if (
              firstDistance > epsilon &&
              firstDistance < 1 - epsilon &&
              secondDistance > epsilon &&
              secondDistance < 1 - epsilon
            ) {
              yEvents.push(first.start.y + firstDistance * firstVector.y);
            }
          }
        }
        const levels = yEvents
          .sort((left, right) => left - right)
          .filter((value, index, all) => index === 0 || Math.abs(value - all[index - 1]) > epsilon);
        for (let level = 0; level + 1 < levels.length; level += 1) {
          if (levels[level + 1] - levels[level] <= epsilon) continue;
          const y = (levels[level] + levels[level + 1]) / 2;
          const crossings = segments
            .filter(({ start, end }) => (start.y <= y && end.y > y) || (end.y <= y && start.y > y))
            .map(({ start, end }) => ({
              x: start.x + ((y - start.y) * (end.x - start.x)) / (end.y - start.y),
              winding: end.y > start.y ? 1 : -1,
            }))
            .sort((left, right) => left.x - right.x);
          let winding = 0;
          for (let crossing = 0; crossing < crossings.length; ) {
            const x = crossings[crossing].x;
            let delta = 0;
            let next = crossing;
            while (next < crossings.length && Math.abs(crossings[next].x - x) <= epsilon) {
              delta += crossings[next].winding;
              next += 1;
            }
            winding += delta;
            if (
              next < crossings.length &&
              crossings[next].x - x > epsilon &&
              (fillRule === 'evenodd' ? Math.abs(winding) % 2 === 1 : winding !== 0)
            ) return false;
            crossing = next;
          }
        }
        return true;
      }
      return false;
    };
    const relevantChain = (table: HTMLTableElement) => {
      const chain: Element[] = [table];
      let ancestor = table.parentElement;
      while (ancestor) {
        chain.push(ancestor);
        if (ancestor.matches('.sl-markdown-content')) break;
        ancestor = ancestor.parentElement;
      }
      return chain;
    };
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
      links: 0,
      text: 0,
      selection: 0,
      size: 0,
      bounds: 0,
      overlap: 0,
      concealment: 0,
      localOverflow: 0,
      parentLocalOverflow: 0,
    };
    const observations = tables.map((table) => {
      const style = getComputedStyle(table);
      const box = table.getBoundingClientRect();
      const direct = table.matches(selectors.direct);
      const component = table.matches(selectors.component);
      const generic = table.matches(selectors.generic);
      const chain = relevantChain(table);
      const parents = chain.slice(1);
      const structural = [
        table,
        ...table.querySelectorAll('table, thead, tbody, tfoot, tr, th, td'),
      ];
      const cells = Array.from(table.querySelectorAll<HTMLTableCellElement>('th, td'));
      const guarded = [...new Set([...structural, ...parents])];
      failures.tag += table.tagName === 'TABLE' ? 0 : 1;
      failures.relation += generic && direct !== component ? 0 : 1;
      failures.wrapper +=
        table.parentElement?.matches('.sl-markdown-content, .eq-stage__body') === true ? 0 : 1;
      failures.role += guarded.some((node) => node.hasAttribute('role')) ? 1 : 0;
      failures.focus +=
        chain.some(
          (node) =>
            node.hasAttribute('tabindex') ||
            (node instanceof HTMLElement && node.tabIndex >= 0) ||
            (node.getAttribute('contenteditable') ?? '').toLowerCase() === 'true',
        ) || table.querySelector('[tabindex], [contenteditable="true" i]')
          ? 1
          : 0;
      failures.handler += guarded.some((node) =>
        ['keydown', 'keypress', 'keyup'].some(
          (event) =>
            node.hasAttribute(`on${event}`) ||
            typeof (node as unknown as Record<string, unknown>)[`on${event}`] === 'function',
        ),
      ) ? 1 : 0;
      failures.rows += table.rows.length > 0 && Array.from(table.rows).every((row) => row.cells.length > 0)
        ? 0
        : 1;
      failures.headers += table.querySelectorAll('th').length > 0 ? 0 : 1;
      failures.cells += cells.length > 0 ? 0 : 1;
      const links = Array.from(table.querySelectorAll<HTMLAnchorElement>('a'));
      failures.links += links.every(
        (link) =>
          normalize(link.innerText).length > 0 &&
          Boolean(link.getAttribute('href')?.trim()) &&
          !['#', 'javascript:void(0)'].includes(link.getAttribute('href')!.trim().toLowerCase()),
      ) ? 0 : 1;
      const textNodes = cells.flatMap((cell) => {
        const nodes: Text[] = [];
        const walker = document.createTreeWalker(cell, NodeFilter.SHOW_TEXT);
        while (walker.nextNode()) {
          const node = walker.currentNode as Text;
          if (normalize(node.data).length > 0) nodes.push(node);
        }
        return nodes;
      });
      const textFacts = textNodes.map((node) => {
        const owner = node.parentElement;
        const range = document.createRange();
        range.selectNodeContents(node);
        const boxes = Array.from(range.getClientRects());
        const ancestors: Element[] = [];
        let current: Element | null = owner;
        while (current) {
          ancestors.push(current);
          if (current === table) break;
          current = current.parentElement;
        }
        const styles = ancestors.map((element) => ({
          element,
          style: getComputedStyle(element),
        }));
        return {
          bounded:
            owner !== null &&
            boxes.length > 0 &&
            boxes.every((textBox) => textBox.width > 0 && textBox.height > 0) &&
            Number.parseFloat(getComputedStyle(owner).fontSize) >= 12,
          selectable: styles.every(({ style: textStyle }) => textStyle.userSelect !== 'none'),
          visible: styles.every(({ element, style: textStyle }) => {
            const horizontalTruncation =
              element instanceof HTMLElement &&
              /hidden|clip/u.test(textStyle.overflowX) &&
              element.scrollWidth > element.clientWidth + 1;
            const verticalTruncation =
              element instanceof HTMLElement &&
              /hidden|clip/u.test(textStyle.overflowY) &&
              element.scrollHeight > element.clientHeight + 1;
            return (
              textStyle.display !== 'none' &&
              !/hidden|collapse/u.test(textStyle.visibility) &&
              Number.parseFloat(textStyle.opacity) > 0 &&
              !fullyClipped(element, textStyle) &&
              !horizontalTruncation &&
              !verticalTruncation &&
              !(textStyle.textOverflow === 'ellipsis' && horizontalTruncation)
            );
          }),
        };
      });
      failures.text +=
        textNodes.length > 0 &&
        cells.every(
          (cell) =>
            normalize(cell.innerText).length > 0 &&
            normalize(cell.textContent ?? '').length > 0,
        )
        ? 0
        : 1;
      failures.selection +=
        textFacts.length > 0 && textFacts.every(({ selectable }) => selectable) ? 0 : 1;
      failures.size +=
        box.width > 0 &&
        box.height > 0 &&
        cells.every((cell) => {
          const cellBox = cell.getBoundingClientRect();
          return cellBox.width > 0 && cellBox.height > 0 && Number.parseFloat(getComputedStyle(cell).fontSize) >= 12;
        }) &&
        textFacts.every(({ bounded }) => bounded)
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
        Number.parseFloat(style.opacity) > 0 &&
        !fullyClipped(table, style) &&
        !/hidden|clip/.test(style.overflowX) &&
        cells.every((cell) => {
          const cellStyle = getComputedStyle(cell);
          return (
            cellStyle.visibility === 'visible' &&
            Number.parseFloat(cellStyle.opacity) > 0 &&
            !fullyClipped(cell, cellStyle) &&
            !/hidden|clip/.test(cellStyle.overflowX) &&
            cellStyle.textOverflow !== 'ellipsis'
          );
        }) &&
        textFacts.every(({ visible }) => visible)
          ? 0
          : 1;
      failures.localOverflow += table.scrollWidth > table.clientWidth + 1 ? 1 : 0;
      failures.parentLocalOverflow += parents.some((parent) => {
        const parentStyle = getComputedStyle(parent);
        return (
          /auto|scroll/u.test(parentStyle.overflowX) &&
          parent instanceof HTMLElement &&
          parent.scrollWidth > parent.clientWidth + 1
        );
      }) ? 1 : 0;
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
  const registered = await session.send('Runtime.evaluate', {
    expression: `(() => {
      const unique = new Set();
      for (const table of document.querySelectorAll('main table')) {
        unique.add(table);
        let ancestor = table.parentElement;
        while (ancestor) {
          unique.add(ancestor);
          if (ancestor.matches('.sl-markdown-content')) break;
          ancestor = ancestor.parentElement;
        }
      }
      let count = 0;
      for (const node of unique) {
        const listeners = getEventListeners(node);
        for (const type of ['keydown', 'keyup', 'keypress']) count += (listeners[type] || []).length;
      }
      return count;
    })()`,
    includeCommandLineAPI: true,
    returnByValue: true,
  });
  if (registered.exceptionDetails || typeof registered.result.value !== 'number') {
    throw new Error('registered keyboard-handler observation failed closed');
  }
  observation.failures.handler += registered.result.value > 0 ? 1 : 0;
  return observation;
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
    links: 0,
    text: 0,
    selection: 0,
    size: 0,
    bounds: 0,
    overlap: 0,
    concealment: 0,
    parentLocalOverflow: 0,
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

export async function assertParentForcedTableBoundary(
  page: Page,
  expected: TableRoute,
): Promise<void> {
  await page.goto(expected.route);
  await assertOrdinaryRoutePage(page, expected.route);
  await expect(page.locator('main table')).toHaveCount(expected.tables);
  const target = PARENT_FORCED_TABLE_RESULTS[expected.route];
  const violations = await seriousAxeViolations(page);
  if (target === null) {
    expect(violations).toEqual([]);
    return;
  }
  expect(violations).toHaveLength(1);
  expect(violations[0].id).toBe('scrollable-region-focusable');
  expect(violations[0].impact).toBe('serious');
  expect(violations[0].nodes).toHaveLength(1);
  expect(violations[0].nodes[0].target).toEqual([target]);
}

export async function assertTableFixtureGreen(page: Page): Promise<void> {
  const observation = await observeTables(page);
  expect(observation.counts).toEqual({ main: 2, all: 2, generic: 2, direct: 1, component: 1 });
  assertTableContentAndSemantics(observation);
  expect(observation.failures.localOverflow).toBe(0);
  expect(await seriousAxeViolations(page)).toEqual([]);
}

const ZERO_TABLE_FAILURES: TableObservation['failures'] = {
  tag: 0,
  relation: 0,
  wrapper: 0,
  role: 0,
  focus: 0,
  handler: 0,
  rows: 0,
  headers: 0,
  cells: 0,
  links: 0,
  text: 0,
  selection: 0,
  size: 0,
  bounds: 0,
  overlap: 0,
  concealment: 0,
  localOverflow: 0,
  parentLocalOverflow: 0,
};

export async function assertTableFixtureOnlyFailure(
  page: Page,
  failure: keyof TableObservation['failures'],
): Promise<void> {
  const observation = await observeTables(page);
  expect(observation.counts).toEqual({ main: 2, all: 2, generic: 2, direct: 1, component: 1 });
  expect(observation.failures).toEqual({ ...ZERO_TABLE_FAILURES, [failure]: 1 });
  expect(await seriousAxeViolations(page)).toEqual([]);
}

type CssRule = { selectors: string[]; declarations: ReadonlyMap<string, string> };

function stripCssComments(css: string): string {
  let result = '';
  let quote = '';
  for (let offset = 0; offset < css.length; offset += 1) {
    const character = css[offset];
    if (quote) {
      result += character;
      if (character === '\\') {
        offset += 1;
        result += css[offset] ?? '';
      } else if (character === quote) quote = '';
      continue;
    }
    if (character === '"' || character === "'") {
      quote = character;
      result += character;
      continue;
    }
    if (character === '/' && css[offset + 1] === '*') {
      const end = css.indexOf('*/', offset + 2);
      if (end < 0) throw new Error('unsupported unterminated CSS comment');
      result += ' ';
      offset = end + 1;
      continue;
    }
    result += character;
  }
  if (quote) throw new Error('unsupported unterminated CSS string');
  return result;
}

function splitCssTopLevel(value: string, delimiter: string): string[] {
  const parts: string[] = [];
  let start = 0;
  let parentheses = 0;
  let brackets = 0;
  let quote = '';
  for (let offset = 0; offset < value.length; offset += 1) {
    const character = value[offset];
    if (quote) {
      if (character === '\\') offset += 1;
      else if (character === quote) quote = '';
      continue;
    }
    if (character === '"' || character === "'") quote = character;
    else if (character === '\\') offset += 1;
    else if (character === '(') parentheses += 1;
    else if (character === ')') parentheses -= 1;
    else if (character === '[') brackets += 1;
    else if (character === ']') brackets -= 1;
    else if (character === delimiter && parentheses === 0 && brackets === 0) {
      parts.push(value.slice(start, offset).trim());
      start = offset + 1;
    }
    if (parentheses < 0 || brackets < 0) throw new Error('unsupported unbalanced CSS grammar');
  }
  if (quote || parentheses !== 0 || brackets !== 0) {
    throw new Error('unsupported unbalanced CSS grammar');
  }
  parts.push(value.slice(start).trim());
  if (parts.some((part) => part.length === 0)) throw new Error('empty CSS list branch');
  return parts;
}

function matchingCssBrace(css: string, opening: number): number {
  let depth = 1;
  let quote = '';
  for (let offset = opening + 1; offset < css.length; offset += 1) {
    const character = css[offset];
    if (quote) {
      if (character === '\\') offset += 1;
      else if (character === quote) quote = '';
      continue;
    }
    if (character === '"' || character === "'") quote = character;
    else if (character === '\\') offset += 1;
    else if (character === '{') depth += 1;
    else if (character === '}' && --depth === 0) return offset;
  }
  throw new Error('unsupported unbalanced CSS block');
}

function parseCssDeclarations(block: string): ReadonlyMap<string, string> {
  if (block.includes('{') || block.includes('}')) throw new Error('unsupported nested qualified rule');
  const declarations = new Map<string, string>();
  const source = block.trim().replace(/;\s*$/u, '');
  for (const declaration of splitCssTopLevel(source, ';')) {
    const colon = declaration.indexOf(':');
    if (colon < 1) throw new Error(`unsupported CSS declaration: ${declaration}`);
    const property = declaration.slice(0, colon).trim().toLowerCase();
    const value = declaration.slice(colon + 1).trim().replace(/\s+/gu, ' ');
    if (!/^-{0,2}[a-z][\w-]*$/u.test(property) || !value || declarations.has(property)) {
      throw new Error(`unsupported CSS declaration: ${declaration}`);
    }
    declarations.set(property, value);
  }
  if (declarations.size === 0) throw new Error('empty CSS declaration block');
  return declarations;
}

function parseCssRules(input: string): CssRule[] {
  const css = stripCssComments(input);
  const rules: CssRule[] = [];
  const parseList = (source: string) => {
    let offset = 0;
    while (offset < source.length) {
      while (/\s/u.test(source[offset] ?? '')) offset += 1;
      if (offset >= source.length) break;
      let opening = -1;
      let parentheses = 0;
      let brackets = 0;
      let quote = '';
      for (let cursor = offset; cursor < source.length; cursor += 1) {
        const character = source[cursor];
        if (quote) {
          if (character === '\\') cursor += 1;
          else if (character === quote) quote = '';
          continue;
        }
        if (character === '"' || character === "'") quote = character;
        else if (character === '\\') cursor += 1;
        else if (character === '(') parentheses += 1;
        else if (character === ')') parentheses -= 1;
        else if (character === '[') brackets += 1;
        else if (character === ']') brackets -= 1;
        else if (character === ';' && parentheses === 0 && brackets === 0) {
          throw new Error('unsupported statement at-rule or stray declaration');
        } else if (character === '{' && parentheses === 0 && brackets === 0) {
          opening = cursor;
          break;
        }
      }
      if (opening < 0) throw new Error('unsupported trailing CSS syntax');
      const prelude = source.slice(offset, opening).trim();
      const closing = matchingCssBrace(source, opening);
      const block = source.slice(opening + 1, closing);
      if (!prelude) throw new Error('empty CSS rule prelude');
      if (prelude.startsWith('@')) {
        if (!/^@(media|supports|layer|container)\b/iu.test(prelude)) {
          throw new Error(`unsupported CSS rule-list authority: ${prelude}`);
        }
        parseList(block);
      } else {
        rules.push({
          selectors: splitCssTopLevel(prelude, ',').map((selector) =>
            selector.replace(/\s+/gu, ' ').replace(/\s*([>+~])\s*/gu, ' $1 ').trim(),
          ),
          declarations: parseCssDeclarations(block),
        });
      }
      offset = closing + 1;
    }
  };
  parseList(css);
  if (rules.length === 0) throw new Error('empty CSS rule authority');
  return rules;
}

function cssRuleSignature(rule: CssRule): string {
  return `${rule.selectors.join(',')}|${[...rule.declarations].map(([key, value]) => `${key}:${value}`).join(';')}`;
}

function decodeCssEscapes(selector: string): string {
  return selector.replace(
    /\\(?:([0-9a-f]{1,6})\s?|([\s\S]))/giu,
    (_escape, hexadecimal: string | undefined, escaped: string | undefined) =>
      hexadecimal ? String.fromCodePoint(Number.parseInt(hexadecimal, 16)) : (escaped ?? ''),
  );
}

function selectorTargetsTable(selector: string): boolean {
  const selectorFunctions = new Set([
    'is',
    'where',
    'not',
    'has',
    'matches',
    '-webkit-any',
    '-moz-any',
    'host',
    'host-context',
    'current',
  ]);
  const matchingParenthesis = (opening: number) => {
    let depth = 1;
    let quote = '';
    for (let offset = opening + 1; offset < selector.length; offset += 1) {
      const character = selector[offset];
      if (quote) {
        if (character === '\\') offset += 1;
        else if (character === quote) quote = '';
      } else if (character === '"' || character === "'") quote = character;
      else if (character === '\\') offset += 1;
      else if (character === '(') depth += 1;
      else if (character === ')' && --depth === 0) return offset;
    }
    throw new Error('unsupported unbalanced functional selector');
  };
  const matchingBracket = (opening: number) => {
    let quote = '';
    for (let offset = opening + 1; offset < selector.length; offset += 1) {
      const character = selector[offset];
      if (quote) {
        if (character === '\\') offset += 1;
        else if (character === quote) quote = '';
      } else if (character === '"' || character === "'") quote = character;
      else if (character === '\\') offset += 1;
      else if (character === ']') return offset;
    }
    throw new Error('unsupported unbalanced attribute selector');
  };
  const nthParts = (body: string): { formula: string; selectorList: string | null } => {
    let parentheses = 0;
    let brackets = 0;
    let quote = '';
    for (let offset = 0; offset < body.length; offset += 1) {
      const character = body[offset];
      if (quote) {
        if (character === '\\') offset += 1;
        else if (character === quote) quote = '';
      } else if (character === '"' || character === "'") quote = character;
      else if (character === '\\') offset += 1;
      else if (character === '(') parentheses += 1;
      else if (character === ')') parentheses -= 1;
      else if (character === '[') brackets += 1;
      else if (character === ']') brackets -= 1;
      else if (
        parentheses === 0 &&
        brackets === 0 &&
        body.slice(offset, offset + 2).toLowerCase() === 'of' &&
        /\s/u.test(body[offset - 1] ?? '') &&
        /\s/u.test(body[offset + 2] ?? '')
      ) {
        return {
          formula: body.slice(0, offset).trim(),
          selectorList: body.slice(offset + 2).trim(),
        };
      }
    }
    return { formula: body.trim(), selectorList: null };
  };
  let typePosition = true;
  for (let offset = 0; offset < selector.length; offset += 1) {
    const character = selector[offset];
    if (/\s/u.test(character)) {
      while (/\s/u.test(selector[offset + 1] ?? '')) offset += 1;
      typePosition = true;
      continue;
    }
    if (character === ',' || character === '>' || character === '+' || character === '~') {
      typePosition = true;
      continue;
    }
    if (character === '|' && selector[offset + 1] === '|') {
      offset += 1;
      typePosition = true;
      continue;
    }
    if (character === '[') {
      offset = matchingBracket(offset);
      typePosition = false;
      continue;
    }
    if (character === ':' && selector[offset + 1] !== ':') {
      const name = selector.slice(offset + 1).match(/^[a-z_][\w-]*/iu)?.[0];
      if (!name) throw new Error(`unsupported pseudo-class selector: ${selector}`);
      const opening = offset + 1 + name.length;
      if (selector[opening] === '(') {
        const closing = matchingParenthesis(opening);
        const body = selector.slice(opening + 1, closing);
        const lowerName = name.toLowerCase();
        if (selectorFunctions.has(lowerName)) {
          if (splitCssTopLevel(body, ',').some(selectorTargetsTable)) return true;
        } else if (/^nth-(?:last-)?child$/u.test(lowerName)) {
          const { formula, selectorList } = nthParts(body);
          const compactFormula = formula.replace(/\s+/gu, '');
          if (!/^(?:odd|even|[+-]?\d+|[+-]?(?:\d*)n(?:[+-]\d+)?)$/iu.test(compactFormula)) {
            throw new Error(`unsupported functional selector: ${selector}`);
          }
          if (selectorList !== null) {
            if (!selectorList) throw new Error(`unsupported functional selector: ${selector}`);
            if (splitCssTopLevel(selectorList, ',').some(selectorTargetsTable)) return true;
          }
        } else if (lowerName === 'lang') {
          if (
            !splitCssTopLevel(body, ',').every((range) =>
              /^(?:[a-z_][\w-]*|\*)$/iu.test(range),
            )
          ) throw new Error(`unsupported functional selector: ${selector}`);
        } else {
          throw new Error(`unsupported functional pseudo-class selector: ${selector}`);
        }
        offset = closing;
      } else offset = opening - 1;
      typePosition = false;
      continue;
    }
    if (character === ':' && selector[offset + 1] === ':') {
      const name = selector.slice(offset + 2).match(/^[a-z_][\w-]*/iu)?.[0];
      if (!name) throw new Error(`unsupported pseudo-element selector: ${selector}`);
      const opening = offset + 2 + name.length;
      if (selector[opening] === '(') offset = matchingParenthesis(opening);
      else offset = opening - 1;
      typePosition = false;
      continue;
    }
    if (character === '.' || character === '#') {
      const name = selector.slice(offset + 1).match(/^[\w-]+/u)?.[0];
      if (!name) throw new Error(`unsupported class or ID selector: ${selector}`);
      offset += name.length;
      typePosition = false;
      continue;
    }
    const token = selector
      .slice(offset)
      .match(/^(?:(?:[a-z_][\w-]*|\*)\|)?(?:[a-z_][\w-]*|\*)/iu)?.[0];
    if (token) {
      const localName = token.slice(token.lastIndexOf('|') + 1).toLowerCase();
      if (typePosition && localName === 'table') return true;
      offset += token.length - 1;
      typePosition = false;
      continue;
    }
    typePosition = false;
  }
  return false;
}

function isCellDescendantSelector(selector: string, root: string): boolean {
  if (!selector.startsWith(`${root} `)) return false;
  const descendant = selector.slice(root.length + 1).trim();
  if (/^(th|td|code)$/u.test(descendant)) return true;
  const functional = descendant.match(/^:is\((.*)\)$/u);
  if (!functional) return false;
  const cells = splitCssTopLevel(functional[1], ',');
  return cells.length > 0 && cells.every((cell) => /^(th|td|code)$/u.test(cell));
}

export function authenticatedParentLayoutCss(): string {
  const repository =
    process.env.EQIORA_SITE_GIT_OBJECT_REPOSITORY ??
    resolve(dirname(fileURLToPath(import.meta.url)), '../../..');
  const css = execFileSync(
    'git',
    ['show', `${PARENT_LAYOUT_COMMIT}:docs/site/src/styles/site/layout.css`],
    { cwd: repository, encoding: 'utf8', maxBuffer: 128 * 1024 },
  );
  if (createHash('sha256').update(css).digest('hex') !== PARENT_LAYOUT_SHA256) {
    throw new Error('authenticated parent layout Git blob changed');
  }
  return css;
}

export function assertExactTableSelectorScope(css: string): void {
  const relevantProperties = new Set([
    'display',
    'width',
    'min-width',
    'max-width',
    'table-layout',
    'overflow',
    'overflow-x',
    'overflow-y',
    'white-space',
    'overflow-wrap',
    'word-break',
    'text-overflow',
    'clip',
    'clip-path',
    'opacity',
    'user-select',
  ]);
  const direct = '.sl-markdown-content > table';
  const component = '.sl-markdown-content .eq-stage__body > table';
  const generic = '.sl-markdown-content table:not(:where(.not-content *))';
  const parentCss = authenticatedParentLayoutCss();
  const parentRules = parseCssRules(parentCss);
  const parentSignatures = new Map<string, number>();
  for (const rule of parentRules) {
    const signature = cssRuleSignature(rule);
    parentSignatures.set(signature, (parentSignatures.get(signature) ?? 0) + 1);
  }
  const parentGeneric = parentRules.find(
    (rule) => rule.selectors.length === 1 && rule.selectors[0] === generic,
  );
  if (!parentGeneric) throw new Error('authenticated parent generic table rule is absent');

  let exactGeneric = 0;
  let exactPair = 0;
  for (const rule of parseCssRules(css)) {
    const relevant = [...rule.declarations].some(([property]) => relevantProperties.has(property));
    const tableSelectors = rule.selectors.map((selector) =>
      selectorTargetsTable(decodeCssEscapes(selector)),
    );
    if (!relevant || !tableSelectors.some(Boolean)) continue;
    const signature = cssRuleSignature(rule);
    const parentRemaining = parentSignatures.get(signature) ?? 0;
    if (parentRemaining > 0) {
      parentSignatures.set(signature, parentRemaining - 1);
      if (signature === cssRuleSignature(parentGeneric)) exactGeneric += 1;
      continue;
    }
    if (rule.selectors.some((selector) => selector.includes('\\'))) {
      throw new Error('unsupported escaped table-target selector authority');
    }
    if (
      rule.selectors.length === 2 &&
      rule.selectors[0] === direct &&
      rule.selectors[1] === component
    ) {
      exactPair += 1;
      continue;
    }
    if (
      rule.selectors.every(
        (selector) =>
          isCellDescendantSelector(selector, direct) ||
          isCellDescendantSelector(selector, component),
      )
    ) continue;
    throw new Error(`unowned table selector branch: ${rule.selectors.join(', ')}`);
  }
  if (exactGeneric !== 1) {
    throw new Error(`authenticated parent generic table rule count: ${exactGeneric}`);
  }
  if (exactPair !== 1) throw new Error(`exact product table selector pair count: ${exactPair}`);
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
