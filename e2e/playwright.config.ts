import { defineConfig, devices } from "@playwright/test";

// End-to-end tests against the built site (`vite preview` of apps/demo, foundation Requirement 16).
// Browsers are installed with `pnpm --filter e2e exec playwright install chromium webkit firefox`.
const PORT = 4173;
const BASE = `http://127.0.0.1:${String(PORT)}/dormbit/`;
const ci = process.env.CI === "true" || process.env.CI === "1";

export default defineConfig({
  testDir: ".",
  testMatch: "**/*.spec.ts",
  outputDir: "../.scratch/playwright/results",
  fullyParallel: true,
  forbidOnly: ci,
  retries: ci ? 1 : 0,
  reporter: [["list"]],
  use: { baseURL: BASE, trace: "retain-on-failure" },
  projects: [
    { name: "chromium", use: { ...devices["Desktop Chrome"] } },
    { name: "webkit", use: { ...devices["Desktop Safari"] } },
    { name: "firefox", use: { ...devices["Desktop Firefox"] } },
    { name: "mobile-webkit", use: { ...devices["iPhone 13"] } },
  ],
  webServer: {
    command: `pnpm --filter @dormbit/demo build && pnpm --filter @dormbit/demo exec vite preview --host 127.0.0.1 --port ${String(PORT)} --strictPort`,
    cwd: "..",
    url: BASE,
    reuseExistingServer: !ci,
    timeout: 180_000,
  },
});
