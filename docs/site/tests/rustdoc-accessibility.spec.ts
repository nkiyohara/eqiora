import AxeBuilder from '@axe-core/playwright';
import { expect, test, type Browser, type BrowserContext, type Page } from '@playwright/test';
import { createHash } from 'node:crypto';
import { BASE_URL, launchOfficialBrowser } from './support';

const ROUTE = '/reference/rust/api/eqiora/struct.Diagnostic.html';
const MAIN_SCRIPT_SHA256 = 'baec8e8981b6e116315ea7ff1fed10b51352e001825228556ccd126317ed91db';
const HREF_ORDER_SHA256 = 'b7a501d938e30e531dcf446574e0cb6383038d606f055df3aa57e3e64578c290';
const SIZES = [1280, 390, 320] as const;
const PRESENTATION_OK = {
  text: 0,
  size: 0,
  bounds: 0,
  display: 0,
  visibility: 0,
  opacity: 0,
  clip: 0,
  clipPath: 0,
  filter: 0,
  transform: 0,
  overflow: 0,
  userSelect: 0,
  affordance: 0,
  pointerEvents: 0,
};
const SUMMARY_OK = { tag: 0, parent: 0, name: 0, tabIndex: 0, forbidden: 0, open: 0 };
const REAL_PROJECTION = {
  details: 107, open: 82, sections: 106, nested: 0,
  sources: 321, groups: 106, links: 321, hrefs: 531, hrefDigest: HREF_ORDER_SHA256,
};

function hashParts(parts: string[]): string {
  const hash = createHash('sha256');
  for (const part of parts) {
    const bytes = Buffer.from(part);
    const length = Buffer.alloc(8);
    length.writeBigUInt64BE(BigInt(bytes.length));
    hash.update(length);
    hash.update(bytes);
  }
  return hash.digest('hex');
}

async function seriousViolations(page: Page) {
  const results = await new AxeBuilder({ page })
    .options({ resultTypes: ['violations'] })
    .analyze();
  return results.violations.filter(({ impact }) => impact === 'serious' || impact === 'critical');
}

async function blockExternal(context: BrowserContext): Promise<string[]> {
  const attempts: string[] = [];
  await context.route('**/*', async (route) => {
    const url = new URL(route.request().url());
    if (url.protocol === 'data:' || url.protocol === 'blob:' || url.origin === BASE_URL) {
      await route.continue();
      return;
    }
    attempts.push(url.href);
    await route.abort('blockedbyclient');
  });
  return attempts;
}

async function assertNoOverflow(page: Page): Promise<void> {
  const viewport = page.viewportSize();
  expect(viewport).not.toBeNull();
  const widths = await page.evaluate(() => ({
    inner: innerWidth,
    documentClient: document.documentElement.clientWidth,
    documentScroll: document.documentElement.scrollWidth,
    bodyScroll: document.body.scrollWidth,
    declarationClient: document.querySelector('pre.rust.item-decl')?.clientWidth ?? 0,
    declarationScroll: document.querySelector('pre.rust.item-decl')?.scrollWidth ?? 0,
    roots: ['html', 'body', 'main'].map((selector) => {
      const style = getComputedStyle(document.querySelector(selector)!);
      return { selector, transform: style.transform, zoom: style.zoom };
    }),
  }));
  expect(widths.inner).toBe(viewport!.width);
  expect(widths.documentClient).toBe(viewport!.width);
  expect(widths.documentScroll, JSON.stringify(widths)).toBeLessThanOrEqual(widths.documentClient + 1);
  expect(widths.bodyScroll, JSON.stringify(widths)).toBeLessThanOrEqual(widths.documentClient + 1);
  expect(widths.declarationClient).toBeGreaterThan(0);
  expect(widths.declarationScroll, JSON.stringify(widths)).toBeLessThanOrEqual(
    widths.declarationClient + 1,
  );
  for (const root of widths.roots) {
    expect(root.zoom, `${root.selector} scales the requested CSS viewport`).toBe('1');
    expect(root.transform, `${root.selector} transforms the requested CSS viewport`).toBe('none');
  }
}

async function assertSyntaxCategoriesDistinct(page: Page): Promise<void> {
  const colors = await page.evaluate(() =>
    ['.code-attribute', '.comment', '.fn', '.since'].map((selector) => {
      const node = document.querySelector(selector);
      if (!node) throw new Error(`missing syntax category ${selector}`);
      return getComputedStyle(node).color;
    }),
  );
  expect(colors).toHaveLength(4);
  expect(new Set(colors).size).toBeGreaterThanOrEqual(3);
}

async function assertNativeToggle(page: Page, selector: string): Promise<void> {
  const summary = page.locator(selector).first();
  await expect(summary).toBeVisible();
  expect((await summary.innerText()).trim()).not.toBe('');
  const details = summary.locator('xpath=..');
  const initial = await details.evaluate((element) => (element as HTMLDetailsElement).open);
  await summary.focus();
  await expect(summary).toBeFocused();
  await page.keyboard.press('Enter');
  expect(await details.evaluate((element) => (element as HTMLDetailsElement).open)).toBe(!initial);
  await page.keyboard.press('Enter');
  expect(await details.evaluate((element) => (element as HTMLDetailsElement).open)).toBe(initial);
}

async function observeProjection(page: Page) {
  return page.evaluate(() => {
    const toggles = Array.from(document.querySelectorAll<HTMLDetailsElement>('details.toggle'));
    const open = toggles.map((details) => details.open);
    const structure = {
      details: toggles.length,
      open: open.filter(Boolean).length,
      sections: document.querySelectorAll('details.toggle > summary > section').length,
      nested: document.querySelectorAll('details.toggle > summary a[href]').length,
      sources: document.querySelectorAll('details.toggle > summary span[data-eqiora-href]').length,
      groups: document.querySelectorAll('details.toggle > summary + .eqiora-signature-links').length,
      links: document.querySelectorAll('.eqiora-signature-links a[href]').length,
    };
    const hrefs = Array.from(document.querySelectorAll<HTMLAnchorElement>('a[href]'), (link) =>
      link.getAttribute('href') ?? '',
    );
    const inspect = (nodes: Element[], activeLinks = false) => {
      const failures = {
        text: 0, size: 0, bounds: 0, display: 0, visibility: 0, opacity: 0,
        clip: 0, clipPath: 0, filter: 0, transform: 0, overflow: 0, userSelect: 0,
        affordance: 0, pointerEvents: 0,
      };
      for (const node of nodes) {
        const style = getComputedStyle(node);
        const box = node.getBoundingClientRect();
        failures.text += (node as HTMLElement).innerText.trim() === '' ? 1 : 0;
        failures.size += box.width <= 0 || box.height <= 0 ? 1 : 0;
        failures.bounds += box.left < -1 || box.right > document.documentElement.clientWidth + 1 ? 1 : 0;
        failures.display += style.display === 'none' ? 1 : 0;
        failures.visibility += style.visibility === 'hidden' ? 1 : 0;
        failures.opacity += style.opacity !== '1' ? 1 : 0;
        failures.clip += style.clip !== 'auto' ? 1 : 0;
        failures.clipPath += style.clipPath !== 'none' ? 1 : 0;
        failures.filter += style.filter !== 'none' ? 1 : 0;
        failures.transform += style.transform !== 'none' ? 1 : 0;
        failures.overflow += /hidden|clip/.test(style.overflowX) ? 1 : 0;
        failures.userSelect += style.userSelect === 'none' ? 1 : 0;
        if (activeLinks) {
          const underline = style.textDecorationLine.split(' ').includes('underline');
          const border = !['none', 'hidden'].includes(style.borderBottomStyle) &&
            Number.parseFloat(style.borderBottomWidth) > 0;
          failures.affordance += underline || border ? 0 : 1;
          failures.pointerEvents += style.pointerEvents === 'none' ? 1 : 0;
        }
      }
      return { count: nodes.length, failures };
    };
    const summaryFailures = { tag: 0, parent: 0, name: 0, tabIndex: 0, forbidden: 0, open: 0 };
    const summaries = Array.from(document.querySelectorAll<HTMLElement>('details.toggle > summary'));
    try {
      for (const details of toggles) details.open = true;
      for (const summary of summaries) {
        summaryFailures.tag += summary.tagName !== 'SUMMARY' ? 1 : 0;
        summaryFailures.parent += summary.parentElement?.tagName !== 'DETAILS' ? 1 : 0;
        summaryFailures.name += summary.innerText.trim() === '' ? 1 : 0;
        summaryFailures.tabIndex += summary.tabIndex < 0 ? 1 : 0;
        summaryFailures.forbidden += ['aria-hidden', 'aria-label', 'inert', 'role', 'tabindex'].some(
          (name) => summary.hasAttribute(name),
        ) ? 1 : 0;
        summaryFailures.open += typeof (summary.parentElement as HTMLDetailsElement | null)?.open !== 'boolean' ? 1 : 0;
      }
      return {
        structure,
        hrefs,
        sources: inspect(Array.from(document.querySelectorAll('details.toggle > summary span[data-eqiora-href]'))),
        groups: inspect(Array.from(document.querySelectorAll('details.toggle > summary + .eqiora-signature-links'))),
        links: inspect(Array.from(document.querySelectorAll('.eqiora-signature-links a[href]')), true),
        summaries: { count: summaries.length, failures: summaryFailures },
      };
    } finally {
      toggles.forEach((details, offset) => { details.open = open[offset]; });
    }
  });
}

type ProjectionExpected = {
  details: number; open: number; sections: number; nested: number;
  sources: number; groups: number; links: number; hrefs: number; hrefDigest?: string;
};

function assertProjectionStructure(
  observation: Awaited<ReturnType<typeof observeProjection>>,
  expected: ProjectionExpected,
): void {
  expect(observation.structure).toEqual({
    details: expected.details, open: expected.open, sections: expected.sections,
    nested: expected.nested, sources: expected.sources, groups: expected.groups, links: expected.links,
  });
  expect(observation.hrefs).toHaveLength(expected.hrefs);
  if (expected.hrefDigest) expect(hashParts(observation.hrefs)).toBe(expected.hrefDigest);
  const { name: _, ...summarySemantics } = observation.summaries.failures;
  const { name: __, ...expectedSemantics } = SUMMARY_OK;
  expect(observation.summaries.count).toBe(expected.details);
  expect(summarySemantics).toEqual(expectedSemantics);
}

function assertProjectionPresentation(
  observation: Awaited<ReturnType<typeof observeProjection>>,
  expected: ProjectionExpected,
): void {
  expect(observation.sources).toEqual({ count: expected.sources, failures: PRESENTATION_OK });
  expect(observation.groups).toEqual({ count: expected.groups, failures: PRESENTATION_OK });
  expect(observation.links).toEqual({ count: expected.links, failures: PRESENTATION_OK });
  expect(observation.summaries).toEqual({ count: expected.details, failures: SUMMARY_OK });
}

async function assertProjectedStructure(page: Page) {
  const observation = await observeProjection(page);
  assertProjectionStructure(observation, REAL_PROJECTION);
  return observation;
}

async function assertProjectedName(page: Page): Promise<void> {
  await expect(page.locator('.top-doc > summary.hideme')).toHaveAccessibleName('Description');
  await expect(page.locator('.top-doc > summary.hideme span')).toBeVisible();
}

async function assertRealRoute(page: Page): Promise<void> {
  await page.goto(ROUTE);
  await expect(page).toHaveTitle('Diagnostic in eqiora - Rust');
  await expect(page.locator('body')).toHaveClass(/\brustdoc\b.*\bstruct\b/);
  await expect(page.locator('main')).toHaveCount(1);
  await expect(page.locator('main h1')).toContainText('Struct Diagnostic');
  await expect(page.locator('main h1')).toContainText('Copy item path');
  await expect(page.locator('.eqiora-return')).toHaveAttribute('href', '/reference/rust/');
  await expect(page.locator('.eqiora-return')).toHaveAccessibleName('Back to the Eqiora Rust reference');
  await expect(page.locator('noscript')).toHaveCount(1);
}

async function assertRealSearch(page: Page, attempts: string[]): Promise<void> {
  const routeUrl = new URL(ROUTE, BASE_URL).href;
  expect(page.url()).toBe(routeUrl);
  const control = page.locator('rustdoc-toolbar #search-button > a');
  await expect(control).toHaveAccessibleName('Search');
  await expect(control).toBeVisible();
  await expect(control).toBeEnabled();
  expect(await control.evaluate((node) => (node as HTMLAnchorElement).href)).toBe(`${routeUrl}?search=`);
  await control.click();
  await expect(control).toHaveAccessibleName('Exit');
  const input = page.locator('input.search-input[type="search"]');
  await expect(input).toHaveAccessibleName('Run search in the documentation');
  await expect(input).toBeVisible();
  await expect(input).toBeEnabled();
  await expect(input).toBeFocused();
  const box = await input.boundingBox();
  expect(box).not.toBeNull();
  expect(box?.width).toBeGreaterThan(0);
  expect(box?.height).toBeGreaterThan(0);
  await input.fill('Diagnostic');
  await expect(input).toHaveValue('Diagnostic');
  await page.keyboard.press('Escape');
  await expect(control).toHaveAccessibleName('Search');
  await expect(input).toBeHidden();
  await expect(input).not.toBeFocused();
  expect(page.url()).toBe(routeUrl);
  expect(attempts).toEqual([]);
}

let browser: Browser;

test.beforeAll(async () => {
  browser = await launchOfficialBrowser();
});

test.afterAll(async () => {
  await browser?.close();
});

test('Diagnostic reference supports projection, search, themes, keyboard controls, and reflow', async () => {
  test.setTimeout(180_000);
  const context = await browser.newContext({ baseURL: BASE_URL, locale: 'en-GB', serviceWorkers: 'block' });
  const attempts = await blockExternal(context);
  const page = await context.newPage();
  let firstObservation = true;
  for (const forcedColors of ['none', 'active'] as const) {
    await page.emulateMedia({ forcedColors });
    for (const width of SIZES) {
      await page.setViewportSize({ width, height: 900 });
      await assertRealRoute(page);
      const observation = await assertProjectedStructure(page);
      if (firstObservation) {
        const mainScript = page.locator('script[src$="main-fcd733ba.js"]');
        await expect(mainScript).toHaveCount(1);
        const scriptResponse = await context.request.get(
          await mainScript.evaluate((node) => (node as HTMLScriptElement).src),
        );
        expect(scriptResponse.ok()).toBe(true);
        expect(createHash('sha256').update(await scriptResponse.body()).digest('hex')).toBe(
          MAIN_SCRIPT_SHA256,
        );
        await assertRealSearch(page, attempts);
        firstObservation = false;
      }
      await assertProjectedName(page);
      assertProjectionPresentation(observation, REAL_PROJECTION);
      await assertNoOverflow(page);
      if (forcedColors === 'none') await assertSyntaxCategoriesDistinct(page);
      await expect(page.locator('div[slot="settings-menu"] a')).toHaveAccessibleName('Settings');
      await expect(page.locator('div[slot="help-menu"] a')).toHaveAccessibleName('Help');
      await expect(page.locator('div[slot="settings-menu"] a')).toBeVisible();
      await expect(page.locator('div[slot="help-menu"] a')).toBeVisible();
      expect(await seriousViolations(page)).toEqual([]);
    }
  }

  await page.emulateMedia({ forcedColors: 'none' });
  await page.setViewportSize({ width: 1280, height: 900 });
  await assertRealRoute(page);
  for (const selector of [
    '.top-doc > summary.hideme',
    'details.implementors-toggle > summary',
    'details.method-toggle:not(.deprecated) > summary',
    'details.method-toggle.deprecated > summary',
  ]) {
    await assertNativeToggle(page, selector);
  }

  for (const theme of ['light', 'dark', 'ayu']) {
    await page.evaluate((name) => {
      localStorage.setItem('rustdoc-use-system-theme', 'false');
      localStorage.setItem('rustdoc-theme', name);
    }, theme);
    await page.reload();
    await expect(page.locator('html')).toHaveAttribute('data-theme', theme);
    const observation = await assertProjectedStructure(page);
    await assertProjectedName(page);
    assertProjectionPresentation(observation, REAL_PROJECTION);
    await assertSyntaxCategoriesDistinct(page);
    expect(await seriousViolations(page)).toEqual([]);
  }

  const toggleAll = page.locator('#toggle-all-docs');
  await expect(toggleAll).toBeVisible();
  await toggleAll.click();
  expect(await page.locator('details.toggle[open]').count()).toBeLessThan(107);
  await toggleAll.click({ modifiers: ['Shift'] });
  expect(await page.locator('details.toggle[open]').count()).toBeGreaterThan(0);

  await page.goto(`${ROUTE}#method.error`);
  await expect(page.locator('[id="method.error"]')).toBeAttached();
  await expect(page.locator('.eqiora-signature-links a[href="#method.error"]').first()).toBeVisible();
  await expect(page.locator('a.src').first()).toHaveAttribute('href', /^https:\/\/doc\.rust-lang\.org\/1\.97\.1\//);
  expect(attempts).toEqual([]);
  await context.close();
});
