import { test, expect } from '@playwright/test';

test.describe('Scenario 4: Invoice creation', () => {
  let token: string;
  let cartId: string;

  test.beforeAll(async ({ request }) => {
    const loginResponse = await request.post('http://localhost:8091/users/login', {
      data: {
        email: 'customer@practicesoftwaretesting.com',
        password: 'welcome01',
      },
    });
    const loginBody = await loginResponse.json();
    token = loginBody.access_token;

    const productsResponse = await request.get('http://localhost:8091/products?page=1');
    const productsBody = await productsResponse.json();
    const productId = productsBody.data[0].id;

    const cartResponse = await request.post('http://localhost:8091/carts', {
      headers: { Authorization: `Bearer ${token}` },
    });
    cartId = (await cartResponse.json()).id;

    await request.post(`http://localhost:8091/carts/${cartId}`, {
      headers: { Authorization: `Bearer ${token}` },
      data: { product_id: productId, quantity: 1 },
    });
  });

  test('create invoice with billing address', async ({ request }) => {
    const invoiceResponse = await request.post('http://localhost:8091/invoices', {
      headers: { Authorization: `Bearer ${token}` },
      data: {
        cart_id: cartId,
        billing_address: '123 Billing St',
        billing_city: 'Bill City',
        billing_state: 'BS',
        billing_country: 'US',
        billing_postcode: '99999',
        payment_method: 'Cash on Delivery',
        payment_account_name: '',
        payment_account_number: '',
      },
    });
    expect(invoiceResponse.status()).toBe(200);
    const invoiceBody = await invoiceResponse.json();
    expect(invoiceBody).toHaveProperty('id');
    expect(invoiceBody).toHaveProperty('invoice_number');

    const listResponse = await request.get('http://localhost:8091/invoices', {
      headers: { Authorization: `Bearer ${token}` },
    });
    expect(listResponse.status()).toBe(200);
  });

  test('422 on invalid invoice', async ({ request }) => {
    const invalidResponse = await request.post('http://localhost:8091/invoices', {
      headers: { Authorization: `Bearer ${token}` },
      data: {
        cart_id: 'nonexistent',
      },
    });
    expect(invalidResponse.status()).toBe(422);
    const errorBody = await invalidResponse.json();
    expect(errorBody).toHaveProperty('message');
  });
});
