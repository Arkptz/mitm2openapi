import { defineConfig } from "@playwright/test";

const chromiumPath = process.env.PLAYWRIGHT_CHROMIUM_PATH || undefined;

export default defineConfig({
  testDir: ".",
  testMatch: /phase[13]\.spec\.ts/,
  fullyParallel: false,
  workers: 1,
  retries: 0,
  reporter: "list",
  use: {
    ignoreHTTPSErrors: true,
    viewport: { width: 1280, height: 720 },
    video: {
      mode: "on",
      size: { width: 1280, height: 720 },
    },
    launchOptions: { executablePath: chromiumPath, slowMo: 400 },
  },
  outputDir: "./test-results",
});
