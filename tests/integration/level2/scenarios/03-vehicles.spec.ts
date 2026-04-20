import { test } from "../fixtures/auth";
import { expect } from "@playwright/test";

test.describe("vehicles: vehicle endpoints", () => {
  let vehicleId: string;

  test("GET vehicles list", async ({ customerRequest }) => {
    const resp = await customerRequest.get("/identity/api/v2/vehicle/vehicles");
    expect(resp.status()).toBe(200);
    const vehicles = await resp.json();
    expect(Array.isArray(vehicles)).toBe(true);
    if (vehicles.length > 0) {
      vehicleId = vehicles[0].uuid || vehicles[0].id;
    }
  });

  test("POST resend vehicle email", async ({ customerRequest }) => {
    const resp = await customerRequest.post(
      "/identity/api/v2/vehicle/resend_email",
    );
    expect([200, 400, 404]).toContain(resp.status());
  });

  test("GET vehicle location (pre-seeded)", async ({
    customerTokenAdam,
    playwright,
  }) => {
    const ctx = await playwright.request.newContext({
      extraHTTPHeaders: { Authorization: `Bearer ${customerTokenAdam}` },
      proxy: { server: "http://mitmproxy:8080" },
      ignoreHTTPSErrors: true,
    });
    try {
      const vehiclesResp = await ctx.get("/identity/api/v2/vehicle/vehicles");
      expect(vehiclesResp.status()).toBe(200);
      const vehicles = await vehiclesResp.json();
      if (vehicles.length > 0) {
        const vid = vehicles[0].uuid || vehicles[0].id;
        const locResp = await ctx.get(
          `/identity/api/v2/vehicle/${vid}/location`,
        );
        expect([200, 403, 404]).toContain(locResp.status());
      }
    } finally {
      await ctx.dispose();
    }
  });
});
