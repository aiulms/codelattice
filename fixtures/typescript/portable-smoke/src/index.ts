import { greet } from "./utils";
import { add, Calculator } from "./math";
import type { Shape, Point } from "./model";
import { Circle } from "./model";

interface User {
  name: string;
  age: number;
}

export function main(): void {
  const user: User = { name: "Alice", age: 30 };
  console.log(greet(user.name));

  const sum = add(1, 2);
  console.log(sum);

  const calc = new Calculator(0);
  calc.add(5);
  calc.add(3);
  console.log(calc.getResult());

  const center: Point = { x: 0, y: 0 };
  const circle: Shape = new Circle("unit", 1);
  console.log(circle.area());
}
