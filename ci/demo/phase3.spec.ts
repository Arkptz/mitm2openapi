// Phase 3 runs ON THE HOST (not in a sidecar): Swagger UI is a sibling container
// on the default bridge network, reached via host-mapped port 8088. No mitmproxy needed.
import { test, expect } from "@playwright/test";
import { installMouseHelper } from "../../tests/integration/level2/fixtures/mouse-helper";
import { execSync } from "child_process";

const chromiumPath = process.env.PLAYWRIGHT_CHROMIUM_PATH || undefined;

test.use({
  launchOptions: {
    executablePath: chromiumPath,
    slowMo: 400,
  },
  viewport: { width: 1280, height: 720 },
  video: {
    mode: "on",
    size: { width: 1280, height: 720 },
  },
  ignoreHTTPSErrors: true,
});

test.beforeAll(() => {
  // Start Swagger UI container with generated spec
  try {
    execSync("docker rm -f swagger-ui 2>/dev/null || true");
    execSync(
      "docker run -d --name swagger-ui -p 8088:8080 " +
        "-e SWAGGER_JSON=/spec/openapi.yaml " +
        "-v ${PWD}/out:/spec " +
        "swaggerapi/swagger-ui",
      { stdio: "inherit" },
    );

    // Wait for Swagger UI to be ready
    let ready = false;
    for (let i = 0; i < 30; i++) {
      try {
        execSync("curl -sf http://localhost:8088 > /dev/null", {
          stdio: "ignore",
        });
        ready = true;
        break;
      } catch {
        execSync("sleep 1");
      }
    }
    if (!ready) {
      throw new Error("Swagger UI did not become ready in 30s");
    }
  } catch (e) {
    console.error("Failed to start Swagger UI:", e);
    throw e;
  }
});

test.afterAll(() => {
  try {
    execSync("docker rm -f swagger-ui");
  } catch {
    // ignore cleanup errors
  }
});

test("Phase 3: Browse the spec in Swagger UI", async ({ page, context }) => {
  // Install mouse-helper for visible cursor
  await context.addInitScript(installMouseHelper());

  // Navigate to Swagger UI
  await page.goto("http://localhost:8088");
  await page.waitForLoadState("networkidle");
  await page.waitForTimeout(1000);

  // Expand the first API section
  const firstSection = page.locator(".opblock-tag-section").first();
  if (await firstSection.isVisible()) {
    const sectionHeader = firstSection.locator(".opblock-tag");
    await sectionHeader.click();
    await page.waitForTimeout(500);
  }

  // Expand 2-3 endpoints
  const endpoints = page.locator(".opblock-summary");
  const endpointCount = Math.min(await endpoints.count(), 3);
  for (let i = 0; i < endpointCount; i++) {
    await endpoints.nth(i).click();
    await page.waitForTimeout(400);
  }

  // Scroll down to show more content
  await page.evaluate(() => window.scrollBy(0, 300));
  await page.waitForTimeout(500);

  // Scroll a bit more
  await page.evaluate(() => window.scrollBy(0, 200));
  await page.waitForTimeout(1000);

  // Output goes to out/phase3.webm (Playwright video recording)
});
