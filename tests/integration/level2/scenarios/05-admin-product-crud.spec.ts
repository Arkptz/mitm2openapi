import { test, expect } from '@playwright/test';

test.describe('Scenario 5: Admin product CRUD', () => {
  let adminToken: string;
  let createdProductId: string;

  test.beforeAll(async ({ request }) => {
    const loginResponse = await request.post('http://localhost:8091/users/login', {
      data: {
        email: 'admin@practicesoftwaretesting.com',
        password: 'welcome01',
      },
    });
    const loginBody = await loginResponse.json();
    adminToken = loginBody.access_token;
  });

  test('POST create product', async ({ request }) => {
    const brandsResponse = await request.get('http://localhost:8091/brands');
    const brands = await brandsResponse.json();
    const brandId = brands[0].id;

    const categoriesResponse = await request.get('http://localhost:8091/categories/tree');
    const categories = await categoriesResponse.json();
    const categoryId = categories[0].id;

    const createResponse = await request.post('http://localhost:8091/products', {
      headers: { Authorization: `Bearer ${adminToken}` },
      data: {
        name: 'Integration Test Product',
        description: 'Created by integration test',
        price: 29.99,
        category_id: categoryId,
        brand_id: brandId,
        product_image_id: 1,
        is_location_offer: false,
        is_rental: false,
      },
    });
    expect(createResponse.status()).toBe(201);
    const created = await createResponse.json();
    expect(created).toHaveProperty('id');
    createdProductId = created.id;
  });

  test('PUT update product', async ({ request }) => {
    const updateResponse = await request.put(`http://localhost:8091/products/${createdProductId}`, {
      headers: { Authorization: `Bearer ${adminToken}` },
      data: {
        name: 'Updated Integration Test Product',
        description: 'Updated by integration test',
        price: 39.99,
      },
    });
    expect(updateResponse.status()).toBe(200);
    const updated = await updateResponse.json();
    expect(updated.name).toBe('Updated Integration Test Product');
  });

  test('GET product by ID', async ({ request }) => {
    const getResponse = await request.get(`http://localhost:8091/products/${createdProductId}`);
    expect(getResponse.status()).toBe(200);
    const product = await getResponse.json();
    expect(product.id).toBe(createdProductId);
  });

  test('DELETE product', async ({ request }) => {
    const deleteResponse = await request.delete(`http://localhost:8091/products/${createdProductId}`, {
      headers: { Authorization: `Bearer ${adminToken}` },
    });
    expect(deleteResponse.status()).toBe(204);
  });

  test('GET deleted product returns 404', async ({ request }) => {
    const getResponse = await request.get(`http://localhost:8091/products/${createdProductId}`);
    expect(getResponse.status()).toBe(404);
  });
});
