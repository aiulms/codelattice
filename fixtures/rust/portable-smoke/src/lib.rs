// portable-smoke lib: 提供可被 main 调用的类型和函数
// 覆盖：Struct + impl block（DESIGNATION）、function（DEFINES + CALLS）、type annotation（ACCESSES）

/// 计算器结构体，有 impl block 产生 DESIGNATION edge
pub struct Calculator {
    value: i32,
}

impl Calculator {
    pub fn new(initial: i32) -> Calculator {
        Calculator { value: initial }
    }

    pub fn add(&mut self, x: i32) {
        self.value += x;
    }

    pub fn get(&self) -> i32 {
        self.value
    }
}

/// 自由函数：创建带初始值的 Calculator
pub fn create_calculator(initial: i32) -> Calculator {
    Calculator::new(initial)
}

/// 加法函数：供同模块调用
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

/// 乘法函数
pub fn multiply(a: i32, b: i32) -> i32 {
    a * b
}
