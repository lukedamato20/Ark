import { defineConfig, devices } from "@playwright/test";

const viewports = [
  { name: "phone", width: 390, height: 844 },
  { name: "tablet", width: 768, height: 1024 },
  { name: "compact-desktop", width: 980, height: 720 },
  { name: "desktop", width: 1280, height: 720 },
] as const;

export default defineConfig({
  testDir: "./tests/e2e",
  outputDir: ".artifacts/playwright",
  snapshotDir: "./tests/e2e/__snapshots__",
  fullyParallel: true,
  forbidOnly: Boolean(process.env.CI),
  retries: process.env.CI ? 1 : 0,
  reporter: process.env.CI ? "github" : "list",
  expect: { toHaveScreenshot: { animations: "disabled", caret: "hide" } },
  use: {
    baseURL: "http://127.0.0.1:1420",
    colorScheme: "dark",
    reducedMotion: "reduce",
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
  },
  projects: viewports.flatMap(({ name, width, height }) =>
    (["light", "dark"] as const).map((colorScheme) => ({
      name: `${name}-${colorScheme}`,
      use: { ...devices["Desktop Chrome"], viewport: { width, height }, colorScheme, reducedMotion: "reduce" },
    })),
  ),
  webServer: {
    command: "pnpm dev",
    url: "http://127.0.0.1:1420",
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
  },
});
