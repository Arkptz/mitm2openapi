import { test, expect } from '@playwright/test';

test.describe('Scenario 2: Register, login, profile', () => {
  const timestamp = Date.now();
  const testEmail = `test${timestamp}@example.com`;
  const testPassword = 'TestPassword123!';

  test('register new user via API', async ({ request }) => {
    const response = await request.post('http://localhost:8091/users/register', {
      data: {
        first_name: 'Test',
        last_name: 'User',
        address: '123 Test St',
        city: 'Testville',
        state: 'TS',
        country: 'US',
        postcode: '12345',
        phone: '555-0100',
        dob: '1990-01-01',
        email: testEmail,
        password: testPassword,
      },
    });
    expect(response.status()).toBe(201);
    const body = await response.json();
    expect(body).toHaveProperty('id');
    expect(body).toHaveProperty('email', testEmail);
  });

  test('login via API and get JWT', async ({ request }) => {
    const loginResponse = await request.post('http://localhost:8091/users/login', {
      data: {
        email: 'customer@practicesoftwaretesting.com',
        password: 'welcome01',
      },
    });
    expect(loginResponse.status()).toBe(200);
    const loginBody = await loginResponse.json();
    expect(loginBody).toHaveProperty('access_token');
    expect(typeof loginBody.access_token).toBe('string');

    const token = loginBody.access_token;
    const profileResponse = await request.get('http://localhost:8091/users/me', {
      headers: {
        Authorization: `Bearer ${token}`,
      },
    });
    expect(profileResponse.status()).toBe(200);
    const profile = await profileResponse.json();
    expect(profile).toHaveProperty('email');
    expect(profile).toHaveProperty('address');
  });
});
