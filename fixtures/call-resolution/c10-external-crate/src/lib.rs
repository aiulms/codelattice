// external crate call fixture — 纯 std 调用（无需 extra deps）
// 验证：std:: path 被检测为 external-crate 并标记 knownCrate
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
