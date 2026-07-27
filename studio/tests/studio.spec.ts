import AxeBuilder from "@axe-core/playwright";
import { expect, type Page, test } from "@playwright/test";

const WCAG_22_AA_TAGS = ["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa"];

async function expectNoSeriousOrCriticalViolations(page: Page) {
  const results = await new AxeBuilder({ page }).withTags(WCAG_22_AA_TAGS).analyze();
  const blocking = results.violations.filter(
    (violation) => violation.impact === "serious" || violation.impact === "critical",
  );
  expect(blocking).toEqual([]);
}

async function executePaletteCommand(page: Page, query: string, commandName: RegExp) {
  await page.keyboard.press("Control+k");
  const dialog = page.getByRole("dialog", { name: "Commands" });
  await page.getByRole("searchbox", { name: "Search commands" }).fill(query);
  await dialog.getByRole("button", { name: commandName }).click();
}

test("projects, inspects, and runs without pointer-only interaction", async ({ page }) => {
  await page.goto("/");

  await expect(page.getByText("Browser preview", { exact: true })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Relation view" })).toBeVisible();
  await expect(page.getByRole("article")).toHaveCount(4);

  await page.getByRole("button", { name: /decay Relation/ }).focus();
  await page.keyboard.press("Enter");
  await expect(page.getByRole("heading", { name: "decay", exact: true })).toBeVisible();

  const canonicalId = page.getByText("Relation:decay", { exact: true });
  await expect(canonicalId).toBeVisible();
  await page.getByRole("button", { name: "Move entity right" }).click();
  await expect(canonicalId).toBeVisible();

  await expect(page.getByText("Plan accepted", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Run accepted plan" }).focus();
  await page.keyboard.press("Enter");
  await expect(page.getByText("41 samples", { exact: true })).toBeVisible();
  await expect(page.getByText("0.0407622", { exact: true })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Evidence" })).toBeVisible();
  await expect(page.getByText("Semantic oracle", { exact: true })).toBeVisible();
  // Read from the run record rather than a fixed sentence. The previous
  // assertion pinned prose that no data fed, so it would have kept passing after
  // an independent verifier existed.
  await expect(page.getByText("No second-backend re-verification", { exact: true })).toBeVisible();
  // RFC 0076: the state is readable as text, and the one provenance segment a
  // run record cannot answer is shown as unavailable rather than omitted, since
  // an absent segment reads as verified.
  await expect(page.getByText("Registered evidence", { exact: true })).toBeVisible();
  await expect(page.getByText("gap in the owning contract", { exact: false })).toBeVisible();
  // The disclosure starts closed, so open it before asserting its content.
  await page.getByText("What supports this result?", { exact: true }).click();
  await expect(page.getByText("is not shown", { exact: false }).first()).toBeVisible();
  await page.screenshot({ path: "test-results/studio-evidence-1440x900.png" });

  const maxStep = page.getByRole("textbox", { name: /Max step/ });
  await maxStep.fill("0.2");
  await expect(page.getByText("Previous run", { exact: true })).toBeVisible();
  await expect(page.getByText(/Run inputs have changed/)).toBeVisible();
  await maxStep.fill("1e-1");
  await expect(page.getByText("Previous run", { exact: true })).toHaveCount(0);

  const editor = page.getByRole("textbox", { name: "Eqiora model source" });
  await editor.fill(`${await editor.inputValue()}\n`);
  await expect(page.getByText("Pending source", { exact: true })).toBeVisible();
  await expect(page.getByText(/trajectory remains evidence for digest/)).toBeVisible();

  const overflow = await page.evaluate(() => ({
    horizontal: document.documentElement.scrollWidth - innerWidth,
    vertical: document.documentElement.scrollHeight - innerHeight,
  }));
  expect(overflow.horizontal).toBeLessThanOrEqual(0);
  expect(overflow.vertical).toBeLessThanOrEqual(0);
  await page.screenshot({ path: "test-results/studio-1440x900.png" });
});

test("keeps every primary action unobstructed at the minimum shell size", async ({ page }) => {
  await page.setViewportSize({ width: 1024, height: 680 });
  await page.goto("/");
  const run = page.getByRole("button", { name: "Run accepted plan" });
  await expect(run).toBeVisible();
  await run.click();
  await expect(page.getByText("41 samples", { exact: true })).toBeVisible();
});

test("selects exact CAD Domains from both the semantic table and keyboard viewport", async ({
  page,
}) => {
  await page.goto("/");
  await executePaletteCommand(page, "open CAD example", /Open CAD example/);

  await expect(page.getByRole("heading", { name: "Semantic geometry" })).toBeVisible();
  await expect(page.getByRole("row")).toHaveCount(8);
  await expect(page.getByText("Geometry 3333333333", { exact: true })).toBeVisible();

  const tableSelection = page
    .getByRole("table")
    .getByRole("button", { name: "x_upper Domain:0-upper" });
  await tableSelection.focus();
  await page.keyboard.press("Enter");
  const inspector = page.getByRole("complementary", { name: "Selection" });
  await expect(inspector.getByRole("heading", { name: "x_upper" })).toBeVisible();
  await expect(inspector.getByText("X upper · parent-outward", { exact: true })).toBeVisible();
  await expect(inspector.getByText("1", { exact: true })).toBeVisible();

  const viewportSelection = page.getByRole("button", {
    name: "Select y_lower, Y lower · parent-outward",
  });
  await viewportSelection.focus();
  await page.keyboard.press("Enter");
  await expect(inspector.getByRole("heading", { name: "y_lower" })).toBeVisible();
  await expectNoSeriousOrCriticalViolations(page);

  const overflow = await page.evaluate(() => ({
    horizontal: document.documentElement.scrollWidth - innerWidth,
    vertical: document.documentElement.scrollHeight - innerHeight,
  }));
  expect(overflow.horizontal).toBeLessThanOrEqual(0);
  expect(overflow.vertical).toBeLessThanOrEqual(0);
  await page.screenshot({ path: "test-results/studio-cad-semantic-selection-1440x900.png" });

  await page.setViewportSize({ width: 1024, height: 680 });
  await expect(page.getByRole("table")).toBeVisible();
  await expect(viewportSelection).toBeVisible();
  const compactOverflow = await page.evaluate(() => ({
    horizontal: document.documentElement.scrollWidth - innerWidth,
    vertical: document.documentElement.scrollHeight - innerHeight,
  }));
  expect(compactOverflow.horizontal).toBeLessThanOrEqual(0);
  expect(compactOverflow.vertical).toBeLessThanOrEqual(0);
});

test("cancels at an accepted boundary and preserves prior completed evidence", async ({ page }) => {
  await page.goto("/");
  const run = page.getByRole("button", { name: "Run accepted plan" });
  await run.click();
  await expect(page.getByText("41 samples", { exact: true })).toBeVisible();

  await run.click();
  const cancel = page.getByRole("button", { name: "Cancel at safe point" });
  await expect(cancel).toBeVisible();
  await expect(
    page.getByRole("progressbar", { name: "Accepted model-time progress" }),
  ).toBeVisible();
  const actionBounds = await page.evaluate(() => {
    const panel = document.querySelector<HTMLElement>(".run-panel")?.getBoundingClientRect();
    const button = Array.from(document.querySelectorAll<HTMLElement>("button"))
      .find((candidate) => candidate.textContent?.trim() === "Cancel at safe point")
      ?.getBoundingClientRect();
    return panel === undefined || button === undefined
      ? null
      : {
          panelBottom: panel.bottom,
          buttonBottom: button.bottom,
          buttonTop: button.top,
          panelTop: panel.top,
        };
  });
  expect(actionBounds).not.toBeNull();
  expect(actionBounds?.buttonTop).toBeGreaterThanOrEqual(actionBounds?.panelTop ?? 0);
  expect(actionBounds?.buttonBottom).toBeLessThanOrEqual(actionBounds?.panelBottom ?? 0);
  await page.screenshot({ path: "test-results/studio-running-1440x900.png" });
  await cancel.click();

  await expect(page.getByText(/Cancelled at .* accepted steps/)).toBeVisible();
  await expect(page.getByText(/No partial result was admitted/)).toBeVisible();
  await expect(page.getByText("41 samples", { exact: true })).toBeVisible();
  await expect(page.getByText("Cancelled", { exact: true })).toBeVisible();
  await expectNoSeriousOrCriticalViolations(page);
  await page.screenshot({ path: "test-results/studio-cancelled-1440x900.png" });
});

test("offers a keyboard compile shortcut and visible focus", async ({ page }) => {
  await page.goto("/");
  await page.keyboard.press("Tab");
  const skipLink = page.getByRole("link", { name: "Skip to model workspace" });
  await expect(skipLink).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(page.locator("#workspace")).toBeFocused();

  const editor = page.getByRole("textbox", { name: "Eqiora model source" });
  await editor.focus();
  await page.keyboard.press("Control+Enter");
  await expect(page.getByText("STPREVIEW", { exact: true })).toBeVisible();
  await expect(page.getByText(/Browser preview cannot compile source/)).toBeVisible();
  await expect(editor).toBeFocused();
  await expect(editor).toHaveCSS("box-shadow", /rgb/);
});

test("commits a typed value transaction and navigates immutable revision lineage", async ({
  page,
}) => {
  await page.goto("/");
  await page.getByRole("button", { name: /rate Parameter/ }).click();
  const value = page.getByRole("textbox", { name: "Value", exact: true });
  await expect(value).toHaveValue("0.8");
  await value.fill("1.2");

  const commit = page.getByRole("button", { name: "Commit revision" });
  await expect(commit).toBeEnabled();
  await expect(page.getByText(/Revision 1 and the current value must still match/)).toBeVisible();
  await expectNoSeriousOrCriticalViolations(page);
  await page.screenshot({ path: "test-results/studio-value-edit-preview-1440x900.png" });
  await commit.click();

  await expect(page.getByText("Revision 2", { exact: true })).toBeVisible();
  await expect(page.getByText("Source basis", { exact: true })).toBeVisible();
  await expect(value).toHaveValue("1.2");
  await expect(page.getByRole("button", { name: "Previous revision" })).toBeEnabled();
  await expect(page.getByRole("button", { name: "Next revision" })).toBeDisabled();

  await page.getByRole("button", { name: "Previous revision" }).click();
  await expect(page.getByText("Revision 1", { exact: true })).toBeVisible();
  await expect(value).toHaveValue("0.8");
  await expect(page.getByText("Source basis", { exact: true })).toHaveCount(0);
  await page.getByRole("button", { name: "Next revision" }).click();
  await expect(page.getByText("Revision 2", { exact: true })).toBeVisible();
  await expect(value).toHaveValue("1.2");
});

test("provides every primary operation through an accessible command palette", async ({ page }) => {
  await page.goto("/");
  const editor = page.getByRole("textbox", { name: "Eqiora model source" });
  await editor.focus();

  await page.keyboard.press("Control+k");
  const dialog = page.getByRole("dialog", { name: "Commands" });
  await expect(dialog).toBeVisible();
  await expect(page.getByRole("searchbox", { name: "Search commands" })).toBeFocused();
  await expectNoSeriousOrCriticalViolations(page);
  await page.screenshot({ path: "test-results/studio-command-palette-1440x900.png" });
  await page.keyboard.press("Escape");
  await expect(editor).toBeFocused();

  await page.keyboard.press("Control+k");
  const search = page.getByRole("searchbox", { name: "Search commands" });
  await search.fill("focus relation");
  await dialog.getByRole("button", { name: /Focus relation view/ }).click();
  await expect(page.locator(".canvas-panel")).toBeFocused();

  await page.getByRole("button", { name: /rate Parameter/ }).click();
  await page.getByRole("textbox", { name: "Value", exact: true }).fill("1.1");
  await expect(page.getByRole("button", { name: "Commit revision" })).toBeEnabled();
  await page.keyboard.press("Control+k");
  await search.fill("commit accepted");
  await dialog.getByRole("button", { name: /Commit accepted value edit/ }).click();
  await expect(page.getByText("Revision 2", { exact: true })).toBeVisible();

  await page.keyboard.press("Control+k");
  await search.fill("accepted plan");
  await dialog.getByRole("button", { name: /Run accepted plan/ }).click();
  await expect(page.getByText("41 samples", { exact: true })).toBeVisible();
});

test("uses one workflow registry for navigation, toolbar, palette, and focus", async ({ page }) => {
  await page.goto("/");

  const geometryNavigation = page.getByRole("button", { name: "Geometry", exact: true });
  await expect(geometryNavigation).toBeDisabled();
  await expect(geometryNavigation).toHaveAttribute(
    "title",
    "This canonical revision has no accepted bounded CAD plan.",
  );

  await page.keyboard.press("Control+k");
  const dialog = page.getByRole("dialog", { name: "Commands" });
  const search = page.getByRole("searchbox", { name: "Search commands" });
  await search.fill("show geometry");
  const geometryCommand = dialog.getByRole("button", {
    name: /Show geometry workspace/,
  });
  await expect(geometryCommand).toBeDisabled();
  await expect(geometryCommand).toContainText(
    "This canonical revision has no accepted bounded CAD plan.",
  );

  await search.fill("open CAD example");
  await dialog.getByRole("button", { name: /Open CAD example/ }).click();
  await expect(page.getByRole("heading", { name: "Semantic geometry" })).toBeVisible();
  await expect(geometryNavigation).toBeEnabled();

  await page.keyboard.press("Control+k");
  await search.fill("focus relation");
  const hiddenRelationFocus = dialog.getByRole("button", { name: /Focus relation view/ });
  await expect(hiddenRelationFocus).toBeDisabled();
  await expect(hiddenRelationFocus).toContainText(
    "This operation is not available in the active workflow.",
  );

  await search.fill("show relations");
  await dialog.getByRole("button", { name: /Show relations workspace/ }).click();
  await expect(page.getByRole("heading", { name: "Relation view" })).toBeVisible();
  await expect(page.locator(".canvas-panel")).toBeFocused();
  await expectNoSeriousOrCriticalViolations(page);
});

test("validates editable run input without throwing or dispatching invalid work", async ({
  page,
}) => {
  const pageErrors: string[] = [];
  page.on("pageerror", (error) => pageErrors.push(error.message));
  await page.goto("/");

  const maxStep = page.getByRole("textbox", { name: /Max step/ });
  const run = page.getByRole("button", { name: "Run accepted plan" });
  await maxStep.fill("");
  await expect(maxStep).toHaveAttribute("aria-invalid", "true");
  await expect(page.getByText("Enter a positive, finite maximum step.")).toBeVisible();
  await expect(run).toBeDisabled();

  await maxStep.fill("1e-7");
  await expect(page.getByText(/at most 5,000,000 integration steps/)).toBeVisible();
  await expect(run).toBeDisabled();
  expect(pageErrors).toEqual([]);
});

test("never invents browser compiler diagnostics or source spans", async ({ page }) => {
  await page.goto("/");
  const editor = page.getByRole("textbox", { name: "Eqiora model source" });
  await editor.fill("field ;");
  await page.getByRole("button", { name: "Compile model" }).click();

  await expect(page.getByText("STPREVIEW", { exact: true })).toBeVisible();
  await expect(page.getByText(/Browser preview cannot compile source/)).toBeVisible();
  await expect(page.getByRole("button", { name: /Go to untitled\.eqi/ })).toHaveCount(0);
});

test("has no serious or critical automated WCAG 2.2 violations in primary states", async ({
  page,
}) => {
  await page.goto("/");
  await expect(page.getByRole("article")).toHaveCount(4);
  await expectNoSeriousOrCriticalViolations(page);

  const maxStep = page.getByRole("textbox", { name: /Max step/ });
  await maxStep.fill("");
  await expect(page.getByText("Enter a positive, finite maximum step.")).toBeVisible();
  await expectNoSeriousOrCriticalViolations(page);

  const editor = page.getByRole("textbox", { name: "Eqiora model source" });
  await editor.fill("field ;");
  await page.getByRole("button", { name: "Compile model" }).click();
  await expect(page.getByText("STPREVIEW", { exact: true })).toBeVisible();
  await expectNoSeriousOrCriticalViolations(page);
});

test("resolves, verifies, and explicitly opens a bounded spatial field", async ({ page }) => {
  await page.goto("/");
  await executePaletteCommand(page, "open spatial example", /Open spatial example/);

  await expect(page.getByRole("heading", { name: "Scalar elliptic solve" })).toBeVisible();
  await expect(page.getByLabel("Lowered model requirements")).toContainText("2D");
  const method = page.getByRole("combobox", { name: /Discretization/ });
  await method.selectOption("finite-volume");
  await page.getByRole("textbox", { name: /Workers/ }).fill("2");
  await expect(page.getByText("Plan accepted", { exact: true })).toBeVisible();
  await expect(page.getByText("256 cells", { exact: true })).toBeVisible();
  await expect(page.getByText("256 values", { exact: true })).toBeVisible();

  const run = page.getByRole("button", { name: "Assemble, solve, verify" });
  await run.click();
  await expect(page.getByRole("progressbar", { name: "Spatial solve in progress" })).toBeVisible();
  const evidence = page.getByLabel("Solution evidence");
  await expect(evidence.getByText("Verified", { exact: true })).toBeVisible();
  await expect(evidence.getByRole("heading", { name: "Solution evidence" })).toBeVisible();
  await expect(page.getByText("independent true residual", { exact: true })).toBeVisible();
  await expect(page.getByText("summary-only control response")).toBeVisible();
  await expect(page.getByRole("heading", { name: "Field viewport" })).toHaveCount(0);

  await evidence.getByRole("button", { name: "View field" }).click();
  await expect(page.getByRole("heading", { name: "Field viewport" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Field values" })).toBeVisible();
  await expect(page.getByText("Explicit owned copy", { exact: true })).toBeVisible();
  await expect(page.getByText(/256 f64 values/)).toBeVisible();
  const fieldTable = page.getByRole("table", {
    name: /solution, cell centre values in canonical last-axis-fastest order/i,
  });
  await expect(fieldTable.getByRole("row")).toHaveCount(51);
  await fieldTable.getByRole("button", { name: "1", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Cell 0, 1" })).toBeVisible();
  const cursor = page.getByRole("button", {
    name: /Use arrow keys to select an exact neighbouring entity/,
  });
  await cursor.focus();
  await page.keyboard.press("ArrowRight");
  await expect(page.getByRole("heading", { name: "Cell 1, 1" })).toBeVisible();
  await expectNoSeriousOrCriticalViolations(page);

  const overflow = await page.evaluate(() => ({
    horizontal: document.documentElement.scrollWidth - innerWidth,
    vertical: document.documentElement.scrollHeight - innerHeight,
  }));
  expect(overflow.horizontal).toBeLessThanOrEqual(0);
  expect(overflow.vertical).toBeLessThanOrEqual(0);
  await page.screenshot({ path: "test-results/studio-scalar-field-1440x900.png" });
});
