import { test } from "../fixtures/auth";
import { expect } from "@playwright/test";
import { existsSync, statSync } from "fs";
import path from "path";

test("smoke: login and dashboard + flow capture verification", async ({
  customerRequest,
}) => {
  const resp = await customerRequest.get("/identity/api/v2/user/dashboard");
  expect(resp.ok()).toBe(true);
  const body = await resp.json();
  expect(body).toHaveProperty("email");
  expect(body.email).toBe("test@example.com");

  // Give mitmproxy time to flush
  await new Promise((r) => setTimeout(r, 2000));

  const flowPath = path.resolve(__dirname, "..", "out", "crapi.flow");
  expect(existsSync(flowPath)).toBe(true);
  const stats = statSync(flowPath);
  expect(stats.size).toBeGreaterThan(0);
});
