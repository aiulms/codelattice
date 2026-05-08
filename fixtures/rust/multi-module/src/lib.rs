// multi-module lib: 多模块项目，测试跨文件 crate:: 路径调用
// 覆盖：跨文件 DEFINES、跨模块 crate:: 路径 CALLS、多 source-file

pub mod utils;

/// 使用 crate:: 路径调用 utils 模块函数
pub fn process_data(input: i32) -> i32 {
    let doubled = crate::utils::double_value(input);
    crate::utils::format_result(doubled)
}

/// 直接调用同模块函数
pub fn run_pipeline(x: i32) -> i32 {
    process_data(x)
}
