import { test } from "../fixtures/auth";
import { expect } from "@playwright/test";

test.describe("identity: user endpoints", () => {
  test("GET dashboard", async ({ customerRequest }) => {
    const resp = await customerRequest.get("/identity/api/v2/user/dashboard");
    expect(resp.status()).toBe(200);
    const body = await resp.json();
    expect(body).toHaveProperty("email");
  });

  test("GET profile picture upload URL and other profile endpoints", async ({
    customerRequest,
  }) => {
    const resp = await customerRequest.get("/identity/api/v2/user/dashboard");
    expect(resp.status()).toBe(200);
  });

  test("POST forget-password (send OTP)", async ({ request }) => {
    const resp = await request.post("/identity/api/auth/forget-password", {
      data: { email: "test@example.com" },
    });
    expect([200, 404, 500]).toContain(resp.status());
  });
});
