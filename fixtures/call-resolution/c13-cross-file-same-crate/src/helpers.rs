// 定义跨文件被调用函数
pub fn compute_value(x: i32) -> i32 {
    x * 2
}

pub struct Calculator;

impl Calculator {
    pub fn new() -> Calculator {
        Calculator
    }
}
