// portable-smoke lib/math: 定义 function + class with methods
package lib

// 基础函数 — 测试 Function symbol extraction + 跨文件 function call
public func add(a: Int64, b: Int64): Int64 {
    a + b
}

public func multiply(a: Int64, b: Int64): Int64 {
    a * b
}

// Class with method — 测试 method definition
public class Calculator {
    public var base: Int64
    public init(base: Int64) {
        this.base = base
    }
    public func compute(x: Int64): Int64 {
        this.base + x
    }
    public func scale(factor: Int64): Int64 {
        this.base * factor
    }
}
