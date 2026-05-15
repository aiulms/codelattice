export function buildOrder(itemId: string, qty: number): { id: string; itemId: string; qty: number } {
  return { id: `ord-${Date.now()}`, itemId, qty };
}
