import { expect, test } from '@playwright/test';

import {
  assertCoreVisible,
  assertKeyboardFocusVisible,
  assertMinimumTargetSizes,
  assertNoFakeExecutionControls,
  assertNoPageOverflow,
  assertNoSeriousAxeViolations,
  assertTextContrast,
} from './support';

const positive = `<!doctype html><html lang="en"><head><title>Fixture</title><style>
html,body{margin:0;max-width:100%;overflow-wrap:anywhere}main{padding:16px}.target{display:inline-flex;min-width:44px;min-height:44px;align-items:center;justify-content:center}.target:focus{outline:3px solid CanvasText;outline-offset:2px}
</style></head><body><main><h1>Ordinary semantic fixture</h1><p>This positive path is usable without client JavaScript.</p><a class="target" href="/">Home</a></main></body></html>`;

test.describe.configure({ mode: 'serial' });

test('00 ordinary browser and accessibility fixture passes before mutants', async ({ page }) => {
  await page.setContent(positive);
  await assertCoreVisible(page);
  await assertNoSeriousAxeViolations(page);
  await assertNoPageOverflow(page);
  await assertMinimumTargetSizes(page.getByRole('link', { name: 'Home' }));
  await assertKeyboardFocusVisible(page, page.getByRole('link', { name: 'Home' }));
  await assertNoFakeExecutionControls(page);
});

test('JavaScript-dependent core-content mutant is rejected with scripting disabled', async ({ browser }) => {
  const context = await browser.newContext({ javaScriptEnabled: false });
  const page = await context.newPage();
  await page.setContent(
    '<!doctype html><html lang="en"><body><main hidden><h1>Hydrated only</h1><p>Core content.</p></main><script>document.querySelector("main").hidden=false</script></body></html>',
  );
  await expect(assertCoreVisible(page)).rejects.toThrow();
  await context.close();
});

test('small target, page overflow, hidden core, focus, contrast, and fake controls are rejected', async ({ page }) => {
  await page.setContent(positive.replace('min-width:44px;min-height:44px', 'width:20px;height:20px'));
  await expect(assertMinimumTargetSizes(page.getByRole('link', { name: 'Home' }))).rejects.toThrow();

  await page.setContent(positive.replace('</main>', '<div style="width:200vw">overflow</div></main>'));
  await expect(assertNoPageOverflow(page)).rejects.toThrow();

  await page.setContent(positive.replace('<main>', '<main hidden>'));
  await expect(assertCoreVisible(page)).rejects.toThrow();

  await page.setContent(positive.replace('.target:focus{outline:3px solid CanvasText;outline-offset:2px}', '.target:focus{outline:none;box-shadow:none}'));
  await expect(assertKeyboardFocusVisible(page, page.getByRole('link', { name: 'Home' }))).rejects.toThrow();

  await page.setContent(positive.replace('<p>', '<p style="color:#777;background:#777">'));
  await expect(assertTextContrast(page.getByText('This positive path is usable without client JavaScript.'))).rejects.toThrow();

  await page.setContent(positive.replace('</main>', '<button>Run now</button></main>'));
  await expect(assertNoFakeExecutionControls(page)).rejects.toThrow();
});
