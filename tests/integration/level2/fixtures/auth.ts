import { test as base, expect, type Page } from '@playwright/test';
import path from 'path';

const ADMIN_EMAIL = 'admin@practicesoftwaretesting.com';
const ADMIN_PASSWORD = 'welcome01';
const CUSTOMER_EMAIL = 'customer@practicesoftwaretesting.com';
const CUSTOMER_PASSWORD = 'welcome01';

type AuthFixture = {
  adminPage: Page;
  customerPage: Page;
};

async function loginViaApi(
  baseURL: string,
  email: string,
  password: string,
): Promise<{ token: string }> {
  const response = await fetch(`${baseURL}/users/login`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ email, password }),
  });
  if (!response.ok) {
    throw new Error(`Login failed for ${email}: ${response.status}`);
  }
  const data = await response.json();
  return { token: data.access_token };
}

export const test = base.extend<AuthFixture>({
  adminPage: async ({ browser }, use) => {
    const context = await browser.newContext({
      storageState: undefined,
    });
    const page = await context.newPage();
    const { token } = await loginViaApi(
      'http://localhost:8091',
      ADMIN_EMAIL,
      ADMIN_PASSWORD,
    );
    await context.addInitScript((t) => {
      localStorage.setItem('auth-token', t);
    }, token);
    await use(page);
    await context.close();
  },
  customerPage: async ({ browser }, use) => {
    const context = await browser.newContext({
      storageState: undefined,
    });
    const page = await context.newPage();
    const { token } = await loginViaApi(
      'http://localhost:8091',
      CUSTOMER_EMAIL,
      CUSTOMER_PASSWORD,
    );
    await context.addInitScript((t) => {
      localStorage.setItem('auth-token', t);
    }, token);
    await use(page);
    await context.close();
  },
});

export { expect };
