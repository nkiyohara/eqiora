import { createHash } from 'node:crypto';
import { createReadStream, lstatSync, readFileSync, realpathSync } from 'node:fs';
import { dirname, resolve, sep } from 'node:path';
import { createRequire } from 'node:module';

import { chromium, expect, test, type Browser, type BrowserContext, type Page } from '@playwright/test';

const BASE_URL = 'http://127.0.0.1:4173';
const BROWSER_SUFFIX = `eqiora-pw-1.62.1-r1234`;
const BROWSER_RELATIVE = 'chromium-1234/chrome-linux64/chrome';
const BROWSER_SHA256 = '0b20b130e7edd9dd51873be867761295fe0cfad490c2b9a64f95bd3cfc08fa71';
const MANIFEST_SHA256 = 'f306eed529599b1eaf2f8a85db9de2b23e1a3fe36c2b66434b7c9434fb627a99';
const MARK_SHA256 = '6c7ae182102b29ed48281c56434f4d57fe37117dc7df3fa0de18fd79215c9598';
const MARK_BYTES = readFileSync(new URL('../src/assets/brand/eqiora-mark.svg', import.meta.url));
const MARK_DATA = `data:image/svg+xml;base64,${MARK_BYTES.toString('base64')}`;
const require = createRequire(import.meta.url);

type Appearance = {
  width: number;
  height: number;
  colorScheme: 'light' | 'dark';
  forcedColors: 'none' | 'active';
};

const APPEARANCES: Appearance[] = [
  { width: 1440, height: 900, colorScheme: 'light', forcedColors: 'none' },
  { width: 1440, height: 900, colorScheme: 'dark', forcedColors: 'none' },
  { width: 390, height: 844, colorScheme: 'light', forcedColors: 'none' },
  { width: 390, height: 844, colorScheme: 'dark', forcedColors: 'none' },
  { width: 320, height: 800, colorScheme: 'light', forcedColors: 'none' },
  { width: 320, height: 800, colorScheme: 'light', forcedColors: 'active' },
];
const ROUTES = ['/', '/gallery/exact-cylinder-steady-stokes/', '/reference/rust/'] as const;

async function sha256(path: string): Promise<string> {
  return new Promise((accept, reject) => {
    const hash = createHash('sha256');
    const stream = createReadStream(path);
    stream.on('data', (chunk) => hash.update(chunk));
    stream.on('error', reject);
    stream.on('end', () => accept(hash.digest('hex')));
  });
}

async function launchPinnedBrowser(): Promise<Browser> {
  expect(createHash('sha256').update(MARK_BYTES).digest('hex')).toBe(MARK_SHA256);
  const root = resolve(process.env.PLAYWRIGHT_BROWSERS_PATH ?? '');
  expect(root.endsWith(`${sep}${BROWSER_SUFFIX}`)).toBe(true);
  const executable = resolve(root, BROWSER_RELATIVE);
  const stat = lstatSync(executable);
  expect(stat.isFile()).toBe(true);
  expect(stat.isSymbolicLink()).toBe(false);
  expect(stat.mode & 0o777).toBe(0o755);
  expect(stat.size).toBe(290_614_600);
  expect(realpathSync(executable)).toBe(executable);
  expect(await sha256(executable)).toBe(BROWSER_SHA256);

  const manifest = resolve(dirname(require.resolve('playwright-core/package.json')), 'browsers.json');
  expect(await sha256(manifest)).toBe(MANIFEST_SHA256);
  const entries = JSON.parse(readFileSync(manifest, 'utf8')).browsers as Array<{
    name: string;
    revision: string;
    browserVersion: string;
  }>;
  for (const name of ['chromium', 'chromium-headless-shell']) {
    expect(entries.find((entry) => entry.name === name)).toMatchObject({
      revision: '1234',
      browserVersion: '151.0.7922.34',
    });
  }
  expect(require('@playwright/test/package.json').version).toBe('1.62.1');
  const axePackage = resolve(dirname(require.resolve('@axe-core/playwright')), '../package.json');
  expect(JSON.parse(readFileSync(axePackage, 'utf8')).version).toBe('4.13.0');
  const browser = await chromium.launch({ executablePath: executable, headless: true });
  expect(browser.version()).toBe('151.0.7922.34');
  return browser;
}

let fullBrowser: Browser;
test.beforeAll(async () => {
  fullBrowser = await launchPinnedBrowser();
});
test.afterAll(async () => {
  await fullBrowser?.close();
});

async function contextFor(appearance: Appearance, javaScriptEnabled = true): Promise<BrowserContext> {
  return fullBrowser.newContext({
    baseURL: BASE_URL,
    colorScheme: appearance.colorScheme,
    forcedColors: appearance.forcedColors,
    javaScriptEnabled,
    locale: 'en-GB',
    serviceWorkers: 'block',
    viewport: { width: appearance.width, height: appearance.height },
  });
}

async function guardNetwork(page: Page): Promise<string[]> {
  const external: string[] = [];
  await page.route('**/*', async (route) => {
    const url = new URL(route.request().url());
    if (url.origin === BASE_URL || url.protocol === 'data:' || url.protocol === 'blob:') {
      await route.continue();
    } else {
      external.push(url.href);
      await route.abort('blockedbyclient');
    }
  });
  return external;
}

async function keyboardReachable(page: Page): Promise<boolean> {
  const anchor = page.getByRole('banner').locator('a.site-title');
  await page.evaluate(() => (document.activeElement as HTMLElement | null)?.blur());
  for (let attempt = 0; attempt < 80; attempt += 1) {
    await page.keyboard.press('Tab');
    if (await anchor.evaluate((element) => document.activeElement === element)) return true;
  }
  return false;
}

async function markDigest(page: Page): Promise<string> {
  const mark = page.getByRole('banner').locator('a.site-title').locator(':scope > img');
  const bytes = await mark.evaluate(async (image) => {
    const response = await fetch((image as HTMLImageElement).src);
    if (!response.ok) throw new Error(`mark fetch failed: ${response.status}`);
    return [...new Uint8Array(await response.arrayBuffer())];
  });
  return createHash('sha256').update(Buffer.from(bytes)).digest('hex');
}

async function headerIssue(page: Page, ignoreTargetName = false): Promise<string | null> {
  const banners = page.getByRole('banner');
  if ((await banners.count()) !== 1) return `banner-cardinality:${await banners.count()}`;
  const anchors = banners.locator('a.site-title');
  if ((await anchors.count()) !== 1) return `anchor-cardinality:${await anchors.count()}`;
  if (!(await anchors.isVisible())) return 'anchor-visibility';
  if ((await anchors.getAttribute('href')) !== '/') return `href:${await anchors.getAttribute('href')}`;

  const marks = anchors.locator(':scope > img');
  if ((await marks.count()) !== 1) return `mark-cardinality:${await marks.count()}`;
  if (!(await marks.isVisible())) return 'mark-visibility';
  const image = await marks.evaluate((element) => {
    const mark = element as HTMLImageElement;
    const box = mark.getBoundingClientRect();
    return { complete: mark.complete, naturalWidth: mark.naturalWidth, width: box.width, height: box.height };
  });
  if (!image.complete || image.naturalWidth <= 0 || image.width <= 0 || image.height <= 0) {
    return `mark-decoding:${JSON.stringify(image)}`;
  }
  if ((await markDigest(page)) !== MARK_SHA256) return 'mark-source';

  const titles = anchors.locator(':scope > span');
  if ((await titles.count()) !== 1) return `visible-word-source:count=${await titles.count()}`;
  const title = await titles.evaluate((element) => {
    const style = getComputedStyle(element);
    const box = element.getBoundingClientRect();
    return {
      text: element.textContent?.trim().split(/\s+/u).filter(Boolean).join(' ') ?? '',
      visible:
        style.display !== 'none' &&
        style.visibility !== 'hidden' &&
        Number.parseFloat(style.opacity) > 0 &&
        box.width > 0 &&
        box.height > 0 &&
        box.right > 0 &&
        box.bottom > 0 &&
        box.left < innerWidth &&
        box.top < innerHeight,
    };
  });
  if (!title.visible || title.text !== 'Eqiora') {
    return `visible-word-source:${JSON.stringify(title)}`;
  }
  const substituteSelector = '[aria-label], [aria-labelledby], [title], [hidden], [aria-hidden="true"], .sr-only';
  const hasSubstitute = await anchors.evaluate(
    (anchor, selector) => anchor.matches(selector) || anchor.querySelector(selector) !== null,
    substituteSelector,
  );
  if (hasSubstitute) {
    return 'visible-word-source:substitute';
  }
  const generated = await anchors.evaluate((anchor) =>
    [anchor, ...anchor.querySelectorAll('*')].flatMap((element) =>
      ['::before', '::after'].map((pseudo) => getComputedStyle(element, pseudo).content),
    ),
  );
  if (generated.some((content) => !['none', 'normal', '""'].includes(content))) {
    return `visible-word-source:generated=${JSON.stringify(generated)}`;
  }
  if (!ignoreTargetName) {
    const alternative = await marks.getAttribute('alt');
    const exact = await banners.getByRole('link', { name: 'Eqiora', exact: true }).count();
    const doubled = await banners.getByRole('link', { name: 'Eqiora Eqiora', exact: true }).count();
    if (alternative !== '' || exact !== 1 || doubled !== 0) {
      return `accessible-name:alt=${JSON.stringify(alternative)},exact=${exact},doubled=${doubled}`;
    }
  }
  if (!(await keyboardReachable(page))) return 'keyboard-focus';
  const overflow = await page.evaluate(() => ({
    client: document.documentElement.clientWidth,
    scroll: document.documentElement.scrollWidth,
  }));
  if (overflow.scroll > overflow.client + 1) return `horizontal-overflow:${JSON.stringify(overflow)}`;
  return null;
}

async function activateHeader(page: Page, expectedPath: string): Promise<void> {
  const anchor = page.getByRole('banner').locator('a.site-title');
  await anchor.focus();
  let navigations = 0;
  const observe = (frame: { parentFrame: () => unknown }) => {
    if (frame.parentFrame() === null) navigations += 1;
  };
  page.on('framenavigated', observe);
  await Promise.all([
    page.waitForURL((url) => url.origin === BASE_URL && url.pathname === expectedPath),
    page.keyboard.press('Enter'),
  ]);
  page.off('framenavigated', observe);
  expect(navigations).toBe(1);
  expect(new URL(page.url()).pathname).toBe(expectedPath);
}

type Mutation =
  | 'doubled-alt' | 'hidden-title' | 'replaces-title' | 'aria-label' | 'aria-labelledby'
  | 'title-source' | 'offscreen-title' | 'hidden-name' | 'anchor-aria-label'
  | 'anchor-aria-labelledby' | 'anchor-title' | 'missing-mark' | 'duplicate-mark'
  | 'duplicate-word' | 'duplicate-anchor' | 'wrong-href' | 'intercept'
  | 'empty-word' | 'whitespace-word' | 'missing-word' | 'dark-mark'
  | 'narrow-title' | 'forced-anchor';

function syntheticHtml(mutation: Mutation | 'control'): string {
  const alt = mutation === 'doubled-alt' ? 'Eqiora' : '';
  const href = mutation === 'wrong-href' ? '/gallery/' : '/';
  const anchorLabel = ['aria-label', 'anchor-aria-label', 'empty-word', 'whitespace-word', 'missing-word'].includes(mutation) ? ' aria-label="Eqiora"' : '';
  const labelled = ['aria-labelledby', 'hidden-name'].includes(mutation)
    ? ' aria-labelledby="name-source"'
    : mutation === 'anchor-aria-labelledby' ? ' aria-labelledby="visible-name"' : '';
  const nativeTitle = ['title-source', 'anchor-title'].includes(mutation) ? ' title="Eqiora"' : '';
  const mark = `<img class="mark" src="${MARK_DATA}" alt="${alt}" width="48" height="48">`;
  const marks = mutation === 'missing-mark' ? '' : mutation === 'duplicate-mark' ? `${mark}${mark}` : mark;
  let word = '<span class="word">Eqiora</span>';
  if (mutation === 'anchor-aria-labelledby') word = '<span id="visible-name" class="word">Eqiora</span>';
  if (mutation === 'hidden-title') word = '<span class="word hidden">Eqiora</span>';
  if (mutation === 'replaces-title') word = '<span class="word sr-only">Eqiora</span>';
  if (mutation === 'offscreen-title') word = '<span class="word offscreen">Eqiora</span>';
  if (mutation === 'aria-labelledby') word = '<span id="name-source" class="word hidden">Eqiora</span>';
  if (mutation === 'hidden-name') word = '<span id="name-source" class="word" hidden>Eqiora</span>';
  if (mutation === 'aria-label' || mutation === 'title-source' || mutation === 'missing-word') word = '';
  if (mutation === 'empty-word') word = '<span class="word"></span>';
  if (mutation === 'whitespace-word') word = '<span class="word">   </span>';
  if (mutation === 'duplicate-word') word += '<span class="word">Eqiora</span>';
  const anchor = `<a class="site-title" href="${href}"${anchorLabel}${labelled}${nativeTitle}>${marks}${word}</a>`;
  const anchors = mutation === 'duplicate-anchor' ? `${anchor}${anchor}` : anchor;
  return `<!doctype html><html><head><base href="${BASE_URL}/"><style>
    *{box-sizing:border-box} body{margin:0} header{height:80px;padding:12px}.site-title{display:inline-flex;align-items:center;gap:8px}.mark{width:48px;height:48px}.word{font:600 20px sans-serif}.hidden{display:none}.sr-only,.offscreen{position:absolute;left:-10000px}
    @media(prefers-color-scheme:dark){.mutant-dark-mark .mark{display:none}}
    @media(max-width:320px){.mutant-narrow-title .word{display:none}}
    @media(forced-colors:active){.mutant-forced-anchor .site-title{display:none}}
    </style></head><body class="mutant-${mutation}"><header role="banner">${anchors}</header></body></html>`;
}

test('ordinary composite admits one decorative mark, one visible word, and native home navigation', async () => {
  const context = await contextFor(APPEARANCES[0]);
  const page = await context.newPage();
  const external = await guardNetwork(page);
  try {
    await page.setContent(syntheticHtml('control'));
    expect(await headerIssue(page)).toBeNull();
    await activateHeader(page, '/');
    expect(external).toEqual([]);
  } finally {
    await context.close();
  }
});

test('real parent has one usable composite and only the strict target-name transition remains', async () => {
  test.setTimeout(180_000);
  const context = await contextFor(APPEARANCES[0]);
  const page = await context.newPage();
  const external = await guardNetwork(page);
  try {
    expect((await page.goto('/'))?.status()).toBe(200);
    expect(await headerIssue(page, true)).toBeNull();
    expect((await page.goto('/gallery/exact-cylinder-steady-stokes/'))?.status()).toBe(200);
    expect(await headerIssue(page, true)).toBeNull();
    await activateHeader(page, '/');
    expect(await headerIssue(page, true)).toBeNull();
    await page.waitForLoadState('networkidle');
    expect(
      { count: external.length, identities: [...external] },
      'real artifact made an external request before the name boundary',
    ).toEqual({ count: 0, identities: [] });
    expect(await headerIssue(page)).toBeNull();
    expect(external).toEqual([]);
  } finally {
    await context.close();
  }

  for (const appearance of APPEARANCES) {
    const matrixContext = await contextFor(appearance);
    const matrixPage = await matrixContext.newPage();
    const matrixExternal = await guardNetwork(matrixPage);
    try {
      for (const route of ROUTES) {
        expect((await matrixPage.goto(route))?.status(), `${route} ${JSON.stringify(appearance)}`).toBe(200);
        expect(await headerIssue(matrixPage), `${route} ${JSON.stringify(appearance)}`).toBeNull();
        if (route === '/gallery/exact-cylinder-steady-stokes/') {
          await activateHeader(matrixPage, '/');
        }
      }
      expect(matrixExternal).toEqual([]);
    } finally {
      await matrixContext.close();
    }
  }

  for (const appearance of [APPEARANCES[2], APPEARANCES[5]]) {
    const noScript = await contextFor(appearance, false);
    const noScriptPage = await noScript.newPage();
    const noScriptExternal = await guardNetwork(noScriptPage);
    try {
      expect((await noScriptPage.goto('/gallery/exact-cylinder-steady-stokes/'))?.status()).toBe(200);
      expect(await headerIssue(noScriptPage)).toBeNull();
      await activateHeader(noScriptPage, '/');
      expect(noScriptExternal).toEqual([]);
    } finally {
      await noScript.close();
    }
  }
});

test('causal composites reject duplicate sources, missing parts, wrong navigation, and media-only loss', async () => {
  test.setTimeout(120_000);
  const context = await contextFor(APPEARANCES[0]);
  const page = await context.newPage();
  const external = await guardNetwork(page);
  const cases: Array<[Mutation, string, Appearance]> = [
    ['doubled-alt', 'accessible-name:', APPEARANCES[0]], ['hidden-title', 'visible-word-source:', APPEARANCES[0]],
    ['replaces-title', 'visible-word-source:', APPEARANCES[0]], ['aria-label', 'visible-word-source:', APPEARANCES[0]],
    ['aria-labelledby', 'visible-word-source:', APPEARANCES[0]], ['title-source', 'visible-word-source:', APPEARANCES[0]],
    ['offscreen-title', 'visible-word-source:', APPEARANCES[0]], ['hidden-name', 'visible-word-source:', APPEARANCES[0]],
    ['anchor-aria-label', 'visible-word-source:substitute', APPEARANCES[0]],
    ['anchor-aria-labelledby', 'visible-word-source:substitute', APPEARANCES[0]],
    ['anchor-title', 'visible-word-source:substitute', APPEARANCES[0]],
    ['missing-mark', 'mark-cardinality:', APPEARANCES[0]], ['duplicate-mark', 'mark-cardinality:', APPEARANCES[0]],
    ['duplicate-word', 'visible-word-source:', APPEARANCES[0]], ['duplicate-anchor', 'anchor-cardinality:', APPEARANCES[0]],
    ['wrong-href', 'href:', APPEARANCES[0]], ['empty-word', 'visible-word-source:', APPEARANCES[0]],
    ['whitespace-word', 'visible-word-source:', APPEARANCES[0]], ['missing-word', 'visible-word-source:', APPEARANCES[0]],
    ['dark-mark', 'mark-visibility', APPEARANCES[1]], ['narrow-title', 'visible-word-source:', APPEARANCES[4]],
    ['forced-anchor', 'anchor-visibility', APPEARANCES[5]],
  ];
  try {
    for (const [mutation, target, appearance] of cases) {
      await page.setViewportSize({ width: appearance.width, height: appearance.height });
      await page.emulateMedia({ colorScheme: appearance.colorScheme, forcedColors: appearance.forcedColors });
      await page.setContent(syntheticHtml('control'));
      expect(await headerIssue(page), `${mutation} same-shape control`).toBeNull();
      if (['dark-mark', 'narrow-title', 'forced-anchor'].includes(mutation)) {
        await page.setViewportSize({ width: 1440, height: 900 });
        await page.emulateMedia({ colorScheme: 'light', forcedColors: 'none' });
        await page.setContent(syntheticHtml(mutation));
        expect(await headerIssue(page), `${mutation} ordinary point`).toBeNull();
        await page.setViewportSize({ width: appearance.width, height: appearance.height });
        await page.emulateMedia({ colorScheme: appearance.colorScheme, forcedColors: appearance.forcedColors });
      }
      await page.setContent(syntheticHtml(mutation));
      if (mutation.startsWith('anchor-')) {
        const banner = page.getByRole('banner');
        await expect(banner.locator('a.site-title').locator(':scope > span')).toBeVisible();
        expect(await banner.getByRole('link', { name: 'Eqiora', exact: true }).count()).toBe(1);
        expect(await banner.getByRole('link', { name: 'Eqiora Eqiora', exact: true }).count()).toBe(0);
      }
      const issue = await headerIssue(page);
      expect(target.endsWith(':') ? issue?.startsWith(target) : issue === target, mutation).toBe(true);
      if (mutation === 'wrong-href') {
        await activateHeader(page, '/gallery/');
      }
    }

    await page.setViewportSize({ width: 1440, height: 900 });
    await page.emulateMedia({ colorScheme: 'light', forcedColors: 'none' });
    await page.setContent(syntheticHtml('control'));
    expect(await headerIssue(page), 'intercept same-shape control').toBeNull();
    await activateHeader(page, '/');
    await page.setContent(syntheticHtml('intercept'));
    expect(await headerIssue(page)).toBeNull();
    await page.evaluate(() => {
      document.querySelector('a.site-title')?.addEventListener('click', (event) => {
        event.preventDefault();
        history.pushState({}, '', '/gallery/');
      });
    });
    await activateHeader(page, '/gallery/');
    expect(external).toEqual([]);
  } finally {
    await context.close();
  }
});
