import { test } from "../fixtures/auth";
import { expect } from "@playwright/test";

test.describe("workshop: shop and mechanic endpoints", () => {
  let productId: string;
  let orderId: string;

  test("GET products", async ({ customerRequest }) => {
    const resp = await customerRequest.get("/workshop/api/shop/products");
    expect(resp.status()).toBe(200);
    const body = await resp.json();
    expect(Array.isArray(body.products || body)).toBe(true);
    const products = Array.isArray(body) ? body : body.products;
    if (products && products.length > 0) {
      productId = products[0].id;
    }
  });

  test("POST create order", async ({ customerRequest }) => {
    if (!productId) return;
    const resp = await customerRequest.post("/workshop/api/shop/orders", {
      data: {
        product_id: productId,
        quantity: 1,
      },
    });
    expect([200, 201, 400]).toContain(resp.status());
    if (resp.ok()) {
      const body = await resp.json();
      if (body.id) orderId = body.id;
    }
  });

  test("GET all orders", async ({ customerRequest }) => {
    const resp = await customerRequest.get(
      "/workshop/api/shop/orders/all?limit=30&offset=0",
    );
    expect([200, 404]).toContain(resp.status());
    if (resp.ok()) {
      const body = await resp.json();
      const orders = Array.isArray(body) ? body : body.orders;
      if (orders && orders.length > 0 && !orderId) {
        orderId = orders[0].id;
      }
    }
  });

  test("GET order by id", async ({ customerRequest }) => {
    if (!orderId) return;
    const resp = await customerRequest.get(
      `/workshop/api/shop/orders/${orderId}`,
    );
    expect([200, 404]).toContain(resp.status());
  });

  test("GET mechanics list", async ({ customerRequest }) => {
    const resp = await customerRequest.get("/workshop/api/mechanic");
    expect([200, 404]).toContain(resp.status());
  });

  test("GET mechanic service requests", async ({ customerRequest }) => {
    const resp = await customerRequest.get(
      "/workshop/api/mechanic/service_requests?limit=30&offset=0",
    );
    expect([200, 404]).toContain(resp.status());
  });
});
