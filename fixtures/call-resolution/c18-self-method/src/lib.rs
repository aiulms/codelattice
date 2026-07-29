// 验证 self 方法解析（B）：self.helper() 在 impl Foo 块内应解析到 Foo::helper。
// 关键：两个类型 Foo/Bar 各有同名 helper()，crate 内 method "helper" 不唯一，
// 但 self.helper() 在 impl Foo 内时，按 enclosing impl_target=Foo 过滤唯一匹配。

pub struct Foo {
    value: i32,
}

impl Foo {
    pub fn new() -> Self {
        Foo { value: 0 }
    }

    pub fn helper(&self) -> i32 {
        self.value
    }

    // self 调用同 impl 块的另一个 method
    pub fn do_work(&self) -> i32 {
        let v = self.helper();
        v + 1
    }
}

pub struct Bar {
    value: i32,
}

impl Bar {
    pub fn new() -> Self {
        Bar { value: 0 }
    }

    // 同名 method，不同 impl_target
    pub fn helper(&self) -> i32 {
        self.value * 2
    }

    pub fn do_work(&self) -> i32 {
        let v = self.helper();
        v + 10
    }
}
