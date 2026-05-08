// utils 模块：被 lib.rs 通过 crate::utils:: 路径调用

/// 将输入值翻倍
pub fn double_value(x: i32) -> i32 {
    x * 2
}

/// 格式化结果（简单包装）
pub fn format_result(value: i32) -> i32 {
    value + 1
}
