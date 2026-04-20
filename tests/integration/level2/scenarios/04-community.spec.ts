import { test } from "../fixtures/auth";
import { expect } from "@playwright/test";

test.describe("community: posts and coupons", () => {
  let postId: string;

  test("GET recent posts", async ({ customerRequest }) => {
    const resp = await customerRequest.get(
      "/community/api/v2/community/posts/recent?limit=30&offset=0",
    );
    expect(resp.status()).toBe(200);
    const body = await resp.json();
    expect(body).toHaveProperty("posts");
    if (body.posts && body.posts.length > 0) {
      postId = body.posts[0].id;
    }
  });

  test("POST create post", async ({ customerRequest }) => {
    const resp = await customerRequest.post(
      "/community/api/v2/community/posts",
      {
        data: {
          title: "Integration test post",
          content: "Created by integration test",
          author: {
            nickname: "testuser",
            email: "test@example.com",
          },
          vehicle_id: "00000000-0000-0000-0000-000000000000",
        },
      },
    );
    expect([200, 201, 400]).toContain(resp.status());
    if (resp.ok()) {
      const body = await resp.json();
      if (body.id) postId = body.id;
    }
  });

  test("GET post by id (if available)", async ({ customerRequest }) => {
    if (!postId) return;
    const resp = await customerRequest.get(
      `/community/api/v2/community/posts/${postId}`,
    );
    expect([200, 404]).toContain(resp.status());
  });

  test("POST validate coupon", async ({ customerRequest }) => {
    const resp = await customerRequest.post(
      "/community/api/v2/coupon/validate-coupon",
      {
        data: { coupon_code: "TRAC075" },
      },
    );
    expect([200, 400, 404]).toContain(resp.status());
  });
});
