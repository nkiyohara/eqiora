import { expect, type Page, test } from "@playwright/test";

const ENABLED = process.env.EQIORA_EXACT_CYLINDER_STOKES_JUPYTER_ORACLE === "1";

class RuntimeTraffic {
	readonly external: string[] = [];

	constructor(page: Page) {
		const observe = (raw: string): void => {
			const url = new URL(raw);
			if (url.protocol === "data:" || url.protocol === "blob:") return;
			if (url.hostname !== "127.0.0.1") this.external.push(raw);
		};
		page.on("request", (request) => observe(request.url()));
		page.on("websocket", (socket) => observe(socket.url()));
	}
}

test.describe("exact-cylinder steady-Stokes Jupyter composition", () => {
	test.skip(!ENABLED, "the exact notebook has its own clean-candidate host launch");

	test("shows live typed ownership and one caller-owned pressure Figure", async ({
		page,
	}) => {
		const traffic = new RuntimeTraffic(page);
		await page.goto("");
		await expect(page.locator(".jp-Notebook")).toBeVisible({ timeout: 60_000 });

		await page.locator(".jp-Notebook .jp-Cell").first().click();
		await page.locator(".lm-MenuBar-item").filter({ hasText: /^Run$/ }).click();
		await page
			.locator(".lm-Menu-item")
			.filter({ hasText: /^Run All Cells$/ })
			.click();

		await expect(
			page.getByText("EQIORA_EXACT_CYLINDER_STOKES_JUPYTER_READY", {
				exact: true,
			}),
		).toBeVisible({ timeout: 120_000 });

		const identity = async (testId: string, typeName: string): Promise<string> => {
			const row = page.getByTestId(testId);
			await expect(row).toBeVisible();
			const text = (await row.textContent()) ?? "";
			expect(text).toContain(typeName);
			return text;
		};

		await identity("eqiora-stokes-geometry", "Geometry");
		await identity("eqiora-stokes-mesh-plan", "MeshPlan");
		await identity("eqiora-stokes-mesh", "Mesh");
		await identity("eqiora-stokes-model", "Model");
		await identity("eqiora-stokes-plan", "SteadyStokesPlan");
		const run = await identity("eqiora-stokes-run", "Run");
		const result = await identity("eqiora-stokes-result", "Result");
		const evidence = await identity("eqiora-stokes-evidence", "SteadyStokesEvidence");

		const runDigest = run.match(/[0-9a-f]{64}/)?.[0];
		const resultDigest = result.match(/[0-9a-f]{64}/)?.[0];
		expect(runDigest).toBeDefined();
		expect(resultDigest).toBe(runDigest);
		expect(evidence).toMatch(/pressure.+Pa/i);
		expect(evidence).toMatch(/force.+N\/m/i);
		expect(evidence).toMatch(/flux.+m(?:\^2|²)\/s/i);

		const pressureImage = page.locator(
			'.jp-Notebook .jp-CodeCell .jp-OutputArea-output img[src^="data:image/png"]',
		);
		await expect(pressureImage).toHaveCount(1);
		await expect(pressureImage).toBeVisible();
		const dimensions = await pressureImage.evaluate((image) => {
			if (!(image instanceof HTMLImageElement)) return [0, 0];
			return [image.naturalWidth, image.naturalHeight];
		});
		expect(dimensions[0]).toBeGreaterThan(0);
		expect(dimensions[1]).toBeGreaterThan(0);
		await expect(
			page.locator(
				'.jp-Notebook .jp-RenderedText[data-mime-type="application/vnd.jupyter.stderr"]',
			),
		).toHaveCount(0);
		expect(traffic.external).toEqual([]);
	});
});
