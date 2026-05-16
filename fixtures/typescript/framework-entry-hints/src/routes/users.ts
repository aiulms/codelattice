// Express-like route handlers — framework entry candidates
import { Router } from "./server";

const router = new Router();

// Route handler — GET /users/:id
router.get("/users/:id", getUser);
// Route handler — POST /orders
router.post("/orders", createOrder);

export function getUser(id: string) {
  return { id, name: "test" };
}

export function createOrder(data: { item: string }) {
  return { id: "order-1", ...data };
}
