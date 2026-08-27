import { defineConfig, devices } from "@playwright/test";

declare const process: { env: Record<string, string | undefined> };

function exactLoopbackUrl(environmentName: string, fallback: string): string {
	const value = process.env[environmentName] ?? fallback;
	const url = new URL(value);
	if (url.protocol !== "http:" || url.hostname !== "127.0.0.1") {
		throw new Error(`${environmentName} must be an exact 127.0.0.1 HTTP URL`);
	}
	return url.href;
}

const marimoUrl = exactLoopbackUrl("EQIORA_MARIMO_URL", "http://127.0.0.1:18889/");

export default defineConfig({
	testDir: "./tests",
	// The exact-cylinder Marimo host starts a native solve before its assertions.
	// This is a hang backstop; per-assertion timeouts remain the binding bounds.
	timeout: 600_000,
	fullyParallel: false,
	forbidOnly: true,
	retries: 0,
	workers: 1,
	reporter: "line",
	outputDir: "./test-results",
	use: {
		...devices["Desktop Chrome"],
		browserName: "chromium",
		viewport: { width: 1440, height: 900 },
		colorScheme: "light",
		trace: "retain-on-failure",
		screenshot: "only-on-failure",
	},
	projects: [
		{
			name: "marimo-0.23.16",
			use: { baseURL: marimoUrl },
		},
	],
});
