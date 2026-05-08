mod helpers;

// 通过 wildcard import 引入跨文件函数
use helpers::*;

pub fn main_fn() -> i32 {
    // 跨文件 free function 调用 — 通过 wildcard import 引入
    compute_value(5)
}

pub fn create_calculator() -> Calculator {
    // 跨文件 associated function 调用
    Calculator::new()
}
