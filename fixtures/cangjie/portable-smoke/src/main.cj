// portable-smoke entry point: 综合测试所有 extraction path
// 跨文件 import — 测试 Imports edges
import lib.types.{Point, Size, Status, MultiInitClass}
import lib.math.{add, multiply, Calculator}

// 本地 Class — 测试 same-file symbol definition + Uses edges
open class App {
    var name: String
    public init(name: String) {
        this.name = name
    }
}

// 本地 Function — 测试 same-file function call
func helper(x: Int64): Int64 {
    x + 1
}

main(): Int64 {
    // 1. Constructor call 跨文件 → Uses edge to Init symbol
    let pt = Point(1.0, 2.0)

    // 2. Struct constructor call → Uses edge to Size Init symbol
    let sz = Size(100, 200)

    // 3. 跨文件 Function call → Uses edge to add
    let sum = add(1, 2)

    // 4. 跨文件 Function call → Uses edge to multiply
    let prod = multiply(3, 4)

    // 5. Type annotation 跨文件 → Uses edge to Point
    let origin: Point = Point(0.0, 0.0)

    // 6. Type annotation 跨文件 → Uses edge to Status
    let status: Status = Status.Active

    // 7. Field read → Accesses edge
    let x = pt.x

    // 8. Constructor call with multiple inits → Uses edge to MultiInitClass Init
    let mi1 = MultiInitClass(1)
    let mi2 = MultiInitClass(2, 3)

    // 9. Same-file function call → Uses edge to helper
    let h = helper(5)

    // 10. Same-file constructor call → Uses edge to App Init
    let app = App("test")

    // 11. Method call → Uses edge to Calculator.compute
    let calc = Calculator(10)
    let result = calc.compute(5)

    // 12. Field write → Modifies edge
    pt.x = 5.0

    0
}
