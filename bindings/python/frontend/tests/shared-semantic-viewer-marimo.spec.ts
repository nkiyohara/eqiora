import { expect, type Page, test } from "@playwright/test";

declare const process: { env: Record<string, string | undefined> };

const ENABLED = process.env.EQIORA_SHARED_SEMANTIC_VIEWER_MARIMO_ORACLE === "1";

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

test.describe("installed-wheel shared semantic viewer in Marimo", () => {
	test.skip(!ENABLED, "the shared viewer app has its own clean-candidate host launch");

	test("composes accepted Geometry and Mesh through the shipped anywidget assets", async ({
		page,
	}) => {
		const traffic = new RuntimeTraffic(page);
		await page.goto("");

		await expect(
			page.getByRole("heading", { name: "Shared semantic viewer", exact: true }),
		).toBeVisible({ timeout: 120_000 });
		await expect(
			page.getByText("EQIORA_SHARED_SEMANTIC_VIEWER_READY", { exact: true }),
		).toBeVisible({ timeout: 120_000 });
		await expect(page.getByTestId("eqiora-viewer-geometry")).toContainText("Geometry");
		await expect(page.getByTestId("eqiora-viewer-mesh")).toContainText(
			"20 accepted vertices; 12 accepted cells",
		);
		await expect(page.getByTestId("eqiora-viewer-python-host")).toContainText("View");

		const viewer = page.getByRole("region", {
			name: "Eqiora semantic scene viewer",
		});
		await expect(viewer).toBeVisible({ timeout: 120_000 });
		await expect(viewer).toHaveAttribute(
			"data-scene-schema",
			"eqiora.viewer.scene/v0-private",
		);
		await viewer.getByRole("button", { name: "Orbit" }).click();
		await viewer.getByRole("button", { name: "Pan" }).click();
		await viewer.getByRole("button", { name: "Zoom in" }).click();
		await viewer.getByRole("button", { name: "Reset" }).click();
		await viewer
			.getByRole("combobox", { name: "Selection" })
			.selectOption({ label: "left · Mesh" });
		await viewer.getByRole("checkbox", { name: "Isolate exact selection" }).check();
		await expect(
			viewer.getByRole("img", { name: /Interactive read-only Eqiora scene/ }),
		).toBeVisible();
		await expect.poll(() => traffic.external).toEqual([]);
	});
});
