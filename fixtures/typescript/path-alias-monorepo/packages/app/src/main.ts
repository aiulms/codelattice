import { logInfo } from "@core/logger";
import { createOrder } from "@shared";
import { formatCurrency } from "@shared/format";
import { Order } from "@models/order";
import { Button } from "./ui/Button";
import { buildOrder } from "./features/orders";
import type { ReactNode } from "react";
import { Missing } from "@shared/missing";

export function main(): void {
  const order = createOrder("item-1", 2);
  logInfo(`Order created: ${order.id}`);
  const total = formatCurrency(order.total);
  logInfo(`Total: ${total}`);

  const built = buildOrder("item-2", 3);
  logInfo(`Built order: ${built.id}`);

  const button: ReactNode = Button("Click me");
  console.log(button);
}
