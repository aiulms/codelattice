import { add, multiply, MathHelper } from '../src/math.js';

function test(name, fn) {
    try {
        fn();
        console.log(`✓ ${name}`);
    } catch (e) {
        console.error(`✗ ${name}: ${e.message}`);
        process.exitCode = 1;
    }
}

function assertEqual(actual, expected) {
    if (actual !== expected) {
        throw new Error(`Expected ${expected}, got ${actual}`);
    }
}

test('add function', () => {
    assertEqual(add(1, 2), 3);
    assertEqual(add(-1, 1), 0);
});

test('multiply function', () => {
    assertEqual(multiply(3, 4), 12);
    assertEqual(multiply(0, 100), 0);
});

test('MathHelper class', () => {
    const helper = new MathHelper(10);
    assertEqual(helper.getValue(), 10);
    helper.add(5);
    assertEqual(helper.getValue(), 15);
    helper.subtract(3);
    assertEqual(helper.getValue(), 12);
});
