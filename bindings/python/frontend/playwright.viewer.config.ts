import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
	testDir: "./tests",
	testMatch: "viewer-spike.spec.ts",
	timeout: 60_000,
	fullyParallel: false,
	forbidOnly: true,
	retries: 0,
	workers: 1,
	reporter: "line",
	use: {
		...devices["Desktop Chrome"],
		browserName: "chromium",
		baseURL: "http://127.0.0.1:18925",
		viewport: { width: 1280, height: 800 },
	},
	webServer: {
		command: "npm run dev:viewer",
		url: "http://127.0.0.1:18925/tests/viewer.html",
		reuseExistingServer: false,
		timeout: 30_000,
	},
});
