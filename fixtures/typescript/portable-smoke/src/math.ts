// math.ts — arithmetic functions for cross-file import/call testing

export function add(a: number, b: number): number {
  return a + b;
}

export function subtract(a: number, b: number): number {
  return a - b;
}

export const PI = 3.14159;

export class Calculator {
  private result: number;

  constructor(initial: number) {
    this.result = initial;
  }

  add(value: number): number {
    this.result += value;
    return this.result;
  }

  getResult(): number {
    return this.result;
  }
}
