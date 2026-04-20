import { defineConfig, devices } from "@playwright/test";

const chromiumPath = process.env.PLAYWRIGHT_CHROMIUM_PATH || undefined;

export default defineConfig({
  testDir: "./scenarios",
  fullyParallel: false,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: 1,
  reporter: "html",
  use: {
    ignoreHTTPSErrors: true,
    viewport: { width: 1280, height: 720 },
    trace: "on-first-retry",
    screenshot: "only-on-failure",
  },
  projects: [
    {
      name: "browser",
      testMatch: /0[01]-.*\.spec\.ts/,
      use: {
        ...devices["Desktop Chrome"],
        baseURL: "http://crapi-web:80",
        proxy: { server: "http://mitmproxy:8080" },
        launchOptions: { executablePath: chromiumPath },
      },
    },
    {
      name: "api",
      testMatch: /0[2-5]-.*\.spec\.ts/,
      use: {
        baseURL: "http://crapi-web:80",
        proxy: { server: "http://mitmproxy:8080" },
      },
    },
  ],
});
