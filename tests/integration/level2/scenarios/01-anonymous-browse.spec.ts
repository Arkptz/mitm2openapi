import { test, expect } from '@playwright/test';

test.describe('Scenario 1: Anonymous browse', () => {
  test('browse products with pagination, filters, and categories', async ({ page }) => {
    // Navigate to homepage
    await page.goto('/');
    await expect(page.locator('app-root')).toBeVisible();

    // Browse products — pagination
    await page.goto('/#/category/hand-tools?page=1');
    await page.waitForLoadState('networkidle');

    await page.goto('/#/category/hand-tools?page=2');
    await page.waitForLoadState('networkidle');

    // Filter by category
    await page.goto('/#/?by_category=1');
    await page.waitForLoadState('networkidle');

    // Filter by price range
    await page.goto('/#/?between=price,1,100');
    await page.waitForLoadState('networkidle');

    // Sort products
    await page.goto('/#/?sort=name,asc');
    await page.waitForLoadState('networkidle');

    // Get categories tree
    const categoriesResponse = await page.request.get('http://localhost:8091/categories/tree');
    expect(categoriesResponse.ok()).toBe(true);
    const categories = await categoriesResponse.json();
    expect(Array.isArray(categories)).toBe(true);

    // Browse individual product
    await page.goto('/');
    await page.waitForLoadState('networkidle');
    const firstProduct = page.locator('[data-test="product-name"]').first();
    if (await firstProduct.isVisible()) {
      await firstProduct.click();
      await page.waitForLoadState('networkidle');
    }

    // Verify we've generated meaningful traffic
    // (actual assertion happens in the orchestrator via mitm2openapi discover)
  });
});
