// constructor-basic: 测试 class init / struct init 的 symbol extraction 和 graph emission

// class with init
open class AppConfig {
    var name: String
    var value: Int64
    public init(name: String, value: Int64) {
        this.name = name
        this.value = value
    }
}

// struct with init
struct Point {
    var x: Float64
    var y: Float64
    public init(x: Float64, y: Float64) {
        this.x = x
        this.y = y
    }
}

// class without init (default constructor)
open class SimpleClass {
    var data: String
}

// class with multiple inits
open class MultiInit {
    var x: Int64
    var y: Int64
    public init(x: Int64) {
        this.x = x
        this.y = 0
    }
    public init(x: Int64, y: Int64) {
        this.x = x
        this.y = y
    }
}

main() {
    let config = AppConfig("test", 42)
    let pt = Point(1.0, 2.0)
}
