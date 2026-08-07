import { defineConfig, devices } from "@playwright/test";

function exactLoopbackUrl(environmentName: string, fallback: string): string {
  const value = process.env[environmentName] ?? fallback;
  const url = new URL(value);
  if (url.protocol !== "http:" || url.hostname !== "127.0.0.1") {
    throw new Error(`${environmentName} must be an exact 127.0.0.1 HTTP URL`);
  }
  return url.href;
}

const jupyterlabUrl = exactLoopbackUrl(
  "EQIORA_JUPYTERLAB_URL",
  "http://127.0.0.1:18888/lab/tree/bindings/python/tests/fixtures/rich_mesh_display/jupyterlab.ipynb",
);
const marimoUrl = exactLoopbackUrl("EQIORA_MARIMO_URL", "http://127.0.0.1:18889/");

export default defineConfig({
  testDir: "./tests",
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
      name: "jupyterlab-4.6.2",
      use: { baseURL: jupyterlabUrl },
    },
    {
      name: "marimo-0.23.16",
      use: { baseURL: marimoUrl },
    },
  ],
});
