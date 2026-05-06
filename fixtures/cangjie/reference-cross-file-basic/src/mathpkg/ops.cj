package mathpkg

public class Point {
    public var x: Int64
    public var y: Int64
    public init(x: Int64, y: Int64) {
        this.x = x
        this.y = y
    }
}

public func add(a: Int64, b: Int64): Int64 {
    a + b
}
