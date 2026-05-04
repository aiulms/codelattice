// external crate call fixture — 纯 std 调用（无需 extra deps）
// 验证：
//   - std:: path 被检测为 external-crate 并标记 knownCrate → resolved by path
//   - 通过 use import 的 Type::method() 被检测为 associated-function → resolved via import binding
//   - prelude types (Vec) 通过 prelude 映射解析
use std::collections::HashMap;
use std::path::PathBuf;

pub fn external_vec() {
    let _v: std::vec::Vec<i32> = std::vec::Vec::new();
}

pub fn external_hashmap() {
    let _map = std::collections::HashMap::<&str, i32>::new();
}

pub fn external_path() {
    let _p = std::path::PathBuf::new();
}

// import-aware resolution: HashMap imported via use → HashMap::new() resolved
pub fn hashmap_via_import() {
    let _map = HashMap::new();
}

// prelude type resolution: Vec is implicitly available (no use needed)
pub fn vec_via_prelude() {
    let _v: Vec<i32> = Vec::new();
}

// stdlib trait method resolution: to_string() → std::string::ToString::to_string
pub fn method_to_string() {
    let s = 42.to_string();
}

// stdlib trait method resolution: clone() → std::clone::Clone::clone
pub fn method_clone() {
    let x = String::from("hello");
    let y = x.clone();
}
