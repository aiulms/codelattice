// c12-let-constructor-method
// 验证 Phase 2d: let-binding 链已知构造函数推断 receiver type
// 无需显式类型注解，通过 RHS 中已知 stdlib 构造函数推断变量类型
// compile-valid

use std::collections::HashMap;

// Vec::new() → push, len, is_empty
pub fn vec_constructor_chain() {
    let v = Vec::new();
    v.push(1);
    let _len = v.len();
    let _empty = v.is_empty();
}

// Vec::with_capacity → push
pub fn vec_with_capacity_chain() {
    let v = Vec::with_capacity(10);
    v.push(2);
}

// String::new() → push_str, len
pub fn string_constructor_chain() {
    let s = String::new();
    s.push_str("hello");
    let _len = s.len();
}

// String::from → len
pub fn string_from_chain() {
    let s = String::from("hello");
    s.len();
}

// HashMap::new() → insert, len
pub fn hashmap_constructor_chain() {
    let mut m = HashMap::new();
    m.insert("key", 42);
    let _len = m.len();
}
