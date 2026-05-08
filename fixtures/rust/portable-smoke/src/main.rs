// portable-smoke main: 跨 target 调用 lib 中的类型和函数
// 覆盖：CALLS（跨 target 调用）、ACCESSES（类型注解）、DEFINES（main 函数）

use portable_smoke::{add, multiply, Calculator};

/// 主函数：调用 lib 中的函数和类型
fn main() {
    // 自由函数调用 → CALLS edge
    let sum: i32 = add(3, 4);
    let product: i32 = multiply(sum, 2);

    // 类型注解引用 Calculator → ACCESSES edge
    let mut calc: Calculator = Calculator::new(product);
    calc.add(1);
    let result: i32 = calc.get();

    println!("sum={}, product={}, result={}", sum, product, result);
}
