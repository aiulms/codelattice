// Legacy module — NOT imported anywhere
export function oldHelper(x: number): number {
  return x * 2;
}

export class UnusedClass {
  private value: number;

  constructor(value: number) {
    this.value = value;
  }

  getValue(): number {
    return this.value;
  }
}

function internalLegacyHelper(): string {
  return "legacy";
}
