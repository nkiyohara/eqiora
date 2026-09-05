import { expect, test, type Browser, type BrowserContext, type Page } from '@playwright/test';
import { BASE_URL, launchOfficialBrowser } from './support';

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

let fullBrowser: Browser;
test.beforeAll(async () => {
  fullBrowser = await launchOfficialBrowser();
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

async function headerIssue(page: Page): Promise<string | null> {
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
  const alternative = await marks.getAttribute('alt');
  const exact = await banners.getByRole('link', { name: 'Eqiora', exact: true }).count();
  const doubled = await banners.getByRole('link', { name: 'Eqiora Eqiora', exact: true }).count();
  if (alternative !== '' || exact !== 1 || doubled !== 0) {
    return `accessible-name:alt=${JSON.stringify(alternative)},exact=${exact},doubled=${doubled}`;
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

test('header branding stays accessible and navigates home across appearances', async () => {
  test.setTimeout(180_000);
  const context = await contextFor(APPEARANCES[0]);
  const page = await context.newPage();
  const external = await guardNetwork(page);
  try {
    expect((await page.goto('/'))?.status()).toBe(200);
    expect(await headerIssue(page)).toBeNull();
    expect((await page.goto('/gallery/exact-cylinder-steady-stokes/'))?.status()).toBe(200);
    expect(await headerIssue(page)).toBeNull();
    await activateHeader(page, '/');
    expect(await headerIssue(page)).toBeNull();
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
