import { defineConfig, devices } from "@playwright/test";

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
  // Playwright's 30 s default test timeout truncates this suite's own waits: the
  // JupyterLab branch of prepareHost declares 60 s + 60 s + 60 s of kernel waiting
  // before a test body starts. This value is a hang backstop, not a measured
  // worst case; the per-assertion timeouts stay the binding bound.
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
    reducedMotion: "reduce",
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
