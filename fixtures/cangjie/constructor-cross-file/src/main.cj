// constructor-cross-file: 使用跨文件 import 的 constructor

import constructor_cross_file.types.DataType
import constructor_cross_file.types.Color

main() {
    let dt = DataType(1, "test")
    let c = Color(1.0, 0.5, 0.0)
}
