import { test, expect } from '@playwright/test';
import { existsSync, statSync } from 'fs';
import path from 'path';

test('smoke: homepage loads through mitmproxy and capture is non-empty', async ({ page }) => {
  // Navigate to Toolshop homepage through the proxy
  await page.goto('/');

  // Verify the page loaded
  await expect(page).toHaveTitle(/Practice Software Testing/i);

  // Verify some content is visible
  await expect(page.locator('app-root')).toBeVisible();

  // Check that the mitmproxy capture file exists and is non-empty
  const flowPath = path.resolve(__dirname, '..', 'out', 'toolshop.flow');

  // Give mitmproxy a moment to flush
  await page.waitForTimeout(2000);

  // The flow file should exist and be non-empty if traffic went through the proxy
  expect(existsSync(flowPath)).toBe(true);
  const stats = statSync(flowPath);
  expect(stats.size).toBeGreaterThan(0);
});
