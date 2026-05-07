// constructor-cross-file: 定义 types

open class DataType {
    var id: Int64
    var label: String
    public init(id: Int64, label: String) {
        this.id = id
        this.label = label
    }
}

struct Color {
    var r: Float64
    var g: Float64
    var b: Float64
    public init(r: Float64, g: Float64, b: Float64) {
        this.r = r
        this.g = g
        this.b = b
    }
}
