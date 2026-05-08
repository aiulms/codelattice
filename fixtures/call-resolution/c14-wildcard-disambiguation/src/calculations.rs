// calculations 模块：被 lib.rs 通过 use calculations::* 导入

/// helper_func 在 calculations 中的实现（与 utils 中的同名函数不同）
pub fn helper_func(x: i32) -> i32 {
    x + 1
}

/// process：只在 calculations 中定义，不与其他模块冲突
pub fn process(x: i32) -> i32 {
    x * 2
}
