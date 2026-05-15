export interface Order {
  id: string;
  itemId: string;
  qty: number;
  total: number;
}

export function createOrder(itemId: string, qty: number): Order {
  return { id: `ord-${Date.now()}`, itemId, qty, total: qty * 10 };
}
