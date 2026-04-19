import { test, expect } from '@playwright/test';

test.describe('Scenario 3: Cart lifecycle', () => {
  let token: string;
  let cartId: string;
  let productId: string;

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
    productId = productsBody.data[0].id;
  });

  test('create cart, add items, and delete', async ({ request }) => {
    const cartResponse = await request.post('http://localhost:8091/carts', {
      headers: { Authorization: `Bearer ${token}` },
    });
    expect(cartResponse.status()).toBe(201);
    const cartBody = await cartResponse.json();
    cartId = cartBody.id;

    const addItemResponse = await request.post(`http://localhost:8091/carts/${cartId}`, {
      headers: { Authorization: `Bearer ${token}` },
      data: {
        product_id: productId,
        quantity: 2,
      },
    });
    expect(addItemResponse.status()).toBe(200);

    const getCartResponse = await request.get(`http://localhost:8091/carts/${cartId}`, {
      headers: { Authorization: `Bearer ${token}` },
    });
    expect(getCartResponse.status()).toBe(200);
    const getCartBody = await getCartResponse.json();
    expect(getCartBody).toHaveProperty('cart_items');

    const deleteResponse = await request.delete(`http://localhost:8091/carts/${cartId}`, {
      headers: { Authorization: `Bearer ${token}` },
    });
    expect(deleteResponse.status()).toBe(204);
  });
});
