// Same-file function call reference fixture
// Tests USES edges from function calls and constructor calls

func add(a: Int64, b: Int64): Int64 {
    a + b
}

func compute(x: Int64, y: Int64): Int64 {
    // Function call: add(x, y) → USES edge to add
    add(x, y)
}

class Point {
    var x: Int64
    var y: Int64
    public init(x: Int64, y: Int64) {
        this.x = x
        this.y = y
    }
}

func createOrigin(): Point {
    // Constructor call: Point(0, 0) → USES edge to Point
    Point(0, 0)
}

class Calculator {
    var value: Int64
    public init(v: Int64) {
        this.value = v
    }
    public func getValue(): Int64 {
        this.value
    }
}

func testMethodCall(): Int64 {
    let calc = Calculator(42)
    // Method call: calc.getValue() → should NOT produce USES edge
    calc.getValue()
}

main(): Int64 {
    let result = compute(1, 2)
    0
}
