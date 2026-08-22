import { expect, test, type Browser } from '@playwright/test';

import {
  assertCoreVisible,
  assertExactTableSelectorScope,
  assertHonest320Reflow,
  assertKeyboardFocusVisible,
  assertMinimumTargetSizes,
  assertNoFakeExecutionControls,
  assertNoPageOverflow,
  assertNoSeriousAxeViolations,
  assertOrdinaryRoutePlan,
  assertSemanticStages,
  assertSupportedStatement,
  assertTableFixtureGreen,
  assertTableFixtureOnlyFailure,
  assertTextContrast,
  authenticatedParentLayoutCss,
  createOrdinaryRoutePlan,
  installTableObserver,
  launchOfficialBrowser,
  rejectExternalRequests,
  seriousAxeViolations,
  SITE_ROUTES,
  STAGES,
  SUPPORTED_STATEMENT,
} from './support';

const positive = `<!doctype html><html lang="en"><head><title>Fixture</title><style>
html,body{margin:0;max-width:100%;overflow-wrap:anywhere}main{padding:16px}.target{display:inline-flex;min-width:44px;min-height:44px;align-items:center;justify-content:center}.target:focus{outline:3px solid CanvasText;outline-offset:2px}
</style></head><body><main><h1>Ordinary semantic fixture</h1><h2>Submit and result</h2><p>This positive path is usable without client JavaScript. Prose may discuss Start computation, Evaluate model, and Begin processing.</p><a class="target" href="/">Home</a></main></body></html>`;

const productTableCss = `
.sl-markdown-content > table,
.sl-markdown-content .eq-stage__body > table{display:table;width:100%;table-layout:fixed;overflow:visible}
.sl-markdown-content > table :is(th,td,code),
.sl-markdown-content .eq-stage__body > table :is(th,td,code){width:auto;white-space:normal;overflow-wrap:anywhere;word-break:break-word}`;

const exactTableCss = `
html,body{margin:0;max-width:100%}.sl-markdown-content{width:300px}.wide{display:block;width:556px;white-space:nowrap}
.sl-markdown-content table:not(:where(.not-content *)){display:block;overflow-x:auto;border-collapse:collapse}
${productTableCss}
.ordinary:is(.comma-one,.comma-two){color:CanvasText}.escaped\\,comma{color:CanvasText}
th,td{border:1px solid;padding:4px;font-size:16px}`;

const tableCellText = 'one_unbreakable_table_value_that_must_reflow_without_concealment';

function tableFixture(
  css = exactTableCss,
  direct = '',
  component = '',
  componentParent = '',
): string {
  const cell = `<code class="wide"><span class="nested-text">${tableCellText}</span></code> <a href="/documentation/">Documentation</a>`;
  return `<!doctype html><html lang="en"><head><title>Table fixture</title><style>${css}</style></head><body><main><h1>Table fixture</h1><div class="sl-markdown-content"><table ${direct}><thead><tr><th>Direct heading</th></tr></thead><tbody><tr><td>${cell}</td></tr></tbody></table><section class="eq-stage"><div class="eq-stage__body" ${componentParent}><table ${component}><thead><tr><th>Component heading</th></tr></thead><tbody><tr><td>${cell}</td></tr></tbody></table></div></section></div></main></body></html>`;
}

function stageFixture(order = STAGES, statement = SUPPORTED_STATEMENT): string {
  const stages = order
    .map(
      ({ id, step, title }) => `<section class="eq-stage" id="${id}" data-step="${step}" aria-labelledby="${id}-title"><h2 id="${id}-title"><a class="eq-stage-marker" href="#${id}"><span><span class="eq-sr-only">Stage </span>${step}</span> <span class="eq-stage-marker__emoji" aria-hidden="true">★</span> <span>${title}</span></a></h2><div class="eq-stage__body"><p>Ordinary stage content for ${title}.</p></div></section>`,
    )
    .join('');
  return `<!doctype html><html lang="en"><body><main><h1>Stages</h1>${stages}<section class="eq-claim-boundary"><div class="eq-claim-boundary__panel--supported" role="group" aria-label="Supported"><p>${statement}</p></div><div class="eq-claim-boundary__panel--not-claimed" role="group" aria-label="Not claimed"><p>No broader claim.</p></div></section></main></body></html>`;
}

test.describe.configure({ mode: 'serial' });

let browser: Browser;

test.beforeAll(async () => {
  browser = await launchOfficialBrowser(true);
});

test.afterAll(async () => {
  await browser?.close();
});

test('00 official browser and every synthetic ordinary control pass first', async () => {
  const context = await browser.newContext({ locale: 'en-GB', serviceWorkers: 'block' });
  const page = await context.newPage();
  await installTableObserver(page);
  await page.setContent(positive);
  await assertCoreVisible(page);
  await assertNoSeriousAxeViolations(page);
  await assertNoPageOverflow(page);
  await assertMinimumTargetSizes(page.getByRole('link', { name: 'Home' }));
  await assertKeyboardFocusVisible(page, page.getByRole('link', { name: 'Home' }));
  await assertNoFakeExecutionControls(page);

  for (const ordinary of [
    '<input type="button" value="Documentation" aria-label="Documentation">',
    '<button><img src="data:image/gif;base64,R0lGODlhAQABAAAAACw=" alt="Documentation"></button>',
    '<input type="text" value="Start computation">',
  ]) {
    await page.setContent(positive.replace('</main>', `${ordinary}</main>`));
    await assertNoFakeExecutionControls(page);
  }

  await page.setContent(
    positive.replace(
      '</main>',
      '<a href="#submit-and-result">Stage 4 Submit and result</a><a href="https://github.com/nkiyohara/eqiora/blob/4fc67a9fc94aeedd44a4ace31d406ac949c81f12/examples/python/exact_cylinder_stokes_marimo.py#L77-L95">Eqiora source form: canonical intent/submit/result cells</a></main>',
    ),
  );
  await assertNoFakeExecutionControls(page);

  const noScript = await browser.newContext({ javaScriptEnabled: false, locale: 'en-GB' });
  const noScriptPage = await noScript.newPage();
  await noScriptPage.setContent(positive);
  await assertCoreVisible(noScriptPage);
  await noScript.close();

  await page.emulateMedia({ forcedColors: 'active' });
  await page.setContent(positive);
  await assertCoreVisible(page);

  await page.emulateMedia({ forcedColors: 'none', colorScheme: 'dark' });
  const external = await rejectExternalRequests(page);
  await page.setContent(
    positive.replace(
      '</main>',
      '<img src="data:image/gif;base64,R0lGODlhAQABAAAAACw=" alt="local data image"></main>',
    ),
  );
  expect(external).toEqual([]);

  await page.setContent(tableFixture());
  assertExactTableSelectorScope(exactTableCss);
  const parentCss = authenticatedParentLayoutCss();
  assertExactTableSelectorScope(`${parentCss}\n${productTableCss}`);
  await assertTableFixtureGreen(page);
  await context.close();
});

test('01 predecessor JavaScript, forced-colour, request, and basic accessibility mutants are causal', async () => {
  const noScript = await browser.newContext({ javaScriptEnabled: false, locale: 'en-GB' });
  const noScriptPage = await noScript.newPage();
  await noScriptPage.setContent(positive);
  await assertCoreVisible(noScriptPage);
  await noScriptPage.setContent(
    '<!doctype html><html lang="en"><body><main hidden><h1>Hydrated only</h1><p>Core content.</p></main><script>document.querySelector("main").hidden=false</script></body></html>',
  );
  await expect(assertCoreVisible(noScriptPage)).rejects.toThrow();
  await noScript.close();

  const context = await browser.newContext({ locale: 'en-GB', serviceWorkers: 'block' });
  const page = await context.newPage();
  await page.emulateMedia({ forcedColors: 'active' });
  await page.setContent(positive);
  await assertCoreVisible(page);
  await page.setContent(
    positive.replace('</style>', '@media (forced-colors: active){main{display:none}}</style>'),
  );
  await expect(assertCoreVisible(page)).rejects.toThrow();

  await page.emulateMedia({ forcedColors: 'none', colorScheme: 'dark' });
  const external = await rejectExternalRequests(page);
  await page.setContent(positive);
  expect(external).toEqual([]);
  await page.setContent(
    positive.replace(
      '</main>',
      '<img src="https://example.com/dark-only.png" alt="mutant"></main>',
    ),
  );
  await expect.poll(() => external.length).toBeGreaterThan(0);

  await page.emulateMedia({ colorScheme: 'light' });
  await page.setContent(
    positive.replace('min-width:44px;min-height:44px', 'width:20px;height:20px'),
  );
  await expect(assertMinimumTargetSizes(page.getByRole('link', { name: 'Home' }))).rejects.toThrow();
  await page.setContent(positive.replace('</main>', '<div style="width:200vw">overflow</div></main>'));
  await expect(assertNoPageOverflow(page)).rejects.toThrow();
  await page.setContent(positive.replace('<main>', '<main hidden>'));
  await expect(assertCoreVisible(page)).rejects.toThrow();
  await page.setContent(
    positive.replace(
      '.target:focus{outline:3px solid CanvasText;outline-offset:2px}',
      '.target:focus{outline:none;box-shadow:none}',
    ),
  );
  await expect(
    assertKeyboardFocusVisible(page, page.getByRole('link', { name: 'Home' })),
  ).rejects.toThrow();
  await page.setContent(positive.replace('<p>', '<p style="color:#777;background:#777">'));
  await expect(
    assertTextContrast(page.getByText('This positive path is usable without client JavaScript.')),
  ).rejects.toThrow();
  await context.close();
});

test('02 O-4 paired controls and every execution-affordance mutant are causal', async () => {
  const context = await browser.newContext({ locale: 'en-GB', serviceWorkers: 'block' });
  const page = await context.newPage();
  for (const [ordinary, mutant] of [
    [
      '<input type="button" value="Documentation" aria-label="Documentation">',
      '<input type="button" value="Start computation" aria-label="Documentation">',
    ],
    [
      '<button><img src="data:image/gif;base64,R0lGODlhAQABAAAAACw=" alt="Documentation"></button>',
      '<button><img src="data:image/gif;base64,R0lGODlhAQABAAAAACw=" alt="Start computation"></button>',
    ],
    [
      '<input type="text" value="Start computation">',
      '<input type="BUTTON" value="Start computation">',
    ],
  ]) {
    await page.setContent(positive.replace('</main>', `${ordinary}</main>`));
    await assertNoFakeExecutionControls(page);
    await page.setContent(positive.replace('</main>', `${mutant}</main>`));
    await expect(assertNoFakeExecutionControls(page), mutant).rejects.toThrow();
  }

  for (const label of [
    'Run simulation',
    'Execute calculation',
    'Launch computation',
    'Submit and result',
    'Start computation',
    'Evaluate model',
    'Begin processing',
    'Generate result',
    'Analyse case',
    'Predict flow',
  ]) {
    await page.setContent(positive.replace('</main>', `<a href="/real-route/">${label}</a></main>`));
    await expect(assertNoFakeExecutionControls(page), label).rejects.toThrow();
  }

  for (const control of [
    '<button>Run now</button>',
    '<a href="/real-route/" aria-label="Run simulation">Documentation</a>',
    '<a href="/real-route/" aria-label="Documentation">Run simulation</a>',
    '<div role="button" aria-label="Execute calculation">Details</div>',
    '<input type="button" value="Launch computation">',
    '<a href="/real-route/" aria-labelledby="execution-label">Docs</a><span id="execution-label">Start computation</span>',
    '<a href="#">Documentation</a>',
    '<a href="javascript:void(0)">Documentation</a>',
  ]) {
    await page.setContent(positive.replace('</main>', `${control}</main>`));
    await expect(assertNoFakeExecutionControls(page), control).rejects.toThrow();
  }

  await page.setContent(
    positive.replace('</main>', '<a href="#submit-and-result">Stage 4 Submit and result</a></main>'),
  );
  await assertNoFakeExecutionControls(page);
  await page.setContent(
    positive.replace('</main>', '<a href="#submit-and-result">Submit Stage 4 result</a></main>'),
  );
  await expect(assertNoFakeExecutionControls(page)).rejects.toThrow();
  await context.close();
});

test('03 O-1 through O-3 exact mechanism, stage, and claim mutants are causal', async () => {
  test.setTimeout(120_000);
  const context = await browser.newContext({ locale: 'en-GB', serviceWorkers: 'block' });
  const page = await context.newPage();
  await page.setViewportSize({ width: 320, height: 844 });
  await page.setContent(positive);
  await assertHonest320Reflow(page);
  await assertCoreVisible(page);
  await assertNoPageOverflow(page);

  for (const mutant of [
    positive.replace('</style>', 'html{zoom:4}</style>'),
    positive.replace('</style>', 'html{transform:scale(.25)}</style>'),
    positive.replace('<title>Fixture</title>', '<title>Fixture</title><meta name="viewport" content="width=1280">'),
  ]) {
    await page.setContent(positive);
    await assertHonest320Reflow(page);
    await page.setContent(mutant);
    await expect(assertHonest320Reflow(page)).rejects.toThrow();
  }

  const ordinaryStages = stageFixture();
  await page.setContent(ordinaryStages);
  await assertSemanticStages(page);
  await assertSupportedStatement(page);

  for (const mutant of [
    ordinaryStages.replace('<span class="eq-sr-only">Stage </span>', ''),
    ordinaryStages.replace('data-step="1"', 'data-step="2"'),
    stageFixture([...STAGES].reverse()),
    ordinaryStages.replace(
      'class="eq-stage-marker__emoji" aria-hidden="true"',
      'class="eq-stage-marker__emoji"',
    ),
    ordinaryStages.replace('aria-labelledby="problem-setup-title"', 'aria-label="Problem setup"'),
  ]) {
    await page.setContent(mutant);
    await expect(assertSemanticStages(page)).rejects.toThrow();
  }

  await page.setContent(stageFixture(STAGES, SUPPORTED_STATEMENT.toLowerCase()));
  await expect(assertSupportedStatement(page)).rejects.toThrow();
  await page.setContent(
    stageFixture().replace(
      `<p>${SUPPORTED_STATEMENT}</p>`,
      `<p>${SUPPORTED_STATEMENT}</p><p>${SUPPORTED_STATEMENT}</p>`,
    ),
  );
  await expect(assertSupportedStatement(page)).rejects.toThrow();
  await page.setContent(
    stageFixture().replace(
      `<div class="eq-claim-boundary__panel--supported" role="group" aria-label="Supported"><p>${SUPPORTED_STATEMENT}</p></div><div class="eq-claim-boundary__panel--not-claimed" role="group" aria-label="Not claimed"><p>No broader claim.</p>`,
      `<div class="eq-claim-boundary__panel--supported" role="group" aria-label="Supported"><p>Different statement.</p></div><div class="eq-claim-boundary__panel--not-claimed" role="group" aria-label="Not claimed"><p>${SUPPORTED_STATEMENT}</p>`,
    ),
  );
  await expect(assertSupportedStatement(page)).rejects.toThrow();
  await context.close();
});

test('04 exact two-shape table scope and concealment/focus/vacuity mutants are causal', async () => {
  test.setTimeout(120_000);
  const context = await browser.newContext({ locale: 'en-GB', serviceWorkers: 'block' });
  const page = await context.newPage();
  await installTableObserver(page);
  await page.setViewportSize({ width: 320, height: 844 });

  const resetPositive = async () => {
    await page.setContent(tableFixture());
    assertExactTableSelectorScope(exactTableCss);
    await assertTableFixtureGreen(page);
  };
  await resetPositive();

  const directOnly = exactTableCss.replaceAll(
    '.sl-markdown-content .eq-stage__body > table',
    '.sl-markdown-content > table',
  );
  await page.setContent(tableFixture(directOnly));
  expect(
    await page
      .locator('.sl-markdown-content > table')
      .evaluate((table) => table.scrollWidth <= table.clientWidth + 1),
  ).toBe(true);
  expect(
    await page
      .locator('.eq-stage__body > table')
      .evaluate((table) => table.scrollWidth > table.clientWidth + 1),
  ).toBe(true);
  await expect(assertTableFixtureGreen(page)).rejects.toThrow();

  await resetPositive();
  const componentOnly = exactTableCss.replaceAll(
    '.sl-markdown-content > table',
    '.sl-markdown-content .eq-stage__body > table',
  );
  await page.setContent(tableFixture(componentOnly));
  expect(
    await page
      .locator('.eq-stage__body > table')
      .evaluate((table) => table.scrollWidth <= table.clientWidth + 1),
  ).toBe(true);
  expect(
    await page
      .locator('.sl-markdown-content > table')
      .evaluate((table) => table.scrollWidth > table.clientWidth + 1),
  ).toBe(true);
  await expect(assertTableFixtureGreen(page)).rejects.toThrow();

  await resetPositive();
  const broad = `${exactTableCss}\n.sl-markdown-content table{display:table;width:100%}`;
  await page.setContent(tableFixture(broad));
  await assertTableFixtureGreen(page);
  await expect(async () => assertExactTableSelectorScope(broad)).rejects.toThrow(
    'unowned table selector branch',
  );

  const addTableBranch = (branch: string) =>
    exactTableCss.replace(
      '.sl-markdown-content > table,\n.sl-markdown-content .eq-stage__body > table{',
      `.sl-markdown-content > table,\n.sl-markdown-content .eq-stage__body > table,\n${branch}{`,
    );
  for (const branch of ['.other table', '.sl-markdown-content .arbitrary table']) {
    await resetPositive();
    const mutantCss = addTableBranch(branch);
    await page.setContent(tableFixture(mutantCss));
    await assertTableFixtureGreen(page);
    await expect(async () => assertExactTableSelectorScope(mutantCss), branch).rejects.toThrow(
      'unowned table selector branch',
    );
  }

  const runSelectorPair = async (
    ordinaryBranch: string,
    mutantBranch: string,
    rejection: string,
  ) => {
    await resetPositive();
    const ordinaryCss = ordinaryBranch.startsWith('@media')
      ? `${exactTableCss}\n${ordinaryBranch}`
      : `${exactTableCss}\n${ordinaryBranch}{overflow-x:auto}`;
    assertExactTableSelectorScope(ordinaryCss);
    await page.setContent(tableFixture(ordinaryCss));
    await assertTableFixtureGreen(page);

    const mutantCss = mutantBranch.startsWith('@media')
      ? `${exactTableCss}\n${mutantBranch}`
      : `${exactTableCss}\n${mutantBranch}{overflow-x:auto}`;
    await page.setContent(tableFixture(mutantCss));
    await assertTableFixtureGreen(page);
    await expect(
      async () => assertExactTableSelectorScope(mutantCss),
      mutantBranch,
    ).rejects.toThrow(rejection);
  };
  for (const pair of [
    ['.other :is(.ordinary)', '.other :is(table)', 'unowned table selector branch'],
    [
      '@media (width > 0px){.safe, .other :is(:where(.ordinary)){overflow-x:auto}}',
      '@media (width > 0px){.safe, .other :is(:where(\\74 able)){overflow-x:auto}}',
      'unsupported escaped table-target selector authority',
    ],
  ] as const) {
    await runSelectorPair(...pair);
  }

  const unrelatedFunctionalCss = `${exactTableCss}\n.other:lang(table),
.other :is([data-kind="table"]),
.other :where([data-kind="\\74 able"]){overflow-x:auto}`;
  await resetPositive();
  assertExactTableSelectorScope(unrelatedFunctionalCss);
  await page.setContent(tableFixture(unrelatedFunctionalCss));
  await assertTableFixtureGreen(page);

  for (const pair of [
    ['.other :current(.ordinary)', '.other :current(table)', 'unowned table selector branch'],
    [
      '@media (width > 0px){.safe, .other :current(:is(.ordinary)){overflow-x:auto}}',
      '@media (width > 0px){.safe, .other :current(:is(\\74 able)){overflow-x:auto}}',
      'unsupported escaped table-target selector authority',
    ],
  ] as const) {
    await runSelectorPair(...pair);
  }

  expect(() =>
    assertExactTableSelectorScope(`${exactTableCss}\n.other :unknown(.ordinary){overflow-x:auto}`),
  ).toThrow('unsupported functional pseudo-class selector');

  const authenticatedParentCss = authenticatedParentLayoutCss();
  const focusParentCss = `${authenticatedParentCss}\nhtml,body{margin:0;max-width:100%}.sl-markdown-content{width:300px}.wide{display:block;width:556px;white-space:nowrap}`;
  const focusParentFixture = tableFixture(focusParentCss).replaceAll(
    ' <a href="/documentation/">Documentation</a>',
    '',
  );
  await page.setContent(focusParentFixture);
  expect(await page.locator('a').count()).toBe(0);
  for (const selector of ['.sl-markdown-content > table', '.eq-stage__body > table']) {
    expect(
      await page.locator(selector).evaluate((table) => table.scrollWidth > table.clientWidth + 1),
    ).toBe(true);
  }
  const parentViolations = await seriousAxeViolations(page);
  expect(parentViolations).toHaveLength(1);
  expect(parentViolations[0].id).toBe('scrollable-region-focusable');
  expect(parentViolations[0].nodes).toHaveLength(2);
  await page.setContent(
    focusParentFixture.replace('<table ><thead><tr><th>Component heading</th>', '<table tabindex="0"><thead><tr><th>Component heading</th>'),
  );
  expect(
    await page
      .locator('.eq-stage__body > table')
      .evaluate((table) => table.scrollWidth > table.clientWidth + 1),
  ).toBe(true);
  const silencedViolations = await seriousAxeViolations(page);
  expect(silencedViolations).toHaveLength(1);
  expect(silencedViolations[0].id).toBe('scrollable-region-focusable');
  expect(silencedViolations[0].nodes).toHaveLength(1);
  await expect(assertTableFixtureGreen(page)).rejects.toThrow();

  await resetPositive();
  await page.locator('.eq-stage__body').evaluate((parent) => parent.setAttribute('tabindex', '0'));
  await assertTableFixtureOnlyFailure(page, 'focus');

  await resetPositive();
  await page.locator('.eq-stage__body').evaluate((parent) => parent.setAttribute('role', 'group'));
  await assertTableFixtureOnlyFailure(page, 'role');

  await resetPositive();
  await page.locator('.eq-stage__body').evaluate((parent) => {
    parent.addEventListener('keydown', () => undefined);
  });
  await assertTableFixtureOnlyFailure(page, 'handler');

  const d4Css = `${exactTableCss}\n.eq-stage{width:300px;max-width:100%;overflow:visible}`;
  await page.setContent(
    tableFixture(
      d4Css,
      '',
      'style="width:300px"',
      'style="width:120px;overflow-x:visible"',
    ),
  );
  assertExactTableSelectorScope(d4Css);
  await assertTableFixtureGreen(page);
  const d4Before = await page.locator('.eq-stage__body').evaluate((parent) => {
    const table = parent.querySelector('table');
    if (!(table instanceof HTMLTableElement)) throw new Error('missing D-4 table');
    return {
      parentClient: parent.clientWidth,
      parentScroll: parent.scrollWidth,
      overflowX: getComputedStyle(parent).overflowX,
      tableClient: table.clientWidth,
      tableScroll: table.scrollWidth,
      documentClient: document.documentElement.clientWidth,
      documentScroll: document.documentElement.scrollWidth,
      bodyScroll: document.body.scrollWidth,
    };
  });
  expect(d4Before).toMatchObject({
    parentClient: 120,
    parentScroll: 300,
    overflowX: 'visible',
    tableClient: 300,
    tableScroll: 300,
  });
  expect(d4Before.documentScroll).toBeLessThanOrEqual(d4Before.documentClient + 1);
  expect(d4Before.bodyScroll).toBeLessThanOrEqual(d4Before.documentClient + 1);
  await page.locator('.eq-stage__body').evaluate((parent) => {
    (parent as HTMLElement).style.overflowX = 'auto';
  });
  const d4After = await page.locator('.eq-stage__body').evaluate((parent) => {
    const table = parent.querySelector('table');
    if (!(table instanceof HTMLTableElement)) throw new Error('missing D-4 table');
    return {
      parentClient: parent.clientWidth,
      parentScroll: parent.scrollWidth,
      overflowX: getComputedStyle(parent).overflowX,
      tableClient: table.clientWidth,
      tableScroll: table.scrollWidth,
    };
  });
  expect(d4After).toEqual({
    parentClient: d4Before.parentClient,
    parentScroll: d4Before.parentScroll,
    overflowX: 'auto',
    tableClient: d4Before.tableClient,
    tableScroll: d4Before.tableScroll,
  });
  await assertTableFixtureOnlyFailure(page, 'parentLocalOverflow');

  for (const [property, value, failure] of [
    ['opacity', '0', 'concealment'],
    ['user-select', 'none', 'selection'],
  ] as const) {
    await resetPositive();
    await page.locator('.nested-text').first().evaluate(
      (descendant, mutation) => {
        (descendant as HTMLElement).style.setProperty(mutation.property, mutation.value);
      },
      { property, value },
    );
    await assertTableFixtureOnlyFailure(page, failure);
  }

  const setClipPath = async (value: string) => {
    const descendant = page.locator('.nested-text').first();
    await descendant.evaluate((node, clipPath) => {
      (node as HTMLElement).style.clipPath = clipPath;
    }, value);
    expect(await descendant.evaluate((node) => getComputedStyle(node).clipPath)).not.toBe('none');
  };
  for (const [ordinaryClip, fullClip] of [
    ['inset(0)', 'inset(100%)'],
    ['circle(100%)', 'circle(0)'],
    ['ellipse(100% 100%)', 'ellipse(0 50%)'],
    ['polygon(0 0, 100% 0, 100% 100%, 0 100%)', 'polygon(0 0, 0 0, 0 0)'],
    ['inset(1px)', 'inset(50%)'],
    ['circle(49% at 50% 50%)', 'circle(0px at 50% 50%)'],
    ['ellipse(49% 49% at 50% 50%)', 'ellipse(0px 49% at 50% 50%)'],
    [
      'polygon(0 0, 100% 0, 100% 90%, 0 90%)',
      'polygon(0 0, 50% 50%, 100% 100%)',
    ],
    [
      'polygon(0 0, 100% 100%, 100% 0, 0 100%)',
      'polygon(0 0, 100% 0, 0 0, 0 100%, 0 0)',
    ],
  ]) {
    await resetPositive();
    await setClipPath(ordinaryClip);
    await assertTableFixtureGreen(page);
    await setClipPath(fullClip);
    await assertTableFixtureOnlyFailure(page, 'concealment');
  }

  await resetPositive();
  await page.setContent(
    tableFixture().replace(
      '<table ><thead><tr><th>Direct heading</th>',
      '<div><table><thead><tr><th>Direct heading</th>',
    ).replace('</tbody></table><section class="eq-stage">', '</tbody></table></div><section class="eq-stage">'),
  );
  await expect(assertTableFixtureGreen(page)).rejects.toThrow();

  await resetPositive();
  await page.setContent(tableFixture(exactTableCss, 'style="overflow:hidden"'));
  await expect(assertTableFixtureGreen(page)).rejects.toThrow();

  await resetPositive();
  await page.setContent(
    tableFixture().replace(
      '<td><code class="wide">',
      '<td style="overflow:hidden;text-overflow:ellipsis"><code class="wide">',
    ),
  );
  await expect(assertTableFixtureGreen(page)).rejects.toThrow();

  await resetPositive();
  await page.setContent(
    tableFixture().replace(
      `<span class="nested-text">${tableCellText}</span>`,
      `<span hidden>concealed</span><span class="nested-text">${tableCellText}</span>`,
    ),
  );
  await expect(assertTableFixtureGreen(page)).rejects.toThrow();

  await resetPositive();
  await page.setContent(
    tableFixture().replace(
      '<td><code class="wide">',
      '<td style="font-size:1px"><code class="wide">',
    ),
  );
  await expect(assertTableFixtureGreen(page)).rejects.toThrow();

  await resetPositive();
  const firstHeader = page.locator('th').first();
  const firstCell = page.locator('td').first();
  const headerBox = await firstHeader.boundingBox();
  const cellBox = await firstCell.boundingBox();
  expect(headerBox).not.toBeNull();
  expect(cellBox).not.toBeNull();
  await firstCell.evaluate(
    (cell, delta) => {
      (cell as HTMLElement).style.transform = `translateY(${delta}px)`;
    },
    headerBox!.y - cellBox!.y,
  );
  await expect(assertTableFixtureGreen(page)).rejects.toThrow();

  await resetPositive();
  await page.setContent(
    tableFixture().replace('</main>', '<table hidden><tr><th>Alternate</th></tr></table></main>'),
  );
  await expect(assertTableFixtureGreen(page)).rejects.toThrow();

  await resetPositive();
  await page.setContent(
    tableFixture().replace('</main>', '<table><tr><th>Duplicate</th></tr></table></main>'),
  );
  await expect(assertTableFixtureGreen(page)).rejects.toThrow();

  await resetPositive();
  await page.setContent(
    '<!doctype html><html lang="en"><body><main><h1>Missing tables</h1><p>Supply is present but the exact tables are absent.</p></main></body></html>',
  );
  await expect(assertTableFixtureGreen(page)).rejects.toThrow();
  await context.close();
});

test('05 stable route-partition order, duplicate, and missing mutants are causal', () => {
  {
    const ordinary = createOrdinaryRoutePlan();
    expect(assertOrdinaryRoutePlan(ordinary)).toEqual([...SITE_ROUTES]);
    const reorderedB = [...ordinary.B];
    const root = reorderedB.indexOf('/');
    const api = reorderedB.indexOf('/api/');
    expect({ root, api }).toEqual({ root: 0, api: 1 });
    [reorderedB[root], reorderedB[api]] = [reorderedB[api], reorderedB[root]];
    const mutant = { A: [...ordinary.A], B: reorderedB, C: [...ordinary.C] };
    expect(() => assertOrdinaryRoutePlan(mutant)).toThrow('ORDER-REORDER B');
  }

  {
    const ordinary = createOrdinaryRoutePlan();
    expect(assertOrdinaryRoutePlan(ordinary)).toHaveLength(34);
    const mutant = {
      A: [...ordinary.A, '/evidence/'],
      B: [...ordinary.B],
      C: [...ordinary.C],
    };
    expect(() => assertOrdinaryRoutePlan(mutant)).toThrow('ORDER-DUPLICATE: /evidence/');
  }

  {
    const ordinary = createOrdinaryRoutePlan();
    expect(assertOrdinaryRoutePlan(ordinary)).toHaveLength(34);
    const mutant = { A: ordinary.A.slice(1), B: [...ordinary.B], C: [...ordinary.C] };
    expect(() => assertOrdinaryRoutePlan(mutant)).toThrow('ORDER-MISSING: /evidence/');
  }
});
