import { test, expect } from "@playwright/test";

test("ui journey: api requests through browser project proxy", async ({
  request,
}) => {
  const loginResp = await request.post("/identity/api/auth/login", {
    data: { email: "test@example.com", password: "Test!123" },
  });
  expect(loginResp.ok()).toBe(true);
  const { token } = await loginResp.json();

  const dashResp = await request.get("/identity/api/v2/user/dashboard", {
    headers: { Authorization: `Bearer ${token}` },
  });
  expect(dashResp.ok()).toBe(true);

  const shopResp = await request.get("/workshop/api/shop/products", {
    headers: { Authorization: `Bearer ${token}` },
  });
  expect(shopResp.ok()).toBe(true);
});
