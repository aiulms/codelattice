// references-basic: fixture for same-file reference extraction tests
// Tests USES (type annotation), ACCESSES (field read), and MODIFIES (write/mutation) edges

class Point {
    var x: Float64
    var y: Float64

    public init(x: Float64, y: Float64) {
        this.x = x
        this.y = y
    }

    public func sum(): Float64 {
        this.x + this.y
    }
}

struct Size {
    var width: Int64
    var height: Int64
}

enum Color {
    | Red
    | Green
    | Blue
}

interface Drawable {
    func draw(): Unit }

// Type annotation reference: p: Point → USES edge
// Field read: p.x, p.y → ACCESSES edges
func distance(p: Point): Float64 {
    p.x * p.y
}

// Field write: p.x = 10.0 → MODIFIES edge
func movePoint(p: Point): Unit {
    p.x = 10.0
    var local: Int64 = 0
    local = 42
}

// Builtin types (Int64, Float64, Unit) → no USES edges
func identity(x: Int64): Int64 {
    x
}

// Generic type argument: Array<Point> → Point → USES, Array → no USES (builtin)
func process(points: Array<Point>): Unit {
}

// Variable declaration type annotation → USES edge
func createPoint(): Point {
    var p: Point = Point(0.0, 0.0)
    p
}

main(): Int64 {
    let origin = Point(0.0, 0.0)
    let dist = distance(origin)
    0
}
