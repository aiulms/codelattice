// 内部模块：提供类型关联函数和自由函数供 crate:: 路径调用

pub struct MyType {
    pub name: String,
}

impl MyType {
    pub fn build(name: &str) -> Self {
        Self {
            name: name.to_string(),
        }
    }
}

pub fn helper(x: u32) -> u32 {
    x * 2
}
