import { test as base, expect, type APIRequestContext } from "@playwright/test";

const LOGIN_URL = "http://crapi-web:80/identity/api/auth/login";

const ADMIN_EMAIL = "admin@example.com";
const ADMIN_PASSWORD = "Admin!123";
const CUSTOMER_EMAIL = "test@example.com";
const CUSTOMER_PASSWORD = "Test!123";
const CUSTOMER_ADAM_EMAIL = "adam007@example.com";
const CUSTOMER_ADAM_PASSWORD = "adam007!123";

async function loginAndGetToken(
  request: APIRequestContext,
  email: string,
  password: string,
): Promise<string> {
  const response = await request.post(LOGIN_URL, {
    data: { email, password },
  });
  if (!response.ok()) {
    throw new Error(
      `Login failed for ${email}: HTTP ${response.status()} ${await response.text()}`,
    );
  }
  const body = await response.json();
  if (!body.token) {
    throw new Error(
      `No token in login response for ${email}: ${JSON.stringify(body)}`,
    );
  }
  return body.token as string;
}

type AuthFixtures = {
  adminToken: string;
  customerToken: string;
  customerTokenAdam: string;
  adminRequest: APIRequestContext;
  customerRequest: APIRequestContext;
};

export const test = base.extend<AuthFixtures>({
  adminToken: async ({ request }, use) => {
    const token = await loginAndGetToken(request, ADMIN_EMAIL, ADMIN_PASSWORD);
    await use(token);
  },

  customerToken: async ({ request }, use) => {
    const token = await loginAndGetToken(
      request,
      CUSTOMER_EMAIL,
      CUSTOMER_PASSWORD,
    );
    await use(token);
  },

  customerTokenAdam: async ({ request }, use) => {
    const token = await loginAndGetToken(
      request,
      CUSTOMER_ADAM_EMAIL,
      CUSTOMER_ADAM_PASSWORD,
    );
    await use(token);
  },

  adminRequest: async ({ playwright, adminToken }, use) => {
    const ctx = await playwright.request.newContext({
      extraHTTPHeaders: {
        Authorization: `Bearer ${adminToken}`,
      },
      proxy: { server: "http://mitmproxy:8080" },
      ignoreHTTPSErrors: true,
    });
    await use(ctx);
    await ctx.dispose();
  },

  customerRequest: async ({ playwright, customerToken }, use) => {
    const ctx = await playwright.request.newContext({
      extraHTTPHeaders: {
        Authorization: `Bearer ${customerToken}`,
      },
      proxy: { server: "http://mitmproxy:8080" },
      ignoreHTTPSErrors: true,
    });
    await use(ctx);
    await ctx.dispose();
  },
});

export { expect };
