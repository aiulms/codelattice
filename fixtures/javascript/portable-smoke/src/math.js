export function add(a, b) {
    return a + b;
}

export function multiply(a, b) {
    return a * b;
}

export class MathHelper {
    constructor(initialValue = 0) {
        this.value = initialValue;
    }

    add(amount) {
        this.value += amount;
        return this;
    }

    subtract(amount) {
        this.value -= amount;
        return this;
    }

    getValue() {
        return this.value;
    }
}

export const PI = 3.14159;
export const E = 2.71828;
