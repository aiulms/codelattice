import { greet } from "./utils";

interface User {
  name: string;
  age: number;
}

export function main(): void {
  const user: User = { name: "Alice", age: 30 };
  console.log(greet(user.name));
}
