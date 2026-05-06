package demo.math

public func add(a: Int64, b: Int64): Int64 {
    a + b
}

public func sub(a: Int64, b: Int64): Int64 {
    a - b
}

public func mul(a: Int64, b: Int64): Int64 {
    a * b
}

public class Calculator {
    public init() {}
    public func compute(x: Int64): Int64 {
        x * 2
    }
}
