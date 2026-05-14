// model.ts — interfaces, type aliases, and class with methods

export interface Shape {
  name: string;
  area(): number;
}

export type Point = { x: number; y: number };

export type Vector = Point & { z: number };

export class Circle implements Shape {
  constructor(public name: string, private radius: number) {}

  area(): number {
    return Math.PI * this.radius * this.radius;
  }

  circumference(): number {
    return 2 * Math.PI * this.radius;
  }
}

export class Rectangle implements Shape {
  constructor(
    public name: string,
    private width: number,
    private height: number
  ) {}

  area(): number {
    return this.width * this.height;
  }
}
