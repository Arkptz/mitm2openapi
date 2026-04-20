import { test, expect, type Page } from "@playwright/test";
import { installMouseHelper } from "../../tests/integration/level2/fixtures/mouse-helper";

test.use({
  launchOptions: {
    slowMo: 300,
  },
  viewport: { width: 1280, height: 720 },
  video: {
    mode: "on",
    size: { width: 1280, height: 720 },
  },
  baseURL: "http://crapi-web:80",
  proxy: {
    server: "http://mitmproxy:8080",
  },
  ignoreHTTPSErrors: true,
});

const pause = (page: Page, ms = 500) => page.waitForTimeout(ms);

// crAPI uses imperative ant-design Modal.success/Modal.error dialogs:
//   1. clicking "OK" does NOT dismiss the modal (bug in the app)
//   2. pressing Escape hides the visible dialog but leaves `.ant-modal-wrap`
//      in the DOM with z-index 2000, which silently blocks ALL subsequent
//      clicks on the page. Playwright waits forever on actionability checks.
// So we press Escape AND strip the lingering overlay nodes by hand.
const dismissDialog = async (page: Page) => {
  const okVisible = await page
    .getByRole("button", { name: "OK" })
    .isVisible({ timeout: 3000 })
    .catch(() => false);
  if (!okVisible) return;
  await page.keyboard.press("Escape");
  await page.waitForTimeout(300);
  await page.evaluate(() => {
    document
      .querySelectorAll(".ant-modal-wrap, .ant-modal-mask")
      .forEach((el) => el.remove());
  });
  await page.waitForTimeout(200);
};

test("Phase 1: Capture crAPI traffic with mitmproxy", async ({
  page,
  context,
}) => {
  test.setTimeout(300_000);

  await context.addInitScript(installMouseHelper());

  // ── Login ────────────────────────────────────────────────────────────
  await page.goto("/login");
  await page.waitForLoadState("domcontentloaded");
  await pause(page, 800);

  await page.getByRole("textbox", { name: "Email" }).fill("test@example.com");
  await pause(page, 300);
  await page.getByRole("textbox", { name: "Password" }).fill("Test!123");
  await pause(page, 300);
  await page.getByRole("button", { name: "Login" }).last().click();

  await expect(page).toHaveURL(/\/dashboard/, { timeout: 15_000 });
  await page.waitForLoadState("domcontentloaded");
  await pause(page, 800);

  // ── Dashboard → Vehicle Service History ──────────────────────────────
  await expect(page.getByText("Vehicles Details")).toBeVisible();
  await pause(page, 600);

  await page
    .getByRole("button", { name: /Vehicle Service History/i })
    .first()
    .click();
  await page.waitForLoadState("domcontentloaded");
  await pause(page, 1200);

  // ── Contact Mechanic ────────────────────────────────────────────────
  await page.goto("/dashboard");
  await page.waitForLoadState("domcontentloaded");
  await pause(page, 600);

  await page
    .getByRole("button", { name: /Contact Mechanic/i })
    .first()
    .click();
  await page.waitForLoadState("domcontentloaded");
  await pause(page, 700);

  const mechanicCombo = page.getByRole("combobox", {
    name: /Available Mechanics/i,
  });
  await mechanicCombo.click();
  await pause(page, 400);
  await mechanicCombo.press("ArrowDown");
  await pause(page, 200);
  await mechanicCombo.press("Enter");
  await pause(page, 300);

  await page
    .getByRole("textbox", { name: /Describe the Issue/i })
    .fill("Engine making strange noise. Please help diagnose.");
  await pause(page, 500);

  await page
    .getByRole("button", { name: /Send Service Request/i })
    .click({ noWaitAfter: true });
  await dismissDialog(page);
  await pause(page, 600);

  // ── Shop: add coupon → buy → past orders → order details ───────────
  await page.getByRole("menuitem", { name: "Shop" }).click();
  await page.waitForLoadState("domcontentloaded");
  await pause(page, 1000);

  await page.getByRole("button", { name: /Add Coupons/i }).click();
  await pause(page, 500);
  await page.getByRole("textbox", { name: /Coupon Code/i }).fill("TRAC075");
  await pause(page, 300);
  await page
    .getByRole("button", { name: "Validate" })
    .click({ noWaitAfter: true });
  await dismissDialog(page);
  await pause(page, 600);

  const buyButtons = page.getByRole("button", { name: /Buy/i });
  const buyCount = await buyButtons.count();
  if (buyCount > 0) {
    await buyButtons.first().click({ force: true });
    await dismissDialog(page);
    await pause(page, 600);
  }

  await page
    .getByRole("button", { name: /Past Orders/i })
    .click({ timeout: 10_000, noWaitAfter: true });
  await page.waitForURL(/\/past-orders/, { timeout: 15_000 });
  await pause(page, 1200);

  const orderDetails = page.getByRole("button", { name: /Order Details/i });
  const odCount = await orderDetails.count();
  if (odCount > 0) {
    await orderDetails.first().click({ timeout: 10_000, noWaitAfter: true });
    await page.waitForURL(/\/orders/, { timeout: 15_000 });
    await pause(page, 1400);
  }

  // ── Community: open post + add comment ──────────────────────────────
  await page.getByRole("menuitem", { name: "Community" }).click();
  await page.waitForLoadState("domcontentloaded");
  await pause(page, 1200);

  const firstPostTitle = page.getByText(/^Title \d+$/).first();
  const postVisible = await firstPostTitle
    .isVisible({ timeout: 3000 })
    .catch(() => false);
  if (postVisible) {
    await firstPostTitle.click();
    await page.waitForLoadState("domcontentloaded");
    await pause(page, 900);

    await page.getByRole("button", { name: /Add Comment/i }).first().click();
    await pause(page, 500);
    await page
      .getByRole("textbox", { name: /Your Comment/i })
      .fill("Great post! Thanks for sharing.");
    await pause(page, 500);
    await page
      .getByRole("button", { name: /Add Comment/i })
      .last()
      .click({ noWaitAfter: true });
    await dismissDialog(page);
    await pause(page, 600);
  }

  // ── Profile ─────────────────────────────────────────────────────────
  await page.goto("/my-profile");
  await page.waitForLoadState("domcontentloaded");
  await pause(page, 1500);
});
