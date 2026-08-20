import AxeBuilder from '@axe-core/playwright';
import { chromium, expect, test, type Browser, type BrowserContext, type Page } from '@playwright/test';
import { createHash } from 'node:crypto';
import { readFile, stat } from 'node:fs/promises';
import { resolve } from 'node:path';

const ROUTE = '/reference/rust/api/eqiora/struct.Diagnostic.html';
const CHROME_VERSION = '151.0.7922.34';
const CHROME_BYTES = 290_614_600;
const CHROME_SHA256 = '0b20b130e7edd9dd51873be867761295fe0cfad490c2b9a64f95bd3cfc08fa71';
const MAIN_SCRIPT_SHA256 = 'baec8e8981b6e116315ea7ff1fed10b51352e001825228556ccd126317ed91db';
const HREF_ORDER_SHA256 = 'b7a501d938e30e531dcf446574e0cb6383038d606f055df3aa57e3e64578c290';
const BASE_URL = 'http://127.0.0.1:4173';
const SIZES = [1280, 390, 320] as const;

const fixtureCss = `
html,body{margin:0;max-width:100%;overflow-wrap:anywhere}main{padding:16px}
summary:focus-visible,a:focus-visible{outline:3px solid CanvasText;outline-offset:2px}
.eqiora-signature-links{display:flex;flex-wrap:wrap;gap:.4rem;max-width:100%}
.eqiora-signature-links a{text-decoration:underline;text-underline-offset:.15em}
pre{white-space:pre-wrap;overflow-wrap:anywhere;max-width:100%}
.code-attribute{color:#5b4a00}.comment{color:#4d5a4c}.fn{color:#7a3f00}.since{color:#4f4f4f;font-size:.75rem}
`;

function fixture(extraCss = '', summaryAnchor = false): string {
  const symbol = summaryAnchor
    ? '<a class="fn" href="#method.fixture">fixture</a>'
    : '<span class="fn" data-eqiora-href="#method.fixture">fixture</span>';
  const group = summaryAnchor
    ? ''
    : '<div class="eqiora-signature-links"><span class="eqiora-signature-links__label">Signature links:</span><a class="fn" href="#method.fixture">fixture</a></div>';
  return `<!doctype html><html lang="en"><head><title>Accessible Rustdoc fixture</title><style>${fixtureCss}${extraCss}</style></head><body>
  <a class="eqiora-return" href="/reference/rust/">Back to Eqiora docs</a><main><h1>Struct Fixture</h1>
  <pre class="rust item-decl"><code><span class="code-attribute">#[fixture]</span> pub struct Fixture { <span class="comment">/* fields */</span> }</code></pre>
  <details class="toggle top-doc" open><summary class="hideme"><span>Description</span></summary><p>An ordinary description.</p></details>
  <details class="toggle implementors-toggle" open><summary><section id="method.fixture"><h2>pub fn ${symbol}() <span class="since">1.0.0</span></h2></section></summary>${group}<p>Documentation.</p></details>
  <div slot="settings-menu"><a href="settings.html">Settings</a></div><div slot="help-menu"><a href="help.html">Help</a></div>
  </main><noscript>Documentation remains available without scripting.</noscript></body></html>`;
}

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
  const results = await new AxeBuilder({ page }).analyze();
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
  const widths = await page.evaluate(() => ({
    inner: innerWidth,
    documentClient: document.documentElement.clientWidth,
    documentScroll: document.documentElement.scrollWidth,
    bodyScroll: document.body.scrollWidth,
    declarationClient: document.querySelector('pre.rust.item-decl')?.clientWidth ?? 0,
    declarationScroll: document.querySelector('pre.rust.item-decl')?.scrollWidth ?? 0,
  }));
  expect(widths.documentScroll, JSON.stringify(widths)).toBeLessThanOrEqual(widths.documentClient + 1);
  expect(widths.bodyScroll, JSON.stringify(widths)).toBeLessThanOrEqual(widths.documentClient + 1);
  expect(widths.declarationClient).toBeGreaterThan(0);
  expect(widths.declarationScroll, JSON.stringify(widths)).toBeLessThanOrEqual(
    widths.declarationClient + 1,
  );
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

async function assertNativeSummaryInventory(page: Page, expected: number): Promise<void> {
  const inventory = await page.locator('details.toggle > summary').evaluateAll((summaries) =>
    summaries.map((summary) => ({
      tag: summary.tagName,
      parent: summary.parentElement?.tagName,
      name: (summary as HTMLElement).innerText.trim(),
      tabIndex: (summary as HTMLElement).tabIndex,
      forbidden: ['aria-hidden', 'aria-label', 'inert', 'role', 'tabindex'].filter((name) =>
        summary.hasAttribute(name),
      ),
      open: (summary.parentElement as HTMLDetailsElement | null)?.open,
    })),
  );
  expect(inventory).toHaveLength(expected);
  for (const [offset, summary] of inventory.entries()) {
    expect(summary.tag, `summary ${offset} tag drift`).toBe('SUMMARY');
    expect(summary.parent, `summary ${offset} parent drift`).toBe('DETAILS');
    expect(summary.name, `summary ${offset} has no visible name`).not.toBe('');
    expect(summary.tabIndex, `summary ${offset} left native focus order`).toBeGreaterThanOrEqual(0);
    expect(summary.forbidden, `summary ${offset} spoofs native semantics`).toEqual([]);
    expect(typeof summary.open, `summary ${offset} has no native open state`).toBe('boolean');
  }
}

async function assertProjectedStructure(page: Page): Promise<void> {
  expect(await page.locator('details.toggle').count()).toBe(107);
  expect(await page.locator('details.toggle[open]').count()).toBe(82);
  expect(await page.locator('details.toggle > summary > section').count()).toBe(106);
  expect(await page.locator('details.toggle > summary a[href]').count()).toBe(0);
  expect(await page.locator('details.toggle > summary span[data-eqiora-href]').count()).toBe(321);
  expect(await page.locator('details.toggle > summary + .eqiora-signature-links').count()).toBe(106);
  expect(await page.locator('.eqiora-signature-links a[href]').count()).toBe(321);
  const hrefs = await page.locator('a[href]').evaluateAll((links) =>
    links.map((link) => link.getAttribute('href') ?? ''),
  );
  expect(hrefs).toHaveLength(531);
  expect(hashParts(hrefs)).toBe(HREF_ORDER_SHA256);
  await expect(page.locator('.top-doc > summary.hideme')).toHaveAccessibleName('Description');
  await expect(page.locator('.top-doc > summary.hideme span')).toBeVisible();
  expect(
    await page.locator('summary span[data-eqiora-href], .eqiora-signature-links').evaluateAll((nodes) =>
      nodes.every((node) => {
        const box = node.getBoundingClientRect();
        const style = getComputedStyle(node);
        const nativelyCollapsed =
          node.classList.contains('eqiora-signature-links') &&
          !(node.closest('details') as HTMLDetailsElement | null)?.open;
        return (
          (nativelyCollapsed || (box.width > 0 && box.height > 0)) &&
          style.display !== 'none' &&
          style.visibility !== 'hidden'
        );
      }),
    ),
  ).toBe(true);
  await assertNativeSummaryInventory(page, 107);
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

async function assertFixturePositive(page: Page, requireDistinctSyntax = true): Promise<void> {
  await page.setContent(fixture());
  await expect(page.locator('main')).toBeVisible();
  await expect(page.locator('main h1')).toHaveText('Struct Fixture');
  await assertProjectedStructureFixture(page);
  await assertNoOverflow(page);
  if (requireDistinctSyntax) await assertSyntaxCategoriesDistinct(page);
  expect(await seriousViolations(page)).toEqual([]);
  await assertNativeToggle(page, 'details.implementors-toggle > summary');
}

async function assertProjectedStructureFixture(page: Page): Promise<void> {
  expect(await page.locator('details.toggle').count()).toBe(2);
  expect(await page.locator('details.toggle > summary > section').count()).toBe(1);
  expect(await page.locator('details.toggle > summary a[href]').count()).toBe(0);
  expect(await page.locator('summary span[data-eqiora-href]').count()).toBe(1);
  expect(await page.locator('.eqiora-signature-links a[href]').count()).toBe(1);
  await expect(page.locator('.top-doc > summary')).toHaveAccessibleName('Description');
  await expect(page.locator('.eqiora-signature-links a')).toHaveCSS('text-decoration-line', /underline/);
  await expect(page.locator('summary span[data-eqiora-href]')).toBeVisible();
  await expect(page.locator('.eqiora-signature-links')).toBeVisible();
  await assertNativeSummaryInventory(page, 2);
}

test.describe.configure({ mode: 'serial' });

let browser: Browser;

test.beforeAll(async () => {
  const browserRoot = process.env.PLAYWRIGHT_BROWSERS_PATH;
  expect(browserRoot?.endsWith('/eqiora-pw-1.62.1-r1234')).toBe(true);
  const executablePath = resolve(browserRoot!, 'chromium-1234/chrome-linux64/chrome');
  const metadata = await stat(executablePath);
  expect(metadata.isFile()).toBe(true);
  expect(metadata.mode & 0o777).toBe(0o755);
  expect(metadata.size).toBe(CHROME_BYTES);
  expect(createHash('sha256').update(await readFile(executablePath)).digest('hex')).toBe(
    CHROME_SHA256,
  );
  browser = await chromium.launch({ executablePath, headless: true });
  expect(browser.version()).toBe(CHROME_VERSION);
});

test.afterAll(async () => {
  await browser?.close();
});

test('00 official browser and ordinary accessible Rustdoc fixture pass first', async () => {
  const context = await browser.newContext({ baseURL: BASE_URL, locale: 'en-GB', serviceWorkers: 'block' });
  const attempts = await blockExternal(context);
  const page = await context.newPage();
  for (const width of SIZES) {
    await page.setViewportSize({ width, height: 900 });
    await page.emulateMedia({ forcedColors: 'none' });
    await assertFixturePositive(page);
  }
  await page.setViewportSize({ width: 320, height: 900 });
  await page.emulateMedia({ forcedColors: 'active' });
  await assertFixturePositive(page, false);
  expect(attempts).toEqual([]);
  await context.close();
});

test('01 exact unmodified parent findings reproduce before product GREEN', async () => {
  const context = await browser.newContext({ baseURL: BASE_URL, locale: 'en-GB', serviceWorkers: 'block' });
  const attempts = await blockExternal(context);
  const page = await context.newPage();
  await page.setViewportSize({ width: 1280, height: 900 });
  await assertRealRoute(page);
  expect(await page.locator('details.toggle').count()).toBe(107);
  expect(await page.locator('details.toggle > summary > section').count()).toBe(106);
  const projected = await page.locator('summary span[data-eqiora-href]').count();
  if (projected === 0) {
    expect(await page.locator('details.toggle > summary a[href]').count()).toBe(321);
    const ordinary = await seriousViolations(page);
    expect(Object.fromEntries(ordinary.map((result) => [result.id, result.nodes.length]))).toEqual({
      'color-contrast': 17,
      'nested-interactive': 48,
      'summary-name': 1,
    });
    await page.emulateMedia({ forcedColors: 'active' });
    await page.setViewportSize({ width: 320, height: 844 });
    await page.goto(ROUTE);
    const forced = await seriousViolations(page);
    expect(Object.fromEntries(forced.map((result) => [result.id, result.nodes.length]))).toEqual({
      'color-contrast': 17,
      'link-name': 2,
      'nested-interactive': 48,
      'scrollable-region-focusable': 1,
      'summary-name': 1,
    });
  } else {
    await assertProjectedStructure(page);
    expect(await seriousViolations(page)).toEqual([]);
  }
  expect(attempts).toEqual([]);
  await context.close();
});

test('02 causal browser mutants reject after the ordinary fixture', async () => {
  const context = await browser.newContext({ baseURL: BASE_URL, locale: 'en-GB', serviceWorkers: 'block' });
  const attempts = await blockExternal(context);
  const page = await context.newPage();
  await page.setViewportSize({ width: 320, height: 844 });
  await assertFixturePositive(page);

  await page.setContent(fixture('', true));
  expect((await seriousViolations(page)).map(({ id }) => id)).toContain('nested-interactive');

  await page.setContent(fixture('.fn{color:#ad7c37!important}.top-doc>summary span{display:none}'));
  const structureOnly = (await seriousViolations(page)).map(({ id }) => id);
  expect(structureOnly).not.toContain('nested-interactive');
  expect(structureOnly).toContain('color-contrast');
  expect(structureOnly).toContain('summary-name');

  for (const [selector, color] of [
    ['.code-attribute', '#999'],
    ['.comment', '#8e908c'],
    ['.fn', '#ad7c37'],
    ['.since', '#808080'],
  ] as const) {
    await page.setContent(fixture(`${selector}{color:${color}!important}`));
    expect((await seriousViolations(page)).map(({ id }) => id), selector).toContain('color-contrast');
  }
  await page.setContent(
    fixture('.code-attribute,.comment,.fn,.since{color:#222!important}'),
  );
  await expect(assertSyntaxCategoriesDistinct(page)).rejects.toThrow();

  await page.setContent(fixture().replace('<summary><section', '<summary role="group" aria-label="Collapse"><section'));
  await expect(assertProjectedStructureFixture(page)).rejects.toThrow();

  await page.setContent(fixture().replace('<summary><section', '<summary tabindex="-1"><section'));
  await expect(assertProjectedStructureFixture(page)).rejects.toThrow();

  await page.setContent(
    fixture().replace(
      '<span class="fn" data-eqiora-href=',
      '<span class="fn" style="display:none" data-eqiora-href=',
    ),
  );
  await expect(assertProjectedStructureFixture(page)).rejects.toThrow();

  await page.setContent(fixture().replace('<summary><section', '<div><section').replace('</section></summary>', '</section></div>'));
  expect(await page.locator('details.toggle > summary > section').count()).toBe(0);

  await page.setContent(fixture().replace('>Description</span>', '>Expand description</span>'));
  await expect(page.locator('.top-doc > summary')).not.toHaveAccessibleName('Description');

  await page.setContent(fixture('@media(forced-colors:active){div[slot="settings-menu"] a,div[slot="help-menu"] a{display:none}}'));
  await page.emulateMedia({ forcedColors: 'active' });
  await expect(page.locator('div[slot="settings-menu"] a')).toBeHidden();
  await expect(page.locator('div[slot="help-menu"] a')).toBeHidden();

  await page.emulateMedia({ forcedColors: 'none' });
  await page.setContent(fixture('pre.rust.item-decl{width:470px;overflow:auto;white-space:pre}'));
  await expect(assertNoOverflow(page)).rejects.toThrow();

  await page.setContent(fixture('.eqiora-signature-links a{text-decoration:none}'));
  await expect(page.locator('.eqiora-signature-links a')).toHaveCSS('text-decoration-line', 'none');

  await page.setContent(fixture().replace('</main>', '<img src="https://example.invalid/mutant.png" alt="mutant"></main>'));
  await expect.poll(() => attempts.length).toBeGreaterThan(0);
  await context.close();
});

test('03 complete real Diagnostic projection, runtime, widths, and accessibility are GREEN', async () => {
  test.setTimeout(180_000);
  const context = await browser.newContext({ baseURL: BASE_URL, locale: 'en-GB', serviceWorkers: 'block' });
  const attempts = await blockExternal(context);
  const page = await context.newPage();
  for (const forcedColors of ['none', 'active'] as const) {
    await page.emulateMedia({ forcedColors });
    for (const width of SIZES) {
      await page.setViewportSize({ width, height: 900 });
      await assertRealRoute(page);
      await assertProjectedStructure(page);
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
  await assertNativeSummaryInventory(page, 107);

  for (const theme of ['light', 'dark', 'ayu']) {
    await page.evaluate((name) => {
      localStorage.setItem('rustdoc-use-system-theme', 'false');
      localStorage.setItem('rustdoc-theme', name);
    }, theme);
    await page.reload();
    await expect(page.locator('html')).toHaveAttribute('data-theme', theme);
    await assertProjectedStructure(page);
    expect(await seriousViolations(page)).toEqual([]);
  }

  const toggleAll = page.locator('#toggle-all-docs');
  await expect(toggleAll).toBeVisible();
  await toggleAll.click();
  expect(await page.locator('details.toggle[open]').count()).toBeLessThan(107);
  await toggleAll.click({ modifiers: ['Shift'] });
  expect(await page.locator('details.toggle[open]').count()).toBeGreaterThan(0);

  await page.goto(`${ROUTE}#method.error`);
  await expect(page.locator('#method.error')).toBeAttached();
  await expect(page.locator('.eqiora-signature-links a[href="#method.error"]').first()).toBeVisible();
  await expect(page.locator('a.src').first()).toHaveAttribute('href', /^https:\/\/doc\.rust-lang\.org\/1\.97\.1\//);
  const search = page.locator('input.search-input[type="search"]');
  await expect(search).toHaveAccessibleName('Run search in the documentation');
  await search.fill('Diagnostic');
  await expect(search).toHaveValue('Diagnostic');
  const mainScript = page.locator('script[src$="main-fcd733ba.js"]');
  await expect(mainScript).toHaveCount(1);
  const scriptResponse = await context.request.get(await mainScript.evaluate((node) => (node as HTMLScriptElement).src));
  expect(scriptResponse.ok()).toBe(true);
  expect(createHash('sha256').update(await scriptResponse.body()).digest('hex')).toBe(MAIN_SCRIPT_SHA256);
  expect(attempts).toEqual([]);
  await context.close();
});
