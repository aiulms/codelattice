// c14-wildcard-disambiguation: 测试 wildcard import 源模块感知消歧
// 两个模块各有一个同名函数 helper_func，caller 通过 use calculations::* 导入 calculations 模块
// wildcard-aware disambiguation 应优先匹配 calculations::helper_func

pub mod calculations;
pub mod utils;

// wildcard import from calculations module — 消歧关键
use calculations::*;

/// 主入口：调用 helper_func（预期解析到 calculations::helper_func）
pub fn run(x: i32) -> i32 {
    helper_func(x)
}

/// 调用另一个只在 calculations 定义的函数（唯一 match，直通）
pub fn run_process(x: i32) -> i32 {
    process(x)
}
