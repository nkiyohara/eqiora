import { expect, test } from "@playwright/test";

test("shared Three.js viewer composes layers and disposes its browser resources", async ({
	page,
}) => {
	await page.goto("/tests/viewer.html");
	const viewer = page.getByRole("region", { name: "Eqiora semantic scene viewer" });
	await expect(viewer).toBeVisible();
	await expect(
		page.getByRole("img", { name: /Interactive read-only Eqiora scene/ }),
	).toBeVisible();
	await page.getByRole("button", { name: "Orbit" }).click();
	await page.getByRole("button", { name: "Pan" }).click();
	await page.getByRole("button", { name: "Zoom in" }).click();
	await page.getByRole("button", { name: "Reset" }).click();

	await page
		.getByRole("combobox", { name: "Selection" })
		.selectOption("selection:mesh:m:left");
	await page.getByRole("textbox", { name: "Selection colour" }).fill("#00aa55");
	await page.getByRole("checkbox", { name: "Isolate exact selection" }).check();
	await expect
		.poll(() =>
			page
				.locator("canvas")
				.evaluate((canvas) => (canvas as HTMLCanvasElement).toDataURL().length),
		)
		.toBeGreaterThan(100);
	await page.getByRole("checkbox", { name: "Isolate exact selection" }).uncheck();
	await page
		.getByRole("combobox", { name: "Scalar field" })
		.selectOption("scalar-field:cell");
	await expect(page.getByText(/temperature: 10 to 20/)).toBeVisible();

	const canvas = page.getByRole("img", { name: /Interactive read-only Eqiora scene/ });
	const bounds = await canvas.boundingBox();
	expect(bounds).not.toBeNull();
	await page.mouse.click(
		(bounds?.x ?? 0) + (bounds?.width ?? 0) / 2,
		(bounds?.y ?? 0) + (bounds?.height ?? 0) / 2,
	);
	await expect(page.getByText(/Accepted cell [01]: (10|20) coherent-si/)).toBeVisible();

	await page
		.getByRole("combobox", { name: "Scalar field" })
		.selectOption("scalar-field:vertex");
	await page.mouse.click(
		(bounds?.x ?? 0) + (bounds?.width ?? 0) / 2,
		(bounds?.y ?? 0) + (bounds?.height ?? 0) / 2,
	);
	await expect(
		page.getByText(/Accepted vertex [0-3]: [0-3] coherent-si/),
	).toBeVisible();

	await page.getByRole("button", { name: "Dispose viewer" }).click();
	await expect(viewer).toHaveCount(0);
});
