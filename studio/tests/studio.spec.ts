import AxeBuilder from "@axe-core/playwright";
import { expect, type Page, test } from "@playwright/test";

async function executePaletteCommand(page: Page, query: string, name: RegExp) {
  await page.keyboard.press("Control+k");
  await page.getByRole("searchbox", { name: "Search commands" }).fill(query);
  await page.getByRole("dialog", { name: "Commands" }).getByRole("button", { name }).click();
}
test("projects and inspects the compiler-owned model", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByRole("status")).toContainText(
    "browser preview demonstrates interaction and layout only",
  );
  await expect(page.getByRole("heading", { name: "Relation view" })).toBeVisible();
  await page.getByRole("button", { name: /decay Relation/ }).click();
  await expect(page.getByRole("heading", { name: "decay", exact: true })).toBeVisible();
});
test("keeps only retained examples in the menu", async ({ page }) => {
  await page.goto("/");
  await page.getByText("Examples", { exact: true }).click();
  await expect(page.getByRole("button", { name: /Sampled DC drive/ })).toBeVisible();
  await expect(
    page.getByRole("button", { name: /cylinder|structural|elastic panel/i }),
  ).toHaveCount(0);
});
test("selects exact CAD Domains", async ({ page }) => {
  await page.goto("/");
  await executePaletteCommand(page, "open CAD example", /Open CAD example/);
  await expect(page.getByRole("heading", { name: "Semantic geometry" })).toBeVisible();
});
test("never fabricates packaged DC execution in browser preview", async ({ page }) => {
  await page.goto("/");
  await executePaletteCommand(page, "dc-drive", /Run DC-drive demo/);
  await expect(
    page.getByText(/browser preview does not fabricate scientific results/i),
  ).toBeVisible();
  await expect(page.getByRole("heading", { name: "Sampled DC drive" })).toHaveCount(0);
});
test("has no serious or critical WCAG violations", async ({ page }) => {
  await page.goto("/");
  const results = await new AxeBuilder({ page })
    .withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa"])
    .analyze();
  expect(
    results.violations.filter((item) => item.impact === "serious" || item.impact === "critical"),
  ).toEqual([]);
});
