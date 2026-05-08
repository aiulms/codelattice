// imports-cross-crate: 测试外部 stdlib 类型引用的 graph 输出
// 覆盖：外部 symbol node（isExternal=true）、ACCESSES edge（同 crate 类型）、
//       external crate 函数调用（Vec::new, HashMap::new, String::from）

use std::collections::HashMap;

/// 本地数据结构：聚合 stdlib 类型
pub struct DataStore {
    items: Vec<String>,
    index: HashMap<String, i32>,
}

impl DataStore {
    pub fn new() -> DataStore {
        DataStore {
            items: Vec::new(),
            index: HashMap::new(),
        }
    }

    pub fn insert(&mut self, key: String, value: i32) {
        self.items.push(key.clone());
        self.index.insert(key, value);
    }

    pub fn get(&self, key: &str) -> Option<&i32> {
        let s = String::from(key);
        self.index.get(&s)
    }
}

/// 创建默认 DataStore 的辅助函数
pub fn create_store() -> DataStore {
    DataStore::new()
}
