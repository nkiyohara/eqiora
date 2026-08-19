import AxeBuilder from '@axe-core/playwright';
import { expect, type Locator, type Page, type TestInfo } from '@playwright/test';

export const ROUTES = [
  '/',
  '/gallery/',
  '/gallery/exact-cylinder-steady-stokes/',
  '/reference/',
  '/reference/python/eqiora/',
  '/reference/rust/',
  '/reference/rust/api/eqiora/struct.Diagnostic.html',
  '/reference/cli/',
  '/reference/control-v2/',
  '/reference/mcp/',
  '/examples/',
] as const;

export const STAGES = [
  'Problem setup',
  'Eqiora model definition',
  'Mesh and boundaries',
  'Submit and result',
  'Pressure visualization',
  'Verified and not claimed',
] as const;

export async function rejectExternalRequests(page: Page): Promise<string[]> {
  const attempts: string[] = [];
  await page.route('**/*', async (route) => {
    const url = new URL(route.request().url());
    if (
      url.protocol === 'data:' ||
      url.protocol === 'blob:' ||
      url.origin === 'http://127.0.0.1:4173'
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
  for (const stage of STAGES) {
    const heading = page.getByRole('heading', { name: stage, exact: true });
    await expect(heading).toBeVisible();
    expect(
      await heading.evaluate((element) =>
        Boolean(element.closest('section, article, [role="region"]')),
      ),
      `${stage} is not contained by a semantic stage landmark`,
    ).toBe(true);
  }
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
    scroll: document.documentElement.scrollWidth,
  }));
  expect(dimensions.scroll, `page overflowed: ${JSON.stringify(dimensions)}`).toBeLessThanOrEqual(
    dimensions.client + 1,
  );
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
  expect(await page.getByRole('button', { name: executionLabel }).count()).toBe(0);
  expect(await page.getByRole('link', { name: executionLabel }).count()).toBe(0);
  const controls = page.locator('a, button, [role="button"], [role="link"], input[type="button"], input[type="submit"], input[type="reset"], input[type="image"]');
  for (let offset = 0; offset < (await controls.count()); offset += 1) {
    expect((await controls.nth(offset).innerText()).trim()).not.toMatch(executionLabel);
  }
  const links = page.locator('a, [role="link"]');
  for (let offset = 0; offset < (await links.count()); offset += 1) {
    const href = (await links.nth(offset).getAttribute('href'))?.trim().toLowerCase();
    expect(['', '#', 'javascript:void(0)']).not.toContain(href ?? '');
  }
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
