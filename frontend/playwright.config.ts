import { defineConfig, devices } from "@playwright/test";

// The suite expects a running stack on a clean database. See README "End-to-end tests".
const baseURL = process.env.E2E_BASE_URL ?? "http://localhost:3000";
// Optional: point at an existing Chromium build (for example in CI images that preinstall one).
const chromiumPath = process.env.PLAYWRIGHT_CHROMIUM_PATH;

export default defineConfig({
  testDir: "./e2e",
  fullyParallel: false,
  workers: 1,
  retries: 0,
  timeout: 60_000,
  reporter: [["list"]],
  use: {
    baseURL,
    trace: "off",
    screenshot: "only-on-failure",
    launchOptions: chromiumPath
      ? { executablePath: chromiumPath, args: ["--no-sandbox"] }
      : {},
  },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"], viewport: { width: 1366, height: 860 } } }],
});
