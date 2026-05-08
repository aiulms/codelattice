// portable-smoke lib/types: 定义多种 symbol kinds
package lib

// Enum — 测试 Enum symbol extraction
public enum Status {
    | Active
    | Inactive
    | Pending
}

// Interface — 测试 Interface symbol extraction
public interface Measurable {
    func measure(): Float64
}

// TypeAlias — 测试 TypeAlias symbol extraction
public type UserId = Int64

// Class with init — 测试 Class + Init symbol extraction
public class Point {
    public var x: Float64
    public var y: Float64
    public init(x: Float64, y: Float64) {
        this.x = x
        this.y = y
    }
    public func distance(): Float64 {
        this.x * this.y
    }
}

// Struct with init — 测试 Struct + Init symbol extraction
public struct Size {
    public var width: Int64
    public var height: Int64
    public init(width: Int64, height: Int64) {
        this.width = width
        this.height = height
    }
}

// Class with multiple inits — 测试 Init#arity suffix
public class MultiInitClass {
    public var x: Int64
    public var y: Int64
    public init(x: Int64) {
        this.x = x
        this.y = 0
    }
    public init(x: Int64, y: Int64) {
        this.x = x
        this.y = y
    }
}
