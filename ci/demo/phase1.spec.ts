import { test } from '@playwright/test';
import { installMouseHelper } from '../../tests/integration/level2/fixtures/mouse-helper';
import path from 'path';

test.use({
  launchOptions: {
    slowMo: 400,
  },
  viewport: { width: 1280, height: 720 },
  video: {
    mode: 'on',
    size: { width: 1280, height: 720 },
  },
  baseURL: 'http://localhost:4200',
  proxy: {
    server: 'http://localhost:8080',
  },
  ignoreHTTPSErrors: true,
});

test('Phase 1: Capture traffic with mitmproxy', async ({ page, context }) => {
  // Install mouse-helper for visible cursor + click ripples
  await context.addInitScript(installMouseHelper());

  // 1. Browse: open homepage, click a category, browse products
  await page.goto('/');
  await page.waitForLoadState('networkidle');
  await page.waitForTimeout(1000);

  // Click on a category in the sidebar/nav
  const categoryLink = page.locator('a[data-test="nav-categories"]').first();
  if (await categoryLink.isVisible()) {
    await categoryLink.click();
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(500);
  }

  // Browse product list
  const firstProduct = page.locator('[data-test="product-name"]').first();
  if (await firstProduct.isVisible()) {
    await firstProduct.click();
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(500);
  }

  // 2. Add to cart
  const addToCartBtn = page.locator('[data-test="add-to-cart"]');
  if (await addToCartBtn.isVisible()) {
    await addToCartBtn.click();
    await page.waitForTimeout(500);
  }

  // 3. Navigate to cart
  const cartLink = page.locator('[data-test="nav-cart"]');
  if (await cartLink.isVisible()) {
    await cartLink.click();
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(500);
  }

  // 4. Open sign-in page
  const signInLink = page.locator('[data-test="nav-sign-in"]');
  if (await signInLink.isVisible()) {
    await signInLink.click();
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(500);

    // Type credentials (visually demonstrates login flow)
    const emailField = page.locator('[data-test="email"]');
    const passwordField = page.locator('[data-test="password"]');
    if (await emailField.isVisible()) {
      await emailField.fill('customer@practicesoftwaretesting.com');
      await page.waitForTimeout(300);
      await passwordField.fill('welcome01');
      await page.waitForTimeout(300);

      const loginBtn = page.locator('[data-test="login-submit"]');
      if (await loginBtn.isVisible()) {
        await loginBtn.click();
        await page.waitForLoadState('networkidle');
        await page.waitForTimeout(500);
      }
    }
  }

  // Final pause for visual completeness
  await page.waitForTimeout(1000);

  // Move the recorded video to out/phase1.webm
  // (Playwright saves video automatically, we rename in afterAll or CI script)
});
